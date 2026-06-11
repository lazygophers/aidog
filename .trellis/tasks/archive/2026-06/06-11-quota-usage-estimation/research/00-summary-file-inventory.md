# Research Summary: 请求驱动 quota 预估增量更新

- **Date**: 2026-06-11
- **基线**: 当前 working tree（master，commit 6ab49e6）。注意 worktree `.trellis/worktrees/06-11-quota-usage-estimation/` 存在且 db.rs 与主树 diff —— 本调研基于**主工作目录 cwd 文件**（cwd = 主树）。

## 调研文档
- `coding-plan-base-feasibility.md` — 清单 1（关键）
- `02-platform-columns-migration.md` — 清单 2
- `estimate-trigger-architecture.md` — 清单 3
- `calibration-trigger.md` — 清单 4
- `frontend-integration.md` — 清单 5
- `resolve-price-coding-plan.md` — 清单 6

## 核心结论速览

### 1. Coding plan 基数（关键）
- **Kimi 可行（方案 A）**：上游 `limit`/`remaining` 是绝对值，当前 quota.rs:265-285 **已拿到但丢弃**，只需保留 → 精确预估。
- **GLM / MiniMax 上游仅给百分比**（quota.rs:333 / 385/391），无绝对基数 → 推荐**方案 B（两次真查间 Δutilization/Σtoken 拟合每 token 的 %）**；冷启动期不预估只显真值；窗口 reset 须丢污染样本。
- 设计用 enum 分预估能力：Precise(Kimi) / Fitted(GLM,MiniMax) / ConfigRequired。

### 2. platform 加列 + migration
- 新列：`est_balance_remaining REAL DEFAULT 0`、`est_coding_plan TEXT DEFAULT '{}'`、`last_real_query_at INTEGER DEFAULT 0`、`estimate_count INTEGER DEFAULT 0`
- **Migration 编号 = 004**（现有 001/002/003）
- 同步改动点：`migrations/004_*.sql` + `db.rs:46-55 init_tables` + `db.rs:74 PLATFORM_COLUMNS` + `db.rs:78 PREFIXED` + `db.rs:82 row_to_platform`（新索引从 12 起）+ **`db.rs:411 get_group_platforms 第二处内联 parser`（偏移 +2，新列索引 13/14/15/16）** + `models.rs:261 Platform struct`
- 预估写入用**独立窄 UPDATE**，勿混入全量 `update_platform`（db.rs:191 会被覆盖）

### 3. 预估触发架构
- 触发点：proxy.rs:487（非流式）/ 流式 token 收尾处（**不是 :581**，那时 token 可能未累加完——既有时序疑点须确认）
- 不阻塞响应：`tokio::spawn` 后台 task，clone Arc<Db> + platform_id/type/model/tokens move 入
- 并发安全：`Db.0` 是 std::sync::Mutex（db.rs:43），余额用**单条 SQL 原子自减**避免丢更新；coding plan JSON 走同临界区 read-modify-write
- **硬约束：禁持 std Mutex 跨 .await**（校准 query_quota 是 async）→ 读判定 drop lock → await → 重 lock 写

### 4. 校准触发
- `now - last_real_query_at > 300_000ms || estimate_count >= 100` → 调 `query_quota`（quota.rs:407 / command lib.rs:901）→ 覆盖预估 + 重置 count/time
- 当前后端无任何校准/计数逻辑，全新增

### 5. 前端
- `quotaMap`（Platforms.tsx:789）当前纯前端 state，load() 每次进页面对所有平台真查上游（:870-878，频繁，正是要优化的）
- 推荐：扩展 Platform struct + api.ts Platform 接口带回预估值，load() 优先读库预估、仅过期才真查；UI 标"预估/实测"（用 last_real_query_at）；refreshQuota（:887）= 手动校准

### 6. resolve_price + coding plan 余额
- `resolve_price(db, model_name, platform_type, fallback_in, fallback_out) -> ResolvedPrice{per-token costs}`（db.rs:1074）
- **coding plan 平台无 model price 意义**：订阅制返回 coding_plan（utilization）不返回 balance → 预估**双轨**：按量平台用 resolve_price 扣金额，coding plan 平台走 utilization，不扣余额

## 完整涉及文件清单（供 subtask 拆分）

### 后端 — schema/db
| 文件 | 改动 | 规模 |
|---|---|---|
| `src-tauri/migrations/004_platform_quota_estimate.sql` | 新建 ALTER ADD COLUMN ×4 | 小 |
| `src-tauri/src/gateway/db.rs` | init_tables(:46)、PLATFORM_COLUMNS(:74/78)、row_to_platform(:82)、get_group_platforms parser(:411)、新增预估读写函数(read_estimate_state/apply_delta/write_real_quota) | 中 |
| `src-tauri/src/gateway/models.rs` | Platform struct(:261) 加 est_* 字段 | 小 |

### 后端 — quota 改造
| 文件 | 改动 | 规模 |
|---|---|---|
| `src-tauri/src/gateway/quota.rs` | Kimi query 保留绝对 limit/remaining(:245-291)；QuotaTier/CodingPlanInfo 扩展绝对量+拟合系数字段(:40-56) | 中 |

### 后端 — proxy 触发
| 文件 | 改动 | 规模 |
|---|---|---|
| `src-tauri/src/gateway/proxy.rs` | 非流式 :487 后 spawn 预估；流式 token 收尾点 spawn；预估函数(resolve_price+校准判定) | 中-大 |

### 前端
| 文件 | 改动 | 规模 |
|---|---|---|
| `src/services/api.ts` | Platform 接口加 est_* 字段(:537 附近 PlatformQuota 复用) | 小 |
| `src/pages/Platforms.tsx` | load()(:855) 优先读库预估；渲染(:1672) 加预估/实测标识；refreshQuota(:887) 语义校准 | 中 |

## 需 design / 用户决策的悬而未决点
1. **migration 落地方式**：走 init_tables include_str(与既有 002/003 一致，但 ALTER 无 IF NOT EXISTS 需检测列存在) vs 走 scripts/ 一次性脚本（db-conventions.md:61-65 spec 要求）。**两者冲突，须裁决。**
2. **GLM/MiniMax 完整 API 响应**：`需要: 抓取一次完整 JSON` 确认无隐藏绝对量字段（决定方案 B 是否唯一路）。
3. **流式 token 时序**：proxy.rs:578 load 是否拿到最终 token 值（既有疑点，预估正确性依赖）。
4. **coding plan 拟合冷启动 UX**：首校准周期无系数时 UI 如何表现（"预估中"占位）。
5. **拟合系数存储**：GLM/MiniMax 的 pct_per_token + utilization 基线存 est_coding_plan JSON 内 还是独立列。
6. **resolve_price platform_type key 格式**：裸协议名 vs serde 引号串（db.rs:1088）。
