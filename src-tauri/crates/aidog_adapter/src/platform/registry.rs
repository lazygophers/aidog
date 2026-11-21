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

static GENERIC: GenericAdapter = GenericAdapter;
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

/// 显式注册表：registry 平台 code → 适配器单例（与 src-tauri/defaults/registry/platforms/ 一一对应；
/// 未列入家族适配器的平台走默认家族基座）。
static REGISTRY: &[(&str, &dyn PlatformAdapter)] = &[
    ("aihubmix", &GENERIC),
    ("anthropic", &GENERIC),
    ("atlascloud", &GENERIC),
    ("bailian", &BAILIAN),
    ("bailian_coding", &BAILIAN),
    ("bailian_coding_en", &BAILIAN),
    ("bailian_en", &BAILIAN),
    ("byteplus", &ARK),
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
    ("doubao", &ARK),
    ("gemini", &GENERIC),
    ("glm", &GLM),
    ("glm_coding", &GLM),
    ("glm_coding_en", &GLM),
    ("glm_en", &GLM),
    ("intern_inkstone", &GENERIC),
    ("kimi", &KIMI),
    ("kimi_coding", &KIMI_CODING),
    ("kimi_en", &KIMI),
    ("litellm", &GENERIC),
    ("longcat", &GENERIC),
    ("magpie", &GENERIC),
    ("meta", &GENERIC),
    ("minimax", &MINIMAX),
    ("minimax_coding", &MINIMAX_CODING),
    ("minimax_en", &MINIMAX_EN),
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
    ("qianfan", &QIANFAN),
    ("qianfan_coding", &QIANFAN),
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
