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
/// - coding_plan 注入：沿用基座按平台协议分派（`Protocol::Kimi` / `Protocol::KimiCoding` 注入
///   `prompt_cache_key`，`kimi_en` no-op），行为不变；chat.md 称「对于 Kimi Code Plan，此字段为必填」。
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
///   `Protocol::KimiCoding` + coding_plan 端点注入（客户端自带键原样保留；缺省时按
///   metadata.session_id / user / model 派生稳定键，不按时间窗口轮换），非 coding_plan 端点
///   不注入（注入条件同源：端点 coding_plan flag）。https://platform.kimi.com/docs/api/chat.md
/// - usage：openai 臂 `usage.prompt_tokens_details.cached_tokens` / `cache_write_tokens`、anthropic 臂
///   `cache_read_input_tokens` / `cache_creation_input_tokens`，同家族基座（家族级取舍不区分缓存
///   写入，见基座说明），不覆盖。Kimi Code 专属文档（models.html / error-reference.html）未列 usage
///   字段。anthropic 端点请求侧新增顶层 `cache_control`（5m/1h TTL，不传默认 5m 读写）与
///   `output_config.effort`（messages.md schema）；aidog 不写即官方默认，不覆盖。
///   https://platform.kimi.com/docs/api/messages.md
/// - 流式 / 错误体：错误参考页条目均为「error, status code: <code>, message: <text>」文案形态，
///   JSON 错误体形状未证实；404「Not found the model kimi-for-coding or Permission denied」由基座
///   特征词 `not found the model` 归模型类、400「Your model id does not exist, recognized as …」
///   命中 `does not exist`（has_model 分支），429 配额与 401/403 按状态码归服务类，与基座一致，不覆盖。
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
        if self.coding_openai_reasoning_effort
            && wire == &Protocol::OpenAI
            && disable
            && matches!(
                body.get("model").and_then(Value::as_str),
                Some("k3.1-pro" | "k3.1-pro-highspeed")
            )
            && let Some(obj) = body.as_object_mut()
        {
            obj.remove("disable_thinking");
            obj.remove("thinking");
            obj.remove("context_management");
            obj.remove("reasoning");
            obj.remove("enable_thinking");
            if obj.get("reasoning_effort").and_then(Value::as_str) == Some("none") {
                obj.remove("reasoning_effort");
            }
            return;
        }
        // kimi_coding openai 端点：官方认收的禁思考写法是顶层 `reasoning_effort`（取值域
        // low/high/max + none；「off / none / disabled … 请求由 K2.8 Preview（无思考）处理」，
        // 「其他未知取值 返回 HTTP 400」），文档未列 `thinking` / `chat_template_kwargs` 参数。
        // 写法与基座官方 OpenAI host 臂同形（`reasoning_effort: "none"`）；K2.7 Code Highspeed
        // 「不支持调整思考程度」，禁思考后由 K2.8 Preview（无思考）处理，不额外剥除。
        // https://www.kimi.com/code/docs/kimi-code/models.html
        // https://platform.kimi.com/docs/api/chat.md
        if self.coding_openai_reasoning_effort
            && wire == &Protocol::OpenAI
            && disable
            && !forced_thinking
            && !matches!(
                body.get("model").and_then(Value::as_str),
                Some("k3.1-pro" | "k3.1-pro-highspeed")
            )
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
    ("kimi_coding", &KIMI_CODING),
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
