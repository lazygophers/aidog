//! aidog test util crate —— 测试 harness 共享工具（mock_app_with_db 等，grill G4）。
//!
//! C3+ 各 commands crate dev-deps 引此 crate；root aidog crate 测试也保留 pub(crate)
//! 版本（过渡期 C10 才统一）。本 crate 不依赖任何 commands_* crate（禁循环）。

use aidog_core::gateway::db::Db;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::{App, Manager};
use tauri::test::MockRuntime;

/// 建一个 manage 了内存 Db 的 mock App。State 生命周期绑定返回的 App，调用方需持有它。
///
/// 跨 crate 测试（commands_platform 等）引此 pub fn；root package 内同名 pub(crate)
/// 实现等价（过渡期双存，C10 统一）。
pub async fn mock_app_with_db() -> App<MockRuntime> {
    let db = Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("build mock app");
    app.manage(db);
    app
}

/// 同 [`mock_app_with_db`]，额外 manage 一个 MiddlewareEngine（供 middleware 命令测试）。
pub async fn mock_app_with_db_and_engine() -> App<MockRuntime> {
    use aidog_core::gateway::middleware::MiddlewareEngine;
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
