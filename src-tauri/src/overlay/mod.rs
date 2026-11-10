//! In-game overlay: click-through windows over the game window that never take focus.
//! It shows only what the dashboard already has: augment ranks on the offer cards (ranks
//! only, never win rates or placement stats), Buy next with the gold missing, the newest call
//! and the objective timers (LoL); the target comp, augment ranks and the roll / level call (TFT).
//!
//! Two native windows (see `native`), each only as big as what it draws and hidden while
//! empty or while the game isn't the foreground window: the strip (Buy next, call, timers)
//! above the minimap (LoL) or top left (TFT), and the rank badges while an offer is up.
//! Exclusive fullscreen draws over every overlay; borderless or windowed only.
//!
//! The strip only flashes up for a few seconds when what it says changes. Measured with
//! PresentMon (2026-10-28): any window over League, even an opaque box, cuts it from 165 to
//! ~90–130 FPS while it shows, MPO on or off; overlays without that cost draw inside the game
//! (Overwolf), which an app outside Overwolf can't do under Vanguard.

#[cfg(windows)]
mod native;

use crate::state::Shared;
use crate::tft::TftShared;
use model::*;
use ocr::offer::Card;
use ocr::scoreboard::Rect;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The strip: at most this share of the game window wide; its corner 0.6 % in from the right
/// and on the minimap's top edge (from League's minimap scale) for LoL, 1 % / 9 % from the top left for TFT.
const STRIP_W: f32 = 0.22;
/// A badge: at most this share of the game window wide.
const BADGE_W: f32 = 0.17;
/// The game window and its focus are followed at this rate.
const TRACK_EVERY: Duration = Duration::from_millis(500);
/// The timers count on from the last view for at most this long (a stalled feed stops them).
const COUNT_ON: f32 = 5.0;
/// The strip shows this long after what it says changes.
const FLASH: Duration = Duration::from_secs(6);
/// A timer this close (s), or up, is news for the strip.
const TIMER_SOON: f32 = 60.0;
/// Offer cards per row, LoL Mayhem and TFT alike. A row with a title unread isn't placed:
/// the screen order no longer says which card is which.
const CARDS: usize = 3;
/// Mayhem cards span x 22–78 % of the client area (ocr's measured title strip), titles at y ≈ 41 %:
/// badges sit just above the titles. The card titles' OCR boxes place them; these are the fallback.
const LOL_PITCH: f32 = ocr::offer::PITCH;
const LOL_BADGE_Y: f32 = 0.33;
/// TFT augment cards, side by side and centred; the badge row at the top of tft-vision's offer
/// strip (y 300 of 1440). First guesses: check in a real game.
const TFT_PITCH: f32 = 0.19;
const TFT_BADGE_Y: f32 = 0.2;
const ORD: [&str; 4] = ["1st", "2nd", "3rd", "4th"];

/// Text colours, the dashboard's palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ink {
    Text,
    Gold,
    Muted,
    Grey,
    Teal,
}

/// A run of text in one ink; a `big` run is a badge's place (1.5×, bold).
#[derive(Clone, Debug, PartialEq)]
struct Run {
    text: String,
    ink: Ink,
    big: bool,
}

fn run(text: impl Into<String>, ink: Ink) -> Run {
    Run { text: text.into(), ink, big: false }
}

/// One line of runs; `bar` puts the call's gold rule at its left.
#[derive(Clone, Debug, PartialEq, Default)]
struct Line {
    runs: Vec<Run>,
    bar: bool,
}

/// What one overlay window draws, in screen pixels; `unit` is 1 % of the game window's height.
#[derive(Clone, Debug, PartialEq)]
enum Scene {
    /// Lines in one box: LoL anchors its bottom-right corner at `at`, TFT its top-left.
    Strip { lines: Vec<Line>, at: (i32, i32), bottom_right: bool, max_w: i32, unit: f32 },
    /// One box per card, centred on its x, tops at `y`; `true` marks the top pick (gold edge).
    Badges { badges: Vec<(i32, Line, bool)>, y: i32, max_w: i32, unit: f32 },
}

