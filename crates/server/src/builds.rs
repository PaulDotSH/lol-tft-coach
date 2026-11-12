use crate::players::write_atomic;
use crate::riot::region_of;
use flate2::read::GzDecoder;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const MIN_GAME_S: u64 = 900;
pub const MIN_PAIR_GAMES: u32 = 100;
pub const FIRST_SHARE: f64 = 0.05;
pub const FIRST_MAX: usize = 4;
pub const NEXT_SHARE: f64 = 0.10;
pub const NEXT_MAX: usize = 3;
pub const BACK_FROM_MS: u64 = 120_000;
pub const BACK_WINDOW_MS: u64 = 30_000;
pub const BACK_ITEMS: usize = 5;
pub const SITUATION_GAMES: u32 = 150;
pub const LEAD_CUT: f64 = 0.10;
pub const FINISHED_GOLD: u32 = 2000;
const RIFT: u32 = 11;
const RIFT_MAP: &str = "11";
const BASIC_BOOTS: &str = "1001";
const BOOTS_TAG: &str = "Boots";
const CONSUMABLE_TAG: &str = "Consumable";
const TRINKET_TAG: &str = "Trinket";
const MAX_ITEM_ID: u32 = 200_000;
const ITEM_PURCHASED: &str = "ITEM_PURCHASED";
const ITEM_UNDO: &str = "ITEM_UNDO";
const TOP_BAND: &str = "top";

#[derive(Deserialize)]
pub struct MatchFile {
    pub metadata: Meta,
    pub info: MatchInfo,
}

#[derive(Deserialize)]
pub struct Meta {
    #[serde(rename = "matchId")]
    pub match_id: String,
}

#[derive(Deserialize)]
pub struct MatchInfo {
    #[serde(rename = "gameVersion")]
    pub game_version: String,
    #[serde(rename = "mapId")]
    pub map_id: u32,
    #[serde(rename = "gameDuration")]
    pub game_duration: u64,
    pub participants: Vec<Participant>,
}

#[derive(Deserialize)]
pub struct Participant {
    #[serde(rename = "participantId")]
    pub participant_id: u32,
    #[serde(rename = "teamId")]
    pub team_id: u32,
    #[serde(rename = "championId")]
    pub champion_id: u32,
    #[serde(rename = "teamPosition", default)]
    pub team_position: String,
    #[serde(rename = "gameEndedInEarlySurrender", default)]
    pub early_surrender: bool,
    #[serde(rename = "teamEarlySurrendered", default)]
    pub team_early_surrendered: bool,
    #[serde(rename = "magicDamageDealtToChampions", default)]
    pub magic_damage: u64,
    #[serde(rename = "physicalDamageDealtToChampions", default)]
    pub physical_damage: u64,
}

#[derive(Deserialize)]
pub struct TimelineFile {
    pub metadata: Meta,
    pub info: TimelineInfo,
}

#[derive(Deserialize)]
pub struct TimelineInfo {
    pub frames: Vec<Frame>,
}

#[derive(Deserialize)]
pub struct Frame {
    pub timestamp: u64,
    #[serde(rename = "participantFrames")]
    pub participants: HashMap<String, ParticipantFrame>,
    pub events: Vec<Event>,
}

#[derive(Deserialize)]
pub struct ParticipantFrame {
    #[serde(rename = "totalGold")]
    pub total_gold: i64,
    #[serde(rename = "currentGold")]
    pub current_gold: i64,
}

#[derive(Deserialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub kind: String,
    pub timestamp: u64,
    #[serde(rename = "participantId", default)]
    pub participant: u32,
    #[serde(rename = "itemId", default)]
    pub item: u32,
    #[serde(rename = "beforeId", default)]
    pub before: u32,
    #[serde(rename = "afterId", default)]
    pub after: u32,
}

