# aiapp-mb Architecture

> This document is updated with every architectural change. This version covers Phase 1: Web MVP + multi-user auth + deployable backend + Phase 2 open-source portion (format / runtime / host container / mobile bridge) + Phase 3 open-source portion (standalone App packaging).

## Goals

Run `aiapp-mb` as a **deployable backend service**: users access the marketplace via the Web, generate and run AI apps; in the future, containerized host Apps can reuse the same backend.

Core principles:

1. **Build once, run everywhere**: description → AI generates MoonBit source → `moon` compiles to WASM → `.aiapp` unified package; each platform plays it via the unified WIT contract.
2. **Deployable**: listen address/port are configurable (`HOST`/`PORT`), static resource path is configurable (`STATIC_DIR`).
3. **Persistable**: metadata goes to PostgreSQL or SQLite (`sqlx::Any`, driver switched by `DATABASE_URL` prefix); artifacts go through the unified `Storage` abstraction (local / COS / OSS).
4. **Multi-user**: built-in register/login, JWT auth; admin role required for admin management; org-level sharing.
5. **Containerizable**: multi-stage Dockerfile + docker-compose, with host mode equally supported.

## Component Overview

```
Natural language description → aiapp-gen generates a MoonBit project → aiapp-build invokes moon to compile → .aiapp package
                                                                          │
      aiapp-host / browser renderer / host App ── plays per the WIT contract (build once, run everywhere)
```

| Stage | Component | Description |
|---|---|---|
| Generation | crates/aiapp-gen | Generates a runnable MoonBit project from a description (templates / mock backend, community edition) |
| Compilation | crates/aiapp-build | `moon build --target wasm-gc` → packages into `.aiapp` |
| Orchestration | crates/aiapp-cli | CLI entry: `create` / `build` / `go` / `templates` / `pack` |
| Format | crates/aiapp-format | `.aiapp` unified package format: manifest / packaging / parsing / validation / WIT contract |
| Runtime | crates/aiapp-engine | Runtime: `Host` trait (host capability abstraction) + permission gating + execution entry (optional real Wasmtime execution) |
| Host | crates/aiapp-host | Desktop/CLI host container: `info`/`validate`/`run`/`capabilities`, optional real Wasmtime execution |
| Mobile bridge | crates/aiapp-host-bridge | Mobile host bridge layer (C-ABI): iOS(Swift) / Android(Kotlin+JNI) reuse the Rust runtime |
| Packaging | crates/aiapp-pack | Standalone App packager: `.aiapp` → Capacitor (mobile) shell projects + branding |
| Service | crates/aiapp-web | Web backend (axum): marketplace / generation / admin / in-browser execution / auth / sharing / review / telemetry |

## aiapp-web Backend Architecture

```
                ┌──────────────────────────────────────────────────────┐
   Browser ────▶ │ aiapp-web (axum on HOST:PORT)                        │
                │  • marketplace / generation / admin / in-browser WASM │
                │  • route auth: AuthUser / AdminUser extractors         │
                │  • in-memory hot cache (Arc<Mutex<Vec<AppUser>>> / apps)│
                │        │ write ops synchronized                        │
                │        ▼                                              │
                │  ┌──────────────────┐   ┌──────────────────────────────┐  │
                │  │ PostgreSQL/SQLite │   │ Storage abstraction           │  │
                │  │ metadata:         │   │ local / COS / OSS             │  │
                │  │ apps/users/       │   │ artifacts: wasm etc.          │  │
                │  │ prompts           │   │ key: apps/{id}/main.wasm     │  │
                │  └──────────────────┘   └──────────────────────────────┘  │
                └──────────────────────────────────────────────────────┘
```

### Module Breakdown (crates/aiapp-web/src)

| File | Responsibility |
|---|---|
| `main.rs` | HTTP routes, each endpoint's implementation, `AppState`, `AppUser`, `ensure_admin`, dev-mode single-admin injection |
| `config.rs` | `AppConfig`: loaded from environment variables + `.env` (HOST/PORT/DATABASE_URL/REQUIRE_DB/storage/auth) |
| `db.rs` | Metadata persistence layer (`users`/`apps`/`prompts`), unified `sqlx::Any` driver; `DATABASE_URL` selects **PostgreSQL** or **SQLite** by prefix, defaults to local SQLite (`sqlite://./data/aiapp.db`) when unset |
| `auth.rs` | Auth: `bcrypt` password hashing + `jsonwebtoken` (HS256) sign/verify; `AuthUser`/`AdminUser` extractors; `extract_token` (Bearer or `aiapp_token` Cookie) |
| `storage.rs` | Unified artifact storage abstraction (local / Tencent Cloud COS / Alibaba Cloud OSS) |
| `index.html` | Embedded frontend (marketplace, generation, login/register modal, in-browser WASM execution) |
| `static/` | sql.js (in-browser local SQLite runtime), embedded via `include_bytes!` |

