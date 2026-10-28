//! Champ select advice (`coach` output, rendered by the UI): picks, bans, build, runes, bench,
//! draft breakdown, matchup memory and the bot lane plan.

pub use model_core::*;
use model_static::MatchupBuild;

data! {
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
        /// Where the item stats came from: "Coachless 16.20", "lolalytics 16.21".
        #[serde(default)]
        pub source: String,
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
        /// Summoner spells to take (Coachless only), e.g. ["Flash", "Ignite"].
        #[serde(default)]
        pub summoners: Vec<String>,
        /// Where the page came from: "Coachless 16.20", "lolalytics 16.21".
        #[serde(default)]
        pub source: String,
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
        /// My history against the enemies revealed so far.
        #[serde(default)]
        pub memory: Vec<MatchupMemory>,
        /// Rift, me support or ADC: the bot lane 2v2.
        #[serde(default)]
        pub bot_lane: Option<BotLanePlan>,
    }

    /// Champ select plan for the bot lane 2v2 when I play support or ADC.
    pub struct BotLanePlan {
        /// My duo, me first.
        pub allies: Vec<ChampKey>,
        /// Their bot laner then their support, as revealed (empty: none yet).
        pub enemies: Vec<ChampKey>,
        pub numbers: Vec<LaneNumber>,
        /// From the hand-written lane notes, one per enemy that has one.
        pub windows: Vec<LaneWindow>,
        /// My past games against their bot laners.
        pub memory: Vec<MatchupMemory>,
        /// Starting and first items against their bot laner (lolalytics matchup page).
        pub start: Option<MatchupBuild>,
    }

    pub struct LaneNumber {
        pub champ: ChampKey,
        /// true: synergy with my lane partner (win-rate points); false: my win rate
        /// against them (percent).
        pub with: bool,
        pub value: f32,
        pub games: u32,
    }

    pub struct LaneWindow {
        pub champ: ChampKey,
        /// "strong level 2", "all-in at 6", "hard engage", "poke", "sustain".
        pub tags: Vec<String>,
        pub note: String,
    }

    /// My past games against one enemy champion (champ select).
    pub struct MatchupMemory {
        pub champ: ChampKey,
        pub games: u32,
        pub wins: u32,
        /// Games they were my lane opponent, and my average gold lead at 15:00 in them.
        pub lane_games: u32,
        pub gold_15: Option<i32>,
        /// Times they killed me, over `games`.
        pub killed_me: u32,
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
}
