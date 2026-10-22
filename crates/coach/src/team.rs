//! Team composition in ARAM / Mayhem champ select (enemies are hidden, so only
//! my team counts) and the bench ranking: Mayhem tier or ARAM win rate plus
//! how well each option fills my team's gaps, every term given as a reason.

use crate::{ChampSelectInput, champ_select::pct, common::*, consts::*, draft};
use model::*;

/// My team (me first) as a composition breakdown.
pub fn side(inp: &ChampSelectInput, mine: Option<ChampKey>) -> Option<DraftSide> {
    let champs: Vec<ChampKey> =
        mine.into_iter().chain(inp.cs.allies.iter().copied().filter(|&a| Some(a) != mine)).collect();
    (!champs.is_empty()).then(|| draft::comp_side(inp.sd, &champs))
}

/// u.gg tier → win-rate points: S = +3, A = +1.5, B = 0 …; "+"/"−" move a third of a letter.
fn tier_points(t: &str) -> Option<f32> {
    let r = tier_rank(t);
    (r < 100).then(|| {
        let letter = (r + 5).div_euclid(10);
        let ord = letter as f32 + (r - letter * 10) as f32 / 3.0;
        MAYHEM_TIER_STEP * (2.0 - ord)
    })
}

/// Me + bench, each scored as my champion next to my current allies.
pub fn bench(inp: &ChampSelectInput, mine: Option<ChampKey>) -> Vec<BenchRank> {
    let (cs, sd) = (inp.cs, inp.sd);
    let mut list: Vec<ChampKey> = Vec::with_capacity(cs.bench.len() + 1);
    for c in mine.into_iter().chain(cs.bench.iter().copied()) {
        if !list.contains(&c) {
            list.push(c);
        }
    }
    let allies = draft::comp(sd, cs.allies.iter().copied().filter(|&a| !list.contains(&a)));
    let shrunk = |t: &TierEntry| shrink(pct(t.win_rate), t.games, ARAM_SHRINK_K);
    let mean = {
        let n = inp.tier_list.len().max(1) as f32;
        inp.tier_list.iter().map(shrunk).sum::<f32>() / n
    };
    let mut v: Vec<BenchRank> = list
        .into_iter()
        .map(|c| {
            let entry = inp.tier_list.iter().find(|t| t.champ == c);
            let ugg = match (cs.mode, inp.mayhem) {
                (Mode::Mayhem, Some(m)) => m.champ_tiers.get(&c).cloned(),
                _ => None,
            };
            let base = match (&ugg, entry) {
                (Some(t), _) => tier_points(t).map(|p| (p, format!("Mayhem tier {t}"))),
                (None, Some(e)) => Some((shrunk(e) - mean, format!("{:.1}% ARAM win rate", pct(e.win_rate)))),
                _ => None,
            };
            let fills = sd.champs.get(&c).map_or(vec![], |ch| draft::fills(&allies, ch));
            let score = base.as_ref().map(|b| b.0 + fills.iter().map(|f| f.0).sum::<f32>());
            BenchRank {
                champ: c,
                tier: ugg.or_else(|| entry.and_then(|e| e.tier.clone())),
                mine: Some(c) == mine,
                win_rate: entry.map(|e| pct(e.win_rate)),
                score,
                reasons: base.into_iter().chain(fills).map(|x| x.1).collect(),
            }
        })
        .collect();
    // Stable: options without data keep their order, after the rest.
    v.sort_by(|a, b| match (a.score, b.score) {
        (Some(x), Some(y)) => y.total_cmp(&x),
        (x, y) => y.is_some().cmp(&x.is_some()),
    });
    v
}

#[cfg(test)]
mod tests {
    use super::tier_points;

    #[test]
    fn tiers_to_points() {
        let p = |t| tier_points(t).unwrap();
        assert_eq!((p("S"), p("A"), p("B"), p("D")), (3.0, 1.5, 0.0, -3.0));
        assert!(p("S+") > p("S") && p("S") > p("S-") && p("S-") > p("A+"));
        assert_eq!(tier_points("?"), None);
    }
}
