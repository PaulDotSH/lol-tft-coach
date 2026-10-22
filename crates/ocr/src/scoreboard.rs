//! ARAM: Mayhem Tab scoreboard: is it open, which row is which player, and
//! which augment icons fill each enemy's augment grid.
//!
//! Geometry is the game's own layout (`KiwiScoreboard/UIBase` on
//! CommunityDragon): 1600×1200 source units around the screen centre, scaled
//! by min(w/1600, h/1200); the grid pitch was measured on a 1080p capture.
//! Pure: works on captured frames positioned in client coordinates.

use crate::Frame;
use model::{AugmentId, ItemId, Rarity, Team};
use std::collections::HashMap;

const SRC_W: f32 = 1600.0;
const SRC_H: f32 = 1200.0;
/// Augment region of row 0 (left / right half x, top y) and the row pitch.
const AUG_X: [f32; 2] = [52.0, 1452.0];
const AUG_Y: f32 = 350.0;
const ROW_PITCH: f32 = 84.0;
const CELL: f32 = 32.0;
const ICON: f32 = 30.0;
const PITCH_X: f32 = 33.3;
const PITCH_Y: f32 = 35.5;
/// Grid slots (column, row): three on top, two below.
const SLOTS: [(f32, f32); 5] = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
const ITEM_X: [f32; 2] = [527.0, 1126.0];
/// Item row top relative to the augment region top.
const ITEM_DY: f32 = 24.0;
const ITEM: f32 = 36.0;
const ITEM_PITCH: f32 = 38.0;
const ITEM_SLOTS: usize = 7;
const BOARD: [f32; 4] = [37.0, 262.0, 1563.0, 779.0];
/// Search radius around the predicted grid, source units.
const SEARCH: f32 = 12.0;

/// A row's grid counts as present at this outline correlation and border contrast (luma).
const GRID_NCC: f32 = 0.35;
const GRID_AMP: f32 = 15.0;
/// Open when this many of the 10 rows show a grid.
const OPEN_ROWS: usize = 8;
/// Luma spread below which a cell or item slot is empty.
const EMPTY_SD: f32 = 7.0;
const ICON_NCC: f32 = 0.72;
/// Icon border share left out of matching (the prismatic frame is drawn over it).
const ICON_INSET: f32 = 0.07;
const ITEM_NCC: f32 = 0.8;
/// Grid backdrop luma (icons are composited over it for templates).
const BACKDROP: f32 = 10.0;

const D: usize = 16;
/// D×D luma samples, zero mean, unit norm.
pub type Desc = [f32; D * D];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Source units → client pixels for one client size.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    s: f32,
    cx: f32,
    cy: f32,
}

impl Geometry {
    pub fn new(w: u32, h: u32) -> Self {
        let s = (w as f32 / SRC_W).min(h as f32 / SRC_H);
        Geometry { s, cx: w as f32 / 2.0, cy: h as f32 / 2.0 }
    }

    fn pt(&self, x: f32, y: f32) -> (f32, f32) {
        (self.cx + (x - SRC_W / 2.0) * self.s, self.cy + (y - SRC_H / 2.0) * self.s)
    }

    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        let (a, b) = (self.pt(x0, y0), self.pt(x1, y1));
        let (x, y) = (a.0.floor() as i32, a.1.floor() as i32);
        Rect { x, y, w: b.0.ceil() as i32 - x, h: b.1.ceil() as i32 - y }
    }

    /// The whole scoreboard.
    pub fn board(&self) -> Rect {
        let [x0, y0, x1, y1] = BOARD;
        self.rect(x0, y0, x1, y1)
    }

    /// One half's augment column, all rows plus the search margin (cheap open check).
    pub fn column(&self, half: usize) -> Rect {
        let x = AUG_X[half];
        let bottom = AUG_Y + 4.0 * ROW_PITCH + CELL + PITCH_Y;
        self.rect(x - SEARCH - 2.0, AUG_Y - SEARCH - 2.0, x + 3.0 * PITCH_X + SEARCH + 2.0, bottom + SEARCH + 2.0)
    }

    /// Predicted top-left of a row's first augment cell.
    fn grid(&self, half: usize, row: usize) -> (f32, f32) {
        self.pt(AUG_X[half], AUG_Y + row as f32 * ROW_PITCH)
    }

    fn item(&self, half: usize, row: usize, slot: usize) -> (f32, f32) {
        self.pt(ITEM_X[half] + slot as f32 * ITEM_PITCH, AUG_Y + ITEM_DY + row as f32 * ROW_PITCH)
    }
}

/// A decoded image, RGBA8, top-down.
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

pub fn decode_png(bytes: &[u8]) -> Option<Rgba> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let px = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => {
            buf.as_chunks::<2>().0.iter().flat_map(|p| [p[0], p[0], p[0], p[1]]).collect()
        }
        png::ColorType::Grayscale => buf.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Rgba { w: info.width, h: info.height, px })
}

