//! Matchup tips written by Claude for one exact pairing, asked through the Claude Code CLI
//! (`claude -p`, the user's own login: no API key) and kept on disk so each pairing is asked
//! once. The answer is `{"tips": [{"text", "keystone"}]}`; tips that are too long or name a
//! keystone Data Dragon doesn't know are dropped.

use model_static::{Lane, PairTip, SpellInfo};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

/// The user asked for Mythos 5.2.
pub const MODEL: &str = "claude-mythos-5-2";
const SYSTEM: &str = "You write short, accurate League of Legends laning tips. Answer with JSON only.";
/// The model thinks before it answers: a run takes about half a minute.
const TIMEOUT: Duration = Duration::from_secs(180);
/// A tip is one or two short sentences on the matchup card.
pub const MAX_TIP: usize = 200;
const MAX_TIPS: usize = 8;

/// What the model is told about a pairing.
pub struct Pairing<'a> {
    /// Champion names as shown in game, and Data Dragon ids (the file name).
    pub me: &'a str,
    pub me_id: &'a str,
    pub them: &'a str,
    pub them_id: &'a str,
    pub lane: Lane,
    pub vs_lane: Lane,
    /// "16.22".
    pub patch: &'a str,
    /// Q W E R of both; empty when not fetched.
    pub my_spells: &'a [SpellInfo],
    pub their_spells: &'a [SpellInfo],
    /// Lines of numbers on this matchup (win rate, items), as far as known.
    pub numbers: &'a [String],
}

#[derive(Serialize, Deserialize)]
struct Tip {
    text: String,
    #[serde(default)]
    keystone: Option<String>,
}

/// The file kept for a pairing.
#[derive(Serialize, Deserialize)]
struct Kept {
    model: String,
    patch: String,
    tips: Vec<Tip>,
}

#[derive(Deserialize)]
struct Answer {
    tips: Vec<Tip>,
}

fn lane_name(l: Lane) -> &'static str {
    match l {
        Lane::Top => "top lane",
        Lane::Jungle => "jungle",
        Lane::Middle => "mid lane",
        Lane::Bottom => "bot lane as the bot laner",
        Lane::Support => "bot lane as the support",
    }
}

pub fn prompt(p: &Pairing) -> String {
    let spells = |who: &str, s: &[SpellInfo]| {
        let names: Vec<String> = s.iter().zip(["Q", "W", "E", "R"]).map(|(s, k)| format!("{k} {}", s.name)).collect();
        if names.is_empty() { String::new() } else { format!("{who}'s spells: {}.\n", names.join(", ")) }
    };
    let facing = if p.lane == p.vs_lane { String::new() } else { format!(" (they play {})", lane_name(p.vs_lane)) };
    let numbers = if p.numbers.is_empty() { String::new() } else { format!("Numbers on this matchup (lolalytics):\n- {}\n", p.numbers.join("\n- ")) };
    format!(
        "League of Legends, patch {patch}, {lane}. I play {me} against {them}{facing}.\n\n\
         Write the laning tips I should have in front of me for exactly this matchup: how the two kits interact, when to trade and when not to, \
         what to do with each keystone I might take, and the level and item timings that change the lane.\n\n\
         {mine}{theirs}{numbers}\n\
         Rules:\n\
         - 5 to 7 tips, the most important first. Each tip is one or two short sentences, at most 180 characters, plain text.\n\
         - Name spells by key (Q, W, E, R) and say whose they are. Be concrete: no advice that would fit any matchup.\n\
         - Up to two tips may hold only with one keystone: give that keystone's exact in-game name in \"keystone\". Otherwise \"keystone\" is null.\n\
         - Only say what you are confident is true of the current kits. Leave out anything you are unsure about.\n\n\
         Answer with JSON only, no prose and no code fence:\n\
         {{\"tips\": [{{\"text\": \"...\", \"keystone\": null}}]}}\n",
        patch = p.patch,
        lane = lane_name(p.lane),
        me = p.me,
        them = p.them,
        mine = spells(p.me, p.my_spells),
        theirs = spells(p.them, p.their_spells),
    )
}

