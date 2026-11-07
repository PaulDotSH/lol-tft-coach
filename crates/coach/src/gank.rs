//! Early-game gank warnings from enemy blips read off the minimap, spoken as `Call`s, and where the enemy jungler
//! was last seen. Blips are named when their portrait was recognised; structures are excluded by zone, and a lane
//! warning needs an enemy off the lanes near yours (the laners themselves are not a gank).

use crate::consts;
use model::*;
use std::collections::{HashMap, VecDeque};

/// A blip in world coordinates seen from your side (your base at the origin), and who it is when recognised.
#[derive(Clone, Copy)]
struct Blip {
    x: i32,
    y: i32,
    champ: Option<ChampKey>,
}

/// The enemy team: champion names for the calls, and their jungler.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GankEnemies {
    pub names: HashMap<ChampKey, String>,
    pub jungler: Option<ChampKey>,
}

/// Which part of the map a position is on, seen from your side: mid lane's surroundings or either side of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Top,
    Mid,
    Bot,
}

impl Side {
    fn of(b: Blip) -> Side {
        if (b.x - b.y).abs() < consts::JUNGLER_MID_BAND {
            Side::Mid
        } else if b.x > b.y {
            Side::Bot
        } else {
            Side::Top
        }
    }

    fn of_lane(lane: Option<Lane>) -> Option<Side> {
        match lane? {
            Lane::Top => Some(Side::Top),
            Lane::Middle => Some(Side::Mid),
            Lane::Bottom | Lane::Support => Some(Side::Bot),
            Lane::Jungle => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Side::Top => "top side",
            Side::Mid => "mid",
            Side::Bot => "bot side",
        }
    }
}

/// Where the enemy jungler was announced: the side, its announcement number, when last seen there, and whether it
/// was spoken (a gank warning on the same sighting says it already).
#[derive(Clone, Copy)]
struct JunglerSeen {
    side: Side,
    n: u32,
    last: f32,
    spoken: bool,
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
    jungler: Option<JunglerSeen>,
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

/// The lane whose ganks you're warned of, its key, and the call for it (`{}` is who): your own lane; a jungler (or an
/// unknown role) watches mid.
fn watched(lane: Option<Lane>) -> (MapZone, &'static str, &'static str, &'static str) {
    match lane {
        Some(Lane::Top) => (MapZone::Top, "top", "{} near top lane — watch for a gank", "{} near top. Watch for a gank."),
        Some(Lane::Bottom | Lane::Support) => (MapZone::Bot, "bot", "{} near bot lane — watch for a gank", "{} near bot. Watch for a gank."),
        Some(Lane::Middle) => (MapZone::Mid, "mid", "{} coming to your lane — watch for a gank", "{} coming mid. Back off."),
        Some(Lane::Jungle) => (MapZone::Mid, "mid", "{} heading mid — counter-gank or ward", "{} heading mid. Counter-gank."),
        None => (MapZone::Mid, "mid", "{} heading mid — watch for a gank", "{} heading mid. Watch for a gank."),
    }
}

/// A call whose text and speech name `who`.
fn call(key: String, who: &str, text: &str, say: &str) -> Call {
    Call { key, text: text.replace("{}", who), say: say.replace("{}", who) }
}

impl GankTracker {
    /// Record the enemies visible on the minimap at `time`: world coordinates with the blue base at the origin, and
    /// the champion when recognised; `blue` is whether you are blue.
    pub fn observe(&mut self, enemies: &[(i32, i32, Option<ChampKey>)], blue: bool, time: f32) {
        if self.history.back().is_some_and(|(t, _)| time < *t) {
            self.history.clear();
            self.jungler = None;
        }
        while self.history.front().is_some_and(|(t, _)| time - *t > consts::GANK_MAX_DT) {
            self.history.pop_front();
        }
        let blips = enemies
            .iter()
            .map(|&(x, y, champ)| {
                let (x, y) = MapZone::from_blue(x, y, blue);
                Blip { x, y, champ }
            })
            .collect();
        self.history.push_back((time, blips));
    }

    /// Warnings for this tick (at most one per threat), stable-keyed so each is spoken once: ganks on your lane, an
    /// enemy in your jungle, and the side the enemy jungler was just seen on. Empty after the laning phase.
    pub fn warnings(&mut self, time: f32, lane: Option<Lane>, enemies: &GankEnemies) -> Vec<Call> {
        let mut out = Vec::new();
        if time > consts::GANK_EARLY_S {
            return out;
        }
        let Some((_, cur)) = self.history.back() else {
            return out;
        };
        let who = |b: &Blip| b.champ.and_then(|c| enemies.names.get(&c)).map_or("Enemy", String::as_str);
        if let Some(b) = cur.iter().find(|b| MapZone::at(b.x, b.y) == MapZone::BlueJungle) {
            let n = self.jungle.next(time);
            let (text, say) = if lane == Some(Lane::Jungle) {
                ("{} invading your jungle", "{} invading your jungle.")
            } else {
                ("{} in your jungle — watch for an invade or gank", "{} in your jungle.")
            };
            out.push(call(format!("gank-jungle-{n}"), who(b), text, say));
        }
        let (zone, key, text, say) = watched(lane);
        if let Some(b) = cur.iter().find(|b| self.threatens(zone, **b)) {
            let n = self.lane.next(time);
            out.push(call(format!("gank-{key}-{n}"), who(b), text, say));
        }
        let ganked = !out.is_empty();
        if let Some(j) = enemies.jungler
            && let Some(name) = enemies.names.get(&j)
            && let Some(c) = self.jungler_call(time, lane, j, name, ganked)
        {
            out.push(c);
        }
        out
    }

