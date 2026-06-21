//! Tray 与 Popover 浮窗配置模型。

use super::default_true;
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "test_tray.rs"]
mod test_tray;

// ─── Tray Config (KV: scope=tray, key=config) ──────────────

/// 单项颜色配置（三态）。
/// - mode="follow": 跟随系统（labelColor，自适应明暗）
/// - mode="preset": value ∈ {"red","green","orange"} → systemRed/Green/Orange（自适应明暗）
/// - mode="custom": value = hex（如 "#RRGGBB"），固定色，可能在某主题下可读性差
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayColor {
    #[serde(default = "default_color_mode")]
    pub mode: String,
    #[serde(default)]
    pub value: String,
}

fn default_color_mode() -> String { "follow".to_string() }

impl Default for TrayColor {
    fn default() -> Self {
        Self { mode: default_color_mode(), value: String::new() }
    }
}

/// 托盘单个展示项。
/// - item_type="platform": platform_id 指定平台，display ∈ {"balance","coding"}
/// - item_type="today_usage": metric ∈ {"tokens","cache_rate","cost","requests"}，display/platform_id 忽略
/// - item_type="separator": display 存分隔符文本（如 "|"、"·"、"—"）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayItem {
    #[serde(default = "default_item_type")]
    pub item_type: String,
    #[serde(default)]
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

fn default_item_type() -> String { "platform".to_string() }
fn default_display() -> String { "balance".to_string() }
fn default_font_size() -> f64 { 9.0 }
fn default_line_mode() -> String { "two".to_string() }
fn default_align() -> String { "left".to_string() }

/// 托盘整体配置（存 settings: scope="tray", key="config"）。
/// 行模式（单/两行）改为每 item 各自 `line_mode`，全局仅保留 separator（多 item 间分隔）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayConfig {
    /// 多 item 横排时各项之间的分隔符
    #[serde(default = "default_separator")]
    pub separator: String,
    #[serde(default)]
    pub items: Vec<TrayItem>,
}

fn default_separator() -> String { "  ".to_string() }

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            separator: default_separator(),
            items: Vec::new(),
        }
    }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn default_popover_scope() -> String { "overall".to_string() }
fn default_popover_time_window() -> String { "7d".to_string() }
fn default_popover_size() -> String { "m".to_string() }

fn default_popover_item_type() -> String { "today_cost".to_string() }

/// Popover 单行布局元信息（按 row 索引）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowMeta {
    /// 该行列数 1 | 2 | 3。缺省视为 1。
    #[serde(default = "default_cols")]
    pub cols: i32,
}

fn default_cols() -> i32 { 1 }

/// Popover 浮窗整体配置（存 settings: scope="popover", key="config"）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopoverConfig {
    #[serde(default)]
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
