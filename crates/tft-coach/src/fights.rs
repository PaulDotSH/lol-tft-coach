//! Which player each of my fights was against. The fight's opponent never changes, so a name plate
//! read at any moment of it links the whole fight (`viewing`: exactly one other plate on screen).
//! Once the next round shows, the player list's HP rows shrink the candidates: after a fight I won
//! my opponent's HP dropped (by about the stage's damage plus my survivors'), after one I lost it
//! didn't. Dead players are out; with an odd number alive one fight is against a ghost army, and a
//! fight no living player explains is that one: skipped.

use crate::scout::{carousel, pve, same_name};
use crate::tracker::Stage;
use tft_model::*;

/// Index into `Fights::players`.
pub type PlayerIdx = usize;

/// Other players kept at most.
const MAX_PLAYERS: usize = 7;
/// Player damage after a won fight (Set 18, not measured): a stage's base range, by stage 1..=7+.
const STAGE_DAMAGE: [(u32, u32); 7] = [(0, 0), (0, 3), (2, 7), (5, 9), (7, 12), (9, 15), (12, 20)];
/// Damage per surviving unit of mine.
const UNIT_DAMAGE: (u32, u32) = (1, 3);

#[derive(Debug, Clone, PartialEq)]
pub struct Fight {
    pub round: Stage,
    /// Players it may have been against (alive, and their HP change fits the result).
    pub candidates: Vec<PlayerIdx>,
    /// Linked by a name plate.
    pub opponent: Option<PlayerIdx>,
}

#[derive(Debug, Clone)]
struct Open {
    fight: Fight,
    before: Vec<Option<u32>>,
    my_before: Option<u32>,
    plates: Vec<PlayerIdx>,
    survivors: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct Fights {
    names: Vec<String>,
    /// Confirmed HP per player and the last read, which confirms the next that agrees.
    hp: Vec<Option<u32>>,
    hp_read: Vec<Option<u32>>,
    my_hp: Option<u32>,
    my_hp_read: Option<u32>,
    open: Option<Open>,
    done: Vec<Fight>,
    /// The previous read's stage: a fight is resolved on the second read past it (its HP confirmed).
    last_stage: Option<Stage>,
    pub(crate) single_reads: bool,
}

/// HP lost after a won fight at `stage` with `survivors` of my units standing.
pub fn expected_drop(stage: u32, survivors: u32) -> (u32, u32) {
    let (lo, hi) = STAGE_DAMAGE[(stage.max(1) as usize - 1).min(STAGE_DAMAGE.len() - 1)];
    (lo + survivors * UNIT_DAMAGE.0, hi + survivors * UNIT_DAMAGE.1)
}

impl Fights {
    /// Replays of sparse captures: every HP read counts at once.
    pub fn single_reads(mut self) -> Fights {
        self.single_reads = true;
        self
    }

    /// Players by first sighting (player list or plate).
    pub fn players(&self) -> &[String] {
        &self.names
    }

    /// Fights whose outcome is known, oldest first (ghost fights left out).
    pub fn done(&self) -> &[Fight] {
        &self.done
    }

    /// The fight on screen (or waiting for the next round's HP), as linked so far.
    pub fn current(&self) -> Option<&Fight> {
        self.open.as_ref().map(|o| &o.fight)
    }

    fn index(&mut self, name: &str) -> Option<PlayerIdx> {
        if let Some(i) = self.names.iter().position(|n| same_name(n, name)) {
            return Some(i);
        }
        if self.names.len() >= MAX_PLAYERS {
            return None;
        }
        self.names.push(name.to_string());
        self.hp.push(None);
        self.hp_read.push(None);
        Some(self.names.len() - 1)
    }

    pub fn observe(&mut self, snap: &TftSnapshot) {
        let single = self.single_reads;
        let confirm = |hp: &mut Option<u32>, read: &mut Option<u32>, now: u32| {
            if read.replace(now) == Some(now) || single {
                *hp = Some(now);
            }
        };
        for (name, now) in &snap.lobby {
            if let Some(i) = self.index(name) {
                confirm(&mut self.hp[i], &mut self.hp_read[i], *now);
            }
        }
        if let Some(now) = snap.hp {
            confirm(&mut self.my_hp, &mut self.my_hp_read, now);
        }
        let Some(stage) = snap.stage else { return };
        let past = |round: Stage| round < stage && (single || self.last_stage.is_some_and(|s| s > round));
        if self.open.as_ref().is_some_and(|o| past(o.fight.round)) {
            self.resolve();
        }
        self.last_stage = Some(stage);
        let fight = !snap.foes.is_empty() && !snap.viewing_other && !pve(stage) && !carousel(stage);
        if !fight {
            return;
        }
        if self.open.is_none() && self.done.last().is_none_or(|f| f.round < stage) {
            self.open = Some(Open {
                fight: Fight { round: stage, candidates: vec![], opponent: None },
                before: self.hp.clone(),
                my_before: self.my_hp,
                plates: vec![],
                survivors: None,
            });
        }
        let plate = snap.viewing.as_deref().and_then(|n| self.index(n));
        let Some(o) = self.open.as_mut().filter(|o| o.fight.round == stage) else { return };
        o.survivors = Some(snap.units.iter().filter(|u| u.bench.is_none()).count() as u32);
        if let Some(i) = plate {
            o.plates.push(i);
        }
        o.fight.opponent = o.plates.first().copied().filter(|p| o.plates.iter().all(|q| q == p));
        o.fight.candidates = match o.fight.opponent {
            Some(p) => vec![p],
            None => (0..self.names.len()).filter(|&i| o.before.get(i).copied().flatten() != Some(0)).collect(),
        };
    }

    /// The round is over: the HP rows now tell who could have been my opponent.
    fn resolve(&mut self) {
        let Some(o) = self.open.take() else { return };
        let before = |i: usize| o.before.get(i).copied().flatten();
        let alive: Vec<PlayerIdx> = (0..self.names.len()).filter(|&i| before(i) != Some(0)).collect();
        let change = |i: usize| before(i).zip(self.hp[i]).map(|(b, a)| b.saturating_sub(a));
        let won = o.my_before.zip(self.my_hp).map(|(b, a)| a >= b);
        let mut candidates = match won {
            Some(true) => {
                let dropped: Vec<PlayerIdx> = alive.iter().copied().filter(|&i| change(i).is_none_or(|d| d > 0)).collect();
                let (lo, hi) = expected_drop(o.fight.round.0, o.survivors.unwrap_or(0));
                let fits: Vec<PlayerIdx> = dropped.iter().copied().filter(|&i| change(i).is_none_or(|d| (lo..=hi).contains(&d))).collect();
                if o.survivors.is_some() && !fits.is_empty() { fits } else { dropped }
            }
            Some(false) => alive.iter().copied().filter(|&i| change(i).is_none_or(|d| d == 0)).collect(),
            None => alive.clone(),
        };
        if candidates.is_empty() {
            if (alive.len() + 1) % 2 == 1 {
                return;
            }
            candidates = alive;
        }
        let opponent = o.fight.opponent.filter(|p| candidates.contains(p));
        if let Some(p) = opponent {
            candidates = vec![p];
        }
        self.done.push(Fight { round: o.fight.round, candidates, opponent });
    }
}
