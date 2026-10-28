//! Draft coach behaviour: lineups, the draft report and how synergy and comp
//! gaps move pick suggestions.

mod common;

use coach::{ChampSelectInput, DraftData, champ_select_advice, draft_lineups};
use common::*;
use model::*;
use std::collections::HashMap;

fn cs(lane: Lane) -> ChampSelect {
    ChampSelect {
        queue_id: 420,
        mode: Mode::Rift,
        my_lane: Some(lane),
        my_hover: None,
        my_pick: None,
        locked: false,
        allies: vec![],
        enemies: vec![],
        bans: vec![],
        bench: vec![],
        timer_phase: "BAN_PICK".into(),
        ally_lanes: vec![],
    }
}

fn with_allies(mut c: ChampSelect, allies: &[(ChampKey, Lane)]) -> ChampSelect {
    c.allies = allies.iter().map(|a| a.0).collect();
    c.ally_lanes = allies.to_vec();
    c
}

fn tier(champ: ChampKey, lane: Lane, wr: f32) -> TierEntry {
    TierEntry { champ, lane, win_rate: wr, pick_rate: 5.0, ban_rate: 1.0, games: 30_000, tier: None }
}

/// Counter-only stats as the draft fetches them.
fn lane_stats(champ: ChampKey, lane: Lane, wr: f32, lanes: &[(Lane, f32)], counters: Vec<Matchup>) -> ChampStats {
    ChampStats {
        champ,
        lane: Some(lane),
        mode: Mode::Rift,
        patch: "16.21".into(),
        win_rate: wr,
        pick_rate: 5.0,
        games: 20_000,
        lanes: lanes.iter().copied().collect(),
        slots: vec![],
        boots: vec![],
        early: vec![],
        counters,
        benchmarks: Default::default(),
        runes: None,
        coachless: None,
        summoners: vec![],
    }
}

fn team(champ: ChampKey, lane: Lane, mates: &[(ChampKey, Lane, f32, u32)]) -> TeamSynergy {
    TeamSynergy {
        champ,
        lane,
        patch: "16.21".into(),
        games: 20_000,
        mates: mates
            .iter()
            .map(|&(c, l, delta, games)| Teammate { champ: c, lane: l, win_rate: 50.0 + delta, delta, games })
            .collect(),
    }
}

struct Run<'a> {
    cs: &'a ChampSelect,
    mine: Option<&'a ChampStats>,
    enemies: &'a [ChampStats],
    tiers: &'a [TierEntry],
    draft: DraftData<'a>,
}

fn run(r: Run) -> ChampSelectAdvice {
    let sd = sd();
    champ_select_advice(&ChampSelectInput {
        cs: r.cs,
        sd: &sd,
        mine: r.mine,
        enemies: r.enemies,
        tier_list: r.tiers,
        mayhem: None,
        draft: r.draft,
    })
}

fn picks(r: Run) -> Vec<PickSuggestion> {
    run(r).picks
}

#[test]
fn all_ad_team_ap_pick_gets_comp_gap_reason() {
    let tiers = [tier(ZED, Lane::Middle, 51.0), tier(SYNDRA, Lane::Middle, 50.5), tier(VLADIMIR, Lane::Middle, 49.0)];
    let alone = cs(Lane::Middle);
    let p = picks(Run { cs: &alone, mine: None, enemies: &[], tiers: &tiers, draft: DraftData::default() });
    assert_eq!(p[0].champ, ZED, "{p:?}");

    let ad = with_allies(cs(Lane::Middle), &[(GAREN, Lane::Top), (LEE_SIN, Lane::Jungle), (CAITLYN, Lane::Bottom)]);
    let p = picks(Run { cs: &ad, mine: None, enemies: &[], tiers: &tiers, draft: DraftData::default() });
    assert_eq!(p[0].champ, SYNDRA, "{p:?}");
    assert!(p[0].reason.starts_with("adds magic damage: team is 9"), "{}", p[0].reason);
    let zed = p.iter().find(|s| s.champ == ZED).unwrap();
    assert_eq!(zed.reason, "51.0% win rate in middle", "no gap filled, no bonus");

    // One ally is too few to call a comp gap.
    let one = with_allies(cs(Lane::Middle), &[(GAREN, Lane::Top)]);
    let p = picks(Run { cs: &one, mine: None, enemies: &[], tiers: &tiers, draft: DraftData::default() });
    assert_eq!(p[0].champ, ZED);
}

