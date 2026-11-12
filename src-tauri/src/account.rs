//! The League account the client is signed in to, and where each account's files live on this
//! PC: `%LOCALAPPDATA%\lol-coach\players\<key>\{history,recaps,ranked}`. `<key>` is the player
//! key the minipc service makes of a Riot ID (`crates/server/src/players.rs`).

use crate::settings;
use clients::lcu::Lcu;
use model::Phase;
use serde_json::Value;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The kinds of files that belong to an account.
pub const KINDS: [&str; 3] = ["history", "recaps", "ranked"];

const MAX_RIOT_ID_CHARS: usize = 64;
const MAX_KEY_BYTES: usize = 255;
const KEY_FILE: &str = "account.key";
const ACCEPTED_FILE: &str = "account.accepted";
const LAST_FILE: &str = "last.txt";
const LEGACY_OWNER: &str = "legacy.txt";
const LEGACY_DONE: &str = "legacy.done";
/// The client is asked who is signed in this often, and this often while nobody is known yet.
const ASK_EVERY: Duration = Duration::from_secs(60);
const ASK_AGAIN: Duration = Duration::from_secs(5);

/// The signed-in player: the Riot ID as the client gives it and the key made of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub riot_id: String,
    pub key: String,
}

fn percent_encode(s: &str, keep: impl Fn(u8) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if keep(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The service's player key of a Riot ID (`RiotId::parse`): trimmed, lowercased, percent-encoded
/// except `a-z 0-9 - _ .`; None where the service refuses the ID.
pub fn player_key(riot_id: &str) -> Option<String> {
    let raw = riot_id.trim();
    if raw.chars().count() > MAX_RIOT_ID_CHARS {
        return None;
    }
    let (name, tag) = raw.split_once('#')?;
    let (name, tag) = (name.trim().to_lowercase(), tag.trim().to_lowercase());
    if name.is_empty() || tag.is_empty() || tag.contains('#') {
        return None;
    }
    let key = percent_encode(&format!("{name}#{tag}"), |b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.'));
    (key.len() <= MAX_KEY_BYTES).then_some(key)
}

/// The Riot ID as one URL path segment, percent-encoded once (`Some%20Player%23EUW`).
pub fn url_player(riot_id: &str) -> String {
    percent_encode(riot_id.trim(), |b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// The player in a `/lol-summoner/v1/current-summoner` reply.
pub fn from_summoner(v: &Value) -> Option<Player> {
    let (name, tag) = (v["gameName"].as_str()?.trim(), v["tagLine"].as_str()?.trim());
    let riot_id = format!("{name}#{tag}");
    Some(Player { key: player_key(&riot_id)?, riot_id })
}

fn safe_key(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_KEY_BYTES && s.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'%' | b'A'..=b'F')) && !s.starts_with('.')
}

pub struct Accounts {
    base: PathBuf,
    state: Mutex<State>,
    epoch: AtomicU64,
}

struct State {
    named: Option<Player>,
    last: Option<String>,
}

impl Accounts {
    pub fn new(base: PathBuf) -> Accounts {
        let last = std::fs::read_to_string(base.join("players").join(LAST_FILE)).ok().map(|s| s.trim().to_owned()).filter(|s| safe_key(s));
        Accounts { base, state: Mutex::new(State { named: None, last }), epoch: AtomicU64::new(0) }
    }

    /// The player the client named in this run.
    pub fn named(&self) -> Option<Player> {
        self.state.lock().unwrap().named.clone()
    }

    /// Changes when the files in use are another account's than before, or were changed by a sync.
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Relaxed)
    }

    /// A sync brought files in: whoever holds them in memory reads them again.
    pub fn reread(&self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
    }

    pub fn player_dir(&self, key: &str) -> PathBuf {
        self.base.join("players").join(key)
    }

    /// The directory for `kind`, created: the named player's, else the one used last, else the
    /// old shared one.
    pub fn dir(&self, kind: &str) -> PathBuf {
        let key = {
            let st = self.state.lock().unwrap();
            st.named.as_ref().map(|p| p.key.clone()).or_else(|| st.last.clone())
        };
        let dir = match key {
            Some(key) => self.player_dir(&key).join(kind),
            None => self.base.join(kind),
        };
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// The client names the signed-in player: the first one gets the files of before accounts
    /// existed, and this account is the one used until the next run names another.
    pub fn set_named(&self, player: Player) {
        let mut st = self.state.lock().unwrap();
        if st.named.as_ref() == Some(&player) {
            return;
        }
        let before = st.named.as_ref().map(|p| p.key.clone()).or_else(|| st.last.clone());
        match migrate_legacy(&self.base, &player.key) {
            Ok(true) => {}
            Ok(false) => tracing::info!("some files of before accounts stay where they are until the next start"),
            Err(e) => tracing::warn!("moving files of before accounts: {e}"),
        }
        if st.last.as_deref() != Some(player.key.as_str()) {
            let players = self.base.join("players");
            let _ = std::fs::create_dir_all(&players);
            if let Err(e) = std::fs::write(players.join(LAST_FILE), &player.key) {
                tracing::warn!("last account not saved: {e}");
            }
            st.last = Some(player.key.clone());
        }
        if before.is_some_and(|b| b != player.key) {
            self.epoch.fetch_add(1, Ordering::Relaxed);
        }
        st.named = Some(player);
    }
}

fn regular_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    rd.filter_map(|e| e.ok()).filter(|e| e.file_type().is_ok_and(|t| t.is_file())).map(|e| e.path()).collect()
}

/// Moves the files in the old shared `history`, `recaps` and `ranked` directories to `key`.
/// The first account that asks owns them (`players\legacy.txt`, written before the first file
/// moves), so a move that stopped half way goes on for the same account at the next start and
/// never for another. A file is moved, never copied, and never over one that is there; one that
/// cannot be moved stays and is tried again. Ok(true) when nothing is left to move.
fn migrate_legacy(base: &Path, key: &str) -> io::Result<bool> {
    let players = base.join("players");
    if players.join(LEGACY_DONE).exists() {
        return Ok(true);
    }
    let owner = std::fs::read_to_string(players.join(LEGACY_OWNER)).ok().map(|s| s.trim().to_owned());
    if owner.as_deref().is_some_and(|o| o != key) {
        return Ok(false);
    }
    let old = KINDS.map(|k| (k, regular_files(&base.join(k))));
    if owner.is_none() && old.iter().any(|(_, files)| !files.is_empty()) {
        std::fs::create_dir_all(&players)?;
        std::fs::write(players.join(LEGACY_OWNER), key)?;
    }
    let mut left = 0;
    for (kind, files) in &old {
        for file in files {
            let Some(name) = file.file_name() else { continue };
            let to = players.join(key).join(kind).join(name);
            let moved = !to.exists() && std::fs::create_dir_all(players.join(key).join(kind)).and_then(|()| std::fs::rename(file, &to)).is_ok();
            if !moved {
                left += 1;
                tracing::info!("{} stays in {kind}", name.to_string_lossy());
            }
        }
    }
    if left > 0 {
        return Ok(false);
    }
    std::fs::create_dir_all(&players)?;
    std::fs::write(players.join(LEGACY_DONE), key)?;
    for kind in KINDS {
        let _ = std::fs::remove_dir(base.join(kind));
    }
    Ok(true)
}

/// The account's secret key (64 lowercase hex characters) in `dir\account.key`, made on first use.
pub fn account_key(dir: &Path) -> io::Result<String> {
    let file = dir.join(KEY_FILE);
    if let Some(key) = std::fs::read_to_string(&file).ok().map(|s| s.trim().to_owned()).filter(|s| s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))) {
        return Ok(key);
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    let key: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::create_dir_all(dir)?;
    let part = dir.join(format!("{KEY_FILE}.part"));
    std::fs::write(&part, &key)?;
    std::fs::rename(&part, &file)?;
    Ok(key)
}

