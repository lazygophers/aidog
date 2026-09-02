use crate::gateway;
use crate::shared::*;
use aidog_db::{self as db, Db};
use gateway::models::*;
use std::future::Future;
use std::pin::Pin;
use tauri::menu::{MenuBuilder, MenuItemBuilder};

// TrayColumn / TrayLayout / TRAY_FONT_SIZE 数据类型 + build_tray_menu / tray_layout /
// tray_separator 等 UI 构造函数 + TrayMenuBuildImpl 全部下沉 core（C3 c3-commands 第 1 批，
// 原 commands_tray::tray 迁入）。TrayMenuBuild trait 原为防 core→commands 反向依赖循环而设
// 的注入点，crate 合并后该循环风险已消失，但保留 trait 间接层是纯搬运的选择（不做设计改动）。

/// 托盘单列：name（标签）+ value（值）+ 颜色（三态）+ 字号 + two_line（该列是否两行展示）。
#[derive(Debug, Clone)]
pub struct TrayColumn {
    pub name: String,
    pub value: String,
    pub color: TrayColor,
    // 以下 4 字段为 macOS 富文本渲染（set_tray_attributed_title）专属参数；
    // 非 macOS 走 fallback 纯文本路径不读取，故平台条件 allow(dead_code)。
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub font_size: f64,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub two_line: bool,
    /// "left" | "center" | "right"
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub align: String,
    /// 两行模式第二行对齐，None = 跟随 align
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub align_row2: Option<String>,
}

/// 托盘渲染布局：columns（数据列）+ gaps（列间间隙）。
/// gaps[i] = columns[i] 与 columns[i+1] 之间的间隙；None = 默认 2px 空白。
pub struct TrayLayout {
    pub columns: Vec<TrayColumn>,
    /// 长度 = columns.len() - 1（若 columns.len() ≥ 2）。
    /// None = 默认空白间隙；Some(text) = 自定义分隔符文本。
    pub gaps: Vec<Option<String>>,
}

#[cfg(target_os = "macos")]
pub const TRAY_FONT_SIZE: f64 = 9.0;

/// UI 构造注入点：refresh_tray_menu 需要的 3 个 UI 辅助函数（build_tray_menu /
/// tray_layout / tray_separator）由 commands::tray 层（root 过渡 → C8 commands-tray）
/// 实现，避免 core 反向依赖 commands crate（循环）。
pub trait TrayMenuBuild: Sync {
    /// 构造菜单（proxy 状态 / quota 详情 / show+quit 项）。
    fn build_menu<'a>(
        &'a self,
        app: &'a tauri::AppHandle,
    ) -> Pin<Box<dyn Future<Output = Result<tauri::menu::Menu<tauri::Wry>, String>> + Send + 'a>>;

    /// 取当前 enabled + ordered 的渲染布局（数据列 + gaps）。
    /// 票 06：只读 Db，故不再收 `AppHandle`（实现自 `AppCtx` 取）。
    fn layout<'a>(&'a self) -> Pin<Box<dyn Future<Output = TrayLayout> + Send + 'a>>;

    /// 配置的 separator（多 item 横排间隔）。
    fn separator<'a>(&'a self) -> Pin<Box<dyn Future<Output = String> + Send + 'a>>;
}

#[cfg(target_os = "macos")]
pub(crate) fn resolve_tray_color(color: &TrayColor) -> objc2::rc::Retained<objc2_app_kit::NSColor> {
    use objc2_app_kit::NSColor;
    match color.mode.as_str() {
        "preset" => match color.value.as_str() {
            "red" => NSColor::systemRedColor(),
            "green" => NSColor::systemGreenColor(),
            "orange" => NSColor::systemOrangeColor(),
            _ => NSColor::labelColor(),
        },
        "custom" => {
            let hex = color.value.trim().trim_start_matches('#');
            if hex.len() == 6
                && let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                )
            {
                return NSColor::colorWithSRGBRed_green_blue_alpha(
                    r as f64 / 255.0,
                    g as f64 / 255.0,
                    b as f64 / 255.0,
                    1.0,
                );
            }
            NSColor::labelColor()
        }
        // "follow" 及未知 → labelColor
        _ => NSColor::labelColor(),
    }
}

