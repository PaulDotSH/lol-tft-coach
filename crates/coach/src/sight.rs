use crate::consts;
use crate::gank::SightLog;
use model::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SightVerdict {
    pub kind: SightKind,
    pub champ: Option<ChampKey>,
    pub before: f32,
}

pub fn lane_opponents(mine: Option<Lane>, enemies: impl IntoIterator<Item = (ChampKey, Option<Lane>)>) -> Vec<ChampKey> {
    let bot = |l: Option<Lane>| matches!(l, Some(Lane::Bottom | Lane::Support));
    enemies.into_iter().filter(|&(_, l)| mine.is_some() && (l == mine || (bot(mine) && bot(l)))).map(|(c, _)| c).collect()
}

pub fn stamp(log: &SightLog, time: f32, involved: &[ChampKey]) -> Option<DeathSight> {
    let from = time - consts::SIGHT_WINDOW_S;
    let covered = log.covered(from, time);
    if covered < consts::SIGHT_MIN_COVER {
        return None;
    }
    let mut seen: Vec<(SeenBefore, f32)> = Vec::new();
    for s in log.window(from, time) {
        let same = |e: &(SeenBefore, f32)| match s.champ {
            Some(_) => e.0.champ == s.champ,
            None => {
                e.0.champ.is_none() && (((e.0.x - s.x) as f32).hypot((e.0.y - s.y) as f32)) <= consts::NAME_JUMP + consts::NAME_SPEED * (s.time - e.1)
            }
        };
        let at = (time - s.time).max(0.0);
        match seen.iter_mut().find(|e| same(e)) {
            Some(e) => *e = (SeenBefore { last: at, x: s.x, y: s.y, ..e.0.clone() }, s.time),
            None => seen.push((SeenBefore { champ: s.champ, first: at, last: at, x: s.x, y: s.y }, s.time)),
        }
    }
    let mut seen: Vec<SeenBefore> = seen.into_iter().map(|e| e.0).collect();
    let rank = |e: &SeenBefore| e.champ.and_then(|c| involved.iter().position(|&i| i == c)).unwrap_or(involved.len());
    seen.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.last.total_cmp(&b.last)));
    let warned = log
        .calls(from, time)
        .map(|(t, c)| (time - t, c))
        .min_by(|a, b| (a.0 < consts::WARN_EARLY_S).cmp(&(b.0 < consts::WARN_EARLY_S)).then(a.0.total_cmp(&b.0)))
        .map(|(b, c)| (b, c.text.clone()));
    Some(DeathSight { covered, seen, warned, verdict: None, line: None })
}

pub fn verdict(note: &DeathNote, lane_opponents: &[ChampKey]) -> Option<SightVerdict> {
    let killer = note.killer?;
    let s = note.sight.as_ref()?;
    let mut gankers: Vec<ChampKey> = Vec::new();
    for c in std::iter::once(killer).chain(note.assisters.iter().copied()) {
        if !lane_opponents.contains(&c) && !gankers.contains(&c) {
            gankers.push(c);
        }
    }
    let v = |kind, champ, before| Some(SightVerdict { kind, champ, before });
    if gankers.is_empty() {
        return v(SightKind::Lane, Some(killer), 0.0);
    }
    if let Some((before, _)) = s.warned.as_ref().filter(|w| w.0 >= consts::WARN_EARLY_S) {
        return v(SightKind::Warned, None, *before);
    }
    let near = |e: &SeenBefore| note.pos.is_none_or(|(x, y)| ((e.x - x) as f32).hypot((e.y - y) as f32) <= consts::SEEN_NEAR);
    let seen = |c: ChampKey| s.seen.iter().find(|e| e.champ == Some(c));
    if let Some((c, e)) = gankers.iter().find_map(|&c| seen(c).filter(|e| e.first >= consts::SEEN_EARLY_S && near(e)).map(|e| (c, e))) {
        return v(SightKind::Visible, Some(c), e.first);
    }
    if gankers.iter().all(|&c| seen(c).is_none()) && !s.seen.iter().any(|e| e.champ.is_none() && near(e)) {
        return v(SightKind::Fog, Some(gankers[0]), 0.0);
    }
    s.seen
        .iter()
        .filter(|e| near(e) && e.champ.is_none_or(|c| gankers.contains(&c)))
        .max_by(|a, b| a.first.total_cmp(&b.first))
        .and_then(|e| v(SightKind::Unclear, e.champ, e.first))
}

