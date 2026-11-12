//! Mayhem augment offer: where the cards are, from the OCR boxes of their titles.
//! Pure: works on boxes already read off a capture of the offer strip.

use crate::matching::{hits, ocr_words};
use crate::scoreboard::Rect;
use crate::{Found, TextBox};
use model_core::AugmentId;
use std::collections::HashMap;

/// Fractions [x0, y0, x1, y1] of the client area holding the three card *titles* (and the
/// category tag under them). Measured on real 1440p Mayhem offers, normal and prismatic:
/// cards span x 22–78 %, titles sit at y ≈ 41 %, descriptions start at ≈ 47 %. Only this
/// strip is read, so chat, kill feed and pings (which often name augments) can't produce
/// false offers.
pub const STRIP: [f32; 4] = [0.18, 0.365, 0.82, 0.462];
/// The middle band read when an offer is expected but the strip shows none.
pub const WIDE: [f32; 4] = [0.10, 0.25, 0.90, 0.60];
/// Top of the title row on the default layout, fraction of the client height.
pub const TITLE_TOP: f32 = 0.40;
/// Card pitch on the default layout, fraction of the client width.
pub const PITCH: f32 = 0.1867;
/// Titles on one row: tops within this share of the client height.
const ROW_TOL: f32 = 0.03;
/// A located row is accepted at this scale of the default layout, centred within
/// `CENTRE_TOL` of the window.
const SCALE: (f32, f32) = (0.6, 1.6);
const CENTRE_TOL: f32 = 0.1;

/// The client rectangle at fractions `f` of a `w`×`h` client area.
pub fn rect(w: u32, h: u32, f: [f32; 4]) -> Rect {
    let (cw, ch) = (w as f32, h as f32);
    Rect { x: (cw * f[0]) as i32, y: (ch * f[1]) as i32, w: (cw * (f[2] - f[0])) as i32, h: (ch * (f[3] - f[1])) as i32 }
}

/// One card title found: its centre x and top y as fractions of the client area, and its box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Card {
    pub id: AugmentId,
    pub x: f32,
    pub y: f32,
    pub found: Found,
}

/// Card titles in `boxes` (OCR of a capture at `at` in a `client`-sized window), in screen
/// order, on the row holding the most of them. `found.scale` is the card pitch over the
/// default pitch (1 for a lone card). Empty when no title matches or the row is outside the
/// sanity range (scale 0.6–1.6, three cards centred within 10 % of the window).
pub fn locate(boxes: &[TextBox], names: &HashMap<AugmentId, String>, at: Rect, client: (u32, u32)) -> Vec<Card> {
    let (cw, ch) = (client.0.max(1) as f32, client.1.max(1) as f32);
    let lines: Vec<String> = boxes.iter().map(|b| b.text.clone()).collect();
    let ocr = ocr_words(&lines);
    // A word's horizontal share of its line box: by characters, so titles OCR merged into one line split apart.
    let (mut offset, mut length) = (vec![0usize; ocr.len()], vec![0usize; boxes.len()]);
    for (i, (l, w)) in ocr.iter().enumerate() {
        let n = &mut length[*l];
        *n += usize::from(*n > 0);
        offset[i] = *n;
        *n += w.chars().count();
    }
    let cards: Vec<Card> = hits(&ocr, names)
        .into_iter()
        .map(|h| {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for i in h.start..h.end {
                let (l, w) = &ocr[i];
                let (b, n) = (&boxes[*l], length[*l].max(1) as f32);
                x0 = x0.min(b.x + b.w * offset[i] as f32 / n);
                x1 = x1.max(b.x + b.w * (offset[i] + w.chars().count()) as f32 / n);
                y0 = y0.min(b.y);
                y1 = y1.max(b.y + b.h);
            }
            let rect = Rect { x: at.x + x0.floor() as i32, y: at.y + y0.floor() as i32, w: (x1 - x0).ceil() as i32, h: (y1 - y0).ceil() as i32 };
            Card { id: h.id, x: (at.x as f32 + (x0 + x1) / 2.0) / cw, y: (at.y as f32 + y0) / ch, found: Found { rect, scale: 1.0, score: h.quality } }
        })
        .collect();
    let row = cards
        .iter()
        .map(|c| cards.iter().filter(|d| (d.y - c.y).abs() <= ROW_TOL).count())
        .enumerate()
        .max_by_key(|&(i, n)| (n, std::cmp::Reverse(i)))
        .map(|(i, _)| cards[i].y);
    let Some(row) = row else { return vec![] };
    let mut cards: Vec<Card> = cards.into_iter().filter(|c| (c.y - row).abs() <= ROW_TOL).collect();
    cards.sort_by(|a, b| a.x.total_cmp(&b.x));
    let span = cards.last().map_or(0.0, |c| c.x) - cards[0].x;
    let scale = if cards.len() < 2 { 1.0 } else { span / (span / PITCH).round().max(1.0) / PITCH };
    let centre = (cards[0].x + cards[cards.len() - 1].x) / 2.0;
    if !(SCALE.0..=SCALE.1).contains(&scale) || (cards.len() == 3 && (centre - 0.5).abs() > CENTRE_TOL) {
        return vec![];
    }
    cards.iter_mut().for_each(|c| c.found.scale = scale);
    cards
}

