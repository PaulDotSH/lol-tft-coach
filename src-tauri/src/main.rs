#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod draft;
mod macro_sync;
mod ingame;
mod orchestrator;
mod settings;
mod state;

use state::Shared;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

fn main() {
    init_logging();
    let http = data::Http::new(settings::local_dir("cache")).expect("http client");
    let shared = Arc::new(Shared::new(http, settings::load()));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_static,
            commands::set_settings,
            commands::lookup
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

            orchestrator::spawn_ocr(shared.clone());
            tauri::async_runtime::spawn(orchestrator::run(app.handle().clone(), shared.clone()));
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

fn init_logging() {
    let file = std::fs::File::create(settings::local_dir("logs").join("lol-coach.log"));
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    match file {
        Ok(f) => tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_writer(Mutex::new(f)).init(),
        Err(_) => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
}
