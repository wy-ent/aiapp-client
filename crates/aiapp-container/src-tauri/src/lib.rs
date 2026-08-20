//! aiapp-container: Tauri host for the aiapp ecosystem.
//!
//! Provides the native bridge (HostApi) and IPC commands for the container frontend.
//! The frontend is a WebView-based HTML/CSS/JS app that manages and runs AI apps.

pub mod bridge;

use std::sync::Arc;

use aiapp_render::host_api::{HostApi, HostResult, HostError};

/// Tauri IPC commands exposed to the frontend via `invoke()`.
pub mod commands {
    use super::*;

    /// Get an app's stored value.
    #[tauri::command]
    pub fn storage_get(key: String) -> Result<Option<Vec<u8>>, String> {
        let bridge = bridge::get();
        bridge.storage_get(&key).map_err(|e| e.to_string())
    }

    /// Set an app's stored value.
    #[tauri::command]
    pub fn storage_set(key: String, value: Vec<u8>) -> Result<(), String> {
        let bridge = bridge::get();
        bridge.storage_set(&key, &value).map_err(|e| e.to_string())
    }

    /// Delete a stored value.
    #[tauri::command]
    pub fn storage_delete(key: String) -> Result<(), String> {
        let bridge = bridge::get();
        bridge.storage_delete(&key).map_err(|e| e.to_string())
    }

    /// Show a system notification.
    #[tauri::command]
    pub fn show_notification(title: String, body: String) -> Result<(), String> {
        let bridge = bridge::get();
        bridge.show_notification(&title, &body).map_err(|e| e.to_string())
    }

    /// Get the platform identifier.
    #[tauri::command]
    pub fn platform() -> String {
        bridge::get().platform().to_string()
    }

    /// Get the renderer mode.
    #[tauri::command]
    pub fn renderer_mode() -> String {
        bridge::get().renderer_mode().to_string()
    }

    /// Log a message.
    #[tauri::command]
    pub fn log(level: String, message: String) {
        bridge::get().log(&level, &message);
    }
}

/// Configure and run the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::storage_get,
            commands::storage_set,
            commands::storage_delete,
            commands::show_notification,
            commands::platform,
            commands::renderer_mode,
            commands::log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running aiapp container");
}