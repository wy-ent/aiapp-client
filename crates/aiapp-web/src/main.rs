//! Employee self-service Web prototype backend (deployable as a multi-user backend service).
//!
//! Capabilities: input a natural-language description + choose a template → reuse the aiapp-gen /
//! aiapp-build pipeline to generate a MoonBit app project, returning source code and manifest;
//! optionally invoke `moon` to compile it into a `.aiapp` package.
//!
//! Auth: multi-user register / login (bcrypt password hash + JWT); write operations such as
//! generate / my apps / publish / admin require login; admin operations require the admin role.
//! Local SQLite persistence is enabled by default to support multi-user mode; only when the database
//! connection/migration fails does it degrade to single-admin mode (registration unavailable).
//!
//! Persistence: metadata (apps/users/prompts) is stored by default in a local SQLite file
//! (`sqlite://./data/aiapp.db`, switchable to PostgreSQL via `DATABASE_URL`; see `db.rs`);
//! artifacts (wasm, etc.) go through the unified storage abstraction (`storage.rs`,
//! switchable between local / COS / OSS).

mod auth;
mod config;
mod db;
mod seed;
mod storage;
mod cos;
mod moon;
mod telemetry;

use axum::{
    extract::{FromRequestParts, Path, Query, Request, State},
    http::{header, request::Parts, StatusCode, Uri},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tokio::sync::Mutex;

use aiapp_gen::{generate_source_with_prompt, write_project, GenConfig, TEMPLATES};

/// An app market entry.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct MarketApp {
    id: String,
    name: String,
    description: String,
    tags: Vec<String>,
    platforms: Vec<String>,
    template: String,
    source: String,
    created_at: String,
    /// Version number, e.g. "1.0.0".
    version: String,
    /// Owner identifier (user id); built-in sample apps are owned by "official".
    owner: String,
    /// Visibility: "public" published to the market; "private" visible/usable only by the owner.
    visibility: String,
    /// Lifecycle status: "draft" draft (owner only) / "reviewing" under review / "published" published / "disabled" disabled.
    status: String,
    /// Usage stats: launch (open) count.
    launches: u64,
    /// Most recent report reason.
    report: String,
    /// Review comment.
    review_note: String,
    /// Key of the compiled WASM artifact in the storage backend; None if not built.
    wasm: Option<String>,
    /// Open/closed-source tier: `open` open source / `commercial` closed source.
    tier: String,
    /// App category (chosen at creation, used for category-based auto review).
    /// Values: tool / office / entertainment / game / family / life / industry / other.
    category: String,
    /// Share scope: "private" private (owner only) / "org" org-shared / "public" publicly shared.
    share: String,
    /// Run mode: "online" needs network to fetch data / "local" keeps data locally (local data is removed on uninstall).
    net: String,
    /// App type: "app" (regular app) / "web" (webpage-style app, shown standalone without the shell).
    kind: String,
    /// When true, the standalone page hides the platform branding bar (paid feature).
    hide_branding: bool,
}

/// A platform user.
#[derive(Clone, Serialize, Deserialize)]
struct AppUser {
    id: String,
    name: String,
    /// Role: "admin" administrator / "user" regular user.
    role: String,
    /// User status: "active" active / "disabled" disabled.
    status: String,
    /// Cumulative number of generated apps.
    apps_generated: u64,
    /// Cumulative app launch count.
    launches: u64,
    /// Cumulative incentives granted.
    incentive: u64,
    /// Owning organization (visibility boundary for org sharing; defaults to "main", the enterprise's primary org).
    org: String,
    created_at: String,
    /// IDs of apps launched (installed): launched apps appear in "My Apps".
    installed: Vec<String>,
    /// Password hash (bcrypt). Not serialized externally.
    #[serde(skip_serializing)]
    password_hash: String,
}

/// App state: shared working directory + market list + user list + active prompt + persistence layer.
#[derive(Clone)]
struct AppState {
    workdir: PathBuf,
    market: Arc<Mutex<Vec<MarketApp>>>,
    users: Arc<AppStateUsers>,
    /// The currently active "generate app" skill prompt.
    prompt: Arc<Mutex<String>>,
    /// Metadata persistence layer (local SQLite by default, switchable to PostgreSQL; degrades to no persistence on connection failure).
    db: db::Database,
    /// Artifact storage abstraction (local / COS / OSS).
    storage: Arc<dyn storage::Storage>,
    /// JWT signing secret.
    auth_secret: String,
    /// Whether database persistence is enabled (determines whether registration/multi-user is available).
    auth_enabled: bool,
    /// Initial admin username (used for login in dev mode).
    admin_username: String,
    /// Initial admin password (used for login in dev mode).
    admin_password: String,
    /// Resolved `moon` executable path (ensured available at startup, may be auto-installed).
    moon_bin: PathBuf,
    /// Anonymous event reporting (telemetry) client (received by the closed-source Pro service).
    telemetry: telemetry::Telemetry,
}

impl AppState {
    /// Sync the in-memory hot cache with the database before each request.
    ///
    /// Historical problem: the market/user lists always read the in-memory cache (loaded only once at
    /// startup); after multiple instances or restarts, each process's cache is independent and out of
    /// sync with the database, so the admin "user management" could not see users registered on other
    /// instances and disable operations were invisible across instances. Now, before each request the
    /// cache is refreshed from the database first, so any instance sees consistent, freshly committed
    /// data (the database remains the single source of truth).
    async fn refresh(&self) {
        // Do not touch the cache when the database is unavailable (degraded single-admin mode),
        // to avoid clearing seed data
        if !self.auth_enabled {
            return;
        }
        if let Some(apps) = self.db.load_apps().await {
            *self.market.lock().await = apps;
        }
        if let Some(users) = self.db.load_users().await {
            *self.users.lock().await = users;
        }
        if let Some(p) = self.db.load_prompt().await {
            if !p.is_empty() {
                *self.prompt.lock().await = p;
            }
        }
    }
}

/// User list wrapped in its own type for a shared lock.
type AppStateUsers = Mutex<Vec<AppUser>>;

/// Generate request body.
#[derive(Deserialize)]
struct GenerateRequest {
    description: String,
    #[serde(default)]
    template: String,
    /// Whether to try invoking `moon` to compile into `.aiapp` (default true).
    #[serde(default = "default_true")]
    build: bool,
    /// Generation mode: "new" create new (default); "update" update an existing app.
    #[serde(default = "default_mode")]
    mode: String,
    /// In update mode, the id of the app to update (must be owned by the current user).
    #[serde(default)]
    target_id: String,
    /// App category (chosen at creation, used for category-based auto review).
    #[serde(default)]
    category: String,
    /// Share scope: "private" private (default) / "org" org-shared / "public" publicly shared.
    #[serde(default)]
    share: String,
}

fn default_mode() -> String {
    "new".to_string()
}

fn default_true() -> bool {
    true
}

/// Generate/build response.
#[derive(Serialize)]
struct GenerateResponse {
    ok: bool,
    source: String,
    manifest: serde_json::Value,
    project_dir: String,
    build_result: Option<String>,
    error: Option<String>,
    app: Option<MarketApp>,
}

/// Template list response.
#[derive(Serialize)]
struct TemplatesResponse {
    templates: Vec<TemplateInfo>,
}

#[derive(Serialize)]
struct TemplateInfo {
    name: &'static str,
    description: &'static str,
    image: String,
}

/// Market list response.
#[derive(Serialize)]
struct MarketResponse {
    apps: Vec<MarketApp>,
    tags: Vec<String>,
    platforms: Vec<String>,
}

/// My apps list response.
#[derive(Serialize)]
struct MyAppsResponse {
    apps: Vec<MarketApp>,
    /// Parallel to apps: whether the app id is owned by the current user (true owned / false only launched, uninstallable).
    mine: Vec<bool>,
}

/// Uninstall request body.
#[derive(Deserialize)]
struct UninstallRequest {
    id: String,
}

/// Publish/update request body.
#[derive(Deserialize)]
struct PublishRequest {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    platforms: Vec<String>,
    /// App category (adjustable at publish time, used for category-based auto review).
    #[serde(default)]
    category: String,
    /// Share scope: "private" private (unshare) / "org" org-shared / "public" publicly shared.
    #[serde(default)]
    share: String,
    /// App type: "app" (regular) / "web" (webpage-style, shown standalone without the shell).
    #[serde(default)]
    kind: String,
    /// Hide the platform branding bar on the standalone page (paid feature).
    #[serde(default)]
    hide_branding: bool,
}

/// Action result response.
#[derive(Serialize)]
struct ActionResponse {
    ok: bool,
    error: Option<String>,
    app: Option<MarketApp>,
}

/// App detail response.
#[derive(Serialize)]
struct AppDetailResponse {
    ok: bool,
    app: Option<MarketApp>,
    mock_content: Option<String>,
    error: Option<String>,
}

/// App categories (chosen when creating an app).
pub const APP_CATEGORIES: &[(&str, &str)] = &[
    ("tool", "Utilities"),
    ("office", "Office"),
    ("entertainment", "Entertainment"),
    ("game", "Games"),
    ("family", "Family & Kids"),
    ("life", "Life & Health"),
    ("industry", "Industry"),
    ("other", "Other"),
];

/// Categories requiring manual review (tool/office auto-approve; the rest are reviewed by content risk).
pub fn category_requires_review(cat: &str) -> bool {
    matches!(
        cat,
        "entertainment" | "game" | "family" | "life" | "industry" | "other"
    )
}

