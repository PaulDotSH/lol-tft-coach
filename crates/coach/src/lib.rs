//! All decisions. Pure functions over `model` types: no I/O, no clock.

use model::*;

/// Inputs for one in-game advice tick.
pub struct GameInput<'a> {
    pub snap: &'a GameSnapshot,
    pub sd: &'a StaticData,
    /// My champion's stats (lane / mode already applied). None if lolalytics is down.
    pub stats: Option<&'a ChampStats>,
    /// Mayhem: my champion's augment ranking + names.
    pub augments: Option<(&'a ChampAugments, &'a MayhemData)>,
    /// Augments currently offered on screen (OCR), possibly empty.
    pub offered: &'a [AugmentId],
}

pub fn game_advice(input: &GameInput) -> GameAdvice {
    let _ = input;
    todo!()
}

pub struct ChampSelectInput<'a> {
    pub cs: &'a ChampSelect,
    pub sd: &'a StaticData,
    /// Stats for my hovered/picked champion in my lane (build plan, bans).
    pub mine: Option<&'a ChampStats>,
    /// Stats for each revealed enemy, in their likely lane. Their `counters`
    /// give their win rate vs every champ, so ours ≈ 100 − theirs.
    pub enemies: &'a [ChampStats],
    /// lolalytics tier list for my lane (pick candidates, meta bans).
    pub tier_list: &'a [TierEntry],
    pub mayhem: Option<&'a MayhemData>,
}

pub fn champ_select_advice(input: &ChampSelectInput) -> ChampSelectAdvice {
    let _ = input;
    todo!()
}

pub fn recap(eog: &EogStats, benchmarks: &Benchmarks, recommended: &[ItemId], sd: &StaticData) -> Recap {
    let _ = (eog, benchmarks, recommended, sd);
    todo!()
}
