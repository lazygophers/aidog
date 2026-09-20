//! Tray 与 Popover 浮窗配置模型。

use super::default_true;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[cfg(test)]
#[path = "test_tray.rs"]
mod test_tray;

// ─── Tray Config (KV: scope=tray, key=config) ──────────────

/// 单项颜色配置（三态）。
/// - mode="follow": 跟随系统（labelColor，自适应明暗）
/// - mode="preset": value ∈ {"red","green","orange"} → systemRed/Green/Orange（自适应明暗）
/// - mode="custom": value = hex（如 "#RRGGBB"），固定色，可能在某主题下可读性差
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct TrayColor {
    #[serde(default = "default_color_mode")]
    pub mode: String,
    #[serde(default)]
    pub value: String,
}

fn default_color_mode() -> String {
    "follow".to_string()
}

impl Default for TrayColor {
    fn default() -> Self {
        Self {
            mode: default_color_mode(),
            value: String::new(),
        }
    }
}

/// 托盘单个展示项。
/// - item_type="platform": platform_id 指定平台，display ∈ {"balance","coding"}
/// - item_type="today_usage": metric ∈ {"tokens","cache_rate","cost","requests"}，display/platform_id 忽略
/// - item_type="routed_platform": 当前命中平台（取最近一条非测试 proxy_log 的平台），无字段
/// - item_type="peak": 高峰指示（按当前命中平台的 peak 窗口判定），无字段
/// - item_type="separator": display 存分隔符文本（如 "|"、"·"、"—"）。
///   票 I15 起菜单栏只画最多 3 段、separator 不再可选，存量 separator 项一律被
///   [`clamp_to_segments`] 置 `enabled=false` 留在配置里（不删）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct TrayItem {
    #[serde(default = "default_item_type")]
    pub item_type: String,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub platform_id: Option<u64>,
    #[serde(default = "default_display")]
    pub display: String,
    #[serde(default)]
    pub metric: Option<String>,
    /// 自定义标签（优先于自动生成的 name）。None = 使用默认。
    #[serde(default)]
    pub label: Option<String>,
    /// 花费/余额小数位数。None = 默认 5 位。
    #[serde(default)]
    pub decimals: Option<u32>,
    #[serde(default)]
    pub color: TrayColor,
    #[serde(default = "default_font_size")]
    pub font_size: f64,
    /// 该项行模式（作为「一列」）："single"（第一行 "名 值"，第二行该列留空占位）
    /// | "two"（第一行该列显 name，第二行该列显 value）。
    /// iStat Menus 式两行多列：任一列 two → 整体两行模式（NSTextTab 列对齐），否则单行横排。
    #[serde(default = "default_line_mode")]
    pub line_mode: String,
    /// 对齐方式："left" | "center" | "right"，默认 "left"
    #[serde(default = "default_align")]
    pub align: String,
    /// 两行模式下第二行对齐："left" | "center" | "right"，默认跟随 align
    #[serde(default)]
    pub align_row2: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub order: i32,
}

fn default_item_type() -> String {
    "platform".to_string()
}
fn default_display() -> String {
    "balance".to_string()
}
fn default_font_size() -> f64 {
    9.0
}
fn default_line_mode() -> String {
    "two".to_string()
}
fn default_align() -> String {
    "left".to_string()
}

/// 托盘整体配置（存 settings: scope="tray", key="config"）。
/// 行模式（单/两行）改为每 item 各自 `line_mode`，全局仅保留 separator（多 item 间分隔）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct TrayConfig {
    /// 多 item 横排时各项之间的分隔符
    #[serde(default = "default_separator")]
    pub separator: String,
    #[serde(default)]
    pub items: Vec<TrayItem>,
}

fn default_separator() -> String {
    "  ".to_string()
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            separator: default_separator(),
            items: Vec::new(),
        }
    }
}

/// 菜单栏最多画几段（票 I15：今日费用 · 当前命中平台 · 高峰指示）。
pub const TRAY_MAX_SEGMENTS: usize = 3;

/// 一个空白 item 模板（除 item_type / order 外全取默认）。
fn segment(item_type: &str, metric: Option<&str>, order: i32) -> TrayItem {
    TrayItem {
        item_type: item_type.to_string(),
        platform_id: None,
        display: String::new(),
        metric: metric.map(str::to_string),
        label: None,
        decimals: None,
        color: TrayColor::default(),
        font_size: default_font_size(),
        line_mode: "single".to_string(),
        align: default_align(),
        align_row2: None,
        enabled: true,
        order,
    }
}

impl TrayConfig {
    /// 票 I15 的出厂三段：今日费用 · 当前命中平台 · 高峰指示。
    pub fn default_segments() -> Self {
        Self {
            separator: default_separator(),
            items: vec![
                segment("today_usage", Some("cost"), 0),
                segment("routed_platform", None, 1),
                segment("peak", None, 2),
            ],
        }
    }
}

