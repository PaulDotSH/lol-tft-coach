//! What I own, kept across ticks from my own screen. With HP bars on screen, the pieces on my
//! board and bench (`board`) are what I own. Without them (or before the first quiet read):
//! shop purchases (a slot empties while gold drops by that unit's cost), best-effort sells,
//! ★ from copies, corrected by the trait panel.
//! Other players' units (scouted boards, my fights' opponents) are kept by `scout`.
//! Every snapshot field may be unreadable on any tick: None never counts as 0.

use crate::board;
use crate::fights::Fights;
use crate::library::Library;
use crate::scout::Scout;
use std::collections::{HashSet, VecDeque};
use tft_model::*;

/// (stage, round), e.g. (4, 2) for 4-2.
pub type Stage = (u32, u32);
/// A trait panel line: (trait, unique units).
pub type TraitCount = (Api, u32);

/// Copies that make ★2 and ★3.
pub const STAR2_COPIES: u32 = 3;
pub const STAR3_COPIES: u32 = 9;
/// Gold per XP buy.
pub const XP_COST: u32 = 4;
/// XP buys tried when matching a gold drop to emptied shop slots.
const MAX_XP_BUYS: u32 = 3;
/// HP readings kept, one per round.
const HP_ROUNDS: usize = 8;
/// Units owned up to this stage count as starting units (Choncc's 4/5-cost).
pub const EARLY_STAGE: u32 = 2;
/// Rounds a unit may be off my trait panel (sold, or sitting on the bench) before it stops
/// counting as owned: ★1 units, then ★2+ ones (kept longer, they're rarely sold).
const OFF_PANEL_ROUNDS: u32 = 4;
const OFF_PANEL_ROUNDS_STARRED: u32 = 6;
/// A free board slot is called after this many reads in a row show it (one misread is no call).
const FREE_READS: u32 = 2;

/// ★ from copies.
pub fn star(copies: u32) -> u32 {
    if copies >= STAR3_COPIES {
        3
    } else if copies >= STAR2_COPIES {
        2
    } else {
        1
    }
}

/// Rounds since the game began: 1-1..1-4 are 1..4, 2-1 is 5, each later stage has 7.
pub fn round_index((stage, round): (u32, u32)) -> u32 {
    if stage <= 1 { round } else { 4 + (stage - 2) * 7 + round }
}

/// One unit I own, in the order I got it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tracked {
    pub api: Api,
    pub copies: u32,
    /// Stage when first owned (None: stage unread so far).
    pub since: Option<(u32, u32)>,
    /// Named from the trait panel alone (carousel, orbs, a starting unit), never seen bought.
    pub seen: bool,
    /// Rounds in a row some trait of it was missing from my panel.
    pub off_panel: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Tracker {
    units: Vec<Tracked>,
    /// Trait counts my tracked units don't explain: (trait, units).
    unknown: Vec<(Api, u32)>,
    /// My last readable trait panel and the first one read (stage 1/2 only).
    panel: Vec<(Api, u32)>,
    first_panel: Option<(Stage, Vec<TraitCount>)>,
    items: Vec<Api>,
    /// Items on my units as last read.
    worn: Vec<Api>,
    /// Other players' units.
    scout: Scout,
    /// Which player each fight was against.
    fights: Fights,
    stage: Option<(u32, u32)>,
    /// Last tick where gold and shop were both readable: the baseline buys compare to.
    gold: Option<u32>,
    shop: Vec<Option<Api>>,
    /// Last tick's buys (slot, unit): undone when the slot shows the unit again (a misread).
    bought: Vec<(usize, Api)>,
    /// (round_index, HP), one per round, oldest first.
    hp: VecDeque<(u32, u32)>,
    /// The last HP read, confirmed by the next that agrees with it.
    hp_read: Option<u32>,
    /// Round of the last off-panel check.
    checked: u32,
    /// My pieces by their HP bars, named.
    board: board::Board,
    /// Units' looks, kept across games (the app loads and saves it).
    library: Library,
    /// Last team size read (units on board, cap) and reads in a row with a free slot.
    team: Option<(u32, u32)>,
    free_reads: u32,
    /// How far the cap runs above my level (bonus slots), from reads with both; and the last level read.
    team_bonus: u32,
    level: Option<u32>,
}

fn count(panel: &[(Api, u32)], t: &str) -> u32 {
    panel.iter().find(|(x, _)| x == t).map_or(0, |p| p.1)
}

