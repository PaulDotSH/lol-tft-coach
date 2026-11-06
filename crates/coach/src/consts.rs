//! Tuning constants, all first guesses to be tuned with recorded games.
//! Values marked SEASON change between seasons and must be checked against
//! the live season before trusting them.

// ---- threat score ----
/// Relative item-gold difference is taken against max(lobby average, this).
pub const GOLD_FLOOR: f32 = 1500.0;
pub const CS_FLOOR: f32 = 20.0;
pub const W_GOLD: f32 = 1.0;
/// Per point of (kills + ASSIST_WEIGHT·assists − deaths) above the lobby average.
pub const W_KDA: f32 = 0.12;
pub const ASSIST_WEIGHT: f32 = 0.5;
pub const W_LEVEL: f32 = 0.25;
pub const W_CS: f32 = 0.5;
pub const KILLS_ON_ME_STEP: f32 = 0.25;
pub const KILLS_ON_ME_CAP: u32 = 4;
pub const LANE_OPP_MULT: f32 = 1.3;
/// Threat assumed for revealed enemies in champ select.
pub const AVG_THREAT: f32 = 50.0;

// ---- damage profile / flags ----
/// 1 AD ≈ this much AP when judging which way an enemy's items lean.
pub const AD_TO_AP: f32 = 1.6;
pub const LETHALITY_TO_AD: f32 = 1.5;
/// Max weight of the item-derived magic share vs the champion's trait.
pub const ITEM_SHARE_MAX: f32 = 0.7;
/// Damage-item gold at which the item share reaches ITEM_SHARE_MAX.
pub const ITEM_SHARE_FULL_GOLD: f32 = 8000.0;
pub const AP_LABEL: f32 = 0.65;
pub const AD_LABEL: f32 = 0.35;
pub const TRUE_LABEL: f32 = 0.5;
pub const FLAG: f32 = 0.5;
/// Burst needed for the "assassin" flag without the Assassin class (above the Mage default).
pub const ASSASSIN_FLAG_BURST: f32 = 0.75;
pub const CRIT_FLAG: f32 = 0.4;

// ---- needs ----
/// Bounds on the armor/MR need multiplier from my current resists.
pub const RESIST_TILT_MIN: f32 = 0.5;
pub const RESIST_TILT_MAX: f32 = 1.5;
pub const NEED_W_FLOOR: f32 = 0.25;
/// Share of a true-damage champion's damage that ignores resists.
pub const TRUE_FRAC: f32 = 0.3;
/// Damage share mapped linearly to armor/MR need 0..1.
pub const SHARE_LO: f32 = 0.2;
pub const SHARE_HI: f32 = 0.8;
/// Top threat score at which defensive needs reach full intensity.
pub const INTENSITY_FULL: f32 = 70.0;
pub const INTENSITY_MIN: f32 = 0.4;
pub const HP_NEED: f32 = 0.4;
/// Blend of threat-weighted mean vs the single biggest (score-scaled) enemy.
pub const MEAN_W: f32 = 0.5;
pub const MEAN_W_CC: f32 = 0.8;
pub const HEAL_FULL: f32 = 0.5;
pub const CC_FULL: f32 = 0.55;
pub const CLEANSE_LO: f32 = 0.35;
pub const CLEANSE_SPAN: f32 = 0.4;
pub const ASSASSIN_BURST: f32 = 0.8;
pub const BURST_LO: f32 = 0.25;
pub const BURST_SPAN: f32 = 0.5;
pub const TANK_TRAIT: f32 = 0.6;
pub const TANK_RESIST_FULL: f32 = 150.0;
pub const TANK_HP_FULL: f32 = 2000.0;
pub const TANK_FULL: f32 = 0.5;
/// Enemy crit chance (fraction) counted as a full crit carry.
pub const CRIT_FULL: f32 = 0.5;
pub const MARKSMAN_CRIT: f32 = 0.5;
pub const CRIT_NEED_FULL: f32 = 0.4;
pub const AS_FULL: f32 = 1.0;
pub const MARKSMAN_AS: f32 = 0.5;
pub const AS_NEED_FULL: f32 = 0.45;
/// need *= 1 − NEED_DECAY·answer after each path pick.
pub const NEED_DECAY: f32 = 0.6;

