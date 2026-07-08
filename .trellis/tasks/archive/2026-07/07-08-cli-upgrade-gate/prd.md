# CLI 升级按钮无反应 + 仅新版本才展示

## Goal

About 页 Codex/Claude CLI 升级按钮两问题：① 点击变「升级中…」后卡住不返回（后端 `cli_upgrade` invoke 挂起）② 升级按钮无条件展示（应仅确认有新版本才展示）。

## Decision (ADR-lite)

**Context**: 
- 根因（research/upgrade-root-cause.md，用户实测确认「变成升级中…后卡住」）= 后端 `cli_upgrade`（cli_env.rs:350）用 `Command::new().output()` 同步阻塞无超时，codex update 走 `brew upgrade --cask codex` 或 npm reinstall 长时间不返回，invoke 永不 resolve。
- 需求 2（research/latest-version-detection.md）= 升级按钮仅 `has_update===true` 才展示，需后端加 latest version 检测。
**Decision**:
1. **挂起修复**：`cli_upgrade` 改 async + `tokio::process::Command`（非阻塞，资源友好）。**不加超时**（用户定）—— brew/npm 正常完成即返回，挂起靠用户关 app 兜底。
2. **has_update gate**：
   - 新增独立 `cli_check_updates` command（不破坏 `cli_check_versions` 既有约定），HTTP 打 npm registry `/latest`（3.2KB），semver 比对。
   - 禁用 `http_client.rs::build_http_client`（耦合 DB settings + 递归防护），单独 `reqwest::Client::builder().no_proxy().timeout(8s)`。
   - `CliToolStatus` 加 `latest_version: Option<String>` + `has_update: Option<bool>`（三态，None=检测失败/离线）。
   - 缓存：`tokio::sync::RwLock<HashMap>` + 1h TTL（借鉴 gateway/middleware/mod.rs:242 OnceLock 模式）。
   - 新增 `semver = "1"` crate（版本比对，parse 失败 has_update=None）。
   - 前端：升级按钮 `s.installed && !s.broken && s.has_update === true`（None/undefined 不展示）。
**Consequences**: 升级不再卡 UI 线程；离线时升级按钮自动隐藏；前端自动 check（handleCliCheck）触发 updates 检测，1h 缓存避免频繁打 registry。

## Requirements

### 后端（src-tauri/src/commands/cli_env.rs）
1. `cli_upgrade` + `cli_install` + `cli_diagnose_conflicts` 全改 async + `tokio::process::Command`（替换 `std::process::Command::output()`），保留 `no_window()` Windows CREATE_NO_WINDOW
2. 新增 `cli_check_updates` async command：对 TOOLS（claude/codex）查 npm registry latest，semver 比对 local version，返 `HashMap<String, CliToolStatus>`（或 merge 进 cli_check_versions 返回结构）
3. `CliToolStatus` struct 加 `latest_version: Option<String>` + `has_update: Option<bool>`（serde snake_case，`#[serde(skip_serializing_if = "Option::is_none")]`）
4. npm registry 请求：`reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(8))`，GET `https://registry.npmjs.org/<pkg>/latest`，pkg = `@anthropic-ai/claude-code` / `@openai/codex`
5. 缓存 `static` + `tokio::sync::RwLock<HashMap<String, (Instant, String)>>` 1h TTL
6. semver crate 加 `src-tauri/Cargo.toml`
7. startup.rs 注册 `cli_check_updates`

### 前端
8. `src/services/api/system.ts` 加 `cliEnvApi.checkUpdates()` invoke `cli_check_updates`
9. `src/services/api/types/part4.ts` `CliToolStatus` 加 `latest_version?: string` + `has_update?: boolean`
10. `src/pages/About.tsx`：
    - 升级按钮条件 `s.installed && !s.broken && s.has_update === true`（约 line 381/393）
    - handleCliCheck 内追加调 checkUpdates（或独立 trigger），刷新 has_update
    - 展示 latest_version 文本（可选，has_update=true 时显示「最新: <version>」）
11. i18n 8 语言加 key（about.localEnv.latestVersion / newVersionAvailable 等）

## Acceptance Criteria

- [ ] 点升级按钮：变「升级中…」→ 正常完成转「升级成功」（codex/claude 各实测一次）
- [ ] 升级按钮仅 has_update===true 时展示（无新版 / 离线 / 检测中 None 时隐藏）
- [ ] codex/claude latest version 正确显示（联网时）
- [ ] 离线/registry 失败：has_update=None，升级按钮不展示，无报错弹窗
- [ ] npm registry 1h 缓存（同会话二次 check 不重复打）
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] `node scripts/check-i18n.mjs` 过
- [ ] `yarn build` clean
- [ ] cross-layer 5 command 签名 ↔ api.ts ↔ TS 类型 ↔ serde 字段四向对齐
- [ ] 主仓零改动（worktree 内）

## Out of Scope

- 不加升级超时（用户定）
- 不改 cli_check_versions 既有逻辑（新增独立 command）
- 不改 codex update 兜底逻辑（uninstall+install 自愈保留）
- 不加自动升级（默认关，纯手动按钮）

## Technical Notes

- 根因证据 = research/upgrade-root-cause.md（用户确认「升级中…卡住」= invoke 挂起）
- has_update 方案 = research/latest-version-detection.md（HTTP registry API + semver + 三态 Option<bool>）
- 真值源：cli_env.rs / startup.rs:234-237 / system.ts:52-58 / part4.ts / About.tsx
- cross-layer 一致性（memory cross-layer-rules）
- 新 Rust command 须 yarn tauri dev 重启（memory tauri-rust-command-needs-restart）
