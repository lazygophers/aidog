# 关于模块: 完整版本信息 + GitHub 信息

## Goal

客户端加侧栏顶级「关于」页，展示完整版本信息（App / Tauri / OS / 架构 / 构建）+ GitHub 静态链接（仓库 / Releases / Issues），点击经 opener 跳浏览器。

## What I already know

- 版本单一事实源 = `src-tauri/tauri.conf.json` version `0.1.0`（= package.json = Cargo.toml）。前端经 `getVersion()`（@tauri-apps/api/app）读，现仅在 system tab 底部只读展示。
- 仓库 remote = `https://github.com/lazygophers/aidog`。
- 导航：`App.tsx` NAV_ITEMS 顶级项 + `effectiveNav === id && <Page/>` 渲染；`Sidebar.tsx` `icons` map（key=icon 字符串）渲染 svg 图标。
- opener 插件已注册（`tauri_plugin_opener::init()` lib.rs:3233，Cargo `tauri-plugin-opener="2"`），前端未用过 → `@tauri-apps/plugin-opener` `openUrl(url)`。
- 后端命令模式：`#[tauri::command]` + 注册进 `tauri::generate_handler!`（lib.rs:3406）。`build.rs` 现仅 `tauri_build::build()`。

## Requirements（决策已定）

- **放置**：侧栏顶级「关于」项（`nav.about`），新页 `src/pages/About.tsx`。
- **版本深度**：App 版本 + Tauri 版本 + OS + 架构 + 构建信息（profile + 构建时间 + git commit）。OS/arch/构建信息需后端命令补。
- **GitHub**：静态链接跳浏览器（仓库 / Releases / Issues / 提 Issue），opener 打开。**无在线更新检查**（无 GitHub API 请求）。

## Technical Approach

**后端**
- `build.rs`：编译期注入 `cargo:rustc-env=AIDOG_GIT_COMMIT`（`git rev-parse --short HEAD`，失败 `unknown`）+ `AIDOG_BUILD_TIME`（`SystemTime::now` epoch 秒，std 无新依赖）+ `rerun-if-changed`，末尾 `tauri_build::build()`。
- 命令 `about_info() -> AboutInfo`（serde Serialize，snake_case 字段）：`app_version`=`env!("CARGO_PKG_VERSION")`、`tauri_version`=`tauri::VERSION`、`os`/`arch`/`family`=`std::env::consts::*`、`profile`=`cfg!(debug_assertions)?"debug":"release"`、`git_commit`=`env!("AIDOG_GIT_COMMIT")`、`build_time`=`env!("AIDOG_BUILD_TIME")`。注册进 generate_handler。

**前端**
- `api.ts`：`AboutInfo` interface（snake_case 对齐后端）+ `aboutApi.info()` invoke 封装（独立 interface，禁 inline 类型）。
- `About.tsx`：`export function About()`，挂载调 `aboutApi.info()`；版本卡（glass-surface 行式 key/value，build_time 经 `new Date(secs*1000).toLocaleString()` 格式化）+ GitHub 链接卡（按钮 `openUrl(...)`）。
- `App.tsx`：NAV_ITEMS 加 `{id:"about", icon:"about", labelKey:"nav.about"}`（放末尾），渲染 `{effectiveNav === "about" && <About/>}`。
- `Sidebar.tsx`：`icons` map 加 `about`（info 圆圈 svg，18×18 currentColor stroke 1.5，与既有一致）。
- i18n：`nav.about` + `about.*`（title/version/tauriVersion/os/arch/profile/buildTime/gitCommit/githubTitle/repo/releases/issues/reportIssue 等）× 8 locale（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）。品牌名 AiDog/GitHub/Tauri 保留原文。

## Acceptance Criteria

- [ ] 侧栏出现「关于」项，点击进 About 页。
- [ ] 版本卡展示 App/Tauri/OS/arch/profile/构建时间/git commit，值来自后端真实数据（非硬编码）。
- [ ] GitHub 4 链接点击经 opener 打开正确 lazygophers/aidog URL。
- [ ] `cargo build` + `cargo clippy`（零 warning）+ `yarn build` green。
- [ ] `node scripts/check-i18n.mjs` exit 0（8 locale 全覆盖）。

## Out of Scope

- 在线更新检查 / GitHub Releases API 调用。
- 自动更新器（updater）。
- 贡献者列表 / 许可证全文展示（可后续）。

## Decision (ADR-lite)

- 放置=侧栏顶级（最显眼，用户选）。
- 版本=全量（App+Tauri+OS+arch+构建），构建信息走 build.rs 编译期注入（无运行时依赖、无新 crate）。
- GitHub=纯静态链接（无网络请求，离线可用）。
