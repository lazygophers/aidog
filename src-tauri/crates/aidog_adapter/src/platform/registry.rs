//! 平台 code → 适配器注册表（issue 68）。
//!
//! 显式登记：门禁测试 `every_registry_platform_has_adapter_registration` 保证
//! registry 全量平台（含无内置默认端点的 devin / newapi / passthrough）都有注册项，
//! 收集全量缺失后一次报错。运行时 `adapter_for` 对未登记 code 回落通用适配
//! （用户平台 `platform_type` 可取 registry 清单外的协议变体）。

use aidog_db::models::Protocol;
use serde_json::Value;

use super::error;
use super::hooks;
use super::PlatformAdapter;
use crate::converter;

/// 家族基座（通用适配）：全部接口与钩子 = trait 默认实现，
/// 与历史「按 wire 协议分派 + 通用钩子」链路逐字节等价。
struct GenericAdapter;

impl PlatformAdapter for GenericAdapter {}

/// AtlasCloud adapter. Verified against official docs on 2026-11-24:
/// - Endpoints: Anthropic Messages `POST /v1/messages` and OpenAI Chat Completions
///   `POST /v1/chat/completions`; runtime uses the user's configured protocol and base URL.
///   https://www.atlascloud.ai/docs/llm-protocols
/// - Request shape: Anthropic requires `model`, `messages`, `max_tokens`; `system`, tools,
///   tool choice, thinking and stop sequences are documented. Only the first four stop
///   sequences are used; `tool_choice: any` maps to `required`. OpenAI requires `model` and
///   `messages`; `stream` defaults to false. The service maps `max_completion_tokens` to
///   `max_tokens` and normalizes reasoning fields. No local default is added.
/// - Request options: OpenAI documents `max_tokens`, `temperature`, `top_p`, `top_k`,
///   `repetition_penalty`, `systemPrompt`, and `thinking`; only `stream=false` is a documented
///   default. `max_completion_tokens` is mapped to `max_tokens` by the service. Anthropic
///   `cache_control` has no effect when the service translates protocols. No max-token ceiling
///   or thinking parameter values/defaults are documented.
/// - Tools/media: client tools are supported. Anthropic hosted tools are unavailable; OpenAI
///   Chat Completions does not expose server-side web search. Anthropic images/documents work,
///   audio/video do not. OpenAI chat supports model-dependent image/video/audio inputs;
///   `video_url` is an AtlasCloud extension. AtlasCloud supplies a default system prompt when absent.
/// - Streaming: Anthropic uses Messages events through `message_stop` and no `[DONE]`; OpenAI
///   chat returns SSE chunks. AtlasCloud forces final usage inclusion for streaming.
///   OpenAI's exact event sequence and terminal marker were not documented.
/// - Usage/errors: OpenAI examples expose prompt/completion/total tokens; cache fields are
///   undocumented. Anthropic usage field names and cache accounting are undocumented. The errors
///   page says LLM protocol endpoints use `{code,msg,request_id,data}`; protocol docs list HTTP
///   401/422 errors. AtlasCloud documents requests over 50 MB as HTTP 413.
/// - This platform is not a coding plan; `Protocol::AtlasCloud` keeps the existing no-op
///   coding-plan injection behavior.
/// - Unverified, keep framework defaults: thinking parameter values/defaults, max-token cap,
///   cache-token fields and semantics, protocol-specific error-code meanings, OpenAI streaming
///   event sequence/terminator, and audio/video limits. No other override is justified by the docs.
///   https://www.atlascloud.ai/docs/llm-protocols
///   https://www.atlascloud.ai/docs/createChatCompletion
///   https://www.atlascloud.ai/docs/errors
struct AtlasCloudAdapter;

impl PlatformAdapter for AtlasCloudAdapter {
    fn classify_error(&self, status: u16, body: &str) -> error::UpstreamErrorClass {
        let Ok(value) = serde_json::from_str::<Value>(body) else {
            return error::classify_upstream_error(status, body);
        };
        let Some(message) = value.get("msg").and_then(Value::as_str) else {
            return error::classify_upstream_error(status, body);
        };
        let normalized = serde_json::json!({
            "error": {"message": message, "code": value.get("code")}
        });
        error::classify_upstream_error(status, &normalized.to_string())
    }
}

/// 月之暗面 Kimi 家族基座（issue 76；成员 kimi / kimi_en / kimi_coding 共用，
/// 成员差异见各成员 issue 117 / 118 / 119）。
///
/// 家族共性取证（官方文档原文，核对日期 2026-11-16；platform.moonshot.cn 已 301 到 platform.kimi.com，
/// 中国站与国际站同一文档结构）：
/// - 思考开关（openai 端点）：kimi-k2.6「`thinking.type` | 默认 `"enabled"`，传 `"disabled"` 可关闭思考」；
///   kimi-k2.7-code 系列「仅支持 `"enabled"`，传 `"disabled"` 会报错」；K3 系列「始终开启思考 … 使用顶层
///   `reasoning_effort` … 不使用 `thinking` 参数」。文档无 `chat_template_kwargs`，故覆写 openai 臂（见下）；
///   不可关闭的模型（kimi / kimi_en 的 kimi-k2.7-code、kimi-k2.7-code-highspeed、kimi-k3、kimi-k3.1-pro、
///   kimi-k3.1-pro-highspeed）在 registry 标 `thinking_toggleable: false`，走基座强制思考剥除、不写 `thinking`，
///   不在此覆盖；kimi_coding 条目未标注（成员 issue 119 核对）。
///   https://platform.kimi.com/docs/api/chat.md https://platform.kimi.ai/docs/api/chat.md
///   https://platform.kimi.com/docs/guide/use-thinking-models.md
///   上述取证只覆盖开放平台模型，openai 臂覆写仅对 kimi / kimi_en 生效；kimi_coding 的 openai 端点
///   禁思考写 Kimi Code 官方认收的 `reasoning_effort: "none"`（成员 issue 119，见下成员节）。
///   重开依据（Kimi Code 模型页原文，核对日期 2026-11-17）：
///   「off / none / disabled … K3 系列、K2.8 Preview 与 K2.7 Code Highspeed：关闭 thinking 后，请求均由
///   K2.8 Preview（无思考）处理」「K3.1 Pro 系列：关闭思考的请求会被拒绝（HTTP 400）」；该页无 temperature
///   固定值说明。https://www.kimi.com/code/docs/kimi-code/models.html
/// - 流式：「先接收 `delta.reasoning_content`，再接收 `delta.content`；`finish_reason="stop"` 表示生成结束，
///   其后的统计块 `choices` 为空，需从顶层 `usage` 读取用量」，以 `data: [DONE]` 结束；与基座 openai 构件一致。
///   `finish_reason` 取值 stop / length / tool_calls，同基座映射。
/// - usage（openai 端点）：`usage.prompt_tokens_details.cached_tokens`（缓存读取），基座已抽取并从 input 扣除。
///   另有 `prompt_tokens_details.cache_write_tokens`（缓存写入，计入 `prompt_tokens`），基座按 input 计；
///   官方价目 K3.1 Pro / K3 的 5m 缓存写入单价与输入单价相同、其余模型无缓存写入，且「Chat Completions 的响应
///   不回显实际应用的 mode/ttl」无法区分 1h 档，故不覆盖（流式 usage 累计也不经适配器，见 PR 遗留）。
/// - usage（anthropic 端点）：`cache_read_input_tokens` / `cache_creation_input_tokens` /
///   `cache_creation.ephemeral_5m|1h_input_tokens`，「总输入 = input_tokens + cache_read_input_tokens +
///   cache_creation_input_tokens」，与基座 anthropic 臂一致。https://platform.kimi.com/docs/api/messages.md
/// - 错误体：openai 端点 `{"error":{"type","message"}}`，anthropic 端点 `{"type":"error","error":{"type","message"}}`；
///   404 `resource_not_found_error`「Model not found, or this account does not have permission to access the model」
///   由基座特征 `not_found_error` 归模型类；401 / 403 / 429 由基座按状态码归服务类。
///   https://platform.kimi.com/docs/api/errors.md https://platform.kimi.ai/docs/api/errors.md
/// - 工具调用 / 图片 / system：标准 OpenAI 形状（`tools[].function`、`finish_reason: "tool_calls"`、
///   `image_url` 内容块、`role: "system"`），与基座一致，不覆盖。
///   https://platform.kimi.com/docs/guide/use-kimi-vision-model.md
/// - tool_choice（issue 76 跟进，kimi / kimi_en）：模型参数参考表
///   「[`tool_choice`](…) | `"auto"` / `"none"` / `"required"` | 传 `"required"` 会报错 | 传 `"required"`
///   会报错」——kimi-k2.6 与 kimi-k2.7-code（kimi-k2.7-code-highspeed 与 kimi-k2.7-code 参数约束相同）
///   上游对 `tool_choice: "required"` 报错；K3 系列同列「`"auto"` / `"none"` / `"required"`」可传，
///   不降级。覆写 [`PlatformAdapter::adjust_tool_choice`] 在 openai wire 出站 body 层把这些模型的
///   `"required"` 改写为 `"auto"`（openai 客户端直发的字符串与任何入站协议转换产物同形覆盖；
///   anthropic 入站的 `tool_choice` 对象当前在入站解析即失败，是入站侧既有缺口，非家族范围）；
///   anthropic wire 不在范围。kimi_coding 不降级（成员 issue 119）。
///   https://platform.kimi.com/docs/api/models-overview.md https://platform.kimi.ai/docs/api/models-overview.md
/// - Messages 端点模型集（issue 76 跟进）：messages.md 的 `model` 参数「本接口当前支持
///   `kimi-k3.1-pro`、`kimi-k3.1-pro-highspeed` 和 `kimi-k3`」（enum 同）——Anthropic Messages 端点
///   只服务 K3 系列。registry kimi / kimi_en 的 K2 系列条目（kimi-k2.6 / kimi-k2.7-code /
///   kimi-k2.7-code-highspeed）`supported_protocols`
///   相应只留 `["openai"]`（K3 系列条目保持含 `anthropic`）。
///   https://platform.kimi.com/docs/api/messages.md
/// - coding_plan 注入：`Protocol::Kimi` 沿用 wire-agnostic 的旧注入；`Protocol::KimiCoding`
///   仅 OpenAI wire 注入；`kimi_en` no-op。chat.md 将 prompt_cache_key 标为 Kimi Code Plan 必填。
/// - 未证实（保持基座默认）：anthropic 端点的 `thinking` 请求参数（messages.md 未列出）、
///   `content_filter` 错误类型（当前归「未知」类）。
///
/// 成员 `kimi`（Kimi（月之暗面），api.moonshot.cn 标准平台；issue 117，核对日期 2026-11-16）：
/// 与家族基座无已证实的成员级差异，登记为 [`KimiFamilyAdapter`]（全量开启家族覆写）；逐项核对结论
/// （官方原文关键句，本成员文档即家族取证所用中国站 platform.kimi.com）：
/// - 端点协议：registry 内置默认 openai `https://api.moonshot.cn/v1` 与 anthropic
///   `https://api.moonshot.cn/anthropic`，与官方一致，运行时只用用户平台配置的端点。
///   messages.md：「已经在使用 Anthropic SDK、Claude Code 等工具的开发者，只需把 base URL 指向
///   `https://api.moonshot.cn/anthropic`，即可直接调用 Kimi 模型」。
///   https://platform.kimi.com/docs/api/chat.md https://platform.kimi.com/docs/api/messages.md
/// - 思考 / tool_choice / temperature：kimi-k2.6「`thinking.type` 默认 `"enabled"`，传 `"disabled"`
///   可关闭思考」；kimi-k2.7-code 系与 K3 系列强制思考；kimi-k2.6 非思考模式 temperature 固定 `0.6`
///   「传入其他值会报错」；kimi-k2.6 / kimi-k2.7-code 对 `tool_choice`「传 `"required"` 会报错」，K3 不限
///   （同基座覆写行为）。https://platform.kimi.com/docs/api/models-overview.md
/// - usage：openai 臂 `usage.prompt_tokens_details.cached_tokens`（缓存读取）与 `cache_write_tokens`
///   （缓存写入，计入 `prompt_tokens`），基座按 input 计不区分读写，不覆盖（家族级取舍见基座说明）；
///   缓存写入行为由 `prompt_cache_options`（openai）/ 顶层 `cache_control`（anthropic messages.md）控制，
///   「不传时默认开启缓存写入（`5m` 档）」，aidog 不写即官方默认，不覆盖。
///   https://platform.kimi.com/docs/api/chat.md
/// - 流式 / 错误体 / 工具 / 多模态 / system：同家族基座（`delta.reasoning_content` → `delta.content`、
///   `data: [DONE]`；`{"error":{...}}` / `{"type":"error","error":{...}}`；标准 openai 形状），不覆盖。
///   https://platform.kimi.com/docs/api/errors.md
/// - prompt_cache_key：chat.md「对于 Kimi Code Plan，此字段为必填以提高缓存命中率。对于其他多轮对话
///   Agent，也建议使用此字段」——标准平台可选。aidog 既有迁移行为为 `Protocol::Kimi` + coding_plan 端点
///   注入（家族按平台协议分派，成员不覆盖），行为不变。
/// - 未证实（保持基座默认）：max_tokens 上限（chat.md 未列统一上限）、`content_filter` 错误类型。
///
/// 成员 `kimi_en`（Kimi 国际版（月之暗面），api.moonshot.ai 标准平台；issue 118，核对日期 2026-11-17）：
/// 与家族基座无已证实的成员级差异，登记为 [`KimiFamilyAdapter`]（全量开启家族覆写）；逐项核对结论
/// （官方原文关键句，国际站 platform.kimi.ai 与中国站同一文档结构）：
/// - 端点协议：registry 内置默认 openai `https://api.moonshot.ai/v1` 与 anthropic
///   `https://api.moonshot.ai/anthropic`，与官方一致，运行时只用用户平台配置的端点。
///   messages.md：「If you already use the Anthropic SDK, Claude Code, or similar tools, simply point
///   the base URL at `https://api.moonshot.ai/anthropic` to call Kimi models directly」。
///   https://platform.kimi.ai/docs/api/chat.md https://platform.kimi.ai/docs/api/messages.md
/// - 思考 / tool_choice / temperature：kimi-k2.6「Thinking is on by default」且「To turn off thinking
///   for `kimi-k2.6`, set `thinking.type` to `"disabled"`」；kimi-k2.7-code 系（「Thinking and Preserved
///   Thinking are always on」）与 K3 系列（`thinking`「Not supported」）强制思考；kimi-k2.6 temperature
///   「Fixed at `1.0` with thinking on and `0.6` with thinking off」；kimi-k2.6 / kimi-k2.7-code 对
///   `tool_choice`「Passing `"required"` returns an error」，K3 系列「`"auto"` / `"none"` / `"required"`」
///   不限（同基座覆写行为）。
///   https://platform.kimi.ai/docs/api/models-overview.md
/// - usage：openai 臂 `usage.prompt_tokens_details.cached_tokens`（缓存读取）与 `cache_write_tokens`
///   （缓存写入，「`usage.prompt_tokens` is always the total number of input tokens」），基座按 input
///   计不区分读写，不覆盖（家族级取舍见基座说明）；anthropic 臂 `cache_read_input_tokens` /
///   `cache_creation_input_tokens`，「Total input = `input_tokens` + `cache_read_input_tokens` +
///   `cache_creation_input_tokens`」，与基座一致，不覆盖。
///   https://platform.kimi.ai/docs/api/chat.md https://platform.kimi.ai/docs/api/messages.md
/// - 流式 / 错误体 / 工具 / 多模态 / system：同家族基座（`delta.reasoning_content` → `delta.content`、
///   `data: [DONE]`；`{"error":{"type","message"}}` / `{"type":"error","error":{...}}`、404
///   `resource_not_found_error`「Model not found, or this account does not have permission to access
///   the model」；标准 openai 形状），不覆盖。
///   https://platform.kimi.ai/docs/api/errors.md
/// - prompt_cache_key：国际站 chat.md 同称「For Kimi Code Plan, this field is required」——`kimi_en`
///   非编码套餐，基座按平台协议分派对 `KimiEn` 为 no-op，行为不变。
/// - 未证实（保持基座默认）：max_tokens 上限（chat.md 未列统一上限）、anthropic 端点的 `thinking`
///   请求参数、`content_filter` 错误类型。
///
/// 成员 `kimi_coding`（Kimi 编程套餐（月之暗面会员），api.kimi.com 编码端点；issue 119，核对日期
/// 2026-11-17）两项已证实差异：openai 端点禁思考写 `reasoning_effort: "none"`；coding_plan 端点
/// 注入 `prompt_cache_key`。其余同家族基座。逐项核对结论（官方原文关键句，本地留档
/// `.tmp/issue-119/docs/`）：
/// - 端点协议：Kimi Code 模型页「Kimi Code API 支持 OpenAI（Responses / Chat Completions）和
///   Anthropic 协议」，Base URL「OpenAI（Responses / Chat Completions）| 国内
///   `https://api.kimi.com/coding/v1`」「Anthropic 兼容 | 国内 `https://api.kimi.com/coding/`」，
///   与 registry 内置默认一致（海外 `api.kimi.ai` registry 未内置，用户可手配）；运行时只用用户
///   平台配置的端点。https://www.kimi.com/code/docs/kimi-code/models.html
/// - 思考开关（两协议端点）：模型页 effort 映射表「未传 effort 使用模型默认值：K3.1 Pro 系列和
///   K3 系列为 high，K2.8 Preview 为 max」「off / none / disabled → K3.1 Pro 系列：关闭思考的请求
///   会被拒绝（HTTP 400）；K3 系列、K2.8 Preview 与 K2.7 Code Highspeed：关闭 thinking 后，请求均由
///   K2.8 Preview（无思考）处理」「其他未知取值 返回 HTTP 400」；chat.md「Kimi K3 系列始终开启思考
///   与 Preserved Thinking，使用顶层 `reasoning_effort`（支持 `"low"`、`"high"`、`"max"`，默认
///   `"max"`），不使用 `thinking` 参数」。故禁思考写顶层 `reasoning_effort: "none"`（kimi-k2.6 式
///   `thinking.type` 与 vLLM 式 `chat_template_kwargs` 均非本套餐参数）；K2.7 Code Highspeed
///   「不支持调整思考程度」，禁思考后由 K2.8 Preview（无思考）处理，不额外剥除。K3.1 Pro 系列
///   registry 未建条目，其「禁思考 400」暂无覆盖路径，复活条件：registry 增加 k3.1-pro 条目时
///   评估强制思考标注。成员不剥 temperature（模型页无固定值说明）。
///   https://www.kimi.com/code/docs/kimi-code/models.html https://platform.kimi.com/docs/api/chat.md
/// - tool_choice：模型页与 chat.md 均未列编码套餐模型的 `tool_choice` 约束，不降级 `"required"`
///   （家族开关对该成员关闭）。
/// - prompt_cache_key：chat.md「`prompt_cache_key` … 对于 Kimi Code Plan，此字段为必填以提高缓存
///   命中率。对于其他多轮对话 Agent，也建议使用此字段」「通常是代表单个会话的 session id 或 task
///   id；退出并恢复会话时应保持不变」。迁移前转发层仅对 `Protocol::Kimi` 注入、
///   `KimiCoding` 走 no-op——与官方「必填」相悖，属迁移缺口；本成员按官方证据扩为
///   `Protocol::KimiCoding` + coding_plan 端点注入（显式键保留；同协议 passthrough 请求可从
///   metadata.session_id / user 派生稳定键；协议转换不会保留这些顶层字段，因而转换请求使用随机键），
///   非 coding_plan 端点不注入。https://platform.kimi.com/docs/api/chat.md
/// - usage：openai 臂 `usage.prompt_tokens_details.cached_tokens` / `cache_write_tokens`、anthropic 臂
///   `cache_read_input_tokens` / `cache_creation_input_tokens`，同家族基座（家族级取舍不区分缓存
///   写入，见基座说明），不覆盖。Kimi Code 专属文档（models.html / error-reference.html）未列 usage
///   字段。anthropic 端点请求侧新增顶层 `cache_control`（5m/1h TTL，不传默认 5m 读写）与
///   `output_config.effort`（messages.md schema）；aidog 不写即官方默认，不覆盖。
///   https://platform.kimi.com/docs/api/messages.md
/// - 流式 / 错误体：错误参考页条目均为「error, status code: <code>, message: <text>」文案形态，
///   JSON 错误体形状未证实；404「Not found the model kimi-for-coding or Permission denied」不能单凭
///   文案区分模型不存在与权限不足，故不扩展共享模型错误标记；400「Your model id does not exist, recognized as …」
///   由基座 `has_model` 分支归模型类，429 配额与 401/403 按状态码归服务类。
///   https://www.kimi.com/code/docs/kimi-code/error-reference.html
/// - 工具 / 多模态 / system：模型页「多模态输入支持 图片、视频」（k3-256k 仅图片）；错误参考
///   「thinking is enabled but reasoning_content is missing … 启用思维链时必须携带」是
///   **Kimi Code 专属校验**，与基座 openai 形状冲突：anthropic 客户端回灌的 thinking 块在
///   openai 转换中被跳过，多轮工具历史的 assistant tool-call 消息将缺 reasoning_content →
///   上游 400。已按成员覆写（openai wire 从 ChatRequest 取回 thinking 文本写入
///   reasoning_content，见 `carry_history_reasoning`）；「function name … is duplicated」
///   为上游校验行为，与基座形状不冲突，不覆盖。
///   https://www.kimi.com/code/docs/kimi-code/models.html
///   https://www.kimi.com/code/docs/kimi-code/error-reference.html
/// - 未证实（保持基座默认）：max_tokens 上限、JSON 错误体形状、anthropic 端点的 `thinking`
///   请求参数、`content_filter` 错误类型。
pub struct KimiFamilyAdapter {
    /// openai 端点禁思考是否写官方 `thinking` 并剥 temperature（kimi / kimi_en 为 true，
    /// kimi_coding 为 false——改写官方认收的 `reasoning_effort: "none"`，见下 coding 分支）。
    official_openai_thinking: bool,
    /// openai wire 禁思考是否写 Kimi Code 官方认收的顶层 `reasoning_effort: "none"`
    /// （仅 kimi_coding 为 true，成员 issue 119）。
    coding_openai_reasoning_effort: bool,
    /// openai wire 多轮工具历史是否携带 reasoning_content（Kimi Code 专属校验，thinking
    /// 启用时缺失 → 上游 400；仅 kimi_coding 为 true，成员 issue 119 review R2）。
    /// https://www.kimi.com/code/docs/kimi-code/error-reference.html
    carry_history_reasoning_openai: bool,
    /// openai wire 出站是否把受影响模型的 `tool_choice:"required"` 降级为 `"auto"`
    /// （kimi / kimi_en 为 true；kimi_coding 无已证实约束，不降级，成员 issue 119）。
    downgrade_required_tool_choice: bool,
}

impl PlatformAdapter for KimiFamilyAdapter {
    /// 差异（review R2，仅 kimi_coding openai wire）：多轮工具历史的 assistant tool-call
    /// 消息携带 reasoning_content（官方错误参考 400，见 [`hooks::carry_history_reasoning`]）。
    fn build_request(
        &self,
        chat_req: &crate::types::ChatRequest,
        wire: &Protocol,
        platform: &Protocol,
    ) -> (Value, String) {
        let (mut body, path) = converter::convert_request(chat_req, wire, platform);
        if self.carry_history_reasoning_openai && wire == &Protocol::OpenAI {
            hooks::carry_history_reasoning(&mut body, chat_req);
        }
        (body, path)
    }

    /// 差异：openai 端点禁思考写官方 `thinking.type=disabled`，不写基座第三方端点用的
    /// vLLM 式 `chat_template_kwargs.enable_thinking`（Kimi Chat Completions 文档无此参数）。
    /// 写出值与基座 anthropic 臂相同，故按 anthropic 臂执行；其余 wire 与强制思考路径同基座。
    /// 关思考时一并剥客户端 `temperature`：kimi-k2.6「思考模式固定 `1.0`，非思考模式固定 `0.6`」，
    /// 「表中“固定”表示该参数不可修改：传入其他值会报错，建议不要显式传入」。`top_p` / `n` /
    /// `presence_penalty` / `frequency_penalty` 两种模式下固定值相同，关思考不改变其合法性，不在此处理。
    /// https://platform.kimi.com/docs/api/models-overview.md https://platform.kimi.ai/docs/api/models-overview.md
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        // kimi_coding openai 端点：官方认收的禁思考写法是顶层 `reasoning_effort`（取值域
        // low/high/max + none；「off / none / disabled … K3 系列、K2.8 Preview 与 K2.7 Code
        // Highspeed：关闭 thinking 后，请求均由 K2.8 Preview（无思考）处理」「K3.1 Pro 系列：
        // 关闭思考的请求会被拒绝（HTTP 400）」「其他未知取值 返回 HTTP 400」），文档未列
        // `thinking` / `chat_template_kwargs` 参数。禁思考统一写 `reasoning_effort: "none"`
        // 交给上游裁决：K3.1 Pro（k3.1-pro / k3.1-pro-highspeed）按官方返回 400
        // 「K3.1 Pro does not support disabled thinking mode」，不静默降级为默认思考请求。
        // 写法与基座官方 OpenAI host 臂同形（`reasoning_effort: "none"`）；K2.7 Code Highspeed
        // 「不支持调整思考程度」，禁思考后由 K2.8 Preview（无思考）处理，不额外剥除。
        // https://www.kimi.com/code/docs/kimi-code/models.html
        // https://www.kimi.com/code/docs/kimi-code/error-reference.html
        // https://platform.kimi.com/docs/api/chat.md
        if self.coding_openai_reasoning_effort
            && wire == &Protocol::OpenAI
            && disable
            && !forced_thinking
            && let Some(obj) = body.as_object_mut()
        {
            obj.remove("disable_thinking");
            for k in [
                "thinking",
                "context_management",
                "reasoning_effort",
                "reasoning",
                "enable_thinking",
            ] {
                obj.remove(k);
            }
            obj.insert(
                "reasoning_effort".to_string(),
                Value::String("none".to_string()),
            );
            return;
        }
        let wire = match wire {
            Protocol::OpenAI if self.official_openai_thinking => {
                if disable
                    && !forced_thinking
                    && let Some(obj) = body.as_object_mut()
                {
                    obj.remove("temperature");
                }
                &Protocol::Anthropic
            }
            w => w,
        };
        hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking)
    }

    /// 差异：kimi-k2.6 / kimi-k2.7-code（含 -highspeed，与 kimi-k2.7-code 参数约束相同）的
    /// Chat Completions 对 `tool_choice: "required"` 报错；openai wire 出站把这些模型的
    /// `"required"` 改写为 `"auto"`。K3 系列接受 `"required"`，不降级；anthropic wire 不在范围；
    /// kimi_coding 经实例开关不降级（成员 issue 119：编码套餐文档未列该约束）。
    /// https://platform.kimi.com/docs/api/models-overview.md
    fn adjust_tool_choice(&self, body: &mut Value, wire: &Protocol, model: &str) {
        if !self.downgrade_required_tool_choice || wire != &Protocol::OpenAI {
            return;
        }
        if !matches!(
            model,
            "kimi-k2.6" | "kimi-k2.7-code" | "kimi-k2.7-code-highspeed"
        ) {
            return;
        }
        if let Some(obj) = body.as_object_mut()
            && obj.get("tool_choice").and_then(Value::as_str) == Some("required")
        {
            obj.insert("tool_choice".to_string(), Value::String("auto".to_string()));
        }
    }}

