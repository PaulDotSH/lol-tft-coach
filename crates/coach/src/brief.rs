//! The coach's note on one reviewed game: a numbered fact sheet from the
//! analysis, and rules that connect related facts into a few takeaways and a
//! focus. Every line cites its facts and must pass `check`.

use crate::analysis::{RICH_GOLD, clock, marks};
use crate::consts::*;
use crate::laning::gold;
use crate::selfcoach::{HOLDING_GOLD, the};
use crate::session::{BEHIND_15, BEHIND_AT_15, DIED_EARLY, EARLY_DEATH_S, MANY, MANY_DEATHS, NO_CONTROL};
use model::*;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
enum Src {
    Result,
    Shares,
    Augments,
    Lane,
    Backs,
    Phase(u32),
    Team,
    Rich,
    Vision,
    Takeaway(usize),
    Focus,
    Death(usize),
    DeathNote(usize),
    Fight(usize),
    Objectives,
    Taken(usize),
    Past,
    Habit(usize),
}

struct Sheet(Vec<(Src, Fact)>);

impl Sheet {
    fn add(&mut self, src: Src, kind: FactKind, good: Option<bool>, text: String) {
        let id = self.0.len() as u32 + 1;
        self.0.push((src, Fact { id, kind, good, text }));
    }

    fn id(&self, src: Src) -> Option<u32> {
        self.0.iter().find(|(s, _)| *s == src).map(|(_, f)| f.id)
    }

    fn ids(&self, srcs: impl IntoIterator<Item = Src>) -> Vec<u32> {
        srcs.into_iter().filter_map(|s| self.id(s)).collect()
    }
}

/// The fact sheet: numbered lines with the numbers already formatted.
pub fn facts(a: &MatchAnalysis, session: Option<&Session>, sd: &StaticData, augments: &HashMap<AugmentId, String>) -> Vec<Fact> {
    sheet(a, session, sd, augments).0.into_iter().map(|(_, f)| f).collect()
}

/// Every champion, item and augment name a line could mention.
pub fn names(sd: &StaticData, augments: &HashMap<AugmentId, String>) -> Vec<String> {
    let mut v: Vec<String> = sd.champs.values().map(|c| c.name.clone()).chain(sd.items.values().map(|i| i.name.clone())).chain(augments.values().cloned()).filter(|n| n.chars().count() >= 2).collect();
    v.sort();
    v.dedup();
    v
}

/// The note, or None when fewer than NOTE_MIN takeaways pass the check.
pub fn note(a: &MatchAnalysis, session: Option<&Session>, sd: &StaticData, augments: &HashMap<AugmentId, String>) -> Option<GameNote> {
    let facts = facts(a, session, sd, augments);
    check(&compose(a, session, sd, augments), &facts, &names(sd, augments))
}

/// A line passes when it cites facts that all exist, is short enough, and every number and
/// champion, item or augment name in it appears in a fact it cites.
pub fn passes(line: &NoteLine, facts: &[Fact], names: &[String]) -> bool {
    let cited: Vec<&Fact> = line.facts.iter().filter_map(|id| facts.iter().find(|f| f.id == *id)).collect();
    if cited.is_empty() || cited.len() != line.facts.len() || line.text.chars().count() >= NOTE_MAX_CHARS {
        return false;
    }
    let known: Vec<String> = cited.iter().flat_map(|f| numbers(&f.text)).collect();
    numbers(&line.text).iter().all(|n| known.contains(n)) && names.iter().filter(|n| has_name(&line.text, n)).all(|n| cited.iter().any(|f| has_name(&f.text, n)))
}

/// Drops the lines that fail `passes`; None when fewer than NOTE_MIN takeaways are left.
pub fn check(note: &GameNote, facts: &[Fact], names: &[String]) -> Option<GameNote> {
    let takeaways: Vec<NoteLine> = note.takeaways.iter().filter(|l| passes(l, facts, names)).cloned().collect();
    if takeaways.len() < NOTE_MIN {
        return None;
    }
    let focus = note.focus.clone().filter(|l| passes(l, facts, names));
    let facts = cited(takeaways.iter().chain(&focus), facts);
    Some(GameNote { takeaways, focus, facts, rules_v: note.rules_v })
}

