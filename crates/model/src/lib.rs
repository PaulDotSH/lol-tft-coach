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
/// Riot perk id: runes 8xxx/9xxx, stat shards 5xxx.
pub type RuneId = u32;

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
        /// Protecting carries: shields, heals on allies, disengage, body-blocking.
        pub peel: f32,
    }

    /// Per-champion averages from lolalytics for the chosen rank (per game).
    pub struct Benchmarks {
        pub cs_per_min: Option<f32>,
        pub gold_per_min: Option<f32>,
        pub damage_per_min: Option<f32>,
        pub vision_per_min: Option<f32>,
    }

    /// Data Dragon champion `stats`: level-1 values and per-level growth.
    pub struct BaseStats {
        pub hp: f32,
        pub hp_per_level: f32,
        pub armor: f32,
        pub armor_per_level: f32,
        pub mr: f32,
        pub mr_per_level: f32,
        pub attack_range: f32,
        pub attack_speed: f32,
    }

    /// Live API `activePlayer.championStats`. Percent stats are fractions;
    /// `*_pen_pct` is the share of resist ignored (0 = none).
    pub struct ChampionStats {
        pub ability_haste: f32,
        pub ability_power: f32,
        pub attack_damage: f32,
        pub attack_range: f32,
        pub attack_speed: f32,
        pub armor: f32,
        pub magic_resist: f32,
        pub max_health: f32,
        pub crit_chance: f32,
        pub heal_shield_power: f32,
        pub life_steal: f32,
        pub omnivamp: f32,
        pub move_speed: f32,
        pub tenacity: f32,
        pub lethality: f32,
        pub armor_pen_pct: f32,
        pub magic_pen_flat: f32,
        pub magic_pen_pct: f32,
        pub resource_max: f32,
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
        pub base: BaseStats,
    }

    pub struct StaticData {
        /// Data Dragon version, e.g. "16.21.1".
        pub version: String,
        pub items: HashMap<ItemId, Item>,
        pub champs: HashMap<ChampKey, Champion>,
        /// Item → one-per-inventory groups it belongs to (Last Whisper, Lifeline, …);
        /// two items sharing a group can't be owned together. Empty if unavailable.
        #[serde(default)]
        pub item_groups: HashMap<ItemId, Vec<u32>>,
        /// Rune and stat shard id → name and icon. Empty if unavailable.
        #[serde(default)]
        pub runes: HashMap<RuneId, RuneInfo>,
    }

    pub struct RuneInfo {
        pub name: String,
        /// Data Dragon image path under `cdn/img/`, e.g. "perk-images/Styles/…png".
        pub icon: String,
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
        /// Build page rune pages and per-rune stats; None when the page had none.
        #[serde(default)]
        pub runes: Option<RuneData>,
    }

    /// One full rune page from lolalytics.
    pub struct RunePage {
        /// Keystone first, then the three primary-tree runes.
        pub primary: Vec<RuneId>,
        pub secondary: Vec<RuneId>,
        pub shards: Vec<RuneId>,
        pub win_rate: f32,
        pub games: u32,
    }

    pub struct RuneStat {
        pub pick_rate: f32,
        pub win_rate: f32,
        pub games: u32,
    }

    pub struct RuneData {
        /// Most played page.
        pub popular: RunePage,
        /// Highest win rate page.
        pub best: RunePage,
        /// Per rune: as a primary-tree pick first, then as a secondary (shards have one entry).
        pub stats: HashMap<RuneId, Vec<RuneStat>>,
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
        /// My live stats (items, levels, augments applied); None if the API omits them.
        pub my_stats: Option<ChampionStats>,
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
        /// Completed legendaries held.
        pub legendaries: u32,
        /// Legendaries vs the lobby average, rounded.
        pub legendary_diff: i32,
        /// Legendaries completed in the last few minutes of game time, newest last.
        pub new_items: Vec<ItemId>,
        /// R rank their visible level allows and its static base cooldown; None before R.
        pub ult: Option<UltInfo>,
        /// Mayhem: augments read off the Tab scoreboard, ranked in this champion's u.gg list.
        pub augments: Vec<AugmentRank>,
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
        /// Tie-break reason when it matches what I built, e.g. "fits your Nashor's Tooth (on-hit)".
        pub synergy: Option<String>,
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
        /// Supports, until upgraded: the World Atlas upgrade to take when the quest completes.
        #[serde(default)]
        pub support_upgrade: Option<ItemPick>,
        /// Remaining late-game path (excludes owned items).
        pub path: Vec<ItemPick>,
        pub owned: Vec<ItemId>,
        pub threats: Vec<Threat>,
        pub matchups: Vec<MatchupTip>,
        pub me: SelfStats,
        pub timers: Vec<ObjectiveTimer>,
        pub augments: Option<AugmentAdvice>,
        pub gold: GoldDiff,
        /// What my gold buys right now (the card to look at while dead).
        pub shopping: ShoppingList,
        /// When Buy next is affordable (income from tracked gold); None with nothing left to buy.
        pub gold_eta: Option<GoldEta>,
        pub deaths: DeathWindow,
        /// Mayhem: augments I took this game (detected, or candidates when unsure).
        pub my_augments: Vec<MyAugment>,
        pub defense: Option<Defense>,
        /// Full build: what else gold can buy (anvils, elixirs, swaps).
        pub gold_sinks: Vec<GoldSink>,
        /// Mayhem: ask the user to hold Tab so enemy augments can be read.
        pub tab_hint: Option<TabHint>,
    }

    pub struct PickSuggestion {
        pub champ: ChampKey,
        pub score: f32,
        pub reason: String,
    }

    pub struct BuildPlan {
        pub start: Vec<ItemId>,
        /// Supports: the World Atlas upgrade to take when its quest completes.
        #[serde(default)]
        pub upgrade: Option<ItemPick>,
        pub core: Vec<ItemPick>,
        pub boots: Option<ItemPick>,
    }

    pub struct RunePick {
        pub id: RuneId,
        pub name: String,
        /// Data Dragon image path under `cdn/img/`.
        pub icon: String,
        /// Why it replaces the most played rune in its row; None when it is the most played.
        pub reason: Option<String>,
    }

    pub struct RuneAdvice {
        /// Keystone first.
        pub primary: Vec<RunePick>,
        pub secondary: Vec<RunePick>,
        pub shards: Vec<RunePick>,
        /// Of the most played page this one starts from.
        pub win_rate: f32,
        pub games: u32,
        /// Highest win rate page when its keystone differs: keystone, win rate, games.
        pub alt: Option<RuneAlt>,
    }

    pub struct RuneAlt {
        pub keystone: RunePick,
        pub win_rate: f32,
        pub games: u32,
    }

    pub struct BenchRank {
        pub champ: ChampKey,
        pub tier: Option<String>,
        pub mine: bool,
        /// ARAM win rate (lolalytics), percent.
        pub win_rate: Option<f32>,
        /// Tier / win rate + team fit, win-rate points; None without tier or win rate.
        pub score: Option<f32>,
        /// Why, strongest first: "Mayhem tier S", "adds frontline: team has none".
        pub reasons: Vec<String>,
    }

    pub struct ChampSelectAdvice {
        pub mode: Mode,
        pub my_lane: Option<Lane>,
        pub my_champ: Option<ChampKey>,
        pub picks: Vec<PickSuggestion>,
        pub bans: Vec<PickSuggestion>,
        pub build: Option<BuildPlan>,
        /// Rift / ARAM rune page for my champion (Mayhem has no runes).
        pub runes: Option<RuneAdvice>,
        /// ARAM / Mayhem: you + bench, best first.
        pub bench: Vec<BenchRank>,
        pub enemies: Vec<ChampKey>,
        /// Rift draft breakdown for both teams.
        pub draft: Option<DraftReport>,
        /// ARAM / Mayhem: my team's composition (enemies are hidden); Rift has it in `draft`.
        pub team: Option<DraftSide>,
    }

    pub struct Recap {
        pub stats: EogStats,
        pub benchmarks: Benchmarks,
        /// Items recommended over the game, in order first suggested.
        pub recommended: Vec<ItemId>,
        pub notes: Vec<String>,
        /// Self-coaching (absent in recaps saved before it existed).
        pub review: Option<DeathReview>,
        pub summary: Option<GameSummary>,
        #[serde(default)]
        pub goals: Vec<GoalProgress>,
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
        /// Up to two self-coaching goals.
        #[serde(default)]
        pub goals: Vec<Goal>,
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
        /// Loss patterns from recent games (idle dashboard).
        pub macro_report: Option<MacroReport>,
        /// Personal trends and goal status across games (idle dashboard).
        pub trends: Option<Trends>,
        /// Live progress on my goals this game (self-coach row).
        pub goal_progress: Vec<GoalProgress>,
        /// Recent games, newest first (idle dashboard; click one to analyse it).
        pub history: Vec<PastGame>,
    }

    // ---- macro coach (own match history) ----

    pub struct TeamObjectives {
        pub dragons: u32,
        pub barons: u32,
        pub heralds: u32,
        /// Void grubs (`hordeKills`).
        pub grubs: u32,
        pub towers: u32,
    }

    /// One of my past games (LCU match history). Immutable, cached on disk.
    pub struct PastGame {
        #[ts(type = "number")]
        pub game_id: u64,
        /// Unix ms.
        #[ts(type = "number")]
        pub created: u64,
        #[ts(type = "number")]
        pub queue_id: i64,
        pub mode: Mode,
        pub champ: ChampKey,
        pub lane: Option<Lane>,
        pub win: bool,
        /// Early surrender / under 5 minutes: ignored by the coach.
        pub remake: bool,
        pub duration_s: u32,
        pub kills: u32,
        pub deaths: u32,
        pub assists: u32,
        pub cs: u32,
        pub gold: u32,
        pub damage_to_champs: u32,
        pub vision: u32,
        /// Game times (s) of my deaths; None without a timeline.
        pub death_times: Option<Vec<u32>>,
        /// Me minus my lane opponent (timeline + a single opponent in my lane).
        pub gold_diff_10: Option<i32>,
        pub gold_diff_15: Option<i32>,
        pub xp_diff_15: Option<i32>,
        /// Objectives of my team and the enemy's (game details).
        pub ours: Option<TeamObjectives>,
        pub theirs: Option<TeamObjectives>,
        /// Some(true) = my team got it first; None = nobody / unknown.
        pub first_dragon: Option<bool>,
        pub first_tower: Option<bool>,
        pub first_blood: Option<bool>,
        /// My CS at 10:00 (timeline).
        pub cs_10: Option<u32>,
        /// Mayhem augments I took.
        pub augments: Vec<AugmentId>,
    }

    pub struct ModeRecord {
        pub mode: Mode,
        pub games: u32,
        pub wins: u32,
    }

    pub struct ChampRecord {
        pub mode: Mode,
        pub champ: ChampKey,
        pub games: u32,
        pub wins: u32,
    }

    /// A fact that separates my losses from my wins, with the numbers behind it.
    pub struct Pattern {
        pub mode: Mode,
        /// Stable id: "early_deaths", "deaths", "cs", "gold_15", "first_dragon", "champion", "bench_cs", …
        pub kind: String,
        pub champ: Option<ChampKey>,
        pub title: String,
        /// One line, e.g. "In 7 of 9 losses you died 2+ times before 14:00 (2 of 8 wins)".
        pub evidence: String,
        /// Short note phrased as information, not an order.
        pub focus: String,
        /// The compared values, e.g. [("Losses", 78), ("Wins", 25)].
        pub bars: Vec<(String, f32)>,
        /// Unit of `bars`: "%", "/min", "/10 min", "g".
        pub unit: String,
        /// Games behind the comparison.
        pub games: u32,
        /// Impact × confidence, 0..=1; patterns are sorted by it.
        pub score: f32,
    }

    pub struct MacroReport {
        /// Games analysed (remakes excluded).
        pub games: u32,
        /// Of those, games with a timeline (death times, gold at 15).
        pub with_timeline: u32,
        pub modes: Vec<ModeRecord>,
        /// Most played first.
        pub champs: Vec<ChampRecord>,
        /// Strongest first, at most a handful.
        pub patterns: Vec<Pattern>,
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
        /// Summed peel trait.
        pub peel: f32,
    }

    pub struct DraftReport {
        pub allies: DraftSide,
        pub enemies: DraftSide,
        pub lanes: Vec<DraftLane>,
    }

    // ---- in-game extras: shopping, gold ETA, spikes, death windows ----

    /// Data Dragon `champion/<Id>.json` → `spells[3]`.
    pub struct UltCooldowns {
        pub max_rank: u32,
        /// Base cooldown per rank, seconds.
        pub cooldown: Vec<f32>,
    }

    pub struct UltInfo {
        /// Highest R rank the level allows (6/11/16; 1/6/11/16 for four-rank ults).
        pub rank: u32,
        /// Base cooldown per rank, seconds (static data, before haste); empty until fetched.
        pub cooldowns: Vec<f32>,
        /// Level is exactly a rank threshold: R just came online / ranked up.
        pub spike: bool,
    }

    #[derive(Copy, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum BuyKind {
        /// A finished path item.
        Full,
        /// A component toward `ShopBuy::toward`.
        Component,
        /// Control ward, elixir.
        Extra,
    }

    pub struct ShopBuy {
        pub item: ItemId,
        pub name: String,
        /// Gold paid now: cost minus components already held.
        pub price: u32,
        pub kind: BuyKind,
        pub toward: Option<ItemId>,
    }

    #[derive(Default)]
    pub struct ShoppingList {
        pub gold: u32,
        /// In purchase order.
        pub buys: Vec<ShopBuy>,
        pub leftover: u32,
        pub dead: bool,
        /// Seconds until respawn (0 when alive).
        pub respawn_in: f32,
    }

    /// A gold amount at which the shopping list grows.
    pub struct BackPlan {
        pub gold: u32,
        /// Seconds until then at the estimated income.
        pub in_s: Option<f32>,
        /// Path items and components bought at that gold (extras left out).
        pub items: Vec<ItemId>,
    }

    pub struct GoldEta {
        /// Buy next.
        pub item: Option<ItemId>,
        /// Estimated income, gold per minute (spending ignored); None until enough samples.
        pub per_min: Option<f32>,
        /// Gold still missing for `item`, held components counted (0 = affordable now).
        pub missing: u32,
        /// Seconds until `item` is affordable at `per_min`.
        pub eta_s: Option<f32>,
        /// Next gold amounts that buy more, before `item` itself.
        pub backs: Vec<BackPlan>,
    }

    pub struct Respawn {
        pub champ: ChampKey,
        /// Seconds until back (the scoreboard death timer).
        pub in_s: f32,
    }

    /// Who is dead right now, soonest back first. Allies exclude me.
    pub struct DeathWindow {
        pub enemies: Vec<Respawn>,
        pub allies: Vec<Respawn>,
    }

    // ---- self-coaching: death review, goals, trends ----

    /// A goal picked in settings. Targets: CS/min at 10:00 ≥, deaths before 14:00 ≤,
    /// vision/min ≥, unspent gold at each death <, Buy next followed ≥ (%).
    pub struct Goal {
        pub kind: GoalKind,
        pub target: f32,
    }

    pub struct GoalProgress {
        pub kind: GoalKind,
        pub target: f32,
        /// In the goal's unit; for `Unspent` the number of deaths at or above the target.
        pub value: Option<f32>,
        /// None while it can still go either way.
        pub met: Option<bool>,
    }

    pub struct DeathNote {
        /// Game time (s).
        pub time: f32,
        /// None for executions (turret, minions).
        pub killer: Option<ChampKey>,
        pub assisters: Vec<ChampKey>,
        /// My item gold minus the killer's.
        pub gold_gap: Option<i32>,
        /// The killer was first on the threat board.
        pub killer_top_threat: bool,
        /// Gold I was holding.
        pub unspent: u32,
        pub buy_next: Option<ItemId>,
        /// Buy next (or its component) cost no more than `unspent`.
        pub could_buy: bool,
        pub owned: Vec<ItemId>,
        /// Killer / assisters who reached level 6, 11 or 16 within 60 s before.
        pub spikes: Vec<(ChampKey, u32)>,
        /// My team got at least as many kills as it lost around this death
        /// (set by the review; absent in recaps saved before it existed).
        #[serde(default)]
        pub traded: bool,
        /// Where on the map, e.g. "enemy jungle" (match history only).
        #[serde(default)]
        pub place: Option<String>,
    }

    pub struct DeathReview {
        pub deaths: Vec<DeathNote>,
        /// Patterns within the game, e.g. "Died 3× within 60 s of Malphite reaching 6/11/16".
        pub notes: Vec<String>,
    }

    /// One game's self-coaching numbers, saved with its recap (trend input).
    pub struct GameSummary {
        /// Unix seconds.
        #[ts(type = "number")]
        pub ended: u64,
        pub mode: Mode,
        pub champ: ChampKey,
        pub win: bool,
        pub duration_s: u32,
        pub cs_10: Option<u32>,
        pub deaths_before_14: Option<u32>,
        pub vision: u32,
        /// Gold held at each death.
        pub unspent_at_death: Vec<u32>,
        /// Completed items bought, and how many were Buy next (or an alternative) at the time.
        pub buys: u32,
        pub followed: u32,
    }

    pub struct Trend {
        pub mode: Mode,
        pub kind: GoalKind,
        /// Per game, oldest → newest (at most 20).
        pub series: Vec<f32>,
        /// Mean of the last 5 games, and of the games before them.
        pub recent: f32,
        pub before: Option<f32>,
        /// Recent vs before; None when flat or unknown.
        pub better: Option<bool>,
    }

    /// "Followed Buy next" share in wins vs losses.
    pub struct FollowSplit {
        pub mode: Mode,
        pub wins: Option<f32>,
        pub losses: Option<f32>,
        pub games: u32,
    }

    /// Mayhem: how often I took an augment and my damage with it. Never win rates (Riot rule).
    pub struct AugmentUse {
        pub id: AugmentId,
        pub games: u32,
        pub damage_per_min: f32,
    }

    pub struct GoalStatus {
        pub goal: Goal,
        /// Recent games that met it, of those where it could be measured.
        pub met: u32,
        pub of: u32,
    }

    pub struct Trends {
        pub trends: Vec<Trend>,
        pub follow: Vec<FollowSplit>,
        pub augments: Vec<AugmentUse>,
        pub goals: Vec<GoalStatus>,
    }

    // ---- augments / resists / gold sinks ----

    /// One closed augment offer. `id` is set only when the pick was identified
    /// (stat change, granted item); otherwise `candidates` lists what is still possible.
    pub struct MyAugment {
        pub id: Option<AugmentId>,
        pub candidates: Vec<AugmentId>,
        /// Game time the offer closed.
        pub time: f32,
    }

    /// Estimated from visible level + items + static base stats.
    pub struct EnemyResist {
        pub champ: ChampKey,
        pub level: u32,
        pub armor: f32,
        pub mr: f32,
    }

    pub struct Defense {
        /// Effective HP vs physical / magic damage from my live stats.
        pub ehp_physical: Option<f32>,
        pub ehp_magic: Option<f32>,
        /// Threat-weighted share of enemy damage (physical = the rest).
        pub enemy_magic: f32,
        pub enemy_true: f32,
        pub enemies: Vec<EnemyResist>,
        /// e.g. "Their armor ~110: % armor pen beats lethality".
        pub pen_notes: Vec<String>,
    }

    pub struct GoldSink {
        /// "anvil" | "elixir" | "sell_boots" | "swap".
        pub kind: String,
        pub item: Option<ItemId>,
        /// Item to sell first (boots, the weakest slot).
        pub sell: Option<ItemId>,
        pub cost: u32,
        pub reason: String,
    }

    // ---- Tab scoreboard (Mayhem) ----

    /// One augment icon read off the scoreboard. Several augments share an
    /// icon, so `ids` holds every candidate, likeliest first.
    pub struct SeenAugment {
        pub ids: Vec<AugmentId>,
        /// From the icon tint.
        pub rarity: Option<Rarity>,
    }

    /// An enemy's augments as last read from the Tab scoreboard.
    pub struct EnemyAugments {
        pub champ: ChampKey,
        pub augments: Vec<SeenAugment>,
        /// Game time (s) and their level when read.
        pub time: f32,
        pub level: u32,
    }

    /// Silent prompt to hold Tab; gone once the scoreboard was read.
    pub struct TabHint {
        pub text: String,
        pub reason: String,
    }

    // ---- one past game, analysed (match history) ----

    /// A player's end-of-game line.
    pub struct MatchPlayer {
        pub champ: ChampKey,
        /// Riot ID game name; empty when hidden.
        pub name: String,
        pub ally: bool,
        pub me: bool,
        pub lane: Option<Lane>,
        pub level: u32,
        pub kills: u32,
        pub deaths: u32,
        pub assists: u32,
        pub cs: u32,
        pub gold: u32,
        pub damage: u32,
        pub taken: u32,
        pub vision: u32,
        pub items: Vec<ItemId>,
    }

    /// An objective or structure taken, for the timeline.
    pub struct TimelineMark {
        /// Game time (s).
        pub time: f32,
        pub ours: bool,
        /// "Dragon", "Baron", "Herald", "Grubs", "Atakhan", "Tower", "Inhibitor".
        pub kind: String,
        /// e.g. "Infernal", "bot outer".
        pub detail: Option<String>,
    }

    pub struct KillMark {
        pub time: f32,
        /// My team got the kill.
        pub ours: bool,
        /// I killed, assisted or died.
        pub mine: bool,
    }

    /// Gold leads at one timeline frame.
    pub struct GoldPoint {
        pub time: f32,
        /// My team's total gold minus theirs.
        pub team: i32,
        /// Mine minus my lane opponent's.
        pub lane: Option<i32>,
    }

    pub struct Takeaway {
        pub good: bool,
        pub text: String,
    }

    /// One game from my match history, reviewed.
    pub struct MatchAnalysis {
        pub game: PastGame,
        /// My team first, then theirs, each in lane order.
        pub players: Vec<MatchPlayer>,
        pub opponent: Option<ChampKey>,
        pub benchmarks: Option<Benchmarks>,
        pub gold: Vec<GoldPoint>,
        pub marks: Vec<TimelineMark>,
        pub kills: Vec<KillMark>,
        /// Shares of my team's totals (0–1).
        pub kill_participation: Option<f32>,
        pub damage_share: Option<f32>,
        pub gold_share: Option<f32>,
        pub taken_share: Option<f32>,
        pub review: DeathReview,
        /// What the coach recommended live, else what it would build into this comp.
        pub recommended: Vec<ItemId>,
        pub takeaways: Vec<Takeaway>,
        /// The app watched this game live: deaths carry held gold and Buy next.
        pub live: bool,
    }
}

