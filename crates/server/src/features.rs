use crate::npz::Npz;
use flate2::Crc;
use flate2::read::MultiGzDecoder;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub const KEYS: [&str; 14] = ["gold", "level", "cs", "kills", "towers", "inhibs", "drakes", "soul", "elder", "baron", "grubs", "dead", "outlook", "side"];
pub const TOP_BAND: &str = "APEX";
const OUTLOOK: usize = 12;
const COLUMNS: usize = 1 + KEYS.len();
const GOLD_PAD: f64 = 3000.0;
const SOUL_DRAGONS: i32 = 4;
const DRAKE_CAP: i32 = SOUL_DRAGONS - 1;
const DEAD_CAP: f64 = 45.0;
const GRUBS_COUNT: f64 = 3.0;
const BARON_BUFF: f64 = 180.0;
const ELDER_BUFF: f64 = 150.0;
const INHIB_RESPAWN: f64 = 300.0;
const RESPAWN: [f64; 18] = [10.0, 10.0, 12.0, 12.0, 14.0, 16.0, 20.0, 25.0, 28.0, 32.5, 35.0, 37.5, 40.0, 42.5, 45.0, 47.5, 50.0, 52.5];
const RESPAWN_GROWTH: [(f64, f64, f64); 3] = [(0.00425, 900.0, 1800.0), (0.003, 1800.0, 2700.0), (0.0145, 2700.0, 3300.0)];
const RESPAWN_GROWTH_MAX: f64 = 0.5;
const PLAN_SHRINK_K: f64 = 400.0;
const LENGTH_FROM: [f64; 7] = [0.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0];
const MIN_GAME_S: i64 = 300;
const TEST_SHARE: u32 = 5;
const FOLDS: u32 = 5;
const FRAME_MS: i64 = 60000;
const BLUE: i64 = 100;
const RED: i64 = 200;
const NOBODY: i64 = 0;
const NEUTRAL: i64 = 300;
const POSSESSOR: i64 = 234;
const PROGRESS: usize = 2500;
const PIDS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"];
const MAX_WORKERS: usize = 4;

const ITEM_PURCHASED: &str = "ITEM_PURCHASED";
const ITEM_SOLD: &str = "ITEM_SOLD";
const ITEM_DESTROYED: &str = "ITEM_DESTROYED";
const ITEM_UNDO: &str = "ITEM_UNDO";
const CHAMPION_KILL: &str = "CHAMPION_KILL";
const BUILDING_KILL: &str = "BUILDING_KILL";
const ELITE_MONSTER_KILL: &str = "ELITE_MONSTER_KILL";
const DRAGON_SOUL_GIVEN: &str = "DRAGON_SOUL_GIVEN";
const GAME_END: &str = "GAME_END";
const IGNORED_EVENTS: [&str; 12] = [
    "PAUSE_START", "PAUSE_END", "SKILL_LEVEL_UP", "LEVEL_UP", "WARD_PLACED", "WARD_KILL", "TURRET_PLATE_DESTROYED", "CHAMPION_SPECIAL_KILL", "CHAMPION_TRANSFORM",
    "OBJECTIVE_BOUNTY_PRESTART", "OBJECTIVE_BOUNTY_FINISH", "FEAT_UPDATE",
];
const TOWER: &str = "TOWER_BUILDING";
const INHIBITOR: &str = "INHIBITOR_BUILDING";
const DRAGON: &str = "DRAGON";
const BARON: &str = "BARON_NASHOR";
const GRUB: &str = "HORDE";
const IGNORED_MONSTERS: [&str; 2] = ["RIFTHERALD", "ATAKHAN"];
const ELDER: &str = "ELDER_DRAGON";
const ELEMENTAL_DRAGONS: [&str; 6] = ["FIRE_DRAGON", "WATER_DRAGON", "EARTH_DRAGON", "AIR_DRAGON", "HEXTECH_DRAGON", "CHEMTECH_DRAGON"];

#[derive(Deserialize)]
struct Meta {
    #[serde(rename = "matchId")]
    match_id: String,
}

#[derive(Deserialize)]
struct Match {
    metadata: Meta,
    info: MatchInfo,
}

#[derive(Deserialize)]
struct MatchInfo {
    #[serde(rename = "gameVersion")]
    version: String,
    #[serde(rename = "mapId")]
    map: i64,
    #[serde(rename = "gameDuration")]
    duration: i64,
    participants: Vec<Participant>,
    teams: Vec<Team>,
}

#[derive(Deserialize)]
struct Participant {
    #[serde(rename = "participantId")]
    id: i64,
    #[serde(rename = "teamId")]
    team: i64,
    #[serde(rename = "championId")]
    champ: i64,
    #[serde(rename = "gameEndedInEarlySurrender")]
    early_surrender: bool,
    #[serde(rename = "teamEarlySurrendered")]
    team_early_surrendered: bool,
    #[serde(default)]
    item0: Option<i64>,
    #[serde(default)]
    item1: Option<i64>,
    #[serde(default)]
    item2: Option<i64>,
    #[serde(default)]
    item3: Option<i64>,
    #[serde(default)]
    item4: Option<i64>,
    #[serde(default)]
    item5: Option<i64>,
    #[serde(default)]
    item6: Option<i64>,
    #[serde(rename = "roleBoundItem", default)]
    role_item: Option<i64>,
}

#[derive(Deserialize)]
struct Team {
    #[serde(rename = "teamId")]
    team: i64,
    win: bool,
}

#[derive(Deserialize)]
struct Timeline {
    metadata: Meta,
    info: TimelineInfo,
}

#[derive(Deserialize)]
struct TimelineInfo {
    #[serde(rename = "frameInterval")]
    interval: i64,
    frames: Vec<Frame>,
}

#[derive(Deserialize)]
struct Frame {
    timestamp: i64,
    #[serde(rename = "participantFrames")]
    participants: BTreeMap<String, PlayerFrame>,
    events: Vec<Event>,
}

