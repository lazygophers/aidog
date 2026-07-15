//! CLI 代理 provider 批量导入命令（cpa-standalone-module s3）。
//!
//! 迁旧 `cpa_import::parse_cpa_config`（s3 已搬至 `aidog_core::gateway::cli_proxy_parser`），
//! 段 → wire_protocol 直映（去旧 cpa-* 协议中转），产 `CreateCliProxyProvider` 批量入库。
//! 与旧 `commands_platform::cpa_import` 解耦：旧 mapper 输出 MappedPlatform（建 platform 表行），
//! 新 mapper 输出 CreateCliProxyProvider（建 cli_proxy_provider 表行）。

use aidog_core::gateway::{
    cli_proxy_parser::{
        parse_cpa_config, CpaOAuthType, CpaProvider, CpaSourceSegment, SkipReason,
    },
    db::{self, Db},
    models::{CreateCliProxyProvider, Protocol},
};
use serde::{Deserialize, Serialize};
use tauri::State;

/// 批量导入结果（非原子：成功入库，失败收集原因）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliProxyImportResult {
    pub created: Vec<aidog_core::gateway::models::CliProxyProvider>,
    pub failed: Vec<CliProxyImportFailure>,
    pub skipped: Vec<SkipReason>,
    pub source_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliProxyImportFailure {
    pub name: String,
    pub error: String,
}

/// 解析 CPA 配置 → 批量创建 cli_proxy_provider（非原子尽力）。
///
/// - `path`: config.yaml/json/zip/tgz/dir
/// - `auth_dir`: 可选 OAuth 凭据目录
/// - `group_id`: 可选归属分组（写入每个新建 provider.group_id）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_proxy_import(
    path: String,
    auth_dir: Option<String>,
    group_id: Option<i64>,
    db: State<'_, Db>,
) -> Result<CliProxyImportResult, String> {
    tracing::debug!(command = "cli_proxy_import", path = %path, "command invoked");
    let parsed = parse_cpa_config(&path, auth_dir.as_deref())?;
    tracing::info!(
        providers = parsed.providers.len(),
        skipped = parsed.skipped.len(),
        "cli_proxy_import parsed"
    );

    let mut created = Vec::new();
    let mut failed = Vec::new();
    for p in parsed.providers {
        let name = resolve_name(&p);
        let input = map_to_create_input(p, group_id);
        match db::create_cli_proxy_provider(&db, input).await {
            Ok(provider) => {
                tracing::info!(provider_id = provider.id, name = %provider.name, "cli_proxy_import created");
                created.push(provider);
            }
            Err(e) => {
                tracing::warn!(name = %name, error = %e, "cli_proxy_import create failed");
                failed.push(CliProxyImportFailure { name, error: e });
            }
        }
    }

    tracing::info!(
        created = created.len(),
        failed = failed.len(),
        "cli_proxy_import done"
    );
    Ok(CliProxyImportResult {
        created,
        failed,
        skipped: parsed.skipped,
        source_files: parsed.source_files,
    })
}

// ─── 段 → wire_protocol 映射（去 cpa-* 中转）──────────────────────────

/// 段 → wire_protocol 字符串。
/// api-key 段：按段类型固定；openai-compat：name 关键词表；OAuth：按 oauth_type。
/// 返回值 = Protocol serde key（candidates.rs::apply_cli_proxy_override 反序列化此串）。
fn resolve_wire_protocol(p: &CpaProvider) -> String {
    let proto = match p.source_segment {
        CpaSourceSegment::GeminiApiKey | CpaSourceSegment::InteractionsApiKey => Protocol::Gemini,
        CpaSourceSegment::CodexApiKey => Protocol::Codex,
        CpaSourceSegment::ClaudeApiKey => Protocol::Anthropic,
        // Vertex API key 走 Gemini wire（Google AI generateContent 格式）。
        CpaSourceSegment::VertexApiKey => Protocol::Gemini,
        CpaSourceSegment::OpenaiCompatibility => protocol_for_openai_compat_name(
            p.name.as_deref().unwrap_or(""),
        ),
        CpaSourceSegment::OAuth => match p.oauth_type {
            // grok 原生 /responses 端点 → openai_responses wire（与旧 CPA Grok 协议标注一致）。
            Some(CpaOAuthType::Xai) => Protocol::OpenAIResponses,
            // Google 系 OAuth（Vertex/Aistudio/Antigravity）均走 gemini wire。
            Some(CpaOAuthType::Vertex)
            | Some(CpaOAuthType::Aistudio)
            | Some(CpaOAuthType::Antigravity) => Protocol::Gemini,
            Some(CpaOAuthType::Claude) => Protocol::Anthropic,
            Some(CpaOAuthType::Codex) => Protocol::Codex,
            Some(CpaOAuthType::Kimi) => Protocol::Kimi,
            None => Protocol::OpenAI,
        },
    };
    // Protocol → serde key（剥引号）。
    serde_json::to_string(&proto)
        .ok()
        .and_then(|s| s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).map(String::from))
        .unwrap_or_else(|| "openai".to_string())
}

