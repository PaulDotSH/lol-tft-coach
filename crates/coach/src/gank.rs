//! Early-game gank warnings from enemy blips read off the minimap, spoken as `Call`s, and where the enemy jungler
//! was last seen. Blips are named when their portrait was recognised; structures are excluded by zone, and a lane
//! warning needs an enemy off the lanes near yours (the laners themselves are not a gank).

use crate::consts;
use model::*;
use std::collections::{HashMap, VecDeque};

mod sights;
pub use sights::{SightLog, Sighting};

/// A blip in world coordinates seen from your side (your base at the origin), and who it is when recognised.
#[derive(Clone, Copy)]
struct Blip {
    x: i32,
    y: i32,
    champ: Option<ChampKey>,
}

/// The enemy team: champion names for the calls, their jungler, and the laners' lanes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GankEnemies {
    pub names: HashMap<ChampKey, String>,
    pub jungler: Option<ChampKey>,
    pub lanes: HashMap<ChampKey, Lane>,
}

/// One enemy laner's roaming: when the current stretch away from home began and was last seen, the announcement
/// count, whether this stretch was announced, and whether that was spoken (a gank warning on the sighting says it).
#[derive(Clone, Copy)]
struct Roam {
    since: f32,
    last: f32,
    n: u32,
    announced: bool,
    spoken: bool,
}

fn lane_name(lane: Lane) -> &'static str {
    match lane {
        Lane::Top => "top",
        Lane::Jungle => "the jungle",
        Lane::Middle => "mid",
        Lane::Bottom | Lane::Support => "bot",
    }
}

/// Whether a laner of `lane` at `b` is where they belong: their lane, their base, their lane's side of the map (the
/// river and jungle beside it: warding, a drake), or a short step off the lane (their own raptors, a tri-bush).
fn at_home(lane: Lane, b: Blip) -> bool {
    let home = match lane {
        Lane::Top => MapZone::Top,
        Lane::Middle => MapZone::Mid,
        Lane::Bottom | Lane::Support => MapZone::Bot,
        Lane::Jungle => return true,
    };
    let zone = MapZone::at(b.x, b.y);
    zone == home || zone == MapZone::RedBase || Side::of_lane(Some(lane)) == Some(Side::of(b)) || off_lane(home, b) < consts::ROAM_SLACK
}

/// Whether a laner of `lane` at `b` is on our half of their lane, past its middle toward our turret.
fn pushed(lane: Lane, b: Blip) -> bool {
    let (along, off) = match lane {
        Lane::Top => (b.y, b.x),
        Lane::Bottom | Lane::Support => (b.x, b.y),
        Lane::Middle => ((b.x + b.y) / 2, (b.x - b.y).abs()),
        Lane::Jungle => return false,
    };
    off < consts::PUSH_BAND && along > consts::PUSH_BASE && b.x + b.y < MAP_END - consts::PUSH_PAST
}

/// Where a roamer is: a lane by name, else the side of the map.
fn whereabouts(b: Blip) -> &'static str {
    match MapZone::at(b.x, b.y) {
        MapZone::Top => "top",
        MapZone::Mid => "mid",
        MapZone::Bot => "bot",
        _ => Side::of(b).name(),
    }
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

    fn lane(self) -> Lane {
        match self {
            Side::Top => Lane::Top,
            Side::Mid => Lane::Middle,
            Side::Bot => Lane::Bottom,
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

/// An enemy as the minimap last showed them: when, the side of the map, and whether pushed up their lane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seen {
    pub time: f32,
    pub side: Lane,
    pub pushed: bool,
}

/// What the lane board reads off the minimap at `time`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapView {
    pub time: f32,
    pub seen: HashMap<ChampKey, Seen>,
}

