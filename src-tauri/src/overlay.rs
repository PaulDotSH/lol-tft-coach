//! In-game overlay: click-through windows over the game window that never take focus.
//! It shows only what the dashboard already has: augment ranks on the offer cards (ranks
//! only, never win rates or placement stats), Buy next with the gold missing, the newest call
//! and the objective timers (LoL); the target comp, augment ranks and the roll / level call (TFT).
//!
//! Shown only while the game window has focus (hidden on alt-tab), like other overlay tools.
//! Two small windows, each only as big as what it draws and hidden while empty: a window
//! covering the whole game made Windows compose every game frame with it (FPS drop). The strip
//! (Buy next, call, timers) sits above the minimap (LoL) or top left (TFT); the card band shows
//! only while an offer is up.
//!
//! Created hidden at startup (never while a game runs), then only ever shown with
//! SWP_NOACTIVATE / hidden with SW_HIDE: Tauri's own `show()` activates the window.
//! Exclusive fullscreen draws over every overlay; borderless or windowed only.

use crate::state::Shared;
use crate::tft::TftShared;
use model::*;
use ocr::scoreboard::Rect;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "overlay";
pub const CARDS_LABEL: &str = "overlay-cards";
/// The strip's window, as shares of the game window: right edge 0.6 % in and bottom 27 % up
/// (above a default-size minimap) for LoL, 1 % / 9 % from the top left for TFT.
const STRIP_W: f32 = 0.22;
const STRIP_H: f32 = 0.16;
/// The card band: across the cards, from the badge row down.
const BAND_X: f32 = 0.15;
const BAND_W: f32 = 0.7;
const BAND_H: f32 = 0.06;
/// The game window and its focus are followed at this rate.
const TRACK_EVERY: Duration = Duration::from_millis(500);
/// Offer cards per row, LoL Mayhem and TFT alike. A row with a title unread isn't placed:
/// the screen order no longer says which card is which.
const CARDS: usize = 3;
/// Mayhem cards span x 22–78 % of the client area (ocr's measured title strip), titles at y ≈ 41 %:
/// badges sit just above the titles.
const LOL_PITCH: f32 = 0.1867;
const LOL_BADGE_Y: f32 = 0.33;
/// TFT augment cards, side by side and centred; the badge row at the top of tft-vision's offer
/// strip (y 300 of 1440). First guesses: check in a real game.
const TFT_PITCH: f32 = 0.19;
const TFT_BADGE_Y: f32 = 0.2;

/// The last view sent, for the overlay page's first load.
#[derive(Default)]
pub struct OverlayShared {
    view: Mutex<Option<Overlay>>,
}

#[tauri::command]
pub fn get_overlay(ov: tauri::State<Arc<OverlayShared>>) -> Option<Overlay> {
    ov.view.lock().unwrap().clone()
}

/// The overlay windows, hidden. Called once from setup, before any game. The page draws its
/// part in units of the game window's height: `h` is the window's share of it.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    window(app, LABEL, &format!("overlay.html?part=strip&h={STRIP_H}"))?;
    window(app, CARDS_LABEL, &format!("overlay.html?part=cards&h={BAND_H}"))
}

fn window(app: &AppHandle, label: &str, url: &str) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title("LoL Coach overlay")
        .visible(false)
        .focused(false)
        .focusable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .build()?;
    w.set_ignore_cursor_events(true)?;
    #[cfg(windows)]
    win::init(w.hwnd()?.0 as isize);
    Ok(())
}

/// Follows the game window and pushes the view, on its own thread.
pub fn spawn(app: AppHandle, sh: Arc<Shared>, tft: Arc<TftShared>, ov: Arc<OverlayShared>) {
    #[cfg(windows)]
    let hwnds = {
        use tauri::Manager;
        [LABEL, CARDS_LABEL].map(|l| app.get_webview_window(l).and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize))
    };
    std::thread::Builder::new()
        .name("overlay".into())
        .spawn(move || {
            let mut placed: [Option<Option<Rect>>; 2] = [None, None];
            loop {
                let view = match tft.overlay_input() {
                    Some((advice, offer)) => Some(tft_view(advice.as_ref(), &offer)),
                    None => {
                        let seen = crate::orchestrator::cards_on_screen(&sh);
                        let st = sh.ui.lock().unwrap();
                        st.game.as_ref().filter(|_| st.phase == Phase::InGame).map(|g| lol_view(g, &seen))
                    }
                };
                let client = placement(sh.settings().overlay, view.is_some() && ocr::league_foreground(), ocr::league_rect());
                let view = view.filter(|_| client.is_some());
                let rects = windows(client, view.as_ref());
                {
                    let mut last = ov.view.lock().unwrap();
                    if *last != view {
                        for label in [LABEL, CARDS_LABEL] {
                            let _ = app.emit_to(label, "overlay", &view);
                        }
                        *last = view;
                    }
                }
                // Re-checked every tick (cheap): the window library may hide one or rewrite its style.
                #[cfg(windows)]
                for ((h, rect), was) in hwnds.iter().zip(rects).zip(placed) {
                    if let Some(h) = *h
                        && (rect.is_some() || was != Some(None))
                    {
                        let _ = app.run_on_main_thread(move || win::place(h, rect));
                    }
                }
                placed = rects.map(Some);
                std::thread::sleep(TRACK_EVERY);
            }
        })
        .expect("spawn overlay thread");
}

