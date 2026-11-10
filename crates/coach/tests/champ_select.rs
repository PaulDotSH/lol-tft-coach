//! Champ select and recap behaviour.

mod common;

use coach::{ChampSelectInput, VsRunes, champ_select_advice, champ_select_runes, consts::TIER_ROWS, recap};
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
        ally_lanes: vec![],
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
    champ_select_advice(&ChampSelectInput { cs: c, sd: &sd, mine, enemies, tier_list: tiers, mayhem, draft: Default::default() })
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
    // Fetched for my lane like every enemy, but played top: the draft places him there.
    let mut z = zed_stats();
    z.lanes = [(Lane::Top, 90.0), (Lane::Middle, 5.0)].into();
    let lane = run(&c, None, &[zed_stats()], &mid_tiers(), None).picks[0].score;
    let off = run(&c, None, &[z], &mid_tiers(), None);
    assert_eq!(off.picks[0].champ, LUX);
    assert!(off.picks[0].reason.ends_with("vs Zed"));
    // Blend weight is the same with a single enemy; scores match.
    assert!((off.picks[0].score - lane).abs() < 1e-3);
}

fn champs(v: &[TierPick]) -> Vec<ChampKey> {
    v.iter().map(|p| p.champ).collect()
}

#[test]
fn tier_board_ranks_counters_to_the_lane_opponent() {
    let mut c = cs(Mode::Rift);
    c.enemies = vec![ZED];
    c.bans = vec![VLADIMIR];
    let mut tiers = mid_tiers();
    tiers[1].tier = Some("S+".into());
    let t = run(&c, None, &[zed_stats()], &tiers, None).tiers.unwrap();
    assert_eq!((t.lane, t.vs), (Lane::Middle, Some(ZED)));
    assert_eq!(champs(&t.best), [AHRI, SYNDRA, LUX], "tier list order; banned, picked and low-sample dropped");
    assert_eq!(champs(&t.counters), [LUX, AHRI, SYNDRA], "by win rate against Zed");
    let syndra = &t.counters[2];
    assert_eq!((syndra.tier.as_deref(), syndra.vs_wr, syndra.vs_games), (Some("S+"), Some(48.0), 3_000));
    assert_eq!(t.best[0].vs_wr, Some(50.0));

    // Zed plays top: no lane opponent, the tier list alone.
    let mut z = zed_stats();
    z.lanes = [(Lane::Top, 90.0)].into();
    let t = run(&c, None, &[z], &tiers, None).tiers.unwrap();
    assert_eq!(t.vs, None);
    assert!(t.counters.is_empty() && t.best.iter().all(|p| p.vs_wr.is_none()));
}

