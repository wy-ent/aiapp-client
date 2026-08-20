# PROJECT_DESCRIPTION.md — Project Description (for humans and AI collaborators)

> This document lets **any AI tooling / developer contributor** understand the project in full without needing access to the Feishu docs.
> Companion documents are listed in the "Document Index" at the end. This document is iterated continuously; keep it in sync with the code implementation.

---

## 1. One-Sentence Summary

**aiapp-mb | OS-level super AI-native application platform**: an operating-system-grade architecture that hosts the AI-native application ecosystem—users generate apps from natural language with a single click, build once and run everywhere—covering Web, Android, iOS, HarmonyOS, Windows, macOS, in-car systems, TV boxes, and all platforms, producing a unified-format app package `.aiapp`. The platform core is independently deployable, evolving toward an AI-native operating system with offline AI models and decentralized distribution.

```
Natural language description → aiapp-gen generates a MoonBit project → aiapp-build compiles → .aiapp unified app package (runs in Web / multi-platform containers)
```

Core development language: **MoonBit** (parsing the intermediate description, generating frontend code, constructing the unified app package); the host container side will later be implemented in Rust.

## 2. The Unified App Package Format `.aiapp`

- WASM bytecode (business logic) + container app interface logic (the interface protocol between the app and the host runtime)
- Key design: `aiapp.json` (manifest, describing app_id / name / version / permissions, etc.) + `main.wasm`
- Runtime and format are open-sourced to build ecosystem and trust; generation skills and operational data are closed-source for monetization (see §4)

## 3. Current Features (simulated single-user prototype)

- **Marketplace home**: template thumbnails/preview display, tag-based categorization, platform filtering, a "current platform not supported" fallback, and a "simulated open App" style interaction when clicking an app (minimize to background / close back to home)
- **Generator**: new app / create based on an existing app as reference / update an existing app (after update, owner approval is required before publishing); generation-skill prompts can be edited and iterated online in the admin panel
- **Admin management system**: app review (approve/reject + comment), listing, disabling, restoring, editing, deletion; user management and enable/disable; usage statistics (total apps, launch count, registered/active users, reports, incentive reserve); view/edit/restore generation prompts
- **Reporting mechanism**: app details allow filling in a reason to report, persisted to the DB and visible in the admin panel

Components: `crates/aiapp-web` (frontend marketplace + admin) / `aiapp-gen` (community-edition generation) / `aiapp-build` (compile & package) / `aiapp-cli` (CLI).

## 4. Commercialization Architecture: Open-Source Community Edition + Closed-Source Pro

> Core principle: **separate "open-sourceable" from "must-be-closed" early**, physically isolating them into independent repositories, so we can later confidently open-source to grow the ecosystem while keeping closed-source for revenue (incentives / ad revenue share / billing). Analogous to AOSP (open source) + GMS (closed source).

### 4.1 Two Deployment Planes (the split of the admin system)
- **Enterprise local admin (open-source community edition)**: deployed by the enterprise itself, managing its **own** members and local app marketplace.
- **Global operations admin (closed-source Pro / SaaS)**: the operator centrally handles **cross-enterprise** statistics, user incentives, ad revenue share, and billing authorization—i.e., the "toll booth."

### 4.2 Open Source (public repository, MIT / Apache 2.0)
- `.aiapp` app package format spec, runtime engine / Player, developer tool `aiapp-cli`, MoonBit example apps
- Enterprise self-hosted service + enterprise local admin (single-tenant management of the enterprise's users/apps/reviews)
- Generation capability **interface contracts** (interfaces only, no implementation)
- Community offline generation: Mock / templates (`aiapp-gen` community edition), `aiapp-web`, `aiapp-build`, `docs`

### 4.3 Closed Source (private repo `pro/`, core commercial value)
- Real generation engine `aiapp-gen-pro`: LLM generation engine + **iterable Prompt DNA** (versioned, structurally partitioned, continuously iterated via online admin editing, taking immediate effect on generation)
- Global operations admin `aiapp-saas` (planned): cross-enterprise statistics, incentives / ad revenue share, billing authorization
- Official premium marketplace, model keys, intranet addresses, Prompt DNA—all sealed off

> An iron rule: **runtime + format are open-sourced to build ecosystem and trust; generation skills + operational data are closed-source for monetization.**

### 4.4 Repository / Directory Mapping
The current single repo is the open-source repo; closed-source lives in `pro/` (already in `.gitignore`; you can `cd pro && git init` to push it as a separate private repo).

```
Current open-source repo
├─ crates
│  ├─ aiapp-format/      (planned) app package format    → open source
│  ├─ aiapp-engine/      (planned) runtime engine         → open source
│  ├─ aiapp-cli/                developer tool            → open source
│  ├─ aiapp-web/                marketplace + enterprise admin → open source (community edition)
│  ├─ aiapp-build/              build & package           → open source
│  └─ aiapp-gen/               generation (community)     → open source (Mock/templates only)
└─ pro/                            ← closed-source private repo (pushed separately)
   ├─ crates/aiapp-gen-pro/        real generation engine + Prompt DNA → closed source
   └─ crates/aiapp-saas/   (planned) global ops admin/statistics/share/billing → closed source
```

The private `pro` repo reuses the open-source repo's public types via path/git; after the split it becomes `aiapp-gen = { git = "<open-source repo>" }`.

### 4.5 Capability Layering

| Capability | Community (open source) | Pro (closed source) |
|---|---|---|
| Generation (AI automatic) | ❌ Mock/templates only (`AIAPP_BACKEND=mock`) | ✅ OpenAI-compatible + prompt iteration |
| Enterprise local admin | ✅ Manage the enterprise's own users/apps/reviews | ★= (ships with the runtime deployment) |
| Global ops admin / share / incentive / billing | ❌ | ✅ `aiapp-saas` |
| Official marketplace | Display published apps | Upload premium/commercialized apps |

### 4.6 Core Control Points (moat)
- **Open-source hooks**: WASM runtime, MoonBit compiler, basic system libraries—to attract developers.
- **Closed-source lifeblood**: AI generation engine, security sandbox, distributed scheduling, app store, cross-app protocols—missing any one makes the open-source core an "empty shell."
- **Ecosystem lock-in**: apps developed on the open-source core must go through closed-source distribution channels and APIs/SDKs to reach users.

### 4.7 Event Reporting Contract (reserved for share / ad / billing)
The open-source side (enterprise deployment) only produces and **anonymously reports** events to the closed-source SaaS: `app_launch`, `app_generate`, `app_report`, `user_register`; the closed-source side aggregates them into statistics and share/billing bills. Enterprises can disable reporting (purely private), they just forgo the share/cloud statistics. Event fields follow a versioned schema (`docs/telemetry.schema.v1.md`).

### 4.8 Phased Rollout
1. **Phase 1 (done)**: physical separation—`aiapp-gen` keeps only community capabilities; the real engine + Prompt DNA moved into `pro/aiapp-gen-pro`; `pro/` added to `.gitignore`.
2. **Phase 2**: normalize the crate split (`aiapp-format`/`aiapp-engine`/`aiapp-host`), and make `pro/aiapp-saas` an independent service.
3. **Phase 3**: define the Telemetry schema; change backend statistics to "event reporting + Pro aggregation."
4. **Phase 4**: wire the reported events into billing/share units.



## Document Index
- `README.md` — project intro, pipeline, quick start
- `docs/ARCHITECTURE.md` — community edition vs Pro commercialization layering (including the split actions)
- `pro/` (closed-source private repo) — real generation engine + global operations admin