/// 智谱 GLM 家族基座（issue 75；成员 glm / glm_en / glm_coding / glm_coding_en 共用，
/// 成员差异见各成员 issue）。
///
/// 家族共性取证（官方文档，核对日期 2026-11-16）：
/// - 思考开关：请求字段 `thinking: {"type": "enabled" | "disabled"}`；GLM-5.3 / GLM-5.3-FLASH
///   为强制思考不可关闭（由 registry `thinking_toggleable: false` 走基座强制思考剥除，不在此覆盖）。
///   https://docs.bigmodel.cn/cn/guide/capabilities/thinking-mode
///   https://docs.z.ai/guides/capabilities/thinking-mode
/// - 流式思考增量 `delta.reasoning_content`、usage `prompt_tokens_details.cached_tokens`：
///   与基座 OpenAI 协议构件一致，不覆盖。
///   https://docs.bigmodel.cn/api-reference/%E6%A8%A1%E5%9E%8B-api/%E5%AF%B9%E8%AF%9D%E8%A1%A5%E5%85%A8
///   https://docs.z.ai/api-reference/llm/chat-completion
/// - 未证实（保持基座默认）：工具调用、图片与多模态、system 提示、错误体格式、
///   流式事件其余字段、Anthropic 端点的差异。
///
/// 成员 `glm`（GLM（智谱），open.bigmodel.cn 标准 API；issue 113，核对日期 2026-11-16）：
/// 与家族基座无已证实的成员级差异，登记为 `GLM`；逐项核对结论（官方原文关键句）：
/// - 端点协议：registry 内置默认为 openai（`/api/paas/v4`）与 anthropic（`/api/anthropic`），
///   运行时只用用户平台配置的端点；Claude 兼容页「替换您访问的 base_url 为
///   https://open.bigmodel.cn/api/anthropic」。https://docs.bigmodel.cn/cn/guide/develop/claude/introduction
/// - usage：「usage.prompt_tokens_details.cached_tokens」为缓存命中 token，基座 openai 臂已按此抽取
///   并从 input 扣除。https://docs.bigmodel.cn/cn/guide/capabilities/cache
/// - 流式：「data: [DONE]」结束，思考增量 `delta.reasoning_content`，与基座 openai 构件一致。
///   https://docs.bigmodel.cn/api-reference/%E6%A8%A1%E5%9E%8B-api/%E5%AF%B9%E8%AF%9D%E8%A1%A5%E5%85%A8
/// - 错误体：`{"error":{"code":"1001","message":"..."}}`；1211 / HTTP 400「模型不存在，请检查模型代码」
///   由基座分类为模型类。https://docs.bigmodel.cn/cn/api/api-code
/// - 保留式思考：「该能力在 Coding Plan 端点默认开启、标准 API 端点默认关闭」，`glm` 为标准端点，
///   aidog 不写 `clear_thinking`，行为即官方默认，不覆盖。
///   https://docs.bigmodel.cn/cn/guide/capabilities/thinking-mode
/// - 本成员无 coding_plan 注入（非编码套餐），基座按平台协议分派为 no-op。
/// - 未落（家族级候选，非 `glm` 独有，需另开 issue 拍板）：1301 内容安全错误当前按基座归「未知」类；
///   `finish_reason` 取值 `sensitive` / `network_error` / `model_context_window_exceeded`
///   当前按基座映射为 `end_turn`。
///
/// 成员 `glm_en`（GLM 国际版，api.z.ai；issue 114，核对日期 2026-11-16）：
/// 唯一已证实差异是错误文案语言（见下 1212），由家族级 `classify_error` 覆写按业务码处理；其余同基座。
/// 逐项核对结论（官方原文关键句）：
/// - 端点协议：registry 内置默认为 openai（`https://api.z.ai/api/paas/v4`）与 anthropic
///   （`https://api.z.ai/api/anthropic`），运行时只用用户平台配置的端点。官方 Anthropic 端点只在
///   Coding Plan 页列出：「Anthropic Messages | `https://api.z.ai/api/anthropic`」。
///   https://docs.z.ai/api-reference/llm/chat-completion https://docs.z.ai/devpack/quick-start
/// - 思考：「"thinking": { "type": "disabled" }」，GLM-5.3 / GLM-5.3-FLASH「use forced thinking and
///   cannot be disabled」，同家族基座。https://docs.z.ai/guides/capabilities/thinking-mode
/// - 保留式思考：「enabled by default on the Coding Plan endpoint and disabled by default on the
///   standard API endpoint」，`glm_en` 为标准端点，aidog 不写 `clear_thinking`，不覆盖。
/// - `reasoning_effort`（GLM-5.2 起，默认 `max`）：bigmodel 文档同有，家族级，非本成员差异。
/// - usage：「`usage.prompt_tokens_details.cached_tokens`」，同基座 openai 臂。
///   https://docs.z.ai/guides/capabilities/cache
/// - 流式：「`choices[0].delta.reasoning_content`: Incremental reasoning content」，以「data: [DONE]」结束。
///   https://docs.z.ai/guides/capabilities/streaming
/// - 错误体：「a top-level error object that includes a `code` and `message` value」，业务码与 bigmodel 同表，
///   文案为英文。1211「Unknown Model, please check the model code.」由基座特征归模型类；
///   1212「Current model does not support `${method}` call method」基座特征不命中（中文「当前模型不支持」命中），
///   落「未知」类——此为本成员差异，按业务码归模型类。https://docs.z.ai/api-reference/api-code
/// - 本成员无 coding_plan 注入（非编码套餐），基座按平台协议分派为 no-op。
/// - 未证实（保持基座默认）：工具调用、图片与多模态、system 提示、max_tokens 上限。
///
/// 成员 `glm_coding`（GLM 编码套餐（智谱），open.bigmodel.cn Coding Plan；issue 115，核对日期 2026-11-16）：
/// 与家族基座无已证实的出站差异，登记为 `GLM`；逐项核对结论（官方原文关键句）：
/// - 端点协议：Coding Plan「支持 Anthropic 协议和 OpenAI 协议两种接入方式」，「Anthropic Message 协议 |
///   `https://open.bigmodel.cn/api/anthropic`」「OpenAI Chat Completion 协议 |
///   `https://open.bigmodel.cn/api/coding/paas/v4`」，与 registry 内置默认一致；运行时只用用户平台配置的端点。
///   同页另列「OpenAI Response 协议 | `https://open.bigmodel.cn/api/v1`」，registry 未内置，用户可手配。
///   https://docs.bigmodel.cn/cn/coding-plan/quick-start
/// - 保留式思考：「该能力在 Coding Plan 端点默认开启、标准 API 端点默认关闭」，开关为请求字段
///   `clear_thinking`。aidog 不写 `clear_thinking`，Coding Plan 端点即按官方默认开启，
///   与标准端点的默认差异由上游端点自身决定，出站无需区分，不覆盖。
///   https://docs.bigmodel.cn/cn/guide/capabilities/thinking-mode
/// - 思考开关 / usage / 流式：同家族基座（`thinking.type`、`prompt_tokens_details.cached_tokens`、
///   `delta.reasoning_content`、`data: [DONE]`）。
/// - 错误体：同 bigmodel 错误码表；套餐专属码「1309 | 429 | 您的 GLM Coding Plan 套餐已到期」、
///   「1308 | 429 | 已达到 … 的使用上限」、「1310 | 429 | 您已达到每周/每月使用上限」均为 HTTP 429，
///   基座按状态码直接归服务类（换平台），不覆盖。https://docs.bigmodel.cn/cn/api/api-code
/// - coding_plan 注入：registry 端点标 `coding_plan: true`，基座按平台协议分派对 `GlmCoding` 为 no-op
///   （迁移前转发层同为 no-op），行为不变。
/// - 未证实（保持基座默认）：工具调用、图片与多模态、system 提示、max_tokens 上限。
///
/// 成员 `glm_coding_en`（GLM 编码套餐国际版，api.z.ai Coding Plan；issue 116，核对日期 2026-11-16）：
/// 与家族基座无已证实的出站差异，登记为 `GLM`；逐项核对结论（官方原文关键句）：
/// - 端点协议：「GLM Coding Plan supports both the Anthropic and OpenAI protocols」，「Anthropic Messages |
///   `https://api.z.ai/api/anthropic`」「OpenAI Chat Completions | `https://api.z.ai/api/coding/paas/v4`」，
///   与 registry 内置默认一致；运行时只用用户平台配置的端点。同页另列「OpenAI Responses |
///   `https://api.z.ai/api/v1`」，registry 未内置，用户可手配。https://docs.z.ai/devpack/quick-start
/// - 保留式思考：「enabled by default on the Coding Plan endpoint and disabled by default on the standard API
///   endpoint」，开关为请求字段 `clear_thinking`。aidog 不写 `clear_thinking`，即按官方端点默认，不覆盖。
///   https://docs.z.ai/guides/capabilities/thinking-mode
/// - 思考开关 / usage / 流式：同家族基座（`thinking.type`、`prompt_tokens_details.cached_tokens`、
///   `delta.reasoning_content`、「a `data: [DONE]`」结束）。https://docs.z.ai/api-reference/llm/chat-completion
/// - 错误体：同 `glm_en` 英文码表，1212 由家族覆写归模型类。错误码页称「a top-level error object that includes
///   a `code` and `message` value」，而 chat-completion OpenAPI 的失败响应 schema 为顶层 `Error`
///   （`code: integer` + `message`，无 `error` 包裹）；两份一手文档形态冲突，覆写两种形态都认。
///   https://docs.z.ai/api-reference/api-code
/// - 套餐码：「1311 | 429 | Your current subscription plan does not yet include access to `${model_name}`」
///   语义是模型权限，但 HTTP 429 先于业务码归服务类（换平台并计入平台健康），不覆盖；复活条件：
///   套餐用户反馈因模型权限换平台而拖累平台健康时重开。1313（Fair Usage 限频）与 1316–1319（5 小时 / 7 天
///   用量上限）同为 HTTP 429，未登记进 registry `rate_limit_error_codes`（registry 数据跟进项）。
/// - coding_plan 注入：registry 端点标 `coding_plan: true`，基座按平台协议分派对 `GlmCodingEn` 为 no-op
///   （迁移前转发层同为 no-op），行为不变。
/// - 未证实（保持基座默认）：工具调用、图片与多模态（套餐视觉经 Vision MCP 提供，非请求字段）、
///   system 提示、max_tokens 上限。
///
/// 覆写范围：`classify_error` 的业务码 1212（issue 114；字符串与整数两种形态，issue 115；`error` 包裹与顶层
/// 两种位置，issue 116）与 `apply_disable_thinking` 的 `Protocol::OpenAI` 臂；
/// 后者中 `Protocol::OpenAICompletions`
/// 不在覆写范围内（家族四个成员的内置默认端点仅 openai / anthropic，无 openai_completions），
/// 用户若手配 openai_completions 端点仍走基座 `chat_template_kwargs` 写法。
pub struct GlmFamilyAdapter;

impl PlatformAdapter for GlmFamilyAdapter {
    /// 差异：OpenAI 端点禁思考写官方 `thinking.type=disabled`，不写基座第三方端点用的
    /// vLLM 式 `chat_template_kwargs.enable_thinking`（GLM 官方 API 文档无此参数）。
    /// 该写出值与基座 anthropic 臂相同，故按 anthropic 臂执行；其余 wire 与强制思考路径同基座。
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        let wire = match wire {
            Protocol::OpenAI => &Protocol::Anthropic,
            w => w,
        };
        hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking)
    }

    /// 差异：业务码 1212（当前模型不支持该调用方式）归模型类，不依赖文案语言——国际版英文文案
    /// 不命中基座特征词（https://docs.z.ai/api-reference/api-code）。其余同基座。
    /// `code` 接受字符串与整数两种形态：错误码页示例为字符串，OpenAPI `Error.code` 为 `type: integer`
    /// （https://docs.z.ai/api-reference/llm/chat-completion）。该 OpenAPI 失败响应的 schema 即 `Error`
    /// 本身（顶层 `code` / `message`，无 `error` 包裹），故无 `error` 对象时读顶层，与基座读法一致（issue 116）。
    fn classify_error(&self, status: u16, body: &str) -> error::UpstreamErrorClass {
        let is_1212 = !matches!(status, 401 | 402 | 403 | 429)
            && serde_json::from_str::<Value>(body).is_ok_and(|v| {
                let code = &v.get("error").unwrap_or(&v)["code"];
                code == "1212" || code == 1212
            });
        if is_1212 {
            return error::UpstreamErrorClass::Model;
        }
        error::classify_upstream_error(status, body)
    }
}

/// MiniMax 家族基座（issue 77；成员 minimax / minimax_en / minimax_coding 共用）。
///
/// 家族共性取证（官方页面原始 HTML，核对日期 2026-11-17）：
/// - OpenAI Chat Completions：`thinking.type` 默认为 `adaptive`；MiniMax-M3 可传 `disabled` 关闭思考；
///   MiniMax-M3.1-Flash 禁止 `disabled`，返回 HTTP 400（code 2013），由 registry 的强制思考标记
///   去除禁用参数。禁思考覆写仅把 OpenAI wire 的禁用值改为 MiniMax 官方参数；Anthropic wire 沿用基座。
///   https://platform.minimaxi.com/docs/api-reference/text-openai-api
///   https://platform.minimax.io/docs/api-reference/text-openai-api
///   https://platform.minimax.cn/docs/api-reference/text-openai-api
/// - OpenAI 请求/响应：文档给出 `tools`、`image_url`、`video_url`、`reasoning_content`、`finish_reason`、
///   `usage.prompt_tokens_details.cached_tokens`；基座 OpenAI 构件维持现状。响应另含 `base_resp`，成功示例
///   `status_code: 0`。错误码页列出 `1001` 超时、`1002` 限流、`1004` 鉴权、`1008` 余额不足、
///   `1013` 服务内部、`1027` 输出内容、`1039` token 超限、`2013` 参数错误；错误分类不扩展，
///   因文档未给出可映射至模型类的专用业务码/错误体契约。
///   https://platform.minimaxi.com/docs/api-reference/errorcode
/// - Anthropic Messages：官方文档说明完整支持 system、tools、stream 与 thinking；`MiniMax-M3.1-Flash`
///   强制 thinking，`MiniMax-M3` 默认关闭并可用 `adaptive` 开启；`output_config.effort` 支持前者。
///   请求、流、错误、usage 与其他响应细节按基座处理；文档未证实 Anthropic 缓存 usage 的额外形态。
///   https://platform.minimaxi.com/docs/api-reference/text-anthropic-api
///   https://platform.minimax.io/docs/api-reference/text-anthropic-api
/// - International (`minimax.io`) 与中国站 API 参考页的上述协议字段一致；coding-plan 官方页描述套餐/额度，
///   未记载新的 chat 字段，故无 coding-plan 请求差异。
///   https://platform.minimaxi.com/docs/token-plan/other-tools
///   https://platform.minimax.io/docs
/// - 未证实（保持基座默认）：错误体是否总包含 `base_resp`、stream usage 的含义及其端点差异、工具选择限制、
///   各模型外的固定 sampling 参数。
///
/// 成员 `minimax`（MiniMax 海螺，中国站；issue 120，核对日期 2026-11-17）：
/// - 端点：OpenAI Chat Completions 与 Anthropic Messages。registry 地址只用于新建表单预填；运行时使用用户配置的
///   protocol + base_url（创建 / 更新均保留该配置）。
/// - OpenAI 请求：`model`、`messages`；支持 `system` role、function `tools`、`image_url` / `video_url`。
///   `thinking.type` 缺省 `adaptive`；M3 可设 `disabled`，M3.1-Flash 强制 thinking；`max_completion_tokens`
///   上限 524288、推荐 131072，缺省值未证实；`max_tokens` 已标 deprecated。sampling 固定 temperature=1、
///   top_p=0.95。文档未证实 tool_choice 限制、Chat Completions 服务端工具、音频请求格式。
/// - Anthropic 请求：`model`、`messages`，支持 system / tools / thinking；`tool_choice` 仅文档列出 `auto`。
///   M3.1-Flash thinking 固定开启，effort 缺省 `max`；M3 默认关闭、可用 `adaptive` 开启。
///   `max_tokens` 上限 524288、推荐 131072，缺省值未证实。支持 JPEG/PNG/GIF/WebP 图片和 MP4/AVI/MOV/MKV
///   视频；音频输入未证实。服务器端 `web_search` 文档支持 Anthropic Messages 与 OpenAI Responses，未列 Chat Completions。
/// - 响应 / usage：OpenAI 有 `prompt_tokens`、`completion_tokens`、`total_tokens` 与
///   `prompt_tokens_details.cached_tokens`；Anthropic 有 `input_tokens`、`output_tokens`、
///   `cache_read_input_tokens`、`cache_creation_input_tokens`。继续由基座 usage 解析。
/// - 错误：OpenAI 文档示例使用 `base_resp.status_code/status_msg`，通用错误码页未定义统一 JSON schema；
///   Anthropic 形状为 `type=error` + `error.type/message`，并列出 HTTP 400/401/403/404/413/429/500/529。
///   不添加 MiniMax 专属错误分类。
/// - 流：OpenAI `choices[].delta.content/reasoning_content`，末块可带 `finish_reason`；usage 末块需
///   `stream_options.include_usage`。Anthropic 事件 `message_start`、`ping`、`content_block_start/delta/stop`、
///   `message_delta`、`message_stop`；额外 SSE sentinel 未证实。继承基座流解析。
/// - 未发现区别于家族基座的转换规则。未证实项保持基座行为，不添加请求改写。
///   https://platform.minimaxi.com/docs/api-reference/text-chat-openai.md
///   https://platform.minimaxi.com/docs/api-reference/text-chat-anthropic.md
///   https://platform.minimaxi.com/docs/api-reference/text-prompt-caching.md
///   https://platform.minimaxi.com/docs/guides/server-tools.md
///   https://platform.minimaxi.com/docs/api-reference/errorcode.md
///
/// 成员 `minimax_en`（MiniMax 国际版；issue 121，核对日期 2026-11-18）：
/// 与家族基座无已证实的成员级差异，登记为 `MiniMaxEnAdapter`；国际站 OpenAI Chat Completions
/// 与 Anthropic Messages 的请求、响应、流式事件、错误体与 usage 字段均继承家族基座。
/// - 端点协议：官方 OpenAI 兼容端点 `https://api.minimax.io/v1/chat/completions`；Anthropic 兼容端点
///   `https://api.minimax.io/anthropic/v1/messages`。运行时仍只使用用户配置的 protocol + base_url。
/// - OpenAI 请求：支持 system role、function tools、`image_url` / `video_url`；图像 JPEG/PNG/GIF/WEBP ≤10 MB，
///   视频 MP4/AVI/MOV/MKV，URL/base64 ≤50 MB、请求体 ≤64 MB；`thinking.type` 缺省 adaptive，M3 可设 disabled，
///   M3.1-Flash 拒绝 disabled。`reasoning_effort`（Flash 专属）默认 max；`reasoning_split=true` 分离
///   `reasoning_content`，false 将 thinking 放进 `<think>`（Flash 拒绝 false）。`max_completion_tokens` 推荐
///   131072、上限 524288，`max_tokens` 已 deprecated；temperature 固定 1，top_p 固定 0.95。响应 `finish_reason`
///   为 stop / length / content_filter / tool_calls，usage 含 `prompt_tokens_details.cached_tokens`；流 usage
///   需 `stream_options.include_usage`。
/// - Anthropic 请求：支持 system、tools、image / video、stream 与 thinking；M3.1-Flash 强制 thinking，M3 缺省关闭、
///   可用 adaptive 开启；`output_config.effort` 支持 low / medium / high / xhigh / max，默认 max；`tool_choice`
///   只支持 `auto`。`max_tokens` 推荐 131072、上限 524288；usage 含 input / output、cache read / creation。
///   `cache_control` 不受支持。服务端 `web_search` 支持 Anthropic Messages 与 OpenAI Responses，不支持 Chat
///   Completions。错误用 HTTP 状态码 + JSON body（`type=error`、`error.type/message`）；流含 message_start、
///   content_block_start/delta/stop、message_delta、message_stop；流错误发 `event: error`；额外 SSE sentinel 未证实。
/// - 未证实（保持基座默认）：Anthropic cache usage 额外字段、OpenAI 错误体是否含 `base_resp`、音频输入、端点间差异。
///   https://platform.minimax.io/docs/api-reference/text-chat-openai.md
///   https://platform.minimax.io/docs/api-reference/text-chat-anthropic.md
///   https://platform.minimax.io/docs/api-reference/text-prompt-caching.md
///   https://platform.minimax.io/docs/guides/server-tools.md
///   https://platform.minimax.io/docs/api-reference/errorcode.md
///
/// 成员 `minimax_coding`（Token Plan；issue 122，核对日期 2026-11-18）：
/// Token Plan 配置页列出 OpenAI Compatible 与 Anthropic Compatible，Base URL 分别为
/// `https://api.minimax.cn/v1` 与 `https://api.minimax.cn/anthropic`。运行时仍只使用用户配置的 protocol + base_url。
/// 两个协议参考页核对结果：
/// - 请求字段 / 默认值：OpenAI 示例含 `model`、`messages`、`max_completion_tokens`、`reasoning_effort`；
///   M3.1-Flash 的思考默认开启，effort 默认 `max`；Anthropic 示例含 `model`、`messages`、`max_tokens`、
///   `output_config.effort`，思考默认开启，effort 默认 `max`。未证实 Token Plan 是否覆盖这些字段或默认值。
/// - 思考：OpenAI `thinking.type` / `reasoning_effort`，Anthropic `thinking` / `output_config.effort`；
///   两页均未描述 Token Plan 专属开关、档位或预算差异，故沿用家族处理。
/// - Tools / system / media：两页均有 function tools 与 system 能力示例；OpenAI 示例用 `image_url`、`video_url`，
///   Anthropic 示例用 `image`、`video` 内容块。服务端工具的 Token Plan 支持情况未证实。
/// - 限制 / usage：两页示例分别使用 `max_completion_tokens` 与 `max_tokens: 4096`，未证实套餐专属上限；
///   OpenAI usage 有 prompt/completion/total 与 `prompt_tokens_details.cached_tokens`，Anthropic usage 有
///   input/output、cache-read 与 cache-creation token。未证实 Token Plan 是否另有 usage 字段。
/// - 错误 / 流：API 参考展示 OpenAI `base_resp` 与 Anthropic `type=error` 形状；支持 stream，但 Token Plan
///   专属错误体、事件、usage 语义及终止标记未证实。沿用家族基座错误、流与 usage 处理，不新增映射。
/// - coding_plan 注入：套餐页未记录 chat 请求专属注入字段，保持 no-op；两协议本地假上游测试覆盖请求、响应、流、错误与 usage。
///
/// https://platform.minimaxi.com/docs/token-plan/other-tools （重定向至 minimax.cn）
/// https://platform.minimaxi.com/docs/api-reference/text-chat-openai.md
/// https://platform.minimaxi.com/docs/api-reference/text-chat-anthropic.md
/// https://platform.minimaxi.com/docs/api-reference/errorcode.md
///
/// `MiniMaxEnAdapter` 与 `MiniMaxCodingAdapter` 继承同一家族基座；成员差异留给各自 issue。
pub struct MiniMaxFamilyAdapter;

impl PlatformAdapter for MiniMaxFamilyAdapter {
    /// OpenAI wire 禁思考使用 MiniMax 的 `thinking.type=disabled`，而非通用 OpenAI host 的
    /// `reasoning_effort=none` / vLLM `chat_template_kwargs`。强制思考模型仍由共享钩子剥禁用参数。
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        let wire = match wire {
            Protocol::OpenAI => &Protocol::Anthropic,
            w => w,
        };
        hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking)
    }
}

/// MiniMax 海螺成员适配器；保持家族的 thinking 约定，其他接口继承协议基座默认实现。
struct MiniMaxAdapter;

impl PlatformAdapter for MiniMaxAdapter {
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        MINIMAX_FAMILY.apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking);
    }
}

/// MiniMax 国际版成员适配器；其他接口继承家族基座默认实现。
struct MiniMaxEnAdapter;

impl PlatformAdapter for MiniMaxEnAdapter {
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        MINIMAX_FAMILY.apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking);
    }
}

/// MiniMax Token Plan 成员适配器；文档未证实专属差异，沿用家族基座默认转换。
struct MiniMaxCodingAdapter;

impl PlatformAdapter for MiniMaxCodingAdapter {
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        MINIMAX_FAMILY.apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking);
    }
}