#[derive(Deserialize)]
struct ItemFile {
    data: HashMap<String, ItemData>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ItemData {
    gold: Gold,
    maps: HashMap<String, bool>,
    from: Vec<String>,
    into: Vec<String>,
    tags: Vec<String>,
    #[serde(rename = "requiredChampion")]
    required_champion: Option<String>,
    #[serde(rename = "requiredAlly")]
    required_ally: Option<String>,
    #[serde(rename = "inStore")]
    in_store: Option<bool>,
    #[serde(rename = "hideFromAll")]
    hide_from_all: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Gold {
    total: u32,
    purchasable: bool,
}

#[derive(Default, Debug)]
pub struct Items {
    pub finished: HashSet<u32>,
    pub boots: HashSet<u32>,
    pub counted: HashSet<u32>,
}

impl Items {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let file: ItemFile = serde_json::from_slice(bytes).map_err(|err| format!("item file: {err}"))?;
        let shop = |id: &str| {
            file.data.get(id).is_some_and(|d| {
                id.parse::<u32>().is_ok_and(|n| n < MAX_ITEM_ID)
                    && d.gold.purchasable
                    && d.maps.get(RIFT_MAP) == Some(&true)
                    && d.required_champion.is_none()
                    && d.required_ally.is_none()
                    && d.in_store != Some(false)
                    && d.hide_from_all != Some(true)
                    && !d.tags.iter().any(|t| t == TRINKET_TAG)
            })
        };
        let mut items = Self::default();
        for (key, d) in &file.data {
            let Some(id) = key.parse::<u32>().ok().filter(|_| shop(key)) else { continue };
            let boots = d.tags.iter().any(|t| t == BOOTS_TAG);
            if !d.tags.iter().any(|t| t == CONSUMABLE_TAG) {
                items.counted.insert(id);
            }
            if boots && d.from.iter().any(|f| f == BASIC_BOOTS) {
                items.boots.insert(id);
            } else if !boots && !d.from.is_empty() && d.gold.total >= FINISHED_GOLD && !d.into.iter().any(|i| shop(i)) {
                items.finished.insert(id);
            }
        }
        Ok(items)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Purchase {
    pub item: u32,
    pub ms: u64,
}

pub fn purchases(timeline: &TimelineFile, pid: u32) -> Vec<Purchase> {
    let mut events: Vec<&Event> = timeline.info.frames.iter().flat_map(|f| &f.events).filter(|e| e.participant == pid && (e.kind == ITEM_PURCHASED || e.kind == ITEM_UNDO)).collect();
    events.sort_by_key(|e| e.timestamp);
    let mut out: Vec<Purchase> = Vec::new();
    for e in events {
        if e.kind == ITEM_PURCHASED {
            out.push(Purchase { item: e.item, ms: e.timestamp });
        } else if e.before != 0 && e.after == 0
            && let Some(at) = out.iter().rposition(|p| p.item == e.before)
        {
            out.remove(at);
        }
    }
    out
}

#[derive(Debug, Default, PartialEq)]
pub struct PlayerBuild {
    pub finished: Vec<Purchase>,
    pub boots: Option<u64>,
    pub back: Option<(u64, Vec<u32>)>,
}

pub fn player_build(items: &Items, buys: &[Purchase]) -> PlayerBuild {
    let mut build = PlayerBuild::default();
    for p in buys {
        if items.finished.contains(&p.item) && !build.finished.iter().any(|f| f.item == p.item) {
            build.finished.push(*p);
        }
        if build.boots.is_none() && items.boots.contains(&p.item) {
            build.boots = Some(p.ms);
        }
    }
    if let Some(first) = buys.iter().find(|p| p.ms >= BACK_FROM_MS) {
        let mut bought: Vec<u32> = buys.iter().filter(|p| p.ms >= first.ms && p.ms <= first.ms + BACK_WINDOW_MS && items.counted.contains(&p.item)).map(|p| p.item).collect();
        bought.sort_unstable();
        bought.dedup();
        build.back = Some((first.ms, bought));
    }
    build
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Situation {
    Physical,
    Mixed,
    Magic,
    Behind,
    Even,
    Ahead,
}

impl Situation {
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Physical => ("damage", "physical"),
            Self::Mixed => ("damage", "mixed"),
            Self::Magic => ("damage", "magic"),
            Self::Behind => ("lead", "behind"),
            Self::Even => ("lead", "even"),
            Self::Ahead => ("lead", "ahead"),
        }
    }
}

pub fn damage_situation(value: f64, cuts: [f64; 2]) -> Situation {
    if value < cuts[0] {
        Situation::Physical
    } else if value < cuts[1] {
        Situation::Mixed
    } else {
        Situation::Magic
    }
}

pub fn lead_situation(mine: i64, theirs: i64) -> Situation {
    let (mine, theirs) = (mine as f64, theirs as f64);
    if mine == theirs {
        Situation::Even
    } else if mine <= theirs * (1.0 - LEAD_CUT) {
        Situation::Behind
    } else if mine >= theirs * (1.0 + LEAD_CUT) {
        Situation::Ahead
    } else {
        Situation::Even
    }
}

#[derive(Default)]
pub struct Dist(BTreeMap<u32, u32>);

impl Dist {
    pub fn add(&mut self, value: u32) {
        *self.0.entry(value).or_default() += 1;
    }

    pub fn count(&self) -> u32 {
        self.0.values().sum()
    }

    pub fn percentiles(&self) -> Option<[u32; 3]> {
        let n = self.count();
        if n == 0 {
            return None;
        }
        let at = |rank: u32| {
            let mut seen = 0;
            for (&value, &count) in &self.0 {
                seen += count;
                if rank < seen {
                    return value as f64;
                }
            }
            0.0
        };
        let q = |q: f64| {
            let pos = q * f64::from(n - 1);
            let (lo, hi) = (pos.floor(), pos.ceil());
            let (a, b) = (at(lo as u32), at(hi as u32));
            (a + (b - a) * (pos - lo)).round() as u32
        };
        Some([q(0.25), q(0.5), q(0.75)])
    }
}

fn secs(ms: u64) -> u32 {
    (ms / 1000) as u32
}

#[derive(Default)]
struct Acc {
    games: u32,
    slots: [Dist; 4],
    first: HashMap<u32, Dist>,
    next: [HashMap<u32, HashMap<u32, u32>>; 2],
    back_time: Dist,
    back_gold: Dist,
    back_items: HashMap<u32, u32>,
    situations: HashMap<(usize, u32, Situation), HashMap<u32, u32>>,
}

struct Seen<'a> {
    build: &'a PlayerBuild,
    back_gold: Option<i64>,
    damage: Situation,
    leads: [Option<Situation>; 3],
}

impl Acc {
    fn add(&mut self, s: &Seen) {
        self.games += 1;
        let b = s.build;
        for (slot, p) in b.finished.iter().take(3).enumerate() {
            self.slots[slot].add(secs(p.ms));
        }
        if let Some(ms) = b.boots {
            self.slots[3].add(secs(ms));
        }
        if let Some(start) = b.finished.first() {
            self.first.entry(start.item).or_default().add(secs(start.ms));
            for slot in 1..3 {
                let Some(p) = b.finished.get(slot) else { continue };
                *self.next[slot - 1].entry(start.item).or_default().entry(p.item).or_default() += 1;
                for situation in std::iter::once(s.damage).chain(s.leads[slot]) {
                    *self.situations.entry((slot, start.item, situation)).or_default().entry(p.item).or_default() += 1;
                }
            }
        }
        if let (Some((ms, bought)), Some(gold)) = (&b.back, s.back_gold) {
            self.back_time.add(secs(*ms));
            self.back_gold.add(gold.max(0) as u32);
            for item in bought {
                *self.back_items.entry(*item).or_default() += 1;
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Pick {
    pub item: u32,
    pub share: f64,
    pub games: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub time: Option<[u32; 3]>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Slots {
    pub first: Option<[u32; 3]>,
    pub second: Option<[u32; 3]>,
    pub third: Option<[u32; 3]>,
    pub boots: Option<[u32; 3]>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Back {
    pub games: u32,
    pub time: [u32; 3],
    pub gold: [u32; 3],
    pub items: Vec<Pick>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SituationPick {
    pub slot: u8,
    pub start: u32,
    pub kind: String,
    pub value: String,
    pub item: u32,
    pub share: f64,
    pub games: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Table {
    pub games: u32,
    pub slots: Slots,
    pub first: Vec<Pick>,
    pub second: BTreeMap<u32, Vec<Pick>>,
    pub third: BTreeMap<u32, Vec<Pick>>,
    pub back: Option<Back>,
    pub situations: Vec<SituationPick>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Pair {
    pub champion: u32,
    pub role: String,
    pub all: Table,
    pub top: Option<Table>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Output {
    pub patch: String,
    pub games: u32,
    pub bands: BTreeMap<String, u32>,
    pub finished: Vec<u32>,
    pub boots: Vec<u32>,
    pub magic_share: BTreeMap<u32, f64>,
    pub damage_cuts: [f64; 2],
    pub lead_cut: f64,
    pub pairs: Vec<Pair>,
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn ranked(counts: &HashMap<u32, u32>) -> Vec<(u32, u32)> {
    let mut list: Vec<(u32, u32)> = counts.iter().map(|(&i, &n)| (i, n)).collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    list
}

fn picks(counts: &HashMap<u32, u32>, total: u32, min_share: f64, max: usize) -> Vec<Pick> {
    ranked(counts)
        .into_iter()
        .filter(|&(_, n)| total > 0 && f64::from(n) >= min_share * f64::from(total))
        .take(max)
        .map(|(item, n)| Pick { item, share: round3(f64::from(n) / f64::from(total)), games: n, time: None })
        .collect()
}

impl Acc {
    fn table(&self) -> Table {
        let starts: HashMap<u32, u32> = self.first.iter().map(|(&i, d)| (i, d.count())).collect();
        let mut first = picks(&starts, self.slots[0].count(), FIRST_SHARE, FIRST_MAX);
        for pick in &mut first {
            pick.time = self.first.get(&pick.item).and_then(Dist::percentiles);
        }
        let next = |slot: usize| -> BTreeMap<u32, Vec<Pick>> {
            first.iter().filter_map(|start| self.next[slot].get(&start.item).map(|counts| (start.item, picks(counts, start.games, NEXT_SHARE, NEXT_MAX)))).collect()
        };
        let mut situations = Vec::new();
        for start in &first {
            for slot in 1..3 {
                let Some(overall) = self.next[slot - 1].get(&start.item).and_then(|c| ranked(c).first().map(|x| x.0)) else { continue };
                let mut keys: Vec<&(usize, u32, Situation)> = self.situations.keys().filter(|(s, i, _)| *s == slot && *i == start.item).collect();
                keys.sort();
                for key in keys {
                    let counts = &self.situations[key];
                    let total: u32 = counts.values().sum();
                    let Some(&(item, n)) = ranked(counts).first() else { continue };
                    if total >= SITUATION_GAMES && item != overall {
                        let (kind, value) = key.2.names();
                        situations.push(SituationPick { slot: slot as u8 + 1, start: start.item, kind: kind.to_owned(), value: value.to_owned(), item, share: round3(f64::from(n) / f64::from(total)), games: total });
                    }
                }
            }
        }
        let back_games = self.back_time.count();
        let back = self.back_time.percentiles().zip(self.back_gold.percentiles()).map(|(time, gold)| Back { games: back_games, time, gold, items: picks(&self.back_items, back_games, 0.0, BACK_ITEMS) });
        Table {
            games: self.games,
            slots: Slots { first: self.slots[0].percentiles(), second: self.slots[1].percentiles(), third: self.slots[2].percentiles(), boots: self.slots[3].percentiles() },
            second: next(0),
            third: next(1),
            first,
            back,
            situations,
        }
    }
}

pub fn patch_of(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

pub fn skip_reason(m: &MatchFile, patch: &str) -> Option<&'static str> {
    if patch_of(&m.info.game_version) != patch {
        return Some("other patch");
    }
    if m.info.map_id != RIFT {
        return Some("not Summoner's Rift");
    }
    if m.info.participants.len() != 10 {
        return Some("not ten players");
    }
    if m.info.participants.iter().any(|p| p.early_surrender || p.team_early_surrendered) {
        return Some("remake or early surrender");
    }
    if m.info.game_duration < MIN_GAME_S {
        return Some("shorter than 15 min");
    }
    None
}

#[derive(Default)]
pub struct Damage {
    sums: HashMap<u32, (u64, u64)>,
    teams: Vec<[u32; 5]>,
}

impl Damage {
    pub fn add(&mut self, m: &MatchFile) {
        for p in &m.info.participants {
            let sum = self.sums.entry(p.champion_id).or_default();
            sum.0 += p.magic_damage;
            sum.1 += p.physical_damage + p.magic_damage;
        }
        self.teams.extend(teams_of(m).into_iter().flatten());
    }

    pub fn finish(&self) -> (BTreeMap<u32, f64>, [f64; 2]) {
        let shares: BTreeMap<u32, f64> = self.sums.iter().map(|(&c, &(magic, all))| (c, if all > 0 { (magic as f64 / all as f64 * 10_000.0).round() / 10_000.0 } else { 0.0 })).collect();
        let mut values: Vec<f64> = self.teams.iter().map(|t| team_value(t, &shares)).collect();
        values.sort_by(f64::total_cmp);
        let q = |q: f64| {
            if values.is_empty() {
                return 0.0;
            }
            let pos = q * (values.len() - 1) as f64;
            let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
            let v = values[lo] + (values[hi] - values[lo]) * (pos - lo as f64);
            (v * 10_000.0).round() / 10_000.0
        };
        (shares, [q(1.0 / 3.0), q(2.0 / 3.0)])
    }
}

fn teams_of(m: &MatchFile) -> [Option<[u32; 5]>; 2] {
    let side = |team: u32| -> Option<[u32; 5]> { m.info.participants.iter().filter(|p| p.team_id == team).map(|p| p.champion_id).collect::<Vec<_>>().try_into().ok() };
    [side(100), side(200)]
}

pub fn team_value(team: &[u32], shares: &BTreeMap<u32, f64>) -> f64 {
    team.iter().map(|c| shares.get(c).copied().unwrap_or(0.0)).sum::<f64>() / team.len().max(1) as f64
}

pub struct Builder {
    pub items: Items,
    pub shares: BTreeMap<u32, f64>,
    pub cuts: [f64; 2],
    pairs: HashMap<(u32, String), [Acc; 2]>,
    games: u32,
    bands: BTreeMap<String, u32>,
}

impl Builder {
    pub fn new(items: Items, shares: BTreeMap<u32, f64>, cuts: [f64; 2]) -> Self {
        Self { items, shares, cuts, pairs: HashMap::new(), games: 0, bands: BTreeMap::new() }
    }

    pub fn add(&mut self, m: &MatchFile, t: &TimelineFile, band: Option<&str>) -> Result<(), String> {
        if t.metadata.match_id != m.metadata.match_id {
            return Err(format!("{}: timeline is for {}", m.metadata.match_id, t.metadata.match_id));
        }
        self.games += 1;
        *self.bands.entry(band.unwrap_or(TOP_BAND).to_owned()).or_default() += 1;
        let frames = &t.info.frames;
        let frame_before = |ms: u64| frames.iter().rev().find(|f| f.timestamp < ms);
        let gold = |frame: &Frame, pid: u32| frame.participants.get(&pid.to_string()).map(|p| (p.total_gold, p.current_gold));
        let values: Vec<f64> = teams_of(m).iter().map(|t| t.map_or(0.0, |t| team_value(&t, &self.shares))).collect();
        for p in &m.info.participants {
            if p.team_position.is_empty() {
                continue;
            }
            let build = player_build(&self.items, &purchases(t, p.participant_id));
            let enemy = m.info.participants.iter().find(|e| e.team_id != p.team_id && e.team_position == p.team_position).map(|e| e.participant_id);
            let damage = damage_situation(if p.team_id == 100 { values[1] } else { values[0] }, self.cuts);
            let lead = |slot: usize| {
                let (enemy, ms) = (enemy?, build.finished.get(slot)?.ms);
                let frame = frame_before(ms)?;
                Some(lead_situation(gold(frame, p.participant_id)?.0, gold(frame, enemy)?.0))
            };
            let back_gold = build.back.as_ref().and_then(|(ms, _)| frame_before(*ms)).and_then(|f| gold(f, p.participant_id)).map(|g| g.1);
            let seen = Seen { build: &build, back_gold, damage, leads: [None, lead(1), lead(2)] };
            let accs = self.pairs.entry((p.champion_id, p.team_position.clone())).or_default();
            accs[0].add(&seen);
            if band.is_none() {
                accs[1].add(&seen);
            }
        }
        Ok(())
    }

    pub fn finish(self, patch: &str) -> Output {
        let mut pairs: Vec<Pair> = self
            .pairs
            .iter()
            .filter(|(_, accs)| accs[0].games >= MIN_PAIR_GAMES)
            .map(|((champion, role), accs)| Pair { champion: *champion, role: role.clone(), all: accs[0].table(), top: (accs[1].games >= MIN_PAIR_GAMES).then(|| accs[1].table()) })
            .collect();
        pairs.sort_by(|a, b| (a.champion, &a.role).cmp(&(b.champion, &b.role)));
        let sorted = |set: &HashSet<u32>| {
            let mut v: Vec<u32> = set.iter().copied().collect();
            v.sort_unstable();
            v
        };
        Output { patch: patch.to_owned(), games: self.games, bands: self.bands, finished: sorted(&self.items.finished), boots: sorted(&self.items.boots), magic_share: self.shares, damage_cuts: self.cuts, lead_cut: LEAD_CUT, pairs }
    }
}

pub struct Args {
    pub raw: PathBuf,
    pub bands: Option<PathBuf>,
    pub items: PathBuf,
    pub patch: String,
    pub out: PathBuf,
}

pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut found: HashMap<&str, String> = HashMap::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let key = arg.strip_prefix("--").filter(|k| ["raw", "bands", "items", "patch", "out"].contains(k)).ok_or_else(|| format!("unknown argument {arg:?}"))?;
        found.insert(key, it.next().cloned().ok_or_else(|| format!("{arg} needs a value"))?);
    }
    let mut take = |key: &str| found.remove(key).ok_or_else(|| format!("--{key} is needed"));
    Ok(Args { raw: take("raw")?.into(), items: take("items")?.into(), patch: take("patch")?, out: take("out")?.into(), bands: found.remove("bands").map(PathBuf::from) })
}

pub fn read_gz<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let mut bytes = Vec::new();
    GzDecoder::new(fs::File::open(path).map_err(|err| format!("{}: {err}", path.display()))?).read_to_end(&mut bytes).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|err| format!("{}: {err}", path.display()))
}

pub fn read_bands(dir: &Path) -> Result<HashMap<String, String>, String> {
    #[derive(Deserialize)]
    struct State {
        #[serde(default)]
        bands: HashMap<String, String>,
    }
    let mut out = HashMap::new();
    for entry in fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") && path.file_stem().and_then(|s| s.to_str()).is_some_and(|s| region_of(s).is_some()) {
            let state: State = serde_json::from_slice(&fs::read(&path).map_err(|err| format!("{}: {err}", path.display()))?).map_err(|err| format!("{}: {err}", path.display()))?;
            out.extend(state.bands);
        }
    }
    Ok(out)
}

fn match_files(raw: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut out: Vec<(PathBuf, PathBuf)> = fs::read_dir(raw)
        .map_err(|err| format!("{}: {err}", raw.display()))?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let id = name.strip_suffix(".match.json.gz")?;
            let timeline = raw.join(format!("{id}.timeline.json.gz"));
            timeline.exists().then(|| (e.path(), timeline))
        })
        .collect();
    out.sort();
    Ok(out)
}

pub fn run(args: &Args) -> Result<String, String> {
    let items = Items::parse(&fs::read(&args.items).map_err(|err| format!("{}: {err}", args.items.display()))?)?;
    let bands = match &args.bands {
        Some(dir) => read_bands(dir)?,
        None => HashMap::new(),
    };
    let files = match_files(&args.raw)?;
    let mut skipped: BTreeMap<String, u32> = BTreeMap::new();
    let mut damage = Damage::default();
    let mut kept = Vec::new();
    for (match_path, timeline_path) in &files {
        match read_gz::<MatchFile>(match_path) {
            Ok(m) => match skip_reason(&m, &args.patch) {
                Some(why) => *skipped.entry(why.to_owned()).or_default() += 1,
                None => {
                    damage.add(&m);
                    kept.push((match_path, timeline_path));
                }
            },
            Err(err) => {
                eprintln!("{err}");
                *skipped.entry("unreadable".to_owned()).or_default() += 1;
            }
        }
    }
    let (shares, cuts) = damage.finish();
    drop(damage);
    let mut builder = Builder::new(items, shares, cuts);
    for (match_path, timeline_path) in kept {
        let added = read_gz::<MatchFile>(match_path).and_then(|m| {
            let t = read_gz::<TimelineFile>(timeline_path)?;
            let band = bands.get(&m.metadata.match_id).map(String::as_str);
            builder.add(&m, &t, band)
        });
        if let Err(err) = added {
            eprintln!("{err}");
            *skipped.entry("unreadable".to_owned()).or_default() += 1;
        }
    }
    if builder.games == 0 {
        return Err(format!("no usable game of patch {} in {}; left out: {skipped:?}", args.patch, args.raw.display()));
    }
    let output = builder.finish(&args.patch);
    let bytes = serde_json::to_vec(&output).map_err(|err| err.to_string())?;
    write_atomic(&args.out, &bytes).map_err(|err| format!("{}: {err}", args.out.display()))?;
    Ok(format!("{} games, {} champion and role pairs, {} bytes to {}; bands {:?}; left out: {skipped:?}", output.games, output.pairs.len(), bytes.len(), args.out.display(), output.bands))
}
