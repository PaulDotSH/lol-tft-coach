//! ARAM: Mayhem augment offers.

use model::*;

pub fn advice(
    sd: &StaticData,
    my_champ: ChampKey,
    data: Option<(&ChampAugments, &MayhemData)>,
    offered: &[AugmentId],
) -> Option<AugmentAdvice> {
    if offered.is_empty() {
        return None;
    }
    let ranks: Vec<AugmentRank> = offered
        .iter()
        .map(|&id| {
            let name = data
                .and_then(|(_, m)| m.augment_names.get(&id).cloned())
                .unwrap_or_else(|| format!("#{id}"));
            let found = data.and_then(|(a, _)| {
                [(Rarity::Prismatic, &a.prismatic), (Rarity::Gold, &a.gold), (Rarity::Silver, &a.silver)]
                    .into_iter()
                    .find_map(|(r, l)| l.iter().position(|&x| x == id).map(|p| (r, p as u32, l.len() as u32)))
            });
            match found {
                Some((r, p, of)) => AugmentRank { id, name, rarity: Some(r), rank: Some(p), of },
                None => AugmentRank { id, name, rarity: None, rank: None, of: 0 },
            }
        })
        .collect();
    let best = ranks
        .iter()
        .filter_map(|r| Some((r, r.rank? as f32 / r.of.max(1) as f32)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(r, _)| r);
    let reason = match best {
        Some(r) => format!(
            "#{} of {} {:?} for {}",
            r.rank.unwrap_or(0) + 1,
            r.of,
            r.rarity.unwrap_or(Rarity::Silver),
            sd.champ_name(my_champ)
        ),
        None => "no ranking for these augments".into(),
    };
    Some(AugmentAdvice { best: best.map(|r| r.id), offered: ranks, reason })
}
