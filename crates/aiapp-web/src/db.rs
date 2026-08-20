//! Metadata persistence layer (PostgreSQL or SQLite, auto-selected by the `DATABASE_URL` prefix).
//!
//! Stores platform/app-repository metadata: app registration info, users, and the admin prompt.
//! Uses an "in-memory hot cache + database persistence layer" strategy:
//! - When `DATABASE_URL` is not configured, defaults to a local SQLite file (`sqlite://./data/aiapp.db`),
//!   data is persisted. Only on connection/migration failure does `pool = None` degrade to no persistence.
//! - Once configured, data is loaded from the database at startup and write operations are written back
//!   synchronously, so nothing is lost across restarts.
//!
//! Driver selection (based on the `DATABASE_URL` prefix):
//! - `postgres://` / `postgresql://` → PostgreSQL (recommended for production, supports multiple connections)
//! - `sqlite://` / `sqlite:`          → local SQLite file (zero external dependencies, works out of the box)
//!
//! Uses the unified `sqlx::Any` driver with runtime `?` placeholders; no real database is needed at compile time.

use anyhow::Result;
use sqlx::any::{AnyPoolOptions, AnyRow};
use sqlx::{AnyPool, Row};
use std::path::PathBuf;

use crate::{AppUser, MarketApp, config::AppConfig};

/// Database wrapper. A `None` pool means persistence is disabled (only in the degraded connection/migration failure case).
#[derive(Clone)]
pub struct Database {
    pool: Option<AnyPool>,
}

impl Database {
    /// Normalize a SQLite connection string: sqlx resolves relative paths (especially `sqlite://./x`)
    /// unreliably, mistaking `./` for the root directory. Here relative paths are uniformly resolved to
    /// absolute paths and handed to sqlx in `sqlite:///abs` form.
    fn normalize_sqlite_url(url: &str) -> String {
        if !url.starts_with("sqlite") {
            return url.to_string();
        }
        // Extract the path portion, supporting sqlite:///abs, sqlite://./x, sqlite:./x, sqlite:x, etc.
        // Note that `sqlite:///abs` is really `sqlite://` + absolute path `/abs`; here we uniformly restore
        // the leading `/` so that strip_prefix("sqlite:///") does not eat the leading slash of an absolute
        // path as if it were part of the protocol prefix.
        let path = if let Some(p) = url.strip_prefix("sqlite:///") {
            format!("/{p}")
        } else if let Some(p) = url.strip_prefix("sqlite://") {
            p.to_string()
        } else if let Some(p) = url.strip_prefix("sqlite:") {
            p.to_string()
        } else {
            url.to_string()
        };
        // Strip the leading ./ to avoid redundant paths like /cwd/./data
        let path = path.trim_start_matches("./").to_string();
        let abs = if std::path::Path::new(&path).is_absolute() {
            path
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(&path).to_string_lossy().into_owned()
        } else {
            return url.to_string();
        };
        let base = format!("sqlite://{abs}");
        // ?mode=rwc lets sqlx create a new database when the file does not exist; otherwise it errors with code 14
        if base.contains('?') {
            format!("{base}&mode=rwc")
        } else {
            format!("{base}?mode=rwc")
        }
    }