    /// The enemy jungler's side, announced when it changes or after the jungler was out of sight `JUNGLER_STALE_S`;
    /// kept on screen briefly after. Not spoken when a gank warning already fires on that sighting.
    fn jungler_call(&mut self, time: f32, lane: Option<Lane>, jungler: ChampKey, name: &str, ganked: bool) -> Option<Call> {
        let seen = self.history.back()?.1.iter().find(|b| b.champ == Some(jungler)).copied();
        if let Some(b) = seen {
            let side = Side::of(b);
            match &mut self.jungler {
                Some(j) if j.side == side && time - j.last <= consts::JUNGLER_STALE_S => j.last = time,
                j => {
                    let n = j.map_or(1, |j| j.n + 1);
                    *j = Some(JunglerSeen { side, n, last: time, spoken: !ganked });
                }
            }
        }
        let j = self.jungler.filter(|j| j.spoken && time - j.last <= consts::JUNGLER_SHOW_S)?;
        let (text, say) = if Side::of_lane(lane) == Some(j.side) {
            (format!("{name} {} — near your lane, play safe", j.side.name()), format!("{name} {}. Play safe.", j.side.name()))
        } else {
            (format!("{name} spotted {}", j.side.name()), format!("{name} {}.", j.side.name()))
        };
        Some(Call { key: format!("jungler-{}", j.n), text, say })
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
    /// The enemy jungle's bot side, far from every lane's gank range.
    const ENEMY_BOT_JUNGLE: (i32, i32) = (10500, 5800);
    const TOP_SIDE_JUNGLE: (i32, i32) = (5800, 10500);
    const JINX: ChampKey = 222;

    /// Anonymous blips and no roster, as before champions were recognised.
    impl GankTracker {
        fn obs(&mut self, pts: &[(i32, i32)], blue: bool, time: f32) {
            self.observe(&pts.iter().map(|&(x, y)| (x, y, None)).collect::<Vec<_>>(), blue, time);
        }

        fn w(&mut self, time: f32, lane: Option<Lane>) -> Vec<Call> {
            self.warnings(time, lane, &GankEnemies::default())
        }
    }

    fn tracker() -> GankTracker {
        GankTracker::default()
    }

    fn jinx_jungles() -> GankEnemies {
        GankEnemies { names: HashMap::from([(JINX, "Jinx".to_string()), (157, "Yasuo".to_string())]), jungler: Some(JINX) }
    }

    /// Jinx seen at `at` at `time`; the calls for a player in `lane`.
    fn jinx_at(t: &mut GankTracker, at: (i32, i32), time: f32, lane: Option<Lane>) -> Vec<Call> {
        t.observe(&[(at.0, at.1, Some(JINX))], true, time);
        t.warnings(time, lane, &jinx_jungles())
    }

    fn jungler_key(w: &[Call]) -> Option<String> {
        w.iter().find(|c| c.key.starts_with("jungler-")).map(|c| c.key.clone())
    }

    #[test]
    fn a_recognised_ganker_is_named() {
        let w = jinx_at(&mut tracker(), RIVER_NEAR_MID, 100.0, Some(Lane::Middle));
        let mid = w.iter().find(|c| c.key.starts_with("gank-mid")).unwrap();
        assert_eq!(mid.say, "Jinx coming mid. Back off.");
        assert!(jungler_key(&w).is_none(), "the gank warning already says where Jinx is: {w:?}");
    }

    #[test]
    fn the_jungler_side_is_called_once_per_side() {
        let mut t = tracker();
        let w = jinx_at(&mut t, ENEMY_BOT_JUNGLE, 100.0, Some(Lane::Middle));
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!((w[0].text.as_str(), w[0].say.as_str()), ("Jinx spotted bot side", "Jinx bot side."));
        let first = jungler_key(&w);
        assert_eq!(jungler_key(&jinx_at(&mut t, ENEMY_BOT_JUNGLE, 110.0, Some(Lane::Middle))), first);
        let top = jungler_key(&jinx_at(&mut t, TOP_SIDE_JUNGLE, 120.0, Some(Lane::Middle)));
        assert!(top.is_some() && top != first);
        assert!(jungler_key(&t.w(130.0, Some(Lane::Middle))).is_none(), "no longer shown once out of sight");
        let again = jungler_key(&jinx_at(&mut t, TOP_SIDE_JUNGLE, 170.0, Some(Lane::Middle)));
        assert!(again.is_some() && again != top, "re-announced after a while out of sight");
    }

    #[test]
    fn the_jungler_on_your_side_says_play_safe() {
        let w = jinx_at(&mut tracker(), ENEMY_BOT_JUNGLE, 100.0, Some(Lane::Support));
        assert_eq!(w.last().unwrap().say, "Jinx bot side. Play safe.");
    }

    #[test]
    fn unrecognised_blips_say_enemy() {
        let mut t = tracker();
        t.obs(&[RIVER_NEAR_MID], true, 100.0);
        assert_eq!(t.warnings(100.0, Some(Lane::Middle), &jinx_jungles())[0].say, "Enemy coming mid. Back off.");
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
            t.obs(&[at(d)], true, time);
            w = t.w(time, lane);
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
        t.obs(&[MY_JUNGLE], true, 100.0);
        let w = t.w(100.0, top);
        assert_eq!(w.len(), 1);
        let key = w[0].key.clone();
        t.obs(&[MY_JUNGLE], true, 102.0);
        assert_eq!(t.w(102.0, top)[0].key, key);
        t.obs(&[], true, 140.0);
        assert!(t.w(140.0, top).is_empty());
        t.obs(&[MY_JUNGLE], true, 160.0);
        assert_ne!(t.w(160.0, top)[0].key, key);
    }

    #[test]
    fn enemy_jungle_is_not_my_jungle() {
        let mut t = tracker();
        t.obs(&[ENEMY_JUNGLE], true, 100.0);
        assert!(t.w(100.0, Some(Lane::Top)).is_empty());
    }

    #[test]
    fn approaching_mid_warns_but_a_standing_blip_does_not() {
        let mut t = tracker();
        t.obs(&[RIVER_FAR], true, 100.0);
        let _ = t.w(100.0, None);
        t.obs(&[RIVER_NEAR_MID], true, 102.0);
        assert!(has_mid(&t.w(102.0, None)));

        let mut t = tracker();
        t.obs(&[RIVER_FAR], true, 100.0);
        let _ = t.w(100.0, None);
        t.obs(&[RIVER_FAR], true, 102.0);
        assert!(!has_mid(&t.w(102.0, None)));
    }

    /// A jungler seen once in the river beside mid, then lost in the fog, warns on that one read.
    #[test]
    fn one_sighting_in_the_river_beside_mid_warns() {
        let mut t = tracker();
        t.obs(&[RIVER_NEAR_MID], true, 100.0);
        assert!(has_mid(&t.w(100.0, Some(Lane::Middle))));
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
        t.obs(&[MY_JUNGLE], true, late);
        assert!(t.w(late, None).is_empty());
    }

    #[test]
    fn red_side_reads_from_its_own_corner() {
        let mut t = tracker();
        t.obs(&[ENEMY_JUNGLE], false, 100.0);
        assert_eq!(t.w(100.0, Some(Lane::Top)).len(), 1);
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
            t.obs(&[RIVER_FAR], true, time);
            assert!(!has_mid(&t.w(time, None)));
        }
    }

