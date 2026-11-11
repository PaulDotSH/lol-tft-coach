//! Win probability from the scoreboard: the model's maths against a hand-set table
//! (`fixtures/coach/winprob_handset.toml`), never the shipped one.

mod common;

use coach::winprob::{Input, KEYS, Model, State, eval, win_prob};
use coach::{Curve, GameInput, game_advice};
use common::*;
use model::*;
use std::collections::HashMap;

const HANDSET: &str = include_str!("../../../fixtures/coach/winprob_handset.toml");

fn model() -> Model {
    Model::parse(HANDSET).unwrap()
}

fn at(time: f32, events: Vec<GameEvent>) -> GameSnapshot {
    GameSnapshot { time, ..snap(lobby(STANDARD), events) }
}

fn even() -> GameSnapshot {
    at(900.0, vec![])
}

fn win_with(s: &GameSnapshot, curves: &HashMap<ChampKey, Curve>, history: &[GoldSample]) -> Option<WinProb> {
    let sd = sd();
    win_prob(&Input { model: &model(), snap: s, sd: &sd, curves, history })
}

fn win(s: &GameSnapshot) -> WinProb {
    win_with(s, &HashMap::new(), &[]).unwrap()
}

/// 1,000 games in each length bucket at these win rates.
fn curve(wr: [f32; 4]) -> Curve {
    (None, [0, 20, 30, 40].iter().zip(wr).map(|(&from_min, w)| LengthWin { from_min, games: 1000, wins: (w * 10.0) as u32 }).collect())
}

fn curves(s: &GameSnapshot, f: impl Fn(&Player) -> [f32; 4]) -> HashMap<ChampKey, Curve> {
    s.players.iter().map(|p| (p.champ, curve(f(p)))).collect()
}

fn dragon(time: f32, team: Team) -> GameEvent {
    GameEvent::Dragon { time, team: Some(team), kind: "Fire".into() }
}

fn tower(team: Team, tier: u32) -> GameEvent {
    GameEvent::Tower { time: 600.0, team, lane: Some(Lane::Middle), tier }
}

fn inhib(time: f32, team: Team, respawned: bool) -> GameEvent {
    GameEvent::Inhib { time, team, lane: Some(Lane::Middle), respawned }
}

fn stake<'a>(w: &'a WinProb, name: &str) -> Option<&'a Stake> {
    w.stakes.iter().find(|s| s.name == name)
}

#[test]
fn no_fit_no_number() {
    let (sd, s) = (sd(), even());
    let shipped = Model::shipped();
    assert_eq!(shipped.games, 0, "ships without a fit until tools/winprob has run");
    assert_eq!(win_prob(&Input { model: shipped, snap: &s, sd: &sd, curves: &HashMap::new(), history: &[] }), None);
    // A fit without a table for the mode.
    let rift_only = Model::parse("patch = \"16.22\"\ngames = 500\nknots = [0, 20]\n[rift]\ngold = [5.0, 6.0]").unwrap();
    let aram = GameSnapshot { mode: Mode::Aram, map: 12, ..even() };
    assert!(win_prob(&Input { model: &rift_only, snap: &s, sd: &sd, curves: &HashMap::new(), history: &[] }).is_some());
    assert_eq!(win_prob(&Input { model: &rift_only, snap: &aram, sd: &sd, curves: &HashMap::new(), history: &[] }), None);
}

#[test]
fn tables_use_known_keys_and_one_value_per_knot() {
    for m in [Model::shipped().clone(), model()] {
        for t in [&m.rift, &m.aram] {
            for (k, v) in &t.beta {
                assert!(KEYS.contains(&k.as_str()), "unknown feature {k}");
                assert_eq!(v.len(), m.knots.len(), "{k}");
            }
        }
    }
    assert_eq!(model().rift.beta.len(), KEYS.len());
}

