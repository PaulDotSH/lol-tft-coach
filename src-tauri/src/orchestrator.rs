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
    recommended: Vec<ItemId>,
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
                            let advice = champ_select_advice(&sh, &sd, &cs);
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
                        if let Ok(Some(q)) = c.queue_id().await {
                            memo.mode = Some(Mode::from_queue(q));
                            tracing::info!("queue {q} → {:?}", memo.mode);
                        }
                        memo.queue_checked = true;
                    }
                let (state, raw) = live.poll(&sd).await;
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
                        game = Some(game_advice(&sh, &sd, &snap, &mut memo));
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
                if !memo.recap_done && let Some(c) = &lcu {
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
            message: debug::dir().map(|d| format!("Debug mode: logging to {}", d.display())),
        };
        let recap = memo.recap.clone();
        let macro_report = macro_sync.tick(&sh, &sd, lcu.as_ref(), phase);
        let trends = self_coach.tick(phase, macro_sync.games(), memo.recap.as_ref(), &settings.goals);
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
        let _ = app.emit("state", &*st);
        *last = json;
    }
}

/// Stats for a champion, fetched in the background on first use.
pub fn stats(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Option<Lane>, mode: Mode) -> Option<Arc<ChampStats>> {
    let lane = if mode == Mode::Rift { lane } else { None };
    let c = sd.champs.get(&champ)?.clone();
    let (http, s, patch) = (sh.http.clone(), sh.settings(), sh.patch()?);
    ensure(sh, |s| &s.stats, |s| &s.lola_ok, StatsKey { champ, lane, mode }, move || async move {
        data::lolalytics::champ_stats(&http, &c, lane, mode, &s.tier, &s.region, &patch).await
    })
}

fn tier_list(sh: &Arc<Shared>, lane: Lane) -> Option<Arc<Vec<TierEntry>>> {
    let (http, s, patch) = (sh.http.clone(), sh.settings(), sh.patch()?);
    ensure(sh, |s| &s.tiers, |s| &s.lola_ok, lane, move || async move {
        data::lolalytics::tier_list(&http, lane, &s.tier, &s.region, &patch).await
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
    let mine = me.and_then(|c| stats(sh, sd, c, lane, cs.mode));
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

fn game_advice(sh: &Arc<Shared>, sd: &StaticData, snap: &GameSnapshot, memo: &mut Memo) -> GameAdvice {
    let me = &snap.players[snap.me];
    let lane = if snap.mode == Mode::Rift { me.position.or(memo.my_lane) } else { None };
    let my_stats = stats(sh, sd, me.champ, lane, snap.mode);

    let mut pool = None;
    if snap.mode == Mode::Mayhem
        && let Some(m) = mayhem(sh, sd)
            && let Some(a) = augments(sh, me.champ, &m.patch) {
                pool = Some((a, m));
            }
    // OCR matches card titles against every Mayhem augment (game data), not just
    // the champion's u.gg list, which misses newer augments.
    *sh.ocr_names.lock().unwrap() = match (&pool, sh.settings().ocr_enabled) {
        (Some((a, m)), true) => {
            let ids: Vec<AugmentId> = crate::tab::pool(sh)
                .unwrap_or_else(|| a.prismatic.iter().chain(&a.gold).chain(&a.silver).copied().collect());
            Some(Arc::new(ids.into_iter().filter_map(|id| Some((id, m.augment_names.get(&id)?.clone()))).collect()))
        }
        _ => None,
    };

    memo.tracker.observe(snap, sd);
    let mayhem_data = if snap.mode == Mode::Mayhem { mayhem(sh, sd) } else { None };
    let enemy_ranks = crate::tab::tick(sh, sd, snap, &mut memo.tab, mayhem_data.as_deref());

    // Cards on screen right now; the offer itself stays pending while they are
    // hidden (shop, respawn) until it is replaced, my scoreboard row grows, or it ages out.
    let seen = {
        let o = sh.offered.lock().unwrap();
        if o.1.elapsed() < OFFER_TTL { o.0.clone() } else { vec![] }
    };
    let mine = memo.tab.mine.len();
    if seen.len() >= 2 {
        for &id in &seen {
            if !memo.offer_history.contains(&id) {
                memo.offer_history.push(id);
            }
        }
        match &mut memo.offer {
            Some(p) if p.ids == seen => p.last_seen = snap.time,
            _ => memo.offer = Some(PendingOffer { ids: seen, last_seen: snap.time, mine }),
        }
    }
    if memo.offer.as_ref().is_some_and(|p| mine > p.mine || snap.time - p.last_seen > OFFER_HOLD_S) {
        memo.offer = None;
    }
    let offered = memo.offer.as_ref().map_or(vec![], |p| p.ids.clone());
    if snap.mode == Mode::Mayhem {
        let before = memo.augs.picks.len();
        memo.augs.observe(&offered, snap, sd);
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
    });
    let ults = crate::ingame::ults(sh, sd, snap);
    coach::live_extras(&mut advice, &coach::LiveInput { snap, sd, tracker: &memo.tracker, ults: &ults });
    if let Some(s) = advice.gold.history.last() {
        memo.gold.push(s.clone());
    }
    if let Some(p) = &advice.buy_next
        && !memo.recommended.contains(&p.item) {
            memo.recommended.push(p.item);
        }
    memo.goal_progress = crate::selfcoach::on_tick(&mut memo.log, snap, &advice, sd, &sh.settings().goals);
    memo.last_champ = Some(me.champ);
    memo.last_lane = lane;
    advice
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
