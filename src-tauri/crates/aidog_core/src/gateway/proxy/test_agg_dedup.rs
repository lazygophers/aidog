//! agg_mark_first 去重 + FIFO 淘汰自检（AGG_DEDUP_CAP 调值不改此逻辑，用例覆盖正确性回归）。
use super::*;
use aidog_db::test_support;
use crate::gateway::middleware::MiddlewareEngine;

async fn make_state() -> Arc<ProxyState> {
    let db = test_support::test_db().await;
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(16);
    Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
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

/// 同一请求重复终态调用（真实场景：insert + 多次 update + 流式 flush 反复命中终态 gate）
/// 只应聚合一次：首次 true，后续全部 false。
#[tokio::test]
async fn dedup_same_id_only_first_call_true() {
    let state = make_state().await;
    assert!(agg_mark_first(&state, "req-1"));
    assert!(!agg_mark_first(&state, "req-1"));
    assert!(!agg_mark_first(&state, "req-1"));
}

/// FIFO 淘汰：插入超过 AGG_DEDUP_CAP 个不同 id 后，最旧 id 被挤出窗口，
/// 其重复调用会被误判为「首次」（有界缓存的预期行为，非 bug）——验证淘汰确实按 FIFO 生效。
#[tokio::test]
async fn fifo_eviction_oldest_id_falls_out_of_window() {
    let state = make_state().await;
    assert!(agg_mark_first(&state, "oldest"));
    for i in 0..AGG_DEDUP_CAP {
        agg_mark_first(&state, &format!("filler-{i}"));
    }
    // "oldest" 被挤出窗口后重新出现 → 视为首次（true），证明 cap 生效、内存有界。
    assert!(agg_mark_first(&state, "oldest"));
}

/// 未过期窗口内的重复调用仍正确去重（cap 调值不破坏未淘汰 id 的去重语义）。
#[tokio::test]
async fn dedup_still_correct_within_window_after_many_inserts() {
    let state = make_state().await;
    assert!(agg_mark_first(&state, "recent"));
    for i in 0..(AGG_DEDUP_CAP / 2) {
        agg_mark_first(&state, &format!("filler-{i}"));
    }
    // 未超过 cap，"recent" 仍在窗口内 → 重复调用应为 false。
    assert!(!agg_mark_first(&state, "recent"));
}
