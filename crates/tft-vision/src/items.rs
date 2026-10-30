//! Items: the item bench (a column of 10 slots at the left edge, beside the trait panel) and the
//! up to 3 icons under each of my units' HP bars. Icons are told apart by colour templates of the
//! Community Dragon item icons: normalised cross-correlation of the icon's centre (its rim, the
//! bench's stack count and the radiant gold frame left out), at the icon's size and 0.9 of it
//! (radiant items are drawn inside a gold frame), shifted up to 1 px.
//! Positions are measured on 2560×1440 captures and scaled to the client size.

use ocr::Frame;
use ocr::scoreboard::Rgba;
use tft_model::{Api, SeenUnit};

/// Bench slot 0's icon: left, top and size; slots are this far apart (base px).
const BENCH: (f32, f32, f32) = (18.0, 353.0, 50.0);
const BENCH_PITCH: f32 = 72.0;
const BENCH_SLOTS: u32 = 10;
/// An empty bench slot is black: its icon square's mean brightness stays under this.
const EMPTY: f32 = 12.0;
/// The first icon under a unit's bar, from the bar's left end and top: x, y, size; the next ones are ICON_PITCH apart.
const ON_UNIT: (f32, f32, f32) = (-13.0, 16.0, 32.0);
const ICON_PITCH: f32 = 35.0;
const ON_UNIT_MAX: u32 = 3;
/// Icons under a bar have a 1 px black outline: points checked per edge, how dark, and the share that must be.
const OUTLINE_POINTS: u32 = 16;
const OUTLINE_DARK: u8 = 50;
const OUTLINE_SHARE: f32 = 0.6;
/// Descriptor: D×D cells × RGB over the icon less INSET of its size on each side.
const D: usize = 12;
const INSET: f32 = 0.2;
/// Icon sizes tried (radiant art sits inside a gold frame) and the shift range (base px).
const SCALES: [f32; 2] = [1.0, 0.9];
const SHIFT: i32 = 1;
/// Correlation a match needs; at SURE the search stops early.
const MATCH: f32 = 0.7;
const SURE: f32 = 0.9;
/// Two items whose icons correlate this well share an icon (radiant potions, the Flora Fatalis emblems).
const SAME_ICON: f32 = 0.995;
/// Bilinear taps per cell side: icons are downscaled a lot, captures little.
const ICON_TAPS: usize = 8;
const FRAME_TAPS: usize = 2;

type Desc = Vec<f32>;

/// Item icon templates for one set.
pub struct ItemIcons {
    tpl: Vec<(Api, Desc)>,
    /// The templates transposed: value i of template n at `i * tpl.len() + n`, so `best` sums
    /// every template at once (each in its own order, as `dot` does) and vectorises.
    cols: Vec<f32>,
}

