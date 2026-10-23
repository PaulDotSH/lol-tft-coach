//! Item scoring (balanced mode), buy-next, alternatives, late-game path.

use crate::{
    augment_tags::{AugEffects, item_has},
    common::*,
    consts::*,
    modes::{self, ARMOR, ARMOR_PEN, MAGIC_PEN, MR, Rules},
    needs::{Needs, decay},
    resists::{PEN_GAIN_FULL, Pen, Targets},
    threat::Foe,
};
use model::*;
use std::collections::HashMap;

/// Score lost by a crit / attack-speed item entirely past the mode's cap (win-rate units).
const CAP_PENALTY: f32 = 0.04;
/// Crit amount of a typical crit item (full penalty weight).
const CAP_REF_CRIT: f32 = 0.25;
/// Base attack speed when Data Dragon stats are missing.
const DEFAULT_BASE_AS: f32 = 0.65;
/// Augment shift at which a reason names the augment.
const AUG_REASON_MIN: f32 = 0.01;

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
    /// Support: the quest item holds a slot, and lolalytics counts its upgrade as item 1.
    pub support: bool,
    pub rules: &'static Rules,
    /// My live stats: crit / attack-speed caps.
    pub live: Option<&'a ChampionStats>,
    /// Mayhem: picked augments.
    pub aug: AugEffects,
    /// Enemy resists: penetration answers become the real damage gain.
    pub pen: Option<Targets>,
}

