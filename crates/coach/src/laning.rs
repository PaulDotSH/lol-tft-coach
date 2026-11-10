//! Laning report for the game review: my lane (bot lane: my duo against theirs)
//! minute by minute up to 14:00, kills on the lane's players, everyone's first
//! back, and a verdict from the gold difference.

use crate::analysis::place;
use model::*;

/// Laning ends with the turret plates.
const LANE_END: u32 = 14;
const MIN_GAME_S: u32 = 600;
/// Gold difference that counts as ahead or behind rather than even.
const EVEN: i32 = 300;
/// Unspent gold dropping by this much between frames is a shop visit.
const SPENT_MIN: u32 = 300;

/// "+1,150" / "−900".
pub(crate) fn gold(v: i32) -> String {
    let n = v.unsigned_abs();
    let s = if n >= 1000 { format!("{},{:03}", n / 1000, n % 1000) } else { n.to_string() };
    format!("{}{s}", if v < 0 { "−" } else { "+" })
}

pub fn laning(d: &MatchDetail) -> Option<Laning> {
    let g = &d.game;
    if g.mode != Mode::Rift || g.remake || g.duration_s < MIN_GAME_S || d.frames.is_empty() {
        return None;
    }
    let lane = d.players[d.me].lane?;
    let lanes = if matches!(lane, Lane::Bottom | Lane::Support) { vec![Lane::Bottom, Lane::Support] } else { vec![lane] };
    let side = |ally: bool| -> Vec<usize> {
        let in_lane = |p: &MatchPlayer| p.ally == ally && p.lane.is_some_and(|l| lanes.contains(&l));
        d.players.iter().enumerate().filter(|(_, p)| in_lane(p)).map(|(i, _)| i).collect()
    };
    let (ours, theirs) = (side(true), side(false));
    // More players than lanes means the lane labels are a guess.
    if theirs.is_empty() || theirs.len() > lanes.len() || ours.len() > lanes.len() {
        return None;
    }
    let opp = theirs.iter().copied().find(|&i| d.players[i].lane == Some(lane));

    let sum = |f: &Frame, who: &[usize], v: fn(&FrameStat) -> u32| who.iter().map(|&i| v(&f.players[i]) as i32).sum::<i32>();
    let minutes: Vec<LaneMinute> = (1..=LANE_END)
        .filter_map(|m| {
            let t = m as f32 * 60.0;
            let f = d.frames.iter().find(|f| f.time >= t && f.time < t + 60.0 && f.time <= g.duration_s as f32)?;
            let cs = |s: &FrameStat| s.cs + s.jungle_cs;
            Some(LaneMinute {
                minute: m,
                gold_diff: sum(f, &ours, |s| s.total_gold) - sum(f, &theirs, |s| s.total_gold),
                xp_diff: sum(f, &ours, |s| s.xp) - sum(f, &theirs, |s| s.xp),
                cs_ours: sum(f, &ours, cs) as u32,
                cs_theirs: sum(f, &theirs, cs) as u32,
                my_level: f.players[d.me].level,
                opp_level: opp.map(|o| f.players[o].level),
            })
        })
        .collect();
    let last = minutes.last()?;

    // Ahead (or behind) from the first minute it stayed that way to the end.
    let state = |diff: i32| if diff > EVEN { 1 } else if diff < -EVEN { -1 } else { 0 };
    let end_state = state(last.gold_diff);
    let turned_at = (end_state != 0)
        .then(|| minutes.iter().rev().take_while(|r| state(r.gold_diff) == end_state).last().map(|r| r.minute))
        .flatten();
    let at = format!("at {}:00", last.minute);
    let since = |word: &str| turned_at.filter(|&m| m < last.minute).map_or(String::new(), |m| format!(", {word} from {m}:00"));
    let verdict = match end_state {
        1 => format!("Won lane: {} gold {at}{}", gold(last.gold_diff), since("ahead")),
        -1 => format!("Lost lane: {} gold {at}{}", gold(last.gold_diff), since("behind")),
        _ => format!("Even lane: {} gold {at}", gold(last.gold_diff)),
    };

    let lane_end = LANE_END as f32 * 60.0;
    let blue = d.players.first().is_some_and(|p| p.ally);
    let players: Vec<usize> = ours.iter().chain(&theirs).copied().collect();
    let events = d
        .kills
        .iter()
        .filter(|k| k.time < lane_end && players.contains(&k.victim))
        .map(|k| LaneEvent {
            time: k.time,
            killer: k.killer.map(|i| d.players[i].champ),
            victim: d.players[k.victim].champ,
            assists: k.assisters.iter().map(|&i| d.players[i].champ).collect(),
            ours: !d.players[k.victim].ally,
            place: k.pos.map(|(x, y)| place(MapZone::at(x, y), blue).into()),
        })
        .collect();

    // First shop visit after the starting items that wasn't a death.
    let backs = players
        .iter()
        .filter_map(|&i| {
            d.frames.windows(2).filter(|w| w[0].time >= 60.0 && w[1].time <= lane_end + 30.0).find_map(|w| {
                let (a, b) = (&w[0].players[i], &w[1].players[i]);
                let spent = (a.current_gold + b.total_gold.saturating_sub(a.total_gold)).saturating_sub(b.current_gold);
                let died = d.kills.iter().any(|k| k.victim == i && k.time > w[0].time && k.time <= w[1].time);
                (spent >= SPENT_MIN && !died).then(|| LaneBack {
                    champ: d.players[i].champ,
                    ally: d.players[i].ally,
                    minute: (w[1].time / 60.0).round() as u32,
                    spent,
                })
            })
        })
        .collect();

    Some(Laning {
        allies: ours.iter().map(|&i| d.players[i].champ).collect(),
        enemies: theirs.iter().map(|&i| d.players[i].champ).collect(),
        verdict,
        turned_at,
        minutes,
        events,
        backs,
    })
}