    /// Initialize the connection pool; when `DATABASE_URL` is not configured, defaults to a local SQLite
    /// file (`DEFAULT_DATABASE_URL`) instead of falling back to an in-memory mode. Degrades to no
    /// persistence (non-fatal) on connection or migration failure; can be promoted to fatal via `REQUIRE_DB`.
    pub async fn init(cfg: &AppConfig) -> Self {
        // Register the Any driver table (PostgreSQL / SQLite features are enabled at compile time).
        sqlx::any::install_default_drivers();
        // When DATABASE_URL is not configured, default to a local SQLite file (config layer already has a fallback; this is a second one)
        let url = cfg
            .database_url
            .clone()
            .unwrap_or_else(|| crate::config::DEFAULT_DATABASE_URL.to_string());
        // Normalize the SQLite path (relative paths resolved to absolute so sqlx can create the file correctly)
        let url = Self::normalize_sqlite_url(&url);
        let is_pg = url.starts_with("postgres");
        let driver = if is_pg { "PostgreSQL" } else { "SQLite" };
        // SQLite needs its parent directory to exist; create it in advance
        if let Some(p) = sqlite_db_path(&url) {
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        eprintln!("[db] Failed to create SQLite parent directory {:?}: {e}", parent);
                    }
                }
            }
        }
        // Concurrent writes to a SQLite file database easily lock tables; a single connection is more stable. PostgreSQL uses a pool
        let max_conn = if is_pg { 5 } else { 1 };
        match AnyPoolOptions::new().max_connections(max_conn).connect(&url).await {
            Ok(pool) => {
                let db = Database { pool: Some(pool) };
                if let Err(e) = db.migrate().await {
                    eprintln!("[db] Migration failed, persistence not enabled: {e}");
                    Database { pool: None }
                } else {
                    println!("[db] Connected to {driver} ({url}), persistence enabled");
                    db
                }
            }
            Err(e) => {
                eprintln!("[db] Failed to connect to {driver}, persistence not enabled: {e}");
                Database { pool: None }
            }
        }
    }

    /// Whether database persistence is enabled.
    pub fn enabled(&self) -> bool {
        self.pool.is_some()
    }

    /// Create tables (idempotent).
    pub async fn migrate(&self) -> Result<()> {
        let pool = match &self.pool {
            Some(p) => p,
            None => return Ok(()),
        };

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS apps (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                tags        TEXT NOT NULL DEFAULT '[]',
                platforms   TEXT NOT NULL DEFAULT '[]',
                template    TEXT NOT NULL DEFAULT 'minimal',
                source      TEXT NOT NULL DEFAULT '',
                created_at  TEXT NOT NULL DEFAULT '',
                version     TEXT NOT NULL DEFAULT '1.0.0',
                owner       TEXT NOT NULL DEFAULT 'me',
                visibility  TEXT NOT NULL DEFAULT 'private',
                status      TEXT NOT NULL DEFAULT 'draft',
                launches    BIGINT NOT NULL DEFAULT 0,
                report      TEXT NOT NULL DEFAULT '',
                review_note TEXT NOT NULL DEFAULT '',
                tier        TEXT NOT NULL DEFAULT 'open',
                category    TEXT NOT NULL DEFAULT 'tool',
                share       TEXT NOT NULL DEFAULT 'private',
                kind        TEXT NOT NULL DEFAULT 'app',
                hide_branding INTEGER NOT NULL DEFAULT 0,
                wasm        TEXT
            )"#,
        )
        .execute(pool)
        .await?;

        // Backfill legacy tables: add category / share scope columns.
        // Note that SQLite does not support `ADD COLUMN IF NOT EXISTS` (it errors syntactically);
        // we must first probe with PRAGMA table_info to see whether the column exists, and only then ALTER.
        self.ensure_column(pool, "apps", "category", "TEXT NOT NULL DEFAULT 'tool'")
            .await;
        self.ensure_column(pool, "apps", "share", "TEXT NOT NULL DEFAULT 'private'")
            .await;
        // Backfill legacy tables: add the run-mode (online/local) column, defaulting to local.
        self.ensure_column(pool, "apps", "net", "TEXT NOT NULL DEFAULT 'local'")
            .await;
        // Backfill legacy tables: app type (webpage vs regular) and branding toggle.
        self.ensure_column(pool, "apps", "kind", "TEXT NOT NULL DEFAULT 'app'")
            .await;
        self.ensure_column(pool, "apps", "hide_branding", "INTEGER NOT NULL DEFAULT 0")
            .await;
        // After backfilling category/share on legacy tables, mark historically public apps as publicly
        // shared, so they do not all become private and invisible
        let _ = sqlx::query("UPDATE apps SET share = 'public' WHERE share = 'private' AND visibility = 'public'")
            .execute(pool)
            .await;

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS users (
                id             TEXT PRIMARY KEY,
                name           TEXT NOT NULL,
                role           TEXT NOT NULL DEFAULT 'user',
                status         TEXT NOT NULL DEFAULT 'active',
                apps_generated BIGINT NOT NULL DEFAULT 0,
                launches       BIGINT NOT NULL DEFAULT 0,
                incentive      BIGINT NOT NULL DEFAULT 0,
                org            TEXT NOT NULL DEFAULT 'main',
                created_at     TEXT NOT NULL DEFAULT '',
                password_hash  TEXT NOT NULL DEFAULT ''
            )"#,
        )
        .execute(pool)
        .await?;

        // Backfill legacy tables: add password / org columns
        self.ensure_column(pool, "users", "password_hash", "TEXT NOT NULL DEFAULT ''")
            .await;
        self.ensure_column(pool, "users", "org", "TEXT NOT NULL DEFAULT 'main'")
            .await;
        // Backfill legacy tables: add the "My Apps" (launch records) column, storing app ids as a JSON array
        self.ensure_column(pool, "users", "installed", "TEXT NOT NULL DEFAULT '[]'")
            .await;

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS prompts (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )"#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Idempotently add a column to a table: SQLite's `ALTER TABLE ADD COLUMN` does not support
    /// `IF NOT EXISTS`, so this probes via `PRAGMA table_info` and only ALTERs when the column is missing.
    async fn ensure_column(&self, pool: &AnyPool, table: &str, column: &str, ddl: &str) {
        let exists = match sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(pool)
            .await
        {
            Ok(rows) => rows.iter().any(|r| {
                r.try_get::<String, _>("name")
                    .map(|n| n == column)
                    .unwrap_or(false)
            }),
            Err(_) => false,
        };
        if exists {
            return;
        }
        let _ = sqlx::query(&format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}"))
            .execute(pool)
            .await;
    }

    /// Insert or update an app record.
    pub async fn save_app(&self, app: &MarketApp) {
        let pool = match &self.pool {
            Some(p) => p,
            None => return,
        };
        let tags = serde_json::to_string(&app.tags).unwrap_or_else(|_| "[]".into());
        let platforms = serde_json::to_string(&app.platforms).unwrap_or_else(|_| "[]".into());
        let _ = sqlx::query(
            r#"INSERT INTO apps
               (id,name,description,tags,platforms,template,source,created_at,version,
                owner,visibility,status,launches,report,review_note,tier,category,share,wasm,net,kind,hide_branding)
               VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
               ON CONFLICT (id) DO UPDATE SET
                 name=excluded.name, description=excluded.description, tags=excluded.tags,
                 platforms=excluded.platforms, template=excluded.template, source=excluded.source,
                 created_at=excluded.created_at, version=excluded.version, owner=excluded.owner,
                 visibility=excluded.visibility, status=excluded.status, launches=excluded.launches,
                 report=excluded.report, review_note=excluded.review_note, tier=excluded.tier,
                 category=excluded.category, share=excluded.share, wasm=excluded.wasm,
                 net=excluded.net, kind=excluded.kind, hide_branding=excluded.hide_branding"#,
        )
        .bind(&app.id)
        .bind(&app.name)
        .bind(&app.description)
        .bind(&tags)
        .bind(&platforms)
        .bind(&app.template)
        .bind(&app.source)
        .bind(&app.created_at)
        .bind(&app.version)
        .bind(&app.owner)
        .bind(&app.visibility)
        .bind(&app.status)
        .bind(app.launches as i64)
        .bind(&app.report)
        .bind(&app.review_note)
        .bind(&app.tier)
        .bind(&app.category)
        .bind(&app.share)
        .bind(&app.wasm)
        .bind(&app.net)
        .bind(&app.kind)
        .bind(app.hide_branding as i32)
        .execute(pool)
        .await;
    }

    /// Delete an app record.
    pub async fn delete_app(&self, id: &str) {
        let pool = match &self.pool {
            Some(p) => p,
            None => return,
        };
        let _ = sqlx::query("DELETE FROM apps WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await;
    }

    /// Load all apps. Returns None when the database is empty or on error.
    pub async fn load_apps(&self) -> Option<Vec<MarketApp>> {
        let pool = self.pool.as_ref()?;
        let rows = sqlx::query("SELECT * FROM apps").fetch_all(pool).await.ok()?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_app(&r));
        }
        Some(out)
    }

    /// Insert or update a user (including the password hash).
    pub async fn save_user(&self, u: &AppUser) {
        let pool = match &self.pool {
            Some(p) => p,
            None => return,
        };
        let installed = serde_json::to_string(&u.installed).unwrap_or_else(|_| "[]".into());
        let _ = sqlx::query(
            r#"INSERT INTO users
               (id,name,role,status,apps_generated,launches,incentive,org,created_at,password_hash,installed)
               VALUES (?,?,?,?,?,?,?,?,?,?,?)
               ON CONFLICT (id) DO UPDATE SET
                 name=excluded.name, role=excluded.role, status=excluded.status,
                 apps_generated=excluded.apps_generated, launches=excluded.launches,
                 incentive=excluded.incentive, org=excluded.org, created_at=excluded.created_at,
                 password_hash=excluded.password_hash, installed=excluded.installed"#,
        )
        .bind(&u.id)
        .bind(&u.name)
        .bind(&u.role)
        .bind(&u.status)
        .bind(u.apps_generated as i64)
        .bind(u.launches as i64)
        .bind(u.incentive as i64)
        .bind(&u.org)
        .bind(&u.created_at)
        .bind(&u.password_hash)
        .bind(&installed)
        .execute(pool)
        .await;
    }

    /// Find a user by username (including password hash), used for login verification.
    pub async fn find_user_by_name(&self, name: &str) -> Option<AppUser> {
        let pool = self.pool.as_ref()?;
        let row = sqlx::query("SELECT * FROM users WHERE name = ?")
            .bind(name)
            .fetch_optional(pool)
            .await
            .ok()??;
        Some(row_to_user(&row))
    }

    /// Find a user by id (including password hash).
    pub async fn get_user(&self, id: &str) -> Option<AppUser> {
        let pool = self.pool.as_ref()?;
        let row = sqlx::query("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .ok()??;
        Some(row_to_user(&row))
    }

    /// Load all users.
    pub async fn load_users(&self) -> Option<Vec<AppUser>> {
        let pool = self.pool.as_ref()?;
        let rows = sqlx::query("SELECT * FROM users").fetch_all(pool).await.ok()?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_user(&r));
        }
        Some(out)
    }

    /// Save the admin prompt.
    pub async fn save_prompt(&self, prompt: &str) {
        let pool = match &self.pool {
            Some(p) => p,
            None => return,
        };
        let _ = sqlx::query(
            "INSERT INTO prompts (key, value) VALUES ('gen_prompt', ?) \
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )
        .bind(prompt)
        .execute(pool)
        .await;
    }

    /// Load the admin prompt.
    pub async fn load_prompt(&self) -> Option<String> {
        let pool = self.pool.as_ref()?;
        let row = sqlx::query("SELECT value FROM prompts WHERE key = 'gen_prompt'")
            .fetch_optional(pool)
            .await
            .ok()??;
        row.try_get("value").ok()
    }
}

