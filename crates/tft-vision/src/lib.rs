//! Reads my TFT screen into a `TftSnapshot`: stage, gold, level, XP, HP, streak,
//! shop names, offered augments and the trait panel. One OCR pass over the whole
//! window plus enlarged crops for small text. Positions are measured on 2560×1440
//! captures (HUD at default scale) and scaled to the client size.
//!
//! Not read yet: item bench icons, team size, other players' boards (scouting).

use ocr::{Frame, TextBox};
use std::collections::HashMap;
use tft_model::{Api, TftSetData, TftSnapshot};

/// Capture width the regions below were measured at.
const BASE_W: f32 = 2560.0;
/// "2-3" at the top centre, read at 2×.
const STAGE: (u32, u32, u32, u32) = (990, 0, 120, 50);
/// Just the "4-6" digits, for the thresholded retry.
const STAGE_TIGHT: (u32, u32, u32, u32) = (1020, 8, 60, 36);
/// Trait panel names, read at 2×, thresholded to dark-on-white.
const PANEL: (u32, u32, u32, u32) = (130, 340, 300, 780);
const PANEL_THRESHOLD: u8 = 150;
/// A trait's count digit, left of its name: x, width, height (top = the name's top).
const COUNT: (u32, u32, u32) = (150, 28, 34);
/// A lone digit reads badly: tile it this many times side by side.
const COUNT_TILES: u32 = 5;
/// Streak digit right of the flame (tiled like trait counts); the flame's colour gives win or loss.
const STREAK: (u32, u32, u32, u32) = (1380, 1170, 36, 40);
const FLAME: (u32, u32, u32, u32) = (1335, 1172, 30, 36);
/// Bottom bar: level "Lvl. 4" and XP "0/10" left, gold centre.
const BAR_Y: (f32, f32) = (1165.0, 1220.0);
const XP_X: (f32, f32) = (460.0, 600.0);
const GOLD_X: (f32, f32) = (1170.0, 1300.0);
/// Shop: names' top edge, and slot 0's left edge and slot pitch.
const SHOP_Y: (f32, f32) = (1380.0, 1435.0);
const SHOP_X0: f32 = 590.0;
const SHOP_PITCH: f32 = 269.0;
/// Player list HP column; my own row's HP is drawn larger.
const HP_X: (f32, f32) = (2280.0, 2445.0);
const MY_HP_H: f32 = 17.0;
/// Augment titles sit mid-screen.
const OFFER_Y: (f32, f32) = (300.0, 1100.0);
/// A trait missing from this many reads in a row is gone (single reads drop rows).
const TRAIT_MISSES: u32 = 3;
/// Fuzzy name match: allowed edits per character.
const NAME_EDITS: f64 = 0.25;

/// A rectangle of `f`, enlarged `scale`× (nearest) so OCR reads small digits.
pub fn crop(f: &Frame, x: u32, y: u32, w: u32, h: u32, scale: u32) -> Frame {
    let (w, h) = (w.min(f.width.saturating_sub(x)), h.min(f.height.saturating_sub(y)));
    let s = scale.max(1);
    let mut bgra = Vec::with_capacity((w * h * s * s * 4) as usize);
    for oy in 0..h * s {
        let row = ((y + oy / s) * f.width) as usize * 4;
        for ox in 0..w * s {
            let i = row + (x + ox / s) as usize * 4;
            bgra.extend_from_slice(&f.bgra[i..i + 4]);
        }
    }
    Frame { width: w * s, height: h * s, bgra }
}

/// Bright pixels → black, the rest white (OCR likes dark text on white).
fn threshold(f: &mut Frame, t: u8) {
    for p in f.bgra.chunks_mut(4) {
        let v = if p[0].max(p[1]).max(p[2]) >= t { 0 } else { 255 };
        (p[0], p[1], p[2]) = (v, v, v);
    }
}

/// A trait-count digit as a bitmap (bright pixels set), at base-resolution size.
pub struct Mask {
    pub w: u32,
    pub h: u32,
    pub bits: Vec<bool>,
}

/// Count box relative to the trait name's top edge, in base pixels.
const MASK: (u32, i32, u32, u32) = (146, -4, 34, 42);
/// Digit templates cut from real captures (`examples/digits.rs`): "digit w h bits".
const DIGITS: &str = include_str!("digits.txt");
/// Overlap (lit pixels in both / lit in either) a template match needs.
const DIGIT_MATCH: f32 = 0.6;

