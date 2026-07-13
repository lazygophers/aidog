//! UI 态持久化命令：前端 _ui_* 键写入 platform/group 的 extra JSON。
use aidog_core::gateway::db::{self, Db};
use tauri::State;

/// 写 UI 态到 extra 单键（读改写）。target="platform"（"group" 待 group 表加 extra 列后开放）。
/// key 推荐 `_ui_` 前缀（`_ui_collapsed` / `_ui_expand_plat` / `_ui_expand_grp`），
/// 与业务键（peak_hours / breaker / disable_during_peak）共存无副作用——serde_json 解析
/// 时忽略未知键。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn set_ui_extra(
    target: String,
    id: u64,
    key: String,
    value: serde_json::Value,
    db: State<'_, Db>,
) -> Result<(), String> {
    tracing::debug!(command = "set_ui_extra", target = %target, id, key = %key, "command invoked");
    db::update_extra_key(&db, &target, id, &key, value)
        .await
        .map_err(|e| {
            tracing::error!(command = "set_ui_extra", target = %target, id, error = %e, "update extra key failed");
            e
        })
}
