//! Team size: the big "N/M" (units on board / cap) drawn on my board's floor in planning, a helmet
//! to its left. Dark see-through glyphs while the board is full (so N = M), bright blue ones while
//! a slot is free (N < M); hidden in combat. The text is centred, so "10/10" sits wider than "7/7".
//! Read at half resolution: a feature map for each style (how much darker than the floor around
//! it, or how strongly that blue), glyph templates slid along the row by normalised correlation
//! (`team.txt`, averaged from captures by `examples/team_glyphs.rs`), the best non-overlapping
//! matches kept and the digits chained to the slash parsed.

use ocr::Frame;

/// The counter's band at 2560 width: x, y, w, h (helmet and up to "10/10").
pub const REGION: (u32, u32, u32, u32) = (860, 285, 860, 146);
/// Glyph tops and bottoms (base px): templates span these rows.
pub const BAND: (u32, u32) = (300, 416);
/// Floor brightness: the brightest pixel within this many half px (glyph strokes are narrower).
const FLOOR_R: usize = 11;
/// Darkness counted up to this share below the floor.
const DARK_MAX: f32 = 0.6;
/// Bright glyphs: green over red (the floor is violet) and blue, each ramped over a range.
const BLUE_GR: (f32, f32) = (10.0, 40.0);
const BLUE_B: (f32, f32) = (150.0, 60.0);
/// Glyph rows may sit this many half px off the band.
const SHIFT_Y: i32 = 3;
/// A glyph match needs this correlation.
const MATCH: f32 = 0.45;
/// Templates are first tried every this many half px and rows; places within NEAR of a match are
/// then tried at every px and row.
const COARSE: usize = 2;
const NEAR: f32 = 0.15;
/// Matches overlapping by more than this share of the narrower one are the same glyph.
const OVERLAP: f32 = 0.35;
/// A map averaging less than this has no glyphs (the bright one while the board is full).
const EMPTY: f32 = 0.02;
/// Glyphs of one number are at most this far apart (half px).
const GAP: usize = 18;
/// A read needs one of its glyphs matched at least this well (a tactician often hides the rest).
const SURE: f32 = 0.85;
/// A read's glyphs must average this.
const READ_MIN: f32 = 0.6;
/// Templates: "glyph style w h values", glyph one of 0-9, '/', 'H' (the helmet); style 0 dark,
/// 1 bright; values 0-9 row by row.
const GLYPHS: &str = include_str!("team.txt");
/// Highest cap read (level 10 and a couple of bonus slots).
const MAX_CAP: u32 = 12;

/// A w×h map of values, row by row.
pub struct Map {
    pub w: usize,
    pub h: usize,
    pub v: Vec<f32>,
}

/// Sliding max (or min) over 2r+1 samples, clamped at the ends.
fn slide(v: &[f32], r: usize, max: bool) -> Vec<f32> {
    (0..v.len())
        .map(|i| {
            let w = &v[i.saturating_sub(r)..(i + r + 1).min(v.len())];
            w.iter().copied().fold(if max { f32::MIN } else { f32::MAX }, |a, b| if max { a.max(b) } else { a.min(b) })
        })
        .collect()
}

/// Separable max or min filter over a w×h grid.
fn filter2(v: &[f32], w: usize, h: usize, r: usize, max: bool) -> Vec<f32> {
    let mut out: Vec<f32> = v.chunks(w).flat_map(|row| slide(row, r, max)).collect();
    for x in 0..w {
        let col: Vec<f32> = (0..h).map(|y| out[y * w + x]).collect();
        for (y, c) in slide(&col, r, max).into_iter().enumerate() {
            out[y * w + x] = c;
        }
    }
    out
}

