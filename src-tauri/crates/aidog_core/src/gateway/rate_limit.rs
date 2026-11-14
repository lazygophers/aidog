//! 平台限频执行（`platform.extra.rate_limits`：平台级上限 + 限频规则列表 `rules`，规则按模型 / 时段选取，并发 / RPM / TPM）。
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
            x.as_i64()
                .or_else(|| x.as_f64().map(|f| f as i64))
                .filter(|n| *n > 0)
        };
        Self {
            concurrency: pos("concurrency"),
            rpm: pos("rpm"),
            tpm: pos("tpm"),
        }
    }

    pub fn is_unlimited(&self) -> bool {
        *self == Self::default()
    }
}

/// 一条限频规则：至少一个条件（`models` 非空 和/或 时段），没填的上限 = 不限。
#[derive(Debug, Clone)]
struct Rule {
    models: Vec<String>,
    time: Option<TimeWindow>,
    limits: Limits,
}

impl Rule {
    /// 无条件（models 空且无时段）→ None（丢弃并 warn）。
    fn from_value(v: &Value, platform_calendar: Option<&str>) -> Option<Self> {
        let models: Vec<String> = v
            .get("models")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|m| m.as_str().map(str::trim))
                    .filter(|m| !m.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let has_time = ["start_hour", "end_hour", "day_type"]
            .iter()
            .any(|k| v.get(k).is_some_and(|x| !x.is_null()))
            || v.get("days_of_week")
                .and_then(Value::as_array)
                .is_some_and(|d| !d.is_empty());
        let time = if has_time {
            // 时段字段形状同 peak 窗口；缺省补 0~24 全天，multiplier 对限频无意义，补 1.0 过 TimeWindow 反序列化，
            // `models` 是规则自己的条件，不交给窗口判定。
            let mut tw = v.clone();
            let o = tw.as_object_mut()?;
            o.remove("models");
            o.entry("start_hour").or_insert(Value::from(0));
            o.entry("end_hour").or_insert(Value::from(24));
            o.entry("multiplier").or_insert(Value::from(1.0));
            if !o.contains_key("holiday_calendar")
                && let Some(code) = platform_calendar
            {
                o.insert("holiday_calendar".into(), Value::from(code));
            }
            Some(serde_json::from_value::<TimeWindow>(tw).ok()?)
        } else {
            None
        };
        if models.is_empty() && time.is_none() {
            tracing::warn!("rate_limits.rules: 规则没有任何条件（模型 / 时段），已丢弃");
            return None;
        }
        Some(Self {
            models,
            time,
            limits: Limits::from_value(v),
        })
    }

    fn time_hit(&self, now_ms: i64) -> bool {
        self.time
            .as_ref()
            .is_none_or(|tw| peak::is_in_peak_window(std::slice::from_ref(tw), now_ms, ""))
    }

    /// 模型名精确匹配，或 `prefix*` 后缀通配（迁移自旧窗口的 models 保持原匹配语义）。
    fn model_hit(&self, model: &str) -> bool {
        self.models
            .iter()
            .any(|m| m == model || m.strip_suffix('*').is_some_and(|p| model.starts_with(p)))
    }
}

/// `extra.rate_limits` 的解析结果。
#[derive(Debug, Clone, Default)]
pub struct RateLimitsCfg {
    base: Limits,
    rules: Vec<Rule>,
}

impl RateLimitsCfg {
    /// 缺失 / 非法 JSON → 空（不限）。`source_policy_id` / `source_last_updated` 不在执行路径上，
    /// 整个 `extra` 原样留在 DB，不会被这里丢掉。旧键 `models` / `windows` 不再读（启动迁移已转 rules）。
    pub fn parse(extra: &str) -> Self {
        Self::parse_with_calendar(extra, None)
    }

