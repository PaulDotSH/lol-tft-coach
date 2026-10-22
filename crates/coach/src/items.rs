//! Item scoring (balanced mode), buy-next, alternatives, late-game path.

use crate::{
    common::*,
    consts::*,
    needs::{Needs, decay},
    threat::Foe,
};
use model::*;
use std::collections::HashMap;

/// [ap, ad, tank] direction of an item.
fn item_vec(it: &Item) -> [f32; 3] {
    let s = &it.stats;
    let ap = s.ap / AP_UNIT + s.magic_pen_flat / MPEN_FLAT_UNIT + frac(s.magic_pen_pct) / MPEN_PCT_UNIT;
    let ad = s.ad / AD_UNIT
        + s.lethality / LETH_UNIT
        + frac(s.crit) / CRIT_UNIT
        + frac(s.attack_speed) / AS_UNIT
        + frac(s.armor_pen_pct) / APEN_UNIT;
    let tank = (s.armor + s.mr) / RESIST_UNIT + s.hp / HP_UNIT;
    [ap, ad, tank]
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 { v.map(|x| x / l) } else { v }
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn champ_dir(c: &Champion) -> [f32; 3] {
    let t = &c.traits;
    let tank = if c.classes.contains(&ChampClass::Tank) { t.tank.max(0.8) } else { t.tank };
    norm([t.magic_share, 1.0 - t.magic_share, tank])
}

/// Build direction: my champion's traits blended with what lolalytics says it builds.
fn build_dir(sd: &StaticData, stats: Option<&ChampStats>, c: &Champion) -> [f32; 3] {
    let cd = champ_dir(c);
    let mut acc = [0f32; 3];
    for r in stats.iter().flat_map(|s| s.slots.iter().flatten()) {
        if let Some(it) = sd.items.get(&r.id).filter(|i| i.completed && !i.boots) {
            let v = norm(item_vec(it));
            (0..3).for_each(|k| acc[k] += v[k] * r.games as f32);
        }
    }
    if acc == [0.0; 3] {
        return cd;
    }
    let sd_ = norm(acc);
    norm([0, 1, 2].map(|k| (1.0 - CHAMP_DIR_WEIGHT) * sd_[k] + CHAMP_DIR_WEIGHT * cd[k]))
}

pub struct Ctx<'a> {
    pub sd: &'a StaticData,
    pub stats: Option<&'a ChampStats>,
    pub champ: &'a Champion,
    /// Completed, map-legal items with their fit to my build direction, sorted by id.
    pool: Vec<(&'a Item, f32, bool)>,
    /// Item → (wins, games) over all slots.
    agg: HashMap<ItemId, (u32, u32)>,
    /// stats_score of an item my champion does not build.
    base: f32,
    /// Snowball items (Mejai's, Dark Seal) are only offered while ahead.
    pub ahead: bool,
}

impl<'a> Ctx<'a> {
    pub fn new(sd: &'a StaticData, stats: Option<&'a ChampStats>, champ: &'a Champion, mode: Mode) -> Self {
        let dir = build_dir(sd, stats, champ);
        let mut pool: Vec<_> = sd
            .items
            .values()
            .filter(|i| i.completed && map_ok(i, mode))
            .map(|i| {
                let v = item_vec(i);
                (i, dot3(norm(v), dir), v[0] + v[1] <= 0.0)
            })
            .collect();
        pool.sort_unstable_by_key(|p| p.0.id);
        let mut agg = HashMap::new();
        for r in stats.iter().flat_map(|s| s.slots.iter().flatten()) {
            let e = agg.entry(r.id).or_insert((0, 0));
            e.0 += r.wins;
            e.1 += r.games;
        }
        let base = match stats {
            Some(s) => wilson_p(frac(s.win_rate), s.games.min(u32::MAX as u64) as u32) - OFF_LIST_PENALTY,
            None => NO_STATS_WR,
        };
        Ctx { sd, stats, champ, pool, agg, base, ahead: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Src {
    Row { wins: u32, games: u32 },
    Other { wins: u32, games: u32 },
    Rule,
}

#[derive(Debug, Clone, Copy)]
pub struct Scored {
    pub id: ItemId,
    pub score: f32,
    pub stats_score: f32,
    pub sit: f32,
    pub src: Src,
    /// Need index with the largest answers·needs term.
    pub need: Option<usize>,
}

#[derive(Clone, Copy, PartialEq)]
enum Decision {
    Slot(usize),
    Boots,
}

/// Unique-ish groups the model does not encode: one Last Whisper, one %magic pen, one anti-heal.
fn conflicts(sd: &StaticData, it: &Item, have: &[ItemId]) -> bool {
    let groups = |i: &Item| {
        [
            i.stats.armor_pen_pct > 0.0,
            i.stats.magic_pen_pct > 0.0,
            i.answers.anti_heal >= 0.5,
        ]
    };
    let g = groups(it);
    have.iter().filter_map(|h| sd.items.get(h)).any(|h| groups(h).iter().zip(g).any(|(a, b)| *a && b))
}

fn score_all(ctx: &Ctx, dec: Decision, have: &[ItemId], need: &[f32; N], out: &mut Vec<Scored>) {
    out.clear();
    let empty = Vec::new();
    let rows = match (ctx.stats, dec) {
        (Some(s), Decision::Slot(k)) => s.slots.get(k).unwrap_or(&empty),
        (Some(s), Decision::Boots) => &s.boots,
        (None, _) => &empty,
    };
    let total: u32 = rows.iter().map(|r| r.games).sum();
    let avg = rows.iter().map(|r| r.wins).sum::<u32>() as f32 / total.max(1) as f32;
    let k = if dec == Decision::Boots { 0.0 } else { SHRINK_GAMES };
    let shrunk = |w: u32, g: u32| (w as f32 + k * avg) / (g as f32 + k).max(1.0);
    let min_games = MIN_ITEM_GAMES.max((MIN_ITEM_SHARE * total as f32) as u32);
    let weight = match dec {
        Decision::Slot(k) => SLOT_WEIGHT[k.min(SLOT_WEIGHT.len() - 1)],
        Decision::Boots => BOOTS_WEIGHT,
    };
    for &(it, fit, defensive) in &ctx.pool {
        if it.boots != (dec == Decision::Boots)
            || have.contains(&it.id)
            || conflicts(ctx.sd, it, have)
            || (!ctx.ahead && SNOWBALL.contains(&it.id))
        {
            continue;
        }
        let ans = arr(&it.answers);
        let (mut dot, mut need_i, mut best) = (0.0, None, 0.0);
        for i in 0..N {
            let t = ans[i] * need[i];
            dot += t;
            if t > best {
                (best, need_i) = (t, Some(i));
            }
        }
        let fits = fit >= FIT_MIN || (it.boots && defensive);
        let (stats_score, src) = if let Some(r) = rows.iter().find(|r| r.id == it.id && r.games >= min_games) {
            let pr = PR_BONUS * r.games as f32 / total.max(1) as f32;
            (shrunk(r.wins, r.games) + pr, Src::Row { wins: r.wins, games: r.games })
        } else if let Some(&(w, g)) = ctx.agg.get(&it.id).filter(|a| a.1 >= MIN_ITEM_GAMES) {
            (wilson(w, g) - OFF_SLOT_PENALTY, Src::Other { wins: w, games: g })
        } else if fits && (ctx.stats.is_none() || dot >= ANSWER_MIN) {
            (ctx.base + FIT_BONUS * fit.max(0.0), Src::Rule)
        } else {
            continue;
        };
        let sit = weight * dot;
        out.push(Scored { id: it.id, score: stats_score + sit, stats_score, sit, src, need: need_i });
    }
    out.sort_unstable_by(|a, b| b.score.total_cmp(&a.score).then(a.id.cmp(&b.id)));
}

pub struct Build {
    /// Starts with buy-next.
    pub path: Vec<Scored>,
    pub alts: Vec<Scored>,
}

/// Greedy path from `owned` up to MAX_ITEMS finished items; needs decay as the path answers them.
pub fn build(ctx: &Ctx, owned: &[ItemId], needs: &Needs) -> Build {
    let sd = ctx.sd;
    let fin = |id: &ItemId| sd.items.get(id).filter(|i| i.completed);
    let mut legend = owned.iter().filter_map(fin).filter(|i| !i.boots).count();
    let has_boots = owned.iter().filter_map(fin).any(|i| i.boots);
    let mut boots_done = has_boots || ctx.stats.is_some_and(|s| s.boots.is_empty() && !s.slots.is_empty());
    let mut filled = legend + has_boots as usize;
    let mut have = owned.to_vec();
    let mut need = needs.p;
    let (mut path, mut alts, mut buf) = (Vec::new(), Vec::new(), Vec::new());
    while filled < MAX_ITEMS {
        let boots_turn = !boots_done && legend >= BOOTS_AFTER;
        let order = if boots_turn {
            [Some(Decision::Boots), Some(Decision::Slot(legend))]
        } else {
            [Some(Decision::Slot(legend)), (!boots_done).then_some(Decision::Boots)]
        };
        let Some(dec) = order.into_iter().flatten().find(|&d| {
            score_all(ctx, d, &have, &need, &mut buf);
            !buf.is_empty()
        }) else {
            break;
        };
        let best = buf[0];
        if path.is_empty() {
            alts.extend(buf.iter().skip(1).take(ALTERNATIVES));
        }
        if let Some(it) = sd.items.get(&best.id) {
            decay(&mut need, &it.answers);
        }
        have.push(best.id);
        path.push(best);
        match dec {
            Decision::Boots => boots_done = true,
            Decision::Slot(_) => legend += 1,
        }
        filled += 1;
    }
    Build { path, alts }
}

/// Gold still needed for `item` given the components I hold.
fn remaining_cost(sd: &StaticData, item: &Item, inv: &mut Vec<ItemId>) -> u32 {
    let mut cost = item.cost;
    let mut stack: Vec<ItemId> = item.from.clone();
    while let Some(id) = stack.pop() {
        if let Some(pos) = inv.iter().position(|&h| h == id) {
            inv.swap_remove(pos);
            cost = cost.saturating_sub(sd.items.get(&id).map_or(0, |c| c.cost));
        } else if let Some(c) = sd.items.get(&id) {
            stack.extend(&c.from);
        }
    }
    cost
}

/// Most expensive affordable component in the item's tree I don't already own.
pub fn component_now(sd: &StaticData, item: &Item, inventory: &[ItemId], gold: f32) -> Option<ItemId> {
    if gold >= remaining_cost(sd, item, &mut inventory.to_vec()) as f32 {
        return None;
    }
    let mut best: Option<&Item> = None;
    let mut stack: Vec<ItemId> = item.from.clone();
    while let Some(id) = stack.pop() {
        if inventory.contains(&id) {
            continue;
        }
        let Some(c) = sd.items.get(&id) else { continue };
        if c.cost as f32 <= gold {
            if best.is_none_or(|b| (c.cost, std::cmp::Reverse(c.id)) > (b.cost, std::cmp::Reverse(b.id))) {
                best = Some(c);
            }
        } else {
            stack.extend(&c.from);
        }
    }
    best.map(|c| c.id)
}

pub fn reason(ctx: &Ctx, s: &Scored, needs: &Needs, foes: &[Foe]) -> String {
    let need_line = s.need.map(|i| match needs.driver[i].and_then(|d| foes.get(d)) {
        Some(f) => format!("{}: {}", NEED_LABELS[i], f.label()),
        None => format!("{} vs their team", NEED_LABELS[i]),
    });
    let name = &ctx.champ.name;
    let pct = |w: u32, g: u32| 100.0 * w as f32 / g.max(1) as f32;
    match (s.src, need_line) {
        (_, Some(l)) if s.sit > (s.stats_score - ctx.base).max(0.0) => l,
        // Threat matters but stats lead: name both.
        (Src::Row { wins, games } | Src::Other { wins, games }, Some(l)) if s.sit >= REASON_SIT_MIN => {
            format!("{l} · {:.0}% on {name}", pct(wins, games))
        }
        (Src::Row { wins, games }, _) => {
            format!("top pick for {name} ({:.0}% over {} games)", pct(wins, games), fmt_games(games as u64))
        }
        (Src::Other { wins, games }, _) => {
            format!("strong on {name} ({:.0}% over {} games)", pct(wins, games), fmt_games(games as u64))
        }
        (_, Some(l)) => l,
        _ => format!("fits {name}'s build"),
    }
}

/// `gold`: (my gold, my inventory) to compute `component_now`.
pub fn to_pick(ctx: &Ctx, s: &Scored, needs: &Needs, foes: &[Foe], gold: Option<(f32, &[ItemId])>) -> ItemPick {
    let it = ctx.sd.items.get(&s.id);
    ItemPick {
        item: s.id,
        name: it.map_or("?".into(), |i| i.name.clone()),
        cost: it.map_or(0, |i| i.cost),
        score: s.score,
        reason: reason(ctx, s, needs, foes),
        component_now: it.zip(gold).and_then(|(i, (g, inv))| component_now(ctx.sd, i, inv, g)),
    }
}
