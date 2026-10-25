//! Draft data for champ select: Rift per-lane matchups and teammate synergy for
//! revealed champions, the ARAM tier list for the bench; fetched in the background.

use crate::state::{Shared, ensure};
use model::*;
use std::sync::{Arc, OnceLock};

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

fn counters_vs(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Lane, vs_lane: Lane) -> Option<Arc<Vec<Matchup>>> {
    let c = sd.champs.get(&champ)?.clone();
    let (http, s, patch) = (sh.http.clone(), sh.settings(), sh.patch()?);
    ensure(
        sh,
        |s| &s.vs_counters,
        |s| &s.lola_ok,
        (champ, lane, vs_lane),
        move || async move { data::lolalytics::counters_vs(&http, &c, lane, vs_lane, &s.tier, &s.region, &patch).await },
    )
}

fn matchup_page(sh: &Arc<Shared>, sd: &StaticData, me: ChampKey, vs: ChampKey, lane: Lane, vs_lane: Lane) -> Option<Arc<MatchupBuild>> {
    let (a, b) = (sd.champs.get(&me)?.clone(), sd.champs.get(&vs)?.clone());
    let (http, s, patch) = (sh.http.clone(), sh.settings(), sh.patch()?);
    ensure(
        sh,
        |s| &s.matchups,
        |s| &s.lola_ok,
        (me, vs, lane, vs_lane),
        move || async move { data::lolalytics::matchup(&http, &a, &b, lane, vs_lane, &s.tier, &s.region, &patch).await },
    )
}

/// Hand-written laning notes, parsed once (empty if the file is broken).
pub fn lane_notes() -> &'static LaneNotes {
    static NOTES: OnceLock<LaneNotes> = OnceLock::new();
    NOTES.get_or_init(|| data::notes::lane_notes().unwrap_or_else(|e| {
        tracing::warn!("lane notes: {e:#}");
        LaneNotes::default()
    }))
}

/// Matchup card for my lane opponent(s) once my lane and champion are known;
/// lolalytics and Data Dragon parts join once fetched.
pub fn matchup(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect, advice: &ChampSelectAdvice) -> Option<MatchupPlan> {
    let me = cs.my_pick.or(cs.my_hover).filter(|_| cs.mode == Mode::Rift)?;
    let draft = advice.draft.as_ref();
    let lanes = draft.map_or(&[][..], |d| d.lanes.as_slice());
    let lane = cs.my_lane.or_else(|| lanes.iter().find(|l| l.ally == Some(me)).map(|l| l.lane))?;
    let enemy = |l: Lane| lanes.iter().find(|x| x.lane == l).and_then(|x| x.enemy);
    let faced: Vec<(Lane, ChampKey)> = coach::opposing_lanes(lane).iter().filter_map(|&l| Some((l, enemy(l)?))).collect();
    let vs: Vec<(Lane, Vec<Matchup>)> = faced.iter().filter_map(|&(l, _)| Some((l, (*counters_vs(sh, sd, me, lane, l)?).clone()))).collect();
    // Direct opponent: same lane, else (support with their support unknown) their bot laner.
    let build = faced.iter().find(|f| f.0 == lane).or(faced.first()).and_then(|&(l, e)| matchup_page(sh, sd, me, e, lane, l));
    let champs: Vec<ChampKey> = faced.iter().map(|f| f.1).collect();
    let spells = crate::ingame::spells(sh, sd, &champs);
    coach::matchup_plan(&coach::MatchupInput {
        cs,
        card: coach::CardData { sd, notes: lane_notes(), me, spells: &spells, memory: &advice.memory },
        lane,
        lanes,
        duos: draft.map_or(&[][..], |d| d.allies.duos.as_slice()),
        vs: &vs,
        build: build.as_deref(),
    })
}

/// ARAM win rates and tiers for the ARAM / Mayhem bench.
pub fn aram_tiers(sh: &Arc<Shared>) -> Option<Arc<Vec<TierEntry>>> {
    let (http, s, patch) = (sh.http.clone(), sh.settings(), sh.patch()?);
    ensure(
        sh,
        |s| &s.aram_tiers,
        |s| &s.lola_ok,
        (),
        move || async move { data::lolalytics_aram::aram_tier_list(&http, &s.tier, &s.region, &patch).await },
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
