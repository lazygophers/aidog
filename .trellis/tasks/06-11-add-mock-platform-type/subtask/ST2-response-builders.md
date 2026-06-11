# ST2: adapter/mock.rs 响应 builder

- **目标**: 5 协议非流式 JSON builder + 流式 SSE 序列生成
- **产出** (adapter/mock.rs):
  - 非流式 builder：按 source_protocol 造完整 JSON body（anthropic/openai/openai_completions/openai_responses/gemini），shape 见 design 表；字段命名对照各 adapter 源码确认（openai_responses.rs / openai_completions.rs / gemini.rs）
  - 流式：自造 `ChatStreamEvent` 序列 `Start{id,model}`→N×`Delta{text}`(response_text 按 chunk_count 切)→`Stop{finish_reason}`，交 `adapter::to_client_sse(event, source_protocol, model)` 转协议 SSE
  - 假 usage 填充辅助（input/output/cache tokens 注入各协议 usage 字段）
- **验证**: cargo build 0；非流式 builder 单测 shape 正确
- **资源**: design.md（5 协议 shape）、converter.rs:72 to_client_sse、types.rs:130 ChatStreamEvent、各 adapter
- **依赖**: ST1
- **失败处理**: openai_responses/completions 字段不确定 → 读源码对照，勿臆造
