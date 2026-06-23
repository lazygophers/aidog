//! 协议枚举与平台/路由状态枚举。

use serde::{Deserialize, Serialize};

/// 支持的 AI 协议类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    // ── AI 请求协议（可作为 endpoint 协议）──
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "openai_responses")]
    OpenAIResponses,
    #[serde(rename = "openai_completions")]
    OpenAICompletions,
    #[serde(rename = "gemini")]
    Gemini,
    // ── 平台类型（仅作为平台主协议，不作为 endpoint 协议）──
    #[serde(rename = "mock")]
    Mock,
    /// Claude Code 原始订阅平台（纯透传，客户端自带 OAuth 认证）
    #[serde(rename = "claude_code")]
    ClaudeCode,
    #[serde(rename = "glm")]
    Glm,
    #[serde(rename = "glm_en")]
    GlmEn,
    #[serde(rename = "kimi")]
    Kimi,
    #[serde(rename = "minimax")]
    MiniMax,
    #[serde(rename = "minimax_en")]
    MiniMaxEn,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "bailian")]
    Bailian,
    #[serde(rename = "bailian_coding")]
    BailianCoding,
    // ── 国内官方平台 ──
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "stepfun")]
    StepFun,
    #[serde(rename = "stepfun_en")]
    StepFunEn,
    #[serde(rename = "doubao")]
    Doubao,
    #[serde(rename = "byteplus")]
    BytePlus,
    #[serde(rename = "qianfan")]
    QianFan,
    #[serde(rename = "xiaomi_mimo")]
    XiaomiMimo,
    #[serde(rename = "bailing")]
    BaiLing,
    #[serde(rename = "longcat")]
    Longcat,
    // ── 聚合平台 ──
    #[serde(rename = "openrouter")]
    OpenRouter,
    #[serde(rename = "siliconflow")]
    SiliconFlow,
    #[serde(rename = "siliconflow_en")]
    SiliconFlowEn,
    #[serde(rename = "aihubmix")]
    AiHubMix,
    #[serde(rename = "dmxapi")]
    DmxApi,
    #[serde(rename = "modelscope")]
    ModelScope,
    #[serde(rename = "shengsuanyun")]
    ShengSuanYun,
    #[serde(rename = "atlascloud")]
    AtlasCloud,
    #[serde(rename = "novita")]
    Novita,
    #[serde(rename = "therouter")]
    TheRouter,
    #[serde(rename = "cherryin")]
    CherryIn,
    // ── 第三方平台 ──
    #[serde(rename = "packycode")]
    PackyCode,
    #[serde(rename = "cubence")]
    Cubence,
    #[serde(rename = "aigocode")]
    AiGoCode,
    #[serde(rename = "rightcode")]
    RightCode,
    #[serde(rename = "aicodemirror")]
    AiCodeMirror,
    #[serde(rename = "nvidia")]
    Nvidia,
    #[serde(rename = "pateway")]
    Pateway,
    #[serde(rename = "ccsub")]
    CcSub,
    #[serde(rename = "apikeyfun")]
    ApiKeyFun,
    #[serde(rename = "apinebula")]
    ApiNebula,
    #[serde(rename = "sudocode")]
    SudoCode,
    #[serde(rename = "claudeapi")]
    ClaudeApi,
    #[serde(rename = "claudecn")]
    ClaudeCN,
    #[serde(rename = "runapi")]
    RunApi,
    #[serde(rename = "relaxycode")]
    RelaxyCode,
    #[serde(rename = "crazyrouter")]
    CrazyRouter,
    #[serde(rename = "sssaicode")]
    SssAiCode,
    #[serde(rename = "compshare")]
    Compshare,
    #[serde(rename = "compshare_coding")]
    CompshareCoding,
    #[serde(rename = "micu")]
    Micu,
    #[serde(rename = "ctok")]
    CTok,
    #[serde(rename = "eflowcode")]
    EFlowCode,
    #[serde(rename = "lemondata")]
    LemonData,
    #[serde(rename = "pipellm")]
    PipeLlm,
    #[serde(rename = "opencode")]
    OpenCode,
    /// OpenCode Zen 免费版（OpenAI 兼容，base_url https://opencode.ai/zen/v1；
    /// 免费模型靠 catalog 定价 0；api_key 留空时 proxy 注入 $opencode 匿名免费 key）
    #[serde(rename = "opencode_zen")]
    OpenCodeZen,
    // ── 中转平台 ──
    #[serde(rename = "newapi")]
    NewApi,
}

/// 路由模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum RoutingMode {
    #[serde(rename = "load_balance")]
    LoadBalance,
    #[serde(rename = "failover")]
    Failover,
    /// 健康集加权随机：准入门摘除熔断 Open 平台后，在健康平台中按 weight 加权随机。
    #[serde(rename = "health_aware")]
    HealthAware,
    /// 最小延迟：按 per-platform 延迟 EMA 升序。
    #[serde(rename = "least_latency")]
    LeastLatency,
    /// 粘性会话：session 键绑定平台（若健康），否则回退加权随机并写绑定。
    #[serde(rename = "sticky")]
    Sticky,
}

impl RoutingMode {
    /// 从 settings 默认字面量解析；未知 → LoadBalance（向后兼容）。
    /// 供 SchedulingBreakerSettings::default_mode 与 GB 创建 Group 时取全局默认用。
    #[allow(dead_code)]
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "failover" => RoutingMode::Failover,
            "health_aware" => RoutingMode::HealthAware,
            "least_latency" => RoutingMode::LeastLatency,
            "sticky" => RoutingMode::Sticky,
            _ => RoutingMode::LoadBalance,
        }
    }
}

/// 平台状态三态：用户启用 / 用户手动禁用 / 401-403 自动禁用。
/// 自动禁用与手动禁用必须区分——自动恢复（退避试探 / 改 api_key）只作用于 auto_disabled，
/// 绝不误开用户主动关闭的平台。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PlatformStatus {
    #[serde(rename = "enabled")]
    #[default]
    Enabled,
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "auto_disabled")]
    AutoDisabled,
}

impl PlatformStatus {
    /// DB 文本值（与 `serde(rename)` 一致）
    pub fn as_db_str(&self) -> &'static str {
        match self {
            PlatformStatus::Enabled => "enabled",
            PlatformStatus::Disabled => "disabled",
            PlatformStatus::AutoDisabled => "auto_disabled",
        }
    }

    /// 从 DB 文本解析；未知值回退 Enabled（向后兼容旧库 / 脏数据）。
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "disabled" => PlatformStatus::Disabled,
            "auto_disabled" => PlatformStatus::AutoDisabled,
            _ => PlatformStatus::Enabled,
        }
    }
}