#[derive(Default)]
pub struct GankTracker {
    history: VecDeque<(f32, Vec<Blip>)>,
    jungle: Episode,
    lane: Episode,
    jungler: Option<JunglerSeen>,
    roams: HashMap<ChampKey, Roam>,
    /// Where and when each champion was last seen named, to reject a name that would have teleported.
    named: HashMap<ChampKey, (f32, Blip)>,
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
            self.roams.clear();
            self.named.clear();
        }
        while self.history.front().is_some_and(|(t, _)| time - *t > consts::GANK_MAX_DT) {
            self.history.pop_front();
        }
        let blips = enemies
            .iter()
            .map(|&(x, y, champ)| {
                let (x, y) = MapZone::from_blue(x, y, blue);
                let mut b = Blip { x, y, champ };
                if let Some(c) = champ {
                    match self.named.get(&c) {
                        Some(&(t, p)) if !self.reachable(p, b, time - t) => b.champ = None,
                        _ => {
                            self.named.insert(c, (time, b));
                        }
                    }
                }
                b
            })
            .collect();
        self.history.push_back((time, blips));
    }

    pub fn view(&self, time: f32, enemies: &GankEnemies) -> MapView {
        let seen = self
            .named
            .iter()
            .map(|(&c, &(t, b))| (c, Seen { time: t, side: Side::of(b).lane(), pushed: enemies.lanes.get(&c).is_some_and(|&l| pushed(l, b)) }))
            .collect();
        MapView { time, seen }
    }

    pub fn last_names(&self) -> Vec<Option<ChampKey>> {
        self.history.back().map_or(vec![], |(_, b)| b.iter().map(|b| b.champ).collect())
    }

    /// Whether a champion seen at `from` could be at `to` `dt` seconds later: within walking (and Flash) range, home
    /// in their base after a recall or death, or out of sight long enough for anything.
    fn reachable(&self, from: Blip, to: Blip, dt: f32) -> bool {
        dt > consts::NAME_TRUST_S
            || MapZone::at(to.x, to.y) == MapZone::RedBase
            || (dist2(from, to) as f32).sqrt() <= consts::NAME_JUMP + consts::NAME_SPEED * dt
    }

    /// Warnings for this tick (at most one per threat), stable-keyed so each is spoken once: ganks on your lane, an
    /// enemy in your jungle, the side the enemy jungler was just seen on, and laners roaming from their lane (your
    /// lane opponent anywhere, the others near your lane). After the laning phase nobody roams and only the solo
    /// laners, still farming a lane, hear of ganks on it.
    pub fn warnings(&mut self, time: f32, lane: Option<Lane>, enemies: &GankEnemies) -> Vec<Call> {
        let mut out = Vec::new();
        let laning = time <= consts::GANK_EARLY_S;
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
        let opponent = |b: &Blip| b.champ.and_then(|c| enemies.lanes.get(&c)).is_some_and(|&l| lane.is_some() && Side::of_lane(Some(l)) == Side::of_lane(lane));
        let in_lane = laning || matches!(lane, Some(Lane::Top | Lane::Middle | Lane::Bottom));
        if in_lane && let Some(b) = cur.iter().find(|b| !opponent(b) && self.threatens(zone, **b)) {
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
        if laning {
            out.extend(self.roam_calls(time, lane, enemies, ganked));
        }
        out
    }

    /// Enemy laners seen away from home on reads `ROAM_DWELL_S` apart, announced once per stretch away (a new one after
    /// `ROAM_GAP_S` back home or unseen) and kept on screen briefly. Called for your lane opponent wherever they go,
    /// for any laner on your side of the map, and for every laner when you jungle.
    fn roam_calls(&mut self, time: f32, lane: Option<Lane>, enemies: &GankEnemies, ganked: bool) -> Vec<Call> {
        let Some((_, cur)) = self.history.back() else {
            return vec![];
        };
        let mine = Side::of_lane(lane);
        let mut out = vec![];
        for b in cur.iter().filter(|b| b.champ.is_some_and(|c| Some(c) != enemies.jungler)) {
            let c = b.champ.unwrap_or_default();
            let (Some(&home), Some(name)) = (enemies.lanes.get(&c), enemies.names.get(&c)) else { continue };
            if at_home(home, *b) {
                continue;
            }
            let opponent = lane.is_some_and(|l| Side::of_lane(Some(l)) == Side::of_lane(Some(home)));
            if !(mine.is_none() || opponent || mine == Some(Side::of(*b))) {
                continue;
            }
            let r = self.roams.entry(c).or_insert(Roam { since: time, last: time, n: 0, announced: false, spoken: false });
            if time - r.last > consts::ROAM_GAP_S {
                (r.since, r.announced) = (time, false);
            }
            r.last = time;
            if !r.announced && time - r.since >= consts::ROAM_DWELL_S {
                (r.n, r.announced, r.spoken) = (r.n + 1, true, !ganked);
            }
            if !(r.announced && r.spoken) {
                continue;
            }
            let at = whereabouts(*b);
            let (text, say) = if opponent {
                (format!("{name} left {} — roaming {at}", lane_name(home)), format!("{name} left {}, roaming {at}.", lane_name(home)))
            } else if mine == Some(Side::of(*b)) {
                (format!("{name} roaming {at} — near your lane"), format!("{name} roaming {at}. Careful."))
            } else {
                (format!("{name} roaming {at}"), format!("{name} roaming {at}."))
            };
            out.push(Call { key: format!("gank-roam-{c}-{}", r.n), text, say });
        }
        out
    }

    /// The enemy jungler's side when they appear: first seen, or seen again after `JUNGLER_STALE_S` out of sight
    /// (moving while in view is on the minimap already); kept on screen briefly. Not spoken when a gank warning
    /// already fires on that sighting.
    fn jungler_call(&mut self, time: f32, lane: Option<Lane>, jungler: ChampKey, name: &str, ganked: bool) -> Option<Call> {
        let seen = self.history.back()?.1.iter().find(|b| b.champ == Some(jungler)).copied();
        if let Some(b) = seen {
            match &mut self.jungler {
                Some(j) if time - j.last <= consts::JUNGLER_STALE_S => j.last = time,
                j => {
                    let n = j.map_or(1, |j| j.n + 1);
                    *j = Some(JunglerSeen { side: Side::of(b), n, last: time, spoken: !ganked });
                }
            }
        }
        let j = self.jungler.filter(|j| j.spoken && time - j.last <= consts::JUNGLER_SHOW_S)?;
        let (text, say) = if lane == Some(Lane::Jungle) {
            (format!("{name} seen {}", j.side.name()), format!("{name} seen {}.", j.side.name()))
        } else if Side::of_lane(lane) == Some(j.side) {
            (format!("{name} {} — near your lane, play safe", j.side.name()), format!("{name} {}. Play safe.", j.side.name()))
        } else {
            (format!("{name} spotted {}", j.side.name()), format!("{name} {}.", j.side.name()))
        };
        Some(Call { key: format!("gank-jg-{}", j.n), text, say })
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
        GankEnemies { names: HashMap::from([(JINX, "Jinx".to_string()), (157, "Yasuo".to_string())]), jungler: Some(JINX), lanes: HashMap::new() }
    }

    /// Jinx seen at `at` at `time`; the calls for a player in `lane`.
    fn jinx_at(t: &mut GankTracker, at: (i32, i32), time: f32, lane: Option<Lane>) -> Vec<Call> {
        t.observe(&[(at.0, at.1, Some(JINX))], true, time);
        t.warnings(time, lane, &jinx_jungles())
    }

    fn jungler_key(w: &[Call]) -> Option<String> {
        w.iter().find(|c| c.key.starts_with("gank-jg-")).map(|c| c.key.clone())
    }

    const SYLAS: ChampKey = 517;
    const NAMI: ChampKey = 267;
    /// Top lane on the enemy's half, where Sylas roamed at 10:37 in a real game.
    const TOP_LANE: (i32, i32) = (6364, 13616);

    fn roster() -> GankEnemies {
        GankEnemies {
            names: HashMap::from([(SYLAS, "Sylas".to_string()), (NAMI, "Nami".to_string()), (JINX, "Jinx".to_string())]),
            jungler: Some(JINX),
            lanes: HashMap::from([(SYLAS, Lane::Middle), (NAMI, Lane::Support), (JINX, Lane::Jungle)]),
        }
    }

    /// `who` seen at each (time, at); the roam calls heard by a player in `lane`.
    fn seen(who: ChampKey, reads: &[(f32, (i32, i32))], lane: Option<Lane>) -> Vec<Call> {
        let mut t = tracker();
        let mut heard = vec![];
        for &(time, at) in reads {
            t.observe(&[(at.0, at.1, Some(who))], true, time);
            for c in t.warnings(time, lane, &roster()) {
                if c.key.starts_with("gank-roam-") && !heard.iter().any(|h: &Call| h.key == c.key) {
                    heard.push(c);
                }
            }
        }
        heard
    }

    #[test]
    fn your_lane_opponent_roaming_is_called() {
        let w = seen(SYLAS, &[(600.0, TOP_LANE), (601.0, TOP_LANE)], Some(Lane::Middle));
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(w[0].say, "Sylas left mid, roaming top.");
    }

    #[test]
    fn one_sighting_away_is_not_a_roam() {
        assert!(seen(SYLAS, &[(600.0, TOP_LANE)], Some(Lane::Middle)).is_empty());
    }

    #[test]
    fn a_laner_at_home_is_not_roaming() {
        let mid = [(600.0, (7600, 7700)), (601.0, (7700, 7800))];
        let raptors = [(600.0, (8300, 9600)), (601.0, (8300, 9600))];
        let base = [(600.0, (12500, 13000)), (601.0, (12500, 13000))];
        for reads in [&mid[..], &raptors[..], &base[..]] {
            assert!(seen(SYLAS, reads, Some(Lane::Middle)).is_empty(), "{reads:?}");
        }
    }

    fn b(x: i32, y: i32) -> Blip {
        Blip { x, y, champ: None }
    }

    #[test]
    fn a_laner_past_the_middle_toward_our_turret_is_pushed() {
        assert!(pushed(Lane::Top, b(1000, 10400)), "at our top outer turret");
        assert!(pushed(Lane::Middle, b(5900, 6400)), "at our mid outer turret");
        assert!(pushed(Lane::Bottom, b(10500, 1000)), "at our bot outer turret");
        assert!(pushed(Lane::Support, b(10500, 1000)));
    }

    #[test]
    fn a_laner_at_the_middle_or_on_their_half_is_not_pushed() {
        assert!(!pushed(Lane::Top, b(1200, 13600)), "top corner");
        assert!(!pushed(Lane::Top, b(TOP_LANE.0, TOP_LANE.1)), "their half of top");
        assert!(!pushed(Lane::Middle, b(7400, 7400)), "mid centre");
        assert!(!pushed(Lane::Bottom, b(13600, 1200)), "bot corner");
        assert!(!pushed(Lane::Jungle, b(1000, 10400)));
    }

    #[test]
    fn a_laner_off_their_lane_or_in_our_fountain_is_not_pushed() {
        assert!(!pushed(Lane::Top, b(3500, 7000)), "in our jungle");
        assert!(!pushed(Lane::Top, b(600, 600)), "fountain");
        assert!(!pushed(Lane::Middle, b(5900, 6400 + 2000)));
        assert!(!pushed(Lane::Bottom, b(RIVER_FAR.0, RIVER_FAR.1)));
    }

    #[test]
    fn the_view_says_who_is_pushed_and_where_the_jungler_was() {
        let mut t = tracker();
        t.observe(&[(1000, 10400, Some(SYLAS)), (ENEMY_BOT_JUNGLE.0, ENEMY_BOT_JUNGLE.1, Some(JINX))], true, 600.0);
        t.observe(&[(7400, 7400, Some(NAMI))], true, 605.0);
        let v = t.view(606.0, &GankEnemies { lanes: HashMap::from([(SYLAS, Lane::Top), (NAMI, Lane::Middle), (JINX, Lane::Jungle)]), ..roster() });
        assert_eq!(v.seen[&SYLAS], Seen { time: 600.0, side: Lane::Top, pushed: true });
        assert_eq!(v.seen[&NAMI], Seen { time: 605.0, side: Lane::Middle, pushed: false });
        assert_eq!(v.seen[&JINX].side, Lane::Bottom);
    }

    #[test]
    fn the_jungler_hears_where_their_jungler_was_seen() {
        let w = jinx_at(&mut tracker(), TOP_SIDE_JUNGLE, 100.0, Some(Lane::Jungle));
        let c = w.iter().find(|c| c.key.starts_with("gank-jg-")).unwrap();
        assert_eq!((c.text.as_str(), c.say.as_str()), ("Jinx seen top side", "Jinx seen top side."));
    }

    #[test]
    fn a_roamer_near_your_lane_is_called() {
        let w = seen(NAMI, &[(600.0, TOP_LANE), (601.0, TOP_LANE)], Some(Lane::Top));
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(w[0].say, "Nami roaming top. Careful.");
    }

    #[test]
    fn a_roam_far_from_you_is_not_called_unless_you_jungle() {
        assert!(seen(SYLAS, &[(600.0, TOP_LANE), (601.0, TOP_LANE)], Some(Lane::Bottom)).is_empty());
        assert_eq!(seen(SYLAS, &[(600.0, TOP_LANE), (601.0, TOP_LANE)], Some(Lane::Jungle)).len(), 1);
    }

    #[test]
    fn a_new_roam_after_a_while_home_is_called_again() {
        let reads = [(600.0, TOP_LANE), (601.0, TOP_LANE), (605.0, TOP_LANE), (640.0, TOP_LANE), (641.0, TOP_LANE)];
        assert_eq!(seen(SYLAS, &reads, Some(Lane::Middle)).len(), 2);
    }

    #[test]
    fn a_recognised_ganker_is_named() {
        let w = jinx_at(&mut tracker(), RIVER_NEAR_MID, 100.0, Some(Lane::Middle));
        let mid = w.iter().find(|c| c.key.starts_with("gank-mid")).unwrap();
        assert_eq!(mid.say, "Jinx coming mid. Back off.");
        assert!(jungler_key(&w).is_none(), "the gank warning already says where Jinx is: {w:?}");
    }

    #[test]
    fn the_jungler_is_called_when_they_appear() {
        let mut t = tracker();
        let w = jinx_at(&mut t, ENEMY_BOT_JUNGLE, 100.0, Some(Lane::Middle));
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!((w[0].text.as_str(), w[0].say.as_str()), ("Jinx spotted bot side", "Jinx bot side."));
        let first = jungler_key(&w);
        assert_eq!(jungler_key(&jinx_at(&mut t, ENEMY_BOT_JUNGLE, 110.0, Some(Lane::Middle))), first);
        assert_eq!(jungler_key(&jinx_at(&mut t, (8000, 9000), 117.0, Some(Lane::Middle))), first, "moving in view is not a new call");
        assert!(jungler_key(&t.w(130.0, Some(Lane::Middle))).is_none(), "no longer shown once out of sight");
        let again = jungler_key(&jinx_at(&mut t, TOP_SIDE_JUNGLE, 170.0, Some(Lane::Middle)));
        assert!(again.is_some() && again != first, "called again on reappearing after a while out of sight");
    }

    /// Real game: the enemy top laner's ring was read as Lillia for one frame, 13 000 units from where Lillia had been
    /// seen 3 s before.
    #[test]
    fn a_name_that_would_teleport_is_dropped() {
        let mut t = tracker();
        let _ = jinx_at(&mut t, (6660, 13616), 538.0, Some(Lane::Middle));
        t.observe(&[(12876, 2368, Some(JINX))], true, 541.0);
        assert_eq!(t.history.back().unwrap().1[0].champ, None);
        t.observe(&[(6512, 13468, Some(JINX))], true, 544.0);
        assert_eq!(t.history.back().unwrap().1[0].champ, Some(JINX));
    }

    #[test]
    fn your_lane_opponent_walking_back_is_not_a_gank() {
        let mut t = tracker();
        t.observe(&[(9400, 5400, Some(SYLAS))], true, 100.0);
        let _ = t.warnings(100.0, Some(Lane::Middle), &roster());
        t.observe(&[(8800, 6000, Some(SYLAS))], true, 102.0);
        assert!(!has_mid(&t.warnings(102.0, Some(Lane::Middle), &roster())));
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
        walk_at(t, lane, step, reads, 100.0, at)
    }

    /// `walk_from` starting at game time `start`.
    fn walk_at(t: &mut GankTracker, lane: Option<Lane>, step: f32, reads: u32, start: f32, at: impl Fn(i32) -> (i32, i32)) -> Vec<Call> {
        let mut w = Vec::new();
        for i in 0..reads {
            let time = start + i as f32 * step;
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
    fn your_jungle_and_the_jungler_are_called_all_game() {
        let late = consts::GANK_EARLY_S + 600.0;
        let mut t = tracker();
        t.obs(&[MY_JUNGLE], true, late);
        assert!(t.w(late, Some(Lane::Support)).iter().any(|c| c.key.starts_with("gank-jungle-")));
        assert!(jungler_key(&jinx_at(&mut tracker(), ENEMY_BOT_JUNGLE, late, Some(Lane::Support))).is_some());
    }

    #[test]
    fn after_laning_only_solo_laners_hear_of_ganks_on_their_lane() {
        let late = consts::GANK_EARLY_S + 600.0;
        let river = |d| (RIVER_FAR.0 - d, RIVER_FAR.1 + d);
        assert!(has_mid(&walk_at(&mut tracker(), Some(Lane::Middle), 0.5, 8, late, river)));
        assert!(!has_mid(&walk_at(&mut tracker(), Some(Lane::Jungle), 0.5, 8, late, river)));
    }

    #[test]
    fn nobody_roams_after_laning() {
        let late = consts::GANK_EARLY_S + 600.0;
        assert!(seen(SYLAS, &[(late, TOP_LANE), (late + 1.0, TOP_LANE)], Some(Lane::Middle)).is_empty());
    }

    #[test]
    fn a_laner_on_their_own_side_is_not_roaming() {
        let dragon_ward = [(475.0, (10486, 5670)), (476.0, (10530, 5582))];
        assert!(seen(NAMI, &dragon_ward, Some(Lane::Support)).is_empty());
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
