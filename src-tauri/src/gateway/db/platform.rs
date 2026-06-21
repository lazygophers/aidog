use super::*;
use rusqlite::{params, OptionalExtension, Result as SqlResult};

/// SELECT 列序
pub(crate) const PLATFORM_COLUMNS: &str =
    "id, name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at, est_balance_remaining, est_coding_plan, last_real_query_at, estimate_count, show_in_tray, tray_display, sort_order, manual_budgets, status, auto_disabled_until, auto_disable_strikes";

/// 同 PLATFORM_COLUMNS，但每列加 `p.` 限定，用于与其他表 JOIN 时消除同名列歧义（如 created_at/updated_at）
pub(crate) const PLATFORM_COLUMNS_PREFIXED: &str =
    "p.id, p.name, p.platform_type, p.base_url, p.api_key, p.extra, p.models, p.available_models, p.endpoints, p.enabled, p.created_at, p.updated_at, p.est_balance_remaining, p.est_coding_plan, p.last_real_query_at, p.estimate_count, p.show_in_tray, p.tray_display, p.sort_order, p.manual_budgets, p.status, p.auto_disabled_until, p.auto_disable_strikes";

/// 从查询行构造 Platform
pub(crate) fn row_to_platform(row: &rusqlite::Row) -> SqlResult<Platform> {
    let platform_type_str: String = row.get(2)?;
    let models_str: String = row.get(6)?;
    let available_str: String = row.get(7)?;
    let endpoints_str: String = row.get(8)?;
    Ok(Platform {
        id: row.get::<_, i64>(0)? as u64,
        name: row.get(1)?,
        platform_type: serde_json::from_str(&platform_type_str).unwrap(),
        base_url: row.get(3)?,
        api_key: row.get(4)?,
        extra: row.get(5)?,
        models: parse_models(&models_str),
        available_models: parse_available_models(&available_str),
        endpoints: parse_endpoints(&endpoints_str),
        enabled: row.get::<_, i64>(9)? == 1,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        deleted_at: 0,
        est_balance_remaining: row.get(12)?,
        est_coding_plan: row.get(13)?,
        last_real_query_at: row.get(14)?,
        estimate_count: row.get(15)?,
        show_in_tray: row.get::<_, i64>(16)? == 1,
        tray_display: row.get(17)?,
        sort_order: row.get::<_, i64>(18)?,
        manual_budgets: crate::gateway::models::parse_manual_budgets(&row.get::<_, String>(19)?),
        status: crate::gateway::models::PlatformStatus::from_db_str(&row.get::<_, String>(20)?),
        auto_disabled_until: row.get::<_, i64>(21)?,
        auto_disable_strikes: row.get::<_, i64>(22)?,
        balance_level: String::new(),
    })
}

#[track_caller]
pub fn create_platform(db: &Db, mut input: CreatePlatform) -> impl std::future::Future<Output = Result<Platform, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    let platform_type_str = serde_json::to_string(&input.platform_type).unwrap();
    // If name is empty, auto-generate: {platform_type}-{random8}
    if input.name.trim().is_empty() {
        let proto_label = format!("{:?}", input.platform_type).to_lowercase();
        let rand_suffix = &uuid::Uuid::new_v4().simple().to_string()[..8];
        input.name = format!("{}-{}", proto_label, rand_suffix);
    }
    let models = input.models.unwrap_or_default();
    let models_str = serialize_models(&models);
    let available_models = input.available_models.unwrap_or_default();
    let available_str = serialize_available_models(&available_models);
    let endpoints = input.endpoints.unwrap_or_default();
    let endpoints_str = serialize_endpoints(&endpoints);
    let manual_budgets = input.manual_budgets.unwrap_or_default();
    let manual_budgets_str = crate::gateway::models::serialize_manual_budgets(&manual_budgets);

    let id = db
        
        .call_traced(None, __db_caller, {
            let name = input.name.clone();
            let base_url = input.base_url.clone();
            let api_key = input.api_key.clone();
            let extra = input.extra.clone();
            move |conn| {
                conn.execute(
                    "INSERT INTO platform (name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at, manual_budgets) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![name, platform_type_str, base_url, api_key, extra, models_str, available_str, endpoints_str, true as i64, ts, ts, manual_budgets_str],
                )?;
                Ok(conn.last_insert_rowid() as u64)
            }
        })
        .await
        .map_err(|e| format!("create platform: {e}"))?;
    // 新平台暂不属任何组，理论上不影响现有 GroupDetail；失效仅为防御性一致（成本极低）。
    db.invalidate_group_details_cache();

    Ok(Platform {
        id,
        name: input.name,
        platform_type: input.platform_type,
        base_url: input.base_url,
        api_key: input.api_key,
        extra: input.extra,
        models,
        available_models,
        endpoints,
        enabled: true,
        created_at: ts,
        updated_at: ts,
        deleted_at: 0,
        est_balance_remaining: 0.0,
        est_coding_plan: String::new(),
        last_real_query_at: 0,
        estimate_count: 0,
        show_in_tray: false,
        tray_display: "balance".to_string(),
        sort_order: 0,
        manual_budgets,
        status: crate::gateway::models::PlatformStatus::Enabled,
        auto_disabled_until: 0,
        auto_disable_strikes: 0,
        balance_level: String::new(),
    })
    }
}

