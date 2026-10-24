//! Per-game observations kept across ticks by the orchestrator: my gold over
//! time (income) and when each player's legendaries first appeared. The coach
//! only reads it.

use crate::consts::*;
use model::*;
use std::collections::VecDeque;

#[derive(Debug, Clone, Default)]
pub struct Tracker {
    /// (game time, my gold), oldest first, about INCOME_WINDOW long.
    gold: VecDeque<(f32, f32)>,
    /// (player index, legendary, game time first seen); None = held when tracking began.
    seen: Vec<(usize, ItemId, Option<f32>)>,
    started: bool,
    last_time: f32,
    /// (game time, my gold) every HOLD_STEP s over the last HOLD_WINDOW s.
    held: VecDeque<(f32, f32)>,
    /// (player index, level, game time first seen); None = reached before tracking began.
    levels: Vec<(usize, u32, Option<f32>)>,
}

const HOLD_STEP: f32 = 5.0;
const HOLD_WINDOW: f32 = 300.0;

pub fn is_legendary(it: &Item) -> bool {
    it.completed && !it.boots
}

impl Tracker {
    pub fn observe(&mut self, snap: &GameSnapshot, sd: &StaticData) {
        if snap.time < self.last_time {
            *self = Tracker::default(); // clock went back: a different game
        }
        if self.gold.back().is_none_or(|&(t, _)| snap.time > t) {
            self.gold.push_back((snap.time, snap.my_gold));
            // Keep one sample at or before the window start so the window stays full.
            while self.gold.len() > 2 && self.gold[1].0 <= snap.time - INCOME_WINDOW {
                self.gold.pop_front();
            }
        }
        if self.held.back().is_none_or(|&(t, _)| snap.time - t >= HOLD_STEP) {
            self.held.push_back((snap.time, snap.my_gold));
            while self.held.front().is_some_and(|&(t, _)| t < snap.time - HOLD_WINDOW) {
                self.held.pop_front();
            }
        }
        for (i, p) in snap.players.iter().enumerate() {
            for &id in &p.items {
                if sd.items.get(&id).is_some_and(is_legendary) && !self.seen.iter().any(|s| s.0 == i && s.1 == id) {
                    self.seen.push((i, id, self.started.then_some(snap.time)));
                }
            }
            for l in 1..=p.level {
                if !self.levels.iter().any(|s| s.0 == i && s.1 == l) {
                    self.levels.push((i, l, self.started.then_some(snap.time)));
                }
            }
        }
        self.started = true;
        self.last_time = snap.time;
    }

    /// Gold per second: gains over the window divided by the time they took.
    /// Intervals where gold dropped (a purchase) are left out entirely.
    pub fn income(&self) -> Option<f32> {
        let (mut gain, mut span) = (0.0, 0.0);
        for (a, b) in self.gold.iter().zip(self.gold.iter().skip(1)) {
            if b.1 >= a.1 {
                gain += b.1 - a.1;
                span += b.0 - a.0;
            }
        }
        (span >= INCOME_MIN_SPAN).then(|| gain / span)
    }

    /// How long (s) my gold has stayed at or above `amount` up to now.
    pub fn held_for(&self, amount: f32, now: f32) -> f32 {
        let mut since = None;
        for &(t, g) in self.held.iter().rev() {
            if g < amount {
                break;
            }
            since = Some(t);
        }
        since.map_or(0.0, |t| now - t)
    }

    /// When player `i` reached `level` (None: not yet, or before tracking began).
    pub fn reached(&self, i: usize, level: u32) -> Option<f32> {
        self.levels.iter().find(|s| s.0 == i && s.1 == level)?.2
    }

    /// Legendaries player `i` finished within ITEM_NEW_WINDOW of `now` and still holds.
    pub fn new_items(&self, i: usize, held: &[ItemId], now: f32) -> Vec<ItemId> {
        self.seen
            .iter()
            .filter(|s| s.0 == i && held.contains(&s.1) && s.2.is_some_and(|t| now - t <= ITEM_NEW_WINDOW))
            .map(|s| s.1)
            .collect()
    }
}
