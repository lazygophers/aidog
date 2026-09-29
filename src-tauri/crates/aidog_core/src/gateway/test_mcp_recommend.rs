//! 推荐清单拉取缓存的覆盖：TTL 早退 / 版本比对不覆盖 / schema 拒收 / 双源回退 /
//! 读路径兜底顺序（缓存 → 内置）。
use super::*;
use aidog_db::test_support::test_db;
use std::collections::BTreeMap;

/// 只认表内路径的 stub server，返回可直接当 base 用的 URL（idiom 抄 test_price_sync）。
async fn spawn_manifest(files: BTreeMap<String, String>) -> String {
    use axum::extract::Path;
    use axum::routing::get;
    let files = Arc::new(files);
    let app = axum::Router::new().route(
        "/{*path}",
        get(move |Path(path): Path<String>| {
            let files = files.clone();
            async move {
                match files.get(path.as_str()) {
                    Some(body) => (axum::http::StatusCode::OK, body.clone()),
                    None => (axum::http::StatusCode::NOT_FOUND, "not found".to_string()),
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    format!("http://{addr}")
}

fn manifest_with(schema_version: u32, updated_at: i64) -> String {
    // entry key 全 snake_case（同 spec §1.1 示例，aidog_mcp::RecommendedEntry 无 rename）
    format!(
        r#"{{"schema_version": {schema_version}, "updated_at": {updated_at}, "entries": [
  {{"name": "alpha", "transport": "stdio", "command": "npx", "args": ["-y", "alpha"],
    "env": {{"K": ""}}, "url": "", "headers": {{}},
    "display_name": "Alpha", "description": {{"zh-Hans": "甲", "en-US": "A"}},
    "category": "tool", "icon": "alpha", "docs_url": "https://example.com",
    "homepage_url": "https://example.com", "required_env_keys": ["K"]}}
]}}"#
    )
}

fn files(schema_version: u32, updated_at: i64) -> BTreeMap<String, String> {
    BTreeMap::from([(
        "recommended.json".to_string(),
        manifest_with(schema_version, updated_at),
    )])
}

/// 直写一份缓存（绕过拉取），fetched_at 可配 0 模拟「已过期」。
async fn prime_cache(db: &Db, updated_at: i64, fetched_at: i64) {
    let doc: CacheDoc = serde_json::from_str(&format!(
        r#"{{"fetchedAt": {fetched_at}, "manifest": {}}}"#,
        manifest_with(1, updated_at)
    ))
    .unwrap();
    save_cache(db, &doc).await;
}

async fn cached_updated_at(db: &Db) -> Option<i64> {
    read_cache(db).await.map(|c| c.manifest.updated_at)
}

#[tokio::test]
async fn fresh_cache_fetches_and_stores() {
    let db = test_db().await;
    let base = spawn_manifest(files(1, 100)).await;
    // 空缓存 → 首拉入库
    assert!(sync_from(&db, &[&base]).await.unwrap());
    assert_eq!(cached_updated_at(&db).await, Some(100));
}

#[tokio::test]
async fn ttl_fresh_cache_skips_fetch() {
    let db = test_db().await;
    // 缓存 fetched_at = now（TTL 内），上游有更新的 updated_at 也不该被拉
    prime_cache(&db, 100, aidog_db::now()).await;
    let base = spawn_manifest(files(1, 200)).await;
    assert!(!sync_from(&db, &[&base]).await.unwrap());
    assert_eq!(cached_updated_at(&db).await, Some(100));
}

#[tokio::test]
async fn stale_cache_older_remote_not_overwritten() {
    let db = test_db().await;
    // fetched_at = 0（过期）触发拉取，但远程 updated_at 更旧 → 清单保留旧的
    prime_cache(&db, 100, 0).await;
    let base = spawn_manifest(files(1, 50)).await;
    assert!(!sync_from(&db, &[&base]).await.unwrap());
    assert_eq!(cached_updated_at(&db).await, Some(100));
    // fetched_at 被刷新（TTL 重置）
    assert!(read_cache(&db).await.unwrap().fetched_at > 0);
}

#[tokio::test]
async fn unknown_schema_version_rejected_cache_kept() {
    let db = test_db().await;
    prime_cache(&db, 100, 0).await;
    let base = spawn_manifest(files(99, 500)).await;
    assert!(!sync_from(&db, &[&base]).await.unwrap());
    assert_eq!(cached_updated_at(&db).await, Some(100));
}

#[tokio::test]
async fn unknown_schema_version_rejected_empty_cache_stays_empty() {
    let db = test_db().await;
    let base = spawn_manifest(files(99, 500)).await;
    assert!(!sync_from(&db, &[&base]).await.unwrap());
    assert!(read_cache(&db).await.is_none());
}

#[tokio::test]
async fn bad_json_rejected() {
    let db = test_db().await;
    let base = spawn_manifest(BTreeMap::from([(
        "recommended.json".to_string(),
        "not json".to_string(),
    )]))
    .await;
    assert!(!sync_from(&db, &[&base]).await.unwrap());
    assert!(read_cache(&db).await.is_none());
}

#[tokio::test]
async fn all_sources_dead_is_err() {
    let db = test_db().await;
    // 空 stub = 全 404，两源全败 → Err（调用方静默记日志）
    let dead = spawn_manifest(BTreeMap::new()).await;
    assert!(sync_from(&db, &[&dead, &dead]).await.is_err());
    assert!(read_cache(&db).await.is_none());
}

#[tokio::test]
async fn primary_dead_falls_back_to_secondary() {
    let db = test_db().await;
    let dead = spawn_manifest(BTreeMap::new()).await;
    let alive = spawn_manifest(files(1, 100)).await;
    assert!(sync_from(&db, &[&dead, &alive]).await.unwrap());
    assert_eq!(cached_updated_at(&db).await, Some(100));
}

#[tokio::test]
async fn recommended_list_reads_cache_then_bundled() {
    let db = test_db().await;
    // 无缓存 → 内置兜底（票 07 的 bundled 清单，至少含 1 条）
    let bundled = recommended_list(&db).await.unwrap();
    assert!(!bundled.is_empty());

    // 缓存命中 → 缓存优先于内置
    prime_cache(&db, 100, aidog_db::now()).await;
    let cached = recommended_list(&db).await.unwrap();
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].name, "alpha");
}
