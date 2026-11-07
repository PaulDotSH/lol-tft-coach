//! Early-game gank warnings from enemy blips read off the minimap, spoken as `Call`s.
//! Blips are anonymous, so structures are excluded by zone, and a mid warning needs an enemy off the lane
//! closing on it (the mid laner in the lane is not a gank).

use crate::consts;
use model::*;
use std::collections::VecDeque;

/// A blip in world coordinates seen from your side (your base at the origin).
#[derive(Clone, Copy)]
struct Blip {
    x: i32,
    y: i32,
}

/// One threat's announcement state: a new episode (re-spoken) once the threat lapses.
#[derive(Default)]
struct Episode {
    n: u32,
    last: Option<f32>,
}

impl Episode {
    fn next(&mut self, time: f32) -> u32 {
        if self.last.is_none_or(|l| time - l > consts::GANK_GAP_S) {
            self.n += 1;
        }
        self.last = Some(time);
        self.n
    }
}

#[derive(Default)]
pub struct GankTracker {
    history: VecDeque<(f32, Vec<Blip>)>,
    jungle: Episode,
    lane: Episode,
}

fn dist2(a: Blip, b: Blip) -> i32 {
    (a.x - b.x).pow(2) + (a.y - b.y).pow(2)
}

/// How far (world units) a blip is outside `lane`'s laning area; negative inside. Side lanes are laned around
/// their corner (the river crosses the lane there, so `MapZone::at` calls part of it river).
fn off_lane(lane: MapZone, b: Blip) -> i32 {
    let from = |cx: i32, cy: i32| ((b.x - cx) as f32).hypot((b.y - cy) as f32) as i32 - consts::GANK_LANE_CORNER;
    match lane {
        MapZone::Top => from(0, MAP_END),
        MapZone::Bot => from(MAP_END, 0),
        _ => ((b.x - b.y).abs() as f32 * std::f32::consts::FRAC_1_SQRT_2) as i32 - 920,
    }
}

/// The lane whose ganks you're warned of, and the call for it: your own lane; a jungler (or an unknown role) watches mid.
fn watched(lane: Option<Lane>) -> (MapZone, &'static str, &'static str, &'static str) {
    match lane {
        Some(Lane::Top) => (MapZone::Top, "top", "Enemy near top lane — watch for a gank", "Enemy near top. Watch for a gank."),
        Some(Lane::Bottom | Lane::Support) => (MapZone::Bot, "bot", "Enemy near bot lane — watch for a gank", "Enemy near bot. Watch for a gank."),
        Some(Lane::Middle) => (MapZone::Mid, "mid", "Enemy coming to your lane — watch for a gank", "Gank coming mid. Back off."),
        Some(Lane::Jungle) => (MapZone::Mid, "mid", "Enemy heading mid — counter-gank or ward", "Enemy heading mid. Counter-gank."),
        None => (MapZone::Mid, "mid", "Enemy heading mid — watch for a gank", "Enemy heading mid. Watch for a gank."),
    }
}

impl GankTracker {
    /// Record the enemies visible on the minimap at `time`: world coordinates with the blue base at the origin; `blue` is whether you are blue.
    pub fn observe(&mut self, enemies: &[(i32, i32)], blue: bool, time: f32) {
        if self.history.back().is_some_and(|(t, _)| time < *t) {
            self.history.clear();
        }
        while self.history.front().is_some_and(|(t, _)| time - *t > consts::GANK_MAX_DT) {
            self.history.pop_front();
        }
        let blips = enemies
            .iter()
            .map(|&(x, y)| {
                let (x, y) = MapZone::from_blue(x, y, blue);
                Blip { x, y }
            })
            .collect();
        self.history.push_back((time, blips));
    }

    /// Warnings for this tick (at most one per threat), stable-keyed so each gank is spoken once. Empty after the laning phase or when nothing threatens.
    pub fn warnings(&mut self, time: f32, lane: Option<Lane>) -> Vec<Call> {
        let mut out = Vec::new();
        if time > consts::GANK_EARLY_S {
            return out;
        }
        let Some((_, cur)) = self.history.back() else {
            return out;
        };
        if cur.iter().any(|b| MapZone::at(b.x, b.y) == MapZone::BlueJungle) {
            let n = self.jungle.next(time);
            let (text, say) = if lane == Some(Lane::Jungle) {
                ("Enemy invading your jungle", "Enemy invading your jungle.")
            } else {
                ("Enemy in your jungle — watch for an invade or gank", "Enemy in your jungle.")
            };
            out.push(Call { key: format!("gank-jungle-{n}"), text: text.into(), say: say.into() });
        }
        let (zone, name, text, say) = watched(lane);
        if cur.iter().any(|b| self.threatens(zone, *b)) {
            let n = self.lane.next(time);
            out.push(Call { key: format!("gank-{name}-{n}"), text: text.into(), say: say.into() });
        }
        out
    }

