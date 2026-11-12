use crate::common::champ;
use crate::consts::*;
use crate::gank::MapView;
use model::*;

const LANES: [Lane; 3] = [Lane::Top, Lane::Middle, Lane::Bottom];

#[derive(Debug, Clone, PartialEq)]
struct Row {
    lane: Lane,
    enemies: Vec<(ChampKey, String)>,
    reasons: Vec<(f32, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Board {
    rows: Vec<Row>,
    jungler: Option<(ChampKey, String)>,
}

fn group(l: Option<Lane>) -> Option<Lane> {
    match l? {
        Lane::Bottom | Lane::Support => Some(Lane::Bottom),
        Lane::Jungle => None,
        l => Some(l),
    }
}

fn side(l: Lane) -> &'static str {
    match l {
        Lane::Top => "top side",
        Lane::Middle => "mid",
        _ => "bot side",
    }
}

fn names(v: &[&str]) -> String {
    match v {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

pub fn board(snap: &GameSnapshot, sd: &StaticData, gold: &GoldDiff) -> Option<Board> {
    let me = snap.players.get(snap.me)?;
    let foes: Vec<&Player> = snap.players.iter().filter(|p| p.team != me.team).collect();
    let jungler = foes.iter().find(|p| p.smite).or_else(|| foes.iter().find(|p| p.position == Some(Lane::Jungle))).map(|p| (p.champ, sd.champ_name(p.champ).to_string()));
    let rows = LANES
        .iter()
        .map(|&lane| {
            let at = |ally: bool| -> Vec<&Player> {
                snap.players
                    .iter()
                    .enumerate()
                    .filter(|(i, p)| *i != snap.me && (p.team == me.team) == ally && group(p.position) == Some(lane))
                    .map(|(_, p)| p)
                    .collect()
            };
            let (ours, theirs) = (at(true), at(false));
            let mut reasons = vec![];
            if let Some(a) = ours.iter().map(|p| champ(sd, p.champ)).filter(|c| c.traits.cc >= JG_CC).max_by(|a, b| a.traits.cc.total_cmp(&b.traits.cc)) {
                reasons.push((JG_W_CC, format!("{} has CC", a.name)));
            }
            let stuck: Vec<&str> = theirs.iter().map(|p| champ(sd, p.champ)).filter(|c| NO_DASH.contains(&c.id.as_str())).map(|c| c.name.as_str()).collect();
            if !stuck.is_empty() {
                reasons.push((JG_W_NO_DASH, format!("{} {} no dash", names(&stuck), if stuck.len() == 1 { "has" } else { "have" })));
            }
            let flashless: Vec<&str> =
                theirs.iter().filter(|p| !p.spells.is_empty() && !p.spells.iter().any(|s| s == "SummonerFlash")).map(|p| sd.champ_name(p.champ)).collect();
            if !flashless.is_empty() {
                reasons.push((JG_W_NO_FLASH, format!("{} {} no Flash", names(&flashless), if flashless.len() == 1 { "has" } else { "have" })));
            }
            let diffs: Vec<i32> = gold.lanes.iter().filter(|d| group(d.lane) == Some(lane)).map(|d| d.gold_diff).collect();
            if !diffs.is_empty() {
                let diff: i32 = diffs.iter().sum();
                let who = if ours.len() > 1 { "your laners are" } else { "your laner is" };
                reasons.push(if diff >= JG_GOLD_EDGE {
                    (JG_W_GOLD, format!("{who} {} ahead", thousands(diff)))
                } else if diff <= -JG_GOLD_EDGE {
                    (JG_W_BEHIND, format!("{who} {} behind: a kill is unlikely", thousands(-diff)))
                } else {
                    (0.0, "lane is even".into())
                });
            }
            let top = |v: &[&Player]| v.iter().map(|p| p.level).max();
            if let (Some(a), Some(e)) = (top(&ours), top(&theirs))
                && a != e
            {
                let d = (a as i32 - e as i32).clamp(-2, 2) as f32;
                reasons.push((JG_W_LEVEL * d, format!("your side is {a}, theirs is {e}")));
            }
            Row { lane, enemies: theirs.iter().map(|p| (p.champ, sd.champ_name(p.champ).to_string())).collect(), reasons }
        })
        .collect();
    Some(Board { rows, jungler })
}

fn thousands(n: i32) -> String {
    crate::brief::thousands(n.unsigned_abs())
}

pub fn lanes(board: &Board, view: Option<&MapView>) -> Vec<LaneOpen> {
    let mut out: Vec<LaneOpen> = board
        .rows
        .iter()
        .map(|r| {
            let mut reasons = r.reasons.clone();
            if let Some(v) = view {
                for (c, name) in &r.enemies {
                    match v.seen.get(c) {
                        Some(s) if s.pushed && v.time - s.time <= JG_PUSHED_S => reasons.push((JG_W_PUSHED, format!("{name} is pushed up"))),
                        Some(s) if v.time - s.time >= JG_UNSEEN_S => {
                            let ago = ((v.time - s.time) / 5.0).floor() as u32 * 5;
                            reasons.push((JG_W_UNSEEN, format!("{name} not seen for {ago} s")));
                        }
                        None => reasons.push((JG_W_UNSEEN, format!("{name} not seen"))),
                        _ => {}
                    }
                }
                if let Some((j, name)) = &board.jungler
                    && let Some(s) = v.seen.get(j).filter(|s| v.time - s.time <= JG_SEEN_FRESH_S)
                {
                    let w = if s.side == r.lane { JG_W_JUNGLER_HERE } else { JG_W_JUNGLER_AWAY };
                    reasons.push((w, format!("{name} was just seen {}", side(s.side))));
                }
            }
            let score: f32 = reasons.iter().map(|r| r.0).sum();
            reasons.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
            let reasons = if score < JG_LANE_FLOOR {
                vec!["nothing special".to_string()]
            } else {
                reasons.into_iter().take(JG_REASONS).map(|r| r.1).collect()
            };
            LaneOpen { lane: r.lane, score, reasons, enemy: r.enemies.iter().map(|e| e.0).collect() }
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out
}
