//! 通道解析 + 标题/正文渲染 + 分发结果结构。

use std::collections::HashMap;

use super::super::models::{NotifForm, NotifType};
use super::vars::substitute_vars;

/// 分发选定通道（按 form 解析；纯函数便于单测）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channels {
    pub tts: bool,
    pub popup: bool,
    pub sound: bool,
    pub inbox: bool,
}

/// 按 form 解析启用的通道。
///
/// - Full       → TTS + popup + sound + inbox
/// - PopupOnly  → popup + inbox（弹窗类落库便于回看）
/// - InboxOnly  → inbox
/// - SoundOnly  → sound
///
/// TTS 还需 setting.tts && settings.tts_enabled 才真正播报（在 dispatch 内取与）。
pub fn channels_for_form(form: NotifForm) -> Channels {
    match form {
        NotifForm::Full => Channels { tts: true, popup: true, sound: true, inbox: true },
        NotifForm::PopupOnly => Channels { tts: false, popup: true, sound: false, inbox: true },
        NotifForm::InboxOnly => Channels { tts: false, popup: false, sound: false, inbox: true },
        NotifForm::SoundOnly => Channels { tts: false, popup: false, sound: true, inbox: false },
    }
}

/// 分发结果（用于端点返回 / 测试断言；不含副作用细节）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DispatchResult {
    /// 总开关 OFF 或被旁路 → false。
    pub dispatched: bool,
    /// 实际渲染后的标题。
    pub title: String,
    /// 实际渲染后的正文。
    pub body: String,
    /// 实际启用的通道（取与设置后）。
    pub tts: bool,
    pub popup: bool,
    pub sound: bool,
    pub inbox: bool,
    /// 收件箱落库行 id（inbox 通道启用时）。
    pub inbox_id: Option<i64>,
}

/// 无 project 时，default_template 渲染用的品牌兜底名（避免 `{project}` 字面泄漏）。
/// render 为纯函数无 AppHandle，用常量最简且稳定（不取 product_name）。
pub(crate) const BRAND_FALLBACK: &str = "aidog";

/// 渲染 title/body：
/// - **title 字段语义 = 项目名**（取 `vars["project"]`，hook 脚本注入的 cwd basename）。
///   无 project（vars 未带或值空）→ 空字符串，前端 fallback 到类型 i18n 标签。
///   弹窗标题在 dispatch 内若 title 空则用 default_title 兜底。
/// - body 兜底链：setting.template > content > default_template > default_title（末位兜底）。
///   template+content 都空时无论有无 project 都渲染 default_template；无 project 时给 `{project}`
///   注入品牌兜底名（`aidog`），故得「aidog 完成」而非空串 / `{project}` 字面 / 英文退化。
pub(crate) fn render(
    notif_type: NotifType,
    template: &str,
    content: Option<&str>,
    vars: &HashMap<String, String>,
) -> (String, String) {
    let title = vars
        .get("project")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    let raw_body = if !template.is_empty() {
        template
    } else {
        content.unwrap_or_default()
    };
    let body = if raw_body.is_empty() {
        let dt = notif_type.default_template();
        if dt.is_empty() {
            // default_template 理论空时最末兜底，避免空串。
            substitute_vars(default_title(notif_type), vars)
        } else {
            // 无 project 时注入品牌兜底名，杜绝 `{project}` 字面残留。
            let has_project = vars
                .get("project")
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if has_project {
                substitute_vars(dt, vars)
            } else {
                let mut vars2 = vars.clone();
                vars2.insert("project".to_string(), BRAND_FALLBACK.to_string());
                substitute_vars(dt, &vars2)
            }
        }
    } else {
        substitute_vars(raw_body, vars)
    };
    (substitute_vars(&title, vars), body)
}

/// 类型默认标题（英文中性，前端按 i18n 展示，此处仅弹窗/收件箱兜底）。
pub(crate) fn default_title(t: NotifType) -> &'static str {
    match t {
        NotifType::TaskComplete => "Task Complete",
        NotifType::WaitingInput => "Waiting for Input",
        NotifType::Error => "Error",
    }
}
