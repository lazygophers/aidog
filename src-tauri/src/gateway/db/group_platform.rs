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
            // 去 JOIN：① 取该平台所在的 group_id 列表；② 按 group_id 批量取 (id, auto_from_platform)，
            // 内存配对。非热路径、行数极小（单平台所属组数）。
            let mut gp_stmt = conn.prepare(
                "SELECT group_id FROM group_platform \
                 WHERE platform_id = ?1 AND deleted_at = 0",
            )?;
            let group_ids = gp_stmt
                .query_map(params![platform_id as i64], |r| r.get::<_, i64>(0))?
                .collect::<SqlResult<Vec<i64>>>()?;
            if group_ids.is_empty() {
                return Ok(Vec::new());
            }
            // 动态 IN 占位（无子查询）：取未软删组的 auto_from_platform，内存配对回 (gid, auto_from)。
            let placeholders: Vec<String> =
                (0..group_ids.len()).map(|i| format!("?{}", i + 1)).collect();
            let mut g_stmt = conn.prepare(&format!(
                "SELECT id, auto_from_platform FROM \"group\" \
                 WHERE id IN ({}) AND deleted_at = 0",
                placeholders.join(", ")
            ))?;
            let binds: Vec<&dyn rusqlite::types::ToSql> =
                group_ids.iter().map(|g| g as &dyn rusqlite::types::ToSql).collect();
            let rows = g_stmt
                .query_map(binds.as_slice(), |r| {
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

/// group_platform 关联行（仅 gp 列），按 priority 升序。供 get_group_platforms 去 JOIN 后
/// 与批量 platform map 内存重组用。
struct GpRow {
    platform_id: i64,
    priority: i32,
    weight: i32,
    level_priority: i32,
}

/// 单组关联行 + 批量 platform map 内存重组为 `GroupPlatformDetail`（替代旧 gp JOIN platform）。
/// gp_rows 已按 priority 升序，platforms 软删的（不在 map）跳过（等价旧 WHERE p.deleted_at=0）。
/// 字段口径：`GpRow`（priority/weight/level_priority）+ `load_platforms_by_ids` 取出的完整 Platform。
fn recompose_group_details(
    gp_rows: Vec<GpRow>,
    platforms: &std::collections::HashMap<i64, Platform>,
) -> Vec<GroupPlatformDetail> {
    gp_rows
        .into_iter()
        .filter_map(|gp| {
            platforms.get(&gp.platform_id).map(|p| GroupPlatformDetail {
                platform: p.clone(),
                priority: gp.priority,
                weight: gp.weight,
                level_priority: crate::gateway::models::clamp_level_priority(gp.level_priority),
            })
        })
        .collect()
}

#[track_caller]
pub fn get_group_platforms(db: &Db, group_id: u64) -> impl std::future::Future<Output = Result<Vec<GroupPlatformDetail>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            // 去 JOIN：① 取本组 group_platform 行（保 ORDER BY priority）；② 按 platform_id 批量取
            // platform；③ 内存按 priority 重组（软删平台不在 map 自然剔除）。
            let mut gp_stmt = conn.prepare(
                "SELECT platform_id, priority, weight, level_priority FROM group_platform \
                 WHERE group_id = ?1 AND deleted_at = 0 ORDER BY priority",
            )?;
            let gp_rows = gp_stmt
                .query_map(params![group_id as i64], |r| {
                    Ok(GpRow {
                        platform_id: r.get::<_, i64>(0)?,
                        priority: r.get::<_, i32>(1)?,
                        weight: r.get::<_, i32>(2)?,
                        level_priority: r.get::<_, i64>(3)? as i32,
                    })
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            if gp_rows.is_empty() {
                return Ok(Vec::new());
            }
            let ids: Vec<i64> = gp_rows.iter().map(|g| g.platform_id).collect();
            let platforms = load_platforms_by_ids(conn, &ids)?;
            Ok(recompose_group_details(gp_rows, &platforms))
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

/// 去 JOIN 版的「按 group_id 批量取平台关联」：① 单表查给定 group_ids 的 group_platform 行
/// （ORDER BY group_id, priority，与逐组版 ORDER BY priority 同口径）；② 按 platform_id 批量取
/// platform（软删不在 map 自然剔除）；③ 内存按 group_id 分桶重组。无 JOIN/子查询。
/// 供 list_group_details / list_group_details_paged 共用。
#[track_caller]
fn list_group_platforms_for_groups<'a>(
    db: &'a Db,
    group_ids: &'a [u64],
) -> impl std::future::Future<Output = Result<std::collections::HashMap<u64, Vec<GroupPlatformDetail>>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    if group_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let group_ids: Vec<i64> = group_ids.iter().map(|&g| g as i64).collect();
    db
        .call_read_traced(None, __db_caller, move |conn| {
            // ① 给定组的 group_platform 行（保 group_id + priority 排序）。
            let placeholders: Vec<String> =
                (0..group_ids.len()).map(|i| format!("?{}", i + 1)).collect();
            let mut gp_stmt = conn.prepare(&format!(
                "SELECT group_id, platform_id, priority, weight, level_priority \
                 FROM group_platform \
                 WHERE deleted_at = 0 AND group_id IN ({}) \
                 ORDER BY group_id, priority",
                placeholders.join(", ")
            ))?;
            let binds: Vec<&dyn rusqlite::types::ToSql> =
                group_ids.iter().map(|g| g as &dyn rusqlite::types::ToSql).collect();
            let gp_rows = gp_stmt
                .query_map(binds.as_slice(), |r| {
                    Ok((
                        r.get::<_, i64>(0)?, // group_id
                        GpRow {
                            platform_id: r.get::<_, i64>(1)?,
                            priority: r.get::<_, i32>(2)?,
                            weight: r.get::<_, i32>(3)?,
                            level_priority: r.get::<_, i64>(4)? as i32,
                        },
                    ))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            if gp_rows.is_empty() {
                return Ok(std::collections::HashMap::new());
            }
            // ② 按 platform_id 批量取 platform（一次查，软删剔除）。
            let pids: Vec<i64> = gp_rows.iter().map(|(_, gp)| gp.platform_id).collect();
            let platforms = load_platforms_by_ids(conn, &pids)?;
            // ③ 内存按 group_id 分桶重组（gp_rows 已按 group_id, priority 升序，桶内仍按 priority）。
            let mut map: std::collections::HashMap<u64, Vec<GroupPlatformDetail>> =
                std::collections::HashMap::new();
            for (gid, gp) in gp_rows {
                if let Some(p) = platforms.get(&gp.platform_id) {
                    map.entry(gid as u64).or_default().push(GroupPlatformDetail {
                        platform: p.clone(),
                        priority: gp.priority,
                        weight: gp.weight,
                        level_priority: crate::gateway::models::clamp_level_priority(gp.level_priority),
                    });
                }
            }
            Ok(map)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 分页取分组详情（前端触底加载用，反转 H6 单 JOIN 全量批量）。
/// 在 list_groups（已缓存、按 sort_order/created_at 排序）结果上切片 `[offset, offset+limit)`，
/// 仅对本页组取平台关联（无 JOIN，单表 + 内存补平台），不触碰其余组。
/// 返回本页 `GroupDetail`；offset 越界返回空 Vec（前端据此停止触底加载）。
pub async fn list_group_details_paged(
    db: &Db,
    offset: u64,
    limit: u64,
) -> Result<Vec<GroupDetail>, String> {
    let groups = list_groups(db).await?;
    let start = (offset as usize).min(groups.len());
    let end = (start + limit as usize).min(groups.len());
    let page: Vec<Group> = groups[start..end].to_vec();
    if page.is_empty() {
        return Ok(Vec::new());
    }
    let gids: Vec<u64> = page.iter().map(|g| g.id).collect();
    let mut by_group = list_group_platforms_for_groups(db, &gids).await?;
    let mut details = Vec::with_capacity(page.len());
    for g in page {
        let platforms = by_group.remove(&g.id).unwrap_or_default();
        let model_mappings = g.model_mappings.clone();
        details.push(GroupDetail {
            group: g,
            platforms,
            model_mappings,
        });
    }
    Ok(details)
}

pub async fn list_group_details(db: &Db) -> Result<Vec<GroupDetail>, String> {
    // 缓存命中：Groups 页一次拉全量（消除前端逐组 N+1）+ refreshStats 复用，命中即返 clone。
    if let Ok(g) = db.1.group_details.read() {
        if let Some(cached) = g.as_ref() {
            return Ok(cached.clone());
        }
    }
    let groups = list_groups(db).await?;
    // 去 JOIN：单查全部组的 group_platform 行 + 批量补 platform，按 group_id 内存分桶（消除逐组 N+1）。
    let all_gids: Vec<u64> = groups.iter().map(|g| g.id).collect();
    let mut by_group = list_group_platforms_for_groups(db, &all_gids).await?;
    let mut details = Vec::with_capacity(groups.len());
    for g in groups {
        let platforms = by_group.remove(&g.id).unwrap_or_default();
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

