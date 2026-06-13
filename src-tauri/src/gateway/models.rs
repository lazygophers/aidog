use serde::{Deserialize, Serialize};

/// 支持的 AI 协议类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    // ── AI 请求协议（可作为 endpoint 协议）──
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "openai_responses")]
    OpenAIResponses,
    #[serde(rename = "openai_completions")]
    OpenAICompletions,
    #[serde(rename = "gemini")]
    Gemini,
    // ── 平台类型（仅作为平台主协议，不作为 endpoint 协议）──
    #[serde(rename = "mock")]
    Mock,
    /// Claude Code 原始订阅平台（纯透传，客户端自带 OAuth 认证）
    #[serde(rename = "claude_code")]
    ClaudeCode,
    #[serde(rename = "glm")]
    Glm,
    #[serde(rename = "glm_en")]
    GlmEn,
    #[serde(rename = "kimi")]
    Kimi,
    #[serde(rename = "minimax")]
    MiniMax,
    #[serde(rename = "minimax_en")]
    MiniMaxEn,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "bailian")]
    Bailian,
    #[serde(rename = "bailian_coding")]
    BailianCoding,
    // ── 国内官方平台 ──
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "stepfun")]
    StepFun,
    #[serde(rename = "stepfun_en")]
    StepFunEn,
    #[serde(rename = "doubao")]
    Doubao,
    #[serde(rename = "doubao_seed")]
    DoubaoSeed,
    #[serde(rename = "byteplus")]
    BytePlus,
    #[serde(rename = "qianfan")]
    QianFan,
    #[serde(rename = "xiaomi_mimo")]
    XiaomiMimo,
    #[serde(rename = "bailing")]
    BaiLing,
    #[serde(rename = "longcat")]
    Longcat,
    // ── 聚合平台 ──
    #[serde(rename = "openrouter")]
    OpenRouter,
    #[serde(rename = "siliconflow")]
    SiliconFlow,
    #[serde(rename = "siliconflow_en")]
    SiliconFlowEn,
    #[serde(rename = "aihubmix")]
    AiHubMix,
    #[serde(rename = "dmxapi")]
    DmxApi,
    #[serde(rename = "modelscope")]
    ModelScope,
    #[serde(rename = "shengsuanyun")]
    ShengSuanYun,
    #[serde(rename = "atlascloud")]
    AtlasCloud,
    #[serde(rename = "novita")]
    Novita,
    #[serde(rename = "therouter")]
    TheRouter,
    #[serde(rename = "cherryin")]
    CherryIn,
    // ── 第三方平台 ──
    #[serde(rename = "packycode")]
    PackyCode,
    #[serde(rename = "cubence")]
    Cubence,
    #[serde(rename = "aigocode")]
    AiGoCode,
    #[serde(rename = "rightcode")]
    RightCode,
    #[serde(rename = "aicodemirror")]
    AiCodeMirror,
    #[serde(rename = "nvidia")]
    Nvidia,
    #[serde(rename = "pateway")]
    Pateway,
    #[serde(rename = "ccsub")]
    CcSub,
    #[serde(rename = "apikeyfun")]
    ApiKeyFun,
    #[serde(rename = "apinebula")]
    ApiNebula,
    #[serde(rename = "sudocode")]
    SudoCode,
    #[serde(rename = "claudeapi")]
    ClaudeApi,
    #[serde(rename = "claudecn")]
    ClaudeCN,
    #[serde(rename = "runapi")]
    RunApi,
    #[serde(rename = "relaxycode")]
    RelaxyCode,
    #[serde(rename = "crazyrouter")]
    CrazyRouter,
    #[serde(rename = "sssaicode")]
    SssAiCode,
    #[serde(rename = "compshare")]
    Compshare,
    #[serde(rename = "compshare_coding")]
    CompshareCoding,
    #[serde(rename = "micu")]
    Micu,
    #[serde(rename = "ctok")]
    CTok,
    #[serde(rename = "eflowcode")]
    EFlowCode,
    #[serde(rename = "lemondata")]
    LemonData,
    #[serde(rename = "pipellm")]
    PipeLlm,
    #[serde(rename = "opencode")]
    OpenCode,
    // ── 中转平台 ──
    #[serde(rename = "newapi")]
    NewApi,
}

/// 路由模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoutingMode {
    #[serde(rename = "load_balance")]
    LoadBalance,
    #[serde(rename = "failover")]
    Failover,
}

// ─── Platform Models ───────────────────────────────────────