fn cited<'a>(lines: impl Iterator<Item = &'a NoteLine>, facts: &[Fact]) -> Vec<Fact> {
    let mut ids: Vec<u32> = lines.flat_map(|l| l.facts.iter().copied()).collect();
    ids.sort();
    ids.dedup();
    facts.iter().filter(|f| ids.contains(&f.id)).cloned().collect()
}

/// Digit runs with `,` `.` `:` inside; thousands separators dropped.
fn numbers(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if !c[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut t = String::new();
        while i < c.len() && (c[i].is_ascii_digit() || (matches!(c[i], ',' | '.' | ':') && c.get(i + 1).is_some_and(|d| d.is_ascii_digit()))) {
            if c[i] != ',' {
                t.push(c[i]);
            }
            i += 1;
        }
        out.push(t);
    }
    out
}

fn has_name(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(i, _)| {
        let before = text[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let after = text[i + name.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        before && after
    })
}

pub(crate) fn thousands(n: u32) -> String {
    if n >= 1000 { format!("{},{:03}", n / 1000, n % 1000) } else { n.to_string() }
}

fn and(v: &[String]) -> String {
    match v {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn count(n: usize) -> String {
    const WORDS: [&str; 11] = ["no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    WORDS.get(n).map_or(n.to_string(), |w| w.to_string())
}

fn lead_in(total: usize, shown: usize) -> &'static str {
    if total > shown { ", among them " } else { ": " }
}

fn rich_at(minutes: &[u32]) -> String {
    match minutes {
        [first, .., last] if minutes.len() > NOTE_INSTANCES => format!("{} minute marks from {first}:00 to {last}:00", count(minutes.len())),
        _ => marks(minutes),
    }
}

pub(crate) fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

fn lane_word(l: Lane) -> &'static str {
    match l {
        Lane::Top => "top",
        Lane::Jungle => "jungle",
        Lane::Middle => "mid",
        Lane::Bottom => "bot",
        Lane::Support => "support",
    }
}

fn presence(p: Presence) -> &'static str {
    match p {
        Presence::There => "you were there",
        Presence::Dead => "you were dead",
        Presence::Away => "you were elsewhere",
    }
}

fn fight_at(f: &Fight) -> String {
    format!("{}{}", clock(f.start), f.place.as_deref().map_or(String::new(), |p| format!(" in {}", the(p))))
}

fn death_at(n: &DeathNote, sd: &StaticData) -> String {
    let place = n.place.as_deref().map_or(String::new(), |p| format!(" in {}", the(p)));
    let killer = n.killer.map_or(" to towers or minions".to_string(), |k| format!(" to {}", sd.champ_name(k)));
    format!("{}{place}{killer}", clock(n.time))
}

fn sheet(a: &MatchAnalysis, session: Option<&Session>, sd: &StaticData, augments: &HashMap<AugmentId, String>) -> Sheet {
    let g = &a.game;
    let rift = g.mode == Mode::Rift;
    let name = |c: ChampKey| sd.champ_name(c).to_string();
    let mut s = Sheet(Vec::new());

    let result = if g.remake { "Remake" } else if g.win { "Victory" } else { "Defeat" };
    let lane = g.lane.filter(|_| rift).map_or(String::new(), |l| format!(" ({})", lane_word(l)));
    s.add(Src::Result, FactKind::Result, (!g.remake).then_some(g.win), format!("{result} as {}{lane} in {}, {}/{}/{}.", name(g.champ), clock(g.duration_s as f32), g.kills, g.deaths, g.assists));
    let shares: Vec<String> = [("kill participation", a.kill_participation), ("damage", a.damage_share), ("gold", a.gold_share)]
        .into_iter()
        .filter_map(|(l, v)| v.map(|v| format!("{l} {:.0}%", v * 100.0)))
        .collect();
    if !shares.is_empty() {
        s.add(Src::Shares, FactKind::Result, None, format!("Your share of your team: {}.", shares.join(", ")));
    }
    let took: Vec<String> = g.augments.iter().filter_map(|id| augments.get(id).cloned()).collect();
    if !took.is_empty() {
        s.add(Src::Augments, FactKind::Items, None, format!("Augments you took: {}.", and(&took)));
    }

    if let Some(l) = &a.laning {
        let side = |v: &[ChampKey]| and(&v.iter().map(|&c| name(c)).collect::<Vec<_>>());
        let good = l.turned_at.and(l.minutes.last()).map(|m| m.gold_diff > 0);
        s.add(Src::Lane, FactKind::Lane, good, format!("{} vs {}: {}.", side(&l.allies), side(&l.enemies), l.verdict));
        if !l.backs.is_empty() {
            let backs: Vec<String> = l.backs.iter().map(|b| format!("{} {}:00 ({} gold spent)", name(b.champ), b.minute, thousands(b.spent))).collect();
            s.add(Src::Backs, FactKind::Lane, None, format!("First backs: {}.", backs.join(", ")));
        }
    }
    for r in a.phases.iter().filter(|r| matches!(r.minute, 10 | 15)) {
        let mut t = format!("At {}:00: you {} CS, level {}", r.minute, r.cs, r.level);
        if let (Some(o), Some(cs), Some(lv)) = (a.opponent, r.opp_cs, r.opp_level) {
            t += &format!("; {} {cs} CS, level {lv}", name(o));
        }
        if let (Some(o), Some(v)) = (a.opponent, r.gold_lead) {
            t += &format!("; gold {} vs {}", gold(v), name(o));
        }
        s.add(Src::Phase(r.minute), FactKind::Economy, r.gold_lead.map(|v| v >= 0), t + ".");
    }
    if rift {
        let best = a.gold.iter().max_by_key(|p| p.team).filter(|p| p.team > 0);
        let worst = a.gold.iter().min_by_key(|p| p.team).filter(|p| p.team < 0);
        let parts: Vec<String> = [best.map(|p| format!("most ahead {} at {}", gold(p.team), clock(p.time))), worst.map(|p| format!("most behind {} at {}", gold(p.team), clock(p.time)))]
            .into_iter()
            .flatten()
            .collect();
        if !parts.is_empty() {
            s.add(Src::Team, FactKind::Economy, None, format!("Team gold against theirs: {}.", parts.join(", ")));
        }
    }
    if !a.rich_marks.is_empty() {
        s.add(Src::Rich, FactKind::Economy, Some(false), format!("You held {RICH_GOLD}+ gold unspent at {}.", marks(&a.rich_marks)));
    }
    if rift && let Some(me) = a.players.iter().find(|p| p.me) {
        let min = (g.duration_s as f32 / 60.0).max(1.0);
        let mut t = format!("Vision score {} ({:.2}/min), {} wards placed, {} Control Wards bought", g.vision, g.vision as f32 / min, me.wards_placed, me.control_wards);
        if let Some(o) = a.players.iter().find(|p| !p.ally && Some(p.champ) == a.opponent) {
            t += &format!("; {} placed {} wards", name(o.champ), o.wards_placed);
        }
        s.add(Src::Vision, FactKind::Vision, None, t + ".");
    }

    for (i, t) in a.takeaways.iter().enumerate() {
        s.add(Src::Takeaway(i), FactKind::Takeaway, Some(t.good), format!("{}.", t.text));
    }
    if let Some(f) = &a.focus {
        s.add(Src::Focus, FactKind::Focus, None, format!("Focus: {}. {}", f.text, f.why));
    }

    for (i, n) in a.review.deaths.iter().enumerate() {
        let mut t = format!("Death at {}", death_at(n, sd));
        if !n.assisters.is_empty() {
            t += &format!(" (with {})", and(&n.assisters.iter().map(|&c| name(c)).collect::<Vec<_>>()));
        }
        if n.unspent > 0 {
            t += &format!(", holding {} gold", thousands(n.unspent));
        }
        if let Some(b) = n.buy_next.filter(|_| n.could_buy) {
            t += &format!("; Buy next {} was affordable", sd.item_name(b));
        }
        for (c, lv) in &n.spikes {
            t += &format!("; {} had just reached level {lv}", name(*c));
        }
        if n.traded {
            t += "; your team won the trade";
        }
        s.add(Src::Death(i), FactKind::Deaths, (!n.traded).then_some(false), t + ".");
    }
    for (i, n) in a.review.notes.iter().enumerate() {
        s.add(Src::DeathNote(i), FactKind::Deaths, Some(false), format!("{n}."));
    }
    for (i, f) in a.fights.iter().enumerate() {
        let mut t = format!("Fight at {}: kills {}–{}; {}", fight_at(f), f.won, f.lost, presence(f.presence));
        if f.died_first {
            t += " and died first";
        }
        if let Some(m) = &f.then {
            t += &format!("; then {} took {}", if m.ours { "your team" } else { "they" }, m.kind);
        }
        let good = (f.won != f.lost).then_some(f.won > f.lost);
        s.add(Src::Fight(i), FactKind::Fights, good, t + ".");
    }
    if rift && let (Some(o), Some(t)) = (&g.ours, &g.theirs) {
        let parts: Vec<String> = [("dragons", o.dragons, t.dragons), ("barons", o.barons, t.barons), ("heralds", o.heralds, t.heralds), ("grubs", o.grubs, t.grubs), ("towers", o.towers, t.towers)]
            .into_iter()
            .filter(|(_, a, b)| a + b > 0)
            .map(|(l, a, b)| format!("{l} {a}–{b}"))
            .collect();
        if !parts.is_empty() {
            s.add(Src::Objectives, FactKind::Objectives, None, format!("Objectives, your team vs theirs: {}.", parts.join(", ")));
        }
    }
    for (i, m) in a.marks.iter().enumerate().filter(|(_, m)| !m.ours && m.kind != "Tower" && m.kind != "Inhibitor") {
        let detail = m.detail.as_deref().map_or(String::new(), |d| format!(" ({d})"));
        let at = m.presence.map_or(String::new(), |p| format!("; {}", presence(p)));
        s.add(Src::Taken(i), FactKind::Objectives, Some(false), format!("They took {}{detail} at {}{at}.", m.kind, clock(m.time)));
    }
    if let Some(p) = a.vs_past.as_ref().filter(|p| !p.rows.is_empty()) {
        let rows: Vec<String> = p
            .rows
            .iter()
            .map(|r| {
                let f = |v: f32| format!("{v:.prec$}", prec = r.digits as usize);
                format!("{} {} (average {}, best {})", r.label, f(r.this), f(r.avg), f(r.best))
            })
            .collect();
        s.add(Src::Past, FactKind::Result, None, format!("Against your last {} {} games: {}.", p.games, name(g.champ), rows.join("; ")));
    }
    if let Some(sn) = session.filter(|sn| sn.game_ids.contains(&g.game_id)) {
        for (i, r) in sn.repeats.iter().enumerate() {
            s.add(Src::Habit(i), FactKind::Habit, Some(false), format!("This session, in {} of {} games: {}.", r.games, r.of, r.text));
        }
    }
    s
}

#[derive(Clone, Copy, PartialEq)]
enum Topic {
    Lead,
    Comeback,
    LaneWon,
    Held,
    Early,
    DiedFirst,
    Away,
    Lane,
    Vision,
    Deaths,
    Cs,
    Plain(usize),
}

struct Cand {
    topic: Topic,
    score: f32,
    good: bool,
    text: String,
    facts: Vec<u32>,
    covers: Vec<usize>,
}

fn covering(a: &MatchAnalysis, keys: &[&str]) -> Vec<usize> {
    a.takeaways.iter().enumerate().filter(|(_, t)| keys.iter().any(|k| t.text.contains(k))).map(|(i, _)| i).collect()
}

fn pick_fights<'a>(mut v: Vec<(usize, &'a Fight)>, worst: bool) -> Vec<(usize, &'a Fight)> {
    v.sort_by_key(|(_, f)| std::cmp::Reverse(if worst { f.lost as i32 - f.won as i32 } else { f.won as i32 - f.lost as i32 }));
    v.truncate(NOTE_INSTANCES);
    v.sort_by(|a, b| a.1.start.total_cmp(&b.1.start));
    v
}

