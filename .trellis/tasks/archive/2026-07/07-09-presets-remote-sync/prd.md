# platform-presets 远端同步结构一致性强制

## Goal

给既有 `defaults_sync.rs` 远端日级自动更新机制加 **结构一致性 schema gate**：远端 `platform-presets.json` 写盘前必须通过结构校验 + 协议集合一致性校验，否则拒绝写入保留本地；同时保护用户手动定制（检测到本地修改后暂停自动更新，仅手动按钮生效）。

**为什么**：当前 `sync_defaults_json` 仅比对 `last_updated` 数值即覆盖写盘（`src-tauri/src/gateway/defaults_sync.rs:67-90`）。远端 JSON 若 protocols 结构损坏 / 字段类型错位 / 协议集合漂移（删了用户已建平台引用的 protocol key）→ 直接覆盖 `~/.aidog/platform-presets.json` → 前端拿残缺数据 / 用户平台引用断裂。用户原话「必须和那个内容及结构保持一致」= 远端结构必须与本地期望（bundled）同构才接受。

## 现状（已实现，本 task 不重建）

- `src-tauri/src/gateway/defaults_sync.rs`（251 行）：jsDelivr 主 + raw fallback 双源 fetch；`last_updated` 比对；24h 节流；启动 hook `maybe_sync_on_startup` + 每日定时器 + 手动按钮 `sync_defaults_json` command 三路触发。
- `src-tauri/src/commands/defaults.rs::get_defaults_json`：读 app data → 缺失/损坏回退 `include_str!` bundled。
- 真值源 = `src-tauri/defaults/platform-presets.json`（顶层 `version` + `last_updated` + `protocols`，61 协议）。

## Requirements

### R1 结构校验门（写盘前，sync_defaults_json 内）

新增 `validate_structure(body: &str) -> Result<(), String>`，在 `parse_last_updated` 通过后、`write_app_data` 之前调用。校验项（任一失败 → `Err`）：

- R1.1 body 必须可解析为 `serde_json::Value`，顶层为 object。
- R1.2 顶层含 `protocols` 为 object（已隐含 `last_updated` 校验，此处补 `protocols`）。
- R1.3 **协议集合**：远端 protocol key 集合 **⊇ 本地 bundled 集合**（远端可增不可减）。缺失任一本地的 key → `Err("missing protocol: <key>")`。
- R1.4 远端每个与本地共有的 protocol 条目必须含：`endpoints`（array）、`models`（object）、`model_list`（array）——存在性 + 粗类型，不验值细节。任一缺失/类型错位 → `Err("protocol <key>: missing/invalid <field>")`。
- R1.5 远端新增协议（本地无的 key）也必须含上述三字段（存在性 + 粗类型）。

参照本地 = `include_str!("../../defaults/platform-presets.json")`（defaults_sync.rs 内独立 `const BUNDLED`，与 commands/defaults.rs 各自 include_str! 同源同文件，编译期同值）。

### R2 校验失败处理

- R2.1 任一 R1 校验失败 → **不调 `write_app_data`**，保留本地 app data 原样。
- R2.2 不写 `last_sync` 时间戳（失败的同步不计入节流，下次启动可重试）。
- R2.3 返回 `DefaultsSyncResult { updated: false, last_updated: <local_ts>, source: "local", error: Some("<具体校验失败原因>") }`。
- R2.4 `tracing::warn` 记录失败原因（带 trace_id）。

### R3 用户定制保护（hash 快照）

- R3.1 每次成功同步写 `~/.aidog/platform-presets.json` 后，额外写 `~/.aidog/platform-presets.json.hash`（文件内容 = 同步写入 body 的 sha256 hex）。
- R3.2 `maybe_sync_on_startup` 在节流判定前先查 user_modified：当前 app data 文件的 sha256 == `.hash` 文件内容 → false；不等 → true。无 `.hash` 文件（首次/旧版升级）→ 视为 false（不阻塞，正常同步并建立基线）。
- R3.3 user_modified=true → `maybe_sync_on_startup` 跳过自动同步，`tracing::info` 记录，返回。
- R3.4 手动按钮 `sync_defaults_json` **不受 user_modified 影响**（用户显式触发，强制覆盖；成功后重置 `.hash` 基线）。
- R3.5 `sync_defaults_json` 返回值新增 `user_modified: bool` 字段（默认 false，手动按钮路径不查此值，仅 startup 路径设置）。

### R4 跨层对称（Rust ↔ TS）

- R4.1 `DefaultsSyncResult` 新增 `user_modified: bool` 字段（`#[serde(rename_all = "camelCase")]` 已有 → 序列化为 `userModified`）。
- R4.2 前端 `src/services/api.ts` 对应类型加 `userModified: boolean`（可选，避免破坏现有调用）。
- R4.3 Settings 页（手动同步按钮所在处）展示状态：上次同步时间 / 远端结构异常计数 / 用户定制暂停标记。**最小化 UI 改动**——若现有 Settings 已有同步按钮，仅扩展状态显示；若无明确入口，本 task 仅加 Rust + api.ts，UI 留下个 task。