/// 估算列宽（pt）：以最长一行字符数 × 估字宽 + padding。
/// menuBarFont 近似等宽（CJK 全角约 1 字宽 = fontSize，ASCII 半角约 fontSize*0.6）。
/// 精确测量文本渲染宽度：用 AppKit sizeWithAttributes 返回实际像素宽。
/// 需要 MainThread（AppKit 要求），调用方已在主线程闭包内。
#[cfg(target_os = "macos")]
pub(crate) fn measure_text_width(text: &str, font_size: f64) -> f64 {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSFont, NSFontAttributeName, NSStringDrawing};
    use objc2_foundation::{NSDictionary, NSString};

    let ns_text = NSString::from_str(text);
    let font = NSFont::boldSystemFontOfSize(font_size);
    let font_key: &NSString = unsafe { NSFontAttributeName };
    let font_obj: &AnyObject = (*font).as_ref();
    let attrs: Retained<NSDictionary<NSString, AnyObject>> =
        NSDictionary::from_slices(&[font_key], &[font_obj]);
    // SAFETY: attrs 类型正确（NSFontAttributeName → NSFont）。
    unsafe { ns_text.sizeWithAttributes(Some(&attrs)) }.width
}

/// macOS：用 attributedTitle 给 tray button 设多列小字（每列独立颜色/字号）。
/// Tauri/tray-icon 的 set_title 走 button.setTitle(NSString) 无字号/颜色控制，故直连 NSStatusItem button。
/// 通过 tauri TrayIcon::with_inner_tray_icon 拿 tray_icon::TrayIcon，再 ns_status_item() 取底层 NSStatusItem。
/// 闭包在主线程执行（with_inner_tray_icon 保证），满足 AppKit 主线程约束。
///
/// 布局（iStat Menus 式）：
/// - 有任一 two_line 列 → **两行多列模式**：
///   - 第一行各列：two_line→name；single→"name value"
///   - 第二行各列：two_line→value；single→""（占位，tab 推进保持列对齐）
///   - 列间 `\t`，行间一个 `\n`；NSParagraphStyle.tabStops 每列一个 NSTextTab(left, 累加列宽)
///   - per-column 着色/字号：逐 cell 用 make_part 构造带 attributes 的子串 append，
///     tab/换行字符用 follow 颜色（无 range:setAttributes，规避 utf16 偏移坑）。
/// - 无 two_line 列 → **单行模式**：沿用 separator 横排拼接（无回归）。
///   整串套用同一 NSParagraphStyle（tabStops + 固定行高居中）+ baselineOffset 垂直居中。
#[cfg(target_os = "macos")]
pub(crate) fn set_tray_attributed_title(
    tray: &tauri::tray::TrayIcon,
    columns: Vec<TrayColumn>,
    gaps: Vec<Option<String>>,
    _separator: String,
) -> Result<(), String> {
    use objc2::AnyThread;
    use objc2::rc::Retained;
    use objc2_app_kit::NSBaselineOffsetAttributeName;
    use objc2_app_kit::{
        NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSParagraphStyleAttributeName,
    };
    use objc2_app_kit::{NSMutableParagraphStyle, NSTextAlignment, NSTextTab, NSTextTabType};
    use objc2_foundation::{
        NSArray, NSAttributedString, NSDictionary, NSMutableAttributedString, NSNumber, NSString,
    };

    tray.with_inner_tray_icon(move |inner| -> Result<(), String> {
        // SAFETY: with_inner_tray_icon 在主线程执行闭包，AppKit 调用满足主线程要求。
        let status_item = inner
            .ns_status_item()
            .ok_or_else(|| "ns_status_item unavailable".to_string())?;
        // MainThreadMarker：闭包已在主线程，断言获取。
        let mtm = objc2_foundation::MainThreadMarker::new()
            .ok_or_else(|| "not on main thread".to_string())?;
        let button = status_item
            .button(mtm)
            .ok_or_else(|| "status item has no button".to_string())?;

        let two_line_mode = columns.iter().any(|c| c.two_line);

        // 段落样式：两行模式压缩行高（min==max）让两行紧凑；单行模式不压缩，字号更大。
        // 两行：9pt × 2 行 ≈ 20pt，贴近菜单栏 ~22pt 高度。
        // 单行：13pt × 1 行，充分利用菜单栏垂直空间。
        let para = NSMutableParagraphStyle::new();
        // 两行模式用左对齐（tabStops 控制列位置）；单行模式居中。
        para.setAlignment(if two_line_mode {
            NSTextAlignment::Left
        } else {
            NSTextAlignment::Center
        });
        let line_h = if two_line_mode {
            TRAY_FONT_SIZE + 5.0 // 两行模式，行间距 10px
        } else {
            0.0 // 单行不压缩行高，使用系统默认
        };
        if two_line_mode {
            para.setMinimumLineHeight(line_h);
            para.setMaximumLineHeight(line_h);
            para.setLineSpacing(0.0);
        }

        // 两行模式：两行共用同一个段落样式（para），均使用 LeftTabStopType。
        // 列宽 = max(第一行该列文字, 第二行该列文字) 实测宽 + padding；位置累加（loc = 各列右边界）。
        // 对齐：通过在文本前填充空格实现右/居中对齐（精确测量 + 空格宽度推算）。
        // 两行都用 left tab @列右边界 → 同一列两行起始位置相同 → 列边界对齐。
        let mut col_widths: Vec<f64> = Vec::new();
        if two_line_mode {
            const COL_PADDING: f64 = 5.0; // 列间最小间距 5px
            let mut left_tabs: Vec<Retained<NSTextTab>> = Vec::new();
            let mut loc: f64 = 0.0;
            for col in columns.iter() {
                let line1 = if col.two_line {
                    col.name.clone()
                } else {
                    format!("{} {}", col.name, col.value)
                };
                let line2 = if col.two_line {
                    col.value.clone()
                } else {
                    String::new()
                };
                let w1 = measure_text_width(&line1, TRAY_FONT_SIZE);
                let w2 = measure_text_width(&line2, TRAY_FONT_SIZE + 3.0);
                let col_w = w1.max(w2) + COL_PADDING;
                col_widths.push(col_w);
                loc += col_w;
                left_tabs.push(NSTextTab::initWithType_location(
                    NSTextTab::alloc(),
                    NSTextTabType::LeftTabStopType,
                    loc,
                ));
            }
            let left_array: Retained<NSArray<NSTextTab>> = NSArray::from_retained_slice(&left_tabs);
            para.setTabStops(Some(&left_array));
        }

        // 根据对齐设置在文本前填充空格：right → 左侧填充至列宽；center → 两侧填充。
        let align_text = |text: &str, col_w: f64, font_size: f64, align: &str| -> String {
            if align == "left" || text.is_empty() {
                return text.to_string();
            }
            let text_w = measure_text_width(text, font_size);
            let space_w = measure_text_width(" ", font_size);
            if space_w <= 0.0 {
                return text.to_string();
            }
            let extra = (col_w - text_w).max(0.0);
            let n_spaces = (extra / space_w).round() as usize;
            match align {
                "right" => format!("{}{}", " ".repeat(n_spaces), text),
                "center" => {
                    let half = n_spaces / 2;
                    format!(
                        "{}{}{}",
                        " ".repeat(half),
                        text,
                        " ".repeat(n_spaces - half)
                    )
                }
                _ => text.to_string(),
            }
        };

        // baselineOffset：两行模式需要负偏移下推居中；单行模式无需偏移。
        let baseline_offset = NSNumber::new_f64(if two_line_mode { -7.0 } else { -5.0 });

        // 单行模式：每列字号覆盖为更大值（只有一行，充分利用菜单栏高度）。
        let single_line_font_size: f64 = 13.0;

        use objc2::runtime::AnyObject;
        let para_key: &NSString = unsafe { NSParagraphStyleAttributeName };
        let baseline_key: &NSString = unsafe { NSBaselineOffsetAttributeName };
        let font_key: &NSString = unsafe { NSFontAttributeName };
        let color_key: &NSString = unsafe { NSForegroundColorAttributeName };

        // 构造单段 attributed string（文字 + 字号 + 颜色 + 指定段落/baseline）。
        // 两行模式：标签行和值行共用 `para`（LeftTabStopType），列边界自然对齐。
        let make_part = |text: &str,
                         font_size: f64,
                         color: &TrayColor,
                         para_style: &NSMutableParagraphStyle|
         -> Retained<NSAttributedString> {
            let ns_text = NSString::from_str(text);
            let font: Retained<NSFont> = NSFont::boldSystemFontOfSize(font_size);
            let ns_color = resolve_tray_color(color);

            let keys: [&NSString; 4] = [font_key, color_key, para_key, baseline_key];
            let font_obj: &AnyObject = (*font).as_ref();
            let color_obj: &AnyObject = (*ns_color).as_ref();
            let para_obj: &AnyObject = para_style.as_ref();
            let baseline_obj: &AnyObject = (*baseline_offset).as_ref();
            let objects: [&AnyObject; 4] = [font_obj, color_obj, para_obj, baseline_obj];
            let attrs: Retained<NSDictionary<NSString, objc2::runtime::AnyObject>> =
                NSDictionary::from_slices(&keys, &objects);
            // SAFETY: attrs 键为 NSAttributedStringKey(NSString)、值为合法 AppKit 对象，类型正确。
            unsafe {
                NSAttributedString::initWithString_attributes(
                    NSAttributedString::alloc(),
                    &ns_text,
                    Some(&attrs),
                )
            }
        };

        let follow_color = TrayColor::default(); // mode=follow（tab/换行/separator 用）
        let result = NSMutableAttributedString::new();

        if two_line_mode {
            let _default_gap = " ".to_string();
            // 第一行（标签行）：各列首段，列间 \t + gap 文字。整行用 `para`（left tab）。
            for (idx, col) in columns.iter().enumerate() {
                if idx > 0 {
                    result.appendAttributedString(&make_part(
                        "\t",
                        TRAY_FONT_SIZE,
                        &follow_color,
                        &para,
                    ));
                    let gap_text = gaps
                        .get(idx - 1)
                        .and_then(|g| g.clone())
                        .unwrap_or_default();
                    if !gap_text.is_empty() {
                        result.appendAttributedString(&make_part(
                            &gap_text,
                            TRAY_FONT_SIZE,
                            &follow_color,
                            &para,
                        ));
                    }
                }
                let line1 = if col.two_line {
                    col.name.clone()
                } else {
                    format!("{} {}", col.name, col.value)
                };
                let col_w = col_widths.get(idx).copied().unwrap_or(0.0);
                let aligned = align_text(&line1, col_w, TRAY_FONT_SIZE, &col.align);
                result.appendAttributedString(&make_part(
                    &aligned,
                    TRAY_FONT_SIZE,
                    &col.color,
                    &para,
                ));
            }
            // 行间换行
            let nl_font = columns
                .first()
                .map(|c| c.font_size)
                .unwrap_or(TRAY_FONT_SIZE);
            result.appendAttributedString(&make_part("\n", nl_font, &follow_color, &para));
            // 第二行（值行）：与标签行相同结构，对齐取 align_row2（fallback align）。字体比标签行大1pt。
            for (idx, col) in columns.iter().enumerate() {
                let row2_font = TRAY_FONT_SIZE + 3.0;
                if idx > 0 {
                    result.appendAttributedString(&make_part(
                        "\t",
                        row2_font,
                        &follow_color,
                        &para,
                    ));
                    let gap_text = gaps
                        .get(idx - 1)
                        .and_then(|g| g.clone())
                        .unwrap_or_default();
                    if !gap_text.is_empty() {
                        result.appendAttributedString(&make_part(
                            &gap_text,
                            row2_font,
                            &follow_color,
                            &para,
                        ));
                    }
                }
                let line2 = if col.two_line {
                    col.value.clone()
                } else {
                    String::new()
                };
                if !line2.is_empty() {
                    let row2_align = col.align_row2.as_deref().unwrap_or(&col.align);
                    let col_w = col_widths.get(idx).copied().unwrap_or(0.0);
                    let aligned = align_text(&line2, col_w, row2_font, row2_align);
                    result
                        .appendAttributedString(&make_part(&aligned, row2_font, &col.color, &para));
                }
            }
        } else {
            // 单行模式：每列 "名 值"，列间用 gap 拼接。字号加大（只有一行，充分利用菜单栏高度）。
            let default_gap = " ".to_string();
            let join_font = single_line_font_size;
            for (idx, col) in columns.iter().enumerate() {
                if idx > 0 {
                    let gap_text = gaps
                        .get(idx - 1)
                        .and_then(|g| g.clone())
                        .unwrap_or_else(|| default_gap.clone());
                    result.appendAttributedString(&make_part(
                        &gap_text,
                        join_font,
                        &follow_color,
                        &para,
                    ));
                }
                let text = format!("{} {}", col.name, col.value);
                result.appendAttributedString(&make_part(
                    &text,
                    single_line_font_size,
                    &col.color,
                    &para,
                ));
            }
        }

        button.setAttributedTitle(&result);
        Ok(())
    })
    .map_err(|e| e.to_string())?
}

