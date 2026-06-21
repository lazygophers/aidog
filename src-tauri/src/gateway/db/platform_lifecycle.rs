use super::*;
use rusqlite::{params, OptionalExtension, Result as SqlResult};

/// 将 quota 查询结果写回 platform 表（余额 + coding plan JSON）。
/// 直写 est_balance/est_coding_plan（不校准、不重置基线）。
/// 已被 estimate::calibrate_from_quota 取代（真查须严格对齐 est=真实 + 重置拟合基线），保留备用。
#[allow(dead_code)]
#[track_caller]
pub fn update_platform_quota<'a>(db: &'a Db, id: u64, balance: f64, coding_plan_json: &'a str) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let coding_plan_json = coding_plan_json.to_string();
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE platform SET est_balance_remaining = ?1, est_coding_plan = ?2 WHERE id = ?3",
                params![balance, coding_plan_json, id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("update platform quota: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

#[track_caller]
pub fn delete_platform(db: &Db, id: u64) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    // ① 软删平台 + 物理清除该平台在所有分组（含手动组与 auto 组）的成员关系。
    //    单事务保证：平台行软删与关联行清理同步，不留指向已删平台的悬空 group_platform。
    db
        .call_traced(None, __db_caller, move |conn| {
            let tx = conn.transaction()?;
            tx.execute("UPDATE platform SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])?;
            tx.execute("DELETE FROM group_platform WHERE platform_id = ?1", params![id as i64])?;
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete platform: {e}"))?;

    // ② 该平台的 auto 分组：移除其唯一源平台后，仅当组内再无存活平台（孤儿 auto 组）才删除。
    //    若用户曾手动把别的平台拖进此 auto 组，则保留该组——不可因删源平台连带销毁其余成员的分组。
    let auto_group_ids: Vec<i64> = db
        
        .call_traced(None, __db_caller, move |conn| {
            Ok(conn.prepare("SELECT id FROM \"group\" WHERE auto_from_platform = ?1 AND deleted_at = 0")?
                .query_map(params![id.to_string()], |row| row.get(0))?
                .collect::<SqlResult<Vec<i64>>>()?)
        })
        .await
        .map_err(|e| e.to_string())?;

    for gid in &auto_group_ids {
        // 步骤①已清掉该平台的关联行，此处直接看组内剩余存活平台数。
        if get_group_platforms(db, *gid as u64).await?.is_empty() {
            force_delete_group(db, *gid as u64).await?;
        }
    }

    // 关联表已变更，刷新分组缓存（force_delete_group 内部也会刷，但无组可删时这里兜底）。
    db.invalidate_groups_cache();
    Ok(())
    }
}

/// 清理失效平台（status='auto_disabled'）的结果。
/// - `deleted_ids`: 被永久删除（软删 platform + 清所有 group_platform + 清孤儿 auto 组）的平台 id。
/// - `unassigned_ids`: 仅从指定分组移除关联（platform 行保留，因仍属其他分组）的平台 id。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeResult {
    pub deleted_ids: Vec<u64>,
    pub unassigned_ids: Vec<u64>,
}

/// 一键清理 auto_disabled 平台。
///
/// - `group_id = None`（全局）：删除全库所有 `status='auto_disabled'` 且未软删的平台，
///   逐个复用 [`delete_platform`]（含清孤儿 auto 组 + 刷新分组缓存）。
/// - `group_id = Some(gid)`（分组级）：仅处理本分组内的 auto_disabled 平台。
///   - 仅属本分组（活跃成员数 == 1）→ 永久删除（复用 `delete_platform`）。
///   - 属多分组（共享，活跃成员数 > 1）→ 仅删本分组的 `group_platform` 关联（platform 行保留）。
///
/// 共享判定的活跃成员数计数必须 `deleted_at=0` 过滤，否则把已软删关联算进来会误判独占。
#[track_caller]
pub fn purge_auto_disabled_platforms(
    db: &Db,
    group_id: Option<u64>,
) -> impl std::future::Future<Output = Result<PurgeResult, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    match group_id {
        // ── 全局：删全库 auto_disabled 平台 ──
        None => {
            let ids: Vec<i64> = db
                
                .call_traced(None, __db_caller, |conn| {
                    let mut stmt = conn.prepare(
                        "SELECT id FROM platform WHERE status = 'auto_disabled' AND deleted_at = 0",
                    )?;
                    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
                    Ok(rows.collect::<SqlResult<Vec<i64>>>()?)
                })
                .await
                .map_err(|e| e.to_string())?;

            let mut deleted_ids = Vec::with_capacity(ids.len());
            for id in ids {
                let pid = id as u64;
                delete_platform(db, pid).await?;
                deleted_ids.push(pid);
            }
            Ok(PurgeResult {
                deleted_ids,
                unassigned_ids: Vec::new(),
            })
        }
        // ── 分组级：本分组内 auto_disabled，独占删 / 共享移关联 ──
        Some(gid) => {
            let gid_i = gid as i64;
            // 本分组内 auto_disabled 平台 id（活跃关联 + 平台未软删）。
            let ids: Vec<i64> = db
                
                .call_traced(None, __db_caller, move |conn| {
                    let mut stmt = conn.prepare(
                        "SELECT p.id FROM platform p \
                         JOIN group_platform gp ON gp.platform_id = p.id \
                         WHERE gp.group_id = ?1 AND gp.deleted_at = 0 \
                         AND p.status = 'auto_disabled' AND p.deleted_at = 0",
                    )?;
                    let rows = stmt.query_map(params![gid_i], |r| r.get::<_, i64>(0))?;
                    Ok(rows.collect::<SqlResult<Vec<i64>>>()?)
                })
                .await
                .map_err(|e| e.to_string())?;

            let mut deleted_ids = Vec::new();
            let mut unassigned_ids = Vec::new();
            for id in ids {
                let pid = id;
                // 该平台跨全库的活跃分组成员数（deleted_at=0 过滤，避免软删残留误判独占）。
                let member_count: i64 = db
                    
                    .call_traced(None, __db_caller, move |conn| {
                        Ok(conn.query_row(
                            "SELECT COUNT(*) FROM group_platform WHERE platform_id = ?1 AND deleted_at = 0",
                            params![pid],
                            |r| r.get::<_, i64>(0),
                        )?)
                    })
                    .await
                    .map_err(|e| e.to_string())?;

                if member_count <= 1 {
                    // 独占本分组 → 永久删除（复用 delete_platform：软删 platform + 清所有关联 + 清孤儿 auto 组）。
                    delete_platform(db, id as u64).await?;
                    deleted_ids.push(id as u64);
                } else {
                    // 共享（属多分组）→ 仅删本分组关联，platform 行保留。
                    // 对齐 move_group_platform 既有模式（db.rs:1622）：物理 DELETE + deleted_at=0 过滤当前活跃行。
                    db
                        .call_traced(None, __db_caller, move |conn| {
                            conn.execute(
                                "DELETE FROM group_platform WHERE group_id = ?1 AND platform_id = ?2 AND deleted_at = 0",
                                params![gid_i, pid],
                            )?;
                            Ok(())
                        })
                        .await
                        .map_err(|e| format!("remove group_platform on purge: {e}"))?;
                    unassigned_ids.push(id as u64);
                }
            }
            // 关联表已变更，刷新分组缓存（delete_platform 内部已刷，纯移关联场景这里兜底）。
            db.invalidate_groups_cache();
            Ok(PurgeResult {
                deleted_ids,
                unassigned_ids,
            })
        }
    }
    }
}

