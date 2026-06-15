# 代理支持 /v1/models 模型列表端点

## 背景（已诊断，request_id 22d5d4efdac1426abc9f737bea3f0efe）
客户端 `GET /proxy/v1/models`（group=glm-coding-plan-auto）返回 **400** `parse request json error: EOF while parsing a value at line 1 column 0`。
根因：proxy.rs:822 对所有非拦截请求强制走 `parse_incoming_request`（chat 解析），`/v1/models` 是 GET 空 body → EOF 400。代理**未实现模型列表端点**。

## 需求
代理支持模型列表端点（至少 `/v1/models`），转发到分组所选平台上游并 relay 其模型列表，不再 400。

## 关键约束 / 设计
- **在 `parse_incoming_request`（proxy.rs:822）之前**分流：识别「模型列表端点」→ 走专用 models 处理，不进 chat 解析。
  - 识别：strip group/proxy 前缀后 api_path == `/v1/models` 或 `/models`（openai/anthropic 同名）；gemini `/v1beta/models` 可选。GET 方法。
- **平台选择**：复用现有路由/分组逻辑选一个**启用平台**（first enabled，或现成 route 选择函数；模型列表不需要 model mapping/重试链，取第一个可用平台即可）。group 已 resolve（glm-coding-plan-auto 能命中）。
- **上游 URL 构造**（关键，遵 url-construction-rule）：`platform.base_url`（含版本前缀，如 glm `.../api/paas/v4`、openai `https://api.openai.com/v1`）trim 尾 `/` + `/models`。**不要用 `build_passthrough_url`**（它拼客户端完整 path 会错）。
- **鉴权**：注入**平台凭证** `platform.api_key`（多数 OpenAI 兼容平台 `Authorization: Bearer <key>`；anthropic 平台用 `x-api-key`）。**不要透传客户端的 group token**（上游不认）。鉴权头风格参考 platform_fetch_models（lib.rs:689 起，已有按协议取 models + 鉴权的逻辑，可复用/参照）。
- **响应**：relay 上游 status + body（JSON 模型列表）回客户端；记录 ProxyLog（status_code/upstream_status_code/upstream_request_url）。失败（无可用平台/上游错误）返回明确错误（非 EOF 400）。
- **优先复用**：`platform_fetch_models` 命令（lib.rs:689）已实现「按协议拉上游 /models + 鉴权」——可抽共享函数或参照其 URL/鉴权构造，避免重复腐化。

## 范围
- 主改 `src-tauri/src/gateway/proxy.rs`（handle_proxy 在 822 前加 models 分流 + 新 handler `handle_models_passthrough` 或复用）。可能touch `lib.rs`（抽 platform_fetch_models 的 URL/鉴权为共享 helper）+ adapter（provider_api_path 类比，models path）。
- 不破坏现有 chat / 同协议透传 / claude-code intercept / Codex。

## 验收
- `cd src-tauri && cargo build && cargo clippy --quiet`（零项目 warning）+ `cargo test`（新增 models 端点分流/URL 构造单测；不破坏现有）。
- 行为：`GET /proxy/v1/models`（任意有效 group）→ 200 + 上游模型列表 JSON（或上游真实错误码），**不再 400 EOF**。日志 status/upstream_url 正确。
- openai 兼容平台（glm 用 openai 协议端点）验证；anthropic 平台 x-api-key 路径正确。
- 不影响 chat/completions、/v1/messages、/v1/responses、claude-code 透传。

## 失败处理
- 分组无启用平台 → 返回明确 4xx（如 503/424 + 说明），非 EOF。
- 平台 base_url 版本前缀差异（/v1 vs /api/paas/v4）→ 统一 base_url trim + "/models"（base_url 已含前缀，禁额外拼版本）。
- gemini models 路径不同（/v1beta/models）→ 本期可只保证 openai/anthropic，gemini 标 TODO 或一并处理，回报取舍。
- 鉴权风格按平台协议分流（Bearer vs x-api-key），参照 platform_fetch_models。
- 门禁红修到绿；卡住标 `需要:`。
