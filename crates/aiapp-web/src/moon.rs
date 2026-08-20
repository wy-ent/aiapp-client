//! MoonBit toolchain availability guarantee.
//!
//! Startup pre-build and AI generation both depend on the `moon` command. For
//! "out-of-the-box deployment", when it is missing this module automatically
//! installs the official toolchain and returns the resolved executable path to
//! the build process.
//!
//! Resolution order:
//! 1. `moon` already exists in `PATH` (most common; satisfied by container images and manual installs)
//! 2. `~/.moon/bin/moon` (default target of the official install script, but PATH may not be refreshed)
//! 3. Try auto-install (unix: `cli.moonbitlang.com/install/unix.sh`; Windows: powershell script)
//! 4. If still failing, return `"moon"`, letting `aiapp_build` report `MoonNotFound` (only examples/generation become unavailable; does not block the service)

use std::path::{Path, PathBuf};
use std::process::Command;

/// Official install script.
const INSTALL_UNIX: &str = "curl -fsSL https://cli.moonbitlang.com/install/unix.sh | bash";
const INSTALL_WIN: &str =
    "Set-ExecutionPolicy RemoteSigned -Scope CurrentUser; irm https://cli.moonbitlang.com/install/powershell.ps1 | iex";

/// Ensure the MoonBit toolchain is available, returning the `moon` executable path (absolute or `moon`).
pub fn ensure_moon_toolchain() -> PathBuf {
    if let Some(p) = find_in_path() {
        return p;
    }
    if let Some(p) = home_moon() {
        eprintln!("[moon] Using ~/.moon/bin/moon: {}", p.display());
        return p;
    }
    eprintln!("[moon] MoonBit toolchain not detected, attempting auto-install ...");
    if auto_install() {
        if let Some(p) = home_moon() {
            eprintln!("[moon] MoonBit toolchain auto-installed: {}", p.display());
            return p;
        }
    }
    eprintln!(
        "[moon] Warning: MoonBit toolchain unavailable, example prebuild and AI generation will fail.\n\
        \tManual install: https://www.moonbitlang.com/download (or run `bash -c \"{INSTALL_UNIX}\"`"
    );
    PathBuf::from("moon")
}

/// When `moon` can be launched from PATH, resolve its real path.
fn find_in_path() -> Option<PathBuf> {
    if Command::new("moon").arg("--version").output().is_err() {
        return None;
    }
    // Resolve absolute path for stability across process/working-directory switches
    if let Ok(out) = Command::new("sh")
        .arg("-c")
        .arg("command -v moon")
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(PathBuf::from(s));
        }
    }
    Some(PathBuf::from("moon"))
}

/// The official install script places the binary at `$HOME/.moon/bin/moon` by default.
fn home_moon() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let p = Path::new(&home).join(".moon/bin/moon");
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

#[cfg(unix)]
fn auto_install() -> bool {
    match Command::new("bash")
        .arg("-c")
        .arg(INSTALL_UNIX)
        .status()
    {
        Ok(s) if s.success() => true,
        Ok(s) => {
            eprintln!("[moon] Auto-install script exit code {s}");
            false
        }
        Err(e) => {
            eprintln!("[moon] Cannot execute install script (requires curl + bash): {e}");
            false
        }
    }
}

#[cfg(not(unix))]
fn auto_install() -> bool {
    match Command::new("powershell")
        .arg("-Command")
        .arg(INSTALL_WIN)
        .status()
    {
        Ok(s) if s.success() => true,
        Ok(s) => {
            eprintln!("[moon] Auto-install script exit code {s}");
            false
        }
        Err(e) => {
            eprintln!("[moon] Cannot execute install script (requires PowerShell + network): {e}");
            false
        }
    }
}
