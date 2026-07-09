# Design — commands cargo workspace 重构

## crate 依赖图

```
                    ┌─────────────┐
                    │   aidog     │ (binary crate, main.rs + startup.rs + generate_handler)
                    └──────┬──────┘
                           │ depends on all
        ┌────────┬─────────┼─────────┬────────┬────────┐
        ▼        ▼         ▼         ▼        ▼        ▼
   cmd-platform cmd-proxy cmd-config cmd-system cmd-ai-tools cmd-tray  cmd-cli-env
        │           │         │         │         │           │           │
        └───────────┴────┬────┴─────────┴─────────┴───────────┴───────────┘
                         ▼
                    ┌─────────────┐
                    │ aidog_core  │ (gateway + shared + models + 下沉业务)
                    └─────────────┘
```

**铁律**：commands_* crate 间**零互依赖**（Cargo.toml 不声明彼此）。所有共用业务逻辑下沉 aidog_core，commands crate 单向依赖 core。

## child 调度 DAG

| child | 内容 | depends_on |
|---|---|---|
| C1 ws-skeleton | workspace root Cargo.toml + 10 空 crate（含 aidog_test_util）+ **PoC 门禁：空 ws + yarn tauri dev 冒烟先过**（验证 build.rs/tauri.conf.json workspace path 解析） | — |
| C2 core-extract | gateway/shared/models 入 aidog_core + 下沉 sync+hooks（互耦同移）+ refresh_tray_menu + pub re-export | C1, **07-10-protocols-rust-enum** |
| C3 cmd-platform | platform/group/model_fetch/stats/price/quota → commands-platform | C2 |
| C4 cmd-proxy | proxy/proxy_log/proxy_timeout/middleware/mitm → commands-proxy | C2, **07-09-mitm-tables-to-setting** |
| C5 cmd-config | settings/defaults + sync/hooks 薄壳（实调 core） → commands-config | C2 |
| C6 cmd-system | about/app_log/auto_update/backup/notification/scheduling/fs_autocomplete → commands-system | C2 |
| C7 cmd-ai-tools | coding_tools/mcp/skills/script_executor/model_test → commands-ai-tools | C2 |
| C8 cmd-tray | tray/tray_render/**popover**(域重划) → commands-tray | C2 |
| C9 cmd-cli-env | cli_env → commands-cli-env | C2 |
| C10 app-wiring | aidog binary crate + startup.rs + generate_handler 186 处跨 crate 路径 + Tauri dev/build/release 验证 | C3,C4,C5,C6,C7,C8,C9 |

**并行**：C3-C9（7 commands crate）文件集不相交 → 全并行（受 task 级并发上限 2 约束，滚动 2 个）。

## 下沉决策（消除 commands 间互依赖）

| 原跨 command 边 | 处理 | 落点 |
|---|---|---|
| group/proxy → sync_settings::{do_sync_group_settings, try_sync_settings} | 下沉 core | aidog_core::sync |
| sync_settings ↔ hooks（互耦：sync 用 hooks::generate_hook_scripts/enabled_hook_events；hooks 用 sync::do_sync_group_settings） | **两者同移 core**（design v1 漏 hooks → core 反向依赖 commands-config = 循环；grill G1 捕获） | aidog_core::{sync, hooks} |
| platform/proxy → tray_render::refresh_tray_menu | 下沉 core（**函数移 core**，commands-tray 留 UI 渲染薄壳调 core；grill G6 明确） | aidog_core::tray_refresh |
| popover → tray::tray_layout | **域重划**：popover 入 commands-tray（已验 popover 仅依赖 tray+shared+gateway+logging，不依赖 platform/group → 干净） | commands-tray crate |
| middleware → mitm::ImportDefaultsResult | 同 commands-proxy 包内 | 不动 |
| test_harness::mock_app_with_db（跨 8 test：platform/proxy/system/config 域） | **独立 aidog_test_util crate**（grill G4 用户裁定），各 commands crate dev-deps 引 | crates/aidog_test_util |

## 关键技术约束

1. **workspace.dependencies**：tauri/serde/tokio/rusqlite/axum 等共享版本集中声明 workspace root，子 crate `{ workspace = true }` 引。禁子 crate 自定版本（冲突）。
2. **profile.release**：上提 workspace root 共享。
3. **build.rs + tauri.conf.json workspace path 解析（grill G3 风险点）**：Tauri 2.0 build.rs 属 binary crate → 放 `crates/aidog/build.rs`。**C1 PoC 门禁必须先验证**：10 空 crate workspace + tauri.conf.json 指向 crates/aidog binary → `yarn tauri dev` 能起。PoC 过才进 C2 全量迁移（避免迁移完才发现 Tauri 路径解析炸，回滚成本高）。
4. **tauri.conf.json**：binary 探测走 crate name `aidog`，路径配置不变；`build.beforeBuildCommand`（yarn build）不变。
5. **test 路径**：`crate::Db` / `crate::SetSettingInput`（test-only re-export, lib.rs:18,20）→ commands crate 测试改 `aidog_core::gateway::db::Db` 或 core 加 `pub use gateway::db::Db` 顶层再导出。
6. **路径迁移正则**：
   - `crate::commands::<file>::<fn>` → `commands_<domain>::<fn>`（domain 按文件归属表）
   - `crate::gateway::<...>` → `aidog_core::gateway::<...>`（commands crate 视角）
   - `crate::shared::<...>` → `aidog_core::shared::<...>`
   - core 内部 `crate::gateway::` 不动
7. **gateway 反向依赖**：已 grep 确认零（gateway 不调 commands），分层干净，无需重构。
8. **dev-deps tauri test feature（grill G5）**：commands-platform（test_group）、commands-config（test_hooks）、commands-system（test_app_log/test_scheduling）、commands-proxy（test_middleware/test_proxy_log/test_proxy_timeout）、commands-ai-tools（test_coding_tools）等含 MockRuntime 测试的 crate，dev-deps 各声明 `tauri = { workspace = true, features = ["test"] }` + `aidog_test_util = { path = "../aidog_test_util" }`。

## 风险

| 风险 | 缓解 |
|---|---|
| 186 处 generate_handler 路径漏改 | startup.rs 改后 cargo build 即捕获（Tauri 宏展开报错） |
| **Tauri build.rs/tauri.conf.json workspace path 解析炸** | **C1 PoC 门禁**：空 ws + yarn tauri dev 冒烟先于全量迁移（grill G3） |
| workspace 共享依赖版本冲突 | [workspace.dependencies] 集中声明，子 crate 禁自定 |
| **下沉 sync 致 core→commands-config 循环** | hooks+sync **同移 core**（互耦，grill G1 捕获） |
| 23 test 文件路径迁移漏 | 随源文件入对应 crate，cargo test --workspace 捕获 |
| **child 早于 blocker start（mitm-tables/protocols-rust-enum 未完就搬路径）** | **child-level deps**：C2 dep protocols-rust-enum，C4 dep mitm-tables（grill G2） |

## Grill trace（v2 修复）

| 轴 | 短板 | 修复 |
|---|---|---|
| B 产出 | test_harness 跨 8 test 文件（platform/proxy/system/config） | 独立 aidog_test_util crate（用户裁定） |
| C scope | design v1 下沉 sync 漏 hooks（互耦）→ core 循环 | hooks+sync 同移 core（G1） |
| C scope | refresh_tray_menu 下沉定义模糊 | 函数移 core，commands-tray 留 UI 薄壳（G6） |
| E edge | build.rs/tauri.conf.json workspace path 未证实 | C1 PoC 门禁先于全量迁移（G3） |
| E edge | tauri test feature dev-deps 未声明 | 各测试 crate dev-deps 声明（G5） |
| J 依赖 | parent dep 不足，children 才执行 | C2 dep protocols-rust-enum，C4 dep mitm-tables（G2） |

## 与其他 task 冲突

- **mitm-tables-to-setting**（planning）：改 commands/mitm.rs DB 层内容 → C4 迁移 mitm.rs 改路径 → **depends_on mitm-tables**（先改内容后搬路径）
- **protocols-rust-enum**（planning）：改 gateway/models Protocol enum → C2 提取 gateway 入 core → **depends_on protocols-rust-enum**（先加 enum 后搬）
- **protocols-json-schema / protocols-frontend-derive**：不改 src-tauri/，无冲突
