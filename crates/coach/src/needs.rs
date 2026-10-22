//! What the enemy team demands, weighted by threat.

use crate::{common::*, consts::*, threat::Foe};
use model::*;

pub struct Needs {
    /// Profile fields in `common::arr` order, each 0..=1.
    pub p: [f32; N],
    /// Index into the foe list of the enemy driving each need.
    pub driver: [Option<usize>; N],
}

struct Agg {
    mean: f32,
    peak: f32,
    driver: Option<usize>,
}

fn agg<'a>(foes: &[Foe<'a>], f: impl Fn(&Foe<'a>) -> f32) -> Agg {
    let (mut sw, mut s, mut peak, mut best, mut driver) = (0.0, 0.0, 0.0f32, 0.0, None);
    for (i, e) in foes.iter().enumerate() {
        let (v, w) = (f(e), NEED_W_FLOOR + e.score / 100.0);
        sw += w;
        s += w * v;
        peak = peak.max(v * e.score / 100.0);
        if v * w > best {
            best = v * w;
            driver = Some(i);
        }
    }
    Agg { mean: if sw > 0.0 { s / sw } else { 0.0 }, peak, driver }
}

fn mix(a: &Agg, mean_w: f32) -> f32 {
    mean_w * a.mean + (1.0 - mean_w) * a.peak
}

pub fn needs(foes: &[Foe]) -> Needs {
    let mut n = Needs { p: [0.0; N], driver: [None; N] };
    if foes.is_empty() {
        return n;
    }
    let tf = |e: &Foe| e.champ.traits.true_damage * TRUE_FRAC;
    let top = foes.iter().map(|e| e.score).fold(0.0, f32::max);
    let intensity = (top / INTENSITY_FULL).clamp(INTENSITY_MIN, 1.0);
    let ramp = |x: f32| clamp01((x - SHARE_LO) / (SHARE_HI - SHARE_LO)) * intensity;

    let phys = agg(foes, |e| (1.0 - e.magic) * (1.0 - tf(e)));
    let magic = agg(foes, |e| e.magic * (1.0 - tf(e)));
    let heal = agg(foes, |e| e.champ.traits.heals);
    let cc = agg(foes, |e| e.champ.traits.cc);
    let burst = agg(foes, Foe::burst);
    let at = agg(foes, Foe::armor_tank);
    let mt = agg(foes, Foe::mr_tank);
    let crit = agg(foes, Foe::crit);
    let aspd = agg(foes, Foe::attack_speed);
    let top_i = foes.iter().enumerate().max_by(|a, b| a.1.score.total_cmp(&b.1.score)).map(|(i, _)| i);

    let cc_v = mix(&cc, MEAN_W_CC);
    let vals = [
        (ramp(phys.mean), phys.driver),
        (ramp(magic.mean), magic.driver),
        (HP_NEED * intensity, top_i),
        (clamp01(mix(&heal, MEAN_W) / HEAL_FULL), heal.driver),
        (clamp01(cc_v / CC_FULL), cc.driver),
        (clamp01((cc_v - CLEANSE_LO) / CLEANSE_SPAN), cc.driver),
        // One fed assassin is enough: use the worst single enemy.
        (clamp01((burst.peak - BURST_LO) / BURST_SPAN), burst.driver),
        (clamp01(mix(&at, MEAN_W) / TANK_FULL), at.driver),
        (clamp01(mix(&mt, MEAN_W) / TANK_FULL), mt.driver),
        (clamp01(mix(&crit, MEAN_W) / CRIT_NEED_FULL), crit.driver),
        (clamp01(mix(&aspd, MEAN_W) / AS_NEED_FULL), aspd.driver),
    ];
    for (i, (v, d)) in vals.into_iter().enumerate() {
        n.p[i] = v;
        n.driver[i] = if v > 0.0 { d } else { None };
    }
    n
}

/// An answered need decays.
pub fn decay(p: &mut [f32; N], answers: &Profile) {
    for (v, a) in p.iter_mut().zip(arr(answers)) {
        *v *= 1.0 - NEED_DECAY * clamp01(a);
    }
}