pub async fn refresh_tray_menu(
    app: &tauri::AppHandle,
    builder: &dyn TrayMenuBuild,
) -> Result<(), String> {
    // 异步准备（可在任意线程）：菜单 + 布局数据。tray 句柄不在此触碰。
    let menu = builder.build_menu(app).await?;
    #[cfg(target_os = "macos")]
    let layout = builder.layout().await;
    #[cfg(target_os = "macos")]
    let separator = builder.separator().await;

    // tauri TrayIcon<R> 内含非原子 Rc（tray-icon crate 的 Rc<RefCell<platform TrayIcon>>），
    // 其 `unsafe impl Send` 的安全契约是「所有访问（含 Drop）恒在主线程」。
    // refresh_tray_menu 跑在 tokio worker：若在此 tray_by_id 克隆（Rc 递增）+ 函数末尾
    // Drop（Rc 递减），且多个 tray-refresh 并发（cold_start_init_tray_estimates 每平台扇出
    // 一个 spawn → 各自 emit tray-refresh），非原子 Rc 会被多线程 clone/drop 竞争 → 计数
    // 损坏 → 提前 free → platform TrayIcon::drop 调 removeStatusItem 在非主线程执行 →
    // BoardServices assertBarrierOnQueue SIGTRAP（崩溃报告 thread 7 tokio-rt-worker）。
    // 故把 tray 获取 + 全部变更 + Drop 整体挪进主线程闭包：高层 set_* 内部的
    // run_item_main_thread 在主线程会 inline 同步执行（send_user_message 判定同线程直跑），
    // 不自锁。
    let app = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    app.clone()
        .run_on_main_thread(move || {
            let r = (|| -> Result<(), String> {
                let tray = app.tray_by_id("main").ok_or("tray not found")?;
                tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
                // macOS 菜单栏：有 quota 值时隐藏 logo + 两行小字 title；无值时恢复 logo + 清 title。
                // 非 macOS 平台仅 menu item 降级（不调 set_title / set_icon）。
                #[cfg(target_os = "macos")]
                {
                    if layout.columns.is_empty() {
                        tray.set_icon(app.default_window_icon().cloned())
                            .map_err(|e| e.to_string())?;
                        tray.set_title(None::<&str>).map_err(|e| e.to_string())?;
                    } else {
                        tray.set_icon(None).map_err(|e| e.to_string())?;
                        // 兜底文字：各列 "名 值"，间隙用 separator
                        let fallback_text = layout
                            .columns
                            .iter()
                            .map(|c| format!("{} {}", c.name, c.value))
                            .collect::<Vec<_>>()
                            .join(separator.as_str());
                        tray.set_title(Some(&fallback_text))
                            .map_err(|e| e.to_string())?;
                        if let Err(e) =
                            set_tray_attributed_title(&tray, layout.columns, layout.gaps, separator)
                        {
                            tracing::warn!(
                                "tray attributed title failed, fallback to default font: {e}"
                            );
                        }
                    }
                }
                Ok(())
                // tray 在此 drop —— 主线程内，Rc 递减安全。
            })();
            let _ = tx.send(r);
        })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

/// TrayMenuBuild 的实现：把 refresh_tray_menu 调用桥接到本文件的
/// build_tray_menu / tray_layout / tray_separator（同 crate 合并后无需再跨 crate 注入，
/// 保留 trait 间接层是纯搬运选择）。root aidog crate（app_setup）经
/// `aidog_core::tray_render::TrayMenuBuildImpl` 路径引用（C3 c3-commands 迁入）。
pub struct TrayMenuBuildImpl;

impl TrayMenuBuild for TrayMenuBuildImpl {
    fn build_menu<'a>(
        &'a self,
        app: &'a tauri::AppHandle,
    ) -> Pin<Box<dyn Future<Output = Result<tauri::menu::Menu<tauri::Wry>, String>> + Send + 'a>>
    {
        Box::pin(build_tray_menu(app))
    }
    fn layout<'a>(&'a self) -> Pin<Box<dyn Future<Output = TrayLayout> + Send + 'a>> {
        Box::pin(tray_layout(aidog_ctx::db()))
    }
    fn separator<'a>(&'a self) -> Pin<Box<dyn Future<Output = String> + Send + 'a>> {
        Box::pin(tray_separator(aidog_ctx::db()))
    }
}