pub fn line(v: &SightVerdict, note: &DeathNote, sd: &StaticData) -> Option<String> {
    let name = |c: Option<ChampKey>| c.map(|c| sd.champ_name(c).to_string());
    let secs = v.before.round() as i32;
    match v.kind {
        SightKind::Lane => None,
        SightKind::Warned => Some(format!("Warned {secs} s before: {}", note.sight.as_ref()?.warned.as_ref()?.1)),
        SightKind::Visible => Some(format!("{} was on the minimap for {secs} s before", name(v.champ)?)),
        SightKind::Fog => {
            let who = name(v.champ)?;
            Some(match &note.place {
                Some(p) => format!("{who} came out of fog in {}", crate::selfcoach::the(p)),
                None => format!("{who} came out of fog"),
            })
        }
        SightKind::Unclear => Some(format!("{} showed {secs} s before", name(v.champ).unwrap_or_else(|| "An enemy".into()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEE: ChampKey = 64;
    const ZED: ChampKey = 238;
    const LUX: ChampKey = 99;

    fn seen(champ: Option<ChampKey>, first: f32, at: (i32, i32)) -> SeenBefore {
        SeenBefore { champ, first, last: (first - 1.0).max(0.0), x: at.0, y: at.1 }
    }

    fn note(killer: Option<ChampKey>, assisters: &[ChampKey], seen: Vec<SeenBefore>, warned: Option<f32>) -> DeathNote {
        DeathNote {
            time: 400.0,
            killer,
            assisters: assisters.to_vec(),
            gold_gap: None,
            killer_top_threat: false,
            unspent: 0,
            buy_next: None,
            could_buy: false,
            owned: vec![],
            spikes: vec![],
            traded: false,
            place: None,
            pos: None,
            sight: Some(DeathSight { covered: 1.0, seen, warned: warned.map(|w| (w, "Lee Sin near mid".into())), verdict: None, line: None }),
        }
    }

    fn kind(n: &DeathNote) -> Option<SightKind> {
        verdict(n, &[ZED]).map(|v| v.kind)
    }

    #[test]
    fn a_call_before_the_death_is_warned() {
        let n = note(Some(LEE), &[ZED], vec![seen(Some(LEE), 7.0, (8000, 6000))], Some(6.0));
        assert_eq!(verdict(&n, &[ZED]), Some(SightVerdict { kind: SightKind::Warned, champ: None, before: 6.0 }));
    }

    #[test]
    fn a_call_too_late_is_not_a_warning() {
        let n = note(Some(LEE), &[], vec![seen(Some(LEE), 7.0, (8000, 6000))], Some(1.0));
        assert_eq!(verdict(&n, &[ZED]), Some(SightVerdict { kind: SightKind::Visible, champ: Some(LEE), before: 7.0 }));
    }

    #[test]
    fn an_assister_seen_early_is_visible() {
        let n = note(Some(ZED), &[LEE], vec![seen(Some(ZED), 15.0, (7000, 7000)), seen(Some(LEE), 5.0, (8000, 6000))], None);
        assert_eq!(verdict(&n, &[ZED]), Some(SightVerdict { kind: SightKind::Visible, champ: Some(LEE), before: 5.0 }));
    }

    #[test]
    fn the_lane_opponent_alone_is_a_lane_death() {
        let n = note(Some(ZED), &[], vec![seen(Some(ZED), 15.0, (7000, 7000))], Some(5.0));
        assert_eq!(kind(&n), Some(SightKind::Lane));
    }

    #[test]
    fn nobody_seen_is_fog() {
        let n = note(Some(ZED), &[LEE], vec![seen(Some(ZED), 15.0, (7000, 7000)), seen(Some(LUX), 9.0, (2000, 12000))], None);
        assert_eq!(verdict(&n, &[ZED]), Some(SightVerdict { kind: SightKind::Fog, champ: Some(LEE), before: 0.0 }));
    }

    #[test]
    fn an_unnamed_blip_is_never_fog() {
        let n = note(Some(LEE), &[], vec![seen(None, 6.0, (8000, 6000))], None);
        assert_eq!(verdict(&n, &[ZED]), Some(SightVerdict { kind: SightKind::Unclear, champ: None, before: 6.0 }));
    }

    #[test]
    fn seen_just_before_is_unclear() {
        let n = note(Some(LEE), &[], vec![seen(Some(LEE), 2.0, (8000, 6000))], None);
        assert_eq!(verdict(&n, &[ZED]), Some(SightVerdict { kind: SightKind::Unclear, champ: Some(LEE), before: 2.0 }));
    }

    #[test]
    fn an_execution_has_no_verdict() {
        assert_eq!(kind(&note(None, &[LEE], vec![], None)), None);
    }

    #[test]
    fn no_sight_no_verdict() {
        assert_eq!(kind(&DeathNote { sight: None, ..note(Some(LEE), &[], vec![], None) }), None);
    }

    #[test]
    fn far_sightings_say_nothing_once_the_death_has_a_place() {
        let far = note(Some(LEE), &[], vec![seen(Some(LEE), 8.0, (2000, 12000))], None);
        assert_eq!(kind(&far), Some(SightKind::Visible));
        assert_eq!(kind(&DeathNote { pos: Some((12000, 2000)), ..far.clone() }), None);
        assert_eq!(kind(&DeathNote { pos: Some((3000, 11000)), ..far }), Some(SightKind::Visible));

        let blip = note(Some(LEE), &[], vec![seen(None, 8.0, (2000, 12000))], None);
        assert_eq!(kind(&blip), Some(SightKind::Unclear));
        assert_eq!(kind(&DeathNote { pos: Some((12000, 2000)), ..blip }), Some(SightKind::Fog), "an unnamed blip across the map is not the killer");
    }

    #[test]
    fn lane_opponents_are_the_lane_or_both_bot_laners() {
        let foes = [(1, Some(Lane::Top)), (2, Some(Lane::Jungle)), (3, Some(Lane::Middle)), (4, Some(Lane::Bottom)), (5, Some(Lane::Support))];
        assert_eq!(lane_opponents(Some(Lane::Middle), foes), [3]);
        assert_eq!(lane_opponents(Some(Lane::Support), foes), [4, 5]);
        assert!(lane_opponents(None, foes).is_empty());
    }

    fn read(l: &mut SightLog, t: f32, blips: &[(i32, i32, Option<ChampKey>)], calls: &[&str]) {
        let calls: Vec<Call> = calls.iter().map(|k| Call { key: k.to_string(), text: format!("text {k}"), say: String::new() }).collect();
        l.push(t, blips, true, &calls);
    }

    #[test]
    fn stamp_keeps_who_was_seen_and_the_last_warning() {
        let mut l = SightLog::default();
        for i in 0..=15 {
            let t = 385.0 + i as f32;
            let calls: &[&str] = match i {
                2 => &["gank-jg-1"],
                8 => &["gank-bot-1"],
                14 => &["gank-bot-2"],
                _ => &[],
            };
            let mut blips = vec![(2000 + 100 * i, 12000, Some(LUX)), (9000 + 200 * i, 5000, None)];
            if i >= 6 {
                blips.push((11000 + 100 * i, 3000, Some(LEE)));
            }
            read(&mut l, t, &blips, calls);
        }
        let s = stamp(&l, 400.0, &[LEE]).unwrap();
        assert!(s.covered > 0.99);
        assert_eq!(s.seen.iter().map(|e| e.champ).collect::<Vec<_>>(), [Some(LEE), Some(LUX), None]);
        assert_eq!((s.seen[0].first, s.seen[0].last, s.seen[0].x), (9.0, 0.0, 12500));
        assert_eq!(s.warned, Some((7.0, "text gank-bot-1".to_string())));
    }

    #[test]
    fn stamp_needs_the_minimap_read_for_half_the_window() {
        let mut l = SightLog::default();
        for t in [385.0, 386.0, 387.0, 399.0] {
            read(&mut l, t, &[], &[]);
        }
        assert!(stamp(&l, 400.0, &[]).is_none());
    }
}
