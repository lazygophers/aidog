# 核查并支持 Responses API 全端点

## 需求
确保 OpenAI Responses API 全端点被代理正确支持（转发/转换到对应协议）：
- `POST /v1/responses`（create）— 已 work，不动。
- `GET /v1/responses/{id}`（retrieve）、`POST /v1/responses/{id}/cancel`、`DELETE /v1/responses/{id}`、`POST /v1/responses/compact`、`GET /v1/responses/{id}/input_items` — **当前 broken**，需修。

## 调研结论（research/ 1-3，已核 file:line）
- **create work**：平台有 openai_responses 端点 → same_protocol_passthrough（proxy.rs:982，不转换）；无则回退 openai 有损转换（既有设计）。**create 无需改**。
- **子端点全 broken**：`detect_source_protocol` `starts_with("/v1/responses")`（proxy.rs:2349）误吞所有子路径为 openai_responses → 走 chat parse → GET 空 body EOF 400 / cancel·compact body 非 responses 请求转换失败。
- **2 障碍**：① `passthrough_api_path`(converter.rs:54-62) 对 OpenAIResponses 硬编码返回 `/v1/responses`，丢子路径；② `build_passthrough_url`(proxy.rs:2011) 拼客户端完整 path（含 /proxy+/v1），responses 平台 base_url 已含 /v1 → 不能直接复用。
- 官方核实（WebSearch 2026-06）：retrieve(GET /v1/responses/{id}) / cancel(POST .../{id}/cancel) / delete(DELETE /v1/responses/{id}) / compact(POST /v1/responses/compact) / input_items(GET .../{id}/input_items) 均真实存在。
- DB 实查：当前只见 create（/v1/responses），子端点未出现（因 broken/未用）。

## 设计（handler 层加分流，detect + create 路径不动 → 回归面最小）
1. **新增 `is_responses_subendpoint(path)`**：strip 前缀后 api_path 以 `/v1/responses/`（注意**带尾斜杠+后续段**）开头 → true。**精确放行 create**：裸 `/v1/responses`（无尾段）= create → false，不拦。覆盖 compact(`/v1/responses/compact`)、{id}、{id}/cancel、{id}/input_items。
2. **分流点**：handle_proxy 在 **models 分流之后、parse_incoming_request 之前**加：`is_responses_subendpoint(&path)` → return `handle_responses_subendpoint(...)`。任意方法（GET/POST/DELETE）。
3. **handle_responses_subendpoint（透传，复用 models 模式但独立 fn）**：
   - **平台选择**：分组首个**支持 responses 的平台**——即有 endpoint protocol == OpenAIResponses 的平台；无则回退首个启用平台。
   - **上游 URL**（关键）：取该平台 responses 端点的 base_url；upstream URL = base_url + 子路径。**子路径** = api_path 去掉 `/v1` 前缀后的部分（如 `/responses/{id}/cancel`），拼到 base_url（base_url 已含版本前缀如 `/v1`），遵 url-construction-rule 禁重复拼版本。**参照 create 的 same_protocol_passthrough 实际怎么拼上游 URL**（proxy.rs:982 区）保持一致，仅替换 path 尾。**不要用 build_passthrough_url**。
   - **鉴权**：平台凭证（`Authorization: Bearer <api_key>` + `OpenAI-Beta: responses=experimental`，参照 proxy.rs:2533/2677）。不透传客户端 token。
   - **方法/body**：保留 orig method + 原样转发 body（GET/DELETE 无 body；POST cancel/compact 原样）。relay 上游 status+body+content-type。写 ProxyLog（platform_id/status/upstream_url/source=target=openai_responses）。
   - **错误**：无 responses 平台 → 明确 4xx（503+说明，非 EOF）；上游失败 → 502。
4. create 路径、detect_source_protocol、models 分流 **均不动**。

## 验收
- `cd src-tauri && cargo build && cargo clippy --quiet`（零项目 warning）+ `cargo test`（新增：is_responses_subendpoint 精确放行 create + 拦子端点 / URL 构造 / 鉴权；不破坏 create same_proto passthrough + models + Codex 回归）。
- 行为：
  - `POST /v1/responses` create 仍走原转换/同协议透传（**不被新分流拦**）—— 关键回归断言。
  - `GET /v1/responses/{id}` / `POST .../{id}/cancel` / `DELETE /v1/responses/{id}` / `POST /v1/responses/compact` / `GET .../{id}/input_items` → 透传到上游 responses 平台 + 平台凭证，**不再 400 EOF**；relay 上游真实 status/body。
  - 上游 URL 正确（base_url 含 /v1 时不重复拼）。
- 不影响 chat/completions、/v1/messages、/v1/models、claude-code 透传。

## 失败处理
- base_url 版本前缀 → 子路径去 /v1 再拼，禁重复版本；以 create same_proto passthrough 的 URL 构造为准镜像。
- 多 responses 平台分组 → response_id 无 platform 映射，取首个 responses 平台（单平台分组安全）；多平台上游可能 404，log 记录、不臆造映射。本期接受此限制并在回报标注。
- is_responses_subendpoint 误拦 create → 必须单测断言裸 `/v1/responses`(POST) = false。
- Codex 是否真发子端点未知 → 不影响：修复是「支持若发了不再 broken」。
- 门禁红修到绿；卡住标 `需要:`。