/// Resolve the local file path from a `sqlite://` / `sqlite:` connection string (to pre-create the parent directory).
fn sqlite_db_path(url: &str) -> Option<PathBuf> {
    // Note that `sqlite:///abs` is really `sqlite://` + absolute path `/abs`; after stripping, the leading
    // `/` must be restored, consistent with normalize_sqlite_url, otherwise an absolute path would be
    // mistaken for a relative one.
    if let Some(p) = url.strip_prefix("sqlite:///") {
        Some(PathBuf::from(format!("/{p}")))
    } else if let Some(p) = url.strip_prefix("sqlite://") {
        Some(PathBuf::from(p))
    } else if let Some(p) = url.strip_prefix("sqlite:") {
        Some(PathBuf::from(p))
    } else {
        None
    }
}

/// Construct a `MarketApp` from a database row.
fn row_to_app(r: &AnyRow) -> MarketApp {
    let tags: String = r.try_get("tags").unwrap_or_else(|_| "[]".into());
    let platforms: String = r.try_get("platforms").unwrap_or_else(|_| "[]".into());
    MarketApp {
        id: r.try_get("id").unwrap_or_default(),
        name: r.try_get("name").unwrap_or_default(),
        description: r.try_get("description").unwrap_or_default(),
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        platforms: serde_json::from_str(&platforms).unwrap_or_default(),
        template: r.try_get("template").unwrap_or_else(|_| "minimal".into()),
        source: r.try_get("source").unwrap_or_default(),
        created_at: r.try_get("created_at").unwrap_or_default(),
        version: r.try_get("version").unwrap_or_else(|_| "1.0.0".into()),
        owner: r.try_get("owner").unwrap_or_else(|_| "me".into()),
        visibility: r.try_get("visibility").unwrap_or_else(|_| "private".into()),
        status: r.try_get("status").unwrap_or_else(|_| "draft".into()),
        launches: r.try_get::<i64, _>("launches").unwrap_or(0) as u64,
        report: r.try_get("report").unwrap_or_default(),
        review_note: r.try_get("review_note").unwrap_or_default(),
        tier: r.try_get("tier").unwrap_or_else(|_| "open".into()),
        category: r.try_get("category").unwrap_or_else(|_| "tool".into()),
        share: r.try_get("share").unwrap_or_else(|_| "private".into()),
        wasm: r.try_get("wasm").unwrap_or(None),
        net: r.try_get("net").unwrap_or_else(|_| "local".into()),
        kind: r.try_get("kind").unwrap_or_else(|_| "app".into()),
        hide_branding: r.try_get::<i32, _>("hide_branding").unwrap_or(0) != 0,
    }
}

/// Construct an `AppUser` from a database row.
fn row_to_user(r: &AnyRow) -> AppUser {
    AppUser {
        id: r.try_get("id").unwrap_or_default(),
        name: r.try_get("name").unwrap_or_default(),
        role: r.try_get("role").unwrap_or_else(|_| "user".into()),
        status: r.try_get("status").unwrap_or_else(|_| "active".into()),
        apps_generated: r.try_get::<i64, _>("apps_generated").unwrap_or(0) as u64,
        launches: r.try_get::<i64, _>("launches").unwrap_or(0) as u64,
        incentive: r.try_get::<i64, _>("incentive").unwrap_or(0) as u64,
        org: r.try_get("org").unwrap_or_else(|_| "main".into()),
        created_at: r.try_get("created_at").unwrap_or_default(),
        installed: r
            .try_get::<String, _>("installed")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        password_hash: r.try_get("password_hash").unwrap_or_default(),
    }
}
