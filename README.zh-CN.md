<p align="right">
  <strong>中文</strong> | <a href="README.md">English</a>
</p>

# aiapp-client

aiapp 生态的用户端模块 — Web 市场、后端 API、桌面端/移动端运行时。

## 架构

```
aiapp-client
├── crates/aiapp-web     后端 API + Web 市场前端
├── crates/aiapp-host     桌面端运行时（Tauri 壳）
├── host-app/             移动端原生代码（iOS Swift / Android Kotlin）
├── cmd/ + moon.mod       生成应用的 MoonBit 源码
├── storage/              WASM 产物存储
└── data/                 SQLite 数据库（元数据）
```

## 快速启动

```bash
# 1. 启动 Web 服务
cargo run -p aiapp-web

# 2. 打开 http://localhost:8080
#    默认管理员：admin / admin123
```

### Docker Compose

```bash
docker compose up -d
```

## 前置依赖

- Rust 1.75+
- [MoonBit 工具链](https://docs.moonbitlang.com)（首次运行自动安装）
- SQLite（默认，零配置）或 PostgreSQL（设置 `DATABASE_URL`）

## 环境配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `HOST` | `0.0.0.0` | 监听地址 |
| `PORT` | `8080` | 监听端口 |
| `DATABASE_URL` | `sqlite://./data/aiapp.db` | 数据库连接 |
| `AUTH_SECRET` | — | JWT 签名密钥（生产环境必填） |
| `ADMIN_PASSWORD` | `admin123` | 默认管理员密码（生产环境请修改） |
| `STORAGE_BACKEND` | `local` | 产物存储后端：`local`、`cos`、`oss` |

## 依赖

本仓库依赖 [aiapp-lib](https://github.com/wy-ent/aiapp-lib) 中的 crate：

- `aiapp-gen` — AI 生成引擎
- `aiapp-build` — WASM 构建工具
- `aiapp-format` — `.aiapp` 包格式定义
- `aiapp-engine` — WASM 运行时

## License

Apache-2.0