#[derive(Deserialize)]
struct PlayerFrame {
    level: i64,
    #[serde(rename = "minionsKilled")]
    minions: i64,
    #[serde(rename = "jungleMinionsKilled")]
    jungle: i64,
    #[serde(rename = "totalGold")]
    _total_gold: i64,
    #[serde(rename = "currentGold")]
    _current_gold: i64,
}

#[derive(Deserialize, Clone)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    timestamp: i64,
    #[serde(rename = "participantId")]
    participant: Option<i64>,
    #[serde(rename = "itemId")]
    item: Option<i64>,
    #[serde(rename = "beforeId")]
    before: Option<i64>,
    #[serde(rename = "afterId")]
    after: Option<i64>,
    #[serde(rename = "killerId")]
    killer: Option<i64>,
    #[serde(rename = "victimId")]
    victim: Option<i64>,
    #[serde(rename = "buildingType")]
    building: Option<String>,
    #[serde(rename = "teamId")]
    team: Option<i64>,
    #[serde(rename = "monsterType")]
    monster: Option<String>,
    #[serde(rename = "monsterSubType")]
    monster_sub: Option<String>,
    #[serde(rename = "killerTeamId")]
    killer_team: Option<i64>,
    #[serde(rename = "winningTeam")]
    winner: Option<i64>,
}

#[derive(Default)]
pub struct Values {
    pub gold: HashMap<i64, i64>,
    pub after: HashMap<i64, i64>,
    pub at_once: HashSet<i64>,
}

impl Values {
    fn of(&self, item: i64) -> i64 {
        self.gold.get(&item).copied().unwrap_or(0)
    }
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

fn int_of(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

pub fn item_values(ddragon: &Value) -> Result<Values, String> {
    let data = ddragon.get("data").and_then(Value::as_object).ok_or("item.json has no data")?;
    let raw: BTreeMap<i64, &Value> = data.iter().filter(|(k, _)| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit())).filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect();
    let has_tag = |d: &Value, tag: &str| d.get("tags").and_then(Value::as_array).is_some_and(|t| t.iter().any(|x| x.as_str() == Some(tag)));
    let shop = |i: i64, d: &Value| {
        let map_on = |m: &str| d.get("maps").and_then(|x| x.get(m)) == Some(&Value::Bool(true));
        let none = |f: &str| d.get(f).is_none_or(Value::is_null);
        i < 200_000
            && truthy(d.get("gold").and_then(|g| g.get("purchasable")))
            && (map_on("11") || map_on("12"))
            && none("requiredChampion")
            && none("requiredAlly")
            && d.get("inStore") != Some(&Value::Bool(false))
            && d.get("hideFromAll") != Some(&Value::Bool(true))
            && !has_tag(d, "Trinket")
    };
    let mut keep: HashSet<i64> = raw.iter().filter(|(i, d)| shop(**i, d)).map(|(i, _)| *i).collect();
    let recipes: Vec<i64> = raw.iter().filter(|(_, d)| d.get("specialRecipe").and_then(Value::as_i64).is_some_and(|r| keep.contains(&r))).map(|(i, _)| *i).collect();
    keep.extend(recipes);
    let mut values = Values::default();
    for &i in &keep {
        let d = raw[&i];
        let gold = if has_tag(d, "Consumable") { 0 } else { d.get("gold").and_then(|g| g.get("total")).and_then(Value::as_i64).ok_or_else(|| format!("item {i}: no gold total"))? };
        values.gold.insert(i, gold);
    }
    for (&i, d) in raw.iter().rev() {
        let only: Vec<i64> = d.get("from").and_then(Value::as_array).map(|a| a.iter().filter_map(int_of).collect()).unwrap_or_default();
        if truthy(d.get("specialRecipe")) {
            let recipe = d.get("specialRecipe").and_then(int_of).ok_or_else(|| format!("item {i}: specialRecipe"))?;
            values.after.insert(recipe, i);
            values.at_once.insert(recipe);
        } else if only.len() == 1 && d.get("gold").and_then(|g| g.get("base")).and_then(Value::as_f64) == Some(0.0) {
            values.after.insert(only[0], i);
        }
    }
    Ok(values)
}

pub fn is_test(match_id: &str) -> bool {
    crc32(&format!("split:{match_id}")).is_multiple_of(TEST_SHARE)
}

pub fn fold(match_id: &str) -> u32 {
    crc32(&format!("fold:{match_id}")) % FOLDS
}

fn crc32(text: &str) -> u32 {
    let mut crc = Crc::new();
    crc.update(text.as_bytes());
    crc.sum()
}

pub fn respawn(level: i64, t: f64) -> f64 {
    let base = RESPAWN[(level.clamp(1, 18) - 1) as usize];
    let mut grow = 0.0;
    for (k, start, to) in RESPAWN_GROWTH {
        grow += k * ((t.min(to) - start) / 30.0).max(0.0);
    }
    base * (1.0 + grow.min(RESPAWN_GROWTH_MAX))
}

fn shrink(wr: f64, games: f64, k: f64) -> f64 {
    50.0 + (wr - 50.0) * games / (games + k)
}

pub fn wr_from(buckets: &[[i64; 2]; 7], minute: f64) -> Option<f64> {
    let (mut games, mut wins) = (0, 0);
    for (i, [g, w]) in buckets.iter().enumerate() {
        if i + 1 == buckets.len() || LENGTH_FROM[i + 1] > minute {
            games += g;
            wins += w;
        }
    }
    (games > 0).then(|| shrink(100.0 * wins as f64 / games as f64, games as f64, PLAN_SHRINK_K))
}

pub fn bucket(duration_min: f64) -> usize {
    LENGTH_FROM.iter().rposition(|f| *f <= duration_min).unwrap_or(0)
}

pub type Tables = HashMap<i64, [[i64; 2]; 7]>;

#[derive(Clone, Debug, PartialEq)]
pub struct Head {
    pub match_id: String,
    pub patch: String,
    pub mode: &'static str,
    pub y: bool,
    pub champs: [[i64; 5]; 2],
    pub duration_min: f64,
}

impl Head {
    pub fn bucket(&self) -> usize {
        bucket(self.duration_min)
    }
}

pub fn add_to_tables(tables: &mut Tables, champs: &[[i64; 5]; 2], y: bool, bucket: usize) {
    for (side, list) in champs.iter().enumerate() {
        for c in list {
            let row = &mut tables.entry(*c).or_insert([[0; 2]; 7])[bucket];
            row[0] += 1;
            row[1] += i64::from(y == (side == 0));
        }
    }
}

pub fn outlook(champs: &[[i64; 5]; 2], y: bool, bucket: usize, minute: f64, tables: &Tables, own: bool) -> f64 {
    let mut total = 0.0;
    for (side, list) in champs.iter().enumerate() {
        for c in list {
            let Some(rows) = tables.get(c) else { continue };
            let mut gw = *rows;
            if own {
                gw[bucket][0] -= 1;
                gw[bucket][1] -= i64::from(y == (side == 0));
            }
            if let Some(wr) = wr_from(&gw, minute) {
                total += if side == 0 { wr - 50.0 } else { 50.0 - wr };
            }
        }
    }
    total
}

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub item_gold: [f64; 2],
    pub level: [f64; 2],
    pub cs: [f64; 2],
    pub kills: [f64; 2],
    pub towers: [i32; 2],
    pub inhibs: [i32; 2],
    pub drakes: [i32; 2],
    pub soul: i32,
    pub elder: i32,
    pub baron: i32,
    pub grubs: [i32; 2],
    pub dead: [f64; 2],
    pub outlook: f64,
    pub side: i32,
}

