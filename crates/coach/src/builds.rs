//! Build tables from the crawled ranked games (`lol-coach-server builds`, served as
//! `/v1/builds.json`): my item times, order and first back beside Emerald+ and Master+ on my
//! champion and role, a few tips, and the pace of my next item in game. Never win rates, and
//! never what to build: that stays Coachless'.

use crate::analysis::clock;
use crate::brief::{capital, thousands};
use crate::common::champ;
use model::*;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

pub const SWIFTPLAY_QUEUE: i64 = 480;
const TIPS: usize = 2;
const BACK_FROM: f32 = 120.0;
const SLOTS: [&str; 3] = ["first", "second", "third"];

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Table {
    pub patch: String,
    pub games: u32,
    #[serde(default)]
    pub bands: BTreeMap<String, u32>,
    pub finished: Vec<ItemId>,
    pub boots: Vec<ItemId>,
    pub magic_share: HashMap<ChampKey, f64>,
    pub damage_cuts: [f64; 2],
    pub lead_cut: f64,
    pub pairs: Vec<Pair>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Pair {
    pub champion: ChampKey,
    pub role: String,
    pub all: Side,
    pub top: Option<Side>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Side {
    pub games: u32,
    pub slots: Slots,
    pub first: Vec<Pick>,
    pub second: BTreeMap<ItemId, Vec<Pick>>,
    pub third: BTreeMap<ItemId, Vec<Pick>>,
    pub back: Option<Back>,
    pub situations: Vec<SituationPick>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Slots {
    pub first: Option<[u32; 3]>,
    pub second: Option<[u32; 3]>,
    pub third: Option<[u32; 3]>,
    pub boots: Option<[u32; 3]>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Pick {
    pub item: ItemId,
    pub share: f64,
    pub games: u32,
    #[serde(default)]
    pub time: Option<[u32; 3]>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Back {
    pub games: u32,
    pub time: [u32; 3],
    pub gold: [u32; 3],
    pub items: Vec<Pick>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct SituationPick {
    pub slot: u8,
    pub start: ItemId,
    pub kind: String,
    pub value: String,
    pub item: ItemId,
    pub share: f64,
    pub games: u32,
}

impl Table {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("build table: {e}"))
    }

    pub fn pair(&self, champion: ChampKey, lane: Lane) -> Option<&Pair> {
        self.pairs.iter().find(|p| p.champion == champion && p.role == role(lane))
    }

    fn damage(&self, enemies: impl Iterator<Item = ChampKey>) -> &'static str {
        let shares: Vec<f64> = enemies.map(|c| self.magic_share.get(&c).copied().unwrap_or(0.0)).collect();
        let value = shares.iter().sum::<f64>() / shares.len().max(1) as f64;
        if value < self.damage_cuts[0] {
            "physical"
        } else if value < self.damage_cuts[1] {
            "mixed"
        } else {
            "magic"
        }
    }

    fn lead(&self, mine: u32, theirs: u32) -> &'static str {
        let (mine, theirs) = (f64::from(mine), f64::from(theirs));
        if mine == theirs {
            "even"
        } else if mine <= theirs * (1.0 - self.lead_cut) {
            "behind"
        } else if mine >= theirs * (1.0 + self.lead_cut) {
            "ahead"
        } else {
            "even"
        }
    }
}

/// Riot's `teamPosition` for a lane.
pub fn role(lane: Lane) -> &'static str {
    match lane {
        Lane::Top => "TOP",
        Lane::Jungle => "JUNGLE",
        Lane::Middle => "MIDDLE",
        Lane::Bottom => "BOTTOM",
        Lane::Support => "UTILITY",
    }
}

fn lane_name(lane: Lane) -> &'static str {
    match lane {
        Lane::Top => "top",
        Lane::Jungle => "jungle",
        Lane::Middle => "mid",
        Lane::Bottom => "bottom",
        Lane::Support => "support",
    }
}

fn middle(p: Option<[u32; 3]>) -> Option<(u32, u32)> {
    p.map(|p| (p[0], p[2]))
}

fn item_name(sd: &StaticData, id: ItemId) -> String {
    sd.items.get(&id).map_or_else(|| format!("item {id}"), |i| i.name.clone())
}

fn percent(share: f64) -> String {
    format!("{} %", (share * 100.0).round() as u32)
}

