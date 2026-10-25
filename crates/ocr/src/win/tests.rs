//! Offscreen-rendered synthetic augment screens (memory DC + DIB section).
//! Nothing here touches the real screen except the `#[ignore]` manual test.

use super::capture::Dib;
use crate::{Frame, match_augments, recognize};
use model_core::AugmentId;
use std::collections::HashMap;
use std::time::Instant;
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Gdi::*;
use windows::core::{HSTRING, PCWSTR};

const W: i32 = 1800;
const H: i32 = 900;

fn pool() -> HashMap<AugmentId, String> {
    include_str!("../../../../fixtures/ocr/mayhem_names.txt")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, n)| (i as AugmentId + 1, n.trim().to_string()))
        .collect()
}

fn ids(names: &[&str]) -> Vec<AugmentId> {
    let pool = pool();
    names
        .iter()
        .map(|n| *pool.iter().find(|(_, v)| v == n).expect(n).0)
        .collect()
}

fn bgr(rgb: u32) -> COLORREF {
    COLORREF(((rgb & 0xff) << 16) | (rgb & 0xff00) | (rgb >> 16))
}

struct Scene {
    dib: Dib,
}

impl Scene {
    /// Dark vertical gradient with three lighter "cards".
    fn new() -> Self {
        let mut dib = Dib::new(W, H).unwrap();
        let px = dib.pixels_mut();
        for y in 0..H as usize {
            for x in 0..W as usize {
                let card = (y > 150 && y < 820)
                    && [(150, 590), (630, 1170), (1210, 1650)]
                        .iter()
                        .any(|&(a, b)| x > a && x < b);
                let base = 14 + (y * 20 / H as usize) as u8 + ((x * 7 + y * 13) % 9) as u8;
                let v = if card { base + 28 } else { base };
                px[(y * W as usize + x) * 4..][..4].copy_from_slice(&[v + 10, v + 4, v, 255]);
            }
        }
        Self { dib }
    }

    fn text(&mut self, cx: i32, y: i32, size: i32, face: &str, rgb: u32, s: &str) {
        let dc = self.dib.dc;
        unsafe {
            let font = CreateFontW(
                -size,
                0,
                0,
                0,
                FW_BOLD.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                ANTIALIASED_QUALITY,
                0,
                PCWSTR(HSTRING::from(face).as_ptr()),
            );
            let old = SelectObject(dc, font.into());
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, bgr(rgb));
            SetTextAlign(dc, TA_CENTER);
            let wide: Vec<u16> = s.encode_utf16().collect();
            let _ = TextOutW(dc, cx, y, &wide);
            SelectObject(dc, old);
            let _ = DeleteObject(font.into());
        }
    }

    /// Card names (each `|` starts a new line), chrome and description text.
    fn cards(mut self, names: [&str; 3], face: &str, name_rgb: u32) -> Frame {
        self.text(W / 2, 60, 40, face, 0xffffff, "Select an augment");
        for (i, name) in names.iter().enumerate() {
            let cx = 370 + 540 * i as i32;
            for (j, part) in name.split('|').enumerate() {
                self.text(cx, 380 + 52 * j as i32, 44, face, name_rgb, part);
            }
            self.text(cx, 520, 24, face, 0x9aa4b0, "Gain 20% bonus damage");
            self.text(cx, 556, 24, face, 0x9aa4b0, "against champions.");
            self.text(cx, 760, 30, face, 0xd8c890, "Reroll");
        }
        self.dib.snapshot()
    }
}

fn check(frame: &Frame, expect: [&str; 3]) {
    let t = Instant::now();
    let lines = recognize(frame).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let got = match_augments(&lines, &pool());
    println!(
        "{}x{} recognize {ms:.1} ms: {lines:?}",
        frame.width, frame.height
    );
    assert_eq!(got, ids(&expect), "lines: {lines:?}");
}

#[test]
fn synthetic_cards_segoe() {
    let names = [
        "Accelerating Sorcery",
        "Blade Waltz",
        "Courage of the Colossus",
    ];
    check(&Scene::new().cards(names, "Segoe UI", 0xffffff), names);
}

#[test]
fn synthetic_cards_wrapped_arial_gold() {
    let f = Scene::new().cards(
        [
            "Accelerating|Sorcery",
            "Dawnbringer's Resolve",
            "Courage of the|Colossus",
        ],
        "Arial",
        0xf0d060,
    );
    check(
        &f,
        [
            "Accelerating Sorcery",
            "Dawnbringer's Resolve",
            "Courage of the Colossus",
        ],
    );
}

#[test]
fn synthetic_cards_other_names() {
    let names = ["Witchful Thinking", "Ice Cold", "Sword of Blossoming Dawn"];
    check(&Scene::new().cards(names, "Segoe UI", 0xe8e0ff), names);
}

#[test]
fn recognize_timing_1800x900() {
    let frame = Scene::new().cards(
        ["Mad Scientist", "Typhoon", "Quantum Computing"],
        "Segoe UI",
        0xffffff,
    );
    recognize(&frame).unwrap(); // engine warm-up
    let n = 5;
    let t = Instant::now();
    for _ in 0..n {
        recognize(&frame).unwrap();
    }
    println!(
        "recognize 1800x900 avg {:.1} ms",
        t.elapsed().as_secs_f64() * 1e3 / n as f64
    );
}

#[test]
fn works_from_many_threads() {
    let frame = std::sync::Arc::new(Scene::new().cards(
        ["Goliath", "Erosion", "Firebrand"],
        "Segoe UI",
        0xffffff,
    ));
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let f = frame.clone();
            std::thread::spawn(move || match_augments(&recognize(&f).unwrap(), &pool()))
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), ids(&["Goliath", "Erosion", "Firebrand"]));
    }
}

#[test]
fn rejects_bad_frame() {
    assert!(
        recognize(&Frame {
            width: 10,
            height: 10,
            bgra: vec![]
        })
        .is_err()
    );
}

/// Manual: run during a real game with
/// `cargo test -p ocr -- --ignored --nocapture live_capture`.
#[test]
#[ignore]
fn live_capture() {
    let t = Instant::now();
    let Some(frame) = crate::capture_league() else {
        // Time a same-sized grab of the screen instead (discarded, not OCR'd).
        let t = Instant::now();
        let f = super::capture::grab_screen(0, 0, 1792, 936);
        println!(
            "League window not found; 1792x936 screen grab {:?} ok={}",
            t.elapsed(),
            f.is_some()
        );
        return;
    };
    let cap = t.elapsed();
    let lines = recognize(&frame).unwrap();
    println!(
        "{}x{} capture {cap:?}, total {:?}",
        frame.width,
        frame.height,
        t.elapsed()
    );
    for l in &lines {
        println!("  {l}");
    }
    println!(
        "matched (fixture pool): {:?}",
        match_augments(&lines, &pool())
    );
}
