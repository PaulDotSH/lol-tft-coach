//! Enemy champion blips on the minimap, found by their red border ring, named by their portrait, plus the
//! minimap's client rectangle. Pure: it reads a `Frame` captured around the minimap.

use crate::Frame;
use crate::scoreboard::{Rect, Rgba};
use model_core::ChampKey;

/// Minimap widget side as a fraction of the client height: `MM_INTERCEPT + MM_SLOPE * MinimapScale`, the scale
/// floored at `MM_MIN_SCALE` (the slider's far left, 0.0, still draws a sizeable map). Fitted across the slider range.
const MM_INTERCEPT: f32 = 0.1651;
const MM_SLOPE: f32 = 0.0739;
const MM_MIN_SCALE: f32 = 0.37;
/// The capture is this much larger than the widget, flush with its corner, so it holds the whole map square and the
/// HUD strip above it.
const CAPTURE_PAD: f32 = 1.14;
/// Where the map square sits in the widget when it can't be found on screen (fractions of the widget side): its
/// side, and its margins from the screen's side and bottom edges. Measured at scale 1.0 on 2560×1440.
const MAP_OF_WIDGET: f32 = 338.0 / 344.2;
const MAP_SIDE_MARGIN: f32 = 16.0 / 344.2;
const MAP_BOTTOM_MARGIN: f32 = 17.0 / 344.2;
/// The map art is framed by pure black padding: a row or column belongs to the map when this share of it is black.
const BLACK_MAX: u8 = 4;
const BLACK_LINE: f32 = 0.25;

/// A pixel counts as enemy red at or above this red channel, with green and blue this much lower and this far below red.
const RED_MIN: i32 = 165;
const CHAN_MAX: i32 = 120;
const RED_DIFF: i32 = 60;
/// Enemy ring radius and band half-width, and the gap and width of the halo just outside it that must stay clear
/// (fractions of the map side; measured 14.1 px on a 338 px map). Rings are searched at these radius scales.
const RING_R: f32 = 14.1 / 338.0;
const RING_BAND: f32 = 1.6 / 338.0;
const HALO_GAP: f32 = 1.5 / 338.0;
const HALO_WIDTH: f32 = 2.5 / 338.0;
const RING_SCALES: [f32; 3] = [0.9, 1.0, 1.1];
/// A ring: this share of its band red, this share of its sectors closed (half red), at most this share of the halo
/// red; two rings at least this many radii apart.
const RING_MIN_COVERAGE: f32 = 0.45;
const RING_SECTORS: usize = 16;
const RING_SECTOR_RED: f32 = 0.5;
const RING_MIN_CLOSURE: f32 = 0.6;
const RING_MAX_HALO: f32 = 0.35;
const RING_MIN_SEPARATION: f32 = 0.8;
const MAX_BLIPS: usize = 10;

/// Portrait descriptor: cells across the face's diameter (inside the circle only), hue × saturation histogram bins,
/// and the histogram's share of the combined score.
const PORTRAIT_N: usize = 12;
const HIST_H: usize = 12;
const HIST_S: usize = 8;
const HIST_WEIGHT: f32 = 0.3;
/// The share of a template's half-width that the minimap face shows: Data Dragon square, HUD circle.
const SQUARE_ZOOM: f32 = 0.85;
const CIRCLE_ZOOM: f32 = 0.88;
/// Face radius sampled inside the ring, and how far around a blip's centre to look for the best fit (fractions of
/// the map side).
const FACE_R: f32 = 11.0 / 338.0;
const FACE_SLACK: f32 = 2.0 / 338.0;
/// A blip is named when its best portrait scores at least this and this far ahead of its runner-up (overlapping
/// icons score low and stay unnamed); each champion names one blip, best scores first.
const NAME_MIN: f32 = 0.45;
const NAME_MARGIN: f32 = 0.12;

/// The map art inside a minimap capture, in frame pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapSquare {
    pub x: f32,
    pub y: f32,
    pub side: f32,
}

impl MapSquare {
    fn px(&self, fx: f32, fy: f32) -> (f32, f32) {
        (self.x + fx * self.side, self.y + fy * self.side)
    }
}