#[test]
fn coefficients_run_straight_between_knots() {
    let m = Model::parse("patch = \"t\"\ngames = 1\nknots = [0, 10, 20]\n[rift]\ntowers = [0.0, 1.0, 3.0]").unwrap();
    let s = State { towers: [1, 0], ..Default::default() };
    let p = |z: f32| 1.0 / (1.0 + (-z).exp());
    for (minute, z) in [(0.0, 0.0), (5.0, 0.5), (10.0, 1.0), (15.0, 2.0), (20.0, 3.0), (55.0, 3.0)] {
        assert!((eval(&m.knots, &m.rift, &s, minute) - p(z)).abs() < 1e-6, "minute {minute}");
    }
    let two = State { towers: [2, 0], ..Default::default() };
    assert!((eval(&m.knots, &m.rift, &two, 10.0) - p(2.0)).abs() < 1e-6);
}

#[test]
fn swapping_sides_mirrors_the_number() {
    let mut p = lobby(STANDARD);
    p[0] = p[0].clone().lvl(12).kda(7, 1, 3).items(&[RABADONS, SORCS]).cs(160);
    p[6].dead = true;
    p[6].respawn_in = 22.0;
    p[3].dead = true;
    p[3].respawn_in = 60.0;
    let s = at(1500.0, vec![dragon(400.0, Team::Order), dragon(800.0, Team::Chaos), tower(Team::Chaos, 1), inhib(1400.0, Team::Order, false), GameEvent::Baron { time: 1450.0, team: Some(Team::Chaos) }]);
    let s = GameSnapshot { players: p, ..s };
    let c = curves(&s, |p| if p.team == Team::Order { [52.0, 53.0, 54.0, 55.0] } else { [50.0; 4] });
    let mine = win_with(&s, &c, &[]).unwrap();
    let theirs = win_with(&GameSnapshot { me: 5, ..s.clone() }, &c, &[]).unwrap();
    assert!((mine.p + theirs.p - 1.0).abs() < 1e-5, "{} {}", mine.p, theirs.p);
    assert!((mine.base + theirs.base - 1.0).abs() < 1e-5);
    let sd = sd();
    let (a, b) = (State::from_snapshot(&s, &sd, &c, Team::Order), State::from_snapshot(&s, &sd, &c, Team::Chaos));
    for (x, y) in a.features().iter().zip(b.features()) {
        assert!((x + y).abs() < 1e-5, "{:?} {:?}", a.features(), b.features());
    }
}

#[test]
fn every_feature_moves_it_the_right_way() {
    let base = win(&even()).p;
    assert!(base > 0.5 && base < 0.53, "an even game on blue side: {base}");
    assert!(win(&GameSnapshot { me: 5, ..even() }).p < 0.5, "red side");
    let me = |f: fn(Player) -> Player| {
        let mut s = even();
        s.players[0] = f(s.players[0].clone());
        win(&s).p
    };
    let ev = |time: f32, events: Vec<GameEvent>| win(&at(time, events)).p - win(&at(time, vec![])).p;
    let grub = |team| GameEvent::Objective { time: 500.0, team: Some(team), name: "Horde".into() };
    let ups = [
        ("gold", me(|p| p.items(&[RUBY, RABADONS])) - base),
        ("level", me(|p| p.lvl(12)) - base),
        ("cs", me(|p| p.cs(190)) - base),
        ("kills", me(|p| p.kda(8, 2, 2)) - base),
        ("towers", ev(900.0, vec![tower(Team::Chaos, 1)])),
        ("inhibs", ev(1500.0, vec![inhib(1400.0, Team::Chaos, false)])),
        ("drakes", ev(900.0, vec![dragon(400.0, Team::Order)])),
        ("elder", ev(1800.0, vec![GameEvent::Dragon { time: 1700.0, team: Some(Team::Order), kind: "Elder".into() }])),
        ("baron", ev(1500.0, vec![GameEvent::Baron { time: 1400.0, team: Some(Team::Order) }])),
        ("grubs", ev(900.0, vec![grub(Team::Order)])),
    ];
    for (k, d) in ups {
        assert!(d > 0.002, "{k} for me: {d}");
    }
    let downs = [
        ("towers", ev(900.0, vec![tower(Team::Order, 1)])),
        ("inhibs", ev(1500.0, vec![inhib(1400.0, Team::Order, false)])),
        ("drakes", ev(900.0, vec![dragon(400.0, Team::Chaos)])),
        ("baron", ev(1500.0, vec![GameEvent::Baron { time: 1400.0, team: Some(Team::Chaos) }])),
        ("grubs", ev(900.0, vec![grub(Team::Chaos)])),
    ];
    for (k, d) in downs {
        assert!(d < -0.002, "{k} for them: {d}");
    }
}

