//! 思考档位（effort 档位名）↔ 思考预算（budget tokens）的**唯一**换算表，以及四协议出站决议。
//!
//! 四家协议用两种互不兼容的表达描述「思考多深」：
//! - 数字预算：Anthropic `thinking.budget_tokens`、Gemini `generationConfig.thinkingConfig.thinkingBudget`
//! - 档位名：OpenAI Chat `reasoning_effort`、OpenAI Responses `reasoning.effort`、
//!   Anthropic（Claude Code 2.x / pi）`output_config.effort`
//!
//! 跨协议转换必然要在两者间换算，本模块是全 crate 唯一一份换算表。
//!
//! # 换算依据
//!
//! **这是本仓自定的约定，不是任何厂商公布的映射。** OpenAI 官方把 `reasoning.effort` 定义为
//! 定性档位、明确不给对应 token 数（<https://developers.openai.com/api/docs/guides/reasoning>）；
//! Anthropic / Gemini 只收 token 数，同样不公布档位对应值。业界各家（OpenRouter 按
//! `max_tokens` 百分比、LiteLLM 用 1024）也各行其是，没有可引的公共标准。
//!
//! 本仓取值：`low = 4096` / `medium = 8192` / `high = 16384`，逐档翻倍；反查取档位上界（`<=`）。
//! 选这组数的理由是**往返幂等**：`effort → budget → effort` 与 `budget → effort → budget`
//! 都回到原值（4096 → low → 4096）。合并前本 crate 有三套互不自洽的表
//! （出站 openai `0..=4096/4097..=8192/_`、入站 openai `2048/8192/16384`、
//! 入站 responses `4096/8192/10000`），openai → responses 往返会把 low 抬成 medium。
//!
//! 未知档位名（`minimal` / `xhigh` / 厂商私有值）不猜数字：[`effort_to_budget`] 返 `None`，
//! 档位原值仍由 [`ThinkingMode::effort`] 原样透传给同样收档位名的目标协议。

use crate::types::{ChatRequest, ThinkingMode};

/// `low` 档对应的思考预算 tokens（本仓约定，见模块文档）
pub const EFFORT_LOW_BUDGET: u32 = 4096;
/// `medium` 档对应的思考预算 tokens（本仓约定，见模块文档）
pub const EFFORT_MEDIUM_BUDGET: u32 = 8192;
/// `high` 档对应的思考预算 tokens（本仓约定，见模块文档）
pub const EFFORT_HIGH_BUDGET: u32 = 16384;

/// Anthropic `thinking.type` 的显式禁用值。
const KIND_DISABLED: &str = "disabled";

/// 档位名 → 预算 tokens。未知档位不猜数字，返 `None`。
pub fn effort_to_budget(effort: &str) -> Option<u32> {
    match effort {
        "low" => Some(EFFORT_LOW_BUDGET),
        "medium" => Some(EFFORT_MEDIUM_BUDGET),
        "high" => Some(EFFORT_HIGH_BUDGET),
        _ => None,
    }
}

/// 预算 tokens → 档位名（取档位上界，与 [`effort_to_budget`] 互逆）。
pub fn budget_to_effort(budget: u32) -> &'static str {
    if budget <= EFFORT_LOW_BUDGET {
        "low"
    } else if budget <= EFFORT_MEDIUM_BUDGET {
        "medium"
    } else {
        "high"
    }
}

/// 入站：只有档位名的协议（OpenAI Chat `reasoning_effort` / Responses `reasoning.effort`）
/// 构造中立档位。
///
/// `none` 是这两家表达「不要思考」的官方值，归一成 Anthropic `thinking.type=disabled` 三态，
/// 四协议出站才有**同一个**禁用判据（[`is_disabled`]）。
pub fn mode_from_effort(effort: Option<&str>) -> Option<ThinkingMode> {
    match effort {
        Some("none") => Some(disabled_mode()),
        other => ThinkingMode::from_parts(None, other.map(str::to_string)),
    }
}

/// 入站：把各协议自己的「不要思考」表达（Responses/Chat 的 `effort:"none"`、
/// Gemini 的 `thinkingBudget: 0`）归一成中立三态的显式禁用。
pub fn disabled_mode() -> ThinkingMode {
    ThinkingMode {
        kind: Some(KIND_DISABLED.to_string()),
        effort: None,
    }
}

/// 客户端是否**显式**要求禁用思考（Anthropic `thinking.type = "disabled"`）。
///
/// 优先级：显式禁用一票否决任何档位/预算。平台侧的 aidog `disable_thinking` 开关是另一条
/// 通道，由 `forward.rs::apply_disable_thinking` 在转换之后无条件剔除思考参数再写显式禁用，
/// 两条通道叠加时仍是禁用胜出。
pub fn is_disabled(req: &ChatRequest) -> bool {
    req.thinking_mode
        .as_ref()
        .and_then(|m| m.kind.as_deref())
        .is_some_and(|k| k == KIND_DISABLED)
}

/// 出站决议：目标协议收 token 数时用哪个预算。
///
/// 优先级：显式禁用 → `None`；客户端给的数字预算 → 原样；否则档位换算。
pub fn outbound_budget(req: &ChatRequest) -> Option<u32> {
    if is_disabled(req) {
        return None;
    }
    req.thinking_budget.or_else(|| {
        req.thinking_mode
            .as_ref()
            .and_then(|m| m.effort.as_deref())
            .and_then(effort_to_budget)
    })
}

/// 出站决议：目标协议收档位名时写哪个档位名。
///
/// 优先级：显式禁用 → `None`；客户端给的档位**原值** → 原样（`xhigh` 这类厂商私有档位不被
/// 换算表压成三档）；否则由数字预算反查。
pub fn outbound_effort(req: &ChatRequest) -> Option<String> {
    if is_disabled(req) {
        return None;
    }
    req.thinking_mode
        .as_ref()
        .and_then(|m| m.effort.clone())
        .or_else(|| req.thinking_budget.map(|b| budget_to_effort(b).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_budget_roundtrip_is_idempotent() {
        for e in ["low", "medium", "high"] {
            let b = effort_to_budget(e).expect("三档必有预算");
            assert_eq!(budget_to_effort(b), e, "{e} 档往返漂移");
        }
        for b in [EFFORT_LOW_BUDGET, EFFORT_MEDIUM_BUDGET, EFFORT_HIGH_BUDGET] {
            assert_eq!(
                effort_to_budget(budget_to_effort(b)),
                Some(b),
                "{b} 预算往返漂移"
            );
        }
    }

    #[test]
    fn unknown_effort_yields_no_budget() {
        assert_eq!(effort_to_budget("xhigh"), None);
        assert_eq!(effort_to_budget("minimal"), None);
    }

    #[test]
    fn budget_to_effort_boundaries() {
        assert_eq!(budget_to_effort(0), "low");
        assert_eq!(budget_to_effort(4097), "medium");
        assert_eq!(budget_to_effort(8193), "high");
    }
}
