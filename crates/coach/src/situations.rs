//! Coachless situations: which of its game situations the live game is in, and an item's term
//! from its `GetItemDetailed` WPA there.

use crate::{
    common::shrink,
    consts::*,
    items::Standing,
    threat::{self, Foe},
};
use model::*;

/// Enemy team: threat-weighted magic share, mean base attack range, mean tankiness, mean CC trait.
pub(crate) fn measures(foes: &[Foe]) -> Option<[f32; 4]> {
    if foes.is_empty() {
        return None;
    }
    let n = foes.len() as f32;
    let w = |f: &Foe| NEED_W_FLOOR + f.score / 100.0;
    let magic = foes.iter().map(|f| w(f) * f.magic).sum::<f32>() / foes.iter().map(w).sum::<f32>();
    let mean = |g: fn(&Foe) -> f32| foes.iter().map(g).sum::<f32>() / n;
    Some([magic, mean(|f| f.champ.base.attack_range), mean(|f| (f.armor_tank() + f.mr_tank()) / 2.0), mean(|f| f.champ.traits.cc)])
}

/// [`measures`] of my snapshot's enemy team, to check the cut-offs against recorded games.
pub fn enemy_measures(snap: &GameSnapshot, sd: &StaticData) -> Option<[f32; 4]> {
    measures(&threat::assess(snap, sd, None).foes)
}

/// One situation per group in Coachless' order; None where the game does not say (no enemies,
/// no win probability).
pub(crate) fn live(foes: &[Foe], standing: Option<Standing>) -> [Option<Situation>; 5] {
    use Situation::*;
    let cut = |x: f32, c: [f32; 2], s: [Situation; 3]| if x < c[0] { s[0] } else if x > c[1] { s[2] } else { s[1] };
    let m = measures(foes);
    [
        m.map(|m| cut(m[0], SIT_MAGIC, [PhysicalDamage, BalancedDamage, MagicDamage])),
        m.map(|m| cut(m[1], SIT_RANGE, [LowRange, NormalRange, HighRange])),
        m.map(|m| cut(m[2], SIT_TANK, [Squishy, NormalTankiness, Tanky])),
        m.map(|m| cut(m[3], SIT_CC, [LowCc, NormalCc, HighCc])),
        standing.map(|s| match s {
            Standing::Behind => Behind,
            Standing::Even => Even,
            Standing::Ahead => Ahead,
        }),
    ]
}

/// Σ over the live situations of (WPA there − WPA overall), shrunk by games, in win-rate units,
/// and the situation with the largest part. None when no live situation has the games.
pub(crate) fn term(s: &ItemSituations, now: &[Option<Situation>; 5]) -> Option<(f32, Situation, f32)> {
    let parts: Vec<(Situation, f32)> = s
        .by
        .iter()
        .filter(|(w, _, g)| now.contains(&Some(*w)) && *g >= SIT_MIN_GAMES)
        .map(|&(w, wpa, g)| (w, (shrink(50.0 + wpa - s.overall, g as u32, SIT_SHRINK_K) - 50.0) / 100.0))
        .collect();
    let top = parts.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1))?;
    Some((parts.iter().map(|p| p.1).sum(), top.0, top.1))
}

/// "better from behind (Coachless)" reads "better " + this.
pub(crate) fn phrase(s: Situation) -> &'static str {
    use Situation::*;
    match s {
        PhysicalDamage => "into physical damage",
        BalancedDamage => "into mixed damage",
        MagicDamage => "into magic damage",
        LowRange => "into short range",
        NormalRange => "into average range",
        HighRange => "into long range",
        Squishy => "into squishy teams",
        NormalTankiness => "into average tankiness",
        Tanky => "into tanky teams",
        LowCc => "into little CC",
        NormalCc => "into average CC",
        HighCc => "into heavy CC",
        Behind => "from behind",
        Even => "in even games",
        Ahead => "from ahead",
    }
}
