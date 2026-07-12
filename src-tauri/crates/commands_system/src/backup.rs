use aidog_core::gateway::{self, db::Db};
use tauri::State;


#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn export_to_file(
    db: State<'_, Db>,
    scopes: Vec<String>,
    path: String,
    // selection: 逐项导出白名单（[scope, key] 对列表）；None = 全量导出（旧客户端兼容）。
    selection: Option<Vec<(String, String)>>,
) -> Result<(), String> {
    tracing::debug!(
        command = "export_to_file",
        scopes = ?scopes,
        path = %path,
        selection = selection.as_ref().map(|s| s.len()).unwrap_or(0),
        "command invoked"
    );
    let mut payload = gateway::import_export::collect::collect(&db, &scopes).await?;
    let sel: Option<gateway::import_export::Selection> =
        selection.map(|v| v.into_iter().collect());
    gateway::import_export::apply::filter_payload(&mut payload, sel.as_ref());
    let bytes = payload.serialize_with_checksum()?;
    let encrypted = gateway::import_export::encrypt(&bytes)?;
    std::fs::write(&path, &encrypted).map_err(|e| format!("write export file: {e}"))?;
    Ok(())
}

/// 导出预览：collect 指定 scope 全量 → 枚举可导出条目（与导入侧 ImportItem 同构），
/// 供前端逐项勾选（默认全选）。无文件 IO、不加密。conflicts 恒空（导出无冲突语义）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn export_preview(
    db: State<'_, Db>,
    scopes: Vec<String>,
) -> Result<gateway::import_export::ImportPreview, String> {
    tracing::debug!(command = "export_preview", scopes = ?scopes, "command invoked");
    let payload = gateway::import_export::collect::collect(&db, &scopes).await?;
    let items = gateway::import_export::apply::export_items(&payload);
    let mut counts = std::collections::BTreeMap::new();
    for item in &items {
        *counts.entry(item.scope.clone()).or_insert(0usize) += 1;
    }
    Ok(gateway::import_export::ImportPreview {
        manifest: payload.manifest.clone(),
        scopes: payload.manifest.scopes.clone(),
        conflicts: Vec::new(),
        counts,
        items,
    })
}

/// 读取定时备份设置 (缺省/解析失败 → 默认)。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn backup_settings_get(db: State<'_, Db>) -> Result<gateway::backup::BackupSettings, String> {
    tracing::debug!(command = "backup_settings_get", "command invoked");
    Ok(gateway::backup::BackupSettings::load(&db).await.sanitized())
}

/// 写入定时备份设置 (前端勾选/改间隔/改保留天数)。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn backup_settings_set(
    db: State<'_, Db>,
    mut settings: gateway::backup::BackupSettings,
) -> Result<gateway::backup::BackupSettings, String> {
    tracing::debug!(command = "backup_settings_set", "command invoked");
    // 走过此命令 (UI 保存入口) = 用户手动确认, 强制标记为当前版本;
    // 前端不传 defaults_version → serde default=0, 这里覆写后即便 enabled=false 也永久尊重。
    settings.defaults_version = gateway::backup::CURRENT_DEFAULTS_VERSION;
    let sanitized = settings.sanitized();
    sanitized.save(&db).await?;
    Ok(sanitized)
}

/// 立即触发一次备份 (忽略 throttle; 失败返回 error, 前端 toast)。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn backup_run_now(db: State<'_, Db>) -> Result<gateway::backup::BackupResult, String> {
    tracing::debug!(command = "backup_run_now", "command invoked");
    let ts = chrono::Utc::now().timestamp_millis();
    match gateway::backup::run_backup(&db).await {
        Ok(path) => Ok(gateway::backup::BackupResult {
            ok: true,
            path: Some(path.to_string_lossy().to_string()),
            error: None,
            timestamp: ts,
        }),
        Err(e) => Ok(gateway::backup::BackupResult {
            ok: false,
            path: None,
            error: Some(e),
            timestamp: ts,
        }),
    }
}

// ─── DB Maintenance (Tier 1: VACUUM reclaim) ──────────────

/// 全量 VACUUM 压缩数据库到最小。设置页「立即压缩数据库」按钮入口。
/// 锁库期间代理写请求排队（busy_timeout 兜底），前端有警示。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn db_compact(db: State<'_, Db>) -> Result<gateway::db::CompactResult, String> {
    tracing::debug!(command = "db_compact", "command invoked");
    gateway::db::compact_database(&db).await
}

