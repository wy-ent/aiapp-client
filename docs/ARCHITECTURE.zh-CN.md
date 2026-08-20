# aiapp-mb 架构说明

> 本文档随每次架构调整更新。本文对应 Phase 1: Web MVP + 多用户鉴权 + 可部署后端 + Phase 2 开源部分（格式 / 运行时 / 宿主容器 / 移动桥接）+ Phase 3 开源部分（独立 App 打包）。

## 目标

把 `aiapp-mb` 作为**可部署的后端服务**运行：用户通过 Web 访问应用市场、生成并运行 AI 应用；未来可由容器化的宿主 App 复用同一后端。

核心原则：

1. **一次生成，多处运行**：描述 → AI 生成 MoonBit 源码 → `moon` 编译 WASM → `.aiapp` 统一包；各端通过统一 WIT 契约播放。
2. **可部署**：监听地址/端口可配置（`HOST`/`PORT`），静态资源路径可配（`STATIC_DIR`）。
3. **可持久化**：元数据走 PostgreSQL 或 SQLite（`sqlx::Any`，靠 `DATABASE_URL` 前缀切换），产物走统一 `Storage` 抽象（local / COS / OSS）。
4. **多用户**：内置注册/登录，JWT 鉴权；后台管理需管理员角色；组织级共享。
5. **可容器化**：多阶段 Dockerfile + docker-compose，宿主机模式同样受支持。

## 组件总览

```
自然语言描述 → aiapp-gen 生成 MoonBit 工程 → aiapp-build 调 moon 编译 → .aiapp 包
                                                                          │
      aiapp-host / 浏览器渲染器 / 宿主 App ── 按 WIT 契约播放（一次生成，多处运行）
```

| 阶段 | 组件 | 说明 |
|---|---|---|
| 生成 | crates/aiapp-gen | 由描述生成可运行的 MoonBit 工程（模板 / mock 后端，社区版） |
| 编译 | crates/aiapp-build | `moon build --target wasm-gc` → 打包 `.aiapp` |
| 编排 | crates/aiapp-cli | CLI 入口：`create` / `build` / `go` / `templates` / `pack` |
| 格式 | crates/aiapp-format | `.aiapp` 统一包格式：清单 / 打包 / 解析 / 校验 / WIT 契约 |
| 运行时 | crates/aiapp-engine | 运行时：`Host` trait（宿主能力抽象）+ 权限门禁 + 执行入口（可选 Wasmtime 真实执行） |
| 宿主 | crates/aiapp-host | 桌面/命令行宿主容器：`info`/`validate`/`run`/`capabilities`，可选 Wasmtime 真实执行 |
| 移动桥 | crates/aiapp-host-bridge | 移动端宿主桥接层（C-ABI）：iOS(Swift) / Android(Kotlin+JNI) 复用 Rust 运行时 |
| 打包 | crates/aiapp-pack | 独立 App 打包器：`.aiapp` → Capacitor（移动）壳工程 + 品牌定制 |
| 服务 | crates/aiapp-web | Web 后端（axum）：市场 / 生成 / 后台 / 浏览器内运行 / 鉴权 / 分享 / 审核 / 遥测 |

## aiapp-web 后端架构

```
                ┌──────────────────────────────────────────────────────┐
   浏览器前端 ─▶ │ aiapp-web (axum on HOST:PORT)                        │
                │  • 应用市场 / 生成 / 后台 / 浏览器内 WASM 运行         │
                │  • 路由鉴权：AuthUser / AdminUser 提取器              │
                │  • 内存热缓存（Arc<Mutex<Vec<AppUser>>> / apps）       │
                │        │ 写操作同步                                    │
                │        ▼                                              │
                │  ┌──────────────────┐   ┌──────────────────────────────┐  │
                │  │ PostgreSQL/SQLite │   │ Storage 抽象                  │  │
                │  │ 元数据:            │   │ local / COS / OSS            │  │
                │  │ apps/users/       │   │ 产物: wasm 等                 │  │
                │  │ prompts           │   │ key: apps/{id}/main.wasm     │  │
                │  └──────────────────┘   └──────────────────────────────┘  │
                └──────────────────────────────────────────────────────┘
```

### 模块划分（crates/aiapp-web/src）

