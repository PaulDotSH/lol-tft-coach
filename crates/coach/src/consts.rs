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
pub const CRIT_FLAG: f32 = 0.4;

// ---- needs ----
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
pub const MIN_ITEM_SHARE: f32 = 0.01;
/// Item my champ builds, but in another slot.
pub const OFF_SLOT_PENALTY: f32 = 0.01;
/// Item my champ does not build at all (vs the champ's own win rate).
pub const OFF_LIST_PENALTY: f32 = 0.03;
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

// ---- recap ----
pub const DEATHS_NOTE: u32 = 6;
