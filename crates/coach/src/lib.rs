//! All decisions. Pure functions over `model` types: no I/O, no clock.

mod augments;
mod champ_select;
mod common;
pub mod consts;
mod draft;
mod items;
mod matchups;
mod needs;
mod recap;
mod self_stats;
mod threat;
mod timers;

use model::*;

pub use draft::{DraftData, Lineup, lineups as draft_lineups};

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
    let GameInput { snap, sd, stats, augments, offered } = *input;
    let me = snap.players.get(snap.me);
    let my_champ = me.map_or(0, |p| p.champ);
    let owned = me.map_or(&[][..], |p| &p.items);
    let a = threat::assess(snap, sd, stats);
    let needs = needs::needs(&a.foes);
    let mut ctx = items::Ctx::new(sd, stats, common::champ(sd, my_champ), snap.mode);
    ctx.ahead = me.is_some_and(|p| {
        p.kills as f32 + p.assists as f32 / 2.0 >= consts::AHEAD_KDA * (p.deaths as f32 + 1.0)
    });
    let b = if me.is_some() { items::build(&ctx, owned, &needs) } else { items::Build { path: vec![], alts: vec![] } };
    let gold = Some((snap.my_gold, owned));
    let pick = |s: &items::Scored, g| items::to_pick(&ctx, s, &needs, &a.foes, g);
    GameAdvice {
        time: snap.time,
        mode: snap.mode,
        my_champ,
        buy_next: b.path.first().map(|s| pick(s, gold)),
        alternatives: b.alts.iter().map(|s| pick(s, gold)).collect(),
        path: b.path.iter().map(|s| pick(s, None)).collect(),
        owned: owned.to_vec(),
        matchups: matchups::matchups(snap, stats, a.lane_opp),
        me: self_stats::self_stats(snap, sd, stats, a.lane_opp),
        timers: timers::timers(snap),
        augments: augments::advice(sd, my_champ, augments, offered),
        threats: a.threats,
    }
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
    /// Rift draft data (lane matchups, synergy); empty until fetched.
    pub draft: DraftData<'a>,
}

pub fn champ_select_advice(input: &ChampSelectInput) -> ChampSelectAdvice {
    champ_select::advice(input)
}

pub fn recap(eog: &EogStats, benchmarks: &Benchmarks, recommended: &[ItemId], sd: &StaticData) -> Recap {
    recap::recap(eog, benchmarks, recommended, sd)
}
