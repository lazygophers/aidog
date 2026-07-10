# CLIENT_TYPES 移除 → defaults JSON 独立 + 远端同步

## Goal

前端硬编码 `CLIENT_TYPES` 常量（constants.ts:20，13 条 default + Claude Code 家族 + Codex + IDE）移除，改由 `src-tauri/defaults/client-types.json`（独立文件，多 locale name/desc）派生 + 远端自动同步（仿 defaults_sync.rs）。Rust `ClientType` enum → `String`（serde arbitrary，完全 JSON 驱动），Protocol→ClientType 映射移入 platform-presets.json per-protocol `default_client_type` 字段。前端 `ClientType` union → `string`，展示全走 JSON 派生。

## Context

- 现状：`src/domains/platforms/constants.ts:20` CLIENT_TYPES 硬编码 13 条（{value, labelKey?, label?, group}）；Rust `ClientType` enum (models/platform.rs:80) 13 变体；Protocol→ClientType 映射 (models/platform.rs:119-120)；前端 `ClientType` union (types/part1.ts:52)
- 先例：`defaults_sync.rs`（platform-presets.json 双源 jsDelivr+raw.github / last_updated 比对 / 24h 节流 / 三路触发：启动 hook + 每日定时 + 手动按钮 / schema gate / .hash 用户定制保护）
- C3 protocols-frontend-derive 已建前端派生层范式（buildProtocolsFromPresets async + useState/useEffect + cancelled flag + locale key）—— spec `frontend/derived-constants.md`
- spec：`guides/cross-layer-rules.md`（公共契约层字段名禁改 / enum 跨层 lowercase）、`backend/cargo-workspace.md`（aidog_core 路径，defaults_sync 在 core）、`frontend/derived-constants.md`（派生层范式）、`frontend/locale-tag-cross-layer.md`（8 locale + zh-Hans）

## 数据流架构（强制）

```
github (master) ──rust sync (client_types_sync.rs)──▶ ~/.aidog/client-types.json
                                                          │
                            打包 include_str! bundled ──┐ │ (app data 优先)
                                                       ▼ ▼
                                              rust reader (get_client_types_json)
                                                          │
                                                  invoke command
                                                          │
                                                     前端派生层
```

- **rust 单一数据源**：前端**禁**直读 github / 直读文件系统。前端一律 `invoke('get_client_types_json')` 拿数据
- **rust reader 优先级**：`~/.aidog/client-types.json`（app data，sync 写入）→ 缺失/损坏/schema gate 失败回退 `include_str!` bundled（编译期注入）
- **sync 写 ~/.aidog**：双源 fetch + last_updated 比对 + schema gate + .hash 用户定制保护，成功写 `~/.aidog/client-types.json`
- 同 `get_defaults_json` / `defaults_sync` 既有模式（platform-presets 已走此链），client-types 复用同架构

## Requirements

### R1 建 client-types.json 真值源

- 路径：`src-tauri/defaults/client-types.json`
- 结构（单文件 + name/desc 多 locale，仿 platform-presets）：
  ```json
  {
    "last_updated": <unix_secs>,
    "client_types": [
      {"value": "default", "group": "", "name": {"zh-Hans": "默认", "en-US": "Default", ...}, "desc": {...}},
      {"value": "claude_code", "group": "Claude Code", "name": {"zh-Hans": "Claude Code CLI", ...}, "desc": {...}},
      ...
    ]
  }
  ```
- 13 entry 从现 CLIENT_TYPES 迁移：value/group 照抄；label（"Claude Code CLI" 等专有名词）→ name 各 locale 同值（不译）；labelKey（platform.mockDefault）→ default 条目 name 多 locale 译（zh-Hans "默认" 等）
- 手维护（禁机器生成覆盖，同 platform-presets 约定）
- last_updated Unix 秒（同步链用）

### R2 platform-presets.json 加 default_client_type

- 每 protocol 的 default endpoint 对象加 `default_client_type` 字段（如 anthropic → "claude_code"，openai → "codex_cli"，glm → "claude_code" 等）
- 仅加有明确默认的 protocol；缺失字段 → defaults.ts `defaultClientForProtocol` 回落 "default"
- 手维护（同 platform-presets 约定）

