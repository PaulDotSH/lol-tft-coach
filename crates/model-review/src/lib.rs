//! After the game and across games (`coach` output, rendered by the UI): recap, death review,
//! goals and trends, loss patterns, ranked LP, and the analysis of one past game.

pub use model_core::*;
use model_game::{EogStats, MatchPlayer, PastGame, Presence, TimelineMark};
use model_static::Benchmarks;

data! {
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
        /// (game time, `WinProb::base`) about every 20 s while the win chance was shown.
        #[serde(default)]
        pub win_curve: Vec<(f32, f32)>,
    }

    /// LP over my ranked games in one queue, from snapshots taken after each.
    /// The client keeps no LP history: games before the app started tracking are missing.
    pub struct RankTrend {
        pub queue: String,
        pub tier: String,
        pub division: String,
        pub lp: i32,
        /// Oldest first, at most 20.
        pub games: Vec<RankPoint>,
        /// Wins (positive) or losses (negative) in a row, newest games.
        pub streak: i32,
    }

    pub struct RankPoint {
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
        pub time: u64,
        /// Place on the whole ladder: 400 per tier, 100 per division, plus LP.
        pub ladder: i32,
        /// LP won or lost since the snapshot before.
        pub delta: i32,
        pub win: bool,
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
        /// Map position as seen from blue side (match history only).
        #[serde(default)]
        pub pos: Option<(i32, i32)>,
        #[serde(default)]
        pub sight: Option<DeathSight>,
    }

    pub struct DeathSight {
        pub covered: f32,
        pub seen: Vec<SeenBefore>,
        pub warned: Option<(f32, String)>,
        #[serde(default)]
        pub verdict: Option<SightKind>,
        #[serde(default)]
        pub line: Option<String>,
    }

    pub struct SeenBefore {
        pub champ: Option<ChampKey>,
        pub first: f32,
        pub last: f32,
        pub x: i32,
        pub y: i32,
    }

    pub struct DeathReview {
        pub deaths: Vec<DeathNote>,
        /// Patterns within the game, e.g. "Died 3× within 60 s of Malphite reaching 6/11/16".
        pub notes: Vec<String>,
    }

    /// One game's self-coaching numbers, saved with its recap (trend input).
    pub struct GameSummary {
        /// Unix seconds.
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
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
        /// My latest run of games.
        #[serde(default)]
        pub session: Option<Session>,
    }

    /// My latest games played with gaps under two hours (remakes left out).
    pub struct Session {
        /// Unix ms: the first game's start and the last game's end.
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
        pub start: u64,
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
        pub end: u64,
        /// Newest first.
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number[]"))]
        pub game_ids: Vec<u64>,
        pub wins: u32,
        pub losses: u32,
        /// Mistakes in at least two games and half of them, most frequent first.
        pub repeats: Vec<SessionRepeat>,
        /// My goals over these games only.
        pub goals: Vec<GoalStatus>,
    }

    pub struct SessionRepeat {
        pub text: String,
        /// Games it happened in, of those where it could be measured.
        pub games: u32,
        pub of: u32,
    }

    /// Me and my lane opponent at one minute mark (timeline frames).
    pub struct PhaseRow {
        pub minute: u32,
        pub cs: u32,
        pub level: u32,
        pub opp_cs: Option<u32>,
        pub opp_level: Option<u32>,
        /// My total gold minus my opponent's.
        pub gold_lead: Option<i32>,
    }

    /// My lane up to 14:00: my duo against theirs in bot lane, else me against my opponent.
    pub struct Laning {
        pub allies: Vec<ChampKey>,
        pub enemies: Vec<ChampKey>,
        /// "Won lane: +1,150 gold at 14:00, ahead from 6:00".
        pub verdict: String,
        /// Minute from which the lane stayed ahead (or behind) to the end.
        pub turned_at: Option<u32>,
        /// Minutes 1–14 (fewer in a short game).
        pub minutes: Vec<LaneMinute>,
        /// Kills on the lane's players before 14:00.
        pub events: Vec<LaneEvent>,
        /// Each player's first back (a shop visit that wasn't a death), estimated from
        /// one-minute frames.
        pub backs: Vec<LaneBack>,
    }

    pub struct LaneMinute {
        pub minute: u32,
        /// Ours minus theirs.
        pub gold_diff: i32,
        pub xp_diff: i32,
        pub cs_ours: u32,
        pub cs_theirs: u32,
        pub my_level: u32,
        /// My direct counterpart (support vs support).
        pub opp_level: Option<u32>,
    }

    pub struct LaneEvent {
        pub time: f32,
        pub killer: Option<ChampKey>,
        pub victim: ChampKey,
        pub assists: Vec<ChampKey>,
        /// My side got the kill.
        pub ours: bool,
        pub place: Option<String>,
    }

    pub struct LaneBack {
        pub champ: ChampKey,
        pub ally: bool,
        pub minute: u32,
        pub spent: u32,
    }

    /// This game against my recent games on the same champion and mode.
    pub struct PastCompare {
        pub games: u32,
        pub wins: u32,
        pub rows: Vec<CompareRow>,
    }

    pub struct CompareRow {
        pub label: String,
        pub this: f32,
        pub avg: f32,
        /// Best of the earlier games (lowest when lower is better).
        pub best: f32,
        pub lower_better: bool,
        pub digits: u32,
    }

    /// The one thing to work on next game, as a goal when it maps to one.
    pub struct Focus {
        pub text: String,
        pub why: String,
        pub goal: Option<Goal>,
    }

    /// A chain of kills at most 10 s apart.
    pub struct Fight {
        pub start: f32,
        pub end: f32,
        /// Enemies / allies who died.
        pub won: u32,
        pub lost: u32,
        pub place: Option<String>,
        pub presence: Presence,
        /// Mine was my team's first death in it.
        pub died_first: bool,
        /// An epic monster taken within a minute after it.
        pub then: Option<TimelineMark>,
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
        /// My team's chance to win (0..1), deaths aside: the live number when the app watched
        /// the game, else the model on this frame. None without a fit for the mode.
        #[serde(default)]
        pub win: Option<f32>,
    }

    /// One of the largest changes of the win chance between two points of the curve.
    pub struct Swing {
        /// Game time (s) of what caused it, else the end of the span.
        pub time: f32,
        /// Points, + for my team.
        pub delta: f32,
        /// "fight", "death", "objective", "structure" or "other".
        pub kind: String,
        /// "Fight lost 1 for 3 in the river, Baron given".
        pub text: String,
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
        /// Fights with at least two deaths, or any I was in.
        pub fights: Vec<Fight>,
        /// What the coach recommended live, else what it would build into this comp.
        pub recommended: Vec<ItemId>,
        pub takeaways: Vec<Takeaway>,
        /// The app watched this game live: deaths carry held gold and Buy next.
        pub live: bool,
        /// CS, levels and gold vs my opponent at 5, 10, 15 and 20 minutes.
        pub phases: Vec<PhaseRow>,
        /// Minute marks (game minutes) where I held 1500+ gold, the closing push aside (Rift).
        pub rich_marks: Vec<u32>,
        pub vs_past: Option<PastCompare>,
        pub focus: Option<Focus>,
        /// Rift: my lane (bot lane: both duos) up to 14:00.
        #[serde(default)]
        pub laning: Option<Laning>,
        /// The coach's note, when the setting is on and the review was built after the game.
        #[serde(default)]
        pub note: Option<GameNote>,
        /// Rift, when I jungled: replaces `laning`.
        #[serde(default)]
        pub jungle: Option<JungleReview>,
        /// The five largest changes of `gold[i].win`, largest first; empty without a curve.
        #[serde(default)]
        pub swings: Vec<Swing>,
    }

    pub struct JungleReview {
        /// Their jungler.
        pub enemy: Option<ChampKey>,
        /// At 3:00 and 4:00.
        pub first_clear: Vec<ClearMark>,
        pub clear: Vec<ClearRate>,
        /// My champion's CS per minute (minions and monsters).
        pub benchmark: Option<f32>,
        /// Lanes where a gank got a kill before 14:00.
        pub ganks: Vec<GankLane>,
        pub objectives: Vec<JungleObjective>,
        /// Minutes I took camps in their jungle.
        pub invades: Vec<u32>,
        /// Against their jungler at 5, 10 and 15 minutes; `cs` is jungle CS.
        pub versus: Vec<PhaseRow>,
    }

    pub struct ClearMark {
        pub minute: u32,
        pub level: u32,
        pub jungle_cs: u32,
    }

    pub struct ClearRate {
        pub from: u32,
        pub to: u32,
        pub jungle_per_min: f32,
        /// Minions and monsters, to compare with the benchmark.
        pub cs_per_min: f32,
    }

    pub struct GankLane {
        pub lane: Lane,
        pub ganks: u32,
        pub kills: u32,
        pub deaths: u32,
    }

    pub struct JungleObjective {
        pub time: f32,
        pub kind: String,
        pub detail: Option<String>,
        pub ours: bool,
        pub presence: Option<Presence>,
        pub enemy_jungler_dead: bool,
    }

    /// One numbered line of the fact sheet a game's note is built from.
    pub struct Fact {
        pub id: u32,
        pub kind: FactKind,
        pub good: Option<bool>,
        pub text: String,
    }

    pub struct NoteLine {
        pub text: String,
        /// Ids of the facts it is built from.
        pub facts: Vec<u32>,
        pub good: Option<bool>,
    }

    /// A few connected takeaways and one focus, composed by rules from the fact sheet only.
    pub struct GameNote {
        pub takeaways: Vec<NoteLine>,
        pub focus: Option<NoteLine>,
        /// The facts its lines cite.
        pub facts: Vec<Fact>,
        pub rules_v: u32,
    }
}

enums! {
    /// Self-coaching goal / trend metric.
    pub enum GoalKind { Cs10, EarlyDeaths, Vision, Unspent, BuyNext }

    pub enum SightKind { Warned, Visible, Lane, Fog, Unclear }

    pub enum FactKind { Result, Lane, Takeaway, Focus, Deaths, Fights, Objectives, Economy, Vision, Items, Habit }
}
