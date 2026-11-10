//! Enemy augments read off the Tab scoreboard (Mayhem).

mod common;

use coach::{EnemyAugmentInput, GameInput, game_advice};
use common::*;
use model::*;

const SOUL_SIPHON: AugmentId = 1077;
const GOLIATH: AugmentId = 1041;
const BLADE_WALTZ: AugmentId = 1006;
const BLUNT_FORCE: AugmentId = 1007;

fn mayhem_data() -> (ChampAugments, MayhemData) {
    let names = [
        (SOUL_SIPHON, "Soul Siphon"),
        (GOLIATH, "Goliath"),
        (BLADE_WALTZ, "Blade Waltz"),
        (BLUNT_FORCE, "Blunt Force"),
    ];
    let md = MayhemData {
        patch: "16_21".into(),
        augment_names: names.map(|(i, n)| (i, n.to_string())).into(),
        champ_tiers: Default::default(),
    };
    (ChampAugments { champ: AHRI, prismatic: vec![], gold: vec![], silver: vec![] }, md)
}

fn seen(champ: ChampKey, ids: &[&[AugmentId]]) -> EnemyAugments {
    EnemyAugments {
        champ,
        augments: ids.iter().map(|i| SeenAugment { ids: i.to_vec(), rarity: None }).collect(),
        time: 60.0,
        level: 7,
    }
}

fn advise(s: &GameSnapshot, input: EnemyAugmentInput) -> GameAdvice {
    let sd = sd();
    let (mine, md) = mayhem_data();
    game_advice(&GameInput {
        snap: s,
        sd: &sd,
        stats: Some(&ahri_stats()),
        augments: Some((&mine, &md)),
        offered: &[],
        gold_history: &[],
        my_augments: &[],
        enemy_augments: input, keep: None, matchup: None,
    })
}

fn mayhem_snap() -> GameSnapshot {
    let mut s = snap(lobby(STANDARD), vec![]);
    s.mode = Mode::Mayhem;
    s.time = 120.0;
    s
}

#[test]
fn healing_augment_puts_anti_heal_in_path_and_shows_on_threat_row() {
    let s = mayhem_snap();
    let base = advise(&s, Default::default());
    // Without healers Morello is at most the last filler (Banshee's MR is wrong into this AD team).
    let morello = |a: &GameAdvice| a.path.iter().position(|p| p.item == MORELLO).unwrap_or(usize::MAX);
    assert!(morello(&base) >= base.path.len() - 1, "{:?}", base.path);

    let enemy = s.players[7].champ;
    let ranks = [ChampAugments { champ: enemy, prismatic: vec![], gold: vec![9, SOUL_SIPHON], silver: vec![] }];
    let read = [seen(enemy, &[&[SOUL_SIPHON]])];
    let a = advise(&s, EnemyAugmentInput { seen: &read, ranks: &ranks, stale_since: None });
    // With it, Morello is there for the augment's holder.
    let m = a.path.iter().find(|p| p.item == MORELLO).expect("Morello in path");
    let name = &a.threats.iter().find(|t| t.champ == enemy).unwrap().name;
    assert!(m.reason.starts_with(&format!("Anti-heal: {name}")), "{}", m.reason);
    let t = a.threats.iter().find(|t| t.champ == enemy).unwrap();
    assert!(t.flags.iter().any(|f| f == "heals"), "{:?}", t.flags);
    assert_eq!(t.augments.len(), 1);
    let r = &t.augments[0];
    assert_eq!(
        (r.id, r.name.as_str(), r.rarity, r.rank, r.of),
        (SOUL_SIPHON, "Soul Siphon", Some(Rarity::Gold), Some(1), 2)
    );
    assert!(a.threats.iter().filter(|t| t.champ != enemy).all(|t| t.augments.is_empty()));
}

#[test]
fn tank_augment_flags_tank_and_shared_icons_stay_neutral() {
    let s = mayhem_snap();
    let enemy = s.players[6].champ;
    let read = [seen(enemy, &[&[GOLIATH]])];
    let a = advise(&s, EnemyAugmentInput { seen: &read, ranks: &[], stale_since: None });
    let t = a.threats.iter().find(|t| t.champ == enemy).unwrap();
    assert!(t.flags.iter().any(|f| f == "tank"), "{:?}", t.flags);

    // Blade Waltz and Blunt Force share one icon: named as the first, no tags.
    let read = [seen(enemy, &[&[BLADE_WALTZ, BLUNT_FORCE]])];
    let b = advise(&s, EnemyAugmentInput { seen: &read, ranks: &[], stale_since: None });
    let t = b.threats.iter().find(|t| t.champ == enemy).unwrap();
    assert_eq!(t.augments[0].name, "Blade Waltz (or 1 with this icon)");
    assert_eq!(b.path, advise(&s, Default::default()).path);
}

#[test]
fn tab_hint_until_every_enemy_is_read() {
    let mut s = mayhem_snap();
    let a = advise(&s, EnemyAugmentInput { seen: &[], ranks: &[], stale_since: Some(90.0) });
    let h = a.tab_hint.expect("hint");
    assert_eq!(h.text, "Hold Tab for 1 s to read enemy augments");

    let read: Vec<EnemyAugments> =
        s.players.iter().filter(|p| p.team == Team::Chaos).map(|p| seen(p.champ, &[&[GOLIATH]])).collect();
    for p in &mut s.players {
        p.level = 6;
    }
    let a = advise(&s, EnemyAugmentInput { seen: &read, ranks: &[], stale_since: None });
    assert!(a.tab_hint.is_none());
    s.mode = Mode::Aram;
    assert!(advise(&s, EnemyAugmentInput { seen: &[], ranks: &[], stale_since: Some(0.0) }).tab_hint.is_none());
}
