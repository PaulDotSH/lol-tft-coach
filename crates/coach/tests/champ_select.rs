//! Champ select and recap behaviour.

mod common;

use coach::{ChampSelectInput, champ_select_advice, recap};
use common::*;
use model::*;

fn cs(mode: Mode) -> ChampSelect {
    ChampSelect {
        queue_id: 420,
        mode,
        my_lane: Some(Lane::Middle),
        my_hover: None,
        my_pick: None,
        locked: false,
        allies: vec![],
        enemies: vec![],
        bans: vec![],
        bench: vec![],
        timer_phase: "BAN_PICK".into(),
    }
}

fn tier(champ: ChampKey, wr: f32, pr: f32, games: u32) -> TierEntry {
    TierEntry { champ, lane: Lane::Middle, win_rate: wr, pick_rate: pr, ban_rate: 1.0, games, tier: None }
}

fn mid_tiers() -> Vec<TierEntry> {
    vec![
        tier(AHRI, 51.0, 8.0, 50_000),
        tier(SYNDRA, 52.0, 6.0, 30_000),
        tier(LUX, 50.0, 7.0, 40_000),
        tier(VLADIMIR, 50.5, 5.0, 20_000),
        tier(ZED, 50.0, 9.0, 45_000),
        tier(MALPHITE, 58.0, 0.1, 100),
    ]
}

/// Zed's lolalytics page: his win rate vs mid champs.
fn zed_stats() -> ChampStats {
    let mut s = ahri_stats();
    s.champ = ZED;
    s.counters = vec![
        matchup(LUX, 45.0, 3_000, Lane::Middle),
        matchup(SYNDRA, 52.0, 3_000, Lane::Middle),
        matchup(AHRI, 50.0, 5_000, Lane::Middle),
    ];
    s
}

fn run(c: &ChampSelect, mine: Option<&ChampStats>, enemies: &[ChampStats], tiers: &[TierEntry], mayhem: Option<&MayhemData>) -> ChampSelectAdvice {
    let sd = sd();
    champ_select_advice(&ChampSelectInput { cs: c, sd: &sd, mine, enemies, tier_list: tiers, mayhem })
}

#[test]
fn picks_counter_the_lane_opponent() {
    let mut c = cs(Mode::Rift);
    c.enemies = vec![ZED];
    let a = run(&c, None, &[zed_stats()], &mid_tiers(), None);
    assert_eq!(a.picks[0].champ, LUX, "{:?}", a.picks);
    assert_eq!(a.picks[0].reason, "55.0% vs Zed (lane)");
    assert!(a.picks.len() <= 5);
    assert!(!a.picks.iter().any(|p| p.champ == ZED || p.champ == MALPHITE), "picked or low-sample champ suggested");
    assert_eq!(a.enemies, vec![ZED]);
}

#[test]
fn off_lane_enemies_weigh_less() {
    let mut c = cs(Mode::Rift);
    c.enemies = vec![ZED];
    let mut z = zed_stats();
    z.lane = Some(Lane::Top);
    let lane = run(&c, None, &[zed_stats()], &mid_tiers(), None).picks[0].score;
    let off = run(&c, None, &[z], &mid_tiers(), None);
    assert_eq!(off.picks[0].champ, LUX);
    assert!(off.picks[0].reason.ends_with("vs Zed"));
    // Blend weight is the same with a single enemy; scores match.
    assert!((off.picks[0].score - lane).abs() < 1e-3);
}

#[test]
fn no_enemies_revealed_gives_best_tier_list_picks() {
    let mut c = cs(Mode::Rift);
    c.bans = vec![VLADIMIR];
    let a = run(&c, None, &[], &mid_tiers(), None);
    assert_eq!(a.picks[0].champ, SYNDRA);
    assert_eq!(a.picks[0].reason, "52.0% win rate in middle");
    assert!(!a.picks.iter().any(|p| p.champ == VLADIMIR));
}

#[test]
fn bans_blend_my_counters_and_meta_threats() {
    let mut c = cs(Mode::Rift);
    c.my_hover = Some(AHRI);
    let a = run(&c, Some(&ahri_stats()), &[], &mid_tiers(), None);
    assert_eq!(a.bans[0].champ, SYNDRA, "{:?}", a.bans);
    assert!(a.bans[0].reason.starts_with("counters Ahri: 44.0%"), "{}", a.bans[0].reason);
    assert!(!a.bans.iter().any(|b| b.champ == AHRI || b.champ == LUX), "{:?}", a.bans);

    c.bans = vec![SYNDRA];
    let a = run(&c, Some(&ahri_stats()), &[], &mid_tiers(), None);
    assert!(!a.bans.iter().any(|b| b.champ == SYNDRA));
    assert_eq!(a.bans[0].champ, ZED);

    // No hover: meta threats only.
    let a = run(&cs(Mode::Rift), None, &[], &mid_tiers(), None);
    assert_eq!(a.bans[0].champ, SYNDRA);
    assert!(a.bans[0].reason.starts_with("meta threat"));
}

