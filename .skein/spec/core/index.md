# SKEIN core 规则索引 (章节粒度: 一行一条规则)

类目: arch(11), db(2), domain(4) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | inclusion | anchors | status/出链 | summary |
|---|---|---|---|---|---|---|---|
| arch/stream-buf-unified-cap.md#关联 | arch | 关联 | - | always | - | active / →hot-path-buffers,sse-chunk-stateless-defect | [[sse-chunk-stateless-defect]] 阐述流缓冲架构，[[hot-path-buffers]] … |
| arch/stream-buf-unified-cap.md#关联 | arch | 关联 | - | always | - | active / →hot-path-buffers,sse-chunk-stateless-defect | [[sse-chunk-stateless-defect]] （流缓冲架构） · [[hot-path-buffers]… |
| arch/stream-buf-unified-cap.md#案例 | arch | 案例 | - | always | - | active | **正例** —— usage 侧定义 `SSE_LINE_BUF_MAX_BYTES = 1MB`，内容侧转换分支引用… |
| arch/stream-buf-unified-cap.md#案例 | arch | 案例 | - | always | - | active | **正例** —— usage 侧定义 `SSE_LINE_BUF_MAX_BYTES = 1MB`，内容侧转换分支引用… |
| arch/stream-buf-unified-cap.md#流缓冲上界单一真值源原则 | arch | 流缓冲上界单一真值源原则 | - | always | - | active | - |
| arch/stream-buf-unified-cap.md#流缓冲上界单一真值源原则 | arch | 流缓冲上界单一真值源原则 | - | always | - | active | - |
| arch/stream-buf-unified-cap.md#硬约束 | arch | 硬约束 | - | always | - | active | 同一隐患在代码库内多条执行路径出现时，上界值**禁止多处定义**。一处常量定义，其余路径引用 → 防止行为割裂。  ##… |
| arch/stream-buf-unified-cap.md#硬约束 | arch | 硬约束 | - | always | - | active | 同一隐患在代码库内多条执行路径出现时，上界值**禁止多处定义**。一处常量定义，其余路径引用 → 防止行为割裂。  ##… |
| arch/stream-buf-unified-cap.md#适用 | arch | 适用 | - | always | - | active | - 任何多路径并发处理同一数据流的缓冲 - 多个解析器共用一个上界（如 SSE / WebSocket 等流协议） - … |
| arch/stream-buf-unified-cap.md#适用 | arch | 适用 | - | always | - | active | - 任何多路径并发处理同一数据流的缓冲 - 多个解析器共用一个上界（如 SSE / WebSocket 等流协议） - … |
| arch/stream-buffer-cap-single-source.md#流缓冲上界单一真值源 | arch | 流缓冲上界单一真值源 | - | always | - | active | - |
| db/sqlite-read-cache-config.md#SQLite 只读缓存定值 | db | SQLite 只读缓存定值 | sqlite,cache,readonly,memory,hardcoded | always | - | active | 通过 `PRAGMA cache_size = -64` 限制每条只读连接的页缓存驻留，实测指标达标。  ### 硬约束… |
| db/sqlite-read-cache-config.md#SQLite 只读缓存配置硬约束 | db | SQLite 只读缓存配置硬约束 | sqlite,cache,readonly,memory,hardcoded | always | - | active | - |
| domain/rule-67.md#关联 | domain | 关联 | - | auto | - | active / →rule-66,time-tiers-apply-idiom | [[rule-66]] [[time-tiers-apply-idiom]] |
| domain/rule-67.md#案例 | domain | 案例 | - | auto | - | active | 原错 → estimate 的两处取价未乘 peak_hours·multiplier，而 calc_est_cost … |
| domain/rule-67.md#硬约束 | domain | 硬约束 | - | auto | - | active | estimate 流程中**任一分支加 peak 倍率，对边必补**（既存 bug 根因）：  - `estimate/… |
| domain/rule-67.md#禁用 | domain | 禁用 | - | auto | - | active | ❌ 仅余额扣减乘倍率，手动预算不乘（口径分裂：扣数 ≠ 前端显示） ❌ 仅某一段乘倍率，其他相关路径不补（隐性 bug，… |
