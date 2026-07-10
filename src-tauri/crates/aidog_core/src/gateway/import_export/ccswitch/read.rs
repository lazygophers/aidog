//! 按源类型（sqlite / json）读取 cc-switch provider，并由原始 settings_config
//! 提取 base_url / api_key / codex 解析等便捷字段。

use std::path::{Path, PathBuf};

use crate::gateway::db::Db;

use super::codex_config::parse_codex_config;
use super::detect::{detect, expand_tilde};
use super::{CcProvider, CcswitchReadResult};

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
    _db: &Db,
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

    Ok(CcswitchReadResult {
        source_type,
        path: path_str,
        providers,
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

#[cfg(test)]
mod test_read;
