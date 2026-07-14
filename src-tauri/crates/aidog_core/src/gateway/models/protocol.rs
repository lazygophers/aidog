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
    /// 智谱 GLM Coding Plan 独立协议（PRD 07-09 D1）：编码套餐端点
    /// base_url `/api/coding/paas/v4`（比普通版多 `/coding/`），peak_hours 仅 GLM-5.2 / 5-Turbo 高阶倍率。
    /// 用户决策恢复独立协议（2026-07-09），与 endpoint `coding_plan` flag 机制并存。
    #[serde(rename = "glm_coding")]
    GlmCoding,
    #[serde(rename = "glm_en")]
    GlmEn,
    #[serde(rename = "kimi")]
    Kimi,
    /// Kimi Coding Plan 独立协议（与 glm_coding 同构，JSON key `kimi_coding`，
    /// 自带独立 endpoints/models/model_list 分支）。
    #[serde(rename = "kimi_coding")]
    KimiCoding,
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
    /// QianFan (百度千帆) Coding Plan 独立协议（与 glm_coding 同构，JSON key `qianfan_coding`）。
    #[serde(rename = "qianfan_coding")]
    QianfanCoding,
    #[serde(rename = "xiaomi_mimo")]
    XiaomiMimo,
    /// XiaomiMimo Coding Plan 独立协议（与 glm_coding 同构，JSON key `xiaomi_mimo_coding`）。
    #[serde(rename = "xiaomi_mimo_coding")]
    XiaomiMimoCoding,
    #[serde(rename = "bailing")]
    BaiLing,
    #[serde(rename = "longcat")]
    Longcat,
    #[serde(rename = "sensenova")]
    SenseNova,
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
    // ── CPA(CLIProxyAPI)导入平台类型 ──
    // 4 协议均无独立 Rust adapter：endpoints[].protocol 决定 wire format
    // (cpa-grok→openai_responses / 其余→gemini)，platform_type 仅作平台标识。
    /// xAI Grok 原生 `/responses` 端点（OAuth token 当 api_key）。wire = openai_responses。
    #[serde(rename = "cpa-grok")]
    CpaGrok,
    /// Google AI Studio 原生 generateContent（OAuth token 当 api_key）。wire = gemini。
    #[serde(rename = "cpa-aistudio")]
    CpaAistudio,
    /// Antigravity(Google Cloud Code internal) `/v1internal:*`：仅存配置，
    /// 路径 `/v1internal:streamGenerateContent` 与 gemini adapter 不兼容 → 路由暂不支持。
    #[serde(rename = "cpa-antigravity")]
    CpaAntigravity,
    /// Vertex AI：URL 含 projects/{p}/locations/{l}/publishers/google/models/ 结构，
    /// gemini adapter 不兼容 → 仅存配置，路由暂不支持。base_url region-specific 用户预览补全。
    #[serde(rename = "cpa-vertex")]
    CpaVertex,
    /// CLI 代理（cpa-standalone-module）：platform_type 仅作平台标识，
    /// wire/base_url/api_key/models 由 candidate resolve 时从 `cli_proxy_provider` 表拉
    /// （`extra.cli_proxy_provider_id` 关联）。`platform.models` 字段只读，被 provider.models 覆盖。
    #[serde(rename = "cli-proxy")]
    CliProxy,
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

#[cfg(test)]
mod test_protocol_coding_variants {
    use super::*;

    /// 3 个新增 cp 独立协议 + glm_coding 模板的 serde key round-trip：
    /// JSON 字符串 ↔ 枚举变体对称（与 platform-presets.json serde rename 对齐）。
    #[test]
    fn coding_variants_serde_roundtrip() {
        // Deserialize（JSON key → 枚举）
        let cases: &[(&str, Protocol)] = &[
            ("glm_coding", Protocol::GlmCoding),
            ("kimi_coding", Protocol::KimiCoding),
            ("qianfan_coding", Protocol::QianfanCoding),
            ("xiaomi_mimo_coding", Protocol::XiaomiMimoCoding),
            // CPA(CLIProxyAPI)导入 4 协议（PRD cpa-import s2）
            ("cpa-grok", Protocol::CpaGrok),
            ("cpa-aistudio", Protocol::CpaAistudio),
            ("cpa-antigravity", Protocol::CpaAntigravity),
            ("cpa-vertex", Protocol::CpaVertex),
            // CLI 代理独立协议（cpa-standalone-module s2）
            ("cli-proxy", Protocol::CliProxy),
        ];
        for (key, expected) in cases {
            let json = format!("\"{key}\"");
            let got: Protocol = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{key}: {e}"));
            assert_eq!(&got, expected, "deserialize mismatch for {key}");
            // Serialize（枚举 → JSON key，round-trip 对称）
            let back = serde_json::to_string(&got).unwrap();
            assert_eq!(back, json, "serialize mismatch for {:?}", expected);
        }
    }

    /// 非独立协议基线变体不受新增变体影响。
    #[test]
    fn non_coding_base_variants_still_parse() {
        assert_eq!(
            serde_json::from_str::<Protocol>("\"kimi\"").unwrap(),
            Protocol::Kimi
        );
        assert_eq!(
            serde_json::from_str::<Protocol>("\"minimax\"").unwrap(),
            Protocol::MiniMax
        );
        assert_eq!(
            serde_json::from_str::<Protocol>("\"minimax_en\"").unwrap(),
            Protocol::MiniMaxEn
        );
        assert_eq!(
            serde_json::from_str::<Protocol>("\"qianfan\"").unwrap(),
            Protocol::QianFan
        );
        assert_eq!(
            serde_json::from_str::<Protocol>("\"xiaomi_mimo\"").unwrap(),
            Protocol::XiaomiMimo
        );
    }
}

#[cfg(test)]
mod test_routing_mode {
    use super::*;

    #[test]
    fn from_str_or_default_all_variants() {
        assert_eq!(RoutingMode::from_str_or_default("failover"), RoutingMode::Failover);
        assert_eq!(RoutingMode::from_str_or_default("health_aware"), RoutingMode::HealthAware);
        assert_eq!(RoutingMode::from_str_or_default("least_latency"), RoutingMode::LeastLatency);
        assert_eq!(RoutingMode::from_str_or_default("sticky"), RoutingMode::Sticky);
        assert_eq!(RoutingMode::from_str_or_default("load_balance"), RoutingMode::LoadBalance);
        assert_eq!(RoutingMode::from_str_or_default("unknown"), RoutingMode::LoadBalance);
        assert_eq!(RoutingMode::from_str_or_default(""), RoutingMode::LoadBalance);
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

#[cfg(test)]
mod test_platform_status {
    use super::*;

    #[test]
    fn as_db_str_roundtrip() {
        assert_eq!(PlatformStatus::Enabled.as_db_str(), "enabled");
        assert_eq!(PlatformStatus::Disabled.as_db_str(), "disabled");
        assert_eq!(PlatformStatus::AutoDisabled.as_db_str(), "auto_disabled");
    }

    #[test]
    fn from_db_str_all_variants() {
        assert_eq!(PlatformStatus::from_db_str("enabled"), PlatformStatus::Enabled);
        assert_eq!(PlatformStatus::from_db_str("disabled"), PlatformStatus::Disabled);
        assert_eq!(PlatformStatus::from_db_str("auto_disabled"), PlatformStatus::AutoDisabled);
        assert_eq!(PlatformStatus::from_db_str("unknown"), PlatformStatus::Enabled);
        assert_eq!(PlatformStatus::from_db_str(""), PlatformStatus::Enabled);
    }
}
