//! Which offered augment did I take? The Live Client API has no augment field (Mayhem or
//! Arena) and OCR sees the offer, not the click, so when an offer closes we compare my
//! `championStats` (and granted items) before and after against each card's known effect.
//! Anything unclear stays a list of candidates.

use crate::{augment_tags as tags, common::*, modes};
use model::*;

/// Residual must reach this share of an augment's expected stat gain …
const MATCH_LO: f32 = 0.5;
/// … and stay under this multiple to single out one of several matches.
const MATCH_TIGHT: f32 = 1.6;
/// A closed offer that comes back with this many of the same cards was not a pick.
const SAME_OFFER: usize = 2;
/// Gold spent on something not in the inventory (anvil, elixir) blurs every stat.
const HIDDEN_SPEND: f32 = 300.0;
/// Base attack speed when Data Dragon stats are missing (the lower end, so the bar stays low).
const FALLBACK_BASE_AS: f32 = 0.6;
/// Bought-for-real: a granted item still counts if gold dropped less than this share of its cost.
const GRANT_GOLD: f32 = 0.5;

#[derive(Clone)]
struct Before {
    stats: Option<ChampionStats>,
    items: Vec<ItemId>,
    gold: f32,
    level: u32,
}

#[derive(Clone)]
struct Window {
    /// Every card seen while open (rerolls included): matches a reopened offer.
    cards: Vec<AugmentId>,
    /// The latest complete offer: rerolled-away cards can't be picked.
    shown: Vec<AugmentId>,
    before: Before,
}

/// Per-game memory: feed every tick with the current on-screen offer.
#[derive(Default)]
pub struct AugmentTracker {
    pub picks: Vec<MyAugment>,
    open: Option<Window>,
    /// Last closed window while its pick is unresolved (undone if the offer reappears).
    last: Option<Window>,
}

impl AugmentTracker {
    pub fn observe(&mut self, offered: &[AugmentId], snap: &GameSnapshot, sd: &StaticData) {
        let Some(me) = snap.players.get(snap.me) else { return };
        let now = Before { stats: snap.my_stats.clone(), items: me.items.clone(), gold: snap.my_gold, level: me.level };
        if !offered.is_empty() {
            match &mut self.open {
                Some(w) => {
                    union(&mut w.cards, offered);
                    // A reroll flip can read as two cards for a moment.
                    if offered.len() >= w.shown.len() {
                        w.shown = offered.to_vec();
                    }
                }
                None => {
                    let reopened = self.last.take().filter(|w| offered.iter().filter(|c| w.cards.contains(c)).count() >= SAME_OFFER);
                    self.open = Some(match reopened {
                        Some(mut w) => {
                            self.picks.pop();
                            union(&mut w.cards, offered);
                            w.shown = offered.to_vec();
                            w
                        }
                        None => Window { cards: offered.to_vec(), shown: offered.to_vec(), before: now },
                    });
                }
            }
            return;
        }
        let Some(w) = self.open.take() else { return };
        let base_as = Some(champ(sd, me.champ).base.attack_speed).filter(|&a| a > 0.0).unwrap_or(FALLBACK_BASE_AS);
        let mut pick = identify(&w.shown, &w.before, &now, sd, base_as);
        pick.time = snap.time;
        let allowed = modes::rules(snap.mode).augments_by(me.level);
        if allowed > 0 && self.picks.len() >= allowed {
            // More closes than offers so far: the previous one was a shop closed without picking.
            self.picks.pop();
        }
        self.last = pick.id.is_none().then_some(w);
        self.picks.push(pick);
    }
}