/// 阿里云百炼 家族基座（issue 78；成员 bailian / bailian_en / bailian_coding /
/// bailian_coding_en 共用，成员差异见各成员 issue 123 / 124 / 125 / 126）。
///
/// 家族共性取证（官方文档原文，核对日期 2026-11-19）：
/// - 思考开关（openai 端点）：混合思考模型用顶层布尔参数 `enable_thinking`（「设为
///   `true`：模型先思考再回复；设为 `false`：模型直接回复」；「该参数非 OpenAI 标准参数。
///   通过 Python SDK 调用时，请放入 extra_body 对象中…若不使用 OpenAI SDK，而是通过
///   HTTP（如 curl）方式直接调用，则无需 extra_body，直接将 enable_thinking 与 model、
///   messages 等参数一样放在请求体的顶层即可」）。控制参数因模型而异：DeepSeek-V4 系列
///   用 `reasoning_effort` 调推理力度、稀宇科技直供 MiniMax 用 `thinking` 参数
///   （per-model 差异，不在基座覆写，成员 issue 层处理）；仅思考模型（deepseek-r1、
///   kimi-k2.7-code 等）无法关闭，registry 标 `thinking_toggleable: false` 走基座强制
///   思考剥除。https://help.aliyun.com/zh/model-studio/deep-thinking
///   https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions
/// - 思考开关（anthropic 端点）：`thinking.type` 取值 `enabled` / `disabled`
///   （「可选值：enabled（开启思考模式）、disabled（关闭思考模式）」），与基座 anthropic
///   臂同形，不覆盖；思考强度新写法为百炼扩展参数 `output_config.effort`（aidog 不写即
///   官方默认）。https://help.aliyun.com/zh/model-studio/anthropic-api-messages
/// - 流式 / usage（openai 端点）：思考增量 `delta.reasoning_content`、回复
///   `delta.content`；usage `prompt_tokens_details.cached_tokens`（命中 Cache）、
///   `completion_tokens_details.reasoning_tokens`（思考 Token，是 text_tokens 的子集）、
///   `cache_creation` / `cache_write_tokens`（「与 cache_write_tokens 相同，统计时不要
///   重复相加」）。基座 openai 构件已抽取 cached_tokens 并从 input 扣除；cache_write
///   是否计入 prompt_tokens 文档未明示，基座按 input 计，不覆盖（家族级取舍同 Kimi 基座）。
///   https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions
///   https://help.aliyun.com/zh/model-studio/deep-thinking
/// - usage（anthropic 端点）：`input_tokens`「仅统计未读取也未创建缓存的输入 Token 数；
///   全部输入为本字段与缓存读取、创建 Token 数之和」+ `cache_read_input_tokens` /
///   `cache_creation_input_tokens`，与基座 anthropic 臂一致，不覆盖。
///   https://help.aliyun.com/zh/model-studio/anthropic-api-messages
/// - 错误体：404 `ModelNotFound` / `model_not_found`（「Model can not be found.」/
///   「The model xxx does not exist.」/「The model xxx does not exist or you do not have
///   access to it.」）——基座特征（model + does not exist / model_not_found）归模型类；
///   401（InvalidApiKey/invalid_api_key）/ 403（AccessDenied/access_denied）/
///   429（Throttling.RateQuota 等）按状态码归服务类；错误体统一 JSON 形状未证实，基座
///   `error.message/code/type` 与顶层双形态读法已覆盖，不扩展。
///   https://help.aliyun.com/zh/model-studio/error-code
/// - 工具 / 图片与多模态 / system：Chat Completions 文档给出 `tools` / `tool_choice` /
///   `image_url` 等 OpenAI 标准形状；Anthropic Messages 支持 system / tools / 图片
///   内容块；均与基座构件一致，不覆盖。
///   https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions
///   https://help.aliyun.com/zh/model-studio/anthropic-api-messages
/// - coding_plan 注入：四个成员均无官方必填/建议的出站附加字段（Claude Code 接入页只要求
///   换 base_url 与 API Key），基座按平台协议分派对 BailianCoding / BailianCodingEn
///   协议为 no-op，行为不变。https://help.aliyun.com/zh/model-studio/claude-code
/// - 未证实（保持基座默认）：DashScope 对 vLLM 式 `chat_template_kwargs.enable_thinking`
///   的认受性（官方只列顶层 `enable_thinking`，故覆写 openai 臂）、错误体统一 JSON 形状、
///   cache_write 是否计入 prompt_tokens、max_tokens 统一上限。
///
/// 成员 `bailian`（百炼（阿里云）；issue 123）、`bailian_en`（百炼国际版；issue 124）、
/// `bailian_coding`（百炼编程；issue 125）、`bailian_coding_en`（百炼编程国际版；issue 126）：
/// 均登记为 [`BailianFamilyAdapter`]；registry 内置默认端点与官方一致（中国站
/// dashscope.aliyuncs.com、国际站 dashscope-intl / dashscope-us、编码套餐
/// coding.dashscope.aliyuncs.com / coding-intl.dashscope.aliyuncs.com 的 compatible-mode/v1
/// 与 apps/anthropic），运行时只用用户平台配置的端点。成员级差异（如按模型的思考参数分派）
/// 由各成员 issue 取证后覆盖。
pub struct BailianFamilyAdapter;

impl PlatformAdapter for BailianFamilyAdapter {
    /// 差异：openai 端点禁思考写官方顶层 `enable_thinking: false`，不写基座第三方端点用的
    /// vLLM 式 `chat_template_kwargs.enable_thinking`（DashScope Chat Completions 文档只列
    /// 顶层参数；`enable_thinking` 已在基座剔参清单内，写 false 前同样剥掉客户端带来的旧值
    /// 与 `thinking` / `reasoning_effort` 等开启型参数）。其余 wire 与强制思考路径同基座；
    /// `Protocol::OpenAICompletions` 不在覆写范围内（家族成员内置默认端点仅 openai /
    /// anthropic，用户手配 openai_completions 端点仍走基座 `chat_template_kwargs` 写法，
    /// 与 GLM 基座同口径）。
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        if wire != &Protocol::OpenAI || forced_thinking {
            return hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking);
        }
        let Some(obj) = body.as_object_mut() else {
            return;
        };
        obj.remove("disable_thinking");
        if !disable {
            return;
        }
        for k in [
            "thinking",
            "context_management",
            "reasoning_effort",
            "reasoning",
            "enable_thinking",
        ] {
            obj.remove(k);
        }
        if let Some(ctk) = obj
            .get_mut("chat_template_kwargs")
            .and_then(|o| o.as_object_mut())
        {
            ctk.remove("enable_thinking");
        }
        if let Some(gc) = obj
            .get_mut("generationConfig")
            .and_then(|o| o.as_object_mut())
        {
            gc.remove("thinkingConfig");
        }
        obj.insert("enable_thinking".to_string(), Value::Bool(false));
    }
}

/// 火山方舟 家族基座（issue 79；成员 doubao / byteplus 共用，成员差异见子 issue 127 / 128）。
///
/// 家族共性取证（官方文档原文，核对日期 2026-11-19；volcengine.com/docs/82379 已 301 到
/// docs.volcengine.com，BytePlus 文档站 docs.byteplus.com/en/docs/ModelArk 页面 JS 渲染抓不到
/// 正文，仅得「Deep reasoning」「Responses API」等条目名，字节跳动同源关系未逐条复核）：
/// - 思考开关（openai 端点）：顶层对象参数 `thinking.type`，「enabled：开启思考模式，模型强制
///   先思考再回答」「disabled：关闭思考模式，模型直接回答问题，不进行思考」；另有
///   `reasoning_effort` 七档（none/minimal/low/medium/high/xhigh/max，「none：关闭思考」）。
///   文档无 `chat_template_kwargs`，故覆写 openai 臂（见下）。
///   https://www.volcengine.com/docs/82379/1494384
/// - 思考开关（anthropic 端点）：Claude Code 接入走「兼容 Anthropic Messages 接口协议的服务」
///   （「Claude Code 通过兼容 Anthropic Messages 接口协议的服务调用豆包搜索能力」），thinking
///   词表与基座 anthropic 臂同形，不覆盖。https://docs.volcengine.com/docs/ark/ai-tools-claude-code?lang=zh
/// - 思考开关（openai_responses 端点）：Responses API 同收 `thinking: {type}` 与
///   `reasoning.effort` 七档（「none：关闭思考」「minimal：关闭思考，直接回答」），基座
///   `reasoning: {"effort": "none"}` 官方可受，不覆盖。
///   https://docs.volcengine.com/docs/ark/responses-api-deep-thinking?lang=zh
/// - 流式（openai 端点）：`choices.delta.reasoning_content`（「思考内容原文」，seed 系列 +
///   deepseek-v4-pro/flash）、`choices.delta.encrypted_content`（思考加密原文，优先级高于
///   reasoning_content）、`delta.content`；usage 随 chunk 返回或经 `stream_options.include_usage`。
///   基座 openai 流式构件已按 delta.reasoning_content 抽取，不覆盖。
///   https://www.volcengine.com/docs/82379/1494384
/// - usage（openai 端点）：`prompt_tokens_details.cached_tokens`（「缓存命中的输入内容…所消耗
///   的 token 总数」）、`completion_tokens_details.reasoning_tokens`（「输出思维链内容花费的
///   token 数」）、`total_tokens`；基座 openai 构件已抽取 cached_tokens 并从 input 扣除，
///   不覆盖（cache_write 未在文档出现，无该键即基座按 0 处理）。
///   https://www.volcengine.com/docs/82379/1494384
/// - usage（openai_responses 端点）：`usage.output_tokens_details.reasoning_tokens`（「为原始
///   思考内容的 tokens，计费仍然按原始思考内容 token 计算」），与基座 responses 构件一致。
///   https://docs.volcengine.com/docs/ark/responses-api-deep-thinking?lang=zh
/// - 错误体：404 `NotFound.InvalidEndpointOrModel.NotFound`（「The model or endpoint %s does not
///   exist or you do not have access to it.」）——message 含 model + does not exist，命中基座
///   特征归模型类；401 `AuthenticationError` / 429 `RateLimitExceeded.*`（EndpointRPMExceeded /
///   ModelAccountRpmRateLimitExceeded 等）按状态码归服务类；错误体统一 JSON 形状未证实（错误码
///   页只列 HTTP状态码/Type/Code/Message 表格，无 JSON 示例），基座 `error.message/code/type` 与
///   顶层双形态读法已覆盖，不扩展。https://docs.volcengine.com/docs/ark/error-codes?lang=zh
/// - 工具 / 图片与多模态 / system：messages role 固定 system/user/assistant/tool；content 数组
///   块 `text` / `image_url`（含 file_id/detail） / `video_url` / `input_audio` / `file`（PDF），
///   tool 消息支持图片（方舟对 OpenAI 形状的兼容扩展）；`tools` type「当前仅支持 function」；
///   均为 OpenAI 标准形状 + 扩展，与基座构件一致，不覆盖。
///   https://www.volcengine.com/docs/82379/1494384
/// - 请求默认值：`max_tokens` 默认 4096（部分模型 131072）、`temperature` 默认 1.0、`top_p`
///   默认 0.7；aidog 不注入默认值（仅按模型上限裁剪超限值），不覆盖。
///   https://www.volcengine.com/docs/82379/1494384
/// - coding_plan 注入：官方 Claude Code 接入页只要求设 base_url 与 API Key（ANTHROPIC_BASE_URL /
///   ANTHROPIC_AUTH_TOKEN），无出站附加字段；基座按平台协议分派对 Doubao / BytePlus 协议为
///   no-op，行为不变。https://docs.volcengine.com/docs/ark/ai-tools-claude-code?lang=zh
/// - 未证实（保持基座默认）：错误体统一 JSON 形状、BytePlus 站正文与火山站的逐条一致性、
///   assistant 消息回传 `encrypted_content` 的网关侧处理（篡改报 Invalid signature，aidog
///   不改写历史消息，默认透传）、cache_write 类字段是否存在。
///
/// 成员 `byteplus`（BytePlus（字节跳动国际版）；issue 128，核对日期 2026-11-21。docs.byteplus.com
/// 正文 JS 渲染，逐页 Playwright 真渲染取正文，页面 Last updated 2026-11-19）：
/// - 端点（官方页原文，仅预填参考，运行时只用用户配置端点）：Chat API
///   `https://ark.ap-southeast.bytepluses.com/api/v3/chat/completions`（quick start base_url
///   `…/api/v3`）、Responses API 同 `…/api/v3`、Messages API（「Compatible with the Anthropic
///   Messages API protocol and can be used to integrate tools such as Claude Code」）
///   `…/api/compatible/v1/messages`。registry 内置默认 anthropic 端点写的是 `/api/plan`
///   （Coding Plan 形态），官方标准 Messages 端点为 `/api/compatible/v1/messages`——预填差异
///   不在本票范围改，出站不受影响。
///   https://docs.byteplus.com/en/docs/ModelArk/text-generation
///   https://docs.byteplus.com/en/docs/ModelArk/messages-api
///   https://docs.byteplus.com/en/docs/ModelArk/coding-plan
/// - 思考开关与档位：Chat API 顶层对象参数 `thinking.type`（示例 `"type":"disabled"` /
///   `"type":"enabled"`），`reasoning_effort`（Chat）/ `reasoning.effort`（Responses）七档
///   none/minimal/low/medium/high/xhigh/max，「none: Turn off reasoning」「minimal: Turns off
///   reasoning and answers directly」，全部支持该参数的模型收全七值（部分值按模型映射表折算）；
///   Messages API（anthropic wire）有 `thinking` object 参数（「Deep reasoning configuration」；
///   参数内部词表未逐字列出，未证实——基座 anthropic 臂不覆盖，词表细节保持基座行为）。
///   与家族基座一致，不覆盖。
///   https://docs.byteplus.com/en/docs/ModelArk/deep-thinking
///   https://docs.byteplus.com/en/docs/ModelArk/messages-api
/// - 流式：Chat API `data:` 帧流 + `data: [DONE]` 结束标记；`delta.reasoning_content`
///   （思考摘要）/ `delta.encrypted_content`（加密思考原文，传回时优先于 reasoning_content，
///   篡改报错，aidog 不改写历史消息默认透传）；usage 经 `stream_options.include_usage`
///   （`[DONE]` 前独立 usage 块）或 `stream_options.chunk_include_usage`（逐块累计值，客户端
///   只留最新值）返回；Responses API 事件流以 `response.completed`（status=completed）/
///   `response.incomplete` / `response.failed` / `error` 为语义终态；Messages API 以
///   `message_stop` 结束、「does not use [DONE] as a substitute」，终态看
///   `message_delta.delta.stop_reason`。基座流式构件按 [DONE] / typed events / anthropic
///   事件处理，一致，不覆盖。
///   https://docs.byteplus.com/en/docs/ModelArk/streaming-output
///   https://docs.byteplus.com/en/docs/ModelArk/deep-thinking
/// - usage：`usage.completion_tokens_details.reasoning_tokens`（「Tokens of the original
///   thinking content. Billing is still calculated based on the tokens of the original
///   thinking content」）；Messages API `usage.input_tokens / output_tokens /
///   cache_creation_input_tokens / cache_read_input_tokens`（Anthropic 标准四件，示例
///   cache_read_input_tokens: 139）；Chat API usage 完整字段表未在已抓取页面列出，
///   `prompt_tokens_details.cached_tokens` 在 BytePlus 未证实——基座抽取器有键即抽、无键按 0，
///   行为安全，不覆盖。与家族基座一致。
///   https://docs.byteplus.com/en/docs/ModelArk/deep-thinking
///   https://docs.byteplus.com/en/docs/ModelArk/messages-api
/// - 错误体：错误码页仅 HTTP 状态码 / Error type / Error code / Message 表格、无 JSON body
///   示例。404 `NotFound.InvalidEndpointOrModel.NotFound`（「The model or endpoint %s does not
///   exist or you do not have access to it.」——message 含 model + does not exist，命中基座
///   特征归模型类）；401 `AuthenticationError`、429 `RateLimitExceeded.EndpointRPMExceeded` 等
///   按状态码归服务类。与家族基座（火山站）错误码逐条同形，不覆盖；错误体统一 JSON 形状未证实，
///   基座 `error.message/code/type` 双形态读法已覆盖。
///   https://docs.byteplus.com/en/docs/ModelArk/error-codes
/// - 请求字段与默认值：`max_tokens` 默认 131072 (128K)（「controls the length of the model's
///   answer and chain-of-thought content. No error is reported if the length is exceeded」，
///   火山站记 4096（部分模型 131072）——成员差异仅记档，aidog 不注入默认值，无行为差异）；
///   messages role 含 system/user/assistant（multi-turn 示例）；`temperature` / `top_p`
///   默认值未在已抓取页面列出，未证实（aidog 不注入默认值）。工具调用走 Function call /
///   remote MCP 文档（tools 兼容 OpenAI 形状，Chat API 页「By integrating custom tools or
///   connecting to remote MCP servers」）；图片与音视频走多模态理解文档（image / video /
///   document / audio understanding 独立章节）；均为 OpenAI 标准形状 + 扩展，与基座构件一致，
///   不覆盖。
///   https://docs.byteplus.com/en/docs/ModelArk/text-generation
/// - coding_plan 注入：官方无出站附加字段（quick start 只设 base_url 与 API key）；基座按
///   平台协议分派对 BytePlus 协议为 no-op，行为不变。
///   https://docs.byteplus.com/en/docs/ModelArk/coding-plan
/// - 未证实（保持基座默认）：错误体统一 JSON 形状、Chat API `prompt_tokens_details.cached_tokens`
///   字段在 BytePlus 的存在性、`temperature` / `top_p` 默认值、system 顶层参数（Messages API
///   有 `system` 参数「System prompt」，Chat API 页 system 走 message role，基座两种 wire
///   各按协议构件处理）。
///
/// 成员 `doubao`（火山引擎（字节跳动）；issue 127）、`byteplus`（BytePlus；issue 128）：
/// 均登记为 [`ArkFamilyAdapter`]；registry 内置默认端点与官方一致（doubao
/// ark.cn-beijing.volces.com/api/plan[/v3]、byteplus ark.ap-southeast.bytepluses.com/api/plan[/v3]，
/// 协议 anthropic / openai / openai_responses），运行时只用用户平台配置的端点。既有平台特定行为
/// 不变（doubao coding 端点 anthropic wire 无条件剥离 redacted_thinking 块，见
/// aidog_core gateway/proxy/forward.rs）。成员级差异由子 issue 127 / 128 取证后覆盖。
pub struct ArkFamilyAdapter;

impl PlatformAdapter for ArkFamilyAdapter {
    /// 差异：openai 端点禁思考写官方顶层 `thinking: {"type": "disabled"}`，不写基座第三方端点用的
    /// vLLM 式 `chat_template_kwargs.enable_thinking`（方舟 Chat API 文档只列顶层 `thinking` 对象；
    /// 写 false 前同样剥掉客户端带来的旧 `thinking` / `reasoning_effort` 等开启型参数）。其余 wire
    /// 与强制思考路径同基座；`Protocol::OpenAICompletions` 不在覆写范围内（家族成员内置默认端点
    /// 仅 anthropic / openai / openai_responses，用户手配 openai_completions 端点仍走基座
    /// `chat_template_kwargs` 写法，与 GLM / 百炼基座同口径）。
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        if wire != &Protocol::OpenAI || forced_thinking {
            return hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking);
        }
        let Some(obj) = body.as_object_mut() else {
            return;
        };
        obj.remove("disable_thinking");
        if !disable {
            return;
        }
        for k in [
            "thinking",
            "context_management",
            "reasoning_effort",
            "reasoning",
            "enable_thinking",
        ] {
            obj.remove(k);
        }
        if let Some(ctk) = obj
            .get_mut("chat_template_kwargs")
            .and_then(|o| o.as_object_mut())
        {
            ctk.remove("enable_thinking");
        }
        if let Some(gc) = obj
            .get_mut("generationConfig")
            .and_then(|o| o.as_object_mut())
        {
            gc.remove("thinkingConfig");
        }
        obj.insert(
            "thinking".to_string(),
            serde_json::json!({"type": "disabled"}),
        );
    }
}

/// 百度千帆 家族基座（issue 80；成员 qianfan / qianfan_coding 共用，成员差异见各成员 issue）。
///
/// 家族共性取证（官方文档原文，核对日期 2026-11-19；cloud.baidu.com/doc/qianfan-api 为
/// Next.js 站、/doc/qianfan 为 Gatsby 站，正文均由原始 HTML 直读）：
/// - 思考开关（openai 端点）：文本生成 API 顶层对象参数 `thinking.type`——「思考模式类型，
///   默认值为 `disabled`。可选值：`enabled`：开启思考模式 / `disabled`：关闭思考模式」；
///   同层另有布尔参数 `enable_thinking`（「默认值为 false」，qwen3 系列与 ernie-4.5-vl 用，
///   deepseek/glm/kimi 系用 `thinking`，见深度思考文档支持清单）与 `thinking_budget` /
///   `thinking_strategy`。`reasoning_effort` 词表只有 `high`（默认）与 `max`
///   （「传入 low 或 medium 将映射为 high；传入 xhigh 将映射为 max」），**无 `none`**；
///   文档无 vLLM 式 `chat_template_kwargs`，故覆写 openai 臂（见下，写官方
///   `thinking: {"type": "disabled"}`，与基座 anthropic 臂同形）。不可关闭模型
///   （glm-5.3 / glm-5.3-flash 等「仅支持开启，不支持关闭」）由 registry
///   `thinking_toggleable: false` 走基座强制思考剥除、不写参数，不在此覆盖。
///   https://cloud.baidu.com/doc/qianfan-api/s/3m7of64lb
///   https://cloud.baidu.com/doc/qianfan-docs/s/Wm95lyynv
/// - 思考开关（anthropic 端点）：MaaS Anthropic 兼容 API 存在（官方公告对
///   「Anthropic API 返回参数 input_tokens」作语义更新），thinking 词表与基座 anthropic
///   臂同形，不覆盖；base_url 未在公开文档单独列出（registry 内置值
///   https://qianfan.baidubce.com/anthropic 仅用于新建平台预填，运行时只用用户配置端点）。
///   https://cloud.baidu.com/doc/qianfan/s/empm33i0q
/// - usage（openai 端点）：`usage.prompt_tokens_details.cached_tokens`（「触发上下文缓存后，
///   命中缓存的 token 数量」，文档注「该字段仅支持模型版本为 ERNIE-4.0-Turbo-8K」），
///   基座 openai 构件已抽取并从 input 扣除，不覆盖；`search_tokens`（检索增强膨胀 token）
///   基座按 0 处理（缓存/搜索均计入 prompt_tokens，aidog 计费不拆检索 token）。
///   `completion_tokens_details.reasoning_tokens`（思维链 token 消耗量）并入 output，同基座。
///   https://cloud.baidu.com/doc/qianfan-api/s/3m7of64lb
///   https://cloud.baidu.com/doc/qianfan-docs/s/Wm95lyynv
/// - usage（anthropic 端点）：官方公告「input_tokens = prompt_tokens - cache_read_input_tokens」
///   （与 Anthropic 定义对齐；命中缓存按 `cache_read_input_tokens` 折扣计费），与基座
///   anthropic 臂一致，不覆盖。https://cloud.baidu.com/doc/qianfan/s/empm33i0q
/// - 流式（openai 端点）：SSE 按 `data: [DONE]` 结束；`stream_options.include_usage`
///   （最后一个 chunk 输出 usage）与千帆扩展 `chunk_include_usage`（每包都带 usage）；
///   思维链走 `message.reasoning_content` / 流式 delta 同名字段。基座 openai 流式构件
///   已按 delta.reasoning_content 抽取，不覆盖；`delta_tag: search_status` 信号包
///   （无 content）基座按空 delta 处理，不覆盖。
///   https://cloud.baidu.com/doc/qianfan-api/s/3m7of64lb
/// - 错误体：`{"error":{"code","message","type"},"id":…}`，与基座 `error.message/code/type`
///   读法一致。400 `invalid_model`「model is empty」→ 基座 Unknown（调用方按模型类处理）；
///   **401** `invalid_request_error/invalid_model`「The model does not exist or you do not
///   have access to it.」——模型不存在挂 401，基座 401 先于业务码按状态码归服务类（换平台），
///   不覆盖（同 GLM 家族 1311 先例；复活条件：套餐用户反馈因模型配置错误换平台拖累平台
///   健康时重开）；403 `model_offline` / 429 `rate_limit_exceeded.*` 按状态码归服务类。
///   Coding Plan 专属码（401 `coding_plan_api_key_required` / `coding_plan_api_key_not_allowed`，
///   403 `coding_plan_not_subscribed` / `coding_plan_subscription_expired` /
///   `coding_plan_model_not_supported`，429 `coding_plan_hour/week/month_quota_exceeded` /
///   `coding_plan_rate_limit_exceeded` / `coding_plan_cluster_rate_limited`）全部落
///   401/403/429，按状态码归服务类，不扩展。https://cloud.baidu.com/doc/qianfan-api/s/Om9b4yj3w
/// - 工具 / 图片与多模态 / system：Chat Completions 标准 OpenAI 形状（`tools[].function`、
///   `tool_choice` none/auto/required + `{type:function,function:{name}}`、
///   `parallel_tool_calls`、`finish_reason: "tool_calls"`、`role: "system"`），
///   `tool_choice: "required"` 官方支持、不降级；图片理解是独立端点（视觉理解，单列 POST），
///   文本生成接口的 content 数组子属性仅列字符串项，图片块经转换分支的行为不在此覆盖。
///   https://cloud.baidu.com/doc/qianfan-api/s/3m7of64lb
/// - coding_plan 注入：官方 Claude Code 接入页（Coding Plan）只要求设 ANTHROPIC_BASE_URL
///   （https://qianfan.baidubce.com/anthropic/coding）与专属 API Key，无出站附加字段；
///   基座按平台协议分派对 Qianfan / QianfanCoding 协议为 no-op，行为不变。
///   https://cloud.baidu.com/doc/qianfan/s/0mn2mnemj
///   https://cloud.baidu.com/doc/qianfan/s/imlg0beiu （Base URL 与 registry 内置默认一致：
///   openai `https://qianfan.baidubce.com/v2/coding`、anthropic
///   `https://qianfan.baidubce.com/anthropic/coding`）
/// - 未证实（保持基座默认）：错误体在流式帧内的形态、`thinking` 参数对 enable_thinking 清单模型
///   （qwen3 系）的实际认受性（两参数均为 API 文档顶层参数，逐模型支持清单不同，成员差异归
///   各成员 issue）、max_tokens 统一上限、`content_filter` 类错误（`flag`/`finish_reason`
///   安全细分当前按基座处理）。
///
/// 成员 `qianfan`（百度千帆，标准 MaaS；成员 issue 待建）、`qianfan_coding`（百度千帆
/// Coding Plan Lite；成员 issue 待建）：均登记为 [`QianfanFamilyAdapter`]；registry 内置
/// 默认端点（qianfan anthropic `https://qianfan.baidubce.com/anthropic`；qianfan_coding
/// openai `https://qianfan.baidubce.com/v2/coding` + anthropic
/// `https://qianfan.baidubce.com/anthropic/coding`，`coding_plan: true`）与官方一致，
/// 运行时只用用户平台配置的端点。成员级差异由各成员 issue 取证后覆盖。
pub struct QianfanFamilyAdapter;

impl PlatformAdapter for QianfanFamilyAdapter {
    /// 差异：openai 端点禁思考写官方顶层 `thinking: {"type": "disabled"}`（API 文档默认值
    /// 即 `disabled`），不写基座第三方端点用的 vLLM 式 `chat_template_kwargs.enable_thinking`
    /// （千帆 Chat Completions 文档无此参数；写出值与基座 anthropic 臂相同，故按 anthropic 臂
    /// 执行）；写 false 前同样剥掉客户端带来的旧 `thinking` / `enable_thinking` /
    /// `reasoning_effort` 等开启型参数（`enable_thinking` 默认 false，qwen3 系模型官方开关）。
    /// 其余 wire 与强制思考路径同基座；`Protocol::OpenAICompletions` 不在覆写范围内（家族成员
    /// 内置默认端点仅 openai / anthropic，用户手配 openai_completions 端点仍走基座
    /// `chat_template_kwargs` 写法，与 GLM / 百炼 / 方舟基座同口径）。
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        let wire = match wire {
            Protocol::OpenAI => &Protocol::Anthropic,
            w => w,
        };
        hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking)
    }
}