| 文件 | 职责 |
|---|---|
| `main.rs` | HTTP 路由、各接口实现、`AppState`、`AppUser`、`ensure_admin`、开发模式注入单管理员 |
| `config.rs` | `AppConfig`：从环境变量 + `.env` 加载（HOST/PORT/DATABASE_URL/REQUIRE_DB/存储/鉴权） |
| `db.rs` | 元数据持久层（`users`/`apps`/`prompts`），统一 `sqlx::Any` 驱动；`DATABASE_URL` 按前缀选 **PostgreSQL** 或 **SQLite**，未配默认本地 SQLite（`sqlite://./data/aiapp.db`） |
| `auth.rs` | 鉴权：`bcrypt` 密码哈希 + `jsonwebtoken`（HS256）签发/校验；`AuthUser`/`AdminUser` 提取器；`extract_token`（Bearer 或 `aiapp_token` Cookie） |
| `storage.rs` | 统一产物存储抽象（local / 腾讯云 COS / 阿里云 OSS） |
| `index.html` | 内嵌前端（市场、生成、登录/注册 modal、浏览器内 WASM 运行） |
| `static/` | sql.js（浏览器本地 SQLite 运行时），`include_bytes!` 内嵌 |

### 鉴权模型

- **密码**：`bcrypt` 哈希，`users` 表 `password_hash TEXT NOT NULL DEFAULT ''`。
- **令牌**：登录成功签发 JWT（HS256，有效期 7 天），签名密钥 `AUTH_SECRET`。
- **传递**：`HttpOnly Cookie`(`aiapp_token`) 或 `Authorization: Bearer <token>`。
- **提取器**：
  - `AuthUser`：任意已登录用户；缺失/失效 → 401。
  - `AdminUser`：`role=admin` 的用户；非管理员 → 403。
- **接口保护**：
  - 需登录：`/api/generate`、`/api/my-apps`、`/api/publish`、`/api/delete`、`/api/uninstall`、`/api/report`。
  - 需管理员：`/api/admin/*`（如后台统计、用户管理）。
  - 公开：市场列表/详情、WASM 下载、注册、登录、`/api/auth/me`。

### 运行模式

| 模式 | 触发 | 行为 |
|---|---|---|
| 数据库模式（默认） | 未配 `DATABASE_URL` 默认本地 SQLite，或显式配置 PostgreSQL/SQLite | 加载库内用户/应用；`ensure_admin()` 保证管理员账号存在并落库；注册开放 |
| 降级开发模式 | 数据库连接/迁移失败 | 内存注入单管理员（`ADMIN_USERNAME`/`ADMIN_PASSWORD`），注册接口不可用 |
| 强制数据库 | `REQUIRE_DB=true` 且连库失败 | 直接 `exit(1)`，避免静默降级无持久化导致数据不持久 |

### 启动预构建（首次运行自检）

服务启动时（`router()` 内）会**并发预构建全部尚无产物的种子示例** `main.wasm` 并写入存储后端：

- **两种模式都触发**：持久化模式与降级开发模式均在首次启动执行，确保「部署后示例立即可用」。
- **工具链自检**：启动期调用 `moon::ensure_moon_toolchain()`，按 `PATH → ~/.moon/bin → 自动安装` 顺序解析 `moon`（官方脚本 `cli.moonbitlang.com/install/unix.sh`）；缺失则自动安装并告警，不阻塞服务。
- **阻塞等待**：首次运行 `JoinSet` 并发构建并 await 完成，便于在部署日志中直接看到构建环境是否正常（编译失败仅告警，HTTP 仍正常启动）。
- **幂等**：产物已存在（如重启、COS 对象仍在）则跳过编译；`build_and_store_wasm` 对 `local` 后端先探测已有产物直接标记，避免重复构建。

### 应用分类 / 审核 / 分享（Phase 1 扩展）

- **分类**：应用注册时可选择分类（`category`）；未选择时按模板自动归类（`default_category_for`）。
- **自动审核**：`category_requires_review(cat)` 判定——工具/办公类（`tool` / `office`）**自动审核通过**并直接发布；其余分类进入 `reviewing` 状态，需管理员在后台 `/api/admin/app/review` 人工审批（通过/驳回带意见）。
- **分享与可见性**：应用 `share` 三态：`private`（仅自己）、`org`（同组织可见）、`public`（公开到市场）。`list_market` 按可见性过滤（公开 + 同组织），`app_detail` 校验访问权。分享链接通过 `?app=<id>` 参数直达。
- **生命周期**：`draft`（草稿）/ `reviewing`（审核中）/ `published`（已发布）/ `disabled`（已停用），管理员可停用/启用。

### 运行模式标签 / 秒启动 / 我的应用 / 卸载（Phase 1 扩展）

