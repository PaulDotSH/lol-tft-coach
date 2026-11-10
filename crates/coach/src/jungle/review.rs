use crate::analysis::place;
use crate::fights::{FIGHT_GAP, dead};
use model::*;

const MIN_GAME_S: u32 = 600;
const GANKS_UNTIL: f32 = 14.0 * 60.0;
const FIRST_CLEAR: [u32; 2] = [3, 4];
const CLEAR: [(u32, u32); 3] = [(0, 5), (5, 10), (10, 15)];
const VERSUS: [u32; 3] = [5, 10, 15];
const INVADE_CS: u32 = 4;

fn lane_of(pos: (i32, i32)) -> Option<Lane> {
    match MapZone::at(pos.0, pos.1) {
        MapZone::Top => Some(Lane::Top),
        MapZone::Mid => Some(Lane::Middle),
        MapZone::Bot => Some(Lane::Bottom),
        _ => None,
    }
}

pub(crate) fn review(d: &MatchDetail, marks: &[TimelineMark], benchmark: Option<f32>) -> Option<JungleReview> {
    let g = &d.game;
    if g.mode != Mode::Rift || g.remake || g.duration_s < MIN_GAME_S || d.frames.is_empty() || d.players[d.me].lane != Some(Lane::Jungle) {
        return None;
    }
    let me = d.me;
    let theirs: Vec<usize> = d.players.iter().enumerate().filter(|(_, p)| !p.ally && p.lane == Some(Lane::Jungle)).map(|(i, _)| i).collect();
    let opp = (theirs.len() == 1).then(|| theirs[0]);
    let at = |minute: u32| {
        let t = minute as f32 * 60.0;
        d.frames.iter().find(|f| f.time >= t && f.time < t + 60.0 && f.time <= g.duration_s as f32)
    };
    let first_clear = FIRST_CLEAR
        .iter()
        .filter_map(|&minute| at(minute).map(|f| ClearMark { minute, level: f.players[me].level, jungle_cs: f.players[me].jungle_cs }))
        .collect();
    let clear = CLEAR
        .iter()
        .filter_map(|&(from, to)| {
            let end = at(to)?.players[me];
            let start = if from == 0 { FrameStat::default() } else { at(from)?.players[me] };
            let span = (to - from) as f32;
            Some(ClearRate {
                from,
                to,
                jungle_per_min: end.jungle_cs.saturating_sub(start.jungle_cs) as f32 / span,
                cs_per_min: end.cs.saturating_sub(start.cs) as f32 / span,
            })
        })
        .collect();

    let early: Vec<&KillEvent> = d.kills.iter().filter(|k| k.time < GANKS_UNTIL).collect();
    let mut chains: Vec<Vec<&KillEvent>> = vec![];
    for k in early {
        match chains.last_mut() {
            Some(c) if k.time - c[c.len() - 1].time <= FIGHT_GAP => c.push(k),
            _ => chains.push(vec![k]),
        }
    }
    let mut ganks: Vec<GankLane> = vec![];
    for c in &chains {
        let mine = |k: &&&KillEvent| !d.players[k.victim].ally && (k.killer == Some(me) || k.assisters.contains(&me));
        let Some(lane) = c.iter().filter(mine).find_map(|k| k.pos.and_then(lane_of)) else { continue };
        let kills = c.iter().filter(|k| !d.players[k.victim].ally).count() as u32;
        let deaths = c.len() as u32 - kills;
        match ganks.iter_mut().find(|r| r.lane == lane) {
            Some(r) => (r.ganks, r.kills, r.deaths) = (r.ganks + 1, r.kills + kills, r.deaths + deaths),
            None => ganks.push(GankLane { lane, ganks: 1, kills, deaths }),
        }
    }
    ganks.sort_by_key(|r| [Lane::Top, Lane::Middle, Lane::Bottom].iter().position(|l| *l == r.lane));

    let objectives = marks
        .iter()
        .filter(|m| m.kind != "Tower" && m.kind != "Inhibitor")
        .map(|m| JungleObjective {
            time: m.time,
            kind: m.kind.clone(),
            detail: m.detail.clone(),
            ours: m.ours,
            presence: m.presence,
            enemy_jungler_dead: opp.is_some_and(|o| dead(d, o, m.time)),
        })
        .collect();

    let blue = d.players.first().is_some_and(|p| p.ally);
    let invades = d
        .frames
        .windows(2)
        .filter(|w| {
            let (a, b) = (&w[0].players[me], &w[1].players[me]);
            b.pos.is_some_and(|(x, y)| place(MapZone::at(x, y), blue) == "enemy jungle") && b.jungle_cs >= a.jungle_cs + INVADE_CS
        })
        .map(|w| (w[1].time / 60.0).round() as u32)
        .collect();

    let versus = VERSUS
        .iter()
        .filter_map(|&minute| {
            let f = at(minute)?;
            let (m, o) = (&f.players[me], opp.map(|o| &f.players[o]));
            Some(PhaseRow {
                minute,
                cs: m.jungle_cs,
                level: m.level,
                opp_cs: o.map(|o| o.jungle_cs),
                opp_level: o.map(|o| o.level),
                gold_lead: o.map(|o| m.total_gold as i32 - o.total_gold as i32),
            })
        })
        .collect();

    Some(JungleReview { enemy: opp.map(|o| d.players[o].champ), first_clear, clear, benchmark, ganks, objectives, invades, versus })
}
