//! Plain TFT data shared by the TFT crates and mirrored to TypeScript (ts-rs,
//! `web/src/lib/tft-bindings/`). Set 18 API names are kept as strings
//! (`DA_18_Lux`, `DA_GuinsoosRageblade`, `DA_18_Defender`): they are the ids
//! Community Dragon, MetaTFT and the LCU match history share.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Unit, trait, item or augment API name.
pub type Api = String;

macro_rules! data {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
        #[ts(export, export_to = "../../../web/src/lib/tft-bindings/")]
        $item
    )*};
}

macro_rules! data_default {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
        #[ts(export, export_to = "../../../web/src/lib/tft-bindings/")]
        $item
    )*};
}

macro_rules! enums {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        #[serde(rename_all = "lowercase")]
        #[ts(export, export_to = "../../../web/src/lib/tft-bindings/")]
        $item
    )*};
}

/// Queue ids this coach supports.
pub const QUEUE_RANKED: i64 = 1100;
pub const QUEUE_NORMAL: i64 = 1090;
pub const QUEUE_CHONCC: i64 = 1230;

enums! {
    /// Which rules apply: the standard set, or a limited-time mode.
    pub enum TftMode { Standard, Choncc }

    /// What the TFT screens show.
    pub enum TftPhase { None, Lobby, InGame, PostGame }

    /// Augment rarity.
    pub enum AugmentTier { Silver, Gold, Prismatic }

    pub enum EconAction { Level, Roll, Save, Hold }
}

data_default! {
    // ---- static set data (Community Dragon) ----

    pub struct TftSetData {
        /// Community Dragon version string, e.g. "16.21.1" or a date.
        pub version: String,
        pub set: u32,
        pub units: HashMap<Api, TftUnit>,
        pub traits: HashMap<Api, TftTrait>,
        pub items: HashMap<Api, TftItem>,
        pub augments: HashMap<Api, TftAugment>,
    }

    // ---- meta stats (MetaTFT) ----

    /// Meta for one queue. Stats are placement histograms collapsed to averages.
    pub struct TftMeta {
        #[ts(type = "number")]
        pub queue: i64,
        /// MetaTFT `updated` (unix ms).
        #[ts(type = "number")]
        pub updated: u64,
        pub comps: Vec<MetaComp>,
        pub units: Vec<PlaceStat>,
        pub items: Vec<PlaceStat>,
        pub traits: Vec<PlaceStat>,
        /// Empty when no augment source answered.
        pub augments: Vec<PlaceStat>,
    }

    // ---- live reading (tft-vision) ----

    /// A unit on my screen, found by its HP bar.
    pub struct SeenUnit {
        /// The bar's left end, in 2560-wide pixels.
        pub x: f32,
        pub y: f32,
        /// ★ from the bar's cap; 0 when unreadable.
        pub star: u32,
        /// Bench slot 0..=8; None on the board.
        pub bench: Option<u32>,
        /// The bar is full (planning, or untouched in combat).
        pub full: bool,
        /// Image embedding of the unit under the bar (empty without the embedder).
        #[serde(skip)]
        #[ts(skip)]
        pub emb: Vec<f32>,
    }

    /// One read of my screen. Every field is None / empty when it wasn't readable.
    pub struct TftSnapshot {
        /// Unix ms when captured.
        #[ts(type = "number")]
        pub at: u64,
        /// Stage and round, e.g. (4, 2) for "4-2".
        pub stage: Option<(u32, u32)>,
        pub gold: Option<u32>,
        pub level: Option<u32>,
        /// XP into the level and needed for the next.
        pub xp: Option<(u32, u32)>,
        pub hp: Option<u32>,
        /// Win (positive) or loss (negative) streak shown next to gold.
        pub streak: Option<i32>,
        /// Units on board and the cap, e.g. (7, 9) with Choncc's Crown.
        pub team_size: Option<(u32, u32)>,
        /// The 5 shop slots (unit API name; None = empty or unreadable).
        pub shop: Vec<Option<Api>>,
        /// Augments on offer (empty unless the choice is on screen).
        pub augment_offer: Vec<Api>,
        /// Trait panel of the board on screen: (trait, unique units counted).
        pub traits: Vec<(Api, u32)>,
        /// Traits the panel has no room for (its "N+" badge; 9 when the badge shows but its number is unread).
        pub traits_hidden: u32,
        /// Item components and completed items on my item bench.
        pub items: Vec<Api>,
        /// The board on screen is another player's (scouting).
        pub viewing_other: bool,
        /// Units with green HP bars, board and bench.
        pub units: Vec<SeenUnit>,
        /// Red HP bars on screen (enemies in combat, PvE monsters).
        pub enemies: u32,
        /// The other player whose units are on screen: the scouted board's owner, or my combat
        /// opponent (their name plate, as in the player list).
        pub viewing: Option<String>,
        /// Units with red HP bars (my combat opponent's, PvE monsters); `emb` only while every bar is full.
        pub foes: Vec<SeenUnit>,
    }
}