#[test]
fn strong_duo_synergy_raises_a_support_pick() {
    let tiers = [tier(THRESH, Lane::Support, 51.0), tier(LEONA, Lane::Support, 50.8), tier(SONA, Lane::Support, 50.5)];
    let c = with_allies(cs(Lane::Support), &[(JINX, Lane::Bottom)]);
    let p = picks(Run { cs: &c, mine: None, enemies: &[], tiers: &tiers, draft: DraftData::default() });
    assert_eq!(p[0].champ, THRESH, "no synergy data yet: {p:?}");

    let syn = [team(JINX, Lane::Bottom, &[(SONA, Lane::Support, 4.0, 3_000), (THRESH, Lane::Support, -1.0, 3_000)])];
    let draft = DraftData { lane_stats: &[], synergy: &syn };
    let p = picks(Run { cs: &c, mine: None, enemies: &[], tiers: &tiers, draft });
    assert_eq!(p[0].champ, SONA, "{p:?}");
    assert_eq!(p[0].reason, "synergy with Jinx +4.0%");
    let thresh = p.iter().find(|s| s.champ == THRESH).unwrap();
    assert!(thresh.score < 51.0, "bad synergy lowers the score: {}", thresh.score);
    assert_eq!(thresh.reason, "51.0% win rate in support", "negative terms are not reasons");
}

#[test]
fn matchup_reason_keeps_a_strong_second_factor() {
    let tiers = [tier(LUX, Lane::Middle, 50.0), tier(ZED, Lane::Middle, 50.0)];
    let mut c = with_allies(cs(Lane::Middle), &[(GAREN, Lane::Top), (LEE_SIN, Lane::Jungle), (CAITLYN, Lane::Bottom)]);
    c.enemies = vec![SYNDRA];
    let mut syndra = ahri_stats();
    syndra.champ = SYNDRA;
    syndra.counters = vec![matchup(LUX, 45.0, 3_000, Lane::Middle), matchup(ZED, 50.0, 3_000, Lane::Middle)];
    let p = picks(Run { cs: &c, mine: None, enemies: &[syndra], tiers: &tiers, draft: DraftData::default() });
    assert_eq!(p[0].champ, LUX);
    assert!(p[0].reason.starts_with("55.0% vs Syndra (lane) · adds magic damage"), "{}", p[0].reason);
}

#[test]
fn lineups_place_enemies_by_lane_share() {
    let mut c = with_allies(cs(Lane::Middle), &[(GAREN, Lane::Top)]);
    c.my_hover = Some(AHRI);
    c.allies.push(JINX); // no assigned lane, no data
    c.enemies = vec![ZED, SYNDRA, MUNDO];
    let zed = lane_stats(ZED, Lane::Middle, 50.0, &[(Lane::Middle, 90.0), (Lane::Top, 8.0)], vec![]);
    let syn = lane_stats(SYNDRA, Lane::Middle, 50.0, &[(Lane::Middle, 95.0), (Lane::Support, 3.0)], vec![]);
    let (al, en) = draft_lineups(&c, &[&zed, &syn]);
    assert_eq!(al, vec![(AHRI, Some(Lane::Middle)), (GAREN, Some(Lane::Top)), (JINX, None)]);
    assert_eq!(en, vec![(ZED, Some(Lane::Top)), (SYNDRA, Some(Lane::Middle)), (MUNDO, None)]);
}

