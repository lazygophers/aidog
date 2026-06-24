use super::*;
use rusqlite::{params, Result as SqlResult};

/// 强制删除分组（含自动分组），仅供平台删除时内部调用
#[track_caller]
pub fn force_delete_group(db: &Db, id: u64) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute("UPDATE \"group\" SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete group: {e}"))?;
    db.invalidate_groups_cache();
    Ok(())
    }
}

// ─── GroupPlatform 关联 ────────────────────────────────────

#[track_caller]
pub fn set_group_platforms<'a>(
    db: &'a Db,
    group_id: u64,
    platforms: &'a [GroupPlatformInput],
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    let platforms = platforms.to_vec();
    db
        .call_traced(None, __db_caller, move |conn| {
            // 物理清除旧关联后重建（关联表无需软删保留）
            conn.execute(
                "DELETE FROM group_platform WHERE group_id = ?1",
                params![group_id as i64],
            )?;

            for p in &platforms {
                let lp = crate::gateway::models::clamp_level_priority(
                    p.level_priority.unwrap_or_else(crate::gateway::models::default_level_priority),
                );
                conn.execute(
                    "INSERT INTO group_platform (group_id, platform_id, priority, weight, level_priority, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![group_id as i64, p.platform_id as i64, p.priority.unwrap_or(0), p.weight.unwrap_or(1), lp, ts, ts],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("set group platforms: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

/// 全量同步某平台的「手动」组成员关系（platform_update 用）：
/// 把 platform 加入 `target_group_ids` 内的每个组、移出不在列表内的手动组。
/// **auto 分组（`group.auto_from_platform` 非空）永不动**——仅操作手动组。
/// group_platform 表本身无 auto 标记，靠 join `group.auto_from_platform` 区分。
#[track_caller]
pub fn sync_platform_manual_groups<'a>(
    db: &'a Db,
    platform_id: u64,
    target_group_ids: &'a [u64],
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    // 该平台当前所在的所有 (group_id, auto_from_platform)。
    let current: Vec<(i64, String)> = db
        
        .call_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT g.id, g.auto_from_platform FROM group_platform gp \
                 JOIN \"group\" g ON gp.group_id = g.id \
                 WHERE gp.platform_id = ?1 AND gp.deleted_at = 0 AND g.deleted_at = 0",
            )?;
            let rows = stmt
                .query_map(params![platform_id as i64], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            Ok(rows)
        })
        .await
        .map_err(|e| format!("sync_platform_manual_groups: list current: {e}"))?;

    let target: std::collections::HashSet<i64> =
        target_group_ids.iter().map(|&g| g as i64).collect();

    // 移出：当前在、target 不含、且非 auto 组。
    for (gid, auto_from) in &current {
        if auto_from.is_empty() && !target.contains(gid) {
            let gid = *gid;
            db
                .call_traced(None, __db_caller, move |conn| {
                    conn.execute(
                        "DELETE FROM group_platform WHERE group_id = ?1 AND platform_id = ?2",
                        params![gid, platform_id as i64],
                    )?;
                    Ok(())
                })
                .await
                .map_err(|e| format!("sync_platform_manual_groups: remove from group {gid}: {e}"))?;
            db.invalidate_group_details_cache();
        }
    }

    // 加入：target 含、当前不在的组。复用 set_group_platforms 追加本平台（保留组内其他平台）。
    for &gid in &target {
        let already = current.iter().any(|(g, _)| *g == gid);
        if !already {
            let existing = get_group_platforms(db, gid as u64).await.unwrap_or_default();
            let mut inputs: Vec<GroupPlatformInput> = existing
                .into_iter()
                .map(|d| GroupPlatformInput {
                    platform_id: d.platform.id,
                    priority: Some(d.priority),
                    weight: Some(d.weight),
                    level_priority: Some(d.level_priority),
                })
                .collect();
            if !inputs.iter().any(|i| i.platform_id == platform_id) {
                inputs.push(GroupPlatformInput {
                    platform_id,
                    priority: Some(0),
                    weight: Some(1),
                    level_priority: None,
                });
            }
            set_group_platforms(db, gid as u64, &inputs).await?;
        }
    }

    Ok(())
    }
}

