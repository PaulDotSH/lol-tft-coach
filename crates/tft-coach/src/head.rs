//! A classifier head over unit embeddings (`tools/tft-render/head.py`, trained on renders and
//! labelled real crops): one hidden layer (GELU) or none, softmax over the units. The naming
//! thresholds come from the evaluation run that wrote the file and live in its header.
//! "TFH1", then little endian: input dimension, hidden size (0: linear), units (u16 each), the
//! least top probability and the least lead over the second (f32 each), per unit its name
//! (u16 length, bytes), then f16s: W1 (hidden × input), b1, W2 (units × hidden), b2. A unit
//! named "-" is the reject class (no unit).

use half::f16;
use tft_model::Api;

const MAGIC: &[u8; 4] = b"TFH1";
/// The reject class's name.
pub const REJECT: &str = "-";

#[derive(Debug, Clone, PartialEq)]
pub struct Head {
    d: usize,
    hidden: usize,
    units: Vec<Api>,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    b2: Vec<f32>,
    /// A unit is named when its probability is at least `sim` and `margin` ahead of the next.
    pub sim: f32,
    pub margin: f32,
}

/// Abramowitz and Stegun 7.1.26 (error under 1.5e-7).
fn erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let y = 1.0 - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_8) * t - 0.284_496_74) * t + 0.254_829_6) * t * (-x * x).exp();
    y.copysign(x)
}

fn gelu(x: f32) -> f32 {
    0.5 * x * (1.0 + erf(x / std::f32::consts::SQRT_2))
}

fn layer(w: &[f32], b: &[f32], x: &[f32]) -> Vec<f32> {
    w.chunks(x.len()).zip(b).map(|(row, b)| row.iter().zip(x).map(|(w, x)| w * x).sum::<f32>() + b).collect()
}

impl Head {
    /// A head from its parts (W1 empty when `hidden` is 0); None when the sizes don't fit.
    #[allow(clippy::too_many_arguments)]
    pub fn new(d: usize, hidden: usize, units: Vec<Api>, w1: Vec<f32>, b1: Vec<f32>, w2: Vec<f32>, b2: Vec<f32>, (sim, margin): (f32, f32)) -> Option<Head> {
        let n = units.len();
        let into = if hidden == 0 { d } else { hidden };
        let ok = n > 0 && d > 0 && w1.len() == hidden * d && b1.len() == hidden && w2.len() == n * into && b2.len() == n;
        ok.then_some(Head { d, hidden, units, w1, b1, w2, b2, sim, margin })
    }

    /// None when the bytes aren't a head (empty: none shipped yet; another format; cut off).
    pub fn from_bytes(b: &[u8]) -> Option<Head> {
        let mut rest = b.strip_prefix(MAGIC)?;
        let mut take = |n: usize| -> Option<&[u8]> {
            let (a, r) = rest.split_at_checked(n)?;
            rest = r;
            Some(a)
        };
        let mut u16s = || take(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as usize);
        let (d, hidden, n) = (u16s()?, u16s()?, u16s()?);
        let mut f32s = || take(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
        let (sim, margin) = (f32s()?, f32s()?);
        let mut units = vec![];
        for _ in 0..n {
            let len = take(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as usize)?;
            units.push(String::from_utf8(take(len)?.to_vec()).ok()?);
        }
        let into = if hidden == 0 { d } else { hidden };
        let mut f16s = |k: usize| take(k * 2).map(|c| c.chunks(2).map(|h| f16::from_le_bytes([h[0], h[1]]).to_f32()).collect::<Vec<f32>>());
        let (w1, b1, w2, b2) = (f16s(hidden * d)?, f16s(hidden)?, f16s(n * into)?, f16s(n)?);
        if !rest.is_empty() {
            return None;
        }
        Head::new(d, hidden, units, w1, b1, w2, b2, (sim, margin))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        for v in [self.d, self.hidden, self.units.len()] {
            b.extend((v as u16).to_le_bytes());
        }
        b.extend(self.sim.to_le_bytes());
        b.extend(self.margin.to_le_bytes());
        for u in &self.units {
            b.extend((u.len() as u16).to_le_bytes());
            b.extend(u.as_bytes());
        }
        for v in self.w1.iter().chain(&self.b1).chain(&self.w2).chain(&self.b2) {
            b.extend(f16::from_f32(*v).to_le_bytes());
        }
        b
    }

    pub fn units(&self) -> &[Api] {
        &self.units
    }

    /// Every unit with its probability for `emb`, best first; empty when the dimension differs.
    pub fn scores(&self, emb: &[f32]) -> Vec<(Api, f32)> {
        if emb.len() != self.d {
            return vec![];
        }
        let logits = if self.hidden == 0 {
            layer(&self.w2, &self.b2, emb)
        } else {
            let h: Vec<f32> = layer(&self.w1, &self.b1, emb).into_iter().map(gelu).collect();
            layer(&self.w2, &self.b2, &h)
        };
        let top = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let e: Vec<f32> = logits.iter().map(|l| (l - top).exp()).collect();
        let sum: f32 = e.iter().sum();
        let mut out: Vec<(Api, f32)> = self.units.iter().cloned().zip(e.into_iter().map(|v| v / sum)).collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }

    /// The unit `emb` shows, when the head is sure enough by its own thresholds (never the reject class).
    pub fn name(&self, emb: &[f32]) -> Option<Api> {
        let s = self.scores(emb);
        let (api, p) = s.first()?;
        let next = s.get(1).map_or(0.0, |n| n.1);
        (*p >= self.sim && p - next >= self.margin && api != REJECT).then(|| api.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Head {
        let units = vec!["A".to_string(), "B".to_string(), REJECT.to_string()];
        let w1 = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.5, 0.5, 0.0];
        let w2 = vec![4.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0];
        Head::new(3, 4, units, w1, vec![0.0; 4], w2, vec![0.0; 3], (0.6, 0.2)).unwrap()
    }

    #[test]
    fn a_head_round_trips_with_its_thresholds() {
        let h = tiny();
        let back = Head::from_bytes(&h.to_bytes()).unwrap();
        assert_eq!((back.sim, back.margin, back.units().len()), (0.6, 0.2, 3));
        assert_eq!(back, h);
        assert!(Head::from_bytes(&[]).is_none());
        assert!(Head::from_bytes(&h.to_bytes()[..20]).is_none());
        assert!(Head::from_bytes(b"TFL2").is_none());
    }

    #[test]
    fn names_only_when_sure_and_never_the_reject_class() {
        let h = tiny();
        assert_eq!(h.name(&[1.0, 0.0, 0.0]).as_deref(), Some("A"));
        assert_eq!(h.name(&[0.0, 1.0, 0.0]).as_deref(), Some("B"));
        assert_eq!(h.name(&[0.0, 0.0, 1.0]), None);
        assert_eq!(h.name(&[0.5, 0.5, 0.0]), None);
        assert_eq!(h.name(&[1.0, 0.0]), None);
        let s = h.scores(&[1.0, 0.0, 0.0]);
        assert!((s.iter().map(|x| x.1).sum::<f32>() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn gelu_matches_the_exact_form() {
        for (x, want) in [(-3.0f32, -0.004_049_7), (-1.0, -0.158_655_25), (0.0, 0.0), (0.5, 0.345_731_14), (2.0, 1.954_5)] {
            assert!((gelu(x) - want).abs() < 1e-4, "{x}: {}", gelu(x));
        }
    }
}