/// 定时清理（内置每日）：永久删除软删超过阈值的平台行。
/// - 条件：`deleted_at > 0 AND deleted_at < now() - older_than_secs`
/// - `delete_platform` 软删时已物理清除所有 `group_platform` 关联（db.rs:1040），此处仅 DELETE platform 行，
///   不留指向已删平台的悬空关联；孤儿 auto 组的清理由 `delete_platform` 当时完成，此处无需重做。
/// - 返回删除行数。仅日志用途，失败仅 warn（定时任务非关键路径）。
#[track_caller]
pub fn purge_old_soft_deleted_platforms(db: &Db, older_than_secs: i64) -> impl std::future::Future<Output = Result<u64, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let cutoff = now() - older_than_secs;
    let n = db
        
        .call_traced(None, __db_caller, move |conn| {
            Ok(conn.execute(
                "DELETE FROM platform WHERE deleted_at > 0 AND deleted_at < ?1",
                params![cutoff],
            )? as u64)
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(n)
    }
}

// ─── Tray 展示（互斥单平台）─────────────────────────────────

/// 互斥设置 tray 展示平台：单事务先清所有 show_in_tray，再置选中平台为 1。
/// `tray_display`: "balance" | "coding"。
#[track_caller]
pub fn set_tray_platform<'a>(db: &'a Db, platform_id: u64, tray_display: &'a str) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let display = if tray_display == "coding" { "coding" } else { "balance" }.to_string();
    let ts = now();
    db
        .call_traced(None, __db_caller, move |conn| {
            let tx = conn.transaction()?;
            tx.execute("UPDATE platform SET show_in_tray = 0, updated_at = ?1 WHERE show_in_tray = 1", params![ts])?;
            tx.execute(
                "UPDATE platform SET show_in_tray = 1, tray_display = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at = 0",
                params![display, ts, platform_id as i64],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| format!("set tray: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

/// 清空所有 tray 展示（关闭）。
#[track_caller]
pub fn clear_tray(db: &Db) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute("UPDATE platform SET show_in_tray = 0, updated_at = ?1 WHERE show_in_tray = 1", params![now()])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("clear tray: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

/// 取当前 tray 展示平台（show_in_tray = 1），无则 None。
#[track_caller]
pub fn get_tray_platform(db: &Db) -> impl std::future::Future<Output = Result<Option<Platform>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, |conn| {
            let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE show_in_tray = 1 AND deleted_at = 0 LIMIT 1");
            let mut stmt = conn.prepare(&sql)?;
            Ok(stmt.query_row([], row_to_platform).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

// ─── Tray Config (settings: scope="tray", key="config") ────

/// 读取 TrayConfig。无配置时（首次/升级）从旧 `show_in_tray=1` 平台迁移生成默认配置并持久化。
/// 返回 None 仅当迁移后仍无任何 enabled 平台（即旧配置也为空）。
pub async fn get_tray_config(db: &Db) -> Result<Option<TrayConfig>, String> {
    if let Some(v) = get_setting(db, "tray", "config").await? {
        if !v.is_null() {
            // 旧全局 layout(single_line/two_line) → 各 item line_mode 迁移：
            // 解析前先抓顶层 layout，若旧配置含该字段则映射到所有 item（two_line→"two" / 其他→"single"）。
            let legacy_line_mode = v
                .get("layout")
                .and_then(|l| l.as_str())
                .map(|l| if l == "two_line" { "two" } else { "single" }.to_string());
            // 容错解析：损坏配置回退默认空（避免整条链 panic）。layout 为未知字段，serde 默认忽略。
            let mut cfg: TrayConfig = serde_json::from_value(v).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "tray config JSON is corrupt, falling back to empty default");
                TrayConfig::default()
            });
            if let Some(lm) = legacy_line_mode {
                for item in &mut cfg.items {
                    item.line_mode = lm.clone();
                }
            }
            return Ok(Some(cfg));
        }
    }
    // 迁移：无 tray config → 从旧 show_in_tray=1 平台生成默认。
    let migrated = migrate_tray_config(db).await?;
    Ok(migrated)
}

/// 从旧 `show_in_tray=1` 平台生成默认 TrayConfig 并存入 settings。
/// 无旧平台 → 存空配置（避免每次启动重复迁移），返回空配置。
async fn migrate_tray_config(db: &Db) -> Result<Option<TrayConfig>, String> {
    let legacy = get_tray_platform(db).await?;
    let mut cfg = TrayConfig::default();
    if let Some(p) = legacy {
        let display = if p.tray_display == "coding" { "coding" } else { "balance" };
        cfg.items.push(TrayItem {
            item_type: "platform".to_string(),
            platform_id: Some(p.id),
            display: display.to_string(),
            metric: None,
            label: None,
decimals: None,
            color: TrayColor::default(),
            font_size: 9.0,
            line_mode: "single".to_string(),
            align: "left".to_string(),
            align_row2: None,
            enabled: true,
            order: 0,
        });
    }
    set_tray_config(db, &cfg).await?;
    Ok(Some(cfg))
}

/// 写入 TrayConfig 到 settings。
pub async fn set_tray_config(db: &Db, cfg: &TrayConfig) -> Result<(), String> {
    let value = serde_json::to_value(cfg).map_err(|e| format!("serialize tray config: {e}"))?;
    set_setting(db, SetSettingInput {
        scope: "tray".to_string(),
        key: "config".to_string(),
        value,
    })
    .await
}

