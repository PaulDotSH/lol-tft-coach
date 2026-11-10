//! Other players' units, from what my screen shows of them: a scouted board (its HP bars named
//! as mine are: looks, then the trait panel's set search; bench units by looks) and, in my
//! fights, my opponent's red bars (looks only, from still reads; away from my board the looks
//! are compared with each side's background taken out). Kept per player by name plate (when
//! none is read: by trait panel likeness, or by the units named), forgotten after SCOUT_DECAY
//! rounds unseen.
//! Only my own confirmed pieces teach the library: other boards only read it. Both also use the
//! library's seed looks (renders) where learnt looks name nothing: on a scouted board as a
//! piece's name and in the panel search's priors, in a fight by a vote per red bar over the
//! fight's still reads, among the units that player's last panel and the pool allow.

use crate::board::{self, Named};
use crate::library::Library;
use crate::tracker::{STAR2_COPIES, STAR3_COPIES, TraitCount};
use std::collections::HashSet;
use tft_model::*;

/// Copies of each unit in the shared pool, by cost 1..=5 (Set 18).
pub const POOL: [u32; 5] = [30, 25, 18, 10, 9];
/// A player is forgotten after this many rounds unseen.
pub const SCOUT_DECAY: u32 = 2;
/// Two trait panels sharing this share of their traits (of the union) are the same board.
const SAME_BOARD: f32 = 0.5;
/// Other players kept at most.
const MAX_PLAYERS: usize = 7;
/// A red bar within this of the last read's (base px) hasn't moved: combat hasn't started.
const STILL: f32 = 6.0;
/// Named by centred looks (`Library::classify_centered`): best match at least this alike and
/// this far ahead of the next unit, once the library knows CENTRED_UNITS units (with fewer,
/// their mean is close to each one and centring leaves noise).
const CENTRED_SIM: f32 = 0.45;
const CENTRED_MARGIN: f32 = 0.05;
const CENTRED_UNITS: usize = 8;
/// Named by seed looks (`Library::with_seed`, best seed look per unit) when no learnt look
/// names it: best at least SEED_SIM alike and SEED_MARGIN ahead of the next unit's seed match
/// (in a fight: means over its still reads). Seed and learnt similarities have their own
/// thresholds and learnt matches win. Placeholders: T8's sweep on the eval set sets them.
pub(crate) const SEED_SIM: f32 = 0.60;
pub(crate) const SEED_MARGIN: f32 = 0.03;
/// A red bar this near (base px) to one in an earlier still read of the fight is the same unit.
const SAME_BAR: f32 = 14.0;
/// A scouted board showing this share of a comp's traits plays that comp.
const CONTEST_SHARE: f32 = 0.5;

/// Copies left in the pool of a unit of `cost` (None: no pool, e.g. summons and 6+ costs).
pub fn pool(cost: u32) -> Option<u32> {
    POOL.get((cost as usize).checked_sub(1)?).copied()
}

/// Copies a unit of ★`star` stands for (an unread ★ counts as 1).
fn copies(star: u32) -> u32 {
    match star {
        3 => STAR3_COPIES,
        2 => STAR2_COPIES,
        _ => 1,
    }
}

/// Same player name, allowing one OCR slip in longer names.
pub(crate) fn same_name(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    let (a, b) = (norm(a), norm(b));
    a == b || (a.len() >= 6 && a.len() == b.len() && a.chars().zip(b.chars()).filter(|(x, y)| x != y).count() <= 1)
}

/// One unit of another player as last read.
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    pub x: f32,
    pub y: f32,
    /// None: the looks and the panel couldn't name it.
    pub api: Option<Api>,
    /// How it was named: "=" by the trait panel, "~" by looks, "^" by the classifier head, "≈" by a fight's seed vote, "?" not.
    pub how: &'static str,
    pub star: u32,
    pub bench: bool,
}

/// Another player: their last trait panel and units.
#[derive(Debug, Clone, Default)]
pub struct Player {
    /// As their player list entry reads; None until a name plate is read.
    pub name: Option<String>,
    pub traits: Vec<TraitCount>,
    /// Traits past the last row of that panel.
    hidden: u32,
    /// `round_index` when last seen.
    pub seen: u32,
    /// Their board as scouted (pieces kept across reads).
    board: board::Board,
    /// Their units from the latest read (scouting, or my fight with them) and its round.
    held: Vec<Held>,
    held_round: u32,
    /// Held came from a fight (replaced by any scouting read, or a better read that round).
    fought: bool,
}