    #[test]
    fn top_laner_gets_jungle_but_not_mid() {
        let mut t = tracker();
        assert!(!has_mid(&walk(&mut t, Some(Lane::Top), 0.5, 8)));
        t.obs(&[MY_JUNGLE], true, 110.0);
        let w = t.w(110.0, Some(Lane::Top));
        assert_eq!(w.len(), 1);
        assert!(w[0].key.starts_with("gank-jungle"));
    }

    #[test]
    fn wording_follows_the_lane() {
        let mid = walk(&mut tracker(), Some(Lane::Middle), 0.5, 8);
        assert_eq!(mid.last().unwrap().say, "Enemy coming mid. Back off.");
        let jg = walk(&mut tracker(), Some(Lane::Jungle), 0.5, 8);
        assert_eq!(jg.last().unwrap().say, "Enemy heading mid. Counter-gank.");
        let mut t = tracker();
        t.obs(&[MY_JUNGLE], true, 100.0);
        assert_eq!(t.w(100.0, Some(Lane::Jungle))[0].say, "Enemy invading your jungle.");
    }

    #[test]
    fn backwards_time_jump_clears_history() {
        let mut t = tracker();
        t.obs(&[(5900, 6100)], true, 300.0);
        t.obs(&[(5500, 5700)], true, 100.0);
        t.obs(&[(5300, 5500)], true, 100.5);
        assert!(!has_mid(&t.w(100.5, None)));
        assert_eq!(t.history.len(), 2);
    }
}
