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
| C1 ws-skeleton | workspace root Cargo.toml + 9 空 crate + 空门禁 | — |
| C2 core-extract | gateway/shared/models 入 aidog_core + 下沉 sync/tray_render 业务 + pub re-export | C1 |
| C3 cmd-platform | platform/group/model_fetch/stats/price/quota → commands-platform | C2 |
| C4 cmd-proxy | proxy/proxy_log/proxy_timeout/middleware/mitm → commands-proxy | C2 |
| C5 cmd-config | settings/sync_settings(薄壳)/defaults/hooks → commands-config | C2 |
| C6 cmd-system | about/app_log/auto_update/backup/notification/scheduling/fs_autocomplete → commands-system | C2 |
| C7 cmd-ai-tools | coding_tools/mcp/skills/script_executor/model_test → commands-ai-tools | C2 |
| C8 cmd-tray | tray/tray_render/**popover**(域重划) → commands-tray | C2 |
| C9 cmd-cli-env | cli_env → commands-cli-env | C2 |
| C10 app-wiring | aidog binary crate + startup.rs + generate_handler 186 处跨 crate 路径 + Tauri dev/build/release 验证 | C3,C4,C5,C6,C7,C8,C9 |

**并行**：C3-C9（7 commands crate）文件集不相交 → 全并行（受 task 级并发上限 2 约束，滚动 2 个）。

## 下沉决策（消除 commands 间互依赖）

| 原跨 command 边 | 处理 | 落点 |
|---|---|---|
| group/proxy → sync_settings::{do_sync_group_settings, try_sync_settings} | 下沉 core | aidog_core::sync（或 gateway 子模） |
| platform/proxy → tray_render::refresh_tray_menu | 下沉 core | aidog_core::tray（或 event 解耦，选 core 最低成本） |
| popover → tray::tray_layout | **域重划**：popover 入 commands-tray | commands-tray crate |
| hooks ↔ sync_settings | 同 commands-config 包内 | 不动 |
| middleware → mitm::ImportDefaultsResult | 同 commands-proxy 包内 | 不动 |

## 关键技术约束

1. **workspace.dependencies**：tauri/serde/tokio/rusqlite/axum 等共享版本集中声明 workspace root，子 crate `{ workspace = true }` 引。禁子 crate 自定版本（冲突）。
2. **profile.release**：上提 workspace root 共享。
3. **build.rs**：Tauri 2.0 build.rs 属 binary crate → 放 `crates/aidog/build.rs`，`tauri_build::build()`。
4. **tauri.conf.json**：binary 探测走 crate name `aidog`，路径配置不变；`build.beforeBuildCommand`（yarn build）不变。
5. **test 路径**：`crate::Db` / `crate::SetSettingInput`（test-only re-export, lib.rs:18,20）→ commands crate 测试改 `aidog_core::gateway::db::Db` 或 core 加 `pub use gateway::db::Db` 顶层再导出。
6. **路径迁移正则**：
   - `crate::commands::<file>::<fn>` → `commands_<domain>::<fn>`（domain 按文件归属表）
   - `crate::gateway::<...>` → `aidog_core::gateway::<...>`（commands crate 视角）
   - `crate::shared::<...>` → `aidog_core::shared::<...>`
   - core 内部 `crate::gateway::` 不动
7. **gateway 反向依赖**：已 grep 确认零（gateway 不调 commands），分层干净，无需重构。

## 风险

| 风险 | 缓解 |
|---|---|
| 186 处 generate_handler 路径漏改 | startup.rs 改后 cargo build 即捕获（Tauri 宏展开报错） |
| Tauri build.rs 位置错致 dev 起不来 | C10 专项 yarn tauri dev 冒烟门禁 |
| workspace 共享依赖版本冲突 | [workspace.dependencies] 集中声明，子 crate 禁自定 |
| 下沉 sync/tray_render 业务致循环依赖 | 下沉 core 后 commands 单向调 core，core 不反向调 commands（已验证 gateway 零反向） |
| 23 test 文件路径迁移漏 | 随源文件入对应 crate，cargo test --workspace 捕获 |

## 与其他 task 冲突

- **mitm-tables-to-setting**（planning）：改 commands/mitm.rs DB 层内容 → C4 迁移 mitm.rs 改路径 → **depends_on mitm-tables**（先改内容后搬路径）
- **protocols-rust-enum**（planning）：改 gateway/models Protocol enum → C2 提取 gateway 入 core → **depends_on protocols-rust-enum**（先加 enum 后搬）
- **protocols-json-schema / protocols-frontend-derive**：不改 src-tauri/，无冲突
