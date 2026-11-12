//! Draft data for champ select: Rift per-lane matchups and teammate synergy for
//! revealed champions, the ARAM tier list for the bench; fetched in the background.

use crate::state::{Shared, ensure};
use model::*;
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

pub fn matchup_page(sh: &Arc<Shared>, sd: &StaticData, me: ChampKey, vs: ChampKey, lane: Lane, vs_lane: Lane) -> Option<Arc<MatchupBuild>> {
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

/// Hand-written laning notes, parsed once (empty if the file is broken).
pub fn lane_notes() -> &'static LaneNotes {
    static NOTES: OnceLock<LaneNotes> = OnceLock::new();
    NOTES.get_or_init(|| data::notes::lane_notes().unwrap_or_else(|e| {
        tracing::warn!("lane notes: {e:#}");
        LaneNotes::default()
    }))
}

/// Revealed enemies in the lanes I face.
type Faced = Vec<(Lane, ChampKey)>;

/// Rift: me, my lane and the revealed enemies in the opposing lane(s).
fn lane_up(cs: &ChampSelect, advice: &ChampSelectAdvice) -> Option<(ChampKey, Lane, Faced)> {
    let me = cs.my_pick.or(cs.my_hover).filter(|_| cs.mode == Mode::Rift)?;
    let lanes = advice.draft.as_ref().map_or(&[][..], |d| d.lanes.as_slice());
    let lane = cs.my_lane.or_else(|| lanes.iter().find(|l| l.ally == Some(me)).map(|l| l.lane))?;
    let enemy = |l: Lane| lanes.iter().find(|x| x.lane == l).and_then(|x| x.enemy);
    Some((me, lane, coach::opposing_lanes(lane).iter().filter_map(|&l| Some((l, enemy(l)?))).collect()))
}

/// Direct opponent: same lane, else (support with their support unknown) their bot laner.
fn direct(lane: Lane, faced: &[(Lane, ChampKey)]) -> Option<(Lane, ChampKey)> {
    faced.iter().find(|f| f.0 == lane).or(faced.first()).copied()
}

fn coachless_runes(sh: &Arc<Shared>, me: ChampKey, lane: Lane, vs: ChampKey, base_wr: f32) -> Option<Arc<Option<RuneData>>> {
    ensure(sh, |s| &s.coachless_runes, |s| &s.coachless_ok, (me, lane, vs), move || {
        let sh = sh.clone();
        async move {
            let s = sh.settings();
            let q = data::coachless::Query { champ: me, lane, vs: Some(vs), tier: &s.tier, region: &s.region, first: None, second: None };
            data::coachless::matchup_runes(&sh.http, &sh.coachless_auth, q, base_wr).await
        }
    })
}

/// My rune pages against my direct lane opponent, as fetched so far.
pub struct VsPages {
    pub vs: ChampKey,
    pub coachless: Option<Arc<Option<RuneData>>>,
    pub lolalytics: Option<Arc<MatchupBuild>>,
}

/// `base_wr`: my champion's win rate, which Coachless' WPA is added to. Coachless is asked
/// only against my own lane's opponent (signed in).
pub fn vs_runes(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect, advice: &ChampSelectAdvice, base_wr: f32) -> Option<VsPages> {
    let (me, lane, faced) = lane_up(cs, advice)?;
    let (l, e) = direct(lane, &faced)?;
    let coachless = if l == lane && sh.coachless_auth.signed_in() { coachless_runes(sh, me, lane, e, base_wr) } else { None };
    Some(VsPages { vs: e, coachless, lolalytics: matchup_page(sh, sd, me, e, lane, l) })
}

/// Matchup card for my lane opponent(s) once my lane and champion are known;
/// lolalytics and Data Dragon parts join once fetched.
pub fn matchup(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect, advice: &ChampSelectAdvice) -> Option<MatchupPlan> {
    let (me, lane, faced) = lane_up(cs, advice)?;
    let draft = advice.draft.as_ref();
    let lanes = draft.map_or(&[][..], |d| d.lanes.as_slice());
    let vs: Vec<(Lane, Vec<Matchup>)> = faced.iter().filter_map(|&(l, _)| Some((l, (*counters_vs(sh, sd, me, lane, l)?).clone()))).collect();
    let build = direct(lane, &faced).and_then(|(l, e)| matchup_page(sh, sd, me, e, lane, l));
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
