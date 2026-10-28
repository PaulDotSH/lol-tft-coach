//! Coachless sign-in: a window on the Coachless login page. The token pair is caught from
//! the site's own login reply (WebView2's response event), so the page gets no access to the
//! app (the window has no capabilities).

use crate::state::Shared;
use std::sync::Arc;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "coachless";

/// Async: a window built inside a sync command (main thread) deadlocks WebView2 on Windows
/// and stays white.
#[tauri::command]
pub async fn coachless_sign_in(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return w.set_focus().map_err(|e| e.to_string());
    }
    let url = data::coachless::LOGIN_URL.parse().map_err(|e| format!("{e}"))?;
    let w = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(url))
        .title("Sign in to Coachless")
        .inner_size(520.0, 760.0)
        .build()
        .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    w.with_webview(move |wv| {
        if let Err(e) = watch(&wv.controller(), app) {
            tracing::warn!("coachless sign-in window: {e}");
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn coachless_sign_out(sh: tauri::State<'_, Arc<Shared>>) {
    set(&sh, None);
}

fn set(sh: &Shared, t: Option<data::coachless::Tokens>) {
    sh.coachless_auth.set(t);
    sh.coachless.lock().unwrap().clear();
    *sh.coachless_ok.lock().unwrap() = None;
}

#[cfg(windows)]
fn watch(
    ctl: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    app: AppHandle,
) -> windows::core::Result<()> {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_2;
    use webview2_com::{WebResourceResponseReceivedEventHandler, WebResourceResponseViewGetContentCompletedHandler};
    use windows::core::{Interface, PWSTR};

    let wv: ICoreWebView2_2 = unsafe { ctl.CoreWebView2()? }.cast()?;
    let handler = WebResourceResponseReceivedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut uri = PWSTR::null();
        unsafe { args.Request()?.Uri(&mut uri)? };
        if !webview2_com::take_pwstr(uri).contains(data::coachless::LOGIN_PATH) {
            return Ok(());
        }
        let app = app.clone();
        let got = WebResourceResponseViewGetContentCompletedHandler::create(Box::new(move |hr, body| {
            hr?;
            if let Some(t) = body.and_then(|s| read_all(&s)).and_then(|b| data::coachless::parse_tokens(&b)) {
                tracing::info!("signed in to Coachless");
                set(&app.state::<Arc<Shared>>(), Some(t));
                if let Some(w) = app.get_webview_window(LABEL) {
                    let _ = w.close();
                }
            }
            Ok(())
        }));
        unsafe { args.Response()?.GetContent(&got) }
    }));
    let mut token = 0;
    unsafe { wv.add_WebResourceResponseReceived(&handler, &mut token) }
}

#[cfg(windows)]
fn read_all(s: &windows::Win32::System::Com::IStream) -> Option<String> {
    let (mut out, mut buf) = (Vec::new(), [0u8; 8192]);
    loop {
        let mut n = 0u32;
        let hr = unsafe { s.Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut n)) };
        if hr.is_err() || n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n as usize]);
    }
    String::from_utf8(out).ok()
}
