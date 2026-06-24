# Research: Rust 覆盖率缺口 (r2, commit 514a95f / 1012 tests)

- **Query**: 度量当前 prod 代码 line 覆盖率, 按缺口排序待补文件, 列剩余 D 项
- **Scope**: internal (cargo llvm-cov 度量)
- **Date**: 2026-06-24
- **口径命令**: `cargo llvm-cov report --summary-only --ignore-filename-regex 'test_|/tests/'`, 再程序化剔除启动胶水 (main/startup/app_setup/shared/logging/lib + adapter 薄 mod.rs), **保留 commands/**

## 关键数字

- **PROD-ONLY line cov (排启动胶水, 保 commands/): 80.22% (14369/17911), 146 文件** → 已达 ≥80% 续推目标
- ALL-files line cov (含启动胶水): 78.61% (14474/18413)
- 基线曾 43.6% → 现 80.22%

## TOP 缺口 (gap = missedRegions × uncovered_line_frac, 降序)

| gap | file | line% | 归属 D | 补测路径 |
|---|---|---|---|---|
| 491 | commands/tray_render.rs | 0% | D7 | 491 region 渲染逻辑, 拆纯函数直测 |
| 314 | commands/tray.rs | 0% | D7 | 薄壳, 需 Tauri State harness 冒烟 |
| 240 | gateway/proxy/passthrough.rs | 35.1% | D6 | 同协议旁路改写分支, 需 mock 上游 |
| 202 | gateway/proxy/responses.rs | 5.9% | D6 | /v1/responses 处理, 需 mock |
| 151 | commands/model_test.rs | 0% | D7 | 薄壳, State harness |
| 121 | commands/proxy.rs | 0% | D7 | 薄壳, State harness |
| 101 | commands/quota.rs | 0% | D7 | 薄壳, State harness |
| 95 | gateway/proxy/notify.rs | 0% | D6 | 通知派发, 可纯函数直测 |
| 79 | commands/skills.rs | 0% | D7 | 薄壳 (115 行), State harness |
| 78 | gateway/skills/ops.rs | 56.8% | (P3外) | npx/FS 操作, 需 mock/tempdir |
| 68 | commands/group.rs | 19.8% | D7 | State harness |
| 66 | commands/model_fetch.rs | 0% | D7 | State harness |
| 66 | gateway/skills/bulk.rs | 52.6% | - | 批量 npx, 需 mock |
| 58 | gateway/proxy/mod.rs | 28.8% | D6 | 入口编排, 需 mock 上游 |
| 55 | gateway/proxy/forward.rs | 64.4% | D6 | 转发主路径, 需 mock |
| 51 | commands/sync_settings.rs | 62.4% | D7 | 拆纯函数 (剩余分支) |
| 51 | gateway/quota/newapi.rs | 58.6% | D5 | JSON→余额解析, 喂 fixture 直测 |
| 43 | commands/settings.rs | 28.3% | D7 | State harness |
| 40 | gateway/quota/balance.rs | 56.9% | D5 | JSON→余额解析, 喂 fixture 直测 |
| 38 | gateway/proxy/group_info.rs | 61.6% | D6 | 需 mock |
| 36 | commands/backup.rs | 17.2% | D7 | State harness |
| 29 | commands/app_log.rs | 33.3% | D7 | State harness |
| 29 | commands/hooks.rs | 57.9% | D7 | State harness |
| 24 | gateway/proxy/handler.rs | 79.1% | D6 | 临界, 需 mock |
| 23 | gateway/notification/tts.rs | 46.9% | - | macOS AVFoundation, say 兜底难测 |

## 剩余未达标 Deliverable

- **D5 (quota 解析)**: 部分完成。coding_plan 82.9% / http 91.7% 达标; **balance.rs 56.9% / newapi.rs 58.6% / mod.rs 55.95% 未达标** — 需抽 JSON→余额纯函数喂脱敏 fixture。
- **D6 (proxy 编排)**: 未达标。responses 5.9% / passthrough 35.1% / notify 0% / mod 28.8% / forward 64.4% / group_info 61.6% 均 <80% — 需 mock 上游基建 (httpmock/本地 axum); notify.rs 可纯函数直测免 mock。
- **D7 (commands/ 全覆盖)**: 大面积未达标。tray_render/tray/model_test/proxy/quota/skills/model_fetch 全 0%; group/settings/backup/app_log/notification/popover/mcp <60% — 需 Tauri State/AppHandle harness; tray_render(491R)/sync_settings 优先拆纯函数。
- **D0-D4**: D1-D4 标 ✅ 且各文件实测达标 (adapter 全 ≥90%, router/i18n/codex/model_price/estimate 均 ≥80%)。D0 工具链就绪。

## Caveats

- gap 权重 = missedRegions × uncovered_line_frac, 兼顾文件体量与未覆盖比例; 非纯 line% 排序。
- 总数已 80.22% 达标; 但若实现阶段欲拉高至 D6/D7 各文件 ≥80%, tray_render/responses/passthrough/notify + quota balance/newapi 是必补点。
- branch% 未测 (本轮只读 line 口径)。