/// 平台模型配置：5 个固定槽位
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformModels {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sonnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub haiku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpt: Option<String>,
}

impl PlatformModels {
    /// 返回所有已配置的模型名（去重）
    #[allow(dead_code)]
    pub fn all_values(&self) -> Vec<String> {
        let mut v = Vec::new();
        for s in [&self.default, &self.sonnet, &self.opus, &self.haiku, &self.gpt].into_iter().flatten() {
            if !v.contains(s) {
                v.push(s.clone());
            }
        }
        v
    }
}

// ─── ClientType (客户端模拟) ─────────────────────────────────

/// 可模拟的客户端类型，用于通过上游的客户端校验。
/// 参考 claude-code-hub 的客户端检测逻辑：
///   - Claude Code 家族: CLI / VSCode / SDK-TS / SDK-PY / GitHub Action
///   - Codex 家族: CLI-Rust / TUI / Desktop / VSCode
///   - IDE: Cursor / Windsurf
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum ClientType {
    #[default]
    #[serde(rename = "default")]
    Default,
    // ── Claude Code family ──
    #[serde(rename = "claude_code")]
    ClaudeCode,
    #[serde(rename = "claude_code_vscode")]
    ClaudeCodeVscode,
    #[serde(rename = "claude_code_sdk_ts")]
    ClaudeCodeSdkTs,
    #[serde(rename = "claude_code_sdk_py")]
    ClaudeCodeSdkPy,
    #[serde(rename = "claude_code_gh_action")]
    ClaudeCodeGhAction,
    // ── Codex family ──
    #[serde(rename = "codex_cli")]
    CodexCli,
    #[serde(rename = "codex_tui")]
    CodexTui,
    #[serde(rename = "codex_desktop")]
    CodexDesktop,
    #[serde(rename = "codex_vscode")]
    CodexVscode,
    // ── IDE ──
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "windsurf")]
    Windsurf,
}

impl ClientType {
    /// 根据 endpoint 协议返回推荐的默认客户端类型：
    /// - anthropic → claude_code (CLI)
    /// - openai → codex_tui
    /// - 其他 → default
    #[allow(dead_code)]
    pub fn default_for_protocol(protocol: &Protocol) -> Self {
        match protocol {
            Protocol::Anthropic => ClientType::ClaudeCode,
            Protocol::OpenAI | Protocol::OpenAIResponses | Protocol::OpenAICompletions => ClientType::CodexTui,
            _ => ClientType::Default,
        }
    }
}

// ─── Platform Endpoint ──────────────────────────────────────

/// 容错反序列化 client_type：未知字符串回退为 ClientType::Default，
/// 而非让整个 endpoints 数组解析失败。
fn deserialize_client_type_lenient<'de, D>(deserializer: D) -> Result<ClientType, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(serde_json::from_value(serde_json::Value::String(s)).unwrap_or_default())
}

/// 平台协议端点：同一平台可支持多种协议，每种协议对应不同的 base_url
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEndpoint {
    pub protocol: Protocol,
    pub base_url: String,
    /// 模拟的客户端类型（用于通过上游客户端校验）。
    /// 用 `deserialize_with` 容错：DB 中历史遗留 / 未知 client_type 字符串
    /// （如旧数据里的 "anthropic"）回退为 Default，避免单个未知值导致整个
    /// endpoints 数组反序列化失败 → 空 Vec → 前端 Protocol Endpoints 丢失。
    #[serde(default, deserialize_with = "deserialize_client_type_lenient")]
    pub client_type: ClientType,
    /// 是否为 Coding Plan（针对支持编程代理订阅的平台，如 Kimi Code Plan）
    #[serde(default)]
    pub coding_plan: bool,
}

// ─── Manual Budget ─────────────────────────────────────────

/// 窗口时长单位（仅 rolling/fixed 有意义）。
/// serde default = Hour，保证旧 JSON（无 window_unit 字段）解析为「小时」，
/// 即 window_hours 数值原意（向后兼容零回退）。month 固定按 30 天换算。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WindowUnit {
    Minute,
    #[default]
    Hour,
    Day,
    Week,
    Month,
}

