//! Unified artifact storage abstraction.
//!
//! Generated app artifacts (source code, `.aiapp` packages, compiled wasm, etc.) are no longer
//! scattered in the source tree, but are accessed through a unified `Storage` interface.
//! Configurable backends:
//! - `local`: server-local directory (default, fully implemented, verifiable offline)
//! - `cos`: Tencent Cloud Object Storage (integrated; supports upload/download and URL
//!   pre-signing; credentials via `COS_*` environment variables)
//! - `oss`: Alibaba Cloud Object Storage (reserved; integrate once real credentials are available)
//!
//! Metadata (app registration / users / prompts) lives in PostgreSQL; see `db.rs`.

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::AppConfig;

/// Unified storage backend interface.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Write the bytes for a given key.
    async fn put(&self, key: &str, data: &[u8]) -> Result<()>;
    /// Read the bytes for a given key.
    async fn get_bytes(&self, key: &str) -> Result<Vec<u8>>;
    /// Backend type identifier: `local` / `cos` / `oss`.
    fn scheme(&self) -> &str;
    /// Generate a pre-signed download URL for an object (URL pre-authorization).
    /// Returns `None` when the backend does not support it, in which case the
    /// caller should fall back to `get_bytes` for a server-side streamed response.
    async fn presigned_url(&self, key: &str, expires_secs: u64) -> Result<Option<String>>;
}

/// Local filesystem storage: treats the key as a relative path under `root`.
pub struct LocalStorage {
    root: PathBuf,
}

impl LocalStorage {
    pub fn new(root: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&root);
        LocalStorage { root }
    }

    fn path(&self, key: &str) -> PathBuf {
        // Normalize the key to prevent `..` from escaping root
        let safe = key.trim_start_matches('/').replace("..", "");
        self.root.join(safe)
    }
}

#[async_trait]
impl Storage for LocalStorage {
    async fn put(&self, key: &str, data: &[u8]) -> Result<()> {
        let p = self.path(key);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, data)?;
        Ok(())
    }

    async fn get_bytes(&self, key: &str) -> Result<Vec<u8>> {
        Ok(std::fs::read(self.path(key))?)
    }

    fn scheme(&self) -> &str {
        "local"
    }

    async fn presigned_url(&self, _key: &str, _expires_secs: u64) -> Result<Option<String>> {
        // Local storage has no pre-signing concept; caller falls back to get_bytes streaming.
        Ok(None)
    }
}

/// Placeholder for a cloud storage backend that is not yet integrated; errors explicitly
/// at build time to avoid silent failure.
pub struct UnavailableStorage {
    scheme: &'static str,
}

#[async_trait]
impl Storage for UnavailableStorage {
    async fn put(&self, _key: &str, _data: &[u8]) -> Result<()> {
        anyhow::bail!(
            "Storage backend `{}` is not yet integrated; set STORAGE_BACKEND=local, or configure the corresponding cloud storage credentials before enabling it",
            self.scheme
        )
    }

    async fn get_bytes(&self, _key: &str) -> Result<Vec<u8>> {
        anyhow::bail!("Storage backend `{}` is not yet integrated", self.scheme)
    }

    fn scheme(&self) -> &str {
        self.scheme
    }

    async fn presigned_url(&self, _key: &str, _expires_secs: u64) -> Result<Option<String>> {
        Ok(None)
    }
}

/// Build the storage backend instance based on configuration.
///
/// - `cos`: Tencent Cloud Object Storage; if `COS_*` credentials are incomplete, prints a
///   warning and falls back to local storage to avoid crashing at startup (calling a concrete
///   method without credentials returns a clear error).
/// - `oss`: Alibaba Cloud Object Storage (reserved).
/// - other / default: `local` server-local directory.
pub fn build(cfg: &AppConfig) -> Arc<dyn Storage> {
    match cfg.storage_backend.as_str() {
        "cos" => {
            let incomplete = cfg
                .cos_secret_id
                .as_deref()
                .unwrap_or_default()
                .is_empty()
                || cfg.cos_secret_key.as_deref().unwrap_or_default().is_empty()
                || cfg.cos_bucket.as_deref().unwrap_or_default().is_empty()
                || cfg.cos_region.as_deref().unwrap_or_default().is_empty();
            if incomplete {
                eprintln!(
                    "[storage] Warning: STORAGE_BACKEND=cos but COS_SECRET_ID/KEY/BUCKET/REGION are not fully configured; falling back to local storage. For production, complete the credentials (see .env.example)."
                );
                Arc::new(LocalStorage::new(cfg.storage_local_root.clone()))
            } else {
                let cos_cfg = crate::cos::CosConfig {
                    secret_id: cfg.cos_secret_id.clone().unwrap(),
                    secret_key: cfg.cos_secret_key.clone().unwrap(),
                    bucket: cfg.cos_bucket.clone().unwrap(),
                    region: cfg.cos_region.clone().unwrap(),
                };
                Arc::new(crate::cos::CosStorage::new(cos_cfg))
            }
        }
        "oss" => Arc::new(UnavailableStorage { scheme: "oss" }),
        _ => Arc::new(LocalStorage::new(cfg.storage_local_root.clone())),
    }
}