- **运行模式标签**：`MarketApp.net`（`联网` / `本地`，`apps` 表 `net` 列，默认 `本地`）。市场卡片与「我的应用」展示徽章：`联网` 应用运行时需联网获取数据；`本地` 应用数据保存在本机浏览器。
- **秒启动收录**：市场卡片「⚡ 秒启动」直达 `app_detail`（`/api/app/:id`）；打开应用时把应用记入启动用户的「我的应用」（`AppUser.installed: Vec<String>`，`users` 表 `installed` 列 JSON 数组，去重），并持久化到 DB。
- **我的应用**（`/api/my-apps`）：返回「我创建 + 我启动过」的应用，附 `mine` 布尔标记。前端据此渲染：自己创建的显示更新/发布/分享/删除；仅启动过的显示「🗑 卸载」。
- **卸载**（`POST /api/uninstall`，需登录）：从当前用户的 `installed` 移除该应用并落库（不在「我的应用」中则报错）。前端卸载成功后同步清理本机数据——浏览器 SQLite 中该 `app_id` 的全部记录、IndexedDB 键 `tv_<id>`（影音片单）、localStorage `mem_best`（记忆翻牌纪录）；确认弹窗明确提示数据不可恢复、可先备份。

### 遥测上报（为 Pro 分成 / 计费预留）

开源侧（企业部署）只负责**产生匿名使用事件**并上报闭源 Pro 服务（`telemetry.rs`，`POST {PRO}/v1/telemetry`），不接触企业隐私数据；字段用 versioned schema（`docs/telemetry.schema.v1.md`）约定。上报失败静默，不影响主流程。

## aiapp-format / aiapp-engine / aiapp-host / aiapp-host-bridge（Phase 2 开源部分）

这些组件构成平台的**运行时引擎**（开源，吸引生态）：

### aiapp-format — 统一应用包格式（生态规则）

```
<name>.aiapp/
  aiapp.json      # 清单：元数据 / 权限声明 / 入口 / WIT 版本（AppManifest）
  main.wasm       # 业务逻辑（MoonBit → WASM）
  wit/app-host.wit # 应用与宿主通信契约（WIT，包内自带副本，自包含）
  resources/      # 可选资源（图片、样式等）
```

- 模块：`manifest.rs`（清单结构 + 校验）、`package.rs`（打包/解析）、`validate.rs`（校验报告）、`wit.rs`（WIT 契约 + 宿主能力目录）。
- `WIT_VERSION = "0.1.0"`：`package aiapp:app-host`，`interface host` 定义 `show-notification` / `save-data` / `load-data` / `log`；`interface app` 定义生命周期 `run` / `stop`。
- `HOST_CAPABILITIES`：能力目录（`storage` / `notifications` / `log`），对应 `manifest.permissions` 声明，是各端实现 WIT 的对照表。
- 原生能力扩展：WIT 已扩展 `network` / `location` / `camera` / `push` 等宿主能力定义，供移动端宿主实现（见 `wit.rs`）。

### aiapp-engine — 运行时（"播放器"）

- `host::Host`：宿主能力抽象（async trait：`show_notification` / `save_data` / `load_data` / `log` / 原生能力），各端实现（网页版 → Web API，宿主 App → 原生，桌面 → 系统）。
- `permissions`：权限门禁，按清单声明授权（`Granted` / `NotDeclared` / `Denied`），未授予的能力在启动时预警。
- `runtime::Runtime`：加载 `.aiapp` 包 → 校验 → 建权限门禁 → 执行入口。无 `wasmtime` feature 时为轻量模式（生命周期钩子），便于预览/调试。
- `wasmtime.rs`（feature `wasmtime`）：Wasmtime + WASI 真实执行 `main.wasm`（`_start` 入口），用内部 tokio runtime 桥接 async Host 与同步回调。`aiapp-host` 与 `aiapp-host-bridge` 均复用此执行入口。

### aiapp-host — 桌面/命令行宿主容器

- `desktop_host.rs`：实现 `Host` trait——数据落本地文件（`safe_path` 防目录穿越）、通知/日志输出终端。
- CLI：`info` / `validate` / `run`（`--exec meta|wasmtime`，`--grant` 权限，`--data-dir` 数据隔离）/ `capabilities`。
- 已知限制：MoonBit `wasm-gc` 目标产物含 const-expr GC 指令（`array.new_default`），wasmtime 24 尚不支持；用 `--target wasm` 经典目标重新编译即可真实执行（错误信息中已给出提示）。

### aiapp-host-bridge — 移动端宿主桥接层（C-ABI）

- 目标：iOS（Swift）与 Android（Kotlin + JNI）宿主 App 通过 `cdylib` 调用 Rust 运行时播放 `.aiapp`。
- 原生能力（存储 / 通知 / 网络 / 定位 / 相机 / 推送）以**回调函数指针**注入，实现 WIT 契约 `aiapp:app-host`。
- FFI 接口：`aiapp_bridge_create(callbacks, ctx)` / `aiapp_bridge_load(bridge, pkg_path)` / `aiapp_bridge_run(bridge, "meta"|"wasmtime")` / `aiapp_bridge_free(bridge)`。
- 线程模型：`aiapp_bridge_run` 内部使用独立 tokio 运行时执行引擎；回调在引擎线程被同步调用，原生侧如需更新 UI 自行派发到主线程。

