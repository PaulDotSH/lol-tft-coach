//! My board and bench as pieces, named. A piece is a green HP bar seen in a quiet planning
//! read (no enemy bars, every bar full, the same places as the read before). A piece keeps
//! its name while it stays put; a moved one is found again by its looks; a new one on the
//! bench takes the name of what I just bought; the board's units must add up to my trait
//! panel, which says which units stand there (and names a piece when only one is left);
//! looks learnt over games (only from pieces named for sure) pair up the rest. Other players'
//! boards (`seed`) are named the same way, also by the library's built-in seed looks.

use crate::library::{Library, similarity, standout};
use crate::scout::{SEED_MARGIN, SEED_SIM};
use crate::tracker::{STAR2_COPIES, STAR3_COPIES, round_index};
use std::collections::HashMap;
use tft_model::*;

/// The same place: bench slots match exactly, board bars within this many base px (bars bob
/// with idle animations).
const SAME_PLACE: f32 = 14.0;
/// A read is quiet when every bar is within this of the last read's.
const STILL: f32 = 6.0;
/// Below this, the unit at an old piece's place is another unit (sold and replaced between reads).
const SWAP_SIM: f32 = 0.55;
/// A piece found again elsewhere (moved) looks at least this alike.
const MOVE_SIM: f32 = 0.78;
/// Named by looks alone: best match at least this alike and this far ahead of the next unit.
const LOOKS_SIM: f32 = 0.80;
const LOOKS_MARGIN: f32 = 0.04;
/// Looks kept per piece; a new one is kept when less alike than NEW_LOOK to all kept.
const PIECE_LOOKS: usize = 6;
const NEW_LOOK: f32 = 0.93;
/// Panel search: at most this many nodes.
const SEARCH_NODES: usize = 300_000;
/// Panel search priors: a unit's looks (its best similarity to a free piece), a unit I bought
/// (tracked from the shop), a unit of the last read's set. Costs: a free piece left without a
/// unit (a training dummy), a panel trait one higher than the units give (an emblem, a misread
/// digit), a unit trait off the panel (hidden past the last row).
const HINT: f32 = 0.3;
const LAST_SET: f32 = 0.3;
const NONE_COST: f32 = 1.5;
const SLACK_COST: f32 = 1.0;
const OFF_COST: f32 = 0.3;
/// Pairing a free piece with a unit: keeping its current name counts this much looks.
const KEEP: f32 = 0.1;
/// Traits the panel may count one higher than the pieces, in all (emblems are few).
const MAX_SLACK: u32 = 2;
/// Most traits a unit has.
const MAX_TRAITS: u32 = 3;
/// Pieces are trusted this many rounds after the last quiet read.
const ACTIVE_ROUNDS: u32 = 3;
/// A vanished piece (sold, or its bar hidden behind another's) can be found again by its
/// looks for this many updates.
const GHOST_UPDATES: u32 = 3;

/// Copies a piece of ★`star` stands for.
pub fn copies(star: u32) -> u32 {
    match star {
        3 => STAR3_COPIES,
        2 => STAR2_COPIES,
        _ => 1,
    }
}

/// How a piece got its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    /// Not named yet.
    No,
    /// By looks alone (library).
    Looks,
    /// By the trait panel: the only way the board adds up.
    Panel,
    /// It's what I just bought.
    Bought,
}

#[derive(Debug, Clone)]
pub struct Piece {
    /// Stays with the piece as it moves.
    pub id: u32,
    pub api: Option<Api>,
    pub named: Named,
    pub star: u32,
    pub bench: Option<u32>,
    pub x: f32,
    pub y: f32,
    looks: Vec<Vec<f32>>,
    /// Its sure name stood on the board in a read the panel agreed with: safe to learn from.
    confirmed: bool,
}

impl Piece {
    fn new(id: u32, u: &SeenUnit) -> Piece {
        Piece { id, api: None, named: Named::No, star: u.star.max(1), bench: u.bench, x: u.x, y: u.y, looks: vec![], confirmed: false }
    }