/// 手动预算限额（仅对无上游 quota 自动支持的平台开放）。
/// 一平台可同时启多条；任一耗尽即阻断转发。est_cost/token 由请求驱动累加。
/// 全字段向后兼容：旧平台无 manual_budgets → 空 Vec → 不阻断、不扣、行为不变。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualBudget {
    /// 限额唯一 id（前端生成，UPDATE 回写按 id 对齐保留 consumed/window_start_at）
    pub id: String,
    /// "total" 不重置 | "rolling" 滑动 N 个 window_unit | "fixed" 固定 N 个 window_unit 钟点对齐 | "daily" 自然日重置
    pub kind: String,
    /// "usd" 扣 est_cost | "token" 扣总 token
    pub unit: String,
    /// 限额额度（usd 为 $ / token 为 token 数）
    pub amount: f64,
    /// 窗口数值（该 window_unit 下的数量），仅 rolling/fixed 有意义。
    /// 历史字段名保留为 window_hours（不改名以最小化迁移）：
    /// 旧数据无 window_unit 时按小时解释（向后兼容）；新数据配合 window_unit 表任意单位。
    #[serde(default)]
    pub window_hours: Option<f64>,
    /// 窗口时长单位（minute/hour/day/week/month），旧数据缺失 → 默认 hour。
    #[serde(default)]
    pub window_unit: WindowUnit,
    /// 当前窗口已消耗（系统维护，请求驱动累加；窗口重置时清零）
    #[serde(default)]
    pub consumed: f64,
    /// 当前窗口起始毫秒戳（系统维护；rolling/fixed/daily 追踪重置基准）
    #[serde(default)]
    pub window_start_at: Option<i64>,
    /// 是否启用此限额
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 解析 platform.manual_budgets JSON 列；空/非法 → 空 Vec
pub fn parse_manual_budgets(s: &str) -> Vec<ManualBudget> {
    if s.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(s).unwrap_or_default()
}

/// 序列化 manual_budgets → JSON 字符串（空 Vec → "[]"）
pub fn serialize_manual_budgets(budgets: &[ManualBudget]) -> String {
    serde_json::to_string(budgets).unwrap_or_else(|_| "[]".to_string())
}

// ─── Platform ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub id: u64,
    pub name: String,
    pub platform_type: Protocol,
    pub base_url: String,
    pub api_key: String,
    /// JSON 额外配置
    pub extra: String,
    /// 平台模型配置
    pub models: PlatformModels,
    /// 从 API 获取到的可用模型列表
    pub available_models: Vec<String>,
    /// 额外协议端点：每种协议对应不同的 base_url
    #[serde(default)]
    pub endpoints: Vec<PlatformEndpoint>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
    /// 预估剩余余额（按量计费平台，请求驱动增量自减；系统维护，前端只读）
    #[serde(default)]
    pub est_balance_remaining: f64,
    /// 预估 coding plan JSON（含 tiers est_utilization + 方案 B 拟合系数/样本；系统维护，前端只读）
    #[serde(default)]
    pub est_coding_plan: String,
    /// 上次真实 quota 查询毫秒戳（校准基准；系统维护，前端只读）
    #[serde(default)]
    pub last_real_query_at: i64,
    /// 自上次真查以来的预估次数（校准计数；系统维护，前端只读）
    #[serde(default)]
    pub estimate_count: i64,
    /// 是否在 tray 中展示此平台
    #[serde(default)]
    pub show_in_tray: bool,
    /// tray 展示类型: "balance" | "coding"
    #[serde(default)]
    pub tray_display: String,
    /// 排序权重（越小越靠前），0 = 按 created_at 排序
    #[serde(default)]
    pub sort_order: i64,
    /// 手动预算限额列表（仅无上游 quota 自动支持平台；请求驱动扣减 + 耗尽阻断）
    #[serde(default)]
    pub manual_budgets: Vec<ManualBudget>,
    /// 余额使用速率配色级别（非 DB 列；`platform_list` 按动态窗口日速率算 days_remaining 后填充）。
    /// "red"|"yellow"|"green"|"neutral"，前端列表页余额只消费此 level 不重算阈值（usage_color 唯一源）。
    /// 缺省空串 → 前端退中性。`skip_deserializing` 避免从前端入参反序列化。
    #[serde(default, skip_deserializing)]
    pub balance_level: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlatform {
    pub name: String,
    pub platform_type: Protocol,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub extra: String,
    #[serde(default)]
    pub models: Option<PlatformModels>,
    #[serde(default)]
    pub available_models: Option<Vec<String>>,
    #[serde(default)]
    pub endpoints: Option<Vec<PlatformEndpoint>>,
    #[serde(default)]
    pub manual_budgets: Option<Vec<ManualBudget>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlatform {
    pub id: u64,
    pub name: Option<String>,
    pub platform_type: Option<Protocol>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub extra: Option<String>,
    pub models: Option<PlatformModels>,
    pub available_models: Option<Vec<String>>,
    pub endpoints: Option<Vec<PlatformEndpoint>>,
    pub enabled: Option<bool>,
    pub manual_budgets: Option<Vec<ManualBudget>>,
}

// ─── Group ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
    /// URL path 前缀，如 "/claude"
    pub path: String,
    pub routing_mode: RoutingMode,
    /// 如果由平台自动创建，记录关联平台 ID（十进制字符串；空串表示非自动）
    pub auto_from_platform: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
    /// 超时设置（秒），0 = 继承系统设置
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    /// 入站协议（默认 anthropic）
    #[serde(default = "default_source_protocol")]
    pub source_protocol: String,
    /// 排序权重（越小越靠前），0 = 按 created_at 排序
    #[serde(default)]
    pub sort_order: i64,
    /// 模型映射（内联 JSON 数组）
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
}

