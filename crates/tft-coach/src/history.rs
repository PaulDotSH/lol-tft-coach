//! Outside the game: top comps before it (Choncc: by starting carry), the comp a past board
//! matches, a post-game review against that comp, and trends per mode.

use crate::econ::{LEVEL_AT, levelling};
use crate::fit::{PRIOR_GAMES, shrunk};
use crate::items::same_item;
use crate::{item_name, unit_name};
use std::collections::HashSet;
use tft_model::*;

/// Comps shown before the game.
const PREGAME_TOP: usize = 8;
/// A past board matches a comp from this similarity on.
const MATCH_MIN: f32 = 0.35;
/// Similarity: weight of the unit overlap; the rest is the trait overlap.
const MATCH_UNITS: f32 = 0.7;
/// Placing this much better or worse than the comp's average is worth saying.
const PLACE_GAP: f32 = 1.0;
/// Placements counted as a top 4.
const TOP4: u32 = 4;
/// Upgrade lines in a review at most.
const STAR_LINES: usize = 2;

fn jaccard(a: &HashSet<&str>, b: &HashSet<&str>) -> f32 {
    a.intersection(b).count() as f32 / a.union(b).count().max(1) as f32
}

fn ordinal(n: u32) -> String {
    let suffix = match n {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// Top comps for the queue by (shrunk) average placement; with a starting unit, the comps that
/// use it (all comps when none does).
pub fn pregame(meta: &TftMeta, _set: &TftSetData, starter: Option<&str>) -> Vec<CompPick> {
    let mut comps: Vec<&MetaComp> = meta.comps.iter().collect();
    if let Some(s) = starter
        && comps.iter().any(|c| c.units.iter().any(|u| u == s))
    {
        comps.retain(|c| c.units.iter().any(|u| u == s));
    }
    comps.sort_by(|a, b| shrunk(a.avg_place, a.games, PRIOR_GAMES).total_cmp(&shrunk(b.avg_place, b.games, PRIOR_GAMES)));
    comps
        .into_iter()
        .take(PREGAME_TOP)
        .map(|c| {
            let (have, missing): (Vec<Api>, Vec<Api>) = c.units.iter().cloned().partition(|u| Some(u.as_str()) == starter);
            CompPick {
                comp: c.id.clone(),
                name: c.name.clone(),
                fit: have.len() as f32 / c.units.len().max(1) as f32,
                avg_place: c.avg_place,
                have,
                missing,
                levelling: c.levelling.clone(),
            }
        })
        .collect()
}

fn similarity(game: &TftPastGame, comp: &MetaComp) -> f32 {
    let mine: HashSet<&str> = game.units.iter().map(|u| u.api.as_str()).collect();
    let theirs: HashSet<&str> = comp.units.iter().map(|u| u.as_str()).collect();
    let my_traits: HashSet<&str> = game.traits.iter().filter(|t| t.2 > 0).map(|t| t.0.as_str()).collect();
    let comp_traits: HashSet<&str> = comp.traits.iter().map(|t| t.0.as_str()).collect();
    MATCH_UNITS * jaccard(&mine, &theirs) + (1.0 - MATCH_UNITS) * jaccard(&my_traits, &comp_traits)
}

/// The nearest MetaTFT comp to a past board (units, then active traits), if any is close enough.
pub fn match_comp(game: &TftPastGame, meta: &TftMeta) -> Option<String> {
    meta.comps
        .iter()
        .map(|c| (c, similarity(game, c)))
        .filter(|(_, s)| *s >= MATCH_MIN)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, _)| c.id.clone())
}

fn comp_of<'m>(game: &TftPastGame, meta: &'m TftMeta) -> Option<&'m MetaComp> {
    let id = game.comp.clone().filter(|id| meta.comps.iter().any(|c| &c.id == id)).or_else(|| match_comp(game, meta))?;
    meta.comps.iter().find(|c| c.id == id)
}

