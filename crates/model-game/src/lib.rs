//! What the League client and the game report: live snapshots, champ select, end-of-game
//! stats, rank, and my match history (past games, parsed matches). Produced by `clients`.

pub use model_core::*;

data_default! {
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

    pub struct ChampSelect {
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
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

    /// My rank in one queue at one moment (LCU current-ranked-stats).
    pub struct RankSnapshot {
        /// Unix ms.
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
        pub time: u64,
        /// "solo" or "flex".
        pub queue: String,
        /// "GOLD", "MASTER", …; division "I"–"IV" ("NA" from Master up).
        pub tier: String,
        pub division: String,
        pub lp: i32,
        pub wins: u32,
        pub losses: u32,
    }

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
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
        pub game_id: u64,
        /// Unix ms.
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
        pub created: u64,
        #[cfg_attr(any(test, feature = "ts"), ts(type = "number"))]
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
        /// Rift: where I died, as seen from blue side (my base bottom-left).
        #[serde(default)]
        pub death_spots: Vec<DeathSpot>,
        /// Enemy champions (game details).
        #[serde(default)]
        pub enemies: Vec<ChampKey>,
        /// My lane opponent, when there was exactly one.
        #[serde(default)]
        pub opponent: Option<ChampKey>,
        /// Who killed me, per death (None: executed).
        #[serde(default)]
        pub killed_by: Vec<Option<ChampKey>>,
        /// Control Wards I bought (game details).
        #[serde(default)]
        pub control_wards: Option<u32>,
    }

    pub struct DeathSpot {
        /// Game time (s).
        pub time: u32,
        pub x: i32,
        pub y: i32,
    }

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
        #[serde(default)]
        pub wards_placed: u32,
        #[serde(default)]
        pub wards_killed: u32,
        /// Control Wards bought.
        #[serde(default)]
        pub control_wards: u32,
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
        pub pos: Option<(i32, i32)>,
        /// I killed or helped kill it (epic monsters).
        pub mine: bool,
        /// Where I was when it was taken (epic monsters; set by the review).
        pub presence: Option<Presence>,
    }
}

enums! {
    /// Where I was for a fight or objective: in it (or close), dead, or elsewhere.
    pub enum Presence { There, Dead, Away }
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

/// Summoner's Rift is symmetric around (MAP_END / 2, MAP_END / 2).
pub const MAP_END: i32 = 14800;

/// Summoner's Rift areas; blue (team 100) is bottom-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapZone { BlueBase, RedBase, Top, Mid, Bot, River, BlueJungle, RedJungle }

impl MapZone {
    /// A position as seen from blue side: red side's map mirrored across the river
    /// diagonal, so the bases and jungles swap but top lane stays top (a 180° turn
    /// would put red-side top-lane deaths in bot lane).
    pub fn from_blue(x: i32, y: i32, blue: bool) -> (i32, i32) {
        if blue { (x, y) } else { (MAP_END - y, MAP_END - x) }
    }

    pub fn at(x: i32, y: i32) -> MapZone {
        let near = |a: i32, b: i32| ((a * a + b * b) as f32).sqrt() < 5000.0;
        if near(x, y) {
            MapZone::BlueBase
        } else if near(MAP_END - x, MAP_END - y) {
            MapZone::RedBase
        } else if x < 2200 || y > 12600 {
            MapZone::Top
        } else if y < 2200 || x > 12600 {
            MapZone::Bot
        } else if (x - y).abs() < 1300 {
            MapZone::Mid
        } else if (x + y - MAP_END).abs() < 1300 {
            MapZone::River
        } else if x + y < MAP_END {
            MapZone::BlueJungle
        } else {
            MapZone::RedJungle
        }
    }
}
