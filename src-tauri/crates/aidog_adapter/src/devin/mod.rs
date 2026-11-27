//! Devin（Cognition）平台适配层。
//!
//! # 接入协议（issue 153 取证，2026-11-28 核对官方文档）
//!
//! 出处的 raw OpenAPI 原文（本仓外下载，核对日 2026-11-28）：
//!   - https://docs.devin.ai/api-reference/v3/sessions/post-organizations-sessions.md
//!   - https://docs.devin.ai/api-reference/v3/sessions/get-organizations-session.md
//!   - https://docs.devin.ai/api-reference/v3/sessions/get-organizations-session-messages.md
//!   - https://docs.devin.ai/api-reference/overview
//!
//! Devin **不是 chat-completions wire 平台**：官方只提供 stateful session API
//! （Organization API v3，base `https://api.devin.ai/v3/organizations/{org_id}`，
//! Enterprise scope 为 `/v3/enterprise/*`），Bearer 认证（service user `cog_` key
//! 或 personal access token）。生命周期 create → poll → fetch messages：
//!   1. `POST /v3/organizations/{org_id}/sessions`，body 必填仅 `prompt`；
//!      其余可选（`attachment_urls` / `bypass_approval` / `max_acu_limit` /
//!      `structured_output_schema` / `playbook_id` / `create_as_user_id` 等）。
//!   2. `GET .../sessions/{devin_id}` 轮询到终态。`status` enum =
//!      new | claimed | running | exit | error | suspended | resuming；
//!      `status_detail` enum 含 working / finished / inactivity /
//!      usage_limit_exceeded / out_of_credits / out_of_quota 等。
//!      响应必带 `acus_consumed`（number）。
//!   3. `GET .../sessions/{devin_id}/messages` 取输出，Message 必带
//!      `source` + `message` 字段。
//!
//! ## 适配器逐项契约（对应 issue 验收清单）
//!
//! - **请求字段与默认值**：网关出站只发 `{"prompt": <messages 拼接>, "devin_mode":
//!   <model 映射>}`。`prompt` 是官方唯一必填字段（已核对 SessionCreateRequest
//!   required: [prompt]）；`devin_mode` 官方 enum = normal | fast | fusion | lite |
//!   ultra | fast-ultra | swe-2-medium/swe-2-high/swe-2-max，网关只映射
//!   normal/fast/lite/ultra/fusion 五档（与官方主档一致），未知模型回落 normal。
//!   其余官方字段网关未建模、不发送（官方缺省语义生效）。
//! - **思考参数**：session API 无 chat-wire thinking 开关/档位/预算字段。
//!   未证实项：无（字段在官方 schema 中不存在即 N/A）。
//! - **工具调用与服务端工具**：官方 schema 无 client tools / server tools 字段；
//!   网关对入站 chat `tools` 非空时丢弃 + warn（契约 8：禁崩）。未证实项：无。
//! - **图片与音视频**：官方支持 `attachment_urls`（URI 数组，格式不限于图片）；
//!   网关未建模——入站 multi-block 非 text block 丢弃 + warn（未接入 attachment
//!   上传，标未证实：attachment 内容类型与图片音视频映射关系官方页未逐项核对）。
//! - **system 提示**：session API 无独立 system 字段；网关把 system 折叠为
//!   prompt 开头 `[system] ...` 前缀（现状实现，行为不变）。
//! - **max_tokens 上限**：官方 schema 无 token 上限字段（按 ACU 计费非 token）；
//!   可选 `max_acu_limit`（integer）网关未建模。未证实项：无。
//! - **usage 字段（含缓存 token）**：官方按 **ACU** 计量，无 token / cache token
//!   字段。网关 `usage` 全 0、`acus_consumed` 落 `proxy_log.est_cost`（不叠 peak
//!   倍率，ACU 即厂商计量，叠加 = 重复计价）。
//! - **错误体格式**：官方 RFC 9457 `ProblemDetail`（`{type, title, status,
//!   detail, error_code, errors[], instance}`，required: title + status），
//!   状态码 401/403/404/409/410/422/429。网关把上游非 2xx 原样转
//!   BAD_GATEWAY + 文本摘要（不解析 ProblemDetail，现状行为不变）。
//! - **流式事件格式与结束标记**：官方 **无原生流式**。网关伪流式：轮询到终态后
//!   按入站协议发 Start → Delta → Stop（openai `[DONE]` / anthropic
//!   `message_stop`）；error/suspended/timeout 终态发 error chunk。
//!
//! # 模块内容
//!
//! - [`quota`]：ACU 用量查询脚本约定（走 registry quota_scripts 数据驱动）。
//! - `openai_chat` / `openai_completions` / `openai_responses`：历史恒等占位
//!   函数，**全仓无调用者**——Devin chat 流量在 `handler.rs` Protocol::Devin
//!   分支被拦截进 `aidog_core` 的 `gateway/proxy/devin.rs::handle_devin`，
//!   不经 forward_attempt / adapter / converter（见上）。保留仅为兼容既有
//!   `pub use` 导出面，禁止新增调用。

pub mod openai_chat;
pub mod openai_completions;
pub mod openai_responses;
pub mod quota;

pub use openai_chat::*;
pub use openai_completions::*;
pub use openai_responses::*;