#[test]
fn dragons_cap_at_three_then_the_soul_counts() {
    let drakes = |n: usize| (0..n).map(|i| dragon(300.0 + 300.0 * i as f32, Team::Order)).collect::<Vec<_>>();
    let p = |n| win(&at(1800.0, drakes(n))).p;
    assert!(p(3) > p(2) && p(4) > p(3));
    let sd = sd();
    let st = |n| State::from_snapshot(&at(1800.0, drakes(n)), &sd, &HashMap::new(), Team::Order);
    assert_eq!((st(3).drakes, st(3).soul), ([3, 0], 0));
    assert_eq!((st(4).drakes, st(4).soul), ([4, 0], 1));
    let f = st(4).features();
    let i = KEYS.iter().position(|k| *k == "drakes").unwrap();
    assert_eq!(f[i], 3.0, "the fourth is the soul, not a fourth dragon");
}

#[test]
fn buffs_run_out_and_inhibitors_come_back() {
    let even = |t| win(&at(t, vec![])).p;
    let baron = |t| win(&at(t, vec![GameEvent::Baron { time: 1400.0, team: Some(Team::Order) }])).p;
    assert!(baron(1500.0) > even(1500.0) + 0.05);
    assert!((baron(1581.0) - even(1581.0)).abs() < 1e-6, "Hand of Baron lasts 180 s");
    let elder = |t| win(&at(t, vec![GameEvent::Dragon { time: 1700.0, team: Some(Team::Order), kind: "Elder".into() }])).p;
    assert!(elder(1849.0) > even(1849.0) + 0.05);
    assert!((elder(1851.0) - even(1851.0)).abs() < 1e-6, "the Elder buff lasts 150 s; an Elder is no dragon stack");
    let unknown = win(&at(1500.0, vec![GameEvent::Baron { time: 1400.0, team: None }])).p;
    assert!((unknown - even(1500.0)).abs() < 1e-6, "a Baron nobody is credited with counts for neither side");

    let down = win(&at(1500.0, vec![inhib(1200.0, Team::Chaos, false)])).p;
    let back = win(&at(1520.0, vec![inhib(1200.0, Team::Chaos, false), inhib(1500.0, Team::Chaos, true)])).p;
    assert!(down > even(1500.0) + 0.02);
    assert!((back - even(1520.0)).abs() < 1e-6);
    let sd = sd();
    let two = at(1500.0, vec![inhib(1200.0, Team::Chaos, false), GameEvent::Inhib { time: 1300.0, team: Team::Chaos, lane: Some(Lane::Top), respawned: false }, inhib(1400.0, Team::Order, false)]);
    assert_eq!(State::from_snapshot(&two, &sd, &HashMap::new(), Team::Order).inhibs, [2, 1], "[theirs down, ours down]");
}

#[test]
fn base_leaves_the_dead_out() {
    let standing = win(&even());
    let mut s = even();
    s.players[6].dead = true;
    s.players[6].respawn_in = 30.0;
    s.players[7].dead = true;
    s.players[7].respawn_in = 90.0;
    let w = win(&s);
    assert!((w.base - standing.base).abs() < 1e-6 && (standing.p - standing.base).abs() < 1e-6);
    assert!(w.p > w.base + 0.005, "{} {}", w.p, w.base);
    let sd = sd();
    let dead = State::from_snapshot(&s, &sd, &HashMap::new(), Team::Order).dead;
    assert!((dead[1] - (30.0 / 45.0 + 1.0)).abs() < 1e-5 && dead[0] == 0.0, "respawns count up to 45 s: {dead:?}");
    s.players[0].dead = true;
    s.players[0].respawn_in = 40.0;
    assert!(win(&s).p < w.p);
}