/// 导入预览：读文件 → 解密 → 校验 → 扫描冲突，返回前端弹窗所需信息。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn import_read_file(
    db: State<'_, Db>,
    path: String,
) -> Result<gateway::import_export::ImportPreview, String> {
    tracing::debug!(command = "import_read_file", path = %path, "command invoked");
    let bytes = std::fs::read(&path).map_err(|e| format!("read import file: {e}"))?;
    gateway::import_export::apply::preview(&bytes, &db).await
}

/// 导入应用：按用户决策写入 db + 文件 + skills。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn import_apply(
    db: State<'_, Db>,
    path: String,
    decisions: Vec<gateway::import_export::ConflictDecision>,
    // selection: 选中条目白名单（[scope, key] 对列表）；None = 导入全部（旧客户端兼容）。
    selection: Option<Vec<(String, String)>>,
) -> Result<gateway::import_export::ImportReport, String> {
    tracing::debug!(
        command = "import_apply",
        path = %path,
        decisions = decisions.len(),
        selection = selection.as_ref().map(|s| s.len()).unwrap_or(0),
        "command invoked"
    );
    let bytes = std::fs::read(&path).map_err(|e| format!("read import file: {e}"))?;
    let plain = gateway::import_export::decrypt(&bytes)?;
    let payload = gateway::import_export::Payload::from_bytes_verified(&plain)?;
    let sel: Option<gateway::import_export::Selection> =
        selection.map(|v| v.into_iter().collect());
    gateway::import_export::apply::apply(payload, &decisions, sel.as_ref(), &db).await
}

/// cc-switch 导入：探测本地 cc-switch 配置（SQLite / 旧 JSON）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn ccswitch_detect(
    override_path: Option<String>,
) -> Result<gateway::import_export::CcswitchDetection, String> {
    tracing::debug!(command = "ccswitch_detect", "command invoked");
    gateway::import_export::ccswitch::detect(override_path).await
}

/// cc-switch 导入：读取 providers（仅 claude + codex），返回原始 DTO。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn ccswitch_read(
    db: State<'_, Db>,
    path: Option<String>,
) -> Result<gateway::import_export::CcswitchReadResult, String> {
    tracing::debug!(command = "ccswitch_read", "command invoked");
    gateway::import_export::ccswitch::read(&db, path).await
}

/// cc-switch 导入：接收前端转换好的 Platform JSON + 决策，走 apply::apply 写入。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn ccswitch_import(
    db: State<'_, Db>,
    platform_payload: Vec<serde_json::Value>,
    decisions: Vec<gateway::import_export::ConflictDecision>,
    auto_group: bool,
) -> Result<gateway::import_export::ImportReport, String> {
    tracing::debug!(
        command = "ccswitch_import",
        payload_count = platform_payload.len(),
        decisions = decisions.len(),
        auto_group,
        "command invoked"
    );
    gateway::import_export::ccswitch::import(platform_payload, &decisions, auto_group, &db).await
}

/// sub2api 导入：解析用户提供的 sub2api-data JSON 文本，返回账号 DTO 列表（预览用）。
/// 无需 db State（纯文本解析）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn sub2api_parse(
    json_text: String,
) -> Result<gateway::import_export::Sub2ApiReadResult, String> {
    tracing::debug!(command = "sub2api_parse", "command invoked");
    gateway::import_export::sub2api::parse(&json_text)
}

/// sub2api 导入：读取用户选择的 JSON 文件文本（前端 dialog 选路径 → 后端 std::fs 读，
/// 避开前端 fs scope 限制，同 import_read_file 路径语义）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn sub2api_read_file(path: String) -> Result<String, String> {
    tracing::debug!(command = "sub2api_read_file", path = %path, "command invoked");
    std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))
}

/// sub2api 导入：接收前端转换好的 Platform JSON + 决策，走 apply::apply 写入；
/// auto_group=true 时关联 `sub2api` 分组。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn sub2api_import(
    db: State<'_, Db>,
    platform_payload: Vec<serde_json::Value>,
    decisions: Vec<gateway::import_export::ConflictDecision>,
    auto_group: bool,
) -> Result<gateway::import_export::ImportReport, String> {
    tracing::debug!(
        command = "sub2api_import",
        payload_count = platform_payload.len(),
        decisions = decisions.len(),
        auto_group,
        "command invoked"
    );
    gateway::import_export::sub2api::import(platform_payload, &decisions, auto_group, &db).await
}

#[cfg(test)]
#[path = "test_backup.rs"]
mod test_backup;