impl State {
    fn new() -> Self {
        Self { item_gold: [0.0; 2], level: [0.0; 2], cs: [0.0; 2], kills: [0.0; 2], towers: [0; 2], inhibs: [0; 2], drakes: [0; 2], soul: 0, elder: 0, baron: 0, grubs: [0; 2], dead: [0.0; 2], outlook: 0.0, side: 1 }
    }

    pub fn features(&self) -> [f64; 14] {
        let diff = |v: [i32; 2]| f64::from(v[0] - v[1]);
        let [mine, theirs] = self.kills;
        [
            ((self.item_gold[0] + GOLD_PAD) / (self.item_gold[1] + GOLD_PAD)).ln(),
            self.level[0] - self.level[1],
            (self.cs[0] - self.cs[1]) / 100.0,
            (mine - theirs) / (mine + theirs + 4.0).sqrt(),
            diff(self.towers),
            diff(self.inhibs),
            diff(self.drakes.map(|n| n.min(DRAKE_CAP))),
            f64::from(self.soul),
            f64::from(self.elder),
            f64::from(self.baron),
            diff(self.grubs) / GRUBS_COUNT,
            self.dead[1] - self.dead[0],
            self.outlook,
            f64::from(self.side),
        ]
    }
}

pub struct Game {
    pub head: Head,
    pub minutes: Vec<f64>,
    pub states: Vec<State>,
    pub final_items: Vec<BTreeMap<i64, i64>>,
    pub final_gold: Vec<i64>,
    pub odd: BTreeMap<&'static str, u32>,
}

pub enum Parsed<T> {
    Kept(T),
    Skipped(String),
}

fn side_of(team: i64, where_: &dyn Fn() -> String) -> Result<usize, String> {
    match team {
        BLUE => Ok(0),
        RED => Ok(1),
        _ => Err(format!("{}: team {team} is neither {BLUE} nor {RED}", where_())),
    }
}

fn patch_of(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

fn read_head(m: &Match) -> Result<Parsed<Head>, String> {
    let mid = &m.metadata.match_id;
    let info = &m.info;
    if info.participants.len() != 10 {
        return Err(format!("{mid}: {} participants", info.participants.len()));
    }
    let mut champs = [Vec::new(), Vec::new()];
    let mut ids = Vec::new();
    for p in &info.participants {
        let side = side_of(p.team, &|| format!("{mid} participant"))?;
        ids.push(p.id);
        champs[side].push(p.champ);
    }
    ids.sort_unstable();
    if ids != (1..=10).collect::<Vec<i64>>() || champs.iter().any(|c| c.len() != 5) {
        return Err(format!("{mid}: participant ids {ids:?} and sides {:?}", champs.iter().map(Vec::len).collect::<Vec<_>>()));
    }
    let champs = [<[i64; 5]>::try_from(champs[0].as_slice()).unwrap(), <[i64; 5]>::try_from(champs[1].as_slice()).unwrap()];
    if info.participants.iter().any(|p| p.early_surrender || p.team_early_surrendered) {
        return Ok(Parsed::Skipped("remake or early surrender".to_owned()));
    }
    if info.duration < MIN_GAME_S {
        return Ok(Parsed::Skipped("shorter than 5 min".to_owned()));
    }
    let mode = match info.map {
        11 => "rift",
        12 => "aram",
        other => return Err(format!("{mid}: map {other} is neither Summoner's Rift nor Howling Abyss")),
    };
    let mut teams: BTreeMap<usize, bool> = BTreeMap::new();
    for t in &info.teams {
        teams.insert(side_of(t.team, &|| mid.clone())?, t.win);
    }
    if teams.keys().copied().collect::<Vec<_>>() != [0, 1] || teams[&0] == teams[&1] {
        return Err(format!("{mid}: teams {teams:?}"));
    }
    Ok(Parsed::Kept(Head { match_id: mid.clone(), patch: patch_of(&info.version), mode, y: teams[&0], champs, duration_min: info.duration as f64 / 60.0 }))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    Purchased,
    Sold,
    Destroyed,
    Undo,
}

#[derive(Clone, Copy)]
struct ItemEvent {
    ts: i64,
    kind: ItemKind,
    pid: i64,
    item: i64,
    before: i64,
    after: i64,
    at: (usize, usize),
}

struct Inventories<'a> {
    values: &'a Values,
    bought_at: HashMap<(i64, i64), Vec<i64>>,
    possessors: HashSet<i64>,
    held: Vec<HashMap<i64, i64>>,
    buys: Vec<Vec<(i64, i64)>>,
    destroyed: Vec<Vec<(i64, i64)>>,
    odd: BTreeMap<&'static str, u32>,
}

