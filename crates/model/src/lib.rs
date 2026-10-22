//! Plain data shared by every crate and mirrored to TypeScript (ts-rs).
//! No behaviour beyond tiny helpers: sources produce these, `coach` consumes
//! them, the UI renders the `*Advice` types.
//!
//! IDs: a champion's Riot numeric key is also the lolalytics `cid` and the u.gg id.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

pub type ChampKey = u32;
pub type ItemId = u32;
pub type AugmentId = u32;

macro_rules! data {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
        #[ts(export, export_to = "../../../web/src/lib/bindings/")]
        $item
    )*};
}

macro_rules! data_default {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
        #[ts(export, export_to = "../../../web/src/lib/bindings/")]
        $item
    )*};
}

macro_rules! enums {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        #[serde(rename_all = "lowercase")]
        #[ts(export, export_to = "../../../web/src/lib/bindings/")]
        $item
    )*};
}

enums! {
    /// Lolalytics lane names; Live API `UTILITY` = Support, `MIDDLE` = Middle.
    pub enum Lane { Top, Jungle, Middle, Bottom, Support }

    /// Data family, not the exact queue: ranked and normal draft share Rift stats.
    pub enum Mode { Rift, Aram, Mayhem }

    pub enum Team { Order, Chaos }

    pub enum ChampClass { Assassin, Fighter, Mage, Marksman, Support, Tank }

    pub enum Rarity { Prismatic, Gold, Silver }

    pub enum Phase { NoClient, Idle, ChampSelect, Loading, InGame, PostGame }

    pub enum SourceState { Off, Ok, Degraded, Error }
}

impl Lane {
    pub const ALL: [Lane; 5] = [Lane::Top, Lane::Jungle, Lane::Middle, Lane::Bottom, Lane::Support];

    /// Lolalytics query value.
    pub fn as_str(self) -> &'static str {
        match self {
            Lane::Top => "top",
            Lane::Jungle => "jungle",
            Lane::Middle => "middle",
            Lane::Bottom => "bottom",
            Lane::Support => "support",
        }
    }

    /// Live Client API `position` / LCU `assignedPosition` (case-insensitive).
    pub fn from_riot(s: &str) -> Option<Lane> {
        match s.to_ascii_lowercase().as_str() {
            "top" => Some(Lane::Top),
            "jungle" => Some(Lane::Jungle),
            "middle" | "mid" => Some(Lane::Middle),
            "bottom" | "bot" | "adc" => Some(Lane::Bottom),
            "utility" | "support" => Some(Lane::Support),
            _ => None,
        }
    }
}

impl Mode {
    /// LCU queue id → data family. 450 = ARAM, 2400 = ARAM: Mayhem.
    pub fn from_queue(queue_id: i64) -> Mode {
        match queue_id {
            450 | 100 => Mode::Aram,
            2400..=2499 => Mode::Mayhem,
            _ => Mode::Rift,
        }
    }
}

data_default! {
    /// What an item answers, or what the enemy team demands. Each field 0..=1.
    /// `coach` scores items by `dot(item.answers, needs)`.
    pub struct Profile {
        pub armor: f32,
        pub mr: f32,
        pub hp: f32,
        pub anti_heal: f32,
        pub tenacity: f32,
        pub cleanse: f32,
        /// Stasis, revive, spell shield, big shields.
        pub anti_burst: f32,
        pub armor_pen: f32,
        pub magic_pen: f32,
        pub anti_crit: f32,
        pub anti_attack_speed: f32,
    }

    pub struct ItemStats {
        pub ad: f32,
        pub ap: f32,
        pub armor: f32,
        pub mr: f32,
        pub hp: f32,
        pub mana: f32,
        pub attack_speed: f32,
        pub crit: f32,
        pub lethality: f32,
        pub armor_pen_pct: f32,
        pub magic_pen_flat: f32,
        pub magic_pen_pct: f32,
        pub ability_haste: f32,
        pub lifesteal: f32,
        pub omnivamp: f32,
        pub move_speed: f32,
    }

    /// Hand-curated champion flags Data Dragon lacks (0..=1 each).
    pub struct Traits {
        /// Share of damage that is magic (0 = all physical).
        pub magic_share: f32,
        pub true_damage: f32,
        pub heals: f32,
        pub cc: f32,
        pub burst: f32,
        pub on_hit: f32,
        pub tank: f32,
    }

    /// Per-champion averages from lolalytics for the chosen rank (per game).
    pub struct Benchmarks {
        pub cs_per_min: Option<f32>,
        pub gold_per_min: Option<f32>,
        pub damage_per_min: Option<f32>,
        pub vision_per_min: Option<f32>,
    }
}

