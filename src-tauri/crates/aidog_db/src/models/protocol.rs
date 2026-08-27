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
    /// MiniMax Coding Plan（Token Plan）独立协议（与 glm_coding 同构，JSON key `minimax_coding`）。
    /// 端点与按量版同域（api.minimaxi.com），靠 `sk-cp` 前缀订阅 Key 区分计费，无独立 per-token 价目。
    #[serde(rename = "minimax_coding")]
    MinimaxCoding,
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
    /// CLI 代理（cpa-standalone-module）：platform_type 仅作平台标识，
    /// wire/base_url/api_key/models 由 candidate resolve 时从 `cli_proxy_provider` 表拉
    /// （`extra.cli_proxy_provider_id` 关联）。`platform.models` 字段只读，被 provider.models 覆盖。
    #[serde(rename = "cli-proxy")]
    CliProxy,
    /// Devin（Cognition）平台：特殊平台，接入走 handler.rs 平台分支不经 wire 协议层。
    /// API base `https://api.devin.ai`，Bearer `cog_` key + `org-` 前缀 org_id，计费 ACU，无原生流式。
    /// preset endpoints 为空（无标准 wire endpoint），models 5 档虚拟映射 devin-normal/fast/lite/ultra/fusion。
    #[serde(rename = "devin")]
    Devin,
}

impl Protocol {
    /// wire 协议名（= serde rename 值），用于 DB `source_protocol`/`target_protocol` 字符串字段
    /// 和 tracing 日志。复用 serde 而非另建映射表，避免与 `#[serde(rename)]` 定义漂移。
    pub fn wire_str(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default()
    }

    /// DB `platform.platform_type` 列解析（永不 panic）。
    ///
    /// 该列历史上既存 JSON 字符串（`"glm"`，`serde_json::to_string` 写入的正常形态），也存在
    /// 裸 wire 名（`minimax_coding`，外部工具 / 老 seed SQL 直写）。两种形态都要认。
    ///
    /// 🔴 禁改回 `unwrap()`：本函数在 `row_to_platform` 内、tokio-rusqlite 后台线程上执行，
    /// panic 会杀死该条连接的后台线程 → 该连接永久返 `ConnectionClosed` → platform 读池逐条
    /// 耗尽 → 路由 `get_group_platforms` 失败 → 全部代理请求 400 "route error: ConnectionClosed"
    /// （2026-08-28 现场：DB 存了裸 `minimax_coding`，整池死，UI + 转发同时瘫）。
    ///
    /// 无法识别的值（枚举里没有的协议名，如降级到旧版本读新版本写的数据）回落 `Anthropic`
    /// 并打 warn：平台可能路由到错误 wire 格式，但服务存活、日志可定位，好过整池崩。
    pub fn from_db_str(raw: &str) -> Protocol {
        if let Ok(p) = serde_json::from_str::<Protocol>(raw) {
            return p;
        }
        let bare = raw.trim().trim_matches('"');
        if let Ok(p) = serde_json::from_str::<Protocol>(&format!("\"{bare}\"")) {
            return p;
        }
        tracing::warn!(
            raw = %raw,
            "unknown platform_type in DB, falling back to anthropic"
        );
        Protocol::Anthropic
    }

    /// 判定两个协议是否属于同一「wire family」（可互相跳过响应转换 / 共用错误体与 SSE 渲染）。
    /// openai / openai_completions / openai_responses 三者共享同一渲染族；其余协议仅与自身同族。
    ///
    /// 注意：这与「端点透传精确匹配」是两回事——透传要求 body 结构完全一致
    /// （openai_responses 与 openai 的请求/响应结构不同），forward.rs 的
    /// `same_protocol_passthrough` 判定必须用精确 `==`，不可用本方法替代。
    pub fn same_wire_family(&self, other: &Protocol) -> bool {
        use Protocol::*;
        let is_openai_family = |p: &Protocol| matches!(p, OpenAI | OpenAICompletions | OpenAIResponses);
        (is_openai_family(self) && is_openai_family(other)) || self == other
    }