fn widget_frac(scale: f32) -> f32 {
    MM_INTERCEPT + MM_SLOPE * scale.max(MM_MIN_SCALE)
}

/// The minimap capture for a client of `cw`×`ch` pixels at League's MinimapScale `scale`: a square flush with the
/// bottom-right corner, or bottom-left with League's "flip minimap" option.
pub fn rect(cw: u32, ch: u32, scale: f32, flipped: bool) -> Rect {
    let side = ((ch as f32 * widget_frac(scale) * CAPTURE_PAD).round() as i32).clamp(1, cw.min(ch) as i32);
    let x = if flipped { 0 } else { cw as i32 - side };
    Rect { x, y: ch as i32 - side, w: side, h: side }
}

/// Top edge of that capture (the minimap and the HUD strip above it) as a fraction of the client height.
pub fn top_frac(scale: f32) -> f32 {
    1.0 - widget_frac(scale) * CAPTURE_PAD
}

/// The map square in a capture from `rect`: found by the black padding around the map art, else placed where the
/// widget puts it.
pub fn map_square(frame: &Frame, flipped: bool) -> MapSquare {
    let (w, h) = (frame.width as usize, frame.height as usize);
    if w == 0 || h == 0 || frame.bgra.len() < w * h * 4 {
        return MapSquare { x: 0.0, y: 0.0, side: w.min(h) as f32 };
    }
    if let Some(sq) = find_square(frame) {
        return sq;
    }
    let widget = w as f32 / CAPTURE_PAD;
    let side = widget * MAP_OF_WIDGET;
    let x = if flipped { widget * MAP_SIDE_MARGIN } else { w as f32 - widget * MAP_SIDE_MARGIN - side };
    MapSquare { x, y: h as f32 - widget * MAP_BOTTOM_MARGIN - side, side }
}

/// The map square by its black padding; None when the padding isn't there (the fallback place is used).
pub fn find_square(frame: &Frame) -> Option<MapSquare> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    if w == 0 || h == 0 || frame.bgra.len() < w * h * 4 {
        return None;
    }
    let (mut rows, mut cols) = (vec![0u32; h], vec![0u32; w]);
    for (i, p) in frame.bgra.as_chunks::<4>().0.iter().take(w * h).enumerate() {
        if p[0].max(p[1]).max(p[2]) <= BLACK_MAX {
            rows[i / w] += 1;
            cols[i % w] += 1;
        }
    }
    let span = |v: &[u32], n: usize| {
        let at = |i: &usize| v[*i] as f32 >= BLACK_LINE * n as f32;
        Some(((0..v.len()).find(at)?, (0..v.len()).rfind(at)?))
    };
    if let (Some((x0, x1)), Some((y0, y1))) = (span(&cols, h), span(&rows, w)) {
        let (sw, sh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        if (sw - sh).abs() <= 0.03 * sw && sw >= 0.6 * w.min(h) as f32 {
            return Some(MapSquare { x: x0 as f32, y: y0 as f32, side: (sw + sh) / 2.0 });
        }
    }
    None
}

fn is_red(&[b, g, r, _]: &[u8; 4]) -> bool {
    let (b, g, r) = (b as i32, g as i32, r as i32);
    r >= RED_MIN && g <= CHAN_MAX && b <= CHAN_MAX && r - g >= RED_DIFF && r - b >= RED_DIFF
}

/// Pixel offsets of one ring scale: the band (with its sector) and the halo outside it.
struct RingGeom {
    band: Vec<(i32, i32, usize)>,
    halo: Vec<(i32, i32)>,
}

fn ring_geom(r: f32, hw: f32, gap: f32, halo: f32) -> RingGeom {
    let reach = (r + hw + gap + halo).ceil() as i32;
    let (mut band, mut out) = (vec![], vec![]);
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let d = (dx as f32).hypot(dy as f32);
            if (d - r).abs() <= hw {
                let a = (dy as f32).atan2(dx as f32) + std::f32::consts::PI;
                band.push((dx, dy, ((a / std::f32::consts::TAU * RING_SECTORS as f32) as usize).min(RING_SECTORS - 1)));
            } else if d > r + hw + gap && d <= r + hw + gap + halo {
                out.push((dx, dy));
            }
        }
    }
    RingGeom { band, halo: out }
}