/// The comp my board matched, my placement against its average, carry items against its best
/// build, final level against its levelling, and units it wants upgraded that weren't.
pub fn review(game: &TftPastGame, meta: &TftMeta, set: &TftSetData) -> TftReview {
    let Some(comp) = comp_of(game, meta) else {
        let takeaways = vec![format!("{} with a board no meta comp matches: try a listed comp next game", ordinal(game.placement))];
        return TftReview { game: game.clone(), comp: None, takeaways };
    };
    let on_board = |u: &str| game.units.iter().find(|x| x.api == u);
    let (have, missing): (Vec<Api>, Vec<Api>) = comp.units.iter().cloned().partition(|u| on_board(u).is_some());
    let pick = CompPick {
        comp: comp.id.clone(),
        name: comp.name.clone(),
        fit: have.len() as f32 / comp.units.len().max(1) as f32,
        avg_place: comp.avg_place,
        have,
        missing,
        levelling: comp.levelling.clone(),
    };
    let mut out = Vec::new();
    let place = game.placement as f32;
    let (p, avg, name) = (ordinal(game.placement), comp.avg_place, &comp.name);
    out.push(if place <= avg - PLACE_GAP {
        format!("{p} with {name}: better than its {avg:.1} average")
    } else if place >= avg + PLACE_GAP {
        format!("{p} with {name}: worse than its {avg:.1} average")
    } else {
        format!("{p} with {name}: about its {avg:.1} average")
    });
    // Carry items against the best build of a carry I had.
    let names = |v: &[Api]| v.iter().map(|i| item_name(set, i)).collect::<Vec<_>>().join(", ");
    match comp.builds.iter().find_map(|b| on_board(&b.unit).map(|u| (b, u))) {
        Some((b, u)) => {
            let carry = unit_name(set, &b.unit);
            let lacking: Vec<Api> = b.items.iter().filter(|w| !u.items.iter().any(|h| same_item(h, w))).cloned().collect();
            if lacking.is_empty() {
                out.push(format!("{carry} had the best build"));
            } else if u.items.is_empty() {
                out.push(format!("{carry} had no items; the best build is {}", names(&b.items)));
            } else {
                out.push(format!("{carry} had {}; the best build is {}", names(&u.items), names(&b.items)));
            }
        }
        None => {
            if let Some(b) = comp.builds.first() {
                out.push(format!("{}, the comp's carry, wasn't on your board", unit_name(set, &b.unit)));
            }
        }
    }
    // Final level against the comp's.
    if let Some(lvl) = levelling(comp)
        && game.level < lvl
    {
        let by = LEVEL_AT.iter().find(|l| l.0 == lvl).map_or(String::new(), |l| format!(" (by {}-{})", l.1.0, l.1.1));
        out.push(format!("You ended at level {}; {name} plays at {lvl}{by}", game.level));
    }
    for s in comp.stars.iter().filter_map(|s| on_board(s)).filter(|u| u.star < 2).take(STAR_LINES) {
        out.push(format!("{} was ★{}; the comp wants it ★2", unit_name(set, &s.api), s.star));
    }
    TftReview { game: game.clone(), comp: Some(pick), takeaways: out }
}

fn record(comp: String, name: String, places: &[u32]) -> CompRecord {
    let n = places.len().max(1) as f32;
    CompRecord {
        comp,
        name,
        games: places.len() as u32,
        avg_place: places.iter().sum::<u32>() as f32 / n,
        top4: places.iter().filter(|&&p| p <= TOP4).count() as f32 / n,
    }
}

/// Per mode (Standard and Choncc apart, only modes I played): average placement, top-4 rate and
/// each comp played (most games first; boards matching no comp as "Other").
pub fn trends(games: &[TftPastGame], meta: &TftMeta) -> Vec<TftTrends> {
    [TftMode::Standard, TftMode::Choncc]
        .into_iter()
        .filter_map(|mode| {
            let mine: Vec<&TftPastGame> = games.iter().filter(|g| g.mode == mode).collect();
            if mine.is_empty() {
                return None;
            }
            let places: Vec<u32> = mine.iter().map(|g| g.placement).collect();
            let total = record(String::new(), String::new(), &places);
            let mut by: Vec<(String, Vec<u32>)> = Vec::new();
            for g in &mine {
                let id = comp_of(g, meta).map_or(String::new(), |c| c.id.clone());
                match by.iter_mut().find(|b| b.0 == id) {
                    Some(b) => b.1.push(g.placement),
                    None => by.push((id, vec![g.placement])),
                }
            }
            let mut comps: Vec<CompRecord> = by
                .into_iter()
                .map(|(id, p)| {
                    let name = meta.comps.iter().find(|c| c.id == id).map_or("Other".to_string(), |c| c.name.clone());
                    record(id, name, &p)
                })
                .collect();
            comps.sort_by(|a, b| b.games.cmp(&a.games).then(a.avg_place.total_cmp(&b.avg_place)));
            Some(TftTrends { mode, games: total.games, avg_place: total.avg_place, top4: total.top4, comps })
        })
        .collect()
}