/// The count box of the trait row whose name starts at `top` (client pixels), scaled to base size.
pub fn count_mask(f: &Frame, scale: f32, top: u32) -> Mask {
    let (bx, dy, w, h) = MASK;
    let mut bits = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let sx = ((bx + x) as f32 * scale) as u32;
            let sy = (top as i32 + ((dy + y as i32) as f32 * scale) as i32).max(0) as u32;
            let i = ((sy.min(f.height - 1) * f.width + sx.min(f.width - 1)) * 4) as usize;
            bits.push(f.bgra[i].max(f.bgra[i + 1]).max(f.bgra[i + 2]) >= PANEL_THRESHOLD);
        }
    }
    Mask { w, h, bits }
}

fn templates() -> &'static [(u32, Mask)] {
    static T: std::sync::OnceLock<Vec<(u32, Mask)>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        DIGITS
            .lines()
            .filter_map(|l| {
                let mut p = l.split_whitespace();
                let (d, w, h, b) = (p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?);
                Some((d, Mask { w, h, bits: b.bytes().map(|c| c == b'1').collect() }))
            })
            .collect()
    })
}

/// The digit whose template agrees best with `m` (shifted up to 2 px), if close enough.
fn match_digit(m: &Mask) -> Option<u32> {
    let agree = |t: &Mask, dx: i32, dy: i32| {
        let (mut both, mut either) = (0u32, 0u32);
        for y in 0..m.h as i32 {
            for x in 0..m.w as i32 {
                let (tx, ty) = (x + dx, y + dy);
                let tb = tx >= 0 && ty >= 0 && tx < t.w as i32 && ty < t.h as i32 && t.bits[(ty * t.w as i32 + tx) as usize];
                let mb = m.bits[(y * m.w as i32 + x) as usize];
                (both, either) = (both + (tb && mb) as u32, either + (tb || mb) as u32);
            }
        }
        both as f32 / either.max(1) as f32
    };
    templates()
        .iter()
        .filter(|(_, t)| (t.w, t.h) == (m.w, m.h))
        .map(|(d, t)| (*d, (-2..=2).flat_map(|dx| (-2..=2).map(move |dy| (dx, dy))).map(|(dx, dy)| agree(t, dx, dy)).fold(0.0, f32::max)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|(_, s)| *s >= DIGIT_MATCH)
        .map(|(d, _)| d)
}

fn tile(f: &Frame, k: u32) -> Frame {
    let row = (f.width * 4) as usize;
    let bgra = f.bgra.chunks(row).flat_map(|r| std::iter::repeat_n(r, k as usize).flatten().copied()).collect();
    Frame { width: f.width * k, height: f.height, bgra }
}

/// Display names → API names, normalised for matching.
struct Names {
    units: Vec<(String, Api)>,
    traits: Vec<(String, Api)>,
    augments: HashMap<u32, String>,
    augment_api: Vec<Api>,
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// The closest name within the edit budget.
fn best<'a>(text: &str, names: &'a [(String, Api)]) -> Option<&'a Api> {
    let t = norm(text);
    if t.len() < 2 {
        return None;
    }
    names
        .iter()
        .map(|(n, api)| (strsim::levenshtein(&t, n), n, api))
        .filter(|(d, n, _)| (*d as f64) <= (n.len() as f64 * NAME_EDITS).max(1.0))
        .min_by_key(|(d, n, _)| (*d, n.len()))
        .map(|(_, _, api)| api)
}

impl Names {
    fn new(set: &TftSetData) -> Names {
        // Several API names can share a display name (Lux forms): prefer `_Base`, then the shortest.
        let mut by_name: HashMap<String, &Api> = HashMap::new();
        for u in set.units.values() {
            let e = by_name.entry(norm(&u.name)).or_insert(&u.api);
            let better = |a: &Api, b: &Api| (!a.contains("_Base"), a.len()) < (!b.contains("_Base"), b.len());
            if better(&u.api, e) {
                *e = &u.api;
            }
        }
        let mut augment_api: Vec<Api> = set.augments.keys().cloned().collect();
        augment_api.sort();
        Names {
            units: by_name.into_iter().map(|(n, a)| (n, a.clone())).collect(),
            traits: set.traits.values().map(|t| (norm(&t.name), t.api.clone())).collect(),
            augments: augment_api.iter().enumerate().map(|(i, a)| (i as u32, set.augments[a].name.clone())).collect(),
            augment_api,
        }
    }
}

/// Region scale for a client width.
struct Geo(f32);