#[track_caller]
pub fn list_platforms(db: &Db) -> impl std::future::Future<Output = Result<Vec<Platform>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, |conn| {
            let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE deleted_at = 0 ORDER BY sort_order, created_at");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], row_to_platform)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

#[track_caller]
pub fn get_platform(db: &Db, id: u64) -> impl std::future::Future<Output = Result<Option<Platform>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE id = ?1 AND deleted_at = 0");
            let mut stmt = conn.prepare(&sql)?;
            Ok(stmt.query_row(params![id as i64], row_to_platform).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

#[track_caller]
pub fn update_platform(db: &Db, input: UpdatePlatform) -> impl std::future::Future<Output = Result<Platform, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let existing = get_platform(db, input.id).await?.ok_or("platform not found")?;

    // 手动预算：编辑表单只提供配置（kind/unit/amount/window_hours/enabled），
    // consumed/window_start_at 由系统维护——按 id 对齐既有项，保留运行时累计值，
    // 避免每次保存清零已用额度。新增项（id 无匹配）保留传入 consumed（默认 0）。
    let manual_budgets = match input.manual_budgets {
        Some(incoming) => incoming
            .into_iter()
            .map(|mut b| {
                if let Some(prev) = existing.manual_budgets.iter().find(|p| p.id == b.id) {
                    b.consumed = prev.consumed;
                    b.window_start_at = prev.window_start_at;
                }
                b
            })
            .collect(),
        None => existing.manual_budgets.clone(),
    };

    // ── 三态 status 解析（优先级：显式 status > 旧 enabled 兼容入参 > 既有值）──
    // 前端三态切换走 status；旧前端 / 旧调用仍可只传 enabled（true→Enabled, false→Disabled）。
    // 禁止从前端入参置 AutoDisabled（仅系统 401/403 联动 set_platform_auto_disabled 设置）。
    use crate::gateway::models::PlatformStatus;
    let mut new_status = match input.status {
        Some(PlatformStatus::AutoDisabled) => existing.status, // 拒绝外部置自动禁用，保持原状
        Some(s) => s,
        None => match input.enabled {
            Some(true) => PlatformStatus::Enabled,
            Some(false) => PlatformStatus::Disabled,
            None => existing.status,
        },
    };
    let mut auto_disabled_until = existing.auto_disabled_until;
    let mut auto_disable_strikes = existing.auto_disable_strikes;

    // 手动重新启用 auto_disabled / disabled 平台 → 清退避状态
    if new_status == PlatformStatus::Enabled {
        auto_disabled_until = 0;
        auto_disable_strikes = 0;
    }

    // ── 改 api_key 自恢复：当前 auto_disabled 且 api_key 变化 → 立即恢复 enabled 清退避 ──
    let new_api_key = input.api_key.clone().unwrap_or_else(|| existing.api_key.clone());
    if existing.status == PlatformStatus::AutoDisabled
        && new_api_key != existing.api_key
        && new_status == PlatformStatus::AutoDisabled
    {
        new_status = PlatformStatus::Enabled;
        auto_disabled_until = 0;
        auto_disable_strikes = 0;
    }

    let updated = Platform {
        name: input.name.unwrap_or(existing.name),
        platform_type: input.platform_type.unwrap_or(existing.platform_type),
        base_url: input.base_url.unwrap_or(existing.base_url),
        api_key: input.api_key.unwrap_or(existing.api_key),
        extra: input.extra.unwrap_or(existing.extra),
        models: input.models.unwrap_or(existing.models),
        available_models: input.available_models.unwrap_or(existing.available_models),
        endpoints: input.endpoints.unwrap_or(existing.endpoints),
        // enabled 列从 status 同步（向后兼容）：仅 Enabled → true
        enabled: new_status == PlatformStatus::Enabled,
        status: new_status,
        auto_disabled_until,
        auto_disable_strikes,
        manual_budgets,
        updated_at: now(),
        ..existing
    };

    let platform_type_str = serde_json::to_string(&updated.platform_type).unwrap();
    let models_str = serialize_models(&updated.models);
    let available_str = serialize_available_models(&updated.available_models);
    let endpoints_str = serialize_endpoints(&updated.endpoints);
    let manual_budgets_str = crate::gateway::models::serialize_manual_budgets(&updated.manual_budgets);
    db
        .call_traced(None, __db_caller, {
            let name = updated.name.clone();
            let base_url = updated.base_url.clone();
            let api_key = updated.api_key.clone();
            let extra = updated.extra.clone();
            let enabled = updated.enabled as i64;
            let status_str = updated.status.as_db_str().to_string();
            let auto_disabled_until = updated.auto_disabled_until;
            let auto_disable_strikes = updated.auto_disable_strikes;
            let updated_at = updated.updated_at;
            let id = updated.id as i64;
            move |conn| {
                conn.execute(
                    "UPDATE platform SET name=?1, platform_type=?2, base_url=?3, api_key=?4, extra=?5, models=?6, available_models=?7, endpoints=?8, enabled=?9, updated_at=?10, manual_budgets=?11, status=?12, auto_disabled_until=?13, auto_disable_strikes=?14 WHERE id=?15",
                    params![
                        name,
                        platform_type_str,
                        base_url,
                        api_key,
                        extra,
                        models_str,
                        available_str,
                        endpoints_str,
                        enabled,
                        updated_at,
                        manual_budgets_str,
                        status_str,
                        auto_disabled_until,
                        auto_disable_strikes,
                        id,
                    ],
                )?;
                Ok(())
            }
        })
        .await
        .map_err(|e| format!("update platform: {e}"))?;
    // platform 字段内嵌于 GroupDetail.platforms，更新后须失效以免 Groups 页读旧值。
    db.invalidate_group_details_cache();

    Ok(updated)
    }
}