/// Derive the default category from the template (used when no category is explicitly chosen).
pub fn default_category_for(template: &str) -> String {
    match template {
        "memory-game" => "game",
        "tv-movies" => "entertainment",
        "pomodoro" => "life",
        _ => "tool",
    }
    .to_string()
}

/// Pre-seeded sample apps (official examples, owned by "official").
fn seed_market_apps() -> Vec<MarketApp> {
    fn app(
        id: &str, name: &str, description: &str, tags: &[&str], platforms: &[&str],
        template: &str, version: &str, owner: &str, visibility: &str,
        category: &str, net: &str,
    ) -> MarketApp {
        MarketApp {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            platforms: platforms.iter().map(|s| s.to_string()).collect(),
            template: template.into(),
            source: String::new(),
            created_at: "2026-08-18 10:00".into(),
            version: version.into(),
            owner: owner.into(),
            visibility: visibility.into(),
            status: if visibility == "public" { "published".into() } else { "draft".into() },
            launches: rand_launches(1000, 50000),
            report: String::new(),
            review_note: String::new(),
            wasm: None,
            tier: "open".into(),
            category: category.into(),
            share: "public".into(),
            net: net.into(),
            kind: "app".into(),
            hide_branding: false,
        }
    }
    vec![
        app("quit-smoking", "Quit Smoking Tracker",
            "A daily quit-smoking companion that tracks smoke-free time, money saved, and health recovery milestones",
            &["Health", "Habit", "Self-Improvement"], &["Web", "Mobile", "Desktop", "macOS", "Windows", "HarmonyOS"],
            "pomodoro", "1.0.0", "official", "public", "life", "local"),
        app("learn-english", "Daily English Learning",
            "A daily English learning app with categorized vocabulary, sentence patterns, and listening exercises",
            &["Education", "Language", "Family"], &["Web", "Mobile", "Desktop", "Tablet", "HarmonyOS"],
            "tv-movies", "1.0.0", "official", "public", "family", "local"),
        app("emergency-room", "Emergency Command Center",
            "A personal emergency preparedness dashboard organizing critical information for crisis situations",
            &["Safety", "Utility", "Family"], &["Web", "Mobile", "Desktop", "Car", "HarmonyOS"],
            "minimal", "1.0.0", "official", "public", "tool", "local"),
        app("project-hub", "Project Management Hub",
            "A full-featured local project management tool with kanban-style task boards, priority labels, and progress tracking",
            &["Office", "Productivity", "Collaboration"], &["Web", "Mobile", "Desktop", "macOS", "Windows", "HarmonyOS"],
            "todo", "1.0.0", "official", "public", "office", "local"),
        app("car-sound", "Car Engine Sound Simulator",
            "An interactive car engine sound simulator with profiles from supercars, muscle cars, JDM, and classic cars",
            &["Entertainment", "Car", "Fun"], &["Car", "Mobile", "Desktop", "Web"],
            "minimal", "1.0.0", "official", "public", "entertainment", "local"),
        app("screensaver-calligraphy", "Calligraphy Screensaver",
            "A dynamic TV screensaver displaying beautiful Chinese calligraphy artworks with smooth transitions",
            &["Entertainment", "Art", "TV", "Screensaver"], &["TV Box", "Car", "Desktop", "HarmonyOS"],
            "minimal", "1.0.0", "official", "public", "entertainment", "local"),
        app("screensaver-art", "Art Gallery Screensaver",
            "An elegant TV screensaver showcasing world-famous paintings with Ken Burns transitions",
            &["Entertainment", "Art", "TV", "Screensaver"], &["TV Box", "Car", "Desktop", "Tablet", "HarmonyOS"],
            "minimal", "1.0.0", "official", "public", "entertainment", "local"),
        app("screensaver-landscape", "Landscape Screensaver",
            "A breathtaking nature landscape screensaver with mountains, oceans, forests, and waterfalls",
            &["Entertainment", "Nature", "TV", "Screensaver"], &["TV Box", "Car", "Desktop", "HarmonyOS"],
            "minimal", "1.0.0", "official", "public", "entertainment", "local"),
        app("screensaver-anime", "Anime World Screensaver",
            "A vibrant anime-themed screensaver with Studio Ghibli and Makoto Shinkai inspired landscapes",
            &["Entertainment", "Anime", "TV", "Screensaver"], &["TV Box", "Car", "Mobile", "Desktop", "HarmonyOS"],
            "minimal", "1.0.0", "official", "public", "entertainment", "local"),
        app("screensaver-funny", "Funny Vibes Screensaver",
            "A lighthearted humorous screensaver with wholesome memes, funny animal photos, and clever puns",
            &["Entertainment", "Fun", "TV", "Screensaver", "Family"], &["TV Box", "Car", "Mobile", "Desktop", "HarmonyOS"],
            "minimal", "1.0.0", "official", "public", "entertainment", "local"),
        app("level-runner", "Level Runner Challenge",
            "A progressively difficult reaction game with 10 levels — Level 1 is easy, Level 10 (The Wall) is brutally hard",
            &["Game", "Challenge", "Fun", "Reaction"], &["Web", "Mobile", "Desktop", "Tablet", "TV Box", "Car"],
            "memory-game", "1.0.0", "official", "public", "game", "local"),
        app("puzzle-master", "Puzzle Maze Master",
            "A brain-teasing puzzle game with 20 levels from simple pattern matching to brutal combinatorial challenges",
            &["Game", "Puzzle", "Brain", "Challenge"], &["Web", "Mobile", "Desktop", "Tablet", "TV Box", "Car"],
            "memory-game", "1.0.0", "official", "public", "game", "local"),
        app("daily-ledger", "Daily Expense Ledger",
            "A personal daily expense tracker with categorized spending, monthly budgets, and visual charts",
            &["Finance", "Life", "Productivity"], &["Web", "Mobile", "Desktop", "macOS", "Windows", "HarmonyOS"],
            "todo", "1.0.0", "official", "public", "life", "local"),
        app("habit-forge", "Habit Forge",
            "A habit-building app with streak tracking, strength meter, and weekly completion charts",
            &["Habit", "Productivity", "Self-Improvement", "Health"], &["Web", "Mobile", "Desktop", "macOS", "Windows", "HarmonyOS"],
            "pomodoro", "1.0.0", "official", "public", "life", "local"),
        app("water-reminder", "Water Reminder",
            "A hydration tracking app with daily goal, smart interval reminders, and progress ring",
            &["Health", "Habit", "Life"], &["Web", "Mobile", "Desktop", "Car", "Wearable", "HarmonyOS"],
            "pomodoro", "1.0.0", "official", "public", "life", "local"),
        app("weight-tracker", "Weight Tracker",
            "A weight management tracker with trend charts, BMI calculator, goal setting, and milestone celebrations",
            &["Health", "Fitness", "Life"], &["Web", "Mobile", "Desktop", "macOS", "Windows", "HarmonyOS"],
            "tv-movies", "1.0.0", "official", "public", "life", "local"),
        app("reading-list", "Reading List",
            "A personal book reading tracker with categories, progress, ratings, and yearly reading goals",
            &["Reading", "Education", "Life", "Family"], &["Web", "Mobile", "Desktop", "Tablet", "HarmonyOS"],
            "tv-movies", "1.0.0", "official", "public", "family", "local"),
        app("workout-log", "Workout Log",
            "A comprehensive workout logging app with sets, reps, weight tracking, and progressive overload charts",
            &["Fitness", "Health", "Life"], &["Web", "Mobile", "Desktop", "Wearable", "HarmonyOS"],
            "todo", "1.0.0", "official", "public", "life", "local"),
        app("sleep-tracker", "Sleep Tracker",
            "A sleep quality tracker with bedtime logging, consistency tracking, and sleep debt calculator",
            &["Health", "Sleep", "Life"], &["Web", "Mobile", "Desktop", "Wearable", "HarmonyOS"],
            "pomodoro", "1.0.0", "official", "public", "life", "local"),
        app("personal-journal", "Personal Journal",
            "A private digital journal with mood tracking, calendar heatmap, and yearly self-reflection summary",
            &["Writing", "Life", "Mental Health", "Self-Improvement"], &["Web", "Mobile", "Desktop", "macOS", "Windows", "HarmonyOS"],
            "tv-movies", "1.0.0", "official", "public", "life", "local"),
    ]
}

/// Generate a stable pseudo-random launch count for seed apps.
fn rand_launches(min: u64, max: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    min + (now + min * 7) % (max - min)
}

/// Simple generated user id (e.g. u_xxxxxx).
fn gen_user_id() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("u_{:x}", n % 0xFFFFFFFFFFFF)
}