data! {
    pub struct Item {
        pub id: ItemId,
        pub name: String,
        /// Full cost including components.
        pub cost: u32,
        pub from: Vec<ItemId>,
        pub into: Vec<ItemId>,
        pub stats: ItemStats,
        pub answers: Profile,
        pub boots: bool,
        /// Finished item worth recommending (legendary / boots tier 2, not a component).
        pub completed: bool,
        pub rift: bool,
        pub aram: bool,
        /// Data Dragon tags (Armor, SpellBlock, Tenacity, …).
        pub tags: Vec<String>,
    }

    pub struct Champion {
        pub key: ChampKey,
        /// Data Dragon id, e.g. "MonkeyKing".
        pub id: String,
        pub name: String,
        /// lolalytics / u.gg slug, e.g. "wukong", "kaisa".
        pub slug: String,
        pub classes: Vec<ChampClass>,
        pub traits: Traits,
    }

    pub struct StaticData {
        /// Data Dragon version, e.g. "16.21.1".
        pub version: String,
        pub items: HashMap<ItemId, Item>,
        pub champs: HashMap<ChampKey, Champion>,
    }

    // ---- lolalytics ----

    pub struct ItemStat {
        pub id: ItemId,
        pub games: u32,
        pub wins: u32,
    }

    pub struct EarlySet {
        pub items: Vec<ItemId>,
        pub win_rate: f32,
        pub pick_rate: f32,
        pub games: u32,
    }

    pub struct Matchup {
        pub champ: ChampKey,
        /// Our win rate vs them, percent.
        pub vs_wr: f32,
        pub games: u32,
        pub default_lane: Option<Lane>,
    }

    pub struct ChampStats {
        pub champ: ChampKey,
        pub lane: Option<Lane>,
        pub mode: Mode,
        /// Patch query actually used, e.g. "16.21" or "30" (days, fallback).
        pub patch: String,
        pub win_rate: f32,
        pub pick_rate: f32,
        #[ts(type = "number")]
        pub games: u64,
        /// Share of the champ's games per lane, percent.
        pub lanes: HashMap<Lane, f32>,
        /// `slots[0]` = first completed item … up to 5 slots.
        pub slots: Vec<Vec<ItemStat>>,
        pub boots: Vec<ItemStat>,
        pub early: Vec<EarlySet>,
        pub counters: Vec<Matchup>,
        pub benchmarks: Benchmarks,
    }

    /// One row of a lolalytics lane tier list.
    pub struct TierEntry {
        pub champ: ChampKey,
        pub lane: Lane,
        pub win_rate: f32,
        pub pick_rate: f32,
        pub ban_rate: f32,
        pub games: u32,
        /// lolalytics tier label if present ("S+", "S", …).
        pub tier: Option<String>,
    }

    // ---- u.gg ARAM: Mayhem ----

    pub struct MayhemData {
        /// u.gg patch form, e.g. "16_21".
        pub patch: String,
        pub augment_names: HashMap<AugmentId, String>,
        /// Champion → tier label ("S+", "S", "A", …).
        pub champ_tiers: HashMap<ChampKey, String>,
    }

    /// Ordered best → worst within each rarity.
    pub struct ChampAugments {
        pub champ: ChampKey,
        pub prismatic: Vec<AugmentId>,
        pub gold: Vec<AugmentId>,
        pub silver: Vec<AugmentId>,
    }

    // ---- live game (Live Client Data API) ----

    pub struct Player {
        pub riot_id: String,
        pub champ: ChampKey,
        pub team: Team,
        pub position: Option<Lane>,
        pub level: u32,
        pub kills: u32,
        pub deaths: u32,
        pub assists: u32,
        pub cs: u32,
        pub ward_score: f32,
        pub items: Vec<ItemId>,
        pub dead: bool,
        pub respawn_in: f32,
    }

    pub struct GameSnapshot {
        /// Seconds since game start.
        pub time: f32,
        pub mode: Mode,
        pub map: u32,
        /// Index into `players` of the active player.
        pub me: usize,
        pub my_gold: f32,
        pub players: Vec<Player>,
        pub events: Vec<GameEvent>,
    }

    pub enum GameEvent {
        /// Player indices into `GameSnapshot::players`; killer is None for turrets/minions.
        Kill { time: f32, killer: Option<usize>, victim: usize, assisters: Vec<usize> },
        Dragon { time: f32, team: Option<Team>, kind: String },
        Baron { time: f32, team: Option<Team> },
        Herald { time: f32, team: Option<Team> },
        /// Any other objective the season adds (grubs, atakhan, …).
        Objective { time: f32, team: Option<Team>, name: String },
        GameEnd { time: f32, win: bool },
    }

    // ---- champ select (LCU) ----

    pub struct ChampSelect {
        #[ts(type = "number")]
        pub queue_id: i64,
        pub mode: Mode,
        pub my_lane: Option<Lane>,
        pub my_hover: Option<ChampKey>,
        pub my_pick: Option<ChampKey>,
        pub locked: bool,
        pub allies: Vec<ChampKey>,
        pub enemies: Vec<ChampKey>,
        pub bans: Vec<ChampKey>,
        /// ARAM bench (swappable champions).
        pub bench: Vec<ChampKey>,
        /// "PLANNING" | "BAN_PICK" | "FINALIZATION" | …
        pub timer_phase: String,
        /// Allies' assigned positions (visible to your team in draft queues).
        pub ally_lanes: Vec<(ChampKey, Lane)>,
    }

    /// LCU end-of-game stats for the local player.
    pub struct EogStats {
        pub champ: ChampKey,
        pub mode: Mode,
        pub win: bool,
        pub duration_s: u32,
        pub kills: u32,
        pub deaths: u32,
        pub assists: u32,
        pub cs: u32,
        pub gold: u32,
        pub damage_to_champs: u32,
        pub damage_taken: u32,
        pub vision: u32,
        pub items: Vec<ItemId>,
    }

    // ---- advice (coach output, rendered by the UI) ----

    pub struct ItemPick {
        pub item: ItemId,
        pub name: String,
        pub cost: u32,
        pub score: f32,
        /// One line, e.g. "Zed is 7/1 and killed you 3×".
        pub reason: String,
        /// Best affordable component toward `item` when gold is short.
        pub component_now: Option<ItemId>,
    }

    pub struct Threat {
        pub champ: ChampKey,
        pub name: String,
        /// 0..=100.
        pub score: f32,
        pub kills: u32,
        pub deaths: u32,
        pub assists: u32,
        pub level_diff: i32,
        pub item_gold_diff: i32,
        pub kills_on_me: u32,
        pub lane_opp: bool,
        /// "AD" | "AP" | "Mixed" | "True".
        pub damage: String,
        /// Short flags: "assassin", "heals", "cc", "tank", "crit", …
        pub flags: Vec<String>,
    }

    pub struct MatchupTip {
        pub champ: ChampKey,
        pub vs_wr: f32,
        pub games: u32,
        pub lane_opp: bool,
        /// "favoured" | "even" | "unfavoured".
        pub label: String,
    }

    pub struct SelfStats {
        pub cs_per_min: f32,
        pub cs_benchmark: Option<f32>,
        /// CS ahead (+) / behind (−) the benchmark at this game time.
        pub cs_diff: Option<f32>,
        pub item_gold_vs_opp: Option<i32>,
        pub level_vs_opp: Option<i32>,
        pub deaths: u32,
        /// Who killed me and how often, most first.
        pub killers: Vec<(ChampKey, u32)>,
        pub vision: f32,
        pub vision_benchmark: Option<f32>,
    }

    pub struct ObjectiveTimer {
        pub name: String,
        /// Game time (s) of next spawn; None when up / unknown.
        pub next_spawn: Option<f32>,
    }

    pub struct AugmentRank {
        pub id: AugmentId,
        pub name: String,
        pub rarity: Option<Rarity>,
        /// 0-based position in this champ's list for that rarity.
        pub rank: Option<u32>,
        pub of: u32,
    }

    pub struct AugmentAdvice {
        pub offered: Vec<AugmentRank>,
        pub best: Option<AugmentId>,
        pub reason: String,
    }

    pub struct GameAdvice {
        pub time: f32,
        pub mode: Mode,
        pub my_champ: ChampKey,
        pub buy_next: Option<ItemPick>,
        pub alternatives: Vec<ItemPick>,
        /// Remaining late-game path (excludes owned items).
        pub path: Vec<ItemPick>,
        pub owned: Vec<ItemId>,
        pub threats: Vec<Threat>,
        pub matchups: Vec<MatchupTip>,
        pub me: SelfStats,
        pub timers: Vec<ObjectiveTimer>,
        pub augments: Option<AugmentAdvice>,
        pub gold: GoldDiff,
    }

    pub struct PickSuggestion {
        pub champ: ChampKey,
        pub score: f32,
        pub reason: String,
    }

    pub struct BuildPlan {
        pub start: Vec<ItemId>,
        pub core: Vec<ItemPick>,
        pub boots: Option<ItemPick>,
    }

    pub struct BenchRank {
        pub champ: ChampKey,
        pub tier: Option<String>,
        pub mine: bool,
    }

    pub struct ChampSelectAdvice {
        pub mode: Mode,
        pub my_lane: Option<Lane>,
        pub my_champ: Option<ChampKey>,
        pub picks: Vec<PickSuggestion>,
        pub bans: Vec<PickSuggestion>,
        pub build: Option<BuildPlan>,
        /// ARAM / Mayhem: you + bench, best first.
        pub bench: Vec<BenchRank>,
        pub enemies: Vec<ChampKey>,
        /// Rift draft breakdown for both teams.
        pub draft: Option<DraftReport>,
    }

    pub struct Recap {
        pub stats: EogStats,
        pub benchmarks: Benchmarks,
        /// Items recommended over the game, in order first suggested.
        pub recommended: Vec<ItemId>,
        pub notes: Vec<String>,
    }

    pub struct Sources {
        pub client: SourceState,
        pub game: SourceState,
        pub lolalytics: SourceState,
        pub ugg: SourceState,
        pub ocr: SourceState,
        pub message: Option<String>,
    }

    pub struct Settings {
        /// lolalytics tier, e.g. "emerald_plus", "all", "diamond_plus".
        pub tier: String,
        /// lolalytics region, "all" by default.
        pub region: String,
        pub ocr_enabled: bool,
        pub record_fixtures: bool,
    }

    pub struct UiChamp {
        /// Data Dragon id (icon file name).
        pub id: String,
        pub name: String,
    }

    pub struct UiItem {
        pub name: String,
        pub cost: u32,
    }

    /// Names and icon keys for the UI (`get_static` command).
    pub struct UiStatic {
        /// Data Dragon version for icon URLs.
        pub version: String,
        /// u.gg patch for augment icon URLs.
        pub ugg_patch: Option<String>,
        pub champs: HashMap<ChampKey, UiChamp>,
        pub items: HashMap<ItemId, UiItem>,
        pub augments: HashMap<AugmentId, String>,
    }

    /// Everything the UI shows; pushed as the `state` event.
    pub struct AppState {
        pub phase: Phase,
        pub sources: Sources,
        pub champ_select: Option<ChampSelectAdvice>,
        pub game: Option<GameAdvice>,
        pub recap: Option<Recap>,
        pub settings: Settings,
    }

    /// One Rift lane pairing; item gold of visible inventories, ally − enemy.
    pub struct LaneDiff {
        /// None when positions are unknown and players were paired in scoreboard order.
        pub lane: Option<Lane>,
        pub ally: ChampKey,
        pub enemy: ChampKey,
        pub gold_diff: i32,
        pub level_diff: i32,
        pub mine: bool,
    }

    pub struct PlayerGold {
        pub champ: ChampKey,
        pub ally: bool,
        pub item_gold: u32,
        pub level: u32,
        pub mine: bool,
    }

    pub struct GoldSample {
        /// Game time, seconds.
        pub time: f32,
        /// Team item-gold diff, mine − enemy.
        pub team: i32,
        /// My lane item-gold diff, when I have a lane opponent.
        pub lane: Option<i32>,
    }

    /// Item gold (cost of held items, as the scoreboard shows them). The game
    /// exposes no enemy gold, so this is never an estimate of it.
    pub struct GoldDiff {
        /// (my team, enemy team).
        pub team_item_gold: (u32, u32),
        /// Mine − enemy.
        pub diff: i32,
        /// Team kills (my team, enemy team).
        pub kills: (u32, u32),
        /// Average level, mine − enemy.
        pub level_diff: f32,
        /// Rift only: top → support, unknown lanes last.
        pub lanes: Vec<LaneDiff>,
        /// My team first, most item gold first within a team.
        pub players: Vec<PlayerGold>,
        /// Sampled over the game, oldest first; the last point is now.
        pub history: Vec<GoldSample>,
    }

    // ---- draft (champ select) ----

    /// lolalytics `build-team` row: one teammate of `TeamSynergy::champ`.
    pub struct Teammate {
        pub champ: ChampKey,
        pub lane: Lane,
        /// Win rate together, percent.
        pub win_rate: f32,
        /// Win rate minus the expectation from both champions' own win rates, points.
        pub delta: f32,
        pub games: u32,
    }

    pub struct TeamSynergy {
        pub champ: ChampKey,
        pub lane: Lane,
        pub patch: String,
        /// The champion's games in `lane`.
        pub games: u32,
        pub mates: Vec<Teammate>,
    }

    /// One lane: your champion vs the enemy most likely there.
    pub struct DraftLane {
        pub lane: Lane,
        pub ally: Option<ChampKey>,
        pub enemy: Option<ChampKey>,
        /// Ally win rate in this matchup, percent.
        pub ally_wr: Option<f32>,
        pub games: u32,
    }

    pub struct DraftDuo {
        pub a: ChampKey,
        pub b: ChampKey,
        /// Synergy (win rate vs expectation), points.
        pub delta: f32,
        pub games: u32,
    }

    /// One term of a side's draft estimate.
    pub struct DraftFactor {
        pub label: String,
        /// Win-rate points for this side vs an even draft.
        pub points: f32,
        pub detail: String,
    }

    pub struct DraftSide {
        pub champs: Vec<ChampKey>,
        /// Damage shares (sum to 1 once anyone is revealed).
        pub physical: f32,
        pub magic: f32,
        pub true_dmg: f32,
        pub frontline: u32,
        pub engage: u32,
        /// Summed CC trait (≈ champions' worth of reliable CC).
        pub cc: f32,
        pub duos: Vec<DraftDuo>,
        /// Short gap notes, e.g. "No frontline", "Mostly physical (88%)".
        pub gaps: Vec<String>,
        pub factors: Vec<DraftFactor>,
        /// Sum of `factors`.
        pub total: f32,
    }

    pub struct DraftReport {
        pub allies: DraftSide,
        pub enemies: DraftSide,
        pub lanes: Vec<DraftLane>,
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings { tier: "emerald_plus".into(), region: "all".into(), ocr_enabled: true, record_fixtures: false }
    }
}

impl StaticData {
    pub fn champ_name(&self, key: ChampKey) -> &str {
        self.champs.get(&key).map_or("?", |c| c.name.as_str())
    }

    pub fn item_name(&self, id: ItemId) -> &str {
        self.items.get(&id).map_or("?", |i| i.name.as_str())
    }

    /// Lookup by display name or Data Dragon id, case/punctuation-insensitive.
    pub fn champ_by_name(&self, name: &str) -> Option<&Champion> {
        let norm = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        let n = norm(name);
        self.champs.values().find(|c| norm(&c.name) == n || norm(&c.id) == n || c.slug == n)
    }
}

impl Profile {
    pub fn dot(&self, o: &Profile) -> f32 {
        self.armor * o.armor
            + self.mr * o.mr
            + self.hp * o.hp
            + self.anti_heal * o.anti_heal
            + self.tenacity * o.tenacity
            + self.cleanse * o.cleanse
            + self.anti_burst * o.anti_burst
            + self.armor_pen * o.armor_pen
            + self.magic_pen * o.magic_pen
            + self.anti_crit * o.anti_crit
            + self.anti_attack_speed * o.anti_attack_speed
    }
}
