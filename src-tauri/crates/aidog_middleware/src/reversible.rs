//! 可还原 mask（Reversible Mask）执行：在最终出站 wire body 的文本槽上，
//! 把规则命中片段换成确定性占位符 `{{PII_<TYPE>_<8hex>}}` 并登记映射。
//!
//! 与 Fixed mask 的差异：固定 mask 在 chat_req / 透传 Value 层执行（见 inbound.rs，
//! 本类步骤在那里被跳过）；可还原 mask 只在协议转换之后的最终 body 上执行一次，
//! 转换不会把原文带回，且占位符不会再被转换层改写。
//! 已是占位符的片段对本趟所有后续规则不可见（匹配与替换都按占位符边界切段）。

use std::sync::atomic::{AtomicBool, Ordering};

use aidog_db::models::{ActionKind, MaskMode, MiddlewareSettings, Target};

use crate::inbound::InboundTexts;
use crate::placeholder::{ScrubMap, PLACEHOLDER_RE};
use crate::{CompiledRule, EvalView, MiddlewareEngine, Mount, RewriteLeaf, collect_patterns, validators};
use aidog_db::models::MatchType;

static FORCE_FAIL: AtomicBool = AtomicBool::new(false);

/// 测试缝：置位后，作用域内存在可还原规则的请求应用时报错（fail-closed 路径）。
pub fn set_reversible_force_fail(on: bool) {
    FORCE_FAIL.store(on, Ordering::SeqCst);
}

/// 规则是否含可还原 mask 步骤。
pub(crate) fn has_reversible(cr: &CompiledRule) -> bool {
    cr.rule.actions.iter().any(is_reversible_step)
}

pub(crate) fn is_reversible_step(s: &aidog_db::models::ActionStep) -> bool {
    s.kind == ActionKind::Mask && s.params.mode == MaskMode::Reversible
}

/// 占位符 TYPE 段：按校验器给语义类型，其余通用 `MASK`。
fn kind_slug(leaf: &RewriteLeaf) -> &'static str {
    match leaf.validator.as_str() {
        "luhn" => "BANK_CARD",
        "cn_id" => "ID_NUMBER",
        "iban" => "IBAN",
        _ => "MASK",
    }
}

/// 在非占位符片段上做替换；占位符片段原样保留。
fn replace_outside_placeholders(s: &str, f: &mut dyn FnMut(&str) -> String) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for m in PLACEHOLDER_RE.find_iter(s) {
        out.push_str(&f(&s[last..m.start()]));
        out.push_str(m.as_str());
        last = m.end();
    }
    out.push_str(&f(&s[last..]));
    out
}

fn replace_segment(leaf: &RewriteLeaf, seg: &str, map: &mut ScrubMap) -> String {
    let kind = kind_slug(leaf);
    let ok = |hit: &str| !hit.is_empty() && validators::validate(&leaf.validator, hit);
    match leaf.match_type {
        MatchType::Regex => match leaf.regex.as_ref() {
            Some(re) => re
                .replace_all(seg, |caps: &regex::Captures| {
                    let hit = &caps[0];
                    if ok(hit) { map.register(kind, hit) } else { hit.to_string() }
                })
                .into_owned(),
            None => seg.to_string(),
        },
        MatchType::Contains => {
            if ok(&leaf.pattern) && seg.contains(&leaf.pattern) {
                seg.replace(&leaf.pattern, &map.register(kind, &leaf.pattern))
            } else {
                seg.to_string()
            }
        }
        MatchType::Exact => {
            if ok(&leaf.pattern) && seg == leaf.pattern {
                map.register(kind, seg)
            } else {
                seg.to_string()
            }
        }
    }
}

fn aggregate_visible(texts: &InboundTexts<'_>) -> String {
    let mut buf = String::new();
    for t in texts.system.iter().chain(texts.messages.iter()) {
        buf.push_str(&PLACEHOLDER_RE.replace_all(t, ""));
        buf.push('\n');
    }
    buf
}

impl MiddlewareEngine {
    /// 可还原 mask 执行。返回本请求的映射表（无命中为空表）；`Err` = 应用失败，
    /// 调用方必须 fail-closed（503、不转发）。
    ///
    /// 只处理 `mode = reversible` 的 mask 步骤；其余步骤已在 chat_req / Value 层执行。
    /// `fields` 限定 messages / system（空 = 两者）。
    pub fn apply_reversible_texts(
        &self,
        settings: &MiddlewareSettings,
        texts: &mut InboundTexts<'_>,
        group_key: Option<&str>,
        platform_id: i64,
        req_headers: Option<&str>,
    ) -> Result<ScrubMap, String> {
        let mut map = ScrubMap::new();
        if !settings.enabled {
            return Ok(map);
        }
        let rules: Vec<CompiledRule> = self
            .request_rules_checked(Mount::Any, group_key, Some(platform_id), texts.model)?
            .into_iter()
            .filter(has_reversible)
            .collect();
        if rules.is_empty() {
            return Ok(map);
        }
        if FORCE_FAIL.load(Ordering::SeqCst) {
            return Err("reversible mask forced failure".into());
        }
        for cr in rules {
            let matched = {
                let view = EvalView {
                    req_text: aggregate_visible(texts),
                    req_headers,
                    model: texts.model,
                    ..Default::default()
                };
                cr.conditions.eval(&view)
            };
            if !matched {
                continue;
            }
            let leaves = collect_patterns(&cr.conditions, Target::RequestBody);
            for step in cr.rule.actions.iter().filter(|s| is_reversible_step(s)) {
                let fields = &step.params.fields;
                let touch_system = fields.is_empty() || fields.iter().any(|f| f == "system");
                let touch_messages = fields.is_empty() || fields.iter().any(|f| f == "messages");
                let mut apply = |s: &str| -> String {
                    let mut out = s.to_string();
                    for leaf in &leaves {
                        out = replace_outside_placeholders(&out, &mut |seg| {
                            replace_segment(leaf, seg, &mut map)
                        });
                    }
                    out
                };
                if touch_system {
                    for t in texts.system.iter_mut() {
                        *t = apply(t);
                    }
                }
                if touch_messages {
                    for t in texts.messages.iter_mut() {
                        *t = apply(t);
                    }
                }
            }
        }
        Ok(map)
    }
}
