use crate::{
    common::*,
    consts::*,
    modes::{RANGED_FROM, Rules},
    resists::{Pen, grow, mult},
    threat::Foe,
};
use model::*;
use serde::Deserialize;
use std::{collections::HashMap, sync::LazyLock};

const KEYS: [&str; 4] = ["Q", "W", "E", "R"];
const MAX_RANK: [u32; 4] = [5, 5, 5, 3];
const R_LEVELS: [u32; 3] = [6, 11, 16];

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Physical,
    Magic,
    True,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PerRank {
    One(f32),
    Ranks(Vec<f32>),
}

impl Default for PerRank {
    fn default() -> Self {
        PerRank::One(0.0)
    }
}

impl PerRank {
    fn at(&self, i: usize) -> f32 {
        match self {
            PerRank::One(x) => *x,
            PerRank::Ranks(v) => at(v, i),
        }
    }
}

fn at(v: &[f32], i: usize) -> f32 {
    v.get(i).or(v.last()).copied().unwrap_or(0.0)
}

fn one() -> f32 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spell {
    pub key: String,
    pub kind: Kind,
    pub base: Vec<f32>,
    #[serde(default)]
    pub ap: PerRank,
    #[serde(default)]
    pub ad: PerRank,
    #[serde(default)]
    pub bonus_ad: PerRank,
    #[serde(default)]
    pub hp: PerRank,
    #[serde(default)]
    pub bonus_hp: PerRank,
    #[serde(default)]
    pub armor: PerRank,
    #[serde(default)]
    pub bonus_armor: PerRank,
    #[serde(default)]
    pub mr: PerRank,
    #[serde(default)]
    pub bonus_mr: PerRank,
    #[serde(default)]
    pub target_max_hp: PerRank,
    #[serde(default = "one")]
    pub hits: f32,
    pub cooldown: Vec<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Autos {
    pub on_hit: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kit {
    #[serde(default)]
    pub base_ad: f32,
    #[serde(default)]
    pub ad_per_level: f32,
    #[serde(default)]
    pub spell: Vec<Spell>,
    #[serde(default)]
    pub autos: Autos,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Effect {
    pub kind: Kind,
    pub ap_mult: Option<f32>,
    pub ap: f32,
    pub hp: f32,
    pub mr: f32,
    pub crit: f32,
    pub base_ad_bonus: f32,
    pub ad_per_bonus_hp: f32,
    pub ap_per_bonus_hp: f32,
    pub crit_damage: f32,
    pub amp: f32,
    pub amp_magic: f32,
    pub amp_in_combat: f32,
    pub amp_vs_hp: f32,
    pub on_hit_flat: f32,
    pub on_hit_ap: f32,
    pub on_hit_hp: f32,
    pub on_hit_current_hp: f32,
    pub burn_max_hp: f32,
    pub burn_s: f32,
    pub proc_flat: f32,
    pub proc_ap: f32,
    pub proc_base_ad: f32,
    pub proc_bonus_ad: f32,
    pub proc_hp: f32,
    pub proc_cd: f32,
    pub shred_armor: f32,
    pub shred_mr: f32,
    pub basic_haste: f32,
    pub ult_haste: f32,
    pub melee_scale: Option<f32>,
    pub shield: f32,
    pub ehp_mult_physical: Option<f32>,
    pub ehp_mult_magic: Option<f32>,
}

pub static KITS: LazyLock<HashMap<String, Kit>> =
    LazyLock::new(|| toml::from_str(include_str!("../kits.toml")).expect("kits.toml is valid"));

pub static EFFECTS: LazyLock<HashMap<ItemId, Effect>> = LazyLock::new(|| {
    toml::from_str::<HashMap<String, Effect>>(include_str!("../item_effects.toml"))
        .expect("item_effects.toml is valid")
        .into_iter()
        .map(|(k, v)| (k.parse().expect("item_effects.toml: numeric item id"), v))
        .collect()
});

static NO_EFFECT: LazyLock<Effect> = LazyLock::new(Effect::default);

fn effect(id: &ItemId) -> &'static Effect {
    EFFECTS.get(id).unwrap_or(&NO_EFFECT)
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stats {
    pub ap: f32,
    pub ad: f32,
    pub base_ad: f32,
    pub attack_speed: f32,
    pub as_ratio: f32,
    pub crit: f32,
    pub armor_pen: Pen,
    pub magic_pen: Pen,
    pub haste: f32,
    pub max_hp: f32,
    pub base_hp: f32,
    pub armor: f32,
    pub base_armor: f32,
    pub mr: f32,
    pub base_mr: f32,
    pub melee: bool,
}

impl Stats {
    pub fn live(s: &ChampionStats, c: &Champion, base_ad: f32, ad_per_level: f32, level: u32) -> Stats {
        let b = &c.base;
        let range = if s.attack_range > 0.0 { s.attack_range } else { b.attack_range };
        Stats {
            ap: s.ability_power,
            ad: s.attack_damage,
            base_ad: grow(base_ad, ad_per_level, level),
            attack_speed: s.attack_speed,
            as_ratio: if b.attack_speed > 0.0 { b.attack_speed } else { crate::items::DEFAULT_BASE_AS },
            crit: s.crit_chance,
            armor_pen: Pen { flat: s.lethality, pct: s.armor_pen_pct },
            magic_pen: Pen { flat: s.magic_pen_flat, pct: s.magic_pen_pct },
            haste: s.ability_haste,
            max_hp: s.max_health,
            base_hp: grow(b.hp, b.hp_per_level, level),
            armor: s.armor,
            base_armor: grow(b.armor, b.armor_per_level, level),
            mr: s.magic_resist,
            base_mr: grow(b.mr, b.mr_per_level, level),
            melee: range <= RANGED_FROM,
        }
    }

    fn bonus_ad(&self) -> f32 {
        (self.ad - self.base_ad).max(0.0)
    }

    fn bonus_hp(&self) -> f32 {
        (self.max_hp - self.base_hp).max(0.0)
    }
}

fn minus(p: Pen, o: Pen) -> Pen {
    Pen { flat: p.flat - o.flat, pct: 1.0 - (1.0 - p.pct) / (1.0 - o.pct).max(1e-6) }
}

fn item_pens(s: &ItemStats) -> (Pen, Pen) {
    (Pen { flat: s.lethality, pct: frac(s.armor_pen_pct) }, Pen { flat: s.magic_pen_flat, pct: frac(s.magic_pen_pct) })
}

fn consume<'a>(sd: &'a StaticData, it: &Item, left: &mut Vec<ItemId>, used: &mut Vec<&'a Item>) {
    for c in &it.from {
        if let Some(i) = left.iter().position(|x| x == c) {
            left.remove(i);
            used.extend(sd.items.get(c));
        } else if let Some(ci) = sd.items.get(c) {
            consume(sd, ci, left, used);
        }
    }
}

pub fn with_item(sd: &StaticData, me: &Stats, item: &Item, owned: &[ItemId], rules: &Rules) -> (Stats, Vec<ItemId>) {
    let (mut left, mut used) = (owned.to_vec(), vec![]);
    consume(sd, item, &mut left, &mut used);
    let sum = |f: &dyn Fn(&ItemStats) -> f32| f(&item.stats) - used.iter().map(|u| f(&u.stats)).sum::<f32>();
    let e = effect(&item.id);
    let mut out = *me;
    let hp = sum(&|s| s.hp) + e.hp;
    out.max_hp += hp;
    out.armor += sum(&|s| s.armor);
    out.mr += sum(&|s| s.mr) + e.mr;
    out.haste += sum(&|s| s.ability_haste);
    let kept = || left.iter().map(effect);
    out.ad += sum(&|s| s.ad) + e.base_ad_bonus * me.base_ad + e.ad_per_bonus_hp * out.bonus_hp() + kept().map(|k| k.ad_per_bonus_hp).sum::<f32>() * hp;
    let ap = sum(&|s| s.ap) + e.ap + e.ap_per_bonus_hp * out.bonus_hp() + kept().map(|k| k.ap_per_bonus_hp).sum::<f32>() * hp;
    let held_mult: f32 = kept().filter_map(|k| k.ap_mult).product();
    out.ap = (me.ap + ap * held_mult) * e.ap_mult.unwrap_or(1.0);
    out.attack_speed = (me.attack_speed + sum(&|s| frac(s.attack_speed)) * me.as_ratio).min(rules.attack_speed_cap.max(me.attack_speed));
    let cap = rules.crit_cap.unwrap_or(1.0).min(1.0);
    out.crit = (me.crit + sum(&|s| frac(s.crit)) + e.crit).min(cap.max(me.crit)).max(0.0);
    let (mut apen, mut mpen) = (me.armor_pen, me.magic_pen);
    for u in &used {
        let (a, m) = item_pens(&u.stats);
        (apen, mpen) = (minus(apen, a), minus(mpen, m));
    }
    let (a, m) = item_pens(&item.stats);
    (out.armor_pen, out.magic_pen) = (apen.plus(a), mpen.plus(m));
    left.push(item.id);
    (out, left)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Target {
    pub armor: f32,
    pub mr: f32,
    pub hp: f32,
    pub bonus_hp: f32,
}

pub struct Caster<'a> {
    pub kit: &'a Kit,
    pub ranks: [u32; 4],
    pub on_hit: f32,
}

pub fn output(p: &Caster, me: &Stats, held: &[ItemId], t: &Target, sustained: bool) -> f32 {
    let fx: Vec<&Effect> = held.iter().map(effect).collect();
    let total = |f: &dyn Fn(&Effect) -> f32| fx.iter().map(|e| f(e)).sum::<f32>();
    let scale = |e: &Effect| if me.melee { e.melee_scale.unwrap_or(1.0) } else { 1.0 };
    let stack = if sustained { 1.0 } else { WHATIF_BURST_STACKS };
    let bonus = |r: f32, base: f32| (r - base).max(0.0);
    let mut parts: Vec<(Kind, f32)> = vec![];
    let (mut casts, mut cast) = (0.0, [false; 4]);
    for s in &p.kit.spell {
        let Some(slot) = KEYS.iter().position(|k| *k == s.key) else { continue };
        let r = p.ranks[slot];
        if r == 0 {
            continue;
        }
        let i = r as usize - 1;
        let v = at(&s.base, i)
            + s.ap.at(i) * me.ap
            + s.ad.at(i) * me.ad
            + s.bonus_ad.at(i) * me.bonus_ad()
            + s.hp.at(i) * me.max_hp
            + s.bonus_hp.at(i) * me.bonus_hp()
            + s.armor.at(i) * me.armor
            + s.bonus_armor.at(i) * bonus(me.armor, me.base_armor)
            + s.mr.at(i) * me.mr
            + s.bonus_mr.at(i) * bonus(me.mr, me.base_mr)
            + s.target_max_hp.at(i) * t.hp;
        let n = if sustained {
            let h = me.haste + if slot == 3 { total(&|e| e.ult_haste) } else { total(&|e| e.basic_haste) };
            let cd = at(&s.cooldown, i) / (1.0 + h.max(0.0) / 100.0);
            if cd > 0.0 { 1.0 + (SUSTAIN_S / cd).floor() } else { 1.0 }
        } else {
            1.0
        };
        if !cast[slot] {
            cast[slot] = true;
            casts += n;
        }
        parts.push((s.kind, v.max(0.0) * s.hits * n));
    }
    let autos = if sustained { me.attack_speed * SUSTAIN_S } else { BURST_AUTOS };
    let crit_damage = CRIT_BASE + total(&|e| e.crit_damage);
    parts.push((Kind::Physical, me.ad * (1.0 + me.crit * (crit_damage - 1.0)) * autos));
    if p.on_hit > 0.0 {
        parts.push((Kind::Magic, p.on_hit * (KIT_ON_HIT_FLAT + KIT_ON_HIT_AP * me.ap) * autos));
    }
    for e in &fx {
        let k = scale(e);
        let hit = e.on_hit_flat + e.on_hit_ap * me.ap + e.on_hit_hp * me.max_hp + e.on_hit_current_hp * t.hp * CURRENT_HP_SHARE;
        if hit > 0.0 {
            parts.push((e.kind, hit * k * autos));
        }
        let proc = e.proc_flat + e.proc_ap * me.ap + e.proc_base_ad * me.base_ad + e.proc_bonus_ad * me.bonus_ad() + e.proc_hp * me.max_hp;
        if proc > 0.0 {
            let n = if sustained && e.proc_cd > 0.0 { (1.0 + (SUSTAIN_S / e.proc_cd).floor()).min(casts.max(1.0)) } else { 1.0 };
            parts.push((e.kind, proc * k * n));
        }
        if e.burn_max_hp > 0.0 {
            let n = if sustained { (SUSTAIN_S / e.burn_s.max(1.0)).max(1.0) } else { 1.0 };
            parts.push((Kind::Magic, e.burn_max_hp * t.hp * k * n));
        }
    }
    let shred = |f: &dyn Fn(&Effect) -> f32| 1.0 - (total(f) * stack).min(1.0);
    let (armor, mr) = (t.armor * shred(&|e| e.shred_armor), t.mr * shred(&|e| e.shred_mr));
    let vs_hp = (t.bonus_hp / AMP_VS_HP_FULL).clamp(0.0, 1.0);
    let amps = (1.0 + total(&|e| e.amp)) * (1.0 + total(&|e| e.amp_in_combat) * stack) * (1.0 + total(&|e| e.amp_vs_hp) * vs_hp);
    let magic_amp = 1.0 + total(&|e| e.amp_magic);
    parts
        .iter()
        .map(|&(kind, v)| {
            v * amps
                * match kind {
                    Kind::Physical => mult(armor, me.armor_pen),
                    Kind::Magic => mult(mr, me.magic_pen) * magic_amp,
                    Kind::True => 1.0,
                }
        })
        .sum()
}

pub fn ranks(snap: &GameSnapshot, stats: Option<&ChampStats>, level: u32) -> [u32; 4] {
    if let Some(r) = snap.my_abilities {
        return r;
    }
    let mut r = [0u32; 4];
    let order = stats.map_or("", |s| s.skill_order.as_str());
    let idx: Option<Vec<usize>> = order.chars().map(|c| KEYS.iter().position(|k| k.starts_with(c))).collect();
    if let Some(idx) = idx.filter(|v| !v.is_empty()) {
        for &k in idx.iter().take(level as usize) {
            r[k] += 1;
        }
        return r;
    }
    r[3] = R_LEVELS.iter().filter(|&&l| l <= level).count() as u32;
    let mut points = level.saturating_sub(r[3]);
    for k in [0, 1, 2].into_iter().chain([0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]) {
        if points == 0 {
            break;
        }
        if r[k] < MAX_RANK[k] {
            r[k] += 1;
            points -= 1;
        }
    }
    r
}

fn class_name(c: ChampClass) -> &'static str {
    match c {
        ChampClass::Assassin => "_Assassin",
        ChampClass::Fighter => "_Fighter",
        ChampClass::Mage => "_Mage",
        ChampClass::Marksman => "_Marksman",
        ChampClass::Support => "_Support",
        ChampClass::Tank => "_Tank",
    }
}

pub fn kit_for(c: &Champion) -> Option<(&'static Kit, bool)> {
    let own = KITS.get(&c.id).filter(|k| !k.spell.is_empty());
    if let Some(k) = own {
        return Some((k, false));
    }
    c.classes.iter().find_map(|&cl| KITS.get(class_name(cl)).filter(|k| !k.spell.is_empty())).map(|k| (k, true))
}

pub struct Enemy {
    pub champ: ChampKey,
    pub target: Target,
    pub w: f32,
    pub lane: bool,
}

pub struct Sim<'a> {
    sd: &'a StaticData,
    rules: &'static Rules,
    kit: &'static Kit,
    pub rough: bool,
    ranks: [u32; 4],
    on_hit: f32,
    me: Stats,
    pub owned: Vec<ItemId>,
    foes: Vec<Enemy>,
    now: Vec<(f32, f32)>,
    ehp_now: [f32; 3],
    mix: (f32, f32),
    pub burst_leads: bool,
    mayhem: bool,
}

