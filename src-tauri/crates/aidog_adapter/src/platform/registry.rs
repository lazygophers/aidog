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

/// 家族基座（通用适配）：全部接口与钩子 = trait 默认实现，
/// 与历史「按 wire 协议分派 + 通用钩子」链路逐字节等价。
struct GenericAdapter;

impl PlatformAdapter for GenericAdapter {}

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
/// - coding_plan 注入：沿用基座按平台协议分派（仅 `Protocol::Kimi` 注入 `prompt_cache_key`，
///   `kimi_en` / `kimi_coding` no-op），行为不变；chat.md 称「对于 Kimi Code Plan，此字段为必填」，
///   是否对 `kimi_coding` 启用归成员 issue 119。
/// - 未证实（保持基座默认）：anthropic 端点的 `thinking` 请求参数（messages.md 未列出）、
///   `content_filter` 错误类型（当前归「未知」类）。
pub struct KimiFamilyAdapter;

impl PlatformAdapter for KimiFamilyAdapter {
    /// 差异：openai 端点禁思考写官方 `thinking.type=disabled`，不写基座第三方端点用的
    /// vLLM 式 `chat_template_kwargs.enable_thinking`（Kimi Chat Completions 文档无此参数）。
    /// 写出值与基座 anthropic 臂相同，故按 anthropic 臂执行；其余 wire 与强制思考路径同基座。
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

static GENERIC: GenericAdapter = GenericAdapter;
static KIMI: KimiFamilyAdapter = KimiFamilyAdapter;
static GLM: GlmFamilyAdapter = GlmFamilyAdapter;

/// 显式注册表：registry 平台 code → 适配器单例（与 src-tauri/defaults/registry/platforms/ 一一对应；
/// 未列入家族适配器的平台走默认家族基座）。
static REGISTRY: &[(&str, &dyn PlatformAdapter)] = &[
    ("aihubmix", &GENERIC),
    ("anthropic", &GENERIC),
    ("atlascloud", &GENERIC),
    ("bailian", &GENERIC),
    ("bailian_coding", &GENERIC),
    ("bailian_coding_en", &GENERIC),
    ("bailian_en", &GENERIC),
    ("byteplus", &GENERIC),
    ("claude_code", &GENERIC),
    ("cline", &GENERIC),
    ("cline_pass", &GENERIC),
    ("codex", &GENERIC),
    ("commandcode", &GENERIC),
    ("compshare", &GENERIC),
    ("compshare_coding", &GENERIC),
    ("crazyrouter", &GENERIC),
    ("deepseek", &GENERIC),
    ("devin", &GENERIC),
    ("doubao", &GENERIC),
    ("gemini", &GENERIC),
    ("glm", &GLM),
    ("glm_coding", &GLM),
    ("glm_coding_en", &GLM),
    ("glm_en", &GLM),
    ("intern_inkstone", &GENERIC),
    ("kimi", &KIMI),
    ("kimi_coding", &KIMI),
    ("kimi_en", &KIMI),
    ("litellm", &GENERIC),
    ("longcat", &GENERIC),
    ("magpie", &GENERIC),
    ("meta", &GENERIC),
    ("minimax", &GENERIC),
    ("minimax_coding", &GENERIC),
    ("minimax_en", &GENERIC),
    ("mistral", &GENERIC),
    ("modelscope", &GENERIC),
    ("newapi", &GENERIC),
    ("novita", &GENERIC),
    ("nvidia", &GENERIC),
    ("openai", &GENERIC),
    ("opencode", &GENERIC),
    ("opencode_zen", &GENERIC),
    ("openrouter", &GENERIC),
    ("passthrough", &GENERIC),
    ("pipellm", &GENERIC),
    ("qianfan", &GENERIC),
    ("qianfan_coding", &GENERIC),
    ("sensenova", &GENERIC),
    ("sensenova_en", &GENERIC),
    ("shengsuanyun", &GENERIC),
    ("siliconflow", &GENERIC),
    ("siliconflow_en", &GENERIC),
    ("stepfun", &GENERIC),
    ("stepfun_en", &GENERIC),
    ("tencent_coding", &GENERIC),
    ("tencent_tokenhub", &GENERIC),
    ("tencent_tokenhub_intl", &GENERIC),
    ("therouter", &GENERIC),
    ("tokenrhythm", &GENERIC),
    ("typesafe", &GENERIC),
    ("xai", &GENERIC),
    ("xiaomi_mimo", &GENERIC),
    ("xiaomi_mimo_coding", &GENERIC),
    ("xiaomi_mimo_coding_en", &GENERIC),
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