/// Where the overlay goes: exactly over the game window's client area, while a game with
/// something to draw runs and the setting is on. None hides it.
fn placement(enabled: bool, in_game: bool, client: Option<Rect>) -> Option<Rect> {
    let r = client.filter(|_| enabled && in_game)?;
    // Minimised windows sit at (-32000, -32000) with an empty client area.
    (r.w > 0 && r.h > 0 && r.x > -32000 && r.y > -32000).then_some(r)
}

/// [strip, card band] over the game's client area; None hides one (nothing to draw).
fn windows(client: Option<Rect>, view: Option<&Overlay>) -> [Option<Rect>; 2] {
    let (Some(r), Some(v)) = (client, view) else { return [None, None] };
    let share = |x: f32, w: f32, y: f32, h: f32| Rect {
        x: r.x + (r.w as f32 * x).round() as i32,
        y: r.y + (r.h as f32 * y).round() as i32,
        w: (r.w as f32 * w).round() as i32,
        h: (r.h as f32 * h).round() as i32,
    };
    let strip = (v.head.is_some() || v.call.is_some() || !v.timers.is_empty()).then(|| match v.tft {
        true => share(0.01, STRIP_W, 0.09, STRIP_H),
        false => share(1.0 - 0.006 - STRIP_W, STRIP_W, 1.0 - 0.27 - STRIP_H, STRIP_H),
    });
    let band = (!v.badges.is_empty()).then(|| share(BAND_X, BAND_W, v.badge_y, BAND_H));
    [strip, band]
}

/// Centres of `n` cards side by side around the middle, `pitch` apart (fractions of the width).
fn card_centres(n: usize, pitch: f32) -> Vec<f32> {
    (0..n).map(|i| 0.5 + (i as f32 - (n as f32 - 1.0) / 2.0) * pitch).collect()
}

/// Badges for cards in screen order: (place, reason) each.
fn badges(cards: Vec<(Option<u32>, Option<String>)>, pitch: f32) -> Vec<OverlayBadge> {
    if cards.len() != CARDS {
        return vec![];
    }
    card_centres(CARDS, pitch)
        .into_iter()
        .zip(cards)
        .map(|(x, (place, reason))| OverlayBadge { x: (x - BAND_X) / BAND_W, place, reason })
        .collect()
}

/// LoL: the advice's offer, while those same cards are on screen.
fn lol_badges(advice: Option<&AugmentAdvice>, seen: &[AugmentId]) -> Vec<OverlayBadge> {
    let Some(a) = advice else { return vec![] };
    if !a.offered.iter().map(|r| r.id).eq(seen.iter().copied()) {
        return vec![];
    }
    let places = coach::offer_places(&a.offered);
    badges(a.offered.iter().zip(places).map(|(r, p)| (p, lol_reason(r))).collect(), LOL_PITCH)
}

/// The card's own reason: what it builds on, else its rank in my champion's list.
fn lol_reason(r: &AugmentRank) -> Option<String> {
    r.synergy.clone().or_else(|| {
        let rank = format!("#{} of {}", r.rank? + 1, r.of);
        Some(match r.rarity {
            Some(x) => format!("{rank} {x:?}"),
            None => rank,
        })
    })
}

/// TFT: places from the coach's ranking (best first), cards in screen order.
fn tft_badges(ranks: &[tft_model::AugmentRank], offer: &[tft_model::Api]) -> Vec<OverlayBadge> {
    let cards = offer
        .iter()
        .map(|a| match ranks.iter().position(|r| &r.augment == a) {
            Some(i) => (Some(i as u32), tft_reason(&ranks[i].reason)),
            None => (None, None),
        })
        .collect();
    badges(cards, TFT_PITCH)
}

/// The comp part of the coach's reason ("good in X"); placement stats stay off the overlay.
fn tft_reason(reason: &str) -> Option<String> {
    reason.split("; ").find(|p| p.starts_with("good in")).map(str::to_string)
}

