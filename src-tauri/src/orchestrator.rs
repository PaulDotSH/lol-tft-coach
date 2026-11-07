//! Phase loop: polls the client / game at the phase's rate, keeps remote data
//! warm in the background, runs the coach and pushes `state` to the UI on change.

use crate::{debug, settings};
use crate::state::{Shared, StatsKey, ensure};
use clients::live::{Live, LiveState};
use model::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// OCR cards count as "on screen" this long after they were last read.
const OFFER_TTL: Duration = Duration::from_secs(3);
/// A pending offer (cards hidden behind the shop, respawn, …) is dropped after this many game seconds unseen.
const OFFER_HOLD_S: f32 = 120.0;
/// Offer OCR cadence while a Mayhem game runs.
const OFFER_SCAN: Duration = Duration::from_millis(500);
/// Minimap read cadence while a Rift match runs, and the parked thread's check rate.
const MINIMAP_SCAN: Duration = Duration::from_millis(500);
const MINIMAP_IDLE: Duration = Duration::from_secs(2);
/// Gank warnings older than this are dropped (the game lost focus, the thread stalled).
const MINIMAP_TTL: Duration = Duration::from_millis(1500);
/// The minimap thread counts the game clock on from the last tick for at most this long.
const CLOCK_ON: f32 = 5.0;
/// Champion portraits (Data Dragon) are re-fetched after this long.
const PORTRAIT_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

struct PendingOffer {
    ids: Vec<AugmentId>,
    /// Game time the cards were last read.
    last_seen: f32,
    /// My scoreboard augment count when the offer appeared; a higher count means it was picked.
    mine: usize,
}
const RECORD_EVERY: Duration = Duration::from_secs(30);

/// Per-game memory across ticks.
#[derive(Default)]
struct Memo {
    mode: Option<Mode>,
    my_lane: Option<Lane>,
    queue_checked: bool,
    /// The running game is not League (TFT): no live coaching.
    not_lol: bool,
    recommended: Vec<ItemId>,
    /// My completed items in the order they showed up (Coachless asks for the next item after them).
    built: Vec<ItemId>,
    /// Last tick's Buy next, kept while still about as good.
    last_buy: Option<ItemId>,
    last_champ: Option<ChampKey>,
    last_lane: Option<Lane>,
    recap_done: bool,
    recap: Option<Recap>,
    last_record: Option<Instant>,
    gold: coach::GoldHistory,
    tracker: coach::Tracker,
    log: coach::GameLog,
    goal_progress: Vec<GoalProgress>,
    /// Mayhem: augments I took, detected as offers close.
    augs: coach::AugmentTracker,
    tab: crate::tab::TabMemo,
    offer: Option<PendingOffer>,
    /// Every augment shown on a card this game (narrows lookalike scoreboard icons).
    offer_history: Vec<AugmentId>,
}

impl Memo {
    fn new_game(&mut self) {
        let recap = self.recap.take();
        *self = Memo { recap, ..Memo::default() };
    }
}

