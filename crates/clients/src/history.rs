//! My own match history from the League client, compacted into `PastGame`s and
//! cached on disk: a played game never changes, so details and timelines are
//! fetched once per game.

use crate::lcu::Lcu;
use crate::id;
use anyhow::{Context, Result};
use model_game::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Games read from the history list, newest first.
pub const RECENT: u32 = 30;
/// Bump when parsing changes so cached games are parsed again.
const CACHE_VERSION: u32 = 7;

/// Recent games, newest first. The client lists only the last few games, but
/// older ones stay fetchable by id, so every game cached in `dir` stays in the
/// history (the newest RECENT by id). Only games missing from the cache, or
/// cached by an older parser, cost requests; a game whose details fail is used
/// from the list entry or its old cache and retried next sync.
pub async fn sync(lcu: &Lcu, dir: &Path) -> Result<Vec<PastGame>> {
    let path = format!("/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=0&endIndex={RECENT}");
    let list = lcu.get(&path).await?.context("no match history")?;
    tokio::fs::create_dir_all(dir).await?;
    let entries: HashMap<u64, &Value> =
        list_games(&list).filter(|g| wanted(g)).filter_map(|g| Some((g["gameId"].as_u64()?, g))).collect();
    // (game id, cache version) per cache file
    let mut cached: Vec<(u64, u32)> = Vec::new();
    let mut rd = tokio::fs::read_dir(dir).await?;
    while let Some(e) = rd.next_entry().await? {
        let name = e.file_name();
        let mut parts = name.to_str().unwrap_or("").split('.');
        if let (Some(Ok(id)), Some(Some(Ok(v)))) =
            (parts.next().map(str::parse), parts.next().map(|v| v.strip_prefix('v').map(str::parse)))
        {
            cached.push((id, v));
        }
    }
    let mut ids: Vec<u64> = entries.keys().copied().chain(cached.iter().map(|c| c.0)).collect();
    ids.sort_unstable_by(|a, b| b.cmp(a));
    ids.dedup();
    ids.truncate(RECENT as usize);

    let me = lcu.get("/lol-summoner/v1/current-summoner").await.ok().flatten();
    let puuid = me.as_ref().and_then(|m| m["puuid"].as_str()).or_else(|| entries.values().find_map(|e| my_puuid(e)));
    let read = |v: u32, gid: u64| {
        let file = dir.join(format!("{gid}.v{v}.json"));
        async move { tokio::fs::read(&file).await.ok().and_then(|b| serde_json::from_slice::<PastGame>(&b).ok()) }
    };
    let mut out = Vec::new();
    for gid in ids {
        if let Some(g) = read(CACHE_VERSION, gid).await {
            out.push(g);
            continue;
        }
        let detail = lcu.get(&format!("/lol-match-history/v1/games/{gid}")).await;
        let timeline = lcu.get(&format!("/lol-match-history/v1/game-timelines/{gid}")).await;
        let tl = timeline.as_ref().ok().and_then(Option::as_ref);
        let mut game = match &detail {
            Ok(Some(d)) => parse_game(d, puuid, tl),
            _ => None,
        };
        let fresh = detail.is_ok() && timeline.is_ok();
        if game.is_none() {
            game = entries.get(&gid).and_then(|e| parse_game(e, puuid, None));
        }
        if game.is_none()
            && let Some(&(_, v)) = cached.iter().filter(|c| c.0 == gid).max_by_key(|c| c.1)
        {
            game = read(v, gid).await;
        }
        let Some(game) = game else { continue };
        // 404s are final; other errors are retried next sync.
        if fresh && let Ok(json) = serde_json::to_vec(&game) {
            let _ = tokio::fs::write(dir.join(format!("{gid}.v{CACHE_VERSION}.json")), json).await;
        }
        out.push(game);
    }
    out.sort_by_key(|g| std::cmp::Reverse(g.created));
    Ok(out)
}