    /// A blip off the lanes (river or a jungle) near `lane` that lurks right beside it (unless walking away) or closed
    /// on it since the newest read at least `GANK_MOVE_DT` old. Blips in a lane are laners.
    fn threatens(&self, lane: MapZone, b: Blip) -> bool {
        if !matches!(MapZone::at(b.x, b.y), MapZone::River | MapZone::BlueJungle | MapZone::RedJungle) {
            return false;
        }
        let gap = off_lane(lane, b);
        if gap <= 0 || gap >= consts::GANK_REACH {
            return false;
        }
        let closing = self.earlier(b).map(|p| off_lane(lane, p) - gap);
        let lurk = if lane == MapZone::Mid { (consts::GANK_MID_HUG..consts::GANK_LURK_MID).contains(&gap) } else { gap < consts::GANK_LURK };
        if lurk && closing.is_none_or(|c| c > -consts::GANK_APPROACH) {
            return true;
        }
        closing.is_some_and(|c| c >= consts::GANK_APPROACH)
    }

    /// Where `b` was in the newest read at least `GANK_MOVE_DT` old: the nearest blip there within `GANK_MAX_STEP`.
    fn earlier(&self, b: Blip) -> Option<Blip> {
        let (now, _) = self.history.back()?;
        let (_, old) = self.history.iter().rev().skip(1).find(|(t, _)| now - t >= consts::GANK_MOVE_DT)?;
        old.iter().copied().filter(|&p| dist2(p, b) <= consts::GANK_MAX_STEP.pow(2)).min_by_key(|&p| dist2(p, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MY_JUNGLE: (i32, i32) = (3500, 7000);
    const RIVER_NEAR_MID: (i32, i32) = (8800, 6000);
    /// In the river, but too far from mid to lurk on it.
    const RIVER_FAR: (i32, i32) = (9400, 5400);
    const ENEMY_JUNGLE: (i32, i32) = (11300, 7800);

    fn tracker() -> GankTracker {
        GankTracker::default()
    }

    fn has_mid(w: &[Call]) -> bool {
        w.iter().any(|c| c.key.starts_with("gank-mid"))
    }

    /// An enemy walking down the river from the bot side into mid lane.
    fn walk(t: &mut GankTracker, lane: Option<Lane>, step: f32, reads: u32) -> Vec<Call> {
        walk_from(t, lane, step, reads, |d| (RIVER_FAR.0 - d, RIVER_FAR.1 + d))
    }

    fn walk_from(t: &mut GankTracker, lane: Option<Lane>, step: f32, reads: u32, at: impl Fn(i32) -> (i32, i32)) -> Vec<Call> {
        let mut w = Vec::new();
        for i in 0..reads {
            let time = 100.0 + i as f32 * step;
            let d = (i as f32 * step * 200.0) as i32;
            t.observe(&[at(d)], true, time);
            w = t.warnings(time, lane);
            if has_mid(&w) {
                break;
            }
        }
        w
    }

    #[test]
    fn enemy_in_my_jungle_warns_once_then_re_warns_after_a_gap() {
        let top = Some(Lane::Top);
        let mut t = tracker();
        t.observe(&[MY_JUNGLE], true, 100.0);
        let w = t.warnings(100.0, top);
        assert_eq!(w.len(), 1);
        let key = w[0].key.clone();
        t.observe(&[MY_JUNGLE], true, 102.0);
        assert_eq!(t.warnings(102.0, top)[0].key, key);
        t.observe(&[], true, 140.0);
        assert!(t.warnings(140.0, top).is_empty());
        t.observe(&[MY_JUNGLE], true, 160.0);
        assert_ne!(t.warnings(160.0, top)[0].key, key);
    }

    #[test]
    fn enemy_jungle_is_not_my_jungle() {
        let mut t = tracker();
        t.observe(&[ENEMY_JUNGLE], true, 100.0);
        assert!(t.warnings(100.0, Some(Lane::Top)).is_empty());
    }

    #[test]
    fn approaching_mid_warns_but_a_standing_blip_does_not() {
        let mut t = tracker();
        t.observe(&[RIVER_FAR], true, 100.0);
        let _ = t.warnings(100.0, None);
        t.observe(&[RIVER_NEAR_MID], true, 102.0);
        assert!(has_mid(&t.warnings(102.0, None)));

        let mut t = tracker();
        t.observe(&[RIVER_FAR], true, 100.0);
        let _ = t.warnings(100.0, None);
        t.observe(&[RIVER_FAR], true, 102.0);
        assert!(!has_mid(&t.warnings(102.0, None)));
    }

    /// A jungler seen once in the river beside mid, then lost in the fog, warns on that one read.
    #[test]
    fn one_sighting_in_the_river_beside_mid_warns() {
        let mut t = tracker();
        t.observe(&[RIVER_NEAR_MID], true, 100.0);
        assert!(has_mid(&t.warnings(100.0, Some(Lane::Middle))));
    }

    /// Regression: the enemy mid laner pushing down the lane toward your tower was called a gank.
    #[test]
    fn mid_laner_walking_down_the_lane_does_not_warn() {
        assert!(!has_mid(&walk_from(&mut tracker(), None, 0.5, 12, |d| (7700 - d, 7900 - d))));
    }

    #[test]
    fn coming_out_of_my_jungle_toward_mid_warns() {
        assert!(has_mid(&walk_from(&mut tracker(), Some(Lane::Middle), 0.5, 8, |d| (4300 + d, 7800 - d))));
    }

    #[test]
    fn walking_away_from_mid_does_not_warn() {
        assert!(!has_mid(&walk_from(&mut tracker(), None, 0.5, 12, |d| (7400 + d, 7400 - d))));
    }

    #[test]
    fn supports_get_no_mid_warnings() {
        assert!(!has_mid(&walk(&mut tracker(), Some(Lane::Support), 0.5, 8)));
    }

    #[test]
    fn silent_after_the_laning_phase() {
        let mut t = tracker();
        let late = consts::GANK_EARLY_S + 1.0;
        t.observe(&[MY_JUNGLE], true, late);
        assert!(t.warnings(late, None).is_empty());
    }

    #[test]
    fn red_side_reads_from_its_own_corner() {
        let mut t = tracker();
        t.observe(&[ENEMY_JUNGLE], false, 100.0);
        assert_eq!(t.warnings(100.0, Some(Lane::Top)).len(), 1);
    }

    #[test]
    fn half_second_reads_with_steady_movement_warn_mid() {
        assert!(has_mid(&walk(&mut tracker(), None, 0.5, 8)));
    }

    #[test]
    fn two_second_reads_with_movement_warn_mid() {
        assert!(has_mid(&walk(&mut tracker(), None, 2.0, 3)));
    }

    #[test]
    fn standing_blip_over_half_second_reads_does_not_warn() {
        let mut t = tracker();
        for i in 0..8 {
            let time = 100.0 + i as f32 * 0.5;
            t.observe(&[RIVER_FAR], true, time);
            assert!(!has_mid(&t.warnings(time, None)));
        }
    }

    #[test]
    fn top_laner_gets_jungle_but_not_mid() {
        let mut t = tracker();
        assert!(!has_mid(&walk(&mut t, Some(Lane::Top), 0.5, 8)));
        t.observe(&[MY_JUNGLE], true, 110.0);
        let w = t.warnings(110.0, Some(Lane::Top));
        assert_eq!(w.len(), 1);
        assert!(w[0].key.starts_with("gank-jungle"));
    }

    #[test]
    fn wording_follows_the_lane() {
        let mid = walk(&mut tracker(), Some(Lane::Middle), 0.5, 8);
        assert_eq!(mid.last().unwrap().say, "Gank coming mid. Back off.");
        let jg = walk(&mut tracker(), Some(Lane::Jungle), 0.5, 8);
        assert_eq!(jg.last().unwrap().say, "Enemy heading mid. Counter-gank.");
        let mut t = tracker();
        t.observe(&[MY_JUNGLE], true, 100.0);
        assert_eq!(t.warnings(100.0, Some(Lane::Jungle))[0].say, "Enemy invading your jungle.");
    }

    #[test]
    fn backwards_time_jump_clears_history() {
        let mut t = tracker();
        t.observe(&[(5900, 6100)], true, 300.0);
        t.observe(&[(5500, 5700)], true, 100.0);
        t.observe(&[(5300, 5500)], true, 100.5);
        assert!(!has_mid(&t.warnings(100.5, None)));
        assert_eq!(t.history.len(), 2);
    }
}
