//! Gank warnings replayed over enemy blips read off a real game's minimap (Swiftplay, red side, support).

use coach::GankTracker;
use model::*;

type Reads = Vec<(f32, Vec<(i32, i32)>)>;

fn reads() -> Reads {
    serde_json::from_str(include_str!("../../../fixtures/coach/gank_red_support.json")).unwrap()
}

/// (time, key) of each newly spoken warning.
fn replay(lane: Option<Lane>) -> Vec<(f32, String)> {
    let mut t = GankTracker::default();
    let mut said: Vec<String> = vec![];
    let mut out = vec![];
    for (time, enemies) in reads() {
        t.observe(&enemies, false, time);
        for c in t.warnings(time, lane) {
            if !said.contains(&c.key) {
                said.push(c.key.clone());
                out.push((time, c.key));
            }
        }
    }
    out
}

fn kind(w: &[(f32, String)], prefix: &str) -> Vec<u32> {
    w.iter().filter(|(_, k)| k.starts_with(prefix)).map(|(t, _)| *t as u32).collect()
}

/// Real events: Warwick in your jungle at 4:18 and 9:44, lurking by bot lane at 5:20 before ganking; the
/// enemy laners standing at the bot lane bend (river crossing) all game are not ganks.
#[test]
fn support_hears_the_real_ganks_only() {
    let w = replay(Some(Lane::Support));
    assert_eq!(kind(&w, "gank-jungle"), [258, 584], "{w:?}");
    let bot = kind(&w, "gank-bot");
    assert!(bot.iter().any(|t| (318..=323).contains(t)), "{w:?}");
    assert!(bot.len() <= 2, "{w:?}");
    assert!(kind(&w, "gank-mid").is_empty(), "{w:?}");
}

/// Regression: the enemy mid laner holding lane was called a gank.
#[test]
fn mid_laner_is_not_a_gank() {
    let w = replay(Some(Lane::Middle));
    assert!(kind(&w, "gank-mid").len() <= 1, "{w:?}");
}
