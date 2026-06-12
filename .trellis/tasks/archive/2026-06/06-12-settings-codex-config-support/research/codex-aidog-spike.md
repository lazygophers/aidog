# Spike: Codex → aidog Responses 链路验证（2026-06-12 实测）

## 结论：链路当前**断**，需 1 处 aidog 小修才通

## 实测
- curl Responses 格式请求到 aidog：
  `POST http://127.0.0.1:9876/proxy/v1/responses` body `{"model":...,"input":"say hi"}` Authorization `Bearer new-api-auto`
  → **400 "failed to parse request for protocol"**（proxy_log: source_protocol=openai, status 400, url /proxy/v1/responses）。

## 根因
- `detect_source_protocol`(proxy.rs:1333) 把 `/v1/responses` 归为 `"openai"`（与 chat/completions 同组，1346）。
- `parse_incoming_request("openai", body)`(converter.rs:60) 按 openai chat 解析（要 `messages`），Codex 发 Responses 格式（`input`）→ 解析失败 → 400。
- 但 converter **已有** `"openai_responses"` 分支（converter.rs:63 → `openai_responses::from_responses`）+ 出站 `to_responses`(converter.rs:23-26)。即转换能力齐，只是 detect 没派发到它。

## 修复（前置，小）
- `detect_source_protocol`：`/v1/responses` 单独返回 `"openai_responses"`（从 openai 组拆出）。
- 这样入站 Responses 请求经 `from_responses` 正确解析；下游路由/出站转换（to_responses 或转 chat 视目标平台）走既有逻辑。
- 顺带修通 Responses API 入站直通（此前从未工作）。

## Codex provider 配置（修复后）
- `[model_providers.aidog]` `base_url = "http://127.0.0.1:9876/proxy"`、`wire_api = "responses"`、`env_key = "<持分组名作 token 的 env>"`（Codex 把 base_url + `/v1/responses` 拼接 → aidog 见 `/proxy/v1/responses`，剥前缀为 `/v1/responses`，按 auth token=分组名路由分组）。
- per-group：每分组一个 profile（`~/.codex/<group>.config.toml` 含 `[model_providers.aidog]` base_url 同 + token=分组名）或共用 provider + `-c` 覆盖 token。**caveat 2**（profile 能否含 model_providers）仍需实测。

## 待验证（修复后）
- from_responses 对 Codex 实际请求体（reasoning/tools 等）覆盖完整度。
- 目标平台若是 chat/completions（非 Responses 兼容），aidog 是否把 Responses→chat 转换（converter 是否支持 openai_responses 入 → openai 出）。
- 分组路由对 Codex 请求（auth token=分组名）是否命中。
