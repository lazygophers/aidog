//! CPA Provider → aidog MappedPlatform 映射器。
//!
//! 路由规则（design.md 映射表）：
//! - `gemini-api-key` / `interactions-api-key` → `gemini`
//! - `codex-api-key`                          → `codex`
//! - `claude-api-key`                         → `anthropic`
//! - `vertex-api-key`                         → `cpa-vertex`
//! - `openai-compatibility`：name 关键词 → 对应 Protocol；无匹配 → `openai`
//! - OAuth channel：`xai`→`cpa-grok`; `vertex`→`cpa-vertex`;
//!   `aistudio`→`cpa-aistudio`; `antigravity`→`cpa-antigravity`;
//!   `claude`/`codex`/`kimi` → 各原生协议。
//!
//! 字段映射：base-url → base_url（OAuth 段 base_url 留空，前端预览从
//! `defaultClientForProtocol`/`getDefaultEndpoints` 回填，避免与 preset 重复）；
//! models[].name → available_models；prefix/headers → extra JSON；disabled=true
//! → apply 时 post-create UpdatePlatform 置 status=disabled（CreatePlatform 无 status 字段）。

use serde::{Deserialize, Serialize};

use crate::gateway::models::Protocol;

use super::parser::{CpaOAuthType, CpaProvider, CpaSourceSegment};

/// 映射后的 aidog 平台（cpa_import_parse 输出 / cpa_import_apply 输入）。
///
/// `protocol` + `base_url` + `api_key` + `models` + `extra` 直接对应 CreatePlatform
/// 字段；`disabled` 与 `source_label` 是 CPA 导入专属（apply 内部消费 / UI 展示）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedPlatform {
    /// 映射后的协议（platform_type）
    pub protocol: Protocol,
    /// 平台名称（openai-compat 从 name；OAuth 从 email；api-key 段由 protocol + host 派生）
    pub name: String,
    /// 上游 base URL（OAuth 段可能为空，前端预览回填）
    pub base_url: String,
    /// API key（OAuth = access_token）
    pub api_key: String,
    /// 可用模型列表（来自 models[].name，alias 丢）
    #[serde(default)]
    pub models: Vec<String>,
    /// 序列化后的 extra JSON（含 prefix/headers/cpa 源信息）
    #[serde(default)]
    pub extra: String,
    /// 是否禁用（来自 CPA `disabled=true`，apply 时 post-create 置 status=disabled）
    #[serde(default)]
    pub disabled: bool,
    /// 来源标签（UI 展示用，如 "openai-compatibility / glm"）
    pub source_label: String,
}

/// 将单个 CpaProvider 映射为 MappedPlatform。
pub fn map_provider(p: CpaProvider) -> MappedPlatform {
    let protocol = resolve_protocol(&p);
    let name = resolve_name(&p, &protocol);
    let source_label = format_source_label(&p, &protocol);
    let extra = build_extra(&p);

    MappedPlatform {
        protocol,
        name,
        base_url: p.base_url,
        api_key: p.api_key,
        models: p.models,
        extra,
        disabled: p.disabled,
        source_label,
    }
}

/// 批量映射。
pub fn map_providers(providers: Vec<CpaProvider>) -> Vec<MappedPlatform> {
    providers.into_iter().map(map_provider).collect()
}

// ─── 协议路由 ───────────────────────────────────────────────────────

fn resolve_protocol(p: &CpaProvider) -> Protocol {
    match p.source_segment {
        CpaSourceSegment::GeminiApiKey | CpaSourceSegment::InteractionsApiKey => Protocol::Gemini,
        CpaSourceSegment::CodexApiKey => Protocol::Codex,
        CpaSourceSegment::ClaudeApiKey => Protocol::Anthropic,
        CpaSourceSegment::VertexApiKey => Protocol::CpaVertex,
        CpaSourceSegment::OpenaiCompatibility => protocol_for_openai_compat_name(
            p.name.as_deref().unwrap_or(""),
        ),
        CpaSourceSegment::OAuth => match p.oauth_type {
            Some(CpaOAuthType::Xai) => Protocol::CpaGrok,
            Some(CpaOAuthType::Vertex) => Protocol::CpaVertex,
            Some(CpaOAuthType::Aistudio) => Protocol::CpaAistudio,
            Some(CpaOAuthType::Antigravity) => Protocol::CpaAntigravity,
            Some(CpaOAuthType::Claude) => Protocol::Anthropic,
            Some(CpaOAuthType::Codex) => Protocol::Codex,
            Some(CpaOAuthType::Kimi) => Protocol::Kimi,
            None => Protocol::OpenAI,
        },
    }
}

