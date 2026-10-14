// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    Manager,
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
};
use std::panic::AssertUnwindSafe;
use std::time::Duration;
use dot::{AppState, commands};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // A menu bar app: no Dock icon. LSUIElement in Info.plist is overridden when the event loop
            // starts (tao sets the Regular policy), so the policy is set here as well
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Initialize app state
            let app_state = AppState::new();
            app.manage(app_state);

            // The pet, the companion's day and passive income move on here, shown or hidden.
            // With SYNC on, the sync folder is read and written here too
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("dot-tick".into())
                .spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(commands::SIGNAL_TICK_SECS));
                    // A panic is logged and the next tick still runs, rather than ending the thread silently
                    let ticked = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        let state = handle.state::<AppState>();
                        commands::tick_and_sync(&state, commands::Moment::current(), commands::save_dir().as_deref())
                    }));
                    match ticked {
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => eprintln!("Tick failed: {}", e),
                        Err(_) => eprintln!("Tick panicked"),
                    }
                })?;

            // Set up window blur event to hide when focus is lost
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = window_clone.hide();
                    }
                });
            }

            // Create tray icon (no menu, just click to toggle)
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .on_tray_icon_event(|tray, event| {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            position,
                            ..
                        } => {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    // Position window directly below the tray icon
                                    // Use logical position (divide by scale factor for Retina)
                                    let scale_factor = window.scale_factor().unwrap_or(2.0);
                                    let click_x = (position.x / scale_factor) as i32;
                                    let window_width = 280;
                                    let x = click_x - (window_width / 2);
                                    let y = 25.0; // Just below menu bar
                                    let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x: x as f64, y }));
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_pet_state,
            commands::feed_pet,
            commands::play_with_pet,
            commands::put_to_sleep,
            commands::revive_pet,
            commands::get_game_stats,
            commands::get_achievements,
            commands::get_snacks,
            commands::start_game,
            commands::game_step,
            commands::report_game_result,
            commands::buy_snack,
            commands::get_load_warning,
            commands::companion_open,
            commands::refresh_companion_mood,
            commands::get_companion_settings,
            commands::set_companion_settings,
            commands::companion_onboarding_answer,
            commands::get_sync_settings,
            commands::set_sync_settings,
            commands::sync_now,
            commands::get_sync_status,
            commands::sync_choose_pet,
            commands::quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
