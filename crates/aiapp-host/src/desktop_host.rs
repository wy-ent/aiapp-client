//! Desktop/CLI host: implements the capabilities of the WIT contract `aiapp:app-host/host`.
//!
//! This is the desktop-side host implementation of "build once, run anywhere":
//! - `save-data` / `load-data` → local files (data is portable, not lost when switching devices);
//! - `show-notification` → terminal output (can be replaced with a system notification on desktop);
//! - `log` → terminal logs.
//!
//! Each application instance has its own `root` (data directory), with data isolated per app,
//! matching the granularity of the web version's IndexedDB (isolated by app_id).

use std::path::{PathBuf};

use aiapp_engine::host::Host;
use async_trait::async_trait;

/// Desktop host. `root` is this application instance's data directory (constructed by the caller from app_id).
pub struct DesktopHost {
    /// Data root directory (where `save-data` / `load-data` land).
    root: PathBuf,
}

impl DesktopHost {
    /// Create the desktop host; data lands under `root` (the application instance data directory).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        DesktopHost { root: root.into() }
    }
}

/// Validate and resolve a data file path, preventing `..` traversal from escaping the data root.
fn safe_path(root: &PathBuf, key: &str) -> Option<PathBuf> {
    let mut base = root.clone();
    for part in key.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains('\\') {
            return None;
        }
        base.push(part);
    }
    Some(base)
}

#[async_trait]
impl Host for DesktopHost {
    async fn show_notification(&self, title: &str, body: &str) {
        // Desktop: display in terminal; replace with a system notification (e.g. notify-rust) to integrate a real notification center.
        println!("[aiapp-host:notification] {title}: {body}");
    }

    async fn save_data(&self, key: &str, value: &[u8]) -> bool {
        let path = match safe_path(&self.root, key) {
            Some(p) => p,
            None => {
                eprintln!("[aiapp-host:storage] invalid key: {key}");
                return false;
            }
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("[aiapp-host:storage] failed to create directory: {e}");
                return false;
            }
        }
        match std::fs::write(&path, value) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("[aiapp-host:storage] failed to write {}: {e}", path.display());
                false
            }
        }
    }

    async fn load_data(&self, key: &str) -> Option<Vec<u8>> {
        let path = safe_path(&self.root, key)?;
        std::fs::read(&path).ok()
    }

    async fn log(&self, level: &str, message: &str) {
        match level {
            "error" => eprintln!("[aiapp-host:error] {message}"),
            _ => println!("[aiapp-host:{level}] {message}"),
        }
    }
}
