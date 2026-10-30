//! Draft data for champ select: Rift per-lane matchups and teammate synergy for
//! revealed champions, the ARAM tier list for the bench; fetched in the background.

use crate::state::{Shared, ensure};
use model::*;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

fn lane_stats(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Option<Lane>) -> Option<Arc<ChampStats>> {
    let (c, patch) = (sd.champs.get(&champ)?, sh.patch()?);
    ensure(
        sh,
        |s| &s.lane_stats,
        |s| &s.lola_ok,
        (champ, lane),
        move || {
            let (http, s, c) = (sh.http.clone(), sh.settings(), c.clone());
            async move { data::lolalytics_team::lane_stats(&http, &c, lane, &s.tier, &s.region, &patch).await }
        },
    )
}

fn team(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Lane) -> Option<Arc<TeamSynergy>> {
    let (c, patch) = (sd.champs.get(&champ)?, sh.patch()?);
    ensure(
        sh,
        |s| &s.team,
        |s| &s.lola_ok,
        (champ, lane),
        move || {
            let (http, s, c) = (sh.http.clone(), sh.settings(), c.clone());
            async move { data::lolalytics_team::team(&http, &c, lane, &s.tier, &s.region, &patch).await }
        },
    )
}

fn counters_vs(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Lane, vs_lane: Lane) -> Option<Arc<Vec<Matchup>>> {
    let (c, patch) = (sd.champs.get(&champ)?, sh.patch()?);
    ensure(
        sh,
        |s| &s.vs_counters,
        |s| &s.lola_ok,
        (champ, lane, vs_lane),
        move || {
            let (http, s, c) = (sh.http.clone(), sh.settings(), c.clone());
            async move { data::lolalytics::counters_vs(&http, &c, lane, vs_lane, &s.tier, &s.region, &patch).await }
        },
    )
}

fn matchup(sh: &Arc<Shared>, sd: &StaticData, me: ChampKey, vs: ChampKey, lane: Lane, vs_lane: Lane) -> Option<Arc<MatchupBuild>> {
    let (a, b, patch) = (sd.champs.get(&me)?, sd.champs.get(&vs)?, sh.patch()?);
    ensure(
        sh,
        |s| &s.matchups,
        |s| &s.lola_ok,
        (me, vs, lane, vs_lane),
        move || {
            let (http, s, a, b) = (sh.http.clone(), sh.settings(), a.clone(), b.clone());
            async move { data::lolalytics::matchup(&http, &a, &b, lane, vs_lane, &s.tier, &s.region, &patch).await }
        },
    )
}

fn lane_notes() -> &'static HashMap<String, LaneNote> {
    static NOTES: OnceLock<HashMap<String, LaneNote>> = OnceLock::new();
    NOTES.get_or_init(|| data::lane_notes::lane_notes().unwrap_or_else(|e| {
        tracing::warn!("lane notes: {e:#}");
        HashMap::new()
    }))
}

/// Bot lane 2v2 plan when I play support or ADC; lolalytics parts join once fetched.
pub fn bot_lane(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect, advice: &ChampSelectAdvice) -> Option<BotLanePlan> {
    let lane = cs.my_lane.filter(|l| matches!(l, Lane::Bottom | Lane::Support) && cs.mode == Mode::Rift)?;
    let me = cs.my_pick.or(cs.my_hover)?;
    let draft = advice.draft.as_ref();
    let lanes = draft.map_or(&[][..], |d| d.lanes.as_slice());
    let enemy = |l: Lane| lanes.iter().find(|x| x.lane == l).and_then(|x| x.enemy);
    let vs: Vec<(Lane, Vec<Matchup>)> = [Lane::Bottom, Lane::Support]
        .into_iter()
        .filter(|&l| enemy(l).is_some())
        .filter_map(|l| Some((l, (*counters_vs(sh, sd, me, lane, l)?).clone())))
        .collect();
    let start = enemy(Lane::Bottom).and_then(|e| matchup(sh, sd, me, e, lane, Lane::Bottom));
    coach::bot_lane_plan(&coach::BotLaneInput {
        cs,
        sd,
        lanes,
        duos: draft.map_or(&[][..], |d| d.allies.duos.as_slice()),
        memory: &advice.memory,
        vs: &vs,
        start: start.as_deref(),
        notes: lane_notes(),
    })
}

/// ARAM win rates and tiers for the ARAM / Mayhem bench.
pub fn aram_tiers(sh: &Arc<Shared>) -> Option<Arc<Vec<TierEntry>>> {
    let patch = sh.patch()?;
    ensure(
        sh,
        |s| &s.aram_tiers,
        |s| &s.lola_ok,
        (),
        move || {
            let (http, s) = (sh.http.clone(), sh.settings());
            async move { data::lolalytics_aram::aram_tier_list(&http, &s.tier, &s.region, &patch).await }
        },
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
