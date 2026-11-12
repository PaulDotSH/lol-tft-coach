//! Posture: a standing read of the next minutes from the win probability's standing state
//! (`base`), who the rest of the game favours (`Outlook`) and what is up next. A read and the
//! choices the scoreboard offers, worded as the game-plan card is: never orders.

use crate::calls::{DEAD_FOR, ITEM_LEAD, STREAK, and_list, clock};
use crate::consts::*;
use model::*;

/// The stance across ticks; reset when the clock goes back (a new game).
#[derive(Debug, Clone, Default)]
pub struct PostureMemo {
    pub stance: Option<Stance>,
    /// Game time the stance took hold.
    pub since: f32,
    /// Stance changes this game; numbers the calls.
    changes: u32,
    last: f32,
}

/// "item gold +9 points".
pub(crate) fn factor_line(f: &WinFactor) -> String {
    format!("{} {} points", f.label, signed(f.points, 0))
}

fn signed(x: f32, digits: usize) -> String {
    let s = format!("{:.*}", digits, x.abs());
    if x < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') { format!("−{s}") } else { format!("+{s}") }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

/// +1 the rest of the game favours me, −1 them, 0 nobody (or no curves).
fn favours(o: Option<&Outlook>) -> i8 {
    match o.map(|o| o.later - o.now) {
        Some(d) if d > PLAN_EVEN => 1,
        Some(d) if d < -PLAN_EVEN => -1,
        _ => 0,
    }
}

/// Ahead +1, even 0, behind −1; leaving the band of `held` takes `POSTURE_MARGIN` past its threshold.
fn band(base: f32, held: Option<i8>) -> i8 {
    let margin = |b: i8| match held {
        None => 0.0,
        Some(h) if h == b => -POSTURE_MARGIN,
        _ => POSTURE_MARGIN,
    };
    if base >= POSTURE_AHEAD + margin(1) {
        1
    } else if base <= POSTURE_BEHIND - margin(-1) {
        -1
    } else {
        0
    }
}

fn band_of(s: Stance) -> i8 {
    match s {
        Stance::Press | Stance::Hold => 1,
        Stance::Even => 0,
        Stance::Stall | Stance::Swing => -1,
    }
}

impl PostureMemo {
    /// The stance for this tick (the table, the hysteresis and the dwell) and its read.
    pub fn decide(&mut self, win: &WinProb, a: &GameAdvice, snap: &GameSnapshot, sd: &StaticData) -> Posture {
        if snap.time < self.last {
            *self = PostureMemo::default();
        }
        self.last = snap.time;
        let time = favours(win.outlook.as_ref());
        let want = match band(win.base, self.stance.map(band_of)) {
            1 if time > 0 => Stance::Hold,
            1 => Stance::Press,
            0 => Stance::Even,
            _ if time > 0 => Stance::Stall,
            _ => Stance::Swing,
        };
        match self.stance {
            None => (self.stance, self.since) = (Some(want), snap.time),
            Some(s) if s != want && snap.time - self.since >= POSTURE_DWELL => {
                (self.stance, self.since) = (Some(want), snap.time);
                self.changes += 1;
            }
            _ => {}
        }
        read(self.stance.unwrap_or(want), time, win, a, snap, sd)
    }

    /// One call while a changed stance is new.
    fn call(&self, p: &Posture, now: f32) -> Option<Call> {
        (self.changes > 0 && now - self.since < POSTURE_CALL_S).then(|| Call {
            key: format!("stance-{}-{}", format!("{:?}", p.stance).to_lowercase(), self.changes),
            text: p.headline.clone(),
            say: match p.stance {
                Stance::Press => "You're ahead. The lead is worth most now.",
                Stance::Hold => "You're ahead, and time is on your side.",
                Stance::Even => "Even game. The next objective decides.",
                Stance::Stall => "Behind, but time is on your side.",
                Stance::Swing => "Behind, and time is against you.",
            }
            .into(),
        })
    }
}

/// Sets `a.posture` from `a.win` (None without it) and adds the stance-change call.
pub fn annotate(a: &mut GameAdvice, memo: &mut PostureMemo, snap: &GameSnapshot, sd: &StaticData) {
    let Some(win) = &a.win else {
        a.posture = None;
        return;
    };
    let p = memo.decide(win, a, snap, sd);
    if let Some(c) = memo.call(&p, snap.time) {
        a.calls.push(c);
    }
    a.posture = Some(p);
}

fn read(stance: Stance, time: i8, win: &WinProb, a: &GameAdvice, snap: &GameSnapshot, sd: &StaticData) -> Posture {
    let rift = snap.mode == Mode::Rift;
    let objectives: Vec<&Stake> = win.stakes.iter().filter(|s| s.name != "Your death" && (rift || !matches!(s.name.as_str(), "Dragon" | "Baron" | "Elder"))).collect();
    let stake = |s: &Stake| {
        let when = a.timers.iter().find(|t| t.name == s.name).map(|t| match t.next_spawn {
            Some(at) if at > snap.time => format!("{} in {}", s.name, clock(at - snap.time)),
            _ => format!("{} up now", s.name),
        });
        let note = s.note.as_ref().map_or(String::new(), |n| format!(", {n}"));
        format!("{}, ±{:.0} points{note}", when.unwrap_or_else(|| format!("next {}", s.name.to_lowercase())), s.swing)
    };
    let top = objectives.first().map(|s| capital(&stake(s)));
    let least = objectives.last().map(|s| format!("Least to lose: {}", stake(s)));
    let death = win.stakes.iter().find(|s| s.name == "Your death").map(|s| {
        format!("Dying now costs {:.0} points{}", s.swing, s.note.as_ref().map_or(String::new(), |n| format!(" ({n})")))
    });
    let shutdown = a.bounties.iter().find(|b| !b.ally && (b.streak >= STREAK || b.item_lead >= ITEM_LEAD)).map(|b| {
        let name = sd.champ_name(b.champ);
        if b.streak >= STREAK {
            format!("{name} is on a {}-kill streak: a big shutdown", b.streak)
        } else {
            format!("{name} holds {:.1}k item gold over their team: a shutdown", b.item_lead as f32 / 1000.0)
        }
    });
    let dead_list = |v: &[Respawn]| and_list(v.iter().filter(|r| r.in_s >= DEAD_FOR).map(|r| format!("{} ({:.0} s)", sd.champ_name(r.champ), r.in_s)).collect());
    let theirs = Some(dead_list(&a.deaths.enemies)).filter(|s| !s.is_empty()).map(|s| format!("Dead on their side: {s}"));
    let mine = Some(dead_list(&a.deaths.allies)).filter(|s| !s.is_empty()).map(|s| format!("Down on your side: {s}"));
    let item = a.gold_eta.as_ref().and_then(|e| {
        let name = sd.item_name(e.item?);
        if e.missing == 0 {
            Some(format!("{name} is affordable now"))
        } else {
            e.eta_s.map(|t| if t < 60.0 { format!("{name} in ~{t:.0} s at your income") } else { format!("{name} in ~{} at your income", clock(t)) })
        }
    });

    let (headline, options) = match stance {
        Stance::Press => ("Ahead: the lead is worth most now".to_string(), [top, theirs, shutdown, death]),
        Stance::Hold => ("Ahead, and time is on your side: nothing has to be forced".to_string(), [top, death, item, theirs]),
        Stance::Even => {
            let h = objectives.first().map_or("Even: the next objective decides".to_string(), |s| {
                let what = if a.timers.iter().any(|t| t.name == s.name) { s.name.clone() } else { format!("the next {}", s.name.to_lowercase()) };
                format!("Even: {what} decides most (±{:.0} points)", s.swing)
            });
            (h, [top, theirs, item, mine])
        }
        Stance::Stall => ("Behind, but time is on your side: the game comes to you".to_string(), [least, item, death, mine]),
        Stance::Swing if time < 0 => ("Behind, and time is against you: it takes a swing".to_string(), [shutdown, theirs, top, item]),
        Stance::Swing => ("Behind, and waiting does not close the gap".to_string(), [shutdown, theirs, top, item]),
    };

    let mut why: Vec<String> = win.factors.iter().filter(|f| f.key != "side").take(POSTURE_WHY).map(factor_line).collect();
    if let Some(o) = &win.outlook {
        let numbers = format!("{} now, {} at {} min", signed(o.now, 1), signed(o.later, 1), o.later_min);
        why.push(match favours(Some(o)) {
            1 => format!("Draft and scaling favour you from here: {numbers}"),
            -1 => format!("Draft and scaling favour them from here: {numbers}"),
            _ => format!("Draft and scaling: about even from here ({numbers})"),
        });
    }
    Posture { stance, headline, why, options: options.into_iter().flatten().take(POSTURE_OPTIONS).collect() }
}