/// The region at half resolution as (dark glyph map, bright glyph map).
pub fn maps(f: &Frame, scale: f32) -> [Map; 2] {
    let (x0, y0, rw, rh) = REGION;
    let (w, h) = (rw as usize / 2, rh as usize / 2);
    let mut rgb = vec![[0f32; 3]; w * h];
    for (i, px) in rgb.iter_mut().enumerate() {
        let (hx, hy) = ((i % w) as u32, (i / w) as u32);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let sx = (((x0 + 2 * hx + dx) as f32 * scale) as u32).min(f.width - 1);
            let sy = (((y0 + 2 * hy + dy) as f32 * scale) as u32).min(f.height - 1);
            let p = ((sy * f.width + sx) * 4) as usize;
            for (c, k) in [2, 1, 0].into_iter().enumerate() {
                px[c] += f.bgra[p + k] as f32 / 4.0;
            }
        }
    }
    let lum: Vec<f32> = rgb.iter().map(|p| p[0].max(p[1]).max(p[2])).collect();
    // Closing: the floor with the dark strokes filled in.
    let floor = filter2(&filter2(&lum, w, h, FLOOR_R, true), w, h, FLOOR_R, false);
    let dark = lum.iter().zip(&floor).map(|(l, fl)| (1.0 - l / fl.max(1.0)).clamp(0.0, DARK_MAX)).collect();
    let ramp = |v: f32, (from, over): (f32, f32)| ((v - from) / over).clamp(0.0, 1.0);
    let bright = rgb.iter().map(|p| ramp(p[1] - p[0], BLUE_GR) * ramp(p[2], BLUE_B)).collect();
    [Map { w, h, v: dark }, Map { w, h, v: bright }]
}

struct Glyph {
    ch: char,
    style: usize,
    w: usize,
    h: usize,
    /// Values less their mean, scaled to unit norm.
    z: Vec<f32>,
}

/// A template's values less their mean, at unit norm (None when flat).
fn unit(v: &[f32]) -> Option<Vec<f32>> {
    let mean = v.iter().sum::<f32>() / v.len().max(1) as f32;
    let z: Vec<f32> = v.iter().map(|x| x - mean).collect();
    let n = z.iter().map(|x| x * x).sum::<f32>().sqrt();
    (n > 0.0).then(|| z.iter().map(|x| x / n).collect())
}

fn glyphs() -> &'static [Glyph] {
    static G: std::sync::OnceLock<Vec<Glyph>> = std::sync::OnceLock::new();
    G.get_or_init(|| {
        GLYPHS
            .lines()
            .filter_map(|l| {
                let mut p = l.split_whitespace();
                let ch = p.next()?.chars().next()?;
                let style = p.next()?.parse().ok()?;
                let (w, h): (usize, usize) = (p.next()?.parse().ok()?, p.next()?.parse().ok()?);
                let v: Vec<f32> = p.next()?.bytes().map(|c| (c - b'0') as f32).collect();
                (v.len() == w * h).then_some(())?;
                Some(Glyph { ch, style, w, h, z: unit(&v)? })
            })
            .collect()
    })
}

/// A glyph found at half px x..x+w with its correlation.
#[derive(Debug, Clone, Copy)]
struct Found {
    score: f32,
    ch: char,
    x0: usize,
    x1: usize,
}

/// Correlation of glyph `g` with the map's window at (x, y).
fn ncc(m: &Map, g: &Glyph, x: usize, y: usize) -> f32 {
    let (mut dot, mut sum, mut sq) = (0.0f32, 0.0f64, 0.0f64);
    for r in 0..g.h {
        let row = &m.v[(y + r) * m.w + x..(y + r) * m.w + x + g.w];
        let (mut s, mut q) = (0.0f32, 0.0f32);
        for (a, t) in row.iter().zip(&g.z[r * g.w..(r + 1) * g.w]) {
            dot += a * t;
            s += a;
            q += a * a;
        }
        (sum, sq) = (sum + s as f64, sq + q as f64);
    }
    // The template sums to 0, so the window's mean drops out of the dot product.
    let var = sq - sum * sum / (g.w * g.h) as f64;
    if var <= 1e-9 { 0.0 } else { dot / var.sqrt() as f32 }
}

