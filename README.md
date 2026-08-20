# aiapp-client

User-facing module of the aiapp ecosystem — Web market, backend API, and desktop/mobile runtime.

## Architecture

```
aiapp-client
├── crates/aiapp-web     Backend API + Web market frontend
├── crates/aiapp-host     Desktop runtime (Tauri shell)
├── host-app/             Mobile native code (iOS Swift / Android Kotlin)
├── cmd/ + moon.mod       MoonBit source code for generated apps
├── storage/              WASM artifact storage
└── data/                 SQLite database (metadata)
```

## Quick Start

```bash
# 1. Start the web server
cargo run -p aiapp-web

# 2. Open http://localhost:8080
#    Default admin: admin / admin123
```

### Docker Compose

```bash
docker compose up -d
```

## Prerequisites

- Rust 1.75+
- [MoonBit toolchain](https://docs.moonbitlang.com) (auto-installed on first run)
- SQLite (default, zero config) or PostgreSQL (set `DATABASE_URL`)

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | `0.0.0.0` | Listen address |
| `PORT` | `8080` | Listen port |
| `DATABASE_URL` | `sqlite://./data/aiapp.db` | Database connection |
| `AUTH_SECRET` | — | JWT signing secret (required in production) |
| `ADMIN_PASSWORD` | `admin123` | Default admin password (change in production) |
| `STORAGE_BACKEND` | `local` | Artifact storage: `local`, `cos`, `oss` |

## Dependencies

This project depends on crates from [aiapp-lib](https://github.com/wy-ent/aiapp-lib):

- `aiapp-gen` — AI generation engine
- `aiapp-build` — WASM build toolchain
- `aiapp-format` — `.aiapp` package format
- `aiapp-engine` — WASM runtime

## License

Apache-2.0