/// SiliconFlow（硅基流动）家族基座（issue 82；成员 siliconflow / siliconflow_en 共用，
/// 成员差异见各成员子 issue）。
///
/// 家族共性取证（官方文档原文，核对日期 2026-11-23；docs.siliconflow.cn 与
/// docs.siliconflow.com 同一文档结构，本页为 Next.js 站、正文由原始 HTML 直读）：
/// - 思考开关（openai 端点）：顶层布尔参数 `enable_thinking`（「在推理模式与非推理模式之间切换。
///   该字段适用于大多数推理模型」），同层另有 `thinking_budget`（128–32768）与
///   `reasoning_effort`（仅 `"high"` / `"max"`，「low 和 medium 会映射为 high，xhigh 会映射为 max」，
///   适用于 DeepSeek-V4、GLM-5.2 等少数模型；**无 `none` 值**）。文档无 `thinking` 对象、
///   无 vLLM 式 `chat_template_kwargs`，故覆写 openai 臂（见下，写官方 `enable_thinking: false`，
///   与百炼基座同形）。默认值文档未列（「Value in false | true」无 default）——aidog 不注入
///   默认值，行为即官方默认。https://docs.siliconflow.cn/cn/api-reference/chat-completions/chat-completions
///   https://docs.siliconflow.com/en/api-reference/chat-completions/chat-completions
/// - 思考开关（anthropic 端点）：官方 Messages 端点 `https://api.siliconflow.cn/v1/messages`
///   （「创建对话请求（Anthropic）POST」），`thinking` 取 `{"type": "disabled"}` /
///   `{"type": "enabled"}`（「思维链控制参数。用于开启或关闭模型的思维链（thinking）功能」），
///   与基座 anthropic 臂同形，不覆盖。https://docs.siliconflow.cn/cn/docs/api/messages-post
/// - 流式（openai 端点）：SSE 以 `data: [DONE]` 结束（「Stream terminates with `data: [DONE]`」），
///   思考增量在 `reasoning_content`；基座 openai 流式构件已按 delta.reasoning_content 抽取，
///   不覆盖。https://docs.siliconflow.com/en/api-reference/chat-completions/chat-completions
/// - usage（openai 端点）：`usage.prompt_tokens_details.cached_tokens`，另有 DeepSeek 系
///   `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`；基座构件两者都已抽取并从 input
///   扣除（`usage.rs`），不覆盖。
/// - usage（anthropic 端点）：`cache_read_input_tokens` / `cache_creation_input_tokens`
///   （Anthropic 标准两件），与基座 anthropic 臂一致，不覆盖。
///   https://docs.siliconflow.cn/cn/docs/api/messages-post
/// - 错误体：顶层 JSON `{"code": <int>, "message": ..., "data": null}`（无 `error` 包裹；
///   示例 `code: 20012`「Model does not exist. Please check it carefully.」）——基座
///   `error.message/code/type` 读法对无 `error` 包裹的 body 读顶层，message 含
///   model + does not exist 命中模型类特征；401（API Key 未设置）/ 402（欠费）/
///   403（权限/实名认证）/ 429（rate limits）按状态码归服务类。基座双形态读法已覆盖，
///   不扩展。https://docs.siliconflow.cn/cn/docs/userguide/faqs/error-code
/// - 工具 / 图片与多模态 / system：Chat Completions 标准 OpenAI 形状（`tools[].function`、
///   `tool_choice`、`image_url`、`role: "system"`），Messages 页同样支持 system / tools /
///   `tool_choice`（Auto | Tool | None + disable_parallel_tool_use）；均与基座构件一致，不覆盖。
///   https://docs.siliconflow.cn/cn/api-reference/chat-completions/chat-completions
///   https://docs.siliconflow.cn/cn/docs/api/messages-post
/// - coding_plan 注入：两成员均非编码套餐，基座按平台协议分派对 SiliconFlow / SiliconFlowEn
///   协议为 no-op，行为不变。
/// - 未证实（保持基座默认）：`enable_thinking` 默认值、max_tokens 统一上限、
///   `reasoning_effort` 对清单外模型的认受性、错误体在流式帧内的形态。
pub struct SiliconFlowFamilyAdapter;

impl PlatformAdapter for SiliconFlowFamilyAdapter {
    /// 差异：openai 端点禁思考写官方顶层布尔 `enable_thinking: false`，不写基座第三方端点用的
    /// vLLM 式 `chat_template_kwargs.enable_thinking`（SiliconFlow Chat Completions 文档无此
    /// 参数；写 false 前同样剥掉客户端带来的旧 `enable_thinking` / `thinking` /
    /// `reasoning_effort` 等开启型参数）。其余 wire 与强制思考路径同基座；
    /// `Protocol::OpenAICompletions` 不在覆写范围内（家族成员内置默认端点仅 openai /
    /// anthropic，用户手配 openai_completions 端点仍走基座 `chat_template_kwargs` 写法，
    /// 与 GLM / 百炼 / 方舟 / 千帆基座同口径）。
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        if wire != &Protocol::OpenAI || forced_thinking {
            return hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking);
        }
        let Some(obj) = body.as_object_mut() else {
            return;
        };
        obj.remove("disable_thinking");
        if !disable {
            return;
        }
        for k in [
            "thinking",
            "context_management",
            "reasoning_effort",
            "reasoning",
            "enable_thinking",
        ] {
            obj.remove(k);
        }
        if let Some(ctk) = obj
            .get_mut("chat_template_kwargs")
            .and_then(|o| o.as_object_mut())
        {
            ctk.remove("enable_thinking");
        }
        if let Some(gc) = obj
            .get_mut("generationConfig")
            .and_then(|o| o.as_object_mut())
        {
            gc.remove("thinkingConfig");
        }
        obj.insert("enable_thinking".to_string(), Value::Bool(false));
    }
}

/// StepFun（阶跃星辰）家族基座（issue 83；成员 stepfun / stepfun_en 共用，成员差异见各成员子
/// issue）。
///
/// 家族共性取证（官方文档原文，核对日期 2026-11-23；中国站 platform.stepfun.com 与国际站
/// platform.stepfun.ai 文档结构一致，均 Mintlify 站、正文走 `llms.txt` 列出的 `.md` 原文页）：
/// - Chat Completions（openai 端点）：OpenAI 兼容形状，`POST {base}/v1/chat/completions`；
///   system 用 `role: "system"`，工具 `tools` / `tool_choice`，图片 / 文档输入在
///   `messages[].content` 多模态块（`image_url` / 文档块）；请求参数按官方定义处理，思考字段详见下文。
///   https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create.md
///   https://platform.stepfun.ai/docs/en/api-reference/chat/chat-completion-create.md
/// - 思考开关（openai 端点）：推理深度用顶层 `reasoning_effort`（`low` / `medium` / `high`，
///   `step-3.5-flash-2603` 兼容 `low`/`high`；「值越高，模型会进行更深入的思考」）——**文档无
///   关闭思考的参数**（无 `none` 值；全文 grep 无 `enable_thinking` / `chat_template_kwargs`）。
///   禁思考覆写 openai 臂只剥开启型参数、不注入任何关闭参数（官方无该形态，写
///   `chat_template_kwargs.enable_thinking=false` 是未经证实的第三方形态，有 400 风险；
///   剥后模型按官方默认行为运行）；强制思考路径同基座（只剥不写）。
///   https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create.md
/// - 思考开关（anthropic 端点）：Messages API 参数表（官方请求参数逐字段列表）思考深度仅
///   `output_config.effort`（`low` / `medium` / `high`；`step-3.5-flash-2603` 兼容
///   `low`/`high`；step-router-v1 下该字段「会被忽略」）——参数表**无 `thinking` 对象**。
///   `thinking` 对象认受性未证实，按票面「未证实保持框架默认」：anthropic 臂不覆盖基座
///   （基座写 Anthropic 标准形 `thinking.type=disabled`）；仅额外剥已证实的
///   `output_config.effort`（禁思考时档位参数与禁用矛盾，剥 effort、保留同对象其他字段）。
///   https://platform.stepfun.com/docs/zh/api-reference/chat/messages-create.md
/// - 思考内容字段（openai 端点）：请求参数 `reasoning_format` **默认 `general`**——「使用
///   `reasoning` 字段返回结果」；`deepseek-style` 才用 `reasoning_content`。响应格式页
///   `message` / `delta` 同时列出 `reasoning` 与 `reasoning_content`（「一同返回且内容一致
///   （DeepSeek 兼容字段名），仅在 step 推理模型下返回」）。基座 openai 构件只读
///   reasoning_content，适配器在转换前把缺 reasoning_content 的 message/delta 用 reasoning
///   补齐（已带 reasoning_content 的帧不动，与通用构件逐字节等价）。
///   https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create.md
/// - 流式（openai 端点）：SSE `data: {chunk}` 帧、`data: [DONE]` 结束；思考增量字段同上
///   （`reasoning` / `reasoning_content`，经补齐后基座构件按 delta.reasoning_content 抽取）。
///   https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create.md
/// - 流式（anthropic 端点）：Messages API「支持流式输出」，Anthropic 兼容事件流（基座
///   anthropic 流式构件），不覆盖。https://platform.stepfun.com/docs/zh/api-reference/chat/messages-create.md
/// - usage（openai 端点）：`usage.prompt_tokens / completion_tokens / total_tokens`（流式帧
///   逐帧携带）+ `usage.prompt_tokens_details.cached_tokens`（响应格式页非流式与流式两臂
///   均列出，「提示信息中命中缓存的 token 数量」；官方示例含 `prompt_tokens_details: null`
///   形态）——基座 usage 构件已按 cached_tokens 抽取并从 input 扣除，不覆盖。
///   https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create.md
/// - usage（anthropic 端点）：Anthropic 兼容 `input_tokens / output_tokens`；缓存两件未见
///   （官方响应示例仅 input/output）——「未证实」，基座构件读不到即缺省，不覆盖。
///   https://platform.stepfun.com/docs/zh/api-reference/chat/messages-create.md
/// - 错误体：错误码按 HTTP 状态码分（400 `request_params_invalid` / 图片下载失败 /
///   模型不存在等；401 认证；429 限频），`unsupported_content_type` 等业务码以字符串出现在
///   错误信息中——基座 `error.message/code/type` 读法覆盖（模型不存在命中模型类特征，
///   401/402/403/429 按状态码归服务类），不扩展。
///   https://platform.stepfun.com/docs/zh/api-reference/error-codes.md
/// - 工具 / 图片与多模态 / system：Chat Completions 标准 OpenAI 形状；Messages API 支持
///   system 顶层字段、工具调用、图片输入（「支持流式输出、工具调用、图片输入与思考深度配置」）
///   ——均与基座构件一致，不覆盖。
///   https://platform.stepfun.com/docs/zh/api-reference/chat/messages-create.md
/// - coding_plan 注入：两成员均非编码套餐（`step_plan` 通道是套餐路径但平台 code 非
///   `*_coding`），基座按平台协议分派对 StepFun / StepFunEn 协议为 no-op，行为不变。
/// - 未证实（保持基座默认/不注入）：`reasoning_effort` 默认档位、anthropic 端点 `thinking`
///   对象认受性（按票面保持框架默认）、错误体在流式帧内的形态、`step_plan` 通道 `max_tokens`
///   64K 上限之外的字段约束、非推理模型收到禁思考时是否需要显式参数（官方无关闭形态可写，
///   剥净即默认）。
pub struct StepFunFamilyAdapter;

/// StepFun 默认 `reasoning_format: general`：思考内容在 `reasoning` 字段（`deepseek-style`
/// 才写 `reasoning_content`；官方响应格式页两字段并存且「内容一致」）。基座 openai 构件只读
/// reasoning_content，转换前把缺 reasoning_content 的 message/delta 用 reasoning 补齐；
/// 已带 reasoning_content 的帧不动，保持与通用构件逐字节等价。
fn stepfun_normalize_reasoning(body: &Value) -> Value {
    let mut out = body.clone();
    let Some(choices) = out.get_mut("choices").and_then(Value::as_array_mut) else {
        return out;
    };
    for choice in choices {
        for key in ["message", "delta"] {
            if let Some(slot) = choice.get_mut(key).and_then(Value::as_object_mut) {
                let has_rc = slot
                    .get("reasoning_content")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty());
                if !has_rc
                    && let Some(r) = slot
                        .get("reasoning")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                {
                    let r = r.to_string();
                    slot.insert("reasoning_content".to_string(), Value::String(r));
                }
            }
        }
    }
    out
}

impl PlatformAdapter for StepFunFamilyAdapter {
    /// 差异一：openai 端点禁思考只剥开启型参数、不注入关闭参数——官方 Chat Completions
    /// 文档无关闭形态（`reasoning_effort` 仅 low/medium/high，全文无 `enable_thinking` /
    /// `chat_template_kwargs`），基座非官方 host 的 `chat_template_kwargs.enable_thinking=false`
    /// 是未经证实的第三方形态。强制思考路径与其余 wire（openai_completions /
    /// openai_responses 等用户手配端点）同基座。
    /// 差异二：anthropic 端点保持框架默认（基座写 `thinking.type=disabled`——`thinking` 对象
    /// 在官方参数表未列出、认受性未证实，按票面「未证实保持框架默认」），仅额外剥已证实的
    /// `output_config.effort` 思考档位（剥 effort、保留同对象其他字段）。
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        if wire != &Protocol::OpenAI {
            hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking);
            if disable && !forced_thinking
                && let Some(obj) = body.as_object_mut()
            {
                let config_empty = obj
                    .get_mut("output_config")
                    .and_then(Value::as_object_mut)
                    .map(|config| {
                        config.remove("effort");
                        config.is_empty()
                    })
                    .unwrap_or(false);
                if config_empty {
                    obj.remove("output_config");
                }
            }
            return;
        }
        if forced_thinking || !disable {
            return hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking);
        }
        let Some(obj) = body.as_object_mut() else {
            return;
        };
        obj.remove("disable_thinking");
        for k in [
            "thinking",
            "context_management",
            "reasoning_effort",
            "reasoning",
            "enable_thinking",
        ] {
            obj.remove(k);
        }
        if let Some(ctk) = obj
            .get_mut("chat_template_kwargs")
            .and_then(|o| o.as_object_mut())
        {
            ctk.remove("enable_thinking");
        }
        if let Some(gc) = obj
            .get_mut("generationConfig")
            .and_then(|o| o.as_object_mut())
        {
            gc.remove("thinkingConfig");
        }
    }

    /// 差异三：openai 端点响应/流式的思考内容在 `reasoning` 字段（`reasoning_format` 默认
    /// `general`），基座构件只读 reasoning_content——转换前补齐（见
    /// [`stepfun_normalize_reasoning`]）；其余 wire 原样走基座构件。
    fn convert_response(
        &self,
        body: &Value,
        wire: &Protocol,
        client: &Protocol,
        model: &str,
    ) -> Option<Value> {
        if wire == &Protocol::OpenAI {
            return converter::convert_response(&stepfun_normalize_reasoning(body), wire, client, model);
        }
        converter::convert_response(body, wire, client, model)
    }

    fn parse_sse_frame(&self, data: &Value, wire: &Protocol) -> Vec<crate::types::ChatStreamEvent> {
        if wire == &Protocol::OpenAI {
            return converter::parse_sse(&stepfun_normalize_reasoning(data), wire);
        }
        converter::parse_sse(data, wire)
    }
}

/// 腾讯云家族基座（issue 84；成员 tencent_coding / tencent_tokenhub / tencent_tokenhub_intl
/// 共用，成员差异见各成员 issue 137 / 138 / 139）。
///
/// 家族共性取证（官方文档原文，核对日期 2026-11-23；TokenHub 国内站与国际站同一组文档，
/// 仅 Base URL 二选一）：
/// - 端点：TokenHub 统一三协议同组 Base URL（广州 `https://tokenhub.tencentmaas.com/v1` /
///   新加坡 `https://tokenhub-intl.tencentmaas.com/v1`），按路径切换协议：openai
///   `/v1/chat/completions`、anthropic `/v1/messages`；与 registry 内置默认一致，运行时只用
///   用户平台配置的端点。Coding Plan（`tencent_coding`）专属 Base URL：openai
///   `https://api.lkeap.cloud.tencent.com/coding/v3`、anthropic
///   `https://api.lkeap.cloud.tencent.com/coding/anthropic`（与预付费 `sk-xxxx` Key 和
///   旧 Base URL 不互通；专属 Key 格式 `sk-sp-xxxx`）。
///   https://cloud.tencent.com/document/product/1823/130079
///   https://cloud.tencent.com/document/product/1823/130092
/// - 鉴权：openai 路径 `Authorization: Bearer`，anthropic 路径 `x-api-key`
///   （平台同时兼容 `Authorization: Bearer`）；网关层注入，适配器不涉。
///   https://cloud.tencent.com/document/product/1823/130079
///   https://cloud.tencent.com/document/product/1823/135874
/// - 思考开关（openai 端点）：**已证实差异**——平台思考模式参数表以 `thinking` 对象为
///   「标准方式」，`thinking.type` 取 `"enabled"` / `"disabled"` / `"adaptive"`（仅给
///   `budget_tokens` 缺 `type` 返 400）；`enable_thinking` 布尔只是部分模型（Qwen、DeepSeek
///   系）的「简化开关」。工具调用节进一步明确：思考开启时 `tool_choice:"required"` 返 400，
///   「需显式传入 `thinking: {"type": "disabled"}` 后使用」——即平台认收的禁思考写法。
///   文档无 `chat_template_kwargs`（基座第三方写法未经本平台证实）。覆写 openai 臂：剥
///   开启型参数（含平台扩展 `thinking_budget` / `interleaved_thinking`）后写
///   `thinking: {"type": "disabled"}`。`Protocol::OpenAICompletions` 不在覆写范围（成员
///   内置默认端点仅 openai / anthropic，用户手配端点仍走基座 `chat_template_kwargs`）。
///   https://cloud.tencent.com/document/product/1823/135872
/// - 思考开关（anthropic 端点）：Messages 协议 `thinking` 对象为标准参数
///   （`enabled` / `disabled` / `adaptive` + `budget_tokens`），与基座 anthropic 臂
///   `thinking.type=disabled` 一致，不覆盖。
///   https://cloud.tencent.com/document/product/1823/135874
/// - 思考内容（openai 端点）：响应 `message.reasoning_content` / 流式
///   `delta.reasoning_content`（先于 content 输出），多轮需原样回填；带签名的分块形态走
///   `reasoning_details` 数组——基座 openai 构件读 `reasoning_content`，不覆盖；
///   `reasoning_details` 在转换分支（anthropic 客户端）无对应块、原样透传分支不受影响。
///   https://cloud.tencent.com/document/product/1823/135872
/// - usage（openai 端点）：`prompt_tokens / completion_tokens / total_tokens`（后者 =
///   前两者之和，completion 已含思维链）；缓存字段两形态并存——顶层 `cache_read_tokens` /
///   `cache_write_tokens`（「部分模型，可减少计费」）与 `prompt_tokens_details.cached_tokens`
///   （prompt_cache_key 节指认的命中查看口）。**顶层两字段与 `prompt_tokens` 的包含关系
///   文档未写**，按票面「未证实保持框架默认」：基座 openai 构件只读
///   `prompt_tokens_details.cached_tokens`（并从 input 扣除），不覆盖顶层字段（复活条件：
///   官方明确包含关系或成员 issue 取证实测后评估）。
///   https://cloud.tencent.com/document/product/1823/135872
/// - usage（anthropic 端点）：`input_tokens / output_tokens` + `cache_creation_input_tokens` /
///   `cache_read_input_tokens`，「三者互斥、真实总输入 = 三者之和」——与基座 anthropic 臂
///   一致（不扣 input），不覆盖。缓存明细（分 TTL 等）官方自述「普遍不完整」。
///   https://cloud.tencent.com/document/product/1823/135874
/// - 流式：openai 端点 SSE `data: {chunk}`、`data: [DONE]` 结束，`stream_options.include_usage`
///   控制末 chunk 携带 usage；错误帧为流中 `data: {"error":{...}}` 后跟 `[DONE]`（客户端侧
///   检测，基座构件不覆盖）。anthropic 端点标准事件链
///   `message_start → content_block_* → message_delta → message_stop`，同基座。
///   https://cloud.tencent.com/document/product/1823/135872
///   https://cloud.tencent.com/document/product/1823/135874
/// - 错误体（openai 端点）：`{"error":{"message","message_zh","code","type","request_id"}}`，
///   业务码 6 位数字、前三位对应 HTTP 状态（400xxx / 401xxx / 429xxx）；早期拦截错误 `type`
///   统一 `gateway_error`；`code` 可能为整型（429 限频场景，携 `Retry-After` 头）。
///   基座分类按状态码归 401/402/403/429 → Service，模型类特征靠 message 文案；TokenHub
///   未列「模型不存在」的确定性文案/状态（`402xxx` 为「模型服务未开通或额度不足」，按
///   状态码已归 Service），不扩展特征。
///   https://cloud.tencent.com/document/product/1823/130079
///   https://cloud.tencent.com/document/product/1823/135872
/// - 工具 / 图片与多模态 / system：openai 端点标准形状（`tools[].function`、`tool_choice`
///   字符串/对象、`image_url` / `video_url` / `file_url` 内容块、`role: "system"`）；anthropic
///   端点标准 Messages 形状（顶层 `system`、`input_schema`、`tool_result` 块、image/video/
///   document 块）——均与基座构件一致，不覆盖。Coding Plan「暂不支持多模态能力」，
///   模型 `tc-code-latest`，属成员 issue 137 的模型层约束，基座不按平台剥块。
///   https://cloud.tencent.com/document/product/1823/135872
///   https://cloud.tencent.com/document/product/1823/135874
///   https://cloud.tencent.com/document/product/1823/130092
/// - coding_plan 注入：Coding Plan 文档只要求专属 Key + Base URL，未列任何必填请求字段
///   （无 prompt_cache_key 类要求），基座按平台协议分派对 `TencentCoding` 为 no-op，行为不变。
///   https://cloud.tencent.com/document/product/1823/130092
/// - 未证实（保持基座默认）：顶层 `cache_read_tokens` / `cache_write_tokens` 与
///   `prompt_tokens` 的包含关系、错误体在流式帧内的完整 JSON 形状（官方只给外层
///   `{"error":{...}}`）、anthropic 端点非 Claude 系模型对 `signature` 回传的校验强度
///   （官方自述按模型而异）。
pub struct TencentFamilyAdapter {
    /// TokenHub 两成员（官方 135872 取证 `thinking.type` 标准形态）openai wire 禁思考写
    /// `thinking: {"type":"disabled"}`；Coding Plan 官方文档（130092）未列思考参数、模型库
    /// 动态——未证实范围保持框架默认（false），成员 issue 137 取证后再收窄。
    official_openai_thinking: bool,
}

impl PlatformAdapter for TencentFamilyAdapter {
    /// 差异（仅 TokenHub 两成员，`official_openai_thinking: true`）：openai 端点禁思考写平台
    /// 标准 `thinking: {"type": "disabled"}`，不写基座第三方端点用的 vLLM 式
    /// `chat_template_kwargs.enable_thinking`（TokenHub 文档无此参数；`enable_thinking` 布尔
    /// 仅是部分模型的「简化开关」，统一标准形态是 `thinking.type`）。一并剥平台扩展的
    /// `thinking_budget` / `interleaved_thinking`（开启型/思考态参数，与显式禁用矛盾）。
    /// 其余 wire、强制思考路径与 `official_openai_thinking: false`（Coding Plan，官方未列
    /// 思考参数，130092）同基座框架默认。
    /// https://cloud.tencent.com/document/product/1823/135872
    /// https://cloud.tencent.com/document/product/1823/130092
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        if wire != &Protocol::OpenAI || forced_thinking || !self.official_openai_thinking {
            return hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking);
        }
        let Some(obj) = body.as_object_mut() else {
            return;
        };
        obj.remove("disable_thinking");
        if !disable {
            return;
        }
        for k in [
            "thinking",
            "context_management",
            "reasoning_effort",
            "reasoning",
            "enable_thinking",
            "thinking_budget",
            "interleaved_thinking",
        ] {
            obj.remove(k);
        }
        if let Some(ctk) = obj
            .get_mut("chat_template_kwargs")
            .and_then(|o| o.as_object_mut())
        {
            ctk.remove("enable_thinking");
        }
        if let Some(gc) = obj
            .get_mut("generationConfig")
            .and_then(|o| o.as_object_mut())
        {
            gc.remove("thinkingConfig");
        }
        obj.insert(
            "thinking".to_string(),
            serde_json::json!({"type": "disabled"}),
        );
    }
}

