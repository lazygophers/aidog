# Research: 修复改造点清单 + 方案 + 风险

- **Query**: 让流式请求日志完整需改哪些函数 + 候选方案 + 风险
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### 缺口归纳（详见 02）

1. 流式 `response_body` / `user_response_body` 永远 `"[stream]"`，SSE 内容丢弃。
2. 主流式分支 token 累计只读顶层 `usage`，漏 Anthropic `message.usage`（input_tokens）。
3. `[DONE]` 回写依赖客户端不断连 + 上游发 `[DONE]`，否则 token/est_cost/duration 缺。
4. 透传（ClaudeCode）流式无 `[DONE]` 回写，token 几乎总为 0。

### 改造点清单（按缺口）

| # | 函数 / 位置 | 改造 |
|---|---|---|
| A | `proxy.rs:925-1019` 流式闭包 | 在闭包内把 SSE chunk（原始或转换后）累计到一个 `Arc<Mutex<String>>` / `Arc<Mutex<Vec<u8>>>`；`[DONE]` 回写时（`:946-963`）写入 `final_log.response_body` / `user_response_body` |
| B | `proxy.rs:982-996` token 累计 | 补 `message.usage` 兜底路径（复用 `accumulate_sse_usage` `proxy.rs:1396`，已有完整路径）— 统一两分支 usage 逻辑，消除主分支漏读 |
| C | `proxy.rs:1321-1359` 透传流式 | 引入与主分支同款 `[DONE]` 回写（fire-and-forget spawn upsert），把累计 token + body 写回；当前 `:1356-1358` 在返回前一次性 upsert，token=0 |
| D | `proxy.rs:1029-1031` / `:1356-1358` 返回前 upsert | 流式返回前的 upsert 可标记「流进行中」（如保留 `[stream]` 占位 + 新增 `is_stream` 列），最终态由 `[DONE]` 回写覆盖 |
| E（可选）| `models.rs:653` + `db.rs:996/1001/1039` + `migrations/00X.sql` | 新增 `is_stream INTEGER DEFAULT 0` 列，让日志可显式区分流式（替代 `response_body=="[stream]"` 哨兵），需同步 4 处（DDL/migration、PROXY_LOG_COLUMNS、row_to_proxy_log、upsert VALUES+params）|
| F（前端）| `src/pages/Logs.tsx:123,138,228-238` | 若 A 落地（body 已记录），移除「(流式响应，内容未记录)」占位，正常展示聚合内容；或保留占位作为 fallback |

### 候选方案

- **方案 1（最小）**：仅做 B + C — 修 token 准确性，body 仍不记录。风险低，不改 schema。
- **方案 2（完整 body）**：A + B + C — 聚合 SSE 内容写 response_body。需注意聚合内存与 [DONE] 触发可靠性。
- **方案 3（含 schema）**：方案 2 + E（is_stream 列）+ F — 可查询/筛选流式日志，前端正常展示。改动面最大。

### 风险

1. **聚合不能阻塞转发**：闭包是 `bytes_stream().map`，每 chunk 必须立即返回给客户端（`Body::from_stream` `proxy.rs:1021`）。累计须用 `Arc<Mutex<..>>` 旁路写入，**禁在闭包内 await / 持锁跨 await**（违反 CLAUDE.md「Db 内部 Mutex，禁持锁跨 await」同理）。
2. **内存**：长流式响应（大量 token）全量聚合 body 进内存 + 入库，单条日志可能 MB 级。建议设上限截断（参考请求体 `to_bytes(10MB)` `proxy.rs:530`）或受 `ProxyLogSettings` 开关控制。
3. **[DONE] 不触发**：客户端断连 / 上游不发 `[DONE]` / 上游用纯 chunked 非标准 SSE → 回写丢失。需考虑 stream Drop 时兜底回写（当前无 Drop guard）。
4. **超时级联**：流式聚合不应改变现有 `resolve_timeout`（`proxy.rs:416`）行为；body 累计是被动旁路，不引入新等待。
5. **est_cost 一致性**：est_cost 在 `upsert_log`（`proxy.rs:323`）按 token 算，token 准了 est_cost 自动准；须确保 `[DONE]` 回写的 `final_log` 走完整 `upsert_log` 路径（当前 `:962` 已走）。
6. **另有 task 正在改 proxy.rs**（group-info handler 颜色相关），改流式分支（`:881-1044`）与 group-info（`:119-290`）不重叠，但提交前需 rebase 确认无冲突。

## Caveats / Not Found

- 是否新增 `is_stream` 列属产品决策（影响 schema migration + 前端），本调查列为可选项 E，未定论。
- Drop-time 兜底回写当前代码无先例，需新增机制（如 `futures` 流包装 + Drop impl），实现复杂度中等。
