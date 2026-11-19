//! Lines about exactly my champion against one lane opponent: rules pairing both kits' tags
//! (`kits.toml`, plus what Data Dragon, traits and the champion notes say) and the numbers
//! of the lolalytics matchup page. Facts and options; the hand-written pair note stays on top.

use crate::{common::*, consts::*};
use model::*;

const MAX_KIT_LINES: usize = 4;
/// Attack range up to this is melee.
const MELEE_RANGE: f32 = 300.0;
/// Magic share of a champion's damage: physical up to the first, magic from the second.
const PHYSICAL_MAX: f32 = 0.3;
const MAGIC_MIN: f32 = 0.7;
const TANK_MIN: f32 = 0.6;
/// Item rows shown per table, each from this share of the matchup's games and this many games.
const ROWS: usize = 4;
const ROW_MIN_SHARE: f32 = 0.05;
const ROW_MIN_GAMES: u32 = 50;
/// Win rate against my usual one is said from this many points.
const USUAL_MIN_DIFF: f32 = 1.0;
/// The highest win rate max order is named from this many games and points over the most played.
const SKILL_MIN_GAMES: u32 = 100;
const SKILL_MIN_DIFF: f32 = 2.0;
/// Win rate by game length: each part from this many games, said when the parts differ by this much.
const LENGTH_MIN_GAMES: u32 = 100;
const LENGTH_MIN_SPREAD: f32 = 3.0;

/// Tags worked out here instead of read from `kits.toml`.
pub const DERIVED: [&str; 7] = ["melee", "ranged", "physical", "magic", "tank", "level2", "level6"];

/// One champion's side of a rule: a tag from `any` (the first held names the spell), every
/// tag in `all` and no tag in `none`.
struct Side {
    any: &'static [&'static str],
    all: &'static [&'static str],
    none: &'static [&'static str],
}

/// `text`: `{my}` and `{their}` are the spells of the `any` tags, `{them}` their name.
/// `active`: the line is about their spell being used, so it isn't said of a passive.
struct Rule {
    mine: Side,
    theirs: Side,
    active: bool,
    text: &'static str,
}

const NO: &[&str] = &[];

const fn side(any: &'static [&'static str], all: &'static [&'static str], none: &'static [&'static str]) -> Side {
    Side { any, all, none }
}

const fn rule(mine: Side, theirs: Side, text: &'static str) -> Rule {
    Rule { mine, theirs, active: false, text }
}

const fn once_used(mine: Side, theirs: Side, text: &'static str) -> Rule {
    Rule { mine, theirs, active: true, text }
}