/// Build the router.
pub async fn router() -> Router {
    // Runtime config (database connection / storage backend / auth / listen address)
    let cfg = config::AppConfig::from_env();
    // Metadata persistence layer (local SQLite by default, switchable to PostgreSQL via DATABASE_URL)
    let db = db::Database::init(&cfg).await;
    // Production deployment: if a database is required but unreachable, exit immediately and let the
    // orchestrator retry, to avoid silently degrading to no persistence and losing data.
    if cfg.require_db && !db.enabled() {
        eprintln!(
            "[db] Fatal: REQUIRE_DB=true but no database connection; exiting. Check DATABASE_URL (PostgreSQL or SQLite) and availability."
        );
        std::process::exit(1);
    }
    // Artifact storage abstraction (local / COS / OSS)
    let storage = storage::build(&cfg);
    println!(
        "[storage] Artifact storage backend: {} ({})",
        storage.scheme(),
        if storage.scheme() == "local" {
            format!("local directory {}", cfg.storage_local_root.display())
        } else {
            "cloud object storage".to_string()
        }
    );

    // App project build directory: placed under the storage root so source dirs don't scatter around
    let workdir = cfg.storage_local_root.join("projects");
    std::fs::create_dir_all(&workdir).expect("Failed to create working directory");

    // In-memory hot cache
    let mut apps = seed::load_seed_apps();
    let mut users: Vec<AppUser> = Vec::new();
    let mut prompt = aiapp_gen::default_system_prompt().to_string();

    let auth_enabled = db.enabled();

    if auth_enabled {
        // Database mode: load from DB, seed if empty and persist
        if let Some(loaded) = db.load_apps().await {
            if !loaded.is_empty() {
                // Merge: keep existing DB apps + add any new seed apps not yet in DB
                let existing_ids: std::collections::HashSet<String> =
                    loaded.iter().map(|a| a.id.clone()).collect();
                for seed in &apps {
                    if !existing_ids.contains(&seed.id) {
                        db.save_app(seed).await;
                    }
                }
                apps = loaded;
            }
        }
        if let Some(loaded) = db.load_users().await {
            users = loaded;
        }
        if let Some(loaded) = db.load_prompt().await {
            if !loaded.is_empty() {
                prompt = loaded;
            }
        }
        // Ensure the admin account exists (first-time write)
        ensure_admin(&db, &cfg, &mut users).await;
        // Persist seed apps on first run so restarts stay consistent
        for a in &apps {
            db.save_app(a).await;
        }
        db.save_prompt(&prompt).await;
    } else {
        // Degraded mode when the database is unavailable (connection/migration failure): single admin, no registration
        eprintln!(
            "[auth] Database unavailable; degrading to single-admin dev mode, registration disabled.\n\
             \tPlease check the DATABASE_URL config (PostgreSQL or SQLite)."
        );
        users.push(AppUser {
            id: "u_admin".into(),
            name: cfg.admin_username.clone(),
            role: "admin".into(),
            status: "active".into(),
            apps_generated: 0,
            launches: 0,
            incentive: 0,
            org: "main".into(),
            created_at: "2026-08-18".into(),
            installed: Vec::new(),
            password_hash: auth::hash_password(&cfg.admin_password),
        });
    }

    let market = Arc::new(Mutex::new(apps));
    let users = Arc::new(Mutex::new(users));
    let prompt = Arc::new(Mutex::new(prompt));
    // Ensure the MoonBit toolchain is available at startup (auto-install if missing), for pre-builds and generation
    let moon_bin = moon::ensure_moon_toolchain();
    // Anonymous event reporting (telemetry) client: reports asynchronously to the closed-source Pro service when AIAPP_PRO_URL is set
    let telemetry = telemetry::from_env();
    let state = Arc::new(AppState {
        workdir,
        market,
        users,
        prompt,
        db,
        storage,
        auth_secret: cfg.auth_secret.clone(),
        auth_enabled,
        admin_username: cfg.admin_username.clone(),
        admin_password: cfg.admin_password.clone(),
        moon_bin: moon_bin.clone(),
        telemetry,
    });

    // Startup pre-build: compile main.wasm for seed samples without artifacts and write them to the
    // storage backend (local directory / COS per config). The first run blocks until done (concurrent
    // builds), so sample apps are immediately usable after deploy and the build environment (moon
    // toolchain) is validated as a bonus. Missing toolchain or compile failures only warn, never block
    // the HTTP service. On later restarts, existing artifacts are skipped automatically (idempotent).
    {
        let seed_to_build: Vec<MarketApp> = {
            let m = state.market.lock().await;
            m.iter().filter(|a| a.wasm.is_none()).cloned().collect()
        };
        let n = seed_to_build.len();
        if n > 0 {
            eprintln!("[seed] Pre-building {n} samples at startup (validating moon toolchain); first run may take a while ...");
            let mut set = tokio::task::JoinSet::new();
            for app in seed_to_build {
                let st = state.clone();
                set.spawn(async move {
                    build_and_store_wasm(&st, &app).await
                });
            }
            let mut ok = 0usize;
            let mut fail = 0usize;
            while let Some(r) = set.join_next().await {
                match r {
                    Ok(true) => ok += 1,
                    Ok(false) => fail += 1,
                    Err(e) => {
                        eprintln!("[seed] Pre-build task failed: {e}");
                        fail += 1;
                    }
                }
            }
            eprintln!("[seed] Pre-build done: ok {ok} / failed {fail}");
            if fail > 0 {
                eprintln!("[seed] Some samples failed to compile; please check the moon toolchain and template sources. The service still starts normally.");
            }
        }
    }

    // Template image directory: defaults to ./templates relative to CWD, overridable via STATIC_DIR (/app/templates in containers)
    let static_dir = cfg.static_dir.clone();
    let serve_static = ServeDir::new(static_dir);

    Router::new()
        .route("/", get(serve_index))
        // —— Separate admin site (admin console lives on its own page, not in the user UI) ——
        .route("/admin", get(serve_admin))
        .route("/api/templates", get(list_templates))
        // —— Auth ——
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/me", get(auth_me))
        // —— Market / detail (public) ——
        .route("/api/market", get(list_market))
        .route("/api/app/:id", get(app_detail))
        .route("/api/app/:id/wasm", get(app_wasm))
        // —— Storage presign (requires login; returns pre-authorized URLs for COS and other cloud storage) ——
        .route("/api/storage/presign", get(storage_presign))
        // —— Requires login ——
        .route("/api/generate", post(generate))
        .route("/api/my-apps", get(list_my_apps))
        .route("/api/publish", post(publish_app))
        .route("/api/report", post(report_app))
        .route("/api/delete", post(delete_app))
        .route("/api/uninstall", post(uninstall_app))
        // —— Admin (requires admin role) ——
        .route("/api/admin/users", get(admin_users))
        .route("/api/admin/user/toggle", post(admin_user_toggle))
        .route("/api/admin/apps", get(admin_apps))
        .route("/api/admin/app/review", post(admin_app_review))
        .route("/api/admin/app/status", post(admin_app_status))
        .route("/api/admin/app/edit", post(admin_app_edit))
        .route("/api/admin/app/delete", post(admin_app_delete))
        .route("/api/admin/stats", get(admin_stats))
        .route("/api/admin/prompt", get(admin_prompt))
        .route("/api/admin/prompt", post(admin_prompt_save))
        .route("/api/admin/prompt/reset", post(admin_prompt_reset))
        // —— Static ——
        .route("/static/sql-wasm.js", get(sql_wasm_js))
        .route("/static/sql-wasm.wasm", get(sql_wasm_wasm))
        .nest_service("/static/templates", serve_static)
        // Unmatched non-API paths fall back to the SPA home page to avoid blank screens
        .fallback(spa_fallback)
        // Refresh the in-memory cache from the database on every request, keeping data consistent across instances/restarts
        .layer(middleware::from_fn_with_state(state.clone(), refresh_cache))
        .with_state(state)
}

/// Refresh-cache middleware before each request: see [`AppState::refresh`]. Static assets carry no business data, skip.
async fn refresh_cache(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    if !req.uri().path().starts_with("/static/") {
        state.refresh().await;
    }
    next.run(req).await
}

/// SPA fallback: unmatched page paths return the home HTML; `/admin/...` paths return the separate
/// admin console page; unmatched API paths still return 404 to avoid disguising API errors as pages.
async fn spa_fallback(uri: Uri) -> Response {
    let path = uri.path();
    if path.starts_with("/api/") {
        return (StatusCode::NOT_FOUND, "Not Found").into_response();
    }
    if path == "/admin" || path.starts_with("/admin/") {
        return serve_admin().await.into_response();
    }
    serve_index().await.into_response()
}

/// Ensure the admin account exists (database mode).
async fn ensure_admin(db: &db::Database, cfg: &config::AppConfig, users: &mut Vec<AppUser>) {
    if let Some(existing) = db.find_user_by_name(&cfg.admin_username).await {
        if !users.iter().any(|u| u.id == existing.id) {
            users.push(existing);
        }
        return;
    }
    let u = AppUser {
        id: gen_user_id(),
        name: cfg.admin_username.clone(),
        role: "admin".into(),
        status: "active".into(),
        apps_generated: 0,
        launches: 0,
        incentive: 0,
        org: "main".into(),
        created_at: chrono_now(),
        installed: Vec::new(),
        password_hash: auth::hash_password(&cfg.admin_password),
    };
    db.save_user(&u).await;
    users.push(u);
}

/// Service home page (embedded user-facing frontend).
async fn serve_index() -> Response {
    no_cache_response(include_str!("index.html"))
}

/// Separate admin console page (embedded; served at `/admin`, independent of the user UI).
async fn serve_admin() -> Response {
    no_cache_response(include_str!("admin.html"))
}

/// HTML responses are embedded at compile time; always serve fresh so the browser never
/// keeps a stale copy after a redeploy (no heuristic/cache control).
fn no_cache_response(body: &'static str) -> Response {
    let mut res = Html(body).into_response();
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-cache, no-store, must-revalidate"),
    );
    res
}

/// List available templates.
async fn list_templates() -> Json<TemplatesResponse> {
    let templates = TEMPLATES
        .iter()
        .map(|(name, description)| TemplateInfo {
            name,
            description,
            image: format!("/static/templates/{}.png", name),
        })
        .collect();
    Json(TemplatesResponse { templates })
}