## aiapp-pack（Phase 3 开源部分）— 独立 App 打包器

`aiapp pack` 把 `.aiapp` 一键生成可构建的独立应用工程，含品牌定制：

| 目标 | 壳工程 | 产物 |
|---|---|---|
| `capacitor`（移动） | `www/` 内含 `main.wasm` + `aiapp.json`，浏览器壳在 WebView 内执行 | `.apk` / `.ipa` |

- 模块：`lib.rs`（`pack()` 入口 + `PackTarget` / `PackConfig`）、`brand.rs`（品牌推导 + 占位图标生成）、`capacitor.rs`（壳工程模板）。
- 品牌定制：`--name` / `--identifier` / `--version` / `--author` / `--icon` / `--homepage`；未提供图标时自动生成 512×512 品牌占位图标（品牌主色 + 抽象应用球，2×2 超采样抗锯齿），同一应用多端呈现一致。
- 输出目录非空时报错，避免覆盖已有工程。

### 存储抽象

产物（wasm 等）通过 `Storage` trait 统一读写，由 `STORAGE_BACKEND` 切换：

- `local`：写入 `STORAGE_LOCAL_ROOT`（默认 `./storage`，已被 `.gitignore` 忽略）。
- `cos`：腾讯云对象存储（已接入）。走 COS XML API 的 V5 查询签名（HMAC-SHA1），`put`/`get_bytes` 上传下载；支持 **URL 预签名**（`presigned_url` 方法）——`/api/storage/presign` 返回带 `q-signature` 的临时直链，`/api/app/:id/wasm` 在 COS 后端自动 302 重定向到该直链。凭证来自 `COS_SECRET_ID`/`COS_SECRET_KEY`/`COS_BUCKET`/`COS_REGION`，缺失则启动告警并回退 local。
- `oss`：阿里云 OSS（预留接口，缺凭证时返回明确报错）。

key 约定：`apps/{id}/main.wasm` 等。

## 部署架构

### 容器（Docker）

多阶段 `Dockerfile`：

- **builder**（`rust:1.83-bookworm`）：安装 `moon` 工具链并预热 `wasm-gc`；`cargo build --release -p aiapp-web`。
- **runtime**（`debian:bookworm-slim`）：复制二进制、`/app/templates` 静态资源、`/root/.moon` 工具链；`/data` 为持久卷（storage 产物 + moon 缓存）。

`docker-compose.yml`：

- `db`：`postgres:16`，带 healthcheck，数据卷 `pgdata`。
- `app`：依赖 `db` healthy 后启动；环境变量通过 `deploy.env`（`--env-file`）注入；数据卷 `appdata` 挂载 `/data`；`REQUIRE_DB=true` 确保连库失败即退出。

```
宿主机 :8080 ──▶ app 容器 (aiapp-web) ──┬─▶ PostgreSQL 容器 (pgdata 卷)
                                        └─▶ /data 卷 (storage + moon)
```

### 宿主机

`cargo build --release -p aiapp-web` 后直接运行，环境变量驱动（`HOST`/`PORT`/`DATABASE_URL`/`AUTH_SECRET` 等），建议用 systemd / nohup 托管，并放在 Nginx/Caddy 之后终止 TLS。

## 配置项速查

见 `README.md`「配置」表格与 `deploy.env.example` / `.env.example`。

## 后续演进

- 真实 OpenAI 兼容后端接入（`AIAPP_BACKEND=openai` 预留，需打通 aiapp-gen 的 openai 路径到 web 生成流；当前真实生成走闭源 Pro `aiapp-gen-pro` + `aiapp-pro-server`）。
- 阿里云 OSS 存储后端接入（COS 已完成）。
- 宿主 App（iOS/Android）：`aiapp-host-bridge`（C-ABI）已就绪，复用本后端 API 拉取 `.aiapp` 包，在端侧实现同一套 WIT 宿主能力（`aiapp-host` 桌面版为参考实现）；待端侧工程（Swift / Kotlin 脚手架）接入。
- 独立 App 生成（Phase 3）：`aiapp pack` 已落地 Capacitor（移动）壳工程生成 + 品牌定制；后续可直接跑 `npx cap build android` 产出安装包。桌面端打包框架选型待定。
- 数据可迁移：网页版（IndexedDB）与宿主 App（SQLite）/桌面端（文件）通过 WIT `save-data`/`load-data` 实现跨端数据迁移。
- 100+ 应用生态：扩充模板与生成技能，丰富市场。