fn candidates(a: &MatchAnalysis, s: &Sheet, session: Option<&Session>, sd: &StaticData) -> Vec<Cand> {
    let g = &a.game;
    let rift = g.mode == Mode::Rift;
    let me = a.players.iter().find(|p| p.me);
    let mut out: Vec<Cand> = Vec::new();
    let mut push = |topic: Topic, score: f32, good: bool, text: String, facts: Vec<u32>, covers: Vec<usize>| out.push(Cand { topic, score, good, text, facts, covers });
    let fights: Vec<(usize, &Fight)> = a.fights.iter().enumerate().collect();
    let lane_won = a.laning.as_ref().filter(|l| l.turned_at.is_some() && l.minutes.last().is_some_and(|m| m.gold_diff > 0));
    let lane_tail = |l: &Laning| l.verdict.split_once(": ").map_or(l.verdict.clone(), |(_, t)| t.to_string());

    if rift && !g.win && !g.remake {
        let best = a.gold.iter().max_by_key(|p| p.team).filter(|p| p.team >= NOTE_TEAM_LEAD);
        if let Some(b) = best {
            let lost = pick_fights(fights.iter().copied().filter(|(_, f)| f.start >= b.time && f.lost > f.won).collect(), true);
            if !lost.is_empty() {
                let at: Vec<String> = lost.iter().map(|(_, f)| fight_at(f)).collect();
                let mut ids = s.ids([Src::Team]);
                ids.extend(s.ids(lost.iter().map(|(i, _)| Src::Fight(*i))));
                let text = format!("Your team was {} gold ahead at {} and lost; the fights after it went their way, at {}.", thousands(b.team as u32), clock(b.time), and(&at));
                push(Topic::Lead, NOTE_SCORE_LEAD + lost.len() as f32, false, text, ids, covering(a, &["led by", "Biggest swing"]));
            }
        }
        if let Some(l) = lane_won {
            let lost = pick_fights(fights.iter().copied().filter(|(_, f)| f.start >= NOTE_LANE_END_S && f.lost > f.won).collect(), true);
            if lost.len() >= 2 {
                let at: Vec<String> = lost.iter().map(|(_, f)| fight_at(f)).collect();
                let mut ids = s.ids([Src::Lane]);
                ids.extend(s.ids(lost.iter().map(|(i, _)| Src::Fight(*i))));
                let text = format!("You won your lane ({}) but lost the game in fights after it, at {}.", lane_tail(l), and(&at));
                push(Topic::Lead, NOTE_SCORE_LEAD - 1.0 + lost.len() as f32, false, text, ids, covering(a, &["Ahead of"]));
            }
        }
    }
    if rift && g.win {
        let worst = a.gold.iter().min_by_key(|p| p.team).filter(|p| p.team <= -NOTE_TEAM_LEAD);
        if let Some(w) = worst {
            let won = pick_fights(fights.iter().copied().filter(|(_, f)| f.start >= w.time && f.won > f.lost).collect(), false);
            if !won.is_empty() {
                let at: Vec<String> = won.iter().map(|(_, f)| fight_at(f)).collect();
                let mut ids = s.ids([Src::Team]);
                ids.extend(s.ids(won.iter().map(|(i, _)| Src::Fight(*i))));
                let text = format!("Your team came back from {} gold behind at {}, winning the fights at {}.", thousands(w.team.unsigned_abs()), clock(w.time), and(&at));
                push(Topic::Comeback, NOTE_SCORE_GOOD + 1.0, true, text, ids, covering(a, &["Came back"]));
            }
        }
        if let Some(l) = lane_won {
            let won = pick_fights(fights.iter().copied().filter(|(_, f)| f.start >= NOTE_LANE_END_S && f.won > f.lost && f.presence == Presence::There).collect(), false);
            let mut ids = s.ids([Src::Lane]);
            let text = if won.is_empty() {
                format!("You won your lane ({}) and the game.", lane_tail(l))
            } else {
                ids.extend(s.ids(won.iter().map(|(i, _)| Src::Fight(*i))));
                format!("You won your lane ({}) and kept it going in the fights at {}.", lane_tail(l), and(&won.iter().map(|(_, f)| fight_at(f)).collect::<Vec<_>>()))
            };
            push(Topic::LaneWon, NOTE_SCORE_GOOD + won.len() as f32 * 0.1, true, text, ids, covering(a, &["Ahead of"]));
        }
    }

    let deaths: Vec<(usize, &DeathNote)> = a.review.deaths.iter().enumerate().collect();
    let held: Vec<(usize, &DeathNote)> = deaths.iter().copied().filter(|(_, n)| n.unspent >= HOLDING_GOLD).collect();
    if !held.is_empty() && (held.len() >= 2 || !a.rich_marks.is_empty()) {
        let shown = &held[..held.len().min(NOTE_INSTANCES)];
        let at: Vec<String> = shown.iter().map(|(_, n)| format!("{} at {}", thousands(n.unspent), clock(n.time))).collect();
        let times = if held.len() > shown.len() { format!(" {} times", count(held.len())) } else { String::new() };
        let mut text = format!("You died holding gold{times}{}{}", lead_in(held.len(), shown.len()), and(&at));
        if let Some((_, n)) = shown.iter().find(|(_, n)| n.could_buy && n.buy_next.is_some()) {
            text += &format!("; Buy next {} was already affordable at {}", sd.item_name(n.buy_next.unwrap_or_default()), clock(n.time));
        }
        let mut ids = s.ids(shown.iter().map(|(i, _)| Src::Death(*i)));
        if !a.rich_marks.is_empty() {
            text += &format!("; you also sat on {RICH_GOLD}+ gold at {}", rich_at(&a.rich_marks));
            ids.extend(s.ids([Src::Rich]));
        }
        push(Topic::Held, NOTE_SCORE_PATTERN + held.len() as f32, false, text + ".", ids, covering(a, &["gold unspent", "g unspent"]));
    }
    let early: Vec<(usize, &DeathNote)> = deaths.iter().copied().filter(|(_, n)| n.time < NOTE_LANE_END_S && !n.traded).collect();
    if rift && early.len() >= 2 {
        let shown = &early[..early.len().min(NOTE_INSTANCES)];
        let at: Vec<String> = shown.iter().map(|(_, n)| death_at(n, sd)).collect();
        let text = format!("{} early deaths came outside fights your team won{}{}.", capital(&count(early.len())), lead_in(early.len(), shown.len()), and(&at));
        push(Topic::Early, NOTE_SCORE_PATTERN + early.len() as f32, false, text, s.ids(shown.iter().map(|(i, _)| Src::Death(*i))), covering(a, &["before 14:00"]));
    }
    let lost_there: Vec<(usize, &Fight)> = fights.iter().copied().filter(|(_, f)| f.lost > f.won && f.won + f.lost >= 2 && f.presence == Presence::There).collect();
    let first: Vec<(usize, &Fight)> = lost_there.iter().copied().filter(|(_, f)| f.died_first).collect();
    if first.len() >= 2 {
        let shown = &first[..first.len().min(NOTE_INSTANCES)];
        let text = format!("You died first in {} lost fights{}{}.", count(first.len()), lead_in(first.len(), shown.len()), and(&shown.iter().map(|(_, f)| fight_at(f)).collect::<Vec<_>>()));
        push(Topic::DiedFirst, NOTE_SCORE_PATTERN + first.len() as f32, false, text, s.ids(shown.iter().map(|(i, _)| Src::Fight(*i))), covering(a, &["died first"]));
    }
    let wards = covering(a, &["Control Wards", "wards vs"]);
    if rift && wards.len() >= 2 && let Some(m) = me {
        let against = a.players.iter().find(|p| !p.ally && Some(p.champ) == a.opponent).map_or(String::new(), |o| format!(" against {} for {}", o.wards_placed, sd.champ_name(o.champ)));
        let text = format!("Vision fell short: {} wards placed{against}, and {} Control Wards bought.", m.wards_placed, m.control_wards);
        push(Topic::Vision, NOTE_SCORE_PATTERN, false, text, s.ids([Src::Vision]), wards);
    }
    let away: Vec<(usize, &TimelineMark)> = a.marks.iter().enumerate().filter(|(_, m)| !m.ours && m.presence == Some(Presence::Away) && m.kind != "Tower" && m.kind != "Inhibitor").collect();
    if rift && away.len() >= 2 {
        let shown = &away[..away.len().min(NOTE_INSTANCES)];
        let at: Vec<String> = shown.iter().map(|(_, m)| format!("{} at {}", m.kind, clock(m.time))).collect();
        let more = if away.len() > shown.len() { format!(", and {} more,", count(away.len() - shown.len())) } else { String::new() };
        let text = format!("They took {}{more} while you were elsewhere.", and(&at));
        push(Topic::Away, NOTE_SCORE_AWAY + away.len() as f32, false, text, s.ids(shown.iter().map(|(i, _)| Src::Taken(*i))), covering(a, &["Away for"]));
    }

    if let Some(sn) = session.filter(|sn| sn.game_ids.contains(&g.game_id)) {
        for (i, r) in sn.repeats.iter().enumerate() {
            let habit = format!("{} of {} games this session", r.games, r.of);
            let line = match r.text.as_str() {
                DIED_EARLY => deaths.iter().find(|(_, n)| n.time < EARLY_DEATH_S as f32).map(|(j, n)| {
                    (Topic::Early, format!("Dying before 10:00 keeps coming back ({habit}); this game at {}.", death_at(n, sd)), vec![Src::Habit(i), Src::Death(*j)])
                }),
                BEHIND_15 => a.phases.iter().find(|p| p.minute == 15).and_then(|p| p.gold_lead).filter(|v| *v <= BEHIND_AT_15).zip(a.opponent).map(|(v, o)| {
                    (Topic::Lane, format!("Falling behind in lane keeps coming back ({habit}); this game {} gold against {} at 15:00.", gold(v), sd.champ_name(o)), vec![Src::Habit(i), Src::Phase(15)])
                }),
                NO_CONTROL => me.filter(|m| m.control_wards == 0).map(|m| {
                    let against = a.players.iter().find(|p| !p.ally && Some(p.champ) == a.opponent).map_or(String::new(), |o| format!(", and {} wards placed against {} for {}", m.wards_placed, o.wards_placed, sd.champ_name(o.champ)));
                    (Topic::Vision, format!("No Control Wards again ({habit}, this one included){against}."), vec![Src::Habit(i), Src::Vision])
                }),
                MANY => (g.deaths >= MANY_DEATHS).then(|| (Topic::Deaths, format!("{} deaths this game; 6 or more in {habit}.", g.deaths), vec![Src::Habit(i), Src::Result])),
                _ => None,
            };
            if let Some((topic, text, srcs)) = line {
                let ids = s.ids(srcs);
                let covers = match topic {
                    Topic::Early => covering(a, &["before 14:00"]),
                    Topic::Vision => covering(a, &["Control Wards", "wards vs"]),
                    _ => vec![],
                };
                push(topic, NOTE_SCORE_HABIT, false, text, ids, covers);
            }
        }
    }

    match a.focus.as_ref().and_then(focus_topic) {
        Some(Topic::Cs) => {
            if let Some(r) = a.phases.iter().find(|r| r.minute == 10) {
                let against = a.opponent.zip(r.opp_cs).map_or(String::new(), |(o, cs)| format!(" against {cs} for {}", sd.champ_name(o)));
                push(Topic::Cs, 0.0, false, format!("This game you had {} CS at 10:00{against}.", r.cs), s.ids([Src::Phase(10)]), vec![]);
            }
        }
        Some(Topic::Lane) => {
            if let (Some(o), Some(v)) = (a.opponent, a.phases.iter().find(|r| r.minute == 15).and_then(|r| r.gold_lead).filter(|v| *v < 0)) {
                push(Topic::Lane, 0.0, false, format!("This game you were {} gold against {} at 15:00.", gold(v), sd.champ_name(o)), s.ids([Src::Phase(15)]), vec![]);
            }
        }
        Some(Topic::Vision) => {
            if let Some(m) = me {
                push(Topic::Vision, 0.0, false, format!("This game: {} wards placed and {} Control Wards bought.", m.wards_placed, m.control_wards), s.ids([Src::Vision]), vec![]);
            }
        }
        _ => {}
    }

    let augs = s.id(Src::Augments);
    for (i, t) in a.takeaways.iter().enumerate() {
        let score = if t.good { NOTE_SCORE_PLAIN - 0.5 } else { NOTE_SCORE_PLAIN };
        match augs.filter(|_| t.text.starts_with("Damage ")) {
            Some(aug) => {
                let took = &s.0[aug as usize - 1].1.text;
                let names = took.trim_start_matches("Augments you took: ").trim_end_matches('.');
                push(Topic::Plain(i), score + 0.5, t.good, format!("{} with {names}.", t.text), s.ids([Src::Takeaway(i), Src::Augments]), vec![i]);
            }
            None => push(Topic::Plain(i), score, t.good, format!("{}.", t.text), s.ids([Src::Takeaway(i)]), vec![i]),
        }
    }
    out
}

