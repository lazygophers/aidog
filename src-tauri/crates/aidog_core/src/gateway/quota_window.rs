//! 额度窗口执行（`platform.extra.quota_windows`：token / 金额 $ / 次数预算 + 三种重置机制）。
//!
//! 三个动作（与 [`super::rate_limit`] 同构：候选筛选只读 + 转发前占位原子）：
//! - **候选筛选**（只读）：[`platform_exhausted_at`]，耗尽平台从候选排除（RouteReason::QuotaWindowExhausted），
//!   临时性维度——窗口过后自动恢复，不改 DB status。
//! - **转发前占位**（原子）：[`reserve_attempt`]，platform.db 单写连接临界区内
//!   惰性窗口重置 → 耗尽判定 → 次数窗口 +1，检查与计数同临界区（并发不超限的硬保证）。
//! - **用量记账**：[`record_usage`]，token / 金额窗口按成功响应的用量累加（est_cost 走
//!   resolve_price 网关计费链，与 manual_budget 同口径）。
//!
//! 用量持久化在 `quota_window_usage` 表（platform.db，重启不清零）；窗口配置留在 extra JSON。
//! 计数口径（2026-11-16 拍板）：token 窗口计 cache（读+写）在内的总 token；金额窗口按
//! est_cost；次数窗口按上游实际尝试次数（含路由重试换模型，forward_attempt 级）。
//! 执行范围照限频占位先例：聊天 + 向量化/重排序；决策 / 搜索转发豁免（不计入、不阻断）。

use std::collections::HashMap;

use chrono::{Local, TimeZone, Timelike};

use aidog_db::models::{parse_quota_windows, ExcludeScope, QuotaBudget, QuotaResetMode, QuotaWindow};
use aidog_db::Db;

/// 阻断原因（route fail 字符串 / blocked_reason / AttemptLoop 终态共用）。
pub(crate) const QUOTA_BLOCKED_REASON: &str = "quota_window_exhausted";

/// 一条窗口的用量状态（quota_window_usage 行；无行 = 该窗口未用过）。
#[derive(Debug, Clone)]
pub struct QuotaUsage {
    pub window_key: String,
    pub window_start_at: i64,
    pub consumed: f64,
}

/// 窗口起点归一化（纯函数）：首用（start<=0）初始化起点不清零；到期 / 跨段 → 重置清零。
/// 返回 (生效起点 ms, 是否清零 consumed)。
/// - sliding：起点 = 本窗口第一笔请求时刻；now-start >= win_ms → 重置，起点 now。
/// - calendar_day：本地自然日 00:00；跨日 → 重置。
/// - calendar_hour：UTC 自然小时整点；跨小时 → 重置。
pub fn effective_start(mode: QuotaResetMode, win_sec: f64, now_ms: i64, start: i64) -> (i64, bool) {
    match mode {
        QuotaResetMode::Sliding => {
            if start <= 0 {
                return (now_ms, false);
            }
            let win_ms = (win_sec * 1000.0) as i64;
            if win_ms > 0 && now_ms - start >= win_ms {
                (now_ms, true)
            } else {
                (start, false)
            }
        }
        QuotaResetMode::CalendarDay => {
            let day_start = day_start_ms(now_ms);
            if start <= 0 || day_start != start {
                (day_start, true)
            } else {
                (start, false)
            }
        }
        QuotaResetMode::CalendarHour => {
            let hour_start = now_ms - (now_ms.rem_euclid(3_600_000));
            if start <= 0 || hour_start != start {
                (hour_start, true)
            } else {
                (start, false)
            }
        }
    }
}

/// 包含 now_ms 的本地自然日 00:00 毫秒戳。
fn day_start_ms(now_ms: i64) -> i64 {
    Local
        .timestamp_millis_opt(now_ms)
        .single()
        .map(|dt| {
            dt.with_hour(0)
                .and_then(|d| d.with_minute(0))
                .and_then(|d| d.with_second(0))
                .and_then(|d| d.with_nanosecond(0))
                .unwrap_or(dt)
                .timestamp_millis()
        })
        .unwrap_or(now_ms)
}

/// 窗口结束时刻（Retry-After 锚点 =「当前命中窗口的剩余秒数」）：
/// sliding = 起点 + 窗口时长；calendar_day = 次一本地午夜；calendar_hour = 次一整点。
/// 下限 now+1s（时钟回拨兜底）。
fn window_end(mode: QuotaResetMode, win_sec: f64, start: i64, now_ms: i64) -> i64 {
    let end = match mode {
        QuotaResetMode::Sliding => start + (win_sec * 1000.0) as i64,
        QuotaResetMode::CalendarDay => day_start_ms(now_ms) + 86_400_000,
        QuotaResetMode::CalendarHour => now_ms - now_ms.rem_euclid(3_600_000) + 3_600_000,
    };
    end.max(now_ms + 1000)
}