impl Inventories<'_> {
    fn remove(&mut self, pid: usize, item: i64) {
        match self.held[pid].get_mut(&item) {
            Some(n) if *n > 0 => *n -= 1,
            _ if self.values.of(item) > 0 => *self.odd.entry("lost an item of value never given").or_default() += 1,
            _ => {}
        }
    }

    fn give(&mut self, pid: usize, item: i64) {
        *self.held[pid].entry(item).or_default() += 1;
    }

    fn apply(&mut self, ev: &ItemEvent, mid: &str) -> Result<(), String> {
        let pid = ev.pid as usize;
        match ev.kind {
            ItemKind::Purchased => {
                self.give(pid, ev.item);
                self.buys[pid].push((ev.item, ev.ts));
            }
            ItemKind::Sold => self.remove(pid, ev.item),
            ItemKind::Destroyed => {
                let bought = self.bought_at.get(&(ev.pid, ev.ts));
                if self.possessors.contains(&ev.pid) && bought.is_none() {
                    return Ok(());
                }
                let turns = self.values.after.get(&ev.item).copied().filter(|_| bought.is_none_or(|b| b.contains(&ev.item) && self.values.at_once.contains(&ev.item)));
                self.remove(pid, ev.item);
                self.destroyed[pid].push((ev.item, ev.ts));
                if let Some(next) = turns {
                    self.give(pid, next);
                }
            }
            ItemKind::Undo => {
                let (before, after) = (ev.before, ev.after);
                if before != 0 && after == 0 {
                    let Some(at) = self.buys[pid].iter().rposition(|b| b.0 == before) else {
                        *self.odd.entry("undid a purchase never seen").or_default() += 1;
                        return Ok(());
                    };
                    let (_, bought) = self.buys[pid].remove(at);
                    self.remove(pid, before);
                    let (back, kept): (Vec<_>, Vec<_>) = self.destroyed[pid].iter().partition(|x| x.1 == bought);
                    self.destroyed[pid] = kept;
                    for (item, _) in back {
                        self.give(pid, item);
                    }
                } else if after != 0 && before == 0 {
                    self.give(pid, after);
                } else if before == 0 {
                    *self.odd.entry("undo with no item").or_default() += 1;
                } else {
                    return Err(format!("{mid} frame {} event {}: ITEM_UNDO with beforeId={before} and afterId={after}", ev.at.0, ev.at.1));
                }
            }
        }
        Ok(())
    }

    fn gold(&self, pid: usize) -> i64 {
        self.held[pid].iter().map(|(i, n)| self.values.of(*i) * n).sum()
    }

    fn items(&self, pid: usize) -> BTreeMap<i64, i64> {
        self.held[pid].iter().filter(|(_, n)| **n > 0).map(|(i, n)| (*i, *n)).collect()
    }
}

fn final_items(p: &Participant) -> BTreeMap<i64, i64> {
    let mut out = BTreeMap::new();
    for i in [p.item0, p.item1, p.item2, p.item3, p.item4, p.item5, p.item6, p.role_item].into_iter().flatten().filter(|i| *i != 0) {
        *out.entry(i).or_default() += 1;
    }
    out
}

fn adopt(orphans: Vec<ItemEvent>, items: Vec<ItemEvent>, last: &BTreeMap<i64, BTreeMap<i64, i64>>, mid: &str) -> Result<Vec<ItemEvent>, String> {
    let mut first: HashMap<(i64, i64), ItemKind> = HashMap::new();
    for ev in items.iter().filter(|e| e.kind != ItemKind::Undo) {
        first.entry((ev.pid, ev.item)).or_insert(ev.kind);
    }
    let mut given: HashMap<i64, HashSet<i64>> = HashMap::new();
    let mut adopted = Vec::with_capacity(orphans.len() + items.len());
    for mut ev in orphans {
        let item = ev.item;
        let lost = |p: i64| match first.get(&(p, item)) {
            Some(kind) => matches!(kind, ItemKind::Sold | ItemKind::Destroyed),
            None => last[&p].get(&item).copied().unwrap_or(0) > 0,
        };
        let taken = given.entry(item).or_default();
        let Some(taker) = last.keys().copied().find(|p| !taken.contains(p) && lost(*p)) else {
            return Err(format!("{mid} frame {} event {}: item {item} bought by nobody and never lost or kept by a player who did not buy it", ev.at.0, ev.at.1));
        };
        taken.insert(taker);
        ev.pid = taker;
        adopted.push(ev);
    }
    adopted.extend(items);
    adopted.sort_by_key(|e| e.ts);
    Ok(adopted)
}

fn need<T: Copy>(v: Option<T>, field: &str, mid: &str, at: (usize, usize), kind: &str) -> Result<T, String> {
    v.ok_or_else(|| format!("{mid} frame {} event {} ({kind}): no field {field:?}", at.0, at.1))
}

fn check_fields(ev: &Event, mid: &str, at: (usize, usize)) -> Result<(), String> {
    let k = ev.kind.as_str();
    let has = |ok: bool, field: &str| if ok { Ok(()) } else { Err(format!("{mid} frame {} event {} ({k}): no field {field:?}", at.0, at.1)) };
    match k {
        ITEM_PURCHASED | ITEM_SOLD | ITEM_DESTROYED => {
            has(ev.participant.is_some(), "participantId")?;
            has(ev.item.is_some(), "itemId")
        }
        ITEM_UNDO => {
            has(ev.participant.is_some(), "participantId")?;
            has(ev.before.is_some(), "beforeId")?;
            has(ev.after.is_some(), "afterId")
        }
        CHAMPION_KILL => {
            has(ev.killer.is_some(), "killerId")?;
            has(ev.victim.is_some(), "victimId")
        }
        BUILDING_KILL => {
            has(ev.building.is_some(), "buildingType")?;
            has(ev.team.is_some(), "teamId")
        }
        ELITE_MONSTER_KILL => {
            has(ev.monster.is_some(), "monsterType")?;
            has(ev.killer_team.is_some(), "killerTeamId")
        }
        DRAGON_SOUL_GIVEN => has(ev.team.is_some(), "teamId"),
        GAME_END => has(ev.winner.is_some(), "winningTeam"),
        _ if IGNORED_EVENTS.contains(&k) => Ok(()),
        _ => Err(format!("{mid} frame {} event {}: unknown event type {k:?}; add it to riot_format.USED_EVENTS or IGNORED_EVENTS once checked", at.0, at.1)),
    }
}

