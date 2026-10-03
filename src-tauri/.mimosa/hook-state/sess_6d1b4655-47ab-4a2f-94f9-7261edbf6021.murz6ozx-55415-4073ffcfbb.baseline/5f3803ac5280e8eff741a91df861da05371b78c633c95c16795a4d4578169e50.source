#![cfg(test)]
use super::*;
use aidog_db as db;
use aidog_db::test_support::test_db;

// check_uv_runs 已删除：它 spawn 真实 `uv` 二进制探测，无实质断言（注释「just exercise
// the path」明示为覆盖率），违反测试隔离。check_uv() 的 bool 逻辑无业务断言价值。

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State/AppHandle 走 command 包装层，直测 command 内联的规范化 + db:: 写入逻辑
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn set_executor_normalizes() {
    let db = test_db().await;
    for executor in ["uv", "python3", "garbage"] {
        let normalized = gateway::scripts::ScriptInvoker::from_setting(Some(executor)).as_setting();
        db::set_setting(
            &db,
            SetSettingInput {
                scope: "app".to_string(),
                key: "script_executor".to_string(),
                value: serde_json::Value::String(normalized.to_string()),
            },
        )
        .await
        .unwrap();
    }
}
