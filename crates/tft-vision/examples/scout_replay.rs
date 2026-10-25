//! Replay saved captures through the reader and the tracker: prints each read of other players'
//! units (scouted boards, my combat opponent's red bars) with the names the looks and their
//! trait panel give, then copies held by others. Builds the looks from my own board like
//! `board_replay`. `cargo run -p tft-vision --release --example scout_replay -- <dir or png>...`
//! Env: `TFT_MODEL` (required), `TFT_ME`, `TFT_LIB` (library, loaded when present; saved at the
//! end unless `TFT_LIB_KEEP=1`), `TFT_STABLE=1` (two still reads, as live), `TFT_LABELS` (JSON
//! lines of every named unit of others, for auditing), `TFT_EMBS` (JSON lines of every bar's
//! embedding, green and red, for measuring looks offline).

use std::io::Write;

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
    let mut embs = std::env::var("TFT_EMBS").ok().map(std::fs::File::create).transpose()?;
    let name = |api: &str| tft_coach::unit_name(&set, api).to_string();
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
    for (k, p) in files.iter().enumerate() {
        let img = ocr::scoreboard::decode_png(&std::fs::read(p)?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        let Some(snap) = reader.read_frame(&ocr::Frame { width: img.w, height: img.h, bgra }, 0) else { continue };
        tracker.observe(&snap, &set);
        if let Some(f) = &mut embs {
            for (red, u) in snap.units.iter().map(|u| (false, u)).chain(snap.foes.iter().map(|u| (true, u))) {
                let j = serde_json::json!({"k": k, "file": p, "stage": snap.stage, "other": snap.viewing_other, "who": snap.viewing, "red": red, "x": u.x, "y": u.y, "star": u.star, "bench": u.bench, "full": u.full, "emb": u.emb});
                writeln!(f, "{j}")?;
            }
        }
        if !snap.viewing_other && snap.foes.is_empty() {
            continue;
        }
        let file = p.rsplit(['/', '\\']).next().unwrap_or(p).to_string();
        let stage = snap.stage.map_or("?".into(), |(a, b)| format!("{a}-{b}"));
        let what = if snap.viewing_other { "scout" } else { "combat" };
        let who = snap.viewing.clone().unwrap_or_else(|| "?".into());
        let read = tracker.scout().last_read();
        let show: Vec<String> = read
            .iter()
            .map(|u| {
                let at = if u.bench { "b" } else { "B" };
                format!("{at}({:.0},{:.0}):{}{}★{}", u.x, u.y, u.api.as_deref().map(name).unwrap_or_default(), u.how, u.star)
            })
            .collect();
        println!(
            "#{k} {file} {stage} {what} {who} green {} red {} enemies {} | {}",
            snap.units.len(),
            snap.foes.len(),
            snap.enemies,
            show.join(" ")
        );
        if snap.viewing_other {
            let panel: Vec<String> = snap.traits.iter().map(|(t, n)| format!("{}{n}", tft_coach::trait_name(&set, t))).collect();
            println!("    panel {} (+{})", panel.join(" "), snap.traits_hidden);
        }
        if let Some(f) = &mut labels {
            for u in &read {
                writeln!(f, "{}", serde_json::json!({"k": k, "file": p, "what": what, "who": who, "x": u.x, "y": u.y, "api": u.api, "how": u.how, "star": u.star, "bench": u.bench}))?;
            }
        }
    }
    println!("held by others:");
    for p in tracker.scout().players() {
        let units: Vec<String> = p.units().iter().map(|(a, c)| format!("{}×{c}", name(a))).collect();
        println!("  {} (seen {}): {}", p.name.as_deref().unwrap_or("?"), p.seen, units.join(" "));
    }
    if let Some(p) = lib_path.filter(|_| std::env::var("TFT_LIB_KEEP").is_err()) {
        std::fs::write(p, tracker.library().to_bytes())?;
    }
    Ok(())
}
