//! Protocol logo 同步：按需下载 → 缓存 `~/.aidog/logos/<protocol>.png`，离线可用。
//!
//! 三路 fallback（首成功即止）：
//! 1. simpleicons.org CDN（CC0/GPL）—— 仅当 protocol 配 `logo_url`（=slug，如 "anthropic"）。
//!    URL = `https://cdn.simpleicons.org/<slug>`，默认返 PNG。
//! 2. 厂商 favicon —— 从 `homepage` 提取域名 → `https://<domain>/favicon.ico`。
//! 3. clearbit logo api —— `https://logo.clearbit.com/<domain>`（末路；隐私：clearbit 知用户访问品牌）。
//!
//! 不写缓存场景：三路全失败 / 无 homepage 且 logo_url 空 → 前端 fallback 首字母圆圈。
//! 缓存命中（文件存在 + size>0 + 旁记 `.src` 与当前 `logo_url|homepage` 一致）→ skip；
//! 上游改了 slug 后旁记对不上，同一次同步即重下（换 logo 不必等发版）。
//!
//! 复用 build_http_client（禁 env proxy 防 forward 递归环，见 http_client.rs 注释）。

use aidog_db::Db;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Logo 缓存子目录名（`~/.aidog/logos/`）。统一 `.png` 扩展名：
/// simpleicons/clearbit 返 PNG，favicon 返 ICO——后者强存 `.png` 浏览器仍可渲染。
const LOGOS_SUBDIR: &str = "logos";

/// `~/.aidog/logos/<protocol_id>.png` —— 前端 `convertFileSrc` 用的缓存路径。
pub fn logo_cache_path(app_data_dir: &Path, protocol_id: &str) -> PathBuf {
    app_data_dir.join(LOGOS_SUBDIR).join(format!("{protocol_id}.png"))
}

/// `~/.aidog/logos/<protocol_id>.src` —— 记下该缓存是用哪个来源下的（`<slug>|<homepage>`）。
/// registry 同步换了 `logo_url` 后这行对不上，缓存即判过期。
fn logo_source_marker_path(app_data_dir: &Path, protocol_id: &str) -> PathBuf {
    app_data_dir.join(LOGOS_SUBDIR).join(format!("{protocol_id}.src"))
}

fn logo_source_key(logo_slug: &str, homepage: &str) -> String {
    format!("{logo_slug}|{homepage}")
}

/// 缓存可直接复用 = 文件非空 **且** 旁记的来源与当前 registry 值一致。
/// 旁记缺失（老版本留下的缓存）算过期，重下一次后补上旁记。
fn cache_is_fresh(app_data_dir: &Path, protocol_id: &str, source_key: &str) -> bool {
    let cache = logo_cache_path(app_data_dir, protocol_id);
    let nonempty = std::fs::metadata(&cache).map(|m| m.len() > 0).unwrap_or(false);
    if !nonempty {
        return false;
    }
    std::fs::read_to_string(logo_source_marker_path(app_data_dir, protocol_id))
        .map(|s| s.trim() == source_key)
        .unwrap_or(false)
}

/// 返回 `~/.aidog/logos/`，不存在则建。失败回 None（caller skip 而非崩）。
fn ensure_logos_dir(app_data_dir: &Path) -> Option<PathBuf> {
    let dir = app_data_dir.join(LOGOS_SUBDIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| tracing::warn!(error = %e, dir = %dir.display(), "create logos dir failed"))
        .ok()?;
    Some(dir)
}

