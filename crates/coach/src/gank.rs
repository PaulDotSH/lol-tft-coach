//! Early-game gank warnings from enemy blips read off the minimap, spoken as `Call`s.
//! Blips are anonymous, so structures are excluded by zone and a mid warning needs movement toward your base.

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
    mid: Episode,
}

fn dist2(a: Blip, b: Blip) -> i32 {
    (a.x - b.x).pow(2) + (a.y - b.y).pow(2)
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
        let wording = match lane {
            Some(Lane::Top | Lane::Bottom) => None,
            Some(Lane::Middle) => Some(("Enemy coming to your lane through the river — watch for a gank", "Gank coming mid. Back off.")),
            Some(Lane::Jungle) => Some(("Enemy heading mid through the river — counter-gank or ward", "Enemy heading mid. Counter-gank.")),
            Some(Lane::Support) | None => Some(("Enemy moving through the river toward mid — watch for a gank", "Enemy heading mid. Watch for a gank.")),
        };
        if let Some((text, say)) = wording
            && cur.iter().any(|b| self.heading_mid(*b))
        {
            let n = self.mid.next(time);
            out.push(Call { key: format!("gank-mid-{n}"), text: text.into(), say: say.into() });
        }
        out
    }

    /// A blip in your half of the river or mid that moved toward your base since the newest read at least `GANK_MOVE_DT` old: a jungler collapsing on mid, not a laner holding lane.
    fn heading_mid(&self, b: Blip) -> bool {
        if !matches!(MapZone::at(b.x, b.y), MapZone::Mid | MapZone::River) || b.x + b.y >= MAP_END {
            return false;
        }
        let Some((now, _)) = self.history.back() else {
            return false;
        };
        let Some((_, old)) = self.history.iter().rev().skip(1).find(|(t, _)| now - t >= consts::GANK_MOVE_DT) else {
            return false;
        };
        let Some(p) = old.iter().copied().min_by_key(|&p| dist2(p, b)) else {
            return false;
        };
        dist2(p, b) <= consts::GANK_MAX_STEP.pow(2) && (p.x + p.y) - (b.x + b.y) >= consts::GANK_APPROACH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MY_JUNGLE: (i32, i32) = (3500, 7000);
    const MY_RIVER_MID: (i32, i32) = (5600, 6000);
    const ENEMY_JUNGLE: (i32, i32) = (11300, 7800);

    fn tracker() -> GankTracker {
        GankTracker::default()
    }

    fn has_mid(w: &[Call]) -> bool {
        w.iter().any(|c| c.key.starts_with("gank-mid"))
    }

    fn walk(t: &mut GankTracker, lane: Option<Lane>, step: f32, reads: u32) -> Vec<Call> {
        let mut w = Vec::new();
        for i in 0..reads {
            let time = 100.0 + i as f32 * step;
            let d = (i as f32 * step * 200.0) as i32;
            t.observe(&[(5900 - d, 6100 - d)], true, time);
            w = t.warnings(time, lane);
            if has_mid(&w) {
                break;
            }
        }
        w
    }

    #[test]
    fn enemy_in_my_jungle_warns_once_then_re_warns_after_a_gap() {
        let mut t = tracker();
        t.observe(&[MY_JUNGLE], true, 100.0);
        let w = t.warnings(100.0, None);
        assert_eq!(w.len(), 1);
        let key = w[0].key.clone();
        t.observe(&[MY_JUNGLE], true, 102.0);
        assert_eq!(t.warnings(102.0, None)[0].key, key);
        t.observe(&[], true, 140.0);
        assert!(t.warnings(140.0, None).is_empty());
        t.observe(&[MY_JUNGLE], true, 160.0);
        assert_ne!(t.warnings(160.0, None)[0].key, key);
    }

    #[test]
    fn enemy_jungle_is_not_my_jungle() {
        let mut t = tracker();
        t.observe(&[ENEMY_JUNGLE], true, 100.0);
        assert!(t.warnings(100.0, None).is_empty());
    }

    #[test]
    fn approaching_mid_warns_but_a_standing_blip_does_not() {
        let mut t = tracker();
        t.observe(&[(6800, 7200)], true, 100.0);
        let _ = t.warnings(100.0, None);
        t.observe(&[MY_RIVER_MID], true, 102.0);
        assert!(has_mid(&t.warnings(102.0, None)));

        let mut t = tracker();
        t.observe(&[MY_RIVER_MID], true, 100.0);
        let _ = t.warnings(100.0, None);
        t.observe(&[MY_RIVER_MID], true, 102.0);
        assert!(!has_mid(&t.warnings(102.0, None)));
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
        assert_eq!(t.warnings(100.0, None).len(), 1);
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
            t.observe(&[MY_RIVER_MID], true, time);
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