/// 自动禁用退避基础时长（1 小时，毫秒）；第 n 次禁用退避 = BASE * 2^(strikes-1)。
const AUTO_DISABLE_BASE_MS: i64 = 60 * 60 * 1000;
/// 退避指数上限（防溢出 / 过长）：strikes 超过此值后退避封顶。
const AUTO_DISABLE_MAX_STRIKES: i64 = 12; // 2^11 h ≈ 85 天封顶

/// 401/403 触发：将平台标记 auto_disabled，strikes++，按指数退避计算下次试探时间。
/// 仅在当前非用户手动 disabled 时生效（不覆盖用户主动关闭的平台）。
/// 返回更新后的退避截止时间戳（毫秒），供日志记录。
#[track_caller]
pub fn set_platform_auto_disabled(db: &Db, id: u64) -> impl std::future::Future<Output = Result<i64, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    let until = db
        
        .call_traced(None, __db_caller, move |conn| {
            // 读当前状态 + strikes（仅对 enabled / auto_disabled 生效，跳过用户 disabled）
            let row: Option<(String, i64)> = conn
                .query_row(
                    "SELECT status, auto_disable_strikes FROM platform WHERE id = ?1 AND deleted_at = 0",
                    params![id as i64],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
                )
                .optional()?;
            let (status, strikes) = match row {
                Some(v) => v,
                None => return Ok(0i64),
            };
            // 用户手动禁用 → 不动（避免 401/403 把用户禁用平台改成自动禁用语义）
            if status == "disabled" {
                return Ok(0i64);
            }
            let new_strikes = (strikes + 1).min(AUTO_DISABLE_MAX_STRIKES);
            let backoff = AUTO_DISABLE_BASE_MS.saturating_mul(1i64 << (new_strikes - 1).max(0));
            let until = ts + backoff;
            conn.execute(
                "UPDATE platform SET status='auto_disabled', enabled=0, auto_disable_strikes=?1, auto_disabled_until=?2, updated_at=?3 WHERE id=?4",
                params![new_strikes, until, ts, id as i64],
            )?;
            Ok(until)
        })
        .await
        .map_err(|e| format!("set platform auto-disabled: {e}"))?;
    // status/auto_disabled_until 内嵌于 GroupDetail.platforms，失效保 Groups 页一致。
    db.invalidate_group_details_cache();
    Ok(until)
    }
}

