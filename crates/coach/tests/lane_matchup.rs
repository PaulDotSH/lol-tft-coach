//! Lane matchup card: champ select plan and the in-game laning panel.

mod common;

use coach::{CardData, GameInput, LiveInput, MatchupInput, Tracker, game_advice, live_extras, matchup_plan};
use common::*;
use model::*;
use std::collections::HashMap;

/// Me Leona support with Jinx (assigned), against Caitlyn + Thresh.
fn cs() -> ChampSelect {
    ChampSelect {
        queue_id: 420,
        mode: Mode::Rift,
        my_lane: Some(Lane::Support),
        my_hover: None,
        my_pick: Some(LEONA),
        locked: true,
        allies: vec![LEONA, JINX],
        enemies: vec![CAITLYN, THRESH],
        bans: vec![],
        bench: vec![],
        timer_phase: "BAN_PICK".into(),
        ally_lanes: vec![(LEONA, Lane::Support), (JINX, Lane::Bottom)],
    }
}

fn lane(lane: Lane, ally: Option<ChampKey>, enemy: Option<ChampKey>) -> DraftLane {
    DraftLane { lane, ally, enemy, ally_wr: None, games: 0 }
}

fn vs(champ: ChampKey, vs_wr: f32, games: u32) -> Matchup {
    Matchup { champ, vs_wr, games, default_lane: None }
}

fn build(vs: ChampKey, games: u32) -> MatchupBuild {
    MatchupBuild {
        vs,
        games,
        win_rate: 55.0,
        start: vec![3865, 2003, 2003],
        start_games: games,
        start_wr: 55.5,
        core: vec![3876, 3047],
        runes: vec![8439, 8463],
        summoners: vec![4, 14],
        skills: "QWE".into(),
        rune_page: None,
    }
}

fn note(watch: &str, wins: &str) -> ChampNote {
    ChampNote {
        level2: true,
        level6: false,
        engage: true,
        poke: false,
        sustain: false,
        wins: wins.into(),
        watch: watch.into(),
        spike: "Level 2".into(),
        trade: "When Q is down".into(),
    }
}

fn notes() -> LaneNotes {
    LaneNotes {
        champs: HashMap::from([("Thresh".to_string(), note("QE", "Hook from fog")), ("Zed".to_string(), note("WR", "Shadow poke"))]),
        pairs: HashMap::from([("Leona".to_string(), HashMap::from([("Thresh".to_string(), "Walk up when his Q is down".to_string())]))]),
    }
}

fn spells() -> HashMap<ChampKey, Vec<SpellInfo>> {
    let s = |name: &str, cooldown: &[f32]| SpellInfo { name: name.into(), icon: format!("{name}.png"), cooldown: cooldown.to_vec() };
    let thresh = vec![s("Death Sentence", &[19.0, 17.0, 15.0]), s("Dark Passage", &[21.0]), s("Flay", &[12.0]), s("The Box", &[140.0, 120.0, 100.0])];
    HashMap::from([(THRESH, thresh)])
}

fn sd_with_runes() -> StaticData {
    let mut sd = sd();
    sd.runes.insert(8439, RuneInfo { name: "Aftershock".into(), icon: "aftershock.png".into() });
    sd
}