#[test]
fn factors_are_the_big_ones_largest_first() {
    let mut s = at(1500.0, vec![GameEvent::Baron { time: 1450.0, team: Some(Team::Chaos) }, tower(Team::Chaos, 1)]);
    s.players[0] = s.players[0].clone().items(&[RUBY, ROD, TOME]);
    let w = win(&s);
    assert!(w.factors.len() >= 3, "{:?}", w.factors);
    assert!(w.factors.windows(2).all(|x| x[0].points.abs() >= x[1].points.abs()));
    assert!(w.factors.iter().all(|f| f.points.abs() >= 1.0));
    let f = |k: &str| w.factors.iter().find(|f| f.key == k).unwrap_or_else(|| panic!("{k}: {:?}", w.factors));
    assert!(f("gold").points > 0.0 && f("baron").points < 0.0 && f("towers").points > 0.0);
    assert_eq!((f("gold").label.as_str(), f("baron").label.as_str()), ("item gold", "Baron"));
    assert!(!w.factors.iter().any(|f| f.key == "side"), "half a point of side is not a factor: {:?}", w.factors);
    assert!(win(&even()).factors.iter().all(|f| f.key == "side"), "an even game has nothing else to name");
}

#[test]
fn stakes_name_what_is_next_and_what_it_swings() {
    let late = win(&at(1500.0, vec![dragon(400.0, Team::Order)]));
    let (baron, drake) = (stake(&late, "Baron").unwrap(), stake(&late, "Dragon").unwrap());
    assert!(baron.swing > drake.swing && drake.swing > 0.0, "{:?}", late.stakes);
    assert!(late.stakes.windows(2).all(|x| x[0].swing >= x[1].swing));
    assert!(stake(&late, "Tower").is_some() && stake(&late, "Your death").is_some());
    assert!(stake(&late, "Elder").is_none() && stake(&late, "Inhibitor").is_none(), "{:?}", late.stakes);
    let death = stake(&late, "Your death").unwrap();
    assert!(death.swing > 0.0 && death.note.as_deref().is_some_and(|n| n.contains(" s")), "{death:?}");

    assert!(stake(&win(&even()), "Baron").is_none(), "Baron is five minutes off at 15:00");

    let point = |team| win(&at(1500.0, (0..3).map(|i| dragon(300.0 + 300.0 * i as f32, team)).collect()));
    let (mine, theirs) = (point(Team::Order), point(Team::Chaos));
    let soul = stake(&mine, "Dragon").unwrap();
    assert!(soul.swing > drake.swing + 1.0 && soul.note.as_deref() == Some("your soul"), "{soul:?}");
    assert_eq!(stake(&theirs, "Dragon").unwrap().note.as_deref(), Some("their soul"));

    let souled = win(&at(1800.0, (0..4).map(|i| dragon(300.0 + 300.0 * i as f32, Team::Chaos)).collect()));
    assert!(stake(&souled, "Dragon").is_none() && stake(&souled, "Elder").is_some_and(|s| s.swing > 10.0), "{:?}", souled.stakes);

    let open = win(&at(1500.0, vec![tower(Team::Chaos, 3)]));
    assert!(stake(&open, "Inhibitor").is_some_and(|s| s.swing > 0.0));

    let mut dead = at(1500.0, vec![]);
    dead.players[0].dead = true;
    dead.players[0].respawn_in = 20.0;
    assert!(stake(&win(&dead), "Your death").is_none());
}