pub async fn run(app: AppHandle, sh: Arc<Shared>) {
    let live = Live::new();
    let mut lcu: Option<clients::lcu::Lcu> = None;
    let mut last_discover: Option<Instant> = None;
    let mut memo = Memo { recap: load_last_recap(), ..Memo::default() };
    let mut last_json = String::new();
    let macro_sync = Arc::new(crate::macro_sync::MacroSync::default());
    let mut self_coach = crate::selfcoach::SelfCoach::default();
    let rank_log = Arc::new(crate::ranked::RankLog::default());
    let (mut last_flow, mut last_phase, mut probed): (Option<String>, Option<Phase>, bool) = (None, None, false);
    let mut lcu_misses = 0u32;

    loop {
        if sh.static_data().is_none() {
            match data::ddragon::load(&sh.http).await {
                Ok(sd) => *sh.sd.write().unwrap() = Some(Arc::new(sd)),
                Err(e) => {
                    tracing::error!("static data: {e:#}");
                    push(&app, &sh, &mut last_json, |st| st.sources.message = Some(format!("Data Dragon unavailable: {e}")));
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
            }
        }
        let sd = sh.static_data().expect("loaded above");

        if lcu.is_none() && last_discover.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
            last_discover = Some(Instant::now());
            lcu = tokio::task::spawn_blocking(clients::lcu::discover).await.ok().flatten();
            if let Some(c) = &lcu {
                tracing::info!("League client found at {}", c.base);
                *sh.league_install.lock().unwrap() = c.install_dir.clone();
                let (c2, sh2) = (c.clone(), sh.clone());
                tokio::spawn(async move {
                    match c2.augment_names().await {
                        Ok(n) => *sh2.local_augment_names.lock().unwrap() = Some(Arc::new(n)),
                        Err(e) => tracing::warn!("client augment names: {e:#}"),
                    }
                });
                if debug::on() && !probed {
                    probed = true;
                    tokio::spawn(debug::probe_lcu(c.clone()));
                }
            }
        }
        let flow = match &lcu {
            Some(c) => match c.phase().await {
                Ok(p) => {
                    lcu_misses = 0;
                    Some(p)
                }
                // The client stalls for a few seconds while launching the game: keep the
                // last phase on timeouts, drop it only when it's gone or keeps failing.
                Err(e) if !e.chain().any(is_connect) && lcu_misses < 3 => {
                    lcu_misses += 1;
                    tracing::debug!("League client slow ({lcu_misses}): {e:#}");
                    last_flow.clone()
                }
                Err(e) => {
                    tracing::info!("League client lost: {e:#}");
                    lcu = None;
                    lcu_misses = 0;
                    None
                }
            },
            None => None,
        };
        if flow != last_flow {
            tracing::info!("gameflow {:?} → {:?}", last_flow, flow);
            last_flow = flow.clone();
        }

        let settings = sh.settings();
        let mut phase = match flow.as_deref() {
            None => Phase::NoClient,
            Some("ChampSelect") => Phase::ChampSelect,
            Some("InProgress" | "GameStart" | "Reconnect") => Phase::Loading,
            Some("PreEndOfGame" | "WaitingForStats" | "EndOfGame") => Phase::PostGame,
            Some(_) => Phase::Idle,
        };
        match phase {
            // TFT has no champ select to start a new game: reset once the client is out of it.
            Phase::Idle | Phase::ChampSelect => {
                memo.queue_checked = false;
                memo.not_lol = false;
            }
            Phase::PostGame if memo.not_lol => phase = Phase::Idle,
            _ => {}
        }
        let mut champ_select = None;
        let mut game = None;
        let mut game_src = SourceState::Off;

        match phase {
            Phase::ChampSelect => {
                if memo.recap_done || memo.last_champ.is_some() {
                    memo.new_game();
                }
                if let Some(c) = &lcu {
                    if debug::on()
                        && let Ok(Some(raw)) = c.get("/lol-champ-select/v1/session").await
                    {
                        debug::dump("lcu-champ-select", Duration::from_secs(2), &raw);
                    }
                    match c.champ_select().await {
                        Ok(Some(cs)) => {
                            memo.mode = Some(cs.mode);
                            memo.my_lane = cs.my_lane;
                            debug::dump("champ-select", Duration::from_secs(2), &cs);
                            let mut advice = champ_select_advice(&sh, &sd, &cs);
                            let games = macro_sync.games();
                            let me = cs.my_pick.or(cs.my_hover);
                            advice.memory = coach::matchup_memory(games.as_deref().map_or(&[], |g| g.as_slice()), &cs.enemies, cs.mode, me);
                            advice.matchup = crate::draft::matchup(&sh, &sd, &cs, &advice);
                            debug::dump("champ-select-advice", Duration::from_secs(5), &advice);
                            champ_select = Some(advice);
                        }
                        Ok(None) => tracing::debug!("champ select: no session parsed"),
                        Err(e) => tracing::warn!("champ select: {e:#}"),
                    }
                }
            }
            // Live API may answer without the client too (spectate, client closed mid-game).
            Phase::Loading | Phase::NoClient | Phase::Idle => {
                if matches!(phase, Phase::Loading) && !memo.queue_checked
                    && let Some(c) = &lcu {
                        if let Ok(Some((q, game_mode))) = c.queue().await {
                            memo.not_lol = game_mode == "TFT";
                            memo.mode = Some(Mode::from_queue_mode(q, &game_mode));
                            tracing::info!("queue {q} ({game_mode}) → {:?}", memo.mode);
                        }
                        memo.queue_checked = true;
                    }
                let (state, raw) = if memo.not_lol {
                    phase = Phase::Idle;
                    (LiveState::Unavailable, None)
                } else {
                    live.poll(&sd).await
                };
                if let Some(raw) = &raw {
                    let parsed = matches!(state, LiveState::InGame(_));
                    if !parsed {
                        tracing::warn!("live API answered but did not parse");
                    }
                    let kind = if parsed { "live-allgamedata" } else { "live-unparsed" };
                    debug::dump(kind, Duration::from_secs(if parsed { 20 } else { 5 }), raw);
                }
                match state {
                    LiveState::InGame(mut snap) => {
                        if memo.recap_done {
                            memo.new_game();
                        }
                        phase = Phase::InGame;
                        game_src = SourceState::Ok;
                        if let Some(m) = memo.mode {
                            snap.mode = m;
                        }
                        if settings.record_fixtures {
                            record(&mut memo, raw);
                        }
                        let games = macro_sync.games();
                        game = Some(game_advice(&sh, &sd, &snap, &settings, &mut memo, games.as_deref().map_or(&[], |g| g.as_slice())));
                    }
                    LiveState::Loading => game_src = SourceState::Degraded,
                    LiveState::Unavailable => {
                        if phase == Phase::Loading {
                            game_src = SourceState::Degraded;
                        }
                    }
                }
            }
            Phase::PostGame => {
                *sh.ocr_names.lock().unwrap() = None;
                if !memo.recap_done && !memo.not_lol && let Some(c) = &lcu {
                    if debug::on()
                        && let Ok(Some(raw)) = c.get("/lol-end-of-game/v1/eog-stats-block").await
                    {
                        debug::dump("lcu-eog", Duration::from_secs(5), &raw);
                    }
                    match c.eog_stats().await {
                        Ok(Some(mut eog)) => {
                            // The queue seen at game start beats the end-of-game block's mode.
                            if let Some(m) = memo.mode {
                                eog.mode = m;
                            }
                            let r = recap(&sh, &sd, &eog, &memo);
                            debug::dump("recap", Duration::ZERO, &r);
                            tracing::info!("recap saved: {:?} win={} {}s", eog.mode, eog.win, eog.duration_s);
                            memo.recap = Some(r);
                            memo.recap_done = true;
                        }
                        Ok(None) => tracing::debug!("end of game: stats not parsed yet"),
                        Err(e) => tracing::warn!("end of game: {e:#}"),
                    }
                }
            }
            Phase::InGame => unreachable!("set only after a live poll"),
        }
        if phase != Phase::InGame {
            *sh.ocr_names.lock().unwrap() = None;
            *sh.gank_ctx.lock().unwrap() = None;
        }
        if last_phase != Some(phase) {
            tracing::info!("phase {:?} → {phase:?}", last_phase);
            last_phase = Some(phase);
        }
        if let Some(g) = &game {
            debug::dump("game-advice", Duration::from_secs(15), g);
        }

        let sources = Sources {
            client: if lcu.is_some() { SourceState::Ok } else { SourceState::Off },
            game: game_src,
            lolalytics: flag(*sh.lola_ok.lock().unwrap()),
            ugg: flag(*sh.ugg_ok.lock().unwrap()),
            ocr: *sh.ocr_state.lock().unwrap(),
            minimap: *sh.minimap_state.lock().unwrap(),
            coachless: match (sh.coachless_auth.signed_in(), *sh.coachless_ok.lock().unwrap()) {
                (false, _) => SourceState::Off,
                (true, Some(false)) => SourceState::Degraded,
                (true, _) => SourceState::Ok,
            },
            coachless_premium: sh.coachless_auth.premium(),
            message: debug::dir().map(|d| format!("Debug mode: logging to {}", d.display())),
        };
        let recap = memo.recap.clone();
        let macro_report = macro_sync.tick(&sh, &sd, lcu.as_ref(), phase);
        let trends = self_coach.tick(phase, macro_sync.games(), memo.recap.as_ref(), &settings.goals);
        let history = macro_sync.games().map(|g| g.to_vec()).unwrap_or_default();
        let ranked = rank_log.tick(lcu.as_ref(), phase);
        let goal_progress = if game.is_some() { memo.goal_progress.clone() } else { vec![] };
        push(&app, &sh, &mut last_json, move |st| {
            st.phase = phase;
            st.sources = sources;
            st.champ_select = champ_select;
            st.game = game;
            st.recap = recap;
            st.macro_report = macro_report;
            st.trends = trends;
            st.goal_progress = goal_progress;
            st.history = history;
            st.ranked = ranked;
        });

        let tick = match phase {
            Phase::ChampSelect => 1000,
            Phase::InGame | Phase::Loading => 2000,
            Phase::NoClient => 4000,
            _ => 3000,
        };
        tokio::time::sleep(Duration::from_millis(tick)).await;
    }
}