/// openai-compatibility 段 name 关键词 → Protocol。
/// 与旧 cpa_import::mapper::protocol_for_openai_compat_name 同实现（已测）；s4 删旧 mapper 后此为唯一源。
/// ponytail: 关键词表手维护，与 platform-presets.json 协议 key 集对齐；无匹配兜底 `openai`。
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

// ─── CpaProvider → CreateCliProxyProvider 映射 ──────────────────────────

/// 名称解析（对齐旧 mapper::resolve_name 语义）：
/// openai-compat/OAuth 优先用 name/email；api-key 段派生 wire-host。
fn resolve_name(p: &CpaProvider) -> String {
    if let Some(n) = &p.name
        && !n.trim().is_empty()
    {
        return n.clone();
    }
    let wire = resolve_wire_protocol(p);
    let host = host_of(&p.base_url);
    if host.is_empty() {
        wire
    } else {
        format!("{wire}-{host}")
    }
}

/// ponytail: 简单 host 提取（剥 scheme/端口/子域 www.|api.），不引 url crate。
fn host_of(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host_part = after_scheme.split('/').next().unwrap_or("");
    let host = host_part.split(':').next().unwrap_or("");
    host.split('.')
        .next_back()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_default()
}

/// prefix/headers → extra JSON（与旧 mapper::build_extra 对齐）。
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

fn map_to_create_input(p: CpaProvider, group_id: Option<i64>) -> CreateCliProxyProvider {
    let name = resolve_name(&p);
    let wire_protocol = resolve_wire_protocol(&p);
    let extra = build_extra(&p);
    let status = if p.disabled { "disabled" } else { "active" };
    // NewAPI 中转 base_url 不匹配 query_quota 原生 dispatch, 按 wire_protocol 回填 quota.type
    // → test_cmd 分流到 query_quota_newapi (cli-proxy-quota-type)。否则导入的 NewAPI 中继测余额返 Unsupported。
    let quota = if wire_protocol == "newapi" {
        r#"{"type":"newapi"}"#.to_string()
    } else {
        String::new()
    };
    CreateCliProxyProvider {
        name,
        wire_protocol,
        base_url: p.base_url,
        api_key: p.api_key,
        models: p.models,
        extra,
        quota,
        status: status.to_string(),
        group_id,
    }
}

