//! One past game reviewed from match history: gold leads over time, deaths
//! with where they happened and whether the fight was won, objectives, and
//! the few takeaways that stand out.

use crate::common::champ;
use crate::selfcoach::review_deaths;
use crate::{fights, items, needs, threat};
use model::*;

/// Frame gold counts as held at a death when the frame is this recent (s).
const HELD_FRAME: f32 = 40.0;
/// A live recap's death this close (s) is the same death.
const SAME_DEATH: f32 = 3.0;
/// Team gold lead that counts as a real lead / deficit.
const BIG_LEAD: i32 = 2500;
/// Gold lost within SWING_SPAN (s) worth calling the turning point.
const SWING: i32 = 2500;
const SWING_SPAN: f32 = 150.0;
/// Lane gold gap at 15:00 (10:00 in short games) worth a takeaway.
const LANE_GAP_15: i32 = 700;
const LANE_GAP_10: i32 = 500;
/// Per-minute stat vs the champion average: below `LOW`× is bad, from `HIGH`× good.
const LOW: f32 = 0.85;
const HIGH: f32 = 1.15;
const KP_GOOD: f32 = 0.65;
const KP_BAD: f32 = 0.35;
/// Team kills before kill participation means anything.
const KP_MIN_KILLS: u32 = 10;
const EARLY_S: f32 = 14.0 * 60.0;
const EARLY_DEATHS: usize = 3;
/// Lost fights / enemy objectives before a pattern in them is a takeaway.
const PATTERN_MIN: usize = 2;
/// Fights I was in before their win share is a takeaway, and the share that counts.
const FIGHTS_MIN: usize = 4;
const FIGHTS_GOOD: f32 = 0.7;
const LANE_ORDER: [Lane; 5] = [Lane::Top, Lane::Jungle, Lane::Middle, Lane::Bottom, Lane::Support];

/// `stats`: my champion's lolalytics data (benchmarks, the build into this comp).
/// `recap`: the app's own recap when it watched this game live.
pub fn analyze_match(d: &MatchDetail, sd: &StaticData, stats: Option<&ChampStats>, recap: Option<&Recap>) -> MatchAnalysis {
    let g = &d.game;
    let me = &d.players[d.me];
    let opp = me.lane.and_then(|l| {
        let mut it = d.players.iter().enumerate().filter(|(_, p)| !p.ally && p.lane == Some(l));
        let (i, _) = it.next()?;
        it.next().is_none().then_some(i)
    });
    let gold: Vec<GoldPoint> = d
        .frames
        .iter()
        .map(|f| {
            let side = |ally: bool| -> i32 {
                d.players.iter().zip(&f.players).filter(|(p, _)| p.ally == ally).map(|(_, s)| s.total_gold as i32).sum()
            };
            GoldPoint {
                time: f.time,
                team: side(true) - side(false),
                lane: opp.map(|o| f.players[d.me].total_gold as i32 - f.players[o].total_gold as i32),
            }
        })
        .collect();
    let kills: Vec<KillMark> = d
        .kills
        .iter()
        .map(|k| KillMark {
            time: k.time,
            ours: !d.players[k.victim].ally,
            mine: k.victim == d.me || k.killer == Some(d.me) || k.assisters.contains(&d.me),
        })
        .collect();

    let team = |f: fn(&MatchPlayer) -> u32| d.players.iter().filter(|p| p.ally).map(f).sum::<u32>();
    let share = |mine: u32, all: u32| (all > 0).then(|| mine as f32 / all as f32);
    let team_kills = team(|p| p.kills);

    let review = review(d, sd, recap);
    let benchmarks = recap.map(|r| r.benchmarks.clone()).or_else(|| stats.map(|s| s.benchmarks.clone()));
    let recommended = match recap.filter(|r| !r.recommended.is_empty()) {
        Some(r) => r.recommended.clone(),
        None => plan(d, sd, stats),
    };

    let mut players = d.players.clone();
    let order = |p: &MatchPlayer| (!p.ally, p.lane.map_or(LANE_ORDER.len(), |l| LANE_ORDER.iter().position(|x| *x == l).unwrap_or(0)));
    players.sort_by_key(order);

    let marks = fights::with_presence(d);
    let fights = fights::fights(d, &marks);
    let mut a = MatchAnalysis {
        game: g.clone(),
        players,
        opponent: opp.map(|o| d.players[o].champ),
        benchmarks,
        gold,
        marks,
        kills,
        kill_participation: share(me.kills + me.assists, team_kills).filter(|_| team_kills > 0),
        damage_share: share(me.damage, team(|p| p.damage)),
        gold_share: share(me.gold, team(|p| p.gold)),
        taken_share: share(me.taken, team(|p| p.taken)),
        review,
        fights,
        recommended,
        takeaways: vec![],
        live: recap.is_some(),
    };
    a.takeaways = takeaways(&a, d, sd, team_kills);
    a
}