/// One game in full for the review: details, the timeline while the client still has it, and me.
pub async fn fetch_match(lcu: &Lcu, game_id: u64) -> Result<MatchDetail> {
    let me = lcu.get("/lol-summoner/v1/current-summoner").await?.context("no summoner")?;
    let game = lcu.get(&format!("/lol-match-history/v1/games/{game_id}")).await?.context("game not found")?;
    let timeline = lcu.get(&format!("/lol-match-history/v1/game-timelines/{game_id}")).await.ok().flatten();
    parse_match(&game, me["puuid"].as_str(), timeline.as_ref()).context("not a game the coach reviews")
}

/// Game entries of a `/matches` response.
pub fn list_games(json: &Value) -> impl Iterator<Item = &Value> {
    json.pointer("/games/games").and_then(Value::as_array).into_iter().flatten()
}

/// Rift and Howling Abyss matchmade games; no customs, practice tool, bots, tutorials or Arena.
pub fn wanted(game: &Value) -> bool {
    let q = game["queueId"].as_i64().unwrap_or(0);
    matches!(game["mapId"].as_i64(), Some(11 | 12)) && q > 0 && !(800..900).contains(&q) && !(2000..=2020).contains(&q)
}

/// The list's own entry carries only my identity.
fn my_puuid(entry: &Value) -> Option<&str> {
    entry["participantIdentities"].as_array()?.first()?.pointer("/player/puuid")?.as_str().filter(|s| !s.is_empty())
}

fn arr(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}

/// Lane from a participant's `timeline.lane` / `role`; None for "NONE".
fn lane_of(p: &Value) -> Option<Lane> {
    let t = &p["timeline"];
    match (t["lane"].as_str()?, t["role"].as_str().unwrap_or_default()) {
        ("BOTTOM" | "BOT", "DUO_SUPPORT" | "SUPPORT") => Some(Lane::Support),
        (l, _) => Lane::from_riot(l),
    }
}

fn objectives(t: &Value) -> TeamObjectives {
    let n = |k: &str| t[k].as_u64().unwrap_or(0) as u32;
    TeamObjectives {
        dragons: n("dragonKills"),
        barons: n("baronKills"),
        heralds: n("riftHeraldKills"),
        grubs: n("hordeKills"),
        towers: n("towerKills"),
    }
}

const LANES: [Lane; 5] = [Lane::Top, Lane::Jungle, Lane::Middle, Lane::Bottom, Lane::Support];

/// Lanes by participantId on the Rift. Riot's `lane`/`role` labels when a team's
/// make a lineup (one of each); otherwise where each player stood from 1:30 to
/// 10:00 and what they farmed, when that tells all five apart.
fn rift_lanes(parts: &[&Value], frames: Option<&Vec<Value>>) -> HashMap<i64, Lane> {
    let mut out = HashMap::new();
    for team in [100, 200] {
        let ps: Vec<&Value> = parts.iter().copied().filter(|p| p["teamId"].as_i64() == Some(team)).collect();
        let labels: Vec<Option<Lane>> = ps.iter().map(|p| lane_of(p)).collect();
        let lineup = ps.len() == 5 && LANES.iter().all(|l| labels.contains(&Some(*l)));
        let lanes = if lineup { labels } else { frames.and_then(|f| infer_lanes(&ps, f)).unwrap_or(labels) };
        for (p, l) in ps.iter().zip(lanes) {
            if let (Some(id), Some(l)) = (p["participantId"].as_i64(), l) {
                out.insert(id, l);
            }
        }
    }
    out
}

