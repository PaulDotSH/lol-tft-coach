//! Mayhem: enemy augments read off the Tab scoreboard. When to ask for Tab,
//! how sure a read is, and a modest nudge to threats and needs.

use crate::common::{N, clamp01};
use crate::needs::Needs;
use crate::threat::Assessed;
use model::*;
use std::collections::HashMap;

/// Augment rounds: game start (level 3), then levels 7, 11 and 15.
const ROUND_LEVELS: [u32; 3] = [7, 11, 15];
/// Seconds after an enemy's round before asking (they may still be choosing).
const GRACE: f32 = 20.0;
/// While dead: ask sooner, if at least this much respawn time is left.
const DEAD_GRACE: f32 = 5.0;
const DEAD_LEFT: f32 = 4.0;

/// What an augment adds to an enemy, approximated as items would.
const AUG_HP: f32 = 900.0;
const AUG_RESIST: f32 = 30.0;
const AUG_CRIT: f32 = 0.4;
const AUG_AS: f32 = 0.5;
/// Anti-heal / anti-burst need from one enemy with such an augment, at threat 0 and 100.
const AUG_HEAL: (f32, f32) = (0.3, 0.7);
const AUG_BURST: (f32, f32) = (0.2, 0.6);
const ANTI_HEAL: usize = 3;
const ANTI_BURST: usize = 6;

/// Inputs from the scoreboard reader; empty outside Mayhem.
#[derive(Clone, Copy, Default)]
pub struct EnemyAugmentInput<'a> {
    pub seen: &'a [EnemyAugments],
    /// u.gg augment rankings for enemy champions (whatever is loaded).
    pub ranks: &'a [ChampAugments],
    /// Game time since which some enemy's augments are unknown (`stale_enemies` > 0).
    pub stale_since: Option<f32>,
}

/// Augments a Mayhem player has picked by `level`.
pub fn expected(level: u32) -> usize {
    1 + ROUND_LEVELS.iter().filter(|&&l| level >= l).count()
}

fn seen_for(seen: &[EnemyAugments], champ: ChampKey) -> Option<&EnemyAugments> {
    seen.iter().find(|e| e.champ == champ)
}

/// Enemies with fewer augments read than their level implies.
pub fn stale_enemies(snap: &GameSnapshot, seen: &[EnemyAugments]) -> usize {
    let Some(me) = snap.players.get(snap.me) else { return 0 };
    snap.players
        .iter()
        .filter(|p| p.team != me.team)
        .filter(|p| seen_for(seen, p.champ).map_or(0, |e| e.augments.len()) < expected(p.level))
        .count()
}

/// Ask to hold Tab: some enemy augment is unknown, they had time to pick,
/// and either a grace period passed or you are dead with time to spare.
pub fn tab_hint(snap: &GameSnapshot, seen: &[EnemyAugments], stale_since: Option<f32>) -> Option<TabHint> {
    if snap.mode != Mode::Mayhem {
        return None;
    }
    let n = stale_enemies(snap, seen);
    let waited = snap.time - stale_since?;
    let me = snap.players.get(snap.me)?;
    let dead = me.dead && me.respawn_in >= DEAD_LEFT && waited >= DEAD_GRACE;
    if n == 0 || !(dead || waited >= GRACE) {
        return None;
    }
    let what = match (seen.is_empty(), n) {
        (true, _) => "No enemy augments read yet".to_string(),
        (false, 1) => "1 enemy has an unread augment".to_string(),
        (false, n) => format!("{n} enemies have unread augments"),
    };
    Some(TabHint {
        text: "Hold Tab for 1 s to read enemy augments".into(),
        reason: if dead { format!("{what} · you're dead, a good moment") } else { what },
    })
}

/// Order the augments sharing one icon, likeliest first: tint matches the
/// rarity u.gg lists it under, listed for this champion, then higher ranked.
pub fn order_candidates(ids: &[AugmentId], tint: Option<Rarity>, ranks: Option<&ChampAugments>) -> Vec<AugmentId> {
    let mut v = ids.to_vec();
    v.sort_by(|&a, &b| {
        let key = |id| match rank_of(ranks, id) {
            Some((r, pos, of)) => (u8::from(tint.is_some_and(|t| t != r)), 0u8, pos as f32 / of.max(1) as f32),
            None => (0, 1, 1.0),
        };
        let (ka, kb) = (key(a), key(b));
        (ka.0, ka.1).cmp(&(kb.0, kb.1)).then(ka.2.total_cmp(&kb.2))
    });
    v
}

