//! Matchup tips ahead of time for the most played pairings, answered by Claude Code subagents
//! instead of one `claude -p` per pairing in the app.
//! `cargo run -p lol-coach --example ai_tips_batch -- queue <dir> <round> <count> <agents>`:
//! ranks pairings by lolalytics games, lane by lane in turn, and writes the app's prompt for the next `count` without
//! tips into `<dir>/todo/<round>-a<agent>-<nn>.txt`, ten to a file.
//! `... -- keep <dir>`: checks `<dir>/answers/<pairing>.json` the way the app checks an answer
//! and keeps the good ones under `%APPDATA%\lol-coach\ai-tips`.

use model::*;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::Path;

const TIER: &str = "emerald_plus";
const REGION: &str = "all";
/// lolalytics' last 30 days: a patch's first days have no counts yet.
const STATS_PATCH: &str = "30";
const CHAMPS_PER_LANE: usize = 45;
const PER_FILE: usize = 10;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let kept = dirs::config_dir().unwrap_or_default().join("lol-coach").join("ai-tips");
    match args.as_slice() {
        ["queue", dir, round, count, agents] => queue(Path::new(dir), &kept, round, count.parse()?, agents.parse()?).await,
        ["keep", dir] => keep(Path::new(dir), &kept).await,
        _ => anyhow::bail!("queue <dir> <round> <count> <agents> | keep <dir>"),
    }
}

fn patch(sd: &StaticData) -> String {
    sd.version.splitn(3, '.').take(2).collect::<Vec<_>>().join(".")
}

async fn queue(dir: &Path, kept: &Path, round: &str, count: usize, agents: usize) -> anyhow::Result<()> {
    let http = data::Http::new(dir.join("cache"))?;
    let sd = data::ddragon::load(&http).await?;
    let patch = patch(&sd);
    let notes = data::notes::lane_notes()?;
    let mut rows = Vec::new();
    for lane in Lane::ALL {
        let mut tier = data::lolalytics::tier_list(&http, lane, TIER, REGION, STATS_PATCH).await?;
        tier.sort_by_key(|t| Reverse(t.games));
        for t in tier.iter().take(CHAMPS_PER_LANE) {
            let Some(c) = sd.champs.get(&t.champ) else { continue };
            for &vs_lane in coach::opposing_lanes(lane).iter() {
                match data::lolalytics::counters_vs(&http, c, lane, vs_lane, TIER, REGION, STATS_PATCH).await {
                    Ok(ms) => rows.extend(ms.iter().map(|m| (m.games, t.champ, m.champ, lane, vs_lane))),
                    Err(e) => eprintln!("counters {} {}: {e:#}", c.id, lane.as_str()),
                }
            }
        }
    }
    rows.sort_by_key(|r| Reverse(r.0));
    eprintln!("{} pairings counted", rows.len());
    let mut nth = [0usize; 5];
    let mut rows: Vec<_> = rows
        .into_iter()
        .map(|r| {
            let n = &mut nth[Lane::ALL.iter().position(|&l| l == r.3).unwrap_or(0)];
            *n += 1;
            (*n, r)
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    let rows = rows.into_iter().map(|r| r.1);
    let (prompts, answers, todo) = (dir.join("prompts"), dir.join("answers"), dir.join("todo"));
    for d in [&prompts, &answers, &todo] {
        std::fs::create_dir_all(d)?;
    }
    let mut seen = HashSet::new();
    let mut new: Vec<(String, String)> = Vec::new();
    for (games, me, them, lane, vs_lane) in rows {
        if new.len() == count {
            break;
        }
        let (Some(a), Some(b)) = (sd.champs.get(&me), sd.champs.get(&them)) else { continue };
        let key = format!("{}-{}-{}", a.id, b.id, lane.as_str());
        if !seen.insert(key.clone()) || notes.pair(&a.id, &b.id).len() > 1 || kept.join(format!("{key}.json")).exists() || prompts.join(format!("{key}.txt")).exists() {
            continue;
        }
        let (mine, theirs) = match (data::spells::spells(&http, &sd.version, &a.id).await, data::spells::spells(&http, &sd.version, &b.id).await) {
            (Ok(m), Ok(t)) => (m, t),
            (m, t) => {
                eprintln!("spells {key}: {:?}", m.err().or(t.err()));
                continue;
            }
        };
        let numbers = match data::lolalytics::matchup(&http, a, b, lane, vs_lane, TIER, REGION, STATS_PATCH).await {
            Ok(page) => coach::pair_tips::numbers(&sd, &page),
            Err(e) => {
                eprintln!("matchup page {key}: {e:#}");
                vec![]
            }
        };
        let p = data::ai_tips::Pairing { me: &a.name, me_id: &a.id, them: &b.name, them_id: &b.id, lane, vs_lane, patch: &patch, my_spells: &mine, their_spells: &theirs, numbers: &numbers };
        let prompt = data::ai_tips::prompt(&p);
        std::fs::write(prompts.join(format!("{key}.txt")), &prompt)?;
        println!("{key}\t{games}");
        new.push((key, prompt));
    }
    for agent in 0..agents {
        let mine: Vec<&(String, String)> = new.iter().skip(agent).step_by(agents).collect();
        for (n, chunk) in mine.chunks(PER_FILE).enumerate() {
            let text: String = chunk.iter().map(|(key, prompt)| format!("##### {key}\nAnswer file: {}\n\n{prompt}\n", answers.join(format!("{key}.json")).display())).collect();
            std::fs::write(todo.join(format!("{round}-a{}-{:02}.txt", agent + 1, n + 1)), text)?;
        }
    }
    Ok(())
}

async fn keep(dir: &Path, kept: &Path) -> anyhow::Result<()> {
    let http = data::Http::new(dir.join("cache"))?;
    let sd = data::ddragon::load(&http).await?;
    let patch = patch(&sd);
    let runes: Vec<&str> = sd.runes.values().map(|r| r.name.as_str()).collect();
    std::fs::create_dir_all(kept)?;
    let (mut new, mut bad) = (0, 0);
    for entry in std::fs::read_dir(dir.join("answers"))? {
        let path = entry?.path();
        let Some(key) = path.file_stem().and_then(|s| s.to_str()).filter(|_| path.extension().is_some_and(|e| e == "json")) else { continue };
        let out = kept.join(format!("{key}.json"));
        if out.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let written = serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|v| Some(v["tips"].as_array()?.len())).unwrap_or(0);
        let problem = match data::ai_tips::parse_answer(&text, &runes) {
            _ if !dir.join("prompts").join(format!("{key}.txt")).exists() => "not a queued pairing".to_string(),
            Ok(tips) if tips.len() == written && tips.len() >= 5 && tips[0].keystone.is_none() => {
                let tips: Vec<_> = tips.iter().map(|t| serde_json::json!({ "text": t.text, "keystone": t.keystone })).collect();
                std::fs::write(&out, serde_json::to_string_pretty(&serde_json::json!({ "model": data::ai_tips::MODEL, "patch": patch, "tips": tips }))?)?;
                new += 1;
                continue;
            }
            Ok(tips) => format!("{} of {written} tips usable (too long, unknown keystone, fewer than 5, or a keystone tip first)", tips.len()),
            Err(e) => format!("{e:#}"),
        };
        println!("{key}: {problem}");
        bad += 1;
    }
    println!("{new} kept, {bad} not kept");
    Ok(())
}
