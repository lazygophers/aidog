use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde::{Deserialize, Serialize};
use tokio_rusqlite::Connection as AsyncConnection;

use super::models::*;

/// 异步 SQLite 连接封装。
///
/// tokio-rusqlite 内部以单后台线程顺序执行所有 `call` 闭包，天然串行化，
/// 故无需 `Mutex`。`AsyncConnection` 自身 `Clone + Send + Sync`（内部仅一个 channel sender），
/// 可直接 `app.manage(Db)` / `State<Db>`，克隆廉价（共享同一后台线程连接）。
#[derive(Clone)]
pub struct Db(pub AsyncConnection);

/// 从 JSON 字符串反序列化 models
fn parse_models(json: &str) -> PlatformModels {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse platform models failed, using default (stored JSON corrupt?)");
        PlatformModels::default()
    })
}

/// 将 models 序列化为 JSON 字符串
fn serialize_models(models: &PlatformModels) -> String {
    serde_json::to_string(models).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize platform models failed, persisting empty object");
        "{}".to_string()
    })
}

/// 从 JSON 字符串反序列化可用模型列表
fn parse_available_models(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse available_models failed, using empty list (stored JSON corrupt?)");
        Vec::new()
    })
}

/// 将可用模型列表序列化为 JSON 字符串
fn serialize_available_models(models: &[String]) -> String {
    serde_json::to_string(models).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize available_models failed, persisting empty array");
        "[]".to_string()
    })
}

/// 从 JSON 字符串反序列化协议端点列表
fn parse_endpoints(json: &str) -> Vec<PlatformEndpoint> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse platform endpoints failed, using empty list (stored JSON corrupt?)");
        Vec::new()
    })
}

/// 将协议端点列表序列化为 JSON 字符串
fn serialize_endpoints(endpoints: &[PlatformEndpoint]) -> String {
    serde_json::to_string(endpoints).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize platform endpoints failed, persisting empty array");
        "[]".to_string()
    })
}

