//! Pre-game comps, matching a past board to a comp, the post-game review and trends.

mod common;

use common::*;
use tft_coach::*;
use tft_model::*;

fn unit(api: &str, star: u32, i: &[&str]) -> TftPastUnit {
    TftPastUnit { api: api.into(), star, items: items(i) }
}

fn game(placement: u32, mode: TftMode, units: Vec<TftPastUnit>, traits: &[(&str, u32, u32)]) -> TftPastGame {
    TftPastGame {
        game_id: placement as u64,
        created: 0,
        queue: if mode == TftMode::Choncc { QUEUE_CHONCC } else { QUEUE_RANKED },
        mode,
        placement,
        level: 7,
        last_round: 30,
        duration_s: 1800,
        units,
        traits: traits.iter().map(|(t, n, tier)| (t.to_string(), *n, *tier)).collect(),
        comp: None,
    }
}

fn rengar_board(placement: u32, mode: TftMode) -> TftPastGame {
    let units = vec![unit(RENGAR, 2, &[BT, IE]), unit(GAREN, 2, &[]), unit(LEONA, 1, &[]), unit(TARIC, 1, &[]), unit(VI, 1, &[])];
    game(placement, mode, units, &[(DEFENDER, 4, 2), (DUELIST, 2, 1), (BRUISER, 2, 1)])
}

fn off_meta(placement: u32, mode: TftMode) -> TftPastGame {
    game(placement, mode, vec![unit(LUX, 1, &[])], &[])
}

#[test]
fn pregame_lists_top_comps_or_the_starters() {
    let (s, m) = (set(), meta());
    assert_eq!(ids(&pregame(&m, &s, None)), [RENGAR_COMP, AHRI_COMP, DUEL_COMP, TARIC_COMP]);
    let p = pregame(&m, &s, Some(TARIC));
    assert_eq!(ids(&p), [RENGAR_COMP, TARIC_COMP]);
    assert_eq!(p[1].have, [TARIC]);
    // A starter no comp uses: all comps.
    assert_eq!(pregame(&m, &s, Some("DA_18_Nobody")).len(), 4);
}

#[test]
fn a_past_board_matches_the_nearest_comp() {
    let m = meta();
    assert_eq!(match_comp(&rengar_board(3, TftMode::Standard), &m).as_deref(), Some(RENGAR_COMP));
    assert_eq!(match_comp(&off_meta(3, TftMode::Standard), &m), None);
    // Most of Defender Rengar without Rengar is the Vi comp it still has the carry of.
    let mut no_carry = rengar_board(3, TftMode::Standard);
    no_carry.units.remove(0);
    assert_eq!(match_comp(&no_carry, &m).as_deref(), Some(DUEL_COMP));
}

#[test]
fn review_compares_placement_items_and_level_with_the_comp() {
    let (s, m) = (set(), meta());
    let r = review(&rengar_board(6, TftMode::Standard), &m, &s);
    let pick = r.comp.unwrap();
    assert_eq!((pick.comp.as_str(), pick.fit, pick.avg_place), (RENGAR_COMP, 1.0, 4.0));
    assert_eq!(
        r.takeaways,
        [
            "6th with Defender Rengar: worse than its 4.0 average",
            "Rengar had Bloodthirster, Infinity Edge; the best build is Bloodthirster, Infinity Edge, Titan's Resolve",
            "You ended at level 7; Defender Rengar plays at 8 (by 4-2)",
        ]
    );
    let mut won = rengar_board(1, TftMode::Standard);
    won.units[0].items.push(TITANS.into());
    won.level = 8;
    let r = review(&won, &m, &s);
    assert_eq!(r.takeaways, ["1st with Defender Rengar: better than its 4.0 average", "Rengar had the best build"]);
    let r = review(&off_meta(7, TftMode::Standard), &m, &s);
    assert_eq!((r.comp, r.takeaways), (None, vec!["7th with a board no meta comp matches: try a listed comp next game".to_string()]));
    let r = review(&off_meta(1, TftMode::Choncc), &m, &s);
    assert_eq!(r.takeaways, ["1st with your own board: no meta comp matches it"]);
}

#[test]
fn trends_per_mode_with_comps_played() {
    let m = meta();
    let games = [
        rengar_board(1, TftMode::Standard),
        rengar_board(6, TftMode::Standard),
        off_meta(3, TftMode::Standard),
        rengar_board(2, TftMode::Choncc),
        off_meta(8, TftMode::Choncc),
    ];
    let t = trends(&games, &m);
    assert_eq!(t.len(), 2);
    let std = &t[0];
    assert_eq!((std.mode, std.games), (TftMode::Standard, 3));
    assert!((std.avg_place - 10.0 / 3.0).abs() < 1e-4 && (std.top4 - 2.0 / 3.0).abs() < 1e-4);
    assert_eq!(std.comps.len(), 2);
    let c = &std.comps[0];
    assert_eq!((c.comp.as_str(), c.name.as_str(), c.games, c.avg_place, c.top4), (RENGAR_COMP, "Defender Rengar", 2, 3.5, 0.5));
    assert_eq!((std.comps[1].name.as_str(), std.comps[1].games), ("Other", 1));
    let ch = &t[1];
    assert_eq!((ch.mode, ch.games, ch.avg_place, ch.top4), (TftMode::Choncc, 2, 5.0, 0.5));
    assert!(trends(&[], &m).is_empty());
}
