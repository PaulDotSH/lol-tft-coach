//! Rank the augments on offer: by MetaTFT augment placement stats when there are any, plus a bonus
//! for those the target comp (or an alternative) lists; without stats by the comps' own lists.
//! No data at all: no ranking.

use crate::fit::{PLACE_MID, shrunk};
use tft_model::*;

/// Games of PLACE_MID an augment's average is shrunk with.
const AUG_PRIOR: f32 = 100.0;
/// Bonus when the target comp lists the augment; an alternative's list counts ALT_SHARE of it.
const COMP_AUG: f32 = 0.3;
const ALT_SHARE: f32 = 0.5;
/// Without stats: the list's first augment scores LIST_TOP, each later one LIST_STEP less.
const LIST_TOP: f32 = 1.0;
const LIST_STEP: f32 = 0.1;

pub fn rank(offer: &[Api], meta: &TftMeta, comps: &[&MetaComp]) -> Vec<AugmentRank> {
    let stats = !meta.augments.is_empty();
    let mut out: Vec<AugmentRank> = offer
        .iter()
        .filter_map(|a| {
            let listed = comps.iter().enumerate().find_map(|(i, c)| c.augments.iter().position(|x| x == a).map(|p| (i, p, c)));
            let share = |i: usize| if i == 0 { 1.0 } else { ALT_SHARE };
            let good_in = listed.map(|(_, _, c)| format!("good in {}", c.name));
            let (score, reason) = if stats {
                let stat = meta.augments.iter().find(|s| &s.api == a);
                if stat.is_none() && listed.is_none() {
                    return None;
                }
                let base = stat.map_or(0.0, |s| PLACE_MID - shrunk(s.avg_place, s.games, AUG_PRIOR));
                let bonus = listed.map_or(0.0, |(i, _, _)| COMP_AUG * share(i));
                let mut why: Vec<String> =
                    stat.map(|s| format!("avg place {:.2} over {} games", s.avg_place, s.games)).into_iter().collect();
                why.extend(good_in);
                (base + bonus, why.join("; "))
            } else {
                let (i, p, _) = listed?;
                ((LIST_TOP - LIST_STEP * p as f32).max(LIST_STEP) * share(i), good_in.unwrap_or_default())
            };
            Some(AugmentRank { augment: a.clone(), score, reason })
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out
}
