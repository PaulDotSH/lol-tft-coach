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
/// Minute marks for the phase table.
const PHASE_MINUTES: [u32; 4] = [5, 10, 15, 20];
/// Gold held at a minute mark that counts as sitting on it.
const RICH_GOLD: u32 = 1500;
const RICH_MINUTES: u32 = 3;
/// Phase gaps worth a takeaway: CS at 15:00.
const CS_GAP_15: i32 = 20;
/// Wards placed below this share of my opponent's (supports, junglers).
const WARD_SHARE: f32 = 0.6;
const WARD_MIN_OPP: u32 = 10;
/// Earlier games on the champion before comparing, and at most this many takeaways from it.
const PAST_MIN: usize = 3;
const PAST_TAKEAWAYS: usize = 2;
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
    let at = |minute: u32| d.frames.iter().find(|f| f.time >= minute as f32 * 60.0 && f.time <= g.duration_s as f32);
    let phases = PHASE_MINUTES
        .iter()
        .filter_map(|&minute| {
            let f = at(minute)?;
            let (m, o) = (&f.players[d.me], opp.map(|o| &f.players[o]));
            Some(PhaseRow {
                minute,
                cs: m.cs,
                level: m.level,
                opp_cs: o.map(|o| o.cs),
                opp_level: o.map(|o| o.level),
                gold_lead: o.map(|o| m.total_gold as i32 - o.total_gold as i32),
            })
        })
        .collect();
    let rich_minutes = if g.mode == Mode::Rift {
        d.frames.iter().filter(|f| f.time >= 180.0 && f.players[d.me].current_gold >= RICH_GOLD).count() as u32
    } else {
        0
    };
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
        phases,
        rich_minutes,
        vs_past: None,
        focus: None,
    };
    a.takeaways = takeaways(&a, d, sd, team_kills);
    a.focus = focus(&a, d, sd);
    a
}

fn round1(x: f32) -> f32 {
    (x * 10.0).round() / 10.0
}

