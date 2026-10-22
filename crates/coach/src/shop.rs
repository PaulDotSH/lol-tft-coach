//! Shopping list (what my gold buys right now, Q1) and the gold ETA / back
//! planner built on it (Q2).

use crate::{common::*, consts::*};
use model::*;

/// Gold to buy `id` now: its cost minus held components, which are taken out of `inv`.
fn price(sd: &StaticData, it: &Item, inv: &mut Vec<ItemId>) -> u32 {
    let mut cost = it.cost;
    let mut stack = it.from.clone();
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

/// Slot-taking items (trinkets are not in static data).
fn slots(sd: &StaticData, inv: &[ItemId]) -> usize {
    inv.iter().filter(|i| sd.items.contains_key(i)).count()
}

/// One shopping simulation at a given gold amount.
struct Sim<'a> {
    sd: &'a StaticData,
    inv: Vec<ItemId>,
    gold: u32,
    spent: u32,
    buys: Vec<ShopBuy>,
    /// Smallest starting gold at which some main buy would change (back planner).
    next: Option<u32>,
}

impl Sim<'_> {
    fn note(&mut self, price: u32) {
        let at = self.spent + price;
        self.next = Some(self.next.map_or(at, |n| n.min(at)));
    }

    fn buy(&mut self, it: &Item, kind: BuyKind, toward: Option<ItemId>) -> bool {
        let mut inv = self.inv.clone();
        let p = price(self.sd, it, &mut inv);
        if p > self.gold {
            if kind != BuyKind::Extra {
                self.note(p);
            }
            return false;
        }
        inv.push(it.id);
        if slots(self.sd, &inv) > INVENTORY_SLOTS {
            return false;
        }
        self.inv = inv;
        self.gold -= p;
        self.spent += p;
        self.buys.push(ShopBuy { item: it.id, name: it.name.clone(), price: p, kind, toward });
        true
    }

    /// Most expensive affordable parts of `target`, repeatedly.
    fn components(&mut self, target: &Item) {
        loop {
            let mut avail = self.inv.clone();
            let mut best: Option<&Item> = None;
            let mut stack = target.from.clone();
            while let Some(id) = stack.pop() {
                if let Some(pos) = avail.iter().position(|&h| h == id) {
                    avail.swap_remove(pos);
                    continue;
                }
                let Some(c) = self.sd.items.get(&id) else { continue };
                let p = price(self.sd, c, &mut avail.clone());
                if p <= self.gold {
                    if best.is_none_or(|b| (c.cost, std::cmp::Reverse(c.id)) > (b.cost, std::cmp::Reverse(b.id))) {
                        best = Some(c);
                    }
                } else {
                    self.note(p);
                    stack.extend(&c.from);
                }
            }
            match best {
                Some(c) if self.buy(c, BuyKind::Component, Some(target.id)) => {}
                _ => return,
            }
        }
    }
}

fn simulate<'a>(sd: &'a StaticData, mode: Mode, me: &Player, gold: u32, path: &[ItemId], extras: bool) -> Sim<'a> {
    let mut s = Sim { sd, inv: me.items.clone(), gold, spent: 0, buys: vec![], next: None };
    for it in path.iter().take(SHOP_PATH_DEPTH).filter_map(|id| sd.items.get(id)) {
        if !s.buy(it, BuyKind::Full, None) {
            s.components(it);
        }
    }
    if !extras {
        return s;
    }
    if let Some(w) = sd.items.get(&CONTROL_WARD).filter(|w| map_ok(w, mode))
        && !s.inv.contains(&CONTROL_WARD)
    {
        s.buy(w, BuyKind::Extra, None);
    }
    // Elixirs are drunk on purchase: no slot, so they bypass `buy`.
    if path.is_empty()
        && me.level >= ELIXIR_LEVEL
        && let Some(e) = sd.items.get(&elixir(champ(sd, me.champ))).filter(|e| map_ok(e, mode) && e.cost <= s.gold)
    {
        s.gold -= e.cost;
        s.buys.push(ShopBuy { item: e.id, name: e.name.clone(), price: e.cost, kind: BuyKind::Extra, toward: None });
    }
    s
}

fn elixir(c: &Champion) -> ItemId {
    if c.classes.contains(&ChampClass::Tank) {
        ELIXIR_IRON
    } else if c.traits.magic_share >= 0.5 {
        ELIXIR_SORCERY
    } else {
        ELIXIR_WRATH
    }
}

/// `path`: remaining build, buy next first.
pub fn shopping(sd: &StaticData, mode: Mode, me: &Player, gold: f32, path: &[ItemId]) -> ShoppingList {
    let gold = gold.max(0.0) as u32;
    let s = simulate(sd, mode, me, gold, path, true);
    ShoppingList { gold, leftover: s.gold, buys: s.buys, dead: me.dead, respawn_in: if me.dead { me.respawn_in } else { 0.0 } }
}

/// `income`: gold per second (`Tracker::income`).
pub fn eta(sd: &StaticData, mode: Mode, me: &Player, gold: f32, path: &[ItemId], income: Option<f32>) -> GoldEta {
    let gold = gold.max(0.0) as u32;
    let rate = income.filter(|r| *r > 0.0);
    let secs = |g: u32| rate.map(|r| g as f32 / r);
    let next = path.first().and_then(|id| sd.items.get(id));
    let missing = next.map_or(0, |it| price(sd, it, &mut me.items.clone()).saturating_sub(gold));
    let ids = |s: &Sim| s.buys.iter().map(|b| b.item).collect::<Vec<_>>();
    let (mut backs, mut at) = (Vec::new(), gold);
    let mut sim = simulate(sd, mode, me, at, path, false);
    let mut prev = ids(&sim);
    // Each step jumps to the exact gold at which the greedy list changes.
    while backs.len() < BACK_PLANS
        && let Some(n) = sim.next.filter(|&n| n > at && n < gold + missing)
    {
        at = n;
        sim = simulate(sd, mode, me, at, path, false);
        let items = ids(&sim);
        if items != prev {
            backs.push(BackPlan { gold: at, in_s: secs(at - gold), items: items.clone() });
            prev = items;
        }
    }
    GoldEta {
        item: next.map(|i| i.id),
        per_min: rate.map(|r| r * 60.0),
        missing,
        eta_s: if missing == 0 { Some(0.0) } else { secs(missing) },
        backs,
    }
}
