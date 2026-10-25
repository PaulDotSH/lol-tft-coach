//! TFT: watches the client for TFT queues, loads Set data and the queue's meta,
//! reads my screen in game (tft-vision, on its own thread) and runs the TFT coach.
//! Pushes `TftState` on the `tft` event. Independent of the LoL orchestrator,
//! which stays quiet in TFT. Units are told apart by an image model fetched once
//! (`%LOCALAPPDATA%\lol-coach\models`) and looks learnt over games (`tft\looks.bin`); items by
//! the Set's item icons (Community Dragon, in the HTTP cache).

use crate::debug;
use clients::lcu::Lcu;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tft_model::*;

/// Screen reads per second budget: one read, then this pause.
const READ_EVERY: Duration = Duration::from_millis(900);
/// Meta is re-fetched this often (the HTTP cache keeps it 6 h anyway).
const META_EVERY: Duration = Duration::from_secs(3600);
/// History after a game: retry until the new game shows up.
const HISTORY_RETRY: Duration = Duration::from_secs(30);
const HISTORY_TRIES: u32 = 10;
const HISTORY_COUNT: u32 = 30;
/// The learnt looks are written this often while reading (and when the game ends).
const LOOKS_SAVE: Duration = Duration::from_secs(120);
/// Built-in looks of every unit from 3D renders (TFL2; empty: no seed yet).
const LOOKS_SEED: &[u8] = include_bytes!("../../fixtures/tft/looks_seed.bin");
/// The image model download may take a while (88 MB).
const MODEL_TIMEOUT: Duration = Duration::from_secs(600);

pub struct TftShared {
    state: Mutex<TftState>,
    set: Mutex<Option<Arc<TftSetData>>>,
    /// The image model on disk, once fetched.
    model: Mutex<Option<std::path::PathBuf>>,
    /// Item icon templates, once fetched; the reader picks them up when they arrive.
    items: Mutex<Option<Arc<tft_vision::ItemIcons>>>,
}

impl Default for TftShared {
    fn default() -> Self {
        TftShared {
            state: Mutex::new(TftState {
                phase: TftPhase::None,
                queue: None,
                mode: TftMode::Standard,
                advice: None,
                meta: vec![],
                history: vec![],
                trends: vec![],
                review: None,
                status: vec![],
            }),
            set: Mutex::new(None),
            model: Mutex::new(None),
            items: Mutex::new(None),
        }
    }
}

type St<'a> = tauri::State<'a, Arc<TftShared>>;

#[tauri::command]
pub fn get_tft(tft: St) -> TftState {
    tft.state.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_tft_static(tft: St) -> Result<TftSetData, String> {
    tft.set.lock().unwrap().as_deref().cloned().ok_or_else(|| "TFT set data not loaded yet".into())
}

/// Client phase → TFT screen, for a TFT queue.
fn phase(gameflow: &str) -> TftPhase {
    match gameflow {
        "InProgress" | "Reconnect" | "GameStart" => TftPhase::InGame,
        "PreEndOfGame" | "EndOfGame" | "WaitingForStats" => TftPhase::PostGame,
        _ => TftPhase::Lobby,
    }
}

/// The in-game reader: screen → snapshot → coach, on a blocking thread.
struct Reader {
    stop: Arc<AtomicBool>,
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn looks_path() -> std::path::PathBuf {
    crate::settings::local_dir("tft").join("looks.bin")
}

fn save_looks(lib: &tft_coach::Library) {
    let path = looks_path();
    let tmp = path.with_extension("tmp");
    if let Err(e) = std::fs::write(&tmp, lib.to_bytes()).and_then(|_| std::fs::rename(&tmp, &path)) {
        tracing::warn!("tft looks {path:?}: {e:#}");
    }
}

/// Item icon templates for the set.
async fn fetch_items(http: &data::Http, set: &TftSetData) -> anyhow::Result<tft_vision::ItemIcons> {
    let bodies = tft_data::item_icons(http, set).await;
    anyhow::ensure!(!bodies.is_empty(), "no item icons");
    let decoded = tokio::task::spawn_blocking(move || bodies.into_iter().filter_map(|(a, b)| Some((a, ocr::scoreboard::decode_png(&b)?))).collect()).await?;
    let icons = tokio::task::spawn_blocking(move || tft_vision::ItemIcons::new(decoded)).await?;
    tracing::info!("tft: {} item icons", icons.len());
    Ok(icons)
}

/// The image model, downloaded once.
async fn fetch_model() -> anyhow::Result<std::path::PathBuf> {
    let path = crate::settings::local_dir("models").join(tft_embed::MODEL_FILE);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1_000_000) {
        return Ok(path);
    }
    tracing::info!("tft: fetching the unit model ({})", tft_embed::MODEL_URL);
    let client = reqwest::Client::builder().timeout(MODEL_TIMEOUT).build()?;
    let bytes = client.get(tft_embed::MODEL_URL).send().await?.error_for_status()?.bytes().await?;
    let tmp = path.with_extension("part");
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, &path).await?;
    Ok(path)
}