/// 单窗口耗尽判定（纯函数，惰性重置作用于克隆语义）：已耗尽 → Some(窗口结束时刻)。
pub fn exhausted_at(win: &QuotaWindow, usage: Option<&QuotaUsage>, now_ms: i64) -> Option<i64> {
    if !win.enabled {
        return None;
    }
    let budget = win.budget()?;
    let (start, reset) = {
        let s = usage.map(|u| u.window_start_at).unwrap_or(0);
        effective_start(win.reset_mode, win.window, now_ms, s)
    };
    let consumed = if reset {
        0.0
    } else {
        usage.map(|u| u.consumed).unwrap_or(0.0)
    };
    let limit = match budget {
        QuotaBudget::Tokens(v) | QuotaBudget::Usd(v) | QuotaBudget::Count(v) => v,
    };
    if consumed >= limit {
        return Some(window_end(win.reset_mode, win.window, start, now_ms));
    }
    None
}

/// 平台级判定（只读）：任一 enabled 窗口耗尽 → 最早的窗口结束时刻（None = 放行）。
pub fn platform_exhausted_at(
    windows: &[QuotaWindow],
    usage: &[QuotaUsage],
    now_ms: i64,
) -> Option<i64> {
    windows
        .iter()
        .filter_map(|w| {
            let key = w.budget().map(|b| b.key(w.window));
            let u = usage.iter().find(|u| Some(&u.window_key) == key.as_ref());
            exhausted_at(w, u, now_ms)
        })
        .min()
}

/// 批量读用量（读池，候选筛选用）：platform_id → 用量行。无行的平台不出现在 map 里。
pub async fn load_usage(db: &Db, platform_ids: &[u64]) -> HashMap<u64, Vec<QuotaUsage>> {
    if platform_ids.is_empty() {
        return HashMap::new();
    }
    let ids: Vec<i64> = platform_ids.iter().map(|p| *p as i64).collect();
    let caller = std::panic::Location::caller();
    let rows: Vec<(i64, String, i64, f64)> = db
        .call_read_platform_traced(None, caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT platform_id, window_key, window_start_at, consumed FROM quota_window_usage WHERE platform_id IN (SELECT value FROM json_each(?1))",
            )?;
            let json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
            let mut rows = Vec::new();
            let mut q = stmt.query(rusqlite::params![json])?;
            while let Some(r) = q.next()? {
                rows.push((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?));
            }
            Ok(rows)
        })
        .await
        .unwrap_or_default();
    let mut out: HashMap<u64, Vec<QuotaUsage>> = HashMap::new();
    for (pid, key, start, consumed) in rows {
        out.entry(pid as u64).or_default().push(QuotaUsage {
            window_key: key,
            window_start_at: start,
            consumed,
        });
    }
    out
}

/// 转发前占位结果。
#[derive(Debug, PartialEq)]
pub enum Reserve {
    /// 放行（次数窗口已在临界区内 +1）
    Allowed,
    /// 超限：retry_at_ms = 命中窗口的结束时刻（Retry-After 锚点）
    Blocked { retry_at_ms: i64 },
}