fn rank_of(ranks: Option<&ChampAugments>, id: AugmentId) -> Option<(Rarity, u32, u32)> {
    let a = ranks?;
    [(Rarity::Prismatic, &a.prismatic), (Rarity::Gold, &a.gold), (Rarity::Silver, &a.silver)]
        .into_iter()
        .find_map(|(r, l)| l.iter().position(|&x| x == id).map(|p| (r, p as u32, l.len() as u32)))
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Tags {
    pub tank: bool,
    pub heal: bool,
    pub crit: bool,
    pub attack_speed: bool,
    pub burst: bool,
}

/// Hand-picked Mayhem augments that change what you should build against them.
fn name_tags(name: &str) -> Tags {
    let n: String = name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    let any = |l: &[&str]| l.contains(&n.as_str());
    Tags {
        tank: any(&[
            "titansresolve",
            "goliath",
            "courageofthecolossus",
            "tankengine",
            "steelyourheart",
            "heavyhitter",
            "celestialbody",
            "mountainsoul",
            "adamant",
            "impassable",
            "perseverance",
            "tankitorleaveit",
            "outlawsgrit",
            "armoredimpact",
        ]),
        heal: any(&[
            "soulsiphon",
            "firstaidkit",
            "vampirism",
            "goredrink",
            "criticalhealing",
            "rejuvenation",
            "natureishealing",
            "dawnbringersresolve",
            "oceansoul",
        ]),
        crit: any(&[
            "itscritical",
            "jeweledgauntlet",
            "vulnerability",
            "criticalrhythm",
            "upgradeinfinityedge",
            "criticalhealing",
        ]),
        attack_speed: any(&["typhoon", "dualwield", "doubletap", "tapdancer", "symphonyofwar", "mysticpunch", "deft"]),
        burst: any(&["glasscannon", "cruelty", "itskillingtime"]),
    }
}

/// Tags every candidate agrees on: a shared icon only counts when all its augments do the same.
fn seen_tags(s: &SeenAugment, names: &HashMap<AugmentId, String>) -> Tags {
    let tags: Vec<Option<Tags>> = s.ids.iter().map(|id| names.get(id).map(|n| name_tags(n))).collect();
    let all = |f: fn(&Tags) -> bool| !tags.is_empty() && tags.iter().all(|t| t.as_ref().is_some_and(f));
    Tags {
        tank: all(|t| t.tank),
        heal: all(|t| t.heal),
        crit: all(|t| t.crit),
        attack_speed: all(|t| t.attack_speed),
        burst: all(|t| t.burst),
    }
}

/// Add read augments to the threat rows and their effect to each foe's
/// inventory stats. Returns per-foe tags (foe order) for [`adjust_needs`].
pub fn apply(a: &mut Assessed, input: &EnemyAugmentInput, names: Option<&HashMap<AugmentId, String>>) -> Vec<Tags> {
    let empty = HashMap::new();
    let names = names.unwrap_or(&empty);
    let mut out = vec![Tags::default(); a.foes.len()];
    for ((foe, threat), tags) in a.foes.iter_mut().zip(&mut a.threats).zip(&mut out) {
        let Some(e) = seen_for(input.seen, threat.champ) else { continue };
        let ranks = input.ranks.iter().find(|r| r.champ == threat.champ);
        for s in &e.augments {
            let t = seen_tags(s, names);
            tags.tank |= t.tank;
            tags.heal |= t.heal;
            tags.crit |= t.crit;
            tags.attack_speed |= t.attack_speed;
            tags.burst |= t.burst;
            threat.augments.push(augment_rank(s, ranks, names));
        }
        if tags.tank {
            foe.inv.hp += AUG_HP;
            foe.inv.armor += AUG_RESIST;
            foe.inv.mr += AUG_RESIST;
        }
        if tags.crit {
            foe.inv.crit += AUG_CRIT;
        }
        if tags.attack_speed {
            foe.inv.attack_speed += AUG_AS;
        }
        for (on, flag) in
            [(tags.tank, "tank"), (tags.heal, "heals"), (tags.crit, "crit"), (tags.attack_speed, "on-hit")]
        {
            if on && !threat.flags.iter().any(|f| f == flag) {
                threat.flags.push(flag.into());
            }
        }
    }
    out
}

fn augment_rank(s: &SeenAugment, ranks: Option<&ChampAugments>, names: &HashMap<AugmentId, String>) -> AugmentRank {
    let id = s.ids.first().copied().unwrap_or(0);
    let mut name = names.get(&id).cloned().unwrap_or_else(|| format!("#{id}"));
    if s.ids.len() > 1 {
        name += &format!(" (or {} with this icon)", s.ids.len() - 1);
    }
    match rank_of(ranks, id) {
        Some((r, pos, of)) => AugmentRank { id, name, rarity: Some(r), rank: Some(pos), of, synergy: None },
        None => AugmentRank { id, name, rarity: s.rarity, rank: None, of: 0, synergy: None },
    }
}

/// Heal and burst augments raise anti-heal / anti-burst toward the foe's threat.
pub fn adjust_needs(needs: &mut Needs, a: &Assessed, tags: &[Tags]) {
    let lerp = |(lo, hi): (f32, f32), score: f32| lo + (hi - lo) * clamp01(score / 100.0);
    for (i, (foe, t)) in a.foes.iter().zip(tags).enumerate() {
        for (on, k, range) in [(t.heal, ANTI_HEAL, AUG_HEAL), (t.burst, ANTI_BURST, AUG_BURST)] {
            let v = lerp(range, foe.score);
            if on && k < N && v > needs.p[k] {
                needs.p[k] = v;
                needs.driver[k] = Some(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(champ: ChampKey, team: Team, level: u32) -> Player {
        Player {
            riot_id: String::new(),
            champ,
            team,
            position: None,
            level,
            kills: 0,
            deaths: 0,
            assists: 0,
            cs: 0,
            ward_score: 0.0,
            items: vec![],
            dead: false,
            respawn_in: 0.0,
            smite: false,
        }
    }

    fn snap(time: f32, enemy_levels: [u32; 2]) -> GameSnapshot {
        GameSnapshot {
            time,
            mode: Mode::Mayhem,
            map: 12,
            me: 0,
            my_gold: 0.0,
            players: vec![
                player(1, Team::Order, 7),
                player(10, Team::Chaos, enemy_levels[0]),
                player(11, Team::Chaos, enemy_levels[1]),
            ],
            events: vec![],
            my_stats: None,
            my_abilities: None,
        }
    }

    fn seen(champ: ChampKey, n: usize) -> EnemyAugments {
        EnemyAugments {
            champ,
            augments: (0..n).map(|i| SeenAugment { ids: vec![1000 + i as u32], rarity: None }).collect(),
            time: 0.0,
            level: 0,
        }
    }

    #[test]
    fn expected_by_level() {
        assert_eq!([3, 6, 7, 10, 11, 15, 18].map(expected), [1, 1, 2, 2, 3, 4, 4]);
    }

    #[test]
    fn stale_counts_missing_reads() {
        let s = snap(100.0, [7, 6]);
        assert_eq!(stale_enemies(&s, &[]), 2);
        assert_eq!(stale_enemies(&s, &[seen(10, 1), seen(11, 1)]), 1);
        assert_eq!(stale_enemies(&s, &[seen(10, 2), seen(11, 1)]), 0);
    }

    #[test]
    fn hint_waits_for_grace_and_clears_once_read() {
        let s = snap(30.0, [3, 3]);
        assert!(tab_hint(&s, &[], Some(15.0)).is_none(), "too early");
        let h = tab_hint(&s, &[], Some(5.0)).unwrap();
        assert_eq!(h.reason, "No enemy augments read yet");
        assert!(tab_hint(&s, &[seen(10, 1), seen(11, 1)], Some(5.0)).is_none(), "read");
        assert!(tab_hint(&s, &[], None).is_none());
        let mut aram = s.clone();
        aram.mode = Mode::Aram;
        assert!(tab_hint(&aram, &[], Some(0.0)).is_none());
    }

    #[test]
    fn hint_comes_sooner_while_dead() {
        let mut s = snap(300.0, [11, 10]);
        let read = [seen(10, 2), seen(11, 2)];
        assert!(tab_hint(&s, &read, Some(294.0)).is_none());
        s.players[0].dead = true;
        s.players[0].respawn_in = 12.0;
        let h = tab_hint(&s, &read, Some(294.0)).unwrap();
        assert!(h.reason.starts_with("1 enemy has an unread augment"), "{}", h.reason);
        s.players[0].respawn_in = 2.0;
        assert!(tab_hint(&s, &read, Some(294.0)).is_none(), "about to respawn");
    }

    #[test]
    fn candidates_by_tint_list_and_rank() {
        let ranks = ChampAugments { champ: 10, prismatic: vec![7, 8], gold: vec![1, 2, 3, 4], silver: vec![5] };
        assert_eq!(order_candidates(&[9, 4, 1], None, Some(&ranks)), [1, 4, 9]);
        assert_eq!(order_candidates(&[1, 8], Some(Rarity::Prismatic), Some(&ranks)), [8, 1]);
        assert_eq!(order_candidates(&[3, 2], None, None), [3, 2]);
    }

    #[test]
    fn tags_need_agreement_across_shared_icons() {
        let names: HashMap<AugmentId, String> =
            [(1, "Goliath"), (2, "Tank Engine"), (3, "Blade Waltz"), (4, "Soul Siphon")]
                .map(|(i, n)| (i, n.to_string()))
                .into();
        let s = |ids: &[u32]| SeenAugment { ids: ids.to_vec(), rarity: None };
        assert!(seen_tags(&s(&[1, 2]), &names).tank);
        assert_eq!(seen_tags(&s(&[1, 3]), &names), Tags::default());
        assert!(seen_tags(&s(&[4]), &names).heal);
    }
}