### R5 测试（defaults_sync.rs `#[cfg(test)]`）

- R5.1 `validate_structure` 合法 body（含全部本地协议 + 字段齐）→ Ok。
- R5.2 缺 `protocols` 顶层 → Err。
- R5.3 远端少一个本地协议 → Err 含 "missing protocol"。
- R5.4 远端多一个新协议（字段齐）→ Ok（前向兼容）。
- R5.5 某 protocol 缺 `endpoints` → Err。
- R5.6 某 protocol `models` 类型错位（给 array）→ Err。
- R5.7 hash 快照写入 + user_modified 检测逻辑（抽 helper 单测，不依赖真实 fs 状态，沿用现有 `should_sync_due_internal` 模式）。

## Acceptance Criteria

- [ ] `validate_structure` 函数实现 + 全部 R5 单测通过 `cargo test defaults_sync`。
- [ ] `sync_defaults_json` 校验失败时不写 app data（R2.1 验证：构造残缺 body → app data 原样不动）。
- [ ] user_modified 检测：手动改 app data → 启动自动同步跳过；手动按钮仍生效。
- [ ] `DefaultsSyncResult.userModified` 字段跨 Rust serde ↔ TS api.ts 对齐。
- [ ] `cargo clippy` 无新 warning；`cargo test` 全过。
- [ ] `yarn build`（前端类型同步，若 R4.3 涉及 UI）/ `npx tsc --noEmit` 无错。

## Definition of Done

- Rust 单测覆盖 R1-R3 全部分支
- 跨层字段对齐（Rust serde ↔ TS type）
- 无新 clippy warning
- journal 记录关键决策（校验严格度选择 / hash 快照方案）

## Technical Approach

```
sync_defaults_json()
  ├─ fetch_defaults_json() (现有)
  ├─ parse_last_updated(body) (现有)
  ├─ validate_structure(body) [新增 — R1]
  │   ├─ serde_json::parse → 顶层 object + protocols object
  │   ├─ 解析本地 BUNDLED → bundled_protocols key 集合
  │   ├─ 远端 protocols key ⊇ bundled (R1.3)
  │   └─ 每个共有 protocol 检 endpoints/models/model_list (R1.4)
  ├─ compare remote_ts vs local_ts (现有)
  ├─ write_app_data(body) (现有)
  └─ write_hash_snapshot(sha256(body)) [新增 — R3.1]

maybe_sync_on_startup()
  ├─ is_user_modified()? [新增 — R3.2] → true 则 return (R3.3)
  ├─ should_sync_due() (现有)
  └─ sync_defaults_json() (现有)
```

hash 用 `sha2` crate（已在 Cargo.lock？查；若无则加依赖）。

## Decision (ADR-lite)

**Context**：远端自动更新破坏性风险 vs 用户期望"结构一致"。
**Decision**：
1. 严格度 = 协议集合（⊇ 本地）+ 关键字段存在性（不验值细节）——平衡安全与前向兼容。
2. 失败 = 拒绝写入保留本地（最安全，用户无感知风险）。
3. 用户保护 = hash 快照检测，改过则停自动更新，手动按钮不受限。
**Consequences**：
- 远端新增协议需发 bundled 才能通过校验？否——R1.3 允许远端新增（⊇），仅禁止删除。
- hash 快照方案增加一个 `.hash` 文件，首次升级无 hash 不阻塞。
- 用户改 app data 后若想恢复自动更新 → 手动按钮触发一次同步即重置基线。

## Out of Scope

- per-protocol merge（整个文件级覆盖，不做字段级合并）
- 远端 schema 版本协商（顶层 `version` 字段不参与校验）
- 用户回滚 / diff UI（仅状态标记，不提供回滚）
- 独立 JSON Schema 文件（避免第二份真值源）
- Settings 页全功能 UI（仅最小状态显示，若有；否则留下个 task）

## Technical Notes

- 参照实现：`price_sync.rs` 同模式（fetch + parse + upsert），但 price_sync 写 DB 不写文件，无文件覆盖风险——本 task 是文件级覆盖特有需求。
- `include_str!` bundled：commands/defaults.rs 已有 `const BUNDLED`（private）；defaults_sync.rs 新增自己的 `const BUNDLED`（同路径同文件，编译期同值，无重复维护负担）。
- `serde_json::Value` 而非强类型 struct（platform-presets.json 现状即用 Value 透传前端，保持一致）。
- 已有 guide：`.trellis/spec/guides/cross-layer-rules.md`（Rust serde ↔ TS 字段对齐）、`.trellis/spec/guides/code-reuse-rules.md`（grep 查既有 sha256 / hash 工具）。
