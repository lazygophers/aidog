//! 平台限频执行（`platform.extra.rate_limits`，并发 / RPM / TPM，平台级 + 模型级，可按时段换上限）。
//!
//! 计数只存进程内存（重启清零，不加表）。与数据库 `platform.rate_limit` 列（上游响应头里的
//! 余量快照）无关，别混。
//!
//! 两个动作：
//! - **候选筛选**（只读）：[`RateLimiter::has_room`]，超限平台从候选排除，不改 status 三态。
//! - **转发前占位**（原子）：[`RateLimiter::begin`] / [`Access::enter_model`]，占一个并发名额、
//!   记一次 RPM、按输入 token 估算预占 TPM。[`Access`] 随响应体存活，Drop 时释放并发
//!   （成功 / 失败 / 流式结束 / 客户端断连都走 Drop）。
//!
//! RPM / TPM 为滑动 60 秒窗口。TPM 预占结束后由 [`Access::settle`] 按上游 usage 修正；无 usage 保留预占。

use super::peak::{self, TimeWindow};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

const WINDOW_MS: i64 = 60_000;

/// 一组上限；`None` = 不限。非正整数一律当不限。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    pub concurrency: Option<i64>,
    pub rpm: Option<i64>,
    pub tpm: Option<i64>,
}

impl Limits {
    fn from_value(v: &Value) -> Self {
        let pos = |k: &str| {
            let x = v.get(k)?;
            x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)).filter(|n| *n > 0)
        };
        Self { concurrency: pos("concurrency"), rpm: pos("rpm"), tpm: pos("tpm") }
    }

    pub fn is_unlimited(&self) -> bool {
        *self == Self::default()
    }

    /// 窗口覆盖：窗口写了的字段覆盖本体，没写的沿用本体。
    fn overlay(&self, win: &Limits) -> Limits {
        Limits {
            concurrency: win.concurrency.or(self.concurrency),
            rpm: win.rpm.or(self.rpm),
            tpm: win.tpm.or(self.tpm),
        }
    }
}

/// `extra.rate_limits` 的解析结果。
#[derive(Debug, Clone, Default)]
pub struct RateLimitsCfg {
    base: Limits,
    models: HashMap<String, Limits>,
    windows: Vec<(TimeWindow, Limits)>,
}

