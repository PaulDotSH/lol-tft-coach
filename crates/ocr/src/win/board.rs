//! Client-area captures of the League window for the Tab scoreboard.

use super::capture::{grab_screen, league_window};
use crate::Frame;
use crate::scoreboard::Rect;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

pub fn client_size() -> Option<(u32, u32)> {
    let mut rc = RECT::default();
    unsafe { GetClientRect(league_window()?, &mut rc).ok()? };
    (rc.right > 0 && rc.bottom > 0).then_some((rc.right as u32, rc.bottom as u32))
}

pub fn capture_client(r: Rect) -> Option<Frame> {
    let hwnd = league_window()?;
    let mut origin = POINT::default();
    if !unsafe { ClientToScreen(hwnd, &mut origin) }.as_bool() {
        return None;
    }
    grab_screen(origin.x + r.x, origin.y + r.y, r.w, r.h)
}