impl Geo {
    fn v(&self, x: f32) -> f32 {
        x * self.0
    }
    fn r(&self, (x, y, w, h): (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
        let s = |v: u32| (v as f32 * self.0).round() as u32;
        (s(x), s(y), s(w), s(h))
    }
    fn within(&self, b: &TextBox, x: (f32, f32), y: (f32, f32)) -> bool {
        b.x >= self.v(x.0) && b.x <= self.v(x.1) && b.y >= self.v(y.0) && b.y <= self.v(y.1)
    }
}

fn digits(s: &str) -> Option<u32> {
    let d: String = s.chars().filter(char::is_ascii_digit).collect();
    d.parse().ok()
}

/// "2-3" / "J 2-3" → (2, 3).
fn stage(text: &str) -> Option<(u32, u32)> {
    let t: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
    let (a, b) = t.trim_matches('-').split_once('-')?;
    let (a, b) = (a.parse().ok()?, b.chars().next()?.to_digit(10)?);
    (1..=9).contains(&a).then_some((a, b))
}

/// One digit in a small region: enlarged, thresholded and tiled so OCR sees "22222".
fn lone_digit(f: &Frame, (x, y, w, h): (u32, u32, u32, u32)) -> Option<u32> {
    let mut c = crop(f, x, y, w, h, 3);
    threshold(&mut c, PANEL_THRESHOLD);
    ocr::recognize_boxes(&tile(&c, COUNT_TILES))
        .unwrap_or_default()
        .iter()
        .find_map(|t| t.text.chars().find(char::is_ascii_digit).and_then(|d| d.to_digit(10)))
}

fn ocr_crop(f: &Frame, r: (u32, u32, u32, u32), scale: u32) -> Vec<TextBox> {
    ocr::recognize_boxes(&crop(f, r.0, r.1, r.2, r.3, scale)).unwrap_or_default()
}

/// Read one captured frame. None when the TFT HUD isn't on screen (alt-tabbed,
/// loading screen).
pub fn read_frame(f: &Frame, set: &TftSetData, at: u64) -> Option<TftSnapshot> {
    read_with(f, &Names::new(set), at)
}

fn read_with(f: &Frame, names: &Names, at: u64) -> Option<TftSnapshot> {
    let g = Geo(f.width as f32 / BASE_W);
    let boxes = ocr::recognize_boxes(f).ok()?;
    let stage = ocr_crop(f, g.r(STAGE), 2).iter().find_map(|b| stage(&b.text)).or_else(|| {
        // Coloured stage icons next to it can break the plain read: thresholded and tiled.
        let (x, y, w, h) = g.r(STAGE_TIGHT);
        let mut c = crop(f, x, y, w, h, 3);
        threshold(&mut c, PANEL_THRESHOLD);
        ocr::recognize_boxes(&tile(&c, 3)).unwrap_or_default().iter().find_map(|b| stage(&b.text))
    });
    let in_bar = |b: &&TextBox, x: (f32, f32)| g.within(b, x, BAR_Y);
    let level = boxes
        .iter()
        .filter(|b| g.within(b, (250.0, 450.0), BAR_Y))
        .find_map(|b| b.text.split_once("vl").and_then(|(_, n)| digits(n)));
    if stage.is_none() && level.is_none() {
        return None;
    }
    let xp = boxes.iter().filter(|b| in_bar(b, XP_X)).find_map(|b| {
        let (a, c) = b.text.split_once('/')?;
        Some((digits(a)?, digits(c)?))
    });
    let gold = boxes.iter().filter(|b| in_bar(b, GOLD_X)).find_map(|b| digits(&b.text));
    let hp = boxes
        .iter()
        .filter(|b| g.within(b, HP_X, (150.0, 1100.0)) && b.h >= g.v(MY_HP_H))
        .filter(|b| !b.text.is_empty() && b.text.chars().all(|c| c.is_ascii_digit()))
        .max_by(|a, b| a.h.total_cmp(&b.h))
        .and_then(|b| digits(&b.text))
        .filter(|&h| (1..=100).contains(&h));

    // Streak: the number right of the flame; a red/orange flame is a win streak.
    let streak = lone_digit(f, g.r(STREAK)).filter(|&n| n > 0).map(|n| {
        let (x, y, w, h) = g.r(FLAME);
        let fl = crop(f, x, y, w, h, 1);
        let (mut r, mut b) = (0u64, 0u64);
        for p in fl.bgra.chunks(4) {
            (b, r) = (b + p[0] as u64, r + p[2] as u64);
        }
        if r >= b { n as i32 } else { -(n as i32) }
    });

    let mut shop: Vec<Option<Api>> = vec![None; 5];
    for b in boxes.iter().filter(|b| g.within(b, (SHOP_X0, SHOP_X0 + 5.0 * SHOP_PITCH), SHOP_Y)) {
        let slot = ((b.x - g.v(SHOP_X0)) / g.v(SHOP_PITCH)) as usize;
        if slot < 5
            && let Some(api) = best(&b.text, &names.units)
        {
            shop[slot] = Some(api.clone());
        }
    }

    // Trait panel: names from a 2× crop read twice (as is, and thresholded: each
    // misses different rows), top to bottom, then each row's count digit.
    let (px, py, pw, ph) = g.r(PANEL);
    let plain = crop(f, px, py, pw, ph, 2);
    let mut dark = plain.clone();
    threshold(&mut dark, PANEL_THRESHOLD);
    let mut rows: Vec<(u32, &Api)> = vec![];
    for b in [plain, dark].iter().flat_map(|p| ocr::recognize_boxes(p).unwrap_or_default()) {
        if let Some(api) = best(&b.text, &names.traits)
            && !rows.iter().any(|(_, a)| *a == api)
        {
            rows.push((py + (b.y / 2.0) as u32, api));
        }
    }
    rows.sort();
    let (cx, _, cw, ch) = g.r((COUNT.0, 0, COUNT.1, COUNT.2));
    // Templates first; OCR of a lone digit is the fallback, and a thin "1" often reads as nothing.
    let traits = rows
        .into_iter()
        .map(|(top, api)| (api.clone(), match_digit(&count_mask(f, g.0, top)).or_else(|| lone_digit(f, (cx, top, cw, ch))).unwrap_or(1)))
        .collect();

    let offer_lines: Vec<String> = boxes.iter().filter(|b| g.within(b, (0.0, BASE_W), OFFER_Y)).map(|b| b.text.clone()).collect();
    let augment_offer: Vec<Api> = ocr::match_augments(&offer_lines, &names.augments)
        .into_iter()
        .filter_map(|i| names.augment_api.get(i as usize).cloned())
        .take(3)
        .collect();

    Some(TftSnapshot {
        at,
        stage,
        gold,
        level,
        xp,
        hp,
        streak,
        team_size: None,
        shop,
        augment_offer,
        traits,
        items: vec![],
        viewing_other: false,
    })
}

/// Reads the TFT window; keeps the name tables OCR text is matched against and
/// the trait panel of the last reads.
pub struct Reader {
    names: Names,
    /// (trait, count, reads in a row it was missing).
    traits: Vec<(Api, u32, u32)>,
}

impl Reader {
    pub fn new(set: &TftSetData) -> Reader {
        Reader { names: Names::new(set), traits: vec![] }
    }

