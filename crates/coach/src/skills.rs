//! Next skill point along a lolalytics skill order (one ability per level).

use crate::{common, consts};
use model::*;
use std::cmp::Reverse;

const KEYS: [char; 4] = ['Q', 'W', 'E', 'R'];
const MAX_RANK: [u32; 4] = [5, 5, 5, 3];
const R_LEVELS: [u32; 3] = [6, 11, 16];

/// The earliest point of `order` I lack, given my Q W E R ranks and level; after the
/// order runs out, R when my level allows it, then the ability with points left in
/// max order. `source` is left empty. None when the order is empty or not Q/W/E/R,
/// my ranks add up to more than my level (a free rank), no basic ability has a point
/// by `SKILL_NONE_LEVEL` (no points to place) or everything is maxed.
pub fn next(order: &str, have: [u32; 4], level: u32) -> Option<SkillNext> {
    let idx = |c: char| KEYS.iter().position(|&k| k == c);
    let steps: Vec<usize> = order.chars().map(idx).collect::<Option<_>>()?;
    let spent: u32 = have.iter().sum();
    if steps.is_empty() || level == 0 || spent > level || (level >= consts::SKILL_NONE_LEVEL && have[..3] == [0, 0, 0]) {
        return None;
    }
    let priority = priority(&steps);
    let mut count = [0u32; 4];
    let on_order = steps.iter().enumerate().find_map(|(i, &k)| {
        count[k] += 1;
        (count[k] > have[k]).then_some((k, i as u32 + 1))
    });
    let (key, step) = match on_order {
        Some(found) => found,
        None => {
            let point = spent + 1;
            let r = R_LEVELS.get(have[3] as usize).is_some_and(|&l| point >= l);
            let basic = || priority.chars().filter_map(idx).find(|&k| have[k] < MAX_RANK[k] && point > 2 * have[k]);
            (if r { Some(3) } else { basic() }?, point)
        }
    };
    let unspent = level - spent;
    Some(SkillNext {
        key: KEYS[key].to_string(),
        unspent,
        at_level: if unspent > 0 { step } else { level + 1 },
        priority,
        source: String::new(),
    })
}

/// Q, W and E by points in the order, then by the level they got their last one.
fn priority(steps: &[usize]) -> String {
    let n = |k: usize| steps.iter().filter(|&&s| s == k).count();
    let last = |k: usize| steps.iter().rposition(|&s| s == k).unwrap_or(usize::MAX);
    let mut keys = [0, 1, 2];
    keys.sort_by_key(|&k| (Reverse(n(k)), last(k)));
    keys.iter().map(|&k| KEYS[k]).collect()
}

/// Along the matchup page's order against my lane opponent when it has at least
/// `SKILL_MATCHUP_MIN` games, else my build page's.
pub fn advice(snap: &GameSnapshot, sd: &StaticData, stats: Option<&ChampStats>, matchup: Option<&MatchupBuild>) -> Option<SkillNext> {
    let (have, level) = (snap.my_abilities?, snap.players.get(snap.me)?.level);
    let (order, priority, source) = match matchup.filter(|m| m.skill_order_games >= consts::SKILL_MATCHUP_MIN && !m.skill_order.is_empty()) {
        Some(m) => {
            let source = format!("vs {}, {} games", common::champ(sd, m.vs).name, common::fmt_games(m.skill_order_games as u64));
            (&m.skill_order, &m.skills, source)
        }
        None => stats.map(|s| (&s.skill_order, &s.skill_priority, common::lolalytics_source(s)))?,
    };
    let mut n = next(order, have, level)?;
    if priority.len() == 3 {
        n.priority = priority.clone();
    }
    n.source = source;
    Some(n)
}
