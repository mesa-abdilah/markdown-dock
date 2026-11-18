pub mod app_state;
pub mod commands;
pub mod config;
pub mod crypto;
pub mod error;
pub mod events;
pub mod gdrive_sync;
pub mod indexer;
pub mod watcher;
pub mod window_manager;

use std::sync::Arc;

#[cfg(any(debug_assertions, test))]
use specta_typescript::Typescript;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::MacosLauncher;
use tauri_specta::{Builder, Event, collect_commands, collect_events};

use crate::app_state::AppState;
use crate::config::{load_settings, settings_path};
use crate::crypto::keystore::VaultManager;
use crate::events::{
    DockFocusRequestEvent, DockModeChangedEvent, DriveConnectionEvent, HotkeyFallbackNoticeEvent,
    HotkeyTriggeredEvent, IndexChangedEvent, IndexProgressEvent, OpenSettingsRequestEvent,
    SyncStatusEvent, VaultStateChangedEvent,
};
use crate::gdrive_sync::client::DriveClient;
use crate::gdrive_sync::oauth::OAuthManager;
use crate::gdrive_sync::worker::SyncManager;
use crate::indexer::db::IndexDb;
use crate::watcher::WatcherManager;
use crate::window_manager::geometry::DockMode;

fn build_specta() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::dock::dock_get_state,
            commands::dock::dock_plan_transition,
            commands::dock::dock_apply,
            commands::dock::dock_set_pinned,
            commands::dock::dock_set_vertical_ratio,
            commands::dock::dock_nudge_vertical_by_screen_y,
            commands::dock::dock_redock,
            commands::settings::settings_get,
            commands::settings::settings_update,
            commands::settings::list_monitors,
            commands::roots::roots_list,
            commands::roots::roots_count_docs,
            commands::roots::roots_create_folder,
            commands::roots::roots_rename_folder,
            commands::roots::roots_add,
            commands::roots::roots_remove,
            commands::roots::roots_set_enabled,
            commands::roots::roots_set_backup,
            commands::roots::roots_rescan,
            commands::search::search_docs,
            commands::search::get_snippets,
            commands::files::get_preview,
            commands::files::copy_document,
            commands::files::open_in_editor,
            commands::files::reveal_in_folder,
            commands::files::open_external_url,
            commands::files::preview_dropped_file,
            commands::files::upload_markdown_files,
            commands::files::move_document_to_folder,
            commands::files::delete_document,
            commands::files::save_document,
            commands::vault::vault_status,
            commands::vault::vault_check_password_strength,
            commands::vault::vault_begin_setup,
            commands::vault::vault_confirm_setup,
            commands::vault::vault_cancel_setup,
            commands::vault::vault_unlock_password,
            commands::vault::vault_unlock_recovery,
            commands::vault::vault_change_password,
            commands::vault::vault_lock,
            commands::sync::sync_status,
            commands::sync::drive_connect,
            commands::sync::drive_disconnect,
            commands::sync::drive_check_remote_keyfile,
            commands::sync::drive_import_remote_keyfile,
            commands::sync::sync_now,
            commands::sync::sync_pull,
            commands::sync::conflicts_list,
            commands::sync::conflicts_resolve,
            commands::sync::restore_list,
            commands::sync::restore_run,
        ])
        .events(collect_events![
            DockModeChangedEvent,
            HotkeyTriggeredEvent,
            HotkeyFallbackNoticeEvent,
            DockFocusRequestEvent,
            OpenSettingsRequestEvent,
            IndexProgressEvent,
            IndexChangedEvent,
            VaultStateChangedEvent,
            SyncStatusEvent,
            DriveConnectionEvent,
        ])
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle_item = MenuItem::with_id(app, "toggle", "Toggle Dock", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let sync_item = MenuItem::with_id(app, "sync", "Backup Now", true, None::<&str>)?;
    let rescan_item = MenuItem::with_id(app, "rescan", "Rescan Folders", true, None::<&str>)?;
    let lock_item = MenuItem::with_id(app, "lock", "Lock Vault", true, None::<&str>)?;
    let redock_item = MenuItem::with_id(app, "redock", "Re-dock to Edge", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle_item,
            &settings_item,
            &sync_item,
            &rescan_item,
            &lock_item,
            &redock_item,
            &quit_item,
        ],
    )?;

    let mut tray_builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Dynamic Markdown Dock")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.hotkey.trigger(app);
                }
            }
            "settings" => {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(plan) = state.dock.plan_transition(app, DockMode::Expanded) {
                        let _ = state.dock.apply_plan(app, plan.seq, true);
                    }
                    let _ = OpenSettingsRequestEvent.emit(app);
                }
            }
            "sync" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let sync = Arc::clone(&state.sync);
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = sync.sync_now(Some(&handle)).await;
                    });
                }
            }
            "rescan" => {
                if let Some(state) = app.try_state::<AppState>() {
                    for root in state.db.list_roots().unwrap_or_default() {
                        if root.enabled {
                            let _ = state.watcher.rescan_root(i64::from(root.id));
                        }
                    }
                }
            }
            "lock" => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.vault.lock(Some(app), false);
                }
            }
            "redock" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = state.dock.redock(app);
                }
            }
            "quit" => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.vault.lock(None, false);
                    state.db.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(state) = app.try_state::<AppState>() {
                    state.hotkey.trigger(app);
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }

    tray_builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = build_specta();

    #[cfg(debug_assertions)]
    specta_builder
        .export(Typescript::default(), "../src/bindings/commands.ts")
        .expect("Failed to export TypeScript bindings");

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|a| a == "--toggle") {
                if let Some(state) = app.try_state::<AppState>() {
                    state.hotkey.trigger(app);
                }
                return;
            }
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(state) = handle.try_state::<AppState>()
                    && let Ok(plan) = state.dock.plan_transition(&handle, DockMode::Expanded)
                {
                    let _ = state.dock.apply_plan(&handle, plan.seq, true);
                    let shortcut = state.hotkey.active().unwrap_or_default();
                    let _ = HotkeyTriggeredEvent {
                        shortcut,
                        target_mode: DockMode::Expanded,
                    }
                    .emit(&handle);
                    let _ = DockFocusRequestEvent { focus_search: true }.emit(&handle);
                }
            });
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);

            let handle = app.handle();
            let cfg_path = settings_path(handle)?;
            let mut settings = load_settings(&cfg_path);

            let data_dir = handle
                .path()
                .app_data_dir()
                .map_err(|e| crate::error::AppError::Db(e.to_string()))?;
            let db = IndexDb::open(&data_dir.join("index.db"))?;
            let watcher = WatcherManager::start(db.clone(), handle.clone())?;
            let vault = Arc::new(VaultManager::init(data_dir.join("keyfile.vault")));
            let oauth = Arc::new(OAuthManager::default());
            let sync = Arc::new(SyncManager::new(
                db.clone(),
                Arc::clone(&vault),
                Arc::clone(&oauth),
                DriveClient::default(),
                settings.google_client_id.clone(),
                settings.google_client_secret.clone(),
                settings.sync_retention_days,
            ));
            sync.spawn_background_worker(handle.clone());

            let state = AppState::new(settings.clone(), cfg_path.clone(), db, watcher, vault, sync);
            if let Err(err) = commands::roots::ensure_default_folder(handle, &state) {
                tracing::warn!("Could not ensure default folder: {err}");
            }
            state.dock.init_window(handle)?;

            match state.hotkey.register_initial(handle, &settings.hotkey) {
                Ok(active) => {
                    if active != settings.hotkey {
                        settings.hotkey = active;
                        if let Ok(mut s) = state.settings.lock() {
                            s.hotkey = settings.hotkey.clone();
                        }
                        let _ = crate::config::save_settings(&cfg_path, &settings);
                    }
                }
                Err(err) => {
                    tracing::error!("Could not register any global hotkey: {err}");
                }
            }

            app.manage(state);
            setup_tray(handle)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == window_manager::DOCK_WINDOW_LABEL
                && let tauri::WindowEvent::ScaleFactorChanged { .. } = event
                && let Some(state) = window.app_handle().try_state::<AppState>()
            {
                let _ = state.dock.redock(window.app_handle());
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Dynamic Markdown Dock");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_typescript_bindings() {
        build_specta()
            .export(Typescript::default(), "../src/bindings/commands.ts")
            .expect("Failed to export TypeScript bindings");
    }
}
