//! `cargo run -p tft-embed --release --example parity -- <model.onnx> <crop.rgb> <w> <h>`: prints the first values.

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (w, h): (usize, usize) = (a[2].parse()?, a[3].parse()?);
    let raw = std::fs::read(&a[1])?;
    let px = raw.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    let mut e = tft_embed::Embedder::load(std::path::Path::new(&a[0]))?;
    let crop = tft_embed::Rgb { w, h, px };
    let t = std::time::Instant::now();
    let v = e.embed(&[crop])?;
    println!("{} ms", t.elapsed().as_millis());
    println!("{:?}", &v[0][..6]);
    std::fs::write("parity.out", v[0].iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"))?;
    Ok(())
}