### Auth Model

- **Password**: `bcrypt` hash; `users` table `password_hash TEXT NOT NULL DEFAULT ''`.
- **Token**: on successful login, a JWT (HS256, valid 7 days) is issued, signed with `AUTH_SECRET`.
- **Transport**: `HttpOnly Cookie` (`aiapp_token`) or `Authorization: Bearer <token>`.
- **Extractors**:
  - `AuthUser`: any logged-in user; missing/expired → 401.
  - `AdminUser`: user with `role=admin`; non-admin → 403.
- **Endpoint protection**:
  - Requires login: `/api/generate`, `/api/my-apps`, `/api/publish`, `/api/delete`, `/api/uninstall`, `/api/report`.
  - Requires admin: `/api/admin/*` (e.g., backend stats, user management).
  - Public: marketplace list/detail, WASM download, register, login, `/api/auth/me`.

### Run Modes

| Mode | Trigger | Behavior |
|---|---|---|
| Database mode (default) | `DATABASE_URL` unset (defaults to local SQLite) or explicitly configured PostgreSQL/SQLite | Load users/apps from DB; `ensure_admin()` guarantees the admin account exists and is persisted; registration open |
| Degraded dev mode | DB connection/migration failure | Single admin injected in memory (`ADMIN_USERNAME`/`ADMIN_PASSWORD`), registration endpoint disabled |
| Forced database | `REQUIRE_DB=true` and DB connection fails | Directly `exit(1)`, avoiding silent degradation with no persistence (data not durable) |

### Startup Pre-Build (first-run self-check)

On startup (inside `router()`), the service **concurrently pre-builds the `main.wasm` of all seed examples that have no artifact yet** and writes them to the storage backend:

- **Triggered in both modes**: both the persistent mode and the degraded dev mode execute on first start, ensuring "examples are immediately usable after deployment."
- **Toolchain self-check**: at startup it calls `moon::ensure_moon_toolchain()`, resolving `moon` in order `PATH → ~/.moon/bin → auto-install` (official script `cli.moonbitlang.com/install/unix.sh`); if missing it auto-installs and warns, without blocking the service.
- **Blocking wait**: on first run, `JoinSet` builds concurrently and awaits completion, so the deploy log directly shows whether the build environment is healthy (compile failure only warns; HTTP still starts normally).
- **Idempotent**: if the artifact already exists (e.g., on restart, or the COS object is still there) compilation is skipped; for the `local` backend, `build_and_store_wasm` probes for an existing artifact first and marks it directly, avoiding redundant builds.

### App Categorization / Review / Sharing (Phase 1 extensions)

- **Categorization**: an app can choose a `category` at registration; if not chosen, it is auto-categorized by template (`default_category_for`).
- **Auto-review**: decided by `category_requires_review(cat)`—tool/office categories (`tool` / `office`) are **auto-approved and published directly**; other categories enter the `reviewing` state and require manual admin approval at `/api/admin/app/review` (approve/reject with comment).
- **Sharing & visibility**: app `share` has three states: `private` (only self), `org` (visible to same org), `public` (published to marketplace). `list_market` filters by visibility (public + same org), `app_detail` checks access rights. The share link uses the `?app=<id>` parameter for direct access.
- **Lifecycle**: `draft` / `reviewing` / `published` / `disabled`, with admin able to disable/enable.

### Run-mode labels / Instant launch / My Apps / Uninstall (Phase 1 extensions)

- **Run-mode label**: `MarketApp.net` (`online` / `local`, the `net` column in the `apps` table, defaults to `local`). Marketplace cards and "My Apps" show a badge: `online` apps need network access to fetch data at runtime; `local` apps store data in the local browser.
- **Instant-launch collection**: the marketplace card's "⚡ Instant Launch" goes directly to `app_detail` (`/api/app/:id`); when the app opens, it is recorded in the launching user's "My Apps" (`AppUser.installed: Vec<String>`, the `installed` column in the `users` table as a JSON array, deduplicated) and persisted to the DB.
- **My Apps** (`/api/my-apps`): returns "apps I created + apps I've launched," with a `mine` boolean flag. The frontend renders accordingly: apps you created show update/publish/share/delete; only launched apps show "🗑 Uninstall".
- **Uninstall** (`POST /api/uninstall`, requires login): removes the app from the current user's `installed` and persists it (errors if not in "My Apps"). After a successful frontend uninstall, local data is cleared synchronously—all records for that `app_id` in the browser SQLite, the IndexedDB key `tv_<id>` (media list), and the localStorage key `mem_best` (memory-match record); a confirmation dialog clearly warns the data is unrecoverable and can be backed up first.