#[test]
fn aram_has_towers_and_no_dragons() {
    let aram = |events| GameSnapshot { mode: Mode::Aram, map: 12, ..at(900.0, events) };
    let w = win(&aram(vec![tower(Team::Chaos, 1), dragon(400.0, Team::Chaos)]));
    assert!(w.p > 0.55, "{}", w.p);
    assert!((w.p - win(&aram(vec![tower(Team::Chaos, 1)])).p).abs() < 1e-6, "the ARAM table has no dragons");
    let names: Vec<&str> = w.stakes.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Tower") && !names.iter().any(|n| ["Dragon", "Baron", "Elder"].contains(n)), "{names:?}");
    assert!(stake(&win(&aram(vec![tower(Team::Chaos, 2)])), "Inhibitor").is_some(), "the second turret opens the inhibitor");
}

#[test]
fn no_rift_only_feature_leaks_into_the_aram_state() {
    let rift_only = ["cs", "drakes", "soul", "elder", "baron", "grubs", "side"];
    for t in [&model().aram, &Model::shipped().aram] {
        assert!(t.beta.keys().all(|k| !rift_only.contains(&k.as_str())), "{:?}", t.beta.keys());
    }
    let sd = sd();
    let mut p = lobby(STANDARD);
    p[0] = p[0].clone().cs(200);
    let grub = GameEvent::Objective { time: 500.0, team: Some(Team::Order), name: "Horde".into() };
    let events = vec![dragon(400.0, Team::Order), grub, GameEvent::Baron { time: 850.0, team: Some(Team::Order) }, tower(Team::Chaos, 1)];
    // A table that has every Rift key reads none of them off a Howling Abyss game.
    let rift_keys = Model { aram: model().rift.clone(), ..model() };
    for mode in [Mode::Aram, Mode::Mayhem] {
        let s = GameSnapshot { mode, map: 12, players: p.clone(), ..at(900.0, events.clone()) };
        let st = State::from_snapshot(&s, &sd, &HashMap::new(), Team::Order);
        let f = st.features();
        for k in rift_only {
            assert_eq!(f[KEYS.iter().position(|x| *x == k).unwrap()], 0.0, "{mode:?} {k}");
        }
        assert_eq!(st.towers, [1, 0]);
        let w = win_prob(&Input { model: &rift_keys, snap: &s, sd: &sd, curves: &HashMap::new(), history: &[] }).unwrap();
        assert!(w.factors.iter().all(|f| !rift_only.contains(&f.key.as_str())) && w.factors.iter().any(|f| f.key == "towers"), "{:?}", w.factors);
    }
}

#[test]
fn rough_when_the_model_does_not_fit_the_game() {
    let s = even();
    let all = curves(&s, |_| [50.0; 4]);
    let w = win_with(&s, &all, &[]).unwrap();
    assert!(!w.rough && w.source.contains("hand-set"), "{}", w.source);
    assert!(win(&s).rough, "no curves");
    let mut nine = all.clone();
    nine.remove(&ZED);
    assert!(win_with(&s, &nine, &[]).unwrap().rough);
    assert!(win_with(&GameSnapshot { swiftplay: true, ..s.clone() }, &all, &[]).unwrap().rough);
    let aram = GameSnapshot { mode: Mode::Aram, map: 12, ..s.clone() };
    assert!(!win_with(&aram, &all, &[]).unwrap().rough);
    let mayhem = win_with(&GameSnapshot { mode: Mode::Mayhem, map: 12, ..s }, &all, &[]).unwrap();
    assert!(mayhem.rough && mayhem.source.contains("ARAM model, augments not counted"), "{}", mayhem.source);
    assert_eq!(mayhem.p, win_with(&aram, &all, &[]).unwrap().p);
}