pub fn parse_game(match_json: &[u8], timeline_json: &[u8], values: &Values) -> Result<Parsed<Game>, String> {
    let m: Match = from_json(match_json, "match")?;
    let tl: Timeline = from_json(timeline_json, &format!("{} timeline", m.metadata.match_id))?;
    game_of(&m, &tl, values)
}

fn from_json<T: DeserializeOwned>(bytes: &[u8], what: &str) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|err| format!("{what}: {err}"))
}

fn game_of(m: &Match, tl: &Timeline, values: &Values) -> Result<Parsed<Game>, String> {
    let head = match read_head(m)? {
        Parsed::Kept(head) => head,
        Parsed::Skipped(why) => return Ok(Parsed::Skipped(why)),
    };
    let mid = head.match_id.as_str();
    let y = head.y;
    if tl.metadata.match_id != mid {
        return Err(format!("{mid}: timeline is for {}", tl.metadata.match_id));
    }
    if tl.info.interval != FRAME_MS {
        return Err(format!("{mid}: frame interval {}, expected {FRAME_MS}", tl.info.interval));
    }
    let frames = &tl.info.frames;
    let mut players: Vec<[(i64, i64); 10]> = Vec::with_capacity(frames.len());
    for (fi, f) in frames.iter().enumerate() {
        let bad = || format!("{mid} frame {fi}: participantFrames keys {:?}", f.participants.keys().collect::<Vec<_>>());
        let mut row = [(0, 0); 10];
        for (key, slot) in PIDS.iter().zip(row.iter_mut()) {
            let p = f.participants.get(*key).ok_or_else(bad)?;
            *slot = (p.level, p.minions + p.jungle);
        }
        if f.participants.len() != 10 {
            return Err(bad());
        }
        players.push(row);
    }
    if frames.windows(2).any(|w| w[0].timestamp > w[1].timestamp) {
        return Err(format!("{mid}: frames out of order"));
    }
    let mut events: Vec<(i64, (usize, usize), &Event)> = Vec::new();
    for (fi, f) in frames.iter().enumerate() {
        for (ei, ev) in f.events.iter().enumerate() {
            check_fields(ev, mid, (fi, ei))?;
            events.push((ev.timestamp, (fi, ei), ev));
        }
    }
    events.sort_by_key(|e| e.0);

    let mut side = [0usize; 11];
    for p in &m.info.participants {
        side[p.id as usize] = side_of(p.team, &|| mid.to_owned())?;
    }
    let pid_ok = |pid: i64, at: (usize, usize)| if (1..=10).contains(&pid) { Ok(pid) } else { Err(format!("{mid} frame {} event {}: participant {pid}", at.0, at.1)) };
    let (mut kills, mut towers, mut inhibs, mut dragons, mut barons, mut grubs, mut souls, mut end) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), None);
    let (mut items, mut orphans) = (Vec::new(), Vec::new());
    for &(ts, at, ev) in &events {
        let kind = ev.kind.as_str();
        let t = ts as f64 / 1000.0;
        let item_kind = match kind {
            ITEM_PURCHASED => Some(ItemKind::Purchased),
            ITEM_SOLD => Some(ItemKind::Sold),
            ITEM_DESTROYED => Some(ItemKind::Destroyed),
            ITEM_UNDO => Some(ItemKind::Undo),
            _ => None,
        };
        if let Some(item_kind) = item_kind {
            let pid = need(ev.participant, "participantId", mid, at, kind)?;
            let item = ItemEvent { ts, kind: item_kind, pid, item: ev.item.unwrap_or(0), before: ev.before.unwrap_or(0), after: ev.after.unwrap_or(0), at };
            if item_kind == ItemKind::Purchased && pid == NOBODY {
                orphans.push(item);
            } else {
                pid_ok(pid, at)?;
                items.push(item);
            }
            continue;
        }
        match kind {
            CHAMPION_KILL => {
                let killer = need(ev.killer, "killerId", mid, at, kind)?;
                let victim = pid_ok(need(ev.victim, "victimId", mid, at, kind)?, at)?;
                if killer != 0 {
                    pid_ok(killer, at)?;
                }
                kills.push((t, killer, victim));
            }
            BUILDING_KILL => {
                let b = ev.building.as_deref().unwrap_or_default();
                if b != TOWER && b != INHIBITOR {
                    return Err(format!("{mid} frame {} event {}: building {b:?}", at.0, at.1));
                }
                let lost = side_of(need(ev.team, "teamId", mid, at, kind)?, &|| format!("{mid} frame {} event {}", at.0, at.1))?;
                if b == TOWER { towers.push((t, lost)) } else { inhibs.push((t, lost)) }
            }
            ELITE_MONSTER_KILL => {
                let monster = ev.monster.as_deref().unwrap_or_default();
                let killer_team = need(ev.killer_team, "killerTeamId", mid, at, kind)?;
                if IGNORED_MONSTERS.contains(&monster) || (monster == GRUB && killer_team == NEUTRAL) {
                    continue;
                }
                let taker = side_of(killer_team, &|| format!("{mid} frame {} event {}", at.0, at.1))?;
                match monster {
                    DRAGON => {
                        let sub = ev.monster_sub.as_deref().ok_or_else(|| format!("{mid} frame {} event {}: no field \"monsterSubType\"", at.0, at.1))?;
                        if sub != ELDER && !ELEMENTAL_DRAGONS.contains(&sub) {
                            return Err(format!("{mid} frame {} event {}: dragon {sub:?}", at.0, at.1));
                        }
                        dragons.push((t, taker, sub == ELDER));
                    }
                    BARON => barons.push((t, taker)),
                    GRUB => grubs.push((t, taker)),
                    _ => return Err(format!("{mid} frame {} event {}: monster {monster:?}", at.0, at.1)),
                }
            }
            DRAGON_SOUL_GIVEN => {
                let team = need(ev.team, "teamId", mid, at, kind)?;
                if team != NOBODY {
                    souls.push(side_of(team, &|| format!("{mid} frame {} event {}", at.0, at.1))?);
                }
            }
            GAME_END => {
                let winner = side_of(need(ev.winner, "winningTeam", mid, at, kind)?, &|| format!("{mid} frame {} event {}", at.0, at.1))?;
                if winner != if y { 0 } else { 1 } {
                    return Err(format!("{mid} frame {} event {}: GAME_END winner disagrees with match teams", at.0, at.1));
                }
                end = Some(t);
            }
            _ => {}
        }
    }
    for s in &souls {
        if dragons.iter().filter(|(_, taker, elder)| taker == s && !elder).count() < SOUL_DRAGONS as usize {
            return Err(format!("{mid}: DRAGON_SOUL_GIVEN for side {s} with fewer than {SOUL_DRAGONS} dragons"));
        }
    }
    let last: BTreeMap<i64, BTreeMap<i64, i64>> = m.info.participants.iter().map(|p| (p.id, final_items(p))).collect();
    let items = adopt(orphans, items, &last, mid)?;
    let mut bought_at: HashMap<(i64, i64), Vec<i64>> = HashMap::new();
    for ev in items.iter().filter(|e| e.kind == ItemKind::Purchased) {
        bought_at.entry((ev.pid, ev.ts)).or_default().push(ev.item);
    }
    let possessors = m.info.participants.iter().filter(|p| p.champ == POSSESSOR).map(|p| p.id).collect();
    let mut inv = Inventories { values, bought_at, possessors, held: vec![HashMap::new(); 11], buys: vec![Vec::new(); 11], destroyed: vec![Vec::new(); 11], odd: BTreeMap::new() };

    let frame_s: Vec<f64> = frames.iter().map(|f| f.timestamp as f64 / 1000.0).collect();
    let mut deaths = Vec::with_capacity(kills.len());
    for (kt, killer, victim) in kills {
        let at = frame_s.partition_point(|ft| *ft <= kt).checked_sub(1).ok_or_else(|| format!("{mid}: a kill before the first frame"))?;
        let level = players[at][victim as usize - 1].0;
        deaths.push((kt, killer, victim, kt + respawn(level, kt)));
    }
    let rows: Vec<usize> = match end {
        None => (0..frames.len().saturating_sub(1)).collect(),
        Some(end) => (0..frames.len()).filter(|&i| frame_s[i] < end).collect(),
    };
    let (mut minutes, mut states, mut done) = (Vec::with_capacity(rows.len()), Vec::with_capacity(rows.len()), 0);
    for fi in rows {
        let f = &frames[fi];
        let t = frame_s[fi];
        while done < items.len() && items[done].ts <= f.timestamp {
            inv.apply(&items[done], mid)?;
            done += 1;
        }
        let mut s = State::new();
        for pid in 1..=10 {
            let (i, (level, cs)) = (side[pid], players[fi][pid - 1]);
            s.item_gold[i] += inv.gold(pid) as f64;
            s.level[i] += level as f64 / 5.0;
            s.cs[i] += cs as f64;
        }
        for &(kt, killer, victim, back) in &deaths {
            if kt > t {
                continue;
            }
            if killer != 0 {
                s.kills[side[killer as usize]] += 1.0;
            }
            let left = back - t;
            if left > 0.0 {
                s.dead[side[victim as usize]] += left.min(DEAD_CAP) / DEAD_CAP;
            }
        }
        for &(bt, lost) in &towers {
            if bt <= t {
                s.towers[1 - lost] += 1;
            }
        }
        for &(bt, lost) in &inhibs {
            if bt <= t && t < bt + INHIB_RESPAWN {
                s.inhibs[1 - lost] += 1;
            }
        }
        for &(dt, taker, elder) in &dragons {
            if dt > t {
                continue;
            }
            if !elder {
                s.drakes[taker] += 1;
            } else if t < dt + ELDER_BUFF {
                s.elder = if taker == 0 { 1 } else { -1 };
            }
        }
        for &(bt, taker) in &barons {
            if bt <= t && t < bt + BARON_BUFF {
                s.baron = if taker == 0 { 1 } else { -1 };
            }
        }
        for &(gt, taker) in &grubs {
            if gt <= t {
                s.grubs[taker] += 1;
            }
        }
        s.soul = i32::from(s.drakes[0] >= SOUL_DRAGONS) - i32::from(s.drakes[1] >= SOUL_DRAGONS);
        minutes.push(t / 60.0);
        states.push(s);
    }
    while done < items.len() {
        inv.apply(&items[done], mid)?;
        done += 1;
    }
    let final_items = (0..11).map(|pid| inv.items(pid)).collect();
    let final_gold = (0..11).map(|pid| inv.gold(pid)).collect();
    Ok(Parsed::Kept(Game { head, minutes, states, final_items, final_gold, odd: inv.odd }))
}

