# 平台协议 preset name 派生展示（reader merge + 同步链诊断）

## Goal

修 glm_coding 平台三处界面（编辑表单 badge / Platforms 卡片列表 / 创建 modal）展示 raw "glm_coding" 而非 preset name "GLM 编码套餐（智谱）"。

**根因**：用户 app data `~/.aidog/platform-presets.json` 旧（last_updated 1783354810，7月8日，60 协议缺 glm_coding），bundled 新（1783599760，含 glm_coding）。reader 优先 app data（存在即返，非缺失/损坏不 fallback bundled）→ 派生层 `getProtocolLabel("glm_coding")` 在 app data 找不到 → fallback raw "glm_coding"。

**「创建后协议不可更改」已生效**（PlatformEditForm.tsx:136-153 editing 分支 badge 展示，非 select），用户确认，本 task 不动。

## 方案（用户选 A+B）

### A. reader deep merge（根治派生层）
- reader 读时：app data 优先，bundled 补 app data 缺的 key（protocol entry / client-type entry）
- 派生层即时拿到全量（bundled 新 protocol 补全），不依赖同步
- 适用 platform-presets reader + client-types reader

### B. 同步链诊断 + 修（app data 旧未覆盖）
- 查为何 .last_sync（7月9日）后未触发覆盖 app data（24h 节流 / 远端 fetch / 同步 bug）
- 修同步链让 app data 定期更新（bundled/远端新 → 覆盖 app data）

## 数据流（强制）

```
bundled (include_str!, 新, 含 glm_coding)
  + app data (~/.aidog/, 旧, 缺 glm_coding)
  ↓ reader deep merge（app data 优先，bundled 补缺 key）
rust reader（返全量合并 JSON）
  ↓ invoke
前端派生层 getProtocolLabel / getProtocolLabelMap
  ↓ labelMap[glm_coding] = "GLM 编码套餐（智谱）"
三处 UI 展示 preset name
```

## Requirements

### R1 platform-presets reader deep merge
- `src-tauri/crates/aidog_core/src/gateway/defaults_sync.rs` 或 `src/commands/defaults.rs::get_defaults_json` reader 改：
  - 读 app data JSON → 解析
  - 读 bundled `include_str!` JSON → 解析
  - **deep merge**：`protocols` 层，每 protocol key：app data 有用 app data，app data 缺用 bundled 补全
  - 顶层 `last_updated` / `version` 取 **较大值**（max），让同步链仍能正确比对（app data 旧 + bundled 新 → 取 bundled 新 → 同步链判定需更新）
  - 顶层其他字段（非 protocols）取 app data（向后兼容）
- app data 缺失/损坏 → 全量 bundled fallback（同现状，不变）
- 路径：grep `fn get_defaults_json` 定位

### R2 client-types reader deep merge（同模式）
- `src-tauri/crates/aidog_core/src/gateway/client_types_sync.rs` 或 `src/commands/defaults.rs::get_client_types_json` reader：
  - 读 app data → 读 bundled → **数组 merge by value 去重**（app data 有用 app data，缺用 bundled 补）
  - 顶层 last_updated 取 max
- 路径：grep `fn get_client_types_json` 定位

### R3 同步链诊断（为何 app data 旧未覆盖）
- 查 `defaults_sync.rs::maybe_sync_on_startup` / `spawn_daily_sync`：
  - 24h 节流逻辑（`.last_sync` 比对，距上次 < 24h 跳过）
  - last_updated 比对（远端 <= 本地跳过）—— 本地 last_updated 现状是 app data 旧值（1783354810），远端新（1783599760）→ 应触发
  - 远端 fetch（双源 jsDelivr + raw.github，主源失败 fallback）
  - `.hash` 用户定制保护（现状 `.hash` 不存在 = 不保护，应能覆盖）
  - `validate_structure` schema gate（远端 ⊇ bundled，glm_coding 在远端应有）
- 诊断方法：加日志或读代码确认哪步跳过；`.last_sync` 7月9日，启动 hook 应在节流外（> 24h）
- 修复：让同步正常覆盖（不破坏 24h 节流 / .hash 保护设计）
- **若同步链代码无 bug**（设计正确，仅节流未到 / 远端延迟），标注「同步链设计正确，reader merge 已根治展示问题，app data 下次同步自动更新」—— 不强行改同步

### R4 验证（三处界面 + reader + sync）
- reader merge 后 `get_defaults_json` 返 JSON 含 glm_coding（即使 app data 缺）
- 派生层 `getProtocolLabel("glm_coding", "zh-Hans")` = "GLM 编码套餐（智谱）"
- 三处 UI 展示 preset name（编辑表单 badge L149 / 卡片列表 PlatformCard.tsx:297 / 创建 modal SmartPasteModal.tsx:258）
- `cargo build --workspace` + `cargo test --workspace`（baseline 1382+，reader merge 新 test）+ `cargo clippy` 无新 warning
- `yarn build` 全绿（前端零改）
- reader merge 单测：app data 旧（缺 glm_coding）+ bundled 新（含）→ merge 后含 glm_coding；app data 缺失 → bundled fallback；last_updated max

## Acceptance Criteria

- [ ] reader deep merge（platform-presets + client-types）落地，app data 旧缺 key 时 bundled 补全
- [ ] reader merge 单测覆盖（缺 key 补全 / 全缺失 fallback / last_updated max）
- [ ] `getProtocolLabel("glm_coding", "zh-Hans")` 在用户当前 app data（旧缺 glm_coding）下返 "GLM 编码套餐（智谱）"
- [ ] 同步链诊断结论明确（无 bug 标注 / 有 bug 修复）
- [ ] cargo build/test/clippy --workspace 全绿，无新 warning
- [ ] yarn build 全绿（前端零改）
- [ ] 主仓零改动（worktree 内）

## Out of Scope

- 「创建后协议不可更改」（已生效，不动）
- cmd-platform 等 commands 搬迁（另一线 C3-C9）
- 前端派生层逻辑（代码正确，reader 修后自动受益，零改）
- platform-presets 真值源内容（glm_coding entry 已在 bundled，不动）

## Technical Notes

- spec: `.trellis/spec/backend/remote-json-sync.md`（7 件套 + reader app data 优先 bundled fallback → 本 task 扩展为 deep merge）
- reader 现状：`commands/defaults.rs::get_defaults_json`（grep 定位）+ `client_types_sync.rs` reader
- app data 当前：last_updated 1783354810（7月8日），60 协议缺 glm_coding；bundled last_updated 1783599760 含 glm_coding
- `.hash` 不存在（无用户定制保护），`.last_sync` 7月9日 14:04
- merge 后 last_updated 取 max 是关键（让同步链仍能正确比对触发覆盖，不因 merge 后「看起来新」而跳过同步）
- deep merge 仅 protocols / entries 层（per-key 补全），非整体 deep merge（避免 app data 旧 protocol entry 被 bundled 覆盖丢用户定制——但 protocol entry 是真值源非用户数据，用户定制在 platform.extra 层不在 preset）