#[test]
fn outlook_reads_who_the_rest_of_the_game_favours() {
    let s = even();
    // My team wins long games, theirs short ones.
    let c = curves(&s, |p| if p.team == Team::Order { [46.0, 50.0, 54.0, 58.0] } else { [54.0, 50.0, 46.0, 42.0] });
    let w = win_with(&s, &c, &[]).unwrap();
    let o = w.outlook.clone().unwrap();
    assert!(o.later > o.now + 5.0 && o.later_min == 25, "{o:?}");
    // At 15:00 every bucket is still ahead: 52 − 48 a champion, shrunk by the games, five a side.
    assert!((o.now - 5.0 * 2.0 * 2.0 * 4000.0 / 4400.0).abs() < 0.05, "{o:?}");
    let late = win_with(&GameSnapshot { time: 2100.0, ..s.clone() }, &c, &[]).unwrap();
    assert!(late.outlook.unwrap().now > o.later, "after 30:00 only the long games are left");
    assert!(w.p > win_with(&s, &curves(&s, |_| [50.0; 4]), &[]).unwrap().p);
    assert_eq!(win(&s).outlook, None);
}

#[test]
fn trend_is_the_change_of_base_over_two_minutes() {
    let s = even();
    let base = win(&s).base;
    let sample = |time: f32, win: Option<f32>| GoldSample { time, team: 0, lane: None, win };
    let h = [sample(700.0, Some(0.30)), sample(780.0, Some(0.40)), sample(860.0, Some(0.48))];
    let t = win_with(&s, &HashMap::new(), &h).unwrap().trend.unwrap();
    assert!((t - 100.0 * (base - 0.40)).abs() < 1e-3, "{t}");
    assert_eq!(win(&s).trend, None);
    assert_eq!(win_with(&s, &HashMap::new(), &[sample(870.0, Some(0.4)), sample(300.0, Some(0.4)), sample(780.0, None)]).unwrap().trend, None);
}

