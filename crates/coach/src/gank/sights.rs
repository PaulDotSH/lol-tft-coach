use crate::consts;
use model::*;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sighting {
    pub time: f32,
    pub x: i32,
    pub y: i32,
    pub champ: Option<ChampKey>,
}

#[derive(Debug, Clone, Default)]
pub struct SightLog {
    reads: VecDeque<(f32, Vec<Sighting>)>,
    calls: Vec<(f32, Call)>,
}

impl SightLog {
    pub fn push(&mut self, time: f32, blips: &[(i32, i32, Option<ChampKey>)], blue: bool, calls: &[Call]) {
        if self.reads.back().is_some_and(|(t, _)| time < *t) {
            *self = SightLog::default();
        }
        while self.reads.front().is_some_and(|(t, _)| time - *t > consts::SIGHT_KEEP_S) {
            self.reads.pop_front();
        }
        let seen = blips
            .iter()
            .map(|&(x, y, champ)| {
                let (x, y) = MapZone::from_blue(x, y, blue);
                Sighting { time, x, y, champ }
            })
            .collect();
        self.reads.push_back((time, seen));
        for c in calls {
            if !self.calls.iter().any(|(_, k)| k.key == c.key) {
                self.calls.push((time, c.clone()));
            }
        }
    }

    pub fn window(&self, from: f32, to: f32) -> impl Iterator<Item = &Sighting> {
        self.reads.iter().filter(move |(t, _)| (from..=to).contains(t)).flat_map(|(_, s)| s)
    }

    pub fn covered(&self, from: f32, to: f32) -> f32 {
        if to <= from {
            return 0.0;
        }
        let (mut sum, mut end) = (0.0, from);
        for &(t, _) in &self.reads {
            let (a, b) = (t.max(end), (t + consts::SIGHT_READ_S).min(to));
            if b > a {
                sum += b - a;
                end = b;
            }
        }
        sum / (to - from)
    }

    pub fn calls(&self, from: f32, to: f32) -> impl Iterator<Item = &(f32, Call)> {
        self.calls.iter().filter(move |(t, _)| (from..=to).contains(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(key: &str) -> Call {
        Call { key: key.into(), text: key.into(), say: key.into() }
    }

    fn log(times: &[f32]) -> SightLog {
        let mut l = SightLog::default();
        for &t in times {
            l.push(t, &[(1000, 2000, Some(64))], true, &[]);
        }
        l
    }

    #[test]
    fn window_bounds_are_inclusive() {
        let l = log(&[10.0, 11.0, 12.0, 13.0]);
        let t: Vec<f32> = l.window(11.0, 12.0).map(|s| s.time).collect();
        assert_eq!(t, [11.0, 12.0]);
    }

    #[test]
    fn old_reads_are_pruned() {
        let l = log(&[10.0, 30.0, 45.0]);
        assert_eq!(l.window(0.0, 100.0).count(), 2);
    }

    #[test]
    fn coverage_counts_gaps() {
        let l = log(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
        assert!((l.covered(0.0, 10.0) - 1.0).abs() < 1e-4);
        let l = log(&[0.0, 1.0, 2.0, 3.0, 4.0]);
        assert!((l.covered(0.0, 10.0) - 0.55).abs() < 1e-4);
        assert_eq!(log(&[]).covered(0.0, 10.0), 0.0);
        assert_eq!(log(&[20.0]).covered(0.0, 10.0), 0.0);
    }

    #[test]
    fn time_going_back_resets() {
        let mut l = log(&[100.0, 101.0]);
        l.push(101.0, &[], true, &[call("gank-jg-1")]);
        l.push(5.0, &[(1, 2, None)], true, &[]);
        assert_eq!(l.window(0.0, 200.0).count(), 1);
        assert_eq!(l.calls(0.0, 200.0).count(), 0);
    }

    #[test]
    fn red_side_is_turned_to_blue_and_calls_keep_their_first_time() {
        let mut l = SightLog::default();
        l.push(10.0, &[(MAP_END - 2000, MAP_END - 1000, None)], false, &[call("gank-bot-1")]);
        l.push(11.0, &[], false, &[call("gank-bot-1"), call("gank-jg-1")]);
        let s = l.window(0.0, 20.0).next().unwrap();
        assert_eq!((s.x, s.y), (1000, 2000));
        let c: Vec<(f32, &str)> = l.calls(0.0, 20.0).map(|(t, c)| (*t, c.key.as_str())).collect();
        assert_eq!(c, [(10.0, "gank-bot-1"), (11.0, "gank-jg-1")]);
    }
}