/// The strip to read next for a title row with its top at `top` at `scale` of the default layout.
pub fn strip_for(top: f32, scale: f32) -> [f32; 4] {
    let half = (0.5 - STRIP[0]) * scale;
    [
        (0.5 - half).max(0.0),
        (top - (TITLE_TOP - STRIP[1]) * scale).max(0.0),
        (0.5 + half).min(1.0),
        (top + (STRIP[3] - TITLE_TOP) * scale).min(1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> HashMap<AugmentId, String> {
        [(1, "Accelerating Sorcery"), (2, "Blade Waltz"), (3, "Courage of the Colossus"), (4, "Goliath")]
            .into_iter()
            .map(|(id, n)| (id, n.to_string()))
            .collect()
    }

    fn tb(text: &str, x: f32, y: f32, w: f32, h: f32) -> TextBox {
        TextBox { text: text.into(), x, y, w, h }
    }

    const CLIENT: (u32, u32) = (2560, 1440);

    fn strip() -> Rect {
        rect(CLIENT.0, CLIENT.1, STRIP)
    }

    /// Title boxes centred on the default card centres, in strip pixels.
    fn titles(texts: [&str; 3], top: f32) -> Vec<TextBox> {
        let s = strip();
        texts
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let cx = (0.5 + (i as f32 - 1.0) * PITCH) * CLIENT.0 as f32 - s.x as f32;
                let w = 18.0 * t.len() as f32;
                tb(t, cx - w / 2.0, top * CLIENT.1 as f32 - s.y as f32, w, 30.0)
            })
            .collect()
    }

    #[test]
    fn cards_sit_at_their_title_centres() {
        let mut boxes = titles(["Accelerating Sorcery", "Blade Waltz", "Courage of the Colossus"], TITLE_TOP);
        boxes.push(tb("Gain 20% bonus damage", 300.0, 120.0, 400.0, 20.0));
        let cards = locate(&boxes, &names(), strip(), CLIENT);
        assert_eq!(cards.iter().map(|c| c.id).collect::<Vec<_>>(), [1, 2, 3]);
        for (c, want) in cards.iter().zip([0.5 - PITCH, 0.5, 0.5 + PITCH]) {
            assert!((c.x - want).abs() < 0.002, "{c:?}");
            assert!((c.y - TITLE_TOP).abs() < 0.002, "{c:?}");
            assert!((c.found.scale - 1.0).abs() < 0.01 && c.found.score == 1.0, "{c:?}");
        }
        let r = cards[1].found.rect;
        assert!((r.x + r.w / 2 - 1280).abs() <= 2 && (r.y - 576).abs() <= 1, "{r:?}");
    }

    #[test]
    fn titles_merged_into_one_line_split_by_characters() {
        let s = strip();
        let boxes = vec![tb("Accelerating Sorcery Blade Waltz Courage of the Colossus", 0.0, 100.0, s.w as f32, 30.0)];
        let cards = locate(&boxes, &names(), s, CLIENT);
        assert_eq!(cards.iter().map(|c| c.id).collect::<Vec<_>>(), [1, 2, 3]);
        assert!((cards[1].x - 0.5).abs() < 0.03 && cards[0].x < 0.4 && cards[2].x > 0.6, "{cards:?}");
    }

    #[test]
    fn wrapped_title_spans_both_lines() {
        let s = strip();
        let cx = 1280.0 - s.x as f32;
        let boxes = vec![tb("Courage of the", cx - 100.0, 100.0, 200.0, 30.0), tb("Colossus", cx - 60.0, 135.0, 120.0, 30.0)];
        let c = locate(&boxes, &names(), s, CLIENT);
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].found.rect.y, c[0].found.rect.h), (s.y + 100, 65));
        assert!((c[0].x - 0.5).abs() < 0.001);
    }

    #[test]
    fn a_bigger_layout_scales_the_pitch_and_moves_the_strip() {
        let s = rect(CLIENT.0, CLIENT.1, WIDE);
        let boxes: Vec<TextBox> = ["Accelerating Sorcery", "Blade Waltz", "Courage of the Colossus"]
            .iter()
            .enumerate()
            .map(|(i, t)| tb(t, (0.5 + (i as f32 - 1.0) * PITCH * 1.2) * 2560.0 - s.x as f32 - 50.0, 0.45 * 1440.0 - s.y as f32, 100.0, 36.0))
            .collect();
        let cards = locate(&boxes, &names(), s, CLIENT);
        assert_eq!(cards.len(), 3);
        assert!((cards[0].found.scale - 1.2).abs() < 0.01, "{cards:?}");
        let st = strip_for(cards[0].y, 1.2);
        assert!(st[1] < 0.45 && st[3] > 0.45 + 0.04 && st[0] < STRIP[0] && st[2] > STRIP[2], "{st:?}");
        assert_eq!(strip_for(TITLE_TOP, 1.0).map(|v| (v * 1e4).round()), STRIP.map(|v| (v * 1e4).round()));
    }

    #[test]
    fn titles_off_the_row_and_off_centre_rows_are_not_cards() {
        let mut boxes = titles(["Accelerating Sorcery", "Blade Waltz", "Courage of the Colossus"], TITLE_TOP);
        boxes.push(tb("Goliath", 600.0, 400.0, 100.0, 30.0));
        assert_eq!(locate(&boxes, &names(), strip(), CLIENT).iter().map(|c| c.id).collect::<Vec<_>>(), [1, 2, 3]);
        let shifted: Vec<TextBox> = titles(["Accelerating Sorcery", "Blade Waltz", "Courage of the Colossus"], TITLE_TOP)
            .into_iter()
            .map(|b| TextBox { x: b.x + 0.15 * 2560.0, ..b })
            .collect();
        assert!(locate(&shifted, &names(), strip(), CLIENT).is_empty());
        assert!(locate(&[tb("Reroll", 10.0, 10.0, 80.0, 20.0)], &names(), strip(), CLIENT).is_empty());
        assert!(locate(&[], &names(), strip(), CLIENT).is_empty());
    }
}
