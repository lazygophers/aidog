//! cc-switch 异源导入子模块。
//!
//! 读取 cc-switch 本地配置（SQLite `cc-switch.db` providers 表 或 旧
//! `config.json` MultiAppConfig），仅筛选 app_type ∈ {claude, codex}，
//! 原样透传成 [`CcProvider`] 中间表示。**不做平台匹配** —— 匹配回退链
//! 在前端纯函数（preset 住前端，记忆 `aidog-add-platform-skill` 反直觉点 1）。
//!
//! 数据结构实证（本地 `~/.cc-switch/cc-switch.db`，2026-06-16）：
//! - claude provider `settings_config` = `{env:{ANTHROPIC_BASE_URL,
//!   ANTHROPIC_AUTH_TOKEN|ANTHROPIC_API_KEY, ANTHROPIC_MODEL,
//!   ANTHROPIC_DEFAULT_*_MODEL, ...}, ...其他 ~/.claude/settings.json 字段}`。
//!   空 provider（如 Claude Official preset 模板）可能为 `{}`。
//! - codex provider `settings_config` = `{auth:{OPENAI_API_KEY},
//!   config:"<config.toml 文本>"}`，config 含 `model_provider` / `model` /
//!   `[model_providers.<id>]` 表的 `base_url` / `wire_api`。
//!
//! 后端只提取 base_url + api_key + (codex 的 config_toml 解析结果)，平台类型
//! 判断全部交给前端 ccswitchMatch.ts。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::gateway::db::Db;

/// 单个 cc-switch provider 的中间表示（原始字段透传 + 提取的便捷字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CcProvider {
    pub id: String,
    /// `claude` | `codex`（SQL 已过滤，前端再校验）。
    pub app_type: String,
    pub name: String,
    /// 原始 settings_config JSON。前端按 app_type 自行解析。
    pub settings_config: serde_json::Value,
    pub website_url: Option<String>,
    /// claude: env.ANTHROPIC_BASE_URL；codex: config.toml base_url。
    pub detected_base_url: Option<String>,
    /// claude: env.ANTHROPIC_AUTH_TOKEN / ANTHROPIC_API_KEY；
    /// codex: auth.OPENAI_API_KEY。
    pub detected_api_key: Option<String>,
    /// codex 专用：解析后的 config.toml 键值（顶层 `model` / `model_provider` /
    /// `wire_api` 等 + `[model_providers.<id>]` 的 `base_url` / `name`）。
    /// claude provider 此字段为 None。后端做轻量 TOML 解析避免前端引依赖。
    pub codex_config_parsed: Option<CodexConfigParsed>,
}

/// codex provider config.toml 解析后的结构化字段（前端用）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexConfigParsed {
    /// 顶层 `model`（主模型 slot）。
    pub model: Option<String>,
    /// 顶层 `model_provider`。
    pub model_provider: Option<String>,
    /// `[model_providers.<id>]` 的 base_url（取 model_provider 对应表）。
    pub base_url: Option<String>,
    /// wire_api：responses / chat。
    pub wire_api: Option<String>,
    /// provider 表里的 name。
    pub provider_name: Option<String>,
}

/// 探测结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CcswitchDetection {
    pub found: bool,
    pub path: Option<String>,
    /// `sqlite` | `json` | `none`。
    pub source_type: String,
    /// 若发现 SQLite，预估的 claude+codex provider 数（-1 = 未统计）。
    pub provider_count: i64,
}

/// 读取结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CcswitchReadResult {
    pub source_type: String,
    pub path: String,
    pub providers: Vec<CcProvider>,
    /// 与现有 aidog 同名 platform 冲突的 name 集合（前端 preview 用）。
    pub existing_platform_names: Vec<String>,
}

// ── 探测 ────────────────────────────────────────────────────

