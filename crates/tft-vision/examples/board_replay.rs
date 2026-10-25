//! Replay saved captures through the reader and the tracker's board: prints each quiet read's
//! pieces with their names. `cargo run -p tft-vision --release --example board_replay -- <dir or png>...`
//! Env: `TFT_MODEL` (dinov2-small.onnx, required for looks), `TFT_ME` (my name),
//! `TFT_LIB` (library file, loaded when present and saved at the end), `TFT_LABELS` (JSON lines
//! of every named piece, for auditing), `TFT_STABLE=1` (wait for two still reads, as live),
//! `TFT_CROPS` (eval crops of my confirmed pieces, see `common/crops.rs`).

#[path = "common/crops.rs"]
mod crops;

use std::io::Write;
use tft_coach::board::Named;

fn main() -> anyhow::Result<()> {
    let set = tft_data::parse_set_data(&std::fs::read_to_string("fixtures/tft/cdragon_set18.json")?, tft_data::SET)?;
    let model = std::env::var("TFT_MODEL")?;
    let embedder = tft_embed::Embedder::load(std::path::Path::new(&model))?;
    let mut reader = tft_vision::Reader::new(&set).with_me(std::env::var("TFT_ME").ok()).with_embedder(Some(embedder));
    let lib_path = std::env::var("TFT_LIB").ok();
    let library = lib_path.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| tft_coach::Library::from_bytes(&b)).unwrap_or_default();
    println!("library: {} looks of {} units", library.len(), library.units());
    let mut tracker = tft_coach::Tracker::default().with_library(library);
    if std::env::var("TFT_STABLE").is_err() {
        tracker = tracker.single_reads();
    }
    let mut labels = std::env::var("TFT_LABELS").ok().map(std::fs::File::create).transpose()?;
    let mut crops = crops::Crops::from_env()?;
    let name = |api: &str| tft_coach::unit_name(&set, api).to_string();
    // A directory stands for its captures in order (one process: xargs would split a long list).
    let mut files: Vec<String> = vec![];
    for a in std::env::args().skip(1) {
        match std::fs::read_dir(&a) {
            Ok(d) => {
                let mut v: Vec<String> = d.filter_map(|e| Some(e.ok()?.path().to_str()?.to_string())).filter(|p| p.ends_with(".png")).collect();
                v.sort();
                files.extend(v);
            }
            Err(_) => files.push(a),
        }
    }
    for p in files {
        let img = ocr::scoreboard::decode_png(&std::fs::read(&p)?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        let f = ocr::Frame { width: img.w, height: img.h, bgra };
        let Some(snap) = reader.read_frame(&f, 0) else { continue };
        let before = tracker.board().pieces().len();
        tracker.observe(&snap, &set);
        let file = p.rsplit(['/', '\\']).next().unwrap_or(&p).to_string();
        if let Some(c) = &mut crops {
            c.mine(&f, file.trim_end_matches(".png"), &snap, &tracker)?;
        }
        let quiet = !snap.viewing_other && snap.enemies == 0 && !snap.units.is_empty() && snap.units.iter().all(|u| u.full);
        if !quiet {
            continue;
        }
        let stage = snap.stage.map_or("?".into(), |(a, b)| format!("{a}-{b}"));
        let pieces = tracker.board().pieces();
        let show: Vec<String> = pieces
            .iter()
            .map(|q| {
                let how = match q.named {
                    Named::No => "?",
                    Named::Looks => "~",
                    Named::Panel => "=",
                    Named::Bought => "$",
                };
                let at = q.bench.map_or("B".into(), |s| format!("b{s}"));
                format!("{at}:{}{how}★{}", q.api.as_deref().map(name).unwrap_or_default(), q.star)
            })
            .collect();
        let panel: Vec<String> = snap.traits.iter().map(|(t, n)| format!("{}{n}", tft_coach::trait_name(&set, t))).collect();
        println!("{file} {stage} lvl {:?} pieces {}→{} | {}\n    panel {}", snap.level, before, pieces.len(), show.join(" "), panel.join(" "));
        if let Some(f) = &mut labels {
            for q in pieces {
                writeln!(f, "{}", serde_json::json!({"file": p, "x": q.x, "y": q.y, "api": q.api, "named": format!("{:?}", q.named), "star": q.star, "bench": q.bench}))?;
            }
        }
    }
    if let Some(c) = &crops {
        println!("crops: {:?}", c.counts);
    }
    println!("library now: {} looks of {} units", tracker.library().len(), tracker.library().units());
    if let Some(p) = lib_path {
        std::fs::write(p, tracker.library().to_bytes())?;
    }
    Ok(())
}