fn or_list(sd: &StaticData, picks: &[Pick]) -> String {
    let named: Vec<String> = picks.iter().take(2).map(|p| format!("{} ({})", item_name(sd, p.item), percent(p.share))).collect();
    named.join(" or ")
}

fn slot_times(side: &Side, slot: usize) -> Option<[u32; 3]> {
    [side.slots.first, side.slots.second, side.slots.third].get(slot).copied().flatten()
}

fn item_times(side: &Side, slot: usize, item: ItemId) -> Option<[u32; 3]> {
    let own = (slot == 0).then(|| side.first.iter().find(|p| p.item == item).and_then(|p| p.time)).flatten();
    own.or_else(|| slot_times(side, slot))
}

fn coachless_has(coachless: Option<&ChampStats>, slot: usize, item: ItemId) -> bool {
    coachless.filter(|s| s.coachless.is_some()).and_then(|s| s.slots.get(slot)).is_some_and(|options| options.iter().any(|o| o.id == item))
}

/// My finished items (first purchase of each, in order) and the time of my finished boots.
fn my_build(table: &Table, purchases: &[Purchase]) -> (Vec<Purchase>, Option<f32>) {
    let mut finished: Vec<Purchase> = Vec::new();
    for p in purchases {
        if table.finished.contains(&p.item) && !finished.iter().any(|f| f.item == p.item) {
            finished.push(*p);
        }
    }
    (finished, purchases.iter().find(|p| table.boots.contains(&p.item)).map(|p| p.time))
}

fn frame_before(d: &MatchDetail, time: f32) -> Option<&Frame> {
    d.frames.iter().rev().find(|f| f.time < time)
}

/// The Build section of a reviewed game. `coachless`: my champion's build when it came from
/// Coachless; an order or situation tip is dropped when the item I took is one of its options.
pub fn review(d: &MatchDetail, table: &Table, coachless: Option<&ChampStats>, sd: &StaticData) -> Option<BuildReview> {
    let g = &d.game;
    if g.mode != Mode::Rift || g.queue_id == SWIFTPLAY_QUEUE {
        return None;
    }
    let me = d.players.get(d.me)?;
    let lane = me.lane?;
    let pair = table.pair(me.champ, lane)?;
    let (all, top) = (&pair.all, pair.top.as_ref());
    let (finished, boots) = my_build(table, &d.purchases);
    let secs = |t: f32| t.max(0.0) as u32;
    let mut items: Vec<BuildRow> = finished
        .iter()
        .take(3)
        .enumerate()
        .map(|(slot, p)| BuildRow {
            slot: format!("{} item", SLOTS[slot]),
            item: p.item,
            time: secs(p.time),
            all: middle(item_times(all, slot, p.item)),
            top: top.and_then(|t| middle(item_times(t, slot, p.item))),
        })
        .collect();
    if let Some(t) = boots {
        let item = d.purchases.iter().find(|p| table.boots.contains(&p.item)).map_or(0, |p| p.item);
        items.push(BuildRow { slot: "boots".into(), item, time: secs(t), all: middle(all.slots.boots), top: top.and_then(|t| middle(t.slots.boots)) });
    }
    let back = d.purchases.iter().find(|p| p.time >= BACK_FROM).map(|first| {
        let gold = frame_before(d, first.time).and_then(|f| f.players.get(d.me)).map_or(0, |s| s.current_gold);
        BackRow {
            time: secs(first.time),
            gold,
            all: all.back.as_ref().map(|b| (b.time[0], b.time[2])),
            all_gold: all.back.as_ref().map(|b| (b.gold[0], b.gold[2])),
            top: top.and_then(|t| t.back.as_ref()).map(|b| (b.time[0], b.time[2])),
            usual: all.back.as_ref().and_then(|b| b.items.first()).map(|p| p.item),
        }
    });
    let ctx = Ctx { d, table, side: all, coachless, sd, name: format!("{} {}", champ(sd, me.champ).name, lane_name(lane)) };
    let mut tips: Vec<String> = [ctx.order(&finished), ctx.situation(&finished), ctx.timing(&items), ctx.first_back(back.as_ref())].into_iter().flatten().collect();
    tips.truncate(TIPS);
    Some(BuildReview { patch: table.patch.clone(), games: all.games, top_games: top.map(|t| t.games), items, back, tips })
}

struct Ctx<'a> {
    d: &'a MatchDetail,
    table: &'a Table,
    side: &'a Side,
    coachless: Option<&'a ChampStats>,
    sd: &'a StaticData,
    name: String,
}