/// Get the app market list: public apps + org-shared apps of the same org + the current user's own apps.
/// Share links (`?app=id`) also load the corresponding app detail normally.
async fn list_market(
    State(state): State<Arc<AppState>>,
    auth: auth::OptionalAuth,
) -> Json<MarketResponse> {
    let all = state.market.lock().await.clone();
    let users = state.users.lock().await.clone();
    let my_id = auth.0.as_ref().map(|a| a.id.clone());
    let my_org = auth
        .0
        .as_ref()
        .and_then(|a| users.iter().find(|u| u.id == a.id).map(|u| u.org.clone()));
    let owner_org = |owner: &str| -> Option<String> {
        users.iter().find(|u| u.id == owner).map(|u| u.org.clone())
    };
    let apps: Vec<MarketApp> = all
        .into_iter()
        .filter(|a| {
            // Apps you own: visible regardless of private / draft / under review (personal-center view)
            if Some(a.owner.as_str()) == my_id.as_deref() {
                return true;
            }
            // Published: public apps are visible to everyone
            if a.status == "published" && a.share == "public" {
                return true;
            }
            // Org-shared: visible to same-org members (including org apps under review, so they can be evaluated before use)
            if a.share == "org" {
                if let Some(org) = &my_org {
                    if owner_org(&a.owner).as_deref() == Some(org.as_str()) {
                        return true;
                    }
                }
            }
            false
        })
        .collect();
    let mut tags: Vec<String> = apps.iter().flat_map(|a| a.tags.clone()).collect();
    tags.sort();
    tags.dedup();
    let mut platforms: Vec<String> = apps.iter().flat_map(|a| a.platforms.clone()).collect();
    platforms.sort();
    platforms.dedup();
    Json(MarketResponse { apps, tags, platforms })
}

/// My apps list (owned by the current logged-in user, or launched and recorded in "My Apps").
/// `mine` is parallel to `apps`: true means owned (manageable), false means only launched (uninstallable).
async fn list_my_apps(
    State(state): State<Arc<AppState>>,
    auth: auth::AuthUser,
) -> Json<MyAppsResponse> {
    let all = state.market.lock().await.clone();
    let users = state.users.lock().await;
    let installed = users
        .iter()
        .find(|u| u.id == auth.id)
        .map(|u| u.installed.clone())
        .unwrap_or_default();
    drop(users);
    let (apps, mine): (Vec<MarketApp>, Vec<bool>) = all
        .into_iter()
        .filter(|a| a.owner == auth.id || installed.contains(&a.id))
        .map(|a| {
            let is_mine = a.owner == auth.id;
            (a, is_mine)
        })
        .unzip();
    Json(MyAppsResponse { apps, mine })
}

/// Uninstall an app: remove it from the current user's "My Apps" (launch records).
/// Only affects apps "launched but not owned"; for your own apps use the delete endpoint.
async fn uninstall_app(
    State(state): State<Arc<AppState>>,
    auth: auth::AuthUser,
    Json(req): Json<UninstallRequest>,
) -> Json<ActionResponse> {
    let mut users = state.users.lock().await;
    let removed = match users.iter_mut().find(|u| u.id == auth.id) {
        Some(u) => {
            let before = u.installed.len();
            u.installed.retain(|id| id != &req.id);
            u.installed.len() != before
        }
        None => false,
    };
    if !removed {
        return Json(ActionResponse {
            ok: false,
            error: Some("The app is not in your \"My Apps\"".into()),
            app: None,
        });
    }
    let u = users.iter().find(|u| u.id == auth.id).cloned();
    drop(users);
    if let Some(u) = u {
        state.db.save_user(&u).await;
    }
    Json(ActionResponse { ok: true, error: None, app: None })
}

/// Publish an app: supplement basic info and make it public on the market.
async fn publish_app(
    State(state): State<Arc<AppState>>,
    auth: auth::AuthUser,
    Json(req): Json<PublishRequest>,
) -> Json<ActionResponse> {
    let mut market = state.market.lock().await;
    let idx = match market.iter().position(|a| a.id == req.id && a.owner == auth.id) {
        Some(i) => i,
        None => {
            return Json(ActionResponse {
                ok: false,
                error: Some("App not found or you have no permission".into()),
                app: None,
            })
        }
    };
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Json(ActionResponse {
            ok: false,
            error: Some("Please enter an app name".into()),
            app: None,
        });
    }
    let app = &mut market[idx];
    app.name = name;
    app.description = req.description.trim().to_string();
    app.tags = req.tags.iter().filter(|s| !s.is_empty()).cloned().collect();
    app.platforms = req.platforms.iter().filter(|s| !s.is_empty()).cloned().collect();
    if app.platforms.is_empty() {
        app.platforms = vec!["Web".into(), "Mobile".into(), "Desktop".into()];
    }
    // Category: explicit choice takes priority, otherwise derived from the template
    let category = {
        let c = req.category.trim().to_string();
        if APP_CATEGORIES.iter().any(|(k, _)| *k == c) {
            c
        } else {
            default_category_for(&app.template)
        }
    };
    app.category = category.clone();
    // Share scope: private unshares / org org-shared / public publicly shared
    let share = match req.share.trim() {
        "org" => "org",
        "public" => "public",
        _ => "private",
    };
    app.share = share.into();
    // App type: "web" marks a webpage-style app (standalone page); everything else stays a regular app.
    app.kind = if req.kind.trim() == "web" { "web".into() } else { "app".into() };
    app.hide_branding = req.hide_branding;

    // Category-based auto review: tool/office categories auto-approve, the rest enter manual review by content risk.
    // Public sharing is only visible on the market after approval; org sharing is visible within the org after approval.
    if share == "private" {
        // Unshared / private: owner-only, back to draft
        app.visibility = "private".into();
        app.status = "draft".into();
        app.review_note.clear();
    } else if category_requires_review(&category) {
        app.visibility = "private".into();
        app.status = "reviewing".into();
        app.review_note = "Submitted for review; awaiting admin approval".into();
    } else {
        app.status = "published".into();
        app.visibility = if share == "public" { "public".into() } else { "private".into() };
        app.review_note = "Auto-approved (tool/office category)".into();
    }
    let cloned = app.clone();
    state.db.save_app(&cloned).await;
    // Telemetry: app publish/share (anonymous, type count + share scope only)
    if share != "private" {
        state.telemetry.report("app_publish", json!({ "share": share }));
    }
    Json(ActionResponse { ok: true, error: None, app: Some(cloned) })
}

/// Delete an app (only your own apps).
async fn delete_app(
    State(state): State<Arc<AppState>>,
    auth: auth::AuthUser,
    Json(req): Json<PublishRequest>,
) -> Json<ActionResponse> {
    let mut market = state.market.lock().await;
    match market.iter().position(|a| a.id == req.id && a.owner == auth.id) {
        Some(i) => {
            let removed = market.remove(i);
            state.db.delete_app(&req.id).await;
            Json(ActionResponse { ok: true, error: None, app: Some(removed) })
        }
        None => Json(ActionResponse {
            ok: false,
            error: Some("App not found or you have no permission".into()),
            app: None,
        }),
    }
}

/// Language preference query parameter for localized content.
#[derive(Deserialize)]
struct LangQuery {
    lang: Option<String>,
}

/// Get app details (called when opening an app). Public apps are accessible to anyone; org-shared apps
/// require same-org membership; private apps are accessible only to the owner and admins.
/// The `lang` query parameter (e.g. `zh-CN`) localizes the mock content for the app window.
async fn app_detail(
    State(state): State<Arc<AppState>>,
    auth: auth::OptionalAuth,
    Path(id): Path<String>,
    Query(params): Query<LangQuery>,
) -> Json<AppDetailResponse> {
    let users = state.users.lock().await.clone();
    let my_id = auth.0.as_ref().map(|a| a.id.clone());
    let my_role = auth.0.as_ref().map(|a| a.role.clone());
    let my_org = auth
        .0
        .as_ref()
        .and_then(|a| users.iter().find(|u| u.id == a.id).map(|u| u.org.clone()));
    let owner_org = |owner: &str| -> Option<String> {
        users.iter().find(|u| u.id == owner).map(|u| u.org.clone())
    };

    let app = {
        let mut market = state.market.lock().await;
        match market.iter_mut().find(|a| a.id == id) {
            Some(a) => {
                // Access control
                let is_owner = Some(a.owner.as_str()) == my_id.as_deref();
                let is_admin = my_role.as_deref() == Some("admin");
                let is_public = a.share == "public"
                    && (a.status == "published" || a.status == "disabled");
                let is_org = a.share == "org"
                    && my_org.is_some()
                    && owner_org(&a.owner).as_deref() == my_org.as_deref();
                if !is_owner && !is_admin && !is_public && !is_org {
                    return Json(AppDetailResponse {
                        ok: false,
                        app: None,
                        mock_content: None,
                        error: Some("No access to this app (not public and not yours)".into()),
                    });
                }
                a.launches += 1;
                {
                    let mut users = state.users.lock().await;
                    if let Some(u) = users.iter_mut().find(|u| u.id == a.owner) {
                        u.launches += 1;
                    }
                    // Launched apps are recorded in the launching user's "My Apps" (logged-in user, deduplicated)
                    if let Some(uid) = &my_id {
                        if let Some(u) = users.iter_mut().find(|u| u.id == *uid) {
                            if !u.installed.contains(&a.id) {
                                u.installed.push(a.id.clone());
                            }
                        }
                    }
                }
                a.clone()
            }
            None => {
                return Json(AppDetailResponse {
                    ok: false,
                    app: None,
                    mock_content: None,
                    error: Some("App not found".into()),
                })
            }
        }
    };
    state.db.save_app(&app).await;
    {
        let users = state.users.lock().await;
        if let Some(u) = users.iter().find(|u| u.id == app.owner) {
            state.db.save_user(u).await;
        }
        // If the launcher is not the owner, also persist their "My Apps" record
        if let Some(uid) = &my_id {
            if *uid != app.owner {
                if let Some(u) = users.iter().find(|u| u.id == *uid) {
                    state.db.save_user(u).await;
                }
            }
        }
    }
    // Telemetry: app launch (anonymous, type count only)
    state.telemetry.report("app_launch", json!({ "template": app.template }));
    let mock_content = Some(mock_app_content(&app.template, &app.name, params.lang.as_deref().unwrap_or("")));
    Json(AppDetailResponse { ok: true, app: Some(app), mock_content, error: None })
}

