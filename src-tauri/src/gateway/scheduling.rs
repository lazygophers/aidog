//! Group 智能调度 + 全局 Platform 级熔断器（内存状态）。
//!
//! 职责划分（与 router/proxy 解耦）：
//! - 熔断器：临时性，针对 5xx/超时等可恢复故障，自动半开探测恢复（本模块）。
//! - auto_disabled：永久性（401/403 鉴权失败），指数退避，状态持久化在 DB（db.rs）。
//! - 候选过滤取 [熔断 Open] ∪ [auto_disabled] 并集，二者状态独立，互不改写。
//!
//! 状态机三态：
//! ```text
//! Closed{fails}:  5xx/超时(retry 耗尽)计 fail → fails+1；达 threshold → Open{until=now+open_secs}。成功 → fails=0。
//! Open{until_ms}: 候选过滤直接踢出；now>=until → 转 HalfOpen{probes=0}。
//! HalfOpen{probes}: 放行至多 half_open_max 个探测；任一成功 → Closed；任一失败 → Open{重置 until}。
//! ```
//! 不计熔断：401/403（走 auto_disabled）、客户端 4xx(非 429)、probe 请求。

use std::collections::HashMap;
use std::sync::RwLock;

/// 熔断三态。
#[derive(Debug, Clone, PartialEq)]
pub enum BreakerState {
    /// 正常放行，记录连续失败次数。
    Closed { fails: u32 },
    /// 熔断打开，until_ms 之前候选过滤踢出。
    Open { until_ms: i64 },
    /// 半开探测，已发出 probes 个探测请求。
    HalfOpen { probes: u32 },
}

impl Default for BreakerState {
    fn default() -> Self {
        BreakerState::Closed { fails: 0 }
    }
}

/// 候选准入判定结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Admission {
    /// 正常放行。
    Allow,
    /// 熔断 Open，踢出候选。
    Reject,
    /// 半开探测放行（限量）。
    Probe,
}

/// per-platform 健康指标（内存）。
#[derive(Debug, Clone)]
pub struct PlatformHealth {
    pub breaker: BreakerState,
    /// 延迟 EMA（毫秒）；0 表示尚无样本。
    pub latency_ema_ms: f64,
    /// 当前在途请求数。
    pub inflight: u32,
}

impl Default for PlatformHealth {
    fn default() -> Self {
        Self {
            breaker: BreakerState::default(),
            latency_ema_ms: 0.0,
            inflight: 0,
        }
    }
}

/// 熔断有效阈值（已解析 platform 覆盖 / 全局默认）。
#[derive(Debug, Clone, Copy)]
pub struct BreakerThresholds {
    pub failure_threshold: u32,
    pub open_secs: u64,
    pub half_open_max: u32,
}

/// EMA 平滑系数（新样本权重）。0.3 ≈ 近 ~6 个样本窗口。
const EMA_ALPHA: f64 = 0.3;

/// 调度器状态单例：per-platform 健康表。随 ProxyState 持有，Arc clone 进后台。
pub struct SchedulerState {
    /// platform_id → 健康指标。
    health: RwLock<HashMap<u64, PlatformHealth>>,
}

impl Default for SchedulerState {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulerState {
    pub fn new() -> Self {
        Self {
            health: RwLock::new(HashMap::new()),
        }
    }

    /// 读取某平台延迟 EMA（无样本 → None），用于 LeastLatency 排序。
    pub fn latency_ema(&self, platform_id: u64) -> Option<f64> {
        let g = self.health.read().ok()?;
        g.get(&platform_id).and_then(|h| {
            if h.latency_ema_ms > 0.0 {
                Some(h.latency_ema_ms)
            } else {
                None
            }
        })
    }

    /// 读取某平台在途请求数（无记录 → 0）。观测 / 诊断用（未来 MinConnections 策略消费）。
    #[allow(dead_code)]
    pub fn inflight(&self, platform_id: u64) -> u32 {
        self.health
            .read()
            .ok()
            .and_then(|g| g.get(&platform_id).map(|h| h.inflight))
            .unwrap_or(0)
    }

    /// 候选准入判定（候选过滤准入门）。在 now_ms 时刻惰性转移 Open→HalfOpen。
    /// `enabled=false`（熔断总开关关）→ 一律 Allow，旁路熔断。
    pub fn admission(&self, platform_id: u64, thresholds: &BreakerThresholds, now_ms: i64, enabled: bool) -> Admission {
        if !enabled {
            return Admission::Allow;
        }
        let mut g = match self.health.write() {
            Ok(g) => g,
            Err(_) => return Admission::Allow,
        };
        let h = g.entry(platform_id).or_default();
        match h.breaker {
            BreakerState::Closed { .. } => Admission::Allow,
            BreakerState::Open { until_ms } => {
                if now_ms >= until_ms {
                    // 惰性转 HalfOpen，本次即作为首个探测放行。
                    h.breaker = BreakerState::HalfOpen { probes: 1 };
                    Admission::Probe
                } else {
                    Admission::Reject
                }
            }
            BreakerState::HalfOpen { probes } => {
                if probes < thresholds.half_open_max {
                    h.breaker = BreakerState::HalfOpen { probes: probes + 1 };
                    Admission::Probe
                } else {
                    // 探测名额用尽，等待探测结果（暂不再放行）。
                    Admission::Reject
                }
            }
        }
    }