impl Ctx<'_> {
    fn order(&self, finished: &[Purchase]) -> Option<String> {
        let start = finished.first()?;
        if !self.side.first.iter().any(|p| p.item == start.item) {
            if coachless_has(self.coachless, 0, start.item) || self.side.first.is_empty() {
                return None;
            }
            return Some(format!("{} first is rare on {}; Emerald+ start {}.", item_name(self.sd, start.item), self.name, or_list(self.sd, &self.side.first)));
        }
        let second = finished.get(1)?;
        let next = self.side.second.get(&start.item)?;
        if next.is_empty() || next.iter().any(|p| p.item == second.item) || coachless_has(self.coachless, 1, second.item) {
            return None;
        }
        Some(format!("{} second after {} is rare on {}; Emerald+ go {}.", item_name(self.sd, second.item), item_name(self.sd, start.item), self.name, or_list(self.sd, next)))
    }

    fn situation(&self, finished: &[Purchase]) -> Option<String> {
        let start = finished.first()?;
        let me = &self.d.players[self.d.me];
        let damage = self.table.damage(self.d.players.iter().filter(|p| !p.ally).map(|p| p.champ));
        let opponent = self.d.players.iter().position(|p| !p.ally && p.lane.is_some() && p.lane == me.lane);
        for (slot, mine) in finished.iter().enumerate().take(3).skip(1) {
            let lists = if slot == 1 { &self.side.second } else { &self.side.third };
            let Some(overall) = lists.get(&start.item).and_then(|l| l.first()) else { continue };
            if mine.item != overall.item || coachless_has(self.coachless, slot, mine.item) {
                continue;
            }
            let lead = opponent.and_then(|o| frame_before(self.d, mine.time).and_then(|f| Some(self.table.lead(f.players.get(self.d.me)?.total_gold, f.players.get(o)?.total_gold))));
            let found = self.side.situations.iter().find(|s| {
                usize::from(s.slot) == slot + 1 && s.start == start.item && s.item != mine.item && ((s.kind == "damage" && s.value == damage) || (s.kind == "lead" && Some(s.value.as_str()) == lead))
            });
            if let Some(s) = found {
                let when = if s.kind == "damage" { format!("Into {} damage", s.value) } else { format!("When {} in gold", s.value) };
                return Some(format!("{when}, Emerald+ go {} {} ({}), not {}.", item_name(self.sd, s.item), SLOTS[slot], percent(s.share), item_name(self.sd, mine.item)));
            }
        }
        None
    }

    fn timing(&self, rows: &[BuildRow]) -> Option<String> {
        let row = rows.iter().find(|r| r.all.is_some_and(|(_, late)| r.time > late))?;
        let (from, to) = row.all?;
        let what = if row.slot == "boots" { "Boots".to_string() } else { capital(&row.slot) };
        let it = if row.slot == "boots" { "them" } else { "it" };
        Some(format!("{what} at {}; Emerald+ finish {it} between {} and {}.", clock(row.time as f32), clock(from as f32), clock(to as f32)))
    }

    fn first_back(&self, back: Option<&BackRow>) -> Option<String> {
        let back = back?;
        let ((_, late), (_, rich)) = (back.all?, back.all_gold?);
        if back.time <= late || back.gold <= rich {
            return None;
        }
        let (from, to) = back.all?;
        let usual = back.usual.map(|i| format!(", most often for {}", item_name(self.sd, i))).unwrap_or_default();
        Some(format!("First back at {} with {} gold; Emerald+ go back between {} and {}{usual}.", clock(back.time as f32), thousands(back.gold), clock(from as f32), clock(to as f32)))
    }
}

/// In game: the slot of my next finished item (the count of finished items I hold) and when
/// Emerald+ finish that slot. None in Swiftplay, outside the Rift, or without a table entry.
pub fn pace(table: &Table, champion: ChampKey, lane: Option<Lane>, mode: Mode, swiftplay: bool, held: &[ItemId]) -> Option<BuildPace> {
    if mode != Mode::Rift || swiftplay {
        return None;
    }
    let side = &table.pair(champion, lane?)?.all;
    let slot = held.iter().filter(|i| table.finished.contains(i)).count();
    let times = slot_times(side, slot)?;
    Some(BuildPace { slot: format!("{} item", SLOTS[slot]), early: times[0], median: times[1], late: times[2] })
}