#[test]
fn build_plan_once_champ_known() {
    let mut c = cs(Mode::Rift);
    let a = run(&c, Some(&ahri_stats()), &[], &mid_tiers(), None);
    assert!(a.build.is_none(), "no champ yet");

    c.my_pick = Some(AHRI);
    c.locked = true;
    c.enemies = vec![SORAKA, VLADIMIR, AATROX, MUNDO];
    // Realistic third-item spread: Morello a little behind Void Staff.
    let mut st = ahri_stats();
    st.slots[2] = vec![row(VOID_STAFF, 8_000, 52.0), row(BANSHEES, 3_000, 52.0), row(ZHONYAS, 4_000, 51.0), row(MORELLO, 3_000, 51.0)];
    let a = run(&c, Some(&st), &[], &mid_tiers(), None);
    let b = a.build.expect("build plan");
    assert_eq!(b.start, vec![1056, 2003, 2003]);
    assert_eq!(b.core.len(), 3);
    assert_eq!(b.core[0].item, LUDENS);
    assert!(b.core.iter().any(|p| p.item == MORELLO), "healers revealed: {:?}", b.core);
    assert_eq!(b.boots.map(|p| p.item), Some(SORCS));
    assert_eq!(a.my_champ, Some(AHRI));

    // Stats for a different champion are ignored.
    c.my_pick = Some(LUX);
    assert!(run(&c, Some(&ahri_stats()), &[], &mid_tiers(), None).build.is_none());
}

#[test]
fn mayhem_bench_ranked_by_tier_no_picks_or_bans() {
    let mut c = cs(Mode::Mayhem);
    c.my_lane = None;
    c.my_pick = Some(AHRI);
    c.bench = vec![LUX, ZED, JINX, AHRI];
    let md = MayhemData {
        patch: "16_21".into(),
        augment_names: Default::default(),
        champ_tiers: [(AHRI, "A".to_string()), (LUX, "S+".into()), (ZED, "B".into()), (JINX, "S".into())].into(),
    };
    let a = run(&c, None, &[], &mid_tiers(), Some(&md));
    assert!(a.picks.is_empty() && a.bans.is_empty());
    let order: Vec<_> = a.bench.iter().map(|b| b.champ).collect();
    assert_eq!(order, vec![LUX, JINX, AHRI, ZED]);
    assert!(a.bench.iter().all(|b| b.mine == (b.champ == AHRI)));
    assert_eq!(a.bench[0].tier.as_deref(), Some("S+"));
}

#[test]
fn aram_bench_by_win_rate_else_kept_in_order() {
    let mut c = cs(Mode::Aram);
    c.my_lane = None;
    c.my_hover = Some(LUX);
    c.bench = vec![ZED, SYNDRA, JINX];
    let a = run(&c, None, &[], &mid_tiers(), None);
    let order: Vec<_> = a.bench.iter().map(|b| b.champ).collect();
    assert_eq!(order, vec![SYNDRA, LUX, ZED, JINX]);
    let a = run(&c, None, &[], &[], None);
    let order: Vec<_> = a.bench.iter().map(|b| b.champ).collect();
    assert_eq!(order, vec![LUX, ZED, SYNDRA, JINX]);
}

#[test]
fn recap_notes_compare_to_benchmarks() {
    let sd = sd();
    let eog = EogStats {
        champ: AHRI,
        mode: Mode::Rift,
        win: false,
        duration_s: 1800,
        kills: 4,
        deaths: 7,
        assists: 6,
        cs: 183,
        gold: 11_000,
        damage_to_champs: 24_000,
        damage_taken: 38_000,
        vision: 21,
        items: vec![LUDENS, SORCS, ZHONYAS],
    };
    let b = ahri_stats().benchmarks;
    let r = recap(&eog, &b, &[LUDENS, SORCS, RABADONS], &sd);
    let has = |s: &str| r.notes.iter().any(|n| n == s);
    assert!(has("4/7/6 on Ahri"), "{:?}", r.notes);
    assert!(has("CS 6.1/min vs 7.4 avg"), "{:?}", r.notes);
    assert!(has("Damage 800/min vs 950 avg"), "{:?}", r.notes);
    assert!(has("Took 38.0k damage (1267/min)"), "{:?}", r.notes);
    assert!(has("Vision score 21"), "{:?}", r.notes);
    assert!(has("Built 2 of 3 recommended items"), "{:?}", r.notes);
    assert!(r.notes.iter().any(|n| n.starts_with("Died 7×")));
    assert_eq!(r.recommended, vec![LUDENS, SORCS, RABADONS]);
    assert_eq!(r.stats, eog);
}