    pub fn parse_with_calendar(extra: &str, registry_calendar: Option<&str>) -> Self {
        if !extra.contains("rate_limits") {
            return Self::default();
        }
        let Ok(root) = serde_json::from_str::<Value>(extra) else {
            return Self::default();
        };
        let Some(rl) = root.get("rate_limits").filter(|v| v.is_object()) else {
            return Self::default();
        };
        let platform_calendar = root
            .get("holiday_calendar")
            .and_then(Value::as_str)
            .or(registry_calendar);
        let rules = rl
            .get("rules")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|rule| Rule::from_value(rule, platform_calendar))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            base: Limits::from_value(rl),
            rules,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.base.is_unlimited() && self.rules.is_empty()
    }

    pub fn has_tpm(&self) -> bool {
        self.base.tpm.is_some() || self.rules.iter().any(|r| r.limits.tpm.is_some())
    }

    /// 平台级上限：首个命中的「只选时段」规则整套替换本体（没填的项不限）；没命中用本体。
    pub fn platform_limits(&self, now_ms: i64) -> Limits {
        self.rules
            .iter()
            .find(|r| r.models.is_empty() && r.time_hit(now_ms))
            .map_or(self.base, |r| r.limits)
    }

    /// 模型级上限（模型 = 实际发往上游的模型名）：首个命中的「模型 + 时段」> 首个「只选模型」> 不限。
    pub fn model_limits(&self, model: &str, now_ms: i64) -> Limits {
        let mut model_only = None;
        for r in self
            .rules
            .iter()
            .filter(|r| !r.models.is_empty() && r.model_hit(model))
        {
            match r.time {
                Some(_) if r.time_hit(now_ms) => return r.limits,
                None => {
                    model_only.get_or_insert(r.limits);
                }
                _ => {}
            }
        }
        model_only.unwrap_or_default()
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

    /// `est` = 本次预占量（候选筛选与 `begin` 同一估算）。窗口里已有占用才拦 TPM：单个超大请求在空窗口放行，
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
            let used: i64 = self
                .tokens
                .iter()
                .map(|(_, c)| c.load(Ordering::Relaxed))
                .sum();
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
    pub fn has_room(&self, pid: u64, model: &str, lim: &Limits, now: i64, est: i64) -> bool {
        if lim.is_unlimited() {
            return true;
        }
        match self.bucket(pid, model, false) {
            None => true,
            Some(b) => {
                b.0.lock()
                    .map(|mut g| g.has_room(lim, now, est))
                    .unwrap_or(true)
            }
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
    pub fn begin(
        self: &Arc<Self>,
        pid: u64,
        lim: &Limits,
        now: i64,
        est: i64,
    ) -> Option<Arc<Access>> {
        let platform = if lim.is_unlimited() {
            None
        } else {
            Some(self.take(pid, "", lim, now, est)?)
        };
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
        let Ok(mut s) = self.slots.lock() else {
            return true;
        };
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
pub(crate) fn hold_until_body_done(
    resp: axum::response::Response,
    access: Arc<Access>,
) -> axum::response::Response {
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
        Limits {
            concurrency: c,
            rpm: r,
            tpm: t,
        }
    }

    const H: i64 = 3_600_000;
    /// 某 UTC 日的 00:00。
    fn day0() -> i64 {
        1_700_000_000_000 - (1_700_000_000_000 % 86_400_000)
    }
    fn cfg(rl: &str) -> RateLimitsCfg {
        RateLimitsCfg::parse(&format!(r#"{{"rate_limits":{rl}}}"#))
    }

    #[test]
    fn parse_rules_shape_drops_conditionless_and_ignores_legacy_keys() {
        let c = cfg(
            r#"{"rpm":60,"tpm":1000,"concurrency":5,"source_policy_id":"v1",
                "rules":[{"models":["m"],"concurrency":2},{"rpm":9},{"models":[],"rpm":9},{"models":"x","rpm":9}],
                "models":{"old":{"rpm":1}},
                "windows":[{"start_hour":0,"end_hour":24,"rpm":1}]}"#,
        );
        assert_eq!(c.base, lim(Some(5), Some(60), Some(1000)));
        assert_eq!(c.model_limits("m", 0), lim(Some(2), None, None));
        assert!(
            c.model_limits("old", 0).is_unlimited(),
            "旧键 models 不再读"
        );
        assert_eq!(
            c.platform_limits(day0() + 12 * H),
            c.base,
            "旧键 windows 不再读；无条件规则被丢弃"
        );
        assert!(RateLimitsCfg::parse("").is_empty());
        assert!(RateLimitsCfg::parse("not-json rate_limits").is_empty());
        assert!(RateLimitsCfg::parse(r#"{"rate_limits":{}}"#).is_empty());
        assert!(!cfg(r#"{"rules":[{"models":["m"],"rpm":1}]}"#).is_empty());
    }

    #[test]
    fn time_only_rule_replaces_platform_and_unfilled_is_unlimited() {
        let c = cfg(
            r#"{"rpm":60,"concurrency":5,"rules":[{"start_hour":0,"end_hour":1,"concurrency":10}]}"#,
        );
        // 命中：整套替换，rpm 没填 = 不限
        assert_eq!(
            c.platform_limits(day0() + 30 * 60_000),
            lim(Some(10), None, None)
        );
        // 时段外：平台级
        assert_eq!(
            c.platform_limits(day0() + 12 * H),
            lim(Some(5), Some(60), None)
        );
    }

    #[test]
    fn time_rule_cross_day_and_timezone_and_weekday() {
        // 跨天 22~6（UTC）
        let c = cfg(r#"{"rpm":9,"rules":[{"start_hour":22,"end_hour":6,"rpm":1}]}"#);
        assert_eq!(c.platform_limits(day0() + 23 * H).rpm, Some(1));
        assert_eq!(c.platform_limits(day0() + 3 * H).rpm, Some(1));
        assert_eq!(c.platform_limits(day0() + 12 * H).rpm, Some(9));
        // 时区：上海 20~22 = UTC 12~14
        let c = cfg(
            r#"{"rpm":9,"rules":[{"start_hour":20,"end_hour":22,"timezone":"Asia/Shanghai","rpm":1}]}"#,
        );
        assert_eq!(c.platform_limits(day0() + 13 * H).rpm, Some(1));
        assert_eq!(c.platform_limits(day0() + 20 * H).rpm, Some(9));
        // 星期：2023-11-14 是周二(2)；只选周三 → 不命中
        let tue = day0();
        assert_eq!((tue / 86_400_000 + 4) % 7, 2);
        let c =
            cfg(r#"{"rpm":9,"rules":[{"start_hour":0,"end_hour":24,"days_of_week":[3],"rpm":1}]}"#);
        assert_eq!(c.platform_limits(tue + 5 * H).rpm, Some(9));
        let c =
            cfg(r#"{"rpm":9,"rules":[{"start_hour":0,"end_hour":24,"days_of_week":[2],"rpm":1}]}"#);
        assert_eq!(c.platform_limits(tue + 5 * H).rpm, Some(1));
    }

    #[test]
    fn overlapping_time_only_rules_first_in_list_wins() {
        let c = cfg(
            r#"{"rules":[{"start_hour":0,"end_hour":24,"rpm":1},{"start_hour":0,"end_hour":24,"rpm":2}]}"#,
        );
        assert_eq!(c.platform_limits(day0() + H).rpm, Some(1));
    }

    #[test]
    fn model_layer_priority_model_time_over_model_only_over_unlimited() {
        let c = cfg(r#"{"rules":[
                {"models":["m"],"rpm":100},
                {"models":["m"],"start_hour":0,"end_hour":1,"concurrency":3},
                {"models":["m"],"start_hour":0,"end_hour":24,"rpm":7}]}"#);
        // 模型 + 时段命中（列表里排在「只选模型」之后也优先）：整条生效，没填的 rpm 不限；同类重叠取第一条
        assert_eq!(
            c.model_limits("m", day0() + 30 * 60_000),
            lim(Some(3), None, None)
        );
        // 1 点后只剩第三条模型 + 时段（全天）
        assert_eq!(
            c.model_limits("m", day0() + 5 * H),
            lim(None, Some(7), None)
        );
        // 其他模型：不限
        assert!(c.model_limits("other", day0()).is_unlimited());
        // 去掉全天条后：时段外回落「只选模型」
        let c = cfg(
            r#"{"rules":[{"models":["m"],"rpm":100},{"models":["m"],"start_hour":0,"end_hour":1,"concurrency":3}]}"#,
        );
        assert_eq!(
            c.model_limits("m", day0() + 5 * H),
            lim(None, Some(100), None)
        );
    }

    #[test]
    fn model_only_overlap_first_wins_and_time_only_does_not_touch_model_layer() {
        let c = cfg(
            r#"{"rpm":5,"rules":[{"start_hour":0,"end_hour":24,"rpm":1},{"models":["a","b"],"rpm":3},{"models":["a"],"rpm":4}]}"#,
        );
        assert_eq!(c.model_limits("a", 0).rpm, Some(3));
        assert_eq!(c.model_limits("b", 0).rpm, Some(3));
        assert!(
            c.model_limits("c", 0).is_unlimited(),
            "只选时段规则不进模型层"
        );
    }

    #[test]
    fn multi_model_rule_counts_each_model_separately() {
        let c = cfg(r#"{"rules":[{"models":["a","b"],"rpm":1}]}"#);
        let rl = Arc::new(RateLimiter::default());
        let acc = rl.begin(1, &Limits::default(), 0, 0).unwrap();
        assert!(acc.enter_model("a", &c.model_limits("a", 0), 0, 0));
        let acc2 = rl.begin(1, &Limits::default(), 1, 0).unwrap();
        assert!(
            !acc2.enter_model("a", &c.model_limits("a", 1), 1, 0),
            "a 的 rpm 已满"
        );
        assert!(
            acc2.enter_model("b", &c.model_limits("b", 1), 1, 0),
            "b 独立计数"
        );
    }

    #[test]
    fn has_tpm_sees_rules() {
        assert!(cfg(r#"{"rules":[{"models":["m"],"tpm":9}]}"#).has_tpm());
        assert!(cfg(r#"{"rules":[{"start_hour":1,"end_hour":2,"tpm":9}]}"#).has_tpm());
        assert!(!cfg(r#"{"rules":[{"models":["m"],"rpm":9}]}"#).has_tpm());
    }

    #[test]
    fn rpm_sliding_window() {
        let rl = Arc::new(RateLimiter::default());
        let l = lim(None, Some(2), None);
        let (a, b) = (
            rl.begin(1, &l, 1000, 0).unwrap(),
            rl.begin(1, &l, 2000, 0).unwrap(),
        );
        assert!(rl.begin(1, &l, 3000, 0).is_none(), "第 3 个超限");
        assert!(!rl.has_room(1, "", &l, 3000, 0));
        assert!(
            rl.begin(1, &l, 1000 + WINDOW_MS, 0).is_some(),
            "60s 后最早一条滑出"
        );
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
        assert!(
            !acc.enter_model("m1", &full, 1, 0),
            "m1 模型级满 → 换下一个"
        );
        assert!(acc.enter_model("m2", &full, 1, 0), "m2 重新判定通过");
        // 平台级 rpm 只记了 begin 那一次
        let b = rl.bucket(1, "", false).unwrap();
        assert_eq!(b.0.lock().unwrap().hits.len(), 1);
        drop(acc);
        assert_eq!(b.0.lock().unwrap().in_flight, 0, "平台级并发随 Access 释放");
    }
}