fn start_reader(
    tft: Arc<TftShared>,
    set: Arc<TftSetData>,
    meta: Arc<TftMeta>,
    queue: i64,
    me: Option<String>,
    model: Option<std::path::PathBuf>,
) -> Reader {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || {
        let embedder = model.and_then(|m| match tft_embed::Embedder::load(&m) {
            Ok(e) => Some(e),
            Err(e) => {
                tracing::warn!("tft unit model {m:?}: {e:#}");
                None
            }
        });
        let mut vision = tft_vision::Reader::new(&set).with_me(me).with_embedder(embedder);
        let library = std::fs::read(looks_path()).ok().and_then(|b| tft_coach::Library::from_bytes(&b)).unwrap_or_default().with_seed(LOOKS_SEED);
        let seed = library.seeded();
        tracing::info!("tft: {} looks of {} units known, seed {} looks of {} units", library.len(), library.units(), seed.0, seed.1);
        let mut tracker = tft_coach::Tracker::default().with_library(library);
        let (mut saved, mut saved_len) = (Instant::now(), tracker.library().len());
        let mut keep: Option<String> = None;
        let hooks: Box<dyn tft_coach::ModeHooks> = match TftMode::of_queue(queue) {
            // Choncc or Treat until 2026-11-17: delete with the tft-choncc crate.
            TftMode::Choncc => Box::new(tft_choncc::ChonccHooks),
            TftMode::Standard => Box::new(tft_coach::StandardHooks),
        };
        while !flag.load(Ordering::Relaxed) {
            let t = Instant::now();
            if !vision.has_items() {
                vision.set_items(tft.items.lock().unwrap().clone());
            }
            if let Some(snap) = vision.read() {
                debug::dump("tft-snapshot", Duration::from_secs(5), &snap);
                tracker.observe(&snap, &set);
                let advice = tft_coach::advise(&tft_coach::CoachInput {
                    snap: &snap,
                    tracker: &tracker,
                    set: &set,
                    meta: &meta,
                    hooks: hooks.as_ref(),
                    keep: keep.as_deref(),
                });
                keep = advice.comps.first().map(|c| c.comp.clone());
                debug::dump("tft-advice", Duration::from_secs(5), &advice);
                tft.state.lock().unwrap().advice = Some(advice);
            }
            if saved.elapsed() > LOOKS_SAVE && tracker.library().len() != saved_len {
                save_looks(tracker.library());
                (saved, saved_len) = (Instant::now(), tracker.library().len());
            }
            std::thread::sleep(READ_EVERY.saturating_sub(t.elapsed()));
        }
        if tracker.library().len() != saved_len {
            save_looks(tracker.library());
        }
    });
    Reader { stop }
}

