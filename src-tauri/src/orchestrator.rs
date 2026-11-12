//! Phase loop: polls the client / game at the phase's rate, keeps remote data
//! warm in the background, runs the coach and pushes `state` to the UI on change.

use crate::health::Reader;
use crate::{account, debug, settings};
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
/// The wider offer band is read at most this often, while an offer is expected and the strip shows none.
const WIDE_EVERY: Duration = Duration::from_secs(5);
/// A Mayhem game this long (s) has shown an offer (the first comes at the start).
const OFFER_BY: f32 = 90.0;
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
    /// Last tick's candidates (`coach::situation_items`) whose Coachless situations are fetched.
    situation_items: Vec<(ItemId, BuildSlot)>,
    last_champ: Option<ChampKey>,
    last_lane: Option<Lane>,
    recap_done: bool,
    recap: Option<Recap>,
    last_record: Option<Instant>,
    gold: coach::GoldHistory,
    tracker: coach::Tracker,
    posture: coach::PostureMemo,
    log: coach::GameLog,
    goal_progress: Vec<GoalProgress>,
    /// Mayhem: augments I took, detected as offers close.
    augs: coach::AugmentTracker,
    tab: crate::tab::TabMemo,
    offer: Option<PendingOffer>,
    /// Every augment shown on a card this game (narrows lookalike scoreboard icons).
    offer_history: Vec<AugmentId>,
    /// The game window's client size, last seen this game.
    client: Option<(u32, u32)>,
    /// Reader health was settled for this game.
    health_done: bool,
    /// Game-plan card, and what it was built from (rebuilt when that changes: a curve landed).
    plan: Option<GamePlan>,
    plan_key: Option<PlanKey>,
}