    pub fn sure(&self) -> bool {
        matches!(self.named, Named::Bought | Named::Panel)
    }

    /// Sure and agreed with by the panel: what the library learns from.
    pub fn confirmed(&self) -> bool {
        self.sure() && self.confirmed
    }

    /// Best similarity of `emb` to this piece's looks (0 without looks).
    fn alike(&self, emb: &[f32]) -> f32 {
        self.looks.iter().map(|l| similarity(l, emb)).fold(0.0, f32::max)
    }

    fn look(&mut self, emb: &[f32]) {
        if emb.is_empty() || self.looks.iter().any(|l| similarity(l, emb) >= NEW_LOOK) {
            return;
        }
        if self.looks.len() >= PIECE_LOOKS {
            self.looks.remove(0);
        }
        self.looks.push(emb.to_vec());
    }

    fn name(&mut self, api: &str, how: Named) {
        if self.api.as_deref() != Some(api) {
            self.confirmed = false;
        }
        self.api = Some(api.into());
        self.named = how;
        // Named by the panel: confirmed by construction.
        self.confirmed |= how == Named::Panel;
    }

    /// `u`'s ★ can be this piece's: unread, the same, or higher (combined).
    fn star_ok(&self, u: &SeenUnit) -> bool {
        u.star == 0 || u.star >= self.star
    }