/// Coarse positions correlated at once by [`ncc_lanes`].
const LANES: usize = 8;

/// The map's even and odd columns apart, so positions COARSE px apart load side by side.
struct Planes {
    even: Vec<f32>,
    odd: Vec<f32>,
    we: usize,
    wo: usize,
}

impl Planes {
    fn new(m: &Map) -> Self {
        let (we, wo) = (m.w.div_ceil(2), m.w / 2);
        let (mut even, mut odd) = (Vec::with_capacity(we * m.h), Vec::with_capacity(wo * m.h));
        for row in m.v.chunks(m.w) {
            even.extend(row.iter().step_by(2));
            odd.extend(row.iter().skip(1).step_by(2));
        }
        Planes { even, odd, we, wo }
    }
}

/// [`ncc`] at x = 2k, 2(k+1), … 2(k+LANES-1): each lane does the same arithmetic in the same order
/// (equal bit for bit), lanes side by side so the compiler vectorises across positions.
fn ncc_lanes(p: &Planes, g: &Glyph, k: usize, y: usize) -> [f32; LANES] {
    let (mut dot, mut sum, mut sq) = ([0.0f32; LANES], [0.0f64; LANES], [0.0f64; LANES]);
    for r in 0..g.h {
        let (s_even, s_odd) = (&p.even[(y + r) * p.we..][..p.we], &p.odd[(y + r) * p.wo..][..p.wo]);
        let (mut s, mut q) = ([0.0f32; LANES], [0.0f32; LANES]);
        for (c, &t) in g.z[r * g.w..(r + 1) * g.w].iter().enumerate() {
            // Column 2k + c of the map.
            let plane = if c % 2 == 0 { s_even } else { s_odd };
            let a: &[f32; LANES] = plane[k + c / 2..][..LANES].try_into().expect("LANES long");
            for l in 0..LANES {
                dot[l] += a[l] * t;
                s[l] += a[l];
                q[l] += a[l] * a[l];
            }
        }
        for l in 0..LANES {
            (sum[l], sq[l]) = (sum[l] + s[l] as f64, sq[l] + q[l] as f64);
        }
    }
    std::array::from_fn(|l| {
        let var = sq[l] - sum[l] * sum[l] / (g.w * g.h) as f64;
        if var <= 1e-9 { 0.0 } else { dot[l] / var.sqrt() as f32 }
    })
}

/// Best matches of the style's templates along the row, overlapping ones dropped, left to right.
fn find(m: &Map, style: usize) -> Vec<Found> {
    let top = ((BAND.0 - REGION.1) / 2) as i32;
    let mut all = vec![];
    if m.v.iter().sum::<f32>() < EMPTY * m.v.len() as f32 {
        return all;
    }
    let planes = Planes::new(m);
    for g in glyphs().iter().filter(|g| g.style == style) {
        let last = m.w.saturating_sub(g.w);
        let rows = |dys: &mut dyn Iterator<Item = i32>| {
            dys.map(|dy| top + dy).filter(|&y| y >= 0 && y as usize + g.h <= m.h).map(|y| y as usize).collect::<Vec<_>>()
        };
        let at = |x: usize, ys: &[usize]| ys.iter().map(|&y| ncc(m, g, x, y)).fold(0.0f32, f32::max);
        // Coarse every COARSE px and rows, then every place and row around the promising ones.
        let coarse_rows = rows(&mut (-SHIFT_Y..=SHIFT_Y).step_by(COARSE));
        let fine_rows = rows(&mut (-SHIFT_Y..=SHIFT_Y));
        const _: () = assert!(COARSE == 2, "planes split columns in two");
        let n = last / COARSE + 1;
        let mut coarse = vec![0.0f32; n];
        let mut k = 0;
        while k < n {
            if COARSE * (k + LANES - 1) <= last {
                let mut best = [0.0f32; LANES];
                for &y in &coarse_rows {
                    for (b, v) in best.iter_mut().zip(ncc_lanes(&planes, g, k, y)) {
                        *b = b.max(v);
                    }
                }
                coarse[k..k + LANES].copy_from_slice(&best);
                k += LANES;
            } else {
                coarse[k] = at(COARSE * k, &coarse_rows);
                k += 1;
            }
        }
        for x in (0..=last).step_by(COARSE) {
            if coarse[x / COARSE] < MATCH - NEAR {
                continue;
            }
            for x in x.saturating_sub(COARSE - 1)..=(x + COARSE - 1).min(last) {
                let best = at(x, &fine_rows);
                if best >= MATCH {
                    all.push(Found { score: best, ch: g.ch, x0: x, x1: x + g.w });
                }
            }
        }
    }
    all.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Found> = vec![];
    for f in all {
        let clash = |k: &Found| (f.x1.min(k.x1) as f32 - f.x0.max(k.x0) as f32) > OVERLAP * (f.x1 - f.x0).min(k.x1 - k.x0) as f32;
        if !kept.iter().any(clash) {
            kept.push(f);
        }
    }
    kept.sort_by_key(|f| f.x0);
    kept
}