/// Follows the game window and pushes the view, on its own thread.
pub fn spawn(sh: Arc<Shared>, tft: Arc<TftShared>) {
    std::thread::Builder::new()
        .name("overlay".into())
        .spawn(move || {
            #[cfg(windows)]
            let (painter, mut wins) = match (native::Painter::new(), native::Window::new(), native::Window::new()) {
                (Ok(p), Ok(a), Ok(b)) => (p, [a, b]),
                (p, a, b) => {
                    let e = [p.err(), a.err(), b.err()].into_iter().flatten().next();
                    return tracing::warn!("overlay windows: {e:?}");
                }
            };
            // The view's game time and when it was taken: the timers count on between views.
            let mut clock = (f32::NAN, Instant::now());
            // What the strip last said, and until when it shows.
            let (mut said, mut until) = (None, Instant::now());
            loop {
                let view = match tft.overlay_input() {
                    Some((advice, offer)) => Some(tft_view(advice.as_ref(), &offer)),
                    None => {
                        let cards = crate::orchestrator::offer_on_screen(&sh);
                        let st = sh.ui.lock().unwrap();
                        st.game.as_ref().filter(|_| st.phase == Phase::InGame).map(|g| lol_view(g, &cards))
                    }
                };
                let enabled = sh.ui.lock().unwrap().settings.overlay;
                let client = placement(enabled, view.is_some() && ocr::league_foreground(), ocr::league_rect());
                let now = match &view {
                    Some(v) if v.time != clock.0 => {
                        clock = (v.time, Instant::now());
                        v.time
                    }
                    _ => clock.0 + clock.1.elapsed().as_secs_f32().min(COUNT_ON),
                };
                let news = view.as_ref().map(|v| strip_news(v, now));
                if news != said {
                    if news.is_some() {
                        until = Instant::now() + FLASH;
                    }
                    said = news;
                }
                let strip_y = ocr::minimap::top_frac(sh.minimap_settings().scale);
                let mut scenes = scenes(client, view.as_ref(), now, strip_y);
                if Instant::now() >= until {
                    scenes[0] = None;
                }
                #[cfg(windows)]
                {
                    for (w, s) in wins.iter_mut().zip(&scenes) {
                        w.show(&painter, s.as_ref());
                    }
                    native::pump();
                    native::wait(TRACK_EVERY.as_millis() as u32);
                }
                #[cfg(not(windows))]
                {
                    let _ = scenes;
                    std::thread::sleep(TRACK_EVERY);
                }
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

/// [strip, badges] over the game's client area at game time `now`, the LoL strip's bottom at
/// `strip_y` (fraction of the height); None hides one.
fn scenes(client: Option<Rect>, view: Option<&Overlay>, now: f32, strip_y: f32) -> [Option<Scene>; 2] {
    let (Some(r), Some(v)) = (client, view) else { return [None, None] };
    let at = |x: f32, y: f32| (r.x + (r.w as f32 * x).round() as i32, r.y + (r.h as f32 * y).round() as i32);
    let unit = r.h as f32 / 100.0;
    let mut lines = vec![];
    if let Some(head) = &v.head {
        let mut runs = vec![run(head, Ink::Gold)];
        runs.extend(v.detail.as_ref().map(|d| run(format!("   {d}"), Ink::Muted)));
        lines.push(Line { runs, bar: false });
    }
    if let Some(call) = &v.call {
        lines.push(Line { runs: vec![run(call, Ink::Text)], bar: true });
    }
    if !v.timers.is_empty() {
        let mut runs = vec![];
        for (i, t) in v.timers.iter().enumerate() {
            let left = t.next_spawn.map_or(0.0, |s| s - now);
            let (value, ink) = match left {
                l if l <= 0.0 => ("up".to_string(), Ink::Teal),
                l if l <= 60.0 => (clock_text(l), Ink::Gold),
                l => (clock_text(l), Ink::Text),
            };
            runs.push(run(format!("{}{} ", if i > 0 { "    " } else { "" }, t.name), Ink::Grey));
            runs.push(run(value, ink));
        }
        lines.push(Line { runs, bar: false });
    }
    let max_w = (r.w as f32 * STRIP_W).round() as i32;
    let strip = (!lines.is_empty()).then(|| match v.tft {
        true => Scene::Strip { lines, at: at(0.01, 0.09), bottom_right: false, max_w, unit },
        false => Scene::Strip { lines, at: at(0.994, strip_y), bottom_right: true, max_w, unit },
    });
    let badges = (!v.badges.is_empty()).then(|| Scene::Badges {
        badges: v
            .badges
            .iter()
            .map(|b| {
                let top = b.place == Some(0);
                let place = b.place.and_then(|p| ORD.get(p as usize)).copied().unwrap_or("–");
                let mut runs = vec![Run { text: place.into(), ink: if top { Ink::Gold } else { Ink::Grey }, big: true }];
                runs.extend(b.reason.as_ref().map(|t| run(format!("  {t}"), Ink::Muted)));
                (at(b.x, 0.0).0, Line { runs, bar: false }, top)
            })
            .collect(),
        y: at(0.0, v.badge_y).1,
        max_w: (r.w as f32 * BADGE_W).round() as i32,
        unit,
    });
    [strip, badges]
}

/// What the strip says that's worth flashing it for: Buy next, the call, and which timers are
/// close or up. Not the counting parts (gold short, the clocks), or it would never hide.
fn strip_news(v: &Overlay, now: f32) -> (Option<String>, Option<String>, Vec<(String, bool)>) {
    let timers = v
        .timers
        .iter()
        .filter_map(|t| {
            let left = t.next_spawn.map_or(0.0, |s| s - now);
            (left <= TIMER_SOON).then(|| (t.name.clone(), left <= 0.0))
        })
        .collect();
    (v.head.clone(), v.call.clone(), timers)
}

/// m:ss, as the dashboard's clockText.
fn clock_text(s: f32) -> String {
    let t = s.max(0.0) as u32;
    format!("{}:{:02}", t / 60, t % 60)
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
        .map(|(x, (place, reason))| OverlayBadge { x, place, reason })
        .collect()
}

/// LoL: the advice's offer, while those same cards are on screen, each badge over its title's box
/// (the default card centre for a title without one).
fn lol_badges(advice: Option<&AugmentAdvice>, cards: &[Card]) -> Vec<OverlayBadge> {
    let Some(a) = advice else { return vec![] };
    if !a.offered.iter().map(|r| r.id).eq(cards.iter().map(|c| c.id)) {
        return vec![];
    }
    let places = coach::offer_places(&a.offered);
    let mut out = badges(a.offered.iter().zip(places).map(|(r, p)| (p, lol_reason(r))).collect(), LOL_PITCH);
    for (b, c) in out.iter_mut().zip(cards) {
        b.x = c.x;
    }
    out
}

/// The badge row's top: as far above the titles as on the default layout, scaled with the cards.
fn lol_badge_y(cards: &[Card]) -> f32 {
    match cards.first() {
        Some(c) => cards.iter().map(|c| c.y).fold(f32::MAX, f32::min) - (ocr::offer::TITLE_TOP - LOL_BADGE_Y) * c.found.scale,
        None => LOL_BADGE_Y,
    }
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

fn lol_view(g: &GameAdvice, cards: &[Card]) -> Overlay {
    let buy = g.buy_next.as_ref();
    Overlay {
        tft: false,
        badge_y: lol_badge_y(cards),
        badges: lol_badges(g.augments.as_ref(), cards),
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
    fn scenes_only_for_what_is_drawn() {
        let view = |tft: bool, badges: usize, timers: bool| Overlay {
            tft,
            badge_y: LOL_BADGE_Y,
            badges: (0..badges).map(|i| OverlayBadge { x: 0.3 + 0.2 * i as f32, place: Some(i as u32), reason: None }).collect(),
            head: None,
            detail: None,
            call: None,
            time: 0.0,
            timers: if timers { vec![ObjectiveTimer { name: "Dragon".into(), next_spawn: Some(300.0) }] } else { vec![] },
        };
        // LoL strip above the minimap, its corner 0.6 % in; nothing to draw, nothing shown.
        let [strip, badges] = scenes(Some(R), Some(&view(false, 0, true)), 250.0, 0.73);
        let Some(Scene::Strip { lines, at, bottom_right, max_w, unit }) = strip else { panic!("{strip:?}") };
        assert_eq!((at, bottom_right, max_w, unit), ((2545, 1051), true, 563, 14.4));
        let text: Vec<(&str, Ink)> = lines[0].runs.iter().map(|r| (r.text.as_str(), r.ink)).collect();
        assert_eq!(text, [("Dragon ", Ink::Grey), ("0:50", Ink::Gold)]);
        assert_eq!(badges, None);
        // A bigger minimap moves the strip up with its top edge.
        let Some(Scene::Strip { at, .. }) = scenes(Some(R), Some(&view(false, 0, true)), 250.0, 0.65)[0].clone() else { panic!() };
        assert_eq!(at, (2545, 936));
        assert_eq!(scenes(Some(R), Some(&view(false, 0, false)), 0.0, 0.73), [None, None]);
        assert_eq!(scenes(None, Some(&view(false, 3, true)), 0.0, 0.73), [None, None]);
        // Cards up: a badge on each, the top pick gold; TFT strip top left.
        let [strip, badges] = scenes(Some(R), Some(&view(true, 3, true)), 400.0, 0.73);
        assert!(matches!(strip, Some(Scene::Strip { at: (26, 130), bottom_right: false, .. })), "{strip:?}");
        let Some(Scene::Badges { badges, y, .. }) = badges else { panic!() };
        assert_eq!(y, 475);
        assert_eq!(badges.iter().map(|b| (b.0, b.1.runs[0].text.as_str(), b.2)).collect::<Vec<_>>(), [(768, "1st", true), (1280, "2nd", false), (1792, "3rd", false)]);
    }

    #[test]
    fn the_strip_flashes_for_news_not_for_counting() {
        let v = |head: &str, detail: &str, spawn: f32| Overlay {
            tft: false,
            badge_y: LOL_BADGE_Y,
            badges: vec![],
            head: Some(head.into()),
            detail: Some(detail.into()),
            call: None,
            time: 0.0,
            timers: vec![ObjectiveTimer { name: "Dragon".into(), next_spawn: Some(spawn) }],
        };
        let base = strip_news(&v("Buy Sunfire", "450 g short", 300.0), 100.0);
        // Gold and clocks counting: no news.
        assert_eq!(strip_news(&v("Buy Sunfire", "300 g short", 300.0), 150.0), base);
        // A new item, a timer within a minute, then up: news each time.
        assert_ne!(strip_news(&v("Buy Thornmail", "450 g short", 300.0), 100.0), base);
        let soon = strip_news(&v("Buy Sunfire", "450 g short", 300.0), 250.0);
        assert_ne!(soon, base);
        assert_eq!(strip_news(&v("Buy Sunfire", "450 g short", 300.0), 260.0), soon);
        assert_ne!(strip_news(&v("Buy Sunfire", "450 g short", 300.0), 300.0), soon);
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

    /// Cards read at the default centres (or `xs`), title tops at `y`, at `scale`.
    fn cards(ids: &[AugmentId], xs: Option<&[f32]>, y: f32, scale: f32) -> Vec<Card> {
        let def = card_centres(ids.len(), LOL_PITCH);
        ids.iter()
            .enumerate()
            .map(|(i, &id)| {
                let x = xs.map_or(def[i], |x| x[i]);
                let rect = Rect { x: (x * 2560.0) as i32 - 100, y: (y * 1440.0) as i32, w: 200, h: 30 };
                Card { id, x, y, found: ocr::Found { rect, scale, score: 1.0 } }
            })
            .collect()
    }

    #[test]
    fn lol_badges_follow_screen_order_while_the_cards_show() {
        let advice = AugmentAdvice {
            offered: vec![lol_rank(7, Some(9), None), lol_rank(8, None, None), lol_rank(9, Some(2), Some("fits your Rabadon's Deathcap (AP)"))],
            best: Some(9),
            reason: String::new(),
        };
        let at = |ids: &[AugmentId]| cards(ids, None, ocr::offer::TITLE_TOP, 1.0);
        let b = lol_badges(Some(&advice), &at(&[7, 8, 9]));
        assert_eq!(b.iter().map(|b| b.place).collect::<Vec<_>>(), [Some(1), None, Some(0)]);
        assert_eq!(b[0].reason.as_deref(), Some("#10 of 20 Gold"));
        assert_eq!(b[1].reason, None);
        assert_eq!(b[2].reason.as_deref(), Some("fits your Rabadon's Deathcap (AP)"));
        assert!(b[0].x < b[1].x && b[1].x < b[2].x);
        assert!((b[1].x - 0.5).abs() < 1e-6);
        // Cards gone (shop open), replaced, or a title unread: no badges.
        assert!(lol_badges(Some(&advice), &[]).is_empty());
        assert!(lol_badges(Some(&advice), &at(&[7, 8, 10])).is_empty());
        let two = AugmentAdvice { offered: advice.offered[..2].to_vec(), ..advice.clone() };
        assert!(lol_badges(Some(&two), &at(&[7, 8])).is_empty());
        assert!(lol_badges(None, &at(&[7, 8, 9])).is_empty());
    }

    #[test]
    fn lol_badges_sit_over_the_title_boxes() {
        let advice = AugmentAdvice { offered: vec![lol_rank(7, Some(0), None), lol_rank(8, Some(1), None), lol_rank(9, Some(2), None)], best: Some(7), reason: String::new() };
        // A 1.2x layout, its titles lower: badges over the read centres, the row above the titles.
        let xs = [0.5 - 1.2 * LOL_PITCH, 0.5, 0.5 + 1.2 * LOL_PITCH];
        let read = cards(&[7, 8, 9], Some(&xs), 0.45, 1.2);
        let b = lol_badges(Some(&advice), &read);
        for (b, x) in b.iter().zip(xs) {
            assert!((b.x - x).abs() < 0.01, "{b:?}");
        }
        let y = lol_badge_y(&read);
        assert!((y - (0.45 - 1.2 * (ocr::offer::TITLE_TOP - LOL_BADGE_Y))).abs() < 1e-5 && y < 0.45);
        // The default layout keeps the hand-placed row; no cards, the constant.
        assert!((lol_badge_y(&cards(&[7, 8, 9], None, ocr::offer::TITLE_TOP, 1.0)) - LOL_BADGE_Y).abs() < 1e-6);
        assert_eq!(lol_badge_y(&[]), LOL_BADGE_Y);
        // On screen, the badges land within 1 % of the window width of the card centres.
        let v = Overlay { tft: false, badge_y: y, badges: b, head: None, detail: None, call: None, time: 0.0, timers: vec![] };
        let [_, badges] = scenes(Some(R), Some(&v), 0.0, 0.73);
        let Some(Scene::Badges { badges, y: top, .. }) = badges else { panic!() };
        for ((x, ..), want) in badges.iter().zip(xs) {
            assert!(((*x as f32 / 2560.0) - want).abs() < 0.01, "{x} vs {want}");
        }
        assert_eq!(top, (y * 1440.0).round() as i32);
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
