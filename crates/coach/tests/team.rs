//! ARAM / Mayhem team composition and bench ranking: tier or ARAM win rate
//! plus how well each option fills my team's gaps.

mod common;

use coach::{ChampSelectInput, champ_select_advice};
use common::*;
use model::*;

fn cs(mode: Mode, mine: ChampKey, allies: &[ChampKey], bench: &[ChampKey]) -> ChampSelect {
    ChampSelect {
        queue_id: if mode == Mode::Aram { 450 } else { 2400 },
        mode,
        my_lane: None,
        my_hover: None,
        my_pick: Some(mine),
        locked: true,
        allies: allies.to_vec(),
        enemies: vec![],
        bans: vec![],
        bench: bench.to_vec(),
        timer_phase: "BAN_PICK".into(),
        ally_lanes: vec![],
    }
}

fn aram(champ: ChampKey, wr: f32) -> TierEntry {
    TierEntry { champ, lane: Lane::Middle, win_rate: wr, pick_rate: 5.0, ban_rate: 0.0, games: 30_000, tier: None }
}

fn mayhem(tiers: &[(ChampKey, &str)]) -> MayhemData {
    MayhemData {
        patch: "16_21".into(),
        augment_names: Default::default(),
        champ_tiers: tiers.iter().map(|&(c, t)| (c, t.to_string())).collect(),
    }
}

fn run(sd: &StaticData, c: &ChampSelect, tiers: &[TierEntry], m: Option<&MayhemData>) -> ChampSelectAdvice {
    champ_select_advice(&ChampSelectInput {
        cs: c,
        sd,
        mine: None,
        enemies: &[],
        tier_list: tiers,
        mayhem: m,
        draft: Default::default(),
    })
}

fn order(a: &ChampSelectAdvice) -> Vec<ChampKey> {
    a.bench.iter().map(|b| b.champ).collect()
}

#[test]
fn best_mayhem_tier_is_not_always_best_for_the_team() {
    let sd = sd();
    // All-magic allies with no frontline.
    let c = cs(Mode::Mayhem, ZED, &[LUX, SYNDRA, SORAKA, VLADIMIR], &[AHRI, JINX, MUNDO]);
    let m = mayhem(&[(AHRI, "S"), (JINX, "A"), (MUNDO, "A"), (ZED, "B")]);
    let a = run(&sd, &c, &[], Some(&m));
    assert_eq!(order(&a), vec![MUNDO, AHRI, JINX, ZED], "{:#?}", a.bench);
    let mundo = &a.bench[0];
    assert_eq!(mundo.reasons[0], "Mayhem tier A");
    assert!(mundo.reasons[1].starts_with("adds physical damage: team is 9"), "{:?}", mundo.reasons);
    assert_eq!(mundo.reasons[2], "adds frontline: team has none");
    assert_eq!(a.bench[1].reasons, vec!["Mayhem tier S"]);
    assert!(a.bench.iter().all(|b| b.mine == (b.champ == ZED)));
    assert!(a.picks.is_empty() && a.draft.is_none());

    let team = a.team.expect("team comp");
    assert_eq!(team.champs, vec![ZED, LUX, SYNDRA, SORAKA, VLADIMIR]);
    assert!(team.gaps.contains(&"No frontline".to_string()), "{:?}", team.gaps);
    assert_eq!(team.factors.len(), 1);
    assert!(team.total < 0.0);
}

#[test]
fn aram_bench_blends_win_rate_with_fit() {
    let sd = sd();
    let tiers = [aram(VAYNE, 53.0), aram(LUX, 52.5), aram(EZREAL, 51.0), aram(GAREN, 50.0), aram(DARIUS, 50.0)];
    let c = cs(Mode::Aram, EZREAL, &[GAREN, DARIUS, CAITLYN], &[VAYNE, LUX, KAYLE]);
    let a = run(&sd, &c, &tiers, None);
    assert_eq!(order(&a), vec![LUX, VAYNE, EZREAL, KAYLE], "Kayle has no data: last");
    let lux = &a.bench[0];
    assert_eq!(lux.win_rate, Some(52.5));
    assert_eq!(lux.reasons[0], "52.5% ARAM win rate");
    assert!(lux.reasons[1].starts_with("adds magic damage: team is"), "{:?}", lux.reasons);
    assert_eq!((a.bench[3].score, a.bench[3].reasons.len()), (None, 0));

    // Alone, win rate decides.
    let solo = cs(Mode::Aram, EZREAL, &[], &[VAYNE, LUX]);
    assert_eq!(order(&run(&sd, &solo, &tiers, None)), vec![VAYNE, LUX, EZREAL]);
}

#[test]
fn peel_for_carries_is_a_gap_and_a_fill() {
    let mut sd = sd();
    sd.champs.get_mut(&SONA).unwrap().traits.peel = 0.9;
    let c = cs(Mode::Aram, EZREAL, &[JINX, CAITLYN, LEONA], &[SONA, ZED]);
    let tiers = [aram(SONA, 50.0), aram(ZED, 50.0), aram(EZREAL, 50.0)];
    let a = run(&sd, &c, &tiers, None);
    assert_eq!(a.bench[0].champ, SONA, "{:#?}", a.bench);
    assert!(a.bench[0].reasons.contains(&"adds peel for the carries".to_string()), "{:?}", a.bench[0].reasons);
    let team = a.team.unwrap();
    assert!(team.gaps.contains(&"No peel for carries".to_string()), "{:?}", team.gaps);
    assert_eq!(team.peel, 0.0);

    // With a peeler on the team the gap is gone.
    let c = cs(Mode::Aram, SONA, &[JINX, CAITLYN, LEONA], &[]);
    let team = run(&sd, &c, &tiers, None).team.unwrap();
    assert!(!team.gaps.iter().any(|g| g.contains("peel")), "{:?}", team.gaps);
    assert!((team.peel - 0.9).abs() < 1e-6);
}