    /// 在途请求 +1（forward 尝试前）。
    pub fn inc_inflight(&self, platform_id: u64) {
        if let Ok(mut g) = self.health.write() {
            g.entry(platform_id).or_default().inflight += 1;
        }
    }

    fn dec_inflight(h: &mut PlatformHealth) {
        h.inflight = h.inflight.saturating_sub(1);
    }

    /// 成功：更新延迟 EMA、breaker 转 Closed（含 HalfOpen→Closed）、inflight-1。
    pub fn record_success(&self, platform_id: u64, latency_ms: i64) {
        if let Ok(mut g) = self.health.write() {
            let h = g.entry(platform_id).or_default();
            Self::dec_inflight(h);
            let sample = latency_ms.max(0) as f64;
            h.latency_ema_ms = if h.latency_ema_ms <= 0.0 {
                sample
            } else {
                EMA_ALPHA * sample + (1.0 - EMA_ALPHA) * h.latency_ema_ms
            };
            h.breaker = BreakerState::Closed { fails: 0 };
        }
    }

    /// 失败（5xx/超时，本平台 retry 耗尽计一次）：breaker fail 计数、inflight-1。
    /// 不更新延迟 EMA（失败样本不计入延迟）。
    pub fn record_failure(&self, platform_id: u64, thresholds: &BreakerThresholds, now_ms: i64) {
        if let Ok(mut g) = self.health.write() {
            let h = g.entry(platform_id).or_default();
            Self::dec_inflight(h);
            let open_until = now_ms + thresholds.open_secs as i64 * 1000;
            h.breaker = match h.breaker {
                BreakerState::Closed { fails } => {
                    let next = fails + 1;
                    if next >= thresholds.failure_threshold {
                        BreakerState::Open { until_ms: open_until }
                    } else {
                        BreakerState::Closed { fails: next }
                    }
                }
                // HalfOpen 探测失败 → 立即重新 Open。
                BreakerState::HalfOpen { .. } => BreakerState::Open { until_ms: open_until },
                // Open 期间不应有 inflight 结果回流，但兜底重置 until。
                BreakerState::Open { .. } => BreakerState::Open { until_ms: open_until },
            };
        }
    }

    /// 不计入熔断的请求结束（401/403/客户端 4xx 非 429）：仅 inflight-1，不动 breaker/EMA。
    pub fn record_ignored(&self, platform_id: u64) {
        if let Ok(mut g) = self.health.write() {
            if let Some(h) = g.get_mut(&platform_id) {
                Self::dec_inflight(h);
            }
        }
    }