fn focus_topic(f: &Focus) -> Option<Topic> {
    match f.goal.as_ref().map(|g| g.kind) {
        Some(GoalKind::EarlyDeaths) => Some(Topic::Early),
        Some(GoalKind::Unspent) => Some(Topic::Held),
        Some(GoalKind::Cs10) => Some(Topic::Cs),
        Some(GoalKind::Vision) => Some(Topic::Vision),
        Some(GoalKind::BuyNext) => None,
        None if f.text.starts_with("Let your frontline") => Some(Topic::DiedFirst),
        None if f.text.starts_with("Play the lane safer") => Some(Topic::Lane),
        None if f.text.starts_with("Move to objectives") => Some(Topic::Away),
        None => None,
    }
}

/// The note before the check: the best connected lines, at most one of them good, and the
/// focus with this game's instance of it.
pub fn compose(a: &MatchAnalysis, session: Option<&Session>, sd: &StaticData, augments: &HashMap<AugmentId, String>) -> GameNote {
    let s = sheet(a, session, sd, augments);
    let mut cands = candidates(a, &s, session, sd);
    cands.sort_by(|x, y| y.score.total_cmp(&x.score));
    let mut used: Vec<Topic> = Vec::new();
    let mut covered: Vec<usize> = Vec::new();

    let focus = a.focus.as_ref().and_then(|f| {
        let id = s.id(Src::Focus)?;
        let plain = NoteLine { text: format!("{}. {}", f.text, f.why), facts: vec![id], good: None };
        let topic = focus_topic(f);
        let Some(c) = cands.iter().find(|c| Some(c.topic) == topic) else { return Some(plain) };
        let text = format!("{}. {}", f.text, c.text);
        if text.chars().count() >= NOTE_MAX_CHARS {
            return Some(plain);
        }
        used.push(c.topic);
        covered.extend(&c.covers);
        let mut facts = vec![id];
        facts.extend(&c.facts);
        Some(NoteLine { text, facts, good: None })
    });

    let open = |c: &Cand, used: &[Topic], covered: &[usize]| !used.contains(&c.topic) && !matches!(c.topic, Topic::Plain(i) if covered.contains(&i)) && !c.facts.is_empty();
    let goods = cands.iter().filter(|c| c.good && open(c, &used, &covered)).count().min(1);
    let mut picked: Vec<&Cand> = Vec::new();
    for (pass, want_good) in [false, true, false].into_iter().enumerate() {
        let room = if pass == 0 { NOTE_TAKEAWAYS - goods } else { NOTE_TAKEAWAYS };
        for c in cands.iter().filter(|c| c.good == want_good) {
            if picked.len() >= room {
                break;
            }
            if open(c, &used, &covered) {
                used.push(c.topic);
                covered.extend(&c.covers);
                picked.push(c);
            }
        }
    }
    picked.sort_by_key(|c| c.good);
    let takeaways: Vec<NoteLine> = picked.iter().map(|c| NoteLine { text: c.text.clone(), facts: c.facts.clone(), good: Some(c.good) }).collect();
    let facts: Vec<Fact> = s.0.into_iter().map(|(_, f)| f).collect();
    GameNote { facts: cited(takeaways.iter().chain(&focus), &facts), takeaways, focus, rules_v: NOTE_RULES_V }
}
