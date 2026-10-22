//! League Client API via the lockfile. Read-only.

use crate::id;
use anyhow::{Context, Result, bail};
use base64::Engine;
use model::{ChampKey, ChampSelect, EogStats, Lane, Mode};
use reqwest::{StatusCode, header::AUTHORIZATION};
use serde_json::Value;
use std::time::Duration;

const LOCKFILE: &str = r"C:\Riot Games\League of Legends\lockfile";
const UX_EXE: &str = "LeagueClientUx.exe";

#[derive(Clone)]
pub struct Lcu {
    pub client: reqwest::Client,
    /// e.g. "https://127.0.0.1:2753"
    pub base: String,
    /// "Basic …" header value.
    pub auth: String,
}

/// Find a running client (lockfile in the default install dir, else via the
/// LeagueClientUx process command line). None when the client is closed.
///
/// Blocking: the process-list fallback may spawn a hidden PowerShell (~1 s),
/// but only when a `LeagueClientUx.exe` process exists. Call from a blocking task.
pub fn discover() -> Option<Lcu> {
    let procs = processes();
    let running = |pid: u32| procs.as_ref().is_none_or(|p| p.iter().any(|(id, _)| *id == pid));
    if let Some((pid, port, password)) = std::fs::read_to_string(LOCKFILE).ok().and_then(|s| parse_lockfile(&s))
        && running(pid)
    {
        return Some(Lcu::new(port, &password));
    }
    let ux_running = procs.as_ref().is_some_and(|p| p.iter().any(|(_, n)| n.eq_ignore_ascii_case(UX_EXE)));
    if !ux_running {
        return None;
    }
    let (port, token) = command_line(UX_EXE).and_then(|c| parse_ux_args(&c))?;
    Some(Lcu::new(port, &token))
}

/// `name:pid:port:password:protocol` → (pid, port, password).
pub(crate) fn parse_lockfile(s: &str) -> Option<(u32, u16, String)> {
    let f: Vec<&str> = s.trim().split(':').collect();
    let [_, pid, port, password, _] = f.as_slice() else { return None };
    Some((pid.parse().ok()?, port.parse().ok()?, password.to_string()))
}

/// `--app-port=` and `--remoting-auth-token=` from a LeagueClientUx command line.
pub(crate) fn parse_ux_args(cmd: &str) -> Option<(u16, String)> {
    let arg = |key: &str| {
        let rest = &cmd[cmd.find(key)? + key.len()..];
        let end = rest.find(|c: char| c == '"' || c.is_whitespace()).unwrap_or(rest.len());
        Some(rest[..end].to_string()).filter(|v| !v.is_empty())
    };
    Some((arg("--app-port=")?.parse().ok()?, arg("--remoting-auth-token=")?))
}

/// (pid, exe name) of every process; None off Windows or on failure.
#[cfg(windows)]
fn processes() -> Option<Vec<(u32, String)>> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
    // SAFETY: plain Win32 snapshot iteration; the entry is sized and the handle closed.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut out = Vec::with_capacity(256);
        let mut ok = Process32FirstW(snap, &mut e);
        while ok != 0 {
            let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            out.push((e.th32ProcessID, String::from_utf16_lossy(&e.szExeFile[..n])));
            ok = Process32NextW(snap, &mut e);
        }
        CloseHandle(snap);
        Some(out)
    }
}

#[cfg(not(windows))]
fn processes() -> Option<Vec<(u32, String)>> {
    None
}

