//! 工具日志的管理接口命令（列表 / 详情 / 保留天数），表与读写在 `aidog_db::tool_log`。
use aidog_db::Db;
use aidog_db::models::{ToolLog, ToolLogFilter, ToolLogPage};

/// 设置键：保留天数（u32，0 = 永久保留）。独立于 proxy_log 的保留设置。
const SCOPE: &str = "tool_log";
const KEY: &str = "retention_days";

pub async fn load_retention_days(db: &Db) -> u32 {
    aidog_db::get_setting(db, SCOPE, KEY)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_u64())
        .map_or(aidog_db::TOOL_LOG_DEFAULT_RETENTION_DAYS, |d| d as u32)
}

pub async fn save_retention_days(db: &Db, days: u32) -> Result<(), String> {
    aidog_db::set_setting(
        db,
        crate::gateway::models::SetSettingInput {
            scope: SCOPE.into(),
            key: KEY.into(),
            value: serde_json::json!(days),
        },
    )
    .await
}

crate::tauri_command! {
    /// 工具日志分页列表（倒序，可按工具 / 触发方 / 分组筛选；不带结果正文）。
    pub async fn tool_log_list(
        filter: Option<ToolLogFilter>,
        limit: Option<u32>,
        offset: Option<u32>) -> Result<ToolLogPage, String> {
        aidog_db::list_tool_logs(
            aidog_ctx::db(),
            &filter.unwrap_or_default(),
            limit.unwrap_or(50).clamp(1, 500) as i64,
            offset.unwrap_or(0) as i64,
        )
        .await
    }
}

crate::tauri_command! {
    /// 单条工具日志详情（带结果正文；`log_user_request` 关时正文为空）。
    pub async fn tool_log_get(id: i64) -> Result<Option<ToolLog>, String> {
        aidog_db::get_tool_log(aidog_ctx::db(), id).await
    }
}

crate::tauri_command! {
    /// 工具日志保留天数（0 = 永久）。
    pub async fn tool_log_retention_get() -> Result<u32, String> {
        Ok(load_retention_days(aidog_ctx::db()).await)
    }
}

crate::tauri_command! {
    /// 设置工具日志保留天数，并立即按新值清理一次。
    pub async fn tool_log_retention_set(days: u32) -> Result<(), String> {
        let db = aidog_ctx::db();
        save_retention_days(db, days).await?;
        aidog_db::cleanup_tool_logs(db, days).await
    }
}

#[cfg(test)]
#[path = "test_tool_log.rs"]
mod test_tool_log;
