# Research: 校准触发（>5min 或 >100 次）

- **Query**: last_real_query_at + estimate_count 读写；校准判定 → 触发 quotaApi.query + 重置
- **Scope**: internal
- **Date**: 2026-06-11

## 现状：真实 quota 查询入口

- `query_quota(base_url, api_key) -> PlatformQuota`（quota.rs:407）— 按 base_url 自动分发到各 query_xxx，返回 `PlatformQuota{success, balance, coding_plan, queried_at}`
- Tauri command：`platform_query_quota`（lib.rs:901-902）薄封装，已注册（lib.rs:1376）
- **当前没有后端定时/计数校准逻辑** —— 真查只由前端 load()（Platforms.tsx:870-878 批量）/ refreshQuota（:887 单个）手动触发。

## 校准判定逻辑（设计建议）

预估更新时（每请求后台 task）读取该平台 `last_real_query_at` + `estimate_count`：
```
now - last_real_query_at > 5*60*1000(ms)   ||   estimate_count >= 100
  → 触发真查
```
- 时间单位：db-conventions 强制毫秒戳（`chrono::Utc::now().timestamp_millis()`，db.rs:60 `now()`）。5min = 300_000ms。
- 触发后：调 `query_quota(base_url, api_key)`（需平台 base_url + api_key，proxy scope 内 `route.platform` 已有）→ 成功则：
  - 用真值覆盖 `est_balance_remaining` / `est_coding_plan`（含重新记录基数/拟合基线）
  - `last_real_query_at = now`，`estimate_count = 0`
- 失败（网络/HTTP）：保留预估值，**不重置计数/时间**（下次请求再试），避免永久卡死可加退避。

## 读写改动点

- 预估 task 内：一次 lock 内 `SELECT last_real_query_at, estimate_count, est_* FROM platform WHERE id=?` → 判定 → 分支：
  - 不校准：原子 `UPDATE ... SET est_balance_remaining = est_balance_remaining - ?, estimate_count = estimate_count + 1 WHERE id=?`
  - 校准：`query_quota`（**异步、不能在持 Mutex 时 await**——Mutex 是 std::sync::Mutex，跨 await 持锁会 panic/死锁）→ 拿到结果后再 lock 写覆盖。
- **关键约束**：`Db.0` 是 `std::sync::Mutex`（db.rs:43），**禁在持锁状态 .await**。校准流程须：① 持锁读判定 + drop lock → ② await query_quota → ③ 重新 lock 写。

## 建议新增 db.rs 函数
- `read_platform_estimate_state(db, id) -> (last_real_query_at, estimate_count)` — 持锁短读
- `apply_estimate_delta(db, id, balance_delta, coding_delta)` — 原子自减 + count+1
- `write_real_quota(db, id, quota: &PlatformQuota)` — 校准覆盖 + 重置 count/time

## Caveats
- query_quota 含 10s 超时（quota.rs:85），后台 task 触发可接受，但高频平台多个并发触发校准会浪费——可加 per-platform "正在校准" 标志避免重复触发（estimate_count>=100 时多请求同时命中）。