### Telemetry Reporting (reserved for Pro share / billing)

The open-source side (enterprise deployment) is only responsible for **producing anonymous usage events** and reporting them to the closed-source Pro service (`telemetry.rs`, `POST {PRO}/v1/telemetry`), without touching enterprise private data; fields follow a versioned schema (`docs/telemetry.schema.v1.md`). Reporting failures are silent and do not affect the main flow.

## aiapp-format / aiapp-engine / aiapp-host / aiapp-host-bridge (Phase 2 open-source portion)

These components form the platform's **runtime engine** (open source, to attract an ecosystem):

### aiapp-format — Unified App Package Format (ecosystem rules)

```
<name>.aiapp/
  aiapp.json      # manifest: metadata / permission declarations / entry / WIT version (AppManifest)
  main.wasm       # business logic (MoonBit → WASM)
  wit/app-host.wit # contract between app and host (WIT, self-contained copy bundled in the package)
  resources/      # optional resources (images, styles, etc.)
```

- Modules: `manifest.rs` (manifest structure + validation), `package.rs` (packaging/parsing), `validate.rs` (validation report), `wit.rs` (WIT contract + host capability catalog).
- `WIT_VERSION = "0.1.0"`: `package aiapp:app-host`, `interface host` defines `show-notification` / `save-data` / `load-data` / `log`; `interface app` defines lifecycle `run` / `stop`.
- `HOST_CAPABILITIES`: capability catalog (`storage` / `notifications` / `log`), corresponding to the `manifest.permissions` declaration—a lookup table for each platform implementing the WIT.
- Native capability extensions: WIT has been extended with `network` / `location` / `camera` / `push` host capability definitions for mobile host implementations (see `wit.rs`).

### aiapp-engine — Runtime ("player")

- `host::Host`: host capability abstraction (async trait: `show_notification` / `save_data` / `load_data` / `log` / native capabilities), implemented per platform (web → Web API, host App → native, desktop → system).
- `permissions`: permission gate, authorizing per the manifest declaration (`Granted` / `NotDeclared` / `Denied`); ungranted capabilities warn at startup.
- `runtime::Runtime`: load `.aiapp` package → validate → build permission gate → execution entry. Without the `wasmtime` feature it is lightweight mode (lifecycle hooks), convenient for preview/debug.
- `wasmtime.rs` (feature `wasmtime`): Wasmtime + WASI really executes `main.wasm` (`_start` entry), using an internal tokio runtime to bridge the async Host and synchronous callbacks. Both `aiapp-host` and `aiapp-host-bridge` reuse this execution entry.

### aiapp-host — Desktop/CLI Host Container

- `desktop_host.rs`: implements the `Host` trait—data lands in local files (`safe_path` prevents directory traversal), notifications/logging output to the terminal.
- CLI: `info` / `validate` / `run` (`--exec meta|wasmtime`, `--grant` permissions, `--data-dir` data isolation) / `capabilities`.
- Known limitation: MoonBit's `wasm-gc` target output contains const-expr GC instructions (`array.new_default`) that wasmtime 24 does not yet support; rebuild with the classic `--target wasm` target to really execute (the error message already gives this hint).

### aiapp-host-bridge — Mobile Host Bridge Layer (C-ABI)

- Goal: iOS (Swift) and Android (Kotlin + JNI) host Apps call the Rust runtime via `cdylib` to play `.aiapp`.
- Native capabilities (storage / notifications / network / location / camera / push) are injected as **callback function pointers**, implementing the WIT contract `aiapp:app-host`.
- FFI interfaces: `aiapp_bridge_create(callbacks, ctx)` / `aiapp_bridge_load(bridge, pkg_path)` / `aiapp_bridge_run(bridge, "meta"|"wasmtime")` / `aiapp_bridge_free(bridge)`.
- Threading model: `aiapp_bridge_run` uses an independent internal tokio runtime to run the engine; callbacks are invoked synchronously on the engine thread, and the native side should dispatch to the main thread itself if it needs to update the UI.