/// Generate mock app content for display based on the template.
/// Generate localized mock app content for display based on the template.
/// `lang` starting with "zh" returns the Chinese variant; everything else returns English.
fn mock_app_content(template: &str, name: &str, lang: &str) -> String {
    let zh = lang.to_ascii_lowercase().starts_with("zh");
    if zh {
        match template {
            "todo" => format!(
                "应用「{}」已启动 [待办事项]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 📋 我的待办              +  │\n\
                 ├─────────────────────────────┤\n\
                 │ ☑ 完成周报              ✓  │\n\
                 │ ☐ 准备会议材料          ○  │\n\
                 │ ☐ 回复客户邮件          ○  │\n\
                 │ ☐ 更新项目进度          ○  │\n\
                 │ ☐ 团队代码审查          ○  │\n\
                 └─────────────────────────────┘\n\n\
                 点击 ○ 勾选完成，按 + 添加新事项",
                name
            ),
            "image-filter" => format!(
                "应用「{}」已启动 [图片滤镜]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🖼 图片滤镜              ⚙  │\n\
                 ├─────────────────────────────┤\n\
                 │                             │\n\
                 │    [ 点击选择图片 ]         │\n\
                 │                             │\n\
                 ├─────────────────────────────┤\n\
                 │ 原图 │ 灰度 │ 暖色 │ 冷色 │\n\
                 └─────────────────────────────┘\n\n\
                 选择滤镜效果，实时预览处理结果",
                name
            ),
            "pomodoro" => format!(
                "应用「{}」已启动 [专注番茄钟]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🍅 专注番茄钟          ▶   │\n\
                 ├─────────────────────────────┤\n\
                 │          25:00              │\n\
                 │    ⏳ 专注时间 · 第 1 个     │\n\
                 ├─────────────────────────────┤\n\
                 │ 今日已完成 0 个 · 累计 0 分  │\n\
                 └─────────────────────────────┘\n\n\
                 工作 25 分钟再休息 5 分钟，专注记录自动保存",
                name
            ),
            "memory-game" => format!(
                "应用「{}」已启动 [亲子记忆翻牌]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🃏 记忆翻牌        🏆 LPS   │\n\
                 ├─────────────────────────────┤\n\
                 │ ◽ ◽ ◽ ◽ ◽ ◽ ◽ ◽          │\n\
                 │ ◽ ◽ ◽ ◽ ◽ ◽ ◽ ◽          │\n\
                 │ ◽ ◽ ◽ ◽ ◽ ◽ ◽ ◽          │\n\
                 │ ◽ ◽ ◽ ◽ ◽ ◽ ◽ ◽          │\n\
                 ├─────────────────────────────┤\n\
                 │ 步数 0 · 配对 0/16           │\n\
                 └─────────────────────────────┘\n\n\
                 翻牌找出成对的图案，看看你的最短纪录！",
                name
            ),
            "tv-movies" => format!(
                "应用「{}」已启动 [家庭影音片单]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🎬 影音片单         ＋收藏   │\n\
                 ├─────────────────────────────┤\n\
                 │ 🎞 电影 · 剧集 · 动画 · 纪录│\n\
                 │ ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔│\n\
                 │ ▶ 影片 A  ★ 8.9  已看       │\n\
                 │ ▶ 影片 B  ★ 9.2  想看       │\n\
                 │ ▶ 影片 C  ★ 8.5  ★收藏      │\n\
                 └─────────────────────────────┘\n\n\
                 分类收藏全家想看的影片，电视/车机大屏友好",
                name
            ),
            "timestamp" => format!(
                "应用「{}」已启动 [时间戳互转工具]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🕐 时间戳互转       ⚡ 实时   │\n\
                 ├─────────────────────────────┤\n\
                 │                             │\n\
                 │  📅 时间 → 时间戳            │\n\
                 │  [2024-01-01 00:00:00]      │\n\
                 │  ────────── 点击选择 ──────  │\n\
                 │  ⏎ 转换结果: 1700000000     │\n\
                 │                             │\n\
                 │  📅 时间戳 → 时间            │\n\
                 │  [1700000000    ]            │\n\
                 │  ──────────────────────────  │\n\
                 │  ⏎ 转换结果: 2024-01-01     │\n\
                 │      00:00:00               │\n\
                 │                             │\n\
                 │  🕰 当前时间戳: 1700000000   │\n\
                 └─────────────────────────────┘\n\n\
                 支持时间戳↔时间互转，点击日期选择器快速选择时间",
                name
            ),
            _ => format!(
                "应用「{}」已启动 [Hello World]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 欢迎使用 {}       │\n\
                 ├─────────────────────────────┤\n\
                 │                             │\n\
                 │    ✨ 应用运行中 ✨          │\n\
                 │                             │\n\
                 │    当前版本: 1.0.0          │\n\
                 │    状态: 正常运行           │\n\
                 │                             │\n\
                 └─────────────────────────────┘\n\n\
                 该应用已就绪，可在设定的平台上运行",
                name, name
            ),
        }
    } else {
        match template {
            "todo" => format!(
                "App \"{}\" launched [Todo List]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 📋 My Todos              +  │\n\
                 ├─────────────────────────────┤\n\
                 │ ☑ Finish weekly report   ✓  │\n\
                 │ ☐ Prepare meeting notes  ○  │\n\
                 │ ☐ Reply to client email  ○  │\n\
                 │ ☐ Update project status  ○  │\n\
                 │ ☐ Team code review       ○  │\n\
                 └─────────────────────────────┘\n\n\
                 Tap ○ to check off, press + to add new items",
                name
            ),
            "image-filter" => format!(
                "App \"{}\" launched [Image Filter]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🖼 Image Filter           ⚙  │\n\
                 ├─────────────────────────────┤\n\
                 │                             │\n\
                 │    [ Click to pick image ]  │\n\
                 │                             │\n\
                 ├─────────────────────────────┤\n\
                 │ Original │ Gray │ Warm │ Cold │\n\
                 └─────────────────────────────┘\n\n\
                 Choose a filter to preview the result in real time",
                name
            ),
            "pomodoro" => format!(
                "App \"{}\" launched [Focus Pomodoro]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🍅 Focus Pomodoro        ▶   │\n\
                 ├─────────────────────────────┤\n\
                 │          25:00              │\n\
                 │    ⏳ Focus · Session 1      │\n\
                 ├─────────────────────────────┤\n\
                 │ Today 0 done · 0 min total  │\n\
                 └─────────────────────────────┘\n\n\
                 Work 25 minutes, rest 5 — focus records are saved automatically",
                name
            ),
            "memory-game" => format!(
                "App \"{}\" launched [Kids Memory Match]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🃏 Memory Match       🏆 LPS │\n\
                 ├─────────────────────────────┤\n\
                 │ ◽ ◽ ◽ ◽ ◽ ◽ ◽ ◽          │\n\
                 │ ◽ ◽ ◽ ◽ ◽ ◽ ◽ ◽          │\n\
                 │ ◽ ◽ ◽ ◽ ◽ ◽ ◽ ◽          │\n\
                 │ ◽ ◽ ◽ ◽ ◽ ◽ ◽ ◽          │\n\
                 ├─────────────────────────────┤\n\
                 │ Moves 0 · Pairs 0/16         │\n\
                 └─────────────────────────────┘\n\n\
                 Flip cards to find matching pairs — beat your best record!",
                name
            ),
            "tv-movies" => format!(
                "App \"{}\" launched [Family Movie List]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🎬 Movie List         ＋Save │\n\
                 ├─────────────────────────────┤\n\
                 │ 🎞 Movies · Series · Anime · Docs│\n\
                 │ ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔│\n\
                 │ ▶ Movie A  ★ 8.9  Watched    │\n\
                 │ ▶ Movie B  ★ 9.2  Watchlist  │\n\
                 │ ▶ Movie C  ★ 8.5  ★Saved     │\n\
                 └─────────────────────────────┘\n\n\
                 Browse and save the whole family's watchlist — TV/car friendly",
                name
            ),
            "timestamp" => format!(
                "App \"{}\" launched [Timestamp Converter]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ 🕐 Timestamp Converter ⚡ Live │\n\
                 ├─────────────────────────────┤\n\
                 │                             │\n\
                 │  📅 Time → Timestamp         │\n\
                 │  [2024-01-01 00:00:00]      │\n\
                 │  ────── Click to pick ─────  │\n\
                 │  ⏎ Result: 1700000000       │\n\
                 │                             │\n\
                 │  📅 Timestamp → Time         │\n\
                 │  [1700000000    ]            │\n\
                 │  ──────────────────────────  │\n\
                 │  ⏎ Result: 2024-01-01       │\n\
                 │      00:00:00               │\n\
                 │                             │\n\
                 │  🕰 Now: 1700000000          │\n\
                 └─────────────────────────────┘\n\n\
                 Convert between Unix timestamps and human-readable time;\npick a date with the built-in date picker",
                name
            ),
            _ => format!(
                "App \"{}\" launched [Hello World]\n\n\
                 ┌─────────────────────────────┐\n\
                 │ Welcome to {}         │\n\
                 ├─────────────────────────────┤\n\
                 │                             │\n\
                 │    ✨ App is running ✨      │\n\
                 │                             │\n\
                 │    Version: 1.0.0           │\n\
                 │    Status: running          │\n\
                 │                             │\n\
                 └─────────────────────────────┘\n\n\
                 This app is ready to run on the configured platforms",
                name, name
            ),
        }
    }
}


