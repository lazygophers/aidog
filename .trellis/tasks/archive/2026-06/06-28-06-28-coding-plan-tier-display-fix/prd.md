# PRD — Coding plan tier statusline 显示修复

## 背景
statusline 输出 `5h 97%·mcp_monthly 0% (重置 383h54m)`，coding plan 配额段存在 5 个显示/计算缺陷。
载体 = Python statusline 渲染引擎 `scripts/statusline-golden/engine.py` `seg_group_coding`（golden 字节回归锁定）；
数据源 = 后端 `group_info.rs`（下发 `name`/`utilization`/`level`/`reset_at`，源自 `est_coding_plan` 预估侧）；
mcp 数值上游 = `coding_plan.rs` GLM `TIME_LIMIT` 字段映射。

## 缺陷清单与期望（用户原文 → 期望行为）

| # | 用户原文 | 期望 | 根因定位 |
|---|---|---|---|
| 1 | 5h 97% 应绿，不应受后面 0% 影响 | 每 tier 按各自剩余% 独立上色（5h 97% → 绿） | `engine.py:499-517` `seg_group_coding` 整行取 `worst=min(tiers, remain)` 单色，mcp 0% 把整行（含 5h）染红 |
| 2 | 重置时间应为灰色 `383h54m`，不要中文"重置" | 去中文"重置 "前缀，倒计时灰色显示 | `engine.py:512` `" (重置 " + ...`；颜色当前随整段红色 |
| 3 | mcp 实际剩 100%，`0%` 是已用，算错了 mcp 重置 | mcp 剩余%正确（已用 0% → 剩余 100%），reset 正确 | `coding_plan.rs:153-163` GLM `TIME_LIMIT` 字段映射 `usage`/`currentValue`/`remaining`/`percentage` 语义需核实，`utilization=used/total*100` 疑似反向 |
| 4 | `mcp_monthly` → `mcp（30d）` | tier 标签显示 `mcp（30d）` | `engine.py:472-473` else 分支直接用原始 `name`，未映射 |
| 5 | `383h54m` 应为 `xxdxxhxxm` 格式 | reset 倒计时 d/h/m 三段进位（如 `15d23h54m`） | `engine.py:510` `h = d // 3600` 小时不进位天 |

## 范围
- in：engine.py coding 段渲染（颜色拆分/标签/reset 格式/去中文）+ coding_plan.rs GLM mcp 字段映射 + golden fixtures 更新 + 若 statusline-gen.ts/runtime.ts 含对应生成逻辑须同步。
- out：PlatformCard.tsx 卡片展示（独立载体，本次不动，除非 tierLabel 需统一——但 bug4 要 `mcp（30d）` 与卡片 `MCP` 不同，**仅改 statusline**）。

## 跨层约束
- engine.py 改动须同步 golden fixtures（`scripts/statusline-golden/`）并通过字节回归。
- 若 statusline 实际运行脚本由 `statusline-gen.ts` 生成而非直接用 engine.py，须保证 TS 生成逻辑与 engine.py 字节一致。
- bug3 须核实 GLM `/api/monitor/usage/quota/limit` 的 `TIME_LIMIT` 项字段真实语义（用户实测 mcp 剩 100%）；不确定时在返回标 `需要:` 请 main 转达用户索取真实响应。

## 验收
- 5 项缺陷逐条修复，statusline 输出 `5h 97%·mcp（30d）100% ...`（5h 绿、mcp 绿、reset `15d23h54m` 灰色无"重置"字）。
- `cargo test`（coding_plan/usage_color）+ statusline golden 回归 + `yarn build`(tsc) 全绿。
- `cargo clippy` 无 warning。
