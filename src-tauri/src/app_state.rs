use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::AppSettings;
use crate::crypto::keystore::VaultManager;
use crate::gdrive_sync::worker::SyncManager;
use crate::indexer::db::IndexDb;
use crate::indexer::render::RenderCache;
use crate::watcher::WatcherManager;
use crate::window_manager::DockController;
use crate::window_manager::hotkey::HotkeyManager;

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub settings_path: PathBuf,
    pub dock: DockController,
    pub hotkey: HotkeyManager,
    pub db: IndexDb,
    pub watcher: WatcherManager,
    pub render_cache: RenderCache,
    pub vault: Arc<VaultManager>,
    pub sync: Arc<SyncManager>,
}

impl AppState {
    pub fn new(
        settings: AppSettings,
        settings_path: PathBuf,
        db: IndexDb,
        watcher: WatcherManager,
        vault: Arc<VaultManager>,
        sync: Arc<SyncManager>,
    ) -> Self {
        let dock = DockController::new(
            settings.vertical_ratio,
            settings.monitor_policy.clone(),
            settings.pinned_by_default,
        );
        Self {
            settings: Mutex::new(settings),
            settings_path,
            dock,
            hotkey: HotkeyManager::default(),
            db,
            watcher,
            render_cache: RenderCache::default(),
            vault,
            sync,
        }
    }
}
