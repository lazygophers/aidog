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

/// 家族示例：KimiAdapter 只登记在 `kimi` 上（platform_type = Kimi）。
/// 基座默认的 coding_plan 注入按平台协议分派（仅 Kimi 协议注入 prompt_cache_key）；
/// 家族成员的无条件注入在 platform=kimi 时与基座 Kimi 臂产出一致——演示「成员只覆盖差异」。
/// `kimi_en` / `kimi_coding` 是独立协议（KimiEn / KimiCoding），走默认基座，不注入
/// （迁移前 headers.rs 同为 no-op，review 必改 1：行为零变化）。
pub struct KimiAdapter;

impl PlatformAdapter for KimiAdapter {
    fn inject_coding_plan(&self, body: &mut Value, _platform: &Protocol) {
        hooks::inject_kimi_prompt_cache_key(body);
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
/// 覆写范围：`classify_error` 的业务码 1212（issue 114）与 `apply_disable_thinking` 的 `Protocol::OpenAI` 臂；
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
    fn classify_error(&self, status: u16, body: &str) -> error::UpstreamErrorClass {
        let is_1212 = !matches!(status, 401 | 402 | 403 | 429)
            && serde_json::from_str::<Value>(body).is_ok_and(|v| v["error"]["code"] == "1212");
        if is_1212 {
            return error::UpstreamErrorClass::Model;
        }
        error::classify_upstream_error(status, body)
    }
}

static GENERIC: GenericAdapter = GenericAdapter;
static KIMI: KimiAdapter = KimiAdapter;
static GLM: GlmFamilyAdapter = GlmFamilyAdapter;

/// 显式注册表：registry 平台 code → 适配器单例（与 src-tauri/defaults/registry/platforms/ 一一对应；
/// 初始映射全部为默认家族基座，Kimi 家族覆盖 coding_plan 注入差异，行为与基座协议臂等价）。
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
    ("kimi_coding", &GENERIC),
    ("kimi_en", &GENERIC),
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
