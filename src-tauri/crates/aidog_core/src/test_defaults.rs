//! `defaults.rs` 的纯函数测试（票 10）。

use super::logo_mime_for_ext;
use crate::gateway::logo_sync::LOGO_CACHE_EXTS;

/// 缓存里可能出现的每一种扩展名都得能映射出 MIME。
///
/// 漏一个的后果是浏览器形态下那种格式的 logo 静默不显示（`get_protocol_logo_data_url`
/// 返空串，前端回落首字母圆圈），桌面形态一切正常 —— 正是那种只在一半形态里出现、
/// 又不报错的 bug。
#[test]
fn every_cached_logo_ext_has_a_mime() {
    for ext in LOGO_CACHE_EXTS {
        assert!(
            logo_mime_for_ext(Some(ext)).is_some(),
            "LOGO_CACHE_EXTS 里的 {ext} 在 logo_mime_for_ext 没有对应 MIME"
        );
    }
}

/// 无扩展名 / 不认识的扩展名不猜类型，返 None（调用方据此返空串）。
#[test]
fn unknown_ext_maps_to_none() {
    assert_eq!(logo_mime_for_ext(None), None);
    assert_eq!(logo_mime_for_ext(Some("exe")), None);
    assert_eq!(logo_mime_for_ext(Some("")), None);
}

/// svg 是最常见的一种（registry 的 simpleicons slug 下来就是 svg），锁一下具体值：
/// 写错 MIME 浏览器会拒渲染。
#[test]
fn svg_maps_to_image_svg_xml() {
    assert_eq!(logo_mime_for_ext(Some("svg")), Some("image/svg+xml"));
    assert_eq!(logo_mime_for_ext(Some("jpg")), Some("image/jpeg"));
}
