# ST2: proxy.rs 纯透传

- **目标**: CC 平台原样 relay 客户端请求到 base_url
- **产出** (proxy.rs):
  - `req.into_parts()`(:201) **之前**捕获 `orig_method = req.method().clone()` / `orig_uri = req.uri().clone()` / `orig_headers = req.headers().clone()`（含真实 Authorization）
  - `select_platform`(:305) 后、`convert_request`(:359) 前加 `if matches!(route.platform.platform_type, Protocol::ClaudeCode) { return handle_passthrough(...).await; }`
  - `handle_passthrough`（新函数）：目标 URL=base_url+orig path(+query)；reqwest orig_method + body=bytes；header 原样转发 orig_headers，**剔除 Host + Content-Length**（reqwest 重设），其余原样（含 Authorization）；超时复用现有设置；响应原样 relay（非流式 resp.bytes() / 流式 resp.bytes_stream() 透传，响应头/status 照搬上游）；proxy_log 记 source=target_protocol="claude_code" + upstream_url/status/response_body/headers + token（extract_usage 尽力解析 anthropic usage，流式 SSE usage）+ actual_model=log.model + platform_id；upsert_log 后 return
  - 单测：URL 拼接、header 剔除 Host/Content-Length 保留 Authorization、log 字段、透传分支不调 convert_request
- **验证**: cargo build + cargo test 0
- **资源**: design.md、proxy.rs（:177 headers/:201 into_parts/:341 mock 参照/:387 build_upstream_headers/extract_usage/流式 :560+）
- **依赖**: ST1
- **失败处理**: 流式透传复杂 → 参照现有流式 :457-552 的 bytes_stream 处理；卡 3 次以上停止报告