impl<'a> Ctx<'a> {
    pub fn new(sd: &'a StaticData, stats: Option<&'a ChampStats>, champ: &'a Champion, mode: Mode) -> Self {
        let dir = build_dir(sd, stats, champ);
        let rules = modes::rules(mode);
        let mut pool: Vec<_> = sd
            .items
            .values()
            .filter(|i| i.completed && map_ok(i, mode) && !rules.exclude.contains(&i.id))
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
        Ctx { sd, stats, champ, pool, agg, base, ahead: false, support: false, rules, live: None, aug: AugEffects::default(), pen: None }
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
    /// Augment shift and the `AugEffects::sources` index behind it.
    pub aug: f32,
    pub aug_src: Option<usize>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Decision {
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
    // The game's own one-per-inventory groups (e.g. Black Cleaver vs Lord Dominik's).
    let shared = |h: &ItemId| match (sd.item_groups.get(&it.id), sd.item_groups.get(h)) {
        (Some(a), Some(b)) => a.iter().any(|x| b.contains(x)),
        _ => false,
    };
    have.iter().any(shared)
        || have.iter().filter_map(|h| sd.items.get(h)).any(|h| groups(h).iter().zip(g).any(|(a, b)| *a && b))
}

/// Crit chance and attack speed already reached (live stats, else owned items) plus the path so far.
#[derive(Clone, Copy, Default)]
pub(crate) struct Load {
    pub crit: f32,
    pub attack_speed: f32,
}

impl Load {
    pub fn new(ctx: &Ctx, owned: &[ItemId]) -> Load {
        match ctx.live {
            Some(s) => Load { crit: s.crit_chance, attack_speed: s.attack_speed },
            None => Load { crit: inventory(ctx.sd, owned).crit, attack_speed: 0.0 },
        }
    }

    fn add(&mut self, ctx: &Ctx, it: &Item) {
        self.crit += frac(it.stats.crit);
        if self.attack_speed > 0.0 {
            self.attack_speed += as_gain(ctx, it);
        }
    }
}

/// Bonus attack speed scales with the base (ratio ≈ base attack speed).
fn as_gain(ctx: &Ctx, it: &Item) -> f32 {
    let base = if ctx.champ.base.attack_speed > 0.0 { ctx.champ.base.attack_speed } else { DEFAULT_BASE_AS };
    frac(it.stats.attack_speed) * base
}

/// Score lost to crit / attack speed past the mode's cap.
fn cap_penalty(ctx: &Ctx, it: &Item, load: Load) -> f32 {
    let wasted = |now: f32, add: f32, cap: f32| ((now + add - cap).max(0.0)).min(add) / add;
    let mut p = 0.0;
    let c = frac(it.stats.crit);
    if let Some(cap) = ctx.rules.crit_cap
        && c > 0.0
    {
        p += CAP_PENALTY * wasted(load.crit, c, cap) * (c / CAP_REF_CRIT).min(1.0);
    }
    let a = as_gain(ctx, it);
    if load.attack_speed > 0.0 && a > 0.0 {
        p += CAP_PENALTY * wasted(load.attack_speed, a, ctx.rules.attack_speed_cap);
    }
    p
}

/// Answers, with penetration replaced by its real gain against the enemy resists.
fn answers(ctx: &Ctx, it: &Item) -> [f32; N] {
    let mut ans = arr(&it.answers);
    if let Some(t) = &ctx.pen {
        let s = &it.stats;
        if s.lethality > 0.0 || s.armor_pen_pct > 0.0 {
            ans[ARMOR_PEN] = clamp01(t.gain(true, Pen { flat: s.lethality, pct: frac(s.armor_pen_pct) }) / PEN_GAIN_FULL);
        }
        if s.magic_pen_flat > 0.0 || s.magic_pen_pct > 0.0 {
            ans[MAGIC_PEN] = clamp01(t.gain(false, Pen { flat: s.magic_pen_flat, pct: frac(s.magic_pen_pct) }) / PEN_GAIN_FULL);
        }
    }
    ans
}

pub(crate) fn score_all(ctx: &Ctx, dec: Decision, have: &[ItemId], need: &[f32; N], load: Load, out: &mut Vec<Scored>) {
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
        let ans = answers(ctx, it);
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
            // Pessimistic both ways: the Wilson bound keeps a weak item weak, and pulling
            // toward this slot's average caps a lucky small sample (Spirit Visage 64% over
            // 115 games, mostly later buys in longer, won games).
            let shrunk = (w as f32 + SHRINK_GAMES * avg) / (g as f32 + SHRINK_GAMES);
            let wr = if total > 0 { wilson(w, g).min(shrunk) } else { wilson(w, g) };
            (wr - OFF_SLOT_PENALTY, Src::Other { wins: w, games: g })
        } else if fits && (ctx.stats.is_none() || dot >= ANSWER_MIN) {
            (ctx.base + FIT_BONUS * fit.max(0.0), Src::Rule)
        } else {
            continue;
        };
        let sit = weight * dot;
        let (aug, aug_src) = ctx.aug.item_bonus(it);
        // Nothing to amplify: Spirit Visage on a champion without self-healing.
        let no_heal = HEAL_AMP_ITEMS.contains(&it.id) && ctx.champ.traits.heals < SELF_HEAL_MIN;
        // MR into a physical team (or armor into a magic one): a good win rate doesn't make up for it.
        let off_type = (ans[MR] - ans[ARMOR]).max(0.0) * (need[ARMOR] - need[MR]).max(0.0)
            + (ans[ARMOR] - ans[MR]).max(0.0) * (need[MR] - need[ARMOR]).max(0.0);
        let score = stats_score + sit + aug - cap_penalty(ctx, it, load) - if no_heal { SELF_HEAL_PENALTY } else { 0.0 }
            - RESIST_MISMATCH * off_type;
        out.push(Scored { id: it.id, score, stats_score, sit, src, need: need_i, aug, aug_src });
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
    let mut legend = owned.iter().filter_map(fin).filter(|i| !i.boots).count() + ctx.support as usize;
    let has_boots = owned.iter().filter_map(fin).any(|i| i.boots);
    let mut boots_done = has_boots || ctx.stats.is_some_and(|s| s.boots.is_empty() && !s.slots.is_empty());
    let mut filled = legend + has_boots as usize;
    let mut have = owned.to_vec();
    let mut need = needs.p;
    let mut load = Load::new(ctx, owned);
    let max = MAX_ITEMS.saturating_sub(ctx.aug.keep_slots());
    let (mut path, mut alts, mut buf) = (Vec::new(), Vec::new(), Vec::new());
    while filled < max {
        let boots_turn = !boots_done && legend >= BOOTS_AFTER;
        let order = if boots_turn {
            [Some(Decision::Boots), Some(Decision::Slot(legend))]
        } else {
            [Some(Decision::Slot(legend)), (!boots_done).then_some(Decision::Boots)]
        };
        let Some(dec) = order.into_iter().flatten().find(|&d| {
            score_all(ctx, d, &have, &need, load, &mut buf);
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
            load.add(ctx, it);
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

/// Supports: the quest upgrade most worth taking, while none is owned.
pub fn support_upgrade(ctx: &Ctx, owned: &[ItemId]) -> Option<ItemPick> {
    if !ctx.support || owned.iter().any(|i| SUPPORT_UPGRADES.contains(i)) {
        return None;
    }
    let seen: Vec<(ItemId, u32, u32)> =
        SUPPORT_UPGRADES.iter().filter_map(|&id| ctx.agg.get(&id).map(|&(w, g)| (id, w, g))).filter(|x| x.2 > 0).collect();
    let (w, g) = seen.iter().fold((0, 0), |a, x| (a.0 + x.1, a.1 + x.2));
    let avg = w as f32 / g.max(1) as f32;
    // Shrunk toward the upgrades' average: a rare pick needs many games to beat the usual one.
    let wr = |x: &(ItemId, u32, u32)| (x.1 as f32 + SHRINK_GAMES * avg) / (x.2 as f32 + SHRINK_GAMES);
    let best = seen.iter().max_by(|a, b| wr(a).total_cmp(&wr(b)))?;
    let item = ctx.sd.items.get(&best.0)?;
    let share = 100.0 * best.2 as f32 / g as f32;
    Some(ItemPick {
        item: item.id,
        name: item.name.clone(),
        cost: 0,
        score: wr(best),
        reason: format!(
            "{:.0}% win over {} games on {} ({share:.0}% of upgrades)",
            100.0 * best.1 as f32 / best.2 as f32,
            fmt_games(best.2 as u64),
            ctx.champ.name
        ),
        component_now: None,
    })
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

/// Components that stack while held (Tear of the Goddess): bought first.
const STACKING: [ItemId; 1] = [3070];

/// Most expensive affordable component in the item's tree I don't already own;
/// a stacking component first.
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
        if STACKING.contains(&id) && c.cost as f32 <= gold {
            return Some(id);
        }
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
    let base = need_reason(ctx, s, needs, foes);
    let Some(src) = s.aug_src.and_then(|i| ctx.aug.sources.get(i)).filter(|_| s.aug >= AUG_REASON_MIN) else {
        return base;
    };
    let tag = ctx.sd.items.get(&s.id).and_then(|it| src.tags.iter().copied().find(|&t| item_has(t, it)));
    let what = match ctx.sd.items.get(&s.id) {
        Some(it) if src.requires.contains(&it.id) => "quest item",
        _ => tag.map_or("", |t| t.label()),
    };
    let favored = src.favors.contains(&s.id);
    let note = match () {
        _ if favored => format!("best with {}", src.name),
        _ if what.is_empty() => format!("with {}", src.name),
        _ => format!("with {} ({what})", src.name),
    };
    let decides = s.aug > s.sit.max(s.stats_score - ctx.base);
    match (decides, tag.is_some_and(|t| t.broad()) && what != "quest item" && !favored) {
        (true, _) => note,
        (false, true) => base,
        (false, false) => format!("{base} · {note}"),
    }
}

fn need_reason(ctx: &Ctx, s: &Scored, needs: &Needs, foes: &[Foe]) -> String {
    let need_line = s.need.map(|i| match needs.driver[i].and_then(|d| foes.get(d)) {
        Some(f) => {
            let resist = ctx.pen.as_ref().and_then(|t| t.of(f.champ.key)).and_then(|(a, m)| match i {
                ARMOR_PEN => Some(format!(" ~{a:.0} armor")),
                MAGIC_PEN => Some(format!(" ~{m:.0} MR")),
                _ => None,
            });
            format!("{}: {}{}", NEED_LABELS[i], f.label(), resist.unwrap_or_default())
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: ItemId, cost: u32, from: &[ItemId]) -> Item {
        Item {
            id,
            name: id.to_string(),
            cost,
            from: from.to_vec(),
            into: vec![],
            stats: ItemStats::default(),
            answers: Profile::default(),
            boots: false,
            completed: from.is_empty(),
            rift: true,
            aram: true,
            tags: vec![],
        }
    }

    #[test]
    fn tear_is_bought_before_pricier_components() {
        // Archangel's Staff: Tear (400) + Lost Chapter (1200).
        let items = [item(3070, 400, &[]), item(3802, 1200, &[]), item(3003, 2900, &[3070, 3802])];
        let sd = StaticData {
            version: String::new(),
            items: items.into_iter().map(|i| (i.id, i)).collect(),
            champs: Default::default(),
            item_groups: Default::default(),
            runes: Default::default(),
        };
        let archangels = &sd.items[&3003];
        assert_eq!(component_now(&sd, archangels, &[], 1300.0), Some(3070));
        assert_eq!(component_now(&sd, archangels, &[3070], 1300.0), Some(3802));
    }

    #[test]
    fn support_upgrade_needs_games_to_beat_the_usual_pick() {
        let ids = [3876, 3869, 3877];
        let sd = StaticData {
            version: String::new(),
            items: ids.map(|id| (id, item(id, 400, &[]))).into_iter().collect(),
            champs: Default::default(),
            item_groups: Default::default(),
            runes: Default::default(),
        };
        let thresh = Champion {
            key: 412,
            id: "Thresh".into(),
            name: "Thresh".into(),
            slug: "thresh".into(),
            classes: vec![],
            traits: Default::default(),
            base: Default::default(),
        };
        let row = |id, games, wins| ItemStat { id, games, wins };
        let stats = ChampStats {
            champ: 412,
            lane: Some(Lane::Support),
            mode: Mode::Rift,
            patch: "16.21".into(),
            win_rate: 52.0,
            pick_rate: 10.0,
            games: 30_000,
            lanes: Default::default(),
            // Solstice Sleigh 52% (20k), Celestial Opposition 54% (5k), Bloodsong 60% (100).
            slots: vec![vec![row(3876, 20_000, 10_400), row(3869, 5_000, 2_700), row(3877, 100, 60)]],
            boots: vec![],
            early: vec![],
            counters: vec![],
            benchmarks: Default::default(),
            runes: None,
        };
        let mut ctx = Ctx::new(&sd, Some(&stats), &thresh, Mode::Rift);
        assert!(support_upgrade(&ctx, &[]).is_none(), "not a support");
        ctx.support = true;
        let up = support_upgrade(&ctx, &[3865]).unwrap();
        assert_eq!(up.item, 3869, "{}", up.reason);
        assert_eq!(up.reason, "54% win over 5.0k games on Thresh (20% of upgrades)");
        assert!(support_upgrade(&ctx, &[3876]).is_none(), "already upgraded");
    }
}