/// The one thing to work on next game: early deaths, then gold held, fight
/// openings, CS, the lane, objectives and vision, the first that applies.
fn focus(a: &MatchAnalysis, d: &MatchDetail, sd: &StaticData) -> Option<Focus> {
    let g = &a.game;
    if g.mode != Mode::Rift || g.remake {
        return None;
    }
    let me = &d.players[d.me];
    let min = (g.duration_s as f32 / 60.0).max(1.0);
    let b = a.benchmarks.as_ref();
    let goal = |kind: GoalKind, target: f32| Some(Goal { kind, target });
    let pick = |text: String, why: String, goal: Option<Goal>| Some(Focus { text, why, goal });

    let early = a.review.deaths.iter().filter(|n| n.time < EARLY_S && !n.traded).count();
    if early >= PATTERN_MIN {
        return pick(
            "At most 1 death before 14:00".into(),
            format!("You died {early}× before 14:00 outside fights your team won."),
            goal(GoalKind::EarlyDeaths, 1.0),
        );
    }
    let held = a.review.deaths.iter().filter(|n| n.unspent >= 1000).count();
    if held >= PATTERN_MIN || a.rich_minutes >= RICH_MINUTES {
        let why = if held >= PATTERN_MIN {
            format!("You died {held}× holding 1000+ gold.")
        } else {
            format!("You held {RICH_GOLD}+ gold at {} minute marks.", a.rich_minutes)
        };
        return pick("Back and spend before you sit on 1000+ gold".into(), why, goal(GoalKind::Unspent, 1000.0));
    }
    let lost: Vec<&Fight> = a.fights.iter().filter(|f| f.lost > f.won && f.won + f.lost >= 2 && f.presence == Presence::There).collect();
    let first = lost.iter().filter(|f| f.died_first).count();
    if first >= PATTERN_MIN && first * 2 >= lost.len() {
        return pick(
            "Let your frontline start fights; stay behind it".into(),
            format!("You died first in {first} of {} lost fights you were in.", lost.len()),
            None,
        );
    }
    let cs = g.cs as f32 / min;
    if me.lane != Some(Lane::Support)
        && let Some(avg) = b.and_then(|b| b.cs_per_min).filter(|avg| cs < LOW * avg)
    {
        let target = round1((cs + 1.0).min(avg));
        return pick(
            format!("Reach {target:.1} CS/min by 10:00"),
            format!("You had {cs:.1} CS/min against {avg:.1} on average for {}.", sd.champ_name(g.champ)),
            goal(GoalKind::Cs10, target),
        );
    }
    if let (Some(o), Some(v)) = (a.opponent, g.gold_diff_15.filter(|v| *v <= -LANE_GAP_15)) {
        return pick(
            format!("Play the lane safer against {}: farm until your first item", sd.champ_name(o)),
            format!("You were {} gold behind at 15:00.", -v),
            None,
        );
    }
    let away = a.marks.iter().filter(|m| !m.ours && m.presence == Some(Presence::Away)).count();
    if away >= PATTERN_MIN {
        return pick("Move to objectives a minute before they spawn".into(), format!("You were away for {away} objectives they took."), None);
    }
    let vision = g.vision as f32 / min;
    let vision_avg = b.and_then(|b| b.vision_per_min);
    if vision_avg.is_some_and(|avg| vision < LOW * avg) || (me.control_wards == 0 && g.duration_s >= 900) {
        let target = round1(vision_avg.unwrap_or(vision + 0.3).max(vision + 0.1));
        let why = match vision_avg {
            Some(avg) => format!("Vision {vision:.2}/min against {avg:.2} on average; {} Control Wards bought.", me.control_wards),
            None => format!("{} Control Wards bought.", me.control_wards),
        };
        return pick(format!("Vision {target:.1}/min: a Control Ward every back"), why, goal(GoalKind::Vision, target));
    }
    None
}