/// Internal: a parsed match (details + timeline) for the analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchDetail {
    pub game: PastGame,
    /// Index of me in `players`.
    pub me: usize,
    /// Participant order (participantId - 1).
    pub players: Vec<MatchPlayer>,
    pub frames: Vec<Frame>,
    pub kills: Vec<KillEvent>,
    pub marks: Vec<TimelineMark>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frame {
    pub time: f32,
    /// Same order as `MatchDetail::players`.
    pub players: Vec<FrameStat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FrameStat {
    pub total_gold: u32,
    pub current_gold: u32,
    pub level: u32,
    pub xp: u32,
    pub cs: u32,
    pub jungle_cs: u32,
    pub pos: Option<(i32, i32)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct KillEvent {
    pub time: f32,
    /// Player indexes; None for executions.
    pub killer: Option<usize>,
    pub victim: usize,
    pub assisters: Vec<usize>,
    pub pos: Option<(i32, i32)>,
}

/// Summoner's Rift areas; blue (team 100) is bottom-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapZone { BlueBase, RedBase, Top, Mid, Bot, River, BlueJungle, RedJungle }

impl MapZone {
    pub fn at(x: i32, y: i32) -> MapZone {
        const END: i32 = 14800;
        let near = |a: i32, b: i32| ((a * a + b * b) as f32).sqrt() < 5000.0;
        if near(x, y) {
            MapZone::BlueBase
        } else if near(END - x, END - y) {
            MapZone::RedBase
        } else if x < 2200 || y > 12600 {
            MapZone::Top
        } else if y < 2200 || x > 12600 {
            MapZone::Bot
        } else if (x - y).abs() < 1300 {
            MapZone::Mid
        } else if (x + y - END).abs() < 1300 {
            MapZone::River
        } else if x + y < END {
            MapZone::BlueJungle
        } else {
            MapZone::RedJungle
        }
    }
}

enums! {
    /// Self-coaching goal / trend metric.
    pub enum GoalKind { Cs10, EarlyDeaths, Vision, Unspent, BuyNext }
}

impl Default for Settings {
    fn default() -> Self {
        Settings { tier: "emerald_plus".into(), region: "all".into(), ocr_enabled: true, record_fixtures: false, goals: vec![] }
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