impl Player {
    /// Copies of each named unit (board and bench).
    pub fn units(&self) -> Vec<(Api, u32)> {
        let mut out: Vec<(Api, u32)> = vec![];
        for h in &self.held {
            let Some(api) = &h.api else { continue };
            match out.iter_mut().find(|o| &o.0 == api) {
                Some(o) => o.1 += copies(h.star),
                None => out.push((api.clone(), copies(h.star))),
            }
        }
        out
    }

    pub fn held(&self) -> &[Held] {
        &self.held
    }

    /// Copies of `api` they hold.
    pub fn copies(&self, api: &str) -> u32 {
        self.held.iter().filter(|h| h.api.as_deref() == Some(api)).map(|h| copies(h.star)).sum()
    }

    /// Some board unit of theirs is unnamed (or none were read): the trait panel speaks for them.
    fn unsure(&self) -> bool {
        self.held.is_empty() || self.held.iter().any(|h| h.api.is_none() && !h.bench)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Scout {
    players: Vec<Player>,
    /// The last combat read's red bars (the still gate) and the last read of others' units.
    last_foes: Vec<SeenUnit>,
    last: Vec<Held>,
    /// Use every read without waiting for a still one (replays of sparse captures).
    pub single_reads: bool,
    /// This round's fight: seed votes per red bar.
    votes: Vec<Vote>,
    votes_round: u32,
}

/// One red bar over a fight's still reads: its seed similarity per unit, summed.
#[derive(Debug, Clone)]
struct Vote {
    x: f32,
    y: f32,
    bench: Option<u32>,
    reads: u32,
    sums: Vec<(Api, f32)>,
}

impl Vote {
    fn same(&self, u: &SeenUnit) -> bool {
        match (self.bench, u.bench) {
            (Some(a), Some(b)) => a == b,
            (None, None) => (self.x - u.x).abs() <= SAME_BAR && (self.y - u.y).abs() <= SAME_BAR,
            _ => false,
        }
    }

    /// Add one read's matches (`Library::classify`; seed ones count).
    fn add(&mut self, u: &SeenUnit, c: &[(Api, f32, bool)]) {
        (self.x, self.y, self.reads) = (u.x, u.y, self.reads + 1);
        for (api, s, _) in c.iter().filter(|m| m.2) {
            match self.sums.iter_mut().find(|v| &v.0 == api) {
                Some(v) => v.1 += s,
                None => self.sums.push((api.clone(), *s)),
            }
        }
    }

    /// The `allowed` unit whose mean similarity is at least SEED_SIM and SEED_MARGIN ahead of the
    /// next allowed one.
    fn named(&self, allowed: impl Fn(&Api) -> bool) -> Option<Api> {
        let n = self.reads.max(1) as f32;
        let mut c: Vec<(Api, f32, bool)> = self.sums.iter().filter(|v| allowed(&v.0)).map(|v| (v.0.clone(), v.1 / n, true)).collect();
        c.sort_by(|a, b| b.1.total_cmp(&a.1));
        crate::library::standout(&c, true, SEED_SIM, SEED_MARGIN).cloned()
    }
}

impl Scout {
    /// Players seen within SCOUT_DECAY rounds.
    pub fn players(&self) -> &[Player] {
        &self.players
    }

    /// Other players' units in the last read that showed some (for replays).
    pub fn last_read(&self) -> Vec<Held> {
        self.last.clone()
    }

    /// Copies of `api` other players hold (named units only).
    pub fn held(&self, api: &str) -> u32 {
        self.players.iter().map(|p| p.copies(api)).sum()
    }

    /// One read of my screen: a scouted board, or my fight's red bars. `round` is `round_index`;
    /// `mine` my last trait panel (a "scouted" board showing it is mine, my plate unread);
    /// `owned` my copies per unit (out of the pool).
    pub fn observe(&mut self, snap: &TftSnapshot, set: &TftSetData, lib: &Library, round: u32, mine: &[TraitCount], owned: &[(Api, u32)]) {
        self.players.retain(|p| p.seen + SCOUT_DECAY >= round);
        // The carousel shows every tactician's plate over my own panel.
        if snap.stage.is_some_and(carousel) {
            return;
        }
        // In a fight the panel may be mine (my plate unread): only the red bars count.
        if snap.viewing_other && snap.enemies == 0 {
            self.last_foes.clear();
            if !same_panel(&snap.traits, mine) {
                self.scouted(snap, set, lib, round);
            }
        } else if !snap.foes.is_empty() && !snap.stage.is_some_and(pve) {
            let still = self.single_reads || still(&self.last_foes, &snap.foes);
            self.last_foes = snap.foes.clone();
            if still {
                self.fought(snap, set, lib, round, owned);
            }
        } else {
            self.last_foes.clear();
        }
    }

    /// The player known by `name`, else the one whose named units overlap `held`'s most
    /// (SAME_BOARD of the union).
    fn find(&self, name: Option<&str>, held: &[Held]) -> Option<usize> {
        if let Some(n) = name {
            return self.players.iter().position(|p| p.name.as_deref().is_some_and(|m| same_name(m, n)));
        }
        let names: HashSet<&Api> = held.iter().filter_map(|h| h.api.as_ref()).collect();
        let similar = |p: &Player| {
            let theirs: HashSet<&Api> = p.held.iter().filter_map(|h| h.api.as_ref()).collect();
            names.intersection(&theirs).count() as f32 / names.union(&theirs).count().max(1) as f32
        };
        self.players.iter().enumerate().map(|(i, p)| (i, similar(p))).filter(|&(_, s)| s >= SAME_BOARD).max_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i)
    }

    /// `api` can be one of `who`'s units: copies of it are left in the pool once other players'
    /// and mine (`owned`) are out, and on the board (not `bench`) every trait of it shows on
    /// their last panel (up to its hidden rows off it). Any unit when their panel isn't known.
    fn allowed(&self, who: Option<usize>, api: &Api, bench: bool, set: &TftSetData, owned: &[(Api, u32)]) -> bool {
        let Some(def) = set.units.get(api) else { return false };
        let others: u32 = self.players.iter().enumerate().filter(|(i, _)| Some(*i) != who).map(|(_, p)| p.copies(api)).sum();
        let mine: u32 = owned.iter().filter(|o| &o.0 == api).map(|o| o.1).sum();
        if pool(def.cost).is_some_and(|n| n <= others + mine) {
            return false;
        }
        let Some(p) = who.map(|i| &self.players[i]).filter(|p| !p.traits.is_empty() && !bench) else { return true };
        let off = def.traits.iter().filter(|t| !p.traits.iter().any(|(x, n)| x == *t && *n > 0)).count() as u32;
        off <= p.hidden
    }

    /// The player a read is of: by name, else (or an unnamed entry) by trait panel likeness; new if none.
    fn player(&mut self, name: Option<&str>, traits: &[TraitCount]) -> &mut Player {
        let by_name = name.and_then(|n| self.players.iter().position(|p| p.name.as_deref().is_some_and(|m| same_name(m, n))));
        let names: HashSet<&Api> = traits.iter().map(|t| &t.0).collect();
        let similar = |p: &Player| {
            let theirs: HashSet<&Api> = p.traits.iter().map(|t| &t.0).collect();
            names.intersection(&theirs).count() as f32 / names.union(&theirs).count().max(1) as f32
        };
        let by_traits = || {
            self.players
                .iter()
                .enumerate()
                .filter(|(_, p)| name.is_none() || p.name.is_none())
                .map(|(i, p)| (i, similar(p)))
                .filter(|&(_, s)| s >= SAME_BOARD && !traits.is_empty())
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        };
        let i = match by_name.or_else(by_traits) {
            Some(i) => i,
            None => {
                if self.players.len() >= MAX_PLAYERS
                    && let Some(i) = self.players.iter().enumerate().min_by_key(|(_, p)| p.seen).map(|(i, _)| i)
                {
                    self.players.remove(i);
                }
                let mut board = board::Board::default();
                (board.single_reads, board.seed) = (self.single_reads, true);
                self.players.push(Player { board, ..Player::default() });
                self.players.len() - 1
            }
        };
        let p = &mut self.players[i];
        if p.name.is_none() {
            p.name = name.map(str::to_string);
        }
        p
    }

    /// The player whose named units overlap `held`'s most (SAME_BOARD of the union); new if none.
    fn player_by_units(&mut self, held: &[Held]) -> &mut Player {
        match self.find(None, held) {
            Some(i) => &mut self.players[i],
            None => self.player(None, &[]),
        }
    }

    fn scouted(&mut self, snap: &TftSnapshot, set: &TftSetData, lib: &Library, round: u32) {
        if snap.traits.is_empty() && snap.units.is_empty() {
            return;
        }
        let p = self.player(snap.viewing.as_deref(), &snap.traits);
        p.seen = round;
        if !snap.traits.is_empty() {
            (p.traits, p.hidden) = (snap.traits.clone(), snap.traits_hidden);
        }
        if !p.board.observe_other(snap, set, lib) {
            return;
        }
        let held: Vec<Held> = p
            .board
            .pieces()
            .iter()
            .map(|q| Held {
                x: q.x,
                y: q.y,
                api: q.api.clone(),
                how: match q.named {
                    Named::Panel | Named::Bought => "=",
                    Named::Looks => "~",
                    Named::No => "?",
                },
                star: q.star,
                bench: q.bench.is_some(),
            })
            .collect();
        (p.held, p.held_round, p.fought) = (held.clone(), round, false);
        self.last = held;
    }

    /// My opponent's red bars, named by learnt looks (as on my board, else centred on this read's
    /// mean), else by the classifier head, else by the seed vote: each bar's seed similarities summed over this fight's still
    /// reads (a bar is the same across reads by bench slot or SAME_BAR), named among the units
    /// `allowed` for them (their plate's player, else the one the learnt names point to).
    /// The read naming the most copies stands for the round (units die and hide as the fight goes
    /// on); a scouting read that round stays. Without their plate, the player holding most of
    /// the same units is them.
    fn fought(&mut self, snap: &TftSnapshot, set: &TftSetData, lib: &Library, round: u32, owned: &[(Api, u32)]) {
        let embs: Vec<&Vec<f32>> = snap.units.iter().chain(&snap.foes).map(|u| &u.emb).filter(|e| !e.is_empty()).collect();
        let d = embs.first().map_or(0, |e| e.len());
        let center: Vec<f32> = (0..d).map(|i| embs.iter().map(|e| e.get(i).copied().unwrap_or(0.0)).sum::<f32>() / embs.len() as f32).collect();
        let foes: Vec<&SeenUnit> = snap.foes.iter().filter(|u| !u.emb.is_empty()).collect();
        if foes.is_empty() {
            return;
        }
        let matches: Vec<Vec<(Api, f32, bool)>> = foes.iter().map(|u| lib.classify(&u.emb)).collect();
        let mut held: Vec<Held> = foes
            .iter()
            .zip(&matches)
            .map(|(u, c)| {
                let looks = board::by_matches(c, set, false).or_else(|| by_centred_looks(lib, &u.emb, &center, set));
                let (api, how) = match looks {
                    Some(a) => (Some(a), "~"),
                    None => match lib.head_name(&u.emb).filter(|a| set.units.contains_key(a)) {
                        Some(a) => (Some(a), "^"),
                        None => (None, "?"),
                    },
                };
                Held { x: u.x, y: u.y, how, api, star: u.star, bench: u.bench.is_some() }
            })
            .collect();
        if self.votes_round != round {
            (self.votes, self.votes_round) = (vec![], round);
        }
        let who = self.find(snap.viewing.as_deref(), &held);
        let mut votes = std::mem::take(&mut self.votes);
        let mut used = vec![false; votes.len()];
        for ((u, c), h) in foes.iter().zip(&matches).zip(held.iter_mut()) {
            let i = match (0..votes.len()).find(|&i| !used[i] && votes[i].same(u)) {
                Some(i) => i,
                None => {
                    votes.push(Vote { x: u.x, y: u.y, bench: u.bench, reads: 0, sums: vec![] });
                    used.push(false);
                    votes.len() - 1
                }
            };
            used[i] = true;
            votes[i].add(u, c);
            if h.api.is_none()
                && let Some(api) = votes[i].named(|a| self.allowed(who, a, h.bench, set, owned))
            {
                (h.api, h.how) = (Some(api), "≈");
            }
        }
        self.votes = votes;
        self.last = held.clone();
        let p = match &snap.viewing {
            Some(name) => self.player(Some(name), &[]),
            None => self.player_by_units(&held),
        };
        p.seen = round;
        let named = |h: &[Held]| h.iter().filter(|h| h.api.is_some()).map(|h| copies(h.star)).sum::<u32>();
        let better = p.held_round < round || (p.fought && named(&held) > named(&p.held));
        if better && named(&held) > 0 {
            (p.held, p.held_round, p.fought) = (held, round, true);
        }
    }

    /// Players likely holding `unit`: it's named among their units, or (when some of their board
    /// is unnamed) every trait of it shows on their panel and either one of them belongs to it
    /// alone (a unique trait) or the panel shows CONTEST_SHARE of `comp`'s traits.
    pub fn holders(&self, unit: &str, comp: &MetaComp, set: &TftSetData) -> u32 {
        let def = set.units.get(unit).filter(|d| !d.traits.is_empty());
        let unique = def.is_some_and(|d| d.traits.iter().any(|t| set.units.values().filter(|u| u.traits.contains(t)).count() == 1));
        self.players
            .iter()
            .filter(|p| {
                if p.copies(unit) > 0 {
                    return true;
                }
                let Some(def) = def else { return false };
                let has = |t: &Api| p.traits.iter().any(|(x, n)| x == t && *n > 0);
                let share = comp.traits.iter().filter(|(t, _)| has(t)).count() as f32 / comp.traits.len().max(1) as f32;
                p.unsure() && def.traits.iter().all(has) && (unique || share >= CONTEST_SHARE)
            })
            .count() as u32
    }

    /// "4 held by others, 7 left" (named copies), else "contested by 2 boards" (trait panels).
    pub fn say(c: &Contest) -> String {
        let s = |n: u32| if n == 1 { "" } else { "s" };
        match (c.held, c.left) {
            (0, _) => format!("contested by {} board{}", c.boards, s(c.boards)),
            (h, Some(l)) => format!("{h} held by others, {l} left"),
            (h, None) => format!("{h} held by others"),
        }
    }

    /// `unit` as contested: players holding it, copies they hold, copies left in the pool once
    /// theirs and my `mine` are out.
    pub fn contest(&self, unit: &str, comp: &MetaComp, set: &TftSetData, mine: u32) -> Contest {
        let held = self.held(unit);
        let left = set.units.get(unit).and_then(|d| pool(d.cost)).map(|n| n.saturating_sub(held + mine));
        Contest { unit: unit.into(), boards: self.holders(unit, comp, set), held, left }
    }
}

/// The same traits at the same counts (in any order).
fn same_panel(a: &[TraitCount], b: &[TraitCount]) -> bool {
    !a.is_empty() && a.len() == b.len() && a.iter().all(|t| b.contains(t))
}

/// A unit named by centred looks when one stands out (and is in the set).
fn by_centred_looks(lib: &Library, emb: &[f32], center: &[f32], set: &TftSetData) -> Option<Api> {
    if lib.units() < CENTRED_UNITS {
        return None;
    }
    let c = lib.classify_centered(emb, center);
    let (api, s) = c.first()?;
    let next = c.get(1).map_or(0.0, |n| n.1);
    (*s >= CENTRED_SIM && s - next >= CENTRED_MARGIN && set.units.contains_key(api)).then(|| api.clone())
}

/// PvE rounds (minions, monsters): 1-2..1-4, then the 7th round of every later stage.
pub(crate) fn pve((stage, round): (u32, u32)) -> bool {
    (stage == 1 && round >= 2) || (stage >= 2 && round == 7)
}

/// Carousel rounds: 1-1, then the 4th round of every later stage.
pub(crate) fn carousel((stage, round): (u32, u32)) -> bool {
    (stage, round) == (1, 1) || (stage >= 2 && round == 4)
}

fn still(a: &[SeenUnit], b: &[SeenUnit]) -> bool {
    a.len() == b.len() && a.iter().all(|u| b.iter().any(|v| (u.x - v.x).abs() <= STILL && (u.y - v.y).abs() <= STILL))
}
