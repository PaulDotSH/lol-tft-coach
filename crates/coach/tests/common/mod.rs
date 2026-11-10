//! Hand-built fixtures: a small StaticData with realistic items/champions,
//! a 10-player lobby and Ahri mid lolalytics stats.
#![allow(dead_code)]

use model::*;
use std::collections::HashMap;

// ---- items ----
pub const BOOTS: ItemId = 1001;
pub const TOME: ItemId = 1052;
pub const WAND: ItemId = 1026;
pub const ROD: ItemId = 1058;
pub const CLOTH: ItemId = 1029;
pub const MANTLE: ItemId = 1033;
pub const RUBY: ItemId = 1028;
pub const LONG_SWORD: ItemId = 1036;
pub const DAGGER: ItemId = 1042;
pub const CLOAK: ItemId = 1018;
pub const SEEKERS: ItemId = 2420;
pub const OBLIVION: ItemId = 3916;
pub const EXECUTIONERS: ItemId = 3123;
pub const SORCS: ItemId = 3020;
pub const MERCS: ItemId = 3111;
pub const PLATED: ItemId = 3047;
pub const BERSERKERS: ItemId = 3006;
pub const ZHONYAS: ItemId = 3157;
pub const MORELLO: ItemId = 3165;
pub const BANSHEES: ItemId = 3102;
pub const VOID_STAFF: ItemId = 3135;
pub const LUDENS: ItemId = 6655;
pub const RABADONS: ItemId = 3089;
pub const RANDUINS: ItemId = 3143;
pub const THORNMAIL: ItemId = 3075;
pub const SPIRIT_VISAGE: ItemId = 3065;
pub const INFINITY_EDGE: ItemId = 3031;
pub const LORD_DOMINIKS: ItemId = 3036;
pub const MORTAL_REMINDER: ItemId = 3033;
pub const GUARDIAN_ANGEL: ItemId = 3026;
pub const YOUMUUS: ItemId = 3142;
pub const EDGE_OF_NIGHT: ItemId = 3814;
/// ARAM-only AP item.
pub const GUARDIANS_ORB: ItemId = 3112;

// ---- champions ----
pub const AHRI: ChampKey = 103;
pub const ZED: ChampKey = 238;
pub const SORAKA: ChampKey = 16;
pub const AATROX: ChampKey = 266;
pub const VLADIMIR: ChampKey = 8;
pub const MUNDO: ChampKey = 36;
pub const JINX: ChampKey = 222;
pub const LEONA: ChampKey = 89;
pub const MALPHITE: ChampKey = 54;
pub const LUX: ChampKey = 99;
pub const SYNDRA: ChampKey = 134;
pub const LEE_SIN: ChampKey = 64;
pub const GAREN: ChampKey = 86;
pub const CAITLYN: ChampKey = 51;
pub const THRESH: ChampKey = 412;
pub const VAYNE: ChampKey = 67;
pub const DARIUS: ChampKey = 122;
pub const JARVAN: ChampKey = 59;
pub const SONA: ChampKey = 37;
pub const EZREAL: ChampKey = 81;
pub const KAYLE: ChampKey = 10;

fn item(id: ItemId, name: &str, cost: u32, from: &[ItemId], stats: ItemStats, answers: Profile) -> Item {
    Item {
        id,
        name: name.into(),
        cost,
        from: from.to_vec(),
        into: vec![],
        stats,
        answers,
        boots: false,
        completed: true,
        rift: true,
        aram: true,
        tags: vec![],
    }
}

fn comp(mut i: Item) -> Item {
    i.completed = false;
    i
}

fn boots(mut i: Item) -> Item {
    i.boots = true;
    i
}

fn s() -> ItemStats {
    ItemStats::default()
}

fn a() -> Profile {
    Profile::default()
}

