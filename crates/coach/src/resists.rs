//! Enemy armor/MR from visible level + items + static base stats, penetration value
//! against them, and my effective HP split. Nothing hidden: no buffs, no runes.

use crate::{
    common::*,
    consts::{NEED_W_FLOOR, RESIST_TILT_MAX, RESIST_TILT_MIN},
    modes::{ARMOR, MR, Rules},
    threat::Foe,
};
use model::*;

/// Damage gain (fraction) at which a pen item fully answers the pen need.
pub const PEN_GAIN_FULL: f32 = 0.25;
/// Reference items for the lethality-vs-% notes: Youmuu's-like vs Lord Dominik's-like,
/// Sorcerer's-like flat vs Void Staff.
const REF_LETHALITY: f32 = 18.0;
const REF_ARMOR_PEN: f32 = 0.35;
const REF_MAGIC_FLAT: f32 = 15.0;
const REF_MAGIC_PEN: f32 = 0.4;
/// My magic share above/below which only one pen note is shown.
const NOTE_SHARE: f32 = 0.35;

/// Data Dragon growth: base + per × (n − 1) × (0.7025 + 0.0175 × (n − 1)).
pub fn grow(base: f32, per: f32, level: u32) -> f32 {
    let n = level.max(1) as f32 - 1.0;
    base + per * n * (0.7025 + 0.0175 * n)
}

