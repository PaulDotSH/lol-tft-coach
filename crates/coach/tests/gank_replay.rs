//! Gank warnings replayed over every enemy blip read off real games' minimaps (Swiftplay).

use coach::{GankEnemies, GankTracker};
use model::*;

type Reads = Vec<(f32, Vec<(i32, i32)>)>;

/// Red side, support; Warwick jungle.
const RED_SUPPORT: &str = include_str!("../../../fixtures/coach/gank_red_support.json");
/// Blue side, mid; Jinx jungle.
const BLUE_MID: &str = include_str!("../../../fixtures/coach/gank_blue_mid.json");

/// Blue side, mid (Zed); Lillia jungle, Sylas mid, Mundo top, Vayne and Nami bot; blips named by portrait.
const BLUE_MID_NAMED: &str = include_str!("../../../fixtures/coach/gank_blue_mid_named.json");

/// (time, say) of each newly spoken call, replaying named reads.
fn replay_named(game: &str, blue: bool, lane: Option<Lane>, enemies: &GankEnemies) -> Vec<(u32, String, String)> {
    let reads: Vec<(f32, Vec<(i32, i32, Option<ChampKey>)>)> = serde_json::from_str(game).unwrap();
    let mut t = GankTracker::default();
    let mut said: Vec<String> = vec![];
    let mut out = vec![];
    for (time, enemies_seen) in reads {
        t.observe(&enemies_seen, blue, time);
        for c in t.warnings(time, lane, enemies) {
            if !said.contains(&c.key) {
                said.push(c.key.clone());
                out.push((time as u32, c.key, c.say));
            }
        }
    }
    out
}

fn zed_game() -> GankEnemies {
    let roster = [(36, "Dr. Mundo", Lane::Top), (876, "Lillia", Lane::Jungle), (517, "Sylas", Lane::Middle), (67, "Vayne", Lane::Bottom), (267, "Nami", Lane::Support)];
    GankEnemies {
        names: roster.iter().map(|&(k, n, _)| (k, n.to_string())).collect(),
        jungler: Some(876),
        lanes: roster.iter().map(|&(k, _, l)| (k, l)).collect(),
    }
}

/// Real game as mid: Sylas (lane opponent) roamed top at 10:37; Lillia ganked mid at 3:35, Nami roamed mid at 5:13.
#[test]
fn mid_hears_sylas_roam_and_the_ganks() {
    let w = replay_named(BLUE_MID_NAMED, true, Some(Lane::Middle), &zed_game());
    for (t, k, s) in &w {
        println!("{:>2}:{:02} {k:<22} {s}", t / 60, t % 60);
    }
    let at = |prefix: &str, from: u32, to: u32| w.iter().any(|(t, k, _)| k.starts_with(prefix) && (from..=to).contains(t));
    assert!(at("gank-roam-517-", 630, 645), "Sylas top roam");
    assert!(at("gank-mid-", 205, 220), "Lillia gank");
}

/// Blue side, support (Thresh); Wukong jungle, Jax top, Viktor mid, Sivir and Zilean bot; blips named by portrait.
const BLUE_SUPPORT_NAMED: &str = include_str!("../../../fixtures/coach/gank_blue_support_named.json");

fn thresh_game() -> GankEnemies {
    let roster = [(24, "Jax", Lane::Top), (62, "Wukong", Lane::Jungle), (112, "Viktor", Lane::Middle), (15, "Sivir", Lane::Bottom), (26, "Zilean", Lane::Support)];
    GankEnemies {
        names: roster.iter().map(|&(k, n, _)| (k, n.to_string())).collect(),
        jungler: Some(62),
        lanes: roster.iter().map(|&(k, _, l)| (k, l)).collect(),
    }
}

/// Real game as support: Wukong ganked bot at 7:21 (Twitch, then you at 7:25) and again at 10:54 (Twitch, you at 11:11).
#[test]
fn support_hears_wukong_before_his_bot_ganks() {
    let w = replay_named(BLUE_SUPPORT_NAMED, true, Some(Lane::Support), &thresh_game());
    for (t, k, s) in &w {
        println!("{:>2}:{:02} {k:<22} {s}", t / 60, t % 60);
    }
    let at = |prefix: &str, from: u32, to: u32| w.iter().any(|(t, k, _)| k.starts_with(prefix) && (from..=to).contains(t));
    assert!(at("gank-jg-", 436, 441), "Wukong showing bot before the 7:21 gank");
    assert!(at("gank-jg-", 652, 654), "Wukong showing bot before the 10:54 gank");
    assert!(!w.iter().any(|(_, _, s)| s.contains("roaming bot side")), "Zilean warding by the drake is home");
}

