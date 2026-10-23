//! Tauri commands called by the UI.

use crate::state::Shared;
use crate::{orchestrator, selfcoach, settings};
use model::*;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, State};

type Sh<'a> = State<'a, Arc<Shared>>;

#[tauri::command]
pub fn get_state(sh: Sh) -> AppState {
    sh.ui.lock().unwrap().clone()
}

/// Waits (up to ~20 s) for the orchestrator's first Data Dragon load.
#[tauri::command]
pub async fn get_static(sh: Sh<'_>) -> Result<UiStatic, String> {
    let mut sd = sh.static_data();
    for _ in 0..100 {
        if sd.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        sd = sh.static_data();
    }
    let sd = sd.ok_or("Data Dragon unavailable (offline?)")?;
    let mayhem = match orchestrator::mayhem(&sh, &sd) {
        Some(m) => Some(m),
        None => data::ugg::mayhem(&sh.http, &sd.version).await.ok().map(Arc::new),
    };
    Ok(UiStatic {
        version: sd.version.clone(),
        ugg_patch: mayhem.as_ref().map(|m| m.patch.clone()),
        champs: sd.champs.iter().map(|(k, c)| (*k, UiChamp { id: c.id.clone(), name: c.name.clone() })).collect(),
        items: sd.items.iter().map(|(k, i)| (*k, UiItem { name: i.name.clone(), cost: i.cost })).collect(),
        augments: mayhem.map(|m| m.augment_names.clone()).unwrap_or_default(),
    })
}

#[tauri::command]
pub fn set_settings(app: tauri::AppHandle, sh: Sh, settings: Settings) -> Result<AppState, String> {
    settings::save(&settings).map_err(|e| e.to_string())?;
    let changed_data = {
        let st = sh.ui.lock().unwrap();
        st.settings.tier != settings.tier || st.settings.region != settings.region
    };
    if changed_data {
        sh.clear_stats();
    }
    let mut st = sh.ui.lock().unwrap();
    st.settings = settings;
    let _ = app.emit("state", &*st);
    Ok(st.clone())
}

#[tauri::command]
pub async fn lookup(sh: Sh<'_>, champ: ChampKey, lane: Option<Lane>, mode: Mode) -> Result<ChampStats, String> {
    let sd = sh.static_data().ok_or("static data not loaded yet")?;
    fetch_stats(&sh, &sd, champ, lane, mode).await
}

async fn fetch_stats(sh: &Shared, sd: &StaticData, champ: ChampKey, lane: Option<Lane>, mode: Mode) -> Result<ChampStats, String> {
    let c = sd.champs.get(&champ).ok_or("unknown champion")?;
    let s = sh.settings();
    let patch = sh.patch().ok_or("no patch")?;
    let lane = if mode == Mode::Rift { lane } else { None };
    data::lolalytics::champ_stats(&sh.http, c, lane, mode, &s.tier, &s.region, &patch).await.map_err(|e| format!("{e:#}"))
}

/// Reviews one game from my match history (needs the League client).
#[tauri::command]
pub async fn analyze_game(sh: Sh<'_>, game_id: u64) -> Result<MatchAnalysis, String> {
    let sd = sh.static_data().ok_or("static data not loaded yet")?;
    let lcu = tokio::task::spawn_blocking(clients::lcu::discover).await.ok().flatten().ok_or("League client not running")?;
    let d = clients::history::fetch_match(&lcu, game_id).await.map_err(|e| format!("{e:#}"))?;
    let g = &d.game;
    let stats = match orchestrator::stats(&sh, &sd, g.champ, g.lane, g.mode) {
        Some(s) => Some(s),
        None => fetch_stats(&sh, &sd, g.champ, g.lane, g.mode).await.ok().map(Arc::new),
    };
    let recap = selfcoach::recap_for(g);
    tracing::info!("analysed game {game_id} (live recap: {})", recap.is_some());
    Ok(coach::analyze_match(&d, &sd, stats.as_deref(), recap.as_ref()))
}

/// Frontend errors and warnings (and debug-mode messages) into the app log.
#[tauri::command]
pub fn log_frontend(level: String, message: String) {
    match level.as_str() {
        "error" => tracing::error!(target: "web", "{message}"),
        "warn" => tracing::warn!(target: "web", "{message}"),
        _ => tracing::debug!(target: "web", "{message}"),
    }
}
