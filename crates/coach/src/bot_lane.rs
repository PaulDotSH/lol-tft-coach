//! Bot lane 2v2 plan for champ select (me support or ADC): my win rates against
//! their bot laner and support, synergy with my partner, their lane windows from
//! the hand-written notes, my past games against them and matchup starting items.

use model::*;
use std::collections::HashMap;

/// Matchup items from fewer games than this aren't worth showing.
const MIN_MATCHUP_GAMES: u32 = 100;

pub struct BotLaneInput<'a> {
    pub cs: &'a ChampSelect,
    pub sd: &'a StaticData,
    /// Draft lanes (enemies placed by lane shares).
    pub lanes: &'a [DraftLane],
    /// My team's duo synergy.
    pub duos: &'a [DraftDuo],
    pub memory: &'a [MatchupMemory],
    /// My win rates against each lane (lolalytics `vslane`), by their lane.
    pub vs: &'a [(Lane, Vec<Matchup>)],
    /// Matchup page against their bot laner.
    pub start: Option<&'a MatchupBuild>,
    pub notes: &'a HashMap<String, LaneNote>,
}

fn tags(n: &LaneNote) -> Vec<String> {
    [(n.level2, "strong level 2"), (n.level6, "all-in at 6"), (n.engage, "hard engage"), (n.poke, "poke"), (n.sustain, "sustain")]
        .into_iter()
        .filter(|t| t.0)
        .map(|t| t.1.to_string())
        .collect()
}

pub fn plan(i: &BotLaneInput) -> Option<BotLanePlan> {
    let cs = i.cs;
    let lane = cs.my_lane.filter(|l| matches!(l, Lane::Bottom | Lane::Support) && cs.mode == Mode::Rift)?;
    let me = cs.my_pick.or(cs.my_hover)?;
    let other = if lane == Lane::Bottom { Lane::Support } else { Lane::Bottom };
    let at = |l: Lane| i.lanes.iter().find(|x| x.lane == l);
    let partner = at(other).and_then(|x| x.ally).or_else(|| cs.ally_lanes.iter().find(|x| x.1 == other).map(|x| x.0));
    let enemies: Vec<(Lane, ChampKey)> = [Lane::Bottom, Lane::Support].into_iter().filter_map(|l| Some((l, at(l)?.enemy?))).collect();

    let mut numbers: Vec<LaneNumber> = enemies
        .iter()
        .filter_map(|&(l, e)| {
            let m = i.vs.iter().find(|v| v.0 == l)?.1.iter().find(|m| m.champ == e)?;
            Some(LaneNumber { champ: e, with: false, value: m.vs_wr, games: m.games })
        })
        .collect();
    if let Some(p) = partner
        && let Some(d) = i.duos.iter().find(|d| (d.a, d.b) == (me, p) || (d.a, d.b) == (p, me))
    {
        numbers.push(LaneNumber { champ: p, with: true, value: d.delta, games: d.games });
    }
    let windows = enemies
        .iter()
        .filter_map(|&(_, e)| {
            let n = i.notes.get(&i.sd.champs.get(&e)?.id)?;
            Some(LaneWindow { champ: e, tags: tags(n), note: n.note.clone() })
        })
        .collect();
    let theirs: Vec<ChampKey> = enemies.iter().map(|e| e.1).collect();
    Some(BotLanePlan {
        allies: std::iter::once(me).chain(partner).collect(),
        memory: i.memory.iter().filter(|m| theirs.contains(&m.champ)).cloned().collect(),
        start: i.start.filter(|m| m.games >= MIN_MATCHUP_GAMES && theirs.contains(&m.vs)).cloned(),
        enemies: theirs,
        numbers,
        windows,
    })
}
