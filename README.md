# aiapp-mb — OS-Level Super AI-Native Application Platform

Let users generate applications from natural language with a single click, **build once and run everywhere**: web, Android, iOS, HarmonyOS, Windows, macOS, in-car systems, TV boxes, and more. The goal is an **operating-system-grade architecture** that hosts the AI-native application ecosystem, with an independently deployable core that evolves into an AI-native operating system.

**User describes → AI generates → MoonBit source → WASM bytecode → `.aiapp` unified app package**; every platform "plays" the same app package through a **unified WIT contract + host container** (`aiapp-engine` / `aiapp-host`).

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License">
  <img src="https://img.shields.io/badge/self--hosted-supported-brightgreen" alt="Self-Hosted">
  <img src="https://img.shields.io/badge/PRs-welcome-orange" alt="PRs Welcome">
</p>

This project follows an **Open Core** model: the core is fully open-source (MIT) and free to use, modify, and commercialize, with independent deployment supported. Pro commercial services (enterprise customization, managed hosting, technical support) help sustain the project's long-term development and keep the open-source version thriving.

The project is currently maintained by the author in spare time. Any form of contribution — Issues, PRs, docs, or usage feedback — is a great way to support it.

**Open Source · Free for Commercial Use · Independent Deployment** — This project is fully open-source and freely available for commercial use. It supports independent deployment via Docker Compose or direct host execution, with a built-in SQLite option that requires zero external dependencies.

## Roadmap

### Phase 1: Web MVP ✅ (current)

- [x] End-to-end pipeline: description → AI generates MoonBit source → moon compiles WASM → .aiapp package
- [x] 6 app templates (minimal / todo / image-filter / pomodoro / memory-game / tv-movies)
- [x] Unified `.aiapp` app package format (manifest + WASM + WIT contract + resources)
- [x] Multi-user authentication (register/login, JWT, admin panel)
- [x] App categorization + auto-review (tool/office categories auto-approved, others manually reviewed)
- [x] App sharing and visibility (private / org / public, share link `?app=id`)
- [x] Anonymous usage event reporting (telemetry, reserved for Pro revenue sharing/billing)
- [x] Online/local labels + "⚡ Instant Launch" one-click start
- [x] "My Apps" auto-collects launched apps; uninstall clears local data (with backup prompt)

### Phase 2: Mini-Program Container 🚧 (Not started)

