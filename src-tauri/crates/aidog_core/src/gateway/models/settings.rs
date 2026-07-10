//! KV 设置与各类全局配置：通用 KV / 统计 / 超时 / 出站代理 / 调度+熔断默认。

use super::{default_true, Platform, RoutingMode};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "test_settings.rs"]
mod test_settings;

// ─── Settings (KV) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SettingEntry {
    pub id: u64,
    pub scope: String,
    pub key: String,
    pub value: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct SetSettingInput {
    pub scope: String,
    pub key: String,
    pub value: serde_json::Value,
}

// ─── Stats Settings ─────────────────────────────────────────

/// 统计聚合表设置（settings 表 scope="stats" key="settings"）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSettings {
    /// stats_agg_hourly 聚合行保留天数；0 = 永久保留。默认 365。
    #[serde(default = "default_stats_retention_days")]
    pub retention_days: u32,
}

fn default_stats_retention_days() -> u32 { 365 }

impl Default for StatsSettings {
    fn default() -> Self {
        Self { retention_days: default_stats_retention_days() }
    }
}

// ─── Proxy Timeout Settings ─────────────────────────────────

/// Upstream request timeout configuration (stored in settings table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTimeoutSettings {
    /// Total request timeout in seconds (0 = no limit)
    #[serde(default)]
    pub request_timeout_secs: u64,
    /// TCP connection timeout in seconds (0 = no limit)
    #[serde(default)]
    pub connect_timeout_secs: u64,
}

impl Default for ProxyTimeoutSettings {
    fn default() -> Self {
        Self {
            request_timeout_secs: 300,  // 5 minutes
            connect_timeout_secs: 10,   // 10 seconds
        }
    }
}

// ─── Proxy Client Settings (upstream HTTP proxy) ──────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyClientSettings {
    #[serde(default)]
    pub enabled: bool,
    /// "socks5" | "http" | "https"
    #[serde(default = "default_proxy_type")]
    pub proxy_type: String,
    #[serde(default = "default_proxy_host")]
    pub host: String,
    #[serde(default = "default_proxy_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    /// SOCKS5 时 DNS 走代理解析 (socks5h vs socks5)
    #[serde(default = "default_true")]
    pub dns_over_proxy: bool,
}

fn default_proxy_type() -> String { "socks5".to_string() }
fn default_proxy_host() -> String { "127.0.0.1".to_string() }
fn default_proxy_port() -> u16 { 7890 }

impl Default for ProxyClientSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_type: default_proxy_type(),
            host: default_proxy_host(),
            port: default_proxy_port(),
            username: String::new(),
            password: String::new(),
            dns_over_proxy: true,
        }
    }
}

impl ProxyClientSettings {
    /// Build a reqwest::Proxy from settings. Returns None if not enabled.
    pub fn to_reqwest_proxy(&self) -> Option<reqwest::Proxy> {
        if !self.enabled { return None; }
        let scheme = match self.proxy_type.as_str() {
            "socks5" if self.dns_over_proxy => "socks5h",
            "socks5" => "socks5",
            "https" => "https",
            _ => "http",
        };
        let url = format!("{}://{}:{}", scheme, self.host, self.port);
        let mut proxy = reqwest::Proxy::all(&url)
            .map_err(|e| { tracing::warn!("invalid proxy URL {}: {e}", url); e })
            .ok()?;
        if !self.username.is_empty() {
            proxy = proxy.basic_auth(&self.username, &self.password);
        }
        Some(proxy)
    }
}

// ─── Scheduling & Breaker Settings ─────────────────────────

/// 全局调度 + 熔断默认设置（settings KV scope=`scheduling`, key=`settings`）。
/// Platform 的 `extra.breaker` 覆盖值为 0/缺省时继承本结构对应默认值。
/// `enabled=false` 时熔断总开关旁路（候选过滤不踢任何 Open 平台）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingBreakerSettings {
    /// 全局默认调度策略字面量（与 RoutingMode serde rename 对齐）；Group routing_mode 覆盖之。
    #[serde(default = "default_routing_mode_str")]
    pub default_routing_mode: String,
    /// 全局默认熔断失败阈值（连续失败达此数 → Open）。
    #[serde(default = "default_breaker_failure_threshold")]
    pub breaker_failure_threshold: u32,
    /// 全局默认 Open 持续秒数。
    #[serde(default = "default_breaker_open_secs")]
    pub breaker_open_secs: u64,
    /// 全局默认 HalfOpen 最大探测数。
    #[serde(default = "default_breaker_half_open_max")]
    pub breaker_half_open_max: u32,
    /// 熔断总开关。
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_routing_mode_str() -> String { "health_aware".to_string() }
fn default_breaker_failure_threshold() -> u32 { 5 }
fn default_breaker_open_secs() -> u64 { 60 }
fn default_breaker_half_open_max() -> u32 { 2 }

impl Default for SchedulingBreakerSettings {
    fn default() -> Self {
        Self {
            default_routing_mode: default_routing_mode_str(),
            breaker_failure_threshold: default_breaker_failure_threshold(),
            breaker_open_secs: default_breaker_open_secs(),
            breaker_half_open_max: default_breaker_half_open_max(),
            enabled: true,
        }
    }
}

impl SchedulingBreakerSettings {
    /// 解析某平台的有效熔断阈值：平台字段非 0 用之，否则全局默认。
    /// 返回 (failure_threshold, open_secs, half_open_max)。
    pub fn effective_thresholds(&self, platform: &Platform) -> (u32, u64, u32) {
        let b = platform.breaker();
        let ft = if b.failure_threshold > 0 {
            b.failure_threshold
        } else {
            self.breaker_failure_threshold
        };
        let os = if b.open_secs > 0 {
            b.open_secs
        } else {
            self.breaker_open_secs
        };
        let hom = if b.half_open_max > 0 {
            b.half_open_max
        } else {
            self.breaker_half_open_max
        };
        (ft.max(1), os.max(1), hom.max(1))
    }

    /// 全局默认调度策略解析为 RoutingMode（GB 创建 Group 时取初值）。
    #[allow(dead_code)]
    pub fn default_mode(&self) -> RoutingMode {
        RoutingMode::from_str_or_default(&self.default_routing_mode)
    }
}
