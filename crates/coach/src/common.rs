//! Small shared helpers.

use crate::consts::*;
use model::*;
use std::sync::LazyLock;

pub const N: usize = 11;
pub const NEED_LABELS: [&str; N] = [
    "Armor",
    "Magic resist",
    "Health",
    "Anti-heal",
    "Tenacity",
    "Cleanse",
    "Anti-burst",
    "Armor pen",
    "Magic pen",
    "Anti-crit",
    "Anti attack speed",
];

pub fn arr(p: &Profile) -> [f32; N] {
    [
        p.armor,
        p.mr,
        p.hp,
        p.anti_heal,
        p.tenacity,
        p.cleanse,
        p.anti_burst,
        p.armor_pen,
        p.magic_pen,
        p.anti_crit,
        p.anti_attack_speed,
    ]
}

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// Accepts a fraction (0.25) or a percent (25) and returns a fraction.
pub fn frac(x: f32) -> f32 {
    if x > 1.0 { x / 100.0 } else { x }
}

pub fn wilson(wins: u32, games: u32) -> f32 {
    wilson_p(wins as f32 / games.max(1) as f32, games)
}

/// Wilson lower bound of a proportion `p` over `games`.
pub fn wilson_p(p: f32, games: u32) -> f32 {
    if games == 0 {
        return 0.0;
    }
    let (n, z, z2) = (games as f32, WILSON_Z, WILSON_Z * WILSON_Z);
    (p + z2 / (2.0 * n) - z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt()) / (1.0 + z2 / n)
}

/// Pull a percent win rate toward 50 by sample size.
pub fn shrink(wr: f32, games: u32, k: f32) -> f32 {
    50.0 + (wr - 50.0) * games as f32 / (games as f32 + k)
}

pub fn map_ok(item: &Item, mode: Mode) -> bool {
    match mode {
        Mode::Rift => item.rift,
        Mode::Aram | Mode::Mayhem => item.aram,
    }
}

/// "Coachless 16.20", "lolalytics 16.21", "lolalytics, last 30 days"; empty without stats.
pub fn build_source(stats: Option<&ChampStats>) -> String {
    match stats {
        Some(ChampStats { coachless: Some(p), .. }) => format!("Coachless {p}"),
        Some(s) if s.patch.contains('.') => format!("lolalytics {}", s.patch),
        Some(s) => format!("lolalytics, last {} days", s.patch),
        None => String::new(),
    }
}

pub fn fmt_games(n: u64) -> String {
    match n {
        0..1000 => n.to_string(),
        1000..10_000 => format!("{:.1}k", n as f32 / 1000.0),
        _ => format!("{:.0}k", n as f32 / 1000.0),
    }
}

static UNKNOWN: LazyLock<Champion> = LazyLock::new(|| Champion {
    key: 0,
    id: String::new(),
    name: "?".into(),
    slug: String::new(),
    classes: vec![],
    traits: Traits { magic_share: 0.5, ..Default::default() },
    base: BaseStats::default(),
});

pub fn champ(sd: &StaticData, key: ChampKey) -> &Champion {
    sd.champs.get(&key).unwrap_or(&UNKNOWN)
}

/// Summed stats of a player's inventory.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inv {
    pub gold: u32,
    /// Gold in items with AD or AP (how much the build says about damage type).
    pub dmg_gold: u32,
    pub ap: f32,
    pub ad: f32,
    pub armor: f32,
    pub mr: f32,
    pub hp: f32,
    pub crit: f32,
    pub attack_speed: f32,
}

pub fn inventory(sd: &StaticData, items: &[ItemId]) -> Inv {
    let mut v = Inv::default();
    for it in items.iter().filter_map(|id| sd.items.get(id)) {
        let s = &it.stats;
        v.gold += held_value(it);
        let ad = s.ad + s.lethality * LETHALITY_TO_AD;
        if s.ap + ad > 0.0 {
            v.dmg_gold += it.cost;
        }
        v.ap += s.ap;
        v.ad += ad;
        v.armor += s.armor;
        v.mr += s.mr;
        v.hp += s.hp;
        v.crit += frac(s.crit);
        v.attack_speed += frac(s.attack_speed);
    }
    v
}

/// Item gold an item adds to an inventory: full cost, except consumables
/// (potions, wards, elixirs), which are spent rather than held.
pub fn held_value(it: &Item) -> u32 {
    if it.tags.iter().any(|t| t == "Consumable") { 0 } else { it.cost }
}

/// Lower is better: "S+" < "S" < "S-" < "A+" … ; unknown labels sort last.
pub fn tier_rank(t: &str) -> i32 {
    let mut ch = t.trim().chars();
    let base = match ch.next().map(|c| c.to_ascii_uppercase()) {
        Some('S') => 0,
        Some('A') => 10,
        Some('B') => 20,
        Some('C') => 30,
        Some('D') => 40,
        Some('F') => 50,
        _ => return 100,
    };
    base + ch.map(|c| match c {
        '+' => -1,
        '-' => 1,
        _ => 0,
    })
    .sum::<i32>()
}
