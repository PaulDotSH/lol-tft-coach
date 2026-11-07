//! Enemy champion blips on the minimap, found by their red border ring, named by their portrait, plus the
//! minimap's client rectangle. Pure: it reads a `Frame` already cropped to the minimap.

use crate::Frame;
use crate::scoreboard::{Rect, Rgba};
use model_core::ChampKey;

/// A pixel counts as enemy red at or above this red channel, with green and blue this much lower and this far below red. First guesses, to tune on a real capture.
const RED_MIN: i32 = 165;
const CHAN_MAX: i32 = 120;
const RED_DIFF: i32 = 60;
/// Blob areas kept, as a fraction of the frame's pixel count. First guesses.
const MIN_AREA_FRAC: f32 = 0.0006;
const MAX_AREA_FRAC: f32 = 0.03;
const MAX_BLIPS: usize = 5;

/// Minimap side as a fraction of the client height is `MM_BASE + MM_SPAN * scale`, inset from the bottom-right corner by `MM_INSET`. First guesses to tune against a real capture.
const MM_BASE: f32 = 0.11;
const MM_SPAN: f32 = 0.154;
const MM_INSET: f32 = 0.006;
/// The map itself inside that square (left, top, side), as fractions of it: the rest is the frame and HUD. Measured at scale 1.0 on 2560Ã—1440 (map at 35,34 size 338 of 380).
const MAP_X0: f32 = 35.0 / 380.0;
const MAP_Y0: f32 = 34.0 / 380.0;
const MAP_SIDE: f32 = 338.0 / 380.0;

/// Portrait descriptor: cells across the icon's diameter (inside the circle only), and the share of the Data Dragon
/// square's width the minimap circle shows.
const PORTRAIT_N: usize = 12;
const PORTRAIT_ZOOM: f32 = 0.85;
/// Minimap icon radius sampled, and how far around a blip's centre to look for the best fit (fractions of the map side).
const ICON_R: f32 = 11.0 / 338.0;
const ICON_SLACK: f32 = 2.0 / 338.0;
/// A blip is named when its best portrait correlates at least this well and this far ahead of the runner-up
/// (overlapping icons score low and stay unnamed).
const NAME_MIN: f32 = 0.45;
const NAME_MARGIN: f32 = 0.15;

const OFF: u8 = 0;
const RED: u8 = 1;
const SEEN: u8 = 2;

fn is_red(&[b, g, r, _]: &[u8; 4]) -> bool {
    let (b, g, r) = (b as i32, g as i32, r as i32);
    r >= RED_MIN && g <= CHAN_MAX && b <= CHAN_MAX && r - g >= RED_DIFF && r - b >= RED_DIFF
}

/// Enemy blips as (fx, fy) fractions of the map inside the minimap frame (fx from the left, fy from the top, both 0..1), largest ring first. Empty when none are visible.
pub fn enemy_blips(frame: &Frame) -> Vec<(f32, f32)> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    if w == 0 || h == 0 || frame.bgra.len() < w * h * 4 {
        return vec![];
    }
    let mut state: Vec<u8> = frame.bgra.as_chunks::<4>().0.iter().take(w * h).map(|px| if is_red(px) { RED } else { OFF }).collect();
    let min_area = (w * h) as f32 * MIN_AREA_FRAC;
    let max_area = (w * h) as f32 * MAX_AREA_FRAC;

    let mut stack: Vec<usize> = Vec::new();
    let mut blobs: Vec<(f32, f32, usize)> = Vec::new();
    for start in 0..w * h {
        if state[start] != RED {
            continue;
        }
        state[start] = SEEN;
        stack.push(start);
        let (mut sx, mut sy, mut area) = (0usize, 0usize, 0usize);
        while let Some(p) = stack.pop() {
            let (px, py) = (p % w, p / w);
            sx += px;
            sy += py;
            area += 1;
            let mut neighbours = [usize::MAX; 4];
            if px > 0 { neighbours[0] = p - 1; }
            if px + 1 < w { neighbours[1] = p + 1; }
            if py > 0 { neighbours[2] = p - w; }
            if py + 1 < h { neighbours[3] = p + w; }
            for q in neighbours.into_iter().filter(|&q| q != usize::MAX) {
                if state[q] == RED {
                    state[q] = SEEN;
                    stack.push(q);
                }
            }
        }
        if (min_area..=max_area).contains(&(area as f32)) {
            blobs.push((sx as f32 / area as f32, sy as f32 / area as f32, area));
        }
    }
    blobs.sort_by_key(|b| std::cmp::Reverse(b.2));
    blobs
        .into_iter()
        .map(|(cx, cy, _)| (((cx + 0.5) / w as f32 - MAP_X0) / MAP_SIDE, ((cy + 0.5) / h as f32 - MAP_Y0) / MAP_SIDE))
        .filter(|&(x, y)| (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y))
        .take(MAX_BLIPS)
        .collect()
}


