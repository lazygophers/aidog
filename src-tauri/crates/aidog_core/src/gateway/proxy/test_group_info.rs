//! group_info 端点覆盖：鉴权 / 未知 group / 多平台 not-applicable / 单平台 applicable。
use super::*;
use crate::gateway::models::GroupPlatformInput;
use aidog_db::test_support::{sample_group, sample_platform, test_db};
use aidog_middleware::MiddlewareEngine;
use axum::http::HeaderMap;
use std::sync::Arc;

async fn make_state(db: aidog_db::Db) -> Arc<ProxyState> {
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(1024);
    Arc::new(ProxyState {
        db: Arc::new(db),
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: dashmap::DashMap::new(),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
        listen_addr: std::sync::OnceLock::new(),
        settings_cache: Arc::new(tokio::sync::RwLock::new(Default::default())),
        log_tx,
    })
}

fn bearer(gk: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("authorization", format!("Bearer {gk}").parse().unwrap());
    h
}

async fn body_json(resp: Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn missing_auth_returns_401() {
    let state = make_state(test_db().await).await;
    let resp = handle_group_info(AxumState(state), HeaderMap::new()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unknown_group_returns_not_applicable() {
    let state = make_state(test_db().await).await;
    let resp = handle_group_info(AxumState(state), bearer("nope")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_json(resp).await;
    assert_eq!(v["applicable"], false);
}

#[tokio::test]
async fn group_no_platform_not_applicable() {
    let state = make_state(test_db().await).await;
    aidog_db::create_group(&state.db, sample_group("g0", vec![]))
        .await
        .unwrap();
    let resp = handle_group_info(AxumState(state), bearer("g0")).await;
    let v = body_json(resp).await;
    assert_eq!(v["applicable"], false);
}

#[tokio::test]
async fn single_platform_applicable() {
    let state = make_state(test_db().await).await;
    let p = aidog_db::create_platform(&state.db, sample_platform("p1"))
        .await
        .unwrap();
    let g = aidog_db::create_group(&state.db, sample_group("g1", vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        g.id,
        &[GroupPlatformInput {
            platform_id: p.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();

    let resp = handle_group_info(AxumState(state), bearer("g1")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_json(resp).await;
    assert_eq!(v["applicable"], true);
    assert!(v["balance_level"].is_string());
    assert!(v["coding_plan"].is_array());
}

#[tokio::test]
async fn two_platforms_not_applicable() {
    let state = make_state(test_db().await).await;
    let p1 = aidog_db::create_platform(&state.db, sample_platform("a"))
        .await
        .unwrap();
    let p2 = aidog_db::create_platform(&state.db, sample_platform("b"))
        .await
        .unwrap();
    let g = aidog_db::create_group(&state.db, sample_group("g2", vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        g.id,
        &[
            GroupPlatformInput {
                platform_id: p1.id,
                priority: Some(0),
                weight: Some(1),
                level_priority: Some(0),
            },
            GroupPlatformInput {
                platform_id: p2.id,
                priority: Some(1),
                weight: Some(1),
                level_priority: Some(0),
            },
        ],
    )
    .await
    .unwrap();

    let resp = handle_group_info(AxumState(state), bearer("g2")).await;
    let v = body_json(resp).await;
    assert_eq!(v["applicable"], false);
}

/// 单启用平台分组短路（single-enabled-platform-shortcut）：3 平台仅 1 enabled，其余 disabled → applicable == true。
#[tokio::test]
async fn multi_platform_sole_enabled_applicable() {
    let state = make_state(test_db().await).await;
    let p1 = aidog_db::create_platform(&state.db, sample_platform("only-enabled"))
        .await
        .unwrap();
    let p2 = aidog_db::create_platform(&state.db, sample_platform("disabled-2"))
        .await
        .unwrap();
    let p3 = aidog_db::create_platform(&state.db, sample_platform("disabled-3"))
        .await
        .unwrap();
    for pid in [p2.id, p3.id] {
        aidog_db::update_platform(
            &state.db,
            crate::gateway::models::UpdatePlatform {
                id: pid,
                name: None,
                platform_type: None,
                base_url: None,
                api_key: None,
                extra: None,
                models: None,
                available_models: None,
                endpoints: None,
                enabled: None,
                status: Some(crate::gateway::models::PlatformStatus::Disabled),
                manual_budgets: None,
                join_group_ids: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
    }
    let g = aidog_db::create_group(&state.db, sample_group("g3", vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        g.id,
        &[
            GroupPlatformInput {
                platform_id: p1.id,
                priority: Some(0),
                weight: Some(1),
                level_priority: Some(0),
            },
            GroupPlatformInput {
                platform_id: p2.id,
                priority: Some(1),
                weight: Some(1),
                level_priority: Some(0),
            },
            GroupPlatformInput {
                platform_id: p3.id,
                priority: Some(2),
                weight: Some(1),
                level_priority: Some(0),
            },
        ],
    )
    .await
    .unwrap();

    let resp = handle_group_info(AxumState(state), bearer("g3")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_json(resp).await;
    assert_eq!(v["applicable"], true);
}