/// 票 I15 迁移：二维网格编辑器 → 「最多挑 3 项」单选清单。
///
/// **规则是「降级不删除」**：按 order 排序后，前 [`TRAY_MAX_SEGMENTS`] 个 enabled 的
/// 数据项保持 enabled，其余（含全部 separator 项）一律置 `enabled=false` 留在 items 里。
/// 字段（line_mode / align / align_row2 / font_size / color / label…）一个不动，
/// 所以降级到旧版本仍能原样渲染，用户也能在新清单里把被关掉的项重新挑回来。
///
/// 返回 true 表示配置被改过（调用方据此决定是否落盘）。
pub fn clamp_to_segments(cfg: &mut TrayConfig) -> bool {
    let mut order: Vec<usize> = (0..cfg.items.len()).collect();
    order.sort_by_key(|&i| cfg.items[i].order);
    let mut kept = 0usize;
    let mut changed = false;
    for i in order {
        let item = &mut cfg.items[i];
        let keepable =
            item.enabled && item.item_type != "separator" && kept < TRAY_MAX_SEGMENTS;
        if keepable {
            kept += 1;
        } else if item.enabled {
            item.enabled = false;
            changed = true;
        }
    }
    changed
}

// ─── Popover Config (KV: scope="popover", key="config") ────

/// Popover 浮窗单个展示项。
/// `item_type` ∈ 预定义指标集：
/// - "today_cost"       今日已用金额
/// - "today_cache_rate" 今日缓存率
/// - "today_tokens"     今日 token 总量
/// - "platform_today"   各平台当日使用（只含已用，列表）
/// - "proxy_status"     代理状态行
/// - "platform_balance" 平台余额 / coding 列（复用 tray 列）
/// - "cost_trend"       消费趋势曲线（按 scope / time_window 维度）
///
/// 预定义指标集内自由组合增删 / 排序 / 显隐；不接受用户输入任意数据源。
// 注：故意不 #[ts(export)] —— item_type/scope/time_window/size 字段 Rust 侧为 String（自由存储 +
// serde default 兼容旧配置），TS 侧用手写字面量联合窄化（见 manual.ts PopoverItem），
// PopoverConfig.items 字段用 #[ts(type=...)] 显式指回手写版，避免生成版(string)与手写版类型冲突。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PopoverItem {
    /// 稳定 id（前端生成，便于拖拽 key），后端仅透传持久化。
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_popover_item_type")]
    pub item_type: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub order: i32,
    /// 仅 cost_trend 用：曲线统计维度 "overall" | "group" | "platform"。
    /// 旧配置无此字段 → 默认 "overall"，向后兼容。
    #[serde(default = "default_popover_scope")]
    pub scope: String,
    /// 仅 cost_trend + scope!=overall 用：维度引用（group → group_key；platform → platform_id 字符串）。
    #[serde(default)]
    pub scope_ref: Option<String>,
    /// 仅 cost_trend 用：时间窗 "today" | "7d" | "30d"。旧配置无此字段 → 默认 "7d"。
    #[serde(default = "default_popover_time_window")]
    pub time_window: String,
    /// 二维布局行号。旧配置无此字段 → 默认 0；渲染层按 `row || order` fallback，老用户各占一行。
    #[serde(default)]
    pub row: i32,
    /// 卡片尺寸 / 内容密度 "s" | "m" | "l"。旧配置无此字段 → 默认 "m"。
    #[serde(default = "default_popover_size")]
    pub size: String,
    /// 卡片数值颜色（复用 tray 三态颜色）。旧配置无此字段 → 默认 follow。
    #[serde(default)]
    pub color: TrayColor,
}

fn default_popover_scope() -> String {
    "overall".to_string()
}
fn default_popover_time_window() -> String {
    "7d".to_string()
}
fn default_popover_size() -> String {
    "m".to_string()
}

fn default_popover_item_type() -> String {
    "today_cost".to_string()
}

/// Popover 单行布局元信息（按 row 索引）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct RowMeta {
    /// 该行列数 1 | 2 | 3。缺省视为 1。
    #[serde(default = "default_cols")]
    pub cols: i32,
}

fn default_cols() -> i32 {
    1
}

/// Popover 浮窗整体配置（存 settings: scope="popover", key="config"）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct PopoverConfig {
    #[serde(default)]
    #[ts(type = "import(\"../manual\").PopoverItem[]")]
    pub items: Vec<PopoverItem>,
    /// 各行布局元信息（按 row 索引）；缺省项 / 越界视为 cols=1。
    #[serde(default)]
    pub rows: Vec<RowMeta>,
}

impl Default for PopoverConfig {
    /// 默认：今日金额 / 缓存率 / token / 各平台当日 + 代理状态 + 平台余额列。
    /// 前 4 项为 prd 默认可见，proxy_status / platform_balance 默认可见（沿用现有 popover 展示）。
    fn default() -> Self {
        let types = [
            "proxy_status",
            "platform_balance",
            "today_cost",
            "today_cache_rate",
            "today_tokens",
            "platform_today",
        ];
        Self {
            items: types
                .iter()
                .enumerate()
                .map(|(i, t)| PopoverItem {
                    id: format!("popover-{t}"),
                    item_type: t.to_string(),
                    visible: true,
                    order: i as i32,
                    scope: default_popover_scope(),
                    scope_ref: None,
                    time_window: default_popover_time_window(),
                    row: i as i32,
                    size: default_popover_size(),
                    color: TrayColor::default(),
                })
                .collect(),
            rows: Vec::new(),
        }
    }
}
