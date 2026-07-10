//! Protocol logo 同步：按需下载 → 缓存 `~/.aidog/logos/<protocol>.png`，离线可用。
//!
//! 三路 fallback（首成功即止）：
//! 1. simpleicons.org CDN（CC0/GPL）—— 仅当 protocol 配 `logo_url`（=slug，如 "anthropic"）。
//!    URL = `https://cdn.simpleicons.org/<slug>`，默认返 PNG。
//! 2. 厂商 favicon —— 从 `homepage` 提取域名 → `https://<domain>/favicon.ico`。
//! 3. clearbit logo api —— `https://logo.clearbit.com/<domain>`（末路；隐私：clearbit 知用户访问品牌）。
//!
//! 不写缓存场景：三路全失败 / 无 homepage 且 logo_url 空 → 前端 fallback 首字母圆圈。
//! 缓存命中（文件存在 + size>0）→ skip。
//!
//! 复用 build_http_client（禁 env proxy 防 forward 递归环，见 http_client.rs 注释）。

use crate::gateway::db::Db;
use crate::shared::aidog_data_dir;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Logo 缓存子目录名（`~/.aidog/logos/`）。统一 `.png` 扩展名：
/// simpleicons/clearbit 返 PNG，favicon 返 ICO——后者强存 `.png` 浏览器仍可渲染。
const LOGOS_SUBDIR: &str = "logos";

/// `~/.aidog/logos/<protocol_id>.png` —— 前端 `convertFileSrc` 用的缓存路径。
pub fn logo_cache_path(app_data_dir: &Path, protocol_id: &str) -> PathBuf {
    app_data_dir.join(LOGOS_SUBDIR).join(format!("{protocol_id}.png"))
}

/// 返回 `~/.aidog/logos/`，不存在则建。失败回 None（caller skip 而非崩）。
fn ensure_logos_dir(app_data_dir: &Path) -> Option<PathBuf> {
    let dir = app_data_dir.join(LOGOS_SUBDIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| tracing::warn!(error = %e, dir = %dir.display(), "create logos dir failed"))
        .ok()?;
    Some(dir)
}

/// 遍历 platform-presets.json 所有 protocols → miss 则下载缓存。后台批量同步入口。
/// 不抛错：解析失败 log warn 后 return（不阻塞 app 启动）。
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn sync_all_logos(db: Arc<Db>, app_data_dir: PathBuf) {
    tracing::info!("protocol logos: batch sync started");
    let presets_json = match read_local_presets_json() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "logos sync: read platform-presets.json failed, abort");
            return;
        }
    };
    let entries = match extract_protocols(&presets_json) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "logos sync: parse protocols failed, abort");
            return;
        }
    };

    let client = crate::gateway::http_client::build_http_client_system(&db, 20, 10).await;

    for (protocol_id, logo_slug, homepage) in entries {
        let cache = logo_cache_path(&app_data_dir, &protocol_id);
        if cache.exists() {
            if let Ok(meta) = std::fs::metadata(&cache) {
                if meta.len() > 0 {
                    continue; // 命中
                }
            }
        }
        if let Err(e) = sync_one_into(&client, &app_data_dir, &protocol_id, &logo_slug, &homepage).await {
            tracing::debug!(protocol = %protocol_id, error = %e, "logos sync: all sources failed, leave uncached");
        }
    }
    tracing::info!("protocol logos: batch sync completed");
}

/// 单 protocol 同步（前端懒加载 miss 时调）。不抛错，三路全失败仅 debug log。
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn sync_one_logo(db: Arc<Db>, app_data_dir: PathBuf, protocol_id: String) {
    let cache = logo_cache_path(&app_data_dir, &protocol_id);
    if cache.exists() {
        if let Ok(meta) = std::fs::metadata(&cache) {
            if meta.len() > 0 {
                return; // 已缓存
            }
        }
    }
    let (logo_slug, homepage) = match read_one_protocol(&protocol_id) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(protocol = %protocol_id, error = %e, "sync_one_logo: lookup protocol failed");
            return;
        }
    };
    let client = crate::gateway::http_client::build_http_client_system(&db, 20, 10).await;
    if let Err(e) = sync_one_into(&client, &app_data_dir, &protocol_id, &logo_slug, &homepage).await {
        tracing::debug!(protocol = %protocol_id, error = %e, "sync_one_logo: all sources failed");
    }
}