### R3 Rust ClientType enum → String

- `crates/aidog_core/src/gateway/models/platform.rs`：
  - 删 `pub enum ClientType {...}` + `impl Default for ClientType` + `impl ClientType` 方法
  - 改 `pub type ClientType = String;`（或保留 struct wrapper serde arbitrary，选简单：`type ClientType = String`）
  - 删 `fn default_client_for_protocol(p: &Protocol) -> ClientType`（移入 presets，R2）
- db migration `schema_early.rs:190-192`：`ClientType::CodexTui` / `ClientType::ClaudeCode` → 字面量 `"codex_tui".to_string()` / `"claude_code".to_string()`（migration 逻辑不变，仅表达换；migration 禁改语义）
- `commands/model_test.rs:69`：`ClientType::default()` → `"default".to_string()`（或 `String::new()`，保语义用 "default"）
- db test `test_platform.rs` / `test_mod.rs`：`ClientType::Default` / `ClientType::CodexTui` → 字面量字符串
- 所有 `ClientType::<Variant>` 引用（grep 9 文件 83 命中）→ 字面量字符串
- serde：ClientType = String 天然 arbitrary，远端新 client_type 不丢（落原值）

### R4 前端 ClientType union → string

- `src/services/api/types/part1.ts:52`：`export type ClientType = string;`（删 13 值 union）
- 消费处 cast 不动（`as ClientType` = `as string`，仍合法）
- `defaults.ts:122 defaultClientForProtocol`：改 async 读 presets per-protocol `default_client_type`（R2），缺失回落 "default"

### R5 新建 client_types_sync.rs（仿 defaults_sync）

- 路径：`crates/aidog_core/src/gateway/client_types_sync.rs`
- 架构照抄 `defaults_sync.rs`：
  - 双源：`https://cdn.jsdelivr.net/gh/lazygophers/aidog@master/src-tauri/defaults/client-types.json`（主）+ `https://raw.githubusercontent.com/lazygophers/aidog/master/src-tauri/defaults/client-types.json`（fallback）
  - `last_updated` 比对（远端较新才写）
  - 24h 节流（`~/.aidog/client-types.json.last_sync`）
  - 三路触发：启动 hook（maybe_sync_on_startup）+ 每日定时器（spawn_daily_sync）+ 设置页手动按钮（sync_client_types_json command，无视节流）
  - **schema gate**：写盘前 `validate_structure` 校验远端 body（含 client_types 数组 + 每 entry value/group/name 关键字段），失败拒绝写入保留本地
  - **用户定制保护**：成功同步后写 `.hash` 快照（sha256 of body）；启动 hook 检测 app data 被手工修改则跳过自动同步；手动按钮强制覆盖 + 重置快照
- bundled：`include_str!("../../../../defaults/client-types.json")` 编译期编入
- 写入 app data (`~/.aidog/client-types.json`)，由 reader 自动优先读取

### R6 commands/defaults.rs reader + command

- reader：`get_client_types_json()` 读 app data (`~/.aidog/client-types.json`) → 缺失/损坏回退 `include_str!` bundled（同 get_defaults_json 模式）
- 新 command：`sync_client_types_json` → 调 `client_types_sync::sync_now`，返 `ClientTypesSyncResult`
- 启动 hook / 每日定时器接入（同 defaults_sync 三路触发）
- `lib.rs` / `startup.rs` generate_handler 加新 command

### R7 前端派生层 + 调用点 async

- `src/domains/platforms/constants.ts`：删 `CLIENT_TYPES`（line 20-36）
- `src/services/api/`：加 `getClientTypesJson()` invoke 包装（`invoke<ClientTypesDoc>('get_client_types_json')`），cmd 字符串 + 返回泛型
- `src/domains/platforms/defaults.ts`：
  - 加 module-level `docPromise = getClientTypesJson()` 单次 RPC 缓存（仿 C3 docPromise 模式）
  - 加 `buildClientTypesFromPresets(locale?): Promise<ClientTypeEntry[]>`（async 读 invoke 结果 + locale 派生 label；**禁前端直读文件/github**，一律走 invoke）
  - 加 `getClientTypeLabelMap(locale?): Promise<Record<string, string>>`（value→label）
  - 加 `defaultClientForProtocol` 改 async 读 presets per-protocol `default_client_type`（R2，invoke get_defaults_json 或复用现有 defaults docPromise）
