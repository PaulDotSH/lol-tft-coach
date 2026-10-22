//! Draft data for Rift champ select: per-lane matchups and teammate synergy
//! for every revealed champion, fetched in the background like the rest.

use crate::state::{Shared, ensure};
use model::*;
use std::sync::Arc;

fn lane_stats(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Option<Lane>) -> Option<Arc<ChampStats>> {
    let c = sd.champs.get(&champ)?.clone();
    let (http, s, patch) = (sh.http.clone(), sh.settings(), sh.patch()?);
    ensure(
        sh,
        |s| &s.lane_stats,
        |s| &s.lola_ok,
        (champ, lane),
        move || async move { data::lolalytics_team::lane_stats(&http, &c, lane, &s.tier, &s.region, &patch).await },
    )
}

fn team(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Lane) -> Option<Arc<TeamSynergy>> {
    let c = sd.champs.get(&champ)?.clone();
    let (http, s, patch) = (sh.http.clone(), sh.settings(), sh.patch()?);
    ensure(
        sh,
        |s| &s.team,
        |s| &s.lola_ok,
        (champ, lane),
        move || async move { data::lolalytics_team::team(&http, &c, lane, &s.tier, &s.region, &patch).await },
    )
}

/// Lane stats and synergy for `coach::DraftData`. `known` = stats already at
/// hand (mine, enemies in my lane); they give lane shares for placing enemies.
/// Only revealed champions are fetched: two requests each, cached for 12 h.
pub fn fetch(
    sh: &Arc<Shared>,
    sd: &StaticData,
    cs: &ChampSelect,
    known: &[&ChampStats],
) -> (Vec<ChampStats>, Vec<TeamSynergy>) {
    if cs.mode != Mode::Rift {
        return (vec![], vec![]);
    }
    let me = cs.my_pick.or(cs.my_hover);
    // Allies without an assigned lane (blind pick): their default lane's stats give lane shares.
    let mut stats: Vec<ChampStats> = cs
        .allies
        .iter()
        .filter(|a| !cs.ally_lanes.iter().any(|x| x.0 == **a))
        .filter_map(|&a| lane_stats(sh, sd, a, None))
        .map(|s| (*s).clone())
        .collect();
    let (al, en) = {
        let all: Vec<&ChampStats> = known.iter().copied().chain(&stats).collect();
        coach::draft_lineups(cs, &all)
    };
    let mut synergy = Vec::new();
    for &(c, lane) in al.iter().chain(&en) {
        // My lane's stats are `mine`; my pairs come from the allies' synergy rows.
        let Some(lane) = lane.filter(|_| Some(c) != me) else { continue };
        if !stats.iter().any(|s| s.champ == c && s.lane == Some(lane))
            && let Some(s) = lane_stats(sh, sd, c, Some(lane))
        {
            stats.push((*s).clone());
        }
        if let Some(t) = team(sh, sd, c, lane) {
            synergy.push((*t).clone());
        }
    }
    (stats, synergy)
}
