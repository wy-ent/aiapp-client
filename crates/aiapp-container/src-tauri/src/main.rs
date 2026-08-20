//! aiapp-container — desktop/mobile container for AI-generated apps.
//!
//! Tauri shell with WebView-based rendering. See `lib.rs` for the implementation.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    aiapp_container::run();
}