/// 计算单个 platform item 的（名, 值）二元组。
/// display="coding" 或平台具 coding plan → 值=`{%}%`（剩余百分比）；否则 值=`{balance:.2}`。
pub(crate) fn platform_item_parts(platform: &Platform, display: &str) -> (String, String) {
    let name = platform.name.clone();
    let plan = gateway::estimate::EstCodingPlan::from_json(&platform.est_coding_plan);
    let first_tier = plan.tiers.first();
    let is_coding = display == "coding" || first_tier.is_some();
    let value = if is_coding {
        let util = first_tier.map(|t| t.est_utilization).unwrap_or(0.0);
        format!("{:.0}%", (100.0 - util).max(0.0))
    } else {
        format!(
            "${}",
            trim_trailing_zeros(&format!("{:.2}", platform.est_balance_remaining))
        )
    };
    (name, value)
}

/// 从托盘配置生成有序渲染布局（已按 order 排序、跳过 disabled、跳过取数失败项）。
/// separator items 不生成列，而是作为相邻数据列之间的间隙。
/// gaps[i] = columns[i] 与 columns[i+1] 之间的间隙；None = 默认空白。
pub async fn tray_layout(db: &Db) -> TrayLayout {
    tray_layout_with_stats(db, None).await
}

/// `tray_layout` 内部实现，`precomputed_today_stats` 可选预取值：
/// popover_data 已单独查过 today_stats 时传入复用，消内部重复聚合；
/// 独立 tray 菜单路径（无预取）传 None，内部按需现查（`tray_layout` 公开入口即此语义）。
pub(crate) async fn tray_layout_with_stats(
    db: &Db,
    precomputed_today_stats: Option<&aidog_stats::TodayStats>,
) -> TrayLayout {
    let empty = TrayLayout {
        columns: Vec::new(),
        gaps: Vec::new(),
    };
    let Ok(Some(config)) = db::get_tray_config(db).await else {
        return empty;
    };
    let mut items: Vec<&TrayItem> = config.items.iter().filter(|i| i.enabled).collect();
    items.sort_by_key(|i| i.order);

    // 批量预取所涉 platform（消 per-item 单查 N+1；IN 批量单次查询替代逐 item get_platform）。
    let platform_ids: Vec<i64> = items
        .iter()
        .filter(|i| i.item_type == "platform")
        .filter_map(|i| i.platform_id)
        .map(|id| id as i64)
        .collect();
    let platforms = if platform_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        db::get_platforms_by_ids(&db, &platform_ids)
            .await
            .unwrap_or_default()
    };

    let mut columns: Vec<TrayColumn> = Vec::new();
    let mut gaps: Vec<Option<String>> = Vec::new();
    let mut pending_sep: Option<String> = None;

    for item in items {
        if item.item_type == "separator" {
            pending_sep = Some(if item.display.is_empty() {
                "·".to_string()
            } else {
                item.display.clone()
            });
            continue;
        }

        // Non-separator item → compute column data
        if !columns.is_empty() {
            gaps.push(pending_sep.take());
        }

        let two_line = item.line_mode == "two";
        let (name, value) = match item.item_type.as_str() {
            "platform" => {
                let Some(pid) = item.platform_id else {
                    continue;
                };
                let Some(platform) = platforms.get(&(pid as i64)) else {
                    continue;
                };
                platform_item_parts(platform, &item.display)
            }
            "today_usage" => {
                let owned_stats;
                let stats = match precomputed_today_stats {
                    Some(s) => s,
                    None => {
                        owned_stats = aidog_stats::today_stats(&db).await.unwrap_or(
                            aidog_stats::TodayStats {
                                tokens: 0,
                                input_tokens: 0,
                                output_tokens: 0,
                                cache_tokens: 0,
                                cache_rate: 0.0,
                                cost: 0.0,
                                total_requests: 0,
                            },
                        );
                        &owned_stats
                    }
                };
                let metric = item.metric.as_deref().unwrap_or("tokens");
                let (label, val) = match metric {
                    "cache_rate" => ("Cache".to_string(), format!("{:.0}%", stats.cache_rate)),
                    "cost" => {
                        let d = item.decimals.unwrap_or(5) as usize;
                        (
                            "花费".to_string(),
                            format!(
                                "${}",
                                trim_trailing_zeros(&format!("{:.d$}", stats.cost, d = d))
                            ),
                        )
                    }
                    "requests" => ("请求".to_string(), format!("{}", stats.total_requests)),
                    _ => ("今日".to_string(), format!("{} tok", stats.tokens)),
                };
                (label, val)
            }
            _ => continue,
        };
        if name.is_empty() && value.is_empty() {
            continue;
        }
        // 自定义 label 优先
        let name = item.label.clone().unwrap_or(name);
        columns.push(TrayColumn {
            name,
            value,
            color: item.color.clone(),
            font_size: item.font_size,
            two_line,
            align: item.align.clone(),
            align_row2: item.align_row2.clone(),
        });
    }

    TrayLayout { columns, gaps }
}

