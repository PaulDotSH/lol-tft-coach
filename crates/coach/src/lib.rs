//! All decisions. Pure functions over `model` types: no I/O, no clock.

mod analysis;
mod augment_tags;
mod augments;
pub mod brief;
mod calls;
mod champ_select;
mod common;
pub mod consts;
mod gold;
mod draft;
mod enemy_augments;
mod fights;
mod gank;
mod items;
pub mod jungle;
mod lane_matchup;
mod laning;
mod macro_coach;
mod late;
mod matchups;
mod memory;
mod modes;
mod needs;
mod picks;
mod plan;
mod recap;
mod resists;
mod runes;
mod self_stats;
mod ranked;
mod selfcoach;
mod session;
pub mod sight;
mod shop;
pub mod skills;
mod spikes;
mod team;
mod threat;
mod tips;
mod timers;
mod tracker;
mod trends;
mod whatif;
mod windows;

pub use analysis::{analyze_match, compare_past};
pub use augments::offer_places;
pub use lane_matchup::{CardData, PlanInput as MatchupInput, opposing as opposing_lanes, plan as matchup_plan};
pub use gold::{GoldHistory, HISTORY_MAX, SAMPLE_STEP};
pub use macro_coach::MAX_PATTERNS;
pub use memory::matchup_memory;
pub use selfcoach::{GameLog, HOLDING_GOLD, Now, SPIKE_LEVELS, death_review, goal_applies, goal_progress, summary};
pub use ranked::{ladder, rank_trends};
pub use trends::trends;
pub use picks::AugmentTracker;
pub use plan::{Curve, PlanInput as GamePlanInput, game_plan, roster_draft};
pub use gank::{GankEnemies, GankTracker, MapView, Seen, SightLog, Sighting};
use model::*;
pub use shop::{eta as gold_eta, shopping};
pub use runes::VsRunes;
pub use threat::lane_opponent;
pub use tracker::Tracker;

pub use draft::{DraftData, Lineup, lineups as draft_lineups};
pub use enemy_augments::{EnemyAugmentInput, expected as expected_augments, order_candidates, stale_enemies};

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
    /// Mayhem: augments taken so far (`AugmentTracker::picks`).
    pub my_augments: &'a [MyAugment],
    /// Mayhem: enemy augments read off the Tab scoreboard.
    pub enemy_augments: EnemyAugmentInput<'a>,
    /// Last tick's Buy next: kept unless something clearly beats it.
    pub keep: Option<ItemId>,
    /// Rift: lolalytics matchup page against my lane opponent (its skill order).
    pub matchup: Option<&'a MatchupBuild>,
}

