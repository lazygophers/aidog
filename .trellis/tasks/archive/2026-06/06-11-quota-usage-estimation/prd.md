# PRD: quota 请求驱动预估增量更新（降频）

## 背景
每次 proxy 请求完成（拿到 token）→ 本地预估增量更新余额 + coding plan，降低上游 quota 查询频率。

## 决策（已确认）
| 项 | 结论 |
| --- | --- |
| 预估范围 | 余额 + coding plan 都预估 |
| coding plan 算法 | **方案 B 拟合全支持**：Kimi 精确（上游 API 有 limit/remaining 绝对基数，quota.rs:265 已解析需保留）；GLM/MiniMax Δutilization/Σtoken 拟合每 token 的 %（冷启动期不预估只显真值，窗口 reset 丢污染样本） |
| 余额预估 | token × `resolve_price`(db.rs:1074) 单价扣减；coding plan 平台无余额（订阅计费）不算余额 |
| 持久化 | **platform 表加列**（非新表） |
| 校准 | 距上次真查 >5min(300_000ms) **或** estimate_count >=100，任一满足 → `query_quota`(quota.rs:407) 覆盖 + 重置 |
| migration | 001_init 加列（新库）+ migration 004 ALTER 幂等（旧库补列，忽略 duplicate column） |

## platform 加列（migration 004）
- `est_balance_remaining REAL NOT NULL DEFAULT 0` — 预估余额
- `est_coding_plan TEXT NOT NULL DEFAULT ''` — 预估 coding plan JSON（含 tiers est_utilization + 方案B 拟合系数/样本）
- `last_real_query_at INTEGER NOT NULL DEFAULT 0` — 上次真实 query 毫秒戳
- `estimate_count INTEGER NOT NULL DEFAULT 0` — 自上次真查以来预估次数

## 关键技术约束（research）
- `Db.0` 是 std::sync::Mutex → **禁持锁跨 .await**；校准是 async；余额预估用单条 SQL 原子自减避免并发丢更新
- 预估在 proxy upsert_log 后 **tokio::spawn 后台**（非流式 :487 后 / 流式 token 收尾处），不阻塞响应
- `get_group_platforms`(db.rs:411) 有**第二处手写 platform row parser**（偏移 +2），加列须同步
- ALTER 无 IF NOT EXISTS → 用 `let _ = execute(ALTER)` 忽略 duplicate column 错（参照旧 migration 数组模式）

## 涉及面（详见 research/00-summary）
后端: migrations/001_init.sql + db.rs(PLATFORM_COLUMNS/row_to_platform/get_group_platforms parser/migration/预估 SQL/校准) + proxy.rs(spawn 预估触发) + quota.rs(Kimi 保留 limit/remaining) + models.rs(Platform 加字段) + lib.rs(command)
前端: api.ts(Platform 加字段) + Platforms.tsx(展示预估值+标识，接刷新图标)

## 验收
- 请求后余额/coding plan 本地预估更新（Kimi 精确 / GLM-MiniMax 拟合）
- 5min 或 100 次触发真查校准覆盖
- 不阻塞请求响应；并发安全
- cargo build + test + tsc 通过

## 关联（独立后续 task，串行）
- **tray quota 展示**（已澄清：系统托盘 tray / 余额或 coding% 二选一 / 单平台互斥 / platform 加 show_in_tray+tray_display 字段）→ 本 task finish 后建，migration 005
- **分组路由 AI 平台拖动排序** → 待建