/// Tips from the model's answer text (JSON, with or without prose or a code fence around it).
/// `runes`: the rune names a keystone may be.
pub fn parse_answer(text: &str, runes: &[&str]) -> anyhow::Result<Vec<PairTip>> {
    let (from, to) = text.find('{').zip(text.rfind('}')).ok_or_else(|| anyhow::anyhow!("no JSON in the answer"))?;
    let answer: Answer = serde_json::from_str(&text[from..=to])?;
    let tips: Vec<PairTip> = answer
        .tips
        .into_iter()
        .map(|t| (t.text.trim().to_owned(), t.keystone.filter(|k| !k.trim().is_empty())))
        .filter(|(text, _)| !text.is_empty() && text.chars().count() <= MAX_TIP)
        .filter_map(|(text, keystone)| match keystone {
            Some(k) => Some(PairTip { text, keystone: Some(runes.iter().find(|r| r.eq_ignore_ascii_case(k.trim()))?.to_string()) }),
            None => Some(PairTip { text, keystone: None }),
        })
        .take(MAX_TIPS)
        .collect();
    anyhow::ensure!(!tips.is_empty(), "no usable tip in the answer");
    Ok(tips)
}

/// The CLI's `--output-format json` reply: `result` holds the answer text.
pub fn parse_cli(stdout: &str, runes: &[&str]) -> anyhow::Result<Vec<PairTip>> {
    let reply: serde_json::Value = serde_json::from_str(stdout.trim())?;
    anyhow::ensure!(reply["is_error"] != true, "claude: {}", reply["result"].as_str().unwrap_or("error"));
    parse_answer(reply["result"].as_str().ok_or_else(|| anyhow::anyhow!("no result in the CLI reply"))?, runes)
}

fn file(dir: &Path, p: &Pairing) -> PathBuf {
    dir.join(format!("{}-{}-{}.json", p.me_id, p.them_id, p.lane.as_str()))
}

/// The tips kept for the pairing, if any.
pub fn kept(dir: &Path, p: &Pairing) -> Option<Vec<PairTip>> {
    let k: Kept = serde_json::from_str(&std::fs::read_to_string(file(dir, p)).ok()?).ok()?;
    Some(k.tips.into_iter().map(|t| PairTip { text: t.text, keystone: t.keystone }).collect())
}

fn keep(dir: &Path, p: &Pairing, tips: &[PairTip]) -> anyhow::Result<()> {
    let k = Kept { model: MODEL.into(), patch: p.patch.into(), tips: tips.iter().map(|t| Tip { text: t.text.clone(), keystone: t.keystone.clone() }).collect() };
    std::fs::create_dir_all(dir)?;
    Ok(std::fs::write(file(dir, p), serde_json::to_string_pretty(&k)?)?)
}

