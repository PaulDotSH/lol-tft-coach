//! Plain data shared by every crate and mirrored to TypeScript (ts-rs).
//! No behaviour beyond tiny helpers: sources produce these, `coach` consumes
//! them, the UI renders the `*Advice` types.
//!
//! The types live in domain crates so an edit rebuilds only the crates that use them:
//! `model-core` (ids, enums), `model-static` (Data Dragon, lolalytics, u.gg), `model-game`
//! (client and game reports, match history), `model-live` (in-game advice), `model-draft`
//! (champ select advice), `model-review` (recap, self-coaching, game review). This crate
//! re-exports all of them and holds the app state the UI receives.

pub use model_core::*;
pub use model_static::*;
pub use model_game::*;
pub use model_live::*;
pub use model_draft::*;
pub use model_review::*;
use std::collections::HashMap;

enums! {
    pub enum Phase { NoClient, Idle, ChampSelect, Loading, InGame, PostGame }

    pub enum SourceState { Off, Ok, Degraded, Error }
}

data! {
    pub struct Sources {
        pub client: SourceState,
        pub game: SourceState,
        pub lolalytics: SourceState,
        pub ugg: SourceState,
        pub ocr: SourceState,
        /// Minimap reading for the gank warnings: off outside the Rift.
        pub minimap: SourceState,
        /// Off while signed out of Coachless.
        pub coachless: SourceState,
        /// The Coachless account has Premium (current patch).
        #[serde(default)]
        pub coachless_premium: bool,
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
        /// Speak in-game calls aloud.
        #[serde(default = "yes")]
        pub audio: bool,
        /// Draw the click-through overlay over the game window (LoL and TFT). Off by default: any
        /// window over League costs it FPS, and drawing inside the game would need Riot's approval.
        #[serde(default)]
        pub overlay: bool,
        /// Read the minimap and warn of ganks on your lane, the enemy jungler, roams and enemies in your jungle (Rift).
        #[serde(default = "yes")]
        pub gank_warn: bool,
        /// Save TFT unit crops of every fight and scouted board for training unit recognition
        /// (`%LOCALAPPDATA%\lol-coach\dev\crops`). Always on in debug mode.
        #[serde(default)]
        pub tft_crops: bool,
    }

    /// One rank badge over an offered augment card. Ranks only, never win rates.
    pub struct OverlayBadge {
        /// Card centre, fraction of the game window's width.
        pub x: f32,
        /// 0-based place among the cards on offer; None when the coach has no rank for it.
        pub place: Option<u32>,
        /// Short reason, e.g. "fits your Nashor's Tooth (on-hit)".
        pub reason: Option<String>,
    }

    /// What the in-game overlay draws; pushed as the `overlay` event (None: nothing, hidden).
    pub struct Overlay {
        pub tft: bool,
        /// Badge row (the badges' top), fraction of the game window's height.
        pub badge_y: f32,
        /// Empty unless the offer cards are on screen.
        pub badges: Vec<OverlayBadge>,
        /// LoL: Buy next; TFT: the target comp.
        pub head: Option<String>,
        /// LoL: gold missing and ETA; TFT: units still missing.
        pub detail: Option<String>,
        /// LoL: the newest call; TFT: the roll / level call.
        pub call: Option<String>,
        /// Game time (s) of this view; the timers count down from it.
        pub time: f32,
        /// LoL objective timers, as the dashboard has them.
        pub timers: Vec<ObjectiveTimer>,
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
        /// LP over my ranked games, Solo/Duo then Flex.
        #[serde(default)]
        pub ranked: Vec<RankTrend>,
    }
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings { tier: "emerald_plus".into(), region: "all".into(), ocr_enabled: true, record_fixtures: false, goals: vec![], audio: true, overlay: false, gank_warn: true, tft_crops: false }
    }
}
