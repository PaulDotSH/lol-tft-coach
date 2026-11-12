use crate::error::{AppError, AppResult};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

pub const LOOKUP_KIND: &str = "lookup";
const MAX_RIOT_ID_CHARS: usize = 64;
const MAX_KEY_BYTES: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiotId {
    pub name: String,
    pub tag: String,
    pub key: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileInfo {
    pub kind: String,
    pub name: String,
    pub size: u64,
    pub modified: u64,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Stored {
    pub size: u64,
    pub modified: u64,
    pub sha256: String,
}

fn refuse() -> AppError {
    AppError::bad_request("player must be a Riot ID like name#tag, at most 64 characters")
}

impl RiotId {
    pub fn parse(raw: &str) -> AppResult<Self> {
        let raw = raw.trim();
        if raw.chars().count() > MAX_RIOT_ID_CHARS {
            return Err(refuse());
        }
        let (name, tag) = raw.split_once('#').ok_or_else(refuse)?;
        let (name, tag) = (name.trim().to_lowercase(), tag.trim().to_lowercase());
        if name.is_empty() || tag.is_empty() || tag.contains('#') {
            return Err(refuse());
        }
        let key = percent_encode(&format!("{name}#{tag}"), |b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.'));
        if key.len() > MAX_KEY_BYTES {
            return Err(refuse());
        }
        Ok(Self { name, tag, key })
    }

    pub fn from_path(segment: &str) -> AppResult<Self> {
        if segment.contains('#') { Self::parse(segment) } else { Self::parse(&percent_decode(segment)?) }
    }
}

pub fn percent_encode(s: &str, keep: impl Fn(u8) -> bool) -> String {
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

pub fn percent_decode(s: &str) -> AppResult<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()).ok_or_else(refuse)?;
            out.push(hex);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| refuse())
}

pub fn valid_name(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=128).contains(&b.len()) && b[0].is_ascii_alphanumeric() && b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

pub fn check_kind(kind: &str) -> AppResult<()> {
    if !valid_name(kind) || kind.eq_ignore_ascii_case(LOOKUP_KIND) {
        return Err(AppError::bad_request("invalid kind"));
    }
    Ok(())
}

pub fn check_name(name: &str) -> AppResult<()> {
    if !valid_name(name) {
        return Err(AppError::bad_request("invalid name"));
    }
    Ok(())
}

pub fn players_dir(root: &Path) -> PathBuf {
    root.join("players")
}

pub fn player_dir(root: &Path, key: &str) -> PathBuf {
    players_dir(root).join(key)
}

fn file_path(root: &Path, key: &str, kind: &str, name: &str) -> AppResult<PathBuf> {
    check_kind(kind)?;
    check_name(name)?;
    Ok(player_dir(root, key).join(kind).join(name))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn unix_secs(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().ok_or_else(|| io::Error::other("path has no directory"))?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".tmp-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    let written = fs::File::create(&tmp).and_then(|mut f| f.write_all(bytes).and_then(|()| f.sync_all())).and_then(|()| fs::rename(&tmp, path));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

fn subdirs(dir: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            out.push((name.to_owned(), entry.path()));
        }
    }
    Ok(out)
}

fn list_blocking(root: &Path, key: &str, only: Option<&str>) -> io::Result<Vec<FileInfo>> {
    let mut out = Vec::new();
    for (kind, dir) in subdirs(&player_dir(root, key))? {
        if !valid_name(&kind) || kind.eq_ignore_ascii_case(LOOKUP_KIND) || only.is_some_and(|k| k != kind) {
            continue;
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
            if !meta.is_file() || !valid_name(&name) {
                continue;
            }
            let sha256 = sha256_hex(&fs::read(entry.path())?);
            out.push(FileInfo { kind: kind.clone(), name, size: meta.len(), modified: unix_secs(&meta), sha256 });
        }
    }
    out.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
    Ok(out)
}

pub async fn list(root: &Path, key: &str, kind: Option<&str>) -> AppResult<Vec<FileInfo>> {
    if let Some(kind) = kind {
        check_kind(kind)?;
    }
    let (root, key, kind) = (root.to_owned(), key.to_owned(), kind.map(str::to_owned));
    Ok(tokio::task::spawn_blocking(move || list_blocking(&root, &key, kind.as_deref())).await??)
}

pub async fn read(root: &Path, key: &str, kind: &str, name: &str) -> AppResult<(Vec<u8>, String)> {
    let path = file_path(root, key, kind, name)?;
    Ok(tokio::task::spawn_blocking(move || {
        let bytes = fs::read(path)?;
        let sha = sha256_hex(&bytes);
        io::Result::Ok((bytes, sha))
    })
    .await??)
}

pub async fn write(root: &Path, key: &str, kind: &str, name: &str, body: Vec<u8>) -> AppResult<Stored> {
    let path = file_path(root, key, kind, name)?;
    Ok(tokio::task::spawn_blocking(move || {
        write_atomic(&path, &body)?;
        io::Result::Ok(Stored { size: body.len() as u64, modified: unix_secs(&fs::metadata(&path)?), sha256: sha256_hex(&body) })
    })
    .await??)
}

pub async fn delete(root: &Path, key: &str, kind: &str, name: &str) -> AppResult<()> {
    let path = file_path(root, key, kind, name)?;
    Ok(tokio::fs::remove_file(path).await?)
}

pub async fn count(root: &Path) -> AppResult<usize> {
    let root = root.to_owned();
    Ok(tokio::task::spawn_blocking(move || subdirs(&players_dir(&root)).map(|d| d.len())).await??)
}
