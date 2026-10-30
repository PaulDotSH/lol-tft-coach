//! The overlay's windows: plain layered popups, drawn on the CPU (Direct2D software target,
//! DirectWrite text) into a bitmap that UpdateLayeredWindow hands to Windows. No WebView and
//! nothing presenting frames: Windows keeps the last bitmap and only takes a new one when the
//! text changes (the timers, once a second). The WebView windows this replaced cost the game
//! FPS the whole time they were up.

use super::{Ink, Line, Scene};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT};
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

/// Never activated, clicks fall through, no taskbar / Alt-Tab entry, above the game.
const EX: WINDOW_EX_STYLE = WINDOW_EX_STYLE(WS_EX_NOACTIVATE.0 | WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0);

fn rgba(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a }
}

/// The dashboard's palette.
fn ink(i: Ink) -> D2D1_COLOR_F {
    match i {
        Ink::Text => rgba(0xeb, 0xe6, 0xd9, 1.0),
        Ink::Gold => rgba(0xdc, 0xb7, 0x5c, 1.0),
        Ink::Muted => rgba(0xc9, 0xc3, 0xb4, 1.0),
        Ink::Grey => rgba(0x8f, 0x9b, 0xab, 1.0),
        Ink::Teal => rgba(0x5f, 0xc4, 0xa9, 1.0),
    }
}
const INKS: [Ink; 5] = [Ink::Text, Ink::Gold, Ink::Muted, Ink::Grey, Ink::Teal];

unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => unsafe { DefWindowProcW(h, msg, w, l) },
    }
}

/// Handles messages for this thread's windows; between calls they need none answered.
pub fn pump() {
    let mut msg = MSG::default();
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
}

/// Waits up to `ms`, waking early for this thread's window messages.
pub fn wait(ms: u32) {
    unsafe {
        let _ = MsgWaitForMultipleObjects(None, false, ms, QS_ALLINPUT);
    }
}

/// Drawing state shared by the windows: one CPU render target and its brushes.
pub struct Painter {
    dw: IDWriteFactory,
    rt: ID2D1DCRenderTarget,
    brushes: Vec<ID2D1SolidColorBrush>,
    bg: ID2D1SolidColorBrush,
    edge: ID2D1SolidColorBrush,
    /// The text format and its ellipsis for the last font size (it changes only with the window).
    format: std::cell::RefCell<Option<(f32, IDWriteTextFormat, IDWriteInlineObject)>>,
}

impl Painter {
    pub fn new() -> Result<Self> {
        unsafe {
            let d2: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let rt = d2.CreateDCRenderTarget(&props)?;
            rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let brush = |c: D2D1_COLOR_F| rt.CreateSolidColorBrush(&c, None);
            let brushes = INKS.iter().map(|&i| brush(ink(i))).collect::<Result<Vec<_>>>()?;
            let bg = brush(rgba(12, 16, 22, 0.78))?;
            let edge = brush(rgba(0xdc, 0xb7, 0x5c, 0.35))?;
            let dw = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            Ok(Self { dw, rt, brushes, bg, edge, format: Default::default() })
        }
    }

    fn brush(&self, i: Ink) -> &ID2D1SolidColorBrush {
        &self.brushes[INKS.iter().position(|&x| x == i).unwrap_or(0)]
    }