/// 转发前占位（原子）：platform.db 单写连接临界区内完成 惰性重置 → 耗尽判定 →
/// 次数窗口 +1 → 回写。并发请求由单写连接串行化（检查与计数同临界区，不超限）。
pub async fn reserve_attempt(
    db: &Db,
    extra: &str,
    platform_id: u64,
    now_ms: i64,
) -> Result<Reserve, String> {
    let windows: Vec<QuotaWindow> = parse_quota_windows(extra)
        .into_iter()
        .filter(|w| w.enabled)
        .collect();
    if windows.is_empty() {
        return Ok(Reserve::Allowed);
    }
    db.platform_write_conn()
        .call(move |conn| -> rusqlite::Result<Reserve> {
            let rows = load_rows(conn, platform_id as i64)?;
            let mut dirty: Vec<(String, i64, f64)> = Vec::new();
            for w in &windows {
                let Some(budget) = w.budget() else { continue };
                let key = budget.key(w.window);
                let (start, consumed) = match rows.iter().find(|r| r.window_key == key) {
                    Some(r) => {
                        let (s, reset) = effective_start(w.reset_mode, w.window, now_ms, r.window_start_at);
                        (s, if reset { 0.0 } else { r.consumed })
                    }
                    None => {
                        let (s, _) = effective_start(w.reset_mode, w.window, now_ms, 0);
                        (s, 0.0)
                    }
                };
                let limit = match budget {
                    QuotaBudget::Tokens(v) | QuotaBudget::Usd(v) | QuotaBudget::Count(v) => v,
                };
                if consumed >= limit {
                    return Ok(Reserve::Blocked { retry_at_ms: window_end(w.reset_mode, w.window, start, now_ms) });
                }
                let new_consumed = match budget {
                    QuotaBudget::Count(_) => consumed + 1.0,
                    _ => consumed, // token / 金额窗口不预占，成功后 record_usage 结算
                };
                dirty.push((key.clone(), start, new_consumed));
            }
            for (key, start, consumed) in dirty {
                upsert_row(conn, platform_id as i64, &key, start, consumed)?;
            }
            Ok(Reserve::Allowed)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 成功响应后的用量结算（原子 RMW）：token 窗口 += 总 token（含 cache 读/写）、
/// 金额窗口 += est_cost；次数窗口不经此（reserve_attempt 已按尝试计数）。
pub async fn record_usage(
    db: &Db,
    platform_id: u64,
    extra: &str,
    total_tokens: f64,
    est_cost: f64,
    now_ms: i64,
) -> Result<(), String> {
    let mut deltas: Vec<(String, QuotaResetMode, f64, f64)> = Vec::new();
    for w in parse_quota_windows(extra).into_iter().filter(|w| w.enabled) {
        let Some(b) = w.budget() else { continue };
        let d = match b {
            QuotaBudget::Tokens(_) => total_tokens,
            QuotaBudget::Usd(_) => est_cost,
            QuotaBudget::Count(_) => continue,
        };
        if d > 0.0 {
            deltas.push((b.key(w.window), w.reset_mode, w.window, d));
        }
    }
    if deltas.is_empty() {
        return Ok(());
    }
    db.platform_write_conn()
        .call(move |conn| -> rusqlite::Result<()> {
            let rows = load_rows(conn, platform_id as i64)?;
            for (key, mode, win_sec, delta) in deltas {
                let start = rows
                    .iter()
                    .find(|r| r.window_key == key)
                    .map(|r| r.window_start_at)
                    .unwrap_or(0);
                let (start, reset) = effective_start(mode, win_sec, now_ms, start);
                let consumed = rows
                    .iter()
                    .find(|r| r.window_key == key)
                    .map(|r| r.consumed)
                    .unwrap_or(0.0);
                let (start, consumed) = if reset { (start, 0.0) } else { (start, consumed) };
                upsert_row(conn, platform_id as i64, &key, start, consumed + delta)?;
            }
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
}

fn load_rows(conn: &rusqlite::Connection, pid: i64) -> rusqlite::Result<Vec<QuotaUsage>> {
    let mut stmt = conn.prepare(
        "SELECT window_key, window_start_at, consumed FROM quota_window_usage WHERE platform_id = ?1",
    )?;
    let mut rows = Vec::new();
    let mut q = stmt.query(rusqlite::params![pid])?;
    while let Some(r) = q.next()? {
        rows.push(QuotaUsage {
            window_key: r.get(0)?,
            window_start_at: r.get(1)?,
            consumed: r.get(2)?,
        });
    }
    Ok(rows)
}

fn upsert_row(
    conn: &rusqlite::Connection,
    pid: i64,
    key: &str,
    start: i64,
    consumed: f64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO quota_window_usage (platform_id, window_key, window_start_at, consumed) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(platform_id, window_key) DO UPDATE SET window_start_at = ?3, consumed = ?4",
        rusqlite::params![pid, key, start, consumed],
    )?;
    Ok(())
}

/// 候选筛选用：从分组平台 extra 里判断平台是否有任何 enabled 窗口（无 → 跳过 load_usage）。
pub fn has_enabled_windows(extra: &str) -> bool {
    parse_quota_windows(extra).iter().any(|w| w.enabled)
}

/// 转发前的额度窗口闸门（聊天 forward_attempt 与 aux 循环共用，限频占位同位）：
/// 放行 true；超限 false（不发上游请求、不计调用次数），副作用 = attempt 留痕
/// （kind=quota_window_platform，排除整个平台）+ blocked_by='router' + Retry-After 锚点。
/// 透传平台豁免（与候选筛选「透传永远在线」同口径）；读库失败放行（fail-open，同 manual_budget 忽略语义）。
pub(crate) async fn gate_attempt(
    db: &Db,
    log: &mut aidog_db::models::ProxyLog,
    attempts: &mut Vec<aidog_db::models::ProxyAttempt>,
    platform: &aidog_db::models::Platform,
    model: &str,
    model_alt: bool,
) -> bool {
    if matches!(platform.platform_type, aidog_db::models::Protocol::Passthrough) {
        return true;
    }
    if log.blocked_reason == QUOTA_BLOCKED_REASON {
        log.blocked_by.clear();
        log.blocked_reason.clear();
    }
    let now = aidog_db::now();
    match reserve_attempt(db, &platform.extra, platform.id, now).await {
        Ok(Reserve::Allowed) => true,
        Ok(Reserve::Blocked { retry_at_ms }) => {
            tracing::warn!(platform = %platform.name, retry_at_ms, "quota window exhausted at dispatch, skipping candidate");
            attempts.push(aidog_db::models::ProxyAttempt {
                kind: Some("quota_window_platform".to_string()),
                chain_stop: model_alt.then(|| "quota_window".to_string()),
                platform_id: platform.id,
                platform_name: platform.name.clone(),
                error: "quota window exhausted".to_string(),
                ts: now,
                model: Some(model.to_string()),
                exclusion: Some(ExcludeScope::Platform),
                ..Default::default()
            });
            log.blocked_by = "router".to_string();
            log.blocked_reason = QUOTA_BLOCKED_REASON.to_string();
            if let Some(r) = log.routing.as_mut() {
                r.note_retry_at_ms(retry_at_ms);
            }
            false
        }
        Err(e) => {
            tracing::warn!(error = %e, platform = %platform.name, "quota gate db error, fail-open");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(sec: f64, budget: QuotaBudget) -> QuotaWindow {
        let (tokens, usd, count) = match budget {
            QuotaBudget::Tokens(v) => (Some(v), None, None),
            QuotaBudget::Usd(v) => (None, Some(v), None),
            QuotaBudget::Count(v) => (None, None, Some(v)),
        };
        QuotaWindow {
            enabled: true,
            reset_mode: QuotaResetMode::Sliding,
            window: sec,
            budget_tokens: tokens,
            budget_usd: usd,
            budget_count: count,
            budget_points: None,
        }
    }

    const H: i64 = 3_600_000;
    const T0: i64 = 1_700_000_000_000;

    // ── sliding：首用起算，窗口内累计，到期重置 ──
    #[test]
    fn sliding_first_use_starts_window_and_resets_after() {
        let w = win(3600.0, QuotaBudget::Count(3.0));
        let u = |consumed: f64, start: i64| QuotaUsage { window_key: "3600|count|3".into(), window_start_at: start, consumed };
        // 无用量 → 放行
        assert_eq!(exhausted_at(&w, None, T0), None);
        // 窗口内 2/3 → 放行
        assert_eq!(exhausted_at(&w, Some(&u(2.0, T0)), T0 + H / 2), None);
        // 3/3 → 耗尽，Retry-After 锚点 = 起点 + 1h
        assert_eq!(exhausted_at(&w, Some(&u(3.0, T0)), T0 + H / 2), Some(T0 + H));
        // 到期（起点 +1h）后 → 重置放行
        assert_eq!(exhausted_at(&w, Some(&u(3.0, T0)), T0 + H), None, "到期瞬间重置");
    }

    #[test]
    fn sliding_first_use_without_row_starts_now() {
        // 首用（无行）：起点 = now，即「本窗口第一笔请求起算」
        assert_eq!(effective_start(QuotaResetMode::Sliding, 3600.0, T0, 0).0, T0);
    }

    // ── calendar_day：本地自然日跨日重置 ──
    #[test]
    fn calendar_day_resets_on_day_boundary() {
        let mut w = win(3600.0, QuotaBudget::Count(1.0));
        w.reset_mode = QuotaResetMode::CalendarDay;
        let day0 = day_start_ms(T0);
        let u = |consumed: f64, start: i64| QuotaUsage { window_key: "k".into(), window_start_at: start, consumed };
        // 同日耗尽 → 锚点 = 次一本地午夜（calendar 模式的自然重置点）
        assert_eq!(exhausted_at(&w, Some(&u(1.0, day0)), day0 + 2 * H), Some(day0 + 24 * H));
        // 次日 → 重置放行（起点滚动到新日 00:00）
        assert_eq!(exhausted_at(&w, Some(&u(1.0, day0)), day0 + 24 * H), None);
    }

    // ── calendar_hour：UTC 整点跨小时重置 ──
    #[test]
    fn calendar_hour_resets_on_hour_boundary() {
        let mut w = win(1800.0, QuotaBudget::Count(1.0));
        w.reset_mode = QuotaResetMode::CalendarHour;
        let hour0 = T0 - T0.rem_euclid(3_600_000);
        let u = |consumed: f64, start: i64| QuotaUsage { window_key: "k".into(), window_start_at: start, consumed };
        assert_eq!(exhausted_at(&w, Some(&u(1.0, hour0)), hour0 + H / 2), Some(hour0 + 3_600_000));
        // 下一整点 → 重置
        assert_eq!(exhausted_at(&w, Some(&u(1.0, hour0)), hour0 + H), None);
    }

    // ── 多窗口：任一耗尽即挡，锚点取最早 ──
    #[test]
    fn any_window_exhausted_blocks_earliest_wins() {
        let ws = vec![win(3600.0, QuotaBudget::Count(5.0)), win(60.0, QuotaBudget::Usd(10.0))];
        let u1 = QuotaUsage { window_key: "3600|count|5".into(), window_start_at: T0, consumed: 2.0 };
        let u2 = QuotaUsage { window_key: "60|usd|10".into(), window_start_at: T0, consumed: 10.0 };
        let at = platform_exhausted_at(&ws, &[u1.clone(), u2], T0 + 30_000).expect("usd 窗口已耗尽");
        assert_eq!(at, T0 + 60_000, "锚点 = 耗尽窗口的结束时刻");
        assert_eq!(platform_exhausted_at(&ws, &[u1], T0), None, "全部有余 → 放行");
    }

    // ── disabled 窗口不阻断 ──
    #[test]
    fn disabled_window_never_blocks() {
        let mut w = win(60.0, QuotaBudget::Count(1.0));
        w.enabled = false;
        let u = QuotaUsage { window_key: "60|count|1".into(), window_start_at: T0, consumed: 1.0 };
        assert_eq!(exhausted_at(&w, Some(&u), T0), None);
    }

    // ── token 窗口按总 token（含 cache）判定 ──
    #[test]
    fn token_window_counts_cache() {
        let w = win(3600.0, QuotaBudget::Tokens(100_000.0));
        let u = QuotaUsage { window_key: "3600|tokens|100000".into(), window_start_at: T0, consumed: 100_000.0 };
        assert_eq!(exhausted_at(&w, Some(&u), T0), Some(T0 + H), "token 口径含 cache：调用方按 input+output+cache+cache_write 累加");
    }

    // ── 解析兜底：无窗口配置的平台不进占位 ──
    #[test]
    fn parse_gate_no_config_short_circuits() {
        assert!(!has_enabled_windows("{}"));
        assert!(has_enabled_windows(r#"{"quota_windows":[{"window":60,"budget_count":1}]}"#));
        assert!(!has_enabled_windows(r#"{"quota_windows":[{"window":60,"budget_count":1,"enabled":false}]}"#));
    }

    // ── DB 集成（三文件库，platform.db 写池真分路由，db-pool-routing 规则）──

    async fn quota_db(extra: &str) -> (aidog_db::Db, tempfile::TempDir, u64) {
        use aidog_db::{create_platform, test_support::sample_platform};
        let (db, dir) = aidog_db::test_support::three_file_db().await;
        let mut cp = sample_platform("quota-win");
        cp.extra = extra.to_string();
        let p = create_platform(&db, cp).await.unwrap();
        (db, dir, p.id)
    }

    #[tokio::test]
    async fn reserve_counts_blocks_and_persists() {
        let (db, _dir, pid) = quota_db(r#"{"quota_windows":[{"window":3600,"budget_count":2}]}"#).await;
        assert_eq!(reserve_attempt(&db, "{}", pid, T0).await.unwrap(), Reserve::Allowed, "无窗口配置直接放行");
        assert_eq!(reserve_attempt(&db, r#"{"quota_windows":[{"window":3600,"budget_count":2}]}"#, pid, T0).await.unwrap(), Reserve::Allowed);
        assert_eq!(reserve_attempt(&db, r#"{"quota_windows":[{"window":3600,"budget_count":2}]}"#, pid, T0 + 1).await.unwrap(), Reserve::Allowed);
        // 第 3 次：consumed=2 >= 2 → 阻断，锚点 = 起点 + 1h
        let blocked = reserve_attempt(&db, r#"{"quota_windows":[{"window":3600,"budget_count":2}]}"#, pid, T0 + 2).await.unwrap();
        assert_eq!(blocked, Reserve::Blocked { retry_at_ms: T0 + H }, "3 > 2 应阻断且锚点=窗口结束");
        // 用量已持久化（重启不清零的载体），可经读池 load_usage 读回
        let usage = load_usage(&db, &[pid]).await;
        let rows = &usage[&pid];
        assert_eq!(rows.len(), 1);
        assert!((rows[0].consumed - 2.0).abs() < 1e-9);
        assert_eq!(rows[0].window_key, "3600|count|2");
    }

    #[tokio::test]
    async fn reserve_sliding_window_resets_persisted() {
        let extra = r#"{"quota_windows":[{"window":3600,"budget_count":1}]}"#;
        let (db, _dir, pid) = quota_db(extra).await;
        assert_eq!(reserve_attempt(&db, extra, pid, T0).await.unwrap(), Reserve::Allowed);
        assert!(matches!(reserve_attempt(&db, extra, pid, T0 + 1).await.unwrap(), Reserve::Blocked { .. }));
        // 窗口到期 → 惰性重置后放行，consumed 清 1（重置后本次占位）
        assert_eq!(reserve_attempt(&db, extra, pid, T0 + H).await.unwrap(), Reserve::Allowed);
        let rows = &load_usage(&db, &[pid]).await[&pid];
        assert!((rows[0].consumed - 1.0).abs() < 1e-9, "到期重置后占位 1 次");
        assert_eq!(rows[0].window_start_at, T0 + H);
    }

    #[tokio::test]
    async fn record_usage_tokens_and_usd_and_block_by_any() {
        let extra = r#"{"quota_windows":[{"window":3600,"budget_tokens":1000},{"window":7200,"budget_usd":5}]}"#;
        let (db, _dir, pid) = quota_db(extra).await;
        record_usage(&db, pid, extra, 600.0, 2.5, T0).await.unwrap();
        record_usage(&db, pid, extra, 600.0, 4.0, T0 + 1).await.unwrap();
        let rows = &load_usage(&db, &[pid]).await[&pid];
        assert_eq!(rows.len(), 2);
        let tokens_row = rows.iter().find(|r| r.window_key == "3600|tokens|1000").unwrap();
        let usd_row = rows.iter().find(|r| r.window_key == "7200|usd|5").unwrap();
        assert!((tokens_row.consumed - 1200.0).abs() < 1e-9, "token 累计（含 cache 由调用方折算进总 token）");
        assert!((usd_row.consumed - 6.5).abs() < 1e-9, "est_cost 累计");
        // 任一窗口超限即挡（token 1200 >= 1000）
        let blocked = reserve_attempt(&db, extra, pid, T0 + 2).await.unwrap();
        assert!(matches!(blocked, Reserve::Blocked { .. }), "token 窗口耗尽应阻断");
        // 只读判定同口径
        let windows = parse_quota_windows(extra);
        let usage = load_usage(&db, &[pid]).await.remove(&pid).unwrap();
        assert!(platform_exhausted_at(&windows, &usage, T0 + 2).is_some());
    }

    /// 并发不超限（硬保证）：budget=3，20 个并发占位 → 恰好 3 个放行，落库 consumed=3。
    /// 单写连接串行化 = 检查与计数同临界区（照 RateSlot「检查-占位」竞态口径）。
    #[tokio::test]
    async fn concurrent_reserves_never_overshoot() {
        let extra = r#"{"quota_windows":[{"window":3600,"budget_count":3}]}"#;
        let (db, _dir, pid) = quota_db(extra).await;
        let db = std::sync::Arc::new(db);
        let mut handles = Vec::new();
        for i in 0..20 {
            let db = db.clone();
            handles.push(tokio::spawn(async move {
                reserve_attempt(&db, extra, pid, T0 + i).await.unwrap()
            }));
        }
        let mut allowed = 0;
        for h in handles {
            if h.await.unwrap() == Reserve::Allowed {
                allowed += 1;
            }
        }
        assert_eq!(allowed, 3, "budget=3：并发 20 次占位恰好放行 3 次");
        let rows = &load_usage(&db, &[pid]).await[&pid];
        assert!((rows[0].consumed - 3.0).abs() < 1e-9, "落库计数不超预算");
    }
}