pub fn items() -> Vec<Item> {
    vec![
        comp(boots(item(BOOTS, "Boots", 300, &[], ItemStats { move_speed: 25.0, ..s() }, a()))),
        comp(item(TOME, "Amplifying Tome", 400, &[], ItemStats { ap: 20.0, ..s() }, a())),
        comp(item(WAND, "Blasting Wand", 850, &[], ItemStats { ap: 45.0, ..s() }, a())),
        comp(item(ROD, "Needlessly Large Rod", 1200, &[], ItemStats { ap: 60.0, ..s() }, a())),
        comp(item(CLOTH, "Cloth Armor", 300, &[], ItemStats { armor: 15.0, ..s() }, a())),
        comp(item(MANTLE, "Null-Magic Mantle", 400, &[], ItemStats { mr: 25.0, ..s() }, a())),
        comp(item(RUBY, "Ruby Crystal", 400, &[], ItemStats { hp: 150.0, ..s() }, a())),
        comp(item(LONG_SWORD, "Long Sword", 350, &[], ItemStats { ad: 10.0, ..s() }, a())),
        comp(item(DAGGER, "Dagger", 250, &[], ItemStats { attack_speed: 0.1, ..s() }, a())),
        comp(item(CLOAK, "Cloak of Agility", 600, &[], ItemStats { crit: 0.15, ..s() }, a())),
        comp(item(SEEKERS, "Seeker's Armguard", 1000, &[TOME, CLOTH], ItemStats { ap: 20.0, armor: 15.0, ..s() }, a())),
        comp(item(
            OBLIVION,
            "Oblivion Orb",
            800,
            &[TOME],
            ItemStats { ap: 25.0, ..s() },
            Profile { anti_heal: 0.6, ..a() },
        )),
        comp(item(
            EXECUTIONERS,
            "Executioner's Calling",
            800,
            &[LONG_SWORD],
            ItemStats { ad: 15.0, ..s() },
            Profile { anti_heal: 0.6, ..a() },
        )),
        boots(item(
            SORCS,
            "Sorcerer's Shoes",
            1100,
            &[BOOTS],
            ItemStats { magic_pen_flat: 12.0, ..s() },
            Profile { magic_pen: 0.4, ..a() },
        )),
        boots(item(
            MERCS,
            "Mercury's Treads",
            1250,
            &[BOOTS, MANTLE],
            ItemStats { mr: 20.0, ..s() },
            Profile { mr: 0.4, tenacity: 1.0, cleanse: 0.2, ..a() },
        )),
        boots(item(
            PLATED,
            "Plated Steelcaps",
            1200,
            &[BOOTS, CLOTH],
            ItemStats { armor: 20.0, ..s() },
            Profile { armor: 0.5, anti_attack_speed: 0.8, anti_crit: 0.3, ..a() },
        )),
        boots(item(
            BERSERKERS,
            "Berserker's Greaves",
            1100,
            &[BOOTS, DAGGER],
            ItemStats { attack_speed: 0.25, ..s() },
            a(),
        )),
        item(
            ZHONYAS,
            "Zhonya's Hourglass",
            3250,
            &[SEEKERS, WAND],
            ItemStats { ap: 105.0, armor: 50.0, ..s() },
            Profile { anti_burst: 1.0, armor: 0.5, ..a() },
        ),
        item(
            MORELLO,
            "Morellonomicon",
            2950,
            &[OBLIVION, RUBY],
            ItemStats { ap: 75.0, hp: 350.0, ..s() },
            Profile { anti_heal: 1.0, hp: 0.3, ..a() },
        ),
        item(
            BANSHEES,
            "Banshee's Veil",
            3000,
            &[WAND, MANTLE],
            ItemStats { ap: 105.0, mr: 40.0, ..s() },
            Profile { mr: 0.6, anti_burst: 0.6, ..a() },
        ),
        item(
            VOID_STAFF,
            "Void Staff",
            3000,
            &[WAND, TOME],
            ItemStats { ap: 95.0, magic_pen_pct: 0.4, ..s() },
            Profile { magic_pen: 1.0, ..a() },
        ),
        item(LUDENS, "Luden's Companion", 2900, &[WAND, TOME, TOME], ItemStats { ap: 95.0, mana: 600.0, ..s() }, a()),
        item(RABADONS, "Rabadon's Deathcap", 3500, &[ROD, WAND], ItemStats { ap: 130.0, ..s() }, a()),
        item(
            RANDUINS,
            "Randuin's Omen",
            2700,
            &[RUBY, CLOTH],
            ItemStats { hp: 350.0, armor: 75.0, ..s() },
            Profile { armor: 0.9, anti_crit: 1.0, anti_attack_speed: 0.4, hp: 0.4, ..a() },
        ),
        item(
            THORNMAIL,
            "Thornmail",
            2450,
            &[CLOTH, RUBY],
            ItemStats { armor: 75.0, hp: 150.0, ..s() },
            Profile { armor: 0.8, anti_heal: 0.8, anti_attack_speed: 0.3, ..a() },
        ),
        item(
            SPIRIT_VISAGE,
            "Spirit Visage",
            2700,
            &[RUBY, MANTLE],
            ItemStats { hp: 400.0, mr: 50.0, ..s() },
            Profile { mr: 0.8, hp: 0.4, ..a() },
        ),
        item(
            INFINITY_EDGE,
            "Infinity Edge",
            3450,
            &[LONG_SWORD, CLOAK],
            ItemStats { ad: 65.0, crit: 0.25, ..s() },
            a(),
        ),
        item(
            LORD_DOMINIKS,
            "Lord Dominik's Regards",
            3000,
            &[LONG_SWORD, CLOAK],
            ItemStats { ad: 35.0, crit: 0.25, armor_pen_pct: 0.35, ..s() },
            Profile { armor_pen: 1.0, ..a() },
        ),
        item(
            MORTAL_REMINDER,
            "Mortal Reminder",
            3000,
            &[EXECUTIONERS, CLOAK],
            ItemStats { ad: 35.0, crit: 0.25, armor_pen_pct: 0.3, ..s() },
            Profile { anti_heal: 1.0, armor_pen: 0.6, ..a() },
        ),
        item(
            GUARDIAN_ANGEL,
            "Guardian Angel",
            3200,
            &[LONG_SWORD, CLOTH],
            ItemStats { ad: 55.0, armor: 45.0, ..s() },
            Profile { anti_burst: 0.8, armor: 0.3, ..a() },
        ),
        item(YOUMUUS, "Youmuu's Ghostblade", 2800, &[LONG_SWORD], ItemStats { ad: 55.0, lethality: 18.0, ..s() }, a()),
        item(
            EDGE_OF_NIGHT,
            "Edge of Night",
            2800,
            &[LONG_SWORD, RUBY],
            ItemStats { ad: 50.0, lethality: 15.0, hp: 250.0, ..s() },
            Profile { anti_burst: 0.3, ..a() },
        ),
        {
            let mut i = item(
                GUARDIANS_ORB,
                "Guardian's Orb",
                950,
                &[],
                ItemStats { ap: 90.0, ..s() },
                Profile { magic_pen: 1.0, anti_burst: 1.0, ..a() },
            );
            i.rift = false;
            i
        },
    ]
}

