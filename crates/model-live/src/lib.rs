//! In-game advice (`coach` output, rendered by the UI): buy next, threats, gold, shopping,
//! death windows, augments, calls, tips and the laning matchup panel.

pub use model_core::*;
use model_draft::MatchupCard;

data! {
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
        /// Where the item stats came from: "Coachless 16.20", "lolalytics 16.21".
        #[serde(default)]
        pub build_source: String,
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
        /// Players worth a shutdown (or very little), from kill streaks and item gold.
        pub bounties: Vec<Bounty>,
        /// Whether my team should take fights right now, and why.
        pub readiness: Option<Readiness>,
        /// Short calls (back for an objective, take one with numbers), newest situation only.
        pub calls: Vec<Call>,
        /// Quiet tips: lane pace, vision, spending.
        #[serde(default)]
        pub tips: Vec<LiveTip>,
        /// Rift: tips against my lane opponent(s), both enemy bot laners in bot lane.
        #[serde(default)]
        pub matchup: Vec<MatchupCard>,
        /// Which ability to level next; None when the skill order can't describe my champion.
        #[serde(default)]
        pub skill: Option<SkillNext>,
        #[serde(default)]
        pub what_if: Vec<WhatIf>,
        /// Rift, when I jungle: clear pace and the lane board.
        #[serde(default)]
        pub jungle: Option<JungleAdvice>,
        /// My team's chance to win; None until the model is fitted for the mode.
        #[serde(default)]
        pub win: Option<WinProb>,
        /// How the next minutes look, from `win`; None without it.
        #[serde(default)]
        pub posture: Option<Posture>,
    }

    /// A standing read of the game from the win probability (`coach::posture`): a read and
    /// options built from live facts, never orders.
    pub struct Posture {
        pub stance: Stance,
        pub headline: String,
        /// The biggest factors and who the rest of the game favours.
        pub why: Vec<String>,
        /// Two or three choices: the biggest stake and its timer, shutdowns, who is dead, my next item.
        pub options: Vec<String>,
    }

    /// My team's chance to win from what the scoreboard shows (`coach::winprob`).
    pub struct WinProb {
        /// 0..1, everything counted.
        pub p: f32,
        /// 0..1 without who is dead right now: the standing state.
        pub base: f32,
        /// Change of `base` over the last two minutes, in points; None until the samples reach back.
        pub trend: Option<f32>,
        /// What moves `p`, largest first.
        pub factors: Vec<WinFactor>,
        /// What the next objectives are worth, largest first.
        pub stakes: Vec<Stake>,
        /// None when no champion's win rates by game length are known.
        pub outlook: Option<Outlook>,
        /// The model fits this game loosely: Swiftplay, Mayhem, champions without data.
        pub rough: bool,
        /// "from the scoreboard · fit on 16.22 (10k games)".
        pub source: String,
    }

    pub struct WinFactor {
        /// Model feature: "gold", "level", "towers", …
        pub key: String,
        pub label: String,
        /// Points of `p` it adds (+) or costs (−).
        pub points: f32,
    }

    pub struct Stake {
        /// "Dragon", "Baron", "Elder", "Tower", "Inhibitor", "Your death".
        pub name: String,
        /// `p` with it on my side minus with it on theirs, in points.
        pub swing: f32,
        pub note: Option<String>,
    }

    /// Who the rest of the game favours: win-rate points of my champions over theirs in games
    /// that last at least until now, and until `later_min`.
    pub struct Outlook {
        pub now: f32,
        pub later: f32,
        pub later_min: u32,
    }

    pub struct JungleAdvice {
        pub pace: Option<String>,
        /// Top, mid and bot, most open first.
        pub lanes: Vec<LaneOpen>,
        /// The lane board includes what the minimap shows.
        pub minimap: bool,
        pub camps: Vec<CampTimer>,
    }

    pub struct LaneOpen {
        pub lane: Lane,
        pub score: f32,
        pub reasons: Vec<String>,
        pub enemy: Vec<ChampKey>,
    }

    pub struct CampTimer {
        pub name: String,
        pub respawn_at: f32,
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
        /// `WinProb::base` at that time (0..1).
        #[serde(default)]
        pub win: Option<f32>,
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

    pub struct Bounty {
        pub champ: ChampKey,
        pub ally: bool,
        pub me: bool,
        /// Kills since their last death.
        pub streak: u32,
        /// Deaths since their last kill.
        pub cold: u32,
        /// Item gold over their team's average.
        pub item_lead: i32,
        pub text: String,
    }

    pub struct Readiness {
        pub verdict: Verdict,
        pub text: String,
        pub reasons: Vec<String>,
    }

    pub struct Call {
        /// Stable while the situation lasts (deduplicates audio).
        pub key: String,
        pub text: String,
        /// Shorter, for speech.
        pub say: String,
    }

    /// Silent prompt to hold Tab; gone once the scoreboard was read.
    pub struct TabHint {
        pub text: String,
        pub reason: String,
    }

    pub struct WhatIf {
        pub item: ItemId,
        pub damage: f32,
        pub burst: f32,
        pub sustained: f32,
        pub burst_leads: bool,
        pub per_enemy: Vec<WhatIfEnemy>,
        pub ehp: f32,
        pub ehp_physical: f32,
        pub ehp_magic: f32,
        pub rough: bool,
        pub note: Option<String>,
    }

    pub struct WhatIfEnemy {
        pub champ: ChampKey,
        pub burst: f32,
        pub sustained: f32,
        pub lane: bool,
    }

    /// The next skill point from a lolalytics skill order.
    pub struct SkillNext {
        /// "Q", "W", "E" or "R".
        pub key: String,
        /// Points I can spend right now.
        pub unspent: u32,
        /// Level the point is for: my level + 1 when none is unspent.
        pub at_level: u32,
        /// Max order, "QEW".
        pub priority: String,
        /// "vs Braum, 1.2k games", "lolalytics 16.21".
        pub source: String,
    }

    /// A quiet in-game tip (not spoken): lane pace, vision, spending.
    pub struct LiveTip {
        pub key: String,
        /// "lane", "vision", "back".
        pub kind: String,
        pub text: String,
    }
}

enums! {
    pub enum Verdict { Fight, Even, Avoid }

    /// Ahead: Press (the rest of the game does not favour us) or Hold (it does); 40–60 %: Even;
    /// behind: Stall (it favours us) or Swing (it does not).
    pub enum Stance { Press, Hold, Even, Stall, Swing }
}
