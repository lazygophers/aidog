---
title: 远端 defaults JSON 同步链
layer: recall
category: ops
keywords: [sync,defaults,json,jsdelivr,remote,validate,presets,hash]
source: trellis
authored-by: skein-memory
created: 1783832114
---

# 远端 defaults JSON 同步链范式

何时被读: 新增 `src-tauri/defaults/*.json` 真值源文件并需远端自动同步时（platform-presets / client-types / 未来 models 等）
谁读: trellis-implement sub-agent / main
不遵守的代价: 远端坏 JSON 覆盖本地好数据 / 用户手改被自动同步覆盖 / 同步无节流轰炸 CDN / 前端绕过后端直读 github 破坏单一数据源

---

## 范式 (MUST，照抄先例 `gateway/defaults_sync.rs`)

`defaults/*.json` 远端同步**MUST** 实现完整 7 件套，缺一致命。先例 `crates/aidog_core/src/gateway/defaults_sync.rs`（platform-presets 同步）+ `client_types_sync.rs`（client-types 同步，第 2 次实例）。

### 1. 双源 fetch（主 + fallback）

- 主源 jsDelivr CDN: `https://cdn.jsdelivr.net/gh/<owner>/<repo>@master/src-tauri/defaults/<file>.json`
- fallback raw.githubusercontent: `https://raw.githubusercontent.com/<owner>/<repo>/master/src-tauri/defaults/<file>.json`
- 主源失败（网络 / 4xx/5xx）→ fallback；fallback 失败 → 跳过本次同步（保本地数据，禁空覆盖）

### 2. `last_updated` 比对 (MUST)

- JSON 顶层 `last_updated: <unix_secs>` 字段（手维护，真值源提交时更新）
- 远端 `last_updated <= 本地 last_updated` → 跳过（远端非较新，禁写）
- 远端 `>` 本地 → 进 schema gate → 写

### 3. 24h 节流 (MUST)

- `THROTTLE_SECS = 24 * 3600`
- 本地 `~/.aidog/<file>.json.last_sync` 记上次同步 Unix 秒
- 距上次 < 24h → 跳过自动同步（手动按钮不受节流，强制）

### 4. 三路触发 (MUST)

- **startup hook**: `maybe_sync_on_startup()` app 启动异步触发（禁阻塞启动，`tokio::spawn`）
- **每日定时器**: `spawn_daily_sync()` 24h 循环 spawn
- **手动按钮**: `sync_<x>_json` command（无视节流强制），设置页 UI 触发，返 `<X>SyncResult` struct

### 5. `validate_structure` schema gate (MUST，写盘前)

- 远端 body JSON 解析 + 结构校验（关键数组 + per-entry 必需字段存在性）
- **远端 ⊇ bundled value 集合** (MUST)：远端 entry value 集合必须 ⊇ `include_str!` bundled（防远端漏条目覆盖本地全量）
- 失败 → 拒绝写入保留本地（禁坏数据覆盖好数据）

### 6. `.hash` 快照用户定制保护 (MUST)

- 成功同步后写 `~/.aidog/<file>.json.hash`（sha256 of body 快照）
- 启动 hook 检测：本地 `<file>.json` 实际 sha256 ≠ `.hash` 快照 → 判用户手工修改 → **跳过自动同步**（保用户定制）
- 手动按钮强制覆盖 + 重置快照（用户显式触发）

### 7. reader + bundled deep merge (MUST)

- `get_<x>_json` command reader 优先级：`~/.aidog/<file>.json`（app data，sync 写入）→ **deep merge** bundled（`include_str!` 编译期注入）补 app data 缺的 key → 缺失/损坏/schema gate 失败回退 bundled 全量
- **deep merge 语义 (MUST)**：app data 优先（已有 key 保留，保用户定制 / endpoint flag），bundled 仅补 app data 缺的 key（protocol entry / client-type entry）；非整体 fallback（避免 app data 旧缺新 key 时派生层拿不到，如 app data 旧缺 glm_coding → reader merge 即时补全 → 派生层展示正确）
- **顶层 `last_updated` 取 max(app, bundled)**（关键：reader 返 merge JSON 仅给派生层展示，**不写盘**；sync 的 `read_local_last_updated` 仍读 app data 原文件，故 max 不污染 sync 比对 —— app 旧 + bundled 新 → 取新 → 同步链仍判需更新触发覆盖）
- **fallback 链 (MUST)**：app data 缺失/空/损坏/非 object/缺顶层集合字段（`protocols` / `client_types`）→ 返 bundled 全量（向后兼容，同原 fallback 语义）
- 打包时 bundled 已是最新 master 版本；`~/.aidog/` 是运行期同步结果（可能旧于 bundled，故 deep merge 补缺）

