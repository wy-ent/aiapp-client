# aiapp-mb — OS 系统级的超级 AI 原生应用平台

让用户使用自然语言一键生成应用，**一次生成，多处运行**：web、安卓、iOS、鸿蒙、Windows、Mac、车机、电视盒等平台。目标是以**操作系统级别的架构**承载 AI 原生应用生态，让平台核心可独立部署，演进为 AI 原生操作系统。

**用户描述 → AI生成 → MoonBit源码 → WASM字节码 → .aiapp 统一应用包**；多端通过**统一 WIT 契约 + 宿主容器**（`aiapp-engine` / `aiapp-host`）播放同一份应用包。

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License">
  <img src="https://img.shields.io/badge/独立部署-supported-brightgreen" alt="独立部署">
  <img src="https://img.shields.io/badge/PRs-welcome-orange" alt="PRs Welcome">
</p>

**开源 · 可商用 · 支持独立部署** — 本项目完全开源，可自由用于商业用途。支持通过 Docker Compose 或直接运行独立部署，内置 SQLite 选项无需任何外部依赖。
本项目采用**开源核心（Open Core）**模式：核心能力完全开源（MIT），可自由使用、修改与商用，支持独立部署；Pro 商业服务（企业定制、托管部署、技术支持）用于反哺项目的长期开发与维护，让开源版本走得更久更好。

项目当前由作者业余时间维护，欢迎任何形式的贡献 —— Issue、PR、文档或使用反馈，都是对项目最好的支持。

## 路线图


### 第一阶段：Web MVP ✅（当前）

- [x] 端到端流水线：描述 → AI 生成 MoonBit 源码 → moon 编译 WASM → .aiapp 包
- [x] 6 种应用模板（minimal / todo / image-filter / pomodoro / memory-game / tv-movies）
- [x] 统一 `.aiapp` 应用包格式（清单 + WASM + WIT 契约 + 资源）
- [x] 多用户鉴权（注册/登录、JWT、管理员后台）
- [x] 应用分类 + 自动审核（工具/办公类自动通过，其余人工审核）
- [x] 应用分享与可见性（私有 / 组织 / 公开，分享链接 `?app=id`）
- [x] 匿名使用事件上报（遥测，为 Pro 分成/计费预留）
- [x] 联网/本地标签 + 「秒启动」一键启动
- [x] 「我的应用」自动收录启动过的应用；卸载即清本地数据（提示备份）

### 第二阶段：小程序容器 🚧（未开始）

- [x] `.aiapp` 统一格式 + WIT 契约（`aiapp-format`）
- [x] 运行时（`aiapp-engine`：权限门禁 + 宿主能力抽象 + Wasmtime 真实执行）
- [x] 桌面/命令行宿主容器（`aiapp-host`：meta / wasmtime 双执行模式）
- [x] 移动端宿主桥接层（`aiapp-host-bridge`：C-ABI，iOS(Swift) / Android(Kotlin+JNI) 复用 Rust 运行时）
- [x] WIT 契约扩展原生能力（网络 / 定位 / 相机 / 推送，见 `aiapp-format` 的 `wit.rs`）
- [ ] 宿主 App（iOS/Android），内嵌 WASM 运行时（桥接层已就绪，待端侧工程接入）
- [ ] 调用原生能力（相机、定位、推送）端到端打通

### 第三阶段：独立 App 生成 🚧（未开始）

- [ ] 桌面端打包（框架选型待定）
- [x] Capacitor 打包后端（移动端 .apk/.ipa）——`aiapp pack --target capacitor`
- [x] 品牌定制（名称 / 标识 / 图标自动生成）
- [ ] 100+ 应用生态

### 第四阶段：AI 原生操作系统

- [ ] 平台核心可独立部署
- [ ] 离线运行 AI 模型
- [ ] 去中心化应用分发协议

***

## 截图演示

