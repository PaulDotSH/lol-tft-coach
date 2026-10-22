//! Per-mode rule overrides from `modes.toml` (sources cited there).

use model::{ItemId, Mode};
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Debug, Deserialize)]
pub struct Rules {
    /// Multipliers on the tenacity / cleanse needs.
    #[serde(default = "one")]
    pub tenacity_need: f32,
    #[serde(default = "one")]
    pub cleanse_need: f32,
    /// None: crit above it is not wasted.
    pub crit_cap: Option<f32>,
    pub attack_speed_cap: f32,
    #[serde(default)]
    pub melee_bonus_resists: f32,
    #[serde(default)]
    pub ranged_bonus_resists: f32,
    /// Items never recommended in this mode.
    #[serde(default)]
    pub exclude: Vec<ItemId>,
    pub stat_anvil: Option<Anvil>,
    #[serde(default)]
    pub elixirs: Vec<ItemId>,
    #[serde(default)]
    pub elixir_level: u32,
    /// Levels at which an augment offer appears.
    #[serde(default)]
    pub augment_levels: Vec<u32>,
}

#[derive(Debug, Deserialize)]
pub struct Anvil {
    pub item: ItemId,
    pub level: u32,
    pub cost: u32,
}

fn one() -> f32 {
    1.0
}

#[derive(Deserialize)]
struct File {
    rift: Rules,
    aram: Rules,
    mayhem: Rules,
}

static RULES: LazyLock<File> =
    LazyLock::new(|| toml::from_str(include_str!("../modes.toml")).expect("modes.toml is valid"));

pub fn rules(mode: Mode) -> &'static Rules {
    match mode {
        Mode::Rift => &RULES.rift,
        Mode::Aram => &RULES.aram,
        Mode::Mayhem => &RULES.mayhem,
    }
}

// Need indices (`common::NEED_LABELS` order).
pub const TENACITY: usize = 4;
pub const CLEANSE: usize = 5;
pub const ARMOR_PEN: usize = 7;
pub const MAGIC_PEN: usize = 8;

/// Ranged champions have a base attack range above this.
pub const RANGED_FROM: f32 = 300.0;

impl Rules {
    pub fn scale_needs(&self, p: &mut [f32; crate::common::N]) {
        p[TENACITY] *= self.tenacity_need;
        p[CLEANSE] *= self.cleanse_need;
    }

    pub fn bonus_resists(&self, attack_range: f32) -> f32 {
        if attack_range > RANGED_FROM { self.ranged_bonus_resists } else { self.melee_bonus_resists }
    }

    /// Augments offered by `level`.
    pub fn augments_by(&self, level: u32) -> usize {
        self.augment_levels.iter().filter(|&&l| l <= level).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn need_indices() {
        use crate::common::NEED_LABELS as L;
        assert_eq!([L[TENACITY], L[CLEANSE], L[ARMOR_PEN], L[MAGIC_PEN]], ["Tenacity", "Cleanse", "Armor pen", "Magic pen"]);
    }

    #[test]
    fn modes_toml_parses_with_expected_rules() {
        let m = rules(Mode::Mayhem);
        assert!(m.tenacity_need < 1.0 && m.cleanse_need < 1.0 && m.crit_cap.is_none());
        assert_eq!(m.attack_speed_cap, 5.0);
        assert_eq!((m.bonus_resists(125.0), m.bonus_resists(550.0)), (15.0, 0.0));
        assert_eq!(m.stat_anvil.as_ref().map(|a| (a.item, a.level, a.cost)), Some((6032, 9, 750)));
        assert_eq!((m.augments_by(2), m.augments_by(3), m.augments_by(18)), (0, 1, 4));
        let r = rules(Mode::Rift);
        assert_eq!((r.tenacity_need, r.crit_cap, r.attack_speed_cap), (1.0, Some(1.0), 3.003));
        assert!(r.stat_anvil.is_none() && r.augment_levels.is_empty());
        assert!(rules(Mode::Aram).exclude.contains(&123430) && m.exclude.is_empty());
    }
}
