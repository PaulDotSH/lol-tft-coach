//! Comp fit: how well each MetaTFT comp suits what I hold right now (units by ★, traits on the
//! panel, components for its carry items, minus units other players hold), tie-broken by its
//! average placement. The last target stays unless another comp clearly beats it (more so later).

use crate::CoachInput;
use crate::tracker::{Tracker, star};
use tft_model::*;

/// Each owned comp unit; every ★ above 1 adds STAR_W more.
const UNIT_W: f32 = 1.0;
const STAR_W: f32 = 0.5;
/// Share of the comp's traits already on my panel.
const TRAIT_W: f32 = 0.3;
/// Share of my held components that build the comp's items.
const ITEM_W: f32 = 0.25;
/// Per other player holding a comp unit, spread over the comp's units.
const CONTEST_W: f32 = 0.3;
/// Per place the (shrunk) average is better than PLACE_MID.
const PLACE_W: f32 = 0.15;
/// The middle placement: stats are shrunk toward it.
pub const PLACE_MID: f32 = 4.5;
/// Games of PLACE_MID a comp's average is shrunk with, so a few lucky games don't top the list.
pub const PRIOR_GAMES: f32 = 200.0;
/// The last target stays first while within this of the best score: a little early, more once
/// the board is built around it (one comp unit is worth 1/units, about 0.12).
pub fn keep_margin(stage: Option<(u32, u32)>) -> f32 {
    match stage.map_or(1, |s| s.0) {
        ..=2 => 0.04,
        3 => 0.1,
        _ => 0.2,
    }
}
/// Comps shown: the target and two alternatives.
pub const TOP_COMPS: usize = 3;
/// Carry builds whose items count for a comp.
pub const BUILDS: usize = 3;

/// Average placement shrunk toward PLACE_MID by PRIOR_GAMES.
pub fn shrunk(avg: f32, games: u32, prior: f32) -> f32 {
    (avg * games as f32 + PLACE_MID * prior) / (games as f32 + prior)
}

/// Items the comp builds: its first carry builds, then its most built list.
pub fn wanted_items(comp: &MetaComp) -> impl Iterator<Item = &Api> {
    comp.builds.iter().take(BUILDS).flat_map(|b| &b.items).chain(&comp.items)
}

/// How other players contest `unit` of `comp`: who holds it (named, else by trait panel),
/// copies they hold and copies left in the pool (see `Scout::contest`).
pub fn contested(unit: &str, comp: &MetaComp, set: &TftSetData, tracker: &Tracker) -> Contest {
    tracker.scout().contest(unit, comp, set, tracker.copies(unit))
}

/// How much the comp suits me now; higher is better.
pub fn score(comp: &MetaComp, input: &CoachInput) -> f32 {
    let CoachInput { tracker, set, hooks, .. } = *input;
    let n = comp.units.len().max(1) as f32;
    let units: f32 = comp
        .units
        .iter()
        .map(|u| match tracker.copies(u) {
            0 => 0.0,
            c => UNIT_W + STAR_W * (star(c) - 1) as f32,
        })
        .sum::<f32>()
        / n;
    let traits = trait_share(comp, tracker);
    let held: Vec<&Api> = tracker.items().iter().filter(|i| set.items.get(*i).is_some_and(|d| d.component)).collect();
    let builds_from: Vec<&Api> = wanted_items(comp).filter_map(|i| set.items.get(i)).flat_map(|d| &d.components).collect();
    let items = held.iter().filter(|c| builds_from.contains(c)).count() as f32 / held.len().max(1) as f32;
    let contest = comp.units.iter().map(|u| tracker.scout().holders(u, comp, set)).sum::<u32>() as f32 / n;
    let place = PLACE_MID - shrunk(comp.avg_place, comp.games, PRIOR_GAMES);
    units + TRAIT_W * traits + ITEM_W * items - CONTEST_W * contest + PLACE_W * place + hooks.comp_bonus(comp, input)
}

/// How much of the comp's traits my panel already shows, 0..=1.
fn trait_share(comp: &MetaComp, tracker: &Tracker) -> f32 {
    let panel = tracker.panel();
    comp.traits
        .iter()
        .map(|(t, want)| panel.iter().find(|p| &p.0 == t).map_or(0, |p| p.1).min(*want) as f32 / (*want).max(1) as f32)
        .sum::<f32>()
        / comp.traits.len().max(1) as f32
}

/// Every comp scored, best first; the last target (`input.keep`) stays first within `keep_margin`.
pub fn rank<'m>(input: &CoachInput<'m>) -> Vec<(&'m MetaComp, f32)> {
    let mut v: Vec<(&MetaComp, f32)> = input.meta.comps.iter().map(|c| (c, score(c, input))).collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.avg_place.total_cmp(&b.0.avg_place)));
    if let Some(best) = v.first().map(|c| c.1)
        && let Some(i) = v.iter().position(|(c, s)| Some(c.id.as_str()) == input.keep && *s >= best - keep_margin(input.tracker.stage()))
    {
        let kept = v.remove(i);
        v.insert(0, kept);
    }
    v
}

/// The comp as shown: which of its units I have and which are missing. Fit is the share of
/// its units I have, or of its traits my panel shows when that's higher (units bought
/// before tracking started are only seen through traits).
pub fn pick(comp: &MetaComp, input: &CoachInput) -> CompPick {
    let (have, missing): (Vec<Api>, Vec<Api>) = comp.units.iter().cloned().partition(|u| input.tracker.copies(u) > 0);
    CompPick {
        comp: comp.id.clone(),
        name: comp.name.clone(),
        fit: (have.len() as f32 / comp.units.len().max(1) as f32).max(trait_share(comp, input.tracker)),
        avg_place: comp.avg_place,
        have,
        missing,
        levelling: comp.levelling.clone(),
    }
}
