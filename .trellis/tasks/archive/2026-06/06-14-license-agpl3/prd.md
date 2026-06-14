# 开源协议改为 AGPL-3.0

## Goal

将项目开源协议设为 AGPL-3.0：新增 `LICENSE` 全文 + 各 manifest license 字段 + README 许可说明。

## What I already know（探得现状）

- 项目**当前无任何 license**：无 `LICENSE` 文件、`package.json`/`Cargo.toml`/`docs/package.json` 无 license 字段、README 无许可节。故「修改」实为从零新增 AGPL-3.0。
- 仓库 `github.com/lazygophers/aidog`。
- 用户表述「agpl3.x」→ SPDX `AGPL-3.0-or-later`（AGPL 仅 3.0 一个版本号，or-later 覆盖未来版本，符合「3.x」意图）。

## Requirements

- `LICENSE`：AGPL-3.0 官方全文（已 curl 自 gnu.org，661 行逐字精确）。
- `package.json` + `docs/package.json`：`"license": "AGPL-3.0-or-later"`。
- `src-tauri/Cargo.toml` `[package]`：`license = "AGPL-3.0-or-later"`。
- `src-tauri/tauri.conf.json`：`bundle.licenseFile` 指向 `../LICENSE`（安装包许可元数据）。
- `README.md`：加「许可 / License」节，注明 AGPL-3.0 + 链接 LICENSE。

## Acceptance Criteria

- [ ] `LICENSE` 存在，首行 `GNU AFFERO GENERAL PUBLIC LICENSE`，含完整条款（661 行）。
- [ ] 3 manifest license 字段 = `AGPL-3.0-or-later`；JSON 合法；`node scripts/sync-version.mjs --check` 仍 exit 0（license 不影响 version）。
- [ ] tauri.conf `bundle.licenseFile` 设置；`cargo build` 或 tauri schema 不报错。
- [ ] README 有许可节。

## Out of Scope

- 每源文件加 SPDX header（侵入性大，未要求）。
- 第三方依赖 license 兼容性审计（AGPL 传染性分析）。
- CLA / 贡献者协议。

## Decision (ADR-lite)

- SPDX id 用 `AGPL-3.0-or-later`（对应「3.x」）。LICENSE 全文取 gnu.org 权威源，禁凭记忆生成（34KB 易错）。