/// My deaths from the timeline, with what a live recap knew about each.
fn review(d: &MatchDetail, sd: &StaticData, recap: Option<&Recap>) -> DeathReview {
    // Participants 1–5 are blue (team 100, bottom-left).
    let blue = d.players.first().is_some_and(|p| p.ally);
    let live = recap.and_then(|r| r.review.as_ref());
    let deaths: Vec<DeathNote> = d
        .kills
        .iter()
        .filter(|k| k.victim == d.me)
        .map(|k| {
            let before = d.frames.iter().rev().find(|f| f.time <= k.time);
            let at = |i: usize| before.and_then(|f| f.players.get(i));
            let note = DeathNote {
                time: k.time,
                killer: k.killer.map(|i| d.players[i].champ),
                assisters: k.assisters.iter().map(|&i| d.players[i].champ).collect(),
                gold_gap: k.killer.and_then(|i| Some(at(d.me)?.total_gold as i32 - at(i)?.total_gold as i32)),
                killer_top_threat: false,
                unspent: before.filter(|f| k.time - f.time <= HELD_FRAME).and_then(|_| at(d.me)).map_or(0, |s| s.current_gold),
                buy_next: None,
                could_buy: false,
                owned: vec![],
                spikes: vec![],
                traded: false,
                place: k.pos.filter(|_| d.game.mode == Mode::Rift).map(|(x, y)| place(MapZone::at(x, y), blue).into()),
            };
            match live.and_then(|r| r.deaths.iter().find(|n| (n.time - k.time).abs() <= SAME_DEATH)) {
                Some(n) => DeathNote { place: note.place, traded: false, ..n.clone() },
                None => note,
            }
        })
        .collect();
    let kills: Vec<(f32, bool)> = d.kills.iter().map(|k| (k.time, !d.players[k.victim].ally)).collect();
    review_deaths(&deaths, &kills, d.game.mode != Mode::Rift, sd)
}

/// A map zone as seen from my side.
pub(crate) fn place(z: MapZone, blue: bool) -> &'static str {
    let mine = |b: bool| b == blue;
    match z {
        MapZone::BlueBase | MapZone::RedBase if mine(z == MapZone::BlueBase) => "your base",
        MapZone::BlueBase | MapZone::RedBase => "enemy base",
        MapZone::BlueJungle | MapZone::RedJungle if mine(z == MapZone::BlueJungle) => "your jungle",
        MapZone::BlueJungle | MapZone::RedJungle => "enemy jungle",
        MapZone::River => "river",
        MapZone::Top => "top lane",
        MapZone::Mid => "mid lane",
        MapZone::Bot => "bot lane",
    }
}

/// What the coach would build into this enemy team, as in champ select.
fn plan(d: &MatchDetail, sd: &StaticData, stats: Option<&ChampStats>) -> Vec<ItemId> {
    let Some(stats) = stats else { return vec![] };
    let me = &d.players[d.me];
    let foes: Vec<threat::Foe> = d.players.iter().filter(|p| !p.ally).map(|p| threat::generic_foe(champ(sd, p.champ))).collect();
    let needs = needs::needs(&foes);
    let mut ctx = items::Ctx::new(sd, Some(stats), champ(sd, me.champ), d.game.mode);
    ctx.support = d.game.mode == Mode::Rift && me.lane == Some(Lane::Support);
    items::build(&ctx, &[], &needs).path.iter().map(|s| s.id).collect()
}

