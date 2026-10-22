//! All decisions. Pure functions over `model` types: no I/O, no clock.

mod augments;
mod champ_select;
mod common;
pub mod consts;
mod gold;
mod draft;
mod items;
mod macro_coach;
mod matchups;
mod needs;
mod recap;
mod self_stats;
mod selfcoach;
mod shop;
mod spikes;
mod team;
mod threat;
mod timers;
mod tracker;
mod trends;
mod windows;

pub use gold::{GoldHistory, HISTORY_MAX, SAMPLE_STEP};
pub use macro_coach::MAX_PATTERNS;
pub use selfcoach::{GameLog, HOLDING_GOLD, Now, SPIKE_LEVELS, death_review, goal_applies, goal_progress, summary};
pub use trends::trends;
use model::*;
pub use shop::{eta as gold_eta, shopping};
pub use tracker::Tracker;

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
    /// Item-gold diff samples so far this game (`GoldHistory::samples`).
    pub gold_history: &'a [GoldSample],
}

pub fn game_advice(input: &GameInput) -> GameAdvice {
    let GameInput { snap, sd, stats, augments, offered, gold_history } = *input;
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
    let me_stats = self_stats::self_stats(snap, sd, stats, a.lane_opp);
    let my_lane = stats.and_then(|s| s.lane);
    let gold_diff = gold::gold_diff(snap, sd, a.lane_opp, my_lane, me_stats.item_gold_vs_opp, gold_history);
    let path_ids: Vec<ItemId> = b.path.iter().map(|s| s.id).collect();
    GameAdvice {
        time: snap.time,
        mode: snap.mode,
        my_champ,
        buy_next: b.path.first().map(|s| pick(s, gold)),
        alternatives: b.alts.iter().map(|s| pick(s, gold)).collect(),
        path: b.path.iter().map(|s| pick(s, None)).collect(),
        owned: owned.to_vec(),
        matchups: matchups::matchups(snap, stats, a.lane_opp),
        me: me_stats,
        timers: timers::timers(snap),
        augments: augments::advice(sd, my_champ, augments, offered),
        threats: a.threats,
        gold: gold_diff,
        shopping: me.map_or_else(ShoppingList::default, |p| shop::shopping(sd, snap.mode, p, snap.my_gold, &path_ids)),
        gold_eta: me.filter(|_| !path_ids.is_empty()).map(|p| shop::eta(sd, snap.mode, p, snap.my_gold, &path_ids, None)),
        deaths: windows::deaths(snap),
    }
}

/// State the orchestrator keeps or fetches across ticks, for [`live_extras`].
pub struct LiveInput<'a> {
    pub snap: &'a GameSnapshot,
    pub sd: &'a StaticData,
    pub tracker: &'a Tracker,
    /// Data Dragon R cooldowns of champions in this game, as fetched so far.
    pub ults: &'a std::collections::HashMap<ChampKey, UltCooldowns>,
}

/// Second pass over [`game_advice`]: gold ETA from tracked income, enemy R ranks
/// and newly finished items.
pub fn live_extras(advice: &mut GameAdvice, input: &LiveInput) {
    let LiveInput { snap, sd, tracker, ults } = *input;
    if let (Some(me), Some(_)) = (snap.players.get(snap.me), &advice.gold_eta) {
        let path: Vec<ItemId> = advice.path.iter().map(|p| p.item).collect();
        advice.gold_eta = Some(shop::eta(sd, snap.mode, me, snap.my_gold, &path, tracker.income()));
    }
    spikes::annotate(&mut advice.threats, snap, tracker, ults);
}

pub struct ChampSelectInput<'a> {
    pub cs: &'a ChampSelect,
    pub sd: &'a StaticData,
    /// Stats for my hovered/picked champion in my lane (build plan, bans).
    pub mine: Option<&'a ChampStats>,
    /// Stats for each revealed enemy, in their likely lane. Their `counters`
    /// give their win rate vs every champ, so ours ≈ 100 − theirs.
    pub enemies: &'a [ChampStats],
    /// lolalytics tier list for my lane (pick candidates, meta bans); in
    /// ARAM / Mayhem the ARAM list for all champions (bench).
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

/// Loss patterns over my recent games; `bench` holds lolalytics averages per (mode, champion).
pub fn macro_report(games: &[PastGame], sd: &StaticData, bench: &std::collections::HashMap<(Mode, ChampKey), Benchmarks>) -> MacroReport {
    macro_coach::report(games, sd, bench)
}