async fn sync_one_into(
    client: &reqwest::Client,
    app_data_dir: &Path,
    protocol_id: &str,
    logo_slug: &str,
    homepage: &str,
) -> Result<(), String> {
    let dir = ensure_logos_dir(app_data_dir).ok_or_else(|| "logos dir init failed".to_string())?;
    let cache = dir.join(format!("{protocol_id}.png"));

    // 路 1 simpleicons：仅当 slug 非空
    if !logo_slug.is_empty() {
        let url = format!("https://cdn.simpleicons.org/{}", logo_slug);
        if let Ok(bytes) = fetch_bytes(client, &url).await {
            if write_if_nonzero(&cache, &bytes) {
                return Ok(());
            }
        }
    }

    // 路 2 / 3 需 homepage 域名
    let Some(domain) = extract_domain(homepage) else {
        return Err("no homepage domain for favicon/clearbit".into());
    };

    // 路 2 favicon
    let fav_url = format!("https://{domain}/favicon.ico");
    if let Ok(bytes) = fetch_bytes(client, &fav_url).await {
        if write_if_nonzero(&cache, &bytes) {
            return Ok(());
        }
    }

    // 路 3 clearbit（末路）
    let cb_url = format!("https://logo.clearbit.com/{domain}");
    if let Ok(bytes) = fetch_bytes(client, &cb_url).await {
        if write_if_nonzero(&cache, &bytes) {
            return Ok(());
        }
    }

    Err("all three sources failed".into())
}

async fn fetch_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let resp = client.get(url).send().await.map_err(|e| format!("fetch: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    resp.bytes().await.map(|b| b.to_vec()).map_err(|e| format!("read body: {e}"))
}

/// 仅写非空 bytes（0 字节响应视为失败，三路都返空时不污染缓存）。
fn write_if_nonzero(cache: &Path, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    match std::fs::write(cache, bytes) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(path = %cache.display(), error = %e, "logos sync: write cache failed");
            false
        }
    }
}

/// 从 `homepage` URL 提取 host（含端口如有）。无效返回 None。
fn extract_domain(homepage: &str) -> Option<String> {
    let trimmed = homepage.trim();
    if trimmed.is_empty() {
        return None;
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    url::Url::parse(&with_scheme).ok().and_then(|u| u.host_str().map(|s| s.to_string()))
}

/// 读 `~/.aidog/platform-presets.json`（运行时同步版本）→ 缺失回退 bundled。
/// 同 commands/defaults.rs::get_defaults_json 的优先级，但返回 String 供本模块解析。
fn read_local_presets_json() -> Result<String, String> {
    if let Ok(dir) = aidog_data_dir() {
        let path = dir.join("platform-presets.json");
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if !content.trim().is_empty() && serde_json::from_str::<serde_json::Value>(&content).is_ok() {
                    return Ok(content);
                }
            }
        }
    }
    // 回退 bundled（commands/defaults.rs::BUNDLED 同源；避免循环引用此处独立 include_str!）
    Ok(std::include_str!("../../../../defaults/platform-presets.json").to_string())
}

/// 解析 presets → `Vec<(protocol_id, logo_slug, homepage)>`。
fn extract_protocols(json: &str) -> Result<Vec<(String, String, String)>, String> {
    let root: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("parse: {e}"))?;
    let obj = root.get("protocols").and_then(|v| v.as_object())
        .ok_or_else(|| "missing `protocols` object".to_string())?;
    Ok(obj.iter().map(|(id, v)| {
        let slug = v.get("logo_url").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let hp = v.get("homepage").and_then(|x| x.as_str()).unwrap_or("").to_string();
        (id.clone(), slug, hp)
    }).collect())
}

/// 单 protocol lookup：返 `(logo_slug, homepage)`，未找到返 Err。
fn read_one_protocol(protocol_id: &str) -> Result<(String, String), String> {
    let json = read_local_presets_json()?;
    let root: serde_json::Value = serde_json::from_str(&json).map_err(|e| format!("parse: {e}"))?;
    let entry = root.get("protocols")
        .and_then(|v| v.get(protocol_id))
        .ok_or_else(|| format!("protocol `{protocol_id}` not found"))?;
    let slug = entry.get("logo_url").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let hp = entry.get("homepage").and_then(|x| x.as_str()).unwrap_or("").to_string();
    Ok((slug, hp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_domain_handles_common_cases() {
        assert_eq!(extract_domain("https://www.anthropic.com").as_deref(), Some("www.anthropic.com"));
        assert_eq!(extract_domain("https://openai.com").as_deref(), Some("openai.com"));
        // scheme-less → 补 https
        assert_eq!(extract_domain("deepseek.com").as_deref(), Some("deepseek.com"));
        assert_eq!(extract_domain("").as_deref(), None);
        assert_eq!(extract_domain("   ").as_deref(), None);
        assert_eq!(extract_domain("not a url :// x").as_deref(), None);
    }

    #[test]
    fn logo_cache_path_format() {
        let dir = Path::new("/tmp/.aidog");
        let p = logo_cache_path(dir, "anthropic");
        assert_eq!(p, Path::new("/tmp/.aidog/logos/anthropic.png"));
    }

    #[test]
    fn write_if_nonzero_rejects_empty() {
        let tmp = std::env::temp_dir().join(format!("aidog_logo_test_{}.png", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        assert!(!write_if_nonzero(&tmp, b""));
        assert!(!tmp.exists(), "空 bytes 不应写文件");
        assert!(write_if_nonzero(&tmp, b"\x89PNG\r\n"));
        assert!(tmp.exists());
        let _ = std::fs::remove_file(&tmp);
    }
}