fn clock(t: f32) -> String {
    let s = t.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn kilo(g: i32) -> String {
    format!("{:.1}k", g.abs() as f32 / 1000.0)
}

fn takeaways(a: &MatchAnalysis, d: &MatchDetail, sd: &StaticData, team_kills: u32) -> Vec<Takeaway> {
    let g = &a.game;
    let rift = g.mode == Mode::Rift;
    let me = &d.players[d.me];
    let mut out = Vec::new();
    let mut say = |good: bool, text: String| out.push(Takeaway { good, text });

    // Game flow: a lead thrown or a comeback, and the swing that decided it.
    let best = a.gold.iter().max_by_key(|p| p.team);
    let worst = a.gold.iter().min_by_key(|p| p.team);
    match (g.win, best, worst) {
        (false, Some(b), _) if b.team >= BIG_LEAD => say(false, format!("Your team led by {} at {} and lost", kilo(b.team), clock(b.time))),
        (true, _, Some(w)) if w.team <= -BIG_LEAD => say(true, format!("Came back from {} behind at {}", kilo(w.team), clock(w.time))),
        _ => {}
    }
    let mut swing: Option<(i32, f32, f32)> = None;
    for (i, p) in a.gold.iter().enumerate() {
        for q in a.gold[i + 1..].iter().take_while(|q| q.time - p.time <= SWING_SPAN) {
            let lost = p.team - q.team;
            // In a win, only a swing that put my team behind is a lesson.
            if lost >= SWING && (!g.win || q.team < 0) && swing.is_none_or(|s| lost > s.0) {
                swing = Some((lost, p.time, q.time));
            }
        }
    }
    if let Some((lost, from, to)) = swing {
        let mut why: Vec<String> = Vec::new();
        let died = d.kills.iter().filter(|k| k.victim == d.me && (from..=to).contains(&k.time)).count();
        if died > 0 {
            why.push(if died == 1 { "you died".into() } else { format!("you died {died}×") });
        }
        let lost_fights = a.kills.iter().filter(|k| !k.ours && (from..=to).contains(&k.time)).count();
        if lost_fights >= 2 {
            why.push(format!("{lost_fights} of your team died"));
        }
        for m in a.marks.iter().filter(|m| !m.ours && (from..=to).contains(&m.time) && m.kind != "Tower") {
            why.push(format!("they took {}", m.kind));
        }
        let why = if why.is_empty() { String::new() } else { format!(": {}", why.join(", ")) };
        say(false, format!("Biggest swing: −{} gold from {} to {}{why}", kilo(lost), clock(from), clock(to)));
    }

    // Lane.
    if let (true, Some(o)) = (rift, a.opponent) {
        let name = sd.champ_name(o);
        let gap = |v: i32, at: &str, lim: i32| {
            (v.abs() >= lim).then(|| (v > 0, format!("{} {name} by {v_abs} gold at {at}", if v > 0 { "Ahead of" } else { "Behind" }, v_abs = v.abs())))
        };
        let lane = match (g.gold_diff_15, g.gold_diff_10) {
            (Some(v), _) => gap(v, "15:00", LANE_GAP_15),
            (None, Some(v)) => gap(v, "10:00", LANE_GAP_10),
            _ => None,
        };
        if let Some((good, text)) = lane {
            say(good, text);
        }
    }

    // Per minute vs the champion average.
    let min = (g.duration_s as f32 / 60.0).max(1.0);
    let b = a.benchmarks.clone().unwrap_or(Benchmarks { cs_per_min: None, gold_per_min: None, damage_per_min: None, vision_per_min: None });
    let champ = sd.champ_name(g.champ);
    let mut vs = |label: &str, mine: f32, avg: Option<f32>, digits: usize| {
        let Some(avg) = avg.filter(|a| *a > 0.0) else { return };
        let r = mine / avg;
        if r < LOW || r >= HIGH {
            say(r >= HIGH, format!("{label} {mine:.digits$}/min vs {avg:.digits$} average on {champ}"));
        }
    };
    if rift && me.lane != Some(Lane::Support) {
        vs("CS", g.cs as f32 / min, b.cs_per_min, 1);
    }
    vs("Damage", g.damage_to_champs as f32 / min, b.damage_per_min, 0);
    if rift {
        vs("Vision", g.vision as f32 / min, b.vision_per_min, 2);
    }

    if rift && team_kills >= KP_MIN_KILLS
        && let Some(kp) = a.kill_participation
        && (kp >= KP_GOOD || kp < KP_BAD)
    {
        say(kp >= KP_GOOD, format!("Kill participation {:.0}%", kp * 100.0));
    }

    // Fights: dying first in lost ones, and the record in the ones I was in.
    let lost: Vec<&Fight> = a.fights.iter().filter(|f| f.lost > f.won && f.won + f.lost >= 2 && f.presence == Presence::There).collect();
    let first = lost.iter().filter(|f| f.died_first).count();
    if first >= PATTERN_MIN && first * 2 >= lost.len() {
        say(false, format!("You died first in {first} of {} lost fights you were in", lost.len()));
    }
    let mine: Vec<&Fight> = a.fights.iter().filter(|f| f.presence == Presence::There && f.won + f.lost >= 2).collect();
    let won = mine.iter().filter(|f| f.won > f.lost).count();
    if mine.len() >= FIGHTS_MIN && won as f32 / mine.len() as f32 >= FIGHTS_GOOD {
        say(true, format!("Your team won {won} of {} fights you were in", mine.len()));
    }

    // Objectives the enemy took while I was elsewhere (not dead).
    let away: Vec<String> = a
        .marks
        .iter()
        .filter(|m| !m.ours && m.presence == Some(Presence::Away))
        .map(|m| format!("{} {}", m.kind, clock(m.time)))
        .collect();
    if rift && away.len() >= PATTERN_MIN {
        let taken = a.marks.iter().filter(|m| !m.ours && m.presence.is_some()).count();
        say(false, format!("Away for {} of {taken} objectives they took ({})", away.len(), away.join(", ")));
    }

    let early = a.review.deaths.iter().filter(|n| n.time < EARLY_S && !n.traded).count();
    if rift && early >= EARLY_DEATHS {
        say(false, format!("Died {early}× before 14:00 outside won fights"));
    }

    if rift && let (Some(o), Some(t)) = (&g.ours, &g.theirs) {
        let total = o.dragons + t.dragons;
        if t.dragons >= 3 && o.dragons <= 1 {
            say(false, format!("They took {} of {total} dragons", t.dragons));
        } else if o.dragons >= 3 && t.dragons <= 1 {
            say(true, format!("Your team took {} of {total} dragons", o.dragons));
        }
    }
    // Bad first: what to work on leads.
    out.sort_by_key(|t| t.good);
    out
}
