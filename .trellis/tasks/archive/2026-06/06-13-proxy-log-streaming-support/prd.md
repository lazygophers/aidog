# PRD: 请求日志兼容流式(SSE)请求

## 背景

aidog proxy (src-tauri/src/gateway/proxy.rs) 渐进式日志对流式(SSE / streaming)请求记录不完整。研究 (research/01-04) 确认缺口:

- **响应 body 100% 缺失**(设计如此): 流式 `response_body` / `user_response_body` 恒为 `"[stream]"` (proxy.rs:1025-26), SSE 内容全丢; 前端显示「(流式响应, 内容未记录)」(Logs.tsx:229)。
- **token / est_cost / duration 条件性缺失**: 依赖上游发 usage chunk + `data: [DONE]` 触发回写 + 客户端不断连; 任一不满足则统计丢失。
- **主转换分支漏读 Anthropic usage** (proxy.rs:982-996): 只读顶层 usage, 未读 `message.usage`, input_tokens 丢。
- **透传(ClaudeCode)流式 token 恒 0** (proxy.rs:1321-1359): 无 `[DONE]` 回写机制。
- **无 is_stream 标记列**: ProxyLog 27 字段靠 `"[stream]"` 哨兵区分流式。
- `parse_anthropic_sse` / `parse_openai_sse` 均不解析 usage; token 累计绕过 parse_sse 直读裸 JSON。

## 目标

请求日志对流式请求与非流式记录一致完整: 响应内容、token/费用统计、断连也不丢数据。

## 决策 (已确认)

1. **修复范围**: 全补 — 响应 body 内容 + token/费用统计 + Anthropic message.usage 修复 + 透传流式 token。
2. **body 记录策略**: 完整记录, 不截断(用户明确选"完整不限")。
3. **is_stream 列**: 新增, 精确标记 + 前端区分。
4. **断连 Drop 兑底**: 做 — 断连(`[DONE]` 未触发)时也回写已聚合的 token/body。

## 实施改造点 (research/04)

### A. 流式响应 body 聚合 (proxy.rs:925-1019 主转换分支)
- 旁路 `Arc<Mutex<...>>` 累积每个 SSE chunk 原文。
- **禁在 stream 闭包内 await / 持锁跨 await** (超时级联 + 阻塞转发)。
- `[DONE]` 回写时 (proxy.rs:946-963) 写入聚合 body 到 `response_body` / `user_response_body`(受 ProxyLogSettings master/user_request/upstream 开关 + retention 控制)。
- 完整不限: 但需注意超长流内存 — 实现时用 `Vec<Bytes>` 累积, 回写时拼接。

### B. token 累计补 message.usage (proxy.rs:982-996)
- 补 Anthropic `message.usage` 兜底(复用已有 `accumulate_sse_usage` proxy.rs:1396)。

### C. 透传流式 [DONE] 回写 (proxy.rs:1321-1359)
- 透传分支引入 `[DONE]` 回写机制, 聚合 token + body。

### D. is_stream 列 (schema)
- ProxyLog 加 `is_stream` 列: db.rs schema migration + `upsert_proxy_log` (db.rs:1034) + 写日志 fn `upsert_log` (proxy.rs:305) + models.rs ProxyLog struct + TS 类型 + 前端 Logs.tsx 区分展示。
- 加列需同步 4 处 (research/03)。

### E. Drop 兑底
- 流式响应 future 被 Drop(客户端断连, `[DONE]` 未达)时, 回写已聚合的 token/body。当前无 Drop 兑底 — 需引入 guard(如 `scopeguard` 或自定义 Drop struct)在闭包结束/drop 时 flush。
- **关键**: Drop 内不能 async; 用 channel / 同步落盘或 spawn 落库。

## 存储确认 (SQLite 字段类型)

已核 `migrations/001_init.sql:77-86`: 全部 body 字段 (`request_body` / `upstream_request_body` / `response_body` / `user_response_body`) 均为 `TEXT NOT NULL DEFAULT ''`。
- SQLite TEXT 无 VARCHAR 式长度限制, 单值上限 = `SQLITE_MAX_LENGTH` 默认 ~1GB (10^9 字节)。完整流式聚合 body (MB 级) 远在限内, **无需改字段类型**。
- WAL 已启用 (db.rs:70), 利于大写入。
- 实施注意: ① 巨大 TEXT 行避免无谓 `SELECT *` (Logs 列表查询应排除 body 列, 详情才取 body); ② body 超 SQLite 单值上限 (~1GB) 时 graceful 处理 (截到上限 + 标记, 禁 panic), 即便用户要"完整不限"也要防物理上限崩溃。

## 范围

- 后端: proxy.rs (流式分支 A/B/C/E) + db.rs (is_stream schema + upsert) + models.rs (ProxyLog 字段) + converter.rs (SSE 解析若需)
- 前端: src/services/api.ts (ProxyLog 类型加 is_stream) + src/pages/Logs.tsx (流式标记展示 + body 不再恒 "内容未记录")
- i18n: 若新文案走 7 语言

## 非目标

- 不改非流式日志路径
- 不改 ProxyLogSettings 现有 3 级开关 / retention 语义(流式 body 仍受其控制)
- 不改超时级联 / 转发性能(聚合必须零阻塞旁路)

## 验收标准

- 流式请求日志 `response_body` 记录真实 SSE 聚合内容(非 "[stream]"), 受 log settings 开关控制
- 流式 token / est_cost / duration 正确(含 Anthropic message.usage + 透传分支)
- 客户端断连(无 `[DONE]`)时已聚合数据不丢(Drop 兑底)
- `is_stream` 列正确标记流式记录, 前端可区分
- 聚合零阻塞: 不在 stream 闭包内 await / 持锁跨 await, 不破坏转发与超时级联
- cargo build + yarn tsc 0 error 无新增 warning; 现有 test 通过
- Rust↔TS 字段契约一致

## 编排

单一交付(流式日志兼容), 单 worktree。改动集中 proxy.rs + db.rs + 前端 Logs, 强耦合(schema↔写入↔展示), 不拆 child。

**依赖**: 与颜色 task 同改 proxy.rs(颜色改 group-info handler, 本 task 改流式响应分支 + 写日志), 区域不重叠但**必须等颜色 task commit/merge 后再 start 本 task**, 避免 worktree 基线冲突。