    /// 厂商直连平台（glm / kimi / minimax / deepseek 等官方端点固定）端点锁死：
    /// 禁止用户填写 / 修改协议端点，保存时强制重置为内置 preset 端点（db/platform.rs）。
    /// 前端镜像集合：`src/domains/platforms/constants.ts::ENDPOINTS_LOCKED_PROTOCOLS`（跨层对称，禁单侧改）。
    /// 通用平台（5 wire 协议 + 聚合 / 第三方 / 中转段 + cli_proxy）不受限。
    pub fn endpoints_locked(&self) -> bool {
        use Protocol::*;
        matches!(
            self,
            Mock | ClaudeCode
                | Glm | GlmCoding | GlmEn | Kimi | KimiCoding
                | MiniMax | MiniMaxEn | MinimaxCoding | Codex | Bailian | BailianCoding
                | DeepSeek | StepFun | StepFunEn | Doubao | BytePlus
                | QianFan | QianfanCoding | XiaomiMimo | XiaomiMimoCoding
                | Longcat | SenseNova | Devin
        )
    }
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
mod test_endpoints_locked {
    use super::*;

    /// 跨层对称锚点：locked 集合与前端 `ENDPOINTS_LOCKED_PROTOCOLS`（constants.ts）必须同集。
    /// 通用段（wire 协议 / 聚合 / 第三方 / 中转 / cli_proxy）抽查不锁，厂商段抽查锁死。
    #[test]
    fn endpoints_locked_set() {
        let locked: &[(&str, bool)] = &[
            ("glm", true), ("glm_coding", true), ("kimi", true), ("minimax", true),
            ("minimax_coding", true), ("deepseek", true), ("qianfan_coding", true), ("xiaomi_mimo", true),
            ("mock", true), ("claude_code", true), ("devin", true),
            ("anthropic", false), ("openai", false), ("gemini", false),
            ("newapi", false), ("openrouter", false), ("packycode", false),
            ("siliconflow", false), ("cli-proxy", false), ("opencode_zen", false),
        ];
        for (key, expect) in locked {
            let p: Protocol = serde_json::from_str(&format!("\"{key}\"")).unwrap();
            assert_eq!(p.endpoints_locked(), *expect, "{key} locked mismatch");
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
            // MiniMax Coding Plan 独立协议（minimax_coding）
        ("minimax_coding", Protocol::MinimaxCoding),
        // CLI 代理独立协议（cpa-standalone-module s2）
            ("cli-proxy", Protocol::CliProxy),
            // Devin 平台（add-devin-support s1）
            ("devin", Protocol::Devin),
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
mod test_from_db_str {
    use super::*;

    /// 正常形态：`serde_json::to_string` 写入的带引号 JSON 字符串。
    #[test]
    fn quoted_json_form_parses() {
        assert_eq!(Protocol::from_db_str("\"glm_coding\""), Protocol::GlmCoding);
        assert_eq!(Protocol::from_db_str("\"anthropic\""), Protocol::Anthropic);
    }

    /// 裸 wire 名（外部工具 / 老 seed SQL 直写）——2026-08-28 整池 ConnectionClosed 现场形态。
    #[test]
    fn bare_wire_name_parses() {
        assert_eq!(Protocol::from_db_str("minimax_coding"), Protocol::MinimaxCoding);
        assert_eq!(Protocol::from_db_str("glm"), Protocol::Glm);
        assert_eq!(Protocol::from_db_str(" newapi "), Protocol::NewApi);
    }

    /// 未知值回落 Anthropic 且不 panic（降级运行 / 未来新协议读旧版本）。
    #[test]
    fn unknown_falls_back_without_panic() {
        assert_eq!(Protocol::from_db_str("no_such_protocol"), Protocol::Anthropic);
        assert_eq!(Protocol::from_db_str(""), Protocol::Anthropic);
        assert_eq!(Protocol::from_db_str("{\"a\":1}"), Protocol::Anthropic);
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