/// First match first: what I can answer with, what answers me, damage against defences,
/// range, then level spikes.
const RULES: &[Rule] = &[
    rule(side(&["as_slow"], NO, NO), side(&["autos"], NO, NO), "Your {my} cuts attack speed and {them} fights with autos: use it when they start hitting you"),
    rule(side(&["auto_block"], NO, NO), side(&["autos"], NO, NO), "Your {my} stops autos and {them} lives on them: take the fight while it is up"),
    rule(side(&["proj_block", "spell_shield"], NO, NO), side(&["hook", "bind"], NO, NO), "Your {my} blocks {them}'s {their}: hold it for that"),
    rule(side(&["spell_shield"], NO, NO), side(&["zone_cc", "point_cc", "engage"], NO, NO), "Your {my} blocks {them}'s {their}: hold it for that"),
    rule(side(&["dodge"], NO, NO), side(&["hook", "bind", "zone_cc", "point_cc", "engage", "burst"], NO, NO), "Your {my} dodges {them}'s {their}: save it for that"),
    rule(side(&["engage", "burst"], NO, NO), side(&["shield"], NO, NO), "{them}'s {their} shield soaks an all-in: go in with {my} once it is down"),
    once_used(side(&["hook", "bind", "zone_cc"], NO, NO), side(&["dodge", "dash", "blink"], NO, NO), "{them}'s {their} gets them out of the way of your {my}: use it after {their}, not before"),
    once_used(side(&["engage"], NO, NO), side(&["dodge"], NO, NO), "{them}'s {their} gets them out of the way of your {my}: use it after {their}, not before"),
    rule(side(&["hook", "bind"], NO, NO), side(&["spell_shield", "proj_block"], NO, NO), "{them}'s {their} blocks your {my}: get it out of the way first"),
    rule(side(&["zone_cc", "point_cc", "engage"], NO, NO), side(&["spell_shield"], NO, NO), "{them}'s {their} blocks your {my}: get it out of the way first"),
    rule(side(&["autos"], NO, NO), side(&["auto_block"], NO, NO), "{them}'s {their} stops your autos: back off while it is up and fight when it ends"),
    rule(side(&["autos"], NO, NO), side(&["as_slow"], NO, NO), "{them}'s {their} cuts your attack speed: don't take a long auto fight through it"),
    rule(side(&["dash"], NO, NO), side(&["anti_dash"], NO, NO), "{them}'s {their} stops your {my}: dash before it lands or after it is gone"),
    once_used(side(&["anti_dash"], NO, NO), side(&["dash"], NO, NO), "Your {my} stops {them}'s {their}: keep it for when they go in"),
    rule(side(&["armor_stack"], NO, NO), side(&["true_dmg"], &["physical"], NO), "{them} deals physical damage, which the armour from your {my} blunts, but {them}'s {their} is true damage"),
    rule(side(&["armor_stack"], NO, NO), side(NO, &["physical"], &["true_dmg"]), "{them} deals physical damage: the armour from your {my} blunts every trade"),
    rule(side(&["armor_stack"], NO, NO), side(NO, &["magic"], NO), "{them} deals magic damage: the armour from your {my} does little in this lane"),
    rule(side(NO, &["tank"], &["armor_stack"]), side(&["true_dmg", "pct_hp"], NO, NO), "{them}'s {their} cuts through health and resists: keep trades short"),
    rule(side(&["pct_hp", "true_dmg"], NO, NO), side(NO, &["tank"], NO), "Your {my} goes through {them}'s health and resists: it keeps hurting however tanky they build"),
    rule(side(&["poke"], NO, NO), side(&["sustain"], NO, NO), "{them}'s {their} heals poke back: use {my} to set up a kill, not to chip"),
    rule(side(&["poke"], NO, &["ranged"]), side(NO, &["melee"], &["sustain"]), "{them} is melee: poke with {my} when they walk up to last-hit"),
    once_used(side(NO, &["ranged"], NO), side(&["dash", "blink", "engage"], &["melee"], NO), "You out-range {them}, but {their} closes the gap: keep your distance until it is used"),
    rule(side(NO, &["ranged"], NO), side(NO, &["melee"], &["dash", "blink", "engage", "hook"]), "You out-range {them} and they have no gap closer: auto when they last-hit"),
    rule(side(&["dash", "blink", "engage"], &["melee"], NO), side(NO, &["ranged"], NO), "{them} out-ranges you: {my} closes the gap, so go when they step up to hit you"),
    rule(side(NO, &["melee"], &["dash", "blink", "engage", "hook", "poke"]), side(NO, &["ranged"], NO), "{them} out-ranges you and you can't reach them: take the safe CS and keep your health"),
    rule(side(&["sustain", "shield"], NO, NO), side(&["poke"], NO, NO), "Your {my} takes the edge off {them}'s {their} poke: you can stay in lane through it"),
    rule(side(NO, &["level6"], NO), side(NO, NO, &["level6"]), "Your level 6 is the stronger one: look for the fight when you have R first"),
    rule(side(NO, NO, &["level2"]), side(NO, &["level2"], NO), "{them} is stronger at level 2: give up the first push and trade from level 3"),
    rule(side(NO, &["level2"], NO), side(NO, NO, &["level2"]), "You are stronger at level 2: push the first wave and trade when you get there first"),
    rule(side(NO, NO, &["level6"]), side(NO, &["level6"], NO), "{them}'s level 6 is the stronger one: respect the all-in once they have R"),
];

/// Every tag a rule asks for.
pub fn rule_tags() -> Vec<&'static str> {
    RULES.iter().flat_map(|r| [&r.mine, &r.theirs]).flat_map(|s| [s.any, s.all, s.none]).flatten().copied().collect()
}

/// A champion's tags with the spell behind each ("" for the derived ones).
fn tags<'a>(c: &Champion, notes: &'a LaneNotes) -> Vec<(&'a str, &'a str)> {
    let mut t: Vec<(&str, &str)> = notes.kits.get(&c.id).map_or(vec![], |k| k.iter().map(|(t, k)| (t.as_str(), k.as_str())).collect());
    let (range, magic, note) = (c.base.attack_range, c.traits.magic_share, notes.champs.get(&c.id));
    let derived = [
        (range > 0.0 && range <= MELEE_RANGE, "melee"),
        (range > MELEE_RANGE, "ranged"),
        (magic <= PHYSICAL_MAX, "physical"),
        (magic >= MAGIC_MIN, "magic"),
        (c.traits.tank >= TANK_MIN, "tank"),
        (note.is_some_and(|n| n.level2), "level2"),
        (note.is_some_and(|n| n.level6), "level6"),
    ];
    t.extend(derived.into_iter().filter(|d| d.0).map(|d| (d.1, "")));
    t
}

/// The spell `s` is about, when the champion fits it.
fn held<'a>(s: &Side, t: &[(&'a str, &'a str)]) -> Option<&'a str> {
    let key = |tag: &str| t.iter().find(|x| x.0 == tag).map(|x| x.1);
    if s.all.iter().any(|a| key(a).is_none()) || s.none.iter().any(|n| key(n).is_some()) {
        return None;
    }
    if s.any.is_empty() { Some("") } else { s.any.iter().find_map(|a| key(a)) }
}