/// (N, M) from the glyphs found: the one slash and the digits chained to it on each side, with the
/// mean score of the glyphs used. Dark (full board): either side alone gives N = M (a tactician
/// often stands on one). Bright (a free slot): M, and N when it reads below M.
fn parse(found: &[Found], bright: bool) -> Option<((Option<u32>, u32), f32)> {
    let slashes: Vec<usize> = (0..found.len()).filter(|&i| found[i].ch == '/').collect();
    let [s] = slashes[..] else { return None };
    let digit = |i: usize| found[i].ch.is_ascii_digit();
    let (mut l, mut r) = (s, s);
    while l > 0 && digit(l - 1) && found[l].x0.saturating_sub(found[l - 1].x1) <= GAP {
        l -= 1;
    }
    while r + 1 < found.len() && digit(r + 1) && found[r + 1].x0.saturating_sub(found[r].x1) <= GAP {
        r += 1;
    }
    let num = |a: usize, b: usize| {
        let n: u32 = (b > a && b - a <= 2).then(|| found[a..b].iter().map(|f| f.ch).collect::<String>().parse().ok())??;
        Some((n, found[a..b].iter().map(|f| f.score).sum::<f32>(), b - a))
    };
    let mean = |parts: &[(u32, f32, usize)]| (found[s].score + parts.iter().map(|p| p.1).sum::<f32>()) / (1 + parts.iter().map(|p| p.2).sum::<usize>()) as f32;
    let (n, cap) = (num(l, s), num(s + 1, r + 1));
    let (team, score) = if bright {
        let cap = cap?;
        match n.filter(|n| n.0 < cap.0) {
            Some(n) => ((Some(n.0), cap.0), mean(&[n, cap])),
            None => ((None, cap.0), mean(&[cap])),
        }
    } else {
        let side = [n, cap].into_iter().flatten().max_by(|a, b| (a.1 / a.2 as f32).total_cmp(&(b.1 / b.2 as f32)))?;
        ((Some(side.0), side.0), mean(&[side]))
    };
    // The bright helmet only shows with a free slot: it counts as sure too.
    let sure = found[l..=r].iter().any(|f| f.score >= SURE) || (bright && found.iter().any(|f| f.ch == 'H' && f.score >= SURE));
    ((1..=MAX_CAP).contains(&team.1) && score >= READ_MIN && sure).then_some((team, score))
}

/// Calibration: each style's glyphs as (glyph, score, x0 at 2560 width), left to right.
pub fn found(f: &Frame, scale: f32) -> Vec<Vec<(char, f32, u32)>> {
    maps(f, scale).iter().enumerate().map(|(k, m)| find(m, k).iter().map(|g| (g.ch, g.score, REGION.0 + 2 * g.x0 as u32)).collect()).collect()
}

