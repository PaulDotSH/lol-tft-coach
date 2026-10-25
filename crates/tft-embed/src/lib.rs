//! Image embeddings for telling TFT units apart: DINOv2-small (ONNX, run by ONNX Runtime)
//! turns a unit's crop into a unit vector; crops of the same unit land close together.
//! The model file is fetched by the app (`MODEL_URL`) and loaded from disk.

use ort::session::Session;
use ort::value::Tensor;

/// DINOv2-small, fp32 (88 MB): int8 runs no faster on CPU and agrees less.
pub const MODEL_URL: &str = "https://huggingface.co/onnx-community/dinov2-small/resolve/main/onnx/model.onnx";
pub const MODEL_FILE: &str = "dinov2-small.onnx";
/// Crops are resized to this square (8×8 patches of 14 px).
pub const SIZE: usize = 112;
/// CLS token and the centre-weighted mean of the patch tokens, 384 each.
pub const DIM: usize = 768;
/// Patch weights fall off as a Gaussian of this many patches from the centre (the unit
/// stands in the middle of its crop; neighbours and the floor fill the edges).
const SIGMA: f32 = 2.0;
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// An RGB crop, top-down rows.
pub struct Rgb {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

pub struct Embedder {
    session: Session,
}

impl Embedder {
    pub fn load(model: &std::path::Path) -> anyhow::Result<Embedder> {
        let env = ort::init().with_name("tft-embed").build()?;
        let session = Session::builder(&env)?
            .with_intra_threads(2)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .commit_from_file(model)?;
        Ok(Embedder { session })
    }

    /// One unit vector per crop, in order.
    pub fn embed(&mut self, crops: &[Rgb]) -> anyhow::Result<Vec<Vec<f32>>> {
        if crops.is_empty() {
            return Ok(vec![]);
        }
        let mut x = Vec::with_capacity(crops.len() * 3 * SIZE * SIZE);
        for c in crops {
            let r = resize(c);
            for ch in 0..3 {
                x.extend(r.iter().map(|p| (p[ch] / 255.0 - MEAN[ch]) / STD[ch]));
            }
        }
        let input = Tensor::from_array(([crops.len(), 3, SIZE, SIZE], x))?;
        let out = self.session.run(ort::inputs!["pixel_values" => input])?;
        let (shape, v) = out["last_hidden_state"].try_extract_tensor::<f32>()?;
        let (tokens, d) = (shape[1] as usize, shape[2] as usize);
        let w = weights(tokens - 1);
        Ok((0..crops.len())
            .map(|i| {
                let t = &v[i * tokens * d..(i + 1) * tokens * d];
                let mut e = t[..d].to_vec();
                e.extend((0..d).map(|j| w.iter().enumerate().map(|(k, w)| w * t[(k + 1) * d + j]).sum::<f32>()));
                normalise(&mut e);
                e
            })
            .collect())
    }
}

/// Gaussian weights over a square grid of `n` patches, summing to 1.
fn weights(n: usize) -> Vec<f32> {
    let side = (n as f32).sqrt().round() as usize;
    let c = (side as f32 - 1.0) / 2.0;
    let w: Vec<f32> = (0..n)
        .map(|k| {
            let (x, y) = ((k % side) as f32 - c, (k / side) as f32 - c);
            (-(x * x + y * y) / (2.0 * SIGMA * SIGMA)).exp()
        })
        .collect();
    let sum: f32 = w.iter().sum();
    w.into_iter().map(|v| v / sum).collect()
}

fn normalise(e: &mut [f32]) {
    let n = e.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
    e.iter_mut().for_each(|v| *v /= n);
}

/// Cosine similarity of two unit vectors.
pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Bilinear resize to SIZE×SIZE (pixel centres aligned, like PIL's BILINEAR when enlarging).
fn resize(c: &Rgb) -> Vec<[f32; 3]> {
    let at = |x: usize, y: usize| c.px[y.min(c.h - 1) * c.w + x.min(c.w - 1)];
    let mut out = Vec::with_capacity(SIZE * SIZE);
    for oy in 0..SIZE {
        let fy = ((oy as f32 + 0.5) * c.h as f32 / SIZE as f32 - 0.5).max(0.0);
        let (y0, ty) = (fy as usize, fy.fract());
        for ox in 0..SIZE {
            let fx = ((ox as f32 + 0.5) * c.w as f32 / SIZE as f32 - 0.5).max(0.0);
            let (x0, tx) = (fx as usize, fx.fract());
            let (a, b, cc, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
            out.push(std::array::from_fn(|ch| {
                let top = a[ch] as f32 * (1.0 - tx) + b[ch] as f32 * tx;
                let bot = cc[ch] as f32 * (1.0 - tx) + d[ch] as f32 * tx;
                top * (1.0 - ty) + bot * ty
            }));
        }
    }
    out
}