#[test]
fn support_plan_against_their_duo() {
    let (c, sd, notes, spells) = (cs(), sd_with_runes(), notes(), spells());
    let lanes = [lane(Lane::Bottom, Some(JINX), Some(CAITLYN)), lane(Lane::Support, Some(LEONA), Some(THRESH))];
    let duos = [DraftDuo { a: JINX, b: LEONA, delta: 1.8, games: 2400 }];
    let thresh_record = MatchupMemory {
        champ: THRESH,
        games: 3,
        wins: 2,
        lane_games: 3,
        gold_15: Some(350),
        killed_me: 4,
        early_kills: 2,
        same_games: 1,
        same_wins: 0,
        last: vec![false, true, true],
    };
    let memory = [thresh_record.clone()];
    let against = [(Lane::Bottom, vec![vs(CAITLYN, 51.5, 3000)]), (Lane::Support, vec![vs(THRESH, 48.2, 90)])];
    let b = build(THRESH, 800);
    let card = CardData { sd: &sd, notes: &notes, me: LEONA, spells: &spells, memory: &memory };
    let input = MatchupInput { cs: &c, card, lane: Lane::Support, lanes: &lanes, duos: &duos, vs: &against, build: Some(&b) };
    let p = matchup_plan(&input).expect("plan");
    assert_eq!((p.lane, p.allies.as_slice()), (Lane::Support, [LEONA, JINX].as_slice()));
    assert_eq!(p.partner.as_ref().map(|s| (s.champ, s.delta, s.games)), Some((JINX, 1.8, 2400)));
    let champs: Vec<(ChampKey, Option<f32>, u32)> = p.cards.iter().map(|c| (c.champ, c.vs_wr, c.games)).collect();
    assert_eq!(champs, [(CAITLYN, Some(51.5), 3000), (THRESH, Some(48.2), 90)]);

    // Caitlyn has no note, spells or record; Thresh has all of them and a pair note against Leona.
    let (cait, thresh) = (&p.cards[0], &p.cards[1]);
    assert!(cait.note.is_none() && cait.pair.is_none() && cait.spells.is_empty() && cait.record.is_none());
    assert_eq!(thresh.tags, ["strong level 2", "hard engage"]);
    assert_eq!(thresh.note.as_ref().map(|n| n.wins.as_str()), Some("Hook from fog"));
    assert_eq!(thresh.pair.as_deref(), Some("Walk up when his Q is down"));
    let keys: Vec<(&str, bool, Option<f32>)> = thresh.spells.iter().map(|s| (s.key.as_str(), s.watch, s.now)).collect();
    assert_eq!(keys, [("Q", true, None), ("W", false, None), ("E", true, None), ("R", false, None)]);
    assert_eq!(thresh.record, Some(thresh_record));

    // Build against my direct opponent; runes without static data are left out.
    assert_eq!(p.build.as_ref().map(|b| b.vs), Some(THRESH));
    assert_eq!(p.runes.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Aftershock"]);
}

#[test]
fn solo_lane_partial_reveals_few_games_and_aram() {
    let (sd, notes, spells) = (sd(), notes(), spells());
    let card = CardData { sd: &sd, notes: &notes, me: AHRI, spells: &spells, memory: &[] };
    let mid = ChampSelect { my_lane: Some(Lane::Middle), my_pick: Some(AHRI), ally_lanes: vec![], ..cs() };
    let lanes = [lane(Lane::Middle, Some(AHRI), Some(ZED)), lane(Lane::Support, None, Some(THRESH))];
    let against = [(Lane::Middle, vec![vs(ZED, 47.0, 5000)])];
    let few = build(ZED, 40);
    let input = MatchupInput { cs: &mid, card, lane: Lane::Middle, lanes: &lanes, duos: &[], vs: &against, build: Some(&few) };
    let p = matchup_plan(&input).expect("plan");
    // Only my lane's enemy; no partner; matchup pages from few games are dropped.
    assert_eq!(p.cards.iter().map(|c| c.champ).collect::<Vec<_>>(), [ZED]);
    assert_eq!((p.cards[0].vs_wr, p.cards[0].tags.len()), (Some(47.0), 2));
    assert!(p.allies == [AHRI] && p.partner.is_none() && p.build.is_none());

    // Nobody revealed in my lane yet: a plan that waits.
    let none = [lane(Lane::Middle, Some(AHRI), None)];
    assert!(matchup_plan(&MatchupInput { lanes: &none, ..input }).unwrap().cards.is_empty());
    // No champion yet, or ARAM: no plan.
    let unpicked = ChampSelect { my_pick: None, my_hover: None, ..mid.clone() };
    assert!(matchup_plan(&MatchupInput { cs: &unpicked, ..input }).is_none());
    let aram = ChampSelect { mode: Mode::Aram, ..mid.clone() };
    assert!(matchup_plan(&MatchupInput { cs: &aram, ..input }).is_none());
}

#[test]
fn laning_panel_shows_both_enemy_bot_laners_with_r_now() {
    let sd = sd();
    let players = vec![
        player(LEONA, Team::Order, Some(Lane::Support)),
        player(JINX, Team::Order, Some(Lane::Bottom)),
        player(ZED, Team::Chaos, Some(Lane::Middle)),
        player(THRESH, Team::Chaos, Some(Lane::Support)),
        player(CAITLYN, Team::Chaos, Some(Lane::Bottom)),
    ];
    let s = snap(players, vec![]);
    let mut a = game_advice(&GameInput { snap: &s, sd: &sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None });
    let (notes, spells) = (notes(), spells());
    live_extras(&mut a, &LiveInput { snap: &s, sd: &sd, tracker: &Tracker::default(), ults: &HashMap::new(), notes: &notes, spells: &spells, memory: &[] });
    // Bot laner first, then their support (my lane opponent).
    assert_eq!(a.matchup.iter().map(|c| c.champ).collect::<Vec<_>>(), [CAITLYN, THRESH]);
    let r = a.matchup[1].spells.last().unwrap();
    // Level 9: R rank 1.
    assert_eq!((r.key.as_str(), r.now), ("R", Some(140.0)));
    assert_eq!(a.matchup[1].pair.as_deref(), Some("Walk up when his Q is down"));

    // Mid: just my lane opponent.
    let players = vec![player(AHRI, Team::Order, Some(Lane::Middle)), player(ZED, Team::Chaos, Some(Lane::Middle)), player(THRESH, Team::Chaos, Some(Lane::Support))];
    let s = snap(players, vec![]);
    let mut a = game_advice(&GameInput { snap: &s, sd: &sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None });
    live_extras(&mut a, &LiveInput { snap: &s, sd: &sd, tracker: &Tracker::default(), ults: &HashMap::new(), notes: &notes, spells: &spells, memory: &[] });
    assert_eq!(a.matchup.iter().map(|c| c.champ).collect::<Vec<_>>(), [ZED]);
    assert!(a.matchup[0].spells.is_empty() && a.matchup[0].note.is_some());
}
