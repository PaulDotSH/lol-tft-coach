//! Static and aggregate game data: Data Dragon items, champions and runes, lolalytics and
//! u.gg stats, hand-written lane notes. Produced by `data`.

pub use model_core::*;
use std::collections::HashMap;

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
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
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
        /// Coachless patch ("16.20") when the item slots, boots, runes and summoners came
        /// from Coachless (Rift, signed in); None = lolalytics.
        #[serde(default)]
        pub coachless: Option<String>,
        /// Summoner spell ids to take (Coachless); empty without Coachless.
        #[serde(default)]
        pub summoners: Vec<u32>,
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

    /// Hand-written bot lane note (`crates/data/lane_notes.toml`).
    pub struct LaneNote {
        #[serde(default)]
        pub level2: bool,
        #[serde(default)]
        pub level6: bool,
        #[serde(default)]
        pub engage: bool,
        #[serde(default)]
        pub poke: bool,
        #[serde(default)]
        pub sustain: bool,
        pub note: String,
    }

    /// lolalytics matchup page: me against one champion, lane against lane.
    pub struct MatchupBuild {
        pub vs: ChampKey,
        pub games: u32,
        /// My win rate in this matchup, percent.
        pub win_rate: f32,
        /// Most played starting items (two potions are two ids).
        pub start: Vec<ItemId>,
        pub start_games: u32,
        pub start_wr: f32,
        /// Most played first, second and third completed items.
        pub core: Vec<ItemId>,
    }

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

    /// Data Dragon `champion/<Id>.json` → `spells[3]`.
    pub struct UltCooldowns {
        pub max_rank: u32,
        /// Base cooldown per rank, seconds.
        pub cooldown: Vec<f32>,
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
        // The champions' side compared char by char: no two Strings per champion per lookup.
        let same = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).eq(n.chars());
        self.champs.values().find(|c| same(&c.name) || same(&c.id) || c.slug == n)
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
