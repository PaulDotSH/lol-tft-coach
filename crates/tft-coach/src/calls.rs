//! Key moments, spoken once per key by the app: level now, roll down now (both from econ), a free
//! slot on my board in planning, a key unit of the target comp contested, take the best augment on
//! offer. Keys name the situation only (never a count or a time), so they stay the same every tick
//! while it lasts.

use crate::scout::Scout;
use crate::{augment_name, unit_name};
use tft_model::*;

/// A key unit held by this many other players is called out.
const CONTEST_CALL: u32 = 2;
/// The best augment is called when it leads the next one by this much.
const AUGMENT_LEAD: f32 = 0.05;

pub fn calls(advice: &TftAdvice, econ: Option<TftCall>, free: Option<(u32, u32)>, set: &TftSetData, target: Option<&MetaComp>) -> Vec<TftCall> {
    let mut out: Vec<TftCall> = econ.into_iter().collect();
    if let Some(slot) = free {
        out.push(free_slot(advice, slot, set, target));
    }
    if let Some(comp) = target {
        // Key units: carries first, then the ones it wants upgraded.
        let keys: Vec<&Api> = comp.builds.iter().map(|b| &b.unit).chain(&comp.stars).collect();
        for c in advice.contested.iter().filter(|c| c.boards >= CONTEST_CALL && keys.contains(&&c.unit)) {
            let name = unit_name(set, &c.unit);
            let n = c.boards;
            out.push(TftCall {
                key: format!("contested-{}", c.unit),
                text: format!("{name}: {}: consider a pivot from {}", Scout::say(c), comp.name),
                say: match c.left.filter(|_| c.held > 0) {
                    Some(left) => format!("{name} is contested: {left} left."),
                    None => format!("{name} is contested by {n}."),
                },
            });
        }
    }
    if let [best, rest @ ..] = advice.augments.as_slice()
        && rest.first().is_none_or(|next| best.score - next.score >= AUGMENT_LEAD)
    {
        let name = augment_name(set, &best.augment);
        out.push(TftCall {
            key: format!("augment-{}", best.augment),
            text: format!("Take {name}: {}", best.reason),
            say: format!("Take {name}."),
        });
    }
    out
}

/// A free board slot (N < M): the bench unit to put on, the target comp's first, then by ★ and
/// cost. Keyed by the cap, so each new level's slot is called once.
fn free_slot(advice: &TftAdvice, (n, cap): (u32, u32), set: &TftSetData, target: Option<&MetaComp>) -> TftCall {
    let cost = |api: &str| set.units.get(api).map_or(0, |u| u.cost);
    let best = advice
        .owned
        .iter()
        .filter(|u| u.bench > 0 && !u.api.is_empty())
        .max_by_key(|u| (target.is_some_and(|c| c.units.contains(&u.api)), u.star, cost(&u.api)));
    let key = format!("free-slot-{cap}");
    match best {
        Some(u) => {
            let name = unit_name(set, &u.api);
            TftCall { key, text: format!("Free slot on your board ({n}/{cap}): put {name} on"), say: format!("Free slot: put {name} on.") }
        }
        None => TftCall { key, text: format!("Free slot on your board ({n}/{cap}): field a unit"), say: "Free slot on your board.".into() },
    }
}