    /// The text format at `px` and its ellipsis sign, made once per size.
    fn format(&self, px: f32) -> Result<(IDWriteTextFormat, IDWriteInlineObject)> {
        let mut cached = self.format.borrow_mut();
        if let Some((size, f, sign)) = &*cached
            && *size == px
        {
            return Ok((f.clone(), sign.clone()));
        }
        unsafe {
            let f = self.dw.CreateTextFormat(
                w!("Bahnschrift"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_CONDENSED,
                px,
                w!("en-us"),
            )?;
            let sign = self.dw.CreateEllipsisTrimmingSign(&f)?;
            *cached = Some((px, f.clone(), sign.clone()));
            Ok((f, sign))
        }
    }

    /// A laid-out line: its runs coloured and sized, wrapped at `max_w` (or cut with "…").
    fn layout(&self, line: &Line, px: f32, max_w: f32, wrap: bool) -> Result<(IDWriteTextLayout, f32, f32)> {
        unsafe {
            let (format, sign) = self.format(px)?;
            let text: Vec<u16> = line.runs.iter().flat_map(|r| r.text.encode_utf16()).collect();
            let layout = self.dw.CreateTextLayout(&text, &format, max_w.max(1.0), 10_000.0)?;
            if !wrap {
                layout.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                let trim = DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, ..Default::default() };
                layout.SetTrimming(&trim, &sign)?;
            }
            let mut start = 0u32;
            for r in &line.runs {
                let range = DWRITE_TEXT_RANGE { startPosition: start, length: r.text.encode_utf16().count() as u32 };
                layout.SetDrawingEffect(self.brush(r.ink), range)?;
                if r.big {
                    layout.SetFontSize(px * 1.5, range)?;
                    layout.SetFontWeight(DWRITE_FONT_WEIGHT_BOLD, range)?;
                }
                start += range.length;
            }
            let mut m = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut m)?;
            Ok((layout, m.width.min(max_w), m.height))
        }
    }

    /// Boxes in window coordinates, the window's top-left on screen and its size.
    fn arrange(&self, scene: &Scene) -> Result<(Vec<Boxed>, POINT, SIZE)> {
        let (unit, max_w) = match scene {
            Scene::Strip { unit, max_w, .. } | Scene::Badges { unit, max_w, .. } => (*unit, *max_w as f32),
        };
        let px = (unit * 1.35).max(12.0);
        let (pad_x, pad_y, gap) = (unit, unit * 0.5, unit * 0.3);
        let (bar_w, bar_gap) = (unit * 0.3, unit * 0.6);
        let mut boxes = vec![];
        match scene {
            Scene::Strip { lines, at, bottom_right, .. } => {
                let mut texts = vec![];
                let (mut w, mut y) = (0f32, pad_y);
                for line in lines {
                    let indent = if line.bar { bar_w + bar_gap } else { 0.0 };
                    let (layout, lw, lh) = self.layout(line, px, max_w - 2.0 * pad_x - indent, true)?;
                    let bar = line.bar.then_some(D2D_RECT_F { left: pad_x, top: y, right: pad_x + bar_w, bottom: y + lh });
                    texts.push(Text { layout, x: pad_x + indent, y, bar });
                    w = w.max(lw + indent);
                    y += lh + gap;
                }
                let size = SIZE { cx: (w + 2.0 * pad_x).ceil() as i32 + 2, cy: (y - gap + pad_y).ceil() as i32 + 2 };
                let origin = match bottom_right {
                    true => POINT { x: at.0 - size.cx, y: at.1 - size.cy },
                    false => POINT { x: at.0, y: at.1 },
                };
                boxes.push(Boxed { rect: frame(0.0, 0.0, size.cx as f32, size.cy as f32), gold: false, texts });
                Ok((boxes, origin, size))
            }
            Scene::Badges { badges, y, .. } => {
                let mut placed = vec![];
                for (x, line, top) in badges {
                    let pad_x = unit * 0.9;
                    let (layout, lw, lh) = self.layout(line, px, max_w - 2.0 * pad_x, false)?;
                    let (bw, bh) = (lw + 2.0 * pad_x + 2.0, lh + unit * 0.6 + 2.0);
                    placed.push((*x as f32 - bw / 2.0, bw, bh, layout, *top, pad_x));
                }
                let left = placed.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor();
                let right = placed.iter().map(|p| p.0 + p.1).fold(f32::MIN, f32::max).ceil();
                let height = placed.iter().map(|p| p.2).fold(0.0, f32::max).ceil();
                for (x, bw, bh, layout, gold, pad_x) in placed {
                    let x = x - left;
                    let texts = vec![Text { layout, x: x + pad_x + 1.0, y: unit * 0.3 + 1.0, bar: None }];
                    boxes.push(Boxed { rect: frame(x, 0.0, bw, bh), gold, texts });
                }
                Ok((boxes, POINT { x: left as i32, y: *y }, SIZE { cx: (right - left) as i32, cy: height as i32 }))
            }
        }
    }

    fn paint(&self, dc: HDC, size: SIZE, boxes: &[Boxed], unit: f32) -> Result<()> {
        unsafe {
            let all = windows::Win32::Foundation::RECT { left: 0, top: 0, right: size.cx, bottom: size.cy };
            self.rt.BindDC(dc, &all)?;
            self.rt.BeginDraw();
            self.rt.Clear(Some(&rgba(0, 0, 0, 0.0)));
            let gold = self.brush(Ink::Gold);
            for b in boxes {
                let rr = D2D1_ROUNDED_RECT { rect: b.rect, radiusX: unit * 0.4, radiusY: unit * 0.4 };
                self.rt.FillRoundedRectangle(&rr, &self.bg);
                let edge: &ID2D1Brush = if b.gold { gold } else { &self.edge };
                self.rt.DrawRoundedRectangle(&rr, edge, 1.0, None);
                for t in &b.texts {
                    if let Some(bar) = &t.bar {
                        self.rt.FillRectangle(bar, gold);
                    }
                    let at = windows_numerics::Vector2 { X: t.x, Y: t.y };
                    self.rt.DrawTextLayout(at, &t.layout, self.brush(Ink::Text), D2D1_DRAW_TEXT_OPTIONS_NONE);
                }
            }
            self.rt.EndDraw(None, None)
        }
    }
}