|                          应用市场                          |                          应用生成                          |                           浏览器内运行                           |
| :----------------------------------------------------: | :----------------------------------------------------: | :--------------------------------------------------------: |
| ![应用市场](https://via.placeholder.com/400x250?text=应用市场) | ![应用生成](https://via.placeholder.com/400x250?text=应用生成) | ![浏览器内运行](https://via.placeholder.com/400x250?text=浏览器内运行) |

|                          移动端视图                         |                          后台管理                          |                          应用打包                          |
| :----------------------------------------------------: | :----------------------------------------------------: | :----------------------------------------------------: |
| ![移动端](https://via.placeholder.com/400x250?text=移动端视图) | ![后台管理](https://via.placeholder.com/400x250?text=后台管理) | ![应用打包](https://via.placeholder.com/400x250?text=应用打包) |

***

## 体验地址

> **在线体验**：`https://demo.aiapp.example.com` ← _替换为实际的体验地址_

体验完整流程：浏览应用市场、生成应用、在浏览器中运行。无需安装。

***

## 项目目标

本项目旨在构建一个 **OS 系统级的超级 AI 原生应用平台**。核心目标是让用户通过自然语言描述需求，即可一键生成可在 Web、安卓、iOS、鸿蒙、Windows、macOS、车机、电视盒等多端及分布式运行的单机或全栈应用程序；平台自身以操作系统级的架构承载应用生态，支持独立部署、离线运行 AI 模型与去中心化分发。

- 以 MoonBit 为主要开发语言，用于解析中间描述语言
- 生成 MoonBit 前端代码，构建统一标准格式的 app 包（类似 `.wasm` + `manifest.json`），包含：
  - WASM 字节码（业务逻辑）
  - 容器 app 接口逻辑（应用与宿主运行时的接口协议）

## 流水线

```
自然语言描述 → aiapp-gen 生成 MoonBit 工程 → aiapp-build 调 moon 编译 → .aiapp 包
                                                              │
                              ┌───────────────────────────────┼──────────────────────────────┐
                              ▼                               ▼                              ▼
         aiapp-host / 浏览器渲染器 / 宿主 App         aiapp-pack 打包独立 App             mobile host bridge
         ── 按 WIT 契约播放（一次生成，多处运行）       桌面 / Capacitor 移动      iOS(Swift)/Android(Kotlin)
```

| 阶段  | 组件                                            | 说明                                                                  |
| --- | --------------------------------------------- | ------------------------------------------------------------------- |
| 生成  | [aiapp-gen](crates/aiapp-gen)                 | 由自然语言描述生成可运行的 MoonBit 工程，支持模板选择（`mock` 后端；真实 AI 走闭源 Pro）            |
| 编译  | [aiapp-build](crates/aiapp-build)             | 调用 MoonBit 工具链 `moon build --target wasm-gc`，自动打包为 `.aiapp` 统一格式    |
| 编排  | [aiapp-cli](crates/aiapp-cli)                 | CLI 入口：`create` / `build` / `go` / `templates` / `pack`             |
| 格式  | [aiapp-format](crates/aiapp-format)           | `.aiapp` 统一应用包格式：清单 + WASM + **WIT 契约** + 资源                        |
| 运行时 | [aiapp-engine](crates/aiapp-engine)           | 运行时：权限门禁 + 宿主能力抽象 + 可选 Wasmtime 真实执行（开源"播放器"）                       |
| 宿主  | [aiapp-host](crates/aiapp-host)               | 桌面/命令行宿主容器：`info` / `validate` / `run` / `capabilities`，可真实执行 WASM  |
| 移动桥 | [aiapp-host-bridge](crates/aiapp-host-bridge) | 移动端宿主桥接层（C-ABI）：iOS(Swift) / Android(Kotlin+JNI) 宿主 App 复用 Rust 运行时 |
| 打包  | [aiapp-pack](crates/aiapp-pack)               | 独立 App 打包器：`.aiapp` → （桌面）/ Capacitor（移动）壳工程 + 品牌定制                 |
| 服务  | [aiapp-web](crates/aiapp-web)                 | Web 后端（axum）：应用市场 / 生成 / 后台管理 / 鉴权 / 分享 / 审核 / 遥测                   |

## 特性

- **统一应用包格式 (`.aiapp`)**：`aiapp.json` 清单 + `main.wasm` + `wit/app-host.wit`（应用与宿主通信契约）+ `resources/`
- **WIT 生态规则**：应用只声明能力（`storage` / `notifications` / `log`），各端渲染器实现同一接口 → 跨平台无需改码
- **预置应用模板**：6 种模板，覆盖办公效率、效率工具、亲子小游戏、影音娱乐、车机/电视端
- **权限模型**：`manifest.permissions` 声明所需权限，`aiapp-engine` 按权限门禁执行
- **双后端支持**：`mock` 后端离线可用；真实 AI 生成指向闭源 Pro（`AIAPP_BACKEND=pro`）
- **独立 App 生成（Phase 3）**：`aiapp pack` 把 `.aiapp` 一键生成 **Capacitor 移动** 壳工程，含品牌定制（名称 / 标识 / 图标自动生成）
- **联网/本地标签**：每个应用标注运行模式——「联网」需联网获取数据、「本地」数据保存在本机（卸载时删除本地数据），市场卡片与「我的应用」均展示徽章
- **秒启动**：市场卡片提供「⚡ 秒启动」一键打开应用，无需先收藏
- **我的应用自动收录**：点过「秒启动」的应用自动收录进「我的应用」（`installed` 字段记录，去重）；自己创建的应用可更新/发布/分享/删除，仅启动过的应用提供「🗑 卸载」
- **卸载即清数据**：卸载会同时删除该应用在本机保存的全部数据（待办、影音片单、最佳纪录等），卸载前明确提示数据不可恢复、可先行备份

## 模板

| 模板             | 说明                      |
| -------------- | ----------------------- |
| `minimal`      | 最小模板：Hello World 风格应用   |
| `todo`         | 待办事项：支持增删改查的待办事项管理      |
| `image-filter` | 图片滤镜：图片滤镜处理应用           |
| `pomodoro`     | 专注番茄钟：工作/休息计时，累积专注次数    |
| `memory-game`  | 亲子记忆翻牌：配对翻牌小游戏，记录最佳成绩   |
| `tv-movies`    | 家庭影音片单：分类收藏影片，电视/车机大屏友好 |

## 使用

需要 [MoonBit 工具链](https://www.moonbitlang.com/download)（提供 `moon` 命令）。

```bash
cargo build --release

# 列出可用模板
./target/release/aiapp templates

# 使用模板生成工程
./target/release/aiapp create "我的待办" -o generated/todo -t todo

# 一键端到端：描述 → MoonBit 工程 → WASM 字节码 → .aiapp 包
./target/release/aiapp go "hello world" -o generated/demo -t minimal

# 分步
./target/release/aiapp create "待办事项" -o generated/todo -t todo   # 只生成工程
./target/release/aiapp build generated/todo                            # 编译 + 打包 .aiapp

# Phase 3：把 .aiapp 一键生成独立应用工程（移动 Capacitor，含品牌定制）
./target/release/aiapp pack generated/todo.aiapp --target capacitor --name "我的待办" --icon icon.png
```

### 生成的工程结构

```text
generated/<app>/
  aiapp.json            # 统一应用包清单（app_id, name, version, permissions 等）
  moon.mod              # MoonBit 包声明
  cmd/main/
    moon.pkg            # 可执行包标记
    main.mbt            # 生成的入口源码
```

### 编译后的 .aiapp 包结构

```text
generated/<app>.aiapp/
  aiapp.json            # 应用清单
  main.wasm             # 编译后的 WASM 字节码
```

### AI 后端配置（环境变量）

默认使用 `mock` 后端（本地模板示例，离线可用）。切换真实 AI：

| 环境变量                    | 说明                 | 默认                          |
| ----------------------- | ------------------ | --------------------------- |
| `AIAPP_BACKEND`         | `mock` 或 `openai`  | `mock`                      |
| `AIAPP_OPENAI_BASE_URL` | OpenAI 兼容 API 基础地址 | `https://api.openai.com/v1` |
| `AIAPP_OPENAI_API_KEY`  | API Key            | 空                           |
| `AIAPP_OPENAI_MODEL`    | 模型名                | `gpt-4o-mini`               |

```bash
AIAPP_BACKEND=openai \
AIAPP_OPENAI_BASE_URL=https://api.openai.com/v1 \
AIAPP_OPENAI_API_KEY=sk-xxx \
AIAPP_OPENAI_MODEL=gpt-4o-mini \
./target/release/aiapp go "专注番茄钟" -o generated/tomato -t pomodoro
```

## 工程结构

```
crates/
  aiapp-cli/    # 二进制入口，编排流水线
  aiapp-gen/    # 生成器：模板 / mock 后端（社区版），产出 .aiapp 工程
  aiapp-build/  # 构建器：moon build → WASM → .aiapp 打包
  aiapp-format/ # 统一应用包格式：清单 / 打包 / 解析 / 校验 / WIT 契约定义
  aiapp-engine/ # 运行时：宿主能力抽象（Host trait）+ 权限门禁 + 应用执行（可选 Wasmtime 真实执行）
  aiapp-host/   # 桌面/命令行宿主容器：info / validate / run / capabilities
  aiapp-host-bridge/ # 移动端宿主桥接层（C-ABI）：iOS(Swift) / Android(Kotlin+JNI) 复用 Rust 运行时
  aiapp-pack/   # 独立 App 打包器：.aiapp → Capacitor 壳工程 + 品牌定制（图标自动生成）
  aiapp-web/    # Web 原型服务（axum）：应用市场 / 生成 / 后台管理 / 浏览器内运行
    src/
      main.rs     # HTTP 路由与各接口
      config.rs   # 运行时配置（数据库连接 / 存储后端），见下方「配置」
      db.rs       # 元数据持久层：默认本地 SQLite，可切 PostgreSQL
      storage.rs  # 统一产物存储抽象（local / 腾讯云 COS / 阿里云 OSS）
      auth.rs     # 鉴权：bcrypt 密码哈希 + JWT（HS256）
      telemetry.rs# 匿名使用事件上报（闭源 Pro 分成/计费预留）
      index.html  # 内嵌前端（应用市场、生成、后台、浏览器内 WASM 运行）
      static/     # sql.js（浏览器本地 SQLite 运行时），用 include_bytes! 内嵌
```

## 桌面/命令行宿主（aiapp-host，Phase 2 开源部分）

`aiapp-host` 是 `.aiapp` 应用包的宿主容器，实现 WIT 契约中的宿主能力（存储落本地文件、通知/日志输出终端）。它是「一次生成，多处运行」的桌面端形态；其它形态（宿主 App、独立 App、浏览器渲染器）实现同一套 WIT 接口即可复用应用。

```bash
cargo build -p aiapp-host                 # 轻量模式（元数据/生命周期）
cargo build -p aiapp-host --features wasmtime   # 启用真实 WASM 执行

./target/debug/aiapp-host capabilities                    # 列出 WIT 宿主能力目录
./target/debug/aiapp-host info demo.aiapp                 # 查看应用包信息（清单+校验）
./target/debug/aiapp-host validate demo.aiapp             # 校验应用包
./target/debug/aiapp-host run demo.aiapp --exec meta      # 轻量运行（校验+权限门禁）
./target/debug/aiapp-host run demo.aiapp --exec wasmtime  # 真实 WASM 执行（需 feature）
```

> 说明：MoonBit 默认 `wasm-gc` 目标产物含 const-expr GC 指令，wasmtime 24 尚不支持；用经典目标（`aiapp build <dir> --target wasm`）重新编译后即可在 `wasmtime` 模式真实执行（见 [aiapp-engine/src/wasmtime.rs](crates/aiapp-engine/src/wasmtime.rs) 的提示）。

### 统一应用包格式（aiapp-format）

`.aiapp` 包 = 清单（`aiapp.json`）+ WASM（`main.wasm`）+ WIT 契约（`wit/app-host.wit`）+ 可选资源（`resources/`）。`aiapp-format` 是格式的权威定义（打包/解析/校验），`aiapp-engine` 负责执行，二者配套，构成平台的"引擎"。

## 移动端宿主桥接（aiapp-host-bridge，Phase 2 开源部分）

`aiapp-host-bridge` 是移动端宿主桥接层（C-ABI），让 iOS（Swift）与 Android（Kotlin + JNI）宿主 App 直接复用 Rust 运行时播放 `.aiapp`。原生能力（存储 / 通知 / 网络 / 定位 / 相机 / 推送）以**回调函数指针**形式注入，实现 WIT 契约 `aiapp:app-host`。

```bash
# 构建 cdylib（macOS 上得到 libaiapp_host_bridge.dylib）
cargo build -p aiapp-host-bridge
```

```text
宿主 App 通过 FFI 调用：
  aiapp_bridge_create(callbacks, ctx)         # 创建宿主会话（注入原生能力回调）
  aiapp_bridge_load(bridge, pkg_path)         # 解析 .aiapp 包
  aiapp_bridge_run(bridge, "meta"|"wasmtime") # 运行应用
  aiapp_bridge_free(bridge)                   # 释放
```

> 线程模型：`aiapp_bridge_run` 内部使用独立 tokio 运行时执行引擎；回调在引擎线程被同步调用，原生侧如需更新 UI 应自行派发到主线程。

## 独立 App 生成（aiapp-pack，Phase 3 开源部分）

`aiapp pack` 把 `.aiapp` 一键生成可构建的独立应用工程，含品牌定制：

| 目标              | 壳工程                                                                                              | 产物                       |
| --------------- | ------------------------------------------------------------------------------------------------ | ------------------------ |
| `capacitor`（移动） | `www/` 内含 `main.wasm` + `aiapp.json`，浏览器壳在 WebView 内执行                                           | `.apk` / `.ipa`          |

- **品牌定制**：`--name` / `--identifier` / `--version` / `--author` / `--icon` / `--homepage`；未提供图标时**自动生成 512×512 品牌占位图标**（品牌主色 + 抽象应用球，2×2 超采样抗锯齿）。
- 品牌主色从清单/品牌名稳定推导，保证同一应用多端呈现一致。

```bash
./target/release/aiapp pack generated/tv.aiapp --target capacitor --name "家庭影音片单"

# 进入壳工程按平台构建
cd <out> && npm install && npx cap add android && npx cap build android
```

## Web 原型服务（aiapp-web）

员工自助原型：输入描述 + 选模板 → 生成 MoonBit 应用 → 应用市场预览 → 点击真实运行。

```bash
cd aiapp-mb
cargo run -p aiapp-web
# 浏览器打开 http://127.0.0.1:8080
```

- 默认 `mock` 后端离线可用，无需 API Key。
- 市场应用点击后在**浏览器内真实运行**编译出的 WASM（`/api/app/:id/wasm` 按需 `moon build` 返回）。
- 待办类应用使用\*\*浏览器本地 SQLite（sql.js + IndexedDB）\*\*持久化用户数据，重开不丢。
- 应用卡片展示**联网/本地**模式徽章与「⚡ 秒启动」按钮；启动过的应用自动收录进「我的应用」；卸载会删除该应用本机数据（详见下方「运行模式与我的应用」）。

### 架构

```
                 ┌─────────────────────────────────────────────┐
   浏览器前端 ──▶ │ aiapp-web (axum)                            │
                 │  • 应用市场 / 生成 / 后台管理                 │
                 │  • 内存热缓存（Vec<MarketApp>）               │
                 │    每个请求进入前以数据库为准刷新缓存         │
                 │        │ 读写同步                            │
                 │        ▼                                    │
                 │  ┌──────────────┐   ┌────────────────────┐  │
                 │  │ PostgreSQL   │   │ 存储抽象 Storage    │  │
                 │  │ 元数据:       │   │ local / COS / OSS   │  │
                 │  │ apps/users/   │   │ 产物: wasm 等        │  │
                 │  │ prompts       │   │ (key: apps/{id}/..) │  │
                 │  └──────────────┘   └────────────────────┘  │
                 └─────────────────────────────────────────────┘
```

- **元数据（应用/用户/提示词）**：存数据库（`db.rs`，统一用 `sqlx::Any` 驱动）。支持 **PostgreSQL**（`postgres://...`）或 **本地 SQLite**（`sqlite://...`），靠 `DATABASE_URL` 前缀自动选择；未配置 `DATABASE_URL` 时默认使用本地 SQLite 文件（`sqlite://./data/aiapp.db`），开箱即用且持久化，不再回退内存模式。`apps` 表含 `tier` 字段（`open` 开源 / `commercial` 闭源），用于开源/闭源分层。
- **数据一致性**：市场/用户列表走「内存热缓存 + 数据库持久层」，但**每个请求进入前先以数据库为准刷新缓存**（中间件 `refresh_cache` → `AppState::refresh`）。数据库是唯一真相源，因此多实例（如 8090/8092 双进程）或重启后各实例读到的都是最新提交的数据——一处注册/停用，各处立即可见。静态资源（`/static/*`）跳过刷新。
- **SPA 路由**：非 `/api/` 的未匹配路径（如直接访问 `/admin`）回落首页 HTML，避免直达白屏；未匹配 API 仍返回 404。
- **产物（wasm 等）**：走统一 `Storage` 抽象（`storage.rs`），由 `STORAGE_BACKEND` 切换 `local` / 腾讯云 COS / 阿里云 OSS，不再散落源码目录。

### 运行模式与「我的应用」

- **运行模式（`net`** **字段）**：`apps` 表新增 `net` 列（`联网` / `本地`）。`联网` 应用运行时需联网获取数据；`本地` 应用数据保存在本机浏览器，卸载时删除本地数据。
- **秒启动收录**：市场卡片提供「⚡ 秒启动」，点击即打开应用（`/api/app/:id`）并把它记入启动用户的「我的应用」（`users.installed` 列，JSON 数组，去重）。
- **我的应用**（`/api/my-apps`）返回「我创建 + 我启动过」的应用，附 `mine` 布尔标记：自己创建的显示更新/发布/分享/删除；仅启动过的显示「🗑 卸载」。
- **卸载**（`POST /api/uninstall`）：从 `installed` 移除该应用；前端同时清理本机数据——SQLite 中该 `app_id` 的全部记录、IndexedDB 键（影音片单 `tv_<id>`）、localStorage 键（记忆翻牌最佳纪录）。卸载前确认弹窗明确提示数据不可恢复、可先行备份。

### 配置：数据库连接与存储后端

配置来源：环境变量，可选仓库根目录 `.env` 文件（参考 `.env.example`）。**数据库连接通过** **`DATABASE_URL`** **配置**，按前缀切换驱动；未配置时默认使用本地 SQLite 文件（`sqlite://./data/aiapp.db`），数据持久化，开箱即用。

| 环境变量                                                             | 说明                                                                                                                                          | 默认                                        |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| `DATABASE_URL`                                                   | 元数据数据库连接串，按前缀切换驱动：`postgres://user:pass@host:5432/db` → PostgreSQL（生产推荐）；`sqlite://./data/aiapp.db` → 本地 SQLite 文件（零依赖，自动建表）。配置后元数据持久化，重启不丢 | `sqlite://./data/aiapp.db`（本地 SQLite，持久化） |
| `REQUIRE_DB`                                                     | 为 `true` 时，若连不上数据库则直接退出（生产配合编排器重试，避免静默降级无持久化丢数据；SQLite 文件库总能连上，可省略）                                                                         | `false`                                   |
| `HOST`                                                           | 监听地址：`0.0.0.0` 对外可访问，`127.0.0.1` 仅本机                                                                                                        | `0.0.0.0`                                 |
| `PORT`                                                           | 监听端口                                                                                                                                        | `8080`                                    |
| `STATIC_DIR`                                                     | 前端静态资源（模板缩略图等）目录，容器内为 `/app/templates`                                                                                                      | `./templates`                             |
| `AUTH_SECRET`                                                    | JWT 签名密钥，**生产必须显式配置强随机值**（如 `openssl rand -hex 32`）                                                                                         | 不安全默认值（告警）                                |
| `ADMIN_USERNAME`                                                 | 初始管理员用户名（首次启动落库）                                                                                                                            | `admin`                                   |
| `ADMIN_PASSWORD`                                                 | 初始管理员密码（首次启动落库；生产务必修改）                                                                                                                      | `admin123`（告警）                            |
| `STORAGE_BACKEND`                                                | 产物存储后端：`local` / `cos` / `oss`                                                                                                              | `local`                                   |
| `STORAGE_LOCAL_ROOT`                                             | `local` 后端根目录（已被 `.gitignore` 忽略）                                                                                                           | `./storage`                               |
| `COS_SECRET_ID` / `COS_SECRET_KEY` / `COS_BUCKET` / `COS_REGION` | 腾讯云 COS 凭证（`STORAGE_BACKEND=cos` 时填写，支持预签名）                                                                                                 | 空（回退 local）                               |
| `OSS_*`                                                          | 阿里云 OSS 凭证（预留，暂未接入）                                                                                                                         | 空                                         |

启动 PostgreSQL 并启用持久化示例：

```bash
# 方式 A：PostgreSQL（生产推荐）
createdb aiapp
export DATABASE_URL="postgres://user:pass@localhost:5432/aiapp"

# 方式 B：本地 SQLite（零依赖，文件即库，自动建表）
export DATABASE_URL="sqlite://./data/aiapp.db"

# 启动（首次会自动建表并写入种子数据）
cargo run -p aiapp-web
```

> 注：阿里云 OSS 后端目前仍为预留接口（返回明确报错），待填入真实凭证后接入；**腾讯云 COS 已完整接入并支持 URL 预签名**。

#### 腾讯云 COS 对象存储（已接入，支持 URL 预签名）

设置 `STORAGE_BACKEND=cos` 并通过以下变量配置凭证（占位符见 `.env.example` / `deploy.env.example`），应用产物（wasm 等）将存入 COS：

| 环境变量             | 说明                                   |
| ---------------- | ------------------------------------ |
| `COS_SECRET_ID`  | 云 API 密钥 SecretId（占位符 `AKID...`）     |
| `COS_SECRET_KEY` | 云 API 密钥 SecretKey                   |
| `COS_BUCKET`     | 存储桶名，含 APPID 后缀，如 `aiapp-1250000000` |
| `COS_REGION`     | 地域，如 `ap-guangzhou`                  |

特性与用法：

- **上传 / 下载**：`put` / `get_bytes` 走 COS XML API（V5 查询签名，纯 Rust TLS，运行镜像无需 OpenSSL）。
- **URL 预授权（预签名）**：
  - 接口 `GET /api/storage/presign?key=<key>&expires=<秒>`（需登录）返回带 `q-signature` 的临时直链，默认 10 分钟有效，可用于前端直连下载。
  - 应用 wasm 下载（`/api/app/:id/wasm`）在后端为 COS 时自动 302 重定向到预签名直链，客户端直连 COS，省服务端带宽。
- 凭证缺失时启动会告警并**回退 local 存储**；若显式指定 `cos` 但调用时凭证仍为空，具体方法返回明确错误。
- 部署（docker compose）：在 `deploy.env` 中设置 `STORAGE_BACKEND=cos` 与 `COS_*`，编排器已将这些变量透传到 `app` 服务。

### 多用户鉴权

`aiapp-web` 已内置多用户鉴权，默认开启：

- **注册 / 登录**：`/api/auth/register`、`/api/auth/login`；登录后下发 `JWT`（HS256，7 天有效），通过 `HttpOnly Cookie`（`aiapp_token`）或 `Authorization: Bearer` 头传递。
- **受保护接口**：生成（`/api/generate`）、我的应用（`/api/my-apps`）、发布/删除/上报等需登录（`AuthUser`）；后台管理接口需管理员角色（`AdminUser`，否则 403）。
- **当前用户**：`/api/auth/me` 返回登录态。
- **管理员种子**：首次启动自动创建 `ADMIN_USERNAME` / `ADMIN_PASSWORD` 账号并落库（默认本地 SQLite 或配置的 `DATABASE_URL` 目标库）；之后可在后台改密码。仅当数据库连接/迁移失败时才降级为**开发模式**——内存中注入单管理员（仅 `ADMIN_USERNAME`/`ADMIN_PASSWORD` 可登录，注册接口不可用）。

> 生产务必通过 `AUTH_SECRET`（强随机）和 `ADMIN_PASSWORD`（非默认）覆盖默认值，否则启动会打印安全告警。

### 部署

支持两种交付形态：**Docker 容器**（推荐，含编排）与**宿主机直接运行**。无论哪种，前端静态资源与运行配置都通过环境变量统一驱动。

#### 启动预构建（首次运行自检）

服务启动时会**并发预构建全部示例应用**的 `main.wasm` 并写入存储后端（首次运行会阻塞等待完成，便于校验构建环境）。若本机/容器内缺 MoonBit 工具链，会**自动安装官方工具链**（写入 `~/.moon/bin`，容器镜像已预装）；工具链或模板编译失败仅告警，不阻塞 HTTP 服务。产物已存在时（如重启）自动跳过，保证幂等。

#### 方式一：Docker / docker compose（推荐）

仓库内含多阶段 `Dockerfile`（Rust 编译 + MoonBit 工具链 + 前端静态资源）与 `docker-compose.yml`（PostgreSQL 16 + web 服务，数据库就绪后才启动 app；也可改用 SQLite，见文件内注释）。

```bash
# 1) 准备部署环境变量
cp deploy.env.example deploy.env
#    编辑 deploy.env，至少设置 AUTH_SECRET / ADMIN_PASSWORD / POSTGRES_PASSWORD
#    （如改用 SQLite：设 DATABASE_URL=sqlite:///data/aiapp.db，并注释 db 服务与 depends_on）

# 2) 构建并后台启动（含 PostgreSQL）
docker compose --env-file deploy.env up -d --build

# 3) 访问
#    http://<服务器IP>:8080
#   数据持久化：pgdata（PostgreSQL）/ appdata（/data：storage 产物 + moon 工具链缓存 + SQLite 库）
```

关键环境变量（完整见 `deploy.env.example` 与 `docker-compose.yml`）：

| 变量                               | 说明                                                    |
| -------------------------------- | ----------------------------------------------------- |
| `DATABASE_URL`                   | `postgres://...`（默认）或 `sqlite:///data/aiapp.db`（零依赖）  |
| `AUTH_SECRET` / `ADMIN_PASSWORD` | 必填，生产强随机                                              |
| `REQUIRE_DB`                     | PostgreSQL 模式设为 `true`，连不上库即退出，杜绝静默降级无持久化（SQLite 可省略） |
| `AIAPP_BACKEND` 等                | **预留**，web 当前内置 mock；留待后续接入真实 OpenAI 兼容 API           |

#### 方式二：宿主机直接运行

适合无容器环境或纳入 systemd / 现有反向代理。

```bash
# 1) 编译
cargo build --release -p aiapp-web

# 2) 配置（写入 .env 或 export）
#    PostgreSQL：
export DATABASE_URL="postgres://user:pass@localhost:5432/aiapp"
#    或本地 SQLite（无需额外服务）：
# export DATABASE_URL="sqlite://./data/aiapp.db"
export REQUIRE_DB=true
export AUTH_SECRET="$(openssl rand -hex 32)"
export ADMIN_PASSWORD="强密码"
export HOST=0.0.0.0
export PORT=8080

# 3) 启动（前台；生产建议用 systemd / nohup 托管）
./target/release/aiapp-web
```

> 容器内静态资源目录为 `/app/templates`（`STATIC_DIR`），宿主机默认相对 `./templates`，请确认该目录与二进制同目录（或显式设置 `STATIC_DIR` 绝对路径）。

### 反向代理（生产建议）

对外建议放在 Nginx / Caddy 之后，终止 TLS 并反代 `http://127.0.0.1:8080`。Cookie 已设为 `HttpOnly`，配合 HTTPS 即可安全传递会话。

## 规划路线

1. 当前版本聚焦于 Web 端：用户输入需求后，系统调用 AI 大模型自动生成 MoonBit 代码并编译为 WASM，在浏览器中实时预览应用效果。后续将基于此开发扩展至移动端与桌面端运行时。主要应用场景包括：
   - 个人开发者与小型团队：快速验证想法，生成 MVP（最小可行产品）或内部工具
   - 企业用户：快速生成表单、仪表盘、轻量级业务应用，提升数字化效率
   - 教育领域：作为编程教学和快速原型的辅助工具
   - 政企领域：作为应急事件、工作成果展示等工具
2. 后续规划：开发跨平台运行时容器，实现"一次生成，多端运行"（Android/iOS/鸿蒙/PC）；完善 `.aiapp` 应用包格式，支持离线分发

## 核心功能范围

- AI 代码生成：通过大模型 API（如 Claude/GPT）将用户自然语言描述转换为 MoonBit 源码
- MoonBit → WASM 编译：调用 MoonBit 工具链（`moon build --target wasm`）将源码编译为 WASM 字节码
- Web 预览沙箱：在浏览器中通过 iframe 或 Web 运行时加载 WASM，实时展示生成的应用
- 应用模板库（基础）：提供 3-5 个预设模板（如待办清单、计算器、图片滤镜），降低生成门槛
- 应用包导出：支持将生成的 WASM + 元数据打包下载（`.aiapp` 格式雏形）



