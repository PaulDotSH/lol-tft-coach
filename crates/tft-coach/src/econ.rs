//! Level / roll / save / hold from stage, gold, level, XP, HP and streak, by the target comp's
//! `levelling`: "lvl 7" and below are reroll comps (roll at that level, slow roll above 50),
//! "lvl 8" / "lvl 9" go fast and roll down at their timing.

use crate::tracker::{Tracker, XP_COST, round_index};
use tft_model::*;

/// Standard timings: the level to be at by each (stage, round).
pub const LEVEL_AT: [(u32, (u32, u32)); 6] = [(4, (2, 1)), (5, (2, 5)), (6, (3, 2)), (7, (4, 1)), (8, (4, 2)), (9, (5, 1))];
/// Comps rolling at this level or lower are reroll comps.
pub const REROLL_MAX: u32 = 7;
/// When a reroll comp rolls, by its level.
const REROLL_AT: [(u32, (u32, u32)); 3] = [(5, (3, 1)), (6, (3, 2)), (7, (4, 1))];
/// Reroll comps level on to 8 from here (their units should be upgraded by then).
const REROLL_EIGHT_AT: (u32, u32) = (5, 1);
/// Roll down at or below this HP.
pub const ROLL_HP: u32 = 40;
/// Gold left after a roll-down: below it there is nothing to roll.
const ROLL_FLOOR: u32 = 10;
/// Max interest from here; reroll comps slow roll the gold above it.
pub const MAX_INTEREST_GOLD: u32 = 50;
/// A roll-down timing lasts this many rounds.
const ROLL_ROUNDS: u32 = 2;
/// A streak this long is worth keeping.
const STREAK_KEEP: i32 = 3;
/// Loss streaks are only kept above this HP.
const STREAK_SAFE_HP: u32 = 50;
/// The comp's level-up timing is mentioned when this many rounds away or fewer.
const TIMING_SOON: u32 = 3;
/// Level when the comp names none (or there is no target): a standard fast-8 comp.
const DEFAULT_LEVEL: u32 = 8;

/// The level in MetaTFT's "lvl 8".
pub fn levelling(comp: &MetaComp) -> Option<u32> {
    let s = comp.levelling.as_deref()?;
    s.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

/// (comp level, reroll?, when it rolls).
fn plan(comp: Option<&MetaComp>) -> (u32, bool, (u32, u32)) {
    let lvl = comp.and_then(levelling).unwrap_or(DEFAULT_LEVEL).clamp(5, 10);
    let reroll = lvl <= REROLL_MAX;
    let table: &[(u32, (u32, u32))] = if reroll { &REROLL_AT } else { &LEVEL_AT };
    let at = table.iter().find(|l| l.0 == lvl).map_or((5, 1), |l| l.1);
    (lvl, reroll, at)
}

/// The level to be at now.
fn wanted(stage: (u32, u32), lvl: u32, reroll: bool) -> u32 {
    let cap = if reroll && stage >= REROLL_EIGHT_AT { lvl.max(8) } else { lvl };
    LEVEL_AT.iter().filter(|l| l.1 <= stage).map(|l| l.0).max().unwrap_or(0).min(cap)
}

fn at((s, r): (u32, u32)) -> String {
    format!("{s}-{r}")
}

fn call(key: String, text: &str, say: &str) -> Option<TftCall> {
    Some(TftCall { key, text: text.into(), say: say.into() })
}

pub fn advise(snap: &TftSnapshot, tracker: &Tracker, comp: Option<&MetaComp>) -> (Option<EconAdvice>, Option<TftCall>) {
    let (Some(stage), Some(gold)) = (tracker.stage(), snap.gold) else { return (None, None) };
    let (lvl, reroll, roll_at) = plan(comp);
    let style = if reroll { format!("reroll comp at {lvl}") } else { format!("lvl {lvl} comp") };
    let advice = |action, text: String| Some(EconAdvice { action, text });

    let level = tracker.level(snap);
    if let Some(level) = level {
        let want = wanted(stage, lvl, reroll);
        let xp = snap.xp.map_or(XP_COST, |(have, need)| need.saturating_sub(have).div_ceil(XP_COST).max(1) * XP_COST);
        if level < want && gold >= xp {
            let text = format!("Level to {want} now: {} timing for a {style}", at(stage));
            return (advice(EconAction::Level, text.clone()), call(format!("level-{want}"), &text, &format!("Level to {want} now.")));
        }
    }
    if let Some(hp) = snap.hp.filter(|&h| h <= ROLL_HP)
        && gold > ROLL_FLOOR
    {
        let text = format!("Roll down now: {hp} HP, stabilize your board");
        return (advice(EconAction::Roll, text.clone()), call("roll-hp".into(), &text, "Roll down now."));
    }
    let leveled = level.is_some_and(|l| l >= lvl);
    let (now, from) = (round_index(stage), round_index(roll_at));
    if leveled && (from..from + ROLL_ROUNDS).contains(&now) && gold > ROLL_FLOOR {
        let text = format!("Roll down at {lvl}: {} is your comp's spike", at(roll_at));
        return (advice(EconAction::Roll, text.clone()), call(format!("roll-{lvl}"), &text, "Roll down now."));
    }
    if reroll && leveled && now >= from && gold > MAX_INTEREST_GOLD {
        let text = format!("Slow roll: roll the {} gold above {MAX_INTEREST_GOLD} for ★ upgrades", gold - MAX_INTEREST_GOLD);
        return (advice(EconAction::Roll, text), None);
    }

    let streak = match snap.streak {
        Some(s) if s >= STREAK_KEEP => format!("; keep the {s}-win streak"),
        Some(s) if s <= -STREAK_KEEP && snap.hp.is_some_and(|h| h > STREAK_SAFE_HP) => format!("; keep the {}-loss streak", -s),
        _ => String::new(),
    };
    let soon = !reroll && now < from && from - now <= TIMING_SOON;
    let next = if soon { format!("; level to {lvl} at {}", at(roll_at)) } else { String::new() };
    if gold >= MAX_INTEREST_GOLD {
        let text = format!("Hold {MAX_INTEREST_GOLD} for max interest: spend only the gold above it{next}{streak}");
        return (advice(EconAction::Hold, text), None);
    }
    let i = interest(gold);
    let more = (i + 1) * 10 - gold;
    let text = format!("Save: {gold} gold, +{i} interest; {more} more for +{}{next}{streak}", i + 1);
    (advice(EconAction::Save, text), None)
}