/// Bilinear sample of `at` (pixel centres at +0.5).
fn bilinear(at: impl Fn(i32, i32) -> f32, x: f32, y: f32) -> f32 {
    let (x, y) = (x - 0.5, y - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (i, j) = (x0 as i32, y0 as i32);
    let top = at(i, j) * (1.0 - fx) + at(i + 1, j) * fx;
    let bot = at(i, j + 1) * (1.0 - fx) + at(i + 1, j + 1) * fx;
    top * (1.0 - fy) + bot * fy
}

/// D×D descriptor of the square (x, y, size), each sample the mean of k×k bilinear taps.
fn describe(at: impl Fn(f32, f32) -> f32, x: f32, y: f32, size: f32, k: usize) -> Desc {
    let mut d = [0.0; D * D];
    let step = size / (D * k) as f32;
    for (i, v) in d.iter_mut().enumerate() {
        let (cx, cy) = ((i % D * k) as f32, (i / D * k) as f32);
        let mut sum = 0.0;
        for t in 0..k * k {
            sum += at(x + (cx + (t % k) as f32 + 0.5) * step, y + (cy + (t / k) as f32 + 0.5) * step);
        }
        *v = sum / (k * k) as f32;
    }
    normalize(&mut d);
    d
}

fn normalize(d: &mut Desc) {
    let mean = d.iter().sum::<f32>() / d.len() as f32;
    d.iter_mut().for_each(|v| *v -= mean);
    let norm = d.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
    d.iter_mut().for_each(|v| *v /= norm);
}

fn dot(a: &Desc, b: &Desc) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Template descriptor of an icon composited over the grid backdrop, inset by `inset` (fraction).
pub fn icon_desc(icon: &Rgba, inset: f32) -> Desc {
    let (w, h) = (icon.w as i32, icon.h as i32);
    let at = |x: i32, y: i32| {
        let p = &icon.px[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize * 4..][..4];
        let a = p[3] as f32 / 255.0;
        p[..3].iter().map(|&c| c as f32 * a + BACKDROP * (1.0 - a)).fold(0.0, f32::max)
    };
    let side = icon.w as f32;
    describe(|x, y| bilinear(at, x, y), side * inset, side * inset, side * (1.0 - 2.0 * inset), 4)
}

/// Mayhem augment icon templates. Augments sharing an icon form one group.
pub struct AugIcons {
    groups: Vec<(Vec<AugmentId>, Desc)>,
}

impl AugIcons {
    pub fn new(icons: impl IntoIterator<Item = (AugmentId, Rgba)>) -> Self {
        let mut groups: Vec<(Vec<AugmentId>, Desc)> = Vec::new();
        for (id, icon) in icons {
            let d = icon_desc(&icon, ICON_INSET);
            match groups.iter_mut().find(|g| dot(&g.1, &d) > 0.995) {
                Some(g) => g.0.push(id),
                None => groups.push((vec![id], d)),
            }
        }
        AugIcons { groups }
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// Augments sharing the icon of `group`.
    pub fn ids(&self, group: usize) -> &[AugmentId] {
        &self.groups[group].0
    }

    fn best(&self, d: &Desc) -> (f32, usize) {
        self.groups
            .iter()
            .enumerate()
            .map(|(i, g)| (dot(&g.1, d), i))
            .fold((f32::MIN, 0), |a, b| if b.0 > a.0 { b } else { a })
    }
}

/// Item icon template (Data Dragon art, border trimmed).
pub fn item_desc(icon: &Rgba) -> Desc {
    icon_desc(icon, ITEM_INSET)
}

const ITEM_INSET: f32 = 0.12;

/// Luma plane of a frame placed at `origin` in client coordinates.
struct Img<'a> {
    f: &'a Frame,
    ox: i32,
    oy: i32,
    luma: Vec<f32>,
}

impl<'a> Img<'a> {
    fn new(f: &'a Frame, origin: (i32, i32)) -> Self {
        let luma = f.bgra.as_chunks::<4>().0.iter().map(|p| p[0].max(p[1]).max(p[2]) as f32).collect();
        Img { f, ox: origin.0, oy: origin.1, luma }
    }

    fn at(&self, x: i32, y: i32) -> f32 {
        let (x, y) = (x - self.ox, y - self.oy);
        if x < 0 || y < 0 || x >= self.f.width as i32 || y >= self.f.height as i32 {
            return 0.0;
        }
        self.luma[(y * self.f.width as i32 + x) as usize]
    }

    fn lerp(&self, x: f32, y: f32) -> f32 {
        bilinear(|i, j| self.at(i, j), x, y)
    }

    fn rgb(&self, x: i32, y: i32) -> Option<[f32; 3]> {
        let (x, y) = (x - self.ox, y - self.oy);
        if x < 0 || y < 0 || x >= self.f.width as i32 || y >= self.f.height as i32 {
            return None;
        }
        let p = &self.f.bgra[(y * self.f.width as i32 + x) as usize * 4..][..3];
        Some([p[2] as f32, p[1] as f32, p[0] as f32])
    }

    /// Mean and standard deviation of luma over a square, on an 8×8 lattice.
    fn spread(&self, x: f32, y: f32, size: f32) -> (f32, f32) {
        let v: Vec<f32> = (0..64)
            .map(|i| {
                self.lerp(x + (i % 8) as f32 * size / 8.0 + size / 16.0, y + (i / 8) as f32 * size / 8.0 + size / 16.0)
            })
            .collect();
        let mean = v.iter().sum::<f32>() / 64.0;
        (mean, (v.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / 64.0).sqrt())
    }

    fn desc(&self, x: f32, y: f32, size: f32) -> Desc {
        describe(|a, b| self.lerp(a, b), x, y, size, 2)
    }
}

/// Running sums for the correlation of a template `t` with image values `i`.
#[derive(Default)]
struct Ncc {
    n: f32,
    t: f32,
    i: f32,
    tt: f32,
    ii: f32,
    ti: f32,
}

impl Ncc {
    fn add(&mut self, t: f32, i: f32) {
        self.n += 1.0;
        self.t += t;
        self.i += i;
        self.tt += t * t;
        self.ii += i * i;
        self.ti += t * i;
    }

    fn cov(&self) -> (f32, f32, f32) {
        let n = self.n.max(1.0);
        (self.ti - self.t * self.i / n, self.tt - self.t * self.t / n, self.ii - self.i * self.i / n)
    }

    fn value(&self) -> f32 {
        let (c, vt, vi) = self.cov();
        if vt <= 0.0 || vi <= 0.0 { 0.0 } else { c / (vt * vi).sqrt() }
    }

    /// Luma difference between template-on and template-off pixels.
    fn amplitude(&self) -> f32 {
        let (c, vt, _) = self.cov();
        if vt <= 0.0 { 0.0 } else { c / vt }
    }
}

fn overlap(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

/// Correlation of the grid's cell borders (lines one source unit thick) with the image.
fn outline(img: &Img, o: (f32, f32), s: f32, vertical: bool, horizontal: bool) -> Ncc {
    let (c, t) = (CELL * s, s.max(1.0));
    let mut acc = Ncc::default();
    // One border band [a, a+t) along the run [r0, r1], one pixel either side included.
    let mut band = |a: f32, r0: f32, r1: f32, vert: bool| {
        for r in r0.ceil() as i32..=r1.floor() as i32 {
            for p in (a - 1.0).floor() as i32..(a + t + 1.0).ceil() as i32 {
                let cov = overlap(p as f32, p as f32 + 1.0, a, a + t);
                acc.add(cov, if vert { img.at(p, r) } else { img.at(r, p) });
            }
        }
    };
    for (sx, sy) in SLOTS {
        let (x0, y0) = (o.0 + sx * PITCH_X * s, o.1 + sy * PITCH_Y * s);
        if vertical {
            band(x0, y0 + t + 1.0, y0 + c - t - 1.0, true);
            band(x0 + c - t, y0 + t + 1.0, y0 + c - t - 1.0, true);
        }
        if horizontal {
            band(y0, x0 + t + 1.0, x0 + c - t - 1.0, false);
            band(y0 + c - t, x0 + t + 1.0, x0 + c - t - 1.0, false);
        }
    }
    acc
}

/// Top-left of the row's first cell when its grid outline is there.
fn find_grid(img: &Img, g: &Geometry, half: usize, row: usize) -> Option<(f32, f32)> {
    let (s, p) = (g.s, g.grid(half, row));
    let r = (SEARCH * s).ceil() as i32;
    let best = |at: &dyn Fn(i32) -> (f32, f32), v: bool, h: bool| {
        (-r..=r)
            .map(|d| (outline(img, at(d), s, v, h).value(), d))
            .fold((f32::MIN, 0), |a, b| if b.0 > a.0 { b } else { a })
            .1
    };
    let dy = best(&|d| (p.0, p.1 + d as f32), false, true);
    let dx = best(&|d| (p.0 + d as f32, p.1 + dy as f32), true, false);
    let dy = best(&|d| (p.0 + dx as f32, p.1 + d as f32), false, true);
    let o = (p.0 + dx as f32, p.1 + dy as f32);
    let acc = outline(img, o, s, true, true);
    (acc.value() >= GRID_NCC && acc.amplitude() >= GRID_AMP).then_some(o)
}

type Grids = [[Option<(f32, f32)>; 5]; 2];

fn grids(img: &Img, g: &Geometry, halves: &[usize]) -> Grids {
    let mut out = [[None; 5]; 2];
    for &h in halves {
        for (r, slot) in out[h].iter_mut().enumerate() {
            *slot = find_grid(img, g, h, r);
        }
    }
    out
}

fn found(gr: &Grids) -> usize {
    gr.iter().flatten().filter(|o| o.is_some()).count()
}

/// Cheap open check on the two augment columns (`Geometry::column` captures).
pub fn is_open(columns: [(&Frame, (i32, i32)); 2], g: &Geometry) -> bool {
    let n: usize = (0..2).map(|h| found(&grids(&Img::new(columns[h].0, columns[h].1), g, &[h]))).sum();
    n >= OPEN_ROWS
}

/// Most likely rarity from the icon tint (gold: warm, prismatic: magenta, silver: neutral).
fn tint(img: &Img, x: f32, y: f32, size: f32) -> Option<Rarity> {
    let mut px: Vec<[f32; 3]> = (y as i32..(y + size) as i32)
        .flat_map(|j| (x as i32..(x + size) as i32).filter_map(move |i| img.rgb(i, j)))
        .collect();
    if px.is_empty() {
        return None;
    }
    px.sort_by(|a, b| b[0].max(b[1]).max(b[2]).total_cmp(&a[0].max(a[1]).max(a[2])));
    let top = &px[..px.len().div_ceil(7)];
    let n = top.len() as f32;
    let [r, g, b] = [0, 1, 2].map(|c| top.iter().map(|p| p[c]).sum::<f32>() / n);
    if b - g > 25.0 && r - g > 15.0 {
        Some(Rarity::Prismatic)
    } else if r - b > 20.0 && r >= g {
        Some(Rarity::Gold)
    } else if (r - b).abs() < 20.0 && (r - g).abs() < 20.0 {
        Some(Rarity::Silver)
    } else {
        None
    }
}

/// One augment icon read off a row: its icon group, correlation and tint.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub group: usize,
    pub score: f32,
    pub rarity: Option<Rarity>,
}

fn read_cell(img: &Img, x: f32, y: f32, s: f32, aug: &AugIcons) -> Option<Seen> {
    let size = ICON * s;
    if img.spread(x, y, size).1 < EMPTY_SD {
        return None;
    }
    let mut best = (f32::MIN, 0);
    for j in 0..25 {
        let (dx, dy) = ((j % 5) as f32 * 0.5 - 1.0, (j / 5) as f32 * 0.5 - 1.0);
        let b = aug.best(&img.desc(
            x + dx + size * ICON_INSET,
            y + dy + size * ICON_INSET,
            size * (1.0 - 2.0 * ICON_INSET),
        ));
        if b.0 > best.0 {
            best = b;
        }
    }
    (best.0 >= ICON_NCC).then(|| Seen { group: best.1, score: best.0, rarity: tint(img, x, y, size) })
}

/// A player as the Live Client API lists them (scoreboard order within a team).
pub struct BoardPlayer<'a> {
    pub team: Team,
    pub items: &'a [ItemId],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowRead {
    /// Index into the `players` passed to [`read`].
    pub player: usize,
    pub augments: Vec<Seen>,
}