    fn same_place(&self, u: &SeenUnit) -> bool {
        match (self.bench, u.bench) {
            (Some(a), Some(b)) => a == b,
            (None, None) => (self.x - u.x).abs() <= SAME_PLACE && (self.y - u.y).abs() <= SAME_PLACE,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Board {
    pieces: Vec<Piece>,
    /// The last read's bars: a read counts once the next shows the same places.
    last: Vec<SeenUnit>,
    /// Bought since the last update, oldest first.
    bought: Vec<Api>,
    /// Vanished pieces and updates since.
    ghosts: Vec<(Piece, u32)>,
    /// `round_index` of the last update.
    updated: Option<u32>,
    /// The units the trait panel says are on my board (best fit).
    on_board: Vec<Api>,
    /// What the panel pinned down (see `solve`).
    groups: Vec<Group>,
    next_id: u32,
    /// Use every quiet read without waiting for a second one (replays of sparse captures).
    pub single_reads: bool,
    /// Another player's board: seed looks name pieces and weigh in the panel search (mine
    /// stays on learnt looks).
    pub(crate) seed: bool,
}

/// No enemies, every bar full: a planning read of my own board.
fn quiet(snap: &TftSnapshot) -> bool {
    !snap.viewing_other && snap.enemies == 0 && !snap.units.is_empty() && snap.units.iter().all(|u| u.full)
}

fn still(a: &[SeenUnit], b: &[SeenUnit]) -> bool {
    a.len() == b.len()
        && a.iter().all(|u| b.iter().any(|v| (u.x - v.x).abs() <= STILL && (u.y - v.y).abs() <= STILL && u.star == v.star))
}

impl Board {
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Pieces were read recently enough to stand for what I own.
    pub fn active(&self, round: u32) -> bool {
        self.updated.is_some_and(|r| r + ACTIVE_ROUNDS >= round) && self.pieces.iter().any(|p| p.api.is_some())
    }

    /// A purchase seen in the shop (named on the next quiet read).
    pub fn bought(&mut self, api: &str) {
        self.bought.push(api.into());
    }

    /// Undo a purchase that was a misread.
    pub fn unbought(&mut self, api: &str) {
        if let Some(i) = self.bought.iter().rposition(|b| b == api) {
            self.bought.remove(i);
        }
    }

    /// Copies of `api` on my board and bench.
    pub fn copies(&self, api: &str) -> u32 {
        self.pieces.iter().filter(|p| p.api.as_deref() == Some(api)).map(|p| copies(p.star)).sum()
    }

    /// Named pieces grouped by unit, then unnamed pieces as `api` "" (star 0 when ★ unread).
    pub fn owned(&self) -> Vec<OwnedUnit> {
        let mut out: Vec<OwnedUnit> = vec![];
        for p in &self.pieces {
            let api = p.api.clone().unwrap_or_default();
            let i = match out.iter().position(|o| o.api == api && !api.is_empty()) {
                Some(i) => i,
                None => {
                    out.push(OwnedUnit { api, copies: 0, star: 0, board: 0, bench: 0 });
                    out.len() - 1
                }
            };
            let o = &mut out[i];
            o.copies += copies(p.star);
            o.star = o.star.max(p.star);
            if p.bench.is_some() {
                o.bench += 1;
            } else {
                o.board += 1;
            }
        }
        out
    }

    /// One read of my screen; `hints` are units I bought (tracked from the shop).
    pub fn observe(&mut self, snap: &TftSnapshot, set: &TftSetData, lib: &mut Library, hints: &[Api]) {
        if !quiet(snap) {
            self.last.clear();
            return;
        }
        let ready = self.single_reads || still(&self.last, &snap.units);
        self.last = snap.units.clone();
        if !ready {
            return;
        }
        self.update(&snap.units, set, lib);
        self.solve(&snap.traits, snap.traits_hidden, hints, set, lib);
        // Only names the panel agreed with are learnt (a purchase can land on the wrong piece).
        for p in self.pieces.iter().filter(|p| p.sure() && p.confirmed) {
            if let (Some(api), Some(l)) = (&p.api, p.looks.last()) {
                lib.learn(api, p.star, l);
            }
        }
        self.updated = snap.stage.map(round_index).or(self.updated);
    }

    /// One read of another player's board (scouting), named as mine is but never learnt from
    /// (the library only grows from my own pieces). True when the read was used: every bar
    /// full, no fight on, and still (or `single_reads`).
    pub fn observe_other(&mut self, snap: &TftSnapshot, set: &TftSetData, lib: &Library) -> bool {
        if snap.enemies > 0 || snap.units.is_empty() || !snap.units.iter().all(|u| u.full) {
            self.last.clear();
            return false;
        }
        let ready = self.single_reads || still(&self.last, &snap.units);
        self.last = snap.units.clone();
        if !ready {
            return false;
        }
        self.update(&snap.units, set, lib);
        self.solve(&snap.traits, snap.traits_hidden, &[], set, lib);
        self.updated = snap.stage.map(round_index).or(self.updated);
        true
    }

    /// Match this read's bars to the pieces: same place, then moved (by looks), then new.
    fn update(&mut self, units: &[SeenUnit], set: &TftSetData, lib: &Library) {
        let mut old: Vec<Option<Piece>> = std::mem::take(&mut self.pieces).into_iter().map(Some).collect();
        let mut now: Vec<Option<Piece>> = vec![None; units.len()];
        // Same place, the same unit or one ★ up (a combine lands on one of the copies).
        for (i, u) in units.iter().enumerate() {
            let at = old.iter().position(|p| {
                p.as_ref().is_some_and(|p| p.same_place(u) && p.star_ok(u) && (p.looks.is_empty() || u.emb.is_empty() || p.alike(&u.emb) >= SWAP_SIM))
            });
            if let Some(j) = at {
                now[i] = old[j].take();
            }
        }
        // Moved (or back from hiding): the most alike pairs first.
        let ghosts = std::mem::take(&mut self.ghosts);
        let n_old = old.len();
        old.extend(ghosts.iter().map(|(p, _)| Some(p.clone())));
        let mut pairs: Vec<(f32, usize, usize)> = vec![];
        for (i, u) in units.iter().enumerate().filter(|(i, _)| now[*i].is_none()) {
            for (j, p) in old.iter().enumerate() {
                if let Some(p) = p
                    && p.star_ok(u)
                {
                    let s = p.alike(&u.emb);
                    if s >= MOVE_SIM {
                        pairs.push((s, i, j));
                    }
                }
            }
        }
        pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, i, j) in pairs {
            if now[i].is_none() && old[j].is_some() {
                now[i] = old[j].take();
            }
        }
        // Place, ★ and looks from this read; combines and purchases consume what I bought. On the
        // first read every piece is new: purchases before it can't be told from what was there.
        let mut bought = std::mem::take(&mut self.bought);
        if self.updated.is_none() && self.next_id == 0 {
            bought.clear();
        }
        for (i, u) in units.iter().enumerate() {
            if let Some(p) = &mut now[i] {
                if u.star > p.star
                    && let Some(api) = &p.api
                    && let Some(k) = bought.iter().position(|b| b == api)
                {
                    bought.remove(k);
                }
                (p.x, p.y, p.bench) = (u.x, u.y, u.bench);
                p.star = p.star.max(u.star);
                p.look(&u.emb);
            }
        }
        // Old pieces not seen become ghosts; ghosts not found age out.
        for (j, p) in old.into_iter().enumerate() {
            if let Some(p) = p {
                let age = if j < n_old { 0 } else { ghosts[j - n_old].1 + 1 };
                if age < GHOST_UPDATES {
                    self.ghosts.push((p, age));
                }
            }
        }
        // New pieces: purchases land on the bench left to right; the rest by looks.
        let mut fresh: Vec<usize> = (0..units.len()).filter(|i| now[*i].is_none()).collect();
        fresh.sort_by_key(|&i| (units[i].bench.is_none(), units[i].bench, (units[i].x * 10.0) as i64));
        // One purchase and one new bench piece: that's it, no panel needed to learn its looks.
        let lone = bought.len() == 1 && fresh.len() == 1 && fresh.iter().all(|&i| units[i].bench.is_some());
        for i in fresh {
            let u = &units[i];
            self.next_id += 1;
            let mut p = Piece::new(self.next_id, u);
            p.look(&u.emb);
            if !bought.is_empty() {
                // Bought and already moved to the board: the panel has to confirm it.
                let api = bought.remove(0);
                p.name(&api, if u.bench.is_some() { Named::Bought } else { Named::Looks });
                p.confirmed = lone;
            } else if let Some(api) = by_looks(lib, &u.emb, set, self.seed) {
                p.name(&api, Named::Looks);
            }
            now[i] = Some(p);
        }
        // Purchases not seen land are dropped (they combined, or the buy was a misread).
        self.pieces = now.into_iter().flatten().collect();
        // Unnamed pieces: try looks again (the library grows during the game).
        let seed = self.seed;
        for p in self.pieces.iter_mut().filter(|p| p.api.is_none()) {
            if let Some(api) = p.looks.last().and_then(|l| by_looks(lib, l, set, seed)) {
                p.name(&api, Named::Looks);
            }
        }
    }

    /// The board's units must add up to the trait panel, which can't tell pieces apart: it
    /// says which units stand there (a set, found by search with looks, purchases and the last
    /// set as priors). What it pins down is kept as groups: pieces known to carry exactly some
    /// units, in some order. A group on the board fixes its units in the next search, so a piece
    /// that joins the board carries what the fits add; a group whose piece left for the bench
    /// splits into what stayed and what left; a group of one names its piece for sure.
    fn solve(&mut self, panel: &[(Api, u32)], hidden: u32, hints: &[Api], set: &TftSetData, lib: &Library) {
        if panel.is_empty() {
            return;
        }
        let board: Vec<usize> = (0..self.pieces.len()).filter(|&i| self.pieces[i].bench.is_none()).collect();
        if board.is_empty() {
            return;
        }
        let on_board = |id: u32, pieces: &[Piece]| pieces.iter().any(|p| p.id == id && p.bench.is_none());
        let want: HashMap<&str, u32> = panel.iter().map(|(t, n)| (t.as_str(), *n)).collect();
        let low = panel.iter().map(|p| p.1).min().unwrap_or(1);
        // Groups wholly on the board fix their units; pieces named for sure outside groups too.
        let (intact, broken): (Vec<Group>, Vec<Group>) =
            std::mem::take(&mut self.groups).into_iter().partition(|g| g.pieces.iter().all(|&id| on_board(id, &self.pieces)));
        let grouped: Vec<u32> = intact.iter().flat_map(|g| g.pieces.iter().copied()).collect();
        let mut fixed: Vec<Api> = intact.iter().flat_map(|g| g.units.iter().cloned()).collect();
        for &i in &board {
            let p = &self.pieces[i];
            if !grouped.contains(&p.id) && p.sure()
                && let Some(a) = &p.api
            {
                fixed.push(a.clone());
            }
        }
        fixed.sort();
        fixed.dedup();
        let fixed_set: Vec<u32> = board.iter().map(|&i| self.pieces[i].id).filter(|id| grouped.contains(id)).collect();
        let mut free: Vec<usize> = board
            .iter()
            .copied()
            .filter(|&i| !fixed_set.contains(&self.pieces[i].id) && !(self.pieces[i].sure() && self.pieces[i].api.as_ref().is_some_and(|a| fixed.contains(a))))
            .collect();
        // Learnt looks only on my board; on another's, a unit's seed look too (its best counts).
        let seed = self.seed;
        let classified: Vec<Vec<(Api, f32, bool)>> = (0..self.pieces.len())
            .map(|i| self.pieces[i].looks.last().map(|l| lib.classify(l).into_iter().filter(|c| seed || !c.2).collect()).unwrap_or_default())
            .collect();
        let looks_of = |i: usize, u: &Api| classified[i].iter().find(|c| &c.0 == u).map_or(0.0, |c| c.1);
        let last_set = self.on_board.clone();
        let run = |free: &[usize], fixed: &[Api]| {
            let prior = |u: &Api| {
                free.iter().map(|&i| looks_of(i, u)).fold(0.0f32, f32::max)
                    + if hints.contains(u) { HINT } else { 0.0 }
                    + if last_set.contains(u) { LAST_SET } else { 0.0 }
            };
            let fixed: Vec<&Api> = fixed.iter().collect();
            let (found, complete) = search(set, &want, hidden, low, &fixed, free.len(), &prior);
            let found: Vec<(Vec<Api>, f32, u32, usize)> = found.into_iter().map(|f| (f.units.into_iter().cloned().collect(), f.score, f.slack, f.missing)).collect();
            (found, complete)
        };
        let (mut found, mut complete) = run(&free, &fixed);
        let mut keep_groups = true;
        if found.is_empty() && !fixed.is_empty() {
            // Something taken as known doesn't fit the panel: search the whole board again.
            keep_groups = false;
            fixed.clear();
            free = board.clone();
            (found, complete) = run(&free, &fixed);
            for &i in &board {
                if self.pieces[i].sure() {
                    self.pieces[i].named = Named::Looks;
                }
            }
        }
        let Some(best) = found.iter().max_by(|a, b| a.1.total_cmp(&b.1)).cloned() else { return };
        self.on_board = fixed.iter().cloned().chain(best.0.iter().cloned()).collect();
        if keep_groups {
            self.groups = intact;
        }
        // Fits with no slack and the fewest pieces left without a unit.
        let least = found.iter().filter(|f| f.2 == 0).map(|f| f.3).min();
        let exact: Vec<&Vec<Api>> = found.iter().filter(|f| f.2 == 0 && Some(f.3) == least).map(|f| &f.0).collect();
        // The panel adds up with the sure names on the board: they're confirmed.
        if keep_groups && !exact.is_empty() {
            for &i in &board {
                let p = &mut self.pieces[i];
                if p.sure() && p.api.as_ref().is_some_and(|a| fixed.contains(a)) {
                    p.confirmed = true;
                }
            }
        }
        let agreed = |units: &dyn Fn(&Vec<Api>) -> Vec<Api>| -> Option<Vec<Api>> {
            let first = units(exact.first()?);
            exact.iter().all(|f| same(&units(f), &first)).then_some(first)
        };
        if complete && !exact.is_empty() && keep_groups {
            // A group that lost pieces to the bench: what stayed carries the group's units that
            // every fit still has; what left carries the rest.
            let mut taken: Vec<usize> = vec![];
            for g in &broken {
                let stayed: Vec<usize> = free.iter().copied().filter(|&i| g.pieces.contains(&self.pieces[i].id)).collect();
                let left: Vec<u32> = g.pieces.iter().copied().filter(|&id| !on_board(id, &self.pieces)).collect();
                let Some(kept) = agreed(&|f: &Vec<Api>| f.iter().filter(|u| g.units.contains(u)).cloned().collect()) else { continue };
                if kept.len() != stayed.len() {
                    continue;
                }
                let gone: Vec<Api> = g.units.iter().filter(|u| !kept.contains(u)).cloned().collect();
                if !stayed.is_empty() {
                    self.groups.push(Group { pieces: stayed.iter().map(|&i| self.pieces[i].id).collect(), units: kept });
                }
                if gone.len() == left.len() && !left.is_empty() {
                    self.groups.push(Group { pieces: left, units: gone });
                }
                taken.extend(stayed);
            }
            // The other free pieces carry whatever else every fit adds.
            let rest: Vec<usize> = free.iter().copied().filter(|i| !taken.contains(i)).collect();
            let claimed: Vec<Api> = self.groups.iter().filter(|g| g.pieces.iter().any(|id| on_board(*id, &self.pieces))).flat_map(|g| g.units.clone()).collect();
            if !rest.is_empty()
                && let Some(units) = agreed(&|f: &Vec<Api>| f.iter().filter(|u| !claimed.contains(u)).cloned().collect())
                && units.len() == rest.len()
            {
                self.groups.push(Group { pieces: rest.iter().map(|&i| self.pieces[i].id).collect(), units });
            }
        }
        self.name_groups(&classified);
        // Free pieces no group covers: the best fit's units paired by looks, unsure.
        let grouped: Vec<u32> = self.groups.iter().flat_map(|g| g.pieces.clone()).collect();
        let loose: Vec<usize> = free.iter().copied().filter(|&i| !grouped.contains(&self.pieces[i].id) && !self.pieces[i].sure()).collect();
        let claimed: Vec<&Api> = board.iter().filter(|&&i| !loose.contains(&i)).filter_map(|&i| self.pieces[i].api.as_ref()).collect();
        let units: Vec<Api> = best.0.iter().filter(|u| !claimed.contains(u)).cloned().collect();
        self.pair(&loose, &units, &classified, Named::Looks);
    }

    /// Name each group's pieces with its units: a group of one for sure; a bigger one by looks
    /// (or in order), unsure (when their ★ match, what I own comes out the same either way).
    fn name_groups(&mut self, classified: &[Vec<(Api, f32, bool)>]) {
        for g in self.groups.clone() {
            let idx: Vec<usize> = g.pieces.iter().filter_map(|id| self.pieces.iter().position(|p| p.id == *id)).collect();
            if let ([i], [u]) = (idx.as_slice(), g.units.as_slice()) {
                if !(self.pieces[*i].sure() && self.pieces[*i].api.as_ref() == Some(u)) {
                    self.pieces[*i].name(u, Named::Panel);
                }
                continue;
            }
            // Pieces already named for sure inside the group keep their units.
            let sure: Vec<(usize, Api)> = idx.iter().filter(|&&i| self.pieces[i].sure()).filter_map(|&i| Some((i, self.pieces[i].api.clone()?))).filter(|(_, a)| g.units.contains(a)).collect();
            let open: Vec<usize> = idx.iter().copied().filter(|i| !sure.iter().any(|(j, _)| j == i)).collect();
            let units: Vec<Api> = g.units.iter().filter(|u| !sure.iter().any(|(_, a)| a == *u)).cloned().collect();
            let how = if open.len() == 1 { Named::Panel } else { Named::Looks };
            self.pair(&open, &units, classified, how);
        }
    }

    /// Pair pieces with units, most alike (by looks) first, then in order; pieces left over
    /// (a training dummy, or the panel undercounts) lose their name.
    fn pair(&mut self, pieces: &[usize], units: &[Api], classified: &[Vec<(Api, f32, bool)>], how: Named) {
        let looks_of = |i: usize, u: &Api| classified.get(i).and_then(|c| c.iter().find(|c| &c.0 == u)).map_or(0.0, |c| c.1);
        let mut pairs: Vec<(f32, usize, usize)> = vec![];
        for (k, &i) in pieces.iter().enumerate() {
            for (j, u) in units.iter().enumerate() {
                let keep = if self.pieces[i].api.as_ref() == Some(u) { KEEP } else { 0.0 };
                pairs.push((looks_of(i, u) + keep, k, j));
            }
        }
        pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let (mut piece_done, mut unit_done) = (vec![false; pieces.len()], vec![false; units.len()]);
        for (_, k, j) in pairs {
            if !piece_done[k] && !unit_done[j] {
                (piece_done[k], unit_done[j]) = (true, true);
                self.pieces[pieces[k]].name(&units[j], how);
            }
        }
        for (k, done) in piece_done.into_iter().enumerate() {
            let p = &mut self.pieces[pieces[k]];
            if !done && !p.sure() {
                (p.api, p.named) = (None, Named::No);
            }
        }
    }
}

/// Same units, in any order.
fn same(a: &[Api], b: &[Api]) -> bool {
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    a.sort();
    b.sort();
    a == b
}

/// Pieces known to carry exactly these units, in some order.
#[derive(Debug, Clone)]
struct Group {
    pieces: Vec<u32>,
    units: Vec<Api>,
}

/// The unit a look matches, when one stands out (and is in the set).
pub(crate) fn by_looks(lib: &Library, emb: &[f32], set: &TftSetData, seed: bool) -> Option<Api> {
    if emb.is_empty() {
        return None;
    }
    by_matches(&lib.classify(emb), set, seed)
}

/// `Library::classify`'s matches naming a unit: learnt looks first (one standing out, see
/// LOOKS_SIM), then with `seed` the seed's (SEED_SIM / SEED_MARGIN, among seed looks only).
/// Learnt and seed similarities aren't compared with each other: they come from different
/// pictures (my board vs renders), so each kind has its own thresholds and learnt ones win.
pub(crate) fn by_matches(c: &[(Api, f32, bool)], set: &TftSetData, seed: bool) -> Option<Api> {
    let real = standout(c, false, LOOKS_SIM, LOOKS_MARGIN).filter(|a| set.units.contains_key(*a));
    real.or_else(|| standout(c, true, SEED_SIM, SEED_MARGIN).filter(|a| seed && set.units.contains_key(*a))).cloned()
}

/// A set of units that adds up to the trait panel (with the fixed units).
struct Fit<'a> {
    units: Vec<&'a Api>,
    score: f32,
    /// Panel traits one higher than the units give.
    slack: u32,
    /// Free pieces left without a unit.
    missing: usize,
}

/// Up to `free` more units (combinations, best priors first) that with `fixed` add up to the
/// panel `want`: no trait over the panel's count, every panel trait reached or one short
/// (MAX_SLACK in all), and at most `hidden` traits off the panel, each no higher than the
/// panel's lowest count (the panel lists traits by count). Also whether it saw every fit.
fn search<'a>(
    set: &'a TftSetData,
    want: &HashMap<&str, u32>,
    hidden: u32,
    low: u32,
    fixed: &[&Api],
    free: usize,
    prior: &dyn Fn(&Api) -> f32,
) -> (Vec<Fit<'a>>, bool) {
    let mut have: HashMap<&'a str, u32> = HashMap::new();
    for a in fixed {
        for t in set.units.get(*a).map(|u| u.traits.as_slice()).unwrap_or_default() {
            *have.entry(t.as_str()).or_default() += 1;
        }
    }
    let off = |u: &TftUnit| u.traits.iter().filter(|t| !want.contains_key(t.as_str())).count() as u32;
    let mut cands: Vec<(&'a TftUnit, f32)> = set
        .units
        .values()
        .filter(|u| !u.traits.is_empty() && off(u) <= hidden && !fixed.contains(&&u.api))
        .map(|u| (u, prior(&u.api) - off(u) as f32 * OFF_COST))
        .collect();
    cands.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.api.cmp(&b.0.api)));
    // How many candidates from each position on carry each panel trait.
    let mut avail: Vec<HashMap<&str, u32>> = vec![HashMap::new(); cands.len() + 1];
    for i in (0..cands.len()).rev() {
        avail[i] = avail[i + 1].clone();
        for t in &cands[i].0.traits {
            *avail[i].entry(t.as_str()).or_default() += 1;
        }
    }
    // Exact fits first; slack (an emblem, a misread digit) only when there are none.
    let mut last = (vec![], true);
    for slack in [0, MAX_SLACK] {
        let mut s = SetSearch { want, hidden, low, slack, cands: cands.clone(), avail: &avail, have: have.clone(), pick: vec![], free, nodes: 0, found: vec![] };
        if s.over() {
            return (vec![], true);
        }
        s.dfs(0, 0.0);
        let complete = s.nodes <= SEARCH_NODES;
        if !s.found.is_empty() {
            return (s.found, complete);
        }
        last = (s.found, complete);
    }
    last
}

