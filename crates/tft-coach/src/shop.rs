//! Buy or skip each shop slot: units of the target comp, ★ upgrades, backup comps while
//! there's still time to pivot, early pairs; with how many copies other players hold and how
//! many are left in the pool.

use crate::fit::contested;
use crate::scout::{Scout, pool};
use crate::tracker::{STAR2_COPIES, STAR3_COPIES, Tracker};
use crate::unit_name;
use tft_model::*;

/// Units of an alternative comp are still bought up to this stage (room to pivot).
const ALT_UNTIL: u32 = 3;
/// Off-comp ★2 upgrades are worth it up to this stage (board strength for streaks).
const UPGRADE_UNTIL: u32 = 3;
/// Pairs of any unit hold the early board up to this stage.
const EARLY_UNTIL: u32 = 2;

pub fn advise(snap: &TftSnapshot, tracker: &Tracker, set: &TftSetData, comps: &[&MetaComp]) -> Vec<ShopAdvice> {
    let stage_at_most = |s: u32| tracker.stage().is_some_and(|st| st.0 <= s);
    snap.shop
        .iter()
        .enumerate()
        .filter_map(|(slot, u)| {
            let u = u.as_ref()?;
            let name = unit_name(set, u);
            let copies = tracker.copies(u);
            let star_up = match copies + 1 {
                STAR2_COPIES => Some(2),
                STAR3_COPIES => Some(3),
                _ => None,
            };
            let target = comps.first().filter(|c| c.units.contains(u));
            let alt = comps.iter().skip(1).find(|c| c.units.contains(u));
            let contest = target.or(alt).map(|c| contested(u, c, set, tracker));
            let (mut buy, mut reason) = if let Some(c) = target {
                (true, star_up.map_or_else(|| format!("for {}", c.name), |s| format!("makes ★{s} {name}")))
            } else if star_up.is_some_and(|s| s == 3 || stage_at_most(UPGRADE_UNTIL)) {
                (true, format!("makes ★{} {name}", star_up.unwrap_or(2)))
            } else if let Some(c) = alt.filter(|_| stage_at_most(ALT_UNTIL)) {
                (true, format!("for {} (backup)", c.name))
            } else if copies > 0 && stage_at_most(EARLY_UNTIL) {
                (true, format!("pair toward ★2 {name}"))
            } else {
                (false, "not in your comp".to_string())
            };
            if let Some(c) = contest.as_ref().filter(|c| c.boards > 0) {
                reason = format!("{reason}, {}", Scout::say(c));
            }
            let cost = set.units.get(u).map(|d| d.cost);
            if buy
                && let (Some(cost), Some(gold)) = (cost, snap.gold)
                && cost > gold
            {
                buy = false;
                reason = format!("{reason}, but it costs {cost} (you have {gold})");
            }
            let held = tracker.scout().held(u);
            let left = cost.and_then(pool).map(|n| n.saturating_sub(held + copies));
            Some(ShopAdvice { slot: slot as u32, unit: u.clone(), buy, reason, contested: contest.map_or(0, |c| c.boards), held, left })
        })
        .collect()
}