fn permutations() -> Vec<[usize; 5]> {
    let mut out = Vec::with_capacity(120);
    let mut p = [0, 1, 2, 3, 4];
    fn go(k: usize, p: &mut [usize; 5], out: &mut Vec<[usize; 5]>) {
        if k == 5 {
            out.push(*p);
            return;
        }
        for i in k..5 {
            p.swap(k, i);
            go(k + 1, p, out);
            p.swap(k, i);
        }
    }
    go(0, &mut p, &mut out);
    out
}

/// Rows for `team`'s players (API order) maximising matched items; ties keep API order.
/// Returns (score, row of each player).
fn assign(m: &[[u32; 5]; 5], n: usize) -> (u32, [usize; 5]) {
    permutations()
        .into_iter()
        .map(|perm| {
            let s = (0..n).map(|p| 10 * m[perm[p]][p] + u32::from(perm[p] == p)).sum::<u32>();
            (s, perm)
        })
        .max_by_key(|x| x.0)
        .unwrap_or((0, [0, 1, 2, 3, 4]))
}

/// Read the enemy team's augments off a capture of [`Geometry::board`].
/// None when the scoreboard is not open.
pub fn read(
    f: &Frame,
    origin: (i32, i32),
    g: &Geometry,
    players: &[BoardPlayer],
    enemy: Team,
    aug: &AugIcons,
    items: &HashMap<ItemId, Desc>,
) -> Option<Vec<RowRead>> {
    let img = Img::new(f, origin);
    let gr = grids(&img, g, &[0, 1]);
    if found(&gr) < OPEN_ROWS {
        return None;
    }
    // Per-half correction (median of found rows) for rows whose grid was missed and for items.
    let offset = |h: usize, r: usize| -> (f32, f32) {
        let p = g.grid(h, r);
        if let Some(o) = gr[h][r] {
            return (o.0 - p.0, o.1 - p.1);
        }
        let mut d: Vec<(f32, f32)> = gr[h]
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.map(|o| (o.0 - g.grid(h, i).0, o.1 - g.grid(h, i).1)))
            .collect();
        d.sort_by(|a, b| a.0.total_cmp(&b.0));
        d.get(d.len() / 2).copied().unwrap_or((0.0, 0.0))
    };

    // Item evidence: m[half][row][k] = how many of the team's k-th player's items show on that row.
    let teams = [Team::Order, Team::Chaos];
    let of_team =
        |t: Team| players.iter().enumerate().filter(|p| p.1.team == t).map(|p| p.0).take(5).collect::<Vec<_>>();
    let item_size = ITEM * g.s * (1.0 - 2.0 * ITEM_INSET);
    let mut patches: [[Vec<Desc>; 5]; 2] = Default::default();
    for (h, rows) in patches.iter_mut().enumerate() {
        for (r, out) in rows.iter_mut().enumerate() {
            let d = offset(h, r);
            for k in 0..ITEM_SLOTS {
                let p = g.item(h, r, k);
                let (x, y) = (p.0 + d.0 + ITEM * g.s * ITEM_INSET, p.1 + d.1 + ITEM * g.s * ITEM_INSET);
                if img.spread(x, y, item_size).1 >= EMPTY_SD {
                    out.push(img.desc(x, y, item_size));
                }
            }
        }
    }
    let evidence = |h: usize, team: &[usize]| {
        let mut m = [[0u32; 5]; 5];
        for (r, row) in m.iter_mut().enumerate() {
            for (k, &p) in team.iter().enumerate() {
                let mut ids = players[p].items.to_vec();
                ids.sort_unstable();
                ids.dedup();
                row[k] = ids
                    .iter()
                    .filter_map(|id| items.get(id))
                    .filter(|t| patches[h][r].iter().any(|d| dot(d, t) >= ITEM_NCC))
                    .count() as u32;
            }
        }
        m
    };
    let layout = |left: Team| {
        let right = if left == Team::Order { Team::Chaos } else { Team::Order };
        let (l, r) = (of_team(left), of_team(right));
        let (a, b) = (assign(&evidence(0, &l), l.len()), assign(&evidence(1, &r), r.len()));
        (a.0 + b.0, [(l, a.1), (r, b.1)])
    };
    let (order_left, flipped) = (layout(teams[0]), layout(teams[1]));
    let (left, halves) = if flipped.0 > order_left.0 { (Team::Chaos, flipped.1) } else { (Team::Order, order_left.1) };
    let h = if enemy == left { 0 } else { 1 };
    let (team, rows) = &halves[h];

    Some(
        team.iter()
            .enumerate()
            .map(|(k, &p)| {
                let r = rows[k];
                let o = gr[h][r];
                let augments = o
                    .map(|o| {
                        SLOTS
                            .iter()
                            .filter_map(|&(sx, sy)| {
                                read_cell(&img, o.0 + sx * PITCH_X * g.s, o.1 + sy * PITCH_Y * g.s, g.s, aug)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                RowRead { player: p, augments }
            })
            .collect(),
    )
}

#[cfg(test)]
pub(crate) mod render {
    //! Offscreen synthetic scoreboards for tests.
    use super::*;

    pub struct Canvas {
        pub frame: Frame,
    }

    impl Canvas {
        /// A noisy game-like background over the whole client area.
        pub fn new(w: u32, h: u32, seed: u32) -> Self {
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            let mut x = seed.wrapping_mul(2654435761).max(1);
            for (i, p) in bgra.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let (px, py) = ((i as u32 % w) as f32 / w as f32, (i as u32 / w) as f32 / h as f32);
                let n = (x % 40) as f32;
                let g = 60.0 + 70.0 * (px * 6.0).sin().abs() * (py * 4.0).cos().abs() + n;
                p.copy_from_slice(&[(g * 0.6) as u8, (g * 0.9) as u8, g.min(255.0) as u8, 255]);
            }
            Canvas { frame: Frame { width: w, height: h, bgra } }
        }

        pub fn fill(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, rgb: [u8; 3], alpha: f32) {
            let (w, h) = (self.frame.width as i32, self.frame.height as i32);
            for y in (y0.round() as i32).max(0)..(y1.round() as i32).min(h) {
                for x in (x0.round() as i32).max(0)..(x1.round() as i32).min(w) {
                    let p = &mut self.frame.bgra[(y * w + x) as usize * 4..][..3];
                    for (c, v) in [rgb[2], rgb[1], rgb[0]].into_iter().enumerate() {
                        p[c] = (p[c] as f32 * (1.0 - alpha) + v as f32 * alpha) as u8;
                    }
                }
            }
        }

        /// Draw `icon` scaled into the square (x, y, size), tinted (multiply).
        pub fn icon(&mut self, icon: &Rgba, x: f32, y: f32, size: f32, tint: [f32; 3]) {
            let (w, h) = (self.frame.width as i32, self.frame.height as i32);
            for j in y.round() as i32..(y + size).round() as i32 {
                for i in x.round() as i32..(x + size).round() as i32 {
                    if i < 0 || j < 0 || i >= w || j >= h {
                        continue;
                    }
                    let (u, v) = (
                        ((i as f32 + 0.5 - x) / size * icon.w as f32) as i32,
                        ((j as f32 + 0.5 - y) / size * icon.h as f32) as i32,
                    );
                    let s = &icon.px[(v.clamp(0, icon.h as i32 - 1) * icon.w as i32 + u.clamp(0, icon.w as i32 - 1))
                        as usize
                        * 4..][..4];
                    let a = s[3] as f32 / 255.0;
                    let p = &mut self.frame.bgra[(j * w + i) as usize * 4..][..3];
                    for c in 0..3 {
                        let src = s[2 - c] as f32 * tint[2 - c];
                        p[c] = (p[c] as f32 * (1.0 - a) + src * a).min(255.0) as u8;
                    }
                }
            }
        }

        /// The scoreboard panel with every row's augment grid (empty cells) and item backdrops.
        pub fn board(&mut self, g: &Geometry) {
            let s = g.s;
            let [x0, y0, x1, y1] = BOARD;
            let (a, b) = (g.pt(x0, y0), g.pt(x1, y1));
            self.fill(a.0, a.1, b.0, b.1, [22, 26, 32], 0.88);
            for h in 0..2 {
                for r in 0..5 {
                    let o = g.grid(h, r);
                    for (sx, sy) in SLOTS {
                        let (x, y) = (o.0 + sx * PITCH_X * s, o.1 + sy * PITCH_Y * s);
                        let t = s.max(1.0);
                        self.fill(x, y, x + CELL * s, y + CELL * s, [57, 59, 63], 1.0);
                        self.fill(x + t, y + t, x + CELL * s - t, y + CELL * s - t, [9, 9, 9], 1.0);
                    }
                    for k in 0..ITEM_SLOTS {
                        let p = g.item(h, r, k);
                        self.fill(p.0, p.1, p.0 + ITEM * s, p.1 + ITEM * s, [12, 14, 18], 1.0);
                    }
                }
            }
        }

        pub fn augment(&mut self, g: &Geometry, half: usize, row: usize, slot: usize, icon: &Rgba, rarity: Rarity) {
            let o = g.grid(half, row);
            let (sx, sy) = SLOTS[slot];
            let (x, y) = (o.0 + sx * PITCH_X * g.s, o.1 + sy * PITCH_Y * g.s);
            let tint = match rarity {
                Rarity::Gold => [0.62, 0.55, 0.45],
                Rarity::Prismatic => [0.8, 0.6, 0.85],
                Rarity::Silver => [0.75, 0.77, 0.8],
            };
            if rarity == Rarity::Prismatic {
                let t = g.s.max(1.0);
                self.fill(x, y, x + CELL * g.s, y + CELL * g.s, [170, 108, 187], 1.0);
                self.fill(x + t, y + t, x + CELL * g.s - t, y + CELL * g.s - t, [9, 9, 9], 1.0);
            }
            self.icon(icon, x, y, ICON * g.s, tint);
        }

        pub fn item(&mut self, g: &Geometry, half: usize, row: usize, slot: usize, icon: &Rgba) {
            let p = g.item(half, row, slot);
            self.icon(icon, p.0, p.1, ITEM * g.s, [1.0; 3]);
        }

        /// Crop a client rect into a frame (as a capture would).
        pub fn crop(&self, r: Rect) -> Frame {
            let w = self.frame.width as i32;
            let mut bgra = Vec::with_capacity((r.w * r.h * 4) as usize);
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    let inside = x >= 0 && y >= 0 && x < w && y < self.frame.height as i32;
                    bgra.extend_from_slice(if inside {
                        &self.frame.bgra[(y * w + x) as usize * 4..][..4]
                    } else {
                        &[0, 0, 0, 255]
                    });
                }
            }
            Frame { width: r.w as u32, height: r.h as u32, bgra }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::render::Canvas;
    use super::*;
    use std::time::Instant;

    const AUG_PNGS: &[(AugmentId, &[u8])] = &[
        (1001, include_bytes!("../../../fixtures/ocr/augment_icons/iamthejuggernaut_small.png")),
        (1006, include_bytes!("../../../fixtures/ocr/augment_icons/bladewaltz_small.png")),
        (1007, include_bytes!("../../../fixtures/ocr/augment_icons/bluntforce_small.png")),
        (1015, include_bytes!("../../../fixtures/ocr/augment_icons/circleofdeath_small.png")),
        (1030, include_bytes!("../../../fixtures/ocr/augment_icons/eureka_small.png")),
        (1068, include_bytes!("../../../fixtures/ocr/augment_icons/recursion_small.png")),
        (1041, include_bytes!("../../../fixtures/ocr/augment_icons/goliath_small.png")),
        (1305, include_bytes!("../../../fixtures/ocr/augment_icons/legday_small.png")),
        (1331, include_bytes!("../../../fixtures/ocr/augment_icons/empyreampromise_small.png")),
        (1077, include_bytes!("../../../fixtures/ocr/augment_icons/soulsiphon_small.png")),
        (1048, include_bytes!("../../../fixtures/ocr/augment_icons/jeweledgauntlet_small.png")),
        (1353, include_bytes!("../../../fixtures/ocr/augment_icons/tank_engine_small.png")),
    ];
    const ITEM_PNGS: &[(ItemId, &[u8])] = &[
        (3031, include_bytes!("../../../fixtures/ocr/item_icons/3031.png")),
        (3089, include_bytes!("../../../fixtures/ocr/item_icons/3089.png")),
        (3068, include_bytes!("../../../fixtures/ocr/item_icons/3068.png")),
        (3157, include_bytes!("../../../fixtures/ocr/item_icons/3157.png")),
        (6672, include_bytes!("../../../fixtures/ocr/item_icons/6672.png")),
        (3075, include_bytes!("../../../fixtures/ocr/item_icons/3075.png")),
        (3020, include_bytes!("../../../fixtures/ocr/item_icons/3020.png")),
        (3047, include_bytes!("../../../fixtures/ocr/item_icons/3047.png")),
    ];

    fn png(id: u32) -> Rgba {
        let b = AUG_PNGS.iter().chain(ITEM_PNGS).find(|p| p.0 == id).unwrap().1;
        decode_png(b).unwrap()
    }

    fn aug_icons() -> AugIcons {
        AugIcons::new(AUG_PNGS.iter().map(|&(id, b)| (id, decode_png(b).unwrap())))
    }

    fn item_icons() -> HashMap<ItemId, Desc> {
        ITEM_PNGS.iter().map(|&(id, b)| (id, item_desc(&decode_png(b).unwrap()))).collect()
    }

    /// Order: players 0-4 (API order), Chaos: 5-9. Items per player.
    fn lobby() -> Vec<(Team, Vec<ItemId>)> {
        let items: [&[ItemId]; 10] = [
            &[3031],
            &[3089, 3020],
            &[3068],
            &[3157],
            &[6672],
            &[3075, 3047],
            &[3031, 3020],
            &[3089],
            &[6672, 3047],
            &[3068, 3157],
        ];
        items.iter().enumerate().map(|(i, it)| (if i < 5 { Team::Order } else { Team::Chaos }, it.to_vec())).collect()
    }

    type Short = (u32, Option<Rarity>);
    /// (row, [(slot, augment, rarity)]).
    type Augs<'a> = (usize, &'a [(usize, u32, Rarity)]);

    struct Scene {
        canvas: Canvas,
        g: Geometry,
    }

    /// Scoreboard with Order on `left_team`'s side; enemy (Chaos) rows hold `augs`
    /// [(row, [(slot, id, rarity)])]; `row_of[p]` = the row player p is drawn in.
    fn scene(w: u32, h: u32, left_team: Team, row_of: [usize; 5], augs: &[Augs]) -> Scene {
        let g = Geometry::new(w, h);
        let mut canvas = Canvas::new(w, h, w ^ h);
        canvas.board(&g);
        let lobby = lobby();
        for (p, (team, items)) in lobby.iter().enumerate() {
            let half = if *team == left_team { 0 } else { 1 };
            let row = row_of[p % 5];
            for (k, &it) in items.iter().enumerate() {
                canvas.item(&g, half, row, k, &png(it));
            }
        }
        let enemy_half = if left_team == Team::Chaos { 0 } else { 1 };
        for &(row, cells) in augs {
            for &(slot, id, rarity) in cells {
                canvas.augment(&g, enemy_half, row, slot, &png(id), rarity);
            }
        }
        Scene { canvas, g }
    }

    fn run(sc: &Scene) -> Option<Vec<RowRead>> {
        let lobby = lobby();
        let players: Vec<BoardPlayer> = lobby.iter().map(|(t, it)| BoardPlayer { team: *t, items: it }).collect();
        let r = sc.g.board();
        let f = sc.canvas.crop(r);
        read(&f, (r.x, r.y), &sc.g, &players, Team::Chaos, &aug_icons(), &item_icons())
    }

    fn open(sc: &Scene) -> bool {
        let [a, b] = [sc.g.column(0), sc.g.column(1)];
        let (fa, fb) = (sc.canvas.crop(a), sc.canvas.crop(b));
        is_open([(&fa, (a.x, a.y)), (&fb, (b.x, b.y))], &sc.g)
    }

    /// (player, [first candidate id]) per enemy row.
    fn summary(rows: &[RowRead]) -> Vec<(usize, Vec<Short>)> {
        let icons = aug_icons();
        rows.iter()
            .map(|r| (r.player, r.augments.iter().map(|s| (icons.ids(s.group)[0], s.rarity)).collect()))
            .collect()
    }

    const G: Rarity = Rarity::Gold;
    const P: Rarity = Rarity::Prismatic;
    const S: Rarity = Rarity::Silver;

    #[test]
    fn decodes_and_groups_shared_icons() {
        let icons = aug_icons();
        assert_eq!(icons.len(), AUG_PNGS.len() - 1);
        let g = (0..icons.len()).find(|&i| icons.ids(i).contains(&1006)).unwrap();
        assert_eq!(icons.ids(g), [1006, 1007]);
    }

    #[test]
    fn closed_scoreboard_is_not_open() {
        for (w, h) in [(1920, 1080), (2560, 1440), (1280, 720)] {
            let g = Geometry::new(w, h);
            let sc = Scene { canvas: Canvas::new(w, h, 7), g };
            assert!(!open(&sc), "{w}x{h}");
            assert!(run(&sc).is_none());
        }
    }

    #[test]
    fn reads_enemy_rows_across_resolutions() {
        let augs: &[Augs] = &[
            (0, &[(0, 1001, G), (1, 1305, S), (2, 1331, P)]),
            (1, &[(0, 1068, G)]),
            (2, &[(0, 1041, S), (1, 1077, G), (2, 1048, P), (3, 1353, G)]),
            (4, &[(0, 1015, P), (1, 1030, P)]),
        ];
        for (w, h) in [(1920, 1080), (2560, 1440), (1680, 1050), (1280, 720), (3440, 1440), (3840, 2160)] {
            let sc = scene(w, h, Team::Order, [0, 1, 2, 3, 4], augs);
            assert!(open(&sc), "{w}x{h}");
            let t = Instant::now();
            let rows = run(&sc).unwrap_or_else(|| panic!("{w}x{h} not read"));
            println!("{w}x{h}: read {:?}", t.elapsed());
            let got = summary(&rows);
            let want = vec![
                (5, vec![(1001, Some(G)), (1305, Some(S)), (1331, Some(P))]),
                (6, vec![(1068, Some(G))]),
                (7, vec![(1041, Some(S)), (1077, Some(G)), (1048, Some(P)), (1353, Some(G))]),
                (8, vec![]),
                (9, vec![(1015, Some(P)), (1030, Some(P))]),
            ];
            assert_eq!(got, want, "{w}x{h}");
        }
    }

    #[test]
    fn rows_follow_items_when_reordered_or_mirrored() {
        // Chaos drawn on the left, rows shuffled: items decide both.
        let augs: &[Augs] = &[(3, &[(0, 1001, G)]), (0, &[(0, 1015, P)])];
        let sc = scene(1920, 1080, Team::Chaos, [2, 0, 3, 1, 4], augs);
        let got = summary(&run(&sc).unwrap());
        // Player 5+k is drawn in row row_of[k]: row 3 = player 7 (k=2), row 0 = player 6 (k=1).
        let by_player: HashMap<usize, Vec<u32>> =
            got.into_iter().map(|(p, a)| (p, a.into_iter().map(|x| x.0).collect())).collect();
        assert_eq!(by_player[&7], [1001]);
        assert_eq!(by_player[&6], [1015]);
        assert!(by_player[&5].is_empty() && by_player[&8].is_empty() && by_player[&9].is_empty());
    }

    #[test]
    fn assignment_prefers_items_then_api_order() {
        let mut m = [[0u32; 5]; 5];
        assert_eq!(assign(&m, 5).1, [0, 1, 2, 3, 4]);
        m[3][0] = 2; // player 0's items are on row 3
        m[0][3] = 1;
        let (_, rows) = assign(&m, 5);
        assert_eq!((rows[0], rows[3]), (3, 0));
        assert_eq!(&rows[1..3], &[1, 2]);
    }

    #[test]
    fn geometry_scales_from_centre() {
        let g = Geometry::new(1920, 1080);
        let o = g.grid(1, 0);
        assert!((o.0 - 1546.8).abs() < 0.01 && (o.1 - 315.0).abs() < 0.01, "{o:?}");
        let b = g.board();
        assert!(b.x > 0 && b.x + b.w < 1920 && b.y > 0 && b.y + b.h < 1080);
        // Ultrawide keeps the height scale and stays centred.
        let u = Geometry::new(3440, 1440);
        assert!((u.grid(0, 0).0 - (1720.0 - 748.0 * 1.2)).abs() < 0.01);
    }

    /// Manual false-positive check on full screenshots: `SCREENSHOTS=a.png;b.png cargo test -p ocr -- --ignored real_closed`.
    #[test]
    #[ignore]
    fn real_closed() {
        for path in std::env::var("SCREENSHOTS").unwrap().split(';') {
            let img = decode_png(&std::fs::read(path).unwrap()).unwrap();
            let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], 255]).collect();
            let f = Frame { width: img.w, height: img.h, bgra };
            let g = Geometry::new(img.w, img.h);
            let im = Img::new(&f, (0, 0));
            let rows: Vec<String> = (0..10)
                .map(|i| {
                    let (h, r) = (i / 5, i % 5);
                    let acc = outline(&im, g.grid(h, r), g.s, true, true);
                    format!(
                        "{:.2}/{:.0}{}",
                        acc.value(),
                        acc.amplitude(),
                        if find_grid(&im, &g, h, r).is_some() { "*" } else { "" }
                    )
                })
                .collect();
            println!(
                "{path}: {rows:?} read={}",
                read(&f, (0, 0), &g, &[], Team::Chaos, &aug_icons(), &HashMap::new()).is_some()
            );
        }
    }

    /// Manual: `SCOREBOARD_PNG=<1080p crop> SCOREBOARD_AT=x,y cargo test -p ocr -- --ignored real_crop`
    /// reads one real grid (top-left of its first cell at x,y) against icons in `LOL_ICON_DIR`.
    #[test]
    #[ignore]
    fn real_crop() {
        let path = std::env::var("SCOREBOARD_PNG").unwrap();
        let img = decode_png(&std::fs::read(path).unwrap()).unwrap();
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        let f = Frame { width: img.w, height: img.h, bgra };
        let at: Vec<f32> = std::env::var("SCOREBOARD_AT").unwrap().split(',').map(|v| v.parse().unwrap()).collect();
        let dir = std::env::var("LOL_ICON_DIR").unwrap();
        let mut names = Vec::new();
        let icons =
            AugIcons::new(std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).enumerate().filter_map(|(i, e)| {
                names.push(e.file_name().to_string_lossy().into_owned());
                Some((i as AugmentId, decode_png(&std::fs::read(e.path()).ok()?)?))
            }));
        let im = Img::new(&f, (0, 0));
        let s = 0.9;
        let o = (at[0], at[1]);
        let acc = outline(&im, o, s, true, true);
        println!("outline ncc {:.3} amp {:.1}", acc.value(), acc.amplitude());
        for (sx, sy) in SLOTS {
            let c = read_cell(&im, o.0 + sx * PITCH_X * s, o.1 + sy * PITCH_Y * s, s, &icons);
            println!(
                "{:?}",
                c.map(|c| (
                    icons.ids(c.group).iter().map(|&i| names[i as usize].clone()).collect::<Vec<_>>(),
                    c.score,
                    c.rarity
                ))
            );
        }
    }
}
