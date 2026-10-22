//! Full build: what else gold buys (Stat Anvils, elixirs, selling boots, a slot swap).

use crate::{
    augment_tags::Tag,
    common::*,
    items::{Ctx, Decision, Load, Scored, score_all, to_pick},
    modes::TENACITY,
    needs::{Needs, decay},
    threat::Foe,
};
use model::*;

/// A swap must beat the weakest owned item by this much (win-rate units).
const SWAP_MIN: f32 = 0.03;
/// Completed items sell for this share of their cost.
const SELL_SHARE: f32 = 0.7;
/// Tenacity need above which Elixir of Iron is the elixir.
const IRON_TENACITY: f32 = 0.5;
const IRON_TANK: f32 = 0.6;

pub fn gold_sinks(ctx: &Ctx, owned: &[ItemId], needs: &Needs, foes: &[Foe], level: u32, mode: Mode) -> Vec<GoldSink> {
    let sd = ctx.sd;
    let r = ctx.rules;
    let mut out = vec![];
    if let Some(a) = &r.stat_anvil
        && level >= a.level
    {
        out.push(GoldSink {
            kind: "anvil".into(),
            item: Some(a.item),
            sell: None,
            cost: a.cost,
            reason: "Permanent stats, no item slot (a free Cappa Juice with each)".into(),
        });
    }
    if level >= r.elixir_level
        && let Some(e) = elixir(ctx, needs).and_then(|id| sd.items.get(&id)).filter(|i| map_ok(i, mode))
    {
        out.push(GoldSink {
            kind: "elixir".into(),
            item: Some(e.id),
            sell: None,
            cost: e.cost,
            reason: format!("{}: temporary stats for the next fights, one elixir at a time", e.name),
        });
    }

    let fin = |id: &ItemId| sd.items.get(id).filter(|i| i.completed);
    let boots = owned.iter().filter_map(fin).find(|i| i.boots);
    let sell = |it: &Item| (SELL_SHARE * it.cost as f32).round() as u32;
    let mut buf = vec![];
    if let (Some(b), Some(src)) = (boots, ctx.aug.find(Tag::MoveSpeed))
        && let Some(top) = best(ctx, owned, needs, &mut buf).first()
        && let Some(it) = sd.items.get(&top.id)
    {
        out.push(GoldSink {
            kind: "sell_boots".into(),
            item: Some(it.id),
            sell: Some(b.id),
            cost: it.cost.saturating_sub(sell(b)),
            reason: format!("{} covers move speed: sell {} for {}", src.name, b.name, it.name),
        });
    }

    // Weakest owned legendary vs the best item not owned, both scored against today's threats.
    let mut swap: Option<(f32, Scored, &Item)> = None;
    for o in owned.iter().filter_map(fin).filter(|i| !i.boots) {
        let rest: Vec<ItemId> = owned.iter().copied().filter(|&i| i != o.id).collect();
        best(ctx, &rest, needs, &mut buf);
        let Some(mine) = buf.iter().find(|s| s.id == o.id) else { continue };
        let Some(top) = buf.iter().find(|s| !owned.contains(&s.id)) else { continue };
        let gap = top.score - mine.score;
        if gap >= SWAP_MIN && swap.as_ref().is_none_or(|s| gap > s.0) {
            swap = Some((gap, *top, o));
        }
    }
    if let Some((_, top, o)) = swap {
        let p = to_pick(ctx, &top, needs, foes, None);
        out.push(GoldSink {
            kind: "swap".into(),
            item: Some(top.id),
            sell: Some(o.id),
            cost: p.cost.saturating_sub(sell(o)),
            reason: format!("{} over {} · {}", p.name, o.name, p.reason),
        });
    }
    out
}

/// Legendaries ranked for one more slot, with needs answered by `have` decayed.
fn best<'b>(ctx: &Ctx, have: &[ItemId], needs: &Needs, buf: &'b mut Vec<Scored>) -> &'b [Scored] {
    let mut need = needs.p;
    for it in have.iter().filter_map(|i| ctx.sd.items.get(i)) {
        decay(&mut need, &it.answers);
    }
    score_all(ctx, Decision::Slot(5), have, &need, Load::new(ctx, have), buf);
    buf
}

/// Iron for tanks or heavy CC, Sorcery for AP, Wrath for AD.
fn elixir(ctx: &Ctx, needs: &Needs) -> Option<ItemId> {
    let pick = |name: &str| ctx.rules.elixirs.iter().copied().find(|id| ctx.sd.items.get(id).is_some_and(|i| i.name.ends_with(name)));
    let c = ctx.champ;
    if c.classes.contains(&ChampClass::Tank) || c.traits.tank >= IRON_TANK || needs.p[TENACITY] >= IRON_TENACITY {
        pick("Iron")
    } else if c.traits.magic_share >= 0.5 {
        pick("Sorcery")
    } else {
        pick("Wrath")
    }
}
