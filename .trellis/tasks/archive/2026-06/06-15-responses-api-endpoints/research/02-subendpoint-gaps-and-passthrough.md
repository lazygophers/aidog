# Research: detect 前缀问题 + 子端点 passthrough 设计可行性

- **Query**: detect_source_protocol 前缀误吞；子端点 passthrough（平台选择/URL/auth）；create 是否应透传
- **Scope**: internal + 设计分析
- **Date**: 2026-06-15

## 1. detect_source_protocol 前缀问题

定义 `proxy.rs:2337-2366`：

```
api_path = path.find("/v1/").map(|i| &path[i..])   // 找不到 /v1/ 则:
   path.contains("/v1beta/") -> return gemini
   else -> return anthropic                          // 兜底
...
else if api_path.starts_with("/v1/responses") -> openai_responses
```

问题点：
- **`starts_with("/v1/responses")` 把 `/v1/responses/{id}`、`.../cancel`、`.../compact`（若拼在 responses 下）全判为 openai_responses。** 主请求与子端点共用一个协议分支，后续无差别走 chat parse → 子端点崩。
- **无 /v1 前缀的 `/responses` 不被识别**：strip 依赖 `path.find("/v1/")`。Codex 实际发 `/proxy/v1/responses`（含 /v1），所以无前缀情况当前不会发生（见下），但若将来客户端发 `/responses`，会落 else→anthropic 兜底（更错）。

### Codex 实际发什么路径（已确认）

`codex.rs:54-88 build_group_profile_toml`：
- `base_url = http://127.0.0.1:{port}/proxy`（codex.rs:68）
- `wire_api = "responses"`（codex.rs:72）

Codex CLI 在 `wire_api=responses` 下对 `base_url` 拼 `/responses`，并带 OpenAI Responses 版本路径。**推测**（Codex 行为，未在本仓代码内固定）：最终 path = `/proxy/v1/responses`（Codex 对 OpenAI-style base_url 会拼 `/v1/responses`；aidog 既有 openai_responses 入站修复也按 `/v1/responses` 设计，见 memory [[codex-config-subsystem]]「入站 /v1/responses 修复」）。
- create：`POST /proxy/v1/responses`
- 子端点 path（retrieve/cancel）：**推测** Codex 当前是否真的调用 `/responses/{id}` 子端点取决于 Codex 版本与功能（流式 create 可能不需要 retrieve；cancel 可能走连接中断而非 HTTP cancel）。`需要:` main agent 核 Codex 实际是否发这些子端点（抓本地代理日志 proxy_log 看真实 path 最可靠，见 03 验证建议）。

`需要:` 确认 Codex 在 aidog 代理下实际发出的 responses 子端点 path 全集（建议跑一次 codex 会话，查 `proxy_log.request_url` 里 `/responses` 后缀分布）。

## 2. 子端点应 passthrough（不转换）

retrieve/cancel/compact 是对**某次 create 产生的 upstream response 对象**的操作，语义上必须打到**同一个上游 responses 平台**，且 body（如有）/path 原样透传，不能经 chat 有损转换。这与 create 在「同协议」下应透传一致（[[protocol-same-proto-passthrough]]）。

### 难点①：平台选择（response_id 归属）

`response_id` 属于某次 create 的具体上游平台。aidog 多平台路由 + 失败重试（[[platform-retry-failover]]）下，create 可能落到候选里任意一个 responses 平台，aidog **不持久化 response_id→platform 映射**（DbCache/proxy_log 里没有这种索引；proxy_log 按 request id=trace_id 存，不按 response_id）。

现实方案（与 `handle_models_passthrough` 同款，proxy.rs:1936）：
- **取分组首个启用且 responses-capable 的平台**（endpoint 协议含 openai_responses，或平台 platform_type 走 responses/codex）。
- 风险：若分组有多个 responses 平台、create 落到非首个，子端点打到首个 → 上游 404（response_id 不属于该平台）。**单 responses 平台分组下无此问题（最常见，Codex 场景通常单平台）。**
- 备选（更重，暂不建议）：proxy_log 落 response_id→platform_id 映射，子端点查映射定位平台。

### 难点②：URL 构造（避免 /v1 重复）

遵 [[url-construction-rule]] + 参考 `build_passthrough_url`（proxy.rs:2011-2018）：
```
fn build_passthrough_url(base_url, uri) = base_url.trim_end_matches('/') + uri.path_and_query()
```
- 该 helper **直接拼客户端原始 path+query**。但客户端 path 含 `/proxy` 前缀 + 可能 `/v1`，而平台 `base_url` 已含版本前缀（如 `.../v1`）→ **直接用 build_passthrough_url 会路径错乱**（既有 ClaudeCode 透传场景里 base_url 是 host 根 `https://api.anthropic.com`，不含 /v1，所以 OK；responses 平台 base_url 含 /v1，不同）。
- 正确做法：**strip 客户端 path 的 `/proxy`+group 前缀和 `/v1` 前缀，取 responses 子路径（`/responses/{id}[/cancel]`），再 `base_url + 子路径`**。或参考 `build_models_url`（proxy.rs:2036-2043）按协议分类构造（Anthropic→/v1/models，其余→/models）。
- 注意 base_url 是否已含 `/v1`：OpenAI 标准 base `https://api.openai.com/v1`，子路径应是 `/responses/{id}`（不再加 /v1）。需按既有 `provider_api_path` 哲学（base_url 负责版本前缀，端点只给后缀）。

### 难点③：鉴权（平台凭证 vs 客户端 token）

- 客户端（Codex）发的 `Authorization: Bearer <group_name>` 是 **aidog 内部路由 token，不是上游凭证**。
- 子端点 passthrough 必须**换成平台凭证**（`platform.api_key`），同 `handle_models_passthrough` 用 `apply_models_auth`（proxy.rs:1966 / 2048-2059：OpenAI 兼容 → `Authorization: Bearer {api_key}`）。
- **不能**用 `passthrough_headers`（proxy.rs:2063，保留客户端 Authorization）——那是 ClaudeCode 订阅 OAuth 场景（客户端自带上游 OAuth），responses 平台是 API key 场景，必须替换。

### 难点④：create 是否也应透传

- 已分析（01 文件端点1）：create 在「平台有 openai_responses 端点」时**已透传**（same_protocol_passthrough=true，proxy.rs:982）。这条已符合 [[protocol-same-proto-passthrough]]，**无需改 create**。
- 仅「平台无 responses 端点、回退 openai」时有损转换，那是刻意设计（让纯 chat 平台也能服务 Codex），**不在本任务范围**（任务是「子端点支持」，create 已 work）。

## Caveats / Not Found

- `build_passthrough_url` 直接拼客户端 path 的行为对 responses 平台（base_url 含 /v1）不安全，**不能直接复用**，需新 URL 构造或泛化。
- response_id→platform 无持久映射，多 responses 平台分组下「取首个平台」方案有上游 404 风险（单平台分组安全）。
- 子端点 body 形态 / 是否带 query 未在 docs 核（见 04）。