    /// One read of the screen; None when the TFT window or its HUD isn't up.
    pub fn read(&mut self) -> Option<TftSnapshot> {
        let (w, h) = ocr::league_client()?;
        let f = ocr::capture_client(ocr::scoreboard::Rect { x: 0, y: 0, w: w as i32, h: h as i32 })?;
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        let mut snap = read_with(&f, &self.names, at)?;
        self.smooth(&mut snap);
        Some(snap)
    }

    /// A trait missing from one read keeps its last count for a couple of reads.
    fn smooth(&mut self, snap: &mut TftSnapshot) {
        if snap.viewing_other {
            return;
        }
        for t in &mut self.traits {
            t.2 += 1;
        }
        for (api, n) in &snap.traits {
            match self.traits.iter_mut().find(|t| &t.0 == api) {
                Some(t) => (t.1, t.2) = (*n, 0),
                None => self.traits.push((api.clone(), *n, 0)),
            }
        }
        self.traits.retain(|t| t.2 < TRAIT_MISSES);
        snap.traits = self.traits.iter().map(|t| (t.0.clone(), t.1)).collect();
    }

    /// Read a saved frame (calibration, tests).
    pub fn read_frame(&self, f: &Frame, at: u64) -> Option<TftSnapshot> {
        read_with(f, &self.names, at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_and_digits() {
        assert_eq!(stage("J 2-3"), Some((2, 3)));
        assert_eq!(stage("3-1"), Some((3, 1)));
        assert_eq!(stage("24"), None);
        assert_eq!(digits("Lvl. 4"), Some(4));
    }

    #[test]
    fn names_match_with_ocr_noise() {
        let names = vec![(norm("Kobuko"), "DA_18_Kobuko".to_string()), (norm("Caitlyn"), "DA_18_Caitlyn".to_string())];
        assert_eq!(best("Kobuko", &names).map(String::as_str), Some("DA_18_Kobuko"));
        assert_eq!(best("Caitlvn", &names).map(String::as_str), Some("DA_18_Caitlyn"));
        assert_eq!(best("Reroll", &names), None);
    }
}
