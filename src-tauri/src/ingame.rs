//! In-game extras fetched in the background: enemy R cooldowns and the spells of
//! matchup opponents (static data).

use crate::state::{Shared, ensure};
use model::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Data Dragon R cooldowns of the enemies in `snap`, as far as fetched.
pub fn ults(sh: &Arc<Shared>, sd: &StaticData, snap: &GameSnapshot) -> HashMap<ChampKey, UltCooldowns> {
    let my_team = snap.players.get(snap.me).map(|p| p.team);
    snap.players
        .iter()
        .filter(|p| Some(p.team) != my_team)
        .filter_map(|p| {
            let id = &sd.champs.get(&p.champ)?.id;
            let u = ensure(sh, |s| &s.ults, |s| &s.ddragon_ok, p.champ, move || {
                let (http, version, id) = (sh.http.clone(), sd.version.clone(), id.clone());
                async move { data::spells::ult(&http, &version, &id).await }
            })?;
            Some((p.champ, (*u).clone()))
        })
        .collect()
}

/// Data Dragon spells of `champs`, as far as fetched (champ select and laning matchup cards).
pub fn spells(sh: &Arc<Shared>, sd: &StaticData, champs: &[ChampKey]) -> HashMap<ChampKey, Vec<SpellInfo>> {
    champs
        .iter()
        .filter_map(|&c| {
            let id = &sd.champs.get(&c)?.id;
            let s = ensure(sh, |s| &s.spells, |s| &s.ddragon_ok, c, move || {
                let (http, version, id) = (sh.http.clone(), sd.version.clone(), id.clone());
                async move { data::spells::spells(&http, &version, &id).await }
            })?;
            Some((c, (*s).clone()))
        })
        .collect()
}
