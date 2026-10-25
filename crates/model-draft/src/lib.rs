//! Champ select advice (`coach` output, rendered by the UI): picks, bans, build, runes, bench,
//! draft breakdown, matchup memory and the lane matchup card.

pub use model_core::*;
use model_static::{ChampNote, MatchupBuild};

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
        /// My history against the enemies revealed so far.
        #[serde(default)]
        pub memory: Vec<MatchupMemory>,
        /// Rift: my lane opponent(s) once my lane and champion are known.
        #[serde(default)]
        pub matchup: Option<MatchupPlan>,
    }

    /// Champ select: my lane opponent(s) with tips; in bot lane both of theirs and my partner.
    pub struct MatchupPlan {
        pub lane: Lane,
        /// Me, then my bot lane partner when known.
        pub allies: Vec<ChampKey>,
        /// Bot lane: synergy with my partner.
        pub partner: Option<PartnerSynergy>,
        /// Their laner(s) as revealed, bot laner before support; empty: none yet.
        pub cards: Vec<MatchupCard>,
        /// lolalytics matchup page against my direct opponent; None until fetched or from few games.
        pub build: Option<MatchupBuild>,
        /// `build.runes` with names and icons.
        pub runes: Vec<RunePick>,
    }

    pub struct PartnerSynergy {
        pub champ: ChampKey,
        /// Win rate together minus expectation, points.
        pub delta: f32,
        pub games: u32,
    }

    /// Tips against one lane opponent (champ select and the in-game laning panel).
    pub struct MatchupCard {
        pub champ: ChampKey,
        /// My win rate against them, percent (lolalytics), and its games; None until fetched.
        pub vs_wr: Option<f32>,
        pub games: u32,
        /// "strong level 2", "all-in at 6", "hard engage", "poke", "sustain".
        pub tags: Vec<String>,
        /// Hand-written notes on this champion; None when missing from the notes.
        pub note: Option<ChampNote>,
        /// Hand-written note for exactly my champion against them.
        pub pair: Option<String>,
        /// Their Q W E R with base cooldowns; empty until Data Dragon answers.
        pub spells: Vec<SpellCd>,
        /// My past games against them.
        pub record: Option<MatchupMemory>,
    }

    pub struct SpellCd {
        /// "Q", "W", "E" or "R".
        pub key: String,
        pub name: String,
        /// Image file under `cdn/<version>/img/spell/`.
        pub icon: String,
        /// Base cooldown per rank, seconds (before ability haste).
        pub cooldown: Vec<f32>,
        /// Named in the notes as a spell to play around.
        pub watch: bool,
        /// In game, R only: base cooldown at the rank their level allows.
        pub now: Option<f32>,
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
        /// Of those, before 15:00 (games with a timeline only).
        #[serde(default)]
        pub early_kills: u32,
        /// Games and wins on the champion I play now.
        #[serde(default)]
        pub same_games: u32,
        #[serde(default)]
        pub same_wins: u32,
        /// Newest first, at most three: true = win.
        #[serde(default)]
        pub last: Vec<bool>,
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