/// Whether the service at `server` has accepted this account's key.
pub fn accepted(dir: &Path, server: &str) -> bool {
    std::fs::read_to_string(dir.join(ACCEPTED_FILE)).is_ok_and(|s| s.trim() == server)
}

pub fn mark_accepted(dir: &Path, server: &str) -> io::Result<()> {
    std::fs::write(dir.join(ACCEPTED_FILE), server)
}

static GLOBAL: OnceLock<Accounts> = OnceLock::new();

pub fn global() -> &'static Accounts {
    GLOBAL.get_or_init(|| Accounts::new(settings::local_dir("")))
}

/// Where `history`, `recaps` or `ranked` are read and written now.
pub fn dir(kind: &str) -> PathBuf {
    global().dir(kind)
}

/// Asks the client who is signed in, now and then, off the phase loop.
#[derive(Default)]
pub struct Watch(Mutex<Asked>);

#[derive(Default)]
struct Asked {
    running: bool,
    at: Option<Instant>,
}

impl Watch {
    pub fn tick(self: &Arc<Self>, lcu: Option<&Lcu>, phase: Phase) {
        let Some(lcu) = lcu else { return };
        let every = if global().named().is_some() { ASK_EVERY } else { ASK_AGAIN };
        let mut st = self.0.lock().unwrap();
        if phase == Phase::InGame || st.running || st.at.is_some_and(|t| t.elapsed() < every) {
            return;
        }
        st.running = true;
        st.at = Some(Instant::now());
        let (me, lcu) = (self.clone(), lcu.clone());
        tokio::spawn(async move {
            match lcu.get("/lol-summoner/v1/current-summoner").await {
                Ok(Some(v)) => match from_summoner(&v) {
                    Some(player) => {
                        let _ = tokio::task::spawn_blocking(move || {
                            if global().named().as_ref() != Some(&player) {
                                tracing::info!("account {}", player.key);
                            }
                            global().set_named(player);
                        })
                        .await;
                    }
                    None => tracing::debug!("current summoner has no Riot ID yet"),
                },
                Ok(None) => tracing::debug!("current summoner: 404"),
                Err(e) => tracing::debug!("current summoner: {e:#}"),
            }
            me.0.lock().unwrap().running = false;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn player(id: &str) -> Player {
        Player { riot_id: id.into(), key: player_key(id).unwrap() }
    }

    fn put(base: &Path, rel: &str, body: &str) {
        let p = base.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn read(base: &Path, rel: &str) -> Option<String> {
        std::fs::read_to_string(base.join(rel)).ok()
    }

    #[test]
    fn key_is_trimmed_lowercased_and_percent_encoded() {
        assert_eq!(player_key("Some Player#EUW").unwrap(), "some%20player%23euw");
        assert_eq!(player_key("  SOME PLAYER#euw  ").unwrap(), "some%20player%23euw");
        assert_eq!(player_key("a#b").unwrap(), "a%23b");
        assert_eq!(player_key("a.b-c_d#1").unwrap(), "a.b-c_d%231");
        assert_eq!(player_key("Zoë#Ñ1").unwrap(), "zo%C3%AB%23%C3%B11");
        assert_eq!(player_key("a/b\\c#..d").unwrap(), "a%2Fb%5Cc%23..d");
        assert_eq!(player_key("a % b#c").unwrap(), "a%20%25%20b%23c");
    }

    #[test]
    fn key_never_is_a_dot_name_or_holds_a_separator() {
        for raw in ["../..#x", "..#..", ".#.", "a/../b#c", "a\\..\\b#c"] {
            let k = player_key(raw).unwrap();
            assert!(k.contains("%23") && !k.contains('/') && !k.contains('\\') && !k.contains('#') && k != "." && k != "..", "{k}");
        }
    }

    #[test]
    fn bad_riot_ids_have_no_key() {
        let long_name = format!("{}#t", "x".repeat(63));
        let wide = format!("{}#t", "é".repeat(62));
        for raw in ["", "   ", "nohash", "#tag", "name#", " # ", "a#b#c", "#", long_name.as_str(), wide.as_str()] {
            assert!(player_key(raw).is_none(), "accepted {raw:?}");
        }
        let longest = format!("{}#t", "x".repeat(62));
        assert!(player_key(&longest).is_some());
    }

    #[test]
    fn the_url_form_is_encoded_once_and_keeps_case() {
        assert_eq!(url_player("Some Player#EUW"), "Some%20Player%23EUW");
        assert_eq!(url_player(" Zoë#1 "), "Zo%C3%AB%231");
        assert_eq!(url_player("a % b#c"), "a%20%25%20b%23c");
    }

    #[test]
    fn the_player_is_read_from_the_summoner() {
        let v = serde_json::json!({ "gameName": "Some Player", "tagLine": "EUW", "puuid": "p" });
        assert_eq!(from_summoner(&v), Some(Player { riot_id: "Some Player#EUW".into(), key: "some%20player%23euw".into() }));
        assert_eq!(from_summoner(&serde_json::json!({ "displayName": "x" })), None);
        assert_eq!(from_summoner(&serde_json::json!({ "gameName": "", "tagLine": "EUW" })), None);
    }

    #[test]
    fn old_directories_are_used_until_an_account_is_known() {
        let t = TempDir::new().unwrap();
        let a = Accounts::new(t.path().to_owned());
        assert_eq!(a.dir("recaps"), t.path().join("recaps"));
        a.set_named(player("Me#EUW"));
        assert_eq!(a.dir("recaps"), t.path().join("players").join("me%23euw").join("recaps"));
        assert_eq!(read(t.path(), "players/last.txt").as_deref(), Some("me%23euw"));
        let next_run = Accounts::new(t.path().to_owned());
        assert_eq!(next_run.named(), None);
        assert_eq!(next_run.dir("ranked"), t.path().join("players").join("me%23euw").join("ranked"));
    }

    #[test]
    fn a_changed_account_bumps_the_epoch_but_the_first_naming_does_not() {
        let t = TempDir::new().unwrap();
        let a = Accounts::new(t.path().to_owned());
        a.set_named(player("Me#EUW"));
        a.set_named(player("Me#EUW"));
        assert_eq!(a.epoch(), 0);
        a.set_named(player("Other#EUW"));
        assert_eq!(a.epoch(), 1);
        assert!(a.dir("history").ends_with(Path::new("other%23euw").join("history")));
        let b = Accounts::new(t.path().to_owned());
        b.set_named(player("Me#EUW"));
        assert_eq!(b.epoch(), 1);
    }

    #[test]
    fn old_files_are_moved_to_the_first_account_only() {
        let t = TempDir::new().unwrap();
        put(t.path(), "history/1.v7.json", "h1");
        put(t.path(), "recaps/100.json", "r1");
        put(t.path(), "ranked/snapshots.json", "s");
        put(t.path(), "tft/looks.bin", "tft");
        let a = Accounts::new(t.path().to_owned());
        a.set_named(player("Me#EUW"));
        let mine = "players/me%23euw";
        assert_eq!(read(t.path(), &format!("{mine}/history/1.v7.json")).as_deref(), Some("h1"));
        assert_eq!(read(t.path(), &format!("{mine}/recaps/100.json")).as_deref(), Some("r1"));
        assert_eq!(read(t.path(), &format!("{mine}/ranked/snapshots.json")).as_deref(), Some("s"));
        assert!(read(t.path(), "history/1.v7.json").is_none() && read(t.path(), "recaps/100.json").is_none());
        assert_eq!(read(t.path(), "tft/looks.bin").as_deref(), Some("tft"));
        put(t.path(), "recaps/200.json", "late");
        a.set_named(player("Other#EUW"));
        assert!(read(t.path(), "players/other%23euw/recaps/200.json").is_none());
        assert_eq!(read(t.path(), "recaps/200.json").as_deref(), Some("late"));
    }

    #[test]
    fn a_move_that_stops_goes_on_for_the_same_account_and_never_overwrites() {
        let t = TempDir::new().unwrap();
        put(t.path(), "recaps/100.json", "old 100");
        put(t.path(), "recaps/200.json", "old 200");
        put(t.path(), "players/me%23euw/recaps/100.json", "already there");
        let a = Accounts::new(t.path().to_owned());
        a.set_named(player("Me#EUW"));
        assert_eq!(read(t.path(), "players/me%23euw/recaps/100.json").as_deref(), Some("already there"));
        assert_eq!(read(t.path(), "recaps/100.json").as_deref(), Some("old 100"));
        assert_eq!(read(t.path(), "players/me%23euw/recaps/200.json").as_deref(), Some("old 200"));
        assert!(read(t.path(), "players/legacy.done").is_none());
        let b = Accounts::new(t.path().to_owned());
        b.set_named(player("Other#EUW"));
        assert_eq!(read(t.path(), "recaps/100.json").as_deref(), Some("old 100"));
        assert!(read(t.path(), "players/other%23euw/recaps/100.json").is_none());
        std::fs::remove_file(t.path().join("players/me%23euw/recaps/100.json")).unwrap();
        let c = Accounts::new(t.path().to_owned());
        c.set_named(player("Me#EUW"));
        assert_eq!(read(t.path(), "players/me%23euw/recaps/100.json").as_deref(), Some("old 100"));
        assert!(read(t.path(), "recaps/100.json").is_none());
        assert!(read(t.path(), "players/legacy.done").is_some());
    }

    #[test]
    fn the_account_key_is_made_once_and_kept() {
        let t = TempDir::new().unwrap();
        let dir = t.path().join("players").join("me%23euw");
        let key = account_key(&dir).unwrap();
        assert!(key.len() == 64 && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        assert_eq!(account_key(&dir).unwrap(), key);
        std::fs::write(dir.join("account.key"), "damaged").unwrap();
        let again = account_key(&dir).unwrap();
        assert!(again.len() == 64 && again != key);
        assert_ne!(account_key(&t.path().join("other")).unwrap(), again);
    }

    #[test]
    fn acceptance_is_remembered_for_one_server() {
        let t = TempDir::new().unwrap();
        assert!(!accepted(t.path(), "https://a"));
        mark_accepted(t.path(), "https://a").unwrap();
        assert!(accepted(t.path(), "https://a"));
        assert!(!accepted(t.path(), "https://b"));
    }
}