fn cost(set: &TftSetData, api: &str) -> Option<u32> {
    set.units.get(api).map(|u| u.cost)
}

impl Tracker {
    pub fn observe(&mut self, snap: &TftSnapshot, set: &TftSetData) {
        if let (Some(now), Some(was)) = (snap.stage, self.stage)
            && now < was
        {
            // Stage went back: a new game (what units look like carries over).
            let (library, single) = (std::mem::take(&mut self.library), self.board.single_reads);
            *self = Tracker::default();
            self.library = library;
            (self.board.single_reads, self.scout.single_reads, self.fights.single_reads) = (single, single, single);
        }
        let same_stage = snap.stage.is_some() && snap.stage == self.stage;
        if snap.stage.is_some() {
            self.stage = snap.stage;
        }
        // The augment screen covers the board: only the offer is worth reading.
        if !snap.augment_offer.is_empty() {
            return;
        }
        self.fights.observe(snap);
        let round = self.round();
        let owned: Vec<(Api, u32)> = self.owned().into_iter().map(|o| (o.api, o.copies)).collect();
        self.scout.observe(snap, set, &self.library, round, &self.panel, &owned);
        if snap.viewing_other {
            return;
        }
        if let Some(l) = snap.level {
            self.level = Some(l);
        }
        // The counter only shows in planning: unread ticks keep the last read.
        if let Some((n, cap)) = snap.team_size {
            let was_free = self.team.is_some_and(|(n, c)| n < c && c == cap);
            self.free_reads = if n >= cap { 0 } else if was_free { self.free_reads + 1 } else { 1 };
            self.team = Some((n, cap));
            if let Some(l) = snap.level.filter(|&l| cap >= l) {
                self.team_bonus = cap - l;
            }
        }
        // One read can be wrong ("4" for 94, another row's 100): it counts once the next agrees.
        if let Some(hp) = snap.hp.filter(|&h| self.hp_read.replace(h) == Some(h) || self.board.single_reads) {
            match self.hp.back_mut() {
                Some(last) if last.0 == round => last.1 = hp,
                _ => self.hp.push_back((round, hp)),
            }
            while self.hp.len() > HP_ROUNDS {
                self.hp.pop_front();
            }
        }
        if let Some(gold) = snap.gold
            && !snap.shop.is_empty()
        {
            if let Some(was) = self.gold {
                self.undo_misreads(snap, gold, was);
                if gold < was {
                    self.buys(snap, set, was - gold);
                } else if gold > was && same_stage {
                    self.sells(&snap.traits, set, gold - was);
                }
            }
            self.gold = Some(gold);
            self.shop = snap.shop.clone();
        }
        if !snap.traits.is_empty() {
            self.off_panel(&snap.traits, set, round);
            self.correct(&snap.traits, set);
            self.panel = snap.traits.clone();
            if self.first_panel.is_none()
                && let Some(s) = self.stage.filter(|s| s.0 <= EARLY_STAGE)
            {
                self.first_panel = Some((s, snap.traits.clone()));
            }
        }
        self.items = snap.items.clone();
        self.worn = snap.units.iter().flat_map(|u| u.items.iter().cloned()).collect();
        let hints: Vec<Api> = self.units.iter().map(|u| u.api.clone()).collect();
        self.board.observe(snap, set, &mut self.library, &hints);
    }

    fn round(&self) -> u32 {
        self.stage.map_or(0, round_index)
    }

    fn add(&mut self, api: &str, copies: u32, seen: bool) {
        match self.units.iter_mut().find(|u| u.api == api) {
            Some(u) => {
                u.copies += copies;
                u.seen &= seen;
            }
            None => self.units.push(Tracked { api: api.into(), copies, since: self.stage, seen, off_panel: 0 }),
        }
    }

    fn remove(&mut self, api: &str, copies: u32) {
        if let Some(u) = self.units.iter_mut().find(|u| u.api == api) {
            u.copies = u.copies.saturating_sub(copies);
        }
        self.units.retain(|u| u.copies > 0);
    }

    /// A slot read empty last tick that shows the same unit again with no gold spent was a misread.
    fn undo_misreads(&mut self, snap: &TftSnapshot, gold: u32, was: u32) {
        let bought = std::mem::take(&mut self.bought);
        if gold != was {
            return;
        }
        for (slot, api) in bought {
            if snap.shop.get(slot).is_some_and(|s| s.as_deref() == Some(api.as_str())) {
                self.remove(&api, 1);
                self.board.unbought(&api);
            }
        }
    }