// ─── 测试 ──────────────────────────────────────────────────────────────

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
    fn wire_protocol_api_key_segments() {
        // 段 → wire 直映（去 cpa-* 中转）
        let cases = &[
            (CpaSourceSegment::GeminiApiKey, "gemini"),
            (CpaSourceSegment::InteractionsApiKey, "gemini"),
            (CpaSourceSegment::CodexApiKey, "codex"),
            (CpaSourceSegment::ClaudeApiKey, "anthropic"),
            (CpaSourceSegment::VertexApiKey, "gemini"),
        ];
        for (seg, expected) in cases {
            let p = mk_provider(*seg);
            assert_eq!(resolve_wire_protocol(&p), *expected, "segment {:?}", seg);
        }
    }

    #[test]
    fn wire_protocol_oauth_channels() {
        let cases = &[
            (CpaOAuthType::Xai, "openai_responses"),
            (CpaOAuthType::Vertex, "gemini"),
            (CpaOAuthType::Aistudio, "gemini"),
            (CpaOAuthType::Antigravity, "gemini"),
            (CpaOAuthType::Claude, "anthropic"),
            (CpaOAuthType::Codex, "codex"),
            (CpaOAuthType::Kimi, "kimi"),
        ];
        for (ot, expected) in cases {
            let mut p = mk_provider(CpaSourceSegment::OAuth);
            p.oauth_type = Some(ot.clone());
            assert_eq!(resolve_wire_protocol(&p), *expected, "oauth={:?}", ot);
        }
    }

    #[test]
    fn wire_protocol_openai_compat_keywords() {
        let cases = &[
            ("glm-4", "glm"),
            ("kimi", "kimi"),
            ("moonshot-v1", "kimi"),
            ("deepseek-chat", "deepseek"),
            ("qwen-max", "bailian"),
            ("openrouter", "openrouter"),
            ("unknown-name", "openai"),
            ("", "openai"),
        ];
        for (name, expected) in cases {
            let mut p = mk_provider(CpaSourceSegment::OpenaiCompatibility);
            p.name = Some((*name).to_string());
            assert_eq!(resolve_wire_protocol(&p), *expected, "name={name:?}");
        }
    }

    #[test]
    fn parse_yaml_config_e2e() {
        // 构造一份 CPA config.yaml，验证 parse + map → CreateCliProxyProvider 链路
        let dir = tempfile::tempdir().unwrap();
        let yaml_path = dir.path().join("config.yaml");
        std::fs::write(
            &yaml_path,
            r#"
gemini_api_key:
  - api-key: AIzaSyTEST
    base-url: https://generativelanguage.googleapis.com
    models:
      - name: gemini-1.5-pro
claude_api_key:
  - api-key: sk-ant-test
    base-url: https://api.anthropic.com
openai_compatibility:
  - name: glm-prod
    base-url: https://open.bigmodel.cn/api/paas/v4
    api-key-entries:
      - api-key: glm-key
"#,
        )
        .unwrap();

        let parsed = parse_cpa_config(
            yaml_path.to_str().unwrap(),
            None,
        )
        .expect("parse_cpa_config 应成功");
        assert_eq!(parsed.providers.len(), 3, "应解析 3 个 provider");

        // 验证映射后 wire_protocol 正确（gemini / anthropic / glm）
        let wires: Vec<String> = parsed
            .providers
            .iter()
            .map(resolve_wire_protocol)
            .collect();
        assert!(wires.contains(&"gemini".to_string()), "wires={:?}", wires);
        assert!(
            wires.contains(&"anthropic".to_string()),
            "wires={:?}", wires
        );
        assert!(wires.contains(&"glm".to_string()), "wires={:?}", wires);

        // E2E map_to_create_input 链路
        let inputs: Vec<_> = parsed
            .providers
            .into_iter()
            .map(|p| map_to_create_input(p, None))
            .collect();
        // openai-compat 段保留 name = "glm-prod"
        let glm = inputs.iter().find(|i| i.wire_protocol == "glm").unwrap();
        assert_eq!(glm.name, "glm-prod");
        assert_eq!(glm.base_url, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(glm.api_key, "glm-key");
        assert_eq!(glm.status, "active");

        // api-key 段 name 派生 = wire-host（gemini 域 → host=com → "gemini-com"）
        let gemini = inputs.iter().find(|i| i.wire_protocol == "gemini").unwrap();
        assert!(
            gemini.name.starts_with("gemini"),
            "got {}",
            gemini.name
        );
    }

    #[test]
    fn disabled_provider_maps_to_disabled_status() {
        let mut p = mk_provider(CpaSourceSegment::OpenaiCompatibility);
        p.name = Some("x".to_string());
        p.disabled = true;
        let input = map_to_create_input(p, None);
        assert_eq!(input.status, "disabled");
    }

    #[test]
    fn prefix_headers_carried_into_extra() {
        let mut p = mk_provider(CpaSourceSegment::OpenaiCompatibility);
        p.name = Some("x".to_string());
        p.prefix = Some("pn".to_string());
        let input = map_to_create_input(p, None);
        assert!(input.extra.contains("cpa_prefix"), "extra={}", input.extra);
    }
}