    /// 测试 / 诊断：读取当前 breaker 状态副本。
    #[cfg(test)]
    pub fn breaker_state(&self, platform_id: u64) -> BreakerState {
        self.health
            .read()
            .ok()
            .and_then(|g| g.get(&platform_id).map(|h| h.breaker.clone()))
            .unwrap_or_default()
    }
}

// ─── Sticky session 绑定（内存 LRU + TTL）────────────────────
//
// session 键取法（见 parent design.md §Sticky session 键）：
// aidog 现有 proxy.rs/models.rs 无 session_id 概念（grep 确认），故 MVP 用
// `group_key + 客户端稳定标识`（调用侧拼接，本模块只存 String→platform_id）。
// 绑定平台失效（不在候选集 / 熔断 Open）时回退正常调度并重写绑定。

/// Sticky 绑定 TTL（毫秒）：30 分钟无访问淘汰。
const STICKY_TTL_MS: i64 = 30 * 60 * 1000;
/// LRU 容量上限（防内存无界增长）。
const STICKY_CAP: usize = 4096;

struct StickyEntry {
    platform_id: u64,
    last_access_ms: i64,
}

/// session → platform 绑定表（内存）。随 SchedulerState 同生命周期。
pub struct StickyTable {
    map: RwLock<HashMap<String, StickyEntry>>,
}

impl Default for StickyTable {
    fn default() -> Self {
        Self::new()
    }
}

impl StickyTable {
    pub fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
        }
    }

    /// 查绑定平台（未过期）；命中刷新访问时间。
    pub fn get(&self, key: &str, now_ms: i64) -> Option<u64> {
        let mut g = self.map.write().ok()?;
        let entry = g.get_mut(key)?;
        if now_ms - entry.last_access_ms > STICKY_TTL_MS {
            g.remove(key);
            return None;
        }
        entry.last_access_ms = now_ms;
        Some(entry.platform_id)
    }

    /// 写绑定（回退调度选定平台后）。超容量时淘汰最久未访问项。
    pub fn put(&self, key: String, platform_id: u64, now_ms: i64) {
        if let Ok(mut g) = self.map.write() {
            if g.len() >= STICKY_CAP && !g.contains_key(&key) {
                if let Some(oldest) = g
                    .iter()
                    .min_by_key(|(_, e)| e.last_access_ms)
                    .map(|(k, _)| k.clone())
                {
                    g.remove(&oldest);
                }
            }
            g.insert(
                key,
                StickyEntry {
                    platform_id,
                    last_access_ms: now_ms,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thresholds(ft: u32, open_secs: u64, hom: u32) -> BreakerThresholds {
        BreakerThresholds {
            failure_threshold: ft,
            open_secs,
            half_open_max: hom,
        }
    }

    #[test]
    fn breaker_closed_to_open_to_halfopen_to_closed() {
        let s = SchedulerState::new();
        let th = thresholds(3, 30, 2);
        let now = 1_000_000i64;
        // 初始 Closed → Allow
        assert_eq!(s.admission(1, &th, now, true), Admission::Allow);
        // 失败累计到阈值前仍 Closed
        s.inc_inflight(1);
        s.record_failure(1, &th, now);
        s.inc_inflight(1);
        s.record_failure(1, &th, now);
        assert_eq!(s.admission(1, &th, now, true), Admission::Allow);
        // 第 3 次失败 → Open
        s.inc_inflight(1);
        s.record_failure(1, &th, now);
        assert!(matches!(s.breaker_state(1), BreakerState::Open { .. }));
        // Open 未到期 → Reject
        assert_eq!(s.admission(1, &th, now + 1000, true), Admission::Reject);
        // 到期 → HalfOpen 首个探测放行
        let until_passed = now + 30 * 1000 + 1;
        assert_eq!(s.admission(1, &th, until_passed, true), Admission::Probe);
        // 探测成功 → Closed
        s.record_success(1, 120);
        assert!(matches!(s.breaker_state(1), BreakerState::Closed { fails: 0 }));
        assert_eq!(s.admission(1, &th, until_passed, true), Admission::Allow);
    }

    #[test]
    fn halfopen_failure_reopens() {
        let s = SchedulerState::new();
        let th = thresholds(1, 30, 2);
        let now = 1_000_000i64;
        s.inc_inflight(2);
        s.record_failure(2, &th, now); // threshold=1 → Open
        let passed = now + 30 * 1000 + 1;
        assert_eq!(s.admission(2, &th, passed, true), Admission::Probe);
        // 探测失败 → 重新 Open
        s.record_failure(2, &th, passed);
        assert_eq!(s.admission(2, &th, passed + 1, true), Admission::Reject);
    }

    #[test]
    fn halfopen_limits_probes() {
        let s = SchedulerState::new();
        let th = thresholds(1, 30, 2);
        let now = 1_000_000i64;
        s.inc_inflight(3);
        s.record_failure(3, &th, now);
        let passed = now + 30 * 1000 + 1;
        // half_open_max=2 → 放 2 个探测，第 3 个 Reject
        assert_eq!(s.admission(3, &th, passed, true), Admission::Probe);
        assert_eq!(s.admission(3, &th, passed, true), Admission::Probe);
        assert_eq!(s.admission(3, &th, passed, true), Admission::Reject);
    }

    #[test]
    fn breaker_disabled_bypass() {
        let s = SchedulerState::new();
        let th = thresholds(1, 30, 2);
        let now = 1_000_000i64;
        s.inc_inflight(4);
        s.record_failure(4, &th, now); // Open
        // enabled=false → 一律 Allow（总开关旁路）
        assert_eq!(s.admission(4, &th, now, false), Admission::Allow);
    }

    #[test]
    fn latency_ema_updates() {
        let s = SchedulerState::new();
        s.inc_inflight(5);
        s.record_success(5, 100);
        assert_eq!(s.latency_ema(5), Some(100.0));
        s.inc_inflight(5);
        s.record_success(5, 200);
        // EMA = 0.3*200 + 0.7*100 = 130
        let ema = s.latency_ema(5).unwrap();
        assert!((ema - 130.0).abs() < 0.001, "ema={ema}");
    }

    #[test]
    fn inflight_inc_dec() {
        let s = SchedulerState::new();
        s.inc_inflight(6);
        s.inc_inflight(6);
        assert_eq!(s.inflight(6), 2);
        s.record_success(6, 50);
        assert_eq!(s.inflight(6), 1);
        s.record_failure(6, &thresholds(5, 30, 2), 0);
        assert_eq!(s.inflight(6), 0);
    }

    #[test]
    fn sticky_bind_and_ttl() {
        let t = StickyTable::new();
        let now = 1_000_000i64;
        assert_eq!(t.get("k", now), None);
        t.put("k".to_string(), 42, now);
        // 命中刷新访问时间（last_access 重置为查询时刻）
        assert_eq!(t.get("k", now + 1000), Some(42));
        // 距上次访问(now+1000)超 TTL → 淘汰
        assert_eq!(t.get("k", now + 1000 + STICKY_TTL_MS + 1), None);
        // 已淘汰后再查仍 None
        assert_eq!(t.get("k", now + 1000 + STICKY_TTL_MS + 2), None);
    }
}