/// 托盘配置的分隔符（多 item 横排间隔）。
pub(crate) async fn tray_separator(db: &Db) -> String {
    if let Ok(Some(config)) = db::get_tray_config(db).await {
        return config.separator;
    }
    default_separator_str()
}

pub(crate) fn default_separator_str() -> String {
    "  ".to_string()
}

/// 菜单内 quota 项的纯文字概要（无颜色/字号，separator 拼接；每列横排 "名 值"）。
pub(crate) async fn tray_quota_text(db: &Db) -> Option<String> {
    let layout = tray_layout(db).await;
    if layout.columns.is_empty() {
        return None;
    }
    let default_sep = tray_separator(db).await;
    let mut texts: Vec<String> = Vec::new();
    for (i, col) in layout.columns.iter().enumerate() {
        if i > 0 {
            let gap = layout
                .gaps
                .get(i - 1)
                .and_then(|g| g.clone())
                .unwrap_or_else(|| " ".to_string());
            texts.push(gap);
        }
        texts.push(format!("{} {}", col.name, col.value));
    }
    Some(texts.join(&default_sep))
}

pub async fn build_tray_menu(
    app: &tauri::AppHandle,
) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    let running = aidog_ctx::ctx().proxy_handle().is_running();

    let settings = load_proxy_settings(aidog_ctx::db()).await?;
    let status_text = if running {
        format!("● Proxy Running :{}", settings.port)
    } else {
        "○ Proxy Stopped".to_string()
    };

    let toggle_id = if running { "proxy_stop" } else { "proxy_start" };
    let toggle_text = if running { "Stop Proxy" } else { "Start Proxy" };

    let mut builder = MenuBuilder::new(app).item(
        &MenuItemBuilder::with_id("status", status_text)
            .enabled(false)
            .build(app)
            .map_err(|e| e.to_string())?,
    );

    // tray quota 详情项（选定平台余额 / coding%）
    if let Some(quota_text) = tray_quota_text(aidog_ctx::db()).await {
        builder = builder.item(
            &MenuItemBuilder::with_id("tray_quota", quota_text)
                .enabled(false)
                .build(app)
                .map_err(|e| e.to_string())?,
        );
    }

    let menu = builder
        .separator()
        .item(
            &MenuItemBuilder::with_id(toggle_id, toggle_text)
                .build(app)
                .map_err(|e| e.to_string())?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("show", "Show Window")
                .build(app)
                .map_err(|e| e.to_string())?,
        )
        .item(
            &MenuItemBuilder::with_id("quit", "Quit")
                .build(app)
                .map_err(|e| e.to_string())?,
        )
        .build()
        .map_err(|e| e.to_string())?;

    Ok(menu)
}

/// 去除浮点数格式化尾部多余的零：10.10 → "10.1", 0.00 → "0", 965.80 → "965.8"
pub(crate) fn trim_trailing_zeros(s: &str) -> String {
    if let Some(_pos) = s.find('.') {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        if trimmed.is_empty() {
            "0".to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        s.to_string()
    }
}