fn infer_lanes(ps: &[&Value], frames: &[Value]) -> Option<Vec<Option<Lane>>> {
    if ps.len() != 5 {
        return None;
    }
    let early: Vec<&Value> =
        frames.iter().filter(|f| f["timestamp"].as_u64().is_some_and(|t| (90_000..=630_000).contains(&t))).collect();
    let last = early.last()?;
    // Affinity per player for LANES: frames in the lane's zone, plus farm that
    // marks a jungler, a laner taking the minions, or a support leaving them.
    let aff: Vec<[i32; 5]> = ps
        .iter()
        .map(|p| {
            let pid = p["participantId"].as_i64().unwrap_or(0).to_string();
            let mut n = [0; 4];
            for f in &early {
                let pos = &f["participantFrames"][&pid]["position"];
                let (Some(x), Some(y)) = (pos["x"].as_i64(), pos["y"].as_i64()) else { continue };
                match MapZone::at(x as i32, y as i32) {
                    MapZone::Top => n[0] += 1,
                    MapZone::Mid => n[1] += 1,
                    MapZone::Bot => n[2] += 1,
                    MapZone::BlueJungle | MapZone::RedJungle => n[3] += 1,
                    _ => {}
                }
            }
            let pf = &last["participantFrames"][&pid];
            let minions = pf["minionsKilled"].as_i64().unwrap_or(0);
            let camps = pf["jungleMinionsKilled"].as_i64().unwrap_or(0);
            let bonus = |c: bool| if c { 3 } else { 0 };
            [n[0], n[3] + bonus(camps >= 20), n[1], n[2] + bonus(minions >= 40), n[2] + bonus(minions < 30)]
        })
        .collect();
    let mut best = None;
    assign(&aff, 0, 0, &mut [0; 5], 0, &mut best);
    best.map(|(_, a)| a.iter().map(|&r| Some(LANES[r])).collect())
}

/// Highest-affinity one-to-one assignment where every player has some affinity for their lane.
fn assign(aff: &[[i32; 5]], i: usize, used: u8, cur: &mut [usize; 5], sum: i32, best: &mut Option<(i32, [usize; 5])>) {
    if i == aff.len() {
        if best.is_none_or(|b| sum > b.0) {
            *best = Some((sum, *cur));
        }
        return;
    }
    for r in (0..5).filter(|r| used & 1 << r == 0 && aff[i][*r] > 0) {
        cur[i] = r;
        assign(aff, i + 1, used | 1 << r, cur, sum + aff[i][r], best);
    }
}

/// Pure parser for one game from `/lol-match-history/v1/games/{id}` (all ten
/// players) or a `/matches` list entry (me only). `puuid` picks me; a lone
/// participant is me. `timeline` is `/lol-match-history/v1/game-timelines/{id}`.
pub fn parse_game(game: &Value, puuid: Option<&str>, timeline: Option<&Value>) -> Option<PastGame> {
    parse(game, puuid, timeline).map(|p| p.game)
}

struct Parsed<'a> {
    game: PastGame,
    me_id: i64,
    /// Sorted by participantId.
    parts: Vec<&'a Value>,
    lanes: HashMap<i64, Lane>,
    frames: Option<&'a Vec<Value>>,
}