fn champ(key: ChampKey, name: &str, classes: &[ChampClass], traits: Traits) -> Champion {
    Champion {
        key,
        id: name.replace([' ', '.', '\''], ""),
        name: name.into(),
        slug: name.to_lowercase().replace([' ', '.', '\''], ""),
        classes: classes.to_vec(),
        traits,
        base: BaseStats::default(),
    }
}

fn t(magic_share: f32) -> Traits {
    Traits { magic_share, ..Default::default() }
}

pub fn champs() -> Vec<Champion> {
    use ChampClass::*;
    vec![
        champ(AHRI, "Ahri", &[Mage, Assassin], Traits { burst: 0.6, cc: 0.4, ..t(0.95) }),
        champ(ZED, "Zed", &[Assassin], Traits { burst: 0.9, ..t(0.05) }),
        champ(SORAKA, "Soraka", &[Support, Mage], Traits { heals: 1.0, cc: 0.3, ..t(0.9) }),
        champ(AATROX, "Aatrox", &[Fighter], Traits { heals: 0.7, cc: 0.3, ..t(0.05) }),
        champ(VLADIMIR, "Vladimir", &[Mage], Traits { heals: 0.8, burst: 0.5, ..t(1.0) }),
        champ(MUNDO, "Dr. Mundo", &[Tank, Fighter], Traits { heals: 0.8, tank: 0.9, ..t(0.1) }),
        champ(JINX, "Jinx", &[Marksman], Traits { cc: 0.2, ..t(0.1) }),
        champ(LEONA, "Leona", &[Tank, Support], Traits { cc: 1.0, tank: 0.8, ..t(0.3) }),
        champ(MALPHITE, "Malphite", &[Tank], Traits { cc: 0.8, tank: 0.9, burst: 0.6, ..t(0.6) }),
        champ(LUX, "Lux", &[Mage, Support], Traits { cc: 0.6, burst: 0.7, ..t(1.0) }),
        champ(SYNDRA, "Syndra", &[Mage], Traits { cc: 0.5, burst: 0.8, ..t(1.0) }),
        champ(LEE_SIN, "Lee Sin", &[Fighter, Assassin], Traits { cc: 0.3, burst: 0.6, ..t(0.1) }),
        champ(GAREN, "Garen", &[Fighter], Traits { true_damage: 0.4, tank: 0.4, ..t(0.0) }),
        champ(CAITLYN, "Caitlyn", &[Marksman], Traits { cc: 0.3, ..t(0.05) }),
        champ(THRESH, "Thresh", &[Support, Tank], Traits { cc: 0.9, tank: 0.5, ..t(0.5) }),
        champ(VAYNE, "Vayne", &[Marksman], Traits { true_damage: 0.5, on_hit: 0.8, cc: 0.3, ..t(0.0) }),
        champ(DARIUS, "Darius", &[Fighter], Traits { tank: 0.4, heals: 0.3, cc: 0.4, ..t(0.0) }),
        champ(JARVAN, "Jarvan IV", &[Tank, Fighter], Traits { cc: 0.7, tank: 0.6, ..t(0.1) }),
        champ(SONA, "Sona", &[Support, Mage], Traits { heals: 0.5, cc: 0.5, ..t(0.9) }),
        champ(EZREAL, "Ezreal", &[Marksman], t(0.3)),
        champ(KAYLE, "Kayle", &[Fighter, Mage], Traits { on_hit: 0.6, ..t(0.5) }),
    ]
}

