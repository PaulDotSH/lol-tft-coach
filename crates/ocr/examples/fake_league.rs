//! Stand-in League window (class `RiotWindowClass`) showing augment card names,
//! for testing the OCR path end to end. Topmost, never activated (no focus).
//! `cargo run -p ocr --example fake_league -- "Big Brain" "Warlock Juicebox" "Quickstep"`

#[cfg(windows)]
fn main() {
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::{HSTRING, w};

    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES.set(std::env::args().skip(1).collect()).unwrap();

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe {
            if msg == WM_PAINT {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                FillRect(dc, &rc, CreateSolidBrush(COLORREF(0x2a1a10)));
                let font = CreateFontW(44, 0, 0, 0, 700, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 0, w!("Segoe UI"));
                SelectObject(dc, font.into());
                SetBkMode(dc, TRANSPARENT);
                SetTextColor(dc, COLORREF(0x8fd7f0));
                let names = NAMES.get().unwrap();
                let w = rc.right / names.len().max(1) as i32;
                for (i, n) in names.iter().enumerate() {
                    let mut r = RECT { left: i as i32 * w + 20, top: rc.bottom * 2 / 5, right: (i as i32 + 1) * w - 20, bottom: rc.bottom * 3 / 5 };
                    let mut text: Vec<u16> = n.encode_utf16().collect();
                    DrawTextW(dc, &mut text, &mut r, DT_CENTER | DT_WORDBREAK);
                }
                let _ = EndPaint(hwnd, &ps);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }

    unsafe {
        let class = w!("RiotWindowClass");
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(proc), lpszClassName: class, ..Default::default() });
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class,
            &HSTRING::from("League of Legends (TM) Client"),
            WS_POPUP,
            1100,
            250,
            1400,
            800,
            None,
            None,
            None,
            None,
        )
        .expect("window");
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let secs: u64 = std::env::var("FAKE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(20);
        let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        let mut msg = MSG::default();
        while std::time::Instant::now() < end {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                DispatchMessageW(&msg);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = DestroyWindow(hwnd);
    }
}

#[cfg(not(windows))]
fn main() {}