pub struct Args {
    pub raw: PathBuf,
    pub out: PathBuf,
    pub patch: String,
    pub items: PathBuf,
    pub bands: Option<PathBuf>,
}

const USAGE: &str = "usage: lol-coach-server features --raw <dir> --out <rows.npz> --patch <patch> --items <item.json> [--bands <patch dir>]";

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let (mut raw, mut out, mut patch, mut items, mut bands) = (None, None, None, None, None);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "--raw" => &mut raw,
            "--out" => &mut out,
            "--patch" => &mut patch,
            "--items" => &mut items,
            "--bands" => &mut bands,
            _ => return Err(format!("unknown argument {arg:?}\n{USAGE}")),
        };
        *slot = Some(args.next().ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))?);
    }
    let need = |v: Option<String>, name: &str| v.ok_or_else(|| format!("{name} is missing\n{USAGE}"));
    Ok(Args { raw: need(raw, "--raw")?.into(), out: need(out, "--out")?.into(), patch: need(patch, "--patch")?, items: need(items, "--items")?.into(), bands: bands.map(PathBuf::from) })
}

pub fn run(args: impl IntoIterator<Item = String>) -> ExitCode {
    match parse_args(args).and_then(|a| features(&a)) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("features: {err}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Deserialize)]
struct WithBands {
    #[serde(default)]
    bands: Option<Value>,
}

pub fn read_bands(dir: &Path) -> HashMap<String, String> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json") && p.is_file()).collect();
    paths.sort();
    let mut out = HashMap::new();
    for path in paths {
        let parsed = fs::read(&path).map_err(|err| err.to_string()).and_then(|bytes| serde_json::from_slice::<WithBands>(&bytes).map_err(|err| err.to_string()));
        match parsed {
            Ok(WithBands { bands: Some(Value::Object(map)) }) => {
                for (id, band) in map {
                    if let Value::String(band) = band
                        && !band.is_empty()
                    {
                        out.entry(id).or_insert(band);
                    }
                }
            }
            Ok(_) => {}
            Err(err) => eprintln!("features: {} has no bands I can read ({err}); its games count as {TOP_BAND}", path.display()),
        }
    }
    out
}

fn raw_pairs(raw: &Path) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = fs::read_dir(raw)
        .map_err(|err| format!("{}: {err}", raw.display()))?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".match.json.gz") && raw.join(n.replace(".match.", ".timeline.")).exists())
        .collect();
    names.sort();
    Ok(names)
}