/// (time, key) of each newly spoken warning.
fn replay(game: &str, blue: bool, lane: Option<Lane>) -> Vec<(u32, String)> {
    let reads: Reads = serde_json::from_str(game).unwrap();
    let mut t = GankTracker::default();
    let mut said: Vec<String> = vec![];
    let mut out = vec![];
    for (time, enemies) in reads {
        t.observe(&enemies.iter().map(|&(x, y)| (x, y, None)).collect::<Vec<_>>(), blue, time);
        for c in t.warnings(time, lane, &Default::default()) {
            if !said.contains(&c.key) {
                said.push(c.key.clone());
                out.push((time as u32, c.key));
            }
        }
    }
    out
}

fn kind(w: &[(u32, String)], prefix: &str) -> Vec<u32> {
    w.iter().filter(|(_, k)| k.starts_with(prefix)).map(|(t, _)| *t).collect()
}

/// Real events: Warwick in your jungle at 4:18 and 9:44, lurking by bot lane at 5:20 before ganking; the
/// enemy laners standing at the bot lane bend (river crossing) all game are not ganks.
#[test]
fn support_hears_the_real_ganks_only() {
    let w = replay(RED_SUPPORT, false, Some(Lane::Support));
    let jungle = kind(&w, "gank-jungle");
    assert!(jungle.iter().any(|t| (256..=259).contains(t)) && jungle.iter().any(|t| (580..=586).contains(t)), "{w:?}");
    let bot = kind(&w, "gank-bot");
    assert!(bot.iter().any(|t| (316..=323).contains(t)), "{w:?}");
    assert!(bot.len() <= 5, "{w:?}");
    assert!(kind(&w, "gank-mid").is_empty(), "{w:?}");
}

/// Regression: the enemy mid laner holding lane was called a gank. The calls left are enemies in the river by mid.
#[test]
fn mid_laner_is_not_a_gank() {
    let w = replay(RED_SUPPORT, false, Some(Lane::Middle));
    assert!(kind(&w, "gank-mid").len() <= 4, "{w:?}");
}

/// Jinx showed for one read in the river beside mid at 8:02, then ganked; she spent the rest of the early game bot side.
#[test]
fn mid_hears_jinx_from_the_river() {
    let w = replay(BLUE_MID, true, Some(Lane::Middle));
    let mid = kind(&w, "gank-mid");
    assert!(mid.iter().any(|t| (478..=483).contains(t)), "{w:?}");
    assert!(mid.len() <= 3, "{w:?}");
}

fn unnamed(game: &str) -> String {
    let reads: Reads = serde_json::from_str(game).unwrap();
    let named: Vec<_> = reads.into_iter().map(|(t, b)| (t, b.into_iter().map(|(x, y)| (x, y, None::<ChampKey>)).collect::<Vec<_>>())).collect();
    serde_json::to_string(&named).unwrap()
}

/// Every replay heard as the jungler, snapshotted in `fixtures/coach/gank_jungle_calls.txt`.
#[test]
fn the_jungler_hears_invades_roams_and_their_jungler() {
    let none = GankEnemies::default();
    let games = [
        ("red_support", unnamed(RED_SUPPORT), false, &none),
        ("blue_mid", unnamed(BLUE_MID), true, &none),
        ("blue_mid_named", BLUE_MID_NAMED.to_string(), true, &zed_game()),
        ("blue_support_named", BLUE_SUPPORT_NAMED.to_string(), true, &thresh_game()),
    ];
    let mut out = String::new();
    for (name, reads, blue, enemies) in games {
        out.push_str(&format!("# {name}\n"));
        for (t, k, s) in replay_named(&reads, blue, Some(Lane::Jungle), enemies) {
            out.push_str(&format!("{:>2}:{:02} {k} | {s}\n", t / 60, t % 60));
        }
    }
    let path = format!("{}/../../fixtures/coach/gank_jungle_calls.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &out).unwrap();
    }
    let want = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    assert_eq!(out, want);
    assert!(out.contains("Lillia seen ") && out.contains("Wukong seen "), "the jungler call says where their jungler was seen");
    assert!(!out.contains("Play safe") && !out.contains("spotted"), "no laner wording");
    assert!(out.contains("invading your jungle"));
}
