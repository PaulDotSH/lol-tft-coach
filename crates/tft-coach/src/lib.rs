//! Pure TFT coaching: owned units from screen snapshots, automatic target comp,
//! shop / econ / item / augment advice, calls, post-game review and trends.
//! No I/O: every input is a value. Limited-time modes plug in through `ModeHooks`.

mod augments;
pub mod board;
mod calls;
mod econ;
pub mod fit;
mod history;
mod items;
pub mod library;
mod shop;
pub mod tracker;

pub use econ::{LEVEL_AT, levelling};
pub use history::{match_comp, pregame, review, trends};
pub use library::Library;
pub use tracker::Tracker;
use tft_model::*;

/// Rules of a mode on top of the standard coach. The app picks one by queue.
pub trait ModeHooks {
    fn mode(&self) -> TftMode;
    /// Added to every comp's fit score (e.g. comps using a mode's free unit).
    fn comp_bonus(&self, _comp: &MetaComp, _input: &CoachInput) -> f32 {
        0.0
    }
    /// Last pass over the finished advice.
    fn adjust(&self, _advice: &mut TftAdvice, _input: &CoachInput) {}
}

/// Ranked and normal games: the coach as is.
pub struct StandardHooks;

impl ModeHooks for StandardHooks {
    fn mode(&self) -> TftMode {
        TftMode::Standard
    }
}

/// Inputs for one advice tick. Call `tracker.observe(snap, set)` first with the same snapshot.
#[derive(Clone, Copy)]
pub struct CoachInput<'a> {
    pub snap: &'a TftSnapshot,
    pub tracker: &'a Tracker,
    pub set: &'a TftSetData,
    /// Meta for this queue (MetaTFT); empty comps give no target.
    pub meta: &'a TftMeta,
    pub hooks: &'a dyn ModeHooks,
    /// Last tick's target comp id (`advice.comps[0].comp`): kept unless another clearly beats it.
    pub keep: Option<&'a str>,
}

pub fn unit_name<'s>(set: &'s TftSetData, api: &'s str) -> &'s str {
    set.units.get(api).map_or(api, |u| u.name.as_str())
}

pub fn item_name<'s>(set: &'s TftSetData, api: &'s str) -> &'s str {
    set.items.get(api).map_or(api, |i| i.name.as_str())
}

pub fn trait_name<'s>(set: &'s TftSetData, api: &'s str) -> &'s str {
    set.traits.get(api).map_or(api, |t| t.name.as_str())
}

pub fn augment_name<'s>(set: &'s TftSetData, api: &'s str) -> &'s str {
    set.augments.get(api).map_or(api, |a| a.name.as_str())
}

/// The meta comp behind the advice's target.
pub fn target<'m>(advice: &TftAdvice, meta: &'m TftMeta) -> Option<&'m MetaComp> {
    let id = &advice.comps.first()?.comp;
    meta.comps.iter().find(|c| &c.id == id)
}

pub fn advise(input: &CoachInput) -> TftAdvice {
    let CoachInput { snap, tracker, set, meta, hooks, .. } = *input;
    let ranked = fit::rank(input);
    let comps: Vec<&MetaComp> = ranked.iter().take(fit::TOP_COMPS).map(|c| c.0).collect();
    let target = comps.first().copied();
    let mut contested: Vec<(Api, u32)> = target
        .map(|c| {
            c.units.iter().map(|u| (u.clone(), fit::contested(u, c, set, tracker.boards()))).filter(|u| u.1 > 0).collect()
        })
        .unwrap_or_default();
    contested.sort_by_key(|c| std::cmp::Reverse(c.1));
    let (econ, econ_call) = econ::advise(snap, tracker, target);
    let mut advice = TftAdvice {
        mode: hooks.mode(),
        stage: tracker.stage(),
        owned: tracker.owned(),
        comps: comps.iter().map(|c| fit::pick(c, input)).collect(),
        shop: shop::advise(snap, tracker, set, &comps),
        econ,
        items: items::advise(snap, tracker, set, target),
        augments: augments::rank(&snap.augment_offer, meta, &comps),
        contested,
        calls: vec![],
        choncc: None,
        team_size: snap.team_size.or(tracker.team_size()),
    };
    advice.calls = calls::calls(&advice, econ_call, tracker.free_slot(snap), set, target);
    hooks.adjust(&mut advice, input);
    advice
}