/// 连续多少次 404/405（死端点信号）后才临时禁用平台。
/// 语义区别 401/403（鉴权失败，单次即禁）：404/405 = 端点不存在 / 方法不允许，
/// 可能为上游瞬时配置抖动，须连续累计到阈值再禁，避免偶发 405 误伤健康平台。
pub const DEAD_ENDPOINT_STRIKE_THRESHOLD: i64 = 3;

/// 404/405 触发（死端点信号）：累计连续失败次数 auto_disable_strikes++。
/// 仅当累计达到 `threshold` 时才把平台标记 auto_disabled（指数退避），换下个候选；
/// 未达阈值仅递增计数、保持 enabled（继续参与调度），返回 until=0。
/// 语义：404=端点不存在 / 405=方法不允许 → 该上游路径是死端点；连续 N 次确认非瞬时后隔离。
/// 退避复用 401/403 同一指数机制（基于达阈后的额外 strikes 计算），不另起一套退避。
/// 仅在当前非用户手动 disabled 时生效；返回 (新 strikes, 退避截止时间戳 / 未禁则 0)。
#[track_caller]
pub fn record_dead_endpoint_strike(
    db: &Db,
    id: u64,
    threshold: i64,
) -> impl std::future::Future<Output = Result<(i64, i64), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    let res = db
        
        .call_traced(None, __db_caller, move |conn| {
            let row: Option<(String, i64)> = conn
                .query_row(
                    "SELECT status, auto_disable_strikes FROM platform WHERE id = ?1 AND deleted_at = 0",
                    params![id as i64],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
                )
                .optional()?;
            let (status, strikes) = match row {
                Some(v) => v,
                None => return Ok((0i64, 0i64)),
            };
            // 用户手动禁用 → 不动（保持手动语义，不被死端点信号改成自动禁用）
            if status == "disabled" {
                return Ok((0i64, 0i64));
            }
            let new_strikes = (strikes + 1).min(AUTO_DISABLE_MAX_STRIKES);
            // 未达阈值：仅累计计数，保持 enabled，继续参与调度（容忍瞬时 404/405）
            if new_strikes < threshold.max(1) {
                conn.execute(
                    "UPDATE platform SET auto_disable_strikes=?1, updated_at=?2 WHERE id=?3",
                    params![new_strikes, ts, id as i64],
                )?;
                return Ok((new_strikes, 0i64));
            }
            // 达阈值：临时禁用 + 指数退避（退避指数按达阈后的额外 strikes 算，与 401/403 一致量级）
            let over = (new_strikes - threshold.max(1)).max(0); // 0,1,2,... → 1h,2h,4h,...
            let backoff = AUTO_DISABLE_BASE_MS.saturating_mul(1i64 << over.min(AUTO_DISABLE_MAX_STRIKES - 1));
            let until = ts + backoff;
            conn.execute(
                "UPDATE platform SET status='auto_disabled', enabled=0, auto_disable_strikes=?1, auto_disabled_until=?2, updated_at=?3 WHERE id=?4",
                params![new_strikes, until, ts, id as i64],
            )?;
            Ok((new_strikes, until))
        })
        .await
        .map_err(|e| format!("record dead-endpoint strike: {e}"))?;
    // strikes/status 内嵌于 GroupDetail.platforms（list_group_details 非热路径，失效廉价）。
    db.invalidate_group_details_cache();
    Ok(res)
    }
}

/// 2xx 成功且平台仍 enabled 但有累计 strikes（死端点累计未达阈值）：清零计数。
/// 一次成功即证明上游端点并非死端点，连续累计须从头重数（避免跨越长时间的偶发失败误累计）。
/// 仅作用于 enabled 平台；auto_disabled 平台的恢复走 recover_platform_auto_disabled。
#[track_caller]
pub fn reset_dead_endpoint_strikes(db: &Db, id: u64) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE platform SET auto_disable_strikes=0, updated_at=?1 WHERE id=?2 AND status='enabled' AND auto_disable_strikes>0",
                params![ts, id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("reset dead-endpoint strikes: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

/// 2xx 成功：若平台当前为 auto_disabled（试探成功），恢复 enabled 并清退避状态。
/// 用户手动 disabled / 已 enabled 平台不动。
#[track_caller]
pub fn recover_platform_auto_disabled(db: &Db, id: u64) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE platform SET status='enabled', enabled=1, auto_disable_strikes=0, auto_disabled_until=0, updated_at=?1 WHERE id=?2 AND status='auto_disabled'",
                params![ts, id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("recover platform auto-disabled: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

