//! GDI capture of the League client area into an offscreen DIB section.

use crate::Frame;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetClientRect, GetForegroundWindow, GetWindowTextW, IsIconic};
use windows::core::{PCWSTR, w};

/// Fractions of the client area holding the three augment card *titles* (and the
/// category tag under them). Measured on real 1440p Mayhem offers, normal and
/// prismatic: cards span x 22–78 %, titles sit at y ≈ 41 %, descriptions start at
/// ≈ 47 %. Only this strip is read, so chat, kill feed and pings (which often
/// name augments) can't produce false offers.
const REGION_X: (f32, f32) = (0.18, 0.82);
const REGION_Y: (f32, f32) = (0.365, 0.462);

/// Top-down 32-bit DIB section selected into its own memory DC.
pub(crate) struct Dib {
    pub dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    pub width: i32,
    pub height: i32,
}

impl Dib {
    pub fn new(width: i32, height: i32) -> Option<Self> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return None;
            }
            let mut bits = std::ptr::null_mut();
            let Ok(bmp) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            else {
                let _ = DeleteDC(dc);
                return None;
            };
            let old = SelectObject(dc, bmp.into());
            Some(Self {
                dc,
                bmp,
                old,
                bits: bits.cast(),
                width,
                height,
            })
        }
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        unsafe {
            let _ = GdiFlush();
            std::slice::from_raw_parts_mut(self.bits, (self.width * self.height * 4) as usize)
        }
    }

    pub fn snapshot(&mut self) -> Frame {
        let (width, height) = (self.width as u32, self.height as u32);
        Frame {
            width,
            height,
            bgra: self.pixels_mut().to_vec(),
        }
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bmp.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

pub(crate) fn league_window() -> Option<HWND> {
    unsafe {
        FindWindowW(w!("RiotWindowClass"), PCWSTR::null())
            .or_else(|_| FindWindowW(PCWSTR::null(), w!("League of Legends (TM) Client")))
            // TFT runs in its own Unreal client; its title is "TFT" plus trailing spaces.
            .or_else(|_| {
                let h = FindWindowW(w!("UnrealWindow"), PCWSTR::null())?;
                let mut t = [0u16; 16];
                let n = GetWindowTextW(h, &mut t) as usize;
                if String::from_utf16_lossy(&t[..n]).trim() == "TFT" { Ok(h) } else { Err(windows::core::Error::empty()) }
            })
            .ok()
            .filter(|h| !h.is_invalid() && !IsIconic(*h).as_bool())
    }
}

/// The game window is the foreground window (the player is in it, not alt-tabbed).
pub fn league_foreground() -> bool {
    league_window().is_some_and(|h| unsafe { GetForegroundWindow() } == h)
}

pub fn capture_league() -> Option<Frame> {
    let hwnd = league_window()?;
    let mut rc = RECT::default();
    let mut origin = POINT::default();
    unsafe {
        GetClientRect(hwnd, &mut rc).ok()?;
        if !ClientToScreen(hwnd, &mut origin).as_bool() {
            return None;
        }
    }
    let (cw, ch) = (rc.right as f32, rc.bottom as f32);
    let x = origin.x + (cw * REGION_X.0) as i32;
    let y = origin.y + (ch * REGION_Y.0) as i32;
    let w = (cw * (REGION_X.1 - REGION_X.0)) as i32;
    let h = (ch * (REGION_Y.1 - REGION_Y.0)) as i32;
    grab_screen(x, y, w, h)
}

/// BitBlt a rectangle of the screen (virtual-screen coordinates).
pub(crate) fn grab_screen(x: i32, y: i32, w: i32, h: i32) -> Option<Frame> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let mut dib = Dib::new(w, h)?;
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let ok = BitBlt(dib.dc, 0, 0, w, h, Some(screen), x, y, SRCCOPY).is_ok();
        ReleaseDC(None, screen);
        ok.then(|| dib.snapshot())
    }
}