pub fn sd() -> StaticData {
    StaticData {
        version: "16.21.1".into(),
        items: items().into_iter().map(|i| (i.id, i)).collect(),
        champs: champs().into_iter().map(|c| (c.key, c)).collect(),
        item_groups: Default::default(),
        runes: Default::default(),
    }
}

// ---- live game ----

pub fn player(champ: ChampKey, team: Team, position: Option<Lane>) -> Player {
    let cs = match position {
        Some(Lane::Jungle) => 70,
        Some(Lane::Support) => 15,
        _ => 90,
    };
    Player {
        riot_id: format!("p{champ}"),
        champ,
        team,
        position,
        level: 9,
        kills: 2,
        deaths: 2,
        assists: 2,
        cs,
        ward_score: 8.0,
        items: vec![RUBY],
        dead: false,
        respawn_in: 0.0,
        smite: false,
    }
}

pub trait PlayerExt {
    fn kda(self, k: u32, d: u32, a: u32) -> Self;
    fn lvl(self, l: u32) -> Self;
    fn cs(self, c: u32) -> Self;
    fn items(self, items: &[ItemId]) -> Self;
}

impl PlayerExt for Player {
    fn kda(mut self, k: u32, d: u32, a: u32) -> Self {
        (self.kills, self.deaths, self.assists) = (k, d, a);
        self
    }
    fn lvl(mut self, l: u32) -> Self {
        self.level = l;
        self
    }
    fn cs(mut self, c: u32) -> Self {
        self.cs = c;
        self
    }
    fn items(mut self, items: &[ItemId]) -> Self {
        self.items = items.to_vec();
        self
    }
}

/// Me = Ahri mid at index 0, allies 1..5, the given enemies at 5..10 (Chaos).
pub fn lobby(enemies: [(ChampKey, Lane); 5]) -> Vec<Player> {
    let mut v = vec![player(AHRI, Team::Order, Some(Lane::Middle))];
    for (c, l) in [(GAREN, Lane::Top), (JARVAN, Lane::Jungle), (EZREAL, Lane::Bottom), (SONA, Lane::Support)] {
        v.push(player(c, Team::Order, Some(l)));
    }
    for (c, l) in enemies {
        v.push(player(c, Team::Chaos, Some(l)));
    }
    v
}

pub fn snap(players: Vec<Player>, events: Vec<GameEvent>) -> GameSnapshot {
    GameSnapshot { time: 900.0, mode: Mode::Rift, map: 11, me: 0, my_gold: 500.0, players, events, my_stats: None, my_abilities: None }
}

