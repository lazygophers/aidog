# 关于页 Claude Code/Codex CLI 版本检查与安装

## Goal

aidog 关于页（`About.tsx`）现仅展示 aidog 自身版本 + Tauri updater。用户依赖 Claude Code / Codex CLI 工作，但无统一入口查看其安装状态、版本、升级、冲突诊断。参考 cc-switch「关于>本地环境」，在关于页加「本地环境」section，覆盖：版本检查 / 安装 / 升级 / 诊断冲突。

## What I already know (research 4 文件已落盘)

- **cc-switch 模式**（`research/cc-switch-local-env.md`）：后端直接 spawn（非 plugin-shell），三命令架构（get_tool_versions / run_tool_lifecycle_action / probe_tool_installations），`$SHELL -lic` 版本探测三态，锚定升级绕 GUI PATH 不对称。
- **CLI 安装技术**（`research/cli-install-tech.md`）：
  - Claude Code：npm `@anthropic-ai/claude-code` + 首推 native installer `claude.ai/install.sh` + `claude update` 自升级
  - Codex：npm-only `@openai/codex`（JS launcher + optional 平台二进制），uninstall+install 自愈损坏二进制
- **Tauri shell 可行性**（`research/tauri-shell-feasibility.md`）：aidog 已具备全基础设施。`install_uv`（`script_executor.rs:37`）后端 spawn 是成熟模板；`ensure_runtime_path`（`app_setup.rs:22` 启动调用）已解 PATH 不对称；建议抄 `install_uv` 后端 Rust command 包裹模式，**不动 capability**。
- **诊断冲突**（`research/install-conflict-diagnosis.md`）：`which -a` / `where` 全路径枚举 + `canonicalize` 去重 + 路径前缀推断 source（nvm/homebrew/volta/npm global/native installer）+ 双阈值判定（严=is_conflicting 版本分歧或运行态混合；宽=needs_confirmation ≥2 处即弹确认）。
- **CREATE_NO_WINDOW**（`0x08000000`）：aidog 未实现，Windows 后端 spawn `cmd /C` 必须加，否则闪黑窗。

## Requirements

### MVP 工具范围
- **Claude Code** + **Codex** 两工具（research 建议，去掉 gemini/opencode/hermes 降维护成本）

### 后端（Rust command，抄 install_uv 模式）
1. `cli_check_versions() -> Vec<CliToolStatus>`：spawn `claude --version` / `codex --version`（复用 `ensure_runtime_path` 后的 PATH），返 `{name, installed, version, path, latest?(可选), conflict?}`。
2. `cli_install(tool: "claude"|"codex")` / `cli_upgrade(tool)`：后端 spawn 安装/升级命令（Claude 走 native installer 或 npm；Codex 走 npm i -g @openai/codex）。Windows 加 `CREATE_NO_WINDOW` flag。
3. `cli_diagnose_conflicts() -> Vec<CliConflict>`：`which -a` / `where` 枚举 + canonicalize 去重 + source 推断，返冲突清单 + 建议。
4. Codex 损坏自愈：检测到平台二进制损坏 → 提示走 uninstall + install（非自动执行，需用户确认）。

### 前端（About.tsx 加「本地环境」section）
5. 工具卡片列表：工具名 + 版本 + 安装路径 + 状态徽标（✅ 已装最新 / ⬆️ 可升级 / ❌ 未装 / ⚠️ 冲突）。
6. 操作按钮：检查版本 / 安装 / 升级 / 诊断冲突（loading 态 + toast 反馈，复用 mitm importDefaults 模式）。
7. 冲突诊断结果：列出所有安装路径 + source + 标红冲突项 + 建议（保留哪个/卸载哪个，**只报告不自动卸载**）。

### i18n（8 语言）
8. 加 `about.localEnv.*` key 命名空间（toolName/version/path/status/install/upgrade/diagnose/conflict）。

## Acceptance Criteria

- [ ] 关于页展示 Claude Code / Codex 版本 + 路径 + 状态
- [ ] 未装工具点「安装」→ 后端 spawn 安装命令 → 成功后版本刷新
- [ ] 已装非最新点「升级」→ 升级成功
- [ ] 冲突诊断：PATH 上 ≥2 个同名二进制 → 列出全部 + 标红 + 建议
- [ ] Windows 不闪黑窗（CREATE_NO_WINDOW）
- [ ] 复用 ensure_runtime_path（GUI 启动 PATH 不对称已解，不额外处理）
- [ ] cargo clippy 0 warning；cargo test 通过；yarn build clean
- [ ] 8 语言 key 齐全（check-i18n 过）
- [ ] 重启 dev 后生效（memory tauri-rust-command-needs-restart）

## 🔴 需要用户确认（grill 硬门1，start 前定）

1. **MVP 工具范围**：claude + codex only？还是加 gemini/opencode？
2. **版本检查触发**：启动自动检查（节流 24h，同 aidog updater 模式）？还是仅手动按钮？
3. **安装/升级执行**：后端直接 spawn（装信任用户）？还是每步确认弹窗？
4. **自动升级**：默认开（定时检查+提示）？默认关（纯手动）？
5. **冲突修复**：只报告 + 建议（不自动卸载）？还是提供「一键卸载旧的」按钮？

## Out of Scope

- 不替代 aidog 自身 updater（Tauri updater 保持不动）
- 不自动卸载冲突二进制（破坏性，只报告）
- 不加 WSL / gemini / opencode / hermes 工具（MVP 后续迭代）
- 不改 capabilities / tauri-plugin-shell 配置（抄 install_uv 后端 spawn，零 capability 改动）
- 不实现「定时后台升级」（若用户选纯手动）

## Technical Notes

- 后端 spawn 参考：`script_executor.rs:37 install_uv` + `shared.rs:157` detect uv + `skills/env.rs:38` `$SHELL -ilc`。
- 复用 `ensure_runtime_path`（`app_setup.rs:22` 启动已调），新 CLI 检测代码自动受益。
- Windows CREATE_NO_WINDOW：`Command::new(...).creation_flags(0x08000000)`（cfg(windows)）。
- codex 二进制损坏检测：spawn `codex --version` 失败 + npm ls 显示已装 → 推断损坏。
- 参考 cc-switch 代码（Tauri 2.0 同栈）：`misc.rs:2139-2183` codex uninstall+install 自愈。