/// openai-compatibility 段 name 关键词 → Protocol。
/// ponytail: 关键词表手维护，与 platform-presets.json 68 协议 key 集对齐；
/// 新增 preset 协议时同步此处。无匹配兜底 `openai`。
fn protocol_for_openai_compat_name(name: &str) -> Protocol {
    let n = name.to_lowercase();
    // 顺序敏感：先具体后泛化（如 minimax_en 在 minimax 之前以保留 en 变体）。
    if n.contains("minimax_en") || n.contains("minimax-en") {
        Protocol::MiniMaxEn
    } else if n.contains("minimax") {
        Protocol::MiniMax
    } else if n.contains("glm_coding") || n.contains("glm-coding") {
        Protocol::GlmCoding
    } else if n.contains("glm") {
        Protocol::Glm
    } else if n.contains("kimi") || n.contains("moonshot") {
        Protocol::Kimi
    } else if n.contains("deepseek") {
        Protocol::DeepSeek
    } else if n.contains("qwen") || n.contains("bailian") || n.contains("tongyi") {
        Protocol::Bailian
    } else if n.contains("qianfan") {
        Protocol::QianFan
    } else if n.contains("openrouter") {
        Protocol::OpenRouter
    } else if n.contains("doubao") {
        Protocol::Doubao
    } else if n.contains("byteplus") {
        Protocol::BytePlus
    } else if n.contains("stepfun_en") || n.contains("stepfun-en") {
        Protocol::StepFunEn
    } else if n.contains("stepfun") {
        Protocol::StepFun
    } else if n.contains("siliconflow_en") || n.contains("siliconflow-en") {
        Protocol::SiliconFlowEn
    } else if n.contains("siliconflow") {
        Protocol::SiliconFlow
    } else if n.contains("newapi") || n.contains("new-api") || n.contains("one-api") {
        Protocol::NewApi
    } else if n.contains("modelscope") {
        Protocol::ModelScope
    } else if n.contains("novita") {
        Protocol::Novita
    } else if n.contains("atlascloud") || n.contains("atlas") {
        Protocol::AtlasCloud
    } else if n.contains("therouter") {
        Protocol::TheRouter
    } else if n.contains("longcat") {
        Protocol::Longcat
    } else if n.contains("sensenova") {
        Protocol::SenseNova
    } else if n.contains("aihubmix") {
        Protocol::AiHubMix
    } else if n.contains("dmxapi") {
        Protocol::DmxApi
    } else if n.contains("shengsuanyun") {
        Protocol::ShengSuanYun
    } else if n.contains("cherryin") {
        Protocol::CherryIn
    } else if n.contains("xiaomi") || n.contains("mimo") {
        Protocol::XiaomiMimo
    } else if n.contains("bailing") {
        Protocol::BaiLing
    } else {
        Protocol::OpenAI
    }
}

// ─── 名称与 extra ──────────────────────────────────────────────────

fn resolve_name(p: &CpaProvider, protocol: &Protocol) -> String {
    if let Some(n) = &p.name
        && !n.trim().is_empty()
    {
        return n.clone();
    }
    if let Some(email) = &p.name
        && p.source_segment == CpaSourceSegment::OAuth
        && !email.trim().is_empty()
    {
        return email.clone();
    }
    // 派生：protocol + host
    let host = host_of(&p.base_url);
    let proto_label = protocol_label(protocol);
    if host.is_empty() {
        proto_label
    } else {
        format!("{proto_label}-{host}")
    }
}

fn protocol_label(p: &Protocol) -> String {
    // ponytail: serde rename 即 preset key，直接序列化取值。
    serde_json::to_string(p)
        .ok()
        .and_then(|s| s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).map(String::from))
        .unwrap_or_else(|| format!("{p:?}").to_lowercase())
}

fn host_of(url: &str) -> String {
    // ponytail: 简单 host 提取，不引 url crate。取 :// 后第一段 path 前的主机名（剥端口、剥子域可选，此处保守留主域）。
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host_part = after_scheme.split('/').next().unwrap_or("");
    let host = host_part.split(':').next().unwrap_or("");
    // 剥掉常见子域 www./api. 前缀，保留主域（视觉友好）。
    host.split('.')
        .next_back()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_default()
}

fn format_source_label(p: &CpaProvider, protocol: &Protocol) -> String {
    let seg = match p.source_segment {
        CpaSourceSegment::GeminiApiKey => "gemini-api-key",
        CpaSourceSegment::InteractionsApiKey => "interactions-api-key",
        CpaSourceSegment::CodexApiKey => "codex-api-key",
        CpaSourceSegment::ClaudeApiKey => "claude-api-key",
        CpaSourceSegment::OpenaiCompatibility => "openai-compatibility",
        CpaSourceSegment::VertexApiKey => "vertex-api-key",
        CpaSourceSegment::OAuth => "oauth",
    };
    format!("{seg} / {}", protocol_label(protocol))
}

/// 把 prefix/headers 拼进 extra JSON（路由不读，存档）。
fn build_extra(p: &CpaProvider) -> String {
    if p.prefix.is_none() && p.headers.is_empty() {
        return String::new();
    }
    let mut root = serde_json::Map::new();
    if let Some(prefix) = &p.prefix {
        root.insert("cpa_prefix".to_string(), serde_json::Value::String(prefix.clone()));
    }
    if !p.headers.is_empty() {
        root.insert("cpa_headers".to_string(), serde_json::to_value(&p.headers).unwrap());
    }
    serde_json::to_string(&serde_json::Value::Object(root)).unwrap_or_default()
}