fn bilinear(at: &impl Fn(i32, i32) -> [f32; 3], x: f32, y: f32) -> [f32; 3] {
    let (x, y) = (x - 0.5, y - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (i, j) = (x0 as i32, y0 as i32);
    let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
    std::array::from_fn(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
}

/// Descriptor of the square (x, y, size): its centre less INSET, D×D cells of `taps`² samples, mean-free, unit length.
fn describe(at: impl Fn(i32, i32) -> [f32; 3], x: f32, y: f32, size: f32, taps: usize) -> Desc {
    let (x, y, side) = (x + size * INSET, y + size * INSET, size * (1.0 - 2.0 * INSET));
    let step = side / (D * taps) as f32;
    let mut d = vec![0.0; D * D * 3];
    for cy in 0..D {
        for cx in 0..D {
            let mut sum = [0.0; 3];
            for t in 0..taps * taps {
                let px = x + ((cx * taps + t % taps) as f32 + 0.5) * step;
                let py = y + ((cy * taps + t / taps) as f32 + 0.5) * step;
                let p = bilinear(&at, px, py);
                (0..3).for_each(|k| sum[k] += p[k]);
            }
            (0..3).for_each(|k| d[(cy * D + cx) * 3 + k] = sum[k] / (taps * taps) as f32);
        }
    }
    let mean = d.iter().sum::<f32>() / d.len() as f32;
    d.iter_mut().for_each(|v| *v -= mean);
    let norm = d.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
    d.iter_mut().for_each(|v| *v /= norm);
    d
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// A Community Dragon icon's template (transparent parts over black, like the bench).
fn icon_desc(icon: &Rgba) -> Desc {
    let (w, h) = (icon.w as i32, icon.h as i32);
    let at = |x: i32, y: i32| {
        let p = &icon.px[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize * 4..][..4];
        let a = p[3] as f32 / 255.0;
        [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a]
    };
    describe(at, 0.0, 0.0, icon.w as f32, ICON_TAPS)
}

fn frame_at(f: &Frame) -> impl Fn(i32, i32) -> [f32; 3] + '_ {
    let (w, h) = (f.width as i32, f.height as i32);
    move |x, y| {
        let p = &f.bgra[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize * 4..][..3];
        [p[2] as f32, p[1] as f32, p[0] as f32]
    }
}

impl ItemIcons {
    /// Templates from (item, decoded icon); items sharing an icon keep the plain one (not radiant, shortest name).
    pub fn new(mut icons: Vec<(Api, Rgba)>) -> ItemIcons {
        icons.sort_by(|a, b| (a.0.contains("Radiant"), a.0.len(), &a.0).cmp(&(b.0.contains("Radiant"), b.0.len(), &b.0)));
        let mut tpl: Vec<(Api, Desc)> = vec![];
        for (api, icon) in icons.iter().filter(|(_, i)| i.w > 0 && i.h > 0 && i.px.len() == (i.w * i.h * 4) as usize) {
            let d = icon_desc(icon);
            if !tpl.iter().any(|(_, t)| dot(t, &d) > SAME_ICON) {
                tpl.push((api.clone(), d));
            }
        }
        let n = tpl.len();
        let mut cols = vec![0.0; D * D * 3 * n];
        for (j, (_, t)) in tpl.iter().enumerate() {
            for (i, &v) in t.iter().enumerate() {
                cols[i * n + j] = v;
            }
        }
        ItemIcons { tpl, cols }
    }

    pub fn len(&self) -> usize {
        self.tpl.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tpl.is_empty()
    }

    /// The best template for `d`: the same scores as `dot` with each template, bit for bit.
    fn best(&self, d: &[f32]) -> Option<(f32, &Api)> {
        let n = self.tpl.len();
        // -0.0: where `Iterator::sum` starts.
        let mut acc = vec![-0.0f32; n];
        for (&x, col) in d.iter().zip(self.cols.chunks_exact(n.max(1))) {
            for (a, &t) in acc.iter_mut().zip(col) {
                *a += t * x;
            }
        }
        self.tpl.iter().zip(acc).map(|((a, _), s)| (s, a)).max_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// The item drawn in the square (x, y, size) of client pixels, if one matches; `px` is one base pixel.
    pub fn identify(&self, f: &Frame, x: f32, y: f32, size: f32, px: f32) -> Option<(Api, f32)> {
        let at = frame_at(f);
        let mut top: Option<(f32, &Api)> = None;
        // Unshifted at full size first: most icons stop there.
        let shifts = std::iter::once((0, 0)).chain((-SHIFT..=SHIFT).flat_map(|dx| (-SHIFT..=SHIFT).map(move |dy| (dx, dy))).filter(|&s| s != (0, 0)));
        'search: for k in SCALES {
            let (s, o) = (size * k, size * (1.0 - k) / 2.0);
            for (dx, dy) in shifts.clone() {
                let d = describe(&at, x + o + dx as f32 * px, y + o + dy as f32 * px, s, FRAME_TAPS);
                if let Some(b) = self.best(&d)
                    && top.is_none_or(|t| b.0 > t.0)
                {
                    top = Some(b);
                }
                if top.is_some_and(|t| t.0 >= SURE) {
                    break 'search;
                }
            }
        }
        top.filter(|t| t.0 >= MATCH).map(|(s, a)| (a.clone(), s))
    }
}

/// Mean brightness of the square (x, y, size), on a coarse grid.
fn brightness(f: &Frame, x: f32, y: f32, size: f32) -> f32 {
    let at = frame_at(f);
    let n = 8;
    let sum: f32 = (0..n * n).map(|i| at((x + size * ((i % n) as f32 + 0.5) / n as f32) as i32, (y + size * ((i / n) as f32 + 0.5) / n as f32) as i32).iter().sum::<f32>()).sum();
    sum / (3 * n * n) as f32
}

/// The item bench, slot by slot (None: empty, or an icon no template matches, e.g. mid-drag).
pub fn bench(f: &Frame, scale: f32, icons: &ItemIcons) -> Vec<Option<Api>> {
    let (x, y0, size) = (BENCH.0 * scale, BENCH.1 * scale, BENCH.2 * scale);
    (0..BENCH_SLOTS)
        .map(|k| {
            let y = y0 + k as f32 * BENCH_PITCH * scale;
            if brightness(f, x, y, size) < EMPTY {
                return None;
            }
            icons.identify(f, x, y, size, scale).map(|m| m.0)
        })
        .collect()
}

/// The square's 1 px outline is dark (the board shows through where no icon is drawn).
fn outlined(f: &Frame, x: f32, y: f32, size: f32, px: f32) -> bool {
    let lum = |x: f32, y: f32| {
        let (xi, yi) = (x.round() as i32, y.round() as i32);
        if xi < 0 || yi < 0 || xi >= f.width as i32 || yi >= f.height as i32 {
            return 255;
        }
        let i = (yi as usize * f.width as usize + xi as usize) * 4;
        f.bgra[i].max(f.bgra[i + 1]).max(f.bgra[i + 2])
    };
    // Each point: the darkest pixel across the line (±1 base px), so the line may sit a pixel off.
    let across = |x: f32, y: f32, dx: f32, dy: f32| (-1..=1).map(|t| lum(x + t as f32 * dx * px, y + t as f32 * dy * px)).min().unwrap_or(255);
    let (top, bot, left, right) = (y - px, y + size, x - px, x + size);
    let dark = (0..OUTLINE_POINTS)
        .map(|i| size * (i as f32 + 0.5) / OUTLINE_POINTS as f32)
        .flat_map(|t| [across(x + t, top, 0.0, 1.0), across(x + t, bot, 0.0, 1.0), across(left, y + t, 1.0, 0.0), across(right, y + t, 1.0, 0.0)])
        .filter(|&l| l < OUTLINE_DARK)
        .count();
    dark as f32 >= OUTLINE_SHARE * (4 * OUTLINE_POINTS) as f32
}

/// The icons under one unit's bar, left to right (None: no icon there, or none matches).
pub fn on_unit(f: &Frame, scale: f32, icons: &ItemIcons, u: &SeenUnit) -> Vec<Option<Api>> {
    let size = ON_UNIT.2 * scale;
    (0..ON_UNIT_MAX)
        .map(|k| {
            let x = (u.x + ON_UNIT.0 + k as f32 * ICON_PITCH) * scale;
            let y = (u.y + ON_UNIT.1) * scale;
            if !outlined(f, x, y, size, scale.max(1.0)) {
                return None;
            }
            icons.identify(f, x, y, size, scale).map(|m| m.0)
        })
        .collect()
}

/// Fill in the bench items and each unit's items.
pub fn read(f: &Frame, scale: f32, icons: &ItemIcons, items: &mut Vec<Api>, units: &mut [SeenUnit]) {
    *items = bench(f, scale, icons).into_iter().flatten().collect();
    for u in units {
        u.items = on_unit(f, scale, icons, u).into_iter().flatten().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!("{}/../../fixtures/tft/items/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn frame(name: &str) -> Frame {
        let img = ocr::scoreboard::decode_png(&fixture(name)).unwrap();
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        Frame { width: img.w, height: img.h, bgra }
    }

    /// Twenty icons (48 px): those on screen in the crops, plus look-alikes (plain versions of radiant ones).
    fn icons() -> ItemIcons {
        let dir = format!("{}/../../fixtures/tft/items/icons", env!("CARGO_MANIFEST_DIR"));
        let icons = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                (p.file_stem().unwrap().to_str().unwrap().to_string(), ocr::scoreboard::decode_png(&std::fs::read(&p).unwrap()).unwrap())
            })
            .collect();
        ItemIcons::new(icons)
    }

    fn unit(x: f32, y: f32) -> SeenUnit {
        SeenUnit { x, y, star: 1, bench: None, full: true, items: vec![], emb: vec![] }
    }

    #[test]
    fn best_scores_equal_dot() {
        let icons = icons();
        for t in icons.tpl.iter().take(5) {
            let d: Desc = t.1.iter().rev().zip(&t.1).map(|(a, b)| a * 0.7 + b * 0.3).collect();
            let naive = icons.tpl.iter().map(|(a, t)| (dot(t, &d), a)).max_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
            let got = icons.best(&d).unwrap();
            assert_eq!((got.0.to_bits(), got.1), (naive.0.to_bits(), naive.1));
        }
    }

    #[test]
    fn bench_slots_with_a_gap_and_a_hovered_icon() {
        // 2560×1440 capture, stage 5: two stacked Reforgers (badge "2"), a gap at slot 7, slots 5-6 hovered.
        let got = bench(&frame("bench.png"), 1.0, &icons());
        let want = [
            "DA_Reforger",
            "DA_Component_NeedlesslyLargeRod",
            "DA_Component_TearOfTheGoddess",
            "DA_Component_TearOfTheGoddess",
            "DA_Component_NeedlesslyLargeRod",
            "DA_Artifact_AegisOfDawn",
            "DA_Evenshroud",
            "",
            "DA_Consumable_GoldenItemRemover",
            "",
        ];
        assert_eq!(got.iter().map(|s| s.as_deref().unwrap_or("")).collect::<Vec<_>>(), want);
    }

    #[test]
    fn items_under_bars_radiant_and_none() {
        let icons = icons();
        // Crops start 20 px left of and 6 px above each bar's left end.
        let read = |name: &str| on_unit(&frame(name), 1.0, &icons, &unit(20.0, 6.0));
        assert_eq!(read("unit_1001_410.png"), [Some("DA_SunfireCape".into()), Some("DA_ProtectorsVow".into()), Some("DA_Artifact_AegisOfDawn".into())]);
        // Gold-framed icons are the radiant items, not the plain ones also in the templates.
        let radiant = ["DA_DragonsClawRadiant", "DA_JeweledGauntletRadiant", "DA_ThiefsGlovesRadiant"].map(|s| Some(s.to_string()));
        assert_eq!(read("unit_1158_640.png"), radiant);
        // No items: the board under the bar is no icon, however well it correlates (a Tear at 0.84).
        assert_eq!(read("unit_833_694.png"), [None, None, None]);
    }

    #[test]
    fn shared_icons_keep_the_plain_item() {
        let img = || ocr::scoreboard::decode_png(&fixture("icons/DA_BlueBuff.png")).unwrap();
        let icons = ItemIcons::new(vec![("DA_HealthPotion18_Radiant".into(), img()), ("DA_HealthPotion18".into(), img())]);
        assert_eq!(icons.tpl.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(), ["DA_HealthPotion18"]);
    }
}
