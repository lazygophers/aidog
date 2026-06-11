# Design: quota 请求驱动预估增量更新

详细插入点见 research/ 7 文档。本文聚焦决策 + 算法 + 架构。

## Schema（migration 004）
platform 加 4 列（NOT NULL DEFAULT，遵 db-conventions）：
```sql
-- 001_init.sql 加列（新库直接含）
est_balance_remaining REAL NOT NULL DEFAULT 0,
est_coding_plan       TEXT NOT NULL DEFAULT '',
last_real_query_at    INTEGER NOT NULL DEFAULT 0,
estimate_count        INTEGER NOT NULL DEFAULT 0
```
- migration 004（旧库补列）：init_tables 加 `let _ = conn.execute("ALTER TABLE platform ADD COLUMN ...", [])`（忽略 duplicate column，参照 v1 ALTER 模式）。注意：schema v2 重构曾删 ALTER 数组改全量 001_init —— 本次恢复一个**最小 ALTER 块**仅补这 4 列
- db.rs 同步：PLATFORM_COLUMNS + PLATFORM_COLUMNS_PREFIXED + row_to_platform(新 index) + **get_group_platforms(db.rs:411) 第二处手写 parser**(偏移+2) + create_platform/update_platform INSERT/UPDATE（预估列由系统维护，create 默认 0/''，update 不覆盖预估列）
- models.rs Platform + api.ts Platform 加 4 字段（est 字段前端只读展示）

## 余额预估（精确，非 coding-plan 平台）
- proxy 请求完成 → 后台 `tokio::spawn`：`resolve_price(db, model, platform_type, fallback_in, fallback_out)` → cost = in_tok×in_cost + out_tok×out_cost + cache_tok×cache_cost
- **原子自减**单 SQL（禁持锁跨 await，避免并发丢更新）：
  `UPDATE platform SET est_balance_remaining = est_balance_remaining - ?cost, estimate_count = estimate_count + 1 WHERE id = ?`
- coding plan 平台（订阅计费）无余额，跳过余额预估

## coding plan 预估（方案 B 拟合 + Kimi 精确）
est_coding_plan JSON：
```json
{"tiers":[{"name":"five_hour","est_utilization":45.2,"coef_per_token":0.00012,"util_at_last_real":40.0,"tokens_since_real":43000,"has_base":true}],"level":"..."}
```
- **Kimi（精确, has_base=true 来自 limit/remaining）**：quota.rs:265 真查时**保留** limit/remaining；每 token 的 % = 100/limit；est_utilization += token×(100/limit)
- **GLM/MiniMax（方案 B 拟合）**：
  - 真查时记 util_at_last_real + 重置 tokens_since_real=0
  - 每请求 tokens_since_real += token；若已有 coef_per_token（上一窗口拟合得）：est_utilization = util_at_last_real + tokens_since_real×coef_per_token
  - 下次真查时拟合更新 coef：`coef = (util_real - util_at_last_real) / tokens_since_real`（仅当无跨 reset；Στoken>0）
  - **冷启动**（无 coef）：不预估，est_utilization = 真值
  - **reset 检测**（resets_at 过期 / util_real < util_at_last_real）：丢弃本窗口样本，coef 保留或重拟合，tokens 归零
- coding plan 预估也在 proxy 后台 spawn（读 est_coding_plan JSON → 更新 → 写回，需平台级串行/CAS 避免并发覆盖；用单事务 read-modify-write 或平台锁）

## 校准（降频核心）
后台预估时检查：`now - last_real_query_at > 300_000 || estimate_count >= 100`
→ 触发真实 `query_quota`(quota.rs:407, async, **锁外调用**) → 覆盖 est_balance_remaining + est_coding_plan(含 Kimi limit / 方案B coef 拟合) + last_real_query_at=now + estimate_count=0

## 前端
- api.ts Platform 加 est_* 字段；Platforms.tsx quota 区展示**预估值**（来自 platform.est_*，非每次 query）+ 「预估」标识（区别真值）；刷新图标触发真查校准（已有 refreshQuota → 后端 query_quota 覆盖 est + 重置）
- 展示优先 est_*（有值），冷启动/无 est 时显真查值

## 不改 / 注意
- 别窗口/worktree 并行改 db.rs → 主工作区改，commit 仅本 task 列，留意冲突
- 流式 token 时序（research 疑点）：确认 proxy.rs:578 load 拿最终累计值后再 spawn 预估

## 验证
- cargo build+test+tsc；单测：余额原子自减、Kimi 精确增量、方案B 拟合 coef、reset 丢样本、校准触发阈值、冷启动不预估
