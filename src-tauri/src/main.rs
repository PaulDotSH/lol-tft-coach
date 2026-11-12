#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account;
mod builds;
mod calib;
mod coachless;
mod commands;
mod debug;
mod draft;
mod health;
mod macro_sync;
mod ingame;
mod orchestrator;
mod overlay;
mod ranked;
mod remote;
mod selfcoach;
mod settings;
mod state;
mod tab;
mod tft;
mod winprob;

use state::Shared;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

fn main() {
    init_logging();
    let http = data::Http::new(settings::local_dir("cache")).expect("http client");
    let shared = Arc::new(Shared::new(http, settings::load()));
    let tft_shared = Arc::new(tft::TftShared::default());

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        // Size and position only: a window closed to the tray must not come back hidden.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::all() - tauri_plugin_window_state::StateFlags::VISIBLE)
                .build(),
        )
        .manage(shared.clone())
        .manage(tft_shared.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_static,
            commands::set_settings,
            commands::lookup,
            commands::analyze_game,
            commands::log_frontend,
            coachless::coachless_sign_in,
            coachless::coachless_sign_out,
            tft::get_tft,
            tft::get_tft_static
        ])
        .setup(move |app| {
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().expect("bundle icon"))
                .tooltip("LoL Coach")
                .menu(&Menu::with_items(app, &[&show_i, &quit_i])?)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "show" => show(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        show(tray.app_handle());
                    }
                })
                .build(app)?;

            overlay::spawn(shared.clone(), tft_shared.clone());
            orchestrator::spawn_ocr(shared.clone());
            orchestrator::spawn_minimap(app.handle().clone(), shared.clone());
            tauri::async_runtime::spawn(orchestrator::run(app.handle().clone(), shared.clone()));
            tauri::async_runtime::spawn(tft::run(app.handle().clone(), shared.clone(), tft_shared.clone()));
            tauri::async_runtime::spawn(winprob::run(shared.clone()));
            tauri::async_runtime::spawn(builds::run(shared.clone()));
            tauri::async_runtime::spawn(remote::run(shared.clone()));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing hides to tray; quit from the tray menu.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running LoL Coach");
}

/// Only on explicit user action (tray click, second launch).
fn show(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// `logs\lol-coach.log` (overwritten each run); in debug mode also a copy in the
/// session folder, debug level for our crates, threads and source lines.
fn init_logging() {
    use tracing_subscriber::fmt::writer::MakeWriterExt;
    let debug_dir = debug::init();
    let level = if debug_dir.is_some() {
        "info,lol_coach=debug,coach=debug,data=debug,clients=debug,ocr=debug,web=debug"
    } else {
        "info"
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| level.into());
    let main = std::fs::File::create(settings::local_dir("logs").join("lol-coach.log"));
    let session = debug_dir.and_then(|d| std::fs::File::create(d.join("lol-coach.log")).ok());
    let fmt = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_thread_names(debug_dir.is_some())
        .with_line_number(debug_dir.is_some());
    match (main, session) {
        (Ok(m), Some(s)) => fmt.with_writer(Mutex::new(m).and(Mutex::new(s))).init(),
        (Ok(m), None) => fmt.with_writer(Mutex::new(m)).init(),
        (Err(_), _) => fmt.init(),
    }

    let debug = debug_dir.is_some();
    std::panic::set_hook(Box::new(move |info| {
        let bt = if debug { std::backtrace::Backtrace::force_capture().to_string() } else { String::new() };
        tracing::error!("panic: {info}\n{bt}");
    }));
    match debug_dir {
        Some(d) => tracing::info!("LoL Coach {} — debug mode, dumps in {}", env!("CARGO_PKG_VERSION"), d.display()),
        None => tracing::info!("LoL Coach {}", env!("CARGO_PKG_VERSION")),
    }
}