/// Command line(s) of processes named `exe`, via a hidden PowerShell CIM query.
#[cfg(windows)]
pub(crate) fn command_line(exe: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = exe.replace('\'', "");
    let script = format!("(Get-CimInstance Win32_Process -Filter 'Name=''{exe}''').CommandLine");
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

#[cfg(not(windows))]
pub(crate) fn command_line(_exe: &str) -> Option<String> {
    None
}

impl Lcu {
    /// Client for `https://127.0.0.1:<port>` with basic auth `riot:<password>`.
    pub fn new(port: u16, password: &str) -> Lcu {
        let token = base64::engine::general_purpose::STANDARD.encode(format!("riot:{password}"));
        Lcu {
            client: crate::local_client(Duration::from_secs(2)),
            base: format!("https://127.0.0.1:{port}"),
            auth: format!("Basic {token}"),
        }
    }

    /// GET a JSON endpoint; 404 → Ok(None).
    pub async fn get(&self, path: &str) -> Result<Option<Value>> {
        let r = self.client.get(format!("{}{path}", self.base)).header(AUTHORIZATION, &self.auth).send().await?;
        if r.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let r = r.error_for_status().with_context(|| format!("LCU {path}"))?;
        Ok(Some(r.json().await.with_context(|| format!("LCU {path} body"))?))
    }

    /// `/lol-gameflow/v1/gameflow-phase`: "None", "Lobby", "ChampSelect", "InProgress", "EndOfGame", …
    pub async fn phase(&self) -> Result<String> {
        match self.get("/lol-gameflow/v1/gameflow-phase").await? {
            Some(Value::String(s)) => Ok(s),
            None => Ok("None".into()),
            Some(v) => bail!("unexpected gameflow-phase: {v}"),
        }
    }

    /// Queue id of the current gameflow session, if any.
    pub async fn queue_id(&self) -> Result<Option<i64>> {
        let s = self.get("/lol-gameflow/v1/session").await?;
        Ok(s.and_then(|s| s.pointer("/gameData/queue/id")?.as_i64()).filter(|&q| q > 0))
    }

    pub async fn champ_select(&self) -> Result<Option<ChampSelect>> {
        let (session, queue) = tokio::join!(self.get("/lol-champ-select/v1/session"), self.queue_id());
        let Some(session) = session? else { return Ok(None) };
        Ok(parse_champ_select(&session, queue.ok().flatten().unwrap_or(-1)))
    }

    pub async fn eog_stats(&self) -> Result<Option<EogStats>> {
        Ok(self.get("/lol-end-of-game/v1/eog-stats-block").await?.as_ref().and_then(parse_eog))
    }
}

fn arr(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}

/// Pure parser for `/lol-champ-select/v1/session` (tests use fixtures).
/// `queue_id` ≤ 0 means unknown; the session's own `queueId` is used then.
pub fn parse_champ_select(json: &Value, queue_id: i64) -> Option<ChampSelect> {
    let me_cell = json.get("localPlayerCellId")?.as_i64()?;
    let my_team = json.get("myTeam")?.as_array()?;
    let queue_id = if queue_id > 0 { queue_id } else { json["queueId"].as_i64().unwrap_or(queue_id) };
    let is_me = |p: &&Value| p["cellId"].as_i64() == Some(me_cell);
    let me = my_team.iter().find(is_me);

    let actions: Vec<&Value> = arr(&json["actions"]).flat_map(arr).collect();
    let mine = actions.iter().rev().find(|a| a["type"] == "pick" && a["actorCellId"].as_i64() == Some(me_cell));
    let team_champ = me.and_then(|p| id(&p["championId"]));
    let intent = me.and_then(|p| id(&p["championPickIntent"]));
    let (pick, hover) = match mine {
        Some(a) if a["completed"] == true => (id(&a["championId"]).or(team_champ), None),
        Some(a) => (None, id(&a["championId"]).or(team_champ).or(intent)),
        // No pick action (ARAM, all-random): the assigned champion is ours.
        None => (team_champ, team_champ.is_none().then_some(intent).flatten()),
    };

    let mut bans: Vec<ChampKey> = Vec::new();
    let ban_ids = ["myTeamBans", "theirTeamBans"].into_iter().flat_map(|k| arr(&json["bans"][k])).filter_map(id);
    let ban_actions = actions.iter().filter(|a| a["type"] == "ban" && a["completed"] == true).filter_map(|a| id(&a["championId"]));
    for b in ban_ids.chain(ban_actions) {
        if !bans.contains(&b) {
            bans.push(b);
        }
    }

    let bench = arr(&json["benchChampions"])
        .filter_map(|b| id(&b["championId"]))
        .chain(arr(&json["benchChampionIds"]).filter_map(id))
        .collect();

    Some(ChampSelect {
        queue_id,
        mode: Mode::from_queue(queue_id),
        my_lane: me.and_then(|p| Lane::from_riot(p["assignedPosition"].as_str()?)),
        my_hover: hover,
        my_pick: pick,
        locked: pick.is_some(),
        allies: my_team.iter().filter(|p| !is_me(p)).filter_map(|p| id(&p["championId"])).collect(),
        enemies: arr(&json["theirTeam"]).filter_map(|p| id(&p["championId"])).collect(),
        bans,
        bench,
        timer_phase: json["timer"]["phase"].as_str().unwrap_or_default().to_string(),
        ally_lanes: my_team
            .iter()
            .filter(|p| !is_me(p))
            .filter_map(|p| Some((id(&p["championId"])?, Lane::from_riot(p["assignedPosition"].as_str()?)?)))
            .collect(),
    })
}

/// Pure parser for `/lol-end-of-game/v1/eog-stats-block`.
pub fn parse_eog(json: &Value) -> Option<EogStats> {
    let teams = || arr(&json["teams"]);
    let local = json.get("localPlayer").filter(|v| v.is_object()).or_else(|| {
        let sid = id(&json["summonerId"]);
        let puuid = json["puuid"].as_str().filter(|s| !s.is_empty());
        teams().flat_map(|t| arr(&t["players"])).find(|p| {
            p["isLocalPlayer"] == true
                || (sid.is_some() && id(&p["summonerId"]) == sid)
                || (puuid.is_some() && p["puuid"].as_str() == puuid)
        })
    })?;
    let stats = &local["stats"];
    let stat = |k: &str| stats[k].as_f64().map(|v| v.max(0.0) as u32);
    let win = match (stat("WIN"), stat("LOSE")) {
        (Some(w), _) if w > 0 => true,
        (_, Some(l)) if l > 0 => false,
        (Some(_), _) => false,
        _ => {
            let tid = local["teamId"].as_i64();
            let mine = teams().find(|t| tid.is_some() && t["teamId"].as_i64() == tid).or_else(|| teams().find(|t| t["isPlayerTeam"] == true));
            mine.is_some_and(|t| t["isWinningTeam"] == true)
        }
    };
    let mode = match json["queueId"].as_i64().filter(|&q| q > 0) {
        Some(q) => Mode::from_queue(q),
        None if json["gameMode"] == "ARAM" => Mode::Aram,
        None => Mode::Rift,
    };
    let s = |k: &str| stat(k).unwrap_or(0);
    Some(EogStats {
        champ: id(&local["championId"])?,
        mode,
        win,
        duration_s: json["gameLength"].as_f64().unwrap_or(0.0).max(0.0) as u32,
        kills: s("CHAMPIONS_KILLED"),
        deaths: s("NUM_DEATHS"),
        assists: s("ASSISTS"),
        cs: s("MINIONS_KILLED") + s("NEUTRAL_MINIONS_KILLED"),
        gold: s("GOLD_EARNED"),
        damage_to_champs: s("TOTAL_DAMAGE_DEALT_TO_CHAMPIONS"),
        damage_taken: s("TOTAL_DAMAGE_TAKEN"),
        vision: s("VISION_SCORE"),
        items: arr(&local["items"]).filter_map(id).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockfile() {
        assert_eq!(parse_lockfile("LeagueClient:21420:52436:Xy-z_9AbC:https\n"), Some((21420, 52436, "Xy-z_9AbC".into())));
        assert_eq!(parse_lockfile("garbage"), None);
        assert_eq!(parse_lockfile(""), None);
    }

    #[test]
    fn ux_args() {
        let cmd = r#""C:/Riot Games/League of Legends/LeagueClientUx.exe" "--riotclient-auth-token=abc" "--riotclient-app-port=50123" "--remoting-auth-token=s3cr3t_Tok-en" "--app-port=52436" "--install-directory=C:\Riot Games\League of Legends""#;
        assert_eq!(parse_ux_args(cmd), Some((52436, "s3cr3t_Tok-en".into())));
        assert_eq!(parse_ux_args("--app-port=1 --remoting-auth-token=t"), Some((1, "t".into())));
        assert_eq!(parse_ux_args("LeagueClientUx.exe --app-port=52436"), None);
    }

    #[test]
    fn basic_auth() {
        let l = Lcu::new(2999, "pw");
        assert_eq!(l.base, "https://127.0.0.1:2999");
        assert_eq!(l.auth, "Basic cmlvdDpwdw==");
    }

    #[cfg(windows)]
    #[test]
    fn process_list_and_command_line() {
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_name().unwrap().to_str().unwrap();
        let procs = processes().unwrap();
        assert!(procs.iter().any(|(pid, n)| *pid == std::process::id() && n.eq_ignore_ascii_case(name)));
        let cmd = command_line(name).expect("own command line");
        assert!(cmd.to_ascii_lowercase().contains(&name.to_ascii_lowercase()), "{cmd}");
    }
}