#[test]
fn report_breaks_down_both_sides() {
    let mut c = with_allies(cs(Lane::Middle), &[(GAREN, Lane::Top), (JINX, Lane::Bottom), (THRESH, Lane::Support)]);
    c.my_hover = Some(AHRI);
    c.enemies = vec![ZED, DARIUS];
    let mut zed = ahri_stats();
    (zed.champ, zed.lanes, zed.counters) = (ZED, HashMap::from([(Lane::Middle, 95.0), (Lane::Top, 3.0)]), vec![]);
    let darius_mid = lane_stats(DARIUS, Lane::Middle, 48.0, &[(Lane::Top, 90.0), (Lane::Middle, 5.0)], vec![]);
    let lanes = [
        lane_stats(GAREN, Lane::Top, 51.0, &[(Lane::Top, 95.0)], vec![matchup(DARIUS, 47.0, 2_000, Lane::Top)]),
        lane_stats(JINX, Lane::Bottom, 52.0, &[(Lane::Bottom, 98.0)], vec![]),
        lane_stats(DARIUS, Lane::Top, 50.5, &[(Lane::Top, 90.0), (Lane::Middle, 5.0)], vec![]),
    ];
    let syn = [team(JINX, Lane::Bottom, &[(THRESH, Lane::Support, 2.0, 1_500)])];
    let enemies = [zed, darius_mid];
    let a = run(Run {
        cs: &c,
        mine: Some(&ahri_stats()),
        enemies: &enemies,
        tiers: &[],
        draft: DraftData { lane_stats: &lanes, synergy: &syn },
    });
    let d = a.draft.expect("draft report");

    let lane = |l: Lane| d.lanes.iter().find(|x| x.lane == l).unwrap();
    assert_eq!((lane(Lane::Middle).ally, lane(Lane::Middle).enemy), (Some(AHRI), Some(ZED)));
    assert_eq!((lane(Lane::Middle).ally_wr, lane(Lane::Middle).games), (Some(49.0), 4_000));
    assert_eq!((lane(Lane::Top).enemy, lane(Lane::Top).ally_wr), (Some(DARIUS), Some(47.0)));
    assert_eq!(
        (lane(Lane::Bottom).ally, lane(Lane::Bottom).enemy, lane(Lane::Bottom).ally_wr),
        (Some(JINX), None, None)
    );
    assert!(d.lanes.iter().all(|l| l.lane != Lane::Jungle), "empty lanes left out");

    let f = |s: &DraftSide, label: &str| s.factors.iter().find(|f| f.label == label).cloned();
    let al = f(&d.allies, "Lanes").unwrap();
    assert_eq!(al.detail, "Top 47.0%, Mid 49.0%");
    assert!(al.points < -2.0, "{al:?}");
    let el = f(&d.enemies, "Lanes").unwrap();
    assert!((el.points + al.points).abs() < 1e-4, "lanes are zero-sum");
    assert_eq!(el.detail, "Top 53.0%, Mid 51.0%");

    // Unmatched champions count by their own lane win rate.
    let champs = f(&d.allies, "Champions").unwrap();
    assert!(champs.detail.contains("Jinx 52.0%") && !champs.detail.contains("Ahri"), "{champs:?}");
    assert!(f(&d.enemies, "Champions").is_none());

    assert_eq!(d.allies.duos, vec![DraftDuo { a: JINX, b: THRESH, delta: 2.0, games: 1_500 }]);
    assert_eq!(f(&d.allies, "Synergy").unwrap().detail, "Jinx + Thresh +2.0");

    assert_eq!(d.allies.champs, vec![AHRI, GAREN, JINX, THRESH]);
    assert_eq!((d.allies.frontline, d.allies.engage), (0, 1));
    assert!(d.allies.gaps.contains(&"No frontline".to_string()), "{:?}", d.allies.gaps);
    assert!((d.allies.physical + d.allies.magic + d.allies.true_dmg - 1.0).abs() < 1e-4);
    assert!(d.enemies.gaps.is_empty(), "two champions: too early for gaps");
    assert!(d.enemies.physical > 0.9);
    let total: f32 = d.allies.factors.iter().map(|f| f.points).sum();
    assert!((d.allies.total - total).abs() < 1e-4);
}

#[test]
fn no_report_outside_rift_or_before_reveals() {
    let c = cs(Lane::Middle);
    assert!(run(Run { cs: &c, mine: None, enemies: &[], tiers: &[], draft: DraftData::default() }).draft.is_none());
    let mut aram = cs(Lane::Middle);
    (aram.mode, aram.my_lane, aram.my_pick) = (Mode::Aram, None, Some(AHRI));
    assert!(run(Run { cs: &aram, mine: None, enemies: &[], tiers: &[], draft: DraftData::default() }).draft.is_none());
}