fn gunzip(path: &Path, buf: &mut Vec<u8>) -> Result<(), String> {
    buf.clear();
    let file = File::open(path).map_err(|err| format!("{}: {err}", path.display()))?;
    MultiGzDecoder::new(BufReader::with_capacity(1 << 16, file)).read_to_end(buf).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(())
}

struct Kept {
    file: u32,
    id: String,
    champs: [[i64; 5]; 2],
    y: bool,
    bucket: u8,
    test: bool,
    band: u8,
    rows: u32,
}

fn counts(list: &[(String, u64)]) -> String {
    let inner: Vec<String> = list.iter().map(|(k, n)| format!("'{k}': {n}")).collect();
    format!("{{{}}}", inner.join(", "))
}

fn count(list: &mut Vec<(String, u64)>, key: &str) {
    match list.iter_mut().find(|(k, _)| k == key) {
        Some((_, n)) => *n += 1,
        None => list.push((key.to_owned(), 1)),
    }
}

fn features(a: &Args) -> Result<String, String> {
    let values = item_values(&from_json::<Value>(&fs::read(&a.items).map_err(io_err(&a.items))?, &a.items.display().to_string())?)?;
    let bands = a.bands.as_deref().map(read_bands).unwrap_or_default();
    let names = raw_pairs(&a.raw)?;
    if names.is_empty() {
        return Err(format!("no crawled games in {}", a.raw.display()));
    }
    let mut band_names: Vec<String> = vec![TOP_BAND.to_owned()];
    let (mut kept, mut skipped, mut buf) = (Vec::new(), Vec::new(), Vec::new());
    for (n, name) in names.iter().enumerate() {
        gunzip(&a.raw.join(name), &mut buf)?;
        let m: Match = from_json(&buf, name)?;
        let patch = patch_of(&m.info.version);
        if patch != a.patch {
            count(&mut skipped, &format!("patch {patch}"));
            continue;
        }
        match read_head(&m)? {
            Parsed::Skipped(why) => count(&mut skipped, &why),
            Parsed::Kept(head) if head.mode != "rift" => count(&mut skipped, &format!("mode {}", head.mode)),
            Parsed::Kept(head) => {
                let band = bands.get(&head.match_id).map_or(TOP_BAND, String::as_str);
                let band = match band_names.iter().position(|b| b == band) {
                    Some(i) => i,
                    None => {
                        band_names.push(band.to_owned());
                        band_names.len() - 1
                    }
                };
                let band = u8::try_from(band).map_err(|_| "more than 255 bands".to_owned())?;
                kept.push(Kept { file: n as u32, test: is_test(&head.match_id), bucket: head.bucket() as u8, y: head.y, champs: head.champs, band, rows: 0, id: head.match_id });
            }
        }
    }
    drop(bands);
    let mut tables = Tables::new();
    for g in kept.iter().filter(|g| !g.test) {
        add_to_tables(&mut tables, &g.champs, g.y, usize::from(g.bucket));
    }
    let train_games = kept.iter().filter(|g| !g.test).count();

    let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, MAX_WORKERS);
    let per = kept.len().div_ceil(workers).max(1);
    let tmps: Vec<PathBuf> = (0..kept.len().div_ceil(per)).map(|i| a.out.with_extension(format!("rows{i}.tmp"))).collect();
    let part = a.out.with_extension("npz.part");
    let (games, read, failed) = (kept.len(), AtomicUsize::new(0), AtomicBool::new(false));
    let shared = Pass { raw: &a.raw, names: &names, values: &values, tables: &tables, games, read: &read, failed: &failed };
    let parts: Vec<Result<usize, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = kept.chunks_mut(per).zip(&tmps).map(|(chunk, tmp)| scope.spawn(|| write_part(&shared, chunk, tmp))).collect();
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a worker panicked".to_owned()))).collect()
    });
    let result = parts.into_iter().sum::<Result<usize, String>>().and_then(|total| {
        write_rows(&part, &tmps, &kept, total, train_games, &a.patch, &band_names).map_err(io_err(&part))?;
        fs::rename(&part, &a.out).map_err(io_err(&a.out))?;
        Ok(total)
    });
    for tmp in &tmps {
        let _ = fs::remove_file(tmp);
    }
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    let total = result?;
    let mut by_band = vec![0u64; band_names.len()];
    for g in &kept {
        by_band[usize::from(g.band)] += 1;
    }
    let by_band: Vec<(String, u64)> = band_names.into_iter().zip(by_band).filter(|(_, n)| *n > 0).collect();
    Ok(format!("{} games ({train_games} training), {total} rows to {}; left out: {}; bands: {}", kept.len(), a.out.display(), counts(&skipped), counts(&by_band)))
}

