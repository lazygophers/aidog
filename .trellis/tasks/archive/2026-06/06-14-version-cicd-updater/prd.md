# .version 唯一版本源 + 发布 CICD + 自动更新对接

## Goal

根目录 `.version` 作版本唯一可信源；sync 脚本传播到各 manifest（build 以此为基准）；`.version` 变更触发 CICD（多平台 release 发版 + 文档部署）；一并实现 Tauri 自动更新并对接 release 产出的 updater artifacts。

## What I already know（探得现状）

- **无 `.version`**。版本散落 `package.json`(0.1.0) / `src-tauri/Cargo.toml`:3 / `src-tauri/tauri.conf.json`:4 / `docs/package.json`(0.1.0)，各自字面量。
- **自动更新未实现**：`tauri-plugin-updater` 在 Cargo/tauri.conf/前端全无引用（用户原以为已实现，已澄清需一并做）。
- CI 仅 `.github/workflows/deploy-docs.yml`（push `docs/**` → GitHub Pages，yarn build docs/doc_build）。无 release workflow。
- tauri v2 + `@tauri-apps/cli ^2`（`yarn tauri signer generate` 可用）。`@tauri-apps/api ^2`。bundle targets `"all"`，无 updater 配置。
- 上轮新增 `about_info` 命令用 `CARGO_PKG_VERSION`（= Cargo.toml literal）→ sync 写 Cargo.toml 后自动反映 `.version`。About 页可挂「检查更新」入口。
- 仓库 `github.com/lazygophers/aidog`。

## Requirements（决策已定）

- **范围**：完整实现自动更新（plugin-updater + tauri.conf endpoints/pubkey + 前端检查 UI + CICD 产 updater artifacts）。
- **平台**：macOS(arm64+x64) + Windows(x64)。**不含 Linux**。
- **同步**：`scripts/sync-version.mjs`（读 `.version` 写各 manifest）+ `--check`（CI 校验一致性，drift 则 fail）。

## Technical Approach（详见 design.md）

3 阶段顺序依赖（A→B→C）：
1. **版本单一源**：`.version`(纯 `0.1.0\n`) + `scripts/sync-version.mjs`(write/`--check`) 同步 package.json / Cargo.toml / tauri.conf.json / docs/package.json + npm scripts `version:sync`/`version:check`。
2. **Tauri 自动更新**：Cargo `tauri-plugin-updater`+`tauri-plugin-process`；lib.rs 注册；capabilities `updater:default`+`process:allow-restart`；tauri.conf `plugins.updater`(endpoints=GitHub latest.json, pubkey) + `bundle.createUpdaterArtifacts:true`；前端 `@tauri-apps/plugin-updater`+`plugin-process`，About 页「检查更新」按钮(check→downloadAndInstall→relaunch)。keypair 经 `tauri signer generate`（私钥落本地 ~/.tauri 不入库，pubkey 入 conf，用户加 GH secret）。
3. **CICD**：`.github/workflows/release.yml`(push `.version` → 读版本 tag `v<ver>` → matrix macos-14/macos-13/windows → `tauri-apps/tauri-action`(签名 secrets, includeUpdaterJson, createRelease) ；docs：`deploy-docs.yml` paths 加 `.version`(版本变更重部署文档)。

## Acceptance Criteria

- [ ] `.version` 存在；`node scripts/sync-version.mjs` 后 4 manifest 版本一致；`--check` 一致时 exit 0、人为 drift 时 exit 1。
- [ ] `cargo build` + `cargo clippy`(零新 warning) + `yarn build` green（含 updater 插件 + 前端检查 UI）。
- [ ] About 页有「检查更新」按钮，调 updater check（无更新/有更新分支可达，离线/失败不崩）。
- [ ] `release.yml` + `deploy-docs.yml` YAML 合法（python yaml.safe_load）；release 触发 paths=`.version`，matrix=macOS arm64+x64 + Windows，含 tauri-action 签名 + updater json。
- [ ] tauri.conf `plugins.updater.pubkey` 为真实生成的公钥；`createUpdaterArtifacts:true`；capabilities 含 updater/process 权限。
- [ ] 文档化：README/docs 注明 `.version` 改版本流程 + 用户需加的 GH secrets(`TAURI_SIGNING_PRIVATE_KEY`/`_PASSWORD`)。

## Out of Scope

- Linux 构建 / 包管理器分发。
- 代码签名证书（macOS notarization / Windows Authenticode）—— 仅 updater minisign 签名（updater 必需），系统级签名留后续。
- 自动版本号 bump（语义化自动递增）；版本仍人工改 `.version`。
- 灰度/回滚发布策略。

## Decision (ADR-lite)

- **Context**: 版本散落 4 处易漂移；无 release 自动化；自动更新未实现但用户需要。
- **Decision**: `.version` 唯一源 + sync 脚本(Cargo.toml version 须字面量, 无法运行时读 → 脚本 pre-build 写 + CI 校验) + tauri-action 一站式多平台 release/签名/updater json + GitHub Releases 作 updater endpoint。
- **Consequences**: 改版本 = 改 `.version` + 跑 sync + commit → CI 自动发版+部署文档+产 updater artifacts；客户端 About 页检查更新。需用户一次性：生成 keypair、加 2 个 GH secrets、首次 release 后验证 latest.json。
