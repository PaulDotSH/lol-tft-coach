//! Mayhem Tab scoreboard: icon templates fetched in the background, the OCR
//! thread's scoreboard polls (only while a Mayhem game runs) and per-game
//! memory of what was read.

use crate::health::Reader;
use crate::state::{Cache, Shared, ensure};
use model::*;
use ocr::scoreboard::{self, AugIcons, BoardPlayer, Desc, Geometry, Rect};
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
/// Game-data icon of ability augments; in game they show the champion's ability art.
const GENERIC_ICON: &str = "genericabilityaugmenticon";
/// An open board refines the calibrated fit at most this often.
const REFIT_EVERY: Duration = Duration::from_secs(10);
/// While enemy augments are unknown and the board hasn't been found under this window's
/// calibration, the whole window is searched for it at most this often.
const WIDE_EVERY: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct TabShared {
    icons: Cache<(), Icons>,
    items: Cache<ItemId, Desc>,
    ok: Mutex<Option<bool>>,
    job: Mutex<Option<Arc<Job>>>,
    read: Mutex<Option<Read>>,
    fit: Mutex<FitState>,
}

#[derive(Default)]
struct FitState {
    refit: Option<Instant>,
    wide: Option<Instant>,
    /// Calibration key the board was found under this session.
    found: Option<String>,
}

struct Icons {
    aug: AugIcons,
    rarity: HashMap<AugmentId, Rarity>,
    /// Augments whose scoreboard art no template shows (ability augments,
    /// placeholder art, failed downloads): the candidates of an unmatched icon.
    unmatched: Vec<AugmentId>,
}

struct Job {
    at: Instant,
    icons: Arc<Icons>,
    items: HashMap<ItemId, Desc>,
    /// Live API order: (champion, team, items).
    players: Vec<(ChampKey, Team, Vec<ItemId>)>,
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
    /// My own row, read from the same scoreboard (exact, unlike stat-diff detection).
    pub mine: Vec<SeenAugment>,
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
            .filter(|p| p.team == enemy || p.champ == me.champ)
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
            if i == snap.me {
                if augments.len() >= memo.mine.len() {
                    memo.mine = augments;
                }
                continue;
            }
            if p.team != enemy {
                continue;
            }
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
    let decoded: Vec<_> = tokio::task::spawn_blocking(move || {
        bodies.into_iter().filter_map(|(id, b)| Some((id, scoreboard::decode_png(&b)?))).collect()
    })
    .await?;
    let mut unmatched: Vec<AugmentId> = augs
        .iter()
        .filter(|(id, a)| a.icon.contains(GENERIC_ICON) || !decoded.iter().any(|d| d.0 == **id))
        .map(|(&id, _)| id)
        .collect();
    unmatched.sort_unstable();
    let aug = AugIcons::new(decoded);
    tracing::info!("scoreboard: {} augments, {} icons, {} without art", augs.len(), aug.len(), unmatched.len());
    Ok(Icons { aug, rarity: augs.into_iter().map(|(id, a)| (id, a.rarity)).collect(), unmatched })
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
    if let Some(rows) = scan(sh, &job) {
        tracing::debug!("scoreboard read in {:?}: {rows:?}", t.elapsed());
        let champs = job.players.iter().map(|p| p.0).collect();
        *sh.tab.read.lock().unwrap() = Some(Read { champs, rows });
    }
    if job.urgent { URGENT } else { PASSIVE }
}

