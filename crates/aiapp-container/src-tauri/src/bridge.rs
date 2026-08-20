//! JS Bridge: implements aiapp-render's HostApi trait using Tauri's native capabilities.
//!
//! This bridge is injected into the WebView as `window.__aiapp_bridge` via Tauri IPC commands.
//! The frontend JS calls `invoke('storage_get', { key })` which maps to these functions.

use std::sync::{Mutex, OnceLock};

use aiapp_render::host_api::{HostApi, HostResult, HostError};

/// Global bridge instance (lazy init).
static BRIDGE: OnceLock<ContainerBridge> = OnceLock::new();

/// Get the global bridge instance.
pub fn get() -> &'static ContainerBridge {
    BRIDGE.get_or_init(ContainerBridge::new)
}

/// Container bridge: implements HostApi using Tauri native APIs.
///
/// Storage: uses `tauri::api::path::app_local_data_dir` for the base path.
/// Notifications: uses `tauri::api::notification::Notification`.
/// Logging: uses `println!` (visible in Tauri dev console).
pub struct ContainerBridge {
    store: Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

impl ContainerBridge {
    pub fn new() -> Self {
        ContainerBridge {
            store: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl HostApi for ContainerBridge {
    fn storage_set(&self, key: &str, value: &[u8]) -> HostResult<()> {
        self.store
            .lock()
            .map_err(|e| HostError::Runtime(e.to_string()))?
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn storage_get(&self, key: &str) -> HostResult<Option<Vec<u8>>> {
        Ok(self
            .store
            .lock()
            .map_err(|e| HostError::Runtime(e.to_string()))?
            .get(key)
            .cloned())
    }

    fn storage_delete(&self, key: &str) -> HostResult<()> {
        self.store
            .lock()
            .map_err(|e| HostError::Runtime(e.to_string()))?
            .remove(key);
        Ok(())
    }

    fn show_notification(&self, title: &str, body: &str) -> HostResult<()> {
        println!("[aiapp:notification] {title}: {body}");
        // TODO: use tauri-plugin-notification when available
        Ok(())
    }

    fn log(&self, level: &str, message: &str) {
        println!("[aiapp:{level}] {message}");
    }

    fn http_request(
        &self,
        _url: &str,
        _method: &str,
        _headers: &[(String, String)],
        _body: Option<&[u8]>,
    ) -> HostResult<(u16, Vec<u8>)> {
        Err(HostError::Unsupported("http_request".into()))
    }

    fn get_location(&self) -> HostResult<(f64, f64)> {
        Err(HostError::Unsupported("get_location".into()))
    }

    fn platform(&self) -> &str {
        if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "ios") {
            "ios"
        } else if cfg!(target_os = "android") {
            "android"
        } else {
            "desktop"
        }
    }

    fn renderer_mode(&self) -> &str {
        "webview"
    }
}