struct SetSearch<'a, 'w> {
    want: &'w HashMap<&'w str, u32>,
    hidden: u32,
    low: u32,
    /// Panel traits allowed one short.
    slack: u32,
    cands: Vec<(&'a TftUnit, f32)>,
    avail: &'w [HashMap<&'a str, u32>],
    have: HashMap<&'a str, u32>,
    pick: Vec<&'a Api>,
    free: usize,
    nodes: usize,
    found: Vec<Fit<'a>>,
}

impl<'a> SetSearch<'a, '_> {
    /// Some trait went past what the panel allows.
    fn over(&self) -> bool {
        let mut off = 0;
        for (t, n) in &self.have {
            match self.want.get(t) {
                Some(w) if n > w => return true,
                Some(_) => {}
                None if *n > self.low => return true,
                None => off += 1,
            }
        }
        off > self.hidden
    }

    /// Slack traits when the pick adds up to the panel.
    fn fits(&self) -> Option<u32> {
        let mut slack = 0;
        for (t, w) in self.want {
            match w - self.have.get(t).copied().unwrap_or(0).min(*w) {
                0 => {}
                1 => slack += 1,
                _ => return None,
            }
        }
        (slack <= self.slack).then_some(slack)
    }

    fn step(&mut self, u: &'a TftUnit, add: bool) {
        for t in &u.traits {
            let n = self.have.entry(t.as_str()).or_default();
            if add {
                *n += 1;
            } else {
                *n -= 1;
            }
        }
        self.have.retain(|_, n| *n > 0);
    }

    fn dfs(&mut self, from: usize, score: f32) {
        self.nodes += 1;
        if self.nodes > SEARCH_NODES {
            return;
        }
        if let Some(slack) = self.fits() {
            let missing = self.free - self.pick.len();
            let score = score - slack as f32 * SLACK_COST - missing as f32 * NONE_COST;
            self.found.push(Fit { units: self.pick.clone(), score, slack, missing });
        }
        if self.pick.len() == self.free {
            return;
        }
        // Every short trait must still be coverable by the candidates left (beyond the slack).
        let mut short = 0;
        let mut uncoverable = 0;
        for (t, w) in self.want {
            let d = w.saturating_sub(self.have.get(t).copied().unwrap_or(0));
            short += d;
            uncoverable += d.saturating_sub(self.avail[from].get(t).copied().unwrap_or(0));
        }
        if uncoverable > self.slack || short > (self.free - self.pick.len()) as u32 * MAX_TRAITS + self.slack {
            return;
        }
        for i in from..self.cands.len() {
            let (u, p) = self.cands[i];
            self.step(u, true);
            if !self.over() {
                self.pick.push(&u.api);
                self.dfs(i + 1, score + p);
                self.pick.pop();
            }
            self.step(u, false);
            if self.nodes > SEARCH_NODES {
                return;
            }
        }
    }
}