pub struct SimInput<'a, 'b> {
    pub snap: &'b GameSnapshot,
    pub sd: &'a StaticData,
    pub stats: Option<&'b ChampStats>,
    pub champ: &'b Champion,
    pub est: &'b [Option<EnemyResist>],
    pub foes: &'b [Foe<'b>],
    pub lane_opp: Option<ChampKey>,
    pub rules: &'static Rules,
}

fn ehp(s: &Stats, e: &Effect) -> [f32; 3] {
    let r = |x: f32, m: Option<f32>| s.max_hp * (1.0 + x.max(0.0) / 100.0) * m.unwrap_or(1.0) + e.shield;
    [r(s.armor, e.ehp_mult_physical), r(s.mr, e.ehp_mult_magic), s.max_hp + e.shield]
}

fn gain(after: f32, now: f32) -> f32 {
    if now > 0.0 { after / now - 1.0 } else { 0.0 }
}

impl<'a> Sim<'a> {
    pub fn new(i: &SimInput<'a, '_>) -> Option<Sim<'a>> {
        let live = i.snap.my_stats.as_ref()?;
        let me = i.snap.players.get(i.snap.me)?;
        let (kit, rough) = kit_for(i.champ)?;
        let own = KITS.get(&i.champ.id).unwrap_or(kit);
        let stats = Stats::live(live, i.champ, own.base_ad, own.ad_per_level, me.level);
        let foes: Vec<Enemy> = i
            .est
            .iter()
            .flatten()
            .filter_map(|e| {
                let p = i.snap.players.iter().find(|p| p.champ == e.champ && p.team != me.team)?;
                let b = &champ(i.sd, e.champ).base;
                let inv = inventory(i.sd, &p.items);
                let hp = grow(b.hp, b.hp_per_level, p.level) + inv.hp;
                let score = i.foes.iter().find(|f| f.champ.key == e.champ).map_or(AVG_THREAT, |f| f.score);
                let lane = i.lane_opp == Some(e.champ);
                let w = (NEED_W_FLOOR + score / 100.0) * if lane { WHATIF_LANE_W } else { 1.0 };
                Some(Enemy { champ: e.champ, target: Target { armor: e.armor, mr: e.mr, hp, bonus_hp: inv.hp }, w, lane })
            })
            .collect();
        if foes.is_empty() {
            return None;
        }
        let (mut magic, mut tru, mut w) = (0.0, 0.0, 0.0);
        for f in i.foes {
            let fw = NEED_W_FLOOR + f.score / 100.0;
            let t = f.champ.traits.true_damage * TRUE_FRAC;
            w += fw;
            magic += fw * f.magic * (1.0 - t);
            tru += fw * t;
        }
        let mix = if w > 0.0 { (magic / w, tru / w) } else { (0.5, 0.0) };
        let mut sim = Sim {
            sd: i.sd,
            rules: i.rules,
            kit,
            rough,
            ranks: ranks(i.snap, i.stats, me.level),
            on_hit: kit.autos.on_hit.unwrap_or(i.champ.traits.on_hit),
            me: stats,
            owned: me.items.clone(),
            foes,
            now: vec![],
            ehp_now: ehp(&stats, &NO_EFFECT),
            mix,
            burst_leads: i.champ.traits.burst >= WHATIF_BURST_TRAIT,
            mayhem: i.snap.mode == Mode::Mayhem,
        };
        let p = sim.profile();
        sim.now = sim.foes.iter().map(|f| (output(&p, &stats, &sim.owned, &f.target, false), output(&p, &stats, &sim.owned, &f.target, true))).collect();
        Some(sim)
    }

    fn profile(&self) -> Caster<'static> {
        Caster { kit: self.kit, ranks: self.ranks, on_hit: self.on_hit }
    }

    fn mixed(&self, v: [f32; 3]) -> f32 {
        let (m, t) = self.mix;
        let p = (1.0 - m - t).max(0.0);
        1.0 / (p / v[0].max(1.0) + m / v[1].max(1.0) + t / v[2].max(1.0))
    }

    pub fn eval(&self, item: &Item) -> WhatIf {
        let (after, held) = with_item(self.sd, &self.me, item, &self.owned, self.rules);
        let p = self.profile();
        let mut per_enemy: Vec<(WhatIfEnemy, f32)> = self
            .foes
            .iter()
            .zip(&self.now)
            .map(|(f, &(b, s))| {
                let e = WhatIfEnemy {
                    champ: f.champ,
                    burst: gain(output(&p, &after, &held, &f.target, false), b),
                    sustained: gain(output(&p, &after, &held, &f.target, true), s),
                    lane: f.lane,
                };
                (e, f.w)
            })
            .collect();
        let den: f32 = per_enemy.iter().map(|x| x.1).sum::<f32>().max(1e-6);
        let burst = per_enemy.iter().map(|x| x.0.burst * x.1).sum::<f32>() / den;
        let sustained = per_enemy.iter().map(|x| x.0.sustained * x.1).sum::<f32>() / den;
        per_enemy.sort_by(|a, b| b.0.lane.cmp(&a.0.lane).then(b.1.total_cmp(&a.1)));
        let v = ehp(&after, effect(&item.id));
        WhatIf {
            item: item.id,
            damage: if self.burst_leads { burst } else { sustained },
            burst,
            sustained,
            burst_leads: self.burst_leads,
            per_enemy: per_enemy.into_iter().map(|x| x.0).collect(),
            ehp: gain(self.mixed(v), self.mixed(self.ehp_now)),
            ehp_physical: gain(v[0], self.ehp_now[0]),
            ehp_magic: gain(v[1], self.ehp_now[1]),
            rough: self.rough,
            note: self.mayhem.then(|| "Augment procs not counted".to_string()),
        }
    }

    pub fn term(&self, item: &Item, peel: f32) -> f32 {
        let w = self.eval(item);
        let k = if self.rough { WHATIF_ROUGH_W } else { 1.0 };
        k * (WHATIF_DMG_W * w.damage * (1.0 - peel).max(0.0) + WHATIF_EHP_W * w.ehp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes;

    fn item(id: ItemId, from: &[ItemId], stats: ItemStats) -> Item {
        Item {
            id,
            name: id.to_string(),
            cost: 3000,
            from: from.to_vec(),
            into: vec![],
            stats,
            answers: model::Profile::default(),
            boots: false,
            completed: true,
            rift: true,
            aram: true,
            tags: vec![],
        }
    }

    fn sd(items: Vec<Item>) -> StaticData {
        StaticData {
            version: String::new(),
            items: items.into_iter().map(|i| (i.id, i)).collect(),
            champs: Default::default(),
            item_groups: Default::default(),
            runes: Default::default(),
        }
    }

    fn spell(kind: Kind, ap: f32, ad: f32) -> Spell {
        Spell {
            key: "Q".into(),
            kind,
            base: vec![0.0],
            ap: PerRank::One(ap),
            ad: PerRank::One(ad),
            bonus_ad: PerRank::default(),
            hp: PerRank::default(),
            bonus_hp: PerRank::default(),
            armor: PerRank::default(),
            bonus_armor: PerRank::default(),
            mr: PerRank::default(),
            bonus_mr: PerRank::default(),
            target_max_hp: PerRank::default(),
            hits: 1.0,
            cooldown: vec![10.0],
        }
    }

    fn kit(spells: Vec<Spell>) -> Kit {
        Kit { base_ad: 0.0, ad_per_level: 0.0, spell: spells, autos: Autos::default() }
    }

    fn ap_spell_only(me: &Stats, held: &[ItemId], t: &Target) -> f32 {
        let k = kit(vec![spell(Kind::Magic, 1.0, 0.0)]);
        let p = Caster { kit: &k, ranks: [1, 0, 0, 0], on_hit: 0.0 };
        let mut m = *me;
        m.ad = 0.0;
        output(&p, &m, held, t, false)
    }

    const RABADON: ItemId = 3089;
    const VOID: ItemId = 3135;
    const ROD: ItemId = 1058;

    fn ap_items() -> StaticData {
        sd(vec![
            item(ROD, &[], ItemStats { ap: 65.0, ..Default::default() }),
            item(RABADON, &[ROD, ROD], ItemStats { ap: 130.0, ..Default::default() }),
            item(VOID, &[], ItemStats { ap: 95.0, magic_pen_pct: 40.0, ..Default::default() }),
        ])
    }

    fn mage(ap: f32) -> Stats {
        Stats { ap, as_ratio: 0.625, attack_speed: 0.7, max_hp: 1500.0, ..Default::default() }
    }

    fn ap_gain(sd: &StaticData, id: ItemId, me: &Stats, mr: f32) -> f32 {
        let t = Target { mr, ..Default::default() };
        let (after, held) = with_item(sd, me, &sd.items[&id], &[], modes::rules(Mode::Rift));
        ap_spell_only(&after, &held, &t) / ap_spell_only(me, &[], &t) - 1.0
    }

    #[test]
    fn rabadons_vs_void_staff_crossover_where_the_formula_says() {
        let sd = ap_items();
        let me = mage(400.0);
        let rab = (400.0f32 + 130.0) * 1.3 / 400.0 - 1.0;
        let void = |mr: f32| 495.0 / 400.0 * (100.0 + mr) / (100.0 + 0.6 * mr) - 1.0;
        for mr in [40.0, 150.0] {
            assert!((ap_gain(&sd, RABADON, &me, mr) - rab).abs() < 1e-4);
            assert!((ap_gain(&sd, VOID, &me, mr) - void(mr)).abs() < 1e-4, "{mr}");
            assert!(ap_gain(&sd, RABADON, &me, mr) > ap_gain(&sd, VOID, &me, mr));
        }
        let cross = (1.0 + rab) / 1.2375;
        let cross = (100.0 * cross - 100.0) / (1.0 - 0.6 * cross);
        assert!((cross - 237.6).abs() < 0.5, "{cross}");
        assert!(ap_gain(&sd, RABADON, &me, cross - 5.0) > ap_gain(&sd, VOID, &me, cross - 5.0));
        assert!(ap_gain(&sd, VOID, &me, cross + 5.0) > ap_gain(&sd, RABADON, &me, cross + 5.0));
    }

    fn ad_gain(add: Pen, armor: f32) -> f32 {
        let k = kit(vec![spell(Kind::Physical, 0.0, 1.0)]);
        let p = Caster { kit: &k, ranks: [1, 0, 0, 0], on_hit: 0.0 };
        let t = Target { armor, ..Default::default() };
        let me = Stats { ad: 200.0, ..Default::default() };
        let with = Stats { armor_pen: me.armor_pen.plus(add), ..me };
        output(&p, &with, &[], &t, false) / output(&p, &me, &[], &t, false) - 1.0
    }

    #[test]
    fn lethality_vs_percent_armor_pen_at_60_and_200_armor() {
        let leth = Pen { flat: 18.0, pct: 0.0 };
        let pct = Pen { flat: 0.0, pct: 0.35 };
        assert!((ad_gain(leth, 60.0) - (160.0 / 142.0 - 1.0)).abs() < 1e-4);
        assert!((ad_gain(pct, 60.0) - (160.0 / 139.0 - 1.0)).abs() < 1e-4);
        assert!((ad_gain(leth, 200.0) - (300.0 / 282.0 - 1.0)).abs() < 1e-4);
        assert!((ad_gain(pct, 200.0) - (300.0 / 230.0 - 1.0)).abs() < 1e-4);
        assert!(ad_gain(pct, 200.0) > 4.0 * ad_gain(leth, 200.0));
    }

    fn autos_gain(crit: f32) -> f32 {
        let sd = sd(vec![item(3031, &[], ItemStats { ad: 75.0, crit: 25.0, ..Default::default() })]);
        let k = kit(vec![]);
        let p = Caster { kit: &k, ranks: [0; 4], on_hit: 0.0 };
        let me = Stats { ad: 200.0, crit, ..Default::default() };
        let (after, held) = with_item(&sd, &me, &sd.items[&3031], &[], modes::rules(Mode::Rift));
        let t = Target::default();
        output(&p, &after, &held, &t, false) / output(&p, &me, &[], &t, false) - 1.0
    }

    #[test]
    fn infinity_edge_at_half_and_full_crit() {
        assert!((autos_gain(0.5) - (275.0 * (1.0 + 0.75 * 1.05) / 275.0 - 1.0)).abs() < 1e-4);
        assert!((autos_gain(1.0) - (275.0 * 2.05 / 350.0 - 1.0)).abs() < 1e-4);
    }

    #[test]
    fn spellblade_procs_once_per_cast_in_the_sustained_window() {
        let k = kit(vec![spell(Kind::Magic, 1.0, 0.0)]);
        let p = Caster { kit: &k, ranks: [1, 0, 0, 0], on_hit: 0.0 };
        let me = Stats { ap: 100.0, base_ad: 60.0, ..Default::default() };
        let t = Target::default();
        let proc = 0.75 * 60.0 + 0.45 * 100.0;
        let extra = |s| output(&p, &me, &[3100], &t, s) - output(&p, &me, &[], &t, s);
        assert!((extra(true) - 2.0 * proc).abs() < 1e-3, "{}", extra(true));
        assert!((extra(false) - proc).abs() < 1e-3);
    }

    #[test]
    fn more_penetration_never_lowers_damage() {
        let k = kit(vec![spell(Kind::Physical, 0.0, 1.0), spell(Kind::Magic, 1.0, 0.0)]);
        let p = Caster { kit: &k, ranks: [1, 0, 0, 0], on_hit: 0.0 };
        for r in [0.0, 20.0, 60.0, 150.0, 300.0] {
            let t = Target { armor: r, mr: r, hp: 2000.0, bonus_hp: 500.0 };
            let mut last = 0.0;
            for n in 0..10 {
                let pen = Pen { flat: 5.0 * n as f32, pct: 0.05 * n as f32 };
                let me = Stats { ad: 150.0, ap: 200.0, armor_pen: pen, magic_pen: pen, ..Default::default() };
                let d = output(&p, &me, &[], &t, true);
                assert!(d >= last, "resist {r} pen step {n}");
                last = d;
            }
        }
    }

    #[test]
    fn an_empty_item_changes_nothing_and_crit_past_the_cap_gains_nothing() {
        let sd = sd(vec![item(9999, &[], ItemStats::default()), item(9998, &[], ItemStats { crit: 25.0, ..Default::default() })]);
        let me = Stats { ad: 200.0, ap: 100.0, crit: 1.0, attack_speed: 1.0, ..mage(100.0) };
        let k = kit(vec![spell(Kind::Magic, 1.0, 0.0)]);
        let p = Caster { kit: &k, ranks: [1, 0, 0, 0], on_hit: 0.0 };
        let t = Target { armor: 80.0, mr: 60.0, hp: 2000.0, bonus_hp: 0.0 };
        for id in [9999, 9998] {
            let (after, held) = with_item(&sd, &me, &sd.items[&id], &[], modes::rules(Mode::Rift));
            assert_eq!(after, me);
            for s in [false, true] {
                assert_eq!(output(&p, &after, &held, &t, s), output(&p, &me, &[], &t, s));
            }
        }
    }

    #[test]
    fn with_item_subtracts_the_components_it_consumes() {
        let sd = ap_items();
        let me = mage(465.0);
        let (after, held) = with_item(&sd, &me, &sd.items[&RABADON], &[ROD, 3020], modes::rules(Mode::Rift));
        assert!((after.ap - (465.0 + 130.0 - 65.0) * 1.3).abs() < 1e-3, "{}", after.ap);
        assert_eq!(held, vec![3020, RABADON]);
        let (_, held) = with_item(&sd, &me, &sd.items[&RABADON], &[ROD, ROD], modes::rules(Mode::Rift));
        assert_eq!(held, vec![RABADON]);
    }

    #[test]
    fn ranks_from_the_api_then_the_skill_order_then_a_default() {
        let snap = GameSnapshot {
            time: 0.0,
            mode: Mode::Rift,
            map: 11,
            me: 0,
            my_gold: 0.0,
            players: vec![],
            events: vec![],
            my_stats: None,
            my_abilities: Some([3, 1, 2, 1]),
        };
        assert_eq!(ranks(&snap, None, 7), [3, 1, 2, 1]);
        let snap = GameSnapshot { my_abilities: None, ..snap };
        let stats = ChampStats { skill_order: "QEWQQRQEQEREEWW".into(), ..test_stats() };
        assert_eq!(ranks(&snap, Some(&stats), 7), [4, 1, 1, 1]);
        assert_eq!(ranks(&snap, None, 7), [4, 1, 1, 1]);
        assert_eq!(ranks(&snap, None, 18), [5, 5, 5, 3]);
    }

    fn test_stats() -> ChampStats {
        ChampStats {
            champ: 0,
            lane: None,
            mode: Mode::Rift,
            patch: String::new(),
            win_rate: 50.0,
            pick_rate: 0.0,
            games: 0,
            lanes: Default::default(),
            slots: vec![],
            boots: vec![],
            early: vec![],
            counters: vec![],
            benchmarks: Default::default(),
            runes: None,
            coachless_runes: None,
            coachless: None,
            summoners: vec![],
            skill_order: String::new(),
            skill_order_games: 0,
            skill_priority: String::new(),
            by_length: vec![],
        }
    }

    #[test]
    fn kits_and_item_effects_name_data_dragon_champions_and_items() {
        let champs: serde_json::Value = serde_json::from_str(include_str!("../../../fixtures/data/ddragon-champion.json")).unwrap();
        let items: serde_json::Value = serde_json::from_str(include_str!("../../../fixtures/data/ddragon-item.json")).unwrap();
        for id in KITS.keys().filter(|k| !k.starts_with('_')) {
            assert!(champs["data"].get(id).is_some(), "kits.toml: unknown champion {id}");
        }
        assert!(KITS.len() > 170, "{}", KITS.len());
        for id in EFFECTS.keys() {
            assert!(items["data"].get(id.to_string()).is_some(), "item_effects.toml: unknown item {id}");
        }
        assert!(EFFECTS.len() >= 40, "{}", EFFECTS.len());
        for c in ["_Mage", "_Marksman", "_Fighter", "_Tank", "_Assassin", "_Support"] {
            assert!(KITS.get(c).is_some_and(|k| !k.spell.is_empty()), "{c}");
        }
    }

    #[test]
    fn ahri_kit_parses_from_the_generated_file() {
        let a = &KITS["Ahri"];
        let q: Vec<&Spell> = a.spell.iter().filter(|s| s.key == "Q").collect();
        assert_eq!(q.iter().map(|s| s.kind).collect::<Vec<_>>(), [Kind::Magic, Kind::True]);
        assert_eq!((q[0].base[0], q[0].ap.at(4)), (35.0, 0.5));
        assert_eq!(a.base_ad, 53.0);
    }
}