/// Generate an app project (new or update). Requires login.
async fn generate(
    State(state): State<Arc<AppState>>,
    auth: auth::AuthUser,
    Json(req): Json<GenerateRequest>,
) -> Json<GenerateResponse> {
    let description = req.description.trim().to_string();
    if description.is_empty() {
        return Json(GenerateResponse {
            ok: false,
            source: String::new(),
            manifest: serde_json::Value::Null,
            project_dir: String::new(),
            build_result: None,
            error: Some("Please describe the app you want to build first".into()),
            app: None,
        });
    }

    let template = if req.template.is_empty() {
        "minimal"
    } else {
        &req.template
    };

    // Update mode: validate app ownership first
    let update_target: Option<MarketApp> = if req.mode == "update" {
        if req.target_id.is_empty() {
            return Json(GenerateResponse {
                ok: false,
                source: String::new(),
                manifest: serde_json::Value::Null,
                project_dir: String::new(),
                build_result: None,
                error: Some("Updating an app requires specifying which app to update".into()),
                app: None,
            });
        }
        let market = state.market.lock().await;
        let found = market
            .iter()
            .find(|a| a.id == req.target_id && a.owner == auth.id)
            .cloned();
        match found {
            Some(a) => Some(a),
            None => {
                return Json(GenerateResponse {
                    ok: false,
                    source: String::new(),
                    manifest: serde_json::Value::Null,
                    project_dir: String::new(),
                    build_result: None,
                    error: Some("The app to update was not found, or you are not its owner".into()),
                    app: None,
                })
            }
        }
    } else {
        None
    };

    let slug = aiapp_gen::slugify(&description);
    let dir_name = format!(
        "{}_{}",
        slug,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    );
    let project_dir = state.workdir.join(&dir_name);

    let config = GenConfig::from_env();
    let override_prompt = state.prompt.lock().await.clone();

    let result = match generate_source_with_prompt(&description, &config, template, &override_prompt) {
        Ok(source) => write_project(&project_dir, &description, &source, template)
            .map(|()| source)
            .map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };

    match result {
        Ok(source) => {
            let manifest_path = project_dir.join("aiapp.json");
            let manifest = std::fs::read_to_string(&manifest_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(serde_json::json!({}));

            let mut build_result = None;
            let mut app_wasm: Option<std::path::PathBuf> = None;
            if req.build {
                match moon_build(&project_dir, &state.moon_bin) {
                    Ok((info, aiapp_dir)) => {
                        build_result = Some(info);
                        app_wasm = aiapp_dir.map(|d| d.join("main.wasm"));
                    }
                    Err(e) => build_result = Some(format!("Build skipped: {e}")),
                }
            }

            let now = chrono_now();
            let mut app_result = {
                let mut market = state.market.lock().await;
                if let Some(target) = &update_target {
                    let idx = market
                        .iter()
                        .position(|a| a.id == target.id)
                        .expect("Target app should exist");
                    let app = &mut market[idx];
                    app.source = source.clone();
                    app.description = description.clone();
                    app.template = template.to_string();
                    app.version = bump_version(&app.version);
                    app.created_at = now;
                    app.review_note.clear();
                    if let Some(w) = &app_wasm {
                        app.wasm = Some(w.to_string_lossy().into_owned());
                    }
                    app.clone()
                } else {
                    let name = manifest.get("name")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|| truncate(&description, 16));
                    let app_id = format!("gen_{}", market_len(&market) + 1);
                    // App category: explicit choice takes priority, otherwise derived from the template
                    let category = {
                        let c = req.category.trim().to_string();
                        if APP_CATEGORIES.iter().any(|(k, _)| *k == c) {
                            c
                        } else {
                            default_category_for(template)
                        }
                    };
                    // Share scope: only private / org / public; anything else falls back to private
                    let share = match req.share.trim() {
                        "org" => "org".to_string(),
                        "public" => "public".to_string(),
                        _ => "private".to_string(),
                    };
                    let app_entry = MarketApp {
                        id: app_id.clone(),
                        name,
                        description: description.clone(),
                        tags: vec![],
                        platforms: vec!["Web".into(), "Mobile".into(), "Desktop".into()],
                        template: template.to_string(),
                        source: source.clone(),
                        created_at: now,
                        version: "1.0.0".into(),
                        owner: auth.id.clone(),
                        visibility: "private".into(),
                        // Category-based auto review: tool/office auto-approve, the rest enter manual review
                        status: if category_requires_review(&category) {
                            "reviewing".into()
                        } else {
                            "draft".into()
                        },
                        launches: 0,
                        report: String::new(),
                        review_note: String::new(),
                        wasm: app_wasm.clone().map(|p| p.to_string_lossy().into_owned()),
                        tier: "open".into(),
                        category,
                        share,
                        net: "local".into(),
                        kind: "app".into(),
                        hide_branding: false,
                    };
                    let cloned = app_entry.clone();
                    market.push(app_entry);
                    {
                        let mut users = state.users.lock().await;
                        if let Some(u) = users.iter_mut().find(|u| u.id == auth.id) {
                            u.apps_generated += 1;
                        }
                    }
                    cloned
                }
            };

            if let Some(ref wasm_path) = app_wasm {
                if let Ok(bytes) = std::fs::read(wasm_path) {
                    let key = format!("apps/{}/main.wasm", app_result.id);
                    if state.storage.put(&key, &bytes).await.is_ok() {
                        app_result.wasm = Some(key);
                        let mut market = state.market.lock().await;
                        if let Some(a) = market.iter_mut().find(|a| a.id == app_result.id) {
                            a.wasm = app_result.wasm.clone();
                        }
                    }
                }
            }

            state.db.save_app(&app_result).await;
            {
                let users = state.users.lock().await;
                if let Some(u) = users.iter().find(|u| u.id == app_result.owner) {
                    state.db.save_user(u).await;
                }
            }
            // Telemetry: app generated (anonymous, type count + template only)
            state.telemetry.report(
                "app_generate",
                json!({
                    "template": template,
                    "update": update_target.is_some(),
                }),
            );

            Json(GenerateResponse {
                ok: true,
                source,
                manifest,
                project_dir: dir_name,
                build_result,
                error: None,
                app: Some(app_result),
            })
        }
        Err(e) => Json(GenerateResponse {
            ok: false,
            source: String::new(),
            manifest: serde_json::Value::Null,
            project_dir: String::new(),
            build_result: None,
            error: Some(e),
            app: None,
        }),
    }
}

/// Count the market list length (for generating app ids).
fn market_len(market: &[MarketApp]) -> usize {
    market.len()
}

/// Simple truncation of long strings.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "..."
    }
}

/// Version bump: 1.2.3 -> 1.2.4.
fn bump_version(v: &str) -> String {
    let parts: Vec<&str> = v.trim().split('.').collect();
    let major = parts.first().unwrap_or(&"1").parse::<u32>().unwrap_or(1);
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    format!("{major}.{minor}.{}", patch + 1)
}

/// Get the current time string (server local timezone, format `YYYY-MM-DD HH:MM`).
///
/// chrono's local time is used for user-facing timestamps like registration time and app creation time;
/// local time is more intuitive than UTC (otherwise same-day registrations in the China timezone would show as yesterday).
pub fn chrono_now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// Try to invoke `moon` to compile the project into a `.aiapp` package.
fn moon_build(project_dir: &std::path::Path, moon_bin: &std::path::Path) -> Result<(String, Option<std::path::PathBuf>), String> {
    let config = aiapp_build::BuildConfig {
        target: "wasm-gc".into(),
        release: false,
        package: true,
        output: None,
        moon_bin: moon_bin.to_path_buf(),
    };
    let out = aiapp_build::build(project_dir, &config).map_err(|e| e.to_string())?;
    let bytes = std::fs::metadata(&out.wasm)
        .map(|m| m.len())
        .unwrap_or(0);
    let mut msg = format!("Compiled WASM ({} bytes)", bytes);
    if let Some(aiapp_dir) = &out.aiapp {
        msg.push_str(&format!(", \\.aiapp package: {}", aiapp_dir.display()));
    }
    Ok((msg, out.aiapp.clone()))
}

