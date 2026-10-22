//! Mayhem Tab scoreboard: icon templates fetched in the background, the OCR
//! thread's scoreboard polls (only while a Mayhem game runs) and per-game
//! memory of what was read.

use crate::state::{Cache, Shared, ensure};
use model::*;
use ocr::scoreboard::{self, AugIcons, BoardPlayer, Desc, Geometry};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Poll intervals: while enemy augments are unknown, otherwise, and without a job.
const URGENT: Duration = Duration::from_millis(250);
const PASSIVE: Duration = Duration::from_millis(1000);
pub const IDLE: Duration = Duration::from_millis(1500);
/// A job the orchestrator stopped refreshing (game over) is ignored.
const JOB_TTL: Duration = Duration::from_secs(6);
const ICON_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Default)]
pub struct TabShared {
    icons: Cache<(), Icons>,
    items: Cache<ItemId, Desc>,
    ok: Mutex<Option<bool>>,
    job: Mutex<Option<Arc<Job>>>,
    read: Mutex<Option<Read>>,
}

struct Icons {
    aug: AugIcons,
    rarity: HashMap<AugmentId, Rarity>,
}

struct Job {
    at: Instant,
    icons: Arc<Icons>,
    items: HashMap<ItemId, Desc>,
    /// Live API order: (champion, team, items).
    players: Vec<(ChampKey, Team, Vec<ItemId>)>,
    enemy: Team,
    urgent: bool,
}

/// Candidate ids and tint of one icon.
type SeenIcon = (Vec<AugmentId>, Option<Rarity>);

struct Read {
    /// The job's champions, so a read never lands in another game.
    champs: Vec<ChampKey>,
    /// (player index, [(candidate ids, tint)]).
    rows: Vec<(usize, Vec<SeenIcon>)>,
}

/// Per-game memory (part of the orchestrator's `Memo`).
#[derive(Default)]
pub struct TabMemo {
    pub seen: Vec<EnemyAugments>,
    /// Game time since which some enemy has unread augments.
    pub stale_since: Option<f32>,
}

/// Fold in a new scoreboard read and refresh the OCR job. Returns the u.gg
/// rankings loaded for enemy champions.
pub fn tick(
    sh: &Arc<Shared>,
    sd: &StaticData,
    snap: &GameSnapshot,
    memo: &mut TabMemo,
    mayhem: Option<&MayhemData>,
) -> Vec<ChampAugments> {
    let Some(me) = snap.players.get(snap.me).filter(|_| snap.mode == Mode::Mayhem) else {
        return vec![];
    };
    let enemy = if me.team == Team::Order { Team::Chaos } else { Team::Order };
    let ranks: Vec<ChampAugments> = mayhem.map_or(vec![], |m| {
        snap.players
            .iter()
            .filter(|p| p.team == enemy)
            .filter_map(|p| crate::orchestrator::augments(sh, p.champ, &m.patch).map(|a| (*a).clone()))
            .collect()
    });

    let read = sh.tab.read.lock().unwrap().take();
    if let Some(r) = read.filter(|r| r.champs.iter().eq(snap.players.iter().map(|p| &p.champ))) {
        for (i, augs) in r.rows {
            let p = &snap.players[i];
            let rank = ranks.iter().find(|a| a.champ == p.champ);
            let augments: Vec<SeenAugment> = augs
                .into_iter()
                .map(|(ids, rarity)| SeenAugment { ids: coach::order_candidates(&ids, rarity, rank), rarity })
                .collect();
            tracing::debug!("scoreboard: {} {:?}", p.champ, augments);
            // A capture mid fade-in can miss icons: never replace a fuller read.
            if memo.seen.iter().any(|e| e.champ == p.champ && e.augments.len() > augments.len()) {
                continue;
            }
            memo.seen.retain(|e| e.champ != p.champ);
            memo.seen.push(EnemyAugments { champ: p.champ, augments, time: snap.time, level: p.level });
        }
        // Someone may still be choosing: a read restarts the wait.
        memo.stale_since = memo.stale_since.map(|_| snap.time);
    }
    let stale = coach::stale_enemies(snap, &memo.seen);
    memo.stale_since = if stale == 0 { None } else { memo.stale_since.or(Some(snap.time)) };

    let icons = {
        let (http, version) = (sh.http.clone(), sd.version.clone());
        ensure(sh, |s| &s.tab.icons, |s| &s.tab.ok, (), move || async move { load_icons(&http, &version).await })
    };
    let job = icons.map(|icons| {
        let mut items = HashMap::new();
        for &id in snap.players.iter().flat_map(|p| &p.items) {
            if let std::collections::hash_map::Entry::Vacant(e) = items.entry(id)
                && let Some(d) = item_desc(sh, &sd.version, id)
            {
                e.insert(*d);
            }
        }
        Arc::new(Job {
            at: Instant::now(),
            icons,
            items,
            players: snap.players.iter().map(|p| (p.champ, p.team, p.items.clone())).collect(),
            enemy,
            urgent: stale > 0,
        })
    });
    *sh.tab.job.lock().unwrap() = job;
    ranks
}

