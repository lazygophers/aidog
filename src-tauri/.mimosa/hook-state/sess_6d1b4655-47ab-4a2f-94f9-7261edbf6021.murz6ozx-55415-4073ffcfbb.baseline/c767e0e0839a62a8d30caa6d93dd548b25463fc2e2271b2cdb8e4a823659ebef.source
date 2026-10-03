//! codex provider `config.toml` 文本的轻量解析（不引 toml crate）。

use super::CodexConfigParsed;

/// 轻量 TOML 解析（只取顶层 + `[model_providers.<id>]` 的 base_url/name/wire_api）。
/// 避免引入 toml crate 依赖（cc-switch 的 config.toml 结构简单且字段固定）。
pub(super) fn parse_codex_config(settings_config: &serde_json::Value) -> Option<CodexConfigParsed> {
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
pub(super) fn parse_toml_kv(line: &str) -> Option<(String, String)> {
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

#[cfg(test)]
mod test_codex_config;
