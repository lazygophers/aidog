# Design: mock 平台类型

## 架构概览
路由到 `platform_type == Mock` 的平台时，在 `proxy.rs:340`（convert_request 后、send 前）拦截，跳过真实上游，本地按入站协议（source_protocol）生成可控假响应（非流式 JSON / 流式 SSE），填假 token 进 proxy_log。

拦截点已具备上下文：`source_protocol`(proxy.rs:261) / `is_stream`(287) / `requested_model`(288) / `route`(305) / `chat_req`(已解析)。

## Protocol::Mock
- models.rs Protocol enum 加 `#[serde(rename = "mock")] Mock,`（平台类型区）
- `default_for_protocol` / `ClientType` 无需改（`_` 兜底）
- 前端 api.ts Protocol union 加 `| "mock"`；Platforms.tsx PROTOCOLS 加项；ENDPOINT_PROTOCOLS 不加

## mock 配置三层覆盖（用户需求 1+2+3）

最终生效值 = 逐字段按优先级取首个存在者：

| 优先级 | 来源 | 格式 |
| --- | --- | --- |
| 1（最高） | 请求 body 顶层 `mock` 对象 | `{"mock":{"input_tokens":100,"status_code":429,...}}` 混入标准请求体 |
| 2 | 请求 messages 的 role 映射 | message `{"role":"<field>","content":"<value>"}`，field ∈ {input_tokens/output_tokens/cache_tokens/status_code/delay_ms/response_text/error_mode}（用户「根据 role 判断」原意：role 当 key，content 当 value） |
| 3（兜底） | platform.extra JSON 的 `mock` 对象 | 见下 schema |

解析顺序：先取 extra 默认 → message role 覆盖 → body.mock 覆盖。每字段独立覆盖（缺省回退下层）。

### platform.extra schema
```json
{
  "mock": {
    "status_code": 200,
    "delay_ms": 0,
    "stream_override": null,        // null=跟随请求 stream；true/false=强制
    "response_text": "Hello from mock",
    "finish_reason": "end_turn",
    "input_tokens": 100,
    "output_tokens": 50,
    "cache_tokens": 0,
    "error_mode": "none",          // none | http_error | rate_limit_429 | timeout
    "chunk_count": 5               // 流式时 response_text 切 N 块
  }
}
```
- mock 配置 struct `MockConfig`（adapter/mock.rs）`#[serde(default)]` 全字段，从 extra 的 `.mock` 反序列化；空 extra → 全默认

## 超时/错误语义（两者都要）
- `delay_ms > 0`：`tokio::time::sleep(delay_ms)` 后正常返回（真延迟）
- `error_mode`:
  - `none` → 正常 mock 响应
  - `http_error` → 返 `status_code`（如 500）+ 错误 body（按协议错误格式或纯文本）
  - `rate_limit_429` → 返 429 + retry-after 头
  - `timeout` → `sleep` 一段**超长**（如 600s，或取请求超时+5s）后返 504，**不真 hang 连接**（用 sleep 上限保护）
- `status_code` 独立可控（即便 error_mode=none 也可返非 200）

## 响应 builder（adapter/mock.rs 新建）

按 source_protocol 分派。**非流式**新建 5 协议 JSON builder：

| source_protocol | 非流式 shape 要点 |
| --- | --- |
| anthropic | `{id,type:"message",role:"assistant",model,content:[{type:"text",text}],stop_reason,usage:{input_tokens,output_tokens,cache_read_input_tokens}}` |
| openai | `{id,object:"chat.completion",model,choices:[{index:0,message:{role:"assistant",content},finish_reason}],usage:{prompt_tokens,completion_tokens,total_tokens}}` |
| openai_completions | `{id,object:"text_completion",model,choices:[{text,index:0,finish_reason}],usage:{...}}`（参照 adapter/openai_completions.rs 字段） |
| openai_responses | `{id,object:"response",model,output:[{type:"message",content:[{type:"output_text",text}]}],usage:{input_tokens,output_tokens}}`（参照 adapter/openai_responses.rs） |
| gemini | `{candidates:[{content:{parts:[{text}],role:"model"},finishReason}],usageMetadata:{promptTokenCount,candidatesTokenCount,totalTokenCount}}` |

实施时对照各 adapter 现有 request/SSE 结构确认字段命名（openai_responses/completions 字段以源码为准）。

**流式 SSE**：复用 `adapter::to_client_sse(event, source_protocol, model)`（converter.rs:72），mock 自造 `ChatStreamEvent` 序列：`Start{id,model}` → N×`Delta{text}`（response_text 按 chunk_count 切）→ `Usage{...}`（若 to_client_sse 支持）→ `Stop{finish_reason}`。响应头三件套照抄 proxy.rs:543-551。body 用 `axum::body::Body::from_stream` + 可选 `tokio::time::sleep` 逐块。

## proxy_log
mock 分支直接写 `log.input_tokens/output_tokens/cache_tokens`（最终生效值）、`log.status_code`、`log.duration_ms`、`log.response_body`/`user_response_body`（mock body）、`log.platform_id`、`log.actual_model`，复用 `upsert_log`。不走 `extract_usage`。

## 前端（Platforms.tsx）
- Protocol union + PROTOCOLS 加 mock；`getDefaultEndpoints` mock 返空
- `platform_type === "mock"` 时：base_url/api_key 可空（去必填校验）、隐藏 endpoints 编辑、显示 **mock 配置编辑器**（表单写 platform.extra 的 mock 对象：status_code/delay_ms/stream_override/response_text/finish_reason/3 个 token/error_mode/chunk_count）
- extra round-trip：读现有 extra JSON、编辑 mock 子对象、保存回 extra

## 不改
- router.rs（不校验 base_url/api_key，对 mock 透明）
- db.rs（extra 列已存在，enum 自动序列化，无 schema 变更）

## 验证
- cargo build + tsc 0
- 单测：5 协议非流式 builder shape、SSE 序列、三层覆盖优先级、error_mode 各分支、假 token 填充
- 手测：建 mock 平台 + 配 extra → 各 source_protocol group 路由返对应格式