fn union(cards: &mut Vec<AugmentId>, more: &[AugmentId]) {
    for c in more {
        if !cards.contains(c) {
            cards.push(*c);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Dim {
    Ap,
    Ah,
    Ad,
    Armor,
    Mr,
    Hp,
    Range,
    Crit,
    Omnivamp,
    LifeSteal,
    Hsp,
    Ms,
    As,
    ArmorPen,
    MagicPen,
}

/// `augments.toml` stat key → (dimension, multiplies the value before).
fn dim(key: &str) -> Option<(Dim, bool)> {
    Some(match key {
        "ap" => (Dim::Ap, false),
        "ah" => (Dim::Ah, false),
        "ad" => (Dim::Ad, false),
        "ad_pct" => (Dim::Ad, true),
        "armor" => (Dim::Armor, false),
        "mr" => (Dim::Mr, false),
        "hp" => (Dim::Hp, false),
        "hp_pct" => (Dim::Hp, true),
        "range" => (Dim::Range, false),
        "crit" => (Dim::Crit, false),
        "omnivamp" => (Dim::Omnivamp, false),
        "life_steal" => (Dim::LifeSteal, false),
        "hsp" => (Dim::Hsp, false),
        "ms" => (Dim::Ms, false),
        "as_pct" => (Dim::As, true),
        "armor_pen_pct" => (Dim::ArmorPen, false),
        "magic_pen_pct" => (Dim::MagicPen, false),
        _ => return None,
    })
}

fn value(s: &ChampionStats, d: Dim) -> f32 {
    match d {
        Dim::Ap => s.ability_power,
        Dim::Ah => s.ability_haste,
        Dim::Ad => s.attack_damage,
        Dim::Armor => s.armor,
        Dim::Mr => s.magic_resist,
        Dim::Hp => s.max_health,
        Dim::Range => s.attack_range,
        Dim::Crit => s.crit_chance,
        Dim::Omnivamp => s.omnivamp,
        Dim::LifeSteal => s.life_steal,
        Dim::Hsp => s.heal_shield_power,
        Dim::Ms => s.move_speed,
        Dim::As => s.attack_speed,
        Dim::ArmorPen => s.armor_pen_pct,
        Dim::MagicPen => s.magic_pen_pct,
    }
}

/// The stat family a tag-only augment could move, to keep it as a rival explanation.
fn tag_dims(t: tags::Tag) -> &'static [Dim] {
    use tags::Tag::*;
    match t {
        Ap => &[Dim::Ap],
        Haste => &[Dim::Ah],
        Crit => &[Dim::Crit],
        AttackSpeed => &[Dim::As],
        Tank => &[Dim::Hp, Dim::Armor, Dim::Mr],
        HealShield => &[Dim::Hsp],
        Range => &[Dim::Range],
        MoveSpeed => &[Dim::Ms],
        _ => &[],
    }
}

/// Dimensions an item's own stats move (its passives are assumed to stay in those).
fn item_dims(it: &Item, out: &mut Vec<Dim>) {
    let s = &it.stats;
    for (on, d) in [
        (s.ap > 0.0, Dim::Ap),
        (s.ability_haste > 0.0, Dim::Ah),
        (s.ad > 0.0, Dim::Ad),
        (s.armor > 0.0, Dim::Armor),
        (s.mr > 0.0, Dim::Mr),
        (s.hp > 0.0, Dim::Hp),
        (s.crit > 0.0, Dim::Crit),
        (s.omnivamp > 0.0, Dim::Omnivamp),
        (s.lifesteal > 0.0, Dim::LifeSteal),
        (tags::item_has(tags::Tag::HealShield, it), Dim::Hsp),
        (s.move_speed > 0.0 || it.boots, Dim::Ms),
        (s.attack_speed > 0.0, Dim::As),
        (s.armor_pen_pct > 0.0, Dim::ArmorPen),
        (s.magic_pen_pct > 0.0, Dim::MagicPen),
    ] {
        if on {
            out.push(d);
        }
    }
}

/// `a` minus `b` as multisets.
fn minus(a: &[ItemId], b: &[ItemId]) -> Vec<ItemId> {
    let mut rest = b.to_vec();
    a.iter()
        .filter(|i| match rest.iter().position(|r| r == *i) {
            Some(p) => {
                rest.swap_remove(p);
                false
            }
            None => true,
        })
        .copied()
        .collect()
}

#[derive(PartialEq, Debug)]
enum Fit {
    /// Every expected effect showed up; `tight` = within MATCH_TIGHT of the expected size.
    Yes { tight: bool },
    /// An effect that should have shown is missing.
    No,
    /// No reliable evidence either way.
    Maybe,
}

fn identify(cards: &[AugmentId], b: &Before, a: &Before, sd: &StaticData, base_as: f32) -> MyAugment {
    let new = minus(&a.items, &b.items);
    let gone = minus(&b.items, &a.items);
    let cost = |ids: &[ItemId]| ids.iter().filter_map(|i| sd.items.get(i)).map(|i| i.cost as f32).sum::<f32>();
    let spent = b.gold - a.gold;
    let hidden = spent - (cost(&new) - cost(&gone)) > HIDDEN_SPEND;
    let mut blurred: Vec<Dim> = vec![];
    for it in new.iter().chain(&gone).filter_map(|i| sd.items.get(i)) {
        item_dims(it, &mut blurred);
    }
    if a.level != b.level {
        blurred.extend([Dim::Hp, Dim::Armor, Dim::Mr, Dim::Ad, Dim::As]);
    }

    let fit = |aug: &tags::Aug| -> Fit {
        if let Some(g) = aug.grants {
            let free = new.contains(&g) && sd.items.get(&g).is_some_and(|it| spent < GRANT_GOLD * it.cost as f32);
            return if free { Fit::Yes { tight: true } } else { Fit::No };
        }
        let (Some(sb), Some(sa)) = (&b.stats, &a.stats) else { return Fit::Maybe };
        if aug.stats.is_empty() || hidden {
            return Fit::Maybe;
        }
        let mut tight = true;
        for (k, &v) in &aug.stats {
            let Some((d, pct)) = dim(k) else { return Fit::Maybe };
            let expect = match (d, pct) {
                (Dim::As, true) => v * base_as,
                (_, true) => v * value(sb, d),
                _ => v,
            };
            if blurred.contains(&d) || expect <= 0.0 {
                return Fit::Maybe;
            }
            let r = value(sa, d) - value(sb, d);
            if r < MATCH_LO * expect {
                return Fit::No;
            }
            tight &= r <= MATCH_TIGHT * expect;
        }
        Fit::Yes { tight }
    };

    let known: Vec<(AugmentId, Option<&tags::Aug>)> = cards.iter().map(|&c| (c, tags::get(c))).collect();
    let fits: Vec<(AugmentId, Fit)> = known.iter().map(|&(c, aug)| (c, aug.map_or(Fit::Maybe, fit))).collect();
    let yes: Vec<_> = fits.iter().filter(|f| matches!(f.1, Fit::Yes { .. })).collect();
    let maybe: Vec<AugmentId> = fits.iter().filter(|f| f.1 == Fit::Maybe).map(|f| f.0).collect();
    let tight: Vec<_> = yes.iter().filter(|f| f.1 == Fit::Yes { tight: true }).collect();
    let single = match (yes.len(), tight.len()) {
        (1, _) => Some(yes[0].0),
        (_, 1) => Some(tight[0].0),
        _ => None,
    };
    // A tag-only or unknown card that could move the same stats is a rival explanation.
    let rivals = |id: AugmentId| {
        let dims: Vec<Dim> = tags::get(id).map_or(vec![], |g| g.stats.keys().filter_map(|k| dim(k)).map(|d| d.0).collect());
        maybe.iter().copied().filter(move |&m| match tags::get(m) {
            None => true,
            Some(g) => g.tags.iter().any(|&t| tag_dims(t).iter().any(|d| dims.contains(d))),
        })
    };
    if let Some(id) = single
        && rivals(id).next().is_none()
    {
        return MyAugment { id: Some(id), candidates: vec![id], time: 0.0 };
    }
    // Something matched: only cards that could explain it remain; else whatever was not ruled out.
    let mut c: Vec<AugmentId> = yes.iter().map(|f| f.0).collect();
    for r in yes.iter().flat_map(|f| rivals(f.0)) {
        union(&mut c, &[r]);
    }
    if c.is_empty() {
        c = if maybe.is_empty() { cards.to_vec() } else { maybe };
    }
    MyAugment { id: None, candidates: c, time: 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiset_minus() {
        assert_eq!(minus(&[1, 2, 2, 3], &[2, 3]), vec![1, 2]);
        assert!(minus(&[], &[1]).is_empty());
    }

    #[test]
    fn toml_stat_keys_all_map_to_dimensions() {
        for id in [1022, 1007, 1097, 1037, 1138, 1068, 1170, 1305, 1013, 1041, 1047, 1084] {
            for k in tags::get(id).unwrap().stats.keys() {
                assert!(dim(k).is_some(), "{id}: {k}");
            }
        }
    }
}