fn default_source_protocol() -> String { "anthropic".to_string() }

#[derive(Debug, Deserialize)]
pub struct CreateGroup {
    pub name: String,
    pub path: String,
    pub routing_mode: RoutingMode,
    #[serde(default)]
    pub auto_from_platform: String,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    #[serde(default = "default_source_protocol_opt")]
    pub source_protocol: Option<String>,
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
}

fn default_source_protocol_opt() -> Option<String> { Some("anthropic".to_string()) }

#[derive(Debug, Deserialize)]
pub struct UpdateGroup {
    pub id: u64,
    pub name: Option<String>,
    pub path: Option<String>,
    pub routing_mode: Option<RoutingMode>,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    #[serde(default)]
    pub source_protocol: Option<String>,
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
}

// ─── GroupPlatform (关联) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct GroupPlatform {
    pub id: u64,
    pub group_id: u64,
    pub platform_id: u64,
    /// 故障转移优先级（越小越优先）
    pub priority: i32,
    /// 负载均衡权重
    pub weight: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct SetGroupPlatforms {
    pub group_id: u64,
    /// (platform_id, priority, weight) 列表
    pub platforms: Vec<GroupPlatformInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupPlatformInput {
    pub platform_id: u64,
    pub priority: Option<i32>,
    pub weight: Option<i32>,
}

// ─── ModelMapping ──────────────────────────────────────────

/// 内联于 group.model_mappings JSON 数组的元素
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMapping {
    /// 对外模型名，如 "claude-sonnet-4-6"
    pub source_model: String,
    pub target_platform_id: u64,
    /// 实际模型名，如 "glm-4-plus"
    pub target_model: String,
    /// 超时设置（秒），0 = 继承分组设置
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
}

// ─── 辅助：带平台详情的分组 ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDetail {
    pub group: Group,
    pub platforms: Vec<GroupPlatformDetail>,
    pub model_mappings: Vec<ModelMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPlatformDetail {
    pub platform: Platform,
    pub priority: i32,
    pub weight: i32,
}

// ─── Settings (KV) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SettingEntry {
    pub id: u64,
    pub scope: String,
    pub key: String,
    pub value: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct SetSettingInput {
    pub scope: String,
    pub key: String,
    pub value: serde_json::Value,
}

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

// ─── ProxyLog ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLog {
    pub id: String,
    pub group_name: String,
    /// 用户请求的原始模型
    pub model: String,
    /// 实际发送给上游的模型（可能因路由映射而不同）
    pub actual_model: String,
    /// 用户请求的协议（固定 anthropic）
    pub source_protocol: String,
    /// 实际请求上游的协议
    pub target_protocol: String,
    /// 路由到的目标平台 ID
    pub platform_id: u64,
    /// 原始请求头（用户发给代理的）
    pub request_headers: String,
    /// 原始请求体（用户发给代理的）
    pub request_body: String,
    /// 代理转发给上游的请求头
    pub upstream_request_headers: String,
    /// 代理转发给上游的请求体（协议转换后）
    pub upstream_request_body: String,
    /// 上游返回的响应体（非流式完整 JSON，流式为 "[stream]"）
    pub response_body: String,
    /// 用户请求的完整 URL
    #[serde(default)]
    pub request_url: String,
    /// 上游请求的完整 URL
    #[serde(default)]
    pub upstream_request_url: String,
    /// 上游返回的响应头
    #[serde(default)]
    pub upstream_response_headers: String,
    /// 上游 HTTP 状态码
    #[serde(default)]
    pub upstream_status_code: i32,
    /// 代理返回给用户的响应头
    #[serde(default)]
    pub user_response_headers: String,
    /// 代理返回给用户的响应体（非流式含模型名替换，流式为 "[stream]"）
    #[serde(default)]
    pub user_response_body: String,
    pub status_code: i32,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    /// 预估花费（$），基于 model_price 定价
    #[serde(default)]
    pub est_cost: f64,
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
}