impl RateLimitsCfg {
    /// 缺失 / 非法 JSON → 空（不限）。`source_policy_id` / `source_last_updated` 不在执行路径上，
    /// 整个 `extra` 原样留在 DB，不会被这里丢掉。
    pub fn parse(extra: &str) -> Self {
        if !extra.contains("rate_limits") {
            return Self::default();
        }
        let Ok(root) = serde_json::from_str::<Value>(extra) else {
            return Self::default();
        };
        let Some(rl) = root.get("rate_limits").filter(|v| v.is_object()) else {
            return Self::default();
        };
        let models = rl
            .get("models")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), Limits::from_value(v)))
                    .filter(|(_, l)| !l.is_unlimited())
                    .collect()
            })
            .unwrap_or_default();
        let windows = rl
            .get("windows")
            .and_then(Value::as_array)
            .map(|ws| {
                ws.iter()
                    .filter_map(|w| {
                        // 窗口形状同 peak（时段字段），上限字段另读；multiplier 对限频无意义，补 1.0 过 TimeWindow 反序列化。
                        let mut tw = w.clone();
                        tw.as_object_mut()?.entry("multiplier").or_insert(Value::from(1.0));
                        Some((serde_json::from_value::<TimeWindow>(tw).ok()?, Limits::from_value(w)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { base: Limits::from_value(rl), models, windows }
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_unlimited() && self.models.is_empty() && self.windows.is_empty()
    }

    pub fn has_tpm(&self) -> bool {
        self.base.tpm.is_some()
            || self.models.values().any(|l| l.tpm.is_some())
            || self.windows.iter().any(|(_, l)| l.tpm.is_some())
    }

    /// 平台级上限：本体 + 首个命中的时段窗口覆盖（first-match，同 peak 窗口语义）。
    pub fn platform_limits(&self, now_ms: i64, request_model: &str) -> Limits {
        for (tw, lim) in &self.windows {
            if peak::is_in_peak_window(std::slice::from_ref(tw), now_ms, request_model) {
                return self.base.overlay(lim);
            }
        }
        self.base
    }

    /// 模型级上限（键精确匹配模型名）；没配 = 不限。
    pub fn model_limits(&self, model: &str) -> Limits {
        self.models.get(model).copied().unwrap_or_default()
    }
}

#[derive(Default)]
struct Inner {
    in_flight: i64,
    hits: VecDeque<i64>,
    /// (时刻, 占用 token)；占用值是共享 cell，settle 时原地修正。
    tokens: VecDeque<(i64, Arc<AtomicI64>)>,
}

impl Inner {
    fn prune(&mut self, now: i64) {
        let cutoff = now - WINDOW_MS;
        while self.hits.front().is_some_and(|t| *t <= cutoff) {
            self.hits.pop_front();
        }
        while self.tokens.front().is_some_and(|(t, _)| *t <= cutoff) {
            self.tokens.pop_front();
        }
    }

    /// `est` = 本次预占量（候选筛选时传 0）。窗口里已有占用才拦 TPM：单个超大请求在空窗口放行，
    /// 否则它永远发不出去。
    fn has_room(&mut self, lim: &Limits, now: i64, est: i64) -> bool {
        self.prune(now);
        if lim.concurrency.is_some_and(|c| self.in_flight >= c) {
            return false;
        }
        if lim.rpm.is_some_and(|r| self.hits.len() as i64 >= r) {
            return false;
        }
        if let Some(t) = lim.tpm {
            let used: i64 = self.tokens.iter().map(|(_, c)| c.load(Ordering::Relaxed)).sum();
            if used > 0 && used + est.max(1) > t {
                return false;
            }
        }
        true
    }
}

#[derive(Default)]
struct Bucket(Mutex<Inner>);

/// 一次占位：持有桶 + 本次预占 cell；Drop 释放并发名额。
struct Item {
    bucket: Arc<Bucket>,
    cell: Arc<AtomicI64>,
}

impl Drop for Item {
    fn drop(&mut self) {
        if let Ok(mut g) = self.bucket.0.lock() {
            g.in_flight -= 1;
        }
    }
}

/// 进程内限频计数器，按 (平台 id, 模型) 分桶；模型为空串 = 平台级。
#[derive(Default)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<(u64, String), Arc<Bucket>>>,
}

impl RateLimiter {
    fn bucket(&self, pid: u64, model: &str, create: bool) -> Option<Arc<Bucket>> {
        let mut m = self.buckets.lock().ok()?;
        let key = (pid, model.to_string());
        if create {
            Some(m.entry(key).or_default().clone())
        } else {
            m.get(&key).cloned()
        }
    }

    /// 只读：该桶当前还放不放得下一个请求（候选筛选用）。没建过桶 = 没有占用 = 放得下。
    pub fn has_room(&self, pid: u64, model: &str, lim: &Limits, now: i64) -> bool {
        if lim.is_unlimited() {
            return true;
        }
        match self.bucket(pid, model, false) {
            None => true,
            Some(b) => b.0.lock().map(|mut g| g.has_room(lim, now, 0)).unwrap_or(true),
        }
    }

    fn take(&self, pid: u64, model: &str, lim: &Limits, now: i64, est: i64) -> Option<Item> {
        let b = self.bucket(pid, model, true)?;
        let mut g = b.0.lock().ok()?;
        if !g.has_room(lim, now, est) {
            return None;
        }
        g.in_flight += 1;
        g.hits.push_back(now);
        let cell = Arc::new(AtomicI64::new(est));
        g.tokens.push_back((now, cell.clone()));
        drop(g);
        Some(Item { bucket: b, cell })
    }

    /// 平台级占位（一次访问只占一次，换模型不重复占）。超限返回 None。
    /// 平台级不限时也返回 `Access`（模型级可能仍要占位）。
    pub fn begin(self: &Arc<Self>, pid: u64, lim: &Limits, now: i64, est: i64) -> Option<Arc<Access>> {
        let platform = if lim.is_unlimited() { None } else { Some(self.take(pid, "", lim, now, est)?) };
        Some(Arc::new(Access {
            limiter: self.clone(),
            pid,
            slots: Mutex::new([platform, None]),
        }))
    }
}

/// 一次访问（一个候选平台，含同平台换模型）的占位句柄；随响应体存活，Drop 释放并发。
pub struct Access {
    limiter: Arc<RateLimiter>,
    pid: u64,
    /// [平台级, 当前模型级]
    slots: Mutex<[Option<Item>; 2]>,
}

impl Access {
    /// 换到 `model` 时重判并占模型级上限（上一个模型的名额先释放）。超限返回 false，调用方换下一个模型。
    pub fn enter_model(&self, model: &str, lim: &Limits, now: i64, est: i64) -> bool {
        let Ok(mut s) = self.slots.lock() else { return true };
        s[1] = None;
        if lim.is_unlimited() {
            return true;
        }
        match self.limiter.take(self.pid, model, lim, now, est) {
            Some(it) => {
                s[1] = Some(it);
                true
            }
            None => false,
        }
    }

    /// 按上游实际 usage（input+output）修正 TPM 预占；`tokens <= 0`（无 usage）保留预占。
    pub fn settle(&self, tokens: i64) {
        if tokens <= 0 {
            return;
        }
        if let Ok(s) = self.slots.lock() {
            for it in s.iter().flatten() {
                it.cell.store(tokens, Ordering::Relaxed);
            }
        }
    }
}

/// 把 `access` 绑到响应体上：body 被丢弃（发完 / 客户端断连）时释放并发。
pub(crate) fn hold_until_body_done(resp: axum::response::Response, access: Arc<Access>) -> axum::response::Response {
    use futures::StreamExt;
    let (parts, body) = resp.into_parts();
    let stream = body.into_data_stream().map(move |chunk| {
        let _held = &access;
        chunk
    });
    axum::response::Response::from_parts(parts, axum::body::Body::from_stream(stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lim(c: Option<i64>, r: Option<i64>, t: Option<i64>) -> Limits {
        Limits { concurrency: c, rpm: r, tpm: t }
    }

    #[test]
    fn parse_new_and_legacy_fields_and_ignore_junk() {
        let cfg = RateLimitsCfg::parse(
            r#"{"rate_limits":{"rpm":60,"tpm":1000,"concurrency":5,"source_policy_id":"v1","source_last_updated":9,
                "models":{"m":{"concurrency":2},"zero":{"rpm":0}},
                "windows":[{"start_hour":0,"end_hour":8,"timezone":"Asia/Shanghai","concurrency":10}]}}"#,
        );
        assert_eq!(cfg.base, lim(Some(5), Some(60), Some(1000)));
        assert_eq!(cfg.model_limits("m"), lim(Some(2), None, None));
        assert!(cfg.model_limits("zero").is_unlimited(), "非正数 = 不限，且不占条目");
        assert_eq!(cfg.windows.len(), 1);
        assert!(RateLimitsCfg::parse("").is_empty());
        assert!(RateLimitsCfg::parse("not-json rate_limits").is_empty());
        assert!(RateLimitsCfg::parse(r#"{"rate_limits":{}}"#).is_empty());
    }

    #[test]
    fn window_overrides_only_written_fields() {
        let cfg = RateLimitsCfg::parse(
            r#"{"rate_limits":{"rpm":60,"concurrency":5,"windows":[{"start_hour":0,"end_hour":24,"concurrency":10}]}}"#,
        );
        // 全天窗口恒命中：concurrency 被覆盖，rpm 沿用本体
        assert_eq!(cfg.platform_limits(1_700_000_000_000, ""), lim(Some(10), Some(60), None));
        // 窗口时段外沿用本体：UTC 0~1 点的窗口，取 UTC 12 点
        let cfg = RateLimitsCfg::parse(
            r#"{"rate_limits":{"concurrency":5,"windows":[{"start_hour":0,"end_hour":1,"concurrency":10}]}}"#,
        );
        let noon_utc = 1_700_000_000_000 - (1_700_000_000_000 % 86_400_000) + 12 * 3_600_000;
        assert_eq!(cfg.platform_limits(noon_utc, "").concurrency, Some(5));
        assert_eq!(cfg.platform_limits(noon_utc - 12 * 3_600_000 + 1000, "").concurrency, Some(10));
    }

    #[test]
    fn rpm_sliding_window() {
        let rl = Arc::new(RateLimiter::default());
        let l = lim(None, Some(2), None);
        let (a, b) = (rl.begin(1, &l, 1000, 0).unwrap(), rl.begin(1, &l, 2000, 0).unwrap());
        assert!(rl.begin(1, &l, 3000, 0).is_none(), "第 3 个超限");
        assert!(!rl.has_room(1, "", &l, 3000));
        assert!(rl.begin(1, &l, 1000 + WINDOW_MS, 0).is_some(), "60s 后最早一条滑出");
        drop((a, b));
    }

    #[test]
    fn concurrency_released_on_drop() {
        let rl = Arc::new(RateLimiter::default());
        let l = lim(Some(1), None, None);
        let a = rl.begin(1, &l, 0, 0).unwrap();
        assert!(rl.begin(1, &l, 1, 0).is_none());
        drop(a);
        assert!(rl.begin(1, &l, 2, 0).is_some());
    }

    #[test]
    fn tpm_reserve_settle_and_keep_without_usage() {
        let rl = Arc::new(RateLimiter::default());
        let l = lim(None, None, Some(100));
        let a = rl.begin(1, &l, 0, 500).unwrap(); // 空窗口放行超大请求
        assert!(rl.begin(1, &l, 1, 1).is_none(), "预占 500 > 100");
        a.settle(0); // 无 usage：保留预占
        assert!(rl.begin(1, &l, 2, 1).is_none());
        a.settle(40); // 按 usage 修正
        assert!(rl.begin(1, &l, 3, 50).is_some(), "40 + 50 <= 100");
        assert!(rl.begin(1, &l, 4, 50).is_none(), "90 + 50 > 100");
    }

    #[test]
    fn model_chain_rejudges_model_level_and_counts_platform_once() {
        let rl = Arc::new(RateLimiter::default());
        let plat = lim(None, Some(10), None);
        let full = lim(Some(1), None, None);
        // 另一访问先占满 m1 的模型级并发
        let other = rl.begin(1, &Limits::default(), 0, 0).unwrap();
        assert!(other.enter_model("m1", &full, 0, 0));
        let acc = rl.begin(1, &plat, 1, 0).unwrap();
        assert!(!acc.enter_model("m1", &full, 1, 0), "m1 模型级满 → 换下一个");
        assert!(acc.enter_model("m2", &full, 1, 0), "m2 重新判定通过");
        // 平台级 rpm 只记了 begin 那一次
        let b = rl.bucket(1, "", false).unwrap();
        assert_eq!(b.0.lock().unwrap().hits.len(), 1);
        drop(acc);
        assert_eq!(b.0.lock().unwrap().in_flight, 0, "平台级并发随 Access 释放");
    }
}
