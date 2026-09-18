//! `menubar` 纯逻辑单测（票 I10）。
//!
//! AppKit 对象本身要主线程 + 真 `NSApplication`，`cargo test` 里起不来，故只测**可测的**：
//! 菜单状态文案、兜底纯文本标题、屏幕坐标翻转。宿主接线（install/render）留给实机验收。

use super::{fallback_title, flip_y, status_text};
use crate::gateway::models::TrayColor;
use crate::tray_render::{TrayColumn, TrayLayout};

fn col(name: &str, value: &str) -> TrayColumn {
    TrayColumn {
        name: name.to_string(),
        value: value.to_string(),
        color: TrayColor::default(),
        font_size: 9.0,
        two_line: false,
        align: "left".to_string(),
        align_row2: None,
    }
}

#[test]
fn status_text_matches_legacy_tauri_menu_wording() {
    // 文案与原 build_tray_menu 逐字一致，换宿主不改用户所见。
    assert_eq!(status_text(true, 9890), "● Proxy Running :9890");
    assert_eq!(status_text(false, 9890), "○ Proxy Stopped");
    // 非默认端口照样带出来。
    assert_eq!(status_text(true, 1), "● Proxy Running :1");
}

#[test]
fn fallback_title_joins_columns_with_separator() {
    let layout = TrayLayout {
        columns: vec![col("今日", "1 tok"), col("Cache", "50%")],
        gaps: vec![None],
    };
    assert_eq!(fallback_title(&layout, "  "), "今日 1 tok  Cache 50%");
    assert_eq!(fallback_title(&layout, " | "), "今日 1 tok | Cache 50%");
}

#[test]
fn fallback_title_edge_cases() {
    let empty = TrayLayout {
        columns: vec![],
        gaps: vec![],
    };
    assert_eq!(fallback_title(&empty, "  "), "");
    let single = TrayLayout {
        columns: vec![col("花费", "$0.1")],
        gaps: vec![],
    };
    // 单列不加 separator。
    assert_eq!(fallback_title(&single, "  "), "花费 $0.1");
}

#[test]
fn flip_y_converts_bottom_left_origin_to_top_left() {
    // 1080 高屏幕、菜单栏条目贴顶（AppKit y = 1080-24 = 1056，高 24）→ 左上原点 y = 0。
    assert_eq!(flip_y(1080.0, 1056.0, 24.0), 0.0);
    // 贴底（y=0，高 24）→ 左上原点 y = 1056。
    assert_eq!(flip_y(1080.0, 0.0, 24.0), 1056.0);
    // 零高度退化：翻转后就是屏幕高减 y。
    assert_eq!(flip_y(1080.0, 100.0, 0.0), 980.0);
}