/// Ask the model through `claude -p` (no tools, no MCP servers, nothing saved by the CLI),
/// with the prompt on stdin and `dir` as the working directory.
async fn ask(dir: &Path, prompt: &str) -> anyhow::Result<String> {
    let mut cmd = tokio::process::Command::new(if cfg!(windows) { "claude.cmd" } else { "claude" });
    cmd.args(["-p", "--model", MODEL, "--output-format", "json", "--effort", "low", "--system-prompt", SYSTEM])
        .args(["--strict-mcp-config", "--no-session-persistence", "--disable-slash-commands", "--tools", ""])
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // No console window over the game.
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let mut child = cmd.spawn().map_err(|e| anyhow::anyhow!("starting the Claude Code CLI: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes()).await?;
    }
    let out = tokio::time::timeout(TIMEOUT, child.wait_with_output()).await.map_err(|_| anyhow::anyhow!("claude gave no answer in {TIMEOUT:?}"))??;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    anyhow::ensure!(out.status.success() || !stdout.trim().is_empty(), "claude exited with {}: {}", out.status, String::from_utf8_lossy(&out.stderr).trim());
    Ok(stdout)
}

/// The pairing's tips: the kept ones, else asked of the model and kept under `dir`.
pub async fn tips(dir: &Path, p: &Pairing<'_>, runes: &[&str]) -> anyhow::Result<Vec<PairTip>> {
    if let Some(t) = kept(dir, p) {
        return Ok(t);
    }
    std::fs::create_dir_all(dir)?;
    let started = std::time::Instant::now();
    let tips = parse_cli(&ask(dir, &prompt(p)).await?, runes)?;
    tracing::info!("AI tips {} vs {}: {} tips in {:?}", p.me_id, p.them_id, tips.len(), started.elapsed());
    keep(dir, p, &tips)?;
    Ok(tips)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spell(name: &str) -> SpellInfo {
        SpellInfo { name: name.into(), icon: String::new(), cooldown: vec![] }
    }

    fn pairing<'a>(mine: &'a [SpellInfo], numbers: &'a [String]) -> Pairing<'a> {
        Pairing { me: "Malphite", me_id: "Malphite", them: "Sett", them_id: "Sett", lane: Lane::Top, vs_lane: Lane::Top, patch: "16.22", my_spells: mine, their_spells: &[], numbers }
    }

    #[test]
    fn prompt_names_the_pairing_spells_and_numbers() {
        let mine = [spell("Seismic Shard"), spell("Thunderclap"), spell("Ground Slam"), spell("Unstoppable Force")];
        let numbers = ["55.1% into Sett against 53.4% across all matchups".to_string()];
        let p = prompt(&pairing(&mine, &numbers));
        assert!(p.starts_with("League of Legends, patch 16.22, top lane. I play Malphite against Sett.\n"), "{p}");
        assert!(p.contains("Malphite's spells: Q Seismic Shard, W Thunderclap, E Ground Slam, R Unstoppable Force.\n"));
        assert!(!p.contains("Sett's spells") && p.contains("- 55.1% into Sett against 53.4% across all matchups\n"));
        assert!(p.ends_with("{\"tips\": [{\"text\": \"...\", \"keystone\": null}]}\n"));
        let support = Pairing { lane: Lane::Support, vs_lane: Lane::Bottom, ..pairing(&[], &[]) };
        assert!(prompt(&support).contains("bot lane as the support. I play Malphite against Sett (they play bot lane as the bot laner)."));
    }

    #[test]
    fn answer_keeps_good_tips_and_known_keystones() {
        let runes = ["Arcane Comet", "Grasp of the Undying"];
        let long = "x".repeat(MAX_TIP + 1);
        let text = format!(
            "```json\n{{\"tips\": [{{\"text\": \" Poke with your Q. \", \"keystone\": null}}, {{\"text\": \"Q then step back.\", \"keystone\": \"arcane comet\"}}, \
             {{\"text\": \"Made up.\", \"keystone\": \"Thunderlord\"}}, {{\"text\": \"{long}\"}}, {{\"text\": \"\", \"keystone\": null}}, {{\"text\": \"No keystone field.\"}}]}}\n```"
        );
        let tips = parse_answer(&text, &runes).unwrap();
        let got: Vec<(&str, Option<&str>)> = tips.iter().map(|t| (t.text.as_str(), t.keystone.as_deref())).collect();
        assert_eq!(got, [("Poke with your Q.", None), ("Q then step back.", Some("Arcane Comet")), ("No keystone field.", None)]);
        assert!(parse_answer("I can't help with that.", &runes).is_err());
        assert!(parse_answer("{\"tips\": []}", &runes).is_err());

        let cli = serde_json::json!({ "type": "result", "is_error": false, "result": "{\"tips\":[{\"text\":\"E when Sett starts punching.\",\"keystone\":null}]}" });
        assert_eq!(parse_cli(&cli.to_string(), &runes).unwrap()[0].text, "E when Sett starts punching.");
        let failed = serde_json::json!({ "type": "result", "is_error": true, "result": "Not logged in" });
        assert!(parse_cli(&failed.to_string(), &runes).unwrap_err().to_string().contains("Not logged in"));
        assert!(parse_cli("", &runes).is_err());
    }

    #[test]
    fn kept_tips_are_read_back_without_asking() {
        let dir = std::env::temp_dir().join(format!("lol-coach-ai-tips-{}", std::process::id()));
        let p = pairing(&[], &[]);
        assert!(kept(&dir, &p).is_none());
        let tips = vec![PairTip { text: "Poke with your Q.".into(), keystone: None }, PairTip { text: "Q then step back.".into(), keystone: Some("Arcane Comet".into()) }];
        keep(&dir, &p, &tips).unwrap();
        assert!(dir.join("Malphite-Sett-top.json").exists());
        assert_eq!(kept(&dir, &p), Some(tips));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    #[ignore = "asks Claude through the Claude Code CLI"]
    async fn live_tips_are_asked_once_and_kept() {
        let dir = std::env::temp_dir().join(format!("lol-coach-ai-tips-live-{}", std::process::id()));
        let mine = [spell("Seismic Shard"), spell("Thunderclap"), spell("Ground Slam"), spell("Unstoppable Force")];
        let p = pairing(&mine, &[]);
        let runes = ["Arcane Comet", "Grasp of the Undying", "Aftershock"];
        let tips = tips(&dir, &p, &runes).await.unwrap();
        assert!(tips.len() >= 3 && tips[0].keystone.is_none(), "{tips:?}");
        assert_eq!(kept(&dir, &p), Some(tips.clone()));
        let started = std::time::Instant::now();
        assert_eq!(super::tips(&dir, &p, &runes).await.unwrap(), tips);
        assert!(started.elapsed() < Duration::from_secs(1), "read from disk");
        for t in &tips {
            println!("{:?} {}", t.keystone, t.text);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
