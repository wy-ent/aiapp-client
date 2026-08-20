//! Runtime configuration: database connection and object-storage backend.
//!
//! Configuration sources (lowest to highest priority): built-in defaults → environment variables → `.env` file at the repo root.
//! Hence the "database connection info" is configured via the `DATABASE_URL` environment variable (with an optional `.env` file).

use std::net::IpAddr;
use std::path::PathBuf;

/// Default database connection string when `DATABASE_URL` is not set:
/// local SQLite file (zero dependencies, auto-creates tables, persists data, no longer falls back to in-memory mode).
pub const DEFAULT_DATABASE_URL: &str = "sqlite://./data/aiapp.db";

/// Application runtime configuration.
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Listen address (IP), default `0.0.0.0` (accessible externally when deployed). For local development set `127.0.0.1`.
    pub host: IpAddr,
    /// Listen port, default `8080`.
    pub port: u16,

    /// Metadata database connection string; the driver is selected by the prefix:
    /// - `postgres://user:pass@host:5432/dbname` → PostgreSQL (recommended for production)
    /// - `sqlite://./data/aiapp.db` or `sqlite:///data/aiapp.db` → local SQLite file (zero dependencies)
    /// When `DATABASE_URL` is not set, a local SQLite file is used by default (`DEFAULT_DATABASE_URL`), with data persisted (no longer falling back to in-memory mode).
    pub database_url: Option<String>,
    /// When true, failure to connect to the database causes an immediate exit (in production, rely on the orchestrator's retry,
    /// to avoid silently degrading to a non-persistent state that loses data). Default false.
    pub require_db: bool,

    /// Artifact storage backend: `local` / `cos` / `oss`, default `local`.
    pub storage_backend: String,
    /// Root directory for the `local` backend, default `./storage`.
    pub storage_local_root: PathBuf,
    /// Frontend static-asset directory (template thumbnails, etc.), overridable at runtime via `STATIC_DIR`,
    /// defaulting to `./templates` relative to the current working directory.
    pub static_dir: PathBuf,

    /// JWT signing secret. In production it must be set to a strong random value via `AUTH_SECRET`.
    pub auth_secret: String,
    /// Initial admin username, default `admin`.
    pub admin_username: String,
    /// Initial admin password, default `admin123` (written to the database on first startup).
    pub admin_password: String,

    // -- Tencent Cloud COS credentials (used when STORAGE_BACKEND=cos) --
    pub cos_secret_id: Option<String>,
    pub cos_secret_key: Option<String>,
    pub cos_bucket: Option<String>,
    pub cos_region: Option<String>,

    // -- Alibaba Cloud OSS credentials (used when STORAGE_BACKEND=oss) --
    pub oss_access_key_id: Option<String>,
    pub oss_access_key_secret: Option<String>,
    pub oss_bucket: Option<String>,
    pub oss_endpoint: Option<String>,
}

impl AppConfig {
    /// Load configuration from environment variables + `.env` file.
    pub fn from_env() -> Self {
        // Load .env if present (ignore failure and continue with env vars / defaults)
        let _ = dotenvy::dotenv();

        // Listen address: HOST accepts "0.0.0.0" / "127.0.0.1", etc.; defaults to externally accessible
        let host = std::env::var("HOST")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .and_then(|s| s.parse::<IpAddr>().ok())
            .unwrap_or_else(|| IpAddr::from([0, 0, 0, 0]));

        let port = std::env::var("PORT")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8080);

        let storage_backend = std::env::var("STORAGE_BACKEND")
            .ok()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "local".to_string());

        let storage_local_root = std::env::var("STORAGE_LOCAL_ROOT")
            .map(|s| PathBuf::from(s.trim().to_string()))
            .unwrap_or_else(|_| PathBuf::from("storage"));

        let static_dir = std::env::var("STATIC_DIR")
            .map(|s| PathBuf::from(s.trim().to_string()))
            .unwrap_or_else(|_| PathBuf::from("templates"));

        // When DATABASE_URL is not set, default to a local SQLite file (persisted, no longer falling back to in-memory mode)
        let database_url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| Some(DEFAULT_DATABASE_URL.to_string()));

        let require_db = std::env::var("REQUIRE_DB")
            .ok()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .map(|s| s == "1" || s == "true")
            .unwrap_or(false);

        let env_opt = |k: &str| std::env::var(k).ok().filter(|s| !s.trim().is_empty());

        // JWT secret: must be explicitly configured in production, otherwise use an insecure default and warn
        let auth_secret = env_opt("AUTH_SECRET")
            .unwrap_or_else(|| "dev-insecure-secret-change-me".to_string());
        if auth_secret == "dev-insecure-secret-change-me" {
            eprintln!(
                "[auth] Warning: AUTH_SECRET is not set; using an insecure default. In production, configure a strong random secret via the environment variable."
            );
        }

        let admin_username = env_opt("ADMIN_USERNAME").unwrap_or_else(|| "admin".to_string());
        let admin_password = env_opt("ADMIN_PASSWORD").unwrap_or_else(|| "admin123".to_string());
        if admin_password == "admin123" {
            eprintln!(
                "[auth] Warning: the admin password is the default admin123; in production change it via ADMIN_PASSWORD."
            );
        }

        AppConfig {
            host,
            port,
            database_url,
            require_db,
            storage_backend,
            storage_local_root,
            static_dir,
            auth_secret,
            admin_username,
            admin_password,
            cos_secret_id: env_opt("COS_SECRET_ID"),
            cos_secret_key: env_opt("COS_SECRET_KEY"),
            cos_bucket: env_opt("COS_BUCKET"),
            cos_region: env_opt("COS_REGION"),
            oss_access_key_id: env_opt("OSS_ACCESS_KEY_ID"),
            oss_access_key_secret: env_opt("OSS_ACCESS_KEY_SECRET"),
            oss_bucket: env_opt("OSS_BUCKET"),
            oss_endpoint: env_opt("OSS_ENDPOINT"),
        }
    }
}