fn parse<'a>(game: &'a Value, puuid: Option<&str>, timeline: Option<&'a Value>) -> Option<Parsed<'a>> {
    if !wanted(game) {
        return None;
    }
    let mut parts: Vec<&Value> = arr(&game["participants"]).collect();
    parts.sort_by_key(|p| p["participantId"].as_i64());
    let me_id = arr(&game["participantIdentities"])
        .find(|i| puuid.is_some() && i.pointer("/player/puuid").and_then(Value::as_str) == puuid)
        .and_then(|i| i["participantId"].as_i64())
        .or_else(|| (parts.len() == 1).then(|| parts[0]["participantId"].as_i64()).flatten())?;
    let me = *parts.iter().find(|p| p["participantId"].as_i64() == Some(me_id))?;
    let s = &me["stats"];
    let n = |k: &str| s[k].as_f64().map_or(0, |v| v.max(0.0) as u32);
    let queue_id = game["queueId"].as_i64()?;
    let mode = Mode::from_queue(queue_id);
    let mut duration_s = game["gameDuration"].as_u64()? as u32;
    if duration_s > 20_000 {
        duration_s /= 1000; // milliseconds in some client versions
    }

    let my_team = me["teamId"].as_i64();
    let frames = timeline.and_then(|t| t["frames"].as_array());
    let lanes = if mode == Mode::Rift { rift_lanes(&parts, frames) } else { HashMap::new() };
    let lane_of = |p: &Value| p["participantId"].as_i64().and_then(|id| lanes.get(&id).copied());
    let lane = lane_of(me);
    let opp = lane.and_then(|l| {
        let mut it = parts.iter().filter(|p| p["teamId"].as_i64() != my_team && lane_of(p) == Some(l));
        let o = it.next()?;
        it.next().is_none().then(|| o["participantId"].as_i64()).flatten()
    });

    let teams: Vec<&Value> = arr(&game["teams"]).collect();
    let ours = teams.iter().find(|t| t["teamId"].as_i64() == my_team);
    let theirs = teams.iter().find(|t| my_team.is_some() && t["teamId"].as_i64() != my_team);
    let first = |keys: &[&str]| {
        let got = |t: Option<&&Value>| t.is_some_and(|t| keys.iter().any(|k| t[*k] == true));
        if got(ours) { Some(true) } else if got(theirs) { Some(false) } else { None }
    };

    let at = |min: u64, key: &str, pid: i64| -> Option<i64> {
        let f = frames?.iter().find(|f| f["timestamp"].as_u64().is_some_and(|t| t >= min * 60_000))?;
        f["participantFrames"][pid.to_string()][key].as_i64()
    };
    let diff = |min, key| Some((at(min, key, me_id)? - at(min, key, opp?)?) as i32);
    let death_times = frames.map(|fs| {
        fs.iter()
            .flat_map(|f| arr(&f["events"]))
            .filter(|e| e["type"] == "CHAMPION_KILL" && e["victimId"].as_i64() == Some(me_id))
            .filter_map(|e| Some((e["timestamp"].as_u64()? / 1000) as u32))
            .collect()
    });
    let champ_of = |pid: i64| parts.iter().find(|p| p["participantId"].as_i64() == Some(pid)).and_then(|p| id(&p["championId"]));
    let killed_by = frames
        .into_iter()
        .flatten()
        .flat_map(|f| arr(&f["events"]))
        .filter(|e| e["type"] == "CHAMPION_KILL" && e["victimId"].as_i64() == Some(me_id))
        .map(|e| e["killerId"].as_i64().and_then(champ_of))
        .collect();
    let blue = my_team == Some(100);
    let death_spots = frames
        .filter(|_| mode == Mode::Rift)
        .into_iter()
        .flatten()
        .flat_map(|f| arr(&f["events"]))
        .filter(|e| e["type"] == "CHAMPION_KILL" && e["victimId"].as_i64() == Some(me_id))
        .filter_map(|e| {
            let p = &e["position"];
            let (x, y) = MapZone::from_blue(p["x"].as_i64()? as i32, p["y"].as_i64()? as i32, blue);
            Some(DeathSpot { time: (e["timestamp"].as_u64()? / 1000) as u32, x, y })
        })
        .collect();

    let game = PastGame {
        game_id: game["gameId"].as_u64()?,
        created: game["gameCreation"].as_u64().unwrap_or(0),
        queue_id,
        mode,
        champ: id(&me["championId"])?,
        lane,
        win: s["win"] == true,
        remake: duration_s < 300 || s["gameEndedInEarlySurrender"] == true,
        duration_s,
        kills: n("kills"),
        deaths: n("deaths"),
        assists: n("assists"),
        cs: n("totalMinionsKilled") + n("neutralMinionsKilled"),
        gold: n("goldEarned"),
        damage_to_champs: n("totalDamageDealtToChampions"),
        vision: n("visionScore"),
        death_times,
        gold_diff_10: diff(10, "totalGold"),
        gold_diff_15: diff(15, "totalGold"),
        xp_diff_15: diff(15, "xp"),
        ours: ours.map(|t| objectives(t)),
        theirs: theirs.map(|t| objectives(t)),
        first_dragon: first(&["firstDargon", "firstDragon"]),
        first_tower: first(&["firstTower"]),
        first_blood: first(&["firstBlood"]),
        cs_10: at(10, "minionsKilled", me_id).map(|m| (m + at(10, "jungleMinionsKilled", me_id).unwrap_or(0)) as u32),
        augments: (1..=6).filter_map(|i| id(&s[&format!("playerAugment{i}")])).collect(),
        death_spots,
        enemies: parts.iter().filter(|p| p["teamId"].as_i64() != my_team).filter_map(|p| id(&p["championId"])).collect(),
        opponent: opp.and_then(champ_of),
        killed_by,
        control_wards: s["visionWardsBoughtInGame"].as_u64().map(|n| n as u32),
    };
    Some(Parsed { game, me_id, parts, lanes, frames })
}

