//! Tauri commands called by the UI.

use crate::state::Shared;
use crate::{orchestrator, settings};
use model::*;
use std::sync::Arc;
use tauri::{Emitter, State};

type Sh<'a> = State<'a, Arc<Shared>>;

#[tauri::command]
pub fn get_state(sh: Sh) -> AppState {
    sh.ui.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_static(sh: Sh) -> Result<UiStatic, String> {
    let sd = sh.static_data().ok_or("static data not loaded yet")?;
    let mayhem = orchestrator::mayhem(&sh, &sd);
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
    let c = sd.champs.get(&champ).ok_or("unknown champion")?;
    let s = sh.settings();
    let patch = sh.patch().ok_or("no patch")?;
    let lane = if mode == Mode::Rift { lane } else { None };
    data::lolalytics::champ_stats(&sh.http, c, lane, mode, &s.tier, &s.region, &patch).await.map_err(|e| format!("{e:#}"))
}
