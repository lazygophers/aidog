# ST3: proxy.rs 拦截分支

- **目标**: 在 proxy.rs:340 后拦截 mock 平台，整合配置/延迟/错误/响应/日志
- **产出** (proxy.rs):
  - `if matches!(route.platform.platform_type, Protocol::Mock)` 分支（convert_request 后、send 前）
  - 调 `resolve_mock_config`（三层覆盖，传 extra + chat_req + 原始 body json）
  - `delay_ms>0` → `tokio::time::sleep`
  - error_mode 分派：none→正常 / http_error→status_code+错误body / rate_limit_429→429+retry-after / timeout→sleep 上限后返 504（不真 hang）
  - stream（stream_override 优先于请求 is_stream）→ SSE body（ST2 序列 + Body::from_stream + 响应头照抄 proxy.rs:543-551）；非流式→ ST2 JSON builder
  - 填假 token（最终生效值）+ status_code/duration_ms/response_body/actual_model 到 log，`upsert_log` 后 return Response，**跳过真实 reqwest**
- **验证**: cargo build 0
- **资源**: design.md、research 拦截点(proxy.rs:340)、proxy.rs upsert_log/响应头
- **依赖**: ST2
- **失败处理**: 上下文变量缺失回 research 对照行号