fn gold_text(e: &GoldEta) -> String {
    match (e.missing, e.eta_s) {
        (0, _) => "affordable now".into(),
        (m, Some(s)) if s >= 90.0 => format!("{m} g short · ~{} min", (s / 60.0).round()),
        (m, Some(s)) => format!("{m} g short · ~{} s", ((s / 5.0).round() * 5.0).max(5.0)),
        (m, None) => format!("{m} g short"),
    }
}

fn lol_view(g: &GameAdvice, seen: &[AugmentId]) -> Overlay {
    let buy = g.buy_next.as_ref();
    Overlay {
        tft: false,
        badge_y: LOL_BADGE_Y,
        badges: lol_badges(g.augments.as_ref(), seen),
        head: buy.map(|p| format!("Buy {}", p.name)),
        detail: g.gold_eta.as_ref().filter(|e| buy.is_some_and(|p| e.item == Some(p.item))).map(gold_text),
        call: g.calls.first().map(|c| c.text.clone()),
        time: g.time,
        timers: g.timers.clone(),
    }
}

fn tft_view(advice: Option<&tft_model::TftAdvice>, offer: &[tft_model::Api]) -> Overlay {
    let target = advice.and_then(|a| a.comps.first());
    Overlay {
        tft: true,
        badge_y: TFT_BADGE_Y,
        badges: advice.map_or(vec![], |a| tft_badges(&a.augments, offer)),
        head: target.map(|c| format!("Play {}", c.name)),
        detail: target.filter(|c| !c.missing.is_empty()).map(|c| format!("{} of {} units", c.have.len(), c.have.len() + c.missing.len())),
        call: advice.and_then(|a| a.econ.as_ref()).map(|e| e.text.clone()),
        time: 0.0,
        timers: vec![],
    }
}

#[cfg(windows)]
mod win {
    use super::Rect;
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::*;

    /// Never activated, clicks fall through, no taskbar / Alt-Tab entry.
    const EX: WINDOW_EX_STYLE = WINDOW_EX_STYLE(WS_EX_NOACTIVATE.0 | WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0 | WS_EX_TOOLWINDOW.0);

    fn hwnd(h: isize) -> HWND {
        HWND(h as *mut _)
    }

    /// Kept on every placement: the window library rewrites the extended style when its flags change.
    fn ensure_style(h: HWND) {
        unsafe {
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            let want = EX.0 as isize;
            if ex & want != want {
                let _ = SetWindowLongPtrW(h, GWL_EXSTYLE, ex | want);
            }
        }
    }

    pub fn init(h: isize) {
        ensure_style(hwnd(h));
    }