/// 遍历 registry 全部 protocols → miss 或来源变更则下载缓存。后台批量同步入口。
/// 不抛错：解析失败 log warn 后 return（不阻塞 app 启动）。
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn sync_all_logos(db: Arc<Db>, app_data_dir: PathBuf) {
    tracing::info!("protocol logos: batch sync started");
    let doc = match aidog_db::presets_doc_value(&db).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "logos sync: read registry presets failed, abort");
            return;
        }
    };
    let entries = match extract_protocols(&doc) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "logos sync: parse protocols failed, abort");
            return;
        }
    };

    let client = crate::gateway::http_client::build_http_client_system(&db, 20, 10).await;

    for (protocol_id, logo_slug, homepage) in entries {
        if cache_is_fresh(&app_data_dir, &protocol_id, &logo_source_key(&logo_slug, &homepage)) {
            continue; // 命中且来源未变
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
    let (logo_slug, homepage) = match read_one_protocol(&db, &protocol_id).await {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(protocol = %protocol_id, error = %e, "sync_one_logo: lookup protocol failed");
            return;
        }
    };
    if cache_is_fresh(&app_data_dir, &protocol_id, &logo_source_key(&logo_slug, &homepage)) {
        return; // 已缓存且来源未变
    }
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
    // 下成功后落旁记，下一轮才知道这张图是用哪个 slug 下的。
    let mark = || {
        let key = logo_source_key(logo_slug, homepage);
        let path = logo_source_marker_path(app_data_dir, protocol_id);
        if let Err(e) = std::fs::write(&path, key) {
            tracing::warn!(path = %path.display(), error = %e, "logos sync: write source marker failed");
        }
    };

    // 路 1 simpleicons：仅当 slug 非空
    if !logo_slug.is_empty() {
        let url = format!("https://cdn.simpleicons.org/{}", logo_slug);
        if let Ok(bytes) = fetch_bytes(client, &url).await
            && write_if_nonzero(&cache, &bytes) {
                mark();
                return Ok(());
            }
    }

    // 路 2 / 3 需 homepage 域名
    let Some(domain) = extract_domain(homepage) else {
        return Err("no homepage domain for favicon/clearbit".into());
    };

    // 路 2 favicon
    let fav_url = format!("https://{domain}/favicon.ico");
    if let Ok(bytes) = fetch_bytes(client, &fav_url).await
        && write_if_nonzero(&cache, &bytes) {
            mark();
            return Ok(());
        }

    // 路 3 clearbit（末路）
    let cb_url = format!("https://logo.clearbit.com/{domain}");
    if let Ok(bytes) = fetch_bytes(client, &cb_url).await
        && write_if_nonzero(&cache, &bytes) {
            mark();
            return Ok(());
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

/// presets → `Vec<(protocol_id, logo_slug, homepage)>`。真值源是 `platform_preset` 表
/// （registry 同步落地）与编译期内置那份的并集，与 `get_defaults_json` 同一条读取链，
/// 所以同步下来的新 `logo_url` 不必等发版就生效。
/// `~/.aidog/platform-presets.json` 覆盖链已移除，禁改回。
fn extract_protocols(root: &serde_json::Value) -> Result<Vec<(String, String, String)>, String> {
    let obj = root.get("protocols").and_then(|v| v.as_object())
        .ok_or_else(|| "missing `protocols` object".to_string())?;
    Ok(obj.iter().map(|(id, v)| {
        let slug = v.get("logo_url").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let hp = v.get("homepage").and_then(|x| x.as_str()).unwrap_or("").to_string();
        (id.clone(), slug, hp)
    }).collect())
}

/// 单 protocol lookup：返 `(logo_slug, homepage)`，未找到返 Err。
/// 走 `preset_entry` 的单行查询——首屏 N 个 logo miss 就是 N 次调用，
/// 不能为查一个 slug 先拼整篇 JSON 再整篇反解析（票 13-H）。
async fn read_one_protocol(db: &Db, protocol_id: &str) -> Result<(String, String), String> {
    let entry = aidog_db::preset_entry(db, protocol_id)
        .await?
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

    /// 同步下来的 `logo_url` 立刻是 logo 的取图依据（DB 有行即以 DB 为准，不再读编译期内置那份）。
    #[tokio::test]
    async fn logo_source_follows_synced_registry_row() {
        let db = aidog_db::test_support::test_db().await;
        // DB 空 → 回落 bundled：真实 registry 里 anthropic 带 slug
        let (bundled_slug, _) = read_one_protocol(&db, "anthropic").await.unwrap();
        assert!(!bundled_slug.is_empty(), "bundled 兜底应给出 anthropic 的 slug");

        // 上游改了 logo：同步一次即生效，无需发版
        aidog_db::upsert_platform_presets(
            &db,
            vec![aidog_db::PlatformPreset {
                code: "anthropic".into(),
                preset_data: r#"{"logo_url":"newslug","homepage":"https://new.example.com"}"#.into(),
                updated_at: 0,
            }],
        )
        .await
        .unwrap();
        let (slug, homepage) = read_one_protocol(&db, "anthropic").await.unwrap();
        assert_eq!(slug, "newslug");
        assert_eq!(homepage, "https://new.example.com");
    }

    /// 同步失败的平台没写库 → 那一行照旧，logo 取图依据保持旧值（best-effort 不留空白）。
    #[tokio::test]
    async fn unsynced_protocol_keeps_previous_logo_source() {
        let db = aidog_db::test_support::test_db().await;
        let rows = vec![
            aidog_db::PlatformPreset {
                code: "alpha".into(),
                preset_data: r#"{"logo_url":"alpha-old","homepage":"https://alpha.example.com"}"#.into(),
                updated_at: 0,
            },
            aidog_db::PlatformPreset {
                code: "beta".into(),
                preset_data: r#"{"logo_url":"beta-old","homepage":"https://beta.example.com"}"#.into(),
                updated_at: 0,
            },
        ];
        aidog_db::upsert_platform_presets(&db, rows).await.unwrap();
        // 第二轮只有 alpha 拉成功（beta 的文件 404 → 压根不 upsert）
        aidog_db::upsert_platform_presets(
            &db,
            vec![aidog_db::PlatformPreset {
                code: "alpha".into(),
                preset_data: r#"{"logo_url":"alpha-new","homepage":"https://alpha.example.com"}"#.into(),
                updated_at: 0,
            }],
        )
        .await
        .unwrap();

        assert_eq!(read_one_protocol(&db, "alpha").await.unwrap().0, "alpha-new");
        assert_eq!(read_one_protocol(&db, "beta").await.unwrap().0, "beta-old", "失败平台的 slug 不该被清空");
    }

    /// 缓存新鲜度按「来源是否变过」判，不只看文件在不在——否则换了 slug 也永远命中旧图。
    #[test]
    fn cache_stale_when_logo_slug_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join(LOGOS_SUBDIR)).unwrap();
        std::fs::write(logo_cache_path(dir, "alpha"), b"\x89PNG\r\n").unwrap();

        // 旁记缺失（老版本遗留缓存）→ 过期，重下一次
        assert!(!cache_is_fresh(dir, "alpha", &logo_source_key("old", "https://a.example.com")));

        std::fs::write(
            logo_source_marker_path(dir, "alpha"),
            logo_source_key("old", "https://a.example.com"),
        )
        .unwrap();
        assert!(cache_is_fresh(dir, "alpha", &logo_source_key("old", "https://a.example.com")));
        assert!(
            !cache_is_fresh(dir, "alpha", &logo_source_key("new", "https://a.example.com")),
            "slug 变了必须重下"
        );
        // 空文件不算命中
        std::fs::write(logo_cache_path(dir, "alpha"), b"").unwrap();
        assert!(!cache_is_fresh(dir, "alpha", &logo_source_key("old", "https://a.example.com")));
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