    /// Slots that emptied while gold dropped: the largest set whose costs (plus a few XP buys) add
    /// up to the drop exactly is what I bought.
    fn buys(&mut self, snap: &TftSnapshot, set: &TftSetData, drop: u32) {
        let emptied: Vec<(usize, Api, u32)> = self
            .shop
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match (s, snap.shop.get(i)) {
                (Some(u), Some(None)) => Some((i, u.clone(), cost(set, u)?)),
                _ => None,
            })
            .collect();
        let mut best: Option<(u32, u32)> = None; // (mask, slots)
        for mask in 1u32..(1 << emptied.len()) {
            let sum: u32 = emptied.iter().enumerate().filter(|(b, _)| mask & (1 << b) != 0).map(|(_, e)| e.2).sum();
            let fits = (0..=MAX_XP_BUYS).any(|xp| sum + xp * XP_COST == drop);
            if fits && best.is_none_or(|(_, n)| mask.count_ones() > n) {
                best = Some((mask, mask.count_ones()));
            }
        }
        let Some((mask, _)) = best else { return };
        for (b, (slot, api, _)) in emptied.into_iter().enumerate() {
            if mask & (1 << b) != 0 {
                self.add(&api, 1, false);
                self.board.bought(&api);
                self.bought.push((slot, api));
            }
        }
    }

    /// Best effort: gold rose by one tracked unit's sell value (★1 full cost, ★2 three copies less
    /// one) while every trait of it dropped on the panel. Only when exactly one unit fits.
    fn sells(&mut self, panel: &[(Api, u32)], set: &TftSetData, rise: u32) {
        if panel.is_empty() || self.panel.is_empty() {
            return;
        }
        let dropped = |t: &Api| count(panel, t) < count(&self.panel, t);
        let fits: Vec<(Api, u32)> = self
            .units
            .iter()
            .filter_map(|u| {
                let def = set.units.get(&u.api)?;
                if def.traits.is_empty() || !def.traits.iter().all(dropped) {
                    return None;
                }
                let two = 3 * def.cost - (def.cost > 1) as u32;
                let n = [(1, def.cost), (STAR2_COPIES, two)].into_iter().find(|&(n, v)| u.copies >= n && v == rise)?.0;
                Some((u.api.clone(), n))
            })
            .collect();
        if let [(api, n)] = fits.as_slice() {
            self.remove(api, *n);
        }
    }

    /// Once per round: units with a trait missing from the panel aren't fielded; after enough
    /// rounds of that they were sold (sells are rarely caught from gold alone) or are idle
    /// on the bench, and stop counting.
    fn off_panel(&mut self, panel: &[(Api, u32)], set: &TftSetData, round: u32) {
        if round <= self.checked {
            return;
        }
        self.checked = round;
        for u in &mut self.units {
            let fielded = set.units.get(&u.api).is_none_or(|d| d.traits.iter().all(|t| count(panel, t) > 0));
            u.off_panel = if fielded { 0 } else { u.off_panel + 1 };
        }
        self.units.retain(|u| u.off_panel < if u.copies >= STAR2_COPIES { OFF_PANEL_ROUNDS_STARRED } else { OFF_PANEL_ROUNDS });
    }

    /// Trait counts above what tracked units explain are unknown units; when they are exactly one
    /// unit's traits (one each) and only one unit in the set has that trait set, it's named.
    fn correct(&mut self, panel: &[(Api, u32)], set: &TftSetData) {
        let explained = |t: &Api| {
            self.units.iter().filter(|u| set.units.get(&u.api).is_some_and(|d| d.traits.contains(t))).count() as u32
        };
        let unknown: Vec<(Api, u32)> = panel
            .iter()
            .filter_map(|(t, n)| {
                let e = explained(t);
                (*n > e).then(|| (t.clone(), n - e))
            })
            .collect();
        if !unknown.is_empty() && unknown.iter().all(|u| u.1 == 1) {
            let want: HashSet<&Api> = unknown.iter().map(|u| &u.0).collect();
            let found: Vec<&TftUnit> = set
                .units
                .values()
                .filter(|u| u.traits.len() == want.len() && u.traits.iter().all(|t| want.contains(t)))
                .filter(|u| !self.units.iter().any(|t| t.api == u.api))
                .collect();
            if let [one] = found.as_slice() {
                let api = one.api.clone();
                self.add(&api, 1, true);
                self.unknown.clear();
                return;
            }
        }
        self.unknown = unknown;
    }

    /// My pieces when the board is being read (unnamed ones with `api` ""); else tracked units
    /// (★ from copies), then unknown units as `star: 0` with the trait as `api`.
    pub fn owned(&self) -> Vec<OwnedUnit> {
        if self.board.active(self.round()) {
            return self.board.owned();
        }
        let known = self.units.iter().map(|u| OwnedUnit { api: u.api.clone(), copies: u.copies, star: star(u.copies), board: 0, bench: 0 });
        let unknown = self.unknown.iter().map(|(t, n)| OwnedUnit { api: t.clone(), copies: *n, star: 0, board: 0, bench: 0 });
        known.chain(unknown).collect()
    }

    /// My pieces by their HP bars.
    pub fn board(&self) -> &board::Board {
        &self.board
    }

    /// Replays of sparse captures: use every quiet read.
    pub fn single_reads(mut self) -> Tracker {
        self.board.single_reads = true;
        self.scout.single_reads = true;
        self.fights.single_reads = true;
        self
    }

    pub fn with_library(mut self, library: Library) -> Tracker {
        self.library = library;
        self
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    /// Tracked units in the order I got them.
    pub fn units(&self) -> &[Tracked] {
        &self.units
    }

    pub fn copies(&self, api: &str) -> u32 {
        if self.board.active(self.round()) {
            return self.board.copies(api);
        }
        self.units.iter().find(|u| u.api == api).map_or(0, |u| u.copies)
    }

    pub fn unknown(&self) -> &[(Api, u32)] {
        &self.unknown
    }

    /// My last readable trait panel.
    pub fn panel(&self) -> &[(Api, u32)] {
        &self.panel
    }

    /// The first trait panel of mine read in stage 1/2, with its stage.
    pub fn first_panel(&self) -> Option<(Stage, &[TraitCount])> {
        self.first_panel.as_ref().map(|(s, p)| (*s, p.as_slice()))
    }

    /// My item bench as last read on my own board.
    pub fn items(&self) -> &[Api] {
        &self.items
    }

    /// Items my units carry (board and bench) as last read.
    pub fn worn(&self) -> &[Api] {
        &self.worn
    }

    /// Other players' units and trait panels, seen within SCOUT_DECAY rounds.
    pub fn scout(&self) -> &Scout {
        &self.scout
    }

    /// My fights and who each was against.
    pub fn fights(&self) -> &Fights {
        &self.fights
    }

    /// The last stage read.
    pub fn stage(&self) -> Option<(u32, u32)> {
        self.stage
    }

    /// Units on my board and the cap, as last read (the counter shows in planning only).
    pub fn team_size(&self) -> Option<(u32, u32)> {
        self.team
    }

    /// My level: read now, else the cap on screen less its bonus slots, else the higher of the
    /// last level read and the last cap's.
    pub fn level(&self, snap: &TftSnapshot) -> Option<u32> {
        let from = |t: Option<(u32, u32)>| t.map(|(_, cap)| cap.saturating_sub(self.team_bonus));
        snap.level.or(from(snap.team_size)).or(self.level.max(from(self.team)))
    }

    /// A free slot on my board this read (planning), once seen on enough reads in a row.
    pub fn free_slot(&self, snap: &TftSnapshot) -> Option<(u32, u32)> {
        let need = if self.board.single_reads { 1 } else { FREE_READS };
        snap.team_size.filter(|(n, cap)| n < cap && self.free_reads >= need)
    }

    /// HP lost over the last `rounds` rounds (None until two rounds are read).
    /// My HP, as last confirmed.
    pub fn hp(&self) -> Option<u32> {
        self.hp.back().map(|h| h.1)
    }

    /// Gold this read, or the last read when it's unreadable.
    pub fn gold(&self, snap: &TftSnapshot) -> Option<u32> {
        snap.gold.or(self.gold)
    }

    pub fn hp_drop(&self, rounds: u32) -> Option<u32> {
        let &(now, hp) = self.hp.back()?;
        let &(then, was) = self.hp.iter().find(|h| h.0 + rounds >= now)?;
        (then < now).then(|| was.saturating_sub(hp))
    }
}
