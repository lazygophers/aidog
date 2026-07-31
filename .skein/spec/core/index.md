# SKEIN core 规则索引 (章节粒度: 一行一条规则)

类目: arch(4), db(2), domain(3) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | inclusion | anchors | status/出链 | summary |
|---|---|---|---|---|---|---|---|
| arch/stream-buf-unified-cap.md#关联 | arch | 关联 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active / →hot-path-buffers,stream-buf-no-batching | [[stream-buf-no-batching]] [[hot-path-buffers]] |
| arch/stream-buf-unified-cap.md#案例 | arch | 案例 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active | **正例** —— usage 侧定义 `SSE_LINE_BUF_MAX_BYTES = 1MB`，内容侧转换分支引用… |
| arch/stream-buf-unified-cap.md#硬约则 | arch | 硬约则 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active | 同一隐患在代码库内多条执行路径出现时，上界值**禁止多处定义**。一处常量定义，其余路径引用 → 防止行为割裂。  ##… |
| arch/stream-buf-unified-cap.md#适用 | arch | 适用 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active | - 任何多路径并发处理同一数据流的缓冲 - 多个解析器共用一个上界（如 SSE / WebSocket 等流协议） - … |
| db/sqlite-read-cache-config.md#SQLite 只读缓存定值 | db | SQLite 只读缓存定值 | sqlite,cache,readonly,memory,hardcoded | auto | - | active | 通过 `PRAGMA cache_size = -64` 限制每条只读连接的页缓存驻留，实测指标达标。  ### 硬约束… |
| db/sqlite-read-cache-config.md#关联 | db | 关联 | sqlite,cache,readonly,memory,hardcoded | auto | - | active / →sqlite-cache-residency-probe-method | [[sqlite-cache-residency-probe-method]] |
| domain/peak-multiplier-symmetry.md#关联 | domain | 关联 | peak,multiplier,estimate,budget,symmetry,billing,财务 | always | - | active / →rule-66,time-tiers-apply-idiom | [[rule-66]] [[time-tiers-apply-idiom]] |
| domain/peak-multiplier-symmetry.md#硬约则 | domain | 硬约则 | peak,multiplier,estimate,budget,symmetry,billing,财务 | always | - | active | estimate 流程中**任意处加 peak 倍率，对边必补同倍率**（既存 bug 根因）：  - 余额扣减（`es… |
| domain/peak-multiplier-symmetry.md#禁用 | domain | 禁用 | peak,multiplier,estimate,budget,symmetry,billing,财务 | always | - | active | ❌ 仅余额扣减乘倍率，手动预算漏乘 → 成本显示 ≠ 实际扣款   ❌ 仅某处乘倍率，其他相关路径不补 → 高峰期估算 … |
