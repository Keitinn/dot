// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    AppHandle, Manager, Monitor, PhysicalPosition, Position, Rect, Runtime, Size,
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
};
use std::panic::AssertUnwindSafe;
use std::time::Duration;
use dot::{AppState, commands};
use dot::popover::{self, IconRect, MonitorArea};

/// Logical width of the popover, as in tauri.conf.json.
const POPOVER_WIDTH: f64 = 280.0;

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
                            rect,
                            ..
                        } => {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    // With no monitor found the window opens where it last was
                                    if let Some((x, y)) = popover_origin(app, position, rect) {
                                        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
                                    }
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

/// The popover's logical top-left under the clicked tray icon, on the monitor
/// that icon is on.
fn popover_origin<R: Runtime>(
    app: &AppHandle<R>,
    click: PhysicalPosition<f64>,
    rect: Rect,
) -> Option<(f64, f64)> {
    let click = (click.x, click.y);
    let cursor = cursor_points(app);
    // On macOS monitor_from_point takes global points, not physical pixels
    let found = cursor.and_then(|(x, y)| app.monitor_from_point(x, y).ok().flatten());
    let monitor = match found {
        Some(monitor) => monitor_area(&monitor),
        None => {
            let monitors: Vec<MonitorArea> =
                app.available_monitors().ok()?.iter().map(monitor_area).collect();
            monitors[popover::find_clicked_monitor(&monitors, cursor, click)?]
        }
    };
    Some(popover::popover_position(&monitor, icon_rect(rect), click, POPOVER_WIDTH))
}

/// The cursor in global points. The tray click can't tell which monitor it is
/// on by itself (it is scaled by that monitor's factor), but on macOS the
/// cursor is always scaled by the primary monitor's.
fn cursor_points<R: Runtime>(app: &AppHandle<R>) -> Option<(f64, f64)> {
    let cursor = app.cursor_position().ok()?;
    let scale = app.primary_monitor().ok()??.scale_factor();
    Some((cursor.x / scale, cursor.y / scale))
}

fn monitor_area(monitor: &Monitor) -> MonitorArea {
    let position = monitor.position();
    let size = monitor.size();
    MonitorArea {
        x: position.x as f64,
        y: position.y as f64,
        width: size.width as f64,
        height: size.height as f64,
        scale: monitor.scale_factor(),
    }
}

/// The tray reports its rect in physical pixels; anything else is not trusted.
fn icon_rect(rect: Rect) -> Option<IconRect> {
    match (rect.position, rect.size) {
        (Position::Physical(position), Size::Physical(size)) => Some(IconRect {
            x: position.x as f64,
            y: position.y as f64,
            width: size.width as f64,
            height: size.height as f64,
        }),
        _ => None,
    }
}
