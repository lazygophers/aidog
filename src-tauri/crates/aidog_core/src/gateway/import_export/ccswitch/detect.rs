//! cc-switch 数据目录解析与探测（SQLite `cc-switch.db` / 旧 `config.json`）。

use std::path::{Path, PathBuf};

use super::CcswitchDetection;

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
pub(super) fn expand_tilde(p: &Path) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── expand_tilde ──
    #[test]
    fn expand_tilde_with_tilde_prefix() {
        let p = PathBuf::from("~/foo/bar");
        let expanded = expand_tilde(&p);
        let s = expanded.to_string_lossy();
        // Should not start with ~ anymore
        assert!(!s.starts_with('~'), "expanded path should not start with ~: {s}");
        assert!(s.contains("foo/bar"), "should contain the path: {s}");
    }

    #[test]
    fn expand_tilde_without_tilde_is_unchanged() {
        let p = PathBuf::from("/absolute/path/to/thing");
        let expanded = expand_tilde(&p);
        assert_eq!(expanded.to_string_lossy(), "/absolute/path/to/thing");
    }

    #[test]
    fn expand_tilde_relative_path_unchanged() {
        let p = PathBuf::from("relative/path");
        let expanded = expand_tilde(&p);
        assert_eq!(expanded.to_string_lossy(), "relative/path");
    }

    // ── count_providers_json ──
    #[test]
    fn count_providers_json_parses_multi_app_config() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let config = serde_json::json!({
            "claudeConfig": {
                "providers": [
                    {"id": "c1", "name": "C1"},
                    {"id": "c2", "name": "C2"}
                ]
            },
            "codexConfig": {
                "providers": [
                    {"id": "x1", "name": "X1"}
                ]
            }
        });
        std::fs::write(tmp.path(), config.to_string()).unwrap();
        let count = count_providers_json(tmp.path()).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn count_providers_json_empty_config() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "{}").unwrap();
        let count = count_providers_json(tmp.path()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn count_providers_json_only_claude() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let config = serde_json::json!({
            "claudeConfig": {
                "providers": [{"id": "c1"}, {"id": "c2"}, {"id": "c3"}]
            }
        });
        std::fs::write(tmp.path(), config.to_string()).unwrap();
        let count = count_providers_json(tmp.path()).unwrap();
        assert_eq!(count, 3);
    }

    // ── detect: not found case ──
    #[tokio::test]
    async fn detect_with_override_nonexistent_dir() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let nonexistent = tmp_dir.path().join("no-such-dir").to_string_lossy().to_string();
        let result = detect(Some(nonexistent)).await.unwrap();
        // Should report not found since the dir has no cc-switch.db or config.json
        assert!(!result.found);
    }

    #[tokio::test]
    async fn detect_finds_config_json() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config = serde_json::json!({
            "claudeConfig": { "providers": [{"id": "c1"}] }
        });
        let config_path = tmp_dir.path().join("config.json");
        std::fs::write(&config_path, config.to_string()).unwrap();
        let result = detect(Some(tmp_dir.path().to_string_lossy().to_string())).await.unwrap();
        assert!(result.found);
        assert_eq!(result.source_type, "json");
        assert_eq!(result.provider_count, 1);
    }

    #[tokio::test]
    async fn detect_finds_sqlite_db() {
        let tmp_dir = tempfile::tempdir().unwrap();
        // Create a minimal SQLite db
        let db_path = tmp_dir.path().join("cc-switch.db");
        // Create an empty valid SQLite file by opening and creating table
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch("CREATE TABLE providers (id TEXT, app_type TEXT, name TEXT, settings_config TEXT, website_url TEXT)").unwrap();
        drop(conn);
        let result = detect(Some(tmp_dir.path().to_string_lossy().to_string())).await.unwrap();
        assert!(result.found);
        assert_eq!(result.source_type, "sqlite");
    }
}