/// 平台使用统计（从 proxy_logs 聚合）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformUsageStats {
    pub total_requests: i64,
    pub success_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_tokens: i64,
    pub cache_rate: f64,
    /// 最近 N 次请求中失败的次数（用于可用性判断）
    pub recent_failures: i64,
    /// 最近 N 次请求的总数
    pub recent_total: i64,
    /// 累计预估花费（$），基于 est_cost 聚合
    #[serde(default)]
    pub total_cost: f64,
}

/// Summary row for list view (excludes large body fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLogSummary {
    pub id: String,
    pub group_name: String,
    pub model: String,
    pub actual_model: String,
    pub source_protocol: String,
    pub target_protocol: String,
    pub platform_id: u64,
    pub status_code: i32,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    pub created_at: i64,
}

/// 日志列表筛选条件
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProxyLogFilter {
    pub platform_id: Option<u64>,
    pub group_name: Option<String>,
    /// None=全部; Some(200)=仅成功; Some(-1)=仅失败
    pub status: Option<i32>,
    pub time_start: Option<i64>,
    pub time_end: Option<i64>,
    pub model: Option<String>,
    /// "original" = 按 model 列; "actual" = 按 actual_model 列
    pub model_type: Option<String>,
}

/// Proxy logging settings stored in settings table (scope=proxy, key=logging)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLogSettings {
    /// Master switch: whether to log proxy requests at all
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether to record user's original request (headers + body)
    #[serde(default)]
    pub log_user_request: bool,

    /// Whether to record actual upstream request (headers + body)
    #[serde(default)]
    pub log_upstream_request: bool,

    /// Days to retain user request data (headers, body); 0 = keep forever
    #[serde(default = "default_user_req_retention")]
    pub user_request_retention_days: u32,

    /// Days to retain upstream request data (headers, body); 0 = keep forever
    #[serde(default = "default_upstream_req_retention")]
    pub upstream_request_retention_days: u32,

    /// Days to retain entire log record; 0 = keep forever
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_true() -> bool { true }
fn default_user_req_retention() -> u32 { 7 }
fn default_upstream_req_retention() -> u32 { 7 }
fn default_retention_days() -> u32 { 90 }

impl Default for ProxyLogSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            log_user_request: false,
            log_upstream_request: false,
            user_request_retention_days: default_user_req_retention(),
            upstream_request_retention_days: default_upstream_req_retention(),
            retention_days: default_retention_days(),
        }
    }
}

// ─── Proxy Timeout Settings ─────────────────────────────────

/// Upstream request timeout configuration (stored in settings table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTimeoutSettings {
    /// Total request timeout in seconds (0 = no limit)
    #[serde(default)]
    pub request_timeout_secs: u64,
    /// TCP connection timeout in seconds (0 = no limit)
    #[serde(default)]
    pub connect_timeout_secs: u64,
}

impl Default for ProxyTimeoutSettings {
    fn default() -> Self {
        Self {
            request_timeout_secs: 300,  // 5 minutes
            connect_timeout_secs: 10,   // 10 seconds
        }
    }
}

// ─── Proxy Client Settings (upstream HTTP proxy) ──────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyClientSettings {
    #[serde(default)]
    pub enabled: bool,
    /// "socks5" | "http" | "https"
    #[serde(default = "default_proxy_type")]
    pub proxy_type: String,
    #[serde(default = "default_proxy_host")]
    pub host: String,
    #[serde(default = "default_proxy_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    /// SOCKS5 时 DNS 走代理解析 (socks5h vs socks5)
    #[serde(default = "default_true")]
    pub dns_over_proxy: bool,
}

fn default_proxy_type() -> String { "socks5".to_string() }
fn default_proxy_host() -> String { "127.0.0.1".to_string() }
fn default_proxy_port() -> u16 { 7890 }

impl Default for ProxyClientSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_type: default_proxy_type(),
            host: default_proxy_host(),
            port: default_proxy_port(),
            username: String::new(),
            password: String::new(),
            dns_over_proxy: true,
        }
    }
}