/// Enemy blips as (fx, fy) fractions of the map square (fx from the left, fy from the top, both 0..1), the clearest
/// ring first. Rings are found by their outline only, so a red portrait or two overlapping icons don't hide them,
/// and solid red (structures, pings) is rejected by its red halo.
pub fn enemy_blips(frame: &Frame, sq: MapSquare) -> Vec<(f32, f32)> {
    let (w, h) = (frame.width as i32, frame.height as i32);
    if w == 0 || h == 0 || frame.bgra.len() < (w * h * 4) as usize || sq.side <= 0.0 {
        return vec![];
    }
    let raw: Vec<bool> = frame.bgra.as_chunks::<4>().0.iter().take((w * h) as usize).map(is_red).collect();
    let red: Vec<bool> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            (-1..=1).any(|dy| (-1..=1).any(|dx| (0..w).contains(&(x + dx)) && (0..h).contains(&(y + dy)) && raw[((y + dy) * w + x + dx) as usize]))
        })
        .collect();
    let mut sat = vec![0u32; ((w + 1) * (h + 1)) as usize];
    for y in 0..h {
        let mut row = 0;
        for x in 0..w {
            row += red[(y * w + x) as usize] as u32;
            sat[((y + 1) * (w + 1) + x + 1) as usize] = sat[(y * (w + 1) + x + 1) as usize] + row;
        }
    }
    let boxed = |x0: i32, y0: i32, x1: i32, y1: i32| {
        let (x0, y0, x1, y1) = (x0.clamp(0, w), y0.clamp(0, h), x1.clamp(0, w), y1.clamp(0, h));
        let at = |x: i32, y: i32| sat[(y * (w + 1) + x) as usize];
        at(x1, y1) + at(x0, y0) - at(x0, y1) - at(x1, y0)
    };
    let red_at = |x: i32, y: i32| (0..w).contains(&x) && (0..h).contains(&y) && red[(y * w + x) as usize];
    let r = RING_R * sq.side;
    let hw = (RING_BAND * sq.side).max(1.0);
    let geoms: Vec<RingGeom> = RING_SCALES.iter().map(|s| ring_geom(r * s, hw, (HALO_GAP * sq.side).max(1.0), (HALO_WIDTH * sq.side).max(1.5))).collect();
    let min_red = (RING_MIN_COVERAGE * geoms.iter().map(|g| g.band.len()).min().unwrap_or(0) as f32) as u32;
    let band_reach = (r * RING_SCALES[2] + hw).ceil() as i32;

    let mut cands: Vec<(f32, i32, i32, usize)> = vec![];
    let (x0, y0) = (sq.x.floor() as i32, sq.y.floor() as i32);
    let (x1, y1) = ((sq.x + sq.side).ceil() as i32, (sq.y + sq.side).ceil() as i32);
    for cy in y0.max(0)..y1.min(h) {
        for cx in x0.max(0)..x1.min(w) {
            if boxed(cx - band_reach, cy - band_reach, cx + band_reach + 1, cy + band_reach + 1) < min_red {
                continue;
            }
            let (cov, gi) = geoms
                .iter()
                .enumerate()
                .map(|(i, g)| (g.band.iter().filter(|&&(dx, dy, _)| red_at(cx + dx, cy + dy)).count() as f32 / g.band.len() as f32, i))
                .fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a });
            if cov >= RING_MIN_COVERAGE {
                cands.push((cov, cx, cy, gi));
            }
        }
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0));
    let valid = |cx: i32, cy: i32, g: &RingGeom| {
        let (mut on, mut all) = ([0u32; RING_SECTORS], [0u32; RING_SECTORS]);
        for &(dx, dy, s) in &g.band {
            all[s] += 1;
            on[s] += red_at(cx + dx, cy + dy) as u32;
        }
        let closed = (0..RING_SECTORS).filter(|&s| all[s] > 0 && on[s] as f32 >= RING_SECTOR_RED * all[s] as f32).count();
        let halo = g.halo.iter().filter(|&&(dx, dy)| red_at(cx + dx, cy + dy)).count() as f32 / g.halo.len().max(1) as f32;
        closed as f32 >= RING_MIN_CLOSURE * RING_SECTORS as f32 && halo <= RING_MAX_HALO
    };
    let min_sep = RING_MIN_SEPARATION * r;
    let mut rings: Vec<(i32, i32)> = vec![];
    for (_, cx, cy, gi) in cands {
        if rings.len() >= MAX_BLIPS {
            break;
        }
        if rings.iter().any(|&(x, y)| ((x - cx) as f32).hypot((y - cy) as f32) < min_sep) || !valid(cx, cy, &geoms[gi]) {
            continue;
        }
        rings.push((cx, cy));
    }
    rings
        .into_iter()
        .map(|(cx, cy)| ((cx as f32 + 0.5 - sq.x) / sq.side, (cy as f32 + 0.5 - sq.y) / sq.side))
        .filter(|&(x, y)| (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y))
        .collect()
}