impl Db {
    pub async fn new(path: &str) -> Result<Self, String> {
        let conn = AsyncConnection::open(path).await.map_err(|e| e.to_string())?;
        // pragma 是 connection 级状态，绑定后台线程那条物理连接，设一次永久生效。
        // WAL 下 synchronous=NORMAL 安全；单连接模型下 busy_timeout 实际罕触发，设置无害。
        conn.call(|c| {
            c.execute_batch(
                "PRAGMA journal_mode=WAL; \
                 PRAGMA foreign_keys=ON; \
                 PRAGMA busy_timeout=5000; \
                 PRAGMA synchronous=NORMAL;",
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(Self(conn))
    }

    pub async fn init_tables(&self) -> Result<(), String> {
        self.0
            .call(|conn| {
                conn.execute_batch(include_str!("../../migrations/001_init.sql"))?;
                conn.execute_batch(include_str!("../../migrations/002_log_filter_indexes.sql"))?;
                conn.execute_batch(include_str!("../../migrations/003_model_price.sql"))?;
                // Migration 004: 旧库补预估列（ALTER 无 IF NOT EXISTS → 忽略 duplicate column）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN est_balance_remaining REAL NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN est_coding_plan TEXT NOT NULL DEFAULT ''", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN last_real_query_at INTEGER NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN estimate_count INTEGER NOT NULL DEFAULT 0", []);
                // Migration 005: tray 展示列（互斥单平台 show_in_tray + balance/coding 二选一 tray_display）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN show_in_tray INTEGER NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN tray_display TEXT NOT NULL DEFAULT 'balance'", []);
                // Migration 006: group 排序权重
                let _ = conn.execute("ALTER TABLE \"group\" ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0", []);
                // Migration 007: platform 排序权重
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0", []);
                // Migration 008: proxy_log 预估花费列
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN est_cost REAL NOT NULL DEFAULT 0", []);
                // Migration 009: platform 手动预算列（无上游 quota 平台手动限额 + 耗尽阻断）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN manual_budgets TEXT NOT NULL DEFAULT '[]'", []);
                // Migration 010: proxy_log 流式标记列（流式 SSE 请求显式标记，替代 response_body=="[stream]" 哨兵）
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN is_stream INTEGER NOT NULL DEFAULT 0", []);
                // Migration 011: 多平台重试 + 401/403 自动禁用 + 尝试记录（见 migrations/007_retry_failover.sql）
                // platform 三态 status + 退避字段；enabled 列保留向后兼容（写入端从 status 同步）
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN status TEXT NOT NULL DEFAULT 'enabled'", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN auto_disabled_until INTEGER NOT NULL DEFAULT 0", []);
                let _ = conn.execute("ALTER TABLE platform ADD COLUMN auto_disable_strikes INTEGER NOT NULL DEFAULT 0", []);
                // 数据迁移：旧 enabled=0 → status='disabled'（幂等：仅作用于仍为默认 'enabled' 的行，
                // 绝不覆盖 auto_disabled，避免重启误判用户禁用 vs 自动禁用）
                let _ = conn.execute("UPDATE platform SET status = 'disabled' WHERE enabled = 0 AND status = 'enabled'", []);
                // group 分组级最大重试次数
                let _ = conn.execute("ALTER TABLE \"group\" ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 2", []);
                // proxy_log 每次尝试快照（JSON 数组）+ 重试次数
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN attempts TEXT NOT NULL DEFAULT '[]'", []);
                let _ = conn.execute("ALTER TABLE proxy_log ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0", []);
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())
    }
}

/// 当前毫秒级 Unix 时间戳
pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// 计算保留期截止时间戳（毫秒）。`days == 0` 表示跳过清理，返回 None。
fn retention_cutoff(days: u32) -> Option<i64> {
    if days == 0 {
        return None;
    }
    Some((chrono::Utc::now() - chrono::Duration::days(days as i64)).timestamp_millis())
}

// ─── Platform CRUD ─────────────────────────────────────────

/// SELECT 列序
const PLATFORM_COLUMNS: &str =
    "id, name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at, est_balance_remaining, est_coding_plan, last_real_query_at, estimate_count, show_in_tray, tray_display, sort_order, manual_budgets, status, auto_disabled_until, auto_disable_strikes";

/// 同 PLATFORM_COLUMNS，但每列加 `p.` 限定，用于与其他表 JOIN 时消除同名列歧义（如 created_at/updated_at）
const PLATFORM_COLUMNS_PREFIXED: &str =
    "p.id, p.name, p.platform_type, p.base_url, p.api_key, p.extra, p.models, p.available_models, p.endpoints, p.enabled, p.created_at, p.updated_at, p.est_balance_remaining, p.est_coding_plan, p.last_real_query_at, p.estimate_count, p.show_in_tray, p.tray_display, p.sort_order, p.manual_budgets, p.status, p.auto_disabled_until, p.auto_disable_strikes";

/// 从查询行构造 Platform
fn row_to_platform(row: &rusqlite::Row) -> SqlResult<Platform> {
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
        manual_budgets: super::models::parse_manual_budgets(&row.get::<_, String>(19)?),
        status: super::models::PlatformStatus::from_db_str(&row.get::<_, String>(20)?),
        auto_disabled_until: row.get::<_, i64>(21)?,
        auto_disable_strikes: row.get::<_, i64>(22)?,
        balance_level: String::new(),
    })
}

pub async fn create_platform(db: &Db, mut input: CreatePlatform) -> Result<Platform, String> {
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
    let manual_budgets_str = super::models::serialize_manual_budgets(&manual_budgets);

    let id = db
        .0
        .call({
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
        status: super::models::PlatformStatus::Enabled,
        auto_disabled_until: 0,
        auto_disable_strikes: 0,
        balance_level: String::new(),
    })
}

pub async fn list_platforms(db: &Db) -> Result<Vec<Platform>, String> {
    db.0
        .call(|conn| {
            let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE deleted_at = 0 ORDER BY sort_order, created_at");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], row_to_platform)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_platform(db: &Db, id: u64) -> Result<Option<Platform>, String> {
    db.0
        .call(move |conn| {
            let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE id = ?1 AND deleted_at = 0");
            let mut stmt = conn.prepare(&sql)?;
            Ok(stmt.query_row(params![id as i64], row_to_platform).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn update_platform(db: &Db, input: UpdatePlatform) -> Result<Platform, String> {
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
    use super::models::PlatformStatus;
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
    let manual_budgets_str = super::models::serialize_manual_budgets(&updated.manual_budgets);
    db.0
        .call({
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

    Ok(updated)
}

/// 自动禁用退避基础时长（1 小时，毫秒）；第 n 次禁用退避 = BASE * 2^(strikes-1)。
const AUTO_DISABLE_BASE_MS: i64 = 60 * 60 * 1000;
/// 退避指数上限（防溢出 / 过长）：strikes 超过此值后退避封顶。
const AUTO_DISABLE_MAX_STRIKES: i64 = 12; // 2^11 h ≈ 85 天封顶

/// 401/403 触发：将平台标记 auto_disabled，strikes++，按指数退避计算下次试探时间。
/// 仅在当前非用户手动 disabled 时生效（不覆盖用户主动关闭的平台）。
/// 返回更新后的退避截止时间戳（毫秒），供日志记录。
pub async fn set_platform_auto_disabled(db: &Db, id: u64) -> Result<i64, String> {
    let ts = now();
    db.0
        .call(move |conn| {
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
        .map_err(|e| format!("set platform auto-disabled: {e}"))
}

/// 2xx 成功：若平台当前为 auto_disabled（试探成功），恢复 enabled 并清退避状态。
/// 用户手动 disabled / 已 enabled 平台不动。
pub async fn recover_platform_auto_disabled(db: &Db, id: u64) -> Result<(), String> {
    let ts = now();
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET status='enabled', enabled=1, auto_disable_strikes=0, auto_disabled_until=0, updated_at=?1 WHERE id=?2 AND status='auto_disabled'",
                params![ts, id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("recover platform auto-disabled: {e}"))
}

/// 将 quota 查询结果写回 platform 表（余额 + coding plan JSON）。
/// 直写 est_balance/est_coding_plan（不校准、不重置基线）。
/// 已被 estimate::calibrate_from_quota 取代（真查须严格对齐 est=真实 + 重置拟合基线），保留备用。
#[allow(dead_code)]
pub async fn update_platform_quota(db: &Db, id: u64, balance: f64, coding_plan_json: &str) -> Result<(), String> {
    let coding_plan_json = coding_plan_json.to_string();
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET est_balance_remaining = ?1, est_coding_plan = ?2 WHERE id = ?3",
                params![balance, coding_plan_json, id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("update platform quota: {e}"))
}

pub async fn delete_platform(db: &Db, id: u64) -> Result<(), String> {
    // 删除关联的自动分组
    let auto_group_ids: Vec<i64> = db
        .0
        .call(move |conn| {
            Ok(conn.prepare("SELECT id FROM \"group\" WHERE auto_from_platform = ?1 AND deleted_at = 0")?
                .query_map(params![id.to_string()], |row| row.get(0))?
                .collect::<SqlResult<Vec<i64>>>()?)
        })
        .await
        .map_err(|e| e.to_string())?;

    for gid in &auto_group_ids {
        force_delete_group(db, *gid as u64).await?;
    }

    db.0
        .call(move |conn| {
            conn.execute("UPDATE platform SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete platform: {e}"))
}

// ─── Tray 展示（互斥单平台）─────────────────────────────────

/// 互斥设置 tray 展示平台：单事务先清所有 show_in_tray，再置选中平台为 1。
/// `tray_display`: "balance" | "coding"。
pub async fn set_tray_platform(db: &Db, platform_id: u64, tray_display: &str) -> Result<(), String> {
    let display = if tray_display == "coding" { "coding" } else { "balance" }.to_string();
    let ts = now();
    db.0
        .call(move |conn| {
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
        .map_err(|e| format!("set tray: {e}"))
}

/// 清空所有 tray 展示（关闭）。
pub async fn clear_tray(db: &Db) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute("UPDATE platform SET show_in_tray = 0, updated_at = ?1 WHERE show_in_tray = 1", params![now()])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("clear tray: {e}"))
}

/// 取当前 tray 展示平台（show_in_tray = 1），无则 None。
pub async fn get_tray_platform(db: &Db) -> Result<Option<Platform>, String> {
    db.0
        .call(|conn| {
            let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE show_in_tray = 1 AND deleted_at = 0 LIMIT 1");
            let mut stmt = conn.prepare(&sql)?;
            Ok(stmt.query_row([], row_to_platform).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
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

/// 今日（本地时区 00:00 起）累计 token 总量（input + output），未删除日志。
#[cfg(test)]
pub async fn today_token_total(db: &Db) -> Result<i64, String> {
    use chrono::{Local, TimeZone};
    let today = Local::now().date_naive();
    let start_dt = today.and_hms_opt(0, 0, 0).ok_or("invalid local midnight")?;
    let start_local = Local
        .from_local_datetime(&start_dt)
        .single()
        .ok_or("ambiguous local midnight")?;
    let start_ms = start_local.timestamp_millis();

    db.0
        .call(move |conn| {
            Ok(conn.query_row(
                "SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM proxy_log WHERE created_at >= ?1 AND deleted_at = 0",
                params![start_ms],
                |row| row.get(0),
            )?)
        })
        .await
        .map_err(|e| format!("today token total: {e}"))
}

/// 今日统计摘要（供托盘预览使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodayStats {
    /// 今日总 token 数（input + output）
    pub tokens: i64,
    /// 今日 cache 命中率（cache_tokens / input_tokens * 100）
    pub cache_rate: f64,
    /// 今日预估花费（$），基于 model_price 定价
    pub cost: f64,
    /// 今日总请求数
    pub total_requests: i64,
}

/// 获取今日统计（本地时区 00:00 起，未删除日志）
pub async fn today_stats(db: &Db) -> Result<TodayStats, String> {
    use chrono::{Local, TimeZone};
    let today = Local::now().date_naive();
    let start_dt = today.and_hms_opt(0, 0, 0).ok_or("invalid local midnight")?;
    let start_local = Local
        .from_local_datetime(&start_dt)
        .single()
        .ok_or("ambiguous local midnight")?;
    let start_ms = start_local.timestamp_millis();

    db.0
        .call(move |conn| {
            // 基础统计
            let (tokens, cache_tokens, input_tokens, total_requests): (i64, i64, i64, i64) = conn
                .query_row(
                    "SELECT COALESCE(SUM(input_tokens + output_tokens), 0), \
                     COALESCE(SUM(cache_tokens), 0), \
                     COALESCE(SUM(input_tokens), 0), \
                     COUNT(*) \
                     FROM proxy_log WHERE created_at >= ?1 AND deleted_at = 0",
                    params![start_ms],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?;

            let cache_rate = if input_tokens > 0 {
                cache_tokens as f64 / input_tokens as f64 * 100.0
            } else {
                0.0
            };

            // 计算花费：直接使用持久化的 est_cost
            let cost: f64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(est_cost), 0.0) FROM proxy_log WHERE created_at >= ?1 AND deleted_at = 0",
                    params![start_ms],
                    |row| row.get(0),
                )
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "today cost sum query failed, reporting cost=0.0");
                    0.0
                });

            Ok(TodayStats {
                tokens,
                cache_rate,
                cost,
                total_requests,
            })
        })
        .await
        .map_err(|e| format!("today stats: {e}"))
}

/// 根据 model_price 定价计算单次请求预估花费（$）
///
/// 复用 `resolve_price` 的回退链（pricing[platform_type] > top_level >
/// default_platform > fallback 默认价），与 preview 命令 `model_price_resolve` 行为一致：
/// 无模型价 / 价为 0 时回退到 `PriceSyncSettings` 的 fallback 默认价（默认 3.0 $/M），不再返回 0。
///
/// 锁安全：本函数不持有 `db.0.lock()`；`get_sync_settings` / `resolve_price`
/// （内部 `get_model_price`）各自获取并释放 db 锁，不会重入死锁。
///
/// `platform_type` 传入平台主类型的 serde 裸名（如 `"deepseek"`）以启用 pricing override；
/// 传 `""` 时 override 不命中，但回退链仍保证非 0。
pub async fn calc_est_cost(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
) -> f64 {
    let settings = super::price_sync::get_sync_settings(db).await;
    let rp = resolve_price(
        db,
        model_name,
        platform_type,
        settings.fallback_input_price,
        settings.fallback_output_price,
    )
    .await
    .unwrap_or_else(|_| super::models::ResolvedPrice {
        // 安全默认：直接用 fallback 默认价（$/M → $/token），保证非 0、不 panic
        input_cost_per_token: settings.fallback_input_price / 1_000_000.0,
        output_cost_per_token: settings.fallback_output_price / 1_000_000.0,
        cache_read_input_token_cost: 0.0,
        source: "fallback".to_string(),
    });

    input_tokens as f64 * rp.input_cost_per_token
        + output_tokens as f64 * rp.output_cost_per_token
        + cache_tokens as f64 * rp.cache_read_input_token_cost
}

// ─── Group CRUD ────────────────────────────────────────────

/// 序列化 / 反序列化内联 model_mappings
fn serialize_mappings(mappings: &[ModelMapping]) -> String {
    serde_json::to_string(mappings).unwrap_or_else(|_| "[]".to_string())
}

fn parse_mappings(json: &str) -> Vec<ModelMapping> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Group SELECT 列序
const GROUP_COLUMNS: &str =
    "id, name, path, routing_mode, auto_from_platform, created_at, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, model_mappings, sort_order, max_retries";

fn row_to_group(row: &rusqlite::Row) -> SqlResult<Group> {
    let routing_str: String = row.get(3)?;
    let mappings_str: String = row.get(10)?;
    Ok(Group {
        id: row.get::<_, i64>(0)? as u64,
        name: row.get(1)?,
        path: row.get(2)?,
        routing_mode: serde_json::from_str(&routing_str).unwrap(),
        auto_from_platform: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        request_timeout_secs: row.get::<_, i64>(7)? as u64,
        connect_timeout_secs: row.get::<_, i64>(8)? as u64,
        source_protocol: row.get::<_, String>(9)?,
        model_mappings: parse_mappings(&mappings_str),
        deleted_at: 0,
        sort_order: row.get::<_, i64>(11)?,
        max_retries: row.get::<_, i64>(12)? as u32,
    })
}

pub async fn create_group(db: &Db, input: CreateGroup) -> Result<Group, String> {
    let ts = now();
    let routing_str = serde_json::to_string(&input.routing_mode).unwrap();
    let source_protocol = input.source_protocol.unwrap_or_else(|| "anthropic".to_string());
    let mappings_str = serialize_mappings(&input.model_mappings);

    let id = db
        .0
        .call({
            let name = input.name.clone();
            let path = input.path.clone();
            let auto_from_platform = input.auto_from_platform.clone();
            let request_timeout_secs = input.request_timeout_secs as i64;
            let connect_timeout_secs = input.connect_timeout_secs as i64;
            let source_protocol = source_protocol.clone();
            let max_retries = input.max_retries as i64;
            move |conn| {
                conn.execute(
                    "INSERT INTO \"group\" (name, path, routing_mode, auto_from_platform, created_at, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, model_mappings, max_retries) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![name, path, routing_str, auto_from_platform, ts, ts, request_timeout_secs, connect_timeout_secs, source_protocol, mappings_str, max_retries],
                )?;
                Ok(conn.last_insert_rowid() as u64)
            }
        })
        .await
        .map_err(|e| format!("create group: {e}"))?;

    Ok(Group {
        id,
        name: input.name,
        path: input.path,
        routing_mode: input.routing_mode,
        auto_from_platform: input.auto_from_platform,
        created_at: ts,
        updated_at: ts,
        request_timeout_secs: input.request_timeout_secs,
        connect_timeout_secs: input.connect_timeout_secs,
        source_protocol,
        model_mappings: input.model_mappings,
        deleted_at: 0,
        sort_order: 0,
        max_retries: input.max_retries,
    })
}

/// 批量更新 group 的 sort_order：接收有序 id 列表，按序赋值 1, 2, 3, …
pub async fn reorder_groups(db: &Db, ordered_ids: &[u64]) -> Result<(), String> {
    let ordered_ids = ordered_ids.to_vec();
    db.0
        .call(move |conn| {
            for (i, &id) in ordered_ids.iter().enumerate() {
                conn.execute(
                    "UPDATE \"group\" SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                    params![(i + 1) as i64, now(), id as i64],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("reorder group: {e}"))
}

/// 批量更新 platform 的 sort_order
pub async fn reorder_platforms(db: &Db, ordered_ids: &[u64]) -> Result<(), String> {
    let ordered_ids = ordered_ids.to_vec();
    db.0
        .call(move |conn| {
            for (i, &id) in ordered_ids.iter().enumerate() {
                conn.execute(
                    "UPDATE platform SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                    params![(i + 1) as i64, now(), id as i64],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("reorder platform: {e}"))
}

pub async fn list_groups(db: &Db) -> Result<Vec<Group>, String> {
    db.0
        .call(|conn| {
            let mut stmt = conn.prepare(&format!("SELECT {GROUP_COLUMNS} FROM \"group\" WHERE deleted_at = 0 ORDER BY sort_order, created_at"))?;
            let rows = stmt.query_map([], row_to_group)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_group(db: &Db, id: u64) -> Result<Option<Group>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(&format!("SELECT {GROUP_COLUMNS} FROM \"group\" WHERE id = ?1 AND deleted_at = 0"))?;
            Ok(stmt.query_row(params![id as i64], row_to_group).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn update_group(db: &Db, input: UpdateGroup) -> Result<Group, String> {
    let existing = get_group(db, input.id).await?.ok_or("group not found")?;

    let updated = Group {
        name: input.name.unwrap_or(existing.name),
        path: input.path.unwrap_or(existing.path),
        routing_mode: input.routing_mode.unwrap_or(existing.routing_mode),
        request_timeout_secs: if input.request_timeout_secs > 0 { input.request_timeout_secs } else { existing.request_timeout_secs },
        connect_timeout_secs: if input.connect_timeout_secs > 0 { input.connect_timeout_secs } else { existing.connect_timeout_secs },
        source_protocol: input.source_protocol.unwrap_or(existing.source_protocol),
        max_retries: input.max_retries.unwrap_or(existing.max_retries),
        model_mappings: input.model_mappings,
        updated_at: now(),
        ..existing
    };

    let routing_str = serde_json::to_string(&updated.routing_mode).unwrap();
    let mappings_str = serialize_mappings(&updated.model_mappings);
    db.0
        .call({
            let name = updated.name.clone();
            let path = updated.path.clone();
            let updated_at = updated.updated_at;
            let request_timeout_secs = updated.request_timeout_secs as i64;
            let connect_timeout_secs = updated.connect_timeout_secs as i64;
            let source_protocol = updated.source_protocol.clone();
            let max_retries = updated.max_retries as i64;
            let id = updated.id as i64;
            move |conn| {
                conn.execute(
                    "UPDATE \"group\" SET name=?1, path=?2, routing_mode=?3, updated_at=?4, request_timeout_secs=?5, connect_timeout_secs=?6, source_protocol=?7, model_mappings=?8, max_retries=?9 WHERE id=?10",
                    params![name, path, routing_str, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, mappings_str, max_retries, id],
                )?;
                Ok(())
            }
        })
        .await
        .map_err(|e| format!("update group: {e}"))?;

    Ok(updated)
}

pub async fn delete_group(db: &Db, id: u64) -> Result<(), String> {
    // 检查是否为自动分组
    let group = get_group(db, id).await?.ok_or("group not found")?;
    if !group.auto_from_platform.is_empty() {
        return Err("auto-created group cannot be deleted manually".to_string());
    }
    force_delete_group(db, id).await
}

/// 强制删除分组（含自动分组），仅供平台删除时内部调用
pub async fn force_delete_group(db: &Db, id: u64) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute("UPDATE \"group\" SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete group: {e}"))
}

// ─── GroupPlatform 关联 ────────────────────────────────────

pub async fn set_group_platforms(
    db: &Db,
    group_id: u64,
    platforms: &[GroupPlatformInput],
) -> Result<(), String> {
    let ts = now();
    let platforms = platforms.to_vec();
    db.0
        .call(move |conn| {
            // 物理清除旧关联后重建（关联表无需软删保留）
            conn.execute(
                "DELETE FROM group_platform WHERE group_id = ?1",
                params![group_id as i64],
            )?;

            for p in &platforms {
                conn.execute(
                    "INSERT INTO group_platform (group_id, platform_id, priority, weight, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![group_id as i64, p.platform_id as i64, p.priority.unwrap_or(0), p.weight.unwrap_or(1), ts, ts],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("set group platforms: {e}"))
}

pub async fn get_group_platforms(db: &Db, group_id: u64) -> Result<Vec<GroupPlatformDetail>, String> {
    db.0
        .call(move |conn| {
    let mut stmt = conn
        .prepare(
            &format!(
                "SELECT gp.priority, gp.weight, {PLATFORM_COLUMNS_PREFIXED} \
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
                    manual_budgets: super::models::parse_manual_budgets(&row.get::<_, String>(21)?),
                    status: super::models::PlatformStatus::from_db_str(&row.get::<_, String>(22)?),
                    auto_disabled_until: row.get::<_, i64>(23)?,
                    auto_disable_strikes: row.get::<_, i64>(24)?,
                    balance_level: String::new(),
                },
                priority: row.get(0)?,
                weight: row.get(1)?,
            })
        })?;

    Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
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
    Ok(details)
}

// ─── Settings CRUD ─────────────────────────────────────────

pub async fn get_setting(
    db: &Db,
    scope: &str,
    key: &str,
) -> Result<Option<serde_json::Value>, String> {
    let scope = scope.to_string();
    let key = key.to_string();
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare("SELECT value FROM setting WHERE scope = ?1 AND key = ?2 AND deleted_at = 0")?;
            stmt.query_row(params![scope, key], |row| {
                let v: String = row.get(0)?;
                Ok(serde_json::from_str(&v).unwrap_or_else(|e| {
                    tracing::warn!(scope = %scope, key = %key, error = %e, "stored setting value is not valid JSON, returning Null");
                    serde_json::Value::Null
                }))
            })
            .optional()
            .map_err(tokio_rusqlite::Error::from)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn set_setting(db: &Db, input: SetSettingInput) -> Result<(), String> {
    let ts = now();
    let value_str =
        serde_json::to_string(&input.value).map_err(|e| format!("serialize setting: {e}"))?;
    db.0
        .call(move |conn| {
            conn.execute(
                "INSERT INTO setting (scope, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(scope, key) DO UPDATE SET value = ?3, updated_at = ?4, deleted_at = 0",
                params![input.scope, input.key, value_str, ts],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert setting: {e}"))
}

pub async fn delete_setting(db: &Db, scope: &str, key: &str) -> Result<(), String> {
    let scope = scope.to_string();
    let key = key.to_string();
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE setting SET deleted_at = ?1 WHERE scope = ?2 AND key = ?3",
                params![now(), scope, key],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete setting: {e}"))
}

pub async fn list_setting_keys(db: &Db, scope: &str) -> Result<Vec<String>, String> {
    let scope = scope.to_string();
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare("SELECT key FROM setting WHERE scope = ?1 AND deleted_at = 0 ORDER BY key")?;
            let rows = stmt.query_map(params![scope], |row| row.get(0))?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

// ─── ProxyLog CRUD ─────────────────────────────────────────

/// proxy_log 全列序（INSERT / 单行 SELECT 共用，与表定义列序一致）
const PROXY_LOG_COLUMNS: &str =
    "id, group_name, model, actual_model, source_protocol, target_protocol, platform_id, request_headers, request_body, upstream_request_headers, upstream_request_body, response_body, request_url, upstream_request_url, upstream_response_headers, upstream_status_code, user_response_headers, user_response_body, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, est_cost, is_stream, attempts, retry_count, created_at, updated_at, deleted_at";

/// 从查询行构造 ProxyLog（列序须与 PROXY_LOG_COLUMNS 一致）
fn row_to_proxy_log(row: &rusqlite::Row) -> SqlResult<super::models::ProxyLog> {
    Ok(super::models::ProxyLog {
        id: row.get(0)?,
        group_name: row.get(1)?,
        model: row.get(2)?,
        actual_model: row.get(3)?,
        source_protocol: row.get(4)?,
        target_protocol: row.get(5)?,
        platform_id: row.get::<_, i64>(6)? as u64,
        request_headers: row.get(7)?,
        request_body: row.get(8)?,
        upstream_request_headers: row.get(9)?,
        upstream_request_body: row.get(10)?,
        response_body: row.get(11)?,
        request_url: row.get(12)?,
        upstream_request_url: row.get(13)?,
        upstream_response_headers: row.get(14)?,
        upstream_status_code: row.get(15)?,
        user_response_headers: row.get(16)?,
        user_response_body: row.get(17)?,
        status_code: row.get(18)?,
        duration_ms: row.get(19)?,
        input_tokens: row.get(20)?,
        output_tokens: row.get(21)?,
        cache_tokens: row.get(22)?,
        est_cost: row.get(23)?,
        is_stream: row.get::<_, i64>(24)? == 1,
        attempts: super::models::parse_attempts(&row.get::<_, String>(25)?),
        retry_count: row.get(26)?,
        created_at: row.get(27)?,
        updated_at: row.get(28)?,
        deleted_at: row.get(29)?,
    })
}

/// Upsert (INSERT OR REPLACE) a proxy log entry — used for incremental logging
pub async fn upsert_proxy_log(db: &Db, log: &super::models::ProxyLog) -> Result<(), String> {
    let log = log.clone();
    db.0
        .call(move |conn| {
            let attempts_str = super::models::serialize_attempts(&log.attempts);
            conn.execute(
                &format!("INSERT OR REPLACE INTO proxy_log ({PROXY_LOG_COLUMNS})
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30)"),
                params![log.id, log.group_name, log.model, log.actual_model, log.source_protocol, log.target_protocol, log.platform_id as i64, log.request_headers, log.request_body, log.upstream_request_headers, log.upstream_request_body, log.response_body, log.request_url, log.upstream_request_url, log.upstream_response_headers, log.upstream_status_code, log.user_response_headers, log.user_response_body, log.status_code, log.duration_ms, log.input_tokens, log.output_tokens, log.cache_tokens, log.est_cost, log.is_stream as i64, attempts_str, log.retry_count, log.created_at, log.updated_at, log.deleted_at],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert proxy log: {e}"))
}

pub async fn list_proxy_logs(db: &Db, limit: u32, offset: u32) -> Result<Vec<super::models::ProxyLogSummary>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, group_name, model, actual_model, source_protocol, target_protocol, platform_id, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at
                 FROM proxy_log WHERE deleted_at = 0 ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
            )?;
            let rows = stmt.query_map(params![limit, offset], row_to_proxy_log_summary)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Summary row mapper (column order must match SELECT)
fn row_to_proxy_log_summary(row: &rusqlite::Row) -> SqlResult<super::models::ProxyLogSummary> {
    Ok(super::models::ProxyLogSummary {
        id: row.get(0)?,
        group_name: row.get(1)?,
        model: row.get(2)?,
        actual_model: row.get(3)?,
        source_protocol: row.get(4)?,
        target_protocol: row.get(5)?,
        platform_id: row.get::<_, i64>(6)? as u64,
        status_code: row.get(7)?,
        duration_ms: row.get(8)?,
        input_tokens: row.get(9)?,
        output_tokens: row.get(10)?,
        cache_tokens: row.get(11)?,
        is_stream: row.get::<_, i64>(12)? == 1,
        retry_count: row.get(13)?,
        created_at: row.get(14)?,
    })
}

pub async fn filtered_list_proxy_logs(
    db: &Db,
    filter: &super::models::ProxyLogFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<super::models::ProxyLogSummary>, String> {
    let filter = filter.clone();
    db.0
        .call(move |conn| {
            let (where_sql, mut p) = build_filter_where(&filter);
            p.push(Box::new(limit));
            p.push(Box::new(offset));
            let sql = format!(
                "SELECT id, group_name, model, actual_model, source_protocol, target_protocol, platform_id, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at \
                 FROM proxy_log WHERE deleted_at = 0{where_sql} ORDER BY created_at DESC LIMIT ? OFFSET ?"
            );
            let mut stmt = conn.prepare(&sql)?;
            let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
            let rows = stmt.query_map(refs.as_slice(), row_to_proxy_log_summary)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn filtered_count_proxy_logs(
    db: &Db,
    filter: &super::models::ProxyLogFilter,
) -> Result<u32, String> {
    let filter = filter.clone();
    db.0
        .call(move |conn| {
            let (where_sql, p) = build_filter_where(&filter);
            let sql = format!("SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0{where_sql}");
            let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
            Ok(conn.query_row(&sql, refs.as_slice(), |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Build WHERE clause extensions + params from filter.
/// Returns (" AND ...", params). Empty filter → ("", []).
fn build_filter_where(filter: &super::models::ProxyLogFilter) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut parts: Vec<String> = Vec::new();
    let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1u32;

    if let Some(ref v) = filter.platform_id {
        parts.push(format!("AND platform_id = ?{idx}"));
        p.push(Box::new(*v as i64));
        idx += 1;
    }
    if let Some(ref v) = filter.group_name {
        parts.push(format!("AND group_name = ?{idx}"));
        p.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(s) = filter.status {
        if s == 200 {
            parts.push("AND status_code >= 200 AND status_code < 300".to_string());
        } else if s == -1 {
            parts.push("AND (status_code < 200 OR status_code >= 300)".to_string());
        } else {
            parts.push(format!("AND status_code = ?{idx}"));
            p.push(Box::new(s));
            idx += 1;
        }
    }
    if let Some(ts) = filter.time_start {
        parts.push(format!("AND created_at >= ?{idx}"));
        p.push(Box::new(ts));
        idx += 1;
    }
    if let Some(ts) = filter.time_end {
        parts.push(format!("AND created_at <= ?{idx}"));
        p.push(Box::new(ts));
        idx += 1;
    }
    if let Some(ref v) = filter.model {
        let col = match filter.model_type.as_deref() {
            Some("actual") => "actual_model",
            _ => "model",
        };
        parts.push(format!("AND {col} = ?{idx}"));
        p.push(Box::new(v.clone()));
    }

    let where_sql = if parts.is_empty() { String::new() } else { format!(" {}", parts.join(" ")) };
    (where_sql, p)
}

pub async fn get_proxy_log(db: &Db, id: &str) -> Result<Option<super::models::ProxyLog>, String> {
    let id = id.to_string();
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {PROXY_LOG_COLUMNS} FROM proxy_log WHERE id = ?1 AND deleted_at = 0"
            ))?;
            Ok(stmt.query_row(params![id], row_to_proxy_log).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn clear_proxy_logs(db: &Db) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute("UPDATE proxy_log SET deleted_at = ?1 WHERE deleted_at = 0", params![now()])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("clear proxy logs: {e}"))
}

/// Delete logs older than N days. Pass 0 to skip.
pub async fn cleanup_proxy_logs(db: &Db, retention_days: u32) -> Result<(), String> {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db.0
        .call(move |conn| {
            conn.execute("UPDATE proxy_log SET deleted_at = ?1 WHERE created_at < ?2 AND deleted_at = 0", params![now(), cutoff])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup proxy logs: {e}"))
}

/// Clear user request fields (headers, body, user response) for logs older than retention_days.
/// Does NOT delete the log row — keeps token stats and metadata.
pub async fn cleanup_user_request_fields(db: &Db, retention_days: u32) -> Result<(), String> {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE proxy_log SET request_headers = '', request_body = '', user_response_headers = '', user_response_body = '' WHERE created_at < ?1 AND (request_headers != '' OR request_body != '')",
                params![cutoff],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup user request fields: {e}"))
}

/// Clear upstream request fields (headers, body, response headers) for logs older than retention_days.
/// Does NOT delete the log row — keeps token stats and metadata.
pub async fn cleanup_upstream_request_fields(db: &Db, retention_days: u32) -> Result<(), String> {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE proxy_log SET upstream_request_headers = '', upstream_request_body = '', upstream_response_headers = '' WHERE created_at < ?1 AND (upstream_request_headers != '' OR upstream_request_body != '')",
                params![cutoff],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup upstream request fields: {e}"))
}

pub async fn count_proxy_logs(db: &Db) -> Result<u32, String> {
    db.0
        .call(move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0", [], |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 共用使用量聚合：按给定 WHERE 子句统计总量 + 最近 5 次健康度。
/// `where_clause` 不含 `WHERE` 关键字；`params` 须与 `where_clause` 占位符一一对应。
fn usage_stats(
    conn: &Connection,
    where_clause: &str,
    params: &[&dyn rusqlite::types::ToSql],
) -> SqlResult<super::models::PlatformUsageStats> {
    let stats: super::models::PlatformUsageStats = conn.query_row(
        &format!("SELECT COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), \
         COALESCE(SUM(est_cost), 0.0) \
         FROM proxy_log WHERE {where_clause}"),
        params,
        |row| {
            let total: i64 = row.get(0).unwrap_or(0);
            let success: i64 = row.get(1).unwrap_or(0);
            let inp: i64 = row.get(2).unwrap_or(0);
            let out: i64 = row.get(3).unwrap_or(0);
            let cache: i64 = row.get(4).unwrap_or(0);
            let cost: f64 = row.get(5).unwrap_or(0.0);
            Ok(super::models::PlatformUsageStats {
                total_requests: total,
                success_count: success,
                total_input_tokens: inp,
                total_output_tokens: out,
                total_cache_tokens: cache,
                cache_rate: if inp > 0 { cache as f64 / inp as f64 * 100.0 } else { 0.0 },
                recent_failures: 0,
                recent_total: 0,
                total_cost: cost,
            })
        },
    )?;

    // Recent 5 requests health
    let (recent_failures, recent_total): (i64, i64) = conn.query_row(
        &format!("SELECT COUNT(*), SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END) \
         FROM (SELECT status_code FROM proxy_log WHERE {where_clause} ORDER BY created_at DESC LIMIT 5)"),
        params,
        |row| Ok((row.get(1).unwrap_or(0), row.get(0).unwrap_or(0))),
    ).unwrap_or((0, 0));

    Ok(super::models::PlatformUsageStats {
        recent_failures,
        recent_total,
        ..stats
    })
}

pub async fn get_platform_usage_stats(db: &Db, platform_id: u64) -> Result<super::models::PlatformUsageStats, String> {
    db.0
        .call(move |conn| {
            // platform_id 现为整数；自动分组日志可能未带 platform_id（=0），通过 group.auto_from_platform（存十进制字符串）回溯
            let where_clause = "deleted_at = 0 AND (platform_id = ?1 OR (platform_id = 0 AND group_name IN (SELECT name FROM \"group\" WHERE auto_from_platform = ?2 AND deleted_at = 0)))";
            let pid = platform_id as i64;
            let pid_str = platform_id.to_string();
            Ok(usage_stats(conn, where_clause, &[&pid, &pid_str])?)
        })
        .await
        .map_err(|e| format!("platform usage stats: {e}"))
}

pub async fn get_group_usage_stats(db: &Db, group_name: &str) -> Result<super::models::PlatformUsageStats, String> {
    let group_name = group_name.to_string();
    db.0
        .call(move |conn| {
            Ok(usage_stats(conn, "group_name = ?1 AND deleted_at = 0", &[&group_name])?)
        })
        .await
        .map_err(|e| format!("group usage stats: {e}"))
}

/// 动态窗口日速率公共常量。
const RATE_MIN_SPAN_MS: i64 = 5 * 60 * 1000; // 5min
const RATE_MAX_SPAN_MS: i64 = 7 * 24 * 60 * 60 * 1000; // 7d

/// 动态窗口日用量速率核心（同步，锁内调用）。
///
/// 算法（prd B）：`?1` = window_start（now-7d），`scope_sql` 为附加维度过滤（group / platform），
/// `scope_params` 从 `?3` 起绑定。span = clamp(now - 最早有效 est_cost 数据时间, 5min, 7d)，
/// `rate_per_hour = SUM(est_cost in span) / span_hours`。无任何用量 → None。
fn hourly_rate_inner(
    conn: &Connection,
    now_ms: i64,
    window_start: i64,
    scope_sql: &str,
    scope_params: &[&dyn rusqlite::types::ToSql],
) -> SqlResult<Option<f64>> {
    let mut binds: Vec<&dyn rusqlite::types::ToSql> = vec![&window_start];
    binds.extend_from_slice(scope_params);
    // 7d 窗口内最早一条有 est_cost(>0) 数据的时间。
    let earliest_sql = format!(
        "SELECT MIN(created_at) FROM proxy_log \
         WHERE created_at >= ?1 AND deleted_at = 0 AND est_cost > 0 AND ({scope_sql})"
    );
    let earliest: Option<i64> = conn
        .query_row(&earliest_sql, binds.as_slice(), |row| row.get(0))
        .optional()?
        .flatten();
    let earliest = match earliest {
        Some(e) => e,
        None => return Ok(None), // 无任何用量 → None
    };
    let total_sql = format!(
        "SELECT COALESCE(SUM(est_cost), 0.0) FROM proxy_log \
         WHERE created_at >= ?1 AND deleted_at = 0 AND ({scope_sql})"
    );
    let total: f64 = conn.query_row(&total_sql, binds.as_slice(), |row| row.get(0))?;
    if total <= 0.0 {
        return Ok(None);
    }
    // span = clamp(now - earliest, 5min, 7d)
    let span_ms = (now_ms - earliest).clamp(RATE_MIN_SPAN_MS, RATE_MAX_SPAN_MS);
    let span_hours = span_ms as f64 / 3_600_000.0;
    Ok(Some(total / span_hours))
}

/// 分组动态窗口日用量速率（$ / 小时），供 statusline 余额「剩余可用天数」配色。
/// 无任何用量 → None（配色侧视作中性 / 不报警）。短持锁，不跨 await。
pub async fn get_group_hourly_rate(db: &Db, group_name: &str) -> Result<Option<f64>, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let window_start = now_ms - RATE_MAX_SPAN_MS;
    let group_name = group_name.to_string();
    db.0
        .call(move |conn| {
            Ok(hourly_rate_inner(conn, now_ms, window_start, "group_name = ?2", &[&group_name])?)
        })
        .await
        .map_err(|e| format!("group hourly rate: {e}"))
}

/// 单平台动态窗口日用量速率（$ / 小时），供 Platforms 列表页余额按速率配色。
///
/// platform 维度过滤同 `get_platform_usage_stats`：自动分组日志可能 platform_id=0，
/// 经 group.auto_from_platform 回溯。无任何用量 → None（前端退中性）。短持锁，不跨 await。
pub async fn get_platform_hourly_rate(db: &Db, platform_id: u64) -> Result<Option<f64>, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let window_start = now_ms - RATE_MAX_SPAN_MS;
    db.0
        .call(move |conn| {
            let pid = platform_id as i64;
            let pid_str = platform_id.to_string();
            let scope = "platform_id = ?2 OR (platform_id = 0 AND group_name IN (SELECT name FROM \"group\" WHERE auto_from_platform = ?3 AND deleted_at = 0))";
            Ok(hourly_rate_inner(conn, now_ms, window_start, scope, &[&pid, &pid_str])?)
        })
        .await
        .map_err(|e| format!("platform hourly rate: {e}"))
}

struct QueryParams {
    start: i64,
    end: i64,
    filter_group: Option<String>,
    filter_model: Option<String>,
    filter_protocol: Option<String>,
}

impl QueryParams {
    fn to_sql_params(&self) -> Vec<Box<dyn rusqlite::types::ToSql>> {
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(self.start),
            Box::new(self.end),
        ];
        if let Some(ref v) = self.filter_group { p.push(Box::new(v.clone())); }
        if let Some(ref v) = self.filter_model { p.push(Box::new(v.clone())); }
        if let Some(ref v) = self.filter_protocol { p.push(Box::new(v.clone())); }
        p
    }
}

pub async fn query_stats(db: &Db, query: &StatsQuery) -> Result<StatsResult, String> {
    let query = query.clone();
    db.0
        .call(move |conn| {
            query_stats_inner(conn, &query)
                .map_err(|e| tokio_rusqlite::Error::Other(e.into()))
        })
        .await
        .map_err(|e| e.to_string())
}

fn query_stats_inner(conn: &Connection, query: &StatsQuery) -> Result<StatsResult, String> {
    let end = query.end.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let start = query.start.unwrap_or_else(|| {
        (chrono::Utc::now() - chrono::Duration::days(7)).timestamp_millis()
    });

    let qp = QueryParams {
        start,
        end,
        filter_group: query.filter_group.clone(),
        filter_model: query.filter_model.clone(),
        filter_protocol: query.filter_protocol.clone(),
    };

    // Build WHERE clause
    let mut where_parts = vec!["created_at >= ?1".to_string(), "created_at <= ?2".to_string()];
    if qp.filter_group.is_some() {
        where_parts.push("group_name = ?3".to_string());
    }
    if qp.filter_model.is_some() {
        let idx = 3 + qp.filter_group.is_some() as usize;
        where_parts.push(format!("(model = ?{idx} OR actual_model = ?{idx})"));
    }
    if qp.filter_protocol.is_some() {
        let idx = 3 + qp.filter_group.is_some() as usize + qp.filter_model.is_some() as usize;
        where_parts.push(format!("target_protocol = ?{idx}"));
    }
    let where_sql = where_parts.join(" AND ");

    let time_fmt = match query.granularity.as_deref() {
        Some("hourly") => "%Y-%m-%d %H:00",
        _ => "%Y-%m-%d",
    };

    // ── Overview ──
    let overview_sql = format!(
        "SELECT COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
         COALESCE(SUM(est_cost), 0.0) \
         FROM proxy_log WHERE deleted_at = 0 AND {where_sql}"
    );
    let p = qp.to_sql_params();
    let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
    let overview = conn.prepare(&overview_sql)
        .map_err(|e| e.to_string())?
        .query_row(refs.as_slice(), |row| {
            let total: i32 = row.get(0).unwrap_or(0);
            let success: i32 = row.get(1).unwrap_or(0);
            Ok(StatsOverview {
                total_requests: total,
                success_rate: if total > 0 { success as f64 / total as f64 * 100.0 } else { 0.0 },
                total_input_tokens: row.get(2).unwrap_or(0),
                total_output_tokens: row.get(3).unwrap_or(0),
                total_cache_tokens: row.get(4).unwrap_or(0),
                cache_rate: {
                    let inp: i64 = row.get(2).unwrap_or(0);
                    if inp > 0 { row.get::<_, i64>(4).unwrap_or(0) as f64 / inp as f64 * 100.0 } else { 0.0 }
                },
                avg_duration_ms: row.get(5).unwrap_or(0.0),
                total_cost: row.get(6).unwrap_or(0.0),
            })
        }).map_err(|e| format!("overview: {e}"))?;

    // ── Time buckets ──
    let bucket_sql = format!(
        "SELECT strftime('{time_fmt}', created_at/1000, 'unixepoch'), COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
         COALESCE(SUM(est_cost), 0.0) \
         FROM proxy_log WHERE deleted_at = 0 AND {where_sql} GROUP BY 1 ORDER BY 1"
    );
    let p = qp.to_sql_params();
    let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
    let buckets: Vec<StatsBucket> = conn.prepare(&bucket_sql)
        .map_err(|e| e.to_string())?
        .query_map(refs.as_slice(), |row| {
            Ok(StatsBucket {
                time_bucket: row.get(0).unwrap_or_default(),
                total_requests: row.get(1).unwrap_or(0),
                success_count: row.get(2).unwrap_or(0),
                error_count: row.get(3).unwrap_or(0),
                input_tokens: row.get(4).unwrap_or(0),
                output_tokens: row.get(5).unwrap_or(0),
                cache_tokens: row.get(6).unwrap_or(0),
                avg_duration_ms: row.get(7).unwrap_or(0.0),
                total_cost: row.get(8).unwrap_or(0.0),
            })
        }).map_err(|e| format!("buckets: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // ── Dimension breakdown ──
    let dimension_data = if let Some(ref gb) = query.group_by {
        let dim_col = match gb.as_str() {
            "platform" => "target_protocol",
            "model" => "actual_model",
            "group" => "group_name",
            _ => "target_protocol",
        };
        let dim_sql = format!(
            "SELECT {dim_col}, COUNT(*), \
             SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
             SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
             COALESCE(SUM(est_cost), 0.0) \
             FROM proxy_log WHERE deleted_at = 0 AND {where_sql} GROUP BY 1 ORDER BY 2 DESC LIMIT 50"
        );
        let p = qp.to_sql_params();
        let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
        conn.prepare(&dim_sql)
            .map_err(|e| e.to_string())?
            .query_map(refs.as_slice(), |row| {
                Ok(DimensionEntry {
                    name: row.get(0).unwrap_or_default(),
                    total_requests: row.get(1).unwrap_or(0),
                    success_count: row.get(2).unwrap_or(0),
                    input_tokens: row.get(3).unwrap_or(0),
                    output_tokens: row.get(4).unwrap_or(0),
                    cache_tokens: row.get(5).unwrap_or(0),
                    avg_duration_ms: row.get(6).unwrap_or(0.0),
                    total_cost: row.get(7).unwrap_or(0.0),
                })
            }).map_err(|e| format!("dimension: {e}"))?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        vec![]
    };

    Ok(StatsResult { overview, buckets, dimension_data })
}

// ─── Model Price CRUD ──────────────────────────────────────

const MODEL_PRICE_COLUMNS: &str =
    "id, model_name, source, price_data, created_at, updated_at, deleted_at";

fn row_to_model_price(row: &rusqlite::Row) -> SqlResult<super::models::ModelPrice> {
    Ok(super::models::ModelPrice {
        id: row.get::<_, i64>(0)? as u64,
        model_name: row.get(1)?,
        source: row.get(2)?,
        price_data: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        deleted_at: row.get(6)?,
    })
}

/// 提取关键字段构建摘要
fn price_data_to_summary(mp: &super::models::ModelPrice) -> super::models::ModelPriceSummary {
    let pd: serde_json::Value = serde_json::from_str(&mp.price_data).unwrap_or_default();
    let input = pd.get("input_cost_per_token").and_then(|v| v.as_f64());
    let output = pd.get("output_cost_per_token").and_then(|v| v.as_f64());
    let cache_read = pd.get("cache_read_input_token_cost").and_then(|v| v.as_f64());
    let default_platform = pd.get("default_platform").and_then(|v| v.as_str()).map(String::from);

    super::models::ModelPriceSummary {
        id: mp.id,
        model_name: mp.model_name.clone(),
        source: mp.source.clone(),
        default_platform,
        // Convert $/token → $/M tokens for display
        input_price: input.map(|v| v * 1_000_000.0),
        output_price: output.map(|v| v * 1_000_000.0),
        cache_read_price: cache_read.map(|v| v * 1_000_000.0),
        updated_at: mp.updated_at,
    }
}

pub async fn list_model_prices(db: &Db, limit: u32, offset: u32) -> Result<Vec<super::models::ModelPriceSummary>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE deleted_at = 0 ORDER BY model_name LIMIT ?1 OFFSET ?2")
            )?;
            let rows = stmt.query_map(params![limit, offset], row_to_model_price)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(price_data_to_summary(&r?));
            }
            Ok(result)
        })
        .await
        .map_err(|e| e.to_string())
}

pub async fn count_model_prices(db: &Db) -> Result<u32, String> {
    db.0
        .call(move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM model_price WHERE deleted_at = 0", [], |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 获取指定模型的最新价格记录（优先 manual > litellm）
pub async fn get_model_price(db: &Db, model_name: &str) -> Result<Option<super::models::ModelPrice>, String> {
    let model_name = model_name.to_string();
    db.0
        .call(move |conn| {
            // 优先取 manual 记录
            let mut stmt = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE model_name = ?1 AND source = 'manual' AND deleted_at = 0")
            )?;
            if let Some(mp) = stmt.query_row(params![model_name], row_to_model_price).optional()? {
                return Ok(Some(mp));
            }
            // 回退到 litellm
            let mut stmt2 = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE model_name = ?1 AND source = 'litellm' AND deleted_at = 0")
            )?;
            Ok(stmt2.query_row(params![model_name], row_to_model_price).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Upsert a model price record (INSERT OR REPLACE by model_name + source)
pub async fn upsert_model_price(
    db: &Db,
    model_name: &str,
    source: &str,
    price_data: &str,
) -> Result<(), String> {
    let ts = now();
    let model_name = model_name.to_string();
    let source = source.to_string();
    let price_data = price_data.to_string();
    db.0
        .call(move |conn| {
            conn.execute(
                "INSERT INTO model_price (model_name, source, price_data, created_at, updated_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?4, 0)
                 ON CONFLICT(model_name, source) DO UPDATE SET price_data = ?3, updated_at = ?4, deleted_at = 0",
                params![model_name, source, price_data, ts],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert model price: {e}"))
}

/// Delete a model price by name (soft delete all sources)
pub async fn delete_model_price(db: &Db, model_name: &str) -> Result<(), String> {
    let model_name = model_name.to_string();
    db.0
        .call(move |conn| {
            conn.execute("UPDATE model_price SET deleted_at = ?1 WHERE model_name = ?2 AND deleted_at = 0", params![now(), model_name])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete model price: {e}"))
}

/// 解析价格：model_name + platform_type → ResolvedPrice
/// 优先级: pricing[platform_type] > top_level > default_platform pricing > fallback settings
pub async fn resolve_price(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    fallback_input: f64,
    fallback_output: f64,
) -> Result<super::models::ResolvedPrice, String> {
    let mp = get_model_price(db, model_name).await?;
    let pd: serde_json::Value = match &mp {
        Some(m) => serde_json::from_str(&m.price_data).unwrap_or_default(),
        None => serde_json::Value::Null,
    };

    // 1. Try pricing[platform_type]
    if let Some(pricing_node) = pd.get("pricing").and_then(|p| p.get(platform_type)) {
        let input = pricing_node.get("input_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let output = pricing_node.get("output_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let cache = pricing_node.get("cache_read_input_token_cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if input > 0.0 || output > 0.0 {
            return Ok(super::models::ResolvedPrice {
                input_cost_per_token: input,
                output_cost_per_token: output,
                cache_read_input_token_cost: cache,
                source: "platform_override".to_string(),
            });
        }
    }

    // 2. Try top-level price
    let top_input = pd.get("input_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let top_output = pd.get("output_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let top_cache = pd.get("cache_read_input_token_cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if top_input > 0.0 || top_output > 0.0 {
        return Ok(super::models::ResolvedPrice {
            input_cost_per_token: top_input,
            output_cost_per_token: top_output,
            cache_read_input_token_cost: top_cache,
            source: "top_level".to_string(),
        });
    }

    // 3. Try default_platform pricing
    if let Some(dp) = pd.get("default_platform").and_then(|v| v.as_str()) {
        if let Some(pricing_node) = pd.get("pricing").and_then(|p| p.get(dp)) {
            let input = pricing_node.get("input_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let output = pricing_node.get("output_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let cache = pricing_node.get("cache_read_input_token_cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if input > 0.0 || output > 0.0 {
                return Ok(super::models::ResolvedPrice {
                    input_cost_per_token: input,
                    output_cost_per_token: output,
                    cache_read_input_token_cost: cache,
                    source: "default_platform".to_string(),
                });
            }
        }
    }

    // 4. Fallback
    Ok(super::models::ResolvedPrice {
        input_cost_per_token: fallback_input / 1_000_000.0,
        output_cost_per_token: fallback_output / 1_000_000.0,
        cache_read_input_token_cost: 0.0,
        source: "fallback".to_string(),
    })
}

/// 搜索模型价格
pub async fn search_model_prices(db: &Db, query: &str, limit: u32) -> Result<Vec<super::models::ModelPriceSummary>, String> {
    let pattern = format!("%{query}%");
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE deleted_at = 0 AND model_name LIKE ?1 ORDER BY model_name LIMIT ?2")
            )?;
            let rows = stmt.query_map(params![pattern, limit], row_to_model_price)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(price_data_to_summary(&r?));
            }
            Ok(result)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Filtered list: optional query (LIKE model_name), optional source, limit, offset.
pub async fn filtered_list_model_prices(
    db: &Db,
    query: Option<&str>,
    source: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<super::models::ModelPriceSummary>, String> {
    let query = query.map(|s| s.to_string());
    let source = source.map(|s| s.to_string());
    db.0
        .call(move |conn| {
            let query = query.as_deref();
            let source = source.as_deref();
    let mut where_parts = vec!["deleted_at = 0".to_string()];
    let mut param_idx = 1;
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(q) = query {
        if !q.is_empty() {
            where_parts.push(format!("model_name LIKE ?{param_idx}"));
            params.push(Box::new(format!("%{q}%")));
            param_idx += 1;
        }
    }
    if let Some(s) = source {
        if !s.is_empty() {
            where_parts.push(format!("source = ?{param_idx}"));
            params.push(Box::new(s.to_string()));
            param_idx += 1;
        }
    }

    let where_sql = where_parts.join(" AND ");
    let sql = format!(
        "SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE {where_sql} ORDER BY model_name LIMIT ?{param_idx} OFFSET ?{}",
        param_idx + 1
    );
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), row_to_model_price)?;
    let mut result = Vec::new();
    for r in rows {
        result.push(price_data_to_summary(&r?));
    }
    Ok(result)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Count matching model prices with optional filters.
pub async fn filtered_count_model_prices(
    db: &Db,
    query: Option<&str>,
    source: Option<&str>,
) -> Result<u32, String> {
    let query = query.map(|s| s.to_string());
    let source = source.map(|s| s.to_string());
    db.0
        .call(move |conn| {
            let query = query.as_deref();
            let source = source.as_deref();
    let mut where_parts = vec!["deleted_at = 0".to_string()];
    let mut param_idx = 1;
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(q) = query {
        if !q.is_empty() {
            where_parts.push(format!("model_name LIKE ?{param_idx}"));
            params.push(Box::new(format!("%{q}%")));
            param_idx += 1;
        }
    }
    if let Some(s) = source {
        if !s.is_empty() {
            where_parts.push(format!("source = ?{param_idx}"));
            params.push(Box::new(s.to_string()));
        }
    }

    let where_sql = where_parts.join(" AND ");
    let sql = format!("SELECT COUNT(*) FROM model_price WHERE {where_sql}");
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    Ok(conn.query_row(&sql, param_refs.as_slice(), |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
}

// ─── Tests: DB Schema v2 规范固化 ──────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个初始化好的内存库
    async fn test_db() -> Db {
        let db = Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");
        db
    }

    fn sample_platform(name: &str) -> CreatePlatform {
        CreatePlatform {
            name: name.to_string(),
            platform_type: Protocol::Anthropic,
            base_url: "https://example.com".to_string(),
            api_key: "sk-test".to_string(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
        }
    }

    fn sample_group(name: &str, path: &str, mappings: Vec<ModelMapping>) -> CreateGroup {
        CreateGroup {
            name: name.to_string(),
            path: path.to_string(),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: 2,
            model_mappings: mappings,
        }
    }

    fn sample_log(id: &str, group_name: &str, created_at: i64) -> ProxyLog {
        ProxyLog {
            id: id.to_string(),
            group_name: group_name.to_string(),
            model: "claude-sonnet-4".to_string(),
            actual_model: "glm-4-plus".to_string(),
            source_protocol: "anthropic".to_string(),
            target_protocol: "openai".to_string(),
            platform_id: 1,
            request_headers: String::new(),
            request_body: String::new(),
            upstream_request_headers: String::new(),
            upstream_request_body: String::new(),
            response_body: String::new(),
            request_url: String::new(),
            upstream_request_url: String::new(),
            upstream_response_headers: String::new(),
            upstream_status_code: 200,
            user_response_headers: String::new(),
            user_response_body: String::new(),
            status_code: 200,
            duration_ms: 100,
            input_tokens: 10,
            output_tokens: 20,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: false,
            attempts: Vec::new(),
            retry_count: 0,
            created_at,
            updated_at: created_at,
            deleted_at: 0,
        }
    }

    /// endpoints 反序列化容错：DB 中含未知 client_type（如旧数据 "anthropic"）的
    /// endpoint 数组应仍能完整解析，而非因单个未知枚举值整体失败 → 空 Vec → 前端丢失。
    #[tokio::test]
    async fn endpoints_with_unknown_client_type_still_parse() {
        let json = r#"[{"protocol":"openai","base_url":"https://x/v1","client_type":"codex_tui","coding_plan":false},{"protocol":"anthropic","base_url":"https://x/anthropic","client_type":"anthropic","coding_plan":false}]"#;
        let parsed = parse_endpoints(json);
        assert_eq!(parsed.len(), 2, "未知 client_type 不应导致整个数组解析失败");
        assert_eq!(parsed[1].client_type, ClientType::Default, "未知值回退为 Default");
        assert_eq!(parsed[1].protocol, Protocol::Anthropic);

        // 端到端：写入 DB 后 list_platforms 应带回 endpoints
        let db = test_db().await;
        let mut input = sample_platform("p1");
        input.endpoints = Some(vec![PlatformEndpoint {
            protocol: Protocol::OpenAI,
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            client_type: ClientType::CodexTui,
            coding_plan: true,
        }]);
        create_platform(&db, input).await.unwrap();
        let listed = list_platforms(&db).await.unwrap();
        assert_eq!(listed[0].endpoints.len(), 1, "list_platforms 应返回 endpoints");
    }

    // ── 单平台动态窗口日速率：按 platform_id 过滤 est_cost，span clamp 5min..7d ──
    #[tokio::test]
    async fn platform_hourly_rate_filters_by_platform() {
        let db = test_db().await;
        let now_ms = now();
        // platform 1：2h 前一条 est_cost=4.0；platform 2：另一条 est_cost=99（不应计入 p1）。
        let mut l1 = sample_log("r1", "g", now_ms - 2 * 3_600_000);
        l1.platform_id = 1;
        l1.est_cost = 4.0;
        let mut l2 = sample_log("r2", "g", now_ms - 1_000);
        l2.platform_id = 2;
        l2.est_cost = 99.0;
        upsert_proxy_log(&db, &l1).await.unwrap();
        upsert_proxy_log(&db, &l2).await.unwrap();

        // p1：span = clamp(now_internal - earliest, 5min, 7d) ≈ 2h → rate ≈ 4.0 / 2 = 2.0 $/h。
        // 容差放宽：查询内部 now 与测试 now 间有毫秒级时钟差 → span 略大于 2h（rate 略小于 2.0）。
        let rate = get_platform_hourly_rate(&db, 1).await.unwrap();
        assert!(rate.is_some());
        assert!((rate.unwrap() - 2.0).abs() < 0.01, "p1 rate = {rate:?}");

        // 无任何用量的平台 → None。
        let none = get_platform_hourly_rate(&db, 999).await.unwrap();
        assert!(none.is_none(), "无用量平台应 None，got {none:?}");
    }

    // ── R2 单数表名 + "group" 转义：init_tables 成功间接验证 DDL ──
    #[tokio::test]
    async fn r2_singular_table_names_and_group_escaped() {
        // init_tables() 已在 test_db 中执行；进一步断言单数表名存在、复数不存在
        let db = test_db().await;
        let names: Vec<String> = db.0.call(|conn| {
            Ok(conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?
                .query_map([], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect())
        }).await.unwrap();
        assert!(names.contains(&"platform".to_string()));
        assert!(names.contains(&"group".to_string()));
        assert!(names.contains(&"group_platform".to_string()));
        assert!(names.contains(&"setting".to_string()));
        assert!(names.contains(&"proxy_log".to_string()));
        // 复数旧表名禁止存在
        assert!(!names.contains(&"platforms".to_string()));
        assert!(!names.contains(&"groups".to_string()));
        assert!(!names.contains(&"model_mappings".to_string()));
    }

    // ── R7 / D1 主键自增 uint64 ──
    #[tokio::test]
    async fn r7_platform_pk_autoincrement_u64() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("p1")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("p2")).await.unwrap();
        assert!(p1.id >= 1, "first id should be >= 1, got {}", p1.id);
        assert_eq!(p2.id, p1.id + 1, "id should auto-increment");
        // 类型为 u64（编译期保证），运行期断言 >0
        let _: u64 = p2.id;
        assert!(p2.id > 0);
    }

    // ── R1 / R9 毫秒级时间戳 ──
    #[tokio::test]
    async fn r1_timestamps_are_millis() {
        let db = test_db().await;
        let before = chrono::Utc::now().timestamp_millis();
        let p = create_platform(&db, sample_platform("ts")).await.unwrap();
        let after = chrono::Utc::now().timestamp_millis();
        // 毫秒级：> 1e12（2001 年之后），且落在 before..=after 区间
        assert!(p.created_at > 1_000_000_000_000, "created_at not ms-level: {}", p.created_at);
        assert!(p.updated_at > 1_000_000_000_000, "updated_at not ms-level: {}", p.updated_at);
        assert!(p.created_at >= before && p.created_at <= after);
        assert_eq!(p.created_at, p.updated_at);
    }

    // ── R9 软删除：delete 后 deleted_at>0；list 不含；get 返回 None ──
    #[tokio::test]
    async fn r9_soft_delete_platform() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("del")).await.unwrap();
        assert_eq!(list_platforms(&db).await.unwrap().len(), 1);

        delete_platform(&db, p.id).await.unwrap();

        // list 不返回已删行
        assert_eq!(list_platforms(&db).await.unwrap().len(), 0);
        // get 返回 None
        assert!(get_platform(&db, p.id).await.unwrap().is_none());

        // 行仍存在且 deleted_at > 0（物理保留）
        let pid = p.id as i64;
        let deleted_at: i64 = db.0.call(move |conn| {
            Ok(conn.query_row("SELECT deleted_at FROM platform WHERE id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert!(deleted_at > 0, "deleted_at should be set, got {deleted_at}");
    }

    // ── R10 禁 NULL：未提供 extra 时为空串而非 NULL ──
    #[tokio::test]
    async fn r10_no_null_defaults() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("nn")).await.unwrap();
        assert_eq!(p.extra, "");

        let g = create_group(&db, sample_group("g", "/g", vec![])).await.unwrap();
        assert_eq!(g.auto_from_platform, "");
        assert_eq!(g.model_mappings.len(), 0);

        // 直接断言列值非 NULL
        let (null_count, g_null): (i64, i64) = db.0.call(|conn| {
            let null_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM platform WHERE extra IS NULL OR base_url IS NULL OR api_key IS NULL",
                [],
                |r| r.get(0),
            )?;
            let g_null: i64 = conn.query_row(
                "SELECT COUNT(*) FROM \"group\" WHERE auto_from_platform IS NULL OR model_mappings IS NULL OR source_protocol IS NULL",
                [],
                |r| r.get(0),
            )?;
            Ok((null_count, g_null))
        }).await.unwrap();
        assert_eq!(null_count, 0, "no platform column should be NULL");
        assert_eq!(g_null, 0, "no group column should be NULL");
    }

    // ── R3 platform_type 列（protocol 改名）往返 ──
    #[tokio::test]
    async fn r3_platform_type_roundtrip() {
        let db = test_db().await;
        let mut input = sample_platform("pt");
        input.platform_type = Protocol::Glm;
        let p = create_platform(&db, input).await.unwrap();
        let fetched = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(fetched.platform_type, Protocol::Glm);
        // 列名为 platform_type（间接：能写入该列即证明列存在）
        let pid = p.id as i64;
        let stored: String = db.0.call(move |conn| {
            Ok(conn.query_row("SELECT platform_type FROM platform WHERE id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(stored, "\"glm\"");
    }

    // ── R4 / D4 model_mappings 内联 JSON 往返 ──
    #[tokio::test]
    async fn r4_group_model_mappings_inline_roundtrip() {
        let db = test_db().await;
        let mappings = vec![
            ModelMapping {
                source_model: "claude-sonnet-4".to_string(),
                target_platform_id: 42,
                target_model: "glm-4-plus".to_string(),
                request_timeout_secs: 30,
                connect_timeout_secs: 5,
            },
            ModelMapping {
                source_model: "claude-haiku".to_string(),
                target_platform_id: 7,
                target_model: "glm-4-air".to_string(),
                request_timeout_secs: 0,
                connect_timeout_secs: 0,
            },
        ];
        let g = create_group(&db, sample_group("mm", "/mm", mappings)).await.unwrap();

        let fetched = get_group(&db, g.id).await.unwrap().unwrap();
        assert_eq!(fetched.model_mappings.len(), 2);
        assert_eq!(fetched.model_mappings[0].source_model, "claude-sonnet-4");
        // target_platform_id 为 u64
        let tpid: u64 = fetched.model_mappings[0].target_platform_id;
        assert_eq!(tpid, 42);
        assert_eq!(fetched.model_mappings[0].target_model, "glm-4-plus");
        assert_eq!(fetched.model_mappings[0].request_timeout_secs, 30);
        assert_eq!(fetched.model_mappings[1].target_platform_id, 7);
    }

    // ── R4 model_mappings 来自 group 字段（get_group_detail）──
    #[tokio::test]
    async fn r4_group_detail_mappings_from_group_field() {
        let db = test_db().await;
        let mappings = vec![ModelMapping {
            source_model: "src".to_string(),
            target_platform_id: 3,
            target_model: "tgt".to_string(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
        }];
        let g = create_group(&db, sample_group("d", "/d", mappings)).await.unwrap();
        // 该分组无关联平台 → get_group_platforms join 为空，规避遗留 BUG-1（见任务遗留）
        let detail = get_group_detail(&db, g.id).await.unwrap().unwrap();
        // detail.model_mappings 来自 group 内联字段（逐字段一致）
        assert_eq!(detail.model_mappings.len(), 1);
        assert_eq!(detail.model_mappings.len(), detail.group.model_mappings.len());
        assert_eq!(detail.model_mappings[0].source_model, detail.group.model_mappings[0].source_model);
        assert_eq!(detail.model_mappings[0].target_platform_id, detail.group.model_mappings[0].target_platform_id);
        assert_eq!(detail.model_mappings[0].target_model, detail.group.model_mappings[0].target_model);
    }

    // ── R8 proxy_log 主键 TEXT hex32（无连字符），软删 + retention ──
    #[tokio::test]
    async fn r8_proxy_log_uuid_no_hyphen_and_retention() {
        let db = test_db().await;
        // hex32 无连字符 id（模拟生产生成方式 uuid simple）
        let new_id = uuid::Uuid::new_v4().simple().to_string();
        assert_eq!(new_id.len(), 32, "simple uuid should be 32 hex chars");
        assert!(!new_id.contains('-'), "uuid must not contain hyphen");

        let now_ms = chrono::Utc::now().timestamp_millis();
        // 一条最近日志
        upsert_proxy_log(&db, &sample_log(&new_id, "g", now_ms)).await.unwrap();
        // 一条很旧的日志（100 天前）
        let old_id = uuid::Uuid::new_v4().simple().to_string();
        let old_ms = now_ms - 100 * 86_400_000;
        upsert_proxy_log(&db, &sample_log(&old_id, "g", old_ms)).await.unwrap();

        assert_eq!(count_proxy_logs(&db).await.unwrap(), 2);

        // retention 30 天：旧日志被软删
        cleanup_proxy_logs(&db, 30).await.unwrap();
        assert_eq!(count_proxy_logs(&db).await.unwrap(), 1);
        assert!(get_proxy_log(&db, &old_id).await.unwrap().is_none());
        assert!(get_proxy_log(&db, &new_id).await.unwrap().is_some());

        // proxy_log 主键存储原样 TEXT
        let fetched = get_proxy_log(&db, &new_id).await.unwrap().unwrap();
        assert_eq!(fetched.id, new_id);
        assert!(fetched.created_at > 1_000_000_000_000);
    }

    // ── D3 复合唯一约束：group_platform 加代理主键 + UNIQUE(group_id, platform_id) ──
    #[tokio::test]
    async fn d3_group_platform_proxy_pk_and_unique() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("a")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("b")).await.unwrap();
        let g = create_group(&db, sample_group("g", "/g", vec![])).await.unwrap();

        set_group_platforms(
            &db,
            g.id,
            &[
                GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1) },
                GroupPlatformInput { platform_id: p2.id, priority: Some(1), weight: Some(2) },
            ],
        ).await
        .unwrap();

        let details = get_group_platforms(&db, g.id).await.unwrap();
        assert_eq!(details.len(), 2);

        // 代理主键 id 存在且自增
        let ids: Vec<i64> = db.0.call(|conn| {
            Ok(conn
                .prepare("SELECT id FROM group_platform ORDER BY id")?
                .query_map([], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect())
        }).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids[0] >= 1 && ids[1] > ids[0]);
    }

    // ── setting 软删除 + upsert ──
    #[tokio::test]
    async fn setting_upsert_and_soft_delete() {
        let db = test_db().await;
        set_setting(&db, SetSettingInput {
            scope: "proxy".to_string(),
            key: "logging".to_string(),
            value: serde_json::json!({"enabled": true}),
        }).await.unwrap();
        assert_eq!(list_setting_keys(&db, "proxy").await.unwrap(), vec!["logging".to_string()]);
        let v = get_setting(&db, "proxy", "logging").await.unwrap().unwrap();
        assert_eq!(v["enabled"], serde_json::json!(true));

        delete_setting(&db, "proxy", "logging").await.unwrap();
        assert!(get_setting(&db, "proxy", "logging").await.unwrap().is_none());
        assert_eq!(list_setting_keys(&db, "proxy").await.unwrap().len(), 0);
    }

    // ─── Tray Config ───────────────────────────────────────

    /// TrayConfig serde 往返：写入后读回各字段一致（separator/items 颜色三态/字号/line_mode/排序）。
    #[tokio::test]
    async fn tray_config_serde_roundtrip() {
        let db = test_db().await;
        let cfg = TrayConfig {
            separator: " | ".to_string(),
            items: vec![
                TrayItem {
                    item_type: "platform".to_string(),
                    platform_id: Some(7),
                    display: "coding".to_string(),
                    metric: None,
                    label: None,
decimals: None,
                    color: TrayColor { mode: "preset".to_string(), value: "green".to_string() },
                    font_size: 11.0,
                    line_mode: "two".to_string(),
                    align: "left".to_string(),
                    align_row2: None,
                    enabled: true,
                    order: 0,
                },
                TrayItem {
                    item_type: "today_usage".to_string(),
                    platform_id: None,
                    display: "balance".to_string(),
                    metric: Some("tokens".to_string()),
                    label: None,
decimals: None,
                    color: TrayColor { mode: "custom".to_string(), value: "#ff8800".to_string() },
                    font_size: 9.0,
                    line_mode: "single".to_string(),
                    align: "left".to_string(),
                    align_row2: None,
                    enabled: false,
                    order: 1,
                },
            ],
        };
        set_tray_config(&db, &cfg).await.unwrap();
        let got = get_tray_config(&db).await.unwrap().expect("config present");
        assert_eq!(got.separator, " | ");
        assert_eq!(got.items.len(), 2);
        assert_eq!(got.items[0].item_type, "platform");
        assert_eq!(got.items[0].platform_id, Some(7));
        assert_eq!(got.items[0].display, "coding");
        assert_eq!(got.items[0].color.mode, "preset");
        assert_eq!(got.items[0].color.value, "green");
        assert!((got.items[0].font_size - 11.0).abs() < 1e-9);
        assert_eq!(got.items[0].line_mode, "two");
        assert!(got.items[0].enabled);
        assert_eq!(got.items[1].line_mode, "single");
        assert_eq!(got.items[1].item_type, "today_usage");
        assert_eq!(got.items[1].metric.as_deref(), Some("tokens"));
        assert_eq!(got.items[1].color.mode, "custom");
        assert_eq!(got.items[1].color.value, "#ff8800");
        assert!(!got.items[1].enabled);
        assert_eq!(got.items[1].order, 1);
    }

    /// 迁移：无 tray config 且无旧 show_in_tray 平台 → 生成空配置并持久化（避免重复迁移）。
    #[tokio::test]
    async fn tray_config_migrate_empty() {
        let db = test_db().await;
        // 首次读取触发迁移；无旧平台 → 空 items。
        let cfg = get_tray_config(&db).await.unwrap().expect("migrated config");
        assert_eq!(cfg.items.len(), 0);
        // 已持久化：settings 中应存在 tray/config。
        assert!(get_setting(&db, "tray", "config").await.unwrap().is_some());
    }

    /// 迁移：旧 show_in_tray=1 平台 → 生成默认 platform item（保留 tray_display）。
    #[tokio::test]
    async fn tray_config_migrate_from_legacy_platform() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("legacy")).await.unwrap();
        set_tray_platform(&db, p.id, "coding").await.unwrap();

        let cfg = get_tray_config(&db).await.unwrap().expect("migrated config");
        assert_eq!(cfg.items.len(), 1, "应从旧平台生成 1 个 platform item");
        let item = &cfg.items[0];
        assert_eq!(item.item_type, "platform");
        assert_eq!(item.platform_id, Some(p.id));
        assert_eq!(item.display, "coding");
        assert!(item.enabled);
    }

    /// 迁移：旧全局 layout=two_line → 各 item line_mode="two"；缺 line_mode 字段时按 serde default "single"。
    #[tokio::test]
    async fn tray_config_migrate_legacy_layout() {
        let db = test_db().await;
        // 模拟旧版本写入：含全局 layout 字段，item 无 line_mode 字段。
        let legacy = serde_json::json!({
            "layout": "two_line",
            "separator": "  ",
            "items": [
                { "item_type": "platform", "platform_id": 3, "display": "balance",
                  "color": { "mode": "follow", "value": "" }, "font_size": 9.0,
                  "enabled": true, "order": 0 }
            ]
        });
        set_setting(&db, SetSettingInput {
            scope: "tray".to_string(),
            key: "config".to_string(),
            value: legacy,
        }).await.unwrap();

        let cfg = get_tray_config(&db).await.unwrap().expect("config present");
        assert_eq!(cfg.items.len(), 1);
        // 旧全局 two_line → item line_mode="two"。
        assert_eq!(cfg.items[0].line_mode, "two");
    }

    /// serde default：缺 line_mode 字段 → "two"（default_line_mode）。
    #[tokio::test]
    async fn tray_item_line_mode_serde_default() {
        let raw = serde_json::json!({
            "item_type": "platform", "platform_id": 1, "display": "balance",
            "color": { "mode": "follow", "value": "" }, "font_size": 9.0,
            "enabled": true, "order": 0
        });
        let item: TrayItem = serde_json::from_value(raw).unwrap();
        assert_eq!(item.line_mode, "two");
    }

    /// today_token_total：仅统计今日（本地 0 点起）未删除日志的 input+output。
    #[tokio::test]
    async fn today_token_total_sums_today_only() {
        use chrono::{Local, Duration};
        let db = test_db().await;
        let now_ms = now();
        // 今日两条：(10+20) + (10+20) = 60
        upsert_proxy_log(&db, &sample_log("a", "g", now_ms)).await.unwrap();
        upsert_proxy_log(&db, &sample_log("b", "g", now_ms)).await.unwrap();
        // 昨日一条：不计入。
        let yesterday_ms = (Local::now() - Duration::days(1)).timestamp_millis();
        upsert_proxy_log(&db, &sample_log("c", "g", yesterday_ms)).await.unwrap();

        assert_eq!(today_token_total(&db).await.unwrap(), 60);
    }

    // ── S1 async DB：增删改查全路径（内存库，验证 tokio-rusqlite 闭包往返）──
    #[tokio::test]
    async fn s1_async_platform_crud_roundtrip() {
        let db = test_db().await;
        // create
        let mut input = sample_platform("crud");
        input.base_url = "https://crud.example/v1".to_string();
        let created = create_platform(&db, input).await.unwrap();
        assert!(created.id >= 1);

        // read (list + get)
        assert_eq!(list_platforms(&db).await.unwrap().len(), 1);
        let got = get_platform(&db, created.id).await.unwrap().unwrap();
        assert_eq!(got.base_url, "https://crud.example/v1");

        // update
        let updated = update_platform(&db, UpdatePlatform {
            id: created.id,
            name: None,
            platform_type: None,
            base_url: Some("https://crud.example/v2".to_string()),
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: None,
            manual_budgets: None,
        }).await.unwrap();
        assert_eq!(updated.base_url, "https://crud.example/v2");
        assert_eq!(get_platform(&db, created.id).await.unwrap().unwrap().base_url, "https://crud.example/v2");

        // delete（软删）→ list 不含、get None
        delete_platform(&db, created.id).await.unwrap();
        assert_eq!(list_platforms(&db).await.unwrap().len(), 0);
        assert!(get_platform(&db, created.id).await.unwrap().is_none());
    }

    /// 401/403 自动禁用：状态变 auto_disabled，strikes 递增，退避指数 1h/2h/4h。
    #[tokio::test]
    async fn auto_disable_exponential_backoff() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("ad")).await.unwrap();
        assert_eq!(p.status, PlatformStatus::Enabled);

        let base = 60 * 60 * 1000i64;
        // 第 1 次：strikes=1, 退避 1h
        let t0 = now();
        let until1 = set_platform_auto_disabled(&db, p.id).await.unwrap();
        let g1 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g1.status, PlatformStatus::AutoDisabled);
        assert!(!g1.enabled, "auto_disabled 平台 enabled 列同步为 false");
        assert_eq!(g1.auto_disable_strikes, 1);
        assert!(until1 >= t0 + base && until1 <= now() + base + 1000, "first backoff ~1h");

        // 第 2 次：strikes=2, 退避 2h
        set_platform_auto_disabled(&db, p.id).await.unwrap();
        let g2 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g2.auto_disable_strikes, 2);
        assert!(g2.auto_disabled_until - now() >= 2 * base - 2000, "second backoff ~2h");

        // 第 3 次：strikes=3, 退避 4h
        set_platform_auto_disabled(&db, p.id).await.unwrap();
        let g3 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g3.auto_disable_strikes, 3);
        assert!(g3.auto_disabled_until - now() >= 4 * base - 2000, "third backoff ~4h");

        // 2xx 恢复：清状态
        recover_platform_auto_disabled(&db, p.id).await.unwrap();
        let g4 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g4.status, PlatformStatus::Enabled);
        assert!(g4.enabled);
        assert_eq!(g4.auto_disable_strikes, 0);
        assert_eq!(g4.auto_disabled_until, 0);
    }

    /// 用户手动 disabled 平台不受 401/403 自动禁用影响（区分手动 vs 自动）。
    #[tokio::test]
    async fn auto_disable_skips_user_disabled() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("ud")).await.unwrap();
        // 用户手动禁用
        let upd = update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: Some(PlatformStatus::Disabled), manual_budgets: None,
        }).await.unwrap();
        assert_eq!(upd.status, PlatformStatus::Disabled);
        assert!(!upd.enabled);

        // 401/403 触发不应改成 auto_disabled
        let until = set_platform_auto_disabled(&db, p.id).await.unwrap();
        assert_eq!(until, 0, "user-disabled 平台不进入退避");
        let g = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g.status, PlatformStatus::Disabled, "保持用户手动禁用");
    }

    /// 改 api_key 自恢复：auto_disabled 平台改 api_key → 立即恢复 enabled 清退避。
    #[tokio::test]
    async fn api_key_change_recovers_auto_disabled() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("rk")).await.unwrap();
        set_platform_auto_disabled(&db, p.id).await.unwrap();
        assert_eq!(get_platform(&db, p.id).await.unwrap().unwrap().status, PlatformStatus::AutoDisabled);

        // 改 api_key（不显式传 status）
        let upd = update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None,
            api_key: Some("sk-new-key".to_string()),
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: None, manual_budgets: None,
        }).await.unwrap();
        assert_eq!(upd.status, PlatformStatus::Enabled, "改 api_key 立即恢复");
        assert_eq!(upd.auto_disable_strikes, 0);
        assert_eq!(upd.auto_disabled_until, 0);
    }

    /// group max_retries 持久化往返
    #[tokio::test]
    async fn group_max_retries_roundtrip() {
        let db = test_db().await;
        let mut input = sample_group("mr", "/mr", vec![]);
        input.max_retries = 5;
        let g = create_group(&db, input).await.unwrap();
        assert_eq!(g.max_retries, 5);
        let fetched = get_group(&db, g.id).await.unwrap().unwrap();
        assert_eq!(fetched.max_retries, 5);

        let upd = update_group(&db, UpdateGroup {
            id: g.id, name: None, path: None, routing_mode: None,
            request_timeout_secs: 0, connect_timeout_secs: 0, source_protocol: None,
            max_retries: Some(0), model_mappings: vec![],
        }).await.unwrap();
        assert_eq!(upd.max_retries, 0);
        assert_eq!(get_group(&db, g.id).await.unwrap().unwrap().max_retries, 0);
    }

    /// proxy_log attempts JSON 列往返
    #[tokio::test]
    async fn proxy_log_attempts_roundtrip() {
        let db = test_db().await;
        let mut log = sample_log("attlog", "g", now());
        log.attempts = vec![
            super::super::models::ProxyAttempt {
                platform_id: 1, platform_name: "p1".into(), status_code: 503,
                error: "boom".into(), duration_ms: 12, ts: now(),
            },
            super::super::models::ProxyAttempt {
                platform_id: 2, platform_name: "p2".into(), status_code: 200,
                error: String::new(), duration_ms: 34, ts: now(),
            },
        ];
        log.retry_count = 1;
        upsert_proxy_log(&db, &log).await.unwrap();
        let fetched = get_proxy_log(&db, "attlog").await.unwrap().unwrap();
        assert_eq!(fetched.attempts.len(), 2);
        assert_eq!(fetched.attempts[0].status_code, 503);
        assert_eq!(fetched.attempts[1].platform_name, "p2");
        assert_eq!(fetched.retry_count, 1);
    }

    // ── S1 async DB：OptionalExtension 路径（query_row().optional() 在闭包内）──
    #[tokio::test]
    async fn s1_async_optional_extension_returns_none_for_missing() {
        let db = test_db().await;
        // 不存在的 id → get_platform 走 query_row().optional() 返回 None（非 Err）
        assert!(get_platform(&db, 99_999).await.unwrap().is_none());
        // 存在则返回 Some
        let p = create_platform(&db, sample_platform("opt")).await.unwrap();
        assert!(get_platform(&db, p.id).await.unwrap().is_some());
        // get_setting 同样走 optional()：缺键 None
        assert!(get_setting(&db, "nope", "nope").await.unwrap().is_none());
    }
}
