#![cfg(test)]
//! commands/ 层共享测试基建：构造 MockRuntime App + manage(Db) → app.state::<Db>()。
//!
//! 用途：`#[tauri::command]` 薄壳多数仅 `db: State<'_, Db>` 转发到已测 gateway 逻辑。
//! 通过 mock app 取 `State<Db>` 直接调用 command async fn，覆盖转发路径 + 错误映射分支。
//! 需要 AppHandle 的 command（tray 刷新等）走 MockRuntime AppHandle，能调用则覆盖，
//! 否则在各自测试里降级测内部 helper。

use crate::gateway::db::Db;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::{App, Manager};
use tauri::test::MockRuntime;

/// 建一个 manage 了内存 Db 的 mock App。State 生命周期绑定返回的 App，调用方需持有它。
pub(crate) async fn mock_app_with_db() -> App<MockRuntime> {
    let db = Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("build mock app");
    app.manage(db);
    app
}

/// 同 [`mock_app_with_db`]，额外 manage 一个 MiddlewareEngine（供 middleware 命令测试）。
pub(crate) async fn mock_app_with_db_and_engine() -> App<MockRuntime> {
    use crate::gateway::middleware::MiddlewareEngine;
    use std::sync::Arc;
    let db = Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("build mock app");
    app.manage(db);
    app.manage(Arc::new(MiddlewareEngine::new()));
    app
}