/// A champion portrait template: RGB over the cells of a circle, mean-free, unit length.
pub struct Portrait(Vec<f32>);

/// Descriptor of the circle of radius `r` around (cx, cy), each cell the mean of kÃ—k taps of `rgb`.
fn circle(rgb: impl Fn(i32, i32) -> [f32; 3], cx: f32, cy: f32, r: f32, k: usize) -> Vec<f32> {
    let cell = 2.0 * r / PORTRAIT_N as f32;
    let mut v = Vec::with_capacity(PORTRAIT_N * PORTRAIT_N * 3);
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
                (0..3).for_each(|c| sum[c] += p[c]);
            }
            v.extend(sum.map(|s| s / (k * k) as f32));
        }
    }
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    v.iter_mut().for_each(|a| *a -= mean);
    let norm = v.iter().map(|a| a * a).sum::<f32>().sqrt().max(1e-6);
    v.iter_mut().for_each(|a| *a /= norm);
    v
}

/// Template of a Data Dragon champion square (`img/champion/<Id>.png`).
pub fn portrait(icon: &Rgba) -> Portrait {
    let (w, h) = (icon.w as i32, icon.h as i32);
    let rgb = |x: i32, y: i32| {
        let p = &icon.px[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize * 4..][..3];
        [p[0] as f32, p[1] as f32, p[2] as f32]
    };
    Portrait(circle(rgb, icon.w as f32 / 2.0, icon.h as f32 / 2.0, icon.w as f32 / 2.0 * PORTRAIT_ZOOM, 4))
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The champion drawn at each of `blips` (from `enemy_blips`) among `portraits`, where one clearly fits; a champion
/// names at most one blip (the better fit).
pub fn name_blips(frame: &Frame, blips: &[(f32, f32)], portraits: &[(ChampKey, &Portrait)]) -> Vec<Option<ChampKey>> {
    let (w, h) = (frame.width as i32, frame.height as i32);
    if portraits.is_empty() || w == 0 || h == 0 || frame.bgra.len() < (w * h * 4) as usize {
        return vec![None; blips.len()];
    }
    let rgb = |x: i32, y: i32| {
        let p = &frame.bgra[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize * 4..][..3];
        [p[2] as f32, p[1] as f32, p[0] as f32]
    };
    let side = MAP_SIDE * w as f32;
    let (r, slack) = (ICON_R * side, (ICON_SLACK * side).round().max(1.0) as i32);
    let mut named: Vec<Option<(ChampKey, f32)>> = blips
        .iter()
        .map(|&(fx, fy)| {
            let (cx, cy) = ((MAP_X0 + fx * MAP_SIDE) * w as f32, (MAP_Y0 + fy * MAP_SIDE) * h as f32);
            let mut best = vec![f32::MIN; portraits.len()];
            for oy in -slack..=slack {
                for ox in -slack..=slack {
                    let d = circle(rgb, cx + ox as f32, cy + oy as f32, r, 2);
                    for (b, (_, p)) in best.iter_mut().zip(portraits) {
                        *b = b.max(dot(&d, &p.0));
                    }
                }
            }
            let mut order: Vec<usize> = (0..best.len()).collect();
            order.sort_by(|&a, &b| best[b].total_cmp(&best[a]));
            let (top, next) = (best[order[0]], order.get(1).map_or(f32::MIN, |&i| best[i]));
            (top >= NAME_MIN && top - next >= NAME_MARGIN).then(|| (portraits[order[0]].0, top))
        })
        .collect();
    for i in 0..named.len() {
        for j in 0..named.len() {
            if let (Some(a), Some(b)) = (named[i], named[j])
                && i != j
                && a.0 == b.0
                && a.1 < b.1
            {
                named[i] = None;
            }
        }
    }
    named.into_iter().map(|n| n.map(|n| n.0)).collect()
}

/// The minimap's client-area rectangle for a client of `cw`Ã—`ch` pixels at League's MinimapScale `scale`: a square anchored bottom-right.
pub fn rect(cw: u32, ch: u32, scale: f32) -> Rect {
    let side = ((ch as f32 * (MM_BASE + MM_SPAN * scale)).round() as i32).max(1);
    let inset = (ch as f32 * MM_INSET).round() as i32;
    Rect { x: cw as i32 - inset - side, y: ch as i32 - inset - side, w: side, h: side }
}

/// Top edge of that square as a fraction of the client height.
pub fn top_frac(scale: f32) -> f32 {
    1.0 - MM_INSET - (MM_BASE + MM_SPAN * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `w`Ã—`h` BGRA frame, black, with filled discs painted in (cx, cy, radius, [b,g,r]).
    fn frame(w: u32, h: u32, discs: &[(i32, i32, i32, [u8; 3])]) -> Frame {
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        for &(cx, cy, rad, [b, g, r]) in discs {
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    if (x - cx).pow(2) + (y - cy).pow(2) <= rad * rad {
                        let i = ((y as u32 * w + x as u32) * 4) as usize;
                        bgra[i..i + 4].copy_from_slice(&[b, g, r, 255]);
                    }
                }
            }
        }
        Frame { width: w, height: h, bgra }
    }

    const RED_PX: [u8; 3] = [20, 20, 210];
    const BLUE: [u8; 3] = [210, 120, 40];

    fn fixture(path: &str) -> Rgba {
        let png = std::fs::read(format!("{}/../../fixtures/ocr/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        crate::scoreboard::decode_png(&png).unwrap()
    }

    fn minimap(name: &str) -> Frame {
        let img = fixture(&format!("minimap/{name}.png"));
        Frame { width: img.w, height: img.h, bgra: img.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect() }
    }

    const YASUO: ChampKey = 157;
    const TEEMO: ChampKey = 17;
    const SORAKA: ChampKey = 16;

    /// The enemy team of the blue-side fixtures.
    fn enemy_portraits() -> Vec<(ChampKey, Portrait)> {
        [(TEEMO, "Teemo"), (222, "Jinx"), (YASUO, "Yasuo"), (81, "Ezreal"), (SORAKA, "Soraka")]
            .into_iter()
            .map(|(k, n)| (k, portrait(&fixture(&format!("champions/{n}.png")))))
            .collect()
    }

    /// Name of the blip nearest (fx, fy).
    fn named_at(f: &Frame, fx: f32, fy: f32) -> Option<ChampKey> {
        let p = enemy_portraits();
        let refs: Vec<(ChampKey, &Portrait)> = p.iter().map(|(k, p)| (*k, p)).collect();
        let b = enemy_blips(f);
        let names = name_blips(f, &b, &refs);
        let i = (0..b.len()).min_by(|&i, &j| (b[i].0 - fx).hypot(b[i].1 - fy).total_cmp(&(b[j].0 - fx).hypot(b[j].1 - fy)))?;
        assert!((b[i].0 - fx).hypot(b[i].1 - fy) < 0.03, "no blip near ({fx}, {fy}): {b:?}");
        names[i]
    }

    /// Real capture, 8:02: Soraka (enemy support) roaming through the river beside mid; Yasuo in the lane.
    #[test]
    fn names_a_roamer_in_the_river() {
        let f = minimap("blue_side_0802");
        assert_eq!(named_at(&f, 0.636, 0.565), Some(SORAKA));
        assert_eq!(named_at(&f, (235.0 - 35.0) / 338.0, (184.0 - 34.0) / 338.0), Some(YASUO));
    }

    /// Real capture, 2:36: Teemo top, Yasuo mid.
    #[test]
    fn names_the_laners() {
        let f = minimap("blue_side_0236");
        assert_eq!(named_at(&f, (90.0 - 35.0) / 338.0, (83.0 - 34.0) / 338.0), Some(TEEMO));
        assert_eq!(named_at(&f, (220.0 - 35.0) / 338.0, (187.0 - 34.0) / 338.0), Some(YASUO));
    }

    #[test]
    fn no_portraits_names_nothing() {
        let f = minimap("blue_side_0236");
        assert!(name_blips(&f, &enemy_blips(&f), &[]).iter().all(Option::is_none));
    }

    /// Pixel of map fraction (fx, fy) in a `side`-pixel minimap frame.
    fn px(fx: f32, fy: f32, side: u32) -> (i32, i32) {
        let s = side as f32;
        (((MAP_X0 + fx * MAP_SIDE) * s) as i32, ((MAP_Y0 + fy * MAP_SIDE) * s) as i32)
    }

    #[test]
    fn finds_red_discs_and_places_them_on_the_map() {
        let ((ax, ay), (bx, by)) = (px(0.25, 0.30, 200), px(0.75, 0.70, 200));
        let f = frame(200, 200, &[(ax, ay, 7, RED_PX), (bx, by, 7, RED_PX)]);
        let mut b = enemy_blips(&f);
        b.sort_by(|a, c| a.0.total_cmp(&c.0));
        assert_eq!(b.len(), 2);
        assert!((b[0].0 - 0.25).abs() < 0.02 && (b[0].1 - 0.30).abs() < 0.02, "{b:?}");
        assert!((b[1].0 - 0.75).abs() < 0.02 && (b[1].1 - 0.70).abs() < 0.02, "{b:?}");
    }

    #[test]
    fn ignores_red_outside_the_map() {
        assert!(enemy_blips(&frame(200, 200, &[(8, 8, 6, RED_PX)])).is_empty());
    }

    /// A real capture (scale 1.0, 2560Ã—1440, red side, 1:08): three enemies, at top lane, mid and bot.
    #[test]
    fn real_minimap_finds_the_three_visible_enemies() {
        let png = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/ocr/minimap/red_side_0108.png")).unwrap();
        let img = crate::scoreboard::decode_png(&png).unwrap();
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
        let mut b = enemy_blips(&Frame { width: img.w, height: img.h, bgra });
        b.sort_by(|a, c| a.0.total_cmp(&c.0));
        assert_eq!(b.len(), 3, "{b:?}");
        for (got, want) in b.iter().zip([(0.156, 0.168), (0.503, 0.479), (0.814, 0.848)]) {
            assert!((got.0 - want.0).abs() < 0.02 && (got.1 - want.1).abs() < 0.02, "{b:?}");
        }
    }

    #[test]
    fn rejects_allied_icon() {
        assert!(enemy_blips(&frame(200, 200, &[(50, 50, 7, BLUE)])).is_empty());
    }

    #[test]
    fn rejects_tiny_dot() {
        assert!(enemy_blips(&frame(200, 200, &[(120, 120, 0, RED_PX)])).is_empty());
    }

    #[test]
    fn rejects_frame_filling_wash() {
        assert!(enemy_blips(&frame(200, 200, &[(100, 100, 190, RED_PX)])).is_empty());
    }

    #[test]
    fn mixed_frame_keeps_only_the_enemy() {
        let (x, y) = px(0.75, 0.70, 200);
        let b = enemy_blips(&frame(200, 200, &[(50, 50, 7, BLUE), (x, y, 7, RED_PX)]));
        assert_eq!(b.len(), 1, "{b:?}");
        assert!((b[0].0 - 0.75).abs() < 0.02 && (b[0].1 - 0.70).abs() < 0.02, "{b:?}");
    }

    #[test]
    fn caps_at_a_team() {
        let discs: Vec<_> = (0..8).map(|i| (20 + i * 20, 100, 6, RED_PX)).collect();
        assert_eq!(enemy_blips(&frame(200, 200, &discs)).len(), MAX_BLIPS);
    }

    #[test]
    fn empty_frame_is_safe() {
        assert!(enemy_blips(&Frame { width: 0, height: 0, bgra: vec![] }).is_empty());
    }

    #[test]
    fn top_frac_at_default_scale() {
        assert!((top_frac(1.0) - 0.73).abs() < 1e-4);
    }

    #[test]
    fn rect_is_a_square_in_the_bottom_right() {
        let r = rect(2560, 1440, 1.0);
        let inset = (1440.0f32 * MM_INSET).round() as i32;
        assert_eq!(r.w, r.h);
        assert_eq!(r.x + r.w, 2560 - inset);
        assert_eq!(r.y + r.h, 1440 - inset);
    }

    #[test]
    fn larger_scale_gives_a_larger_square() {
        let (a, b) = (rect(2560, 1440, 1.0), rect(2560, 1440, 1.5));
        assert!(b.w > a.w);
        assert!(top_frac(1.5) < top_frac(1.0));
    }
}
