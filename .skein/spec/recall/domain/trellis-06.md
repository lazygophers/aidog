---
title: mock 平台类型规范
layer: recall
category: domain
keywords: [mock,platform,extra,test,builder,error_mode]
source: trellis
authored-by: skein-memory
created: 1783832114
---

# Mock Platform Type

何时被读: 改 mock 平台逻辑（adapter/mock.rs / proxy.rs handle_mock）/ 新增 mock 可控字段 / 加入站协议 / 调错误模拟语义时
谁读: trellis-implement sub-agent / main
不遵守的代价: mock 链路与真实代理链路行为漂移 / 假 token 记错 / 协议响应 shape 不符客户端预期 / 真 hang 连接

---

## What & When (MUST)

- `Protocol::Mock`（`models.rs`，serde rename `"mock"`）是**平台主类型**，不是 endpoint 协议；禁加入 `ENDPOINT_PROTOCOLS`
- 路由到 mock 平台时**禁转发真实上游**：`proxy.rs` 在 `convert_request` 之后、构建 reqwest client 之前（`matches!(route.platform.platform_type, Protocol::Mock)`）拦截，走 `handle_mock` 本地生成假响应
- mock 平台 `base_url` / `api_key` 可空；前端 `platform_type === "mock"` 时去必填校验、隐藏 endpoints 编辑、显示 mock 配置编辑器（写 `platform.extra` 的 `mock` 子对象）
- 假响应必须按**入站协议**（`source_protocol`，来自 `group.source_protocol`）返对应格式，非按平台/endpoint 协议

## Config Carrier — extra.mock (MUST)

- mock 配置载体必须为现有 `platform.extra`（TEXT JSON 列），禁新增专用 DB 列（零迁移，复用 CRUD）
- extra schema（全字段 `#[serde(default)]`，空 extra → 全默认）：

```json
{
  "mock": {
    "status_code": 200,
    "delay_ms": 0,
    "stream_override": null,
    "response_text": "Hello from mock",
    "finish_reason": "end_turn",
    "input_tokens": 100,
    "output_tokens": 50,
    "cache_tokens": 0,
    "error_mode": "none",
    "chunk_count": 5
  }
}
```

- `stream_override`: `null`=跟随请求 `stream`；`true`/`false`=强制覆盖
- `error_mode` ∈ `none | http_error | rate_limit_429 | timeout`
- `chunk_count`: 流式时把 `response_text` 切 N 块（超文本字符数时按字符数封顶；`<=1` 或空文本 → 单块）

## Three-Layer Config Override (MUST)

最终生效值 = 逐字段按优先级取首个存在者（`resolve_mock_config(extra, chat_req, body_json)`）：

| 优先级 | 来源 | 格式 |
| --- | --- | --- |
| 1（最高） | 请求 body 顶层 `mock` 对象 | `{"mock":{"input_tokens":100,...}}` 混入标准请求体 |
| 2 | 请求 messages 的 role 映射 | message `{"role":"<field>","content":"<value>"}`，role 当 key、content 当 value |
| 3（兜底） | `platform.extra` 的 `mock` 对象 | 见上 schema |

- 解析顺序必须：先取 extra 默认 → message role 覆盖 → body.mock 覆盖。**每字段独立覆盖**，缺省回退下层
- message role 映射可识别字段名: `input_tokens` / `output_tokens` / `cache_tokens` / `status_code` / `delay_ms` / `response_text` / `error_mode`；标准 Role（user/assistant/system/tool）不匹配任何字段，不改配置
- message 层须扫**原始 body messages**（自定义 role 名经 `parse_incoming_request` 归一化会丢失，故直接从 body_json 再扫一遍）

## Response Builders (MUST)

- 非流式: `build_response(cfg, source_protocol, model)` 按 5 协议返完整 JSON，假 token 注入各协议 usage 字段；未知协议兜底 anthropic
- 流式: `build_sse_chunks(cfg, source_protocol, model)` 造 `ChatStreamEvent` 序列 `Start → N×Delta → Stop`，复用 `converter::to_client_sse` 转协议格式
- 错误: `build_error_body(source_protocol, status_code, message)` 按协议错误 shape

各协议非流式关键字段（断言依据，改 builder 须同步本表 + 测试）:

| source_protocol | 关键字段 |
| --- | --- |
| anthropic | `type:"message"` / `content[0].text` / `stop_reason` / `usage.{input_tokens,output_tokens,cache_read_input_tokens}` |
| openai | `object:"chat.completion"` / `choices[0].message.content` / `finish_reason`（`end_turn`→`stop`）/ `usage.{prompt_tokens,completion_tokens,total_tokens,prompt_tokens_details.cached_tokens}` |
| openai_completions | `object:"text_completion"` / `choices[0].text` / `usage.{prompt_tokens,completion_tokens,total_tokens}` |
| openai_responses | `object:"response"` / `status:"completed"` / `output[0].content[0].{type:"output_text",text}` / `usage.{input_tokens,output_tokens,total_tokens}` |
| gemini | `candidates[0].content.parts[0].text` / `finishReason`（STOP/MAX_TOKENS）/ `usageMetadata.{promptTokenCount,candidatesTokenCount,cachedContentTokenCount,totalTokenCount}` |

## error_mode Semantics (MUST)

`handle_mock`（proxy.rs）按 `error_mode` 分派，两类语义并存（delay 与 error 都生效）:

- `delay_ms > 0`: 进入分支前 `tokio::time::sleep(delay_ms)` 真延迟（流式时也每块延迟）
- `none`: 正常 mock 响应；`status_code` 独立可控（即便 200 外也可返）
- `http_error`: 返 `status_code` + 协议错误 body
- `rate_limit_429`: 返 429 + `retry-after` 头
- `timeout`: **禁真 hang 连接** —— 必须 `tokio::time::sleep` 上限保护（当前 600s）后返 504

## proxy_log (MUST)

- mock 分支直接写最终生效值 `log.{input_tokens,output_tokens,cache_tokens}`，**禁走 extract_usage**
- 同时填 `log.status_code` / `log.duration_ms` / `log.response_body` / `log.user_response_body` / `log.user_response_headers`，复用 `upsert_log`
- 流式 body_response 记 `"[mock stream]"` 占位

## Verification

```bash
cd src-tauri && cargo test mock   # 全部通过（三层覆盖 / 5 协议 builder / SSE / error_mode / 假 token）

# Mock 不入 endpoint 协议
grep -n "Mock" src/pages/Platforms.tsx   # PROTOCOLS 有 mock，ENDPOINT_PROTOCOLS 无
```