    /// Show over `r` without activating, or hide. Main thread.
    pub fn place(h: isize, r: Option<Rect>) {
        let h = hwnd(h);
        unsafe {
            let visible = IsWindowVisible(h).as_bool();
            let Some(r) = r else {
                if visible {
                    let _ = ShowWindow(h, SW_HIDE);
                }
                return;
            };
            ensure_style(h);
            let mut cur = RECT::default();
            let _ = GetWindowRect(h, &mut cur);
            if !visible || (cur.left, cur.top, cur.right - cur.left, cur.bottom - cur.top) != (r.x, r.y, r.w, r.h) {
                let _ = SetWindowPos(h, Some(HWND_TOPMOST), r.x, r.y, r.w, r.h, SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect { x: 0, y: 0, w: 2560, h: 1440 };

    #[test]
    fn placement_follows_window_setting_and_game() {
        assert_eq!(placement(true, true, Some(R)), Some(R));
        let windowed = Rect { x: 200, y: 131, w: 1600, h: 900 };
        assert_eq!(placement(true, true, Some(windowed)), Some(windowed));
        assert_eq!(placement(false, true, Some(R)), None);
        assert_eq!(placement(true, false, Some(R)), None);
        assert_eq!(placement(true, true, None), None);
        assert_eq!(placement(true, true, Some(Rect { x: -32000, y: -32000, w: 160, h: 28 })), None);
        assert_eq!(placement(true, true, Some(Rect { w: 0, h: 0, ..R })), None);
    }

    #[test]
    fn small_windows_only_for_what_is_drawn() {
        let view = |tft: bool, badges: usize, timers: bool| Overlay {
            tft,
            badge_y: LOL_BADGE_Y,
            badges: (0..badges).map(|_| OverlayBadge { x: 0.5, place: None, reason: None }).collect(),
            head: None,
            detail: None,
            call: None,
            time: 0.0,
            timers: if timers { vec![ObjectiveTimer { name: "Dragon".into(), next_spawn: Some(300.0) }] } else { vec![] },
        };
        // LoL strip above the minimap, right edge 0.6 % in; nothing to draw, nothing shown.
        let [strip, band] = windows(Some(R), Some(&view(false, 0, true)));
        let s = strip.unwrap();
        assert_eq!((s.w, s.h), (563, 230));
        assert_eq!((s.x + s.w, s.y + s.h), (2544, 1051));
        assert_eq!(band, None);
        assert_eq!(windows(Some(R), Some(&view(false, 0, false))), [None, None]);
        assert_eq!(windows(None, Some(&view(false, 3, true))), [None, None]);
        // Cards up: a band across them from the badge row; TFT strip top left.
        let [strip, band] = windows(Some(R), Some(&view(true, 3, true)));
        assert_eq!(strip.map(|s| (s.x, s.y)), Some((26, 130)));
        assert_eq!(band, Some(Rect { x: 384, y: 475, w: 1792, h: 86 }));
    }

    #[test]
    fn cards_are_centred() {
        let c = card_centres(3, LOL_PITCH);
        assert!((c[0] - 0.3133).abs() < 1e-3 && (c[1] - 0.5).abs() < 1e-6 && (c[2] - 0.6867).abs() < 1e-3);
        // Mayhem cards span 22–78 %: the outer centres sit half a card in.
        assert!((c[0] - LOL_PITCH / 2.0 - 0.22).abs() < 1e-3);
        assert_eq!(card_centres(1, 0.2), [0.5]);
    }

    fn lol_rank(id: AugmentId, rank: Option<u32>, synergy: Option<&str>) -> AugmentRank {
        AugmentRank { id, name: String::new(), rarity: rank.map(|_| Rarity::Gold), rank, of: 20, synergy: synergy.map(str::to_string) }
    }

    #[test]
    fn lol_badges_follow_screen_order_while_the_cards_show() {
        let advice = AugmentAdvice {
            offered: vec![lol_rank(7, Some(9), None), lol_rank(8, None, None), lol_rank(9, Some(2), Some("fits your Rabadon's Deathcap (AP)"))],
            best: Some(9),
            reason: String::new(),
        };
        let b = lol_badges(Some(&advice), &[7, 8, 9]);
        assert_eq!(b.iter().map(|b| b.place).collect::<Vec<_>>(), [Some(1), None, Some(0)]);
        assert_eq!(b[0].reason.as_deref(), Some("#10 of 20 Gold"));
        assert_eq!(b[1].reason, None);
        assert_eq!(b[2].reason.as_deref(), Some("fits your Rabadon's Deathcap (AP)"));
        assert!(b[0].x < b[1].x && b[1].x < b[2].x);
        // Centres as shares of the card band.
        assert!((b[1].x - 0.5).abs() < 1e-6 && b[0].x > 0.0 && b[2].x < 1.0);
        // Cards gone (shop open), replaced, or a title unread: no badges.
        assert!(lol_badges(Some(&advice), &[]).is_empty());
        assert!(lol_badges(Some(&advice), &[7, 8, 10]).is_empty());
        let two = AugmentAdvice { offered: advice.offered[..2].to_vec(), ..advice.clone() };
        assert!(lol_badges(Some(&two), &[7, 8]).is_empty());
        assert!(lol_badges(None, &[7, 8, 9]).is_empty());
    }

    #[test]
    fn tft_badges_rank_in_screen_order_without_stats() {
        let rank = |a: &str, reason: &str| tft_model::AugmentRank { augment: a.into(), score: 0.0, reason: reason.into() };
        let ranks = [rank("B", "avg place 3.90 over 900 games; good in Defender Rengar"), rank("C", "avg place 4.20 over 300 games")];
        let offer: Vec<String> = ["A", "B", "C"].map(String::from).to_vec();
        let b = tft_badges(&ranks, &offer);
        assert_eq!(b.iter().map(|b| b.place).collect::<Vec<_>>(), [None, Some(0), Some(1)]);
        assert_eq!(b[1].reason.as_deref(), Some("good in Defender Rengar"));
        assert_eq!(b[2].reason, None);
        assert!(b.iter().all(|b| !b.reason.as_deref().unwrap_or_default().contains("avg")));
        assert!(tft_badges(&ranks, &offer[..2]).is_empty());
    }

    #[test]
    fn gold_text_matches_the_dashboard() {
        let eta = |missing, eta_s| GoldEta { item: Some(3089), per_min: Some(400.0), missing, eta_s, backs: vec![] };
        assert_eq!(gold_text(&eta(0, None)), "affordable now");
        assert_eq!(gold_text(&eta(450, Some(41.0))), "450 g short · ~40 s");
        assert_eq!(gold_text(&eta(450, Some(1.0))), "450 g short · ~5 s");
        assert_eq!(gold_text(&eta(1800, Some(200.0))), "1800 g short · ~3 min");
        assert_eq!(gold_text(&eta(1800, None)), "1800 g short");
    }
}