// ---- item scoring ----
pub const WILSON_Z: f32 = 1.28;
pub const PR_BONUS: f32 = 0.02;
pub const MIN_ITEM_GAMES: u32 = 50;
pub const MIN_ITEM_SHARE: f32 = 0.02;
/// Prior strength (games) pulling a slot row's win rate toward the slot average;
/// tames snowball items (Mejai's) whose small samples are bought only when ahead.
pub const SHRINK_GAMES: f32 = 1500.0;
/// Situational score (win-rate units) above which a reason names the threat.
pub const REASON_SIT_MIN: f32 = 0.01;
/// Mejai's Soulstealer, Dark Seal: their win rates come from games already won.
pub const SNOWBALL: [model::ItemId; 2] = [3041, 1082];
/// "Ahead" for snowball items: (kills + assists/2) ≥ this × (deaths + 1).
pub const AHEAD_KDA: f32 = 2.5;
/// Starting gold: the start set is the opening purchases that fit in it.
pub const START_GOLD: u32 = 500;
/// Item my champ builds, but in another slot.
pub const OFF_SLOT_PENALTY: f32 = 0.01;
/// Item my champ does not build at all (vs the champ's own win rate).
pub const OFF_LIST_PENALTY: f32 = 0.03;
/// Resist item against the other damage type: score lost per unit of
/// (its armor-vs-MR answer gap) × (the opposite need's lead). Stats can't fix a wrong resist.
pub const RESIST_MISMATCH: f32 = 0.08;
/// Spirit Visage: amplifies my own healing and shields, so it needs some.
pub const HEAL_AMP_ITEMS: [model::ItemId; 1] = [3065];
/// Items whose anti-burst is a magic-only shield: Kaenic Rookern, Maw of Malmortius.
pub const MAGIC_SHIELDS: [model::ItemId; 2] = [2504, 3156];
/// Score for upgrading a component I already hold.
pub const HELD_COMPONENT_BONUS: f32 = 0.02;
/// Last tick's Buy next stays first unless another item beats it by this much
/// (near-equal boots otherwise swap every few ticks).
pub const KEEP_MARGIN: f32 = 0.02;
/// Below this `heals` trait, a heal-amp item loses SELF_HEAL_PENALTY (win-rate units).
pub const SELF_HEAL_MIN: f32 = 0.3;
pub const SELF_HEAL_PENALTY: f32 = 0.03;
pub const NO_STATS_WR: f32 = 0.5;
pub const FIT_BONUS: f32 = 0.02;
pub const FIT_MIN: f32 = 0.5;
/// Off-list items need at least this dot(answers, needs) when stats exist.
pub const ANSWER_MIN: f32 = 0.15;
/// Balanced mode: items 1–2 follow stats, items 3–6 and boots swing to threats.
pub const SLOT_WEIGHT: [f32; 6] = [0.012, 0.02, 0.06, 0.07, 0.08, 0.08];
pub const BOOTS_WEIGHT: f32 = 0.06;
/// Boots come after this many completed legendaries.
pub const BOOTS_AFTER: usize = 1;
pub const MAX_ITEMS: usize = 6;
/// SEASON: World Atlas → Runic Compass → Bounty of Worlds, then a free upgrade.
pub const SUPPORT_QUEST: [model::ItemId; 3] = [3865, 3866, 3867];
/// SEASON: Celestial Opposition, Dream Maker, Zaz'Zak's Realmspike, Solstice Sleigh, Bloodsong.
pub const SUPPORT_UPGRADES: [model::ItemId; 5] = [3869, 3870, 3871, 3876, 3877];
pub const ALTERNATIVES: usize = 2;
/// Weight of champion traits vs lolalytics builds for the build direction.
pub const CHAMP_DIR_WEIGHT: f32 = 0.3;
// Item stat → direction-vector units.
pub const AP_UNIT: f32 = 100.0;
pub const AD_UNIT: f32 = 60.0;
pub const LETH_UNIT: f32 = 20.0;
pub const CRIT_UNIT: f32 = 0.5;
pub const AS_UNIT: f32 = 0.8;
pub const APEN_UNIT: f32 = 0.6;
pub const MPEN_FLAT_UNIT: f32 = 30.0;
pub const MPEN_PCT_UNIT: f32 = 0.6;
pub const RESIST_UNIT: f32 = 80.0;
pub const HP_UNIT: f32 = 450.0;