/// Units on my board and the cap, when the counter is on screen and reads cleanly. A free slot
/// whose N is hidden (a tactician on it) takes N from `board`, the HP bars on my board, kept below
/// the cap (bars count a little high: some units don't take a slot).
pub fn read(f: &Frame, scale: f32, board: u32) -> Option<(u32, u32)> {
    let maps = maps(f, scale);
    let ((n, cap), _) = (0..2).filter_map(|k| parse(&find(&maps[k], k), k == 1)).max_by(|a, b| a.1.total_cmp(&b.1))?;
    Some((n.unwrap_or(board.min(cap - 1)), cap))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ch: char, x0: usize, x1: usize) -> Found {
        Found { score: 0.9, ch, x0, x1 }
    }

    #[test]
    fn parses_the_digits_next_to_the_slash() {
        // Helmet legs matched as "1"s far left of the number don't count.
        let f = [at('1', 63, 91), at('1', 123, 151), at('1', 183, 211), at('/', 218, 271), at('2', 270, 324)];
        assert_eq!(parse(&f, true).map(|r| r.0), Some((Some(1), 2)));
        let f = [at('H', 12, 96), at('1', 120, 158), at('0', 168, 222), at('/', 218, 271), at('1', 276, 315), at('0', 324, 375)];
        assert_eq!(parse(&f, false).map(|r| r.0), Some((Some(10), 10)));
        // Dark: one side is enough. Bright: N only below M.
        assert_eq!(parse(&[at('/', 218, 271), at('4', 270, 324)], false).map(|r| r.0), Some((Some(4), 4)));
        assert_eq!(parse(&[at('/', 218, 271), at('4', 270, 324)], true).map(|r| r.0), Some((None, 4)));
        assert_eq!(parse(&[at('7', 168, 222), at('/', 218, 271), at('3', 270, 324)], true).map(|r| r.0), Some((None, 3)));
        assert_eq!(parse(&[at('7', 168, 222), at('/', 218, 271)], true), None);
        // No slash, or only weak matches: nothing.
        assert_eq!(parse(&[at('4', 168, 222), at('4', 270, 324)], false), None);
        let weak = |ch, x0, x1| Found { score: 0.55, ch, x0, x1 };
        assert_eq!(parse(&[weak('6', 168, 222), weak('/', 218, 271), weak('6', 270, 324)], false), None);
    }

    #[test]
    fn lanes_equal_the_scalar_correlation() {
        let (w, h) = (101, 60);
        let mut x = 12345u32;
        let v = (0..w * h)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x % 1000) as f32 / 1000.0
            })
            .collect();
        let m = Map { w, h, v };
        let p = Planes::new(&m);
        for g in glyphs().iter().filter(|g| g.w <= w && g.h <= h).take(6) {
            let last = (m.w - g.w) / COARSE;
            for k in (0..=last.saturating_sub(LANES - 1)).step_by(3) {
                for (l, v) in ncc_lanes(&p, g, k, 0).into_iter().enumerate() {
                    assert_eq!(v.to_bits(), ncc(&m, g, COARSE * (k + l), 0).to_bits(), "{} at {}", g.ch, k + l);
                }
            }
        }
    }

    #[test]
    fn correlation_is_one_on_its_own_template() {
        let g = &glyphs()[0];
        let top = ((BAND.0 - REGION.1) / 2) as usize;
        let mut m = Map { w: g.w + 10, h: top + g.h + 4, v: vec![0.0; (g.w + 10) * (top + g.h + 4)] };
        for r in 0..g.h {
            for c in 0..g.w {
                m.v[(top + r) * m.w + 5 + c] = 0.3 + g.z[r * g.w + c];
            }
        }
        assert!((ncc(&m, g, 5, top) - 1.0).abs() < 1e-3, "{}", ncc(&m, g, 5, top));
        let f = find(&m, g.style);
        assert!(f.iter().any(|k| k.ch == g.ch && k.x0 == 5 && k.score > 0.99), "{f:?}");
    }
}