struct Pass<'a> {
    raw: &'a Path,
    names: &'a [String],
    values: &'a Values,
    tables: &'a Tables,
    games: usize,
    read: &'a AtomicUsize,
    failed: &'a AtomicBool,
}

fn write_part(pass: &Pass, chunk: &mut [Kept], tmp: &Path) -> Result<usize, String> {
    let result = write_part_rows(pass, chunk, tmp);
    if result.is_err() {
        pass.failed.store(true, Ordering::Relaxed);
    }
    result
}

fn write_part_rows(pass: &Pass, chunk: &mut [Kept], tmp: &Path) -> Result<usize, String> {
    let mut rows_file = BufWriter::with_capacity(1 << 20, File::create(tmp).map_err(io_err(tmp))?);
    let (mut buf, mut tl_buf, mut total) = (Vec::new(), Vec::new(), 0);
    for g in chunk {
        if pass.failed.load(Ordering::Relaxed) {
            return Ok(0);
        }
        let name = &pass.names[g.file as usize];
        gunzip(&pass.raw.join(name), &mut buf)?;
        gunzip(&pass.raw.join(name.replace(".match.", ".timeline.")), &mut tl_buf)?;
        let m: Match = from_json(&buf, name)?;
        let tl: Timeline = from_json(&tl_buf, &format!("{} timeline", m.metadata.match_id))?;
        let Parsed::Kept(game) = game_of(&m, &tl, pass.values)? else { return Err(format!("{name}: left out on the second read")) };
        if game.head.match_id != g.id {
            return Err(format!("{name}: changed between the two reads"));
        }
        let mut row = [0u8; COLUMNS * 8];
        for (minute, s) in game.minutes.iter().zip(&game.states) {
            let mut x = s.features();
            x[OUTLOOK] = outlook(&g.champs, g.y, usize::from(g.bucket), *minute, pass.tables, !g.test);
            row[..8].copy_from_slice(&minute.to_le_bytes());
            for (k, v) in x.iter().enumerate() {
                row[8 + 8 * k..16 + 8 * k].copy_from_slice(&v.to_le_bytes());
            }
            rows_file.write_all(&row).map_err(io_err(tmp))?;
        }
        g.rows = game.states.len() as u32;
        total += game.states.len();
        let read = pass.read.fetch_add(1, Ordering::Relaxed) + 1;
        if read.is_multiple_of(PROGRESS) {
            println!("{read} of {} games read", pass.games);
        }
    }
    rows_file.flush().map_err(io_err(tmp))?;
    Ok(total)
}

fn write_rows(path: &Path, tmps: &[PathBuf], kept: &[Kept], total: usize, train_games: usize, patch: &str, band_names: &[String]) -> std::io::Result<()> {
    let mut npz = Npz::create(path)?;
    npz.text("mode", &["rift"], true)?;
    npz.text("patch", &[patch], true)?;
    npz.text("source", &["riot"], true)?;
    let per_row = |npz: &mut Npz, name: &str, descr: &str, cell: &dyn Fn(usize, &Kept) -> Vec<u8>| -> std::io::Result<()> {
        let mut member = npz.member(name, descr, &[total])?;
        for (gi, g) in kept.iter().enumerate() {
            let one = cell(gi, g);
            member.write(&one.repeat(g.rows as usize))?;
        }
        member.end()
    };
    per_row(&mut npz, "game", "<i8", &|gi, _| (gi as i64).to_le_bytes().to_vec())?;
    columns(&mut npz, tmps, "minute", &[total], 0..1)?;
    per_row(&mut npz, "y", "<f8", &|_, g| f64::from(u8::from(g.y)).to_le_bytes().to_vec())?;
    columns(&mut npz, tmps, "X", &[total, KEYS.len()], 1..COLUMNS)?;
    per_row(&mut npz, "test", "|b1", &|_, g| vec![u8::from(g.test)])?;
    per_row(&mut npz, "fold", "|i1", &|_, g| vec![fold(&g.id) as u8])?;
    let ids: Vec<&str> = kept.iter().map(|g| g.id.as_str()).collect();
    npz.text("match_ids", &ids, false)?;
    npz.text("keys", &KEYS, false)?;
    npz.array("train_games", "<i8", &[], &(train_games as i64).to_le_bytes())?;
    let bands: Vec<&str> = kept.iter().map(|g| band_names[usize::from(g.band)].as_str()).collect();
    npz.text("band", &bands, false)?;
    npz.finish()
}

fn columns(npz: &mut Npz, tmps: &[PathBuf], name: &str, shape: &[usize], cols: std::ops::Range<usize>) -> std::io::Result<()> {
    let mut member = npz.member(name, "<f8", shape)?;
    let mut row = [0u8; COLUMNS * 8];
    let (mut chunk, mut rows) = (Vec::with_capacity(1 << 16), 0);
    for tmp in tmps {
        let mut reader = BufReader::with_capacity(1 << 20, File::open(tmp)?);
        while reader.read_exact(&mut row).is_ok() {
            rows += 1;
            chunk.extend_from_slice(&row[cols.start * 8..cols.end * 8]);
            if chunk.len() >= 1 << 16 {
                member.write(&chunk)?;
                chunk.clear();
            }
        }
    }
    if rows != shape[0] {
        return Err(std::io::Error::other(format!("{rows} rows in the scratch files, expected {}", shape[0])));
    }
    member.write(&chunk)?;
    member.end()
}

fn io_err(what: &Path) -> impl Fn(std::io::Error) -> String + '_ {
    move |err| format!("{}: {err}", what.display())
}