/// 小米 MiMo 家族基座（issue 85；成员 xiaomi_mimo / xiaomi_mimo_coding /
/// xiaomi_mimo_coding_en 共用，成员差异见各成员 issue）。
///
/// 家族共性取证（官方文档 `.md` 原文页，核对日期 2026-11-23；本地留档
/// `.tmp/issue-85/docs/`）：
/// - 端点：openai `https://api.xiaomimimo.com/v1/chat/completions`、anthropic
///   `https://api.xiaomimimo.com/anthropic/v1/messages`（registry 内置默认 base_url
///   `…/v1` / `…/anthropic` 与官方一致）；Token Plan 套餐三集群
///   `token-plan-cn|sgp|ams.xiaomimimo.com` 的 `/v1` 与 `/anthropic`（订阅页）。
///   鉴权 `api-key` 头与 `Authorization: Bearer` 双形态官方均认收（网关注入 Bearer），
///   网关层注入，适配器不涉。运行时只用用户平台配置的端点。
///   https://mimo.mi.com/static/docs/api/chat/openai-api-post.md
///   https://mimo.mi.com/static/docs/api/chat/anthropic-api-post.md
///   https://mimo.mi.com/docs/zh-CN/tokenplan/Token%20Plan/subscription
/// - 思考开关（openai 端点）：顶层对象参数 `thinking.type`——「Set the `thinking.type`
///   parameter in the request to control deep thinking: `enabled` to enable deep thinking
///   or `disabled` to disable deep thinking」（官方示例 `extra_body={"thinking":{"type":"disabled"}}`）；
///   全文 grep 无 `chat_template_kwargs` / `enable_thinking` / `reasoning_effort`（基座第三方
///   写法未经本平台证实）。已证实差异 → 覆写 openai 臂按基座 anthropic 臂执行
///   （写 `thinking.type=disabled`）。`Protocol::OpenAICompletions` 不在覆写范围（家族成员
///   内置默认端点仅 openai / anthropic，用户手配端点仍走基座写法，与 GLM / 百炼 / 方舟 /
///   千帆基座同口径）。
///   https://mimo.mi.com/static/docs/quick-start/usage-guide/text-generation/deep-thinking.md
/// - 思考开关（anthropic 端点）：参数表 `thinking` 对象 `type` 取 `enabled` / `disabled`
///   （v2.6 系默认 `enabled`），与基座 anthropic 臂同形，不覆盖。
///   https://mimo.mi.com/static/docs/api/chat/anthropic-api-post.md
/// - temperature / top_p：思考模式下 v2.6 系「不支持自定义，即使传入也会被强制覆盖为推荐
///   默认值 1.0 / 0.95」——静默覆盖**不报错**（与 Kimi「传入其他值会报错」不同），禁思考
///   不剥 temperature（传了无害）。https://mimo.mi.com/static/docs/api/guidance/model-hyperparameters.md
/// - reasoning_content（openai 端点）：`message.reasoning_content` / 流式
///   `delta.reasoning_content`；多轮工具调用 thinking 模式「历史 assistant 消息必须完整回传
///   `reasoning_content`，否则 API 返回 400」——anthropic 客户端回灌的 thinking 块在
///   openai 转换中被跳过 → assistant tool-call 消息缺字段 → 上游 400，与 Kimi Code
///   （issue 119）同形。按官方证实证据开启 [`hooks::carry_history_reasoning`]（仅
///   `xiaomi_mimo`；套餐成员官方文档未列 API 行为，不外推，见成员节）。
///   https://mimo.mi.com/static/docs/quick-start/usage-guide/text-generation/deep-thinking.md
///   https://mimo.mi.com/static/docs/api/guidance/error-codes.md
/// - 流式（openai 端点）：`data: {chunk}` 帧、`data: [DONE]` 结束；末帧 `choices` 空、
///   顶层带 `usage`（官方示例，请求侧未出现 `stream_options.include_usage`）——基座构件
///   逐帧解析一致，不覆盖。
///   https://mimo.mi.com/static/docs/quick-start/usage-guide/text-generation/deep-thinking.md
/// - usage（openai 端点）：`prompt_tokens_details.cached_tokens`（「Number of tokens served
///   from cache」）与 `completion_tokens_details.reasoning_tokens`——基座构件已抽取
///   cached_tokens 并从 input 扣除、reasoning_tokens 并入 output，不覆盖。
///   https://mimo.mi.com/static/docs/api/chat/openai-api-post.md
/// - usage（anthropic 端点）：`input_tokens / output_tokens / cache_read_input_tokens`
///   （**无 `cache_creation_input_tokens`**，全文零命中）——基座构件读不到即缺省 0，
///   不覆盖。https://mimo.mi.com/static/docs/api/chat/anthropic-api-post.md
/// - 错误体：错误码页仅「400 / 401 / 429 + 原因 / 解决建议」表格，**无 JSON 错误体形状
///   示例**——未证实，不扩展错误特征、不在测试伪造模型类契约；401 / 429 按状态码归
///   服务类（换平台）。https://mimo.mi.com/static/docs/api/guidance/error-codes.md
/// - 工具 / 图片与多模态 / system：openai 端点标准形状（`tools` / `tool_choice`、
///   `image_url` 等、`role: "system"`）；anthropic 端点标准 Messages 形状（顶层 `system`、
///   `input_schema`、image/audio/video 内容块、thinking 块带 `signature` 回传）——均与
///   基座构件一致，不覆盖。tool_choice 两端点官方均只列 `auto`，且「传非 auto 值后端
///   默认移除该字段、行为等价 auto」（不报错）→ 不降级、不覆盖 [`PlatformAdapter::adjust_tool_choice`]。
///   https://mimo.mi.com/static/docs/api/chat/openai-api-post.md
///   https://mimo.mi.com/static/docs/api/chat/anthropic-api-post.md
/// - coding_plan 注入：Token Plan 订阅页只给套餐专属 Key（`tp-xxxxx`）与 Base URL，未列
///   必填请求字段（无 prompt_cache_key 类要求）；基座按平台协议分派对 XiaomiMimoCoding /
///   XiaomiMimoCodingEn 为 no-op，行为不变。
///   https://mimo.mi.com/docs/zh-CN/tokenplan/Token%20Plan/subscription
/// - 未证实（保持基座默认）：错误体 JSON 形状与「模型不存在」确定性文案、openai 端点对
///   基座出站键 `max_tokens` 的认受性（官方参数表只列 `max_completion_tokens`；基座 openai
///   构件出站统一写 `max_tokens` 第三方通行写法，不改名；复活条件：实测 400 / 被忽略时
///   评估换名）、`stream_options.include_usage` 认受性、anthropic 端点 signature 回传校验
///   强度、max_tokens 统一上限。
///
/// 成员 `xiaomi_mimo`（小米 MiMo，api.xiaomimimo.com 标准平台；成员 issue 待建）：
/// 本成员官方文档即家族取证来源，登记为 [`MimoFamilyAdapter`] 全量开启家族覆写
/// （`official_openai_thinking` / `carry_history_reasoning_openai` 均 true）。
///
/// 成员 `xiaomi_mimo_coding`（小米 MiMo 套餐（小米），token-plan-cn/sgp/ams；成员 issue 待建）、
/// `xiaomi_mimo_coding_en`（小米 MiMo 套餐国际版，token-plan-sgp/ams；成员 issue 待建）：
/// 官方文档（订阅页）只给套餐专属 Key 与三集群 Base URL，未列任何 API 请求参数与行为
/// 差异——未证实范围按票面保持框架默认（`official_openai_thinking: false` → openai wire
/// 禁思考走基座 vLLM 式 `chat_template_kwargs.enable_thinking=false`；`carry_history_reasoning_openai:
/// false`），成员 issue 取证后收窄。腾讯 Coding Plan（issue 84 `official_openai_thinking: false`）
/// 同口径先例。registry 端点标 `coding_plan: true`，基座注入对两协议为 no-op。
pub struct MimoFamilyAdapter {
    /// openai 端点禁思考是否写官方 `thinking: {"type":"disabled"}`（仅 `xiaomi_mimo` 为
    /// true——标准平台官方证实；套餐两成员官方未列 API 参数，保持框架默认）。
    official_openai_thinking: bool,
    /// openai wire 多轮工具历史是否携带 reasoning_content（官方 400 校验；仅 `xiaomi_mimo`
    /// 为 true，套餐成员不外推）。https://mimo.mi.com/static/docs/api/guidance/error-codes.md
    carry_history_reasoning_openai: bool,
}

impl PlatformAdapter for MimoFamilyAdapter {
    /// 差异（仅 `carry_history_reasoning_openai` 成员，openai wire）：多轮工具历史的
    /// assistant tool-call 消息携带 reasoning_content（官方 400 校验，见
    /// [`hooks::carry_history_reasoning`]）。
    fn build_request(
        &self,
        chat_req: &crate::types::ChatRequest,
        wire: &Protocol,
        platform: &Protocol,
    ) -> (Value, String) {
        let (mut body, path) = converter::convert_request(chat_req, wire, platform);
        if self.carry_history_reasoning_openai && wire == &Protocol::OpenAI {
            hooks::carry_history_reasoning(&mut body, chat_req);
        }
        (body, path)
    }

    /// 差异（仅 `official_openai_thinking` 成员）：openai 端点禁思考写官方
    /// `thinking: {"type": "disabled"}`，不写基座第三方端点用的 vLLM 式
    /// `chat_template_kwargs.enable_thinking`（MiMo 官方 API 文档无此参数）。写出值与
    /// 基座 anthropic 臂相同，故按 anthropic 臂执行；禁思考**不剥** `temperature` /
    /// `top_p`（官方：思考模式传入会被静默覆盖为 1.0 / 0.95，不报错，与 Kimi 报错语义
    /// 不同）。其余 wire 与强制思考路径同基座。
    /// https://mimo.mi.com/static/docs/quick-start/usage-guide/text-generation/deep-thinking.md
    /// https://mimo.mi.com/static/docs/api/guidance/model-hyperparameters.md
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        let wire = match wire {
            Protocol::OpenAI if self.official_openai_thinking => &Protocol::Anthropic,
            w => w,
        };
        hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking)
    }
}

/// OpenCode 家族基座（issue 87；成员 opencode / opencode_zen 共用；成员工作项 #145 / #146）。
///
/// 官方文档核对日期：2026-11-23。
/// - 请求：OpenCode Console 文档给出 OpenAI Chat Completions 的 JSON 请求，含 `model` 与
///   `messages`。Go 与 Zen 文档说明模型有原生 API，文本模型也可经其他 API 并由 Console 转换；
///   其它请求字段和默认值未证实，保持 OpenAI 构件默认行为。
///   https://opencode.ai/docs/console/api/inference
///   https://opencode.ai/docs/console/go/ https://opencode.ai/docs/zen/
/// - 思考参数、工具调用、图片 / 音视频能力、system prompt、max token 限制：成员端点的专属行为
///   未证实，保持框架默认行为。
///   https://opencode.ai/docs/console/go/ https://opencode.ai/docs/zen/
/// - Usage：Console inference 文档展示 OpenAI `usage` 与额外的 `opencode` 信息，并说明启用
///   `stream_options.include_usage` 时流末 usage chunk 携带统计。文档未证明 `/zen/go/v1` 与
///   `/zen/v1` 的缓存 token 字段或计数语义，保持框架默认 usage 抽取。
///   https://opencode.ai/docs/console/api/inference
/// - 错误体与 SSE 事件 / 结束标记：成员端点专属格式未证实，保持框架默认处理。
///   https://opencode.ai/docs/console/go/ https://opencode.ai/docs/zen/
///
/// 目前无可证实的成员共性覆写；成员均继承 OpenAI 协议构件默认实现。
pub struct OpenCodeFamilyAdapter;

impl PlatformAdapter for OpenCodeFamilyAdapter {}

/// OpenAI 官方家族基座（issue 90；成员 openai / codex 共用，成员差异见后续 issue）。
///
/// 官方文档核对日期：2026-11-23。
/// - OpenAI API reference 将 Responses API 列为文本、图像与工具调用首选接口；请求含 `model`、`input`，
///   可选 `instructions`、`stream`、`tools`、`tool_choice`、`reasoning.effort`；本文仅记录 schema，
///   框架现有转换器负责协议映射。https://platform.openai.com/docs/api-reference
///   https://developers.openai.com/api/reference/resources/responses/methods/create.md
/// - Codex 文档说明 Codex 是 OpenAI 编码 agent；未证实 Codex 服务端对网关请求有额外字段约定，
///   因此沿用协议构件默认行为。https://platform.openai.com/docs/codex
/// - Chat Completions 的请求 / 响应以及参数默认值按 API reference；`stream` 默认值、不同模型参数支持范围，
///   以及 system/developer 消息、工具调用与图片具体限制依模型而异。基座不覆写请求构造。
///   https://platform.openai.com/docs/api-reference
/// - reasoning：Responses 接口用 `reasoning.effort`，文档列 none / minimal / low / medium / high / xhigh / max；
///   Chat Completions 的 reasoning 字段依模型而异。基座不猜测转换之外的默认值。
///   https://developers.openai.com/api/reference/resources/responses/methods/create.md
/// - usage：Chat Completions `prompt_tokens_details.cached_tokens`；Responses `input_tokens_details.cached_tokens`、
///   `cache_write_tokens`。现有 usage extractor 已识别两臂并扣除计入 input 的缓存 token。
///   https://developers.openai.com/api/reference/resources/responses/methods/create.md
/// - Responses SSE 使用 `response.output_text.delta` 等事件，终帧 `response.completed` 携带 usage；现有
///   converter 按 Responses 事件结构解析。https://developers.openai.com/api/reference/resources/responses/methods/create.md
/// - 错误对象字段 `type`、`message`、`param`、`code`；OpenAI 错误指南列出 HTTP 错误处理，框架已有解析。
///   https://developers.openai.com/api/reference/resources/responses/methods/create.md
///   https://platform.openai.com/docs/guides/error-codes
/// - Codex API 独有的错误形状、缓存 token 计量语义以及 Chat Completions 的逐模型默认值未证实，保持框架默认。
///
/// 两成员无可证实的网关覆写；均继承协议构件默认实现。
///
/// 官方未证实项不外推；本适配器保持空实现。
pub struct OpenAIFamilyAdapter;

impl PlatformAdapter for OpenAIFamilyAdapter {}

/// Cline 家族基座（issue 86；成员 cline / cline_pass 共用，成员差异见 issue 143 / 144）。
///
/// 家族共性取证（官方文档，核对日期 2026-11-23）：
/// - 协议：两成员使用 `POST /api/v1/chat/completions`，兼容 OpenAI Chat Completions；运行时
///   使用用户配置端点。`model` 与 `messages` 必填；`stream` 默认 true，`temperature` 范围
///   0.0–2.0、缺省采用模型默认值。https://docs.cline.bot/api/overview
///   https://docs.cline.bot/api/chat-completions
/// - 思考：部分模型支持 reasoning；响应可带 `reasoning` / `reasoning_details`，模型目录以
///   `supportsReasoning` 标记。无通用请求开关、档位或预算参数证据，未证实，保持框架默认。
///   https://docs.cline.bot/api/models
/// - 工具：请求使用 OpenAI function tools，响应使用 `tool_calls` 与 `finish_reason=tool_calls`；
///   未证实 Cline 自有服务端工具。https://docs.cline.bot/api/chat-completions
/// - 图片：仅支持图片的模型接受 `image_url` 内容块；音频、视频支持未证实。
///   https://docs.cline.bot/api/models
/// - system：支持 `role: system`，建议作为首条 message；无注入的默认 system prompt。
///   https://docs.cline.bot/api/chat-completions
/// - usage：`prompt_tokens`、`completion_tokens`、`prompt_tokens_details.cached_tokens` 与 USD `cost`；
///   缓存 token 计入 prompt tokens 的关系未证实，保持框架默认。
///   https://docs.cline.bot/api/chat-completions
/// - 流式：SSE `data: {chunk}`，以 `data: [DONE]` 结束；末帧可带 usage。流中错误以
///   `finish_reason=error` 及 `error` 对象表达。https://docs.cline.bot/api/chat-completions
/// - 错误：`error.code` 为数字或字符串，含 `message` 与可选 `metadata`；文档列出 400/401/402/
///   403/404/429/500/502/503。https://docs.cline.bot/api/errors
/// - 非流式响应：请求头 `X-Response-Format: openai` 选择顶层 OpenAI 响应形状；不带该头时
///   completion 嵌在 `data` 中。流式响应始终为标准形状。基座只在非流式 OpenAI 请求添加该头。
///   https://docs.cline.bot/api/chat-completions
///   https://docs.cline.bot/getting-started/clinepass
///
/// 官方未证实的请求行为保持协议构件默认值；基座不增加其他协议覆写。
pub struct ClineFamilyAdapter;

impl PlatformAdapter for ClineFamilyAdapter {
    fn additional_request_headers(
        &self,
        wire: &Protocol,
        is_stream: bool,
    ) -> &'static [(&'static str, &'static str)] {
        if wire == &Protocol::OpenAI && !is_stream {
            &[("x-response-format", "openai")]
        } else {
            &[]
        }
    }
}

/// Anthropic 家族基座（issue 89；成员 anthropic / claude_code 共用，成员差异见各成员 issue）。
///
/// 官方文档核对日期：2026-11-23。
/// - 协议与端点（`anthropic`）：Messages API `POST /v1/messages`，`model` 与 `messages`
///   必填、`max_tokens` 必填；默认 base_url `https://api.anthropic.com`。鉴权头
///   `x-api-key`，版本头 `anthropic-version: 2023-06-01`（网关 build_upstream_headers /
///   headers.rs 名单已带）。https://docs.anthropic.com/en/api/messages
///   https://docs.anthropic.com/en/api/overview
/// - 思考：`thinking` 配置 `{"type":"enabled","budget_tokens":≥1024}` / `{"type":"disabled"}`
///   （另有 `adaptive` / `between_tools`，模型相关）；thinking 块须原样回传，篡改返回
///   400 invalid_request_error。与框架 hooks / converter 现行为一致，无家族覆写。
///   https://docs.anthropic.com/en/api/messages
/// - 工具调用：`tools[]`（name/input_schema/description）+ `tool_choice`；响应
///   `tool_use` / `tool_result` 块。框架 converter anthropic 臂已覆盖，无覆写。
///   https://docs.anthropic.com/en/api/messages
/// - 图片多模态：`source.type = "base64" | "url"` 的 `image` 内容块。converter 已支持，
///   无覆写。https://docs.anthropic.com/en/api/messages
/// - system：顶层 `system` 字符串或内容块数组（可带 `cache_control`）。converter 已支持。
/// - usage：`input_tokens` / `output_tokens` + 缓存 `cache_read_input_tokens` /
///   `cache_creation_input_tokens`（细粒度 `cache_creation.ephemeral_5m/1h_input_tokens`）
///   + 按次 `server_tool_use`。框架 `usage.rs` `usage_cache_arms` anthropic 臂已按官方
///     语义采集（input 不含缓存、扁平 creation 记 5m 桶），无覆写。
///     https://docs.anthropic.com/en/api/messages
/// - 流式：SSE `message_start` / `content_block_start` / `content_block_delta`
///   （text_delta / input_json_delta / thinking_delta / signature_delta）/
///   `content_block_stop` / `message_delta`（带累积 usage.output_tokens）/ `message_stop`，
///   `data: [DONE]` 不发（以 message_stop 结束）。converter `parse_sse` anthropic 臂已覆盖。
///   https://docs.anthropic.com/en/api/messages
/// - 错误体：`{"type":"error","error":{"type":"<error type>","message":"..."}}`，状态码
///   400/401/403/404/405/409/413/429/500/529（overloaded）。框架 `classify_upstream_error`
///   已按该形状分类，无覆写。https://docs.anthropic.com/en/api/errors
/// - `claude_code`（Claude Code 订阅）为透传平台（ADR 0008）：`Protocol::ClaudeCode`
///   平台请求在 handler.rs 透传拦截分支原样 relay（handler.rs:784 起，test_endpoint.rs
///   注释明示不引用 convert_request / build_upstream_headers），不经过 PlatformAdapter
///   转换链；家族基座对它只承担注册语义（`adapter_for("claude_code")` 统一指向本基座，
///   与 `anthropic` 成员指针相同），不产生行为差异。
///
/// 结论：官方行为与框架协议构件默认实现一致，无可证实的家族级覆写；两成员均继承
/// trait 默认（与 OpenCodeFamilyAdapter 同形）。
pub struct AnthropicFamilyAdapter;

impl PlatformAdapter for AnthropicFamilyAdapter {}

/// AiHubMix 聚合平台专属适配器（issue 91；单成员，非编码套餐）。
///
/// 逐项核对官方文档（核对日期 2026-11-24）：
/// - 端点（registry 内置默认，只用于新建表单预填；运行时一律用用户配置端点）：
///   anthropic `https://aihubmix.com`、openai `https://aihubmix.com/v1`、
///   gemini `https://aihubmix.com/gemini`（registry `platforms/aihubmix/platform.json`，
///   与 docs.aihubmix.com 一致）。
/// - 请求字段与默认值：三协议均镜像官方 schema——anthropic 兼容接口 OpenAPI 标
///   `x-source: avs://anthropic/official`（https://docs.aihubmix.com/cn/api-reference/anthropic-compatible/create-a-message.md）；
///   openai 镜像 Chat Completions（https://docs.aihubmix.com/cn/api-reference/openai-compatible/create-a-chat-completion.md）；
///   gemini「Mirrors the request and response shapes of generativelanguage.googleapis.com」
///   （https://docs.aihubmix.com/cn/api-reference/google-vertex-ai-compatible/generate-content.md）。
///   官方形状即协议构件默认形状，无需覆写。
/// - 思考参数：anthropic wire = 官方 `thinking`（enabled+budget_tokens / disabled；
///   另有 `display: summarized|omitted` 控制思考内容呈现，缺省 summarized）。
///   openai wire（Claude 深度适配博客 https://docs.aihubmix.com/cn/blogs/aihubmix-openai-upgrade-claude.md，
///   更新 2026-06-01）= `reasoning_effort` / `reasoning.effort` / `reasoning.max_tokens`
///   （优先级 reasoning_effort > reasoning.max_tokens > reasoning.effort）；思考内容响应侧
///   走 `reasoning_content` + `reasoning_details`（多轮需原样回传；流式 `delta.reasoning_content`）
///   ——基座 converter 已覆盖（thinking↔reasoning_effort 映射与 reasoning_content 解析）。
///   openai wire 的思考**禁用**写法官方未给值（档位只见 low/max/xhigh），未证实，
///   保持基座第三方端点默认（chat_template_kwargs.enable_thinking=false）。
/// - 工具调用与服务端工具：镜像官方 schema，无平台专有服务端工具文档 → 基座默认。
/// - 图片与音视频：anthropic base64/url image source 镜像官方；其余未证实 → 基座默认。
/// - system：镜像官方（anthropic 字符串/块数组；openai wire 亦支持消息内 `cache_control`
///   断点，≤4 断点，5m 缺省 / `ttl:"1h"`）。
/// - max_tokens 上限：per-model，官方未给全局上限 → 基座 cap_max_tokens 按 registry 条目裁剪。
/// - usage：anthropic 镜像官方（input/output/cache_read_input_tokens/cache_creation_input_tokens）；
///   openai prompt/completion_tokens + prompt_tokens_details.cached_tokens → 基座
///   usage_cache_arms 两 arm 已覆盖（anthropic 不扣 input、openai 扣）。
/// - 错误体：OpenAI 形 `{"error":{...}}`；官方把 response ID 改为 `tid`（traceId）用于追踪，
///   400 多为上游透传（https://docs.aihubmix.com/cn/FAQs/HTTP-Codes.md）。分类读
///   error.message/code/type，tid 不参与 → 基座 classify_upstream_error 原样适用。
/// - 流式事件：anthropic 官方事件集（message_start / content_block_start / content_block_delta /
///   content_block_stop / message_delta / message_stop，OpenAPI discriminator 实锤）；openai
///   chat.completion.chunk + [DONE]；gemini usageMetadata SSE → 基座 parse_sse 已覆盖。
/// - 鉴权头：网关自签 key——openai 形端点 `Authorization: Bearer`，anthropic 形 `x-api-key`
///   （OpenAPI securitySchemes gatewayBearer / gatewayAnthropicKey）；客户端可带 `anthropic-beta`
///   头开启 Claude beta（博客 §3，平台透传）→ 基座 additional_request_headers 默认空即可。
/// - 转发层现状（grep 2026-11-24）：gateway 对 aihubmix 零特判，无 coding_plan 注入 /
///   特殊 UA / 模型链等既有差异可迁；行为不变由网关回归测试锁定。
///
/// 结论：三协议均镜像官方形状，无可证实的平台级覆写；专属适配器只承担显式注册语义
/// （与 AnthropicFamilyAdapter / OpenCodeFamilyAdapter 同形）。
pub struct AiHubMixAdapter;

impl PlatformAdapter for AiHubMixAdapter {}

/// DeepSeek（深度求索）独立平台专属适配器（issue 95；单成员，非编码套餐）。
///
/// 官方文档逐项核对（核对日期 2026-11-24）：
/// - 端点（registry 内置默认，只用于新建表单预填；运行时一律用用户配置端点）：
///   openai `https://api.deepseek.com/v1`（`POST /chat/completions`）、anthropic
///   `https://api.deepseek.com/anthropic`（`POST /anthropic/v1/messages`）。
///   https://api-docs.deepseek.com/api/create-chat-completion
///   https://api-docs.deepseek.com/guides/anthropic_api
/// - 请求字段与默认值：openai 端点镜像 Chat Completions——`stream` 缺省 false、
///   `response_format` 缺省 null、`logprobs` 缺省 false、`temperature` 取值 [0, 2]、
///   `top_p` 取值 (0, 1] 且按模型/模式有固定值（deepseek-flash 思考模式缺省 1.0 低于
///   0.95 按 0.95、非思考固定 1.0 忽略入参；deepseek-v4-pro 思考模式忽略入参）——
///   上游自行裁决，不覆写。anthropic 端点：`anthropic-version` / `anthropic-beta`
///   （除 `files-api-2025-04-14`）忽略；`top_k` / `cache_control` 忽略；`stop_sequences`
///   ≤16 条、单条 ≤512 字节；`max_tokens` HTTP 请求可选；`container` / `mcp_servers` /
///   `service_tier` 忽略；`metadata.user_id` 支持。
///   https://api-docs.deepseek.com/api/create-chat-completion
///   https://api-docs.deepseek.com/guides/anthropic_api
/// - 思考参数：openai 端点顶层 `thinking: {"type": "enabled" | "disabled"}`（缺省或 null
///   时跟 `reasoning_effort`；`reasoning_effort` 也缺省时思考**开启**）；`reasoning_effort`
///   取值 `none` / `low` / `high` / `max`（缺省 high；`minimal`→low、`medium`/`xhigh`→high，
///   未列值报 400——透传客户端原值，由上游裁决）。无思考预算参数（anthropic 端点
///   `budget_tokens` 被忽略）。**禁思考覆写**：官方写法 `thinking.type=disabled`（官方示例
///   `extra_body={"thinking": {"type": "disabled"}}`），基座 openai 第三方端点默认写 vLLM 式
///   `chat_template_kwargs.enable_thinking=false`，官方文档无此参数 → 覆写为基座 anthropic
///   臂同形；anthropic wire 官方 enabled/disabled 词表与基座同形，不另覆写。
///   https://api-docs.deepseek.com/api/create-chat-completion
///   https://api-docs.deepseek.com/guides/thinking_mode
///   https://api-docs.deepseek.com/guides/anthropic_api
/// - 工具调用与服务端工具：客户端工具 openai 形 `tools[].function`；anthropic 端点客户端
///   工具须省略 `type`（传 `type: "custom"` 报 422）。**tool_choice 差异**：思考模式下
///   openai 端点 `required` 与命名 choice 报 400（声明 web tools 时上游把 required 视为
///   auto）；anthropic 端点 `any` 在思考模式仅声明服务端工具时按 auto、仅客户端工具时报
///   400，`tool` 报 400；思考缺省开启，`thinking.type=disabled` / `reasoning_effort="none"`
///   显式关闭时这些值合法 → [`PlatformAdapter::adjust_tool_choice`] 在思考模式下把
///   required / 命名 choice / any / tool 降级为 auto，关闭时不降级。服务端工具：anthropic
///   端点 `web_search_20250305`（name `web_search`）与 `deepseek_web_search` /
///   `deepseek_read_url`；`usage.server_tool_use` 计数 `{web_search_requests, tool_search}`
///   → 基座 server_tool_counts 开放键集已覆盖。计费口径（2026-11-18 起模型触发的服务端
///   搜索/抓取不计费，结果按模型 token 计）不属出站适配范围。
///   https://api-docs.deepseek.com/guides/tool_calls
///   https://api-docs.deepseek.com/guides/anthropic_api#server-tool-usage
///   https://api-docs.deepseek.com/quick_start/pricing
/// - 图片与音视频：openai 端点 `image_url`（base64 / 外部 URL ≤8192 字符 / Files API
///   file_id，file 需 anthropic-beta 头）；anthropic 端点 image `source.type` base64 / url /
///   file。deepseek-v4-pro 不处理图片（替换为 `[Unsupported Image]`）、deepseek-flash 支持
///   用户与 tool_result 消息图片（assistant 消息图片报 400）——形状与协议构件一致，
///   模型差异由上游裁决，不覆写。无音视频输入端点文档（未证实项保持框架默认）。
///   https://api-docs.deepseek.com/guides/vision
/// - system：anthropic 端点 Fully Supported；openai 端点标准 role:system → 基座默认。
/// - max_tokens：openai 端点 1..384K（393216），缺省非思考 8K / 思考 64K（effort max
///   128K）；anthropic 端点可选（Claude Code / OpenCode 主代理请求的服务端兼容处理可抬高
///   实际上限）。上限经基座 cap_max_tokens 按 registry 条目裁剪，不注入缺省值。
///   https://api-docs.deepseek.com/api/create-chat-completion
///   https://api-docs.deepseek.com/guides/anthropic_api
/// - usage：`prompt_tokens` = `prompt_cache_hit_tokens` + `prompt_cache_miss_tokens`
///   （两字段 required）；`prompt_tokens_details.cached_tokens`（optional）与 hit 同值；
///   基座 usage_cache_arms 按 anthropic → openai cached_tokens → deepseek
///   prompt_cache_hit_tokens 顺序命中并扣 input，不覆写。anthropic 端点
///   `cache_read_input_tokens` 天然不含于 `input_tokens`（总输入 = input + cache_read +
///   cache_creation，`cache_creation_input_tokens` 当前恒 0）→ 基座 anthropic arm 同形。
///   服务端工具计数：openai 端点搭载末 chunk usage / 非流式 usage；anthropic 端点在
///   message_delta.usage（message_start 不含）→ 基座 parse_sse 按帧抽取已覆盖。
///   https://api-docs.deepseek.com/api/create-chat-completion
///   https://api-docs.deepseek.com/guides/anthropic_api#streaming
/// - 多轮 reasoning_content（思考模式）：带 tools 的请求官方要求回传前序各轮
///   reasoning_content（拼入上下文）；不带 tools 的请求回传被忽略。anthropic 端点文档：
///   思考内容缺失时服务端按 tool-call ID 恢复、恢复失败报 400（「Do not rely on automatic
///   recovery」）；anthropic 入站的 thinking 块在 to_openai 转换中被跳过 →
///   [`PlatformAdapter::build_request`] 在 openai wire 从 ChatRequest 的 thinking 块取回文本
///   写进 assistant tool-call 消息（hooks::carry_history_reasoning，kimi_coding 先例）。
///   https://api-docs.deepseek.com/guides/thinking_mode#tool-calls
///   https://api-docs.deepseek.com/guides/anthropic_api
/// - 错误体：官方错误码表 400 格式错 / 401 鉴权 / 402 余额不足 / 408·413·504 文件类 /
///   422 参数错 / 429 限频 / 500 / 503 过载；**非流式错误体 JSON schema 官方未给（未证实）**
///   → 分类不扩展，基座按状态码 + error.message/type 特征判定（401/402/403/429 → 服务类，
///   其余读特征）。anthropic 端点流式：首帧超时可发无名 `data: {"error":{"message":"..."}}`
///   帧后断连；生成中错误走 `event: error` + `{"type":"error","error":{"type","message"}}`。
///   https://api-docs.deepseek.com/quick_start/error_codes
///   https://api-docs.deepseek.com/guides/anthropic_api#streaming
/// - 流式事件：openai 端点 chat.completion.chunk + `data: [DONE]`（usage 搭载 [DONE] 前最后
///   一个内容 chunk，不发独立 usage chunk；`stream_options.include_usage` 使每 chunk 带.usage）；
///   思考增量 `delta.reasoning_content`。anthropic 端点标准事件链 message_start →
///   content_block_start/delta/stop → message_delta → message_stop（ping 可现），
///   **明确不发 `data: [DONE]`** → 基座协议构件同形，不覆写。
///   https://api-docs.deepseek.com/api/create-chat-completion
///   https://api-docs.deepseek.com/guides/anthropic_api#streaming
/// - 鉴权头：openai 端点 `Authorization: Bearer`；anthropic 端点 `x-api-key` 或
///   `Authorization: Bearer` 均可 → 基座 additional_request_headers 默认空即可。
/// - 转发层现状（grep 2026-11-24）：gateway 对 deepseek 平台零特判（billing tools_pricing /
///   tokenizer 选择不属出站适配差异，无 coding_plan 注入 / 特殊 UA 可迁）；行为不变由
///   网关回归测试锁定。
///
/// 覆写三处（均官方文档证实）：openai wire 禁思考写 `thinking.type=disabled`；思考模式下
/// tool_choice 降级；openai wire 多轮工具历史携带 reasoning_content。其余接口继承协议构件
/// 默认实现。
pub struct DeepSeekAdapter;

