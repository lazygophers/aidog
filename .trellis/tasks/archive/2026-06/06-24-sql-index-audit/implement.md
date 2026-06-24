# 执行编排 — sql-index-audit

## 阶段
1. **research（只读 fan-out，可与 bug-hunt 并行）** — 多 reader agent 分片盘点，各产结构化发现：
   - R1: 索引 DDL 全盘点 — 所有 `CREATE INDEX` / migration 内索引，列 (表/列/唯一性/出处)。
   - R2: SQL 触点盘点 — `db/**`、proxy.rs、router.rs、quota.rs 等所有 SQL 字符串，按 表×操作 归类，标调用方（grep 反查死代码）。
   - R3: 查询-索引匹配分析 — 每条高频查询的 WHERE/JOIN/ORDER BY 列 vs 现有索引，跑 EXPLAIN QUERY PLAN 判 SCAN/SEARCH，找缺索引 + 未命中索引。
   - R4: 冗余/重复 SQL + 可合并查询 — 同语义重复查询、N+1、相关子查询、SELECT * 整行取。
2. **synthesize（main）** — 汇总 4 路发现，去重，按风险分级（低=自动落地 / 高=待确认），出报告。低风险项形成 apply 工单。
3. **gate** — 等 aidog-bug-hunt 完成（防 usage_stats.rs / db 文件撞）。
4. **implement（workflow，worktree 隔离）** — writer agent 落地低风险项：加缺失索引（新 migration，版本号续 schema 链）、删死代码 SQL。
5. **verify（workflow）** — checker agent：cargo build/clippy/test 全绿 + 加索引项 EXPLAIN 前后对比。
6. **finalize** — task.py finish（hook commit→merge→archive→销 worktree）。高风险项清单回传用户另议。

## 资源互斥
- research 只读 → 无互斥，可全并行。
- implement 写 `db/schema*.rs` / migration → 与 bug-hunt 串行（gate 卡）。worktree 隔离。

## 失败回退
- EXPLAIN 无法证明加索引收益 → 该项降级为「待确认」不自动落地。
- migration 改动致测试失败 → 修测试 setup，不削断言；仍失败回退该索引。