// ---- matchups ----
pub const FAVOURED: f32 = 52.5;
pub const UNFAVOURED: f32 = 47.5;

// ---- self stats: vision score per minute when lolalytics has none (rough) ----
pub const VISION_TOP: f32 = 0.8;
pub const VISION_JUNGLE: f32 = 1.1;
pub const VISION_MID: f32 = 0.8;
pub const VISION_BOT: f32 = 0.7;
pub const VISION_SUPPORT: f32 = 2.0;

// ---- objective timers (seconds of game time). SEASON: verify every season. ----
pub const DRAGON_FIRST: f32 = 300.0;
pub const DRAGON_RESPAWN: f32 = 300.0;
pub const SOUL_DRAGONS: u32 = 4;
pub const ELDER_RESPAWN: f32 = 360.0;
pub const BARON_FIRST: f32 = 1200.0;
pub const BARON_RESPAWN: f32 = 360.0;
/// Herald spawns once and leaves when baron spawns.
pub const HERALD_FIRST: f32 = 900.0;
/// Void grubs spawn once (one wave) and leave when herald spawns.
pub const GRUBS_FIRST: f32 = 480.0;
pub const GRUBS_COUNT: u32 = 3;

// ---- champ select ----
pub const TIER_MIN_GAMES: u32 = 500;
pub const TIER_SHRINK_K: f32 = 1000.0;
pub const MATCHUP_MIN_GAMES: u32 = 20;
pub const MATCHUP_SHRINK_K: f32 = 200.0;
pub const LANE_OPP_W: f32 = 1.0;
pub const OTHER_ENEMY_W: f32 = 0.3;
/// Share of the pick score from matchups (rest: tier-list win rate).
pub const MATCHUP_BLEND: f32 = 0.6;
pub const PICKS: usize = 5;
pub const BANS: usize = 5;
pub const BAN_COUNTER_W: f32 = 1.0;
pub const BAN_META_W: f32 = 0.6;
pub const BAN_OFFLANE_W: f32 = 0.3;
pub const CORE_ITEMS: usize = 3;
/// A rune swapped in for the enemy team needs this many games on the champion…
pub const RUNE_MIN_GAMES: u32 = 300;
/// …and at most this many win-rate points below the rune it replaces.
pub const RUNE_WR_SLACK: f32 = 1.5;
/// Enemies with burst ≥ ASSASSIN_FLAG_BURST (or tanks) needed for Bone Plating (Cut Down).
pub const RUNE_THREATS: usize = 2;
/// Enemies with CC trait ≥ RUNE_HARD_CC needed for the tenacity shard.
pub const RUNE_CC_FOES: usize = 3;
pub const RUNE_HARD_CC: f32 = 0.8;
/// The highest win rate page is shown when its keystone differs and has this many games.
pub const RUNE_ALT_MIN_GAMES: u32 = 1000;

// ---- draft ----
/// Lane share (percent) below which a champion is not placed in that lane.
pub const MIN_LANE_SHARE: f32 = 1.0;
/// Duo rows with fewer games are ignored (their deltas swing ±15).
pub const SYN_MIN_GAMES: u32 = 150;
/// Prior games pulling a duo's synergy delta toward 0.
pub const SYN_SHRINK_K: f32 = 500.0;
/// How much a duo's synergy counts, by lane pair; other pairs DUO_W_OTHER.
pub const DUO_W: [(model::Lane, model::Lane, f32); 4] = [
    (model::Lane::Bottom, model::Lane::Support, 1.0),
    (model::Lane::Jungle, model::Lane::Middle, 0.8),
    (model::Lane::Jungle, model::Lane::Top, 0.6),
    (model::Lane::Middle, model::Lane::Support, 0.5),
];
pub const DUO_W_OTHER: f32 = 0.3;
/// Pick score points per (weighted, shrunk) synergy point with a revealed ally.
pub const SYN_PICK_W: f32 = 1.0;
/// A champion's damage counts less the tankier it is (1 − this·tank).
pub const DMG_TANK_DISCOUNT: f32 = 0.5;
pub const FRONTLINE_TANK: f32 = 0.6;
pub const ENGAGE_CC: f32 = 0.7;
pub const ENGAGE_TANK: f32 = 0.5;
/// Damage share at which a team counts as one-dimensional.
pub const ONE_SIDED: f32 = 0.8;
/// Team CC below this per champion counts as little CC.
pub const LOW_CC_PER_CHAMP: f32 = 0.3;
/// Comp gaps are judged once a team shows this many champions (report) …
pub const GAP_MIN_CHAMPS: usize = 3;
/// … and pick bonuses once this many allies are revealed.
pub const PICK_GAP_MIN_ALLIES: usize = 2;
/// Draft points lost per gap (win-rate points, first guesses).
pub const GAP_FRONTLINE: f32 = 1.0;
pub const GAP_DAMAGE: f32 = 1.0;
pub const GAP_CC: f32 = 0.5;
pub const GAP_ENGAGE: f32 = 0.5;
pub const GAP_PEEL: f32 = 0.75;
/// Team peel (summed trait) below which carries count as unprotected.
pub const PEEL_MIN: f32 = 0.5;
/// Mages tankier than this don't count as carries needing peel.
pub const CARRY_MAX_TANK: f32 = 0.3;
/// Candidate traits that fill a gap.
pub const FILL_MAGIC: f32 = 0.6;
pub const FILL_PHYSICAL: f32 = 0.4;
pub const FILL_CC: f32 = 0.6;
pub const FILL_PEEL: f32 = 0.6;
/// A second pick reason is appended when its term is worth at least this.
pub const REASON_SECOND_MIN: f32 = 0.5;

