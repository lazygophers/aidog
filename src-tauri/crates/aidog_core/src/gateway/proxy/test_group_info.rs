//! group_info 端点覆盖：鉴权 / 未知 group / 多平台 not-applicable / 单平台 applicable。
use super::*;
use crate::gateway::db::test_support::{sample_group, sample_platform, test_db};
use crate::gateway::middleware::MiddlewareEngine;
use crate::gateway::models::GroupPlatformInput;
use axum::http::HeaderMap;
use std::sync::Arc;

async fn make_state(db: crate::gateway::db::Db) -> Arc<ProxyState> {
    Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
        listen_addr: std::sync::OnceLock::new(),
    })
}

fn bearer(gk: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("authorization", format!("Bearer {gk}").parse().unwrap());
    h
}

async fn body_json(resp: Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
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
    crate::gateway::db::create_group(&state.db, sample_group("g0", vec![]))
        .await
        .unwrap();
    let resp = handle_group_info(AxumState(state), bearer("g0")).await;
    let v = body_json(resp).await;
    assert_eq!(v["applicable"], false);
}

#[tokio::test]
async fn single_platform_applicable() {
    let state = make_state(test_db().await).await;
    let p = crate::gateway::db::create_platform(&state.db, sample_platform("p1"))
        .await
        .unwrap();
    let g = crate::gateway::db::create_group(&state.db, sample_group("g1", vec![]))
        .await
        .unwrap();
    crate::gateway::db::set_group_platforms(
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
    let p1 = crate::gateway::db::create_platform(&state.db, sample_platform("a"))
        .await
        .unwrap();
    let p2 = crate::gateway::db::create_platform(&state.db, sample_platform("b"))
        .await
        .unwrap();
    let g = crate::gateway::db::create_group(&state.db, sample_group("g2", vec![]))
        .await
        .unwrap();
    crate::gateway::db::set_group_platforms(
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