/// Everything the game review reads: all ten players' lines, the per-minute
/// frames, kills and objectives. Needs the full game details (not a list entry).
pub fn parse_match(game: &Value, puuid: Option<&str>, timeline: Option<&Value>) -> Option<MatchDetail> {
    let Parsed { game: past, me_id, parts, lanes, frames } = parse(game, puuid, timeline)?;
    let idx: HashMap<i64, usize> = parts.iter().enumerate().filter_map(|(i, p)| Some((p["participantId"].as_i64()?, i))).collect();
    let me = *idx.get(&me_id)?;
    let my_team = parts[me]["teamId"].as_i64();
    let names: HashMap<i64, &str> = arr(&game["participantIdentities"])
        .filter_map(|i| Some((i["participantId"].as_i64()?, i.pointer("/player/gameName")?.as_str()?)))
        .collect();
    let players = parts
        .iter()
        .map(|p| {
            let pid = p["participantId"].as_i64().unwrap_or(0);
            let s = &p["stats"];
            let n = |k: &str| s[k].as_f64().map_or(0, |v| v.max(0.0) as u32);
            MatchPlayer {
                champ: id(&p["championId"]).unwrap_or(0),
                name: names.get(&pid).unwrap_or(&"").to_string(),
                ally: p["teamId"].as_i64() == my_team,
                me: pid == me_id,
                lane: lanes.get(&pid).copied(),
                level: n("champLevel"),
                kills: n("kills"),
                deaths: n("deaths"),
                assists: n("assists"),
                cs: n("totalMinionsKilled") + n("neutralMinionsKilled"),
                gold: n("goldEarned"),
                damage: n("totalDamageDealtToChampions"),
                taken: n("totalDamageTaken"),
                vision: n("visionScore"),
                items: (0..=6).filter_map(|i| id(&s[&format!("item{i}")])).collect(),
                wards_placed: n("wardsPlaced"),
                wards_killed: n("wardsKilled"),
                control_wards: n("visionWardsBoughtInGame"),
            }
        })
        .collect();
    let pos = |v: &Value| Some((v["x"].as_i64()? as i32, v["y"].as_i64()? as i32));
    let secs = |v: &Value| v["timestamp"].as_u64().unwrap_or(0) as f32 / 1000.0;
    let frame = |f: &Value| Frame {
        time: secs(f),
        players: parts
            .iter()
            .map(|p| {
                let pf = &f["participantFrames"][p["participantId"].as_i64().unwrap_or(0).to_string()];
                let n = |k: &str| pf[k].as_u64().unwrap_or(0) as u32;
                FrameStat {
                    total_gold: n("totalGold"),
                    current_gold: n("currentGold"),
                    level: n("level"),
                    xp: n("xp"),
                    cs: n("minionsKilled") + n("jungleMinionsKilled"),
                    jungle_cs: n("jungleMinionsKilled"),
                    pos: pos(&pf["position"]),
                }
            })
            .collect(),
    };
    let events: Vec<&Value> = frames.into_iter().flatten().flat_map(|f| arr(&f["events"])).collect();
    let who = |v: &Value| v.as_i64().and_then(|i| idx.get(&i).copied());
    let kills = events
        .iter()
        .filter(|e| e["type"] == "CHAMPION_KILL")
        .filter_map(|e| {
            Some(KillEvent {
                time: secs(e),
                killer: who(&e["killerId"]),
                victim: who(&e["victimId"])?,
                assisters: arr(&e["assistingParticipantIds"]).filter_map(who).collect(),
                pos: pos(&e["position"]),
            })
        })
        .collect();
    let killer_team = |e: &Value| match e["killerTeamId"].as_i64().filter(|t| *t > 0) {
        Some(t) => Some(t),
        None => who(&e["killerId"]).and_then(|i| parts[i]["teamId"].as_i64()),
    };
    let mut marks: Vec<TimelineMark> = Vec::new();
    for e in &events {
        let time = secs(e);
        let (ours, kind, detail) = match e["type"].as_str() {
            Some("ELITE_MONSTER_KILL") => {
                let Some(t) = killer_team(e) else { continue };
                let (kind, detail) = match (e["monsterType"].as_str().unwrap_or(""), e["monsterSubType"].as_str().unwrap_or("")) {
                    ("DRAGON", sub) => ("Dragon", dragon_name(sub)),
                    ("BARON_NASHOR", _) => ("Baron", None),
                    ("RIFTHERALD", _) => ("Herald", None),
                    ("HORDE", _) => ("Grubs", None),
                    ("ATAKHAN", _) => ("Atakhan", None),
                    _ => continue,
                };
                (Some(t) == my_team, kind, detail.map(str::to_string))
            }
            Some("BUILDING_KILL") => {
                // `teamId` is the side that lost the building.
                let Some(lost) = e["teamId"].as_i64().filter(|t| *t > 0) else { continue };
                let lane = match e["laneType"].as_str() {
                    Some("TOP_LANE") => Some("top"),
                    Some("MID_LANE") => Some("mid"),
                    Some("BOT_LANE") => Some("bot"),
                    _ => None,
                };
                let (kind, tier) = match (e["buildingType"].as_str(), e["towerType"].as_str()) {
                    (Some("INHIBITOR_BUILDING"), _) => ("Inhibitor", None),
                    (_, Some("OUTER_TURRET")) => ("Tower", Some("outer")),
                    (_, Some("INNER_TURRET")) => ("Tower", Some("inner")),
                    (_, Some("BASE_TURRET")) => ("Tower", Some("inhibitor")),
                    (_, Some("NEXUS_TURRET")) => ("Tower", Some("nexus")),
                    _ => ("Tower", None),
                };
                let detail = [lane, tier].into_iter().flatten().collect::<Vec<_>>().join(" ");
                (Some(lost) != my_team, kind, Some(detail).filter(|d| !d.is_empty()))
            }
            _ => continue,
        };
        let mine = e["type"] == "ELITE_MONSTER_KILL"
            && (e["killerId"].as_i64() == Some(me_id) || arr(&e["assistingParticipantIds"]).any(|a| a.as_i64() == Some(me_id)));
        // Grubs die in quick succession: one mark per team per pit visit.
        if kind == "Grubs"
            && let Some(m) = marks.iter_mut().rev().find(|m| m.kind == "Grubs" && m.ours == ours && time - m.time < 90.0)
        {
            let n = m.detail.as_deref().and_then(|d| d.trim_start_matches('×').parse::<u32>().ok()).unwrap_or(1);
            m.detail = Some(format!("×{}", n + 1));
            m.mine |= mine;
            continue;
        }
        marks.push(TimelineMark { time, ours, kind: kind.to_string(), detail, pos: pos(&e["position"]), mine, presence: None });
    }
    let frames = frames.into_iter().flatten().map(frame).collect();
    Some(MatchDetail { game: past, me, players, frames, kills, marks })
}

fn dragon_name(sub: &str) -> Option<&'static str> {
    Some(match sub {
        "FIRE_DRAGON" => "Infernal",
        "WATER_DRAGON" => "Ocean",
        "EARTH_DRAGON" => "Mountain",
        "AIR_DRAGON" => "Cloud",
        "HEXTECH_DRAGON" => "Hextech",
        "CHEMTECH_DRAGON" => "Chemtech",
        "ELDER_DRAGON" => "Elder",
        _ => return None,
    })
}