/// Up to two small column captures decide whether the scoreboard is open (at the calibrated
/// geometry); only then is the board captured and read.
fn scan(sh: &Shared, job: &Job) -> Option<Vec<(usize, Vec<SeenIcon>)>> {
    let (w, h) = ocr::league_client()?;
    let key = sh.calib_key(w, h);
    let stored = sh.calib.lock().unwrap().get("lol", &key).and_then(|e| e.scoreboard);
    let g = stored.map_or_else(|| Geometry::new(w, h), |b| Geometry { s: b.s, cx: b.cx, cy: b.cy });
    let open = scoreboard::is_open_with(
        |h| {
            let c = g.column(h);
            Some((ocr::capture_client(c)?, (c.x, c.y)))
        },
        &g,
    )?;
    if !open {
        if job.urgent {
            wide_fit(sh, &key, (w, h), &g);
        }
        return None;
    }
    let r = g.board();
    let f = ocr::capture_client(r)?;
    crate::debug::frame("tab-board", Duration::from_secs(15), &f);
    let players: Vec<BoardPlayer> = job.players.iter().map(|p| BoardPlayer { team: p.1, items: &p.2 }).collect();
    let Some(rows) = scoreboard::read_all(&f, (r.x, r.y), &g, &players, &job.icons.aug, &job.items) else {
        tracing::debug!("scoreboard open ({w}x{h}) but rows unreadable");
        return None;
    };
    let due = {
        let mut st = sh.tab.fit.lock().unwrap();
        let due = st.refit.is_none_or(|t| t.elapsed() >= REFIT_EVERY);
        if due {
            st.refit = Some(Instant::now());
        }
        st.found = Some(key.clone());
        due
    };
    if due {
        let fitted = Geometry::fit(&f, (r.x, r.y), (w, h), &g).map(|x| x.0);
        board_found(sh, &key, (w, h), fitted.unwrap_or(g), false);
    }
    let candidates = |s: &scoreboard::Seen| {
        let ids = s.group.map_or(&job.icons.unmatched[..], |g| job.icons.aug.ids(g));
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

/// The board isn't where the calibration expects it: look over the whole window, unless it was
/// already found under this calibration this session or was looked for just now.
fn wide_fit(sh: &Shared, key: &str, client: (u32, u32), g: &Geometry) {
    {
        let mut st = sh.tab.fit.lock().unwrap();
        if st.found.as_deref() == Some(key) || st.wide.is_some_and(|t| t.elapsed() < WIDE_EVERY) {
            return;
        }
        st.wide = Some(Instant::now());
    }
    let Some(f) = ocr::capture_client(Rect { x: 0, y: 0, w: client.0 as i32, h: client.1 as i32 }) else { return };
    let t = Instant::now();
    match Geometry::fit(&f, (0, 0), client, g) {
        Some((fit, found)) => {
            tracing::info!("scoreboard found at {:.2}× ({}x{}) in {:?}", found.scale, client.0, client.1, t.elapsed());
            sh.tab.fit.lock().unwrap().found = Some(key.to_string());
            board_found(sh, key, client, fit, true);
        }
        None => crate::debug::frame("tab-wide-miss", Duration::from_secs(60), &f),
    }
}

/// A board read at `g`: into the calibration (taken as is when `set`) and the reader's health.
fn board_found(sh: &Shared, key: &str, client: (u32, u32), g: Geometry, set: bool) {
    if let Some(patch) = sh.patch() {
        let b = crate::calib::Board { s: g.s, cx: g.cx, cy: g.cy };
        sh.calib.lock().unwrap().scoreboard("lol", key, &patch, unix_secs(), b, set);
    }
    let base = Geometry::new(client.0, client.1);
    let off = ((g.cx - base.cx) / client.0 as f32).abs().max(((g.cy - base.cy) / client.1 as f32).abs());
    sh.health.lock().unwrap().found(Reader::Scoreboard, Shared::now_ms(), crate::health::shift(g.s / base.s, off));
}

fn unix_secs() -> u64 {
    Shared::now_ms() / 1000
}

/// Every Mayhem augment id from the game data, once the icon set has loaded.
pub fn pool(sh: &Shared) -> Option<Vec<AugmentId>> {
    match sh.tab.icons.lock().unwrap().get(&()) {
        Some(crate::state::Slot::Ready(i)) => Some(i.rarity.keys().copied().collect()),
        _ => None,
    }
}