pub fn game_advice(input: &GameInput) -> GameAdvice {
    let GameInput { snap, sd, stats, augments, offered, gold_history, my_augments, enemy_augments, keep, matchup } = *input;
    let me = snap.players.get(snap.me);
    let my_champ = me.map_or(0, |p| p.champ);
    let owned = me.map_or(&[][..], |p| &p.items);
    let rules = modes::rules(snap.mode);
    let mut a = threat::assess(snap, sd, stats);
    let aug_tags = enemy_augments::apply(&mut a, &enemy_augments, augments.map(|(_, m)| &m.augment_names));
    let mut needs = needs::needs(&a.foes);
    enemy_augments::adjust_needs(&mut needs, &a, &aug_tags);
    rules.scale_needs(&mut needs.p);
    resists::tilt_needs(&mut needs.p, snap.my_stats.as_ref());
    let mut ctx = items::Ctx::new(sd, stats, common::champ(sd, my_champ), snap.mode);
    ctx.keep = keep;
    ctx.ahead = me.is_some_and(|p| {
        p.kills as f32 + p.assists as f32 / 2.0 >= consts::AHEAD_KDA * (p.deaths as f32 + 1.0)
    });
    ctx.live = snap.my_stats.as_ref();
    ctx.support = snap.mode == Mode::Rift
        && (me.is_some_and(|p| p.position == Some(Lane::Support))
            || owned.iter().any(|i| consts::SUPPORT_QUEST.contains(i) || consts::SUPPORT_UPGRADES.contains(i)));
    ctx.aug = augment_tags::AugEffects::new(my_augments);
    if ctx.aug.find(augment_tags::Tag::Tenacity).is_some() {
        needs.p[modes::TENACITY] *= augment_tags::AUG_TENACITY_NEED;
    }
    let est = resists::enemies(snap, sd, rules);
    ctx.pen = resists::Targets::new(&est, &a.foes, snap.my_stats.as_ref(), resists::item_pen(sd, owned));
    let notes = ctx.pen.as_ref().map_or(vec![], |t| t.notes(sd, ctx.champ.traits.magic_share));
    let lane_opp = a.lane_opp.and_then(|i| snap.players.get(i)).map(|p| p.champ);
    ctx.sim = whatif::Sim::new(&whatif::SimInput { snap, sd, stats, champ: ctx.champ, est: &est, foes: &a.foes, lane_opp, rules });
    let b = if me.is_some() { items::build(&ctx, owned, &needs) } else { items::Build { path: vec![], alts: vec![] } };
    let finished = owned.iter().filter(|i| sd.items.get(i).is_some_and(|i| i.completed)).count();
    let gold_sinks = match me {
        Some(p) if b.path.is_empty() && finished + ctx.support as usize >= consts::MAX_ITEMS.saturating_sub(ctx.aug.keep_slots()) => {
            late::gold_sinks(&ctx, owned, &needs, &a.foes, p.level, snap.mode)
        }
        _ => vec![],
    };
    let gold = Some((snap.my_gold, owned));
    let pick = |s: &items::Scored, g| items::to_pick(&ctx, s, &needs, &a.foes, g);
    let me_stats = self_stats::self_stats(snap, sd, stats, a.lane_opp);
    let my_lane = stats.and_then(|s| s.lane);
    let gold_diff = gold::gold_diff(snap, sd, a.lane_opp, my_lane, me_stats.item_gold_vs_opp, gold_history);
    let path_ids: Vec<ItemId> = b.path.iter().map(|s| s.id).collect();
    let what_if = ctx.sim.as_ref().map_or(vec![], |s| b.path.first().into_iter().chain(&b.alts).filter_map(|x| sd.items.get(&x.id)).map(|it| s.eval(it)).collect());
    GameAdvice {
        time: snap.time,
        mode: snap.mode,
        my_champ,
        buy_next: b.path.first().map(|s| pick(s, gold)),
        alternatives: b.alts.iter().map(|s| pick(s, gold)).collect(),
        support_upgrade: items::support_upgrade(&ctx, owned),
        path: b.path.iter().map(|s| pick(s, None)).collect(),
        build_source: common::build_source(stats),
        owned: owned.to_vec(),
        matchups: matchups::matchups(snap, stats, a.lane_opp),
        me: me_stats,
        timers: timers::timers(snap),
        augments: augments::advice(sd, my_champ, augments, offered, owned),
        defense: resists::defense(snap.my_stats.as_ref(), &a.foes, est, notes),
        threats: a.threats,
        gold: gold_diff,
        shopping: me.map_or_else(ShoppingList::default, |p| shop::shopping(sd, snap.mode, p, snap.my_gold, &path_ids)),
        gold_eta: me.filter(|_| !path_ids.is_empty()).map(|p| shop::eta(sd, snap.mode, p, snap.my_gold, &path_ids, None)),
        deaths: windows::deaths(snap),
        my_augments: my_augments.to_vec(),
        gold_sinks,
        tab_hint: enemy_augments::tab_hint(snap, enemy_augments.seen, enemy_augments.stale_since),
        bounties: vec![],
        readiness: None,
        calls: vec![],
        tips: vec![],
        matchup: vec![],
        skill: skills::advice(snap, sd, stats, matchup),
        what_if,
        jungle: None,
    }
}

/// State the orchestrator keeps or fetches across ticks, for [`live_extras`].
pub struct LiveInput<'a> {
    pub snap: &'a GameSnapshot,
    pub sd: &'a StaticData,
    pub tracker: &'a Tracker,
    /// Data Dragon R cooldowns of champions in this game, as fetched so far.
    pub ults: &'a std::collections::HashMap<ChampKey, UltCooldowns>,
    /// Laning panel: notes, Data Dragon spells of my lane opponents, my record against the enemies.
    pub notes: &'a LaneNotes,
    pub spells: &'a std::collections::HashMap<ChampKey, Vec<SpellInfo>>,
    pub memory: &'a [MatchupMemory],
}

/// Second pass over [`game_advice`]: gold ETA from tracked income, enemy R ranks,
/// newly finished items, calls, quiet tips and the laning matchup panel.
pub fn live_extras(advice: &mut GameAdvice, input: &LiveInput) {
    let LiveInput { snap, sd, tracker, ults, notes, spells, memory } = *input;
    if let (Some(me), Some(_)) = (snap.players.get(snap.me), &advice.gold_eta) {
        let path: Vec<ItemId> = advice.path.iter().map(|p| p.item).collect();
        advice.gold_eta = Some(shop::eta(sd, snap.mode, me, snap.my_gold, &path, tracker.income()));
    }
    spikes::annotate(&mut advice.threats, snap, tracker, ults);
    calls::annotate(advice, snap, sd);
    tips::annotate(advice, snap, sd, tracker);
    jungle::annotate(advice, snap, sd, tracker);
    let me = snap.players.get(snap.me).map_or(0, |p| p.champ);
    advice.matchup = lane_matchup::laning(advice, snap, &CardData { sd, notes, me, spells, memory });
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

/// `champ_select_advice`'s rune pages with the pages against my lane opponent added.
pub fn champ_select_runes(input: &ChampSelectInput, vs: &VsRunes) -> Option<RuneAdvice> {
    champ_select::rune_pages(input, Some(vs))
}

pub fn recap(eog: &EogStats, benchmarks: &Benchmarks, recommended: &[ItemId], sd: &StaticData) -> Recap {
    recap::recap(eog, benchmarks, recommended, sd)
}

/// Loss patterns over my recent games; `bench` holds lolalytics averages per (mode, champion).
pub fn macro_report(games: &[PastGame], sd: &StaticData, bench: &std::collections::HashMap<(Mode, ChampKey), Benchmarks>) -> MacroReport {
    macro_coach::report(games, sd, bench)
}
