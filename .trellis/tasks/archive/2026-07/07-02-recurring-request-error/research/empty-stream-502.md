# Research: GLM 空流 502（request_id=d3a0ce30 + 1301 条同类）

- **Query**: 定位 d3a0ce30 错误根因 + 历史量化
- **Scope**: internal（DB 取证 + 代码读）
- **Date**: 2026-07-02
- **只读诊断，未改码**

---

## 1. d3a0ce30 完整字段（实测 DB）

| 字段 | 值 |
|---|---|
| status_code / upstream_status_code | **502 / 200** |
| is_stream | **1**（客户端正常发了 stream=true） |
| created_at | 1782984371891（2026-07-02 17:26:11） |
| group_key / model / actual_model | glm / claude-opus-4-8 / glm-5.2 |
| platform_id | 38（GLM-自用，**唯一候选**） |
| retry_count / attempts | 0 / 单条 [{platform 38, 200, "200 but empty/invalid stream", 895ms}] |
| request_body / upstream_request_body len | 207084 / 229019 |
| response_body | 28 字节占位 `"200 but empty/invalid stream"` |
| duration_ms | 1014 |

upstream_response_headers：`content-type: text/event-stream; charset=utf-8` + `transfer-encoding: chunked` + `x-log-id` —— **上游声明 SSE 流式，但流无内容**。

## 2. 时间窗（±10s）

| rid | status | ts | resp |
|---|---|---|---|
| 0d88add1 | 502 | 17:26:02 | empty/invalid stream |
| d3a0ce30 | 502 | 17:26:11 | empty/invalid stream |

间隔 9s = 客户端重试一次（同 payload，新 request_id），两次都空流 502。

## 3. 历史量化（"反复多次"）

| 范围 | 502 总数 | 空流 502（"empty/invalid stream"）|
|---|---|---|
| 全库 | 2148 | **1301** |
| glm 组近 2 天 | 109 | 主要构成 |

glm 组近 2 天错误码：502(109) / 429(16) / 400(16) / 499(6)。502 是高频主体。

1301 条空流 502 的 response_body **全是 28 字节占位**（min=max=28）—— peek_buf 未持久化，上游 GLM 真实首块内容无 DB 证据。

## 4. 根因（代码证据）

### proxy 处理正确（非 bug）

流式场景 proxy 先 peek 上游首块再决定（`forward.rs:456-497`）：
- `StreamPeek::EmptyOrError`（立即[DONE]/立即error/秒断/空body/流结束无内容）→ retry/failover
- `StreamPeek::Meaningful`（真实内容事件）→ 提交转发

`classify_stream_first`（`retry.rs:150`）：扫描 SSE 原文，遇 `event: error` / 顶层 `error` / `[DONE]` 前置 / 流结束无内容 → EmptyOrError。

d3a0ce30 走 `stream_ended=true → EmptyOrError`（上游流结束仍无有效内容事件）。**proxy 判定正确**，failover 触发，但 platform 38 是 glm 组唯一候选 → 无下家 → 502 直返客户端。

### 真根因：GLM 侧间歇空流

GLM 返 200 + SSE content-type + chunked，但流无内容即结束。这是 GLM 端间歇性行为（可能是限流/过载的另一种表现 —— 返 200 占位而非 429）。**proxy 无法控上游**。

## 5. 已排除（带证据）

- ❌ proxy peek 逻辑错：classifier 逻辑正确，覆盖 anthropic/openai/透传三类
- ❌ 大小超限：req 207K 在成功范围（成功 body 最大 509K）
- ❌ 超时：duration 895ms/1014ms，远未到超时阈值
- ❌ 连接错误：upstream_status=200，非 "error sending request" 类（其他 502 样本是连接错，本类不是）

## 6. 取证盲区 + 后续

- **peek_buf 未落库** → 不知 GLM 真实首块（真空？keepalive 注释行后断？无法解析残帧？）
- 需改码补诊断日志（peek 判空时把 peek_text 落 response_body），下次复现取证
- 取证后再判：若 GLM 真空 → 架构层（加候选/降级）；若 proxy 误判 → classifier 修正

## 7. 与 cb3603ac（hoist）的关系

两类独立根因，同 group/model/session 的间歇失败：
- cb3603ac = proxy bug（漏 stream → hoist 误触 → GLM 1210）—— **可修**
- d3a0ce30 = GLM 间歇空流 + 单候选架构 —— **proxy 处理正确，修法待取证**

## Files Found

| File | 证据 |
|---|---|
| `src-tauri/src/gateway/proxy/forward.rs:456-497` | 流式 peek 兜底逻辑 |
| `src-tauri/src/gateway/proxy/forward.rs:498` | `retry_on_empty_2xx!("200 but empty/invalid stream")` |
| `src-tauri/src/gateway/proxy/retry.rs:150` | `classify_stream_first` 实现 |
| DB `/Users/luoxin/.aidog/aidog.db` | 1301 条空流 502，d3a0ce30 attempts 详情 |
