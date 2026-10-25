//! Key moments, spoken once per key by the app: level now, roll down now (both from econ), a key
//! unit of the target comp contested, take the best augment on offer. Keys name the situation
//! only (never a count or a time), so they stay the same every tick while it lasts.

use crate::{augment_name, unit_name};
use tft_model::*;

/// A key unit held by this many scouted boards is called out.
const CONTEST_CALL: u32 = 2;
/// The best augment is called when it leads the next one by this much.
const AUGMENT_LEAD: f32 = 0.05;

pub fn calls(advice: &TftAdvice, econ: Option<TftCall>, set: &TftSetData, target: Option<&MetaComp>) -> Vec<TftCall> {
    let mut out: Vec<TftCall> = econ.into_iter().collect();
    if let Some(comp) = target {
        // Key units: carries first, then the ones it wants upgraded.
        let keys: Vec<&Api> = comp.builds.iter().map(|b| &b.unit).chain(&comp.stars).collect();
        for (u, n) in advice.contested.iter().filter(|(u, n)| *n >= CONTEST_CALL && keys.contains(&u)) {
            let name = unit_name(set, u);
            out.push(TftCall {
                key: format!("contested-{u}"),
                text: format!("{name} is contested by {n} boards: consider a pivot from {}", comp.name),
                say: format!("{name} is contested by {n}."),
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
