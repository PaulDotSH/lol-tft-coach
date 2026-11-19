//! Item tables for the lane build and the game build.

mod common;

use coach::vs_build;
use common::*;
use model::*;

fn page(slots: Vec<Vec<ItemStat>>, boots: Vec<ItemStat>) -> MatchupBuild {
    MatchupBuild {
        vs: ZED,
        games: 4_000,
        win_rate: 47.0,
        start: vec![1055, 2003],
        start_games: 3_000,
        start_wr: 47.5,
        core: vec![],
        runes: vec![],
        rune_page: None,
        summoners: vec![],
        skills: String::new(),
        skill_order: String::new(),
        skill_order_games: 0,
        usual_wr: None,
        slots,
        boots,
        skill_max: vec![],
        by_length: vec![],
        path: vec![],
        best_path: vec![],
        patch: "30".into(),
    }
}

fn counts(v: &[ItemStat]) -> Vec<(ItemId, u32, u32)> {
    v.iter().map(|r| (r.id, r.games, r.wins)).collect()
}

#[test]
fn game_tables_cap_every_source_and_never_scale_one_up() {
    let mine = ahri_stats();
    // My first slot: Luden's 55% of 20,000, Zhonya's 50% of 2,000, cut to 2,000 games together.
    let full = page(vec![vec![row(ZHONYAS, 3_000, 60.0), row(MORELLO, 1_000, 50.0)]], vec![row(SORCS, 300, 60.0)]);
    let g = vs_build::game(&mine, &[&full]);
    assert_eq!(counts(&g.slots[0]), [(LUDENS, 1818, 1000), (ZHONYAS, 182 + 1500, 91 + 900), (MORELLO, 500, 250)]);
    assert_eq!(g.slots.len(), mine.slots.len(), "slots the page lacks keep my own, capped");
    assert_eq!(g.slots[1].iter().map(|r| r.games).sum::<u32>(), 2000);
    // 300 games of boots stay 300.
    let sorcs = g.boots.iter().find(|r| r.id == SORCS).unwrap();
    let own: u32 = mine.boots.iter().map(|r| r.games).sum();
    let mine_sorcs = mine.boots.iter().find(|r| r.id == SORCS).map_or(0.0, |r| r.games as f32 * (2000.0 / own as f32).min(1.0));
    assert_eq!(sorcs.games, mine_sorcs.round() as u32 + 300);

    // Two pages weigh the same once both are past the cap, whatever their size.
    let big = page(vec![vec![row(ZHONYAS, 30_000, 60.0)]], vec![]);
    let small = page(vec![vec![row(MORELLO, 2_000, 50.0)]], vec![]);
    let g = vs_build::game(&mine, &[&big, &small]);
    assert_eq!(counts(&g.slots[0])[..3], [(ZHONYAS, 182 + 2000, 91 + 1200), (MORELLO, 2000, 1000), (LUDENS, 1818, 1000)]);

    // A page with more slots than my own stats adds them.
    let deep = page((0..6).map(|k| if k == 5 { vec![row(MORELLO, 40, 50.0)] } else { vec![] }).collect(), vec![]);
    assert_eq!(counts(&vs_build::game(&mine, &[&deep]).slots[5]), [(MORELLO, 40, 20)]);

    let none = vs_build::game(&mine, &[]);
    assert_eq!((counts(&none.slots[0]), counts(&none.boots)), (counts(&mine.slots[0]), counts(&mine.boots)), "no page: my own tables untouched");
}

#[test]
fn lane_tables_are_the_page() {
    let mine = ahri_stats();
    let p = page(vec![vec![row(ZHONYAS, 3_000, 60.0)]], vec![row(SORCS, 300, 60.0)]);
    let l = vs_build::lane(&mine, &p);
    assert_eq!((counts(&l.slots[0]), counts(&l.boots), l.slots.len()), (vec![(ZHONYAS, 3_000, 1_800)], vec![(SORCS, 300, 180)], 1));
    assert_eq!((l.win_rate, l.games, l.patch.as_str(), l.start.as_slice()), (47.0, 4_000, "30", [1055, 2003].as_slice()));
    assert_eq!((l.champ, l.lane), (mine.champ, mine.lane));
    // A page without a start set or patch keeps my own.
    let bare = MatchupBuild { start: vec![], patch: String::new(), ..p };
    let l = vs_build::lane(&mine, &bare);
    assert_eq!((l.start, l.patch), (mine.start, mine.patch));
}