/// Champion art a portrait template is built from.
pub enum Art<'a> {
    /// Data Dragon `img/champion/<Id>.png`.
    Square(&'a Rgba),
    /// CommunityDragon HUD circle (`hud/<id>_circle.png`): the art the minimap draws.
    Circle(&'a Rgba),
}

/// A champion's portrait templates, one per art: face colours over the cells of a circle (mean-free, unit length)
/// and a hue × saturation histogram (mean-free, unit length).
pub struct Portrait(Vec<(Vec<f32>, Vec<f32>)>);

fn unit(mut v: Vec<f32>) -> Vec<f32> {
    let mean = v.iter().sum::<f32>() / v.len().max(1) as f32;
    v.iter_mut().for_each(|a| *a -= mean);
    let norm = v.iter().map(|a| a * a).sum::<f32>().sqrt().max(1e-6);
    v.iter_mut().for_each(|a| *a /= norm);
    v
}

fn hue_sat([r, g, b]: [f32; 3]) -> (usize, usize) {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    let s = if max <= 0.0 { 0.0 } else { d / max };
    (((h * HIST_H as f32) as usize).min(HIST_H - 1), ((s * HIST_S as f32) as usize).min(HIST_S - 1))
}

/// Descriptor and histogram of the circle of radius `r` around (cx, cy), each cell the mean of k×k taps of `rgb`.
fn face(rgb: impl Fn(i32, i32) -> [f32; 3], cx: f32, cy: f32, r: f32, k: usize) -> (Vec<f32>, Vec<f32>) {
    let cell = 2.0 * r / PORTRAIT_N as f32;
    let mut v = Vec::with_capacity(PORTRAIT_N * PORTRAIT_N * 3);
    let mut hist = vec![0.0; HIST_H * HIST_S];
    for j in 0..PORTRAIT_N {
        for i in 0..PORTRAIT_N {
            let (dx, dy) = ((i as f32 + 0.5) / PORTRAIT_N as f32 * 2.0 - 1.0, (j as f32 + 0.5) / PORTRAIT_N as f32 * 2.0 - 1.0);
            if dx * dx + dy * dy > 1.0 {
                continue;
            }
            let mut sum = [0.0; 3];
            for t in 0..k * k {
                let x = cx + dx * r + ((t % k) as f32 + 0.5) / k as f32 * cell - cell / 2.0;
                let y = cy + dy * r + ((t / k) as f32 + 0.5) / k as f32 * cell - cell / 2.0;
                let p = rgb(x.floor() as i32, y.floor() as i32);
                let (hb, sb) = hue_sat(p);
                hist[hb * HIST_S + sb] += 1.0;
                (0..3).for_each(|c| sum[c] += p[c]);
            }
            v.extend(sum.map(|s| s / (k * k) as f32));
        }
    }
    (unit(v), unit(hist))
}

/// Templates of a champion from its art (any of square and HUD circle; the best fit counts).
pub fn portrait(art: &[Art]) -> Portrait {
    Portrait(
        art.iter()
            .map(|a| {
                let (icon, zoom, alpha) = match a {
                    Art::Square(i) => (*i, SQUARE_ZOOM, false),
                    Art::Circle(i) => (*i, CIRCLE_ZOOM, true),
                };
                let (w, h) = (icon.w as i32, icon.h as i32);
                let rgb = |x: i32, y: i32| {
                    let p = &icon.px[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize * 4..][..4];
                    let a = if alpha { p[3] as f32 / 255.0 } else { 1.0 };
                    [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a]
                };
                face(rgb, icon.w as f32 / 2.0, icon.h as f32 / 2.0, icon.w as f32 / 2.0 * zoom, 4)
            })
            .collect(),
    )
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// How well each of `portraits` fits each of `blips`: the best over a small search around the blip's centre and over
/// the champion's art, colours and histogram combined.
fn scores(frame: &Frame, sq: MapSquare, blips: &[(f32, f32)], portraits: &[(ChampKey, &Portrait)]) -> Vec<Vec<f32>> {
    let (w, h) = (frame.width as i32, frame.height as i32);
    let rgb = |x: i32, y: i32| {
        let p = &frame.bgra[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize * 4..][..3];
        [p[2] as f32, p[1] as f32, p[0] as f32]
    };
    let (r, slack) = (FACE_R * sq.side, (FACE_SLACK * sq.side).round().max(1.0) as i32);
    blips
        .iter()
        .map(|&(fx, fy)| {
            let (cx, cy) = sq.px(fx, fy);
            let mut best = vec![f32::MIN; portraits.len()];
            for oy in -slack..=slack {
                for ox in -slack..=slack {
                    let (d, hist) = face(rgb, cx + ox as f32, cy + oy as f32, r, 2);
                    for (b, (_, p)) in best.iter_mut().zip(portraits) {
                        for (td, th) in &p.0 {
                            *b = b.max((1.0 - HIST_WEIGHT) * dot(&d, td) + HIST_WEIGHT * dot(&hist, th));
                        }
                    }
                }
            }
            best
        })
        .collect()
}

/// The champion drawn at each of `blips` (from `enemy_blips`) among `portraits`, where one clearly fits; each champion
/// names one blip at most, best fits first.
pub fn name_blips(frame: &Frame, sq: MapSquare, blips: &[(f32, f32)], portraits: &[(ChampKey, &Portrait)]) -> Vec<Option<ChampKey>> {
    let (w, h) = (frame.width as i32, frame.height as i32);
    let mut named = vec![None; blips.len()];
    if portraits.is_empty() || w == 0 || h == 0 || frame.bgra.len() < (w * h * 4) as usize {
        return named;
    }
    let s = scores(frame, sq, blips, portraits);
    let mut pairs: Vec<(f32, usize, usize)> = vec![];
    for (b, row) in s.iter().enumerate() {
        for (c, &v) in row.iter().enumerate() {
            let next = row.iter().enumerate().filter(|&(i, _)| i != c).map(|(_, &x)| x).fold(f32::MIN, f32::max);
            if v >= NAME_MIN && v - next >= NAME_MARGIN {
                pairs.push((v, b, c));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut taken = vec![false; portraits.len()];
    for (_, b, c) in pairs {
        if named[b].is_none() && !taken[c] {
            named[b] = Some(portraits[c].0);
            taken[c] = true;
        }
    }
    named
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED_PX: [u8; 3] = [20, 20, 210];
    const BLUE: [u8; 3] = [210, 120, 40];
    const GREY: [u8; 3] = [60, 70, 60];

    /// Synthetic `side`-pixel capture: a grey frame around a black-padded map square at (m, m), with (cx, cy, inner,
    /// outer, [b,g,r]) annuli painted in map fractions (inner 0 = a filled disc).
    fn frame(side: u32, rings: &[(f32, f32, f32, f32, [u8; 3])]) -> (Frame, MapSquare) {
        let m = side / 10;
        let sq = MapSquare { x: m as f32, y: m as f32, side: (side - 2 * m) as f32 };
        let mut bgra = vec![0u8; (side * side * 4) as usize];
        for y in 0..side {
            for x in 0..side {
                let i = ((y * side + x) * 4) as usize;
                let inside = (m..side - m).contains(&x) && (m..side - m).contains(&y);
                let pad = inside && (x < m + 8 || x >= side - m - 8 || y < m + 8 || y >= side - m - 8);
                let c = if !inside { GREY } else if pad { [0, 0, 0] } else { [40, 60, 50] };
                bgra[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                for &(fx, fy, inner, outer, [b, g, r]) in rings {
                    let (cx, cy) = sq.px(fx, fy);
                    let d = (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy);
                    if d >= inner && d <= outer {
                        bgra[i..i + 4].copy_from_slice(&[b, g, r, 255]);
                    }
                }
            }
        }
        (Frame { width: side, height: side, bgra }, sq)
    }

    /// An enemy ring on a 380 px capture (map 304 px): radius ≈ 12.7.
    fn enemy(fx: f32, fy: f32) -> (f32, f32, f32, f32, [u8; 3]) {
        (fx, fy, 11.5, 14.0, RED_PX)
    }

    fn near(b: &[(f32, f32)], fx: f32, fy: f32) -> bool {
        b.iter().any(|&(x, y)| (x - fx).hypot(y - fy) < 0.015)
    }

    #[test]
    fn finds_the_map_square_by_its_black_padding() {
        let (f, sq) = frame(380, &[]);
        assert_eq!(map_square(&f, false), sq);
    }

    #[test]
    fn finds_rings_and_places_them_on_the_map() {
        let (f, sq) = frame(380, &[enemy(0.25, 0.30), enemy(0.75, 0.70)]);
        let b = enemy_blips(&f, map_square(&f, false));
        assert_eq!(b.len(), 2, "{b:?}");
        assert!(near(&b, 0.25, 0.30) && near(&b, 0.75, 0.70), "{b:?} on {sq:?}");
    }

    #[test]
    fn overlapping_icons_stay_two_blips() {
        let (f, _) = frame(380, &[enemy(0.50, 0.50), enemy(0.555, 0.52)]);
        let b = enemy_blips(&f, map_square(&f, false));
        assert_eq!(b.len(), 2, "{b:?}");
    }

    #[test]
    fn a_red_portrait_does_not_hide_its_ring() {
        let (f, _) = frame(380, &[(0.4, 0.6, 0.0, 9.0, [30, 40, 200]), enemy(0.4, 0.6)]);
        assert!(near(&enemy_blips(&f, map_square(&f, false)), 0.4, 0.6));
    }

    #[test]
    fn rejects_allies_solid_red_and_specks() {
        let (f, sq) = frame(
            380,
            &[(0.2, 0.2, 11.5, 14.0, BLUE), (0.5, 0.5, 0.0, 14.0, RED_PX), (0.8, 0.3, 0.0, 30.0, RED_PX), (0.3, 0.8, 0.0, 2.0, RED_PX)],
        );
        assert!(enemy_blips(&f, sq).is_empty());
    }

    #[test]
    fn empty_frame_is_safe() {
        let f = Frame { width: 0, height: 0, bgra: vec![] };
        assert!(enemy_blips(&f, map_square(&f, false)).is_empty());
    }

    #[test]
    fn rect_is_the_fitted_square_in_the_corner() {
        let r = rect(2560, 1440, 1.0, false);
        assert_eq!((r.w, r.h, r.x + r.w, r.y + r.h), (392, 392, 2560, 1440));
        assert_eq!(rect(2560, 1440, 1.0, true).x, 0, "flipped: bottom-left");
        assert!(rect(2560, 1440, 2.0, false).w > r.w && rect(2560, 1440, 0.0, false).w == rect(2560, 1440, MM_MIN_SCALE, false).w);
        assert!((top_frac(1.0) - (1.0 - 392.0 / 1440.0)).abs() < 1e-3);
    }

    #[test]
    fn fallback_square_matches_the_measured_map() {
        let f = Frame { width: 392, height: 392, bgra: vec![200; 392 * 392 * 4] };
        let sq = map_square(&f, false);
        assert!((sq.side - 338.0).abs() < 4.0 && (392.0 - sq.x - sq.side - 16.0).abs() < 2.0, "{sq:?}");
        assert!(map_square(&f, true).x < 20.0);
        assert_eq!(find_square(&f), None);
    }

    fn fixture(path: &str) -> Rgba {
        let png = std::fs::read(format!("{}/../../fixtures/ocr/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        crate::scoreboard::decode_png(&png).unwrap()
    }

    fn minimap(name: &str) -> Frame {
        let img = fixture(&format!("minimap/{name}.png"));
        Frame { width: img.w, height: img.h, bgra: img.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect() }
    }

    /// Map fraction of a pixel in the fixture captures (map at 35,34, side 338).
    fn at(x: f32, y: f32) -> (f32, f32) {
        ((x - 35.0) / 338.0, (y - 34.0) / 338.0)
    }

    #[test]
    fn real_captures_find_the_map_square() {
        for n in ["red_side_0108", "blue_side_0236", "blue_side_0802"] {
            let sq = map_square(&minimap(n), false);
            assert!((sq.x - 35.0).abs() <= 1.0 && (sq.y - 34.0).abs() <= 1.0 && (sq.side - 338.0).abs() <= 1.5, "{n}: {sq:?}");
            assert_eq!(find_square(&minimap(n)), Some(sq), "{n}");
        }
    }

    /// Real capture (scale 1.0, 2560×1440, red side, 1:08): three enemies, at top lane, mid and bot.
    #[test]
    fn real_minimap_finds_the_three_visible_enemies() {
        let f = minimap("red_side_0108");
        let b = enemy_blips(&f, map_square(&f, false));
        assert_eq!(b.len(), 3, "{b:?}");
        for (x, y) in [(0.156, 0.168), (0.503, 0.479), (0.814, 0.848)] {
            assert!(b.iter().any(|&(bx, by)| (bx - x).hypot(by - y) < 0.02), "{b:?}");
        }
    }

    const YASUO: ChampKey = 157;
    const TEEMO: ChampKey = 17;
    const SORAKA: ChampKey = 16;

    /// The enemy team of the blue-side fixtures, from both arts.
    fn enemy_portraits() -> Vec<(ChampKey, Portrait)> {
        [(TEEMO, "Teemo"), (222, "Jinx"), (YASUO, "Yasuo"), (81, "Ezreal"), (SORAKA, "Soraka")]
            .into_iter()
            .map(|(k, n)| {
                let (sq, ci) = (fixture(&format!("champions/{n}.png")), fixture(&format!("champions/{n}_circle.png")));
                (k, portrait(&[Art::Square(&sq), Art::Circle(&ci)]))
            })
            .collect()
    }

    /// Name of the blip nearest the fixture pixel (x, y).
    fn named_at(f: &Frame, x: f32, y: f32) -> Option<ChampKey> {
        let (fx, fy) = at(x, y);
        let p = enemy_portraits();
        let refs: Vec<(ChampKey, &Portrait)> = p.iter().map(|(k, p)| (*k, p)).collect();
        let sq = map_square(f, false);
        let b = enemy_blips(f, sq);
        let names = name_blips(f, sq, &b, &refs);
        let i = (0..b.len()).min_by(|&i, &j| (b[i].0 - fx).hypot(b[i].1 - fy).total_cmp(&(b[j].0 - fx).hypot(b[j].1 - fy)))?;
        assert!((b[i].0 - fx).hypot(b[i].1 - fy) < 0.03, "no blip near ({fx}, {fy}): {b:?}");
        names[i]
    }

    /// Real capture, 8:02: Soraka (enemy support) roaming through the river beside mid; Yasuo in the lane.
    #[test]
    fn names_a_roamer_in_the_river() {
        let f = minimap("blue_side_0802");
        assert_eq!(named_at(&f, 250.0, 226.0), Some(SORAKA));
        assert_eq!(named_at(&f, 235.0, 184.0), Some(YASUO));
    }

    /// Real capture, 2:36: Teemo top, Yasuo mid.
    #[test]
    fn names_the_laners() {
        let f = minimap("blue_side_0236");
        assert_eq!(named_at(&f, 90.0, 83.0), Some(TEEMO));
        assert_eq!(named_at(&f, 220.0, 187.0), Some(YASUO));
    }

    #[test]
    fn no_portraits_names_nothing() {
        let f = minimap("blue_side_0236");
        let sq = map_square(&f, false);
        assert!(name_blips(&f, sq, &enemy_blips(&f, sq), &[]).iter().all(Option::is_none));
    }
}