/// Every enemy of mine, in player order. None for a champion without base stats.
pub fn enemies(snap: &GameSnapshot, sd: &StaticData, rules: &Rules) -> Vec<Option<EnemyResist>> {
    let Some(me) = snap.players.get(snap.me) else { return vec![] };
    snap.players
        .iter()
        .filter(|p| p.team != me.team)
        .map(|p| {
            let b = &champ(sd, p.champ).base;
            if b.armor <= 0.0 {
                return None;
            }
            let inv = inventory(sd, &p.items);
            let bonus = rules.bonus_resists(b.attack_range);
            Some(EnemyResist {
                champ: p.champ,
                level: p.level,
                armor: grow(b.armor, b.armor_per_level, p.level) + inv.armor + bonus,
                mr: grow(b.mr, b.mr_per_level, p.level) + inv.mr + bonus,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pen {
    pub flat: f32,
    pub pct: f32,
}

impl Pen {
    pub fn plus(self, o: Pen) -> Pen {
        Pen { flat: self.flat + o.flat, pct: 1.0 - (1.0 - self.pct) * (1.0 - o.pct) }
    }
}

/// (armor pen, magic pen) of the items in `ids`.
pub fn item_pen(sd: &StaticData, ids: &[ItemId]) -> (Pen, Pen) {
    ids.iter().filter_map(|i| sd.items.get(i)).fold((Pen::default(), Pen::default()), |(a, m), it| {
        let s = &it.stats;
        (a.plus(Pen { flat: s.lethality, pct: frac(s.armor_pen_pct) }), m.plus(Pen { flat: s.magic_pen_flat, pct: frac(s.magic_pen_pct) }))
    })
}

/// Damage multiplier against `resist` (negative resists ignored).
pub fn mult(resist: f32, p: Pen) -> f32 {
    100.0 / (100.0 + (resist.max(0.0) * (1.0 - p.pct) - p.flat).max(0.0))
}

/// Enemies weighted by threat, with my current penetration.
pub struct Targets {
    /// (champ, armor, mr, weight).
    pub list: Vec<(ChampKey, f32, f32, f32)>,
    pub armor_pen: Pen,
    pub magic_pen: Pen,
}

impl Targets {
    /// Needs every enemy's base stats; None otherwise (scoring falls back to static answers).
    pub fn new(est: &[Option<EnemyResist>], foes: &[Foe], mine: Option<&ChampionStats>, owned: (Pen, Pen)) -> Option<Targets> {
        let list = est
            .iter()
            .map(|e| {
                let e = e.as_ref()?;
                let w = foes.iter().find(|f| f.champ.key == e.champ).map_or(NEED_W_FLOOR, |f| NEED_W_FLOOR + f.score / 100.0);
                Some((e.champ, e.armor, e.mr, w))
            })
            .collect::<Option<Vec<_>>>()?;
        if list.is_empty() {
            return None;
        }
        let (armor_pen, magic_pen) = match mine {
            Some(s) => (Pen { flat: s.lethality, pct: s.armor_pen_pct }, Pen { flat: s.magic_pen_flat, pct: s.magic_pen_pct }),
            None => owned,
        };
        Some(Targets { list, armor_pen, magic_pen })
    }

    /// Threat-weighted damage gain from adding `add` (physical: vs armor, else vs MR).
    pub fn gain(&self, physical: bool, add: Pen) -> f32 {
        let have = if physical { self.armor_pen } else { self.magic_pen };
        let (mut num, mut den) = (0.0, 0.0);
        for &(_, a, m, w) in &self.list {
            let r = if physical { a } else { m };
            num += w * (mult(r, have.plus(add)) / mult(r, have) - 1.0);
            den += w;
        }
        if den > 0.0 { num / den } else { 0.0 }
    }

    /// (armor, mr) of an enemy champion.
    pub fn of(&self, champ: ChampKey) -> Option<(f32, f32)> {
        self.list.iter().find(|t| t.0 == champ).map(|t| (t.1, t.2))
    }

    fn avg(&self, physical: bool) -> f32 {
        let den: f32 = self.list.iter().map(|t| t.3).sum();
        self.list.iter().map(|t| t.3 * if physical { t.1 } else { t.2 }).sum::<f32>() / den.max(1e-6)
    }

    /// "Their armor ~110 (Malphite ~190): % armor pen beats lethality".
    pub fn notes(&self, sd: &StaticData, my_magic: f32) -> Vec<String> {
        let mut out = vec![];
        for physical in [true, false] {
            if (physical && my_magic > 1.0 - NOTE_SHARE) || (!physical && my_magic < NOTE_SHARE) {
                continue;
            }
            let (flat, pct, what, flat_name, pct_name) = if physical {
                (REF_LETHALITY, REF_ARMOR_PEN, "armor", "lethality", "% armor pen")
            } else {
                (REF_MAGIC_FLAT, REF_MAGIC_PEN, "MR", "flat magic pen", "% magic pen")
            };
            let g_flat = self.gain(physical, Pen { flat, pct: 0.0 });
            let g_pct = self.gain(physical, Pen { flat: 0.0, pct });
            let r = |t: &(ChampKey, f32, f32, f32)| if physical { t.1 } else { t.2 };
            let top = self.list.iter().max_by(|a, b| r(a).total_cmp(&r(b)));
            let top = top.map(|t| format!(" ({} ~{:.0})", sd.champ_name(t.0), r(t))).unwrap_or_default();
            let (win, lose) = if g_pct > g_flat { (pct_name, flat_name) } else { (flat_name, pct_name) };
            out.push(format!("Their {what} ~{:.0}{top}: {win} beats {lose}", self.avg(physical)));
        }
        out
    }
}

/// Tilt the armor and MR needs toward the resist I lack. Damage taken scales
/// with 1/(1 + R/100), so a point of R is worth 1/(1 + R/100)²: with 120 armor
/// and 40 MR, MR is worth about twice as much. Unchanged without live stats.
pub fn tilt_needs(p: &mut [f32; N], mine: Option<&ChampionStats>) {
    let Some(s) = mine else { return };
    let worth = |r: f32| (1.0 + r.max(0.0) / 100.0).powi(-2);
    let (a, m) = (worth(s.armor), worth(s.magic_resist));
    let k = |x: f32| (2.0 * x / (a + m)).clamp(RESIST_TILT_MIN, RESIST_TILT_MAX);
    p[ARMOR] *= k(a);
    p[MR] *= k(m);
}

/// My EHP split and the threat-weighted enemy damage mix.
pub fn defense(mine: Option<&ChampionStats>, foes: &[Foe], est: Vec<Option<EnemyResist>>, notes: Vec<String>) -> Option<Defense> {
    if foes.is_empty() {
        return None;
    }
    let (mut w, mut magic, mut tru) = (0.0, 0.0, 0.0);
    for f in foes {
        let fw = NEED_W_FLOOR + f.score / 100.0;
        let t = f.champ.traits.true_damage * crate::consts::TRUE_FRAC;
        w += fw;
        magic += fw * f.magic * (1.0 - t);
        tru += fw * t;
    }
    let ehp = |r: f32| mine.map(|s| s.max_health * (1.0 + r.max(0.0) / 100.0));
    Some(Defense {
        ehp_physical: mine.and_then(|s| ehp(s.armor)),
        ehp_magic: mine.and_then(|s| ehp(s.magic_resist)),
        enemy_magic: magic / w,
        enemy_true: tru / w,
        enemies: est.into_iter().flatten().collect(),
        pen_notes: notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_tilt_toward_the_missing_resist() {
        let tilted = |armor, magic_resist| {
            let mut p = [0.5; N];
            tilt_needs(&mut p, Some(&ChampionStats { armor, magic_resist, ..Default::default() }));
            (p[ARMOR], p[MR])
        };
        let (a, m) = tilted(120.0, 40.0);
        assert!(a < 0.4 && m > 0.6, "armor {a} mr {m}");
        assert_eq!(tilted(60.0, 60.0), (0.5, 0.5));
        let mut p = [0.5; N];
        tilt_needs(&mut p, None);
        assert_eq!(p, [0.5; N]);
    }

    #[test]
    fn growth_formula_matches_data_dragon() {
        // Malphite armor 37 + 4.95/level: level 18 ≈ 37 + 4.95 × 17 × 1.0 = 121.15.
        assert!((grow(37.0, 4.95, 18) - 121.15).abs() < 0.01);
        assert_eq!(grow(37.0, 4.95, 1), 37.0);
    }

    #[test]
    fn percent_pen_wins_against_high_armor_lethality_against_low() {
        let t = |armor: f32| Targets { list: vec![(1, armor, 30.0, 1.0)], armor_pen: Pen::default(), magic_pen: Pen::default() };
        let leth = Pen { flat: REF_LETHALITY, pct: 0.0 };
        let pct = Pen { flat: 0.0, pct: REF_ARMOR_PEN };
        assert!(t(40.0).gain(true, leth) > t(40.0).gain(true, pct));
        assert!(t(200.0).gain(true, pct) > t(200.0).gain(true, leth));
        // Already holding % pen: a second source is worth less.
        let mut held = t(200.0);
        held.armor_pen = pct;
        assert!(held.gain(true, pct) < t(200.0).gain(true, pct));
    }
}
