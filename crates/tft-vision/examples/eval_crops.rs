//! The looks eval set: replays one game's captures through the reader and tracker (single reads,
//! empty library) and writes every naming crop of all groups (`mine`, `combat`, `scout`, see
//! `common/crops.rs`) into `<out>/<game dir name>/`, replacing what was there. One game per run
//! (the embedder's runtime loads once per process).
//! `cargo run -p tft-vision --release --example eval_crops -- <out> <capture dir>`
//! Env: `TFT_MODEL` (required), `TFT_ME` (my name).

#[path = "common/crops.rs"]
mod crops;

use std::path::{Path, PathBuf};

fn main() -> anyhow::Result<()> {
    let set = tft_data::parse_set_data(&std::fs::read_to_string("fixtures/tft/cdragon_set18.json")?, tft_data::SET)?;
    let model = std::env::var("TFT_MODEL")?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, game] = &args[..] else { anyhow::bail!("usage: eval_crops <out> <capture dir>") };
    let name = Path::new(game).file_name().and_then(|n| n.to_str()).ok_or_else(|| anyhow::anyhow!("bad dir {game}"))?;
    let dir = Path::new(out).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    let mut crops = crops::Crops::open(dir)?;
    let embedder = tft_embed::Embedder::load(Path::new(&model))?;
    let mut reader = tft_vision::Reader::new(&set).with_me(std::env::var("TFT_ME").ok()).with_embedder(Some(embedder));
    let mut tracker = tft_coach::Tracker::default().single_reads();
    let mut files: Vec<PathBuf> = std::fs::read_dir(game)?.filter_map(|e| Some(e.ok()?.path())).filter(|p| p.extension().is_some_and(|x| x == "png")).collect();
    files.sort();
    let (mut scouting, mut fights) = (0u32, 0u32);
    for p in &files {
        let img = ocr::scoreboard::decode_png(&std::fs::read(p)?).ok_or_else(|| anyhow::anyhow!("bad png {p:?}"))?;
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        let f = ocr::Frame { width: img.w, height: img.h, bgra };
        let Some(snap) = reader.read_frame(&f, 0) else { continue };
        tracker.observe(&snap, &set);
        let frame = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        scouting += (snap.viewing_other && snap.enemies == 0) as u32;
        fights += !snap.foes.is_empty() as u32;
        crops.mine(&f, frame, &snap, &tracker)?;
        crops.others(&f, frame, &snap, &tracker)?;
    }
    let lib = tracker.library();
    println!("{name}: {} frames, {scouting} viewing another board, {fights} with red bars; crops {:?}; library {} looks of {} units", files.len(), crops.counts, lib.len(), lib.units());
    Ok(())
}