impl ProxyClientSettings {
    /// Build a reqwest::Proxy from settings. Returns None if not enabled.
    pub fn to_reqwest_proxy(&self) -> Option<reqwest::Proxy> {
        if !self.enabled { return None; }
        let scheme = match self.proxy_type.as_str() {
            "socks5" if self.dns_over_proxy => "socks5h",
            "socks5" => "socks5",
            "https" => "https",
            _ => "http",
        };
        let url = format!("{}://{}:{}", scheme, self.host, self.port);
        let mut proxy = reqwest::Proxy::all(&url)
            .map_err(|e| { tracing::warn!("invalid proxy URL {}: {e}", url); e })
            .ok()?;
        if !self.username.is_empty() {
            proxy = proxy.basic_auth(&self.username, &self.password);
        }
        Some(proxy)
    }
}

// ─── Statistics ───────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct StatsQuery {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub granularity: Option<String>,
    pub group_by: Option<String>,
    pub filter_group: Option<String>,
    pub filter_model: Option<String>,
    pub filter_protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsOverview {
    pub total_requests: i32,
    pub success_rate: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_tokens: i64,
    pub cache_rate: f64,
    pub avg_duration_ms: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsBucket {
    pub time_bucket: String,
    pub total_requests: i32,
    pub success_count: i32,
    pub error_count: i32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_tokens: i64,
    pub avg_duration_ms: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionEntry {
    pub name: String,
    pub total_requests: i32,
    pub success_count: i32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_tokens: i64,
    pub avg_duration_ms: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResult {
    pub overview: StatsOverview,
    pub buckets: Vec<StatsBucket>,
    pub dimension_data: Vec<DimensionEntry>,
}

// ─── Model Testing ────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ModelTestRequest {
    pub platform_id: u64,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTestResult {
    pub success: bool,
    pub model: String,
    pub prompt_preview: String,
    pub response_preview: String,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub error: String,
}

/// Built-in test prompts — short, harmless, clearly not real requests
#[allow(dead_code)]
pub const TEST_PROMPTS: &[&str] = &[
    "Respond with only the word 'hello' in lowercase.",
    "Calculate 7 x 13 and respond with only the number.",
    "List exactly 3 primary colors, comma-separated.",
    "What is the capital of France? Answer in one word.",
    "Translate 'good morning' to Japanese. One word only.",
    "Count the letters in 'artificial'. Respond with only the number.",
    "What is 15% of 200? Answer with only the number.",
    "Name the 4th planet from the Sun. One word.",
    "What element has the symbol 'O'? One word.",
    "How many days are in a leap year? Answer with only the number.",
];

// ─── Model Price ───────────────────────────────────────────

/// 模型价格记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    pub id: u64,
    pub model_name: String,
    /// "litellm" | "manual"
    pub source: String,
    /// JSON: {input_cost_per_token, output_cost_per_token, cache_read_input_token_cost, pricing: {platform_type: {...}}, default_platform, ...}
    pub price_data: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
}

/// 模型价格摘要（列表展示用，解析了关键字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPriceSummary {
    pub id: u64,
    pub model_name: String,
    pub source: String,
    pub default_platform: Option<String>,
    /// $/M input tokens
    pub input_price: Option<f64>,
    /// $/M output tokens
    pub output_price: Option<f64>,
    /// $/M cache read tokens
    pub cache_read_price: Option<f64>,
    pub updated_at: i64,
}

/// 价格解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPrice {
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    pub cache_read_input_token_cost: f64,
    pub source: String,  // "platform_override" | "default_platform" | "top_level" | "fallback"
}

/// 模型价格同步设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSyncSettings {
    #[serde(default)]
    pub auto_sync_enabled: bool,
    /// 同步间隔（秒），默认 86400 = 24h
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
    /// 上次同步时间（ms timestamp）
    #[serde(default)]
    pub last_sync_at: i64,
    /// 兜底默认价格 $/M tokens
    #[serde(default = "default_fallback_price")]
    pub fallback_input_price: f64,
    #[serde(default = "default_fallback_price")]
    pub fallback_output_price: f64,
}

fn default_sync_interval() -> u64 { 86400 }
fn default_fallback_price() -> f64 { 3.0 }

impl Default for PriceSyncSettings {
    fn default() -> Self {
        Self {
            auto_sync_enabled: false,
            sync_interval_secs: default_sync_interval(),
            last_sync_at: 0,
            fallback_input_price: default_fallback_price(),
            fallback_output_price: default_fallback_price(),
        }
    }
}

/// 同步结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSyncResult {
    pub added: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub failed: u32,
    pub total: u32,
}