impl PlatformAdapter for DeepSeekAdapter {
    /// 差异：思考模式 + tools 的多轮工具历史须回传 reasoning_content（官方 400 证据链与
    /// 调用点见 struct doc「多轮 reasoning_content」节）。无 thinking 历史的请求 no-op，
    /// openai wire 之外的 wire 不处理。
    fn build_request(
        &self,
        chat_req: &crate::types::ChatRequest,
        wire: &Protocol,
        platform: &Protocol,
    ) -> (Value, String) {
        let (mut body, path) = converter::convert_request(chat_req, wire, platform);
        if wire == &Protocol::OpenAI {
            hooks::carry_history_reasoning(&mut body, chat_req);
        }
        (body, path)
    }

    /// 差异：openai 端点禁思考写官方 `thinking.type=disabled`（官方示例
    /// `extra_body={"thinking": {"type": "disabled"}}`），不写基座第三方端点的 vLLM 式
    /// `chat_template_kwargs.enable_thinking`（DeepSeek 文档无此参数）。官方词表与基座
    /// anthropic 臂同形，按 anthropic 臂执行；其余 wire 与强制思考路径同基座。
    /// https://api-docs.deepseek.com/api/create-chat-completion
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        let wire = match wire {
            Protocol::OpenAI | Protocol::OpenAICompletions => &Protocol::Anthropic,
            w => w,
        };
        hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking)
    }

    /// 差异：思考模式下 tool_choice 降级为 auto（官方 400 证据与判定条件见 struct doc
    /// 「工具调用与服务端工具」节）。思考缺省开启：`thinking.type=disabled` /
    /// `reasoning_effort="none"` / `reasoning.effort="none"` 显式关闭时保留原值。
    fn adjust_tool_choice(&self, body: &mut Value, wire: &Protocol, model: &str) {
        let _ = model;
        let Some(obj) = body.as_object_mut() else {
            return;
        };
        let thinking_disabled = obj
            .get("thinking")
            .and_then(|t| t.get("type"))
            .and_then(Value::as_str)
            == Some("disabled")
            || obj.get("reasoning_effort").and_then(Value::as_str) == Some("none")
            || obj
                .get("reasoning")
                .and_then(|r| r.get("effort"))
                .and_then(Value::as_str)
                == Some("none");
        if thinking_disabled {
            return;
        }
        match wire {
            Protocol::OpenAI | Protocol::OpenAICompletions => {
                let downgrade = match obj.get("tool_choice") {
                    Some(Value::String(s)) => s == "required",
                    Some(Value::Object(m)) => {
                        m.get("type").and_then(Value::as_str) == Some("function")
                    }
                    _ => false,
                };
                if downgrade {
                    obj.insert("tool_choice".to_string(), Value::String("auto".to_string()));
                }
            }
            Protocol::Anthropic => {
                let downgrade = obj
                    .get("tool_choice")
                    .and_then(|t| t.get("type"))
                    .and_then(Value::as_str)
                    .is_some_and(|t| t == "any" || t == "tool");
                if downgrade {
                    obj.insert(
                        "tool_choice".to_string(),
                        serde_json::json!({"type": "auto"}),
                    );
                }
            }
            _ => {}
        }
    }
}
/// CrazyRouter 聚合平台专属适配器（issue 94；单成员，非编码套餐）。
///
/// 逐项核对官方文档（核对日期 2026-11-24）：
/// - 端点（registry 内置默认，只用于新建表单预填；运行时一律用用户配置端点）：
///   anthropic `https://api.crazyrouter.com`（「Anthropic-native clients use root domain …
///   do not append `/v1` manually」）、openai `https://api.crazyrouter.com/v1`（「OpenAI-compatible
///   SDKs use `base_url="https://api.crazyrouter.com/v1"`」）、gemini `https://api.crazyrouter.com`
///   （「`gemini` maps to `POST /v1beta/models/{model}:generateContent`」，llms.txt）——三者与
///   registry 内置默认一致。同页另列 openai responses `/v1/responses` 与 legacy `/v1/completions`、
///   东亚入口 `cn.crazyrouter.com`，registry 未内置，用户可手配。路径重复拼接告警
///   （`/v1/v1/messages`）印证 anthropic 打根域名 `/v1/messages`。
///   https://docs.crazyrouter.com/api-endpoint.md https://docs.crazyrouter.com/llms.txt
/// - 请求字段与默认值：quickstart 示例请求即 `{"model","messages"}`，响应示例为标准
///   OpenAI Chat Completions 形状（`choices[].message` / `finish_reason`）——镜像官方 wire，
///   无平台专有字段文档，协议构件默认形状，无需覆写。鉴权统一 `Authorization: Bearer`
///   （网关注入）。https://docs.crazyrouter.com/quickstart.md https://docs.crazyrouter.com/authentication.md
/// - 思考参数：官方文档站（quickstart / api-endpoint / error-handling / ai-tools /
///   authentication 五页 + llms.txt 索引）均未记载思考开关 / 档位 / 预算参数——未证实，
///   保持基座第三方端点默认（`chat_template_kwargs.enable_thinking=false`）。
/// - 工具调用与服务端工具：官方仅提示「Claude 原生 `tool_use` … 应优先走 Anthropic
///   Messages 链路（`POST /v1/messages`）」，属客户端接入建议而非出站差异；无服务端工具
///   文档——未证实，保持基座默认。https://docs.crazyrouter.com/api-endpoint.md
/// - 图片与音视频：独立图像 / 视频 / 音频端点（`/v1/images/*`、`/mj/submit/*` 等）单列，
///   chat 链路多模态未记载——未证实，保持基座默认。https://docs.crazyrouter.com/api-endpoint.md
/// - system 提示 / max_tokens 上限：文档未记载——未证实，保持基座默认（max_tokens 上限
///   按 registry 条目 `cap_max_tokens` 裁剪）。
/// - usage：quickstart 响应示例 `usage.prompt_tokens / completion_tokens / total_tokens`，
///   与基座 openai 臂一致；缓存 token 字段未记载——未证实，基座构件读不到即缺省 0，
///   不覆盖。anthropic / gemini 臂未记载，同上。
///   https://docs.crazyrouter.com/quickstart.md
/// - 错误体：统一 `{"error":{"message","type","code"}}`（error-handling.md 唯一 JSON 示例），
///   与基座 `error.message/code/type` 读法一致；状态码表 400 / 401 / 403 / 429 / 500 / 502 /
///   503（403「Token 无权访问该模型」按状态码归服务类，无模型类业务码契约）——基座
///   classify_upstream_error 原样适用，不扩展。https://docs.crazyrouter.com/error-handling.md
/// - 流式事件格式与结束标记：文档未记载 SSE 帧形状 / 终止标记——未证实，保持基座构件
///   默认（openai wire `data: [DONE]`、anthropic wire `message_stop`、gemini wire 事件流）。
/// - coding_plan 注入：非编码套餐，基座按平台协议分派对 `Protocol::CrazyRouter` 为 no-op，
///   行为不变。
/// - 转发层现状（grep 2026-11-24）：gateway 对 crazyrouter 零特判（仅 `protocol.rs`
///   serde rename 与 peak 时段数据），无既有差异可迁；行为不变由网关回归测试锁定。
///
/// 结论：官方文档只证实端点布局 / 鉴权 / 错误体三件，均与协议构件默认一致；专属适配器
/// 只承担显式注册语义（与 AiHubMixAdapter / AnthropicFamilyAdapter 同形）。
/// https://docs.crazyrouter.com/ https://crazyrouter.com/pricing
pub struct CrazyRouterAdapter;

impl PlatformAdapter for CrazyRouterAdapter {}

static CRAZYROUTER: CrazyRouterAdapter = CrazyRouterAdapter;
/// Command Code（GOAT/Pro/Max/Provider 套餐 Provider API；api.commandcode.ai）。
/// 官方文档核对日期 2026-11-24，结论：三 chat 协议 + 决策协议均镜像官方形状，
/// 无可证实的平台级覆写；专属适配器承担显式注册语义（与 [`AiHubMixAdapter`] 同形）。
///
/// 逐项核对结论（官方 Provider API 页原文，本平台文档即取证所用来源）：
/// - 端点：`/v1/chat/completions`（OpenAI Chat Completions）、`/v1/responses`（OpenAI Responses，
///   仅 OpenAI 与开放模型；Claude 模型只在 `/v1/messages`）、`/v1/messages`（Anthropic Messages）、
///   `GET /v1/models`、`POST /v1/systemone`（typesafe/jev 决策，永不流式）。与 registry 内置默认
///   端点一致；运行时一律用用户平台配置端点，模型路由由 `/v1/models` 条目 `supported_endpoints`
///   决定。https://commandcode.ai/docs/provider
/// - 请求字段与默认值：官方明示「Request and response bodies follow the OpenAI Chat Completions
///   schema / OpenAI Responses schema / Anthropic Messages schema」——三 chat 协议请求构造与
///   基座协议构件一致，官方未列平台级新增字段或默认值。
/// - 思考参数：Provider API 页未列 thinking 档位 / 预算 / 开关差异，保持基座默认（未证实）。
/// - 工具调用：`function` / `custom` 工具由客户端执行，内置声明（如 `web_search`）原样转发
///   由上游接受或拒绝；remote `type:"mcp"` 工具上游拒绝（须客户端侧跑 MCP）；`x-cmd-zdr: 1`
///   请求只收 `function` / `custom` / `local_shell`。该头是客户端自选的 ZDR 开关，aidog 不代加
///   （`additional_request_headers` 保持默认空）。
/// - 图片与音视频：Provider API 页未列多模态输入差异（413 上限 32 MiB 与 Anthropic 一致），
///   保持基座默认（未证实）。
/// - system 提示 / max_tokens 上限：官方未列平台级差异或统一上限，保持基座默认（未证实）。
/// - 流式事件：`stream: true`；「Every endpoint emits token usage at the end of every stream …
///   No opt-in required」——chat 末块 usage、responses `response.completed`、anthropic
///   `message_delta`，与基座 `wire_usage` 各臂一致，无需覆写。
/// - usage（含缓存 token）：typesafe 决策响应 `usage: {input_tokens, output_tokens}`，chat 三协议
///   为官方标准形状，基座 `extract_usage` 已覆盖；缓存 token 字段名官方文档未列（pricing 页有
///   per-model 缓存价但无 usage 字段证据），保持基座各协议臂（未证实）。
/// - 错误体：OpenAI envelope `{error:{message,type,code,param}}`、Anthropic envelope
///   `{type:"error",error:{type,message}}`——基座 `classify_upstream_error` 直接读取。
///   文档错误类别与基座分类对照：401 authentication_error / 403（upgrade_required、
///   model_region_unavailable、content_policy_refused、upstream_refused）/ 429 rate_limit_error
///   按状态码归 Service；502 model_temporarily_unavailable / upstream_unavailable 与 5xx
///   server_error / api_error 归 Service；400 `unsupported_model`（OpenAI code，Anthropic envelope
///   用 invalid_request_error）不含基座模型类标记 → Unknown（调用方按模型类处理，等效换模型）；
///   413 request_too_large → Unknown（框架默认；官方建议勿重试同 body，但分类语义=换模型试
///   一次，不据此覆写）。https://commandcode.ai/docs/provider#errors
/// - coding_plan：platform.json `is_coding_plan: true` 且端点带 `coding_plan` flag；迁移自转发层的
///   既有行为 = 基座 `_` 臂 no-op（不注入 prompt_cache_key），保持不变。
/// - 未证实（保持基座默认）：max_tokens 上限、thinking 参数、缓存 token usage 字段、音视频输入
///   边界、流中错误帧形状、responses 流事件全集（官方只点名 `response.completed`）。
///   https://commandcode.ai/docs/provider https://commandcode.ai/docs/resources/pricing-limits
struct CommandCodeAdapter;

impl PlatformAdapter for CommandCodeAdapter {}

/// LiteLLM Gateway: the OpenAI-compatible routes use the configured endpoint.
/// Gateway budget errors carry `budget_exceeded` in `error.type` on OpenAI-shaped
/// routes and in `error.litellm_error_type` on the Anthropic Messages routes
/// (where `error.type` holds Anthropic's per-status type). Budget matching is
/// status-agnostic because `litellm.budget_exceeded_status_code` is configurable
/// (default 422). Stream failures before the first chunk reuse the non-stream
/// error body; after streaming begins the gateway emits the same error object as
/// a final SSE data frame (gateway-side stream peek already classifies such
/// frames). Other errors use the base classifier.
/// https://docs.litellm.ai/docs/proxy/error_reference (checked 2026-11-25).
/// Request defaults, thinking, tools, media, system prompts, token limits, cache usage,
/// and the full responses-API stream event set require further verification against
/// the gateway documentation.
struct LiteLlmAdapter;

impl PlatformAdapter for LiteLlmAdapter {
    fn classify_error(&self, status: u16, body: &str) -> error::UpstreamErrorClass {
        if serde_json::from_str::<Value>(body).is_ok_and(|v| {
            v.pointer("/error/type").and_then(Value::as_str) == Some("budget_exceeded")
                || v.pointer("/error/litellm_error_type").and_then(Value::as_str)
                    == Some("budget_exceeded")
        }) {
            return error::UpstreamErrorClass::Service;
        }
        error::classify_upstream_error(status, body)
    }
}

static LITELLM: LiteLlmAdapter = LiteLlmAdapter;
static GENERIC: GenericAdapter = GenericAdapter;

/// Gemini adapter; official GenerateContent thinking guide checked 2026-11-25:
/// - `generationConfig.thinkingConfig.thinkingLevel` is used by Gemini 3/4;
///   Gemini 2.5 uses `thinkingBudget`. Supported values and defaults depend on model.
/// - Thinking summaries use `includeThoughts`; usage includes `thoughtsTokenCount`.
/// - Function calls require preserving thought signatures in subsequent requests.
///   https://ai.google.dev/gemini-api/docs/thinking?generate-content=true
///
/// Other request fields, tools, media, system prompts, output limits, usage cache fields,
/// errors and streaming wire details require further verification.
struct GeminiAdapter;

impl PlatformAdapter for GeminiAdapter {}

static GEMINI: GeminiAdapter = GeminiAdapter;

/// 书生·端砚 Intern InkStone TokenPlan 专属适配器（issue 97；单成员，非编码套餐）。
///
/// 逐项核对（核对日期 2026-11-25）：
/// - 端点（registry 内置默认，只用于新建表单预填；运行时一律用用户配置端点）：
///   openai `https://discovery-api.intern-ai.org.cn/v1`、openai_responses
///   `https://discovery-api.intern-ai.org.cn`、anthropic 同根。与 registry 内置默认一致。
/// - 官方站 https://discovery.intern-ai.org.cn/token-plan/home 为控制台（API Key 管理、
///   RPM 默认 500、墨点计价），帮助中心 /docs/userCenter/api-key 仅密钥管理——官方无
///   线协议级 API 文档页，请求/响应/流式/错误形状无官方原文可核对。
/// - OpenAI / Anthropic 双标准协议由第三方多源佐证（lininn.cn / say.li / wn12.cn，
///   2026-11-25 检索，均为教程引述官方地址）；`openai_responses` 为模型条目
///   `supported_protocols` 登记现状（部分模型三协议、部分仅 openai+anthropic）。
/// - DSML 文本工具标记（dsv4 系把工具调用以 `<｜DSML｜tool_calls>` 写进 text 块）由
///   `aidog_adapter::dsml` + 网关 `finish.rs` 对 anthropic wire 客户端通用改写承担，
///   不经本适配器，行为不变。
/// - 鉴权：API Key（网关注入 openai 形 `Authorization: Bearer` / anthropic 形
///   `x-api-key`，基座默认）；限频 RPM 默认 500 由调用方限频承担，出站无差异。
/// - 未证实（保持基座协议构件默认）：思考参数、工具调用与服务端工具、图片与音视频、
///   system 提示、max_tokens 上限、usage 缓存 token 字段、错误体格式、SSE 帧格式与
///   结束标记。
///
/// 结论：三协议均按标准协议形状处理（第三方佐证 + 官方无反证）；专属适配器只承担
/// 显式注册语义（与 CrazyRouterAdapter / CommandCodeAdapter 同形）。
/// https://discovery.intern-ai.org.cn/token-plan/home
pub struct InternInkstoneAdapter;

impl PlatformAdapter for InternInkstoneAdapter {}

static INTERN_INKSTONE: InternInkstoneAdapter = InternInkstoneAdapter;

/// LongCat（龙猫）专属适配器（issue 99；单成员，非编码套餐）。
///
/// 逐项核对（核对日期 2026-11-25）：
/// - 端点（registry 内置默认，只用于新建表单预填；运行时一律用用户配置端点）：
///   anthropic `https://api.longcat.chat/anthropic`（出站 `/v1/messages`）、
///   openai `https://api.longcat.chat/openai/v1`（出站 `/chat/completions`）。
///   https://longcat.chat/platform/docs/
/// - 请求字段：双 wire 标准 envelope（model / messages / max_tokens / stream / top_p；
///   anthropic wire `temperature` 0~1、system 走顶层参数）。`temperature` 官方固定
///   （思考开 1.0 / 关 0.7），其他值**被覆写不报错** → 不剥客户端值（与 kimi 报错语义
///   不同），保持基座透传。
///   https://longcat.chat/platform/docs/api/chat https://longcat.chat/platform/docs/api/messages
/// - 思考：双 wire 官方参数 `thinking {"type":"enabled"|"disabled"}`（无 budget_tokens 形；
///   `thinking_budget` 固定 65536、其他值忽略）。**禁思考覆写**：openai wire 官方参数与
///   基座 anthropic 臂同词表 → 按 anthropic 臂执行，不写基座第三方端点的 vLLM 式
///   `chat_template_kwargs.enable_thinking`（官方文档无此参数）。思考开启路径：openai
///   协议构件写 `reasoning_effort` 档位，官方文档未列该参数（未证实）→ 保持基座默认，
///   上游未证实会拒未知字段。
///   https://longcat.chat/platform/docs/api/model-params
/// - Responses API 支持 function 和 custom 工具，工具字段位于对象顶层；
///   服务端工具支持情况未证实。https://longcat.chat/platform/docs/api/responses
/// - 图片与音视频：另有 Visual Understanding 页（图片/视频理解端点），chat 端点多模态
///   形状未在 Chat Completions 文档出现 → 未证实，基座默认。
///   https://longcat.chat/platform/docs/image-video-understanding
/// - system：anthropic wire 顶层参数（标准）；openai wire role:system（标准）→ 基座。
/// - max_tokens 上限：LongCat-2.5 上下文 1,048,576、max_tokens 262,144（registry 条目
///   同值，超限 400 invalid_parameter）→ 基座 cap_max_tokens 按 registry 条目裁剪。
///   https://longcat.chat/platform/docs/api/model-params
/// - usage：openai wire `usage{prompt_tokens,completion_tokens,total_tokens,
///   completion_tokens_details.reasoning_tokens}`；anthropic wire
///   `usage{input_tokens,output_tokens}`；Responses API 使用
///   `input_tokens_details.cached_tokens` 和 `output_tokens_details.reasoning_tokens`。
///   https://longcat.chat/platform/docs/api/responses
///   响应侧思考文本：openai `message.reasoning_content`、anthropic 挂在 text 块的
///   `thinking` 字段（非标准块型）→ 基座解析按既有 reasoning_content / text 块处理，
///   非标字段不参与计费。
/// - 错误体：标准 OpenAI envelope（`error{message,type,code}`；429 带 `retry_after`，
///   400 `invalid_request_error`/`invalid_parameter`）→ 基座按状态码 + 特征分类，
///   不覆写。https://longcat.chat/platform/docs/ （Rate Limiting / Range validation 节）
/// - 流式：Chat Completions 和 Messages 的逐帧形状未证实；Responses API 文档列出
///   `response.completed`、`response.incomplete`、`response.failed` 等事件，正常结束
///   时直接关闭连接，不附加 `[DONE]`。https://longcat.chat/platform/docs/api/responses
/// - 鉴权：双 wire 均 `Authorization: Bearer`（anthropic SDK 示例也带 Bearer）→
///   基座 openai 形鉴权头默认。
/// - 转发层现状（grep 2026-11-25）：gateway 对 longcat 平台零特判，无 coding_plan
///   注入等既有差异可迁；行为不变由网关回归测试锁定。
///
/// 结论：唯一覆写 = openai wire 禁思考走官方 `thinking.type=disabled`；其余接口继承
/// 协议构件默认实现。
/// https://longcat.chat/platform/docs/api/chat https://longcat.chat/platform/docs/api/messages
pub struct LongcatAdapter;

impl PlatformAdapter for LongcatAdapter {
    /// 差异：openai wire 禁思考写官方 `thinking {"type":"disabled"}`（官方参数与基座
    /// anthropic 臂同词表，DeepSeek 先例同形），不写基座第三方端点的 vLLM 式
    /// `chat_template_kwargs.enable_thinking`（官方文档无此参数）。其余 wire 与
    /// 强制思考路径同基座。
    /// https://longcat.chat/platform/docs/api/model-params
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        let wire = match wire {
            Protocol::OpenAI | Protocol::OpenAICompletions => &Protocol::Anthropic,
            w => w,
        };
        hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking)
    }
}

static LONGCAT: LongcatAdapter = LongcatAdapter;

/// SenseNova（商汤日日新）家族基座（issue 81；成员 sensenova / sensenova_en 共用，成员差异见子 issue #131 / #132）。
///
/// 家族共性取证（核对日期 2026-11-24）：
/// - 端点（registry 内置默认，只用于新建表单预填；运行时一律用用户配置端点）：
///   `sensenova` openai `https://token.sensenova.cn/v1` + anthropic `https://token.sensenova.cn`；
///   `sensenova_en` openai `https://token.sensenova.ai/v1` + anthropic `https://token.sensenova.ai`。
///   https://github.com/lazygophers/aidog/issues/81（票面端点表）
/// - 官方文档站 https://platform.sensenova.cn 与 https://platform.sensenova.ai/docs 为客户端渲染 SPA，
///   raw curl 无正文、无 sitemap / openapi.json / llms.txt，抓取不到文档原文；直连 API 网关
///   `token.sensenova.cn` / `token.sensenova.ai` 的一手探测（curl，2026-11-24）：
///   `POST /v1/chat/completions` 与 `POST /v1/messages` 未带鉴权时均返回 HTTP 401
///   （仅证明网关要求鉴权；未对照过假路径的 401 差异，不证实路由命中，更不证实 wire 协议）；
///   错误体形状 `{"error":{"code":16,"message":"Authorization Not Found"}}`（数字 gRPC 风格 code，
///   `{"code":5,"message":"NOT_FOUND","details":[]}` 404 形）；`GET /v1/models` 返回 401
///   （同上，仅证明要求鉴权，不证实该端点存在）。
/// - 请求字段、思考参数、工具调用、图片多模态、system、usage（含缓存 token）、流式事件：
///   文档抓取不到，全部未证实，保持 trait 默认（与 GenericAdapter 同形）。
/// - 错误体：实锤与 OpenAI 标准形状（`error.type` 字符串）不同——数字 `code` + `message`；
///   但框架 `classify_upstream_error` 读 error.message/code/type，message 文本仍可参与分类，
///   数字 code 的映射语义未证实，不扩展错误分类，保持框架默认。
///
/// 结论：官方文档不可抓取，无可证实的家族级覆写；两成员继承 trait 默认实现。
pub struct SenseNovaFamilyAdapter;

