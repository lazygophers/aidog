# Research: statusline 金额颜色机制

- **Query**: statusline 脚本位置；如何拿金额并上色（ANSI?）；POST /api/group-info 响应结构；金额颜色当前怎么定
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### statusline 脚本不是静态文件 —— 前端拼 bash，后端落盘

- 脚本**内容**在前端 TS 生成：`src/components/settings/editors.tsx`（段定义 `SEGMENT_DEFS` + group 段 bash 生成器）。
- 脚本**落盘**走 Rust tauri 命令 `generate_statusline_script`（`src-tauri/src/lib.rs:1402-1427`），写入 `~/.aidog/aidog-statusline.sh`（普通）/ `aidog-subagent-statusline.sh`（subagent），chmod 0755。
- 鉴权链路：`do_sync_group_settings`（lib.rs:924+）为每 group 生成 `~/.aidog/settings.{group}.json`，注入 `ANTHROPIC_BASE_URL=http://127.0.0.1:{port}/proxy`（lib.rs:962-965）+ `ANTHROPIC_AUTH_TOKEN={group_name}`（lib.rs:966-969），并 strip `_aidog_statusline`/`_aidog_subagent_statusline`（lib.rs:975-976）。

### statusline **能上色**，且已用 ANSI truecolor（关键结论）

group 段 bash 生成器在 `editors.tsx` 直接 `printf '\033[38;2;%sm%s\033[0m'`（24-bit 真彩前景）：

- ANSI 三色常量（editors.tsx:1761-1763）：
  ```ts
  const ANSI_RED = "255;69;58";    // #FF453A
  const ANSI_AMBER = "255;214;10"; // #FFD60A
  const ANSI_GREEN = "52;199;89";  // #34C759
  ```
- **coding plan 段** `groupCodingDynBash()`（editors.tsx:1780-1803）：按 `pace` 上色——
  `fast→红 / normal→黄 / busy→绿`（editors.tsx:1787-1801），红色时若有最快档 `reset_at` 额外拼接 `(reset {h}h{m}m)`（editors.tsx:1789-1796）。输出 `printf '\033[38;2;%sm%s\033[0m'`（editors.tsx:1802）。
- **余额段** `groupBalanceDynBash(prefix)`（editors.tsx:1810-1830）：按 `balance_days_remaining` 上色——
  `<1 天→红 / <3 天→黄 / 否则绿（null→绿）`（editors.tsx:1819-1828，用 awk 比较），输出同样的 truecolor printf（editors.tsx:1829）。

**这意味着 statusline 侧的目标颜色规则（余额 <1/<3 天、coding pace）基本已存在**，颜色判定的"数据"由后端 group-info 端点算好下发，bash 只做阈值上色。

### group-info 拉取与缓存（editors.tsx）
- `aidogFetchPrelude()`（editors.tsx:1881-1896）：每次渲染最多 curl 一次，`POST {proxy_root}/api/group-info`，`-H "Authorization: Bearer ${ANTHROPIC_AUTH_TOKEN}"`，`--max-time 1` 静默失败，结果缓存到 `$__AIDOG_INFO_FILE` 共享给同行多个 group 段。
- proxy_root 推导：从 `ANTHROPIC_BASE_URL` 剥 scheme+host:port，去掉所有路径后缀（editors.tsx:1888-1889）。
- 守卫 `GROUP_GUARD`（editors.tsx:1769-1772）：`ANTHROPIC_BASE_URL` 未设 / 缓存为空 / `applicable!=true` → 降级空输出。

### POST /api/group-info 响应结构（`src-tauri/src/gateway/proxy.rs`）
路由注册：proxy.rs:50 `.route("/api/group-info", post(handle_group_info))`。handler：proxy.rs:119-290。

`GroupInfoResp`（proxy.rs:86-103）含金额/配额字段：
| 字段 | 类型 | 说明 / 来源 |
|---|---|---|
| `applicable` | bool | 仅"恰好 1 个关联平台"的 group 为 true（proxy.rs:181-183） |
| `balance` | f64 | `max(est_balance_remaining, manual total 预算剩余)`（proxy.rs:256-263） |
| `spent` | f64 | `stats.total_cost`（累计预估花费，proxy.rs:279） |
| `coding_plan` | `Vec<CodingTierResp>` | 见下 |
| `requests` / `success_rate` / `cache_rate` / `total_tokens` | — | usage stats（proxy.rs:201-207） |
| `currency` | String | **当前恒为空串**（proxy.rs:285） |
| `balance_days_remaining` | `Option<f64>` | **余额段上色依据**，见下 |

`CodingTierResp`（proxy.rs:105-114）：`name` / `utilization`(0-100) / `pace`("fast"|"normal"|"busy", coding 段上色依据) / `reset_at`(Option<i64> unix 秒)。

### 后端如何算颜色依据数据
- `balance_days_remaining`（proxy.rs:265-274）：
  ```rust
  let spent_7d = get_group_spent_since(&db, &group.name, 7).await?;
  let daily = spent_7d / 7.0;
  if daily > 0.0 && balance > 0.0 { Some(balance / daily) } else { None }
  ```
  即"近 7 天总花费 / 7 = 日均，余额 / 日均 = 剩余天数"。
- coding `pace`（proxy.rs:210-222）：来自 `estimate::tier_pace(&t)`（**纯利用率阈值**，详见 04 文件），`reset_at` 当前**恒为 None**（proxy.rs:219）。
- manual budget 窗口预算也被压成 coding tier，pace 按 util 80/50 阈值（proxy.rs:226-254）。

## Caveats / 结论

- **statusline 能上色（已用 ANSI 24-bit truecolor），不是只显数字** —— 余额段已实现 `<1红/<3黄/否则绿`，coding 段已实现 pace 三色。
- **改造重心在后端 group-info 的数据算法**，而非 bash 上色：
  - 余额"日用量"目前是 `spent_7d/7`（固定除 7），**任务要求"最近 7 天均值，无 7 天则用近 1 天，精确到小时"** —— 当前实现**不满足**（无"无 7 天回退近 1 天"、无小时级精度），需改 `get_group_spent_since` 或新增按天/小时聚合（见 03 文件）。
  - coding `pace` 目前**不是按剩余时间速率**，是利用率阈值；`reset_at` 恒 None（见 04 文件）。任务要求"剩余可用时间% = 剩余时间/周期时长"，需后端补 reset_at + 周期时长来源后改 `tier_pace` 或新增 pace 算法。
- **最大不确定点**：bash 侧三色阈值（余额 <1/<3、coding fast/normal/busy）与任务文案的 coding 阈值（<40%/40-60%）**语义不同**——coding 现按"利用率高→fast→红"，任务要按"剩余时间%低→红"。需确认是改 bash 阈值还是改后端把"剩余时间%"映射成 pace 字符串后复用现有 bash（后者改动最小）。