data! {
    pub struct TftUnit {
        pub api: Api,
        pub name: String,
        /// 1..=5 (6+ for special units).
        pub cost: u32,
        pub traits: Vec<Api>,
        /// Community Dragon asset path (square icon).
        pub icon: String,
    }

    pub struct TftTrait {
        pub api: Api,
        pub name: String,
        /// Unit counts that activate a tier, ascending.
        pub breakpoints: Vec<u32>,
        pub icon: String,
    }

    pub struct TftItem {
        pub api: Api,
        pub name: String,
        /// The two components of a craftable item; empty for components and specials.
        pub components: Vec<Api>,
        pub component: bool,
        pub emblem: bool,
        pub radiant: bool,
        pub artifact: bool,
        pub icon: String,
    }

    pub struct TftAugment {
        pub api: Api,
        pub name: String,
        pub tier: AugmentTier,
        pub desc: String,
        pub icon: String,
    }

    /// One MetaTFT comp cluster.
    pub struct MetaComp {
        /// MetaTFT cluster id, e.g. "426003".
        pub id: String,
        /// Display name from its headline trait/unit, e.g. "Defender Rengar".
        pub name: String,
        pub units: Vec<Api>,
        /// Units it wants upgraded (MetaTFT `stars`).
        pub stars: Vec<Api>,
        /// Active traits as (trait, units).
        pub traits: Vec<(Api, u32)>,
        pub avg_place: f32,
        pub games: u32,
        /// "lvl 7" (reroll), "lvl 8", "lvl 9", … as MetaTFT names it.
        pub levelling: Option<String>,
        /// Carry item sets, best first.
        pub builds: Vec<CarryBuild>,
        /// Most built items in the comp.
        pub items: Vec<Api>,
        /// Augments that do well in it (may be empty).
        pub augments: Vec<Api>,
    }

    pub struct CarryBuild {
        pub unit: Api,
        pub items: Vec<Api>,
        pub avg_place: f32,
        pub games: u32,
    }

    /// Placement stats for a unit, item, trait or augment.
    pub struct PlaceStat {
        pub api: Api,
        pub games: u32,
        pub avg_place: f32,
        /// Share of games in the top 4, 0..=1.
        pub top4: f32,
    }

    // ---- coach output ----

    /// A unit I own: recognised on my board and bench (a piece not recognised yet has `api`
    /// ""), else tracked from purchases (or seen only through traits, `star` 0).
    pub struct OwnedUnit {
        pub api: Api,
        pub copies: u32,
        /// 1..=3 from copies.
        pub star: u32,
        /// Pieces of it on the board and on the bench (both 0 when only purchases are known).
        pub board: u32,
        pub bench: u32,
    }

    pub struct CompPick {
        pub comp: String,
        pub name: String,
        /// 0..=1, how much of it I already have.
        pub fit: f32,
        pub avg_place: f32,
        pub have: Vec<Api>,
        pub missing: Vec<Api>,
        pub levelling: Option<String>,
    }

    pub struct ShopAdvice {
        /// 0..5.
        pub slot: u32,
        pub unit: Api,
        pub buy: bool,
        pub reason: String,
        /// Other players hold it (scouted, or fought).
        pub contested: u32,
        /// Copies other players hold (units named on their boards and benches).
        pub held: u32,
        /// Copies left in the pool once theirs and mine are out (None: the unit has no pool).
        pub left: Option<u32>,
    }

    /// A unit other players hold.
    pub struct Contest {
        pub unit: Api,
        /// Players holding it: named among their units, or guessed from their trait panel.
        pub boards: u32,
        /// Copies they hold (★1 1, ★2 3, ★3 9; bench included), named units only.
        pub held: u32,
        /// Copies left in the pool once theirs and mine are out (None: no pool).
        pub left: Option<u32>,
    }

    pub struct EconAdvice {
        pub action: EconAction,
        pub text: String,
    }

    pub struct ItemAdvice {
        pub item: Api,
        pub from: Vec<Api>,
        /// Who should hold it, from the target comp's carry builds.
        pub holder: Option<Api>,
        /// Make it now rather than hold the components.
        pub now: bool,
        pub reason: String,
    }

    pub struct AugmentRank {
        pub augment: Api,
        pub score: f32,
        pub reason: String,
    }

    /// Spoken once per key, like the LoL coach's calls.
    pub struct TftCall {
        pub key: String,
        pub text: String,
        pub say: String,
    }

    /// Choncc or Treat additions (None in the standard mode).
    pub struct ChonccAdvice {
        /// The 4- or 5-cost unit I started with, once seen.
        pub starter: Option<Api>,
        /// Comps that use the starter, best first.
        pub comps: Vec<CompPick>,
        /// Loot to use first, with reasons.
        pub loot: Vec<String>,
        pub notes: Vec<String>,
    }

    pub struct TftAdvice {
        pub mode: TftMode,
        pub stage: Option<(u32, u32)>,
        pub owned: Vec<OwnedUnit>,
        /// The automatic target first, then the alternatives.
        pub comps: Vec<CompPick>,
        pub shop: Vec<ShopAdvice>,
        pub econ: Option<EconAdvice>,
        pub items: Vec<ItemAdvice>,
        pub augments: Vec<AugmentRank>,
        /// Units of my target comp other players hold, most held first.
        pub contested: Vec<Contest>,
        pub calls: Vec<TftCall>,
        pub choncc: Option<ChonccAdvice>,
    }

    // ---- history, review, trends ----

    pub struct TftPastUnit {
        pub api: Api,
        pub star: u32,
        pub items: Vec<Api>,
    }

    /// One of my games from the LCU TFT match history.
    pub struct TftPastGame {
        #[ts(type = "number")]
        pub game_id: u64,
        /// Unix ms.
        #[ts(type = "number")]
        pub created: u64,
        #[ts(type = "number")]
        pub queue: i64,
        pub mode: TftMode,
        pub placement: u32,
        pub level: u32,
        pub last_round: u32,
        pub duration_s: u32,
        pub units: Vec<TftPastUnit>,
        /// Active traits as (trait, units, tier reached).
        pub traits: Vec<(Api, u32, u32)>,
        /// Nearest MetaTFT comp, when one matched.
        pub comp: Option<String>,
    }

    pub struct TftReview {
        pub game: TftPastGame,
        /// The comp it matched, and that comp's average placement.
        pub comp: Option<CompPick>,
        pub takeaways: Vec<String>,
    }

    pub struct CompRecord {
        pub comp: String,
        pub name: String,
        pub games: u32,
        pub avg_place: f32,
        pub top4: f32,
    }

    pub struct TftTrends {
        pub mode: TftMode,
        pub games: u32,
        pub avg_place: f32,
        pub top4: f32,
        pub comps: Vec<CompRecord>,
    }

    /// Everything the TFT screens show; sent on the `tft` event.
    pub struct TftState {
        pub phase: TftPhase,
        #[ts(type = "number | null")]
        pub queue: Option<i64>,
        pub mode: TftMode,
        pub advice: Option<TftAdvice>,
        /// Lobby: top comps for the queue (Choncc: by starting carry when known).
        pub meta: Vec<CompPick>,
        pub history: Vec<TftPastGame>,
        pub trends: Vec<TftTrends>,
        pub review: Option<TftReview>,
        /// Sources: set data, meta, screen reading.
        pub status: Vec<(String, bool)>,
    }
}

impl TftMode {
    pub fn of_queue(queue: i64) -> TftMode {
        if queue == QUEUE_CHONCC { TftMode::Choncc } else { TftMode::Standard }
    }
}

/// Gold interest thresholds: +1 per 10 gold, up to 5.
pub fn interest(gold: u32) -> u32 {
    (gold / 10).min(5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_interest() {
        assert_eq!(TftMode::of_queue(QUEUE_CHONCC), TftMode::Choncc);
        assert_eq!(TftMode::of_queue(QUEUE_RANKED), TftMode::Standard);
        assert_eq!((interest(9), interest(37), interest(80)), (0, 3, 5));
    }
}