/// Lineups, curves ready, draft and lane card win rate known, record rows.
type PlanKey = (Vec<(ChampKey, Option<Lane>)>, usize, bool, bool, usize);

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
    let mut macro_sync = Arc::new(crate::macro_sync::MacroSync::default());
    let mut self_coach = crate::selfcoach::SelfCoach::default();
    let mut rank_log = Arc::new(crate::ranked::RankLog::default());
    let account_watch = Arc::new(account::Watch::default());
    let mut account_epoch = account::global().epoch();
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
                *sh.lcu.lock().unwrap() = Some(c.clone());
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
        if account::global().epoch() != account_epoch {
            account_epoch = account::global().epoch();
            tracing::info!("the account's files changed: reading them again");
            macro_sync = Arc::new(crate::macro_sync::MacroSync::default());
            self_coach = crate::selfcoach::SelfCoach::default();
            rank_log = Arc::new(crate::ranked::RankLog::default());
            memo.recap = load_last_recap();
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
                    *sh.lcu.lock().unwrap() = None;
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
                            select_plan(&sh, &sd, &cs, &advice, &mut memo);
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
                if !memo.health_done && memo.last_champ.is_some() {
                    memo.health_done = true;
                    end_game_health(&sh, memo.client);
                }
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
            *sh.offer_expected.lock().unwrap() = false;
        }
        sh.calib.lock().unwrap().save(false);
        if last_phase != Some(phase) {
            tracing::info!("phase {:?} → {phase:?}", last_phase);
            last_phase = Some(phase);
        }
        if let Some(g) = &game {
            debug::dump("game-advice", Duration::from_secs(15), g);
        }

        let mut sources = Sources {
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
        let readers = sh.health.lock().unwrap().report();
        crate::health::degrade(&mut sources, &readers);
        let recap = memo.recap.clone();
        let macro_report = macro_sync.tick(&sh, &sd, lcu.as_ref(), phase);
        let trends = self_coach.tick(phase, macro_sync.games(), memo.recap.as_ref(), &settings.goals);
        let history = macro_sync.games().map(|g| g.to_vec()).unwrap_or_default();
        let ranked = rank_log.tick(lcu.as_ref(), phase);
        account_watch.tick(lcu.as_ref(), phase);
        let sync = sh.sync_status.lock().unwrap().clone();
        let goal_progress = if game.is_some() { memo.goal_progress.clone() } else { vec![] };
        let plan = if matches!(phase, Phase::ChampSelect | Phase::Loading | Phase::InGame) { memo.plan.clone() } else { None };
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
            st.readers = readers;
            st.plan = plan;
            st.sync = sync;
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

/// Coachless WPA by game situation for `want` (last tick's candidates), on the Rift and signed
/// in, as far as fetched; asked after my first two completed items, as `my_stats` asks.
fn situations(sh: &Arc<Shared>, champ: ChampKey, lane: Option<Lane>, vs: Option<ChampKey>, built: &[ItemId], want: &[(ItemId, BuildSlot)]) -> std::collections::HashMap<ItemId, ItemSituations> {
    let (Some(lane), true) = (lane, sh.coachless_auth.signed_in()) else { return Default::default() };
    let built = if lane == Lane::Support { &[][..] } else { built };
    let (first, second) = (built.first().copied(), built.get(1).copied());
    let vs = vs.filter(|_| sh.coachless_auth.premium());
    let mut out = std::collections::HashMap::new();
    for &(item, slot) in want {
        let sh2 = sh.clone();
        let found = ensure(sh, |s| &s.situations, |s| &s.coachless_ok, (champ, lane, vs, first, second, item, slot), move || async move {
            let s = sh2.settings();
            let q = data::coachless::Query { champ, lane, vs, tier: &s.tier, region: &s.region, first, second };
            data::coachless::item_situations(&sh2.http, &sh2.coachless_auth, q, item, slot).await
        });
        if let Some(f) = found {
            out.insert(item, (*f).clone());
        }
    }
    out
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
    let input = coach::ChampSelectInput {
        cs,
        sd,
        mine: mine.as_deref(),
        enemies: &enemies,
        tier_list: tiers.as_deref().map_or(&[], |t| t.as_slice()),
        mayhem: mayhem.as_deref(),
        draft: coach::DraftData { lane_stats: &lane_stats, synergy: &synergy },
    };
    let mut advice = coach::champ_select_advice(&input);
    // Rune pages against my lane opponent need the lanes the draft report placed.
    if let Some(v) = mine.as_deref().and_then(|m| crate::draft::vs_runes(sh, sd, cs, &advice, m.win_rate)) {
        let vs = coach::VsRunes { vs: v.vs, coachless: v.coachless.as_deref().and_then(Option::as_ref), lolalytics: v.lolalytics.as_deref() };
        advice.runes = coach::champ_select_runes(&input, &vs);
    }
    advice
}

/// `games`: my match history (record against my lane opponents).
fn game_advice(sh: &Arc<Shared>, sd: &StaticData, snap: &GameSnapshot, settings: &Settings, memo: &mut Memo, games: &[PastGame]) -> GameAdvice {
    let me = &snap.players[snap.me];
    if memo.last_champ.is_none() {
        sh.health.lock().unwrap().start_game();
    }
    memo.client = ocr::league_client().or(memo.client);
    let lane = if snap.mode == Mode::Rift { me.position.or(memo.my_lane) } else { None };
    let base = stats(sh, sd, me.champ, lane, snap.mode);
    let opp = coach::lane_opponent(snap, base.as_deref()).map(|i| &snap.players[i]);
    let vs = opp.map(|p| p.champ);
    let vs_page = lane.zip(opp).and_then(|(l, p)| crate::draft::matchup_page(sh, sd, me.champ, p.champ, l, p.position.unwrap_or(l)));
    for &id in &me.items {
        if sd.items.get(&id).is_some_and(|i| i.completed && !i.boots) && !memo.built.contains(&id) {
            memo.built.push(id);
        }
    }
    let my_stats = my_stats(sh, base, lane, vs, &memo.built);
    let situations = situations(sh, me.champ, lane, vs, &memo.built, &memo.situation_items);

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
    let reading = snap.mode == Mode::Mayhem && settings.ocr_enabled;
    *sh.offer_expected.lock().unwrap() = reading && seen.len() < 2 && coach::expected_augments(me.level) > my_augments.len();

    let rift = snap.mode == Mode::Rift;
    let curves = lengths(sh, sd, snap.players.iter().map(|p| (p.champ, p.position.filter(|_| rift))), snap.mode);
    let (no_fit, fetched) = (coach::winprob::Model::default(), sh.winprob.lock().unwrap().clone());
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
        matchup: vs_page.as_deref(),
        curves: &curves,
        income: memo.tracker.income(),
        winprob: if settings.win_prob { fetched.as_deref() } else { Some(&no_fit) },
        situations: &situations,
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
    coach::posture(&mut advice, &mut memo.posture, snap, sd);
    let watch = snap.mode == Mode::Rift && settings.gank_warn;
    *sh.gank_ctx.lock().unwrap() = watch.then(|| crate::state::GankCtx {
        time: snap.time,
        at: Instant::now(),
        blue: me.team == Team::Order,
        lane,
        enemies: Arc::new(gank_enemies(sh, sd, snap, me.team)),
        board: advice.jungle.as_ref().and_then(|_| coach::jungle::board(snap, sd, &advice.gold)).map(Arc::new),
    });
    if watch {
        let calls = sh.gank_calls.lock().unwrap();
        if calls.1.elapsed() < MINIMAP_TTL {
            inject_gank(&mut advice, &calls.0);
        }
        let lanes = sh.jungle_lanes.lock().unwrap();
        if lanes.1.elapsed() < MINIMAP_TTL {
            inject_lanes(&mut advice, &lanes.0);
        }
    }
    let since = memo.client.zip(sh.patch()).and_then(|((w, h), patch)| sh.calib.lock().unwrap().unverified("lol", &sh.calib_key(w, h), &patch));
    {
        let mut h = sh.health.lock().unwrap();
        h.enable(Reader::Offer, reading);
        h.enable(Reader::Scoreboard, snap.mode == Mode::Mayhem);
        h.enable(Reader::Minimap, watch);
        if reading && snap.time >= OFFER_BY {
            h.expect(Reader::Offer);
        }
        if advice.tab_hint.is_some() {
            h.expect(Reader::Scoreboard);
        }
        h.calibrated(Reader::Offer, since.clone());
        h.calibrated(Reader::Scoreboard, since);
    }
    if let Some(s) = advice.gold.history.last() {
        memo.gold.push(s.clone());
    }
    memo.last_buy = advice.buy_next.as_ref().map(|p| p.item);
    memo.situation_items = if advice.win.is_some() { coach::situation_items(&advice, snap, sd) } else { vec![] };
    if let Some(p) = &advice.buy_next
        && !memo.recommended.contains(&p.item) {
            memo.recommended.push(p.item);
        }
    let sights = sh.sights.lock().unwrap().clone();
    memo.goal_progress = crate::selfcoach::on_tick(&mut memo.log, snap, &advice, sd, &settings.goals, watch.then_some(&sights));
    game_plan(sd, snap, &advice, &memory, lane, &curves, memo);
    memo.last_champ = Some(me.champ);
    memo.last_lane = lane;
    advice
}

/// Win rate by game length for one champion (its build page), fetched in the background.
fn length(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Option<Lane>, mode: Mode) -> Option<Arc<coach::Curve>> {
    let lane = if mode == Mode::Rift { lane } else { None };
    let (c, patch) = (sd.champs.get(&champ)?, sh.patch()?);
    ensure(sh, |s| &s.lengths, |s| &s.lola_ok, StatsKey { champ, lane, mode }, move || {
        let (http, s, c) = (sh.http.clone(), sh.settings(), c.clone());
        async move { data::lolalytics::length_stats(&http, &c, lane, mode, &s.tier, &s.region, &patch).await }
    })
}

/// The curves of a roster (champion and the lane it plays), as far as fetched.
fn lengths(sh: &Arc<Shared>, sd: &StaticData, roster: impl Iterator<Item = (ChampKey, Option<Lane>)>, mode: Mode) -> std::collections::HashMap<ChampKey, coach::Curve> {
    roster.filter_map(|(c, l)| Some((c, (*length(sh, sd, c, l, mode)?).clone()))).collect()
}

/// Build the game-plan card into `memo` unless its inputs are unchanged. A card that can't be
/// built yet (no curve fetched) leaves the last one standing.
#[allow(clippy::too_many_arguments)]
fn build_plan(
    sd: &StaticData,
    memo: &mut Memo,
    allies: Vec<(ChampKey, Option<Lane>)>,
    enemies: Vec<(ChampKey, Option<Lane>)>,
    length: &std::collections::HashMap<ChampKey, coach::Curve>,
    mode: Mode,
    draft: Option<&DraftReport>,
    matchup: Option<&MatchupPlan>,
    memory: &[MatchupMemory],
) {
    let lane_wr = matchup.is_some_and(|m| m.cards.iter().any(|c| c.vs_wr.is_some()));
    let key: PlanKey = (allies.iter().chain(&enemies).copied().collect(), length.len(), draft.is_some(), lane_wr, memory.len());
    if memo.plan_key.as_ref() == Some(&key) {
        return;
    }
    let plan = coach::game_plan(&coach::GamePlanInput {
        allies: &allies,
        enemies: &enemies,
        length,
        draft,
        matchup,
        memory,
        notes: crate::draft::lane_notes(),
        sd,
        mode,
    });
    if plan.is_some() {
        memo.plan = plan;
    }
    memo.plan_key = Some(key);
}

/// Champ select preview: Rift once all ten champions are shown, lanes as the draft placed them.
fn select_plan(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect, advice: &ChampSelectAdvice, memo: &mut Memo) {
    let Some(d) = advice.draft.as_ref().filter(|d| cs.mode == Mode::Rift && d.allies.champs.len() == 5 && d.enemies.champs.len() == 5) else {
        (memo.plan, memo.plan_key) = (None, None);
        return;
    };
    let lineup = |s: &DraftSide, ally: bool| -> Vec<(ChampKey, Option<Lane>)> {
        s.champs.iter().map(|&c| (c, d.lanes.iter().find(|l| (if ally { l.ally } else { l.enemy }) == Some(c)).map(|l| l.lane))).collect()
    };
    let (allies, enemies) = (lineup(&d.allies, true), lineup(&d.enemies, false));
    let length = lengths(sh, sd, allies.iter().chain(&enemies).copied(), cs.mode);
    build_plan(sd, memo, allies, enemies, &length, cs.mode, Some(d), advice.matchup.as_ref(), &advice.memory);
}

/// In game, from the Live API roster: lanes from `position`, the comps rebuilt from the
/// champions, my lane from the laning panel's cards. `curves`: the ten champions' (`lengths`).
fn game_plan(sd: &StaticData, snap: &GameSnapshot, advice: &GameAdvice, memory: &[MatchupMemory], lane: Option<Lane>, curves: &std::collections::HashMap<ChampKey, coach::Curve>, memo: &mut Memo) {
    let Some(me) = snap.players.get(snap.me) else { return };
    let rift = snap.mode == Mode::Rift;
    let side = |ally: bool| -> Vec<(ChampKey, Option<Lane>)> {
        snap.players.iter().filter(|p| (p.team == me.team) == ally).map(|p| (p.champ, p.position.filter(|_| rift))).collect()
    };
    let (allies, enemies) = (side(true), side(false));
    let keys = |v: &[(ChampKey, Option<Lane>)]| v.iter().map(|x| x.0).collect::<Vec<_>>();
    let draft = coach::roster_draft(sd, &keys(&allies), &keys(&enemies));
    let matchup = lane.filter(|_| rift && !advice.matchup.is_empty()).map(|l| MatchupPlan {
        lane: l,
        allies: vec![me.champ],
        partner: None,
        cards: advice.matchup.clone(),
        build: None,
        runes: vec![],
    });
    build_plan(sd, memo, allies, enemies, curves, snap.mode, Some(&draft), matchup.as_ref(), memory);
}

/// Mayhem augment cards on screen right now (OCR, screen order); empty when none.
pub fn cards_on_screen(sh: &Shared) -> Vec<AugmentId> {
    offer_on_screen(sh).iter().map(|c| c.id).collect()
}

/// [`cards_on_screen`] with where each title was read.
pub fn offer_on_screen(sh: &Shared) -> Vec<ocr::offer::Card> {
    let o = sh.offered.lock().unwrap();
    if o.1.elapsed() < OFFER_TTL { o.0.clone() } else { vec![] }
}

/// The game ended: a reader that should have found its element and never did is warned about once,
/// with what it needs to be fixed.
fn end_game_health(sh: &Shared, client: Option<(u32, u32)>) {
    let missed = sh.health.lock().unwrap().end_game();
    let size = client.map_or("unknown size".to_string(), |(w, h)| format!("{w}x{h}"));
    let hud = sh.minimap_settings().hud_scale.map_or("unknown".to_string(), |s| format!("{s:.2}"));
    for r in missed {
        tracing::warn!("screen reading: {} not found in a whole game ({size}, HUD scale {hud})", r.name());
    }
    sh.calib.lock().unwrap().save(true);
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
    let path = account::dir("recaps").join(format!("{}.json", unix_now()));
    if let Ok(json) = serde_json::to_string_pretty(&r) {
        let _ = std::fs::write(path, json);
    }
    r
}

fn load_last_recap() -> Option<Recap> {
    let dir = account::dir("recaps");
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

/// capture → OCR → locate the card titles, on the calibrated strip or the `wide` band, with debug
/// logging of new text and frames of (partial) offers, so real-game layouts can be checked afterwards.
fn scan_offer(sh: &Shared, names: &std::collections::HashMap<AugmentId, String>, last_lines: &mut Vec<String>, wide: bool) -> anyhow::Result<Vec<ocr::offer::Card>> {
    let t = Instant::now();
    let Some((w, h)) = ocr::league_client() else { return Ok(vec![]) };
    let strip = match wide {
        true => ocr::offer::WIDE,
        false => sh.calib.lock().unwrap().get("lol", &sh.calib_key(w, h)).and_then(|e| e.offer_strip).unwrap_or(ocr::offer::STRIP),
    };
    let r = ocr::offer::rect(w, h, strip);
    let Some(f) = ocr::capture_client(r) else { return Ok(vec![]) };
    let captured = t.elapsed();
    let boxes = ocr::recognize_boxes(&f)?;
    let cards = ocr::offer::locate(&boxes, names, r, (w, h));
    if debug::on() {
        let lines: Vec<String> = boxes.iter().map(|b| b.text.clone()).collect();
        if lines != *last_lines {
            let at: Vec<(AugmentId, f32, f32)> = cards.iter().map(|c| (c.id, c.x, c.y)).collect();
            tracing::debug!("ocr {}x{} at {r:?}{} capture {captured:?} total {:?}, {} lines, cards {at:?}: {lines:?}", f.width, f.height, if wide { " (wide)" } else { "" }, t.elapsed(), lines.len());
            *last_lines = lines;
        }
        let kind = match (wide, cards.len()) {
            (true, 0) => "ocr-wide-miss",
            (true, _) => "ocr-wide",
            (false, 0) => "",
            (false, 1) => "ocr-partial",
            _ => "ocr-offer",
        };
        if !kind.is_empty() {
            debug::frame(kind, Duration::from_secs(if wide { 30 } else { 10 }), &f);
        }
    }
    Ok(cards)
}

/// A read offer: on screen for the overlay and the coach, its title row into the calibration
/// (taken as is when the wide band found it), the reader found.
fn offer_read(sh: &Shared, cards: Vec<ocr::offer::Card>, wide: bool) {
    let top = cards.iter().map(|c| c.y).fold(f32::MAX, f32::min);
    let scale = cards[0].found.scale;
    if let (Some((w, h)), Some(patch)) = (ocr::league_client(), sh.patch()) {
        let key = sh.calib_key(w, h);
        sh.calib.lock().unwrap().offer_strip("lol", &key, &patch, unix_now(), ocr::offer::strip_for(top, scale), wide);
    }
    let shift = crate::health::shift(scale, top - ocr::offer::TITLE_TOP).or_else(|| wide.then(|| "found by the wide scan".to_string()));
    sh.health.lock().unwrap().found(Reader::Offer, Shared::now_ms(), shift);
    *sh.offered.lock().unwrap() = (cards, Instant::now());
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

fn inject_lanes(advice: &mut GameAdvice, lanes: &[LaneOpen]) {
    if let Some(j) = advice.jungle.as_mut()
        && !lanes.is_empty()
    {
        j.lanes = lanes.to_vec();
        j.minimap = true;
    }
}

/// Minimap vision thread: reads enemy blips off the minimap while the orchestrator asks for it
/// (on the Rift, gank warnings on) and the game is in the foreground, tracks them and
/// pushes gank warnings to the UI as soon as they change. Never sends input or takes focus.
pub fn spawn_minimap(app: AppHandle, sh: Arc<Shared>) {
    std::thread::Builder::new()
        .name("minimap".into())
        .spawn(move || {
            let mut tracker = coach::GankTracker::default();
            let mut sights = coach::SightLog::default();
            let mut said: Vec<Call> = vec![];
            let mut shown: Vec<LaneOpen> = vec![];
            loop {
                let ctx = sh.gank_ctx.lock().unwrap().clone();
                let Some(ctx) = ctx else {
                    tracker = coach::GankTracker::default();
                    sights = coach::SightLog::default();
                    *sh.sights.lock().unwrap() = coach::SightLog::default();
                    said.clear();
                    shown.clear();
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
                let found = ocr::minimap::find_square(&frame);
                {
                    let mut h = sh.health.lock().unwrap();
                    h.expect(Reader::Minimap);
                    if found.is_some() {
                        h.found(Reader::Minimap, Shared::now_ms(), None);
                    }
                }
                let sq = found.unwrap_or_else(|| ocr::minimap::map_square(&frame, cfg.flipped));
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
                let named: Vec<(i32, i32, Option<ChampKey>)> = world.iter().zip(tracker.last_names()).map(|(&(x, y, _), c)| (x, y, c)).collect();
                sights.push(time, &named, ctx.blue, &calls);
                *sh.sights.lock().unwrap() = sights.clone();
                if debug::on() {
                    debug::frame("minimap", Duration::from_secs(3), &frame);
                    let keys: Vec<&str> = calls.iter().map(|c| c.key.as_str()).collect();
                    let square = (sq.x, sq.y, sq.side);
                    debug::dump("minimap-blips", Duration::from_secs(1), &(time, (frame.width, frame.height), cfg.scale, &blips, &world, keys, square));
                }
                *sh.minimap_state.lock().unwrap() = SourceState::Ok;
                *sh.gank_calls.lock().unwrap() = (calls.clone(), Instant::now());
                let lanes = ctx.board.as_ref().map_or(vec![], |b| coach::jungle::lanes(b, Some(&tracker.view(time, &ctx.enemies))));
                *sh.jungle_lanes.lock().unwrap() = (lanes.clone(), Instant::now());
                let new_calls = calls.iter().map(|c| &c.key).ne(said.iter().map(|c| &c.key));
                if new_calls || lanes != shown {
                    let json = {
                        let mut st = sh.ui.lock().unwrap();
                        let Some(g) = st.game.as_mut() else { continue };
                        inject_gank(g, &calls);
                        inject_lanes(g, &lanes);
                        serde_json::to_string(&*st).unwrap_or_default()
                    };
                    if !json.is_empty() {
                        let _ = app.emit_str("state", json);
                    }
                    said = calls;
                    shown = lanes;
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
            let mut last_wide: Option<Instant> = None;
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
                let read = scan_offer(&sh, &names, &mut last_lines, false).and_then(|cards| {
                    let due = last_wide.is_none_or(|t| t.elapsed() >= WIDE_EVERY);
                    if cards.len() >= 2 || !due || !*sh.offer_expected.lock().unwrap() {
                        return Ok((cards, false));
                    }
                    last_wide = Some(Instant::now());
                    Ok((scan_offer(&sh, &names, &mut last_lines, true)?, true))
                });
                match read {
                    Ok((cards, wide)) => {
                        *sh.ocr_state.lock().unwrap() = SourceState::Ok;
                        if cards.len() >= 2 {
                            if cards.iter().map(|c| c.id).ne(cards_on_screen(&sh)) {
                                let n: Vec<&str> = cards.iter().filter_map(|c| names.get(&c.id).map(String::as_str)).collect();
                                tracing::info!("augment offer{}: {n:?}", if wide { " (wide scan)" } else { "" });
                            }
                            offer_read(&sh, cards, wide);
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
