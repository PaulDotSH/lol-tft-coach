//! Items from my bench: the best completed items my components make for the target comp's carry
//! builds (MetaTFT `builds`, best first) and who holds them; finished items (loot, carousel) get a
//! holder too. "Slam now" when HP is falling, a power spike is near, or the game is late.

use crate::fit::BUILDS;
use crate::tracker::{Tracker, round_index};
use crate::{item_name, unit_name};
use tft_model::*;

/// An item in the n-th carry build scores BUILD_W / (1 + n), less POS_W per place in that build.
const BUILD_W: f32 = 1.0;
const POS_W: f32 = 0.05;
/// An item only in the comp's most built list.
const LIST_W: f32 = 0.4;
/// Slam when HP fell this much over SLAM_ROUNDS rounds, or is under SLAM_HP.
const SLAM_DROP: u32 = 15;
const SLAM_ROUNDS: u32 = 2;
const SLAM_HP: u32 = 50;
/// Power spikes (level / roll timings, augments): slam on them and the round before.
const SPIKES: [(u32, u32); 5] = [(2, 1), (3, 2), (4, 1), (4, 2), (5, 1)];
/// From this stage on there's nothing left to hold components for.
const SLAM_FROM: u32 = 5;

/// Radiant copies share the base item's api with a "Radiant" suffix.
pub fn same_item(have: &str, want: &str) -> bool {
    have == want || have.strip_suffix("Radiant").map(|s| s.trim_end_matches('_')) == Some(want)
}

/// How much the comp wants the item, and who holds it (the first build that has it).
pub fn want(comp: &MetaComp, item: &str) -> Option<(f32, Option<Api>)> {
    let build = comp.builds.iter().take(BUILDS).enumerate().find_map(|(i, b)| {
        let pos = b.items.iter().position(|w| same_item(item, w))?;
        Some((BUILD_W / (1 + i) as f32 - POS_W * pos as f32, Some(b.unit.clone())))
    });
    build.or_else(|| comp.items.iter().any(|w| same_item(item, w)).then_some((LIST_W, None)))
}

/// Why to make items now; None to hold the components.
fn slam(snap: &TftSnapshot, tracker: &Tracker) -> Option<String> {
    if let Some(hp) = snap.hp.filter(|&h| h < SLAM_HP) {
        return Some(format!("{hp} HP"));
    }
    if let Some(d) = tracker.hp_drop(SLAM_ROUNDS).filter(|&d| d >= SLAM_DROP) {
        return Some(format!("HP falling ({d} lost lately)"));
    }
    let stage = tracker.stage()?;
    if stage.0 >= SLAM_FROM {
        return Some("late game, nothing to wait for".into());
    }
    let now = round_index(stage);
    SPIKES.iter().find(|&&s| (now..=now + 1).contains(&round_index(s))).map(|s| format!("power spike at {}-{}", s.0, s.1))
}

pub fn advise(snap: &TftSnapshot, tracker: &Tracker, set: &TftSetData, comp: Option<&MetaComp>) -> Vec<ItemAdvice> {
    let Some(comp) = comp else { return vec![] };
    let held = tracker.items();
    let why = slam(snap, tracker);
    let holder_text = |h: &Option<Api>| match h {
        Some(u) => format!("best on {} in {}", unit_name(set, u), comp.name),
        None => format!("one of {}'s most built items", comp.name),
    };
    let mut out = Vec::new();
    // Finished items already on the bench: just hand them out.
    for it in held.iter().filter(|i| set.items.get(*i).is_some_and(|d| !d.component)) {
        if let Some((_, holder)) = want(comp, it) {
            let reason = format!("{}: give it now", holder_text(&holder));
            out.push(ItemAdvice { item: it.clone(), from: vec![], holder, now: true, reason });
        }
    }
    // Components: greedily the best item they make, until nothing the comp wants is left.
    let mut pool: Vec<&Api> = held.iter().filter(|i| set.items.get(*i).is_some_and(|d| d.component)).collect();
    loop {
        let best = set
            .items
            .values()
            .filter(|d| d.components.len() == 2 && !d.radiant && !d.artifact)
            .filter(|d| {
                let (a, b) = (&d.components[0], &d.components[1]);
                let n = |c: &Api| pool.iter().filter(|p| **p == c).count();
                if a == b { n(a) >= 2 } else { n(a) >= 1 && n(b) >= 1 }
            })
            .filter_map(|d| want(comp, &d.api).map(|(s, h)| (d, s, h)))
            .max_by(|x, y| x.1.total_cmp(&y.1).then_with(|| y.0.api.cmp(&x.0.api)));
        let Some((d, _, holder)) = best else { break };
        for c in &d.components {
            if let Some(i) = pool.iter().position(|p| *p == c) {
                pool.remove(i);
            }
        }
        let from = d.components.iter().map(|c| item_name(set, c)).collect::<Vec<_>>().join(" + ");
        let reason = match &why {
            Some(w) => format!("{} ({from}); slam now: {w}", holder_text(&holder)),
            None => format!("{} ({from}); can hold the components for now", holder_text(&holder)),
        };
        out.push(ItemAdvice { item: d.api.clone(), from: d.components.clone(), holder, now: why.is_some(), reason });
    }
    out
}