## aiapp-pack (Phase 3 open-source portion) — Standalone App Packager

`aiapp pack` turns a `.aiapp` into a buildable standalone app project with one click, including branding:

| Target | Shell project | Artifact |
|---|---|---|
| `capacitor` (mobile) | `www/` containing `main.wasm` + `aiapp.json`, the browser shell executes inside the WebView | `.apk` / `.ipa` |

- Modules: `lib.rs` (`pack()` entry + `PackTarget` / `PackConfig`), `brand.rs` (brand derivation + placeholder icon generation), `capacitor.rs` (shell project templates).
- Branding: `--name` / `--identifier` / `--version` / `--author` / `--icon` / `--homepage`; when no icon is supplied, a 512×512 branded placeholder icon is auto-generated (brand primary color + abstract app orb, 2×2 supersampled anti-aliasing), consistent across platforms for the same app.
- Errors if the output directory is non-empty, to avoid overwriting an existing project.

### Storage Abstraction

Artifacts (wasm, etc.) are read/written uniformly through the `Storage` trait, switched by `STORAGE_BACKEND`:

- `local`: written to `STORAGE_LOCAL_ROOT` (default `./storage`, already ignored by `.gitignore`).
- `cos`: Tencent Cloud Object Storage (integrated). Uses the COS XML API's V5 query signing (HMAC-SHA1); `put`/`get_bytes` for upload/download; supports **URL presigning** (`presigned_url` method)—`/api/storage/presign` returns a temporary direct link carrying `q-signature`, and `/api/app/:id/wasm` auto 302-redirects to that link when the backend is COS. Credentials come from `COS_SECRET_ID`/`COS_SECRET_KEY`/`COS_BUCKET`/`COS_REGION`; if missing, startup warns and falls back to local.
- `oss`: Alibaba Cloud OSS (reserved interface, returns a clear error when credentials are missing).

Key convention: `apps/{id}/main.wasm`, etc.

## Deployment Architecture

### Container (Docker)

Multi-stage `Dockerfile`:

- **builder** (`rust:1.83-bookworm`): install the `moon` toolchain and warm up `wasm-gc`; `cargo build --release -p aiapp-web`.
- **runtime** (`debian:bookworm-slim`): copy binary, `/app/templates` static resources, `/root/.moon` toolchain; `/data` is a persistent volume (storage artifacts + moon cache).

`docker-compose.yml`:

- `db`: `postgres:16`, with healthcheck, data volume `pgdata`.
- `app`: starts only after `db` is healthy; environment variables injected via `deploy.env` (`--env-file`); data volume `appdata` mounted at `/data`; `REQUIRE_DB=true` ensures exit on DB connection failure.

```
Host :8080 ──▶ app container (aiapp-web) ──┬─▶ PostgreSQL container (pgdata volume)
                                            └─▶ /data volume (storage + moon)
```

### Host Machine

After `cargo build --release -p aiapp-web`, run directly, driven by environment variables (`HOST`/`PORT`/`DATABASE_URL`/`AUTH_SECRET`, etc.); it is recommended to manage it with systemd / nohup and place it behind Nginx/Caddy to terminate TLS.

## Configuration Quick Reference

See the "Configuration" table in `README.md` and `deploy.env.example` / `.env.example`.

## Future Evolution

- Integration of a real OpenAI-compatible backend (`AIAPP_BACKEND=openai` reserved; needs to wire aiapp-gen's openai path into the web generation flow; current real generation goes through the closed-source Pro `aiapp-gen-pro` + `aiapp-pro-server`).
- Alibaba Cloud OSS storage backend integration (COS already done).
- Host App (iOS/Android): `aiapp-host-bridge` (C-ABI) is ready, reusing this backend's API to pull `.aiapp` packages and implementing the same set of WIT host capabilities on-device (`aiapp-host` desktop is the reference implementation); awaiting client-side project (Swift / Kotlin scaffolding) integration.
- Standalone App generation (Phase 3): `aiapp pack` already delivers Capacitor (mobile) shell project generation + branding; later `npx cap build android` can directly produce install packages. Desktop packaging framework selection is still under evaluation.
- Data portability: the web version (IndexedDB) and host App (SQLite) / desktop (files) achieve cross-platform data migration via WIT `save-data`/`load-data`.
- 100+ app ecosystem: expand templates and generation skills to enrich the marketplace.