/// My Riot ID game name, as my tactician's plate shows it.
async fn my_name(lcu: &Lcu) -> Option<String> {
    let me = lcu.get("/lol-summoner/v1/current-summoner").await.ok().flatten()?;
    me["gameName"].as_str().or(me["displayName"].as_str()).map(str::to_string).filter(|n| !n.is_empty())
}

async fn history(lcu: &Lcu) -> anyhow::Result<Vec<TftPastGame>> {
    let me = lcu.get("/lol-summoner/v1/current-summoner").await?.ok_or_else(|| anyhow::anyhow!("no summoner"))?;
    let puuid = me["puuid"].as_str().ok_or_else(|| anyhow::anyhow!("no puuid"))?;
    let path = format!("/lol-match-history/v1/products/tft/{puuid}/matches?begin=0&count={HISTORY_COUNT}");
    let json = lcu.get(&path).await?.ok_or_else(|| anyhow::anyhow!("no TFT history"))?;
    Ok(tft_data::parse_history(&json, Some(puuid)))
}

pub async fn run(app: AppHandle, sh: Arc<crate::state::Shared>, tft: Arc<TftShared>) {
    let mut lcu: Option<Lcu> = None;
    let mut metas: HashMap<i64, (Arc<TftMeta>, Instant)> = HashMap::new();
    let mut reader: Option<Reader> = None;
    let mut games: Vec<TftPastGame> = vec![];
    let (mut want_history, mut tries, mut last_try) = (true, 0u32, None::<Instant>);
    let mut prev = TftPhase::None;
    let mut last_json = String::new();
    let mut me: Option<String> = None;
    let mut model_task: Option<tauri::async_runtime::JoinHandle<()>> = None;
    let mut items_task: Option<tauri::async_runtime::JoinHandle<()>> = None;
    loop {
        if lcu.is_none() {
            lcu = tokio::task::spawn_blocking(clients::lcu::discover).await.ok().flatten();
        }
        let flow = match &lcu {
            Some(c) => c.get("/lol-gameflow/v1/session").await.ok().flatten(),
            None => None,
        };
        if flow.is_none() {
            lcu = None;
        }
        let flow = flow.unwrap_or_default();
        let queue = flow.pointer("/gameData/queue/id").and_then(|v| v.as_i64()).filter(|&q| q > 0);
        let tft_queue = flow.pointer("/gameData/queue/gameMode").and_then(|v| v.as_str()) == Some("TFT");
        let now = if tft_queue { phase(flow["phase"].as_str().unwrap_or_default()) } else { TftPhase::None };

        let mut status = vec![];
        let set = if now == TftPhase::None {
            None
        } else {
            let have = tft.set.lock().unwrap().clone();
            match have {
                Some(s) => Some(s),
                None => match tft_data::set_data(&sh.http).await {
                    Ok(s) => {
                        let s = Arc::new(s);
                        *tft.set.lock().unwrap() = Some(s.clone());
                        Some(s)
                    }
                    Err(e) => {
                        tracing::warn!("tft set data: {e:#}");
                        None
                    }
                },
            }
        };
        status.push(("Set data".to_string(), set.is_some()));
        let meta = match (&set, queue.filter(|_| tft_queue)) {
            (Some(set), Some(q)) => {
                let fresh = metas.get(&q).filter(|(_, at)| at.elapsed() < META_EVERY).map(|(m, _)| m.clone());
                match fresh {
                    Some(m) => Some(m),
                    None => match tft_data::meta(&sh.http, q, set).await {
                        Ok(m) => {
                            let m = Arc::new(m);
                            metas.insert(q, (m.clone(), Instant::now()));
                            Some(m)
                        }
                        Err(e) => {
                            tracing::warn!("tft meta {q}: {e:#}");
                            metas.get(&q).map(|(m, _)| m.clone())
                        }
                    },
                }
            }
            _ => None,
        };
        status.push(("Meta".to_string(), meta.is_some()));
        // The unit model: fetched in the background the first time a TFT queue shows up.
        if now != TftPhase::None && model_task.is_none() {
            let tft = tft.clone();
            model_task = Some(tauri::async_runtime::spawn(async move {
                match fetch_model().await {
                    Ok(p) => *tft.model.lock().unwrap() = Some(p),
                    Err(e) => tracing::warn!("tft unit model: {e:#}"),
                }
            }));
        }
        let model = tft.model.lock().unwrap().clone();
        status.push(("Unit recognition".to_string(), model.is_some()));
        // Item icons: fetched in the background once the set is known.
        if let Some(set) = &set
            && items_task.is_none()
        {
            let (tft, http, set) = (tft.clone(), sh.http.clone(), set.clone());
            items_task = Some(tauri::async_runtime::spawn(async move {
                match fetch_items(&http, &set).await {
                    Ok(i) => *tft.items.lock().unwrap() = Some(Arc::new(i)),
                    Err(e) => tracing::warn!("tft item icons: {e:#}"),
                }
            }));
        }
        status.push(("Item recognition".to_string(), tft.items.lock().unwrap().is_some()));
        if me.is_none()
            && now != TftPhase::None
            && let Some(c) = &lcu
        {
            me = my_name(c).await;
        }

        // In game: one reader thread per game.
        match (now, &set, &meta, queue) {
            (TftPhase::InGame, Some(s), Some(m), Some(q)) if reader.is_none() => {
                tracing::info!("tft: reading the screen (queue {q})");
                tft.state.lock().unwrap().advice = None;
                reader = Some(start_reader(tft.clone(), s.clone(), m.clone(), q, me.clone(), model.clone()));
            }
            (TftPhase::InGame, ..) => {}
            _ => reader = None,
        }
        status.push(("Screen reading".to_string(), reader.is_some()));
        if prev == TftPhase::InGame && now != TftPhase::InGame {
            (want_history, tries) = (true, 0);
        }
        if prev != now {
            tracing::info!("tft phase {prev:?} → {now:?}");
            prev = now;
        }

        // History after a game (and once at start), retried until the new game appears.
        if now != TftPhase::InGame
            && want_history
            && last_try.is_none_or(|t| t.elapsed() > HISTORY_RETRY)
            && let Some(c) = &lcu
        {
            last_try = Some(Instant::now());
            match history(c).await {
                Ok(g) => {
                    tries += 1;
                    if g.first().map(|x| x.game_id) != games.first().map(|x| x.game_id) || tries >= HISTORY_TRIES {
                        want_history = false;
                    }
                    games = g;
                }
                Err(e) => tracing::warn!("tft history: {e:#}"),
            }
        }

        {
            let mut st = tft.state.lock().unwrap();
            st.phase = now;
            st.queue = queue.filter(|_| tft_queue);
            st.mode = queue.map_or(TftMode::Standard, TftMode::of_queue);
            st.status = status;
            // The last advice stays up after the game, until the next lobby.
            if matches!(now, TftPhase::Lobby | TftPhase::None) {
                st.advice = None;
            }
            if let (Some(set), Some(meta)) = (&set, &meta) {
                st.meta = tft_coach::pregame(meta, set, None);
                let mut hist = games.clone();
                for g in &mut hist {
                    g.comp = g.comp.take().or_else(|| tft_coach::match_comp(g, meta));
                }
                st.trends = tft_coach::trends(&hist, meta);
                st.review = hist.first().map(|g| tft_coach::review(g, meta, set));
                st.history = hist;
            }
            let json = serde_json::to_string(&*st).unwrap_or_default();
            if json != last_json {
                let _ = app.emit("tft", &*st);
                last_json = json;
            }
        }
        let pause = if now == TftPhase::InGame { 1000 } else { 3000 };
        tokio::time::sleep(Duration::from_millis(pause)).await;
    }
}