**验收（reader merge 单测矩阵，MUST）**：
- app 缺 key 时 bundled 补全（synthetic + 真 BUNDLED 集成，如 platform-presets 删 glm_coding 模拟旧 app data → merge 后必含）
- app 已有 key 保留不覆盖（保 endpoint flag 用户定制 / app 独有 protocol 保留）
- 顶层 last_updated max（双向：app > bundled / bundled > app；app 缺 ts 用 bundled）
- app 缺失/非 object/缺顶层集合字段 → bundled 全量 fallback

## 数据流架构 (MUST，禁前端直读 github)

```
github (master) ──rust sync (<x>_sync.rs)──▶ ~/.aidog/<file>.json
                                                │
                  打包 include_str! bundled ──┐ │ (app data 优先)
                                             ▼ ▼
                                    rust reader (get_<x>_json)
                                                │
                                        invoke command
                                                │
                                           前端派生层
```

- **rust 单一数据源 (MUST)**：前端**禁**直读 github / 直读文件系统。前端一律 `invoke('get_<x>_json')` 拿数据
- 违反后果：前端直读 github → 本地打包 bundled + 用户定制 + sync 优化全失效，且 CORS / 网络抖动直击前端崩页
- 验收 grep：`grep -rn 'jsdelivr\|raw.githubusercontent' src/` 必须 0（同步 URL 仅在 Rust sync 文件内）

## 验收断言（可复用）

```bash
# 7 件套齐全（双源 / last_updated / 24h / 三路触发 / schema gate / .hash / reader）
grep -n 'cdn.jsdelivr.net\|raw.githubusercontent' crates/aidog_core/src/gateway/<x>_sync.rs  # 双源
grep -n 'last_updated' crates/aidog_core/src/gateway/<x>_sync.rs  # 比对
grep -n 'THROTTLE_SECS\|24 \* 3600\|24\*3600' crates/aidog_core/src/gateway/<x>_sync.rs  # 节流
grep -n 'maybe_sync_on_startup\|spawn_daily_sync\|sync_now' crates/aidog_core/src/gateway/<x>_sync.rs  # 三路
grep -n 'fn validate_structure' crates/aidog_core/src/gateway/<x>_sync.rs  # schema gate
grep -n 'write_hash_snapshot\|is_user_modified\|\.hash' crates/aidog_core/src/gateway/<x>_sync.rs  # .hash
grep -n 'fn get_<x>_json\|include_str!' src/commands/defaults.rs  # reader + bundled
grep -n 'fn merge_with_bundled\|merge_top_last_updated' crates/aidog_core/src/gateway/<x>_sync.rs  # reader deep merge

# 启动 + 定时器 + command 接入
grep -n '<x>_sync::maybe_sync\|<x>_sync::spawn_daily' src/app_setup.rs  # 启动 + 定时
grep -n 'sync_<x>_json\|get_<x>_json' src/startup.rs  # generate_handler

# 前端禁直读 github
grep -rn 'jsdelivr\|raw.githubusercontent' src/  # 0

# bundled 与 defaults JSON 同源
grep -n 'include_str!.*defaults/<file>.json' crates/aidog_core/src/gateway/<x>_sync.rs src/commands/defaults.rs  # 路径一致
```

## 实例

- task 07-09-*（platform-presets 同步首次落地，`defaults_sync.rs` 先例建立 7 件套）
- task 07-10-client-types-json-sync（`client_types_sync.rs` 第 2 次实例化，照抄 `defaults_sync.rs` 全套，含 11 单测；`get_client_types_json` reader + `sync_client_types_json` command + `app_setup.rs` 启动 hook / 24h 循环 + `ClientTypesSyncSection` 手动按钮 UI + 8 locale 反馈文案）
- task 07-10-protocol-name-display（reader **deep merge** 范式落地：`defaults_sync.rs` + `client_types_sync.rs` 各加 `merge_with_bundled` + `merge_top_last_updated`；reader `get_defaults_json` / `get_client_types_json` 改 deep merge；13 单测含真 BUNDLED glm_coding 补全集成；app data 旧缺 glm_coding → reader merge 补全 → 派生层 `getProtocolLabel` 展示 preset name。同步链诊断结论：无 bug，24h 节流未到，reader merge 根治展示不依赖同步时序）

## Cross-reference

- 先例代码: `crates/aidog_core/src/gateway/defaults_sync.rs`（platform-presets 同步全套 7 件套）
- 前端派生层消费: [前端派生层](../frontend/derived-constants.md)（`buildXFromPresets` + `docPromise` 单 RPC 缓存）
- 公共契约层禁改: [Cross-Layer Rules 持久化路径换公共契约零改](../guides/cross-layer-rules.md)
- platform-presets 真值源手维护约定: project CLAUDE.md「平台默认配置」段