/// Return the app's compiled artifact `main.wasm`: if an artifact exists, fetch it directly from the
/// storage backend (COS via presigned direct link); otherwise build lazily from the template/source and
/// persist it (reusing the same logic as the startup pre-build).
async fn app_wasm(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let key = format!("apps/{id}/main.wasm");

    // Existing artifact: prefer a presigned direct link (COS and other cloud storage), otherwise fall back to reading local bytes
    let has_artifact = {
        let market = state.market.lock().await;
        market.iter().find(|a| a.id == id).map(|a| a.wasm.is_some()).unwrap_or(false)
    };
    if has_artifact {
        if let Ok(Some(url)) = state.storage.presigned_url(&key, 600).await {
            return redirect_to(url);
        }
        if let Ok(bytes) = state.storage.get_bytes(&key).await {
            return serve_wasm_bytes(bytes);
        }
        // The key exists in the DB but fetching from storage failed; fall through to rebuild below
    }

    // No artifact (first visit to a seed sample / user-created app not yet built): build lazily and persist
    let app = {
        let market = state.market.lock().await;
        market.iter().find(|a| a.id == id).cloned()
    };
    let Some(app) = app else {
        return (StatusCode::NOT_FOUND, "App not found").into_response();
    };
    if build_and_store_wasm(&state, &app).await {
        if let Ok(Some(url)) = state.storage.presigned_url(&key, 600).await {
            return redirect_to(url);
        }
        if let Ok(bytes) = state.storage.get_bytes(&key).await {
            return serve_wasm_bytes(bytes);
        }
    }
    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to build app artifact").into_response()
}

/// Serve wasm bytes as application/wasm.
fn serve_wasm_bytes(bytes: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, "application/wasm")], bytes).into_response()
}

/// Compile the app source and write the `main.wasm` artifact to the storage backend (local directory / COS,
/// per config), while writing the `wasm` key back to the market cache and database. Used in two places:
/// 1. "Pre-build" of seed samples at service startup — samples run immediately after deploy;
/// 2. "Lazy build" in the `app_wasm` route on the first access when no artifact exists yet.
///
/// Returns success; any failure (missing toolchain / compile error / storage write failure) only prints a
/// warning and does not propagate, to avoid blocking startup or requests.
async fn build_and_store_wasm(state: &Arc<AppState>, app: &MarketApp) -> bool {
    let id = app.id.clone();
    let key = format!("apps/{id}/main.wasm");

    // Idempotent: if the local backend already has the artifact (persisted by a previous run), mark it and
    // skip compilation to avoid repeated builds on restart.
    if state.storage.scheme() == "local" {
        if let Ok(bytes) = state.storage.get_bytes(&key).await {
            let mut market = state.market.lock().await;
            if let Some(a) = market.iter_mut().find(|a| a.id == id) {
                a.wasm = Some(key.clone());
                state.db.save_app(a).await;
            }
            eprintln!("[seed] Sample {id} artifact already exists, skipping build ({} bytes at {})", key, bytes.len());
            return true;
        }
    }

    let src = if !app.source.trim().is_empty() {
        app.source.clone()
    } else {
        match aiapp_gen::templates::get_template_source(&app.template, &app.name) {
            Some(s) => s,
            None => {
                eprintln!("[seed] Sample {id}: cannot get template source, skipping pre-build");
                return false;
            }
        }
    };
    let build_dir = state.workdir.join(format!("run_{id}"));
    let _ = std::fs::remove_dir_all(&build_dir);
    if let Err(e) = aiapp_gen::write_project(&build_dir, &app.name, &src, &app.template) {
        eprintln!("[seed] Sample {id}: failed to write project: {e}");
        return false;
    }
    let mut config = aiapp_build::BuildConfig::default();
    config.moon_bin = state.moon_bin.clone();
    let out = match aiapp_build::build(&build_dir, &config) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[seed] Sample {id}: build failed (check moon toolchain): {e}");
            return false;
        }
    };
    let bytes = match std::fs::read(&out.wasm) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[seed] Sample {id}: failed to read WASM: {e}");
            return false;
        }
    };
    if let Err(e) = state.storage.put(&key, &bytes).await {
        eprintln!("[seed] Sample {id}: failed to upload artifact to storage backend: {e}");
        return false;
    }
    {
        let mut market = state.market.lock().await;
        if let Some(a) = market.iter_mut().find(|a| a.id == id) {
            a.wasm = Some(key.clone());
            state.db.save_app(a).await;
        }
    }
    eprintln!("[seed] Sample {id}: pre-build done -> {key}");
    true
}

/// 302 redirect to the given URL (used for presigned direct links).
fn redirect_to(url: String) -> Response {
    ([(header::LOCATION, url)], "").into_response()
}

/// Presigned URL query parameters.
#[derive(Debug, Deserialize)]
struct PresignQuery {
    key: String,
    expires: Option<u64>,
}

/// Return a presigned download URL for an object (pre-authorized URL). Requires login to avoid anonymous enumeration.
/// The COS backend returns a temporary direct link with q-signature; the local backend does not support presigning and returns 400.
async fn storage_presign(
    State(state): State<Arc<AppState>>,
    _auth: auth::AuthUser,
    Query(q): Query<PresignQuery>,
) -> Response {
    match state.storage.presigned_url(&q.key, q.expires.unwrap_or(600)).await {
        Ok(Some(url)) => Json(json!({ "ok": true, "url": url })).into_response(),
        Ok(None) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "The current storage backend does not support presigning (only cos does)" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// The frontend uses sql.js (SQLite WASM) for local persistence in the browser; these handlers serve its runtime files.
async fn sql_wasm_js() -> Response {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_bytes!("../static/sql-wasm.js").as_slice(),
    )
        .into_response()
}

async fn sql_wasm_wasm() -> Response {
    (
        [(header::CONTENT_TYPE, "application/wasm")],
        include_bytes!("../static/sql-wasm.wasm").as_slice(),
    )
        .into_response()
}

// ====== Auth endpoints ======

/// Register/login request body.
#[derive(Deserialize)]
struct AuthRequest {
    username: String,
    password: String,
}

/// Uniform error response.
fn auth_err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "ok": false, "error": msg }))).into_response()
}

