//! 预估 coding plan JSON 模型 + 校准阈值常量

use serde::{Deserialize, Serialize};

/// 校准阈值：距上次真查超过 5min
pub const CALIBRATE_INTERVAL_MS: i64 = 300_000;
/// 校准阈值：自上次真查以来预估次数
pub const CALIBRATE_COUNT: i64 = 100;

/// 持久化于 `platform.est_coding_plan` 的预估状态
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EstCodingPlan {
    #[serde(default)]
    pub tiers: Vec<EstTier>,
    #[serde(default)]
    pub level: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EstTier {
    /// "five_hour" | "weekly_limit"
    pub name: String,
    /// 当前预估利用率 (0-100)
    pub est_utilization: f64,
    /// 方案 B 拟合系数：每 token 增加的利用率百分点（冷启动为 0 = 未知）
    #[serde(default)]
    pub coef_per_token: f64,
    /// 上次真查时的利用率（拟合基线）
    #[serde(default)]
    pub util_at_last_real: f64,
    /// 自上次真查以来累计 token（拟合分母）
    #[serde(default)]
    pub tokens_since_real: f64,
    /// 是否有绝对基数（Kimi limit/remaining → 精确预估）
    #[serde(default)]
    pub has_base: bool,
    /// 绝对配额上限（仅 has_base 时有意义）
    #[serde(default)]
    pub limit: f64,
    /// 本周期起点（unix ms）。真查拿到 resets_at 时落地 `window_start = resets_at - cycle`，
    /// 之后预估侧用 `window_start + cycle` 推算 remain（无 resets_at 时也能算「剩余可用时间%」配色）。
    /// 0 / 缺失 = 无可靠周期起点 → 配色退中性（usage_color，不静默走旧利用率阈值）。
    #[serde(default)]
    pub window_start: i64,
}

impl EstCodingPlan {
    pub fn from_json(s: &str) -> Self {
        if s.trim().is_empty() {
            return Self::default();
        }
        serde_json::from_str(s).unwrap_or_default()
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}
