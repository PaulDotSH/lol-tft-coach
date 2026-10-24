//! Bot lane 2v2 plan in champ select.

mod common;

use coach::{BotLaneInput, bot_lane_plan};
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
    MatchupBuild { vs, games, win_rate: 55.0, start: vec![3865, 2003, 2003], start_games: games, start_wr: 55.5, core: vec![3876, 3047] }
}

fn notes() -> HashMap<String, LaneNote> {
    let n = LaneNote { level2: true, level6: false, engage: true, poke: false, sustain: false, note: "Hook from fog".into() };
    HashMap::from([("Thresh".to_string(), n)])
}

#[test]
fn support_plan_against_their_duo() {
    let (c, sd, notes) = (cs(), sd(), notes());
    let lanes = [lane(Lane::Bottom, Some(JINX), Some(CAITLYN)), lane(Lane::Support, Some(LEONA), Some(THRESH))];
    let duos = [DraftDuo { a: JINX, b: LEONA, delta: 1.8, games: 2400 }];
    let memory = [
        MatchupMemory { champ: THRESH, games: 3, wins: 2, lane_games: 3, gold_15: Some(350), killed_me: 4 },
        MatchupMemory { champ: ZED, games: 2, wins: 0, lane_games: 0, gold_15: None, killed_me: 1 },
    ];
    let against = [(Lane::Bottom, vec![vs(CAITLYN, 51.5, 3000)]), (Lane::Support, vec![vs(THRESH, 48.2, 90)])];
    let b = build(CAITLYN, 800);
    let p = bot_lane_plan(&BotLaneInput { cs: &c, sd: &sd, lanes: &lanes, duos: &duos, memory: &memory, vs: &against, start: Some(&b), notes: &notes })
        .expect("plan");
    assert_eq!((p.allies.as_slice(), p.enemies.as_slice()), ([LEONA, JINX].as_slice(), [CAITLYN, THRESH].as_slice()));
    let nums: Vec<(ChampKey, bool, f32, u32)> = p.numbers.iter().map(|n| (n.champ, n.with, n.value, n.games)).collect();
    assert_eq!(nums, [(CAITLYN, false, 51.5, 3000), (THRESH, false, 48.2, 90), (JINX, true, 1.8, 2400)]);
    assert_eq!(p.windows.len(), 1);
    assert_eq!((p.windows[0].champ, p.windows[0].tags.as_slice()), (THRESH, ["strong level 2".to_string(), "hard engage".to_string()].as_slice()));
    assert_eq!(p.memory.iter().map(|m| m.champ).collect::<Vec<_>>(), [THRESH]);
    assert_eq!(p.start.as_ref().map(|s| s.vs), Some(CAITLYN));
}

#[test]
fn partial_reveals_few_games_and_other_lanes() {
    let (sd, notes) = (sd(), notes());
    // Only their support revealed; matchup items from too few games are dropped.
    let lanes = [lane(Lane::Bottom, Some(JINX), None), lane(Lane::Support, Some(LEONA), Some(THRESH))];
    let b = build(CAITLYN, 40);
    let c = cs();
    let input = BotLaneInput { cs: &c, sd: &sd, lanes: &lanes, duos: &[], memory: &[], vs: &[], start: Some(&b), notes: &notes };
    let p = bot_lane_plan(&input).expect("plan");
    assert_eq!(p.enemies, [THRESH]);
    assert!(p.numbers.is_empty() && p.start.is_none());

    // Nobody revealed yet: a plan that waits.
    let none = [lane(Lane::Bottom, Some(JINX), None), lane(Lane::Support, Some(LEONA), None)];
    assert!(bot_lane_plan(&BotLaneInput { lanes: &none, ..input }).unwrap().enemies.is_empty());

    // Mid laners and ARAM get no plan.
    let mid = ChampSelect { my_lane: Some(Lane::Middle), ..cs() };
    assert!(bot_lane_plan(&BotLaneInput { cs: &mid, ..input }).is_none());
    let aram = ChampSelect { mode: Mode::Aram, ..cs() };
    assert!(bot_lane_plan(&BotLaneInput { cs: &aram, ..input }).is_none());
}