#[track_caller]
pub fn get_group_platforms(db: &Db, group_id: u64) -> impl std::future::Future<Output = Result<Vec<GroupPlatformDetail>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
    let mut stmt = conn
        .prepare(
            &format!(
                "SELECT gp.priority, gp.weight, {PLATFORM_COLUMNS_PREFIXED}, gp.level_priority \
                 FROM group_platform gp JOIN platform p ON gp.platform_id = p.id \
                 WHERE gp.group_id = ?1 AND gp.deleted_at = 0 AND p.deleted_at = 0 ORDER BY gp.priority"
            ),
        )?;

    let rows = stmt
        .query_map(params![group_id as i64], |row| {
            // row layout: priority(0), weight(1), then platform columns starting at 2
            let platform_type_str: String = row.get(4)?;
            let models_str: String = row.get(8)?;
            let available_str: String = row.get(9)?;
            let endpoints_str: String = row.get(10)?;
            Ok(GroupPlatformDetail {
                platform: Platform {
                    id: row.get::<_, i64>(2)? as u64,
                    name: row.get(3)?,
                    platform_type: serde_json::from_str(&platform_type_str).unwrap(),
                    base_url: row.get(5)?,
                    api_key: row.get(6)?,
                    extra: row.get(7)?,
                    models: parse_models(&models_str),
                    available_models: parse_available_models(&available_str),
                    endpoints: parse_endpoints(&endpoints_str),
                    enabled: row.get::<_, i64>(11)? == 1,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                    deleted_at: 0,
                    est_balance_remaining: row.get(14)?,
                    est_coding_plan: row.get(15)?,
                    last_real_query_at: row.get(16)?,
                    estimate_count: row.get(17)?,
                    show_in_tray: row.get::<_, i64>(18)? == 1,
                    tray_display: row.get(19)?,
                    sort_order: row.get::<_, i64>(20)?,
                    manual_budgets: crate::gateway::models::parse_manual_budgets(&row.get::<_, String>(21)?),
                    status: crate::gateway::models::PlatformStatus::from_db_str(&row.get::<_, String>(22)?),
                    auto_disabled_until: row.get::<_, i64>(23)?,
                    auto_disable_strikes: row.get::<_, i64>(24)?,
                    balance_level: String::new(),
                },
                priority: row.get(0)?,
                weight: row.get(1)?,
                level_priority: crate::gateway::models::clamp_level_priority(row.get::<_, i64>(25)? as i32),
            })
        })?;

    Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

// ─── 聚合查询 ──────────────────────────────────────────────

pub async fn get_group_detail(db: &Db, id: u64) -> Result<Option<GroupDetail>, String> {
    let group = match get_group(db, id).await? {
        Some(g) => g,
        None => return Ok(None),
    };
    let platforms = get_group_platforms(db, id).await?;
    // GroupDetail 同时携带 group（含其 model_mappings）与独立的 model_mappings 副本，
    // 二者均被消费方读取（见测试 r4_group_detail_mappings_from_group_field），故须 clone 而非 move。
    let model_mappings = group.model_mappings.clone();

    Ok(Some(GroupDetail {
        group,
        platforms,
        model_mappings,
    }))
}

pub async fn list_group_details(db: &Db) -> Result<Vec<GroupDetail>, String> {
    // 缓存命中：Groups 页一次拉全量（消除前端逐组 N+1）+ refreshStats 复用，命中即返 clone。
    if let Ok(g) = db.1.group_details.read() {
        if let Some(cached) = g.as_ref() {
            return Ok(cached.clone());
        }
    }
    let groups = list_groups(db).await?;
    let mut details = Vec::with_capacity(groups.len());
    for g in groups {
        let platforms = get_group_platforms(db, g.id).await?;
        let model_mappings = g.model_mappings.clone();
        details.push(GroupDetail {
            group: g,
            platforms,
            model_mappings,
        });
    }
    if let Ok(mut g) = db.1.group_details.write() {
        *g = Some(details.clone());
    }
    Ok(details)
}

// ─── Settings CRUD ─────────────────────────────────────────

