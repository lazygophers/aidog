//! 额度窗口模型（`platform.extra.quota_windows[]` 的配置解析）。
//!
//! 配置由前端限频表单写入 extra JSON（QuotaWindowConfig），执行语义在
//! `aidog_core::gateway::quota_window`（窗口重置 / 耗尽阻断 / 用量持久化）。
//! 用量状态不存 extra：独立表 `quota_window_usage`（见 schema_late），重启不清零。

use super::default_true;
use serde::{Deserialize, Serialize};

/// 窗口重置机制：sliding（默认，本窗口第一笔请求起算 window 秒）/
/// calendar_day（本地自然日 00:00 对齐）/ calendar_hour（自然小时对齐，UTC 整点）。
/// 旧数据无键 → Sliding（serde default）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuotaResetMode {
    #[default]
    Sliding,
    CalendarDay,
    CalendarHour,
}

/// 一条额度窗口（预算三选一：token / 金额 $ / 次数）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaWindow {
    /// 是否启用此窗口（缺省 true；关掉 = 不参与阻断与计数）
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 窗口重置机制（缺省 sliding）
    #[serde(default)]
    pub reset_mode: QuotaResetMode,
    /// 窗口时长（秒）
    #[serde(default)]
    pub window: f64,
    #[serde(default)]
    pub budget_tokens: Option<f64>,
    #[serde(default)]
    pub budget_usd: Option<f64>,
    #[serde(default)]
    pub budget_count: Option<f64>,
    /// 存量兼容：budget_points 已废（2026-10-23 拍板），读取层忽略不报错
    #[serde(default, skip_serializing)]
    pub budget_points: Option<f64>,
}

impl QuotaWindow {
    /// 预算三选一（token / usd / count）里首个非空者；全空 → None（无效窗口，不参与）。
    pub fn budget(&self) -> Option<QuotaBudget> {
        if let Some(v) = self.budget_tokens {
            return Some(QuotaBudget::Tokens(v));
        }
        if let Some(v) = self.budget_usd {
            return Some(QuotaBudget::Usd(v));
        }
        self.budget_count.map(QuotaBudget::Count)
    }
}

/// 预算维度 + 额度值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuotaBudget {
    Tokens(f64),
    Usd(f64),
    Count(f64),
}

impl QuotaBudget {
    /// 用量状态表的窗口键（配置内容稳定派生）：`<窗口秒>|<维度>|<额度>`。
    /// 用户改额度 → 新键（旧用量行变孤儿，无害；新窗口从零起算）。
    pub fn key(&self, window_sec: f64) -> String {
        match self {
            Self::Tokens(v) => format!("{window_sec}|tokens|{v}"),
            Self::Usd(v) => format!("{window_sec}|usd|{v}"),
            Self::Count(v) => format!("{window_sec}|count|{v}"),
        }
    }
}

/// 解析 `platform.extra` JSON 里的 `quota_windows` 数组；空 / 非法 / 无键 → 空 Vec。
/// 单条缺 budget 或 window <= 0 的条目按无效丢弃（不参与阻断与计数）。
pub fn parse_quota_windows(extra: &str) -> Vec<QuotaWindow> {
    if extra.trim().is_empty() || !extra.contains("quota_windows") {
        return Vec::new();
    }
    let Ok(root) = serde_json::from_str::<serde_json::Value>(extra) else {
        return Vec::new();
    };
    let Some(arr) = root.get("quota_windows").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| serde_json::from_value::<QuotaWindow>(v.clone()).ok())
        .filter(|w| w.window > 0.0 && w.budget().is_some())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_enabled_and_sliding() {
        let ws = parse_quota_windows(r#"{"quota_windows":[{"window":18000,"budget_tokens":120000}]}"#);
        assert_eq!(ws.len(), 1);
        assert!(ws[0].enabled, "缺 enabled → 默认开启");
        assert_eq!(ws[0].reset_mode, QuotaResetMode::Sliding, "缺 reset_mode → 默认 sliding");
    }

    #[test]
    fn parse_explicit_fields_and_drops_invalid() {
        let ws = parse_quota_windows(
            r#"{"quota_windows":[
                {"window":3600,"budget_usd":5.5,"enabled":false,"reset_mode":"calendar_day"},
                {"window":60,"budget_count":10,"reset_mode":"calendar_hour"},
                {"window":0,"budget_tokens":5},
                {"window":60},
                {"budget_tokens":5},
                {"window":-1,"budget_tokens":5}
            ]}"#,
        );
        assert_eq!(ws.len(), 2, "window<=0 / 无 budget 的条目丢弃");
        assert!(!ws[0].enabled);
        assert_eq!(ws[0].reset_mode, QuotaResetMode::CalendarDay);
        assert_eq!(ws[1].reset_mode, QuotaResetMode::CalendarHour);
    }

    #[test]
    fn parse_empty_and_garbage() {
        assert!(parse_quota_windows("").is_empty());
        assert!(parse_quota_windows("not json").is_empty());
        assert!(parse_quota_windows(r#"{"rate_limits":{"rpm":1}}"#).is_empty());
    }

    #[test]
    fn budget_key_stable_per_config() {
        let b = QuotaBudget::Tokens(120000.0);
        assert_eq!(b.key(18000.0), "18000|tokens|120000");
        assert_eq!(QuotaBudget::Usd(5.5).key(3600.0), "3600|usd|5.5");
        assert_eq!(QuotaBudget::Count(10.0).key(60.0), "60|count|10");
    }

    #[test]
    fn budget_priority_tokens_usd_count() {
        let w = QuotaWindow {
            enabled: true,
            reset_mode: QuotaResetMode::Sliding,
            window: 60.0,
            budget_tokens: None,
            budget_usd: Some(1.0),
            budget_count: Some(2.0),
            budget_points: None,
        };
        assert_eq!(w.budget(), Some(QuotaBudget::Usd(1.0)), "三值并存取首个非空（tokens > usd > count）");
    }
}