// ---- bench (ARAM / Mayhem) ----
/// Win-rate points per u.gg tier letter (S = +3, A = +1.5, B = 0, C = −1.5, …).
pub const MAYHEM_TIER_STEP: f32 = 1.5;
/// ARAM win rates are shrunk toward the list mean with this many prior games.
pub const ARAM_SHRINK_K: f32 = 1000.0;

// ---- recap ----
pub const DEATHS_NOTE: u32 = 6;

// ---- enemy item spikes (V6) ----
/// Threat z per completed legendary above the lobby average. Item gold already
/// counts the gold, so this only rewards finished power spikes; kept small.
pub const W_LEGEND: f32 = 0.15;
/// A legendary counts as newly finished for this many game seconds.
pub const ITEM_NEW_WINDOW: f32 = 90.0;

// ---- ult ranks (Q5): levels at which R ranks up, static ----
pub const ULT_LEVELS: [u32; 3] = [6, 11, 16];
/// Four-rank ults (Jayce, Elise, Nidalee, Karma) start at level 1.
pub const ULT_LEVELS_4: [u32; 4] = [1, 6, 11, 16];

// ---- shopping (Q1) / gold ETA (Q2) ----
pub const INVENTORY_SLOTS: usize = 6;
/// Path items the shopping list spends on, in order (buy next, then the one after).
pub const SHOP_PATH_DEPTH: usize = 2;
pub const CONTROL_WARD: model::ItemId = 2055;
/// Elixirs of Iron, Sorcery, Wrath; bought only once the build is complete.
pub const ELIXIR_IRON: model::ItemId = 2138;
pub const ELIXIR_SORCERY: model::ItemId = 2139;
pub const ELIXIR_WRATH: model::ItemId = 2140;
pub const ELIXIR_LEVEL: u32 = 9;
/// Income is averaged over this much game time; spending drops are skipped.
pub const INCOME_WINDOW: f32 = 120.0;
/// No income estimate before this much non-spending time was observed.
pub const INCOME_MIN_SPAN: f32 = 20.0;
pub const BACK_PLANS: usize = 2;

// ---- minimap gank warnings (early game). First guesses, tune against a real game. ----
/// Warn only before this much game time (laning phase).
pub const GANK_EARLY_S: f32 = 14.0 * 60.0;
/// A warning is re-announced only after its threat has been absent this long.
pub const GANK_GAP_S: f32 = 25.0;
/// Mid-gank movement is measured against the newest read at least this old (seconds).
pub const GANK_MOVE_DT: f32 = 1.5;
/// Reads older than this are dropped from the history (seconds).
pub const GANK_MAX_DT: f32 = 6.0;
/// A blip is tied to one from the previous read within this distance (world units).
pub const GANK_MAX_STEP: i32 = 3500;
/// A mid warning needs the blip to have moved at least this far toward your base.
pub const GANK_APPROACH: i32 = 350;