impl PlatformAdapter for SenseNovaFamilyAdapter {}

static SENSENOVA: SenseNovaFamilyAdapter = SenseNovaFamilyAdapter;
static OPENAI: OpenAIFamilyAdapter = OpenAIFamilyAdapter;
static ANTHROPIC_FAMILY: AnthropicFamilyAdapter = AnthropicFamilyAdapter;
static COMPSHARE: CompshareFamilyAdapter = CompshareFamilyAdapter;
static OPENCODE: OpenCodeFamilyAdapter = OpenCodeFamilyAdapter;
static CLINE: ClineFamilyAdapter = ClineFamilyAdapter;
static AIHUBMIX: AiHubMixAdapter = AiHubMixAdapter;
static DEEPSEEK: DeepSeekAdapter = DeepSeekAdapter;
static COMMAND_CODE: CommandCodeAdapter = CommandCodeAdapter;
/// TypeSafe（System One 决策平台；api.typesafe.ai）专属适配器（issue 111；单成员，非编码套餐）。
///
/// 逐项核对（官方文档核对日期 2026-11-26；Mintlify 站，raw `.md` 与 OpenAPI 均可抓取）：
/// - 端点：决策 `POST base_url + /systemone`（registry 内置默认 base_url `https://api.typesafe.ai/v1`
///   已含版本前缀；决策 URL = base_url + `/systemone`，与 `decision_api_path()` 现状一致）；
///   模型列表 `GET /v1/models`（SDK `models.list()`，平台拉模型列表走通用逻辑）。
///   运行时一律用用户平台配置端点。https://docs.typesafe.ai/api/system-one https://docs.typesafe.ai/api/list-models
/// - 鉴权：`Authorization: Bearer <API_KEY>`；网关决策路径现状注入同形，行为不变。
/// - 请求字段与默认值：`SystemOneRequest {state, model, questions}`（questions 为具名问题对象，
///   type = choice / score / noul / box）；决策 body 由客户端透传、网关仅改写 `model`（R8），
///   无平台级新增字段。https://docs.typesafe.ai/api/system-one
/// - 思考参数 / 工具调用 / system 提示 / max_tokens 上限：决策 API 无这些概念，文档无对应参数
///   （不适用）；400 `max_tokens_exceeded` 仅指请求超模型上下文长度。
/// - 图片与音视频：图片走 multipart 变体（request JSON part + image parts）或
///   `TypeSafe-Danger-Zone-Images: true` 头 + box 问题，官方标注「dreamer-ready only」实验特性
///   （2026-11-14 起）；音视频未支持。网关决策路径只发 JSON body，multipart / danger-zone 头
///   不由 aidog 代加（客户端自选开关）。https://docs.typesafe.ai/api/system-one https://docs.typesafe.ai/images
/// - 流式：决策 API 无流式（文档无 stream 参数，响应整体 JSON），网关决策路径现状不发流式，
///   行为不变。
/// - usage：响应 `usage: {input_tokens, output_tokens}`；缓存 token 字段文档未记载——未证实，
///   不覆盖。官方示例无 `usage.cost`；网关 R14 在存在时读 `usage.cost`、不存在回落 registry 价，
///   行为不变。https://docs.typesafe.ai/introduction/quickstart
/// - 错误体：400（请求规则违规或 `max_tokens_exceeded`）、401（鉴权）、413（body > 32 MiB）、
///   415（不支持的压缩编码）、422 `{detail:[{loc,msg,type}]}`（校验失败）、429（限频）、
///   529（过载）。400 信封 HTTPBadRequestError 的字段细节未逐字取证——未证实。
///   429 / 529 由框架按状态码归服务类，不扩展错误分类。https://docs.typesafe.ai/api/errors
/// - coding_plan 注入：非编码套餐，基座按平台协议分派对 `Protocol::TypeSafe` 为 no-op，行为不变。
/// - 转发层现状（grep 2026-11-26）：决策链路 `decision.rs` 直构请求（`decision_api_path()`），
///   不经 chat 转换器；gateway 对 typesafe 无其他特判，无既有差异可迁；行为不变由
///   `decision_request_end_to_end` 等决策回归测试锁定。
///
/// 结论：typesafe 是决策专属 wire 协议（非 openai / anthropic 系），chat 协议构件不适用；
/// 专属适配器只承担显式注册语义（与 [`AiHubMixAdapter`] 同形），决策行为全部在网关决策链路，
/// 原样保持。
pub struct TypeSafeAdapter;

impl PlatformAdapter for TypeSafeAdapter {}

static TYPESAFE: TypeSafeAdapter = TypeSafeAdapter;
/// xAI Grok API（api.x.ai）专属适配器（issue 112；单成员，非编码套餐）。
///
/// 逐项核对（官方文档核对日期 2026-11-26；Mintlify 站）：
/// - 端点：OpenAI 兼容，base_url `https://api.x.ai/v1`（与 registry 内置默认一致，仅预填用；
///   运行时一律用用户配置端点）；Chat Completions `POST /v1/chat/completions`（官方标 legacy，
///   仍支持）+ Responses API `POST /v1/responses`；`Authorization: Bearer <XAI_API_KEY>`。
///   https://docs.x.ai/developers/model-capabilities/legacy/chat-completions
/// - 请求字段与默认值：Chat Completions 为标准 OpenAI 形状（`messages` 数组、system / user /
///   assistant 角色、content 支持文本分段数组），官方示例用 OpenAI SDK 指向 base_url 调用——
///   与基座 openai 协议构件一致，无平台级新增字段。官方推荐 Responses API（aidog 用户可配
///   `openai_responses` 端点协议，同走基座构件）。
/// - 思考参数：`reasoning_effort`（Chat Completions）/ `reasoning.effort`（Responses），
///   `grok-4.7` / `grok-4.6`：low / medium / high（默认）/ xhigh；`grok-4.5`：low / medium /
///   high（不支持 xhigh，请求按 high 处理）；**不可关闭**（cannot be disabled）。框架
///   reasoning efforts 闸门按 registry `model_entry` 数据驱动（`forward.rs` 按条目查询），
///   适配器无平台级覆写；强制思考路径不适用（无关闭开关）。
///   https://docs.x.ai/developers/model-capabilities/text/reasoning
/// - 工具调用与服务端工具：Function Calling（客户端执行）+ 服务端工具（web search / x search /
///   code execution / remote MCP，Responses API）；具体 wire 形状逐字例未核对——未证实，
///   保持基座构件默认。
/// - 图片与音视频：Image Understanding / Audio Understanding（多模态输入）存在，OpenAI 形状；
///   具体字段未逐字核对——未证实，保持基座构件默认。
/// - system 提示 / max_tokens 上限：system 走 messages 标准角色；统一 max_tokens 上限官方
///   未列（per-model 上下文在 models 页）——上限按 registry 条目 `cap_max_tokens` 裁剪（现状）。
/// - usage（含缓存 token）：响应示例 `usage.prompt_tokens / completion_tokens / total_tokens`
///   及 `prompt_tokens_details {text_tokens, audio_tokens, image_tokens, cached_tokens}`——
///   基座 `usage.rs` 已读 `prompt_tokens_details.cached_tokens`（`usage.rs:64`），无需覆写。
///   https://docs.x.ai/developers/model-capabilities/legacy/chat-completions
/// - 错误体：状态码表 400（含「不正确的 API key」）、401、403、404（含「模型不存在」）、
///   405、415、422、429（限频）；错误 JSON 信封官方只说「error message self-explanatory」，
///   未给原文示例——信封细节未证实，基座 `classify_upstream_error` 读
///   error.message/code/type + 状态码（429 → 服务类）照常适用，不扩展。
///   https://docs.x.ai/developers/debugging
/// - 流式事件格式与结束标记：Chat Completions 流式官方转引 Streaming 页，SSE `data:` 帧形状
///   未逐字取证——未证实，保持基座 openai 臂默认（`data: [DONE]` 终止）。
///   https://docs.x.ai/developers/model-capabilities/text/streaming
/// - coding_plan 注入：非编码套餐，基座按平台协议分派对 `Protocol::Xai` 为 no-op，行为不变。
/// - 转发层现状（grep 2026-11-26）：gateway 对 xai 零特判（仅 `protocol.rs` serde rename），
///   无既有差异可迁；行为不变由基座分派回归测试锁定。
///
/// 结论：官方只证实「OpenAI 兼容 + 端点/鉴权 + usage 缓存字段 + 思考档位」四件，均与协议
/// 构件/registry 数据链路现状一致；专属适配器只承担显式注册语义（与 [`AiHubMixAdapter`] 同形）。
pub struct XaiAdapter;

impl PlatformAdapter for XaiAdapter {}

static XAI: XaiAdapter = XaiAdapter;
/// PipeLLM Relay（Auto Router；api.pipellm.ai / cc-api.pipellm.ai）专属适配器
/// （issue 107；单成员，非编码套餐）。
///
/// 逐项核对（官方文档核对日期 2026-11-26；Mintlify 站，raw `.md` 可抓取）：
/// - 端点（registry 内置默认仅预填用；运行时一律用用户配置端点）：官方 Relay 公开两类路由——
///   native（`/v1/chat/completions`、`/v1/responses`、`/v1/messages`、
///   `/v1beta/models/{model}:generateContent` 等挂 `https://api.pipellm.ai`）与 converter 前缀
///   路由（`/openai/v1`、`/responses/v1`、`/anthropic`、`/gemini`）。registry 内置默认
///   openai = `https://api.pipellm.ai/openai/v1`、gemini = `https://api.pipellm.ai/gemini`
///   （converter 路线）；anthropic = `https://cc-api.pipellm.ai`（票面登记值；官方
///   authentication 页 native anthropic base URL 为 `https://api.pipellm.ai`，两处不一致
///   属 registry 数据现状，行为不受影响——出站只读用户配置）。
///   https://docs.pipellm.ai/guides/routing-protocols https://docs.pipellm.ai/api-reference/authentication
/// - 鉴权：openai wire `Authorization: Bearer`、anthropic wire `x-api-key`、gemini wire
///   `x-goog-api-key`，Relay 侧统一 API key（禁发 provider key）——基座各协议臂默认注入同形。
///   https://docs.pipellm.ai/api-reference/authentication
/// - 请求字段与默认值：三协议均为官方标准形状（openai `messages` / anthropic Messages /
///   gemini GenerateContent），Relay 只做路由与策略，无平台级新增字段。native 路由有协议
///   守卫（openai 形只路由 OpenAI 兼容平台等）；converter 前缀路由不受守卫限制——均为
///   服务端路由行为，不影响 aidog 出站构造，基座构件原样适用。
/// - 思考参数 / 工具调用 / system / max_tokens 上限：官方 API 参考未列平台级差异——未证实，
///   保持基座构件默认（上限按 registry 条目 `cap_max_tokens` 裁剪）。
/// - 图片与音视频：官方另有图像生成 / 视频（`/v2/videos` 异步任务）与 `/v1/systemone`
///   决策（jev）端点；chat 链路多模态未记载——未证实，保持基座构件默认。
/// - usage（含缓存 token）：prompt caching 文档响应示例为 anthropic 形
///   `usage.cache_creation_input_tokens / cache_read_input_tokens`——基座 `usage_cache_arms`
///   anthropic 臂已读（`usage.rs:43`）；openai / responses 形由基座对应臂覆盖。无平台级覆写。
///   https://docs.pipellm.ai/guides/prompt-caching
/// - 错误体：统一 `{"error":{"type","code","message"}}`，状态码表 400 invalid_request_error /
///   401 authentication_error / 402 insufficient_balance / 403 permission_error /
///   404 not_found_error / 405 invalid_request_error / 429 rate_limit_error / 500+ api_error /
///   529 overloaded_error——基座 `classify_upstream_error` 读 error.message/code/type + 状态码
///   照常适用，不扩展。https://docs.pipellm.ai/api-reference/errors
/// - 流式事件格式与结束标记：Chat Completions `stream: true` 走 SSE（openai 臂默认）；
///   官方明示「cross-protocol streaming output in Gemini format is not implemented」——
///   gemini converter 路线不支持流式出站，用户配 gemini 协议端点时流式请求行为按基座现状
///   （不发流式保障，属服务端限制，如实透传）。https://docs.pipellm.ai/api-reference/streaming
/// - coding_plan 注入：非编码套餐，基座按平台协议分派对 `Protocol::PipeLlm` 为 no-op，行为不变。
/// - 转发层现状（grep 2026-11-26）：gateway 对 pipellm 零特判（仅 `protocol.rs` serde
///   rename 与 registry 数据），无既有差异可迁；行为不变由基座分派回归测试锁定。
///
/// 结论：官方证实端点布局 / 鉴权 / 错误体 / usage 缓存字段 / gemini 流式限制五件，均与
/// 协议构件现状一致；专属适配器只承担显式注册语义（与 [`AiHubMixAdapter`] 同形）。
pub struct PipeLlmAdapter;

impl PlatformAdapter for PipeLlmAdapter {}

static PIPELLM: PipeLlmAdapter = PipeLlmAdapter;
static MIMO: MimoFamilyAdapter = MimoFamilyAdapter {
    official_openai_thinking: true,
    carry_history_reasoning_openai: true,
};
static MIMO_CODING: MimoFamilyAdapter = MimoFamilyAdapter {
    official_openai_thinking: false,
    carry_history_reasoning_openai: false,
};
static MINIMAX_FAMILY: MiniMaxFamilyAdapter = MiniMaxFamilyAdapter;
static MINIMAX: MiniMaxAdapter = MiniMaxAdapter;
static MINIMAX_CODING: MiniMaxCodingAdapter = MiniMaxCodingAdapter;
static MINIMAX_EN: MiniMaxEnAdapter = MiniMaxEnAdapter;
static KIMI: KimiFamilyAdapter = KimiFamilyAdapter {
    official_openai_thinking: true,
    coding_openai_reasoning_effort: false,
    carry_history_reasoning_openai: false,
    downgrade_required_tool_choice: true,
};
static KIMI_CODING: KimiFamilyAdapter = KimiFamilyAdapter {
    official_openai_thinking: false,
    coding_openai_reasoning_effort: true,
    carry_history_reasoning_openai: true,
    downgrade_required_tool_choice: false,
};
static GLM: GlmFamilyAdapter = GlmFamilyAdapter;
static BAILIAN: BailianFamilyAdapter = BailianFamilyAdapter;
static ARK: ArkFamilyAdapter = ArkFamilyAdapter;
static QIANFAN: QianfanFamilyAdapter = QianfanFamilyAdapter;
static SILICONFLOW: SiliconFlowFamilyAdapter = SiliconFlowFamilyAdapter;
static STEPFUN: StepFunFamilyAdapter = StepFunFamilyAdapter;
static TENCENT: TencentFamilyAdapter = TencentFamilyAdapter {
    official_openai_thinking: true,
};
static TENCENT_CODING: TencentFamilyAdapter = TencentFamilyAdapter {
    official_openai_thinking: false,
};

/// Magpie 网关适配器。官方参考（核对日期 2026-11-25）列出 OpenAI Chat
/// (`/v1/chat/completions`)、Responses (`/v1/responses`)、Anthropic Messages
/// (`/v1/messages`) 和 Gemini (`/v1beta/models/{model}:generateContent`) 端点，并称支持
/// 流式、工具调用和推理内容；本适配器运行时使用用户配置端点。
/// 官方 Anthropic 请求结构接受 system、max_tokens、thinking.type、thinking.budget_tokens
/// 和 output_config.effort；Chat 接受 max_tokens、max_completion_tokens 和 reasoning_effort。
/// Responses 用 instructions、max_output_tokens，usage 包含 input_tokens_details.cached_tokens。
/// Chat 流输出以 `[DONE]` 结束，Anthropic 流发送 `message_stop`；Responses 发送
/// `response.completed`、`response.incomplete` 或 `response.failed`。Gemini 支持
/// `inlineData`、`fileData`，usage 包含 `cachedContentTokenCount`。
/// https://github.com/yetone/magpie/blob/main/internal/gateway/gemini.go
/// https://github.com/yetone/magpie/blob/main/internal/gateway/anthropic.go
/// https://github.com/yetone/magpie/blob/main/internal/gateway/chat.go
/// https://github.com/yetone/magpie/blob/main/internal/gateway/responses.go
/// 未证实：请求默认值、各协议思考参数的完整语义、工具及媒体细节、max_tokens 上限、
/// 错误体和各协议完整流事件；保持协议构件默认行为。
/// 官方网关端点列表未列 OpenAI Completions；仅按 issue 验收要求，对用户配置该协议做
/// 合成假上游兼容性测试，不表示 Magpie 官方支持该端点。
/// https://github.com/yetone/magpie/blob/main/docs/reference.md#connecting-anything-else
struct MagpieAdapter;
impl PlatformAdapter for MagpieAdapter {}
static MAGPIE: MagpieAdapter = MagpieAdapter;

/// Meta Model API，官方文档核对日期 2026-11-25。
/// Chat 支持 system/developer、工具调用及 SSE `[DONE]`，默认 temperature 和 top_p 为 1；
/// max_completion_tokens 是输出上限，max_tokens 为兼容别名，模型不接受 reasoning_effort=none。
/// Messages 要求 max_tokens，支持 system 文本、图片、视频和工具；推理模型不接受 disabled，
/// enabled 的 budget_tokens 至少 1024 且小于 max_tokens，流以 message_stop 结束。
/// usage 可包含 cache_read_input_tokens；错误采用 Anthropic error 封装。
/// https://dev.meta.ai/docs/protocols/chat-completions
/// https://dev.meta.ai/docs/protocols/messages
/// 未证实：各模型的输出上限、所有工具和媒体细节、各协议完整错误体及流事件；保持默认行为。
struct MetaAdapter;
impl PlatformAdapter for MetaAdapter {}
static META: MetaAdapter = MetaAdapter;

/// Mistral API，官方文档核对日期 2026-11-25（Chat Completions `POST /v1/chat/completions`）。
/// 请求字段：messages、model、max_tokens、temperature（默认随模型）、top_p、stop、stream
/// （默认 false，SSE 以 `data: [DONE]` 结束）、n、parallel_tool_calls（默认 true）、
/// random_seed、safe_prompt（默认 false）、response_format（text/json_object/json_schema，
/// strict 默认 false）、reasoning_effort、prompt_mode、prompt_cache_key、tool_choice
/// （none/auto/any/required/具名）。消息内容块：TextChunk、ThinkChunk（type=thinking，含
/// closed 与跨轮 replay 的 signature）、ImageURLChunk（image_url + detail）、DocumentURLChunk。
/// 响应 usage（UsageInfo）：prompt_tokens、completion_tokens、total_tokens、
/// prompt_tokens_details.cached_tokens、prompt_audio_seconds、service_tier；choices 带
/// finish_reason。system 经 SystemMessage 支持。
/// https://docs.mistral.ai/llms/api/endpoint/chat/llms.txt
/// 未证实：错误体格式、reasoning_effort 的取值词表、max_tokens 各模型上限、工具与音频细节；
/// 保持协议构件默认行为。官方未列其他 wire 协议端点；对用户配置其他协议仅做合成假上游
/// 兼容性测试，不表示 Mistral 官方支持。
struct MistralAdapter;
impl PlatformAdapter for MistralAdapter {}
static MISTRAL: MistralAdapter = MistralAdapter;

/// ModelScope（魔搭）API-Inference，官方文档核对日期 2026-11-25。
/// LLM 提供 OpenAI API 兼容接口（`https://api-inference.modelscope.cn/v1`），支持 system
/// 角色与流式；Anthropic API 兼容接口处于 beta，参数参照 Anthropic 官方文档；reasoning
/// 模型的调用方式与标准 LLM 有细微差异，官方要求以各模型页面的示例代码为准。
/// 视觉模型图片支持线上 URL 与 Base64 data URL；音频/视频生成走独立异步任务接口，
/// 与聊天转发路径无关。
/// https://www.modelscope.cn/docs/model-service/API-Inference/intro.md
/// 未证实：usage 与缓存 token 字段、错误体格式、流式事件与结束标记、思考开关/档位/
/// 预算、max_tokens 上限、工具调用细节；保持协议构件默认行为。官方未列其他 wire 协议
/// 端点；对用户配置其他协议仅做合成假上游兼容性测试，不表示 ModelScope 官方支持。
struct ModelscopeAdapter;
impl PlatformAdapter for ModelscopeAdapter {}
static MODELSCOPE: ModelscopeAdapter = ModelscopeAdapter;

/// Novita AI，官方文档核对日期 2026-11-25。
/// OpenAI 兼容接口（官方 llms.txt 写 `https://api.novita.ai/openai`；registry 内置默认
/// `https://api.novita.ai/v3/openai` 仅用于预填表单）与 Anthropic 兼容接口
/// （`https://api.novita.ai/anthropic`，仅部分模型）。Chat：消息角色 system/user/assistant；
/// 内容块支持 image_url（URL 或 base64）；max_tokens 超上下文默认自动下调而非报错
/// （context_length_exceeded_behavior）；stream 默认 false，`stream_options.include_usage`
/// 会在 `data: [DONE]` 前追加一个整段 usage 块；工具仅支持 functions；strict 开启
/// Structured Outputs；separate_reasoning（默认 false）把思考链放 reasoning_content；
/// enable_thinking（默认 true）只作用于支持关闭思考的混合推理模型，常开推理模型忽略。
/// usage 含 prompt_tokens、completion_tokens、total_tokens（文档未列缓存 token 字段）。
/// https://docs.novita.ai/api-reference/model-apis-llm-create-chat-completion.md
/// https://docs.novita.ai/guides/llm-anthropic-compatibility.md
/// 未证实：错误体格式、usage 缓存字段、思考档位/预算参数、工具与媒体细节全貌；保持协议
/// 构件默认行为。官方未列其他 wire 协议端点；对用户配置其他协议仅做合成假上游兼容性
/// 测试，不表示 Novita 官方支持。
struct NovitaAdapter;
impl PlatformAdapter for NovitaAdapter {}
static NOVITA: NovitaAdapter = NovitaAdapter;

/// NVIDIA NIM 托管 API，官方资料核对日期 2026-11-25。
/// 托管端点 base `https://integrate.api.nvidia.com/v1`（OpenAI 兼容 `/v1/chat/completions`，
/// 官方文档站 docs.api.nvidia.com 各路径探测返回 404/429，未能取得页面原文）。
/// NVIDIA 官方仓库证实（第 2 类来源）：请求可带顶层 `nvext` 扩展对象（NVIDIA/TensorRT-LLM
/// `tensorrt_llm/serve/render/serving.py` 的 DYNAMO_ONLY_FIELDS；NVIDIA/NeMo-Agent-Toolkit
/// `packages/nvidia_nat_core/src/nat/llm/dynamo_llm.py` 的 `nvext.agent_hints` 输出长度提示
/// LOW/MEDIUM/HIGH 对应 128/512/2048 token）；思考开关经 `chat_template_kwargs` 的 thinking
/// 字段进入 reasoning 解析器（serving.py 的 `reasoning_ended = not thinking`）。
/// https://docs.api.nvidia.com/
/// 未证实：请求字段默认值、usage 与缓存 token 字段、错误体格式、流式事件与结束标记、
/// max_tokens 上限、工具/图片/system 细节；保持协议构件默认行为。官方未列其他 wire 协议
/// 端点；对用户配置其他协议仅做合成假上游兼容性测试，不表示 NVIDIA 官方支持。
struct NvidiaAdapter;
impl PlatformAdapter for NvidiaAdapter {}
static NVIDIA: NvidiaAdapter = NvidiaAdapter;

/// OpenRouter，官方文档核对日期 2026-11-25。
/// Chat Completions 端点 `/api/v1/chat/completions`（base `https://openrouter.ai/api/v1`）；
/// Anthropic 兼容接口 base `https://openrouter.ai/api`（reasoning 预算走 Anthropic 式
/// `reasoning.max_tokens`）。`reasoning` 参数：`effort` 取 max/xhigh/high/medium/low/minimal，
/// `max_tokens` 为 Anthropic 式预算，`enabled` 缺省由 effort/max_tokens 推断，`exclude`
/// 仅 Chat Completions 支持；模型元数据带 supported_efforts/default_effort/mandatory。
/// usage 含 `output_tokens_details.reasoning_tokens`；思考耗尽预算时返回 200 +
/// `finish_reason:"length"` + 空 content。流式 SSE 以 `data: [DONE]` 结束；错误体为
/// `{error: {code, message, metadata}}`，HTTP 状态与 error.code 一致，429 限频的
/// `error.metadata.limit_source` 标记来源，审核拦截细节在 error.metadata。
/// https://openrouter.ai/docs/guides/best-practices/reasoning-tokens.md
/// https://openrouter.ai/docs/api_reference/errors-and-debugging.md
/// 未证实：usage 缓存 token 字段形状、max_tokens 各模型上限、工具与图片细节、
/// Anthropic 兼容端点的完整流事件；保持协议构件默认行为。官方未列其他 wire 协议端点；
/// 对用户配置其他协议仅做合成假上游兼容性测试，不表示 OpenRouter 官方支持。
struct OpenrouterAdapter;
impl PlatformAdapter for OpenrouterAdapter {}
static OPENROUTER: OpenrouterAdapter = OpenrouterAdapter;

/// TheRouter.ai，官方文档核对日期 2026-11-25（https://therouter.ai/docs/api/reference/overview/ ）。
/// OpenAI 兼容核心端点 `/v1/*`（含 `/v1/chat/completions`）；另有 Anthropic Messages 与
/// Responses API 端点；`/api/v1/*` 为 OpenRouter 兼容层。Bearer 鉴权，每个响应带
/// x-request-id。流式 SSE：`stream` 默认 false，`stream_options.include_usage` 必须为
/// true 才在 `data: [DONE]` 前发独立的 usage 终块（choices 为空数组）。错误体：
/// 标准 `{error: {message, type, code, param}}`；402 扩展带 `error.balance` 与
/// `error.actions`，语义码 `insufficient_credits`；404 `model_not_found` 带
/// did_you_mean/suggestions/docs/trace_id。错误码：400/401/402/404/429/500/502/503。
/// https://therouter.ai/docs/api/reference/streaming/
/// https://therouter.ai/docs/api/reference/errors-and-debugging/
/// 未证实：usage 缓存 token 字段、思考开关/档位、工具与媒体细节、max_tokens 上限、
/// Anthropic/Responses 端点的完整事件与错误形状；保持协议构件默认行为。官方未列其他
/// wire 协议端点；对用户配置其他协议仅做合成假上游兼容性测试，不表示 TheRouter 官方支持。
struct TherouterAdapter;
impl PlatformAdapter for TherouterAdapter {}
static THEROUTER: TherouterAdapter = TherouterAdapter;

