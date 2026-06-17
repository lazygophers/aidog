# PRD: 代理透传客户端 SDK 请求头到上游

## 现象

代理转发（convert 路径，非 passthrough）时，`apply_claude_code_family_headers` 等用**硬编码静态值**覆盖这批客户端身份/SDK 头：

| 头 | 硬编码值 | 用户客户端真实发 |
|---|---|---|
| X-Stainless-Package-Version | 0.60.0 | 0.94.0 |
| X-Stainless-Runtime-Version | v22.19.0 | v24.3.0 |
| X-Stainless-Timeout | 600 | 3000 |
| anthropic-version | 2023-06-01 | （可能含 beta tag） |
| anthropic-beta | （未设） | 用户实际 beta 列表 |
| x-claude-code-session-id | （未设） | 客户端会话 id |
| x-stainless-retry-count / -arch / -os / -lang / -runtime | 静态 | 真实运行时值 |

上游看不到客户端真实 SDK 版本/会话/重试计数，可能导致上游风控误判、版本路由错、诊断困难。

handle_passthrough（1:1 relay）已原样转发（`passthrough_headers(&orig_headers)`），问题仅在 convert 路径（`apply_client_headers` 系列）。

## 目标（用户已定：全 family 一致 + 跨协议兼容 + UA 不透传）

convert 路径上游头：**入站 header 全量透传**（剔 hop-by-hop + 强覆盖 auth/Content-Type），**再叠加** client_type 模拟身份头（UA 等不可变项）。跨协议时（如 Claude Code 入站 → OpenAI responses 上游），入站的 `anthropic-*` / `x-stainless-*` 等透明自定义头一并随入站透传到 OpenAI 上游（上游忽略未知头不报错，保留无害且利于上游风控/诊断见真实 SDK 版本/会话）。User-Agent 保持按 client_type 模拟（路由/身份推断依据）。

## 方案

1. convert 路径构建上游 req 时，**先透传 `passthrough_headers(orig_headers)` 全量入站头**（已剔 host/content-length），再由 apply_client_headers **覆盖** auth + Content-Type + UA（不可变身份项）。
2. apply_*_family_headers 里**删去硬编码的 x-stainless-* / anthropic-* / anthropic-beta / x-app / session-id 等**（这些随入站透传，入站无则不强塞默认 —— 上游不依赖它们存在）。保留 auth 覆盖 + UA 模拟 + Codex/Cursor 的协议必需头（如 OpenAI-Beta responses=experimental 若入站无）。
3. `build_upstream_headers` 同步：日志记录改为反映「入站透传 + 模拟覆盖」的真实上游头集（取 req_builder 最终头或合并记录）。
4. 主路径 1210-1216：`req_builder.headers(passthrough_headers(&orig_headers))` 先铺底，再 `apply_client_headers` 覆盖。
5. hop-by-hop 扩充：除 host/content-length，connection / keep-alive / proxy-* / te / trailers / upgrade 也剔（标准代理规范，避免上游混乱）。

## 强制覆盖清单（不透传入站值）

- `x-api-key` / `Authorization` / `x-goog-api-key`（= 平台 api_key）
- `Content-Type`（application/json）
- `User-Agent`（按 client_type 模拟）
- `Host` / `Content-Length` / hop-by-hop（reqwest 按目标重设）

## 透传清单（入站有则用，跨协议也带）

`anthropic-beta` / `anthropic-version` / `anthropic-dangerous-direct-browser-access` / `x-app` / `x-claude-code-session-id` / `x-stainless-*`（arch/lang/os/package-version/retry-count/runtime/runtime-version/timeout）/ Codex 的 `originator`/`version`/`session_id`/`conversation_id` / 其它未知自定义头（透传无害）。

## 不改

- handle_passthrough（已原样转发）。
- client_type 识别（UA 路由推断）。
- convert_request body 转换。

## 验收

1. `cargo test`（现有 passthrough_headers 测试 + 新增：convert 路径入站带 anthropic-beta/x-stainless-*/session-id → 上游头含透传值；auth 被平台 key 覆盖；hop-by-hop 剔除）。
2. `cargo clippy` 0 warning（除已接受 block）。
3. dev 实测：CC 发请求经代理转 OpenAI 平台 → 上游收 anthropic-beta + 真实 x-stainless-* + 平台 Authorization。

## 文件 / 范围

- `src-tauri/src/gateway/proxy.rs`：passthrough_headers（扩 hop-by-hop）+ apply_*_family_headers（删硬编码可变头）+ build_upstream_headers + 主路径铺底透传 + 测试。

## subtask

单一交付，不拆 child。

## 不改

- handle_passthrough（已原样转发）。
- auth 头（x-api-key/Authorization）覆盖逻辑。
- User-Agent 模拟（路由依据）。
- 客户端识别（client_type 推断）。

## 验收

1. `cargo test`（proxy.rs 现有 header 测试 + 新增：入站带 x-stainless-* / anthropic-beta → 上游收透传值；入站无 → 回退默认）。
2. `cargo clippy` 0 warning（除已接受 block）。
3. dev 实测：Claude Code 发请求经代理 → 上游（或日志 upstream_request_headers）见真实 0.94.0/v24.3.0/3000 + anthropic-beta + session-id。

## 文件 / 范围

- `src-tauri/src/gateway/proxy.rs`：apply_client_headers 系列 + build_upstream_headers + 主路径传参 + helper + 测试。

## subtask

单一交付（proxy header 透传），不拆 child。
