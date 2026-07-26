# 架构深化: C1-C10 候选全执行 (deletion test / locality / leverage) — 详细设计

架构 / 数据流 / 关键取舍 / 技术选型 (不含调度图, 调度归 task.json):

## 依赖 DAG

```
Wave 1 (无依赖, 并行 max 2):
  C1 死代码       — settings/shared (前端)
  C2 forward 502  — proxy/forward.rs (Rust)
  C3 parser 同构  — cli_proxy_parser/parser.rs (Rust)
  C4 passthrough  — proxy/passthrough.rs (Rust)
  C8 mitm seam    — proxy/connect.rs + mitm/ (Rust)
  C9 db mod 拆    — gateway/db/mod.rs (Rust)

Wave 2 (C1 完):
  C5 PlatformCard — platforms 前端 (C1 删 EnhancedSelect 先, 避 grep 误判)

Wave 3 (C5 完):
  C6 event 总线   — platforms 前端 (PlatformCard 改完再收 event)

Wave 4 (C6 完):
  C7 god surface  — platforms 前端 (event 收完再 reducer 化)

Wave 5 (C9 完):
  C10 schema 编号 — gateway/db (db 拆完再整 migration 编号)
```

跨子系: Wave1 六 task 分属前端(C1) / Rust proxy(C2/C4/C8) / Rust parser(C3) / Rust db(C9) — 文件无重叠, 真并行。Wave2-4 前端串行 (同 platforms 子系)。Wave5 db 串行 (C9 后)。

## 各候选 deletion test 判定

### Strong (deletion test 通过 — 删浓缩)

| 候选 | 判定 | 证据 |
|---|---|---|
| C1 死代码 | 删浓缩 | 3165 行零 caller, barrel 假活, 删后 _shared.tsx 消费面收窄连锁简化 |
| C2 forward 502 | 删浓缩 (真冗余) | 5 处逐字近似, 各 ~30 行, 抽 finalize_proxy_502 单点防字段错位 |
| C3 parser array | 删浓缩 (真冗余) | 5 个 parse_*_array 逐行同构, 加新 CPA 段成本归零 |
| C4 passthrough | 删浓缩 (真冗余) | 双 handler ~95% 同构, StreamLogGuard 双写漂移已致 bug (自述) |
| C5 PlatformCard | 删浓缩 | 6 effect → 1 (useProtocolMeta), 健康判定可测, getBaseUrl 三真相收一 |

### Worth exploring (边界明显, 需 grill 定形态)

| 候选 | 待决 | grill 点 |
|---|---|---|
| C6 event 总线 | afterPlatformMutation opts 形态 | ① 全收口 (3 event 废) / ② 部分收 (test-completed 保留, groups-changed 收) / ③ ref + event 并存文档化。Groups 跨页面 (App.tsx tab) ref 生命周期需验 |
| C8 mitm seam | MitmOutcome 形态 | ① 单 enum (Connected/PinningSuspect/IoError/BlindRelay) / ② trait object / ③ 保留 handle_mitm 在 connect 仅收反向依赖。serve_plaintext 跨模块递归是否可消 |

### Speculative (结构性, 长尾, grill 评估是否做)

| 候选 | deletion test | grill 点 |
|---|---|---|
| C7 god surface | **未通过** (抽 usePlatformForm 只挪) | ① reducer (interface 70→~5, leverage 高迁移面大) / ② PlatformPasteCtx 改 ref (消第三真相) / ③ 保持现状文档化 |
| C9 db mod 拆 | 部分通过 | KeyPair/DbCache (40行 micro-opt) 移 cache.rs, parse/serialize 移 platform.rs — 纯结构收益 |
| C10 schema 编号 | N/A (非模块拆分) | ① 改日期戳 / ② 文档化编号-执行序分离 / ③ 按库拆文件。痛感低 (注释密度非结构) |

## 关键取舍

- **不重构 call_*_traced 6 份复制** (db F1): maintenance.rs:64 注释 self-admitted 选复制, 泛型签名复杂度≈复制成本, 等第三个变化点再收 (ponytail ladder)
- **不引入 query builder**: SQL 不复杂, 手拼 format! + 参数化已正确 (build_filter_where ?{idx} 动态编号), diesel/sqlx 反增依赖
- **不拆 schema_late.rs 按库分文件**: 已按库分 3 fn (run_migrations_late/proxy_log_late/platform_late), 1330 行痛感来自注释密度非结构, 仅 C10 整编号体系
- **C7 优先级最低**: deletion test 未过 = 浅模块, 真 reducer 化需 form state model 重设计, ROI 不如 Strong 候选; grill 后可能判"保持现状文档化"
- **proxy test 兜底**: forward.rs 290 行 / parser.rs 305 行 / passthrough 部分 / db test_mod 102-340 — 抽 helper 不破测试, 反增可测性 (纯函数层 → helper 层)

## 执行载体

- **Rust 改动** (C2/C3/C4/C8/C9/C10): 派 skein-executor 在仓库根改, 跑 cargo test + clippy 验
- **前端改动** (C1/C5/C6/C7): 派 skein-executor 在仓库根改, 跑 yarn test + tsc + check-i18n 验
- **grill 决策** (C6/C7/C8/C10): main 跑 /grilling skill 与用户走决策树, 定形态后落 design.md 再派 exec

## grill 待决点汇总 (Worth exploring + Speculative)

1. **C6**: 3 magic event 全收口 vs 部分保留? afterPlatformMutation opts 字段集? Groups 跨页面 ref 生命周期?
2. **C7**: reducer 路线 (高 leverage 大迁移) vs ref 路线 (消第三真相) vs 文档化 (零改动)?
3. **C8**: MitmOutcome 形态 (enum / trait / 保留)? serve_plaintext 跨模块递归可消否?
4. **C10**: 编号方案 (日期戳 / 文档化 / 按库拆)? 痛感低是否值得做?

Strong 候选 (C1-C5) 无 grill 待决点, 形态已定 (审查报告 before/after 图明确)。