async fn load_icons(http: &data::Http, version: &str) -> anyhow::Result<Icons> {
    let augs = data::icons::mayhem_augments(http, version).await?;
    let urls = augs.iter().map(|(&id, a)| (id, a.icon.as_str())).collect();
    let mut bodies = data::icons::fetch_icons(http, &urls).await;
    anyhow::ensure!(!bodies.is_empty(), "no augment icons");
    bodies.sort_by_key(|b| b.0);
    let aug = tokio::task::spawn_blocking(move || {
        AugIcons::new(bodies.into_iter().filter_map(|(id, b)| Some((id, scoreboard::decode_png(&b)?))))
    })
    .await?;
    tracing::info!("scoreboard: {} augments, {} icons", augs.len(), aug.len());
    Ok(Icons { aug, rarity: augs.into_iter().map(|(id, a)| (id, a.rarity)).collect() })
}

fn item_desc(sh: &Arc<Shared>, version: &str, id: ItemId) -> Option<Arc<Desc>> {
    let (http, url) = (sh.http.clone(), data::icons::item_icon_url(version, id));
    ensure(
        sh,
        |s| &s.tab.items,
        |s| &s.tab.ok,
        id,
        move || async move {
            let b = data::icons::get_bytes(&http, &url, ICON_TTL).await?;
            let img = scoreboard::decode_png(&b).ok_or_else(|| anyhow::anyhow!("bad item icon {url}"))?;
            Ok(scoreboard::item_desc(&img))
        },
    )
}

/// One OCR-thread poll; returns how long to wait before the next.
pub fn poll(sh: &Shared) -> Duration {
    let Some(job) = sh.tab.job.lock().unwrap().clone().filter(|j| j.at.elapsed() < JOB_TTL) else {
        return IDLE;
    };
    let t = Instant::now();
    if let Some(rows) = scan(&job) {
        tracing::debug!("scoreboard read in {:?}: {rows:?}", t.elapsed());
        let champs = job.players.iter().map(|p| p.0).collect();
        *sh.tab.read.lock().unwrap() = Some(Read { champs, rows });
    }
    if job.urgent { URGENT } else { PASSIVE }
}

/// Two small column captures decide whether the scoreboard is open; only then is the board captured and read.
fn scan(job: &Job) -> Option<Vec<(usize, Vec<SeenIcon>)>> {
    let (w, h) = ocr::league_client()?;
    let g = Geometry::new(w, h);
    let (a, b) = (g.column(0), g.column(1));
    let (fa, fb) = (ocr::capture_client(a)?, ocr::capture_client(b)?);
    if !scoreboard::is_open([(&fa, (a.x, a.y)), (&fb, (b.x, b.y))], &g) {
        return None;
    }
    let r = g.board();
    let f = ocr::capture_client(r)?;
    crate::debug::frame("tab-board", Duration::from_secs(15), &f);
    let players: Vec<BoardPlayer> = job.players.iter().map(|p| BoardPlayer { team: p.1, items: &p.2 }).collect();
    let Some(rows) = scoreboard::read(&f, (r.x, r.y), &g, &players, job.enemy, &job.icons.aug, &job.items) else {
        tracing::debug!("scoreboard open ({w}x{h}) but rows unreadable");
        return None;
    };
    let candidates = |s: &scoreboard::Seen| {
        let ids = job.icons.aug.ids(s.group);
        // Augments sharing an icon: keep those of the tinted rarity when any are.
        let tinted: Vec<AugmentId> = ids
            .iter()
            .copied()
            .filter(|id| s.rarity.is_some() && job.icons.rarity.get(id) == s.rarity.as_ref())
            .collect();
        if tinted.is_empty() { ids.to_vec() } else { tinted }
    };
    Some(
        rows.into_iter()
            .map(|row| (row.player, row.augments.iter().map(|s| (candidates(s), s.rarity)).collect()))
            .collect(),
    )
}
