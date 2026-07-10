//! 时段模型配置（time_models）：按时段窗口切换主力模型档。
//!
//! 数据源：`platform.extra.time_models`（用户级配置，preset 不带）。
//! 路由按当前时段 first-match 命中 → 用该时段 models 替换 platform.models 调 resolve_model；
//! 未命中 → platform.models default。

use crate::gateway::models::PlatformModels;

/// 从 `platform.extra` JSON 字符串解析 `time_models` 字段；非法 / 缺失 → 空。
pub fn parse_platform_time_models(extra: &str) -> Vec<serde_json::Value> {
    if extra.trim().is_empty() {
        return Vec::new();
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(extra) else {
        return Vec::new();
    };
    let Some(arr) = v.get("time_models") else {
        return Vec::new();
    };
    if let Some(a) = arr.as_array() {
        // 验证每个元素都有 windows 和 models 字段
        a.iter()
            .filter(|item| item.is_object() && item.get("windows").is_some() && item.get("models").is_some())
            .cloned()
            .collect()
    } else {
        Vec::new()
    }
}

/// 按当前时段（epoch_ms UTC）first-match 命中 time_models，返回对应 models；
/// 未命中 → 返回 default_models（platform.models）。
pub fn resolve_time_models(
    rules: &[serde_json::Value],
    default_models: &PlatformModels,
    epoch_ms: i64,
) -> PlatformModels {
    if rules.is_empty() {
        return default_models.clone();
    }

    // 复用 peak_hours 的 utc_time 和 hit 判定（含 minute + day_of_month）
    let (hour, minute, weekday, day_of_month) = crate::gateway::peak_hours::utc_time(epoch_ms);

    // first-match: 遍历 rules，找到第一个命中的窗口
    for rule in rules {
        if let Some(windows) = rule.get("windows").and_then(|w| w.as_array()) {
            // 转换窗口为 PeakWindow 格式进行判定
            for w in windows {
                if let Some(parsed) = parse_peak_window(w) {
                    if crate::gateway::peak_hours::hit(&parsed, hour, minute, weekday, day_of_month) {
                        // 命中：解析该 rule 的 models
                        if let Some(models_val) = rule.get("models") {
                            if let Ok(models) = serde_json::from_value::<PlatformModels>(models_val.clone()) {
                                return models;
                            }
                        }
                    }
                }
            }
        }
    }

    // 未命中：返回默认
    default_models.clone()
}

/// 解析单个窗口（从 JSON Value），失败返回 None。支持 minute + day_of_month（向后兼容旧 schema）。
fn parse_peak_window(v: &serde_json::Value) -> Option<crate::gateway::peak_hours::PeakWindow> {
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct WindowHelper {
        start_hour: i32,
        end_hour: i32,
        #[serde(default)]
        days_of_week: Option<Vec<i32>>,
        #[serde(default)]
        start_minute: Option<i32>,
        #[serde(default)]
        end_minute: Option<i32>,
        #[serde(default)]
        days_of_month: Option<Vec<i32>>,
    }
    let helper: WindowHelper = serde_json::from_value(v.clone()).ok()?;
    // multiplier 字段不需要，设 1.0；time_models 仅用时间维度切换 models，model scope 不参与
    // （peak_hours 的 model scope 过滤语义不适用 time_models，故 models 字段不解析 / 不传递到 hit）
    Some(crate::gateway::peak_hours::PeakWindow {
        start_hour: helper.start_hour,
        end_hour: helper.end_hour,
        multiplier: 1.0,
        days_of_week: helper.days_of_week,
        start_minute: helper.start_minute,
        end_minute: helper.end_minute,
        days_of_month: helper.days_of_month,
        models: None,
        start_at: None,
        end_at: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extra_empty_returns_empty() {
        assert!(parse_platform_time_models("").is_empty());
        assert!(parse_platform_time_models("not-json").is_empty());
        assert!(parse_platform_time_models("{}").is_empty());
    }

    #[test]
    fn parse_extra_time_models_valid() {
        let extra = r#"{
            "time_models": [
                {
                    "windows": [{"start_hour": 14, "end_hour": 22, "days_of_week": [1,2,3,4,5]}],
                    "models": {"default": "gpt-4o", "sonnet": "claude-sonnet-4-20250514"}
                }
            ]
        }"#;
        let rules = parse_platform_time_models(extra);
        assert_eq!(rules.len(), 1);
        assert!(rules[0].get("windows").is_some());
        assert!(rules[0].get("models").is_some());
    }

    #[test]
    fn parse_extra_time_models_invalid_windows_filtered() {
        // 缺少 windows 或 models 的 rule 应被过滤
        let extra = r#"{
            "time_models": [
                {"windows": [{"start_hour": 14, "end_hour": 22}]},
                {"models": {"default": "gpt-4o"}},
                {"windows": [], "models": {"default": "gpt-4o"}}
            ]
        }"#;
        let rules = parse_platform_time_models(extra);
        // 第三个 rule（空 windows 但有 models）会保留
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn resolve_time_models_empty_returns_default() {
        let default = PlatformModels {
            default: Some("default-model".into()),
            ..Default::default()
        };
        let result = resolve_time_models(&[], &default, 1_700_000_000_000);
        assert_eq!(result.default, Some("default-model".into()));
    }

    #[test]
    fn resolve_time_models_no_match_returns_default() {
        let default = PlatformModels {
            default: Some("default-model".into()),
            ..Default::default()
        };
        // 规则：14-22 点，但测试时间是 0 点（不命中）
        let rule = serde_json::json!({
            "windows": [{"start_hour": 14, "end_hour": 22}],
            "models": {"default": "peak-model"}
        });
        let result = resolve_time_models(&[rule], &default, 1_700_000_000_000);
        assert_eq!(result.default, Some("default-model".into()));
    }

    #[test]
    fn resolve_time_models_match_returns_rule_models() {
        let default = PlatformModels {
            default: Some("default-model".into()),
            sonnet: Some("default-sonnet".into()),
            ..Default::default()
        };
        // 规则：全天 0-24 点，必然命中
        let rule = serde_json::json!({
            "windows": [{"start_hour": 0, "end_hour": 24}],
            "models": {"default": "peak-model", "sonnet": "peak-sonnet"}
        });
        let result = resolve_time_models(&[rule], &default, 1_700_000_000_000);
        assert_eq!(result.default, Some("peak-model".into()));
        assert_eq!(result.sonnet, Some("peak-sonnet".into()));
    }

    #[test]
    fn resolve_time_models_first_match_wins() {
        let default = PlatformModels {
            default: Some("default-model".into()),
            ..Default::default()
        };
        // 两条规则：第一个 0-12，第二个 0-24
        // 测试时间 8 点 → 同时命中两条规则，应取第一条（first-match）
        let rule1 = serde_json::json!({
            "windows": [{"start_hour": 0, "end_hour": 12}],
            "models": {"default": "first-match"}
        });
        let rule2 = serde_json::json!({
            "windows": [{"start_hour": 0, "end_hour": 24}],
            "models": {"default": "second-match"}
        });
        // 2024-01-01T08:00:00Z（周一）→ timestamp 1704096000
        let ms = 1704096000 * 1000;
        let result = resolve_time_models(&[rule1, rule2], &default, ms);
        assert_eq!(result.default, Some("first-match".into()));
    }

    #[test]
    fn resolve_time_models_cross_midnight() {
        let default = PlatformModels {
            default: Some("default-model".into()),
            ..Default::default()
        };
        // 跨天规则：22-6 点
        let rule = serde_json::json!({
            "windows": [{"start_hour": 22, "end_hour": 6}],
            "models": {"default": "overnight-model"}
        });
        // 2024-01-01T23:00:00Z（周一 23 点）→ 应命中
        let ms = (1704067200 + 23 * 3600) * 1000;
        let result = resolve_time_models(&[rule.clone()], &default, ms);
        assert_eq!(result.default, Some("overnight-model".into()));

        // 2024-01-01T15:00:00Z（周一 15 点）→ 不命中
        let ms2 = (1704067200 + 15 * 3600) * 1000;
        let result2 = resolve_time_models(&[rule], &default, ms2);
        assert_eq!(result2.default, Some("default-model".into()));
    }

    #[test]
    fn resolve_time_models_days_of_week_filter() {
        let default = PlatformModels {
            default: Some("default-model".into()),
            ..Default::default()
        };
        // 仅周末（0=周日，6=周六）
        let rule = serde_json::json!({
            "windows": [{"start_hour": 0, "end_hour": 24, "days_of_week": [0, 6]}],
            "models": {"default": "weekend-model"}
        });
        // 2024-01-07T02:50:00Z（周日）→ 应命中
        let ms = 1704595800 * 1000;
        let result = resolve_time_models(&[rule.clone()], &default, ms);
        assert_eq!(result.default, Some("weekend-model".into()));

        // 2024-01-01T08:00:00Z（周一）→ 不命中
        let ms2 = 1704105600 * 1000;
        let result2 = resolve_time_models(&[rule], &default, ms2);
        assert_eq!(result2.default, Some("default-model".into()));
    }
}
