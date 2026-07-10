use super::*;
use crate::gateway::models::{NotifForm, NotifType};
use std::collections::HashMap;

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn channels_per_form() {
    assert_eq!(
        channels_for_form(NotifForm::Full),
        Channels { tts: true, popup: true, sound: true, inbox: true }
    );
    assert_eq!(
        channels_for_form(NotifForm::PopupOnly),
        Channels { tts: false, popup: true, sound: false, inbox: true }
    );
    assert_eq!(
        channels_for_form(NotifForm::InboxOnly),
        Channels { tts: false, popup: false, sound: false, inbox: true }
    );
    assert_eq!(
        channels_for_form(NotifForm::SoundOnly),
        Channels { tts: false, popup: false, sound: true, inbox: false }
    );
}

#[test]
fn render_template_priority() {
    let v = vars(&[("project", "aidog")]);
    // template 优先；title 现取 vars["project"]
    let (title, body) = render(NotifType::TaskComplete, "{project} 完成", Some("ignored"), &v);
    assert_eq!(title, "aidog");
    assert_eq!(body, "aidog 完成");
    // template 空 → content
    let (_, body2) = render(NotifType::Error, "", Some("构建失败 {project}"), &v);
    assert_eq!(body2, "构建失败 aidog");
    // 都空 + 有 project → default_template 兜底
    let (_, body3) = render(NotifType::WaitingInput, "", None, &v);
    assert_eq!(body3, "aidog 等待用户输入");
}

#[test]
fn render_body_uses_default_template_when_project_present() {
    let v = vars(&[("project", "aidog")]);
    // 3 类型 default_template 兜底
    let (_, b1) = render(NotifType::TaskComplete, "", None, &v);
    assert_eq!(b1, "aidog 完成");
    let (_, b2) = render(NotifType::WaitingInput, "", None, &v);
    assert_eq!(b2, "aidog 等待用户输入");
    let (_, b3) = render(NotifType::Error, "", None, &v);
    assert_eq!(b3, "aidog 出错");
}

#[test]
fn render_body_uses_default_template_with_brand_fallback_when_no_project() {
    // 无 project 空模板 → 仍渲染 default_template，`{project}` 用品牌兜底 "aidog"
    // 核心新行为：不再退化到英文 default_title，也不泄漏 `{project}` 字面。
    let v = HashMap::new();
    for (t, expect) in [
        (NotifType::TaskComplete, "aidog 完成"),
        (NotifType::WaitingInput, "aidog 等待用户输入"),
        (NotifType::Error, "aidog 出错"),
    ] {
        let body = render(t, "", None, &v).1;
        assert_eq!(body, expect);
        assert!(!body.contains("{project}"), "残留 {{project}} 字面: {body}");
        assert_ne!(body, default_title(t), "不应退化到英文 default_title");
    }

    // project 仅空白也按无 project 处理 → 品牌兜底
    let v_blank = vars(&[("project", "   ")]);
    assert_eq!(render(NotifType::Error, "", None, &v_blank).1, "aidog 出错");
}

#[test]
fn render_title_is_project_name() {
    // 有 project：title = project（不是 default_title）
    let v = vars(&[("project", "aidog")]);
    let (title, _) = render(NotifType::Error, "", None, &v);
    assert_eq!(title, "aidog");

    // project 含周围空白 → trim
    let v2 = vars(&[("project", "  myproj  ")]);
    let (title2, _) = render(NotifType::WaitingInput, "", None, &v2);
    assert_eq!(title2, "myproj");
}

#[test]
fn render_title_empty_when_no_project() {
    // vars 无 project → title 空字符串（前端 fallback typeLabel；弹窗 dispatch 内 fallback default_title）
    let v = HashMap::new();
    let (title, body) = render(NotifType::TaskComplete, "", None, &v);
    assert_eq!(title, "");
    assert_eq!(body, "aidog 完成"); // body 用 default_template + 品牌兜底

    // project 值为空字符串 → 视同无
    let v2 = vars(&[("project", "")]);
    let (title2, _) = render(NotifType::Error, "", None, &v2);
    assert_eq!(title2, "");

    // project 仅空白 → trim 后空 → 视同无
    let v3 = vars(&[("project", "   ")]);
    let (title3, _) = render(NotifType::Error, "", None, &v3);
    assert_eq!(title3, "");
}