#[test]
fn tier_board_keeps_my_champion_in_view() {
    let mut c = cs(Mode::Rift);
    c.my_hover = Some(SYNDRA);
    // Ten champions ranked above the usual mid tier list.
    let tiers: Vec<TierEntry> = (0..TIER_ROWS as u32 + 2).map(|i| tier(1000 + i, 55.0, 5.0, 10_000)).chain(mid_tiers()).collect();
    let t = run(&c, None, &[], &tiers, None).tiers.unwrap();
    assert_eq!(t.best.len(), TIER_ROWS + 1);
    assert_eq!((t.best[TIER_ROWS].champ, t.best[TIER_ROWS].mine), (SYNDRA, true));
    assert!(t.best[..TIER_ROWS].iter().all(|p| !p.mine));
    assert!(run(&cs(Mode::Aram), None, &[], &tiers, None).tiers.is_none());
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

/// Sona support: Aftershock / Font of Life / Conditioning / Unflinching + Inspiration.
fn sona_runes() -> ChampStats {
    let page = |primary: Vec<RuneId>, win_rate, games| RunePage {
        primary,
        secondary: vec![8347, 8345],
        shards: vec![5008, 5008, 5001],
        win_rate,
        games,
    };
    let st = |games, win_rate| RuneStat { pick_rate: 10.0, win_rate, games };
    let mut s = ahri_stats();
    s.champ = SONA;
    s.runes = Some(RuneData {
        popular: page(vec![8439, 8463, 8429, 8242], 52.9, 30_000),
        best: page(vec![8465, 8463, 8444, 8242], 54.4, 13_000),
        stats: [
            (8429, vec![st(20_000, 52.8)]),
            (8473, vec![st(2_000, 52.5)]),
            (8444, vec![st(100, 60.0)]),
            (5001, vec![st(40_000, 52.9)]),
            (5013, vec![st(400, 53.0)]),
        ]
        .into_iter()
        .collect(),
    });
    s
}

fn rune_run(enemies: &[ChampKey], mode: Mode) -> Option<RuneAdvice> {
    let mut c = cs(mode);
    c.my_pick = Some(SONA);
    c.enemies = enemies.to_vec();
    run(&c, Some(&sona_runes()), &[], &[], None).runes
}

fn ids(v: &[RunePick]) -> Vec<RuneId> {
    v.iter().map(|r| r.id).collect()
}

fn labels(r: &RuneAdvice) -> Vec<(&str, &str)> {
    r.options.iter().map(|o| (o.label.as_str(), o.source.as_str())).collect()
}

#[test]
fn runes_follow_the_most_played_page() {
    let a = rune_run(&[JINX], Mode::Rift).unwrap();
    assert_eq!(labels(&a), [("Most played", "lolalytics 16.21"), ("Highest win rate", "lolalytics 16.21")]);
    let r = &a.options[0];
    assert_eq!(ids(&r.primary), vec![8439, 8463, 8429, 8242]);
    assert_eq!(ids(&r.secondary), vec![8347, 8345]);
    assert_eq!(ids(&r.shards), vec![5008, 5008, 5001]);
    assert!(r.primary.iter().chain(&r.shards).all(|p| p.reason.is_none()));
    let best = &a.options[1];
    assert_eq!((best.primary[0].id, best.win_rate, best.games), (8465, 54.4, 13_000), "Guardian wins more");
    assert!(rune_run(&[JINX], Mode::Mayhem).is_none(), "Mayhem has no runes");
}

#[test]
fn runes_answer_burst_and_cc() {
    let a = rune_run(&[ZED, SYNDRA, LEONA, MALPHITE, THRESH], Mode::Rift).unwrap();
    let r = &a.options[0];
    assert_eq!(ids(&r.primary), vec![8439, 8463, 8473, 8242], "Bone Plating vs two assassins");
    assert!(r.primary[2].reason.as_deref().unwrap().starts_with("burst from Zed, Syndra"), "{:?}", r.primary[2].reason);
    assert_eq!(ids(&r.shards), vec![5008, 5008, 5013], "Tenacity vs three hard CC");
    // Second Wind wins more but has too few games to be offered.
    assert!(!ids(&r.primary).contains(&8444));
    // The highest win rate page keeps its Second Wind: Bone Plating wins far less than it.
    assert_eq!(ids(&a.options[1].primary), vec![8465, 8463, 8444, 8242]);
    assert_eq!(ids(&a.options[1].shards), vec![5008, 5008, 5013]);
}

/// Sona with Coachless signed in: a Summon Aery page (best WPA) beside lolalytics'.
fn sona_coachless() -> ChampStats {
    let st = |games, win_rate| RuneStat { pick_rate: 10.0, win_rate, games };
    let mut s = sona_runes();
    s.coachless = Some("16.20".into());
    s.summoners = vec![4, 3];
    let page = RunePage { primary: vec![8214, 8226, 8210, 8237], secondary: vec![8463, 8453], shards: vec![5008, 5008, 5001], win_rate: 53.5, games: 20_000 };
    let stats = [(8226, vec![st(15_000, 53.0), st(900, 52.0)]), (8224, vec![st(3_000, 52.4), st(500, 52.0)])];
    s.coachless_runes = Some(RuneData { popular: page.clone(), best: page, stats: stats.into_iter().collect() });
    s
}

#[test]
fn runes_from_both_sources() {
    let mut c = cs(Mode::Rift);
    c.my_pick = Some(SONA);
    c.enemies = vec![JINX];
    let a = run(&c, Some(&sona_coachless()), &[], &[], None).runes.unwrap();
    assert_eq!(labels(&a), [("Best WPA", "Coachless 16.20"), ("Most played", "lolalytics 16.21"), ("Highest win rate", "lolalytics 16.21")]);
    assert_eq!(ids(&a.options[0].primary), vec![8214, 8226, 8210, 8237]);
    assert_eq!(a.summoners, ["Flash", "Exhaust"]);
    // Two magic bursters: Nullifying Orb, which Coachless has the games for.
    c.enemies = vec![SYNDRA, AHRI];
    let a = run(&c, Some(&sona_coachless()), &[], &[], None).runes.unwrap();
    let orb = &a.options[0].primary[1];
    assert_eq!(orb.id, 8224);
    assert!(orb.reason.as_deref().unwrap().starts_with("magic burst from Syndra, Ahri"), "{:?}", orb.reason);
}

#[test]
fn runes_against_the_lane_opponent_come_first() {
    let mut c = cs(Mode::Rift);
    c.my_pick = Some(SONA);
    c.enemies = vec![ZED];
    let mine = sona_coachless();
    let sd = sd();
    let inp = ChampSelectInput { cs: &c, sd: &sd, mine: Some(&mine), enemies: &[], tier_list: &[], mayhem: None, draft: Default::default() };
    let vs_page = RunePage { primary: vec![8465, 8463, 8473, 8242], secondary: vec![8345, 8347], shards: vec![5008, 5008, 5001], win_rate: 49.0, games: 600 };
    let vs_cl = RuneData { popular: vs_page.clone(), best: vs_page, stats: Default::default() };
    // lolalytics' matchup page is its most played page: one option, both named.
    let lola_vs = MatchupBuild {
        vs: ZED,
        games: 900,
        win_rate: 47.0,
        start: vec![],
        start_games: 0,
        start_wr: 0.0,
        core: vec![],
        runes: vec![],
        rune_page: mine.runes.as_ref().map(|r| RunePage { games: 800, win_rate: 47.5, ..r.popular.clone() }),
        summoners: vec![],
        skills: String::new(),
        skill_order: String::new(),
        skill_order_games: 0,
    };
    let a = champ_select_runes(&inp, &VsRunes { vs: ZED, coachless: Some(&vs_cl), lolalytics: Some(&lola_vs) }).unwrap();
    assert_eq!(
        labels(&a),
        [("vs Zed", "Coachless 16.20"), ("vs Zed", "lolalytics 16.21"), ("Best WPA", "Coachless 16.20"), ("Highest win rate", "lolalytics 16.21")]
    );
    assert_eq!((a.options[0].vs, a.options[0].games), (Some(ZED), 600));
    assert_eq!(a.options[1].also, ["Most played · lolalytics 16.21"]);
    assert_eq!((a.options[1].win_rate, a.options[1].games), (47.5, 800));
    // Too few games in the matchup: not offered.
    let thin = RuneData { popular: RunePage { games: 100, ..vs_cl.popular.clone() }, ..vs_cl.clone() };
    let a = champ_select_runes(&inp, &VsRunes { vs: ZED, coachless: Some(&thin), lolalytics: None }).unwrap();
    assert_eq!(a.options[0].label, "Best WPA");
}