/// 基元律动 TokenRhythm，官方文档核对日期 2026-11-25（https://tokenrhythm.studio/docs/api-integration ）。
/// 端点：GET /v1/models、POST /v1/chat/completions（OpenAI）、POST /v1/messages（Anthropic）、
/// POST /v1/embeddings；JEV 决策与 Wan 视频为独立接口，不经聊天转发。统一 base
/// `https://tokenrhythm.studio/v1`，Bearer 鉴权。Anthropic：anthropic-version 未传或为空
/// 时默认按 2023-06-01 处理，显式传入不校验白名单、向上游透传；max_tokens 必填。
/// 流式：OpenAI 传 stream:true 后按 SSE 返回，增量在 choices[0].delta.content；Anthropic
/// 按原生 Messages 流式事件返回，两种协议解析器不混用。工具：支持 OpenAI tools、
/// tool_choice、tool_calls；DeepSeek 的 tool_choice 只收 none/auto/required，不收对象形式。
/// 错误体统一 {code, message, traceId}；401 密钥无效、403 未获灰度权限、413 请求体超限、
/// 422 参数或上游输入限制、429 容量限流、502/503/504 上游故障。
/// 未证实：usage 缓存 token 字段、思考开关/档位、max_tokens 上限、图片与音视频细节；
/// 保持协议构件默认行为。官方未列其他 wire 协议端点；对用户配置其他协议仅做合成假上游
/// 兼容性测试，不表示 TokenRhythm 官方支持。
struct TokenrhythmAdapter;
impl PlatformAdapter for TokenrhythmAdapter {}
static TOKENRHYTHM: TokenrhythmAdapter = TokenrhythmAdapter;

/// 显式注册表：registry 平台 code → 适配器单例（与 src-tauri/defaults/registry/platforms/ 一一对应；
/// 未列入家族适配器的平台走默认家族基座）。
static ATLASCLOUD: AtlasCloudAdapter = AtlasCloudAdapter;

static REGISTRY: &[(&str, &dyn PlatformAdapter)] = &[
    ("aihubmix", &AIHUBMIX),
    ("anthropic", &ANTHROPIC_FAMILY),
    ("atlascloud", &ATLASCLOUD),
    ("bailian", &BAILIAN),
    ("bailian_coding", &BAILIAN),
    ("bailian_coding_en", &BAILIAN),
    ("bailian_en", &BAILIAN),
    ("byteplus", &ARK),
    ("claude_code", &ANTHROPIC_FAMILY),
    ("cline", &CLINE),
    ("cline_pass", &CLINE),
    ("codex", &OPENAI),
    ("commandcode", &COMMAND_CODE),
    ("compshare", &COMPSHARE),
    ("compshare_coding", &COMPSHARE),
    ("crazyrouter", &CRAZYROUTER),
    ("deepseek", &DEEPSEEK),
    ("devin", &GENERIC),
    ("doubao", &ARK),
    ("gemini", &GEMINI),
    ("glm", &GLM),
    ("glm_coding", &GLM),
    ("glm_coding_en", &GLM),
    ("glm_en", &GLM),
    ("intern_inkstone", &INTERN_INKSTONE),
    ("kimi", &KIMI),
    ("kimi_coding", &KIMI_CODING),
    ("kimi_en", &KIMI),
    ("litellm", &LITELLM),
    ("longcat", &LONGCAT),
    ("magpie", &MAGPIE),
    ("meta", &META),
    ("minimax", &MINIMAX),
    ("minimax_coding", &MINIMAX_CODING),
    ("minimax_en", &MINIMAX_EN),
    ("mistral", &MISTRAL),
    ("modelscope", &MODELSCOPE),
    ("newapi", &GENERIC),
    ("novita", &NOVITA),
    ("nvidia", &NVIDIA),
    ("openai", &OPENAI),
    ("opencode", &OPENCODE),
    ("opencode_zen", &OPENCODE),
    ("openrouter", &OPENROUTER),
    ("passthrough", &GENERIC),
    ("pipellm", &PIPELLM),
    ("qianfan", &QIANFAN),
    ("qianfan_coding", &QIANFAN),
    ("sensenova", &SENSENOVA),
    ("sensenova_en", &SENSENOVA),
    ("shengsuanyun", &GENERIC),
    ("siliconflow", &SILICONFLOW),
    ("siliconflow_en", &SILICONFLOW),
    ("stepfun", &STEPFUN),
    ("stepfun_en", &STEPFUN),
    ("tencent_coding", &TENCENT_CODING),
    ("tencent_tokenhub", &TENCENT),
    ("tencent_tokenhub_intl", &TENCENT),
    ("therouter", &THEROUTER),
    ("tokenrhythm", &TOKENRHYTHM),
    ("typesafe", &TYPESAFE),
    ("xai", &XAI),
    ("xiaomi_mimo", &MIMO),
    ("xiaomi_mimo_coding", &MIMO_CODING),
    ("xiaomi_mimo_coding_en", &MIMO_CODING),
];

fn table() -> &'static std::collections::HashMap<&'static str, &'static dyn PlatformAdapter> {
    static T: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static dyn PlatformAdapter>> =
        std::sync::OnceLock::new();
    T.get_or_init(|| REGISTRY.iter().copied().collect())
}

/// 门禁用：registry 平台 code 是否有显式注册项。
pub fn lookup(code: &str) -> Option<&'static dyn PlatformAdapter> {
    table().get(code).copied()
}

/// 平台 code → 出站适配器；未登记 code（registry 清单外的用户协议变体）回落通用适配。
pub fn adapter_for(code: &str) -> &'static dyn PlatformAdapter {
    table().get(code).copied().unwrap_or(&GENERIC)
}

/// Compshare（优云）家族基座（issue 88；成员 compshare / compshare_coding 共用，
/// 成员差异见各成员 issue）。
///
/// 官方文档核对日期：2026-11-23。
/// - 协议与端点（标准平台 `compshare`）：官方「我们的服务完全兼容这套标准（OpenAI API）」；
///   OpenAI 兼容接口 `/v1/chat/completions`、`/v1/response`、`/v1/models`，Base URL
///   `https://api.modelverse.cn/v1`；Anthropic 兼容 Messages 端点 `https://api.modelverse.cn/v1/messages`
///   （认证 `x-api-key` 或 `Authorization: Bearer`，仅 Claude 系模型走此端点）；Gemini 兼容
///   Base URL `https://api.modelverse.cn`。与 registry 内置默认端点一致。
///   https://www.compshare.cn/docs/modelverse/models/quick-start
///   https://www.compshare.cn/docs/modelverse/models/text_api/claude_compatible
/// - 思考开关（openai 端点）：顶层对象参数 `thinking.type`，`enabled` / `disabled`
///   （DeepSeek V3.1 系；Doubao 系另列 `auto`），请求示例 `"thinking": {"type": "enabled"}`。
///   仅对官方证实模型清单按 `body["model"]` 门控写出（清单见 impl 注释），未证实模型
///   保持基座第三方端点默认，不外推。
///   https://www.compshare.cn/docs/modelverse/models/text_api/thinking/deepseek
///   https://www.compshare.cn/docs/modelverse/models/text_api/thinking/doubao
/// - 工具调用、图片与多模态、system 提示、usage 缓存 token 字段、错误体格式、流式
///   SSE 事件：官方文档未给出 Modelverse 网关层差异证据，未证实，保持框架默认。
///   https://www.compshare.cn/docs/modelverse/models/text_api/api-expand
///   https://www.compshare.cn/docs/modelverse/models/text_api/model-competi
/// - 套餐成员 `compshare_coding`（`https://cp.compshare.cn`，anthropic wire，官方文档
///   https://www.compshare.cn/docs/modelverse/models/quick-start 未给 API 参数差异；
///   cp.compshare.cn 文档站不可达 HTTP 404）：成员差异未证实，继承家族基座。
///
/// 家族级覆写只有 openai 端点禁思考的模型级门控（见 impl 注释）；其余接口继承协议构件
/// 默认实现。
pub struct CompshareFamilyAdapter;

impl PlatformAdapter for CompshareFamilyAdapter {
    /// 差异：openai 端点禁思考仅对官方证实支持 `thinking.type` 的模型写官方
    /// `thinking: {"type": "disabled"}`（与基座 anthropic 臂同形），其余模型保持基座
    /// 第三方端点默认（vLLM 式 `chat_template_kwargs.enable_thinking=false`）。
    /// 证实模型清单（官方文档，核对 2026-11-23，按 `body["model"]` 逐字匹配）：
    /// `deepseek-ai/DeepSeek-V3.1`、`deepseek-ai/DeepSeek-V3.1-Terminus`（词表
    /// `enabled` / `disabled`），https://www.compshare.cn/docs/modelverse/models/text_api/thinking/deepseek；
    /// `ByteDance/doubao-seed-1.6`、`ByteDance/doubao-1.5-thinking-vision-pro`（另列 `auto`），
    /// https://www.compshare.cn/docs/modelverse/models/text_api/thinking/doubao。
    /// 未证实模型不外推（全家族覆写在其余模型上的接受度未证实，f406b7f8f 收回后按
    /// review 结论改为模型级门控）；新模型取证后在 [`compshare_official_thinking_model`]
    /// 扩清单。anthropic / gemini wire 与强制思考路径同基座。
    fn apply_disable_thinking(
        &self,
        body: &mut Value,
        disable: bool,
        wire: &Protocol,
        upstream_url: &str,
        forced_thinking: bool,
    ) {
        let gated = disable
            && !forced_thinking
            && wire == &Protocol::OpenAI
            && body
                .get("model")
                .and_then(Value::as_str)
                .is_some_and(compshare_official_thinking_model);
        let wire = match wire {
            Protocol::OpenAI if gated => &Protocol::Anthropic,
            w => w,
        };
        hooks::apply_disable_thinking(body, disable, wire, upstream_url, forced_thinking)
    }
}

/// 官方证实支持 `thinking.type` 的 Modelverse 模型（逐字 ID，出处见
/// `CompshareFamilyAdapter::apply_disable_thinking` 注释）。
fn compshare_official_thinking_model(model: &str) -> bool {
    matches!(
        model,
        "deepseek-ai/DeepSeek-V3.1"
            | "deepseek-ai/DeepSeek-V3.1-Terminus"
            | "ByteDance/doubao-seed-1.6"
            | "ByteDance/doubao-1.5-thinking-vision-pro"
    )
}

#[cfg(test)]
mod compshare_family_tests {
    use super::*;

    #[test]
    fn members_share_registered_family_adapter() {
        let standard = lookup("compshare").unwrap();
        let coding = lookup("compshare_coding").unwrap();
        assert!(std::ptr::eq(standard, coding));
        assert!(!std::ptr::eq(standard, adapter_for("openai")));
    }

    /// openai 端点禁思考按模型门控：官方证实模型写 `thinking.type=disabled`（官方词表
    /// enabled/disabled，见 impl 注释），不写基座第三方 vLLM 式参数；未证实模型不外推，
    /// 保持基座第三方端点默认；强制思考只剥禁用形参数不写关闭参数；非 openai wire 同基座。
    #[test]
    fn openai_disable_thinking_gated_on_official_models() {
        let pa = adapter_for("compshare");

        for model in [
            "deepseek-ai/DeepSeek-V3.1",
            "deepseek-ai/DeepSeek-V3.1-Terminus",
            "ByteDance/doubao-seed-1.6",
            "ByteDance/doubao-1.5-thinking-vision-pro",
        ] {
            let mut body = serde_json::json!({"model": model, "enable_thinking": true});
            pa.apply_disable_thinking(&mut body, true, &Protocol::OpenAI, "https://stub", false);
            assert_eq!(
                body["thinking"],
                serde_json::json!({"type": "disabled"}),
                "{model}"
            );
            assert!(body.get("enable_thinking").is_none(), "{model}");
            assert!(body.get("chat_template_kwargs").is_none(), "{model}");
        }

        // 未证实模型：保持基座第三方端点默认（chat_template_kwargs），不写 thinking。
        let mut body = serde_json::json!({"model": "deepseek-ai/DeepSeek-V3.2"});
        pa.apply_disable_thinking(&mut body, true, &Protocol::OpenAI, "https://stub", false);
        assert_eq!(
            body.pointer("/chat_template_kwargs/enable_thinking"),
            Some(&Value::Bool(false))
        );
        assert!(body.get("thinking").is_none());

        // 强制思考：证实模型同样只剥不写（基座强制思考路径）。
        let mut body =
            serde_json::json!({"model": "deepseek-ai/DeepSeek-V3.1", "thinking": {"type": "disabled"}});
        pa.apply_disable_thinking(&mut body, true, &Protocol::OpenAI, "https://stub", true);
        assert!(body.get("thinking").is_none());

        // anthropic / gemini wire 同基座（anthropic 臂写 thinking.type=disabled、
        // gemini 臂写 thinkingBudget=0），锁不回归即可。
        let mut body = serde_json::json!({"model": "claude-3"});
        pa.apply_disable_thinking(&mut body, true, &Protocol::Anthropic, "https://stub", false);
        assert_eq!(body["thinking"], serde_json::json!({"type": "disabled"}));
        let mut body = serde_json::json!({"model": "gemini-2.5"});
        pa.apply_disable_thinking(&mut body, true, &Protocol::Gemini, "https://stub", false);
        assert_eq!(
            body["generationConfig"]["thinkingConfig"],
            serde_json::json!({"thinkingBudget": 0})
        );
    }
}

#[cfg(test)]
mod sensenova_family_tests {
    use super::*;

    /// 两成员注册了同一家族基座单例，不再回落通用适配（GENERIC 与 deepseek 等平台共享）。
    #[test]
    fn members_share_registered_family_adapter() {
        let cn = lookup("sensenova").expect("SenseNova adapter must be registered");
        let en = lookup("sensenova_en").expect("SenseNova En adapter must be registered");

        assert!(std::ptr::eq(cn, en));
        // GENERIC 也是显式注册项（lookup 对它同样 Some），须断言成员不再回落通用适配。
        // 对照件用 devin（GENERIC 注册项；openrouter 自 issue 106 起为专属适配器）。
        assert!(!std::ptr::eq(cn, adapter_for("devin")));
        assert!(!std::ptr::eq(en, adapter_for("devin")));
        assert!(!std::ptr::eq(cn, adapter_for("openai")));
    }
}

#[cfg(test)]
mod opencode_family_tests {
    use super::*;

    #[test]
    fn members_share_registered_family_adapter() {
        let go = lookup("opencode").expect("OpenCode Go adapter must be registered");
        let zen = lookup("opencode_zen").expect("OpenCode Zen adapter must be registered");

        assert!(std::ptr::eq(go, zen));
        assert!(!std::ptr::eq(go, adapter_for("openai")));
    }
}

#[cfg(test)]
mod openai_family_tests {
    use super::*;

    #[test]
    fn members_share_registered_family_adapter() {
        let openai = lookup("openai").expect("OpenAI adapter must be registered");
        let codex = lookup("codex").expect("Codex adapter must be registered");
        assert!(std::ptr::eq(openai, codex));
        // GENERIC 也是显式注册项（lookup 对它同样 Some），须断言成员不再回落通用适配。
        // 对照件用 devin（GENERIC 注册项；openrouter 自 issue 106 起为专属适配器）。
        assert!(!std::ptr::eq(openai, adapter_for("devin")));
        assert!(!std::ptr::eq(codex, adapter_for("devin")));
        assert!(!std::ptr::eq(openai, adapter_for("anthropic")));
    }
}

#[cfg(test)]
mod anthropic_family_tests {
    use super::*;

    #[test]
    fn members_share_registered_family_adapter() {
        let anthropic = lookup("anthropic").expect("Anthropic adapter must be registered");
        let claude_code = lookup("claude_code").expect("Claude Code adapter must be registered");

        assert!(std::ptr::eq(anthropic, claude_code));
        assert!(!std::ptr::eq(anthropic, adapter_for("openai")));
    }
}

#[cfg(test)]
mod cline_family_tests {
    use super::*;

    #[test]
    fn cline_members_request_openai_response_shape() {
        for code in ["cline", "cline_pass"] {
            let adapter = adapter_for(code);
            assert_eq!(
                adapter.additional_request_headers(&Protocol::OpenAI, false),
                &[("x-response-format", "openai")],
                "{code} non-stream OpenAI response shape"
            );
            assert!(adapter.additional_request_headers(&Protocol::OpenAI, true).is_empty());
            assert!(adapter.additional_request_headers(&Protocol::Anthropic, false).is_empty());
        }
    }
}

#[cfg(test)]
mod aihubmix_tests {
    use super::*;

    /// issue 91：aihubmix 必须走专属适配器（显式注册），不再回落通用适配。
    /// 对照件用 devin（GENERIC 注册项；openrouter 自 issue 106 起为专属适配器）。
    #[test]
    fn aihubmix_uses_registered_adapter() {
        let a = adapter_for("aihubmix");
        assert!(!std::ptr::eq(a, adapter_for("devin")));
    }

    /// 官方把错误体 response ID 改为 tid（traceId）；分类仍读 error.message/type，
    /// tid 字段不得影响分类（与基座函数逐字节等价）。
    #[test]
    fn aihubmix_error_classification_ignores_tid_field() {
        let body = r#"{"error":{"type":"invalid_request_error","message":"model not found: gpt-x","tid":"trace-91"}}"#;
        assert_eq!(
            adapter_for("aihubmix").classify_error(400, body),
            error::classify_upstream_error(400, body)
        );
    }

    /// usage 抽取三 arm：anthropic（缓存读/写不扣 input）、openai（cached_tokens 在
    /// input 内须扣）、gemini（usageMetadata）。
    #[test]
    fn aihubmix_usage_extraction_matches_wire_shapes() {
        let a = adapter_for("aihubmix");
        let anthropic = r#"{"usage":{"input_tokens":7,"output_tokens":3,"cache_read_input_tokens":2,"cache_creation_input_tokens":4}}"#;
        assert_eq!(
            a.extract_usage(anthropic),
            (7, 3, 2, 4, 0, Vec::<(String, i64)>::new())
        );
        let openai = r#"{"usage":{"prompt_tokens":10,"completion_tokens":2,"prompt_tokens_details":{"cached_tokens":4}}}"#;
        assert_eq!(
            a.extract_usage(openai),
            (6, 2, 4, 0, 0, Vec::<(String, i64)>::new())
        );
        let gemini = r#"{"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":2}}"#;
        assert_eq!(
            a.extract_usage(gemini),
            (5, 2, 0, 0, 0, Vec::<(String, i64)>::new())
        );
    }
}

#[cfg(test)]
mod deepseek_tests {
    use super::*;

    /// issue 95：deepseek 必须走专属适配器（显式注册），不再回落通用适配。
    #[test]
    fn deepseek_uses_dedicated_adapter() {
        let a = adapter_for("deepseek");
        assert!(!std::ptr::eq(a, lookup("devin").unwrap()));
    }

    /// 官方禁思考写法是顶层 `thinking: {"type": "disabled"}`（思考开关缺省 = 开启，
    /// https://api-docs.deepseek.com/api/create-chat-completion 的 thinking 参数、
    /// https://api-docs.deepseek.com/guides/thinking_mode ）；openai wire 不写基座第三方
    /// vLLM 式 `chat_template_kwargs.enable_thinking`。anthropic wire 官方同词表，
    /// 走基座 anthropic 臂；强制思考路径只剥不写（基座）。
    #[test]
    fn deepseek_openai_disable_thinking_writes_thinking_disabled() {
        let pa = adapter_for("deepseek");
        let mut body = serde_json::json!({
            "model": "deepseek-flash",
            "enable_thinking": true,
            "thinking": {"type": "enabled"},
            "reasoning_effort": "high",
            "chat_template_kwargs": {"enable_thinking": true},
        });
        pa.apply_disable_thinking(
            &mut body,
            true,
            &Protocol::OpenAI,
            "https://api.deepseek.com/v1",
            false,
        );
        assert_eq!(body["thinking"], serde_json::json!({"type": "disabled"}), "{body}");
        assert!(body.get("reasoning_effort").is_none(), "{body}");
        assert!(
            body.get("chat_template_kwargs")
                .and_then(|c| c.get("enable_thinking"))
                .is_none(),
            "{body}"
        );
        // anthropic wire：官方 enabled/disabled 词表同基座 anthropic 臂。
        let mut anth = serde_json::json!({"model": "deepseek-flash"});
        pa.apply_disable_thinking(
            &mut anth,
            true,
            &Protocol::Anthropic,
            "https://api.deepseek.com/anthropic",
            false,
        );
        assert_eq!(anth["thinking"]["type"], "disabled", "{anth}");
        // 强制思考：只剥禁用形参数、不写任何禁用参数（基座）。
        let mut forced = serde_json::json!({"model": "deepseek-flash", "thinking": {"type": "disabled"}});
        pa.apply_disable_thinking(&mut forced, true, &Protocol::OpenAI, "https://stub", true);
        assert!(forced.get("thinking").is_none(), "{forced}");
    }

    /// tool_choice：思考模式下 openai 端点 `required` 与命名 choice 报 400、anthropic 端点
    /// `any`（仅客户端工具时）/ `tool` 报 400（https://api-docs.deepseek.com/api/create-chat-completion
    /// tool_choice 参数、https://api-docs.deepseek.com/guides/anthropic_api Tool Fields 节）。
    /// 思考缺省开启：无任何思考键即视为思考模式；`thinking.type=disabled` /
    /// `reasoning_effort="none"` 显式关闭时不降级（思考关闭时这些值官方合法）。
    #[test]
    fn deepseek_tool_choice_downgrades_in_thinking_mode() {
        let pa = adapter_for("deepseek");
        // 默认思考开启（无任何思考键）→ 降级。
        let mut body = serde_json::json!({"model": "deepseek-flash", "tool_choice": "required"});
        pa.adjust_tool_choice(&mut body, &Protocol::OpenAI, "deepseek-flash");
        assert_eq!(body["tool_choice"], "auto", "{body}");
        // 命名 choice 同样降级。
        let mut body = serde_json::json!({"model": "m", "tool_choice": {"type": "function", "function": {"name": "f"}}});
        pa.adjust_tool_choice(&mut body, &Protocol::OpenAI, "m");
        assert_eq!(body["tool_choice"], "auto", "{body}");
        // 显式禁思考 → 官方合法，保留。
        for mut body in [
            serde_json::json!({"model": "m", "tool_choice": "required", "thinking": {"type": "disabled"}}),
            serde_json::json!({"model": "m", "tool_choice": "required", "reasoning_effort": "none"}),
        ] {
            pa.adjust_tool_choice(&mut body, &Protocol::OpenAI, "m");
            assert_eq!(body["tool_choice"], "required", "{body}");
        }
        // anthropic wire：思考模式 any / tool → auto。
        let mut body = serde_json::json!({"model": "m", "tool_choice": {"type": "any"}});
        pa.adjust_tool_choice(&mut body, &Protocol::Anthropic, "m");
        assert_eq!(body["tool_choice"]["type"], "auto", "{body}");
        let mut body = serde_json::json!({"model": "m", "tool_choice": {"type": "tool", "name": "f"}});
        pa.adjust_tool_choice(&mut body, &Protocol::Anthropic, "m");
        assert_eq!(body["tool_choice"]["type"], "auto", "{body}");
        // 禁思考时 anthropic wire 不降级。
        let mut body = serde_json::json!({"model": "m", "tool_choice": {"type": "any"}, "thinking": {"type": "disabled"}});
        pa.adjust_tool_choice(&mut body, &Protocol::Anthropic, "m");
        assert_eq!(body["tool_choice"]["type"], "any", "{body}");
        // auto / none / 无 tool_choice 不动。
        let mut body = serde_json::json!({"model": "m", "tool_choice": "auto"});
        pa.adjust_tool_choice(&mut body, &Protocol::OpenAI, "m");
        assert_eq!(body["tool_choice"], "auto", "{body}");
        let mut body = serde_json::json!({"model": "m"});
        pa.adjust_tool_choice(&mut body, &Protocol::OpenAI, "m");
        assert!(body.get("tool_choice").is_none(), "{body}");
    }

    /// 思考模式 + tools 的多轮工具历史须回传 reasoning_content（thinking_mode 工具调用节
    /// 「带 tools 时前序各轮 reasoning_content 应回传并拼入上下文」；anthropic 官方文档：
    /// 缺失时服务端按 tool-call ID 恢复，恢复失败即 400，「Do not rely on automatic
    /// recovery」）。anthropic 入站的 thinking 块在 to_openai 转换中被跳过 → openai wire
    /// 从 ChatRequest 的 thinking 块取回文本（与 hooks::carry_history_reasoning 同形）。
    #[test]
    fn deepseek_openai_tool_history_carries_reasoning_content() {
        let chat_req: crate::types::ChatRequest = serde_json::from_value(serde_json::json!({
            "model": "deepseek-v4-pro",
            "max_tokens": 64,
            "messages": [
                {"role": "user", "content": "北京天气?"},
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "需要调用天气工具", "signature": "sig"},
                    {"type": "tool_use", "id": "tu1", "name": "get_weather", "input": {"city": "北京"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "tu1", "content": "晴"}
                ]}
            ]
        }))
        .unwrap();
        let (body, _) = adapter_for("deepseek").build_request(
            &chat_req,
            &Protocol::OpenAI,
            &Protocol::DeepSeek,
        );
        let asst = body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["role"] == "assistant" && m.get("tool_calls").is_some_and(|v| !v.is_null()))
            .expect("转换应产出 assistant tool-call 消息");
        assert_eq!(asst["reasoning_content"], "需要调用天气工具", "{asst}");
    }

    /// usage：DeepSeek 缓存计数 `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`
    /// 与 `prompt_tokens_details.cached_tokens` 同源（create-chat-completion CompletionUsage
    /// required 字段集），基座 arm 已扣 input（回归护栏，旧码已绿）。
    #[test]
    fn deepseek_usage_cache_arms_follow_base() {
        let body = r#"{"usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"prompt_cache_hit_tokens":30,"prompt_cache_miss_tokens":70,"prompt_tokens_details":{"cached_tokens":30}}}"#;
        assert_eq!(
            adapter_for("deepseek").extract_usage(body),
            (70, 20, 30, 0, 0, Vec::<(String, i64)>::new())
        );
    }

    /// 错误分类：官方错误码表（https://api-docs.deepseek.com/quick_start/error_codes）
    /// 401 鉴权 / 402 余额 / 429 限频按状态码归服务类；400 格式错 / 422 参数错的模型类
    /// 语义由 message 特征判定；非流式错误体 JSON schema 官方未给 → 基座分类不扩展
    /// （回归护栏，旧码已绿）。
    #[test]
    fn deepseek_error_classification_tracks_base() {
        let pa = adapter_for("deepseek");
        assert_eq!(
            pa.classify_error(402, r#"{"error":{"message":"Insufficient Balance"}}"#),
            error::UpstreamErrorClass::Service
        );
        assert_eq!(
            pa.classify_error(422, r#"{"error":{"message":"model not found: deepseek-x"}}"#),
            error::UpstreamErrorClass::Model
        );
        assert_eq!(
            pa.classify_error(429, r#"{"error":{"message":"rate limit"}}"#),
            error::UpstreamErrorClass::Service
        );
    }
}