/// Connection refused / reset: the client is really gone.
fn is_connect(e: &(dyn std::error::Error + 'static)) -> bool {
    e.downcast_ref::<reqwest::Error>().is_some_and(|r| r.is_connect())
}

fn flag(ok: Option<bool>) -> SourceState {
    match ok {
        None => SourceState::Off,
        Some(true) => SourceState::Ok,
        Some(false) => SourceState::Degraded,
    }
}

/// Apply `f` to the shared state and emit it if anything changed.
fn push(app: &AppHandle, sh: &Shared, last: &mut String, f: impl FnOnce(&mut AppState)) {
    let mut st = sh.ui.lock().unwrap();
    f(&mut st);
    let json = serde_json::to_string(&*st).unwrap_or_default();
    if json != *last {
        // The JSON just built is the payload: no second serialization.
        if !json.is_empty() {
            let _ = app.emit_str("state", json.clone());
        }
        *last = json;
    }
}

/// Stats for a champion, fetched in the background on first use.
pub fn stats(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Option<Lane>, mode: Mode) -> Option<Arc<ChampStats>> {
    let lane = if mode == Mode::Rift { lane } else { None };
    let (c, patch) = (sd.champs.get(&champ)?, sh.patch()?);
    // Cloned only on a cache miss.
    ensure(sh, |s| &s.stats, |s| &s.lola_ok, StatsKey { champ, lane, mode }, move || {
        let (http, s, c) = (sh.http.clone(), sh.settings(), c.clone());
        async move { data::lolalytics::champ_stats(&http, &c, lane, mode, &s.tier, &s.region, &patch).await }
    })
}

/// My champion's stats: on the Rift, signed in to Coachless, its item slots, boots, runes and
/// summoners replace lolalytics' (`vs` = lane opponent, `built` = my completed items in order).
/// While a narrower answer is on its way, a broader one already fetched stands in.
fn my_stats(sh: &Arc<Shared>, base: Option<Arc<ChampStats>>, lane: Option<Lane>, vs: Option<ChampKey>, built: &[ItemId]) -> Option<Arc<ChampStats>> {
    let base = base?;
    let (Mode::Rift, Some(lane), true) = (base.mode, lane, sh.coachless_auth.signed_in()) else { return Some(base) };
    // A support's first slot is the quest item: Coachless gets no item conditions.
    let built = if lane == Lane::Support { &[][..] } else { built };
    let (first, second) = (built.first().copied(), built.get(1).copied());
    let champ = base.champ;
    for key in [(champ, lane, vs, first, second), (champ, lane, vs, first, None), (champ, lane, vs, None, None)] {
        let (sh2, b) = (sh.clone(), base.clone());
        let found = ensure(sh, |s| &s.coachless, |s| &s.coachless_ok, key, move || async move {
            let s = sh2.settings();
            let q = data::coachless::Query { champ, lane, vs, tier: &s.tier, region: &s.region, first: key.3, second: key.4 };
            data::coachless::champ_stats(&sh2.http, &sh2.coachless_auth, q, &b).await
        });
        if found.is_some() {
            return found;
        }
    }
    Some(base)
}

fn tier_list(sh: &Arc<Shared>, lane: Lane) -> Option<Arc<Vec<TierEntry>>> {
    let patch = sh.patch()?;
    ensure(sh, |s| &s.tiers, |s| &s.lola_ok, lane, move || {
        let (http, s) = (sh.http.clone(), sh.settings());
        async move { data::lolalytics::tier_list(&http, lane, &s.tier, &s.region, &patch).await }
    })
}

pub fn mayhem(sh: &Arc<Shared>, sd: &StaticData) -> Option<Arc<MayhemData>> {
    let (http, version) = (sh.http.clone(), sd.version.clone());
    ensure(sh, |s| &s.mayhem, |s| &s.ugg_ok, (), move || async move { data::ugg::mayhem(&http, &version).await })
}

pub fn augments(sh: &Arc<Shared>, champ: ChampKey, patch: &str) -> Option<Arc<ChampAugments>> {
    let (http, patch) = (sh.http.clone(), patch.to_string());
    ensure(sh, |s| &s.augments, |s| &s.ugg_ok, champ, move || async move {
        data::ugg::champ_augments(&http, &patch, champ).await
    })
}

fn champ_select_advice(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect) -> ChampSelectAdvice {
    let lane = cs.my_lane;
    let me = cs.my_pick.or(cs.my_hover);
    let mine = me.and_then(|c| my_stats(sh, stats(sh, sd, c, lane, cs.mode), lane, None, &[]));
    // Enemy stats in my lane: their counters give their win rate vs each candidate.
    let enemies: Vec<ChampStats> =
        cs.enemies.iter().filter_map(|&e| stats(sh, sd, e, lane, cs.mode)).map(|s| (*s).clone()).collect();
    let tiers = match (cs.mode, lane) {
        (Mode::Rift, Some(l)) => tier_list(sh, l),
        (Mode::Rift, None) => None,
        _ => crate::draft::aram_tiers(sh),
    };
    let mayhem = if cs.mode == Mode::Mayhem { mayhem(sh, sd) } else { None };
    let known: Vec<&ChampStats> = mine.as_deref().into_iter().chain(&enemies).collect();
    let (lane_stats, synergy) = crate::draft::fetch(sh, sd, cs, &known);
    coach::champ_select_advice(&coach::ChampSelectInput {
        cs,
        sd,
        mine: mine.as_deref(),
        enemies: &enemies,
        tier_list: tiers.as_deref().map_or(&[], |t| t.as_slice()),
        mayhem: mayhem.as_deref(),
        draft: coach::DraftData { lane_stats: &lane_stats, synergy: &synergy },
    })
}

/// `games`: my match history (record against my lane opponents).
fn game_advice(sh: &Arc<Shared>, sd: &StaticData, snap: &GameSnapshot, settings: &Settings, memo: &mut Memo, games: &[PastGame]) -> GameAdvice {
    let me = &snap.players[snap.me];
    let lane = if snap.mode == Mode::Rift { me.position.or(memo.my_lane) } else { None };
    let base = stats(sh, sd, me.champ, lane, snap.mode);
    let vs = coach::lane_opponent(snap, base.as_deref()).map(|i| snap.players[i].champ);
    for &id in &me.items {
        if sd.items.get(&id).is_some_and(|i| i.completed && !i.boots) && !memo.built.contains(&id) {
            memo.built.push(id);
        }
    }
    let my_stats = my_stats(sh, base, lane, vs, &memo.built);

    let mut pool = None;
    if snap.mode == Mode::Mayhem
        && let Some(m) = mayhem(sh, sd)
            && let Some(a) = augments(sh, me.champ, &m.patch) {
                pool = Some((a, m));
            }
    // OCR matches card titles against every Mayhem augment (game data), not just
    // the champion's u.gg list, which misses newer augments.
    *sh.ocr_names.lock().unwrap() = match (&pool, settings.ocr_enabled) {
        (Some((a, m)), true) => {
            let ids: Vec<AugmentId> = crate::tab::pool(sh)
                .unwrap_or_else(|| a.prismatic.iter().chain(&a.gold).chain(&a.silver).copied().collect());
            let local = sh.local_augment_names.lock().unwrap().clone();
            let name = |id: AugmentId| {
                let en = m.augment_names.get(&id)?;
                Some(match local.as_ref().and_then(|l| l.get(&id)).filter(|n| !n.eq_ignore_ascii_case(en)) {
                    Some(n) => format!("{en}
{n}"),
                    None => en.clone(),
                })
            };
            Some(Arc::new(ids.into_iter().filter_map(|id| Some((id, name(id)?))).collect()))
        }
        _ => None,
    };

    memo.tracker.observe(snap, sd);
    let mayhem_data = if snap.mode == Mode::Mayhem { mayhem(sh, sd) } else { None };
    let enemy_ranks = crate::tab::tick(sh, sd, snap, &mut memo.tab, mayhem_data.as_deref());

    // Cards on screen right now; the offer itself stays pending while they are
    // hidden (shop, respawn) until it is replaced, my scoreboard row grows, or it ages out.
    let seen = cards_on_screen(sh);
    let mine = memo.tab.mine.len();
    if seen.len() >= 2 {
        for &id in &seen {
            if !memo.offer_history.contains(&id) {
                memo.offer_history.push(id);
            }
        }
        match &mut memo.offer {
            Some(p) if p.ids == seen => p.last_seen = snap.time,
            _ => memo.offer = Some(PendingOffer { ids: seen.clone(), last_seen: snap.time, mine }),
        }
    }
    if memo.offer.as_ref().is_some_and(|p| mine > p.mine || snap.time - p.last_seen > OFFER_HOLD_S) {
        memo.offer = None;
    }
    let offered = memo.offer.as_ref().map_or(vec![], |p| p.ids.clone());
    if snap.mode == Mode::Mayhem {
        let before = memo.augs.picks.len();
        // Stats are compared the moment the cards leave the screen, before shopping blurs
        // them; cards coming back (only the shop was open) undo that close.
        let on_screen = if seen.len() >= 2 { &seen[..] } else { &[] };
        memo.augs.observe(on_screen, snap, sd);
        if memo.augs.picks.len() != before {
            tracing::info!("augment picks (stat diff): {:?} (stats {:?})", memo.augs.picks, snap.my_stats);
        }
    }
    let my_augments = my_augments(memo, snap.time);

    let mut advice = coach::game_advice(&coach::GameInput {
        snap,
        sd,
        stats: my_stats.as_deref(),
        augments: pool.as_ref().map(|(a, m)| (&**a, &**m)),
        offered: &offered,
        gold_history: &memo.gold.samples,
        my_augments: &my_augments,
        enemy_augments: coach::EnemyAugmentInput {
            seen: &memo.tab.seen,
            ranks: &enemy_ranks,
            stale_since: memo.tab.stale_since,
        },
        keep: memo.last_buy,
    });
    let ults = crate::ingame::ults(sh, sd, snap);
    let enemies: Vec<ChampKey> = snap.players.iter().filter(|p| p.team != me.team).map(|p| p.champ).collect();
    let memory = coach::matchup_memory(games, &enemies, snap.mode, Some(me.champ));
    // Spells only for the laning panel's opponents: the lane opponent and the bot lane pair.
    let faced: Vec<ChampKey> = if snap.mode == Mode::Rift {
        let bot = |l: Option<Lane>| matches!(l, Some(Lane::Bottom | Lane::Support));
        let opp = advice.threats.iter().find(|t| t.lane_opp).map(|t| t.champ);
        let pair = bot(lane) || snap.players.iter().any(|p| Some(p.champ) == opp && bot(p.position));
        snap.players.iter().filter(|p| p.team != me.team && (Some(p.champ) == opp || (pair && bot(p.position)))).map(|p| p.champ).collect()
    } else {
        vec![]
    };
    let spells = crate::ingame::spells(sh, sd, &faced);
    let notes = crate::draft::lane_notes();
    coach::live_extras(&mut advice, &coach::LiveInput { snap, sd, tracker: &memo.tracker, ults: &ults, notes, spells: &spells, memory: &memory });
    let watch = snap.mode == Mode::Rift && settings.gank_warn;
    *sh.gank_ctx.lock().unwrap() = watch.then(|| crate::state::GankCtx {
        time: snap.time,
        at: Instant::now(),
        blue: me.team == Team::Order,
        lane,
        enemies: Arc::new(gank_enemies(sh, sd, snap, me.team)),
    });
    if watch {
        let calls = sh.gank_calls.lock().unwrap();
        if calls.1.elapsed() < MINIMAP_TTL {
            inject_gank(&mut advice, &calls.0);
        }
    }
    if let Some(s) = advice.gold.history.last() {
        memo.gold.push(s.clone());
    }
    memo.last_buy = advice.buy_next.as_ref().map(|p| p.item);
    if let Some(p) = &advice.buy_next
        && !memo.recommended.contains(&p.item) {
            memo.recommended.push(p.item);
        }
    memo.goal_progress = crate::selfcoach::on_tick(&mut memo.log, snap, &advice, sd, &settings.goals);
    memo.last_champ = Some(me.champ);
    memo.last_lane = lane;
    advice
}

/// Mayhem augment cards on screen right now (OCR, screen order); empty when none.
pub fn cards_on_screen(sh: &Shared) -> Vec<AugmentId> {
    let o = sh.offered.lock().unwrap();
    if o.1.elapsed() < OFFER_TTL { o.0.clone() } else { vec![] }
}

/// My augments: the Tab scoreboard row when read (an icon shared by several
/// augments is narrowed to the one I was offered), else stat-diff detection.
fn my_augments(memo: &Memo, time: f32) -> Vec<MyAugment> {
    if memo.tab.mine.len() < memo.augs.picks.len() {
        return memo.augs.picks.clone();
    }
    memo.tab
        .mine
        .iter()
        .map(|s| {
            let offered: Vec<AugmentId> = s.ids.iter().copied().filter(|id| memo.offer_history.contains(id)).collect();
            let candidates = if offered.is_empty() { s.ids.clone() } else { offered };
            MyAugment { id: (candidates.len() == 1).then(|| candidates[0]), candidates, time }
        })
        .collect()
}

fn recap(sh: &Arc<Shared>, sd: &StaticData, eog: &EogStats, memo: &Memo) -> Recap {
    let bench = stats(sh, sd, eog.champ, memo.last_lane, eog.mode).map(|s| s.benchmarks.clone()).unwrap_or_default();
    let r = coach::recap(eog, &bench, &memo.recommended, sd);
    let r = crate::selfcoach::finish(r, &memo.log, sd, &sh.settings().goals, unix_now());
    let path = settings::local_dir("recaps").join(format!("{}.json", unix_now()));
    if let Ok(json) = serde_json::to_string_pretty(&r) {
        let _ = std::fs::write(path, json);
    }
    r
}

fn load_last_recap() -> Option<Recap> {
    let dir = settings::local_dir("recaps");
    let newest = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).max()?;
    serde_json::from_str(&std::fs::read_to_string(newest).ok()?).ok()
}

fn record(memo: &mut Memo, raw: Option<serde_json::Value>) {
    if memo.last_record.is_some_and(|t| t.elapsed() < RECORD_EVERY) {
        return;
    }
    if let Some(raw) = raw {
        memo.last_record = Some(Instant::now());
        let path = settings::local_dir("fixtures").join(format!("allgamedata-{}.json", unix_now()));
        let _ = std::fs::write(path, raw.to_string());
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// capture → OCR → match, like `ocr::scan`, with debug logging of new text and
/// frames of (partial) offers, so real-game layouts can be checked afterwards.
fn scan_offer(names: &std::collections::HashMap<AugmentId, String>, last_lines: &mut Vec<String>) -> anyhow::Result<Vec<AugmentId>> {
    let t = Instant::now();
    let Some(f) = ocr::capture_league() else { return Ok(vec![]) };
    let captured = t.elapsed();
    let lines = ocr::recognize(&f)?;
    let ids = ocr::match_augments(&lines, names);
    if debug::on() {
        if lines != *last_lines {
            tracing::debug!(
                "ocr {}x{} capture {captured:?} total {:?}, {} lines, matched {ids:?}: {lines:?}",
                f.width,
                f.height,
                t.elapsed(),
                lines.len()
            );
            *last_lines = lines;
        }
        if !ids.is_empty() {
            debug::frame(if ids.len() >= 2 { "ocr-offer" } else { "ocr-partial" }, Duration::from_secs(10), &f);
        }
    }
    Ok(ids)
}

/// The enemy team for the minimap thread (names, jungler), fetching their portraits in the background.
fn gank_enemies(sh: &Arc<Shared>, sd: &StaticData, snap: &GameSnapshot, mine: Team) -> coach::GankEnemies {
    let foes: Vec<&Player> = snap.players.iter().filter(|p| p.team != mine).collect();
    for p in &foes {
        if let Some(c) = sd.champs.get(&p.champ) {
            let (http, url, circles) = (sh.http.clone(), data::icons::champion_icon_url(&sd.version, &c.id), data::icons::champion_circle_urls(&c.id));
            ensure(sh, |s| &s.portraits, |s| &s.ddragon_ok, p.champ, move || async move {
                let png = |b: Vec<u8>| ocr::scoreboard::decode_png(&b);
                let square = png(data::icons::get_bytes(&http, &url, PORTRAIT_TTL).await?).ok_or_else(|| anyhow::anyhow!("bad champion icon {url}"))?;
                let mut circle = None;
                for u in &circles {
                    if let Ok(b) = data::icons::get_bytes(&http, u, PORTRAIT_TTL).await {
                        circle = png(b);
                        break;
                    }
                }
                let mut art = vec![ocr::minimap::Art::Square(&square)];
                art.extend(circle.as_ref().map(ocr::minimap::Art::Circle));
                Ok(ocr::minimap::portrait(&art))
            });
        }
    }
    coach::GankEnemies {
        names: foes.iter().filter_map(|p| Some((p.champ, sd.champs.get(&p.champ)?.name.clone()))).collect(),
        jungler: foes.iter().find(|p| p.smite).or_else(|| foes.iter().find(|p| p.position == Some(Lane::Jungle))).map(|p| p.champ),
        lanes: foes.iter().filter_map(|p| Some((p.champ, p.position?))).collect(),
    }
}

/// Replace the gank warnings in `advice` with `calls`, ahead of the other calls (they're the
/// most time-critical: first on the overlay strip and first to be spoken).
fn inject_gank(advice: &mut GameAdvice, calls: &[Call]) {
    advice.calls.retain(|c| !c.key.starts_with("gank-"));
    advice.calls.splice(0..0, calls.iter().cloned());
}

/// Minimap vision thread: reads enemy blips off the minimap while the orchestrator asks for it
/// (on the Rift, gank warnings on) and the game is in the foreground, tracks them and
/// pushes gank warnings to the UI as soon as they change. Never sends input or takes focus.
pub fn spawn_minimap(app: AppHandle, sh: Arc<Shared>) {
    std::thread::Builder::new()
        .name("minimap".into())
        .spawn(move || {
            let mut tracker = coach::GankTracker::default();
            let mut said: Vec<Call> = vec![];
            loop {
                let ctx = sh.gank_ctx.lock().unwrap().clone();
                let Some(ctx) = ctx else {
                    tracker = coach::GankTracker::default();
                    said.clear();
                    *sh.minimap_state.lock().unwrap() = SourceState::Off;
                    std::thread::sleep(MINIMAP_IDLE);
                    continue;
                };
                std::thread::sleep(MINIMAP_SCAN);
                if !ocr::league_foreground() {
                    continue;
                }
                let cfg = sh.minimap_settings();
                let Some(frame) = ocr::league_client().and_then(|(cw, ch)| ocr::capture_client(ocr::minimap::rect(cw, ch, cfg.scale, cfg.flipped))) else {
                    *sh.minimap_state.lock().unwrap() = SourceState::Error;
                    continue;
                };
                let sq = ocr::minimap::map_square(&frame, cfg.flipped);
                let blips = ocr::enemy_blips(&frame, sq);
                let portraits: Vec<(ChampKey, Arc<ocr::minimap::Portrait>)> = {
                    let cache = sh.portraits.lock().unwrap();
                    ctx.enemies
                        .names
                        .keys()
                        .filter_map(|k| match cache.get(k) {
                            Some(crate::state::Slot::Ready(p)) => Some((*k, p.clone())),
                            _ => None,
                        })
                        .collect()
                };
                let refs: Vec<(ChampKey, &ocr::minimap::Portrait)> = portraits.iter().map(|(k, p)| (*k, &**p)).collect();
                let names = ocr::minimap::name_blips(&frame, sq, &blips, &refs);
                let time = ctx.time + ctx.at.elapsed().as_secs_f32().min(CLOCK_ON);
                let world: Vec<(i32, i32, Option<ChampKey>)> = blips
                    .iter()
                    .zip(&names)
                    .map(|(&(fx, fy), &champ)| {
                        let (x, y) = MapZone::from_minimap(fx, fy);
                        (x, y, champ)
                    })
                    .collect();
                tracker.observe(&world, ctx.blue, time);
                let calls = tracker.warnings(time, ctx.lane, &ctx.enemies);
                if debug::on() {
                    debug::frame("minimap", Duration::from_secs(3), &frame);
                    let keys: Vec<&str> = calls.iter().map(|c| c.key.as_str()).collect();
                    let square = (sq.x, sq.y, sq.side);
                    debug::dump("minimap-blips", Duration::from_secs(1), &(time, (frame.width, frame.height), cfg.scale, &blips, &world, keys, square));
                }
                *sh.minimap_state.lock().unwrap() = SourceState::Ok;
                *sh.gank_calls.lock().unwrap() = (calls.clone(), Instant::now());
                if calls.iter().map(|c| &c.key).ne(said.iter().map(|c| &c.key)) {
                    let json = {
                        let mut st = sh.ui.lock().unwrap();
                        let Some(g) = st.game.as_mut() else { continue };
                        inject_gank(g, &calls);
                        serde_json::to_string(&*st).unwrap_or_default()
                    };
                    if !json.is_empty() {
                        let _ = app.emit_str("state", json);
                    }
                    said = calls;
                }
            }
        })
        .expect("spawn minimap thread");
}

/// OCR thread: scans while the orchestrator provides an augment pool.
pub fn spawn_ocr(sh: Arc<Shared>) {
    std::thread::Builder::new()
        .name("ocr".into())
        .spawn(move || {
            let mut last_offer: Option<Instant> = None;
            let mut last_lines = Vec::new();
            loop {
                std::thread::sleep(crate::tab::poll(&sh).min(OFFER_SCAN));
                if last_offer.is_some_and(|t| t.elapsed() < OFFER_SCAN) {
                    continue;
                }
                last_offer = Some(Instant::now());
                let Some(names) = sh.ocr_names.lock().unwrap().clone() else {
                    *sh.ocr_state.lock().unwrap() = SourceState::Off;
                    continue;
                };
                match scan_offer(&names, &mut last_lines) {
                    Ok(ids) => {
                        *sh.ocr_state.lock().unwrap() = SourceState::Ok;
                        if ids.len() >= 2 {
                            let mut o = sh.offered.lock().unwrap();
                            if o.0 != ids {
                                let n: Vec<&str> = ids.iter().filter_map(|i| names.get(i).map(String::as_str)).collect();
                                tracing::info!("augment offer: {n:?}");
                            }
                            *o = (ids, Instant::now());
                        }
                    }
                    Err(e) => {
                        tracing::warn!("ocr: {e:#}");
                        *sh.ocr_state.lock().unwrap() = SourceState::Error;
                    }
                }
            }
        })
        .expect("spawn ocr thread");
}
