#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

// check_uv_runs 已删除：它 spawn 真实 `uv` 二进制探测，无实质断言（注释「just exercise
// the path」明示为覆盖率），违反测试隔离。check_uv() 的 bool 逻辑无业务断言价值。

#[tokio::test]
async fn set_executor_normalizes() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    set_script_executor("uv".into(), db.clone()).await.unwrap();
    set_script_executor("python3".into(), db.clone()).await.unwrap();
    set_script_executor("garbage".into(), db.clone()).await.unwrap();
}