fn spell(key: &str) -> &str {
    if key == "P" { "passive" } else { key }
}

/// Lines from both kits, the most useful first; empty when no rule fits the pair.
pub fn kit(sd: &StaticData, notes: &LaneNotes, me: ChampKey, them: ChampKey) -> Vec<String> {
    let (Some(a), Some(b)) = (sd.champs.get(&me), sd.champs.get(&them)) else { return vec![] };
    let (mine, theirs) = (tags(a, notes), tags(b, notes));
    let (mut said, mut out): (Vec<&str>, Vec<String>) = (Vec::new(), Vec::new());
    for r in RULES {
        let (Some(my), Some(their)) = (held(&r.mine, &mine), held(&r.theirs, &theirs)) else { continue };
        if out.len() == MAX_KIT_LINES || said.contains(&r.text) || (r.active && their == "P") {
            continue;
        }
        said.push(r.text);
        out.push(r.text.replace("{my}", spell(my)).replace("{their}", spell(their)).replace("{them}", &b.name));
    }
    out
}

/// The matchup page's most played build (starting set first) and, when it is another build,
/// its highest win rate one, each step with the win rate of the games that took it.
pub fn paths(b: &MatchupBuild) -> (Vec<PathStep>, Vec<PathStep>) {
    let step = |i: &ItemStat| PathStep { items: vec![i.id], win_rate: 100.0 * i.wins as f32 / i.games.max(1) as f32, games: i.games };
    let start = (!b.start.is_empty()).then(|| PathStep { items: b.start.clone(), win_rate: b.start_wr, games: b.start_games });
    let same = b.path.iter().map(|i| i.id).eq(b.best_path.iter().map(|i| i.id));
    (start.into_iter().chain(b.path.iter().map(step)).collect(), if same { vec![] } else { b.best_path.iter().map(step).collect() })
}

/// What the matchup page's numbers say about this pairing, as facts: my win rate against my
/// usual one, first items and boots by win rate, the max orders and win rate by game length.
pub fn numbers(sd: &StaticData, b: &MatchupBuild) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(usual) = b.usual_wr.filter(|u| (b.win_rate - u).abs() >= USUAL_MIN_DIFF) {
        out.push(format!("{:.1}% into {} against {usual:.1}% across all matchups", b.win_rate, sd.champ_name(b.vs)));
    }
    let pct = |wins: u32, games: u32| 100.0 * wins as f32 / games.max(1) as f32;
    let table = |label: &str, take: &str, rows: &[ItemStat]| {
        let shown: Vec<&ItemStat> = rows
            .iter()
            .filter(|i| sd.items.contains_key(&i.id) && i.games >= ROW_MIN_GAMES && i.games as f32 >= ROW_MIN_SHARE * b.games as f32)
            .take(ROWS)
            .collect();
        (shown.len() > 1).then(|| {
            let cell = |(n, i): (usize, &&ItemStat)| format!("{} {:.1}% ({:.0}%{})", sd.item_name(i.id), pct(i.wins, i.games), pct(i.games, b.games), if n == 0 { take } else { "" });
            format!("{label}: {}", shown.iter().enumerate().map(cell).collect::<Vec<_>>().join(", "))
        })
    };
    out.extend(table("First item", " take it", b.slots.first().map_or(&[], Vec::as_slice)));
    out.extend(table("Boots", " take them", &b.boots));
    if let [most, best] = b.skill_max.as_slice()
        && best.games >= SKILL_MIN_GAMES
        && best.win_rate - most.win_rate >= SKILL_MIN_DIFF
    {
        let max = |s: &SkillMax| format!("{} {:.1}% ({} games)", s.order, s.win_rate, fmt_games(s.games as u64));
        out.push(format!("Max order: {}, {}", max(most), max(best)));
    }
    let [mid, late] = PLAN_RIFT_PHASES;
    let part = |from: u32, to: u32| {
        let (games, wins) = b.by_length.iter().filter(|l| (from..to).contains(&l.from_min)).fold((0, 0), |a, l| (a.0 + l.games, a.1 + l.wins));
        (games >= LENGTH_MIN_GAMES).then(|| pct(wins, games))
    };
    if let (Some(a), Some(m), Some(l)) = (part(0, mid), part(mid, late), part(late, u32::MAX))
        && a.max(m).max(l) - a.min(m).min(l) >= LENGTH_MIN_SPREAD
    {
        out.push(format!("Games over before {mid}:00: {a:.1}%; {mid}:00 to {late}:00: {m:.1}%; later: {l:.1}%"));
    }
    out
}