/// 解析 cc-switch 数据目录：默认 `~/.cc-switch/`，读 settings.json 的
/// `configDir` 字段（cc-switch 自定义目录）。
fn resolve_ccswitch_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home dir")?;
    let default_dir = home.join(".cc-switch");
    // settings.json 含 configDir 自定义。
    let settings_path = default_dir.join("settings.json");
    if settings_path.exists() {
        if let Ok(txt) = std::fs::read_to_string(&settings_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                if let Some(custom) = v.get("configDir").and_then(|x| x.as_str()) {
                    if !custom.is_empty() {
                        let p = PathBuf::from(custom);
                        if p.is_absolute() {
                            return Ok(expand_tilde(&p));
                        }
                    }
                }
            }
        }
    }
    Ok(default_dir)
}

/// 展开 `~` 前缀（cc-switch settings 可能写 `~/xxx`）。
fn expand_tilde(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest.trim_start_matches('/'));
        }
    }
    p.to_path_buf()
}

/// 探测：返回数据库路径 / 旧 JSON 路径 / 未检测到。
pub async fn detect(override_path: Option<String>) -> Result<CcswitchDetection, String> {
    let dir = match override_path {
        Some(ref p) if !p.is_empty() => expand_tilde(&PathBuf::from(p)),
        _ => resolve_ccswitch_dir()?,
    };

    let db_path = dir.join("cc-switch.db");
    if db_path.exists() {
        let count = count_providers_sqlite(&db_path).unwrap_or(-1);
        return Ok(CcswitchDetection {
            found: true,
            path: Some(db_path.to_string_lossy().into_owned()),
            source_type: "sqlite".into(),
            provider_count: count,
        });
    }

    let json_path = dir.join("config.json");
    if json_path.exists() {
        let count = count_providers_json(&json_path).unwrap_or(-1);
        return Ok(CcswitchDetection {
            found: true,
            path: Some(json_path.to_string_lossy().into_owned()),
            source_type: "json".into(),
            provider_count: count,
        });
    }

    Ok(CcswitchDetection {
        found: false,
        path: Some(dir.to_string_lossy().into_owned()),
        source_type: "none".into(),
        provider_count: 0,
    })
}

fn count_providers_sqlite(db_path: &Path) -> Result<i64, String> {
    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| format!("open cc-switch db: {e}"))?;
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM providers WHERE app_type IN ('claude','codex')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok(n)
}

