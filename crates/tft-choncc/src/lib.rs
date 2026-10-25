//! Choncc or Treat (queue 1230, 2026-10-21 to 2026-11-17): mode rules, comps by
//! starting carry and loot priorities, as `tft_coach::ModeHooks`. Delete this crate
//! and its one line in `src-tauri/src/tft.rs` when the event ends.
//!
//! Rules the advice relies on: everyone starts with one 4- or 5-cost unit; a loot orb drops after
//! each carousel; Choncc's Crown (+2) and Chalice (+1) raise the team size (the snapshot's
//! `team_size` already shows the real cap); a comeback Golden Orb drops under 50 HP.

use std::collections::HashSet;
use tft_coach::tracker::{EARLY_STAGE, Tracker};
use tft_coach::{CoachInput, ModeHooks, fit, item_name, target, trait_name, unit_name};
use tft_model::*;

/// The starting unit costs at least this.
pub const STARTER_COST: u32 = 4;
/// Fit bonus for comps that use the starting unit: about one owned unit's worth, so
/// the board still decides (the starter is a guess from the first trait panel).
const STARTER_BONUS: f32 = 0.1;
/// The comeback Golden Orb drops under this HP.
pub const GOLDEN_ORB_HP: u32 = 50;
/// Comps listed for the starter.
const STARTER_COMPS: usize = 3;

pub struct ChonccHooks;

/// The 4/5-cost unit I started with: the first tracked one owned in stage 1/2, else the one whose
/// traits all show on my first trait panel (most traits matched, only when unambiguous).
pub fn starter(tracker: &Tracker, set: &TftSetData) -> Option<Api> {
    let big = |api: &str| set.units.get(api).is_some_and(|u| u.cost >= STARTER_COST && u.cost <= 5);
    if let Some(u) = tracker.units().iter().find(|u| big(&u.api) && u.since.is_some_and(|s| s.0 <= EARLY_STAGE)) {
        return Some(u.api.clone());
    }
    let (_, panel) = tracker.first_panel()?;
    let shown: HashSet<&Api> = panel.iter().filter(|p| p.1 > 0).map(|p| &p.0).collect();
    let mut fits: Vec<&TftUnit> =
        set.units.values().filter(|u| big(&u.api) && !u.traits.is_empty() && u.traits.iter().all(|t| shown.contains(t))).collect();
    fits.sort_by_key(|u| std::cmp::Reverse(u.traits.len()));
    match fits.as_slice() {
        [one] => Some(one.api.clone()),
        [a, b, ..] if a.traits.len() > b.traits.len() => Some(a.api.clone()),
        _ => None,
    }
}

/// Loot to use first: radiant > artifact > emblem / spatula for the target comp > components.
pub fn loot(items: &[Api], set: &TftSetData, comp: Option<&MetaComp>) -> Vec<String> {
    let carry = comp.and_then(|c| c.builds.first()).map(|b| unit_name(set, &b.unit));
    let on = |what: &str| carry.map_or(format!("{what}: on your carry"), |c| format!("{what}: on {c}"));
    let comp_trait = |it: &TftItem| {
        comp?.traits.iter().map(|t| &t.0).find(|t| it.api.starts_with(t.as_str()) || it.name.contains(trait_name(set, t)))
    };
    let spatula = |it: &TftItem| it.api.contains("Spatula") || it.api.contains("FryingPan");
    let mut lines: Vec<(u32, String)> = items
        .iter()
        .filter_map(|api| {
            let it = set.items.get(api)?;
            let name = item_name(set, api);
            Some(if it.radiant {
                (0, on(&format!("{name} (radiant)")))
            } else if it.artifact {
                (1, on(&format!("{name} (artifact)")))
            } else if it.emblem && let Some(t) = comp_trait(it) {
                (2, format!("{name}: adds {} for {}", trait_name(set, t), comp.map_or("", |c| c.name.as_str())))
            } else if spatula(it) {
                let t = comp.and_then(|c| c.traits.first()).map_or("your main trait", |t| trait_name(set, &t.0));
                (3, format!("{name}: make a {t} emblem"))
            } else if it.component {
                (4, format!("{name}: component"))
            } else {
                (5, format!("{name}: spare item"))
            })
        })
        .collect();
    lines.sort_by_key(|l| l.0);
    let rule = match comp {
        Some(c) => format!("Loot orbs: radiant > artifact > emblem for {} > components", c.name),
        None => "Loot orbs: radiant > artifact > emblem > components".to_string(),
    };
    std::iter::once(rule).chain(lines.into_iter().map(|l| l.1)).collect()
}

impl ModeHooks for ChonccHooks {
    fn mode(&self) -> TftMode {
        TftMode::Choncc
    }

    fn comp_bonus(&self, comp: &MetaComp, input: &CoachInput) -> f32 {
        match starter(input.tracker, input.set) {
            Some(s) if comp.units.contains(&s) => STARTER_BONUS,
            _ => 0.0,
        }
    }

    fn adjust(&self, advice: &mut TftAdvice, input: &CoachInput) {
        let CoachInput { snap, tracker, set, meta, .. } = *input;
        let starter = starter(tracker, set);
        let mut comps: Vec<(&MetaComp, f32)> = match &starter {
            Some(s) => meta.comps.iter().filter(|c| c.units.contains(s)).map(|c| (c, fit::score(c, input))).collect(),
            None => vec![],
        };
        comps.sort_by(|a, b| b.1.total_cmp(&a.1));
        let target = target(advice, meta);
        let mut notes = Vec::new();
        match (&starter, tracker.stage()) {
            (Some(s), _) => notes.push(format!("Start: {}, build around it", unit_name(set, s))),
            (None, Some(st)) if st.0 <= EARLY_STAGE => notes.push("Field your starting 4/5-cost so the trait panel shows it".into()),
            _ => {}
        }
        if let Some((on, cap)) = snap.team_size
            && on < cap
        {
            let open = cap - on;
            notes.push(format!("Field {cap} units: {open} slot{} open", if open == 1 { "" } else { "s" }));
            if snap.level.is_some_and(|l| cap > l) {
                advice.calls.push(TftCall {
                    key: format!("field-{cap}"),
                    text: format!("Team size {cap} with Choncc's loot: field {open} more"),
                    say: format!("You can field {cap}. Add {open} more."),
                });
            }
        }
        if let Some(hp) = snap.hp.filter(|&h| h < GOLDEN_ORB_HP) {
            notes.push(format!("{hp} HP: the comeback Golden Orb (radiant, reforger, gold) is yours, spend it on your board"));
        }
        notes.push("A loot orb drops after each carousel".into());
        advice.choncc = Some(ChonccAdvice {
            starter,
            comps: comps.into_iter().take(STARTER_COMPS).map(|(c, _)| fit::pick(c, input)).collect(),
            loot: loot(tracker.items(), set, target),
            notes,
        });
    }
}