/// A box outline inset half a pixel, so its 1 px edge lands on whole pixels.
fn frame(x: f32, y: f32, w: f32, h: f32) -> D2D_RECT_F {
    D2D_RECT_F { left: x + 0.5, top: y + 0.5, right: x + w - 0.5, bottom: y + h - 0.5 }
}

struct Text {
    layout: IDWriteTextLayout,
    x: f32,
    y: f32,
    bar: Option<D2D_RECT_F>,
}

struct Boxed {
    rect: D2D_RECT_F,
    gold: bool,
    texts: Vec<Text>,
}

/// One overlay window; redrawn only when its scene changes.
pub struct Window {
    hwnd: HWND,
    shown: Option<Scene>,
}

impl Window {
    /// Hidden. Created on the thread that later draws it and pumps its messages.
    pub fn new() -> Result<Self> {
        unsafe {
            let inst = GetModuleHandleW(None)?;
            let class = w!("LoLCoachOverlay");
            let wc = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: inst.into(), lpszClassName: class, ..Default::default() };
            RegisterClassW(&wc); // 0 once registered: the second window reuses it
            let hwnd = CreateWindowExW(EX, class, w!("LoL Coach overlay"), WS_POPUP, 0, 0, 0, 0, None, None, Some(inst.into()), None)?;
            Ok(Self { hwnd, shown: None })
        }
    }

    /// Draw `scene` and show without activating, or hide (None).
    pub fn show(&mut self, p: &Painter, scene: Option<&Scene>) {
        if self.shown.as_ref() == scene {
            return;
        }
        let Some(scene) = scene else {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            self.shown = None;
            return;
        };
        match self.draw(p, scene) {
            Ok(()) => self.shown = Some(scene.clone()),
            Err(e) => tracing::warn!("overlay draw: {e}"),
        }
    }

    fn draw(&self, p: &Painter, scene: &Scene) -> Result<()> {
        let (boxes, origin, size) = p.arrange(scene)?;
        if size.cx <= 0 || size.cy <= 0 {
            return Ok(());
        }
        let unit = match scene {
            Scene::Strip { unit, .. } | Scene::Badges { unit, .. } => *unit,
        };
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size.cx,
                    biHeight: -size.cy,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let result = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).and_then(|bmp| {
                let old = SelectObject(dc, bmp.into());
                let r = p.paint(dc, size, &boxes, unit).and_then(|()| {
                    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
                    UpdateLayeredWindow(self.hwnd, Some(screen), Some(&origin), Some(&size), Some(dc), Some(&POINT::default()), COLORREF(0), Some(&blend), ULW_ALPHA)
                });
                SelectObject(dc, old);
                let _ = DeleteObject(bmp.into());
                r
            });
            let _ = DeleteDC(dc);
            ReleaseDC(None, screen);
            result?;
            if !IsWindowVisible(self.hwnd).as_bool() {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
            Ok(())
        }
    }
}