- 调用点 async 化（仿 C3 范式：useState 空初始 + useEffect + cancelled flag + locale key [i18n.language]）：
  - `src/pages/platforms/formSectionsEndpoints.tsx`：下拉选项 CLIENT_TYPES → buildClientTypesFromPresets 派生（useState + useEffect）
  - 其他 `CLIENT_TYPES` 引用点 grep 清
- AppContext 预热 docPromise（best-effort，仿 C3）

### R8 设置页同步 UI

- 设置页加「client-types 同步」按钮（仿 platform-presets 同步 UI，若存在）：手动触发 sync_client_types_json + 显示结果/错误
- 同步开关 + 频率（若 defaults_sync 有 UI 先例，照抄；否则仅手动按钮 + 后台自动）

## Acceptance Criteria

- [ ] `src-tauri/defaults/client-types.json` 建（13 entry + last_updated + 多 locale name/desc）
- [ ] platform-presets.json 每 protocol default endpoint 加 default_client_type（有默认的 protocol）
- [ ] Rust ClientType enum → type ClientType = String；grep `ClientType::` 零残留（migration/test/model_test 全改字面量）
- [ ] 前端 ClientType union → string；消费处编译过
- [ ] client_types_sync.rs 建好（双源 + last_updated + 24h + 三路触发 + schema gate + .hash）
- [ ] commands/defaults.rs reader + sync_client_types_json command + generate_handler 接入
- [ ] 前端 CLIENT_TYPES 删；buildClientTypesFromPresets 派生；调用点 async 化（cancelled flag + locale key 对称）
- [ ] 设置页同步 UI（手动按钮 + 后台自动）
- [ ] `cargo build --workspace` 0 errors
- [ ] `cargo test --workspace` baseline 不回归（ClientType enum 删后相关 test 改字面量字符串，逻辑不变）
- [ ] `cargo clippy --workspace --all-targets` 无新 warning
- [ ] `yarn build` 0 errors / `yarn test` 全绿 / `yarn check:i18n` 无新缺失（client-types.json name/desc 8 locale 完整）
- [ ] grep `\bCLIENT_TYPES\b` src/ 仅注释
- [ ] 主仓零改动（git status 仅 .trellis/）

## Out of Scope

- client-types.json 内容运营（label 文案精修，后续迭代）
- platform-presets default_client_type 全 60+ protocol 覆盖（仅加有明确默认的，缺失回落 default）
- models.json / 其他 defaults 文件同步（本 task 仅 client-types）

## Technical Notes

- spec 合规：
  - `cross-layer-rules.md` Format Contracts：client_type 字段名 snake_case 不变；enum→string 是 owner 明示类型简化（非契约字段名改）
  - `frontend/derived-constants.md`：前端派生层范式（buildXFromPresets async + 调用点 useEffect + cancelled flag + locale key + AppContext 预热）
  - `frontend/locale-tag-cross-layer.md`：8 locale（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES），zh-Hans BCP47 script
  - `backend/cargo-workspace.md`：aidog_core 路径（client_types_sync 在 core/gateway/）
- 远端同步先例：`crates/aidog_core/src/gateway/defaults_sync.rs`（platform-presets 同步全套）
- 前端派生先例：`crates/aidog_core/src/...` + C3 `buildProtocolsFromPresets`（defaults.ts）
- migration 字面量化：schema_early.rs:190 `ClientType::CodexTui` → `"codex_tui".to_string()`（serde rename 值，禁驼峰）

## Definition of Done

- R1-R8 全完成 + 验收全绿
- worktree 内 commit；主仓 post-merge yarn tauri dev 验同步链