fn count_providers_json(json_path: &Path) -> Result<i64, String> {
    let txt = std::fs::read_to_string(json_path).map_err(|e| format!("read json: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&txt).map_err(|e| format!("parse json: {e}"))?;
    // MultiAppConfig: {claudeConfig:{providers:[...]}, codexConfig:{providers:[...]}}
    let mut n = 0i64;
    for key in &["claudeConfig", "codexConfig"] {
        if let Some(arr) = v.get(key).and_then(|x| x.get("providers")).and_then(|x| x.as_array()) {
            n += arr.len() as i64;
        }
    }
    Ok(n)
}

// ── 读取 ────────────────────────────────────────────────────

/// 若 `path` 指向实存的 cc-switch 数据文件 → 直接定 source_type
/// （`config.json`→json，否则 sqlite），返回 `(source_type, 文件路径)`。
/// 缺省 / 指向目录 / 不存在 → `None`，由调用方走 `detect()` 探测。
///
/// 抽成纯函数：`read()` 收到的 path 语义是**文件路径**（前端只传
/// `detect()` 返回的 `.db` / `config.json`），不应再无条件重跑 `detect()`——
/// 后者把文件路径当目录 join 出 `…/cc-switch.db/cc-switch.db`，必然
/// `exists()=false`，误报「配置未检测到」。
fn direct_source_if_file(path: Option<&str>) -> Option<(String, String)> {
    let raw = path?.trim();
    if raw.is_empty() {
        return None;
    }
    let p = expand_tilde(&PathBuf::from(raw));
    if !p.is_file() {
        return None;
    }
    let source_type = match p.file_name().and_then(|n| n.to_str()) {
        // 与 detect() 的目录探测分类保持一致：config.json → json。
        Some("config.json") => "json",
        _ => "sqlite",
    };
    Some((source_type.to_string(), p.to_string_lossy().into_owned()))
}

pub async fn read(
    db: &Db,
    path: Option<String>,
) -> Result<CcswitchReadResult, String> {
    // path = 文件路径 → 直读（不重跑 detect，避开文件被当目录的错配）；
    // 缺省 / 指向目录 / 文件不存在 → 探测后读。
    let (source_type, path_str) = match direct_source_if_file(path.as_deref()) {
        Some(direct) => direct,
        None => {
            let det = detect(path.clone()).await?;
            if !det.found {
                return Err(format!(
                    "cc-switch 配置未检测到（探测路径：{}）",
                    det.path.unwrap_or_default()
                ));
            }
            (det.source_type, det.path.unwrap_or_default())
        }
    };

    let providers = match source_type.as_str() {
        "sqlite" => read_sqlite(&PathBuf::from(&path_str))?,
        "json" => read_json(&PathBuf::from(&path_str))?,
        _ => Vec::new(),
    };

    let existing = crate::gateway::db::list_platforms(db).await?;
    let existing_names: Vec<String> = existing.into_iter().map(|p| p.name).collect();

    Ok(CcswitchReadResult {
        source_type,
        path: path_str,
        providers,
        existing_platform_names: existing_names,
    })
}

fn read_sqlite(db_path: &Path) -> Result<Vec<CcProvider>, String> {
    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| format!("open cc-switch db: {e}"))?;
    // 只取需要的列；gemini/hermes/... 等不导入。
    let mut stmt = conn
        .prepare(
            "SELECT id, app_type, name, settings_config, website_url
             FROM providers WHERE app_type IN ('claude','codex')
             ORDER BY app_type, sort_index, name",
        )
        .map_err(|e| format!("prepare select: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let app_type: String = r.get(1)?;
            let name: String = r.get(2)?;
            let sc_txt: String = r.get(3)?;
            let website_url: Option<String> = r.get(4).ok();
            let sc: serde_json::Value = serde_json::from_str(&sc_txt).unwrap_or(serde_json::Value::Null);
            Ok((id, app_type, name, sc, website_url))
        })
        .map_err(|e| format!("query providers: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        let (id, app_type, name, sc, website_url) = row.map_err(|e| format!("row: {e}"))?;
        out.push(build_provider(id, app_type, name, sc, website_url));
    }
    Ok(out)
}

fn read_json(json_path: &Path) -> Result<Vec<CcProvider>, String> {
    let txt = std::fs::read_to_string(json_path).map_err(|e| format!("read json: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&txt).map_err(|e| format!("parse json: {e}"))?;

    let mut out = Vec::new();
    // claudeConfig / codexConfig 各含 providers 数组。
    for (app_type, key) in [("claude", "claudeConfig"), ("codex", "codexConfig")] {
        let Some(arr) = v.get(key).and_then(|x| x.get("providers")).and_then(|x| x.as_array()) else {
            continue;
        };
        for (i, p) in arr.iter().enumerate() {
            let id = p
                .get("id")
                .and_then(|x| x.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("json-{app_type}-{i}"));
            let name = p
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("unnamed")
                .to_string();
            let website_url = p
                .get("websiteUrl")
                .and_then(|x| x.as_str())
                .map(String::from);
            let settings_config = p
                .get("settingsConfig")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            out.push(build_provider(id, app_type.to_string(), name, settings_config, website_url));
        }
    }
    Ok(out)
}

/// 由原始 settings_config 提取便捷字段（base_url / api_key / codex 解析）。
fn build_provider(
    id: String,
    app_type: String,
    name: String,
    settings_config: serde_json::Value,
    website_url: Option<String>,
) -> CcProvider {
    let (base_url, api_key, codex_parsed) = match app_type.as_str() {
        "claude" => {
            let env = settings_config.get("env");
            let base = env
                .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let key = env
                .and_then(|e| e.get("ANTHROPIC_AUTH_TOKEN"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .or_else(|| {
                    env.and_then(|e| e.get("ANTHROPIC_API_KEY"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                });
            (base, key, None)
        }
        "codex" => {
            let parsed = parse_codex_config(&settings_config);
            let base = parsed.as_ref().and_then(|p| p.base_url.clone());
            let key = settings_config
                .get("auth")
                .and_then(|a| a.get("OPENAI_API_KEY"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            (base, key, parsed)
        }
        _ => (None, None, None),
    };
    CcProvider {
        id,
        app_type,
        name,
        settings_config,
        website_url,
        detected_base_url: base_url,
        detected_api_key: api_key,
        codex_config_parsed: codex_parsed,
    }
}

/// 轻量 TOML 解析（只取顶层 + `[model_providers.<id>]` 的 base_url/name/wire_api）。
/// 避免引入 toml crate 依赖（cc-switch 的 config.toml 结构简单且字段固定）。
fn parse_codex_config(settings_config: &serde_json::Value) -> Option<CodexConfigParsed> {
    let config_txt = settings_config.get("config")?.as_str()?;
    let mut parsed = CodexConfigParsed::default();

    // 先扫顶层键值。
    let mut current_section: Option<String> = None;
    for raw_line in config_txt.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = Some(line[1..line.len() - 1].trim().to_string());
            continue;
        }
        let Some((k, v)) = parse_toml_kv(line) else {
            continue;
        };
        match current_section.as_deref() {
            None => match k.as_str() {
                "model" => parsed.model = Some(v),
                "model_provider" => parsed.model_provider = Some(v),
                _ => {}
            },
            Some(sec) if sec.starts_with("model_providers.") => {
                let sec_id = sec.trim_start_matches("model_providers.").trim();
                // 取 model_provider 对应的 provider 表。
                if parsed
                    .model_provider
                    .as_deref()
                    .map(|mp| mp == sec_id)
                    .unwrap_or(false)
                {
                    match k.as_str() {
                        "base_url" => parsed.base_url = Some(v),
                        "wire_api" => parsed.wire_api = Some(v),
                        "name" => parsed.provider_name = Some(v),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Some(parsed)
}

/// 解析 `key = "value"` / `key = value` / `key = true`。
/// 支持引号值（含 `#` 不被当注释）+ 行尾 inline comment（`# ...`）。
fn parse_toml_kv(line: &str) -> Option<(String, String)> {
    let eq = line.find('=')?;
    let key = line[..eq].trim().to_string();
    let raw = line[eq + 1..].trim();
    if key.is_empty() {
        return None;
    }

    // 引号值：取引号内全文（避免引号内 # 被误当注释）。
    let val = if raw.starts_with('"') || raw.starts_with('\'') {
        let q = &raw[0..1];
        if raw.len() < 2 {
            return None;
        }
        let inner = &raw[1..];
        let end = inner.find(q).unwrap_or(inner.len());
        inner[..end].to_string()
    } else {
        // 裸值：去行尾 inline comment。
        let cut = raw.find(" #").unwrap_or(raw.len());
        raw[..cut].trim().to_string()
    };
    Some((key, val))
}

// ── apply 复用入口 ──────────────────────────────────────────

/// 把前端转换好的 platform payload + 决策应用进 aidog DB。
/// 复用 [`super::apply::apply`]，不另造一套写入路径。
pub async fn import(
    platform_payload: Vec<serde_json::Value>,
    decisions: &[super::ConflictDecision],
    db: &Db,
) -> Result<super::ImportReport, String> {
    let payload = super::Payload {
        manifest: super::Manifest {
            format_version: 1,
            aidog_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            source_machine: "cc-switch-import".to_string(),
            scopes: vec![super::SCOPE_PLATFORM.to_string()],
            checksum: String::new(),
        },
        platform: platform_payload,
        group: Vec::new(),
        group_platform: Vec::new(),
        setting: Vec::new(),
        codex_global: None,
        codex_profiles: Vec::new(),
        claude_code_global: None,
        claude_code_group_settings: Vec::new(),
        skills: Vec::new(),
    };
    super::apply::apply(payload, decisions, db).await
}

// ── 单测 ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn claude_env_extract() {
        let sc = json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.example.com",
                "ANTHROPIC_AUTH_TOKEN": "sk-token-xxx",
                "ANTHROPIC_MODEL": "claude-sonnet-4-6",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5"
            }
        });
        let p = build_provider(
            "id1".into(),
            "claude".into(),
            "Test".into(),
            sc,
            None,
        );
        assert_eq!(p.detected_base_url.as_deref(), Some("https://api.example.com"));
        assert_eq!(p.detected_api_key.as_deref(), Some("sk-token-xxx"));
        assert!(p.codex_config_parsed.is_none());
    }

    #[test]
    fn claude_api_key_fallback_to_anthropic_api_key() {
        let sc = json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.example.com",
                "ANTHROPIC_API_KEY": "sk-ak-xxx"
            }
        });
        let p = build_provider("id".into(), "claude".into(), "N".into(), sc, None);
        assert_eq!(p.detected_api_key.as_deref(), Some("sk-ak-xxx"));
    }

    #[test]
    fn claude_empty_key_is_none() {
        let sc = json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.example.com",
                "ANTHROPIC_AUTH_TOKEN": ""
            }
        });
        let p = build_provider("id".into(), "claude".into(), "N".into(), sc, None);
        assert!(p.detected_api_key.is_none());
    }

    #[test]
    fn claude_empty_settings() {
        // Claude Official preset 模板 = {}。
        let p = build_provider(
            "id".into(),
            "claude".into(),
            "Official".into(),
            json!({}),
            None,
        );
        assert!(p.detected_base_url.is_none());
        assert!(p.detected_api_key.is_none());
    }

    #[test]
    fn codex_settings_config_extract() {
        // 实证样本：本地 cc-switch.db comet codex provider。
        let sc = json!({
            "auth": {"OPENAI_API_KEY": "sk-y21zAr0Mp5UL600I7DyetzQ6kFYITzXDELdoY5vU3tmtZ6o6"},
            "config": "model_provider = \"newapi\"\nmodel = \"gpt-5.4\"\n\n[model_providers]\n[model_providers.newapi]\nname = \"NewAPI\"\nbase_url = \"https://api.cometapi.com/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n"
        });
        let p = build_provider("codex1".into(), "codex".into(), "Comet".into(), sc, None);
        assert_eq!(p.detected_api_key.as_deref(), Some("sk-y21zAr0Mp5UL600I7DyetzQ6kFYITzXDELdoY5vU3tmtZ6o6"));
        let cp = p.codex_config_parsed.expect("codex_config_parsed");
        assert_eq!(cp.model.as_deref(), Some("gpt-5.4"));
        assert_eq!(cp.model_provider.as_deref(), Some("newapi"));
        assert_eq!(cp.base_url.as_deref(), Some("https://api.cometapi.com/v1"));
        assert_eq!(cp.wire_api.as_deref(), Some("responses"));
        assert_eq!(cp.provider_name.as_deref(), Some("NewAPI"));
        assert_eq!(p.detected_base_url.as_deref(), Some("https://api.cometapi.com/v1"));
    }

    #[test]
    fn codex_wire_api_chat() {
        let sc = json!({
            "auth": {"OPENAI_API_KEY": "sk-x"},
            "config": "model = \"gpt-4\"\nmodel_provider = \"p\"\n[model_providers.p]\nbase_url = \"https://x.com/v1\"\nwire_api = \"chat\"\n"
        });
        let p = build_provider("c".into(), "codex".into(), "N".into(), sc, None);
        let cp = p.codex_config_parsed.unwrap();
        assert_eq!(cp.wire_api.as_deref(), Some("chat"));
    }

    #[test]
    fn legacy_json_multi_app_config() {
        // cc-switch 旧 config.json(MultiAppConfig) 形态。
        let v = json!({
            "claudeConfig": {
                "providers": [
                    {"id": "c1", "name": "C1", "settingsConfig": {"env": {"ANTHROPIC_BASE_URL": "https://c1.com", "ANTHROPIC_AUTH_TOKEN": "k1"}}},
                    {"id": "c2", "name": "C2", "settingsConfig": {}}
                ]
            },
            "codexConfig": {
                "providers": [
                    {"id": "x1", "name": "X1", "settingsConfig": {"auth": {"OPENAI_API_KEY": "ok"}, "config": "model = \"g\"\nmodel_provider = \"p\"\n[model_providers.p]\nbase_url = \"https://x.com\"\n"}}
                ]
            },
            "geminiConfig": {
                "providers": [{"id": "g1", "name": "G1", "settingsConfig": {}}]
            }
        });
        let txt = serde_json::to_string(&v).unwrap();
        let tmp = std::env::temp_dir().join("aidog_ccswitch_test.json");
        std::fs::write(&tmp, &txt).unwrap();
        let result = read_json(&tmp).unwrap();
        std::fs::remove_file(&tmp).ok();
        // 仅 claude + codex（gemini 过滤）。
        assert_eq!(result.len(), 3);
        assert_eq!(result.iter().filter(|p| p.app_type == "claude").count(), 2);
        assert_eq!(result.iter().filter(|p| p.app_type == "codex").count(), 1);
        let x1 = result.iter().find(|p| p.id == "x1").unwrap();
        assert_eq!(x1.detected_api_key.as_deref(), Some("ok"));
        assert_eq!(
            x1.codex_config_parsed.as_ref().unwrap().base_url.as_deref(),
            Some("https://x.com")
        );
    }

    #[test]
    fn toml_kv_parser() {
        assert_eq!(parse_toml_kv("model = \"gpt-5\""), Some(("model".into(), "gpt-5".into())));
        assert_eq!(parse_toml_kv("wire_api = 'responses'"), Some(("wire_api".into(), "responses".into())));
        assert_eq!(parse_toml_kv("requires_openai_auth = true"), Some(("requires_openai_auth".into(), "true".into())));
        // inline comment。
        assert_eq!(
            parse_toml_kv("base_url = \"https://x.com\" # primary"),
            Some(("base_url".into(), "https://x.com".into()))
        );
    }

    #[test]
    fn direct_source_file_path_not_treated_as_dir() {
        // 回归：read() 收到的 path 是 detect 返回的 .db 文件路径。旧逻辑把它
        // 当目录 join 出 `…/cc-switch.db/cc-switch.db`，exists()=false 误报
        // 未检测到。直读路径必须把文件路径识别为 sqlite 源。
        let dir = std::env::temp_dir().join(format!("aidog_ccsw_direct_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_file = dir.join("cc-switch.db");
        std::fs::write(&db_file, b"").unwrap();

        let got = direct_source_if_file(Some(&db_file.to_string_lossy()));
        assert_eq!(
            got,
            Some(("sqlite".to_string(), db_file.to_string_lossy().into_owned()))
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn direct_source_classifies_config_json_as_json() {
        let dir = std::env::temp_dir().join(format!("aidog_ccsw_json_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let json_file = dir.join("config.json");
        std::fs::write(&json_file, b"{}").unwrap();

        let got = direct_source_if_file(Some(&json_file.to_string_lossy()));
        assert_eq!(
            got,
            Some(("json".to_string(), json_file.to_string_lossy().into_owned()))
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn direct_source_returns_none_for_dir_or_missing_or_empty() {
        // 目录路径 → None（须走 detect 探测目录内文件）。
        let dir = std::env::temp_dir().join(format!("aidog_ccsw_none_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(direct_source_if_file(Some(&dir.to_string_lossy())), None);

        // 不存在路径 → None。
        let missing = dir.join("nope.db");
        assert_eq!(direct_source_if_file(Some(&missing.to_string_lossy())), None);

        // 缺省 / 空串 → None。
        assert_eq!(direct_source_if_file(None), None);
        assert_eq!(direct_source_if_file(Some("")), None);
        assert_eq!(direct_source_if_file(Some("   ")), None);

        std::fs::remove_dir_all(&dir).ok();
    }
}
