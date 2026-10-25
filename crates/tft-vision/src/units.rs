//! My units by their HP bars. Every unit on my board and bench draws a green bar over
//! its head, with a cap on the left whose colour is its ★ (bronze, silver, gold); enemies
//! and PvE monsters draw red ones. A bar's left end is fixed while HP drains from the right.
//! Positions are measured on 2560×1440 captures and scaled to the client size.

use ocr::Frame;
use tft_model::SeenUnit;

/// Bars are searched in this area (base px): both halves of the board and the bench, above the shop.
const AREA: (f32, f32, f32, f32) = (350.0, 60.0, 2200.0, 1050.0);
/// Bar height range, the narrowest (nearly dead) bar and a full bar's width (base px).
const BAR_H: (f32, f32) = (3.5, 7.5);
const BAR_MIN_W: f32 = 10.0;
const FULL_W: f32 = 80.0;
/// Gaps in a row bridged while scanning (the HP ticks between bar segments).
const GAP: f32 = 6.0;
/// Bars lower than this are on the bench; bench slot 0's bar x and the slot pitch.
const BENCH_Y: f32 = 830.0;
const BENCH_X0: f32 = 485.0;
const BENCH_PITCH: f32 = 163.5;
const BENCH_SLOTS: u32 = 9;
/// The ★ cap left of the bar: x from the bar's left end, y from its top (base px).
const CAP: (f32, f32, f32, f32) = (-15.0, -1.0, 0.0, 11.0);
/// Cap pixels of one colour needed to call the ★.
const CAP_MIN: u32 = 6;
/// The unit under its bar, relative to the bar's left end (base px): x0, y0, x1, y1.
pub const CROP: (f32, f32, f32, f32) = (-18.0, 30.0, 82.0, 150.0);

/// Points along the bar's top edge checked for the bar's dark frame; how many must hit.
const FRAME_POINTS: u32 = 6;
const FRAME_MIN: u32 = 4;

/// The bars' navy frame (#021223), with some blending.
fn frame(p: &[u8]) -> bool {
    let (b, g, r) = (p[0] as i32, p[1] as i32, p[2] as i32);
    r < 40 && g < 50 && b < 75 && b > r
}

/// The bar's frame shows just above it (orange hair or red VFX has none).
fn framed(f: &Frame, scale: f32, b: &Bar) -> bool {
    (1..=3).any(|dy| {
        let y = b.y as i32 - (dy as f32 * scale).round().max(1.0) as i32;
        y >= 0
            && (0..FRAME_POINTS)
                .filter(|k| {
                    let x = b.x + b.w * (2 * k + 1) / (2 * FRAME_POINTS);
                    let i = (y as usize * f.width as usize + x as usize) * 4;
                    frame(&f.bgra[i..i + 4])
                })
                .count() as u32
                >= FRAME_MIN
    })
}

fn green(p: &[u8]) -> bool {
    let (b, g, r) = (p[0] as i32, p[1] as i32, p[2] as i32);
    g > 170 && r < 120 && b < 120 && g - r > 90
}

fn red(p: &[u8]) -> bool {
    let (b, g, r) = (p[0] as i32, p[1] as i32, p[2] as i32);
    r > 80 && g < 45 && b < 45 && r > 3 * g
}

