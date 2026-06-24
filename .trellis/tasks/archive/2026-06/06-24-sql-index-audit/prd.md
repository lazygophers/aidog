# SQL/索引效率审计与优化

## 目标
审计 aidog 后端（`src-tauri/src/gateway/db/**` + proxy/router 等所有 SQL 触点）的 SQL 与 SQLite 索引，找出并消除效率问题：
- 不必要 / 重复 / 可合并的 SQL
- 可移除的死代码 SQL（无调用方）
- 应新增的索引（高频 WHERE/JOIN/ORDER BY 列缺索引 → 全表扫）
- 可移除的冗余 / 重复 / 未命中索引（写放大、占空间）
- 低效查询模式（N+1、相关子查询、SELECT * 取整行、缺 covering index）

## 交付边界（用户已批 2026-06-24）
- **审计 + 落地安全优化**：先出完整发现报告，再直接落地低风险项（加缺失索引走新 migration、删确认的死代码 SQL）。
- **高风险项单列确认**：删除现有索引、改查询结构、改语义的改动 → 不自动落地，列清单交用户逐项拍板。
- **索引变更走新 SQLite migration**：版本号递增，对齐现有 migration 链（见 schema*.rs）。

## 风险分级
| 级别 | 例 | 处理 |
| --- | --- | --- |
| 低（自动落地） | 加缺失索引、删无调用方 SQL/常量 | exec 直接改 + migration |
| 高（待确认） | 删现有索引、改 WHERE/JOIN 结构、合并查询改语义 | 列报告，用户逐项批 |

## 验收
- 全部 SQL 触点 + 索引 DDL 已盘点（无遗漏文件）
- 每条发现含 file:line + 理由 + 风险级 + 验证依据（EXPLAIN QUERY PLAN / grep 调用方）
- 低风险项落地后：`cargo build` + `cargo clippy`(零 warning) + `cargo test` 全绿
- 加索引项有 EXPLAIN QUERY PLAN 前后对比（SCAN → SEARCH）佐证
- 不改数值语义（统计/聚合结果与改前等价）

## 非目标
- 不重构表结构（除索引外的 schema 改动）
- 不动前端
- 高风险项不在本 task 自动落地

## 依赖 / 冲突
- ⚠️ 后台 aidog-bug-hunt 正改 `usage_stats.rs`（reroute 平台 usage → stats_agg_hourly）。本 task **apply 阶段须等其完成**再开，防 db 文件撞。research（只读）可并行。