/// Adds this game against my earlier games on the same champion and mode
/// (`past`: the match history), with up to two takeaways from it.
pub fn compare_past(a: &mut MatchAnalysis, past: &[PastGame], sd: &StaticData) {
    let g = a.game.clone();
    let earlier: Vec<&PastGame> = past
        .iter()
        .filter(|p| p.champ == g.champ && p.mode == g.mode && p.game_id != g.game_id && !p.remake && p.created < g.created)
        .collect();
    if earlier.len() < PAST_MIN || g.remake {
        return;
    }
    let per_min = |v: u32, p: &PastGame| v as f32 / (p.duration_s as f32 / 60.0).max(1.0);
    let kda = |p: &PastGame| (p.kills + p.assists) as f32 / p.deaths.max(1) as f32;
    let mut rows: Vec<CompareRow> = Vec::new();
    let mut row = |label: &str, this: f32, vals: Vec<f32>, lower_better: bool, digits: u32| {
        if vals.is_empty() {
            return;
        }
        let avg = vals.iter().sum::<f32>() / vals.len() as f32;
        let best = if lower_better { vals.iter().copied().fold(f32::MAX, f32::min) } else { vals.iter().copied().fold(f32::MIN, f32::max) };
        rows.push(CompareRow { label: label.into(), this, avg, best, lower_better, digits });
    };
    let rift = g.mode == Mode::Rift;
    row("Deaths", g.deaths as f32, earlier.iter().map(|p| p.deaths as f32).collect(), true, 0);
    row("KDA", kda(&g), earlier.iter().map(|p| kda(p)).collect(), false, 1);
    if rift && g.lane != Some(Lane::Support) {
        row("CS / min", per_min(g.cs, &g), earlier.iter().map(|p| per_min(p.cs, p)).collect(), false, 1);
    }
    row("Damage / min", per_min(g.damage_to_champs, &g), earlier.iter().map(|p| per_min(p.damage_to_champs, p)).collect(), false, 0);
    if rift {
        row("Vision / min", per_min(g.vision, &g), earlier.iter().map(|p| per_min(p.vision, p)).collect(), false, 2);
    }
    if let Some(v) = g.gold_diff_15 {
        let vals = earlier.iter().filter_map(|p| p.gold_diff_15.map(|x| x as f32)).collect();
        row("Gold vs opponent at 15:00", v as f32, vals, false, 0);
    }

    let champ = sd.champ_name(g.champ);
    let n = earlier.len() + 1;
    let mut notes: Vec<Takeaway> = Vec::new();
    for r in &rows {
        let fmt = |v: f32| format!("{v:.prec$}", prec = r.digits as usize);
        let better = if r.lower_better { r.this < r.best } else { r.this > r.best };
        // Deaths: three over my usual; per-minute stats well under it.
        let worse = if r.lower_better { r.this >= r.avg + 3.0 } else { r.label.ends_with("/ min") && r.this < LOW * r.avg };
        if worse {
            notes.push(Takeaway { good: false, text: format!("{} {} vs your usual {} on {champ}", r.label, fmt(r.this), fmt(r.avg)) });
        } else if better {
            let what = if r.lower_better { format!("Fewest {}", r.label.to_lowercase()) } else { format!("Your best {}", r.label) };
            notes.push(Takeaway { good: true, text: format!("{what} in {n} {champ} games ({})", fmt(r.this)) });
        }
    }
    notes.sort_by_key(|t| t.good);
    a.takeaways.extend(notes.into_iter().take(PAST_TAKEAWAYS));
    a.takeaways.sort_by_key(|t| t.good);
    a.vs_past = Some(PastCompare { games: earlier.len() as u32, wins: earlier.iter().filter(|p| p.win).count() as u32, rows });
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
                pos: k.pos.filter(|_| d.game.mode == Mode::Rift).map(|(x, y)| MapZone::from_blue(x, y, blue)),
            };
            match live.and_then(|r| r.deaths.iter().find(|n| (n.time - k.time).abs() <= SAME_DEATH)) {
                Some(n) => DeathNote { place: note.place, pos: note.pos, traded: false, ..n.clone() },
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

    // Wards: Control Wards bought, and placed against my opponent's (supports, junglers).
    if rift && g.duration_s >= 900 && me.control_wards == 0 {
        say(false, "No Control Wards bought".into());
    }
    let opp = d.players.iter().find(|p| !p.ally && Some(p.champ) == a.opponent);
    if rift
        && matches!(me.lane, Some(Lane::Support | Lane::Jungle))
        && let Some(o) = opp.filter(|o| o.wards_placed >= WARD_MIN_OPP && (me.wards_placed as f32) < WARD_SHARE * o.wards_placed as f32)
    {
        say(false, format!("Placed {} wards vs {} for {}", me.wards_placed, o.wards_placed, sd.champ_name(o.champ)));
    }

    // Phases: levels at 10:00, CS at 15:00, gold sat on.
    if rift && let Some(o) = a.opponent {
        let name = sd.champ_name(o);
        let row = |m: u32| a.phases.iter().find(|r| r.minute == m);
        if let Some(r) = row(10)
            && let Some(ol) = r.opp_level.filter(|ol| *ol > r.level)
        {
            say(false, format!("Level {} vs {name} {ol} at 10:00", r.level));
        }
        if me.lane != Some(Lane::Support)
            && let Some(r) = row(15)
            && let Some(oc) = r.opp_cs
        {
            let gap = r.cs as i32 - oc as i32;
            if gap.abs() >= CS_GAP_15 {
                say(gap > 0, format!("CS {} vs {name} {oc} at 15:00", r.cs));
            }
        }
    }
    if a.rich_minutes >= RICH_MINUTES {
        say(false, format!("Held {RICH_GOLD}+ gold at {} minute marks: back and spend sooner", a.rich_minutes));
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
