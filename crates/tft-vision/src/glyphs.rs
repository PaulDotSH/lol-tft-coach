//! Stage ("2-1") and gold ("6") by their glyphs: Windows OCR drops lone digits and "N-1".
//! Lit columns split the region into glyphs; each is scaled to one size and matched against
//! templates cut from real captures (`examples/glyphs.rs`).

use crate::Mask;
use ocr::Frame;

/// The top bar where the stage sits (it moves right in stage 1, which has fewer round icons).
const STAGE_BAR: (u32, u32, u32, u32) = (960, 6, 260, 40);
/// Gold digits, right of the coin.
const GOLD: (u32, u32, u32, u32) = (1218, 1178, 90, 40);
/// Glyphs are compared at this size.
const W: u32 = 12;
const H: u32 = 18;
/// Digits are at least this tall (base px); the dash at most DASH_H.
const DIGIT_H: u32 = 14;
const DASH_H: u32 = 6;
/// Glyphs of one number are at most this far apart (base px).
const GAP: u32 = 8;
/// Overlap a template match needs.
const MATCH: f32 = 0.6;
/// "kind char bits" (kind s = stage, g = gold), W×H bits each.
const TEMPLATES: &str = include_str!("glyphs.txt");

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Stage,
    Gold,
}

impl Kind {
    fn tag(self) -> char {
        match self {
            Kind::Stage => 's',
            Kind::Gold => 'g',
        }
    }

    /// The stage is cream, gold near white.
    fn lit(self, p: &[u8]) -> bool {
        let (b, g, r) = (p[0] as u32, p[1] as u32, p[2] as u32);
        match self {
            Kind::Stage => r >= 170 && g >= 150 && b >= 100 && r >= b + 25,
            Kind::Gold => r + g + b >= 500,
        }
    }

    fn region(self) -> (u32, u32, u32, u32) {
        match self {
            Kind::Stage => STAGE_BAR,
            Kind::Gold => GOLD,
        }
    }
}

/// One glyph: left edge and size in base px, and its W×H mask.
pub struct Glyph {
    pub x: u32,
    pub w: u32,
    pub h: u32,
    pub mask: Mask,
}

/// The glyphs of `kind`'s region, left to right.
pub fn glyphs(f: &Frame, scale: f32, kind: Kind) -> Vec<Glyph> {
    let (rx, ry, rw, rh) = kind.region();
    let lit = |x: u32, y: u32| {
        let sx = (((rx + x) as f32 * scale) as u32).min(f.width - 1);
        let sy = (((ry + y) as f32 * scale) as u32).min(f.height - 1);
        let i = ((sy * f.width + sx) * 4) as usize;
        kind.lit(&f.bgra[i..i + 4])
    };
    let on: Vec<Vec<bool>> = (0..rh).map(|y| (0..rw).map(|x| lit(x, y)).collect()).collect();
    let col = |x: u32| (0..rh).any(|y| on[y as usize][x as usize]);
    let mut out = vec![];
    let mut x = 0;
    while x < rw {
        if !col(x) {
            x += 1;
            continue;
        }
        let x0 = x;
        while x < rw && col(x) {
            x += 1;
        }
        let rows: Vec<u32> = (0..rh).filter(|&y| (x0..x).any(|c| on[y as usize][c as usize])).collect();
        let (y0, y1) = (rows[0], *rows.last().unwrap());
        let (w, h) = (x - x0, y1 - y0 + 1);
        let bits = (0..H)
            .flat_map(|my| (0..W).map(move |mx| (mx, my)))
            .map(|(mx, my)| on[(y0 + my * h / H) as usize][(x0 + mx * w / W) as usize])
            .collect();
        out.push(Glyph { x: x0, w, h, mask: Mask { w: W, h: H, bits } });
    }
    out
}

fn templates() -> &'static [(char, char, Mask)] {
    static T: std::sync::OnceLock<Vec<(char, char, Mask)>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        TEMPLATES
            .lines()
            .filter_map(|l| {
                let mut p = l.split_whitespace();
                let (k, c, b) = (p.next()?.chars().next()?, p.next()?.chars().next()?, p.next()?);
                Some((k, c, Mask { w: W, h: H, bits: b.bytes().map(|c| c == b'1').collect() }))
            })
            .collect()
    })
}

/// lit in both / lit in either.
fn overlap(a: &Mask, b: &Mask) -> f32 {
    let both = a.bits.iter().zip(&b.bits).filter(|(x, y)| **x && **y).count();
    let either = a.bits.iter().zip(&b.bits).filter(|(x, y)| **x || **y).count();
    both as f32 / either.max(1) as f32
}

/// The digit a glyph shows, if a template of `kind` matches well enough.
fn digit(g: &Glyph, kind: Kind) -> Option<u32> {
    if g.h < DIGIT_H {
        return None;
    }
    templates()
        .iter()
        .filter(|(k, ..)| *k == kind.tag())
        .map(|(_, c, t)| (c, overlap(&g.mask, t)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|(_, s)| *s >= MATCH)
        .and_then(|(c, _)| c.to_digit(10))
}

fn dash(g: &Glyph) -> bool {
    g.h <= DASH_H && g.w >= 5
}

/// The stage: a digit, a dash and a digit, close together.
pub fn stage(f: &Frame, scale: f32) -> Option<(u32, u32)> {
    let gs = glyphs(f, scale, Kind::Stage);
    (1..gs.len().saturating_sub(1)).find_map(|i| {
        let (a, d, b) = (&gs[i - 1], &gs[i], &gs[i + 1]);
        let close = d.x - (a.x + a.w) <= GAP && b.x - (d.x + d.w) <= GAP;
        if !(dash(d) && close) {
            return None;
        }
        let (a, b) = (digit(a, Kind::Stage)?, digit(b, Kind::Stage)?);
        (1..=9).contains(&a).then_some((a, b))
    })
}

/// Gold: the run of digits from the first glyph.
pub fn gold(f: &Frame, scale: f32) -> Option<u32> {
    let gs: Vec<Glyph> = glyphs(f, scale, Kind::Gold).into_iter().filter(|g| g.h >= DIGIT_H).collect();
    let first = gs.first()?;
    let mut n = digit(first, Kind::Gold)?;
    let mut end = first.x + first.w;
    for g in &gs[1..] {
        if g.x - end > GAP {
            break;
        }
        n = n * 10 + digit(g, Kind::Gold)?;
        end = g.x + g.w;
    }
    Some(n)
}