- [x] `.aiapp` unified format + WIT contract (`aiapp-format`)
- [x] Runtime (`aiapp-engine`: permission gating + host capability abstraction + real Wasmtime execution)
- [x] Desktop/CLI host container (`aiapp-host`: meta / wasmtime dual execution modes)
- [x] Mobile host bridge layer (`aiapp-host-bridge`: C-ABI, iOS(Swift) / Android(Kotlin+JNI) reuse the Rust runtime)
- [x] WIT contract extended with native capabilities (network / location / camera / push, see `aiapp-format`'s `wit.rs`)
- [ ] Host App (iOS/Android) with embedded WASM runtime (bridge layer ready, awaiting client-side project integration)
- [ ] End-to-end native capability invocation (camera, location, push)

### Phase 3: Standalone App Generation 🚧 (Not started)

- [ ] Desktop packaging (framework TBD)
- [x] Capacitor packaging backend (mobile .apk/.ipa) — `aiapp pack --target capacitor`
- [x] Branding customization (name / identifier / auto-generated icon)
- [ ] 100+ app ecosystem

### Phase 4: AI-Native Operating System

- [ ] Platform core deployable standalone
- [ ] Run AI models offline
- [ ] Decentralized app distribution protocol

---

## Screenshots

| Marketplace | App Generation | In-Browser Execution |
|:---:|:---:|:---:|
| ![Marketplace](https://via.placeholder.com/400x250?text=Marketplace) | ![Generation](https://via.placeholder.com/400x250?text=App+Generation) | ![Execution](https://via.placeholder.com/400x250?text=In-Browser+Execution) |

| Mobile View | Admin Panel | App Packaging |
|:---:|:---:|:---:|
| ![Mobile](https://via.placeholder.com/400x250?text=Mobile+View) | ![Admin](https://via.placeholder.com/400x250?text=Admin+Panel) | ![Packaging](https://via.placeholder.com/400x250?text=App+Packaging) |

---

## Live Demo

> **Try it online**: `https://demo.aiapp.example.com` ← _Replace with the actual demo URL_

Experience the full pipeline: browse the marketplace, generate apps, and run them in your browser. No installation required.

---

## Project Goals

This project aims to build an **OS-level super AI-native application platform**. The core goal is to let users describe their needs in natural language and, with a single click, generate standalone or full-stack applications that run across multiple platforms—Web, Android, iOS, HarmonyOS, Windows, macOS, in-car systems, TV boxes—and in a distributed manner. The platform itself hosts the application ecosystem with an operating-system-grade architecture, supporting independent deployment, offline AI model execution, and decentralized distribution.

- Use MoonBit as the primary development language for parsing the intermediate description language.
- Generate MoonBit frontend code and assemble it into a uniformly formatted app package (similar to `.wasm` + `manifest.json`), which contains:
  - WASM bytecode (business logic)
  - Container app interface logic (the interface protocol between the app and the host runtime)

## Pipeline

```
Natural language description → aiapp-gen generates a MoonBit project → aiapp-build invokes moon to compile → .aiapp package
                                                                      │
                                  ┌──────────────────────────────────┼──────────────────────────────────┐
                                  ▼                                  ▼                                 ▼
         aiapp-host / browser renderer / host App          aiapp-pack builds a standalone App        mobile host bridge
         ── plays per the WIT contract (build once,         Capacitor mobile                     iOS(Swift)/Android(Kotlin)
            run everywhere)
```

| Stage | Component | Description |
|---|---|---|
| Generation | [aiapp-gen](crates/aiapp-gen) | Generates a runnable MoonBit project from a natural-language description, with template selection (the `mock` backend; real AI runs in the closed-source Pro) |
| Compilation | [aiapp-build](crates/aiapp-build) | Invokes the MoonBit toolchain `moon build --target wasm-gc` and automatically packages the result into the unified `.aiapp` format |
| Orchestration | [aiapp-cli](crates/aiapp-cli) | CLI entry point: `create` / `build` / `go` / `templates` / `pack` |
| Format | [aiapp-format](crates/aiapp-format) | The unified `.aiapp` app package format: manifest + WASM + **WIT contract** + resources |
| Runtime | [aiapp-engine](crates/aiapp-engine) | Runtime: permission gating + host capability abstraction + optional real Wasmtime execution (the open-source "player") |
| Host | [aiapp-host](crates/aiapp-host) | Desktop/CLI host container: `info` / `validate` / `run` / `capabilities`; can genuinely execute WASM |
| Mobile bridge | [aiapp-host-bridge](crates/aiapp-host-bridge) | Mobile host bridge layer (C-ABI): iOS (Swift) / Android (Kotlin+JNI) host Apps reuse the Rust runtime |
| Packaging | [aiapp-pack](crates/aiapp-pack) | Standalone App packager: `.aiapp` → Capacitor (mobile) shell projects + branding customization |
| Service | [aiapp-web](crates/aiapp-web) | Web backend (axum): app marketplace / generation / admin / auth / sharing / review / telemetry |

## Features

- **Unified app package format (`.aiapp`)**: `aiapp.json` manifest + `main.wasm` + `wit/app-host.wit` (the contract between app and host) + `resources/`
- **WIT ecosystem rules**: Apps only declare capabilities (`storage` / `notifications` / `log`); each platform's renderer implements the same interface → no per-platform code changes needed
- **Prebuilt app templates**: 6 templates covering productivity, efficiency tools, parent-child mini-games, audio/video entertainment, and in-car/TV platforms
- **Permission model**: `manifest.permissions` declares the required permissions, and `aiapp-engine` enforces them via its permission gate
- **Dual-backend support**: The `mock` backend works offline; real AI generation is handled by the closed-source Pro (`AIAPP_BACKEND=openai`)
- **Standalone App generation (Phase 3)**: `aiapp pack` turns a `.aiapp` into a **Capacitor mobile** shell project with one click, including branding customization (name / identifier / automatically generated icon)
- **Online/local labels**: Each app is tagged with a run mode—"online" needs network access to fetch data, "local" stores data on the device (cleared on uninstall); both the marketplace card and "My Apps" show a badge
- **Instant launch**: Marketplace cards offer a "⚡ Instant Launch" one-click open, with no need to favorite first
- **Automatic "My Apps" collection**: Apps you've "instant-launched" are automatically added to "My Apps" (recorded in the `installed` field, deduplicated); apps you created yourself can be updated/published/shared/deleted, and only launched apps offer a "🗑 Uninstall"
- **Uninstall clears data**: Uninstalling also deletes all data the app stored locally (todos, media lists, best scores, etc.); before uninstalling, a clear warning states the data is unrecoverable and can be backed up first

## Templates

| Template | Description |
|---|---|
| `minimal` | Minimal template: a Hello World–style app |
| `todo` | Todo list: add/delete/edit/query todo management |
| `image-filter` | Image filter: an image filtering app |
| `pomodoro` | Focus pomodoro: work/rest timer that accumulates focus counts |
| `memory-game` | Parent-child memory match: a card-matching mini-game that records the best score |
| `tv-movies` | Family media list: categorized movie collection, friendly to TV/in-car large screens |

## Usage

Requires the [MoonBit toolchain](https://www.moonbitlang.com/download) (provides the `moon` command).

```bash
cargo build --release

# List available templates
./target/release/aiapp templates

# Generate a project from a template
./target/release/aiapp create "My Todo" -o generated/todo -t todo

# One-click end-to-end: description → MoonBit project → WASM bytecode → .aiapp package
./target/release/aiapp go "hello world" -o generated/demo -t minimal

# Step by step
./target/release/aiapp create "Todo List" -o generated/todo -t todo   # generate project only
./target/release/aiapp build generated/todo                            # compile + package .aiapp

# Phase 3: turn .aiapp into a standalone app project (mobile Capacitor, with branding)
./target/release/aiapp pack generated/todo.aiapp --target capacitor --name "My Todo" --icon icon.png
```

### Generated Project Structure

```text
generated/<app>/
  aiapp.json            # unified app package manifest (app_id, name, version, permissions, etc.)
  moon.mod              # MoonBit package declaration
  cmd/main/
    moon.pkg            # executable package marker
    main.mbt            # generated entry-point source
```

### Compiled .aiapp Package Structure

```text
generated/<app>.aiapp/
  aiapp.json            # app manifest
  main.wasm             # compiled WASM bytecode
```

### AI Backend Configuration (Environment Variables)

The `mock` backend is used by default (a local template example, works offline). To switch to a real AI:

| Environment Variable | Description | Default |
|---|---|---|
| `AIAPP_BACKEND` | `mock` or `openai` | `mock` |
| `AIAPP_OPENAI_BASE_URL` | OpenAI-compatible API base URL | `https://api.openai.com/v1` |
| `AIAPP_OPENAI_API_KEY` | API Key | empty |
| `AIAPP_OPENAI_MODEL` | Model name | `gpt-4o-mini` |

```bash
AIAPP_BACKEND=openai \
AIAPP_OPENAI_BASE_URL=https://api.openai.com/v1 \
AIAPP_OPENAI_API_KEY=sk-xxx \
AIAPP_OPENAI_MODEL=gpt-4o-mini \
./target/release/aiapp go "Focus Pomodoro" -o generated/tomato -t pomodoro
```

## Project Structure

```
crates/
  aiapp-cli/    # binary entry point, orchestrates the pipeline
  aiapp-gen/    # generator: templates / mock backend (community edition), produces .aiapp projects
  aiapp-build/  # builder: moon build → WASM → .aiapp packaging
  aiapp-format/ # unified app package format: manifest / packaging / parsing / validation / WIT contract definition
  aiapp-engine/ # runtime: host capability abstraction (Host trait) + permission gating + app execution (optional real Wasmtime execution)
  aiapp-host/   # desktop/CLI host container: info / validate / run / capabilities
  aiapp-host-bridge/ # mobile host bridge layer (C-ABI): iOS(Swift) / Android(Kotlin+JNI) reuse the Rust runtime
  aiapp-pack/   # standalone App packager: .aiapp → Capacitor shell projects + branding customization (auto-generated icon)
  aiapp-web/    # Web prototype service (axum): app marketplace / generation / admin / in-browser execution
    src/
      main.rs     # HTTP routes and each endpoint
      config.rs   # runtime configuration (database connection / storage backend), see "Configuration" below
      db.rs       # metadata persistence layer: local SQLite by default, can switch to PostgreSQL
      storage.rs  # unified artifact storage abstraction (local / Tencent Cloud COS / Alibaba Cloud OSS)
      auth.rs     # auth: bcrypt password hashing + JWT (HS256)
      telemetry.rs# anonymous usage event reporting (reserved for closed-source Pro revenue sharing/billing)
      index.html  # embedded frontend (marketplace, generation, admin, in-browser WASM execution)
      static/     # sql.js (in-browser local SQLite runtime), embedded via include_bytes!
```

## Desktop/CLI Host (aiapp-host, Phase 2 open-source portion)

`aiapp-host` is the host container for `.aiapp` packages; it implements the host capabilities defined in the WIT contract (storage to local files, notifications/logging to the terminal). It is the desktop form of "build once, run everywhere"; other forms (host App, standalone App, browser renderer) can reuse the app by implementing the same set of WIT interfaces.

```bash
cargo build -p aiapp-host                 # lightweight mode (metadata/lifecycle)
cargo build -p aiapp-host --features wasmtime   # enable real WASM execution

./target/debug/aiapp-host capabilities                    # list the WIT host capability catalog
./target/debug/aiapp-host info demo.aiapp                 # view app package info (manifest + validation)
./target/debug/aiapp-host validate demo.aiapp             # validate the app package
./target/debug/aiapp-host run demo.aiapp --exec meta      # lightweight run (validation + permission gating)
./target/debug/aiapp-host run demo.aiapp --exec wasmtime  # real WASM execution (requires feature)
```

> Note: MoonBit's default `wasm-gc` target output contains const-expr GC instructions that wasmtime 24 does not yet support; rebuild with the classic target (`aiapp build <dir> --target wasm`) and it can then really execute in `wasmtime` mode (see the hint in [aiapp-engine/src/wasmtime.rs](crates/aiapp-engine/src/wasmtime.rs)).

### Unified App Package Format (aiapp-format)

A `.aiapp` package = manifest (`aiapp.json`) + WASM (`main.wasm`) + WIT contract (`wit/app-host.wit`) + optional resources (`resources/`). `aiapp-format` is the authoritative definition of the format (packaging/parsing/validation), and `aiapp-engine` is responsible for execution; together they form the platform's "engine."

## Mobile Host Bridge (aiapp-host-bridge, Phase 2 open-source portion)

`aiapp-host-bridge` is a mobile host bridge layer (C-ABI) that lets iOS (Swift) and Android (Kotlin + JNI) host Apps directly reuse the Rust runtime to play `.aiapp`. Native capabilities (storage / notifications / network / location / camera / push) are injected as **callback function pointers**, implementing the WIT contract `aiapp:app-host`.

```bash
# Build the cdylib (on macOS this yields libaiapp_host_bridge.dylib)
cargo build -p aiapp-host-bridge
```

```text
The host App calls via FFI:
  aiapp_bridge_create(callbacks, ctx)         # create a host session (inject native capability callbacks)
  aiapp_bridge_load(bridge, pkg_path)         # parse the .aiapp package
  aiapp_bridge_run(bridge, "meta"|"wasmtime") # run the app
  aiapp_bridge_free(bridge)                   # release
```

> Threading model: `aiapp_bridge_run` uses an independent tokio runtime internally to run the engine; callbacks are invoked synchronously on the engine thread. If the native side needs to update the UI, it should dispatch to the main thread itself.

## Standalone App Generation (aiapp-pack, Phase 3 open-source portion)

`aiapp pack` turns a `.aiapp` into a buildable standalone app project with one click, including branding customization:

| Target | Shell project | Artifact |
|---|---|---|
| `capacitor` (mobile) | `www/` containing `main.wasm` + `aiapp.json`, the browser shell executes inside the WebView | `.apk` / `.ipa` |

- **Branding customization**: `--name` / `--identifier` / `--version` / `--author` / `--icon` / `--homepage`; when no icon is supplied, a **512×512 branded placeholder icon is auto-generated** (brand primary color + abstract app orb, 2×2 supersampled anti-aliasing).
- The brand primary color is stably derived from the manifest/brand name, ensuring a consistent appearance of the same app across platforms.

```bash
./target/release/aiapp pack generated/tv.aiapp --target capacitor --name "Family Media List"

# Enter the shell project and build per platform
cd <out> && npm install && npx cap add android && npx cap build android
```

## Web Prototype Service (aiapp-web)

An employee self-service prototype: enter a description + pick a template → generate a MoonBit app → preview in the marketplace → click to actually run.

```bash
cd aiapp-mb
cargo run -p aiapp-web
# open http://127.0.0.1:8080 in a browser
```

- The `mock` backend works offline by default, no API Key required.
- Clicking a marketplace app **really runs the compiled WASM in the browser** (`/api/app/:id/wasm` builds on demand via `moon build` and returns it).
- Todo-style apps persist user data using an **in-browser local SQLite (sql.js + IndexedDB)**, so reopening does not lose data.
- App cards show **online/local** mode badges and an "⚡ Instant Launch" button; launched apps are auto-collected into "My Apps"; uninstalling deletes the app's local data (see "Run Modes and My Apps" below).

### Architecture

```
                 ┌─────────────────────────────────────────────┐
   Browser ─────▶ │ aiapp-web (axum)                            │
                 │  • marketplace / generation / admin         │
                 │  • in-memory hot cache (Vec<MarketApp>)      │
                 │    refreshed from DB before each request     │
                 │        │ synchronous read/write               │
                 │        ▼                                     │
                 │  ┌──────────────┐   ┌────────────────────┐  │
                 │  │ PostgreSQL   │   │ Storage abstraction │  │
                 │  │ metadata:     │   │ local / COS / OSS   │  │
                 │  │ apps/users/   │   │ artifacts: wasm etc. │  │
                 │  │ prompts       │   │ (key: apps/{id}/..) │  │
                 │  └──────────────┘   └────────────────────┘  │
                 └─────────────────────────────────────────────┘
```
- **Metadata (apps/users/prompts)**: stored in the database (`db.rs`, unified `sqlx::Any` driver). Supports **PostgreSQL** (`postgres://...`) or **local SQLite** (`sqlite://...`), auto-selected by the `DATABASE_URL` prefix; when `DATABASE_URL` is not set, it defaults to a local SQLite file (`sqlite://./data/aiapp.db`) that works out of the box and persists—no longer falling back to an in-memory mode. The `apps` table has a `tier` column (`open` for open source / `commercial` for closed source) used for the open/closed-source split.
- **Data consistency**: Marketplace/user lists use an "in-memory hot cache + database persistence layer," but **the cache is refreshed from the database as the source of truth before every request** (middleware `refresh_cache` → `AppState::refresh`). The database is the single source of truth, so multiple instances (e.g., dual processes on 8090/8092) or a restart all read the latest committed data—once registered/disabled somewhere, it is immediately visible everywhere. Static resources (`/static/*`) skip the refresh.
- **SPA routing**: Unmatched non-`/api/` paths (e.g., directly visiting `/admin`) fall back to the home-page HTML to avoid a blank screen; unmatched APIs still return 404.
- **Artifacts (wasm, etc.)**: go through the unified `Storage` abstraction (`storage.rs`), switched by `STORAGE_BACKEND` between `local` / Tencent Cloud COS / Alibaba Cloud OSS, no longer scattered across the source tree.

### Run Modes and "My Apps"

- **Run mode (`net` field)**: the `apps` table gains a `net` column (`online` / `local`). `online` apps need network access to fetch data at runtime; `local` apps store data in the local browser, deleted on uninstall.
- **Instant-launch collection**: marketplace cards offer "⚡ Instant Launch"—clicking opens the app (`/api/app/:id`) and records it in the launching user's "My Apps" (`users.installed` column, a JSON array, deduplicated).
- **My Apps** (`/api/my-apps`) returns "apps I created + apps I've launched," with a `mine` boolean flag: apps you created show update/publish/share/delete; only launched apps show "🗑 Uninstall."
- **Uninstall** (`POST /api/uninstall`): removes the app from `installed`; the frontend simultaneously clears local data—all records for that `app_id` in SQLite, the IndexedDB key (media list `tv_<id>`), and the localStorage key (memory-match best score). Before uninstalling, a confirmation dialog clearly warns the data is unrecoverable and can be backed up first.

### Configuration: Database Connection and Storage Backend

Configuration sources: environment variables, plus an optional `.env` file at the repo root (see `.env.example`). **The database connection is configured via `DATABASE_URL`**, switching drivers by prefix; when unset, it defaults to a local SQLite file (`sqlite://./data/aiapp.db`), persisting data and working out of the box.

| Environment Variable | Description | Default |
|---|---|---|
| `DATABASE_URL` | Metadata DB connection string, driver switched by prefix: `postgres://user:pass@host:5432/db` → PostgreSQL (recommended for production); `sqlite://./data/aiapp.db` → local SQLite file (zero-dependency, auto-creates tables). Once set, metadata persists across restarts | `sqlite://./data/aiapp.db` (local SQLite, persistent) |
| `REQUIRE_DB` | When `true`, exits immediately if the database cannot be reached (in production, paired with an orchestrator retry to avoid silent degradation and data loss from no persistence; a SQLite file DB can always connect, so this can be omitted) | `false` |
| `HOST` | Bind address: `0.0.0.0` for external access, `127.0.0.1` for local only | `0.0.0.0` |
| `PORT` | Listen port | `8080` |
| `STATIC_DIR` | Frontend static resource (template thumbnails, etc.) directory; `/app/templates` inside the container | `./templates` |
| `AUTH_SECRET` | JWT signing secret; **production MUST explicitly set a strong random value** (e.g., `openssl rand -hex 32`) | insecure default (warns) |
| `ADMIN_USERNAME` | Initial admin username (persisted on first launch) | `admin` |
| `ADMIN_PASSWORD` | Initial admin password (persisted on first launch; change in production) | `admin123` (warns) |
| `STORAGE_BACKEND` | Artifact storage backend: `local` / `cos` / `oss` | `local` |
| `STORAGE_LOCAL_ROOT` | `local` backend root directory (already ignored by `.gitignore`) | `./storage` |
| `COS_SECRET_ID` / `COS_SECRET_KEY` / `COS_BUCKET` / `COS_REGION` | Tencent Cloud COS credentials (fill when `STORAGE_BACKEND=cos`, supports presigning) | empty (falls back to local) |
| `OSS_*` | Alibaba Cloud OSS credentials (reserved, not yet wired up) | empty |

Example: start PostgreSQL and enable persistence:

```bash
# Option A: PostgreSQL (recommended for production)
createdb aiapp
export DATABASE_URL="postgres://user:pass@localhost:5432/aiapp"

# Option B: local SQLite (zero-dependency, file-as-DB, auto-creates tables)
export DATABASE_URL="sqlite://./data/aiapp.db"

# Start (first run auto-creates tables and seeds data)
cargo run -p aiapp-web
```

> Note: The Alibaba Cloud OSS backend is still a reserved interface (returns a clear error) until real credentials are filled in; **Tencent Cloud COS is fully integrated and supports URL presigning**.

#### Tencent Cloud COS Object Storage (integrated, supports URL presigning)

Set `STORAGE_BACKEND=cos` and configure credentials via the variables below (placeholders in `.env.example` / `deploy.env.example`), and the app's artifacts (wasm, etc.) will be stored in COS:

| Environment Variable | Description |
|---|---|
| `COS_SECRET_ID` | Cloud API SecretId (placeholder `AKID...`) |
| `COS_SECRET_KEY` | Cloud API SecretKey |
| `COS_BUCKET` | Bucket name, including the APPID suffix, e.g. `aiapp-1250000000` |
| `COS_REGION` | Region, e.g. `ap-guangzhou` |

Features and usage:

- **Upload / download**: `put` / `get_bytes` go through the COS XML API (V5 query signing, pure-Rust TLS, no OpenSSL needed in the runtime image).
- **URL pre-authorization (presigning)**:
  - The endpoint `GET /api/storage/presign?key=<key>&expires=<seconds>` (requires login) returns a temporary direct link carrying `q-signature`, valid for 10 minutes by default, usable for direct frontend download.
  - App wasm downloads (`/api/app/:id/wasm`) automatically 302-redirect to the presigned direct link when the backend is COS, so clients hit COS directly and save server bandwidth.
- When credentials are missing, startup warns and **falls back to local storage**; if `cos` is explicitly specified but credentials are still empty at call time, the specific method returns a clear error.
- Deployment (docker compose): set `STORAGE_BACKEND=cos` and `COS_*` in `deploy.env`; the orchestrator already passes these variables through to the `app` service.

### Multi-User Authentication

`aiapp-web` has built-in multi-user authentication, enabled by default:

- **Register / login**: `/api/auth/register`, `/api/auth/login`; on success a `JWT` (HS256, valid 7 days) is issued, passed via the `HttpOnly Cookie` (`aiapp_token`) or the `Authorization: Bearer` header.
- **Protected endpoints**: generation (`/api/generate`), my apps (`/api/my-apps`), publish/delete/report, etc. require login (`AuthUser`); admin endpoints require the admin role (`AdminUser`, otherwise 403).
- **Current user**: `/api/auth/me` returns the login state.
- **Admin seeding**: on first launch, the `ADMIN_USERNAME` / `ADMIN_PASSWORD` account is automatically created and persisted (to the default local SQLite or the configured `DATABASE_URL` target DB); the password can later be changed in the admin panel. Only when the DB connection/migration fails does it degrade to **dev mode**—injecting a single admin in memory (only `ADMIN_USERNAME`/`ADMIN_PASSWORD` can log in, registration disabled).

> In production, you MUST override the default values via `AUTH_SECRET` (strong random) and `ADMIN_PASSWORD` (non-default), otherwise startup prints a security warning.

### Deployment

Two delivery forms are supported: **Docker container** (recommended, with orchestration) and **running directly on the host**. In both cases, frontend static resources and runtime configuration are uniformly driven by environment variables.

#### Startup Pre-Build (first-run self-check)

On startup, the service **concurrently pre-builds the `main.wasm` of all example apps** and writes them to the storage backend (the first run blocks until completion, to verify the build environment). If the MoonBit toolchain is missing on the machine/in the container, it **auto-installs the official toolchain** (into `~/.moon/bin`; the container image pre-installs it); toolchain or template compilation failures only warn and do not block the HTTP service. When artifacts already exist (e.g., on restart) they are skipped automatically, ensuring idempotency.

#### Option 1: Docker / docker compose (recommended)

The repo contains a multi-stage `Dockerfile` (Rust build + MoonBit toolchain + frontend static resources) and a `docker-compose.yml` (PostgreSQL 16 + web service, starting the app only after the DB is ready; can also use SQLite, see the in-file comments).

```bash
# 1) Prepare deployment environment variables
cp deploy.env.example deploy.env
#    Edit deploy.env, at least set AUTH_SECRET / ADMIN_PASSWORD / POSTGRES_PASSWORD
#    (If switching to SQLite: set DATABASE_URL=sqlite:///data/aiapp.db, and comment out the db service and depends_on)

# 2) Build and start in the background (including PostgreSQL)
docker compose --env-file deploy.env up -d --build

# 3) Access
#    http://<server-IP>:8080
#    Persistence: pgdata (PostgreSQL) / appdata (/data: storage artifacts + moon toolchain cache + SQLite DB)
```

Key environment variables (full list in `deploy.env.example` and `docker-compose.yml`):

| Variable | Description |
|---|---|
| `DATABASE_URL` | `postgres://...` (default) or `sqlite:///data/aiapp.db` (zero-dependency) |
| `AUTH_SECRET` / `ADMIN_PASSWORD` | Required, strong random in production |
| `REQUIRE_DB` | Set `true` in PostgreSQL mode so a failed DB connection exits immediately, preventing silent degradation with no persistence (omit for SQLite) |
| `AIAPP_BACKEND` etc. | **Reserved**; web currently has mock built in; reserved for later wiring to a real OpenAI-compatible API |

#### Option 2: Run directly on the host

Suitable for environments without containers, or for integration with systemd / an existing reverse proxy.

```bash
# 1) Build
cargo build --release -p aiapp-web

# 2) Configure (write to .env or export)
#    PostgreSQL:
export DATABASE_URL="postgres://user:pass@localhost:5432/aiapp"
#    or local SQLite (no extra service needed):
# export DATABASE_URL="sqlite://./data/aiapp.db"
export REQUIRE_DB=true
export AUTH_SECRET="$(openssl rand -hex 32)"
export ADMIN_PASSWORD="strong password"
export HOST=0.0.0.0
export PORT=8080

# 3) Start (foreground; in production, manage with systemd / nohup)
./target/release/aiapp-web
```

> The static resource directory inside the container is `/app/templates` (`STATIC_DIR`); on the host it defaults to relative `./templates`. Make sure this directory sits next to the binary (or set an absolute `STATIC_DIR` explicitly).

### Reverse Proxy (recommended for production)

It is recommended to place the service behind Nginx / Caddy to terminate TLS and reverse-proxy `http://127.0.0.1:8080`. The cookie is already `HttpOnly`, so pairing it with HTTPS safely transmits the session.

## Planning Roadmap

1. The current version focuses on the Web side: after the user inputs a requirement, the system calls a large AI model to automatically generate MoonBit code and compile it to WASM, previewing the app live in the browser. Based on this, we will later expand to mobile and desktop runtimes. Main scenarios include:
   - Individual developers and small teams: quickly validate ideas, generate MVPs (minimum viable products) or internal tools
   - Enterprise users: quickly generate forms, dashboards, and lightweight business apps to improve digital efficiency
   - Education: as an aid for programming teaching and rapid prototyping
   - Government/enterprise: as a tool for emergency events, work-result showcases, etc.
2. Future planning: develop a cross-platform runtime container to achieve "build once, run on multiple platforms" (Android/iOS/HarmonyOS/PC); refine the `.aiapp` app package format to support offline distribution

## Core Feature Scope

- AI code generation: convert the user's natural-language description into MoonBit source via a large-model API (e.g., Claude/GPT)
- MoonBit → WASM compilation: invoke the MoonBit toolchain (`moon build --target wasm`) to compile source into WASM bytecode
- Web preview sandbox: load WASM in the browser via an iframe or Web runtime to show the generated app live
- App template library (basic): provide 3–5 preset templates (e.g., todo list, calculator, image filter) to lower the generation barrier
- App package export: support downloading the generated WASM + metadata as a package (early `.aiapp` format)