#[test]
fn a_past_game_frame_gives_the_same_features() {
    use Lane::*;
    let line = |champ: ChampKey, ally: bool, lane: Lane| MatchPlayer {
        champ,
        name: String::new(),
        ally,
        me: false,
        lane: Some(lane),
        level: 18,
        kills: 0,
        deaths: 0,
        assists: 0,
        cs: 0,
        gold: 0,
        damage: 0,
        taken: 0,
        vision: 0,
        items: vec![],
        wards_placed: 0,
        wards_killed: 0,
        control_wards: 0,
    };
    let players = vec![
        line(AHRI, true, Middle),
        line(GAREN, true, Top),
        line(LEE_SIN, true, Jungle),
        line(JINX, true, Bottom),
        line(LEONA, true, Support),
        line(DARIUS, false, Top),
        line(JARVAN, false, Jungle),
        line(ZED, false, Middle),
        line(CAITLYN, false, Bottom),
        line(THRESH, false, Support),
    ];
    let frames = (0..=26u32)
        .map(|m| Frame {
            time: m as f32 * 60.0,
            players: (0..10).map(|i| FrameStat { total_gold: 500 + m * if i < 5 { 450 } else { 400 }, current_gold: 300, level: 1 + m / 2 + u32::from(i < 5 && m > 10), cs: m * 6, jungle_cs: 0, ..Default::default() }).collect(),
        })
        .collect();
    let kill = |time: f32, killer: usize, victim: usize| KillEvent { time, killer: Some(killer), victim, assisters: vec![], pos: None };
    let mark = |time: f32, ours: bool, kind: &str, detail: Option<&str>| TimelineMark { time, ours, kind: kind.into(), detail: detail.map(str::to_string), pos: None, mine: false, presence: None };
    let game = PastGame {
        game_id: 1,
        created: 0,
        queue_id: 420,
        mode: Mode::Rift,
        champ: AHRI,
        lane: Some(Middle),
        win: true,
        remake: false,
        duration_s: 1560,
        kills: 0,
        deaths: 0,
        assists: 0,
        cs: 0,
        gold: 0,
        damage_to_champs: 0,
        vision: 0,
        death_times: None,
        gold_diff_10: None,
        gold_diff_15: None,
        xp_diff_15: None,
        ours: None,
        theirs: None,
        first_dragon: None,
        first_tower: None,
        first_blood: None,
        cs_10: None,
        augments: vec![],
        death_spots: vec![],
        enemies: vec![],
        opponent: None,
        killed_by: vec![],
        control_wards: None,
    };
    let d = MatchDetail {
        game,
        me: 0,
        players,
        frames,
        kills: vec![kill(200.0, 0, 7), kill(400.0, 7, 0), kill(900.0, 1, 5), kill(1495.0, 0, 7)],
        marks: vec![
            mark(400.0, true, "Dragon", Some("Infernal")),
            mark(500.0, true, "Grubs", Some("×3")),
            mark(700.0, true, "Tower", Some("mid outer")),
            mark(800.0, false, "Tower", Some("top outer")),
            mark(1000.0, false, "Dragon", Some("Ocean")),
            mark(1150.0, true, "Inhibitor", Some("mid")),
            mark(1300.0, false, "Inhibitor", Some("bot")),
            mark(1400.0, true, "Baron", None),
        ],
    };
    let s = State::from_frame(&d, 25);
    assert_eq!(s.minute, 25.0);
    assert_eq!(s.item_gold, [5.0 * (200.0 + 25.0 * 450.0), 5.0 * (200.0 + 25.0 * 400.0)], "gold spent stands in for item gold");
    assert!((s.level[0] - s.level[1] - 1.0).abs() < 1e-6 && s.cs == [750.0, 750.0]);
    assert_eq!((s.kills, s.towers, s.drakes, s.grubs), ([3.0, 1.0], [1, 1], [1, 1], [3, 0]));
    assert_eq!((s.inhibs, s.baron, s.elder), ([0, 1], 1, 0), "an inhibitor is back after five minutes; Baron taken 100 s ago");
    assert!(s.dead[1] > 0.0 && s.dead[0] == 0.0, "Zed died five seconds ago: {:?}", s.dead);
    assert_eq!(State::from_frame(&d, 20).inhibs, [1, 0]);
    assert_eq!(s.side, 1, "participants 1–5 are blue");
    let red = MatchDetail { players: d.players.iter().map(|p| MatchPlayer { ally: !p.ally, ..p.clone() }).collect(), me: 7, ..d.clone() };
    assert_eq!(State::from_frame(&red, 25).side, -1);
    let m = model();
    assert!(eval(&m.knots, &m.rift, &s, s.minute) > 0.6);
    assert_eq!(State::from_frame(&d, 99), State::default());
}

#[test]
fn the_advice_carries_it_and_the_gold_history_keeps_the_base() {
    let (sd, m) = (sd(), model());
    let mut s = even();
    s.players[6].dead = true;
    s.players[6].respawn_in = 40.0;
    let advise = |winprob| game_advice(&GameInput { snap: &s, sd: &sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob, situations: &Default::default() });
    let a = advise(Some(&m));
    let w = a.win.clone().unwrap();
    assert_eq!(w, win(&s));
    assert!(w.p > w.base);
    assert_eq!(a.gold.history.last().unwrap().win, Some(w.base));
    let none = advise(None);
    assert_eq!((none.win, none.gold.history.last().unwrap().win), (None, None), "the built-in table has no fit yet");
}

/// `fit.py` writes held-out states with its own probability; absent until a fit exists.
#[test]
fn rust_reproduces_the_fitted_probabilities() {
    let path = format!("{}/../../fixtures/coach/winprob_cases.json", env!("CARGO_MANIFEST_DIR"));
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let cases: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    let m = Model::shipped();
    for c in &cases {
        let table = if c["mode"] == "aram" { &m.aram } else { &m.rift };
        let z: f32 = KEYS.iter().filter_map(|k| Some(coach::winprob::beta(&m.knots, table.beta.get(*k)?, c["minute"].as_f64()? as f32) * c["x"][*k].as_f64()? as f32)).sum();
        let p = 1.0 / (1.0 + (-z).exp());
        assert!((p - c["p"].as_f64().unwrap() as f32).abs() < 1e-4, "{c}");
    }
}