// ─── 测试 ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn mk_provider(segment: CpaSourceSegment) -> CpaProvider {
        CpaProvider {
            source_segment: segment,
            name: None,
            base_url: String::new(),
            api_key: "k".to_string(),
            models: vec![],
            prefix: None,
            headers: HashMap::new(),
            disabled: false,
            oauth_type: None,
        }
    }

    #[test]
    fn test_map_api_key_segments() {
        let cases = [
            (CpaSourceSegment::GeminiApiKey, Protocol::Gemini),
            (CpaSourceSegment::InteractionsApiKey, Protocol::Gemini),
            (CpaSourceSegment::CodexApiKey, Protocol::Codex),
            (CpaSourceSegment::ClaudeApiKey, Protocol::Anthropic),
            (CpaSourceSegment::VertexApiKey, Protocol::CpaVertex),
        ];
        for (seg, expected) in cases {
            let mapped = map_provider(mk_provider(seg));
            assert_eq!(mapped.protocol, expected, "segment {:?}", seg);
        }
    }

    #[test]
    fn test_openai_compat_name_routing() {
        let cases = &[
            ("glm-4", Protocol::Glm),
            ("kimi", Protocol::Kimi),
            ("moonshot-v1", Protocol::Kimi),
            ("minimax", Protocol::MiniMax),
            ("minimax_en", Protocol::MiniMaxEn),
            ("deepseek-chat", Protocol::DeepSeek),
            ("qwen-max", Protocol::Bailian),
            ("openrouter", Protocol::OpenRouter),
            ("doubao", Protocol::Doubao),
            ("stepfun", Protocol::StepFun),
            ("siliconflow", Protocol::SiliconFlow),
            ("newapi", Protocol::NewApi),
            ("some-unknown", Protocol::OpenAI),
            ("", Protocol::OpenAI),
        ];
        for (name, expected) in cases {
            let mut p = mk_provider(CpaSourceSegment::OpenaiCompatibility);
            p.name = Some((*name).to_string());
            let mapped = map_provider(p);
            assert_eq!(mapped.protocol, *expected, "name={name:?}");
        }
    }

    #[test]
    fn test_oauth_channel_routing() {
        let cases = &[
            (CpaOAuthType::Xai, Protocol::CpaGrok),
            (CpaOAuthType::Vertex, Protocol::CpaVertex),
            (CpaOAuthType::Aistudio, Protocol::CpaAistudio),
            (CpaOAuthType::Antigravity, Protocol::CpaAntigravity),
            (CpaOAuthType::Claude, Protocol::Anthropic),
            (CpaOAuthType::Codex, Protocol::Codex),
            (CpaOAuthType::Kimi, Protocol::Kimi),
        ];
        for (ot, expected) in cases {
            let mut p = mk_provider(CpaSourceSegment::OAuth);
            p.oauth_type = Some(ot.clone());
            p.name = Some(format!("user@{ot:?}.example"));
            let mapped = map_provider(p);
            assert_eq!(mapped.protocol, *expected, "oauth={ot:?}");
        }
    }

    #[test]
    fn test_name_derivation() {
        // openai-compat 带 name → 直用
        let mut p = mk_provider(CpaSourceSegment::OpenaiCompatibility);
        p.name = Some("glm-prod".to_string());
        assert_eq!(map_provider(p).name, "glm-prod");

        // api-key 段无 name → protocol + host
        let mut p = mk_provider(CpaSourceSegment::GeminiApiKey);
        p.base_url = "https://generativelanguage.googleapis.com/v1beta".to_string();
        let n = map_provider(p).name;
        assert!(n.starts_with("gemini-"), "got {n}");

        // OAuth email
        let mut p = mk_provider(CpaSourceSegment::OAuth);
        p.oauth_type = Some(CpaOAuthType::Claude);
        p.name = Some("user@example.com".to_string());
        assert_eq!(map_provider(p).name, "user@example.com");
    }

    #[test]
    fn test_disabled_and_extra_carry_through() {
        let mut p = mk_provider(CpaSourceSegment::OpenaiCompatibility);
        p.name = Some("x".to_string());
        p.disabled = true;
        p.prefix = Some("pn".to_string());
        let mapped = map_provider(p);
        assert!(mapped.disabled);
        assert!(mapped.extra.contains("cpa_prefix"));
    }

    #[test]
    fn test_host_of() {
        assert_eq!(host_of("https://api.deepseek.com/v1"), "com");
        assert_eq!(host_of("https://api.x.ai/"), "ai");
        assert_eq!(host_of(""), "");
    }

    #[test]
    fn test_batch_mapping_preserves_count() {
        let providers = vec![
            mk_provider(CpaSourceSegment::GeminiApiKey),
            mk_provider(CpaSourceSegment::CodexApiKey),
        ];
        assert_eq!(map_providers(providers).len(), 2);
    }
}