/// User registration (only available in database mode).
async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AuthRequest>,
) -> Response {
    if !state.auth_enabled {
        return auth_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "Persistence is not enabled; registration is unavailable. Check the DATABASE_URL config (PostgreSQL or SQLite)",
        );
    }
    let username = req.username.trim();
    if username.is_empty() || req.password.len() < 6 {
        return auth_err(StatusCode::BAD_REQUEST, "Username must not be empty, and password must be at least 6 characters");
    }
    if state.db.find_user_by_name(username).await.is_some() {
        return auth_err(StatusCode::CONFLICT, "Username already exists");
    }
    let id = gen_user_id();
    let u = AppUser {
        id: id.clone(),
        name: username.to_string(),
        role: "user".into(),
        status: "active".into(),
        apps_generated: 0,
        launches: 0,
        incentive: 0,
        org: "main".into(),
        created_at: chrono_now(),
        installed: Vec::new(),
        password_hash: auth::hash_password(&req.password),
    };
    state.db.save_user(&u).await;
    {
        let mut users = state.users.lock().await;
        if !users.iter().any(|x| x.id == id) {
            users.push(u.clone());
        }
    }
    // Telemetry: new user registered (anonymous, type count only)
    state.telemetry.report("user_register", json!({}));
    match auth::make_token(&state.auth_secret, &id, "user") {
        Ok(token) => auth::login_response(&token, &u).into_response(),
        Err(e) => auth_err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

/// User login (returns JWT and writes an HttpOnly cookie).
async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AuthRequest>,
) -> Response {
    let username = req.username.trim();
    if username.is_empty() {
        return auth_err(StatusCode::BAD_REQUEST, "Please enter a username");
    }

    // Dev mode: only the admin can log in with initial credentials
    if !state.auth_enabled {
        if username == state.admin_username && req.password == state.admin_password {
            let token = auth::make_token(&state.auth_secret, "u_admin", "admin")
                .unwrap_or_default();
            let u = AppUser {
                id: "u_admin".into(),
                name: username.to_string(),
                role: "admin".into(),
                status: "active".into(),
                apps_generated: 0,
                launches: 0,
                incentive: 0,
                org: "main".into(),
                created_at: "2026-08-18".into(),
                installed: Vec::new(),
                password_hash: String::new(),
            };
            return auth::login_response(&token, &u).into_response();
        }
        return auth_err(StatusCode::UNAUTHORIZED, "Persistence is not enabled; only the admin can log in");
    }

    let u = match state.db.find_user_by_name(username).await {
        Some(u) => u,
        None => return auth_err(StatusCode::UNAUTHORIZED, "Incorrect username or password"),
    };
    if u.status != "active" {
        return auth_err(StatusCode::FORBIDDEN, "This account has been disabled");
    }
    if !auth::verify_password(&req.password, &u.password_hash) {
        return auth_err(StatusCode::UNAUTHORIZED, "Incorrect username or password");
    }
    match auth::make_token(&state.auth_secret, &u.id, &u.role) {
        Ok(token) => auth::login_response(&token, &u).into_response(),
        Err(e) => auth_err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

/// Current logged-in user info.
async fn auth_me(
    State(state): State<Arc<AppState>>,
    auth: auth::AuthUser,
) -> Json<serde_json::Value> {
    let name = if state.auth_enabled {
        state.db.get_user(&auth.id).await.map(|u| u.name)
    } else {
        state.users.lock().await.iter().find(|u| u.id == auth.id).map(|u| u.name.clone())
    }
    .unwrap_or_default();
    Json(json!({
        "ok": true,
        "user": { "id": auth.id, "name": name, "role": auth.role }
    }))
}

// ====== Reports ======

/// User report request body.
#[derive(Deserialize)]
struct ReportRequest {
    id: String,
    reason: String,
}

/// Record an app report (requires login).
async fn report_app(
    State(state): State<Arc<AppState>>,
    auth: auth::AuthUser,
    Json(req): Json<ReportRequest>,
) -> Json<ActionResponse> {
    let mut market = state.market.lock().await;
    match market.iter_mut().find(|a| a.id == req.id) {
        Some(app) => {
            let reason = req.reason.trim();
            app.report = if reason.is_empty() {
                "User report".into()
            } else {
                reason.to_string()
            };
            let cloned = app.clone();
            state.db.save_app(&cloned).await;
            // Telemetry: app reported (anonymous, type count only)
            state.telemetry.report("app_report", json!({ "app_id": cloned.id }));
            Json(ActionResponse { ok: true, error: None, app: Some(cloned) })
        }
        None => Json(ActionResponse {
            ok: false,
            error: Some("App not found".into()),
            app: None,
        }),
    }
}

// ====== Admin system (requires admin) ======

/// User list response.
#[derive(Serialize)]
struct AdminUsersResponse {
    users: Vec<AppUser>,
}

async fn admin_users(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
) -> Json<AdminUsersResponse> {
    let users = state.users.lock().await.clone();
    Json(AdminUsersResponse { users })
}

/// Admin app list (all, including under review / draft / disabled).
#[derive(Serialize)]
struct AdminAppsResponse {
    apps: Vec<MarketApp>,
}

async fn admin_apps(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
) -> Json<AdminAppsResponse> {
    let apps = state.market.lock().await.clone();
    Json(AdminAppsResponse { apps })
}

/// Admin user operation request body.
#[derive(Deserialize)]
struct UserToggleRequest {
    id: String,
    action: String,
}

async fn admin_user_toggle(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
    Json(req): Json<UserToggleRequest>,
) -> Json<ActionResponse> {
    let mut users = state.users.lock().await;
    match users.iter_mut().find(|u| u.id == req.id) {
        Some(u) => {
            if u.role == "admin" {
                return Json(ActionResponse {
                    ok: false,
                    error: Some("Cannot disable an admin account".into()),
                    app: None,
                });
            }
            u.status = if req.action == "disable" { "disabled".into() } else { "active".into() };
            state.db.save_user(u).await;
            Json(ActionResponse { ok: true, error: None, app: None })
        }
        None => Json(ActionResponse {
            ok: false,
            error: Some("User not found".into()),
            app: None,
        }),
    }
}

/// Admin app review request body.
#[derive(Deserialize)]
struct AdminReviewRequest {
    id: String,
    action: String,
    note: String,
}

async fn admin_app_review(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
    Json(req): Json<AdminReviewRequest>,
) -> Json<ActionResponse> {
    let mut market = state.market.lock().await;
    match market.iter_mut().find(|a| a.id == req.id) {
        Some(app) => {
            if req.action == "approve" {
                app.status = "published".into();
                app.visibility = "public".into();
                app.review_note = "Approved".into();
            } else {
                app.status = "draft".into();
                app.visibility = "private".into();
                app.review_note = if req.note.trim().is_empty() {
                    "Review not approved".into()
                } else {
                    req.note.trim().into()
                };
            }
            let cloned = app.clone();
            state.db.save_app(&cloned).await;
            Json(ActionResponse { ok: true, error: None, app: Some(cloned) })
        }
        None => Json(ActionResponse {
            ok: false,
            error: Some("App not found".into()),
            app: None,
        }),
    }
}

/// Admin app operation request body.
#[derive(Deserialize)]
struct AdminStatusRequest {
    id: String,
    action: String,
}

async fn admin_app_status(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
    Json(req): Json<AdminStatusRequest>,
) -> Json<ActionResponse> {
    let mut market = state.market.lock().await;
    match market.iter_mut().find(|a| a.id == req.id) {
        Some(app) => match req.action.as_str() {
            "disable" => {
                app.status = "disabled".into();
                app.visibility = "private".into();
            }
            "enable" => {
                app.status = "draft".into();
                app.visibility = "private".into();
            }
            "publish" => {
                app.status = "published".into();
                app.visibility = "public".into();
            }
            _ => {
                return Json(ActionResponse {
                    ok: false,
                    error: Some("Unknown operation".into()),
                    app: None,
                })
            }
        },
        None => {
            return Json(ActionResponse {
                ok: false,
                error: Some("App not found".into()),
                app: None,
            })
        }
    }
    let cloned = market
        .iter()
        .find(|a| a.id == req.id)
        .cloned()
        .unwrap();
    state.db.save_app(&cloned).await;
    Json(ActionResponse { ok: true, error: None, app: Some(cloned) })
}

/// Admin edit-app request body.
#[derive(Deserialize)]
struct AdminAppEditRequest {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    platforms: Vec<String>,
}

async fn admin_app_edit(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
    Json(req): Json<AdminAppEditRequest>,
) -> Json<ActionResponse> {
    let mut market = state.market.lock().await;
    match market.iter_mut().find(|a| a.id == req.id) {
        Some(app) => {
            if !req.name.trim().is_empty() {
                app.name = req.name.trim().into();
            }
            app.description = req.description.to_string();
            if !req.tags.is_empty() {
                app.tags = req.tags.iter().filter(|s| !s.is_empty()).cloned().collect();
            }
            if !req.platforms.is_empty() {
                app.platforms = req.platforms.iter().filter(|s| !s.is_empty()).cloned().collect();
            }
            let cloned = app.clone();
            state.db.save_app(&cloned).await;
            Json(ActionResponse { ok: true, error: None, app: Some(cloned) })
        }
        None => Json(ActionResponse {
            ok: false,
            error: Some("App not found".into()),
            app: None,
        }),
    }
}

/// Admin delete request body.
#[derive(Deserialize)]
struct AdminAppDeleteRequest {
    id: String,
}

async fn admin_app_delete(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
    Json(req): Json<AdminAppDeleteRequest>,
) -> Json<ActionResponse> {
    let mut market = state.market.lock().await;
    match market.iter().position(|a| a.id == req.id) {
        Some(i) => {
            let removed = market.remove(i);
            state.db.delete_app(&req.id).await;
            Json(ActionResponse { ok: true, error: None, app: Some(removed) })
        }
        None => Json(ActionResponse {
            ok: false,
            error: Some("App not found".into()),
            app: None,
        }),
    }
}

/// Stats overview response.
#[derive(Serialize)]
struct StatsResponse {
    total_apps: u64,
    published: u64,
    reviewing: u64,
    disabled: u64,
    total_launches: u64,
    total_users: u64,
    active_users: u64,
    total_incentive: u64,
    top_apps: Vec<MarketApp>,
    users: Vec<AppUser>,
}

async fn admin_stats(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
) -> Json<StatsResponse> {
    let market = state.market.lock().await.clone();
    let users = state.users.lock().await.clone();

    let total_apps = market.len() as u64;
    let published = market.iter().filter(|a| a.status == "published").count() as u64;
    let reviewing = market.iter().filter(|a| a.status == "reviewing").count() as u64;
    let disabled = market.iter().filter(|a| a.status == "disabled").count() as u64;
    let total_launches: u64 = market.iter().map(|a| a.launches).sum();
    let total_users = users.len() as u64;
    let active_users = users.iter().filter(|u| u.status == "active").count() as u64;
    let total_incentive: u64 = users.iter().map(|u| u.incentive).sum();

    let mut top_apps = market.clone();
    top_apps.sort_by(|a, b| b.launches.cmp(&a.launches));
    top_apps.truncate(6);

    Json(StatsResponse {
        total_apps,
        published,
        reviewing,
        disabled,
        total_launches,
        total_users,
        active_users,
        total_incentive,
        top_apps,
        users,
    })
}

/// Prompt view/save response.
#[derive(Serialize)]
struct PromptResponse {
    ok: bool,
    version: &'static str,
    prompt: String,
    error: Option<String>,
}

async fn admin_prompt(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
) -> Json<PromptResponse> {
    let prompt = state.prompt.lock().await.clone();
    Json(PromptResponse {
        ok: true,
        version: aiapp_gen::SYSTEM_PROMPT_VERSION,
        prompt,
        error: None,
    })
}

/// Save request body.
#[derive(Deserialize)]
struct PromptSaveRequest {
    prompt: String,
}

async fn admin_prompt_save(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
    Json(req): Json<PromptSaveRequest>,
) -> Json<PromptResponse> {
    let content = req.prompt.trim().to_string();
    if content.is_empty() {
        return Json(PromptResponse {
            ok: false,
            version: aiapp_gen::SYSTEM_PROMPT_VERSION,
            prompt: String::new(),
            error: Some("Prompt content must not be empty".into()),
        });
    }
    *state.prompt.lock().await = content.clone();
    state.db.save_prompt(&content).await;
    Json(PromptResponse {
        ok: true,
        version: aiapp_gen::SYSTEM_PROMPT_VERSION,
        prompt: content,
        error: None,
    })
}

async fn admin_prompt_reset(
    State(state): State<Arc<AppState>>,
    _admin: auth::AdminUser,
) -> Json<PromptResponse> {
    let default = aiapp_gen::default_system_prompt().to_string();
    *state.prompt.lock().await = default.clone();
    state.db.save_prompt(&default).await;
    Json(PromptResponse {
        ok: true,
        version: aiapp_gen::SYSTEM_PROMPT_VERSION,
        prompt: default,
        error: None,
    })
}

/// Start the service.
#[tokio::main]
async fn main() {
    let app = router().await;
    let cfg = config::AppConfig::from_env();
    let addr = SocketAddr::from((cfg.host, cfg.port));
    println!("aiapp-mb started: http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind port; check the HOST/PORT config");
    axum::serve(listener, app).await.expect("Service exited abnormally");
}