pub fn kill(time: f32, killer: usize, victim: usize) -> GameEvent {
    GameEvent::Kill { time, killer: Some(killer), victim, assisters: vec![] }
}

/// `n` kills of me (index 0) by player `killer`.
pub fn killed_me(killer: usize, n: usize) -> Vec<GameEvent> {
    (0..n).map(|i| kill(300.0 + 60.0 * i as f32, killer, 0)).collect()
}

pub const STANDARD: [(ChampKey, Lane); 5] =
    [(DARIUS, Lane::Top), (LEE_SIN, Lane::Jungle), (ZED, Lane::Middle), (JINX, Lane::Bottom), (THRESH, Lane::Support)];
pub const HEALERS: [(ChampKey, Lane); 5] = [
    (AATROX, Lane::Top),
    (MUNDO, Lane::Jungle),
    (VLADIMIR, Lane::Middle),
    (JINX, Lane::Bottom),
    (SORAKA, Lane::Support),
];

// ---- lolalytics ----

pub fn row(id: ItemId, games: u32, wr_pct: f32) -> ItemStat {
    ItemStat { id, games, wins: (games as f32 * wr_pct / 100.0).round() as u32 }
}

pub fn matchup(champ: ChampKey, vs_wr: f32, games: u32, lane: Lane) -> Matchup {
    Matchup { champ, vs_wr, games, default_lane: Some(lane) }
}

pub fn ahri_stats() -> ChampStats {
    ChampStats {
        champ: AHRI,
        lane: Some(Lane::Middle),
        mode: Mode::Rift,
        patch: "16.21".into(),
        win_rate: 51.0,
        pick_rate: 8.0,
        games: 100_000,
        lanes: HashMap::from([(Lane::Middle, 95.0)]),
        slots: vec![
            vec![row(LUDENS, 20_000, 55.0), row(ZHONYAS, 2_000, 50.0)],
            vec![row(RABADONS, 10_000, 54.0), row(LUDENS, 3_000, 53.0), row(ZHONYAS, 2_000, 51.0)],
            vec![
                row(VOID_STAFF, 8_000, 55.0),
                row(BANSHEES, 3_000, 53.0),
                row(ZHONYAS, 4_000, 51.0),
                row(MORELLO, 1_000, 49.0),
            ],
            vec![
                row(VOID_STAFF, 3_000, 55.0),
                row(RABADONS, 3_000, 54.0),
                row(ZHONYAS, 2_000, 52.0),
                row(BANSHEES, 2_000, 53.0),
                row(MORELLO, 1_500, 49.0),
            ],
            vec![
                row(BANSHEES, 2_000, 54.0),
                row(ZHONYAS, 1_500, 53.0),
                row(MORELLO, 1_000, 50.0),
                row(VOID_STAFF, 1_000, 54.0),
            ],
        ],
        boots: vec![row(SORCS, 30_000, 54.0), row(MERCS, 4_000, 51.0), row(PLATED, 3_000, 51.0)],
        early: vec![
            EarlySet { items: vec![1056, 2003, 2003], win_rate: 52.0, pick_rate: 70.0, games: 50_000 },
            EarlySet { items: vec![TOME, 2003], win_rate: 56.0, pick_rate: 0.3, games: 200 },
        ],
        counters: vec![
            matchup(SYNDRA, 44.0, 5_000, Lane::Middle),
            matchup(ZED, 49.0, 4_000, Lane::Middle),
            matchup(LUX, 53.0, 3_000, Lane::Middle),
            matchup(VLADIMIR, 50.0, 3_000, Lane::Middle),
            matchup(DARIUS, 51.0, 200, Lane::Top),
            matchup(JINX, 50.0, 200, Lane::Bottom),
        ],
        benchmarks: Benchmarks {
            cs_per_min: Some(7.4),
            gold_per_min: Some(420.0),
            damage_per_min: Some(950.0),
            vision_per_min: None,
        },
        runes: None,
        coachless_runes: None,
        coachless: None,
        summoners: vec![],
        skill_order: "QWEQQRQWQWRWWEE".into(),
        skill_order_games: 5_000,
        skill_priority: "QWE".into(),
    }
}