/// A bar in client pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Bars of pixels passing `hit`: rows scanned with small gaps bridged, runs on touching rows joined.
pub fn bars(f: &Frame, scale: f32, hit: fn(&[u8]) -> bool) -> Vec<Bar> {
    let s = |v: f32| (v * scale).round() as u32;
    let (x0, y0) = (s(AREA.0), s(AREA.1));
    let (x1, y1) = (s(AREA.2).min(f.width), s(AREA.3).min(f.height));
    let (gap, min_w) = (s(GAP).max(1), s(BAR_MIN_W).max(2));
    // Open bars: (x0, x1, y0, last row).
    let mut open: Vec<(u32, u32, u32, u32)> = vec![];
    let mut done: Vec<(u32, u32, u32, u32)> = vec![];
    for y in y0..y1 {
        let row = (y * f.width) as usize * 4;
        let mut runs: Vec<(u32, u32)> = vec![];
        let mut run: Option<(u32, u32)> = None;
        for x in x0..x1 {
            let i = row + x as usize * 4;
            if hit(&f.bgra[i..i + 4]) {
                run = match run {
                    Some((a, b)) if x - b <= gap => Some((a, x)),
                    Some(r) => {
                        runs.push(r);
                        Some((x, x))
                    }
                    None => Some((x, x)),
                };
            }
        }
        runs.extend(run);
        runs.retain(|(a, b)| b - a + 1 >= min_w);
        let (mut keep, mut ended): (Vec<_>, Vec<_>) = open.drain(..).partition(|o| o.3 + 1 == y);
        done.append(&mut ended);
        for (a, b) in runs {
            match keep.iter_mut().find(|o| a <= o.1 && b >= o.0) {
                Some(o) => *o = (o.0.min(a), o.1.max(b), o.2, y),
                None => keep.push((a, b, y, y)),
            }
        }
        open = keep;
    }
    done.extend(open);
    let (hmin, hmax) = (BAR_H.0 * scale, BAR_H.1 * scale);
    done.into_iter()
        .map(|(a, b, top, last)| Bar { x: a, y: top, w: b - a + 1, h: last - top + 1 })
        .filter(|b| (hmin..=hmax).contains(&(b.h as f32)) && b.w as f32 >= b.h as f32 * 1.5 && framed(f, scale, b))
        .collect()
}

/// ★ from the cap's colour: bronze 1, silver 2, gold 3; 0 when unclear.
pub fn star(f: &Frame, scale: f32, bar: &Bar) -> u32 {
    let px = |v: f32| (v * scale).round() as i32;
    let (mut bronze, mut silver, mut gold) = (0u32, 0u32, 0u32);
    for y in bar.y as i32 + px(CAP.1)..bar.y as i32 + px(CAP.3) {
        for x in bar.x as i32 + px(CAP.0)..bar.x as i32 + px(CAP.2) {
            if x < 0 || y < 0 || x >= f.width as i32 || y >= f.height as i32 {
                continue;
            }
            let i = (y as usize * f.width as usize + x as usize) * 4;
            let (b, g, r) = (f.bgra[i] as i32, f.bgra[i + 1] as i32, f.bgra[i + 2] as i32);
            if r > 190 && g > 160 && b < 130 && r >= g {
                gold += 1;
            } else if r - b > 60 && r > g && g > b && r > 110 && r - g < 110 {
                bronze += 1;
            } else if b >= r && g > 140 && b > 160 && r > 100 {
                silver += 1;
            }
        }
    }
    let most = bronze.max(silver).max(gold);
    if most < CAP_MIN {
        0
    } else if most == bronze {
        1
    } else if most == silver {
        2
    } else {
        3
    }
}

/// My units (green bars) and the number of red bars.
pub fn read(f: &Frame, scale: f32) -> (Vec<SeenUnit>, u32) {
    let units = bars(f, scale, green)
        .iter()
        .map(|b| {
            let (x, y) = (b.x as f32 / scale, b.y as f32 / scale);
            let slot = ((x - BENCH_X0) / BENCH_PITCH).round();
            SeenUnit {
                x,
                y,
                star: star(f, scale, b),
                bench: (y > BENCH_Y).then_some(slot.clamp(0.0, (BENCH_SLOTS - 1) as f32) as u32),
                full: b.w as f32 / scale >= FULL_W,
                items: vec![],
                emb: vec![],
            }
        })
        .collect();
    (units, bars(f, scale, red).len() as u32)
}

/// The crop under a unit's bar, for the embedder.
pub fn crop(f: &Frame, scale: f32, u: &SeenUnit) -> tft_embed::Rgb {
    let c = |v: f32, max: u32| ((v * scale).round().max(0.0) as u32).min(max);
    let (x0, y0) = (c(u.x + CROP.0, f.width - 1), c(u.y + CROP.1, f.height - 1));
    let (x1, y1) = (c(u.x + CROP.2, f.width).max(x0 + 1), c(u.y + CROP.3, f.height).max(y0 + 1));
    let mut px = Vec::with_capacity(((x1 - x0) * (y1 - y0)) as usize);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * f.width + x) * 4) as usize;
            px.push([f.bgra[i + 2], f.bgra[i + 1], f.bgra[i]]);
        }
    }
    tft_embed::Rgb { w: (x1 - x0) as usize, h: (y1 - y0) as usize, px }
}
