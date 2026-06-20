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
    /// OpenCode Zen 免费版（OpenAI 兼容，base_url https://opencode.ai/zen/v1；
    /// 免费模型靠 catalog 定价 0；api_key 留空时 proxy 注入 $opencode 匿名免费 key）
    #[serde(rename = "opencode_zen")]
    OpenCodeZen,
    // ── 中转平台 ──
    #[serde(rename = "newapi")]
    NewApi,
}

/// 路由模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum RoutingMode {
    #[serde(rename = "load_balance")]
    LoadBalance,
    #[serde(rename = "failover")]
    Failover,
    /// 健康集加权随机：准入门摘除熔断 Open 平台后，在健康平台中按 weight 加权随机。
    #[serde(rename = "health_aware")]
    HealthAware,
    /// 最小延迟：按 per-platform 延迟 EMA 升序。
    #[serde(rename = "least_latency")]
    LeastLatency,
    /// 粘性会话：session 键绑定平台（若健康），否则回退加权随机并写绑定。
    #[serde(rename = "sticky")]
    Sticky,
}

impl RoutingMode {
    /// 从 settings 默认字面量解析；未知 → LoadBalance（向后兼容）。
    /// 供 SchedulingBreakerSettings::default_mode 与 GB 创建 Group 时取全局默认用。
    #[allow(dead_code)]
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "failover" => RoutingMode::Failover,
            "health_aware" => RoutingMode::HealthAware,
            "least_latency" => RoutingMode::LeastLatency,
            "sticky" => RoutingMode::Sticky,
            _ => RoutingMode::LoadBalance,
        }
    }
}

/// 平台状态三态：用户启用 / 用户手动禁用 / 401-403 自动禁用。
/// 自动禁用与手动禁用必须区分——自动恢复（退避试探 / 改 api_key）只作用于 auto_disabled，
/// 绝不误开用户主动关闭的平台。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PlatformStatus {
    #[serde(rename = "enabled")]
    #[default]
    Enabled,
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "auto_disabled")]
    AutoDisabled,
}

impl PlatformStatus {
    /// DB 文本值（与 `serde(rename)` 一致）
    pub fn as_db_str(&self) -> &'static str {
        match self {
            PlatformStatus::Enabled => "enabled",
            PlatformStatus::Disabled => "disabled",
            PlatformStatus::AutoDisabled => "auto_disabled",
        }
    }

    /// 从 DB 文本解析；未知值回退 Enabled（向后兼容旧库 / 脏数据）。
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "disabled" => PlatformStatus::Disabled,
            "auto_disabled" => PlatformStatus::AutoDisabled,
            _ => PlatformStatus::Enabled,
        }
    }
}

/// proxy_log.attempts JSON 数组元素：每次平台尝试的快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyAttempt {
    pub platform_id: u64,
    pub platform_name: String,
    /// 上游返回的 HTTP 状态码；连接失败 / 超时为 0
    pub status_code: i32,
    /// 错误描述（连接失败 / 超时 / 上游错误体摘要）；成功为空串
    #[serde(default)]
    pub error: String,
    pub duration_ms: i64,
    /// 本次尝试发起时间（毫秒 unix 时间戳）
    pub ts: i64,
}

/// 序列化 attempts 列（出错回退空数组）
pub fn serialize_attempts(items: &[ProxyAttempt]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string())
}

/// 解析 attempts 列（出错回退空数组）
pub fn parse_attempts(s: &str) -> Vec<ProxyAttempt> {
    serde_json::from_str(s).unwrap_or_default()
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
    /// 旧布尔启用位，保留向后兼容（旧读者 / 旧前端）。写入端从 status 同步：
    /// `status==Enabled → true`，否则 false。新逻辑（router 过滤 / 前端三态）走 status。
    pub enabled: bool,
    /// 三态状态：enabled / disabled(用户手动) / auto_disabled(401/403 自动)
    #[serde(default)]
    pub status: PlatformStatus,
    /// auto_disabled 下次试探时间（毫秒 unix 时间戳）；退避用，0 = 立即可试探
    #[serde(default)]
    pub auto_disabled_until: i64,
    /// 连续自动禁用次数（指数退避指数）；恢复 enabled 时清零
    #[serde(default)]
    pub auto_disable_strikes: i64,
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

/// 平台级熔断阈值覆盖，存于 `platform.extra` JSON 的嵌套对象 `breaker`。
/// 每字段 0/缺省 = 继承全局 `SchedulingBreakerSettings` 默认（语义同旧顶层列）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlatformBreaker {
    #[serde(default)]
    pub failure_threshold: u32,
    #[serde(default)]
    pub open_secs: u64,
    #[serde(default)]
    pub half_open_max: u32,
}

/// 从 `extra` JSON 字符串解析 `breaker` 嵌套对象；空/非法/缺键 → 全 0（继承全局默认）。
pub fn parse_breaker(extra: &str) -> PlatformBreaker {
    if extra.trim().is_empty() {
        return PlatformBreaker::default();
    }
    serde_json::from_str::<serde_json::Value>(extra)
        .ok()
        .and_then(|v| v.get("breaker").cloned())
        .and_then(|b| serde_json::from_value(b).ok())
        .unwrap_or_default()
}

/// 把 breaker 阈值合并进 `extra` JSON 的 `breaker` 键（保留 extra 其余字段）。
/// 三值全 0 时移除 `breaker` 键（无覆盖 → 继承全局，不留空对象）。空 extra → "{}" 起步。
pub fn merge_breaker_into_extra(extra: &str, b: &PlatformBreaker) -> String {
    let mut root = serde_json::from_str::<serde_json::Value>(extra.trim())
        .ok()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    let obj = root.as_object_mut().expect("object");
    if b.failure_threshold == 0 && b.open_secs == 0 && b.half_open_max == 0 {
        obj.remove("breaker");
    } else {
        obj.insert("breaker".to_string(), serde_json::to_value(b).unwrap_or_default());
    }
    serde_json::to_string(&root).unwrap_or_else(|_| "{}".to_string())
}

impl Platform {
    /// 解析本平台 `extra.breaker` 覆盖阈值（缺省全 0 = 继承全局默认）。
    pub fn breaker(&self) -> PlatformBreaker {
        parse_breaker(&self.extra)
    }
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
    /// 是否自动创建默认分组（transient 输入，不入库）：None→true 保持旧行为；
    /// false=创建时不建默认分组。该选择是创建时一次性判断，不持久化。
    #[serde(default)]
    pub auto_group: Option<bool>,
    /// 额外加入的已有分组 ID 列表（plain membership，不写 auto_from_platform）。
    #[serde(default)]
    pub join_group_ids: Option<Vec<u64>>,
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
    /// 前端三态切换：显式置 enabled / disabled。
    /// 注意：禁止前端直接置 auto_disabled（仅系统 401/403 联动设置）；置 enabled 会清空退避状态。
    pub status: Option<PlatformStatus>,
    pub manual_budgets: Option<Vec<ManualBudget>>,
    /// 全量同步该平台的手动组成员关系（None=不动；Some(set)=加入 set 内、移出 set 外，
    /// auto 分组不受影响）。
    /// 注：熔断阈值覆盖现走 `extra.breaker`（随 `extra` 字段整体更新），不再有独立列。
    #[serde(default)]
    pub join_group_ids: Option<Vec<u64>>,
}

// ─── Group ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
    /// 分组密钥：Bearer token + 路由匹配键 + proxy_log 归属键（前端按 group_key 反查 name 显示）。
    /// UNIQUE。创建时若未提供则自动生成 `gk_<32hex>`；创建后锁定不可改。
    #[serde(default)]
    pub group_key: String,
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
    /// 分组级最大重试次数：失败后最多再换几个候选平台（0 = 不重试，只试 1 次）
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// 模型映射（内联 JSON 数组）
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
    /// 是否为默认分组（单选）：true 时该组 config merge 写入
    /// `~/.claude/settings.json` + `~/.codex/config.toml`，使用户直接 `claude`/`codex`
    /// 不带 `-c`/`--profile` 即走此组。全局文件用 deep merge 保护用户其它字段。
    #[serde(default)]
    pub is_default: bool,
}

fn default_source_protocol() -> String { "anthropic".to_string() }
fn default_max_retries() -> u32 { 10 }

#[derive(Debug, Deserialize)]
pub struct CreateGroup {
    pub name: String,
    /// 分组密钥；None 或空 → 自动生成 `gk_<32hex>`。创建后锁定不可改。
    #[serde(default)]
    pub group_key: Option<String>,
    pub routing_mode: RoutingMode,
    #[serde(default)]
    pub auto_from_platform: String,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    #[serde(default = "default_source_protocol_opt")]
    pub source_protocol: Option<String>,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
}

fn default_source_protocol_opt() -> Option<String> { Some("anthropic".to_string()) }

#[derive(Debug, Deserialize)]
pub struct UpdateGroup {
    pub id: u64,
    pub name: Option<String>,
    pub routing_mode: Option<RoutingMode>,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    #[serde(default)]
    pub source_protocol: Option<String>,
    /// 分组级最大重试次数；None = 不变（保留既有值）
    #[serde(default)]
    pub max_retries: Option<u32>,
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
    /// 默认分组标记：本字段不参与 update_group UPDATE（默认组经 group_set_default
    /// command + db::set_default_group 单选切换）。这里保留仅为统一 struct 形态，
    /// update_group 返回的 `..existing` 透传原值，不丢失。
    #[serde(default)]
    #[allow(dead_code)]
    pub is_default: Option<bool>,
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
    /// per-group 平台优先级（1~10，默认 5，10=最高优先；数大优先高）
    #[serde(default = "default_level_priority")]
    pub level_priority: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
}

/// level_priority 默认值（5 = 中等优先）
pub fn default_level_priority() -> i32 {
    5
}

/// 把 level_priority clamp 到合法区间 [1, 10]
pub fn clamp_level_priority(v: i32) -> i32 {
    v.clamp(1, 10)
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
    /// per-group 平台优先级（1~10，None → 默认 5）
    #[serde(default)]
    pub level_priority: Option<i32>,
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
    /// per-group 平台优先级（1~10，默认 5，10=最高优先）
    #[serde(default = "default_level_priority")]
    pub level_priority: i32,
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

#[cfg(test)]
mod popover_config_model_tests {
    use super::*;

    #[test]
    fn legacy_item_without_trend_fields_deserializes() {
        // 旧配置（无 scope / scope_ref / time_window）必须反序列化成功并取默认值。
        let json = r#"{"id":"popover-today_cost","item_type":"today_cost","visible":true,"order":2}"#;
        let item: PopoverItem = serde_json::from_str(json).expect("legacy item must deserialize");
        assert_eq!(item.item_type, "today_cost");
        assert_eq!(item.scope, "overall");
        assert!(item.scope_ref.is_none());
        assert_eq!(item.time_window, "7d");
        // 旧配置无 row/size/color → serde default 兜底。
        assert_eq!(item.row, 0);
        assert_eq!(item.size, "m");
        assert_eq!(item.color.mode, "follow");
        assert_eq!(item.color.value, "");
    }

    #[test]
    fn cost_trend_item_roundtrips() {
        let item = PopoverItem {
            id: "popover-trend-1".to_string(),
            item_type: "cost_trend".to_string(),
            visible: true,
            order: 0,
            scope: "group".to_string(),
            scope_ref: Some("gk_abc".to_string()),
            time_window: "30d".to_string(),
            row: 2,
            size: "l".to_string(),
            color: TrayColor { mode: "custom".to_string(), value: "#ff8800".to_string() },
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: PopoverItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back.scope, "group");
        assert_eq!(back.scope_ref.as_deref(), Some("gk_abc"));
        assert_eq!(back.time_window, "30d");
        assert_eq!(back.row, 2);
        assert_eq!(back.size, "l");
        assert_eq!(back.color.mode, "custom");
        assert_eq!(back.color.value, "#ff8800");
    }

    #[test]
    fn legacy_config_without_new_fields_deserializes() {
        let json = r#"{"items":[{"id":"a","item_type":"proxy_status","visible":true,"order":0}]}"#;
        let cfg: PopoverConfig = serde_json::from_str(json).expect("legacy config must deserialize");
        assert_eq!(cfg.items.len(), 1);
        assert_eq!(cfg.items[0].scope, "overall");
        assert_eq!(cfg.items[0].time_window, "7d");
        // 旧配置无 rows → 空 vec；item 新字段取默认。
        assert!(cfg.rows.is_empty());
        assert_eq!(cfg.items[0].row, 0);
        assert_eq!(cfg.items[0].size, "m");
        assert_eq!(cfg.items[0].color.mode, "follow");
    }

    #[test]
    fn config_with_rows_roundtrips() {
        // 含二维布局新字段的完整配置往返。
        let json = r#"{
            "items":[{"id":"a","item_type":"today_cost","visible":true,"order":0,"row":0,"size":"s","color":{"mode":"preset","value":"green"}}],
            "rows":[{"cols":2},{"cols":3}]
        }"#;
        let cfg: PopoverConfig = serde_json::from_str(json).expect("config with rows must deserialize");
        assert_eq!(cfg.rows.len(), 2);
        assert_eq!(cfg.rows[0].cols, 2);
        assert_eq!(cfg.rows[1].cols, 3);
        assert_eq!(cfg.items[0].size, "s");
        assert_eq!(cfg.items[0].color.mode, "preset");
        assert_eq!(cfg.items[0].color.value, "green");

        // 序列化回去再读，字段保真。
        let s = serde_json::to_string(&cfg).unwrap();
        let back: PopoverConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(back.rows[0].cols, 2);
        assert_eq!(back.items[0].size, "s");
    }

    #[test]
    fn row_meta_without_cols_defaults_to_one() {
        // rows 项缺 cols → default_cols=1。
        let json = r#"{"items":[],"rows":[{}]}"#;
        let cfg: PopoverConfig = serde_json::from_str(json).expect("row without cols must deserialize");
        assert_eq!(cfg.rows[0].cols, 1);
    }

    #[test]
    fn default_config_populates_new_fields() {
        let cfg = PopoverConfig::default();
        // 默认配置各 item row=order（各占一行），size="m"，color follow。
        for (i, item) in cfg.items.iter().enumerate() {
            assert_eq!(item.row, i as i32);
            assert_eq!(item.size, "m");
            assert_eq!(item.color.mode, "follow");
        }
        assert!(cfg.rows.is_empty());
    }
}

// ─── ProxyLog ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLog {
    pub id: String,
    pub group_key: String,
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
    /// 是否为流式（SSE）请求；流式日志的 body 为聚合的真实 SSE 内容（非 "[stream]" 哨兵）
    #[serde(default)]
    pub is_stream: bool,
    /// 每次平台尝试快照（JSON 数组列）；单平台一次成功时长度 1
    #[serde(default)]
    pub attempts: Vec<ProxyAttempt>,
    /// 重试次数 = attempts.len()-1（0 表示一次成功，无重试）
    #[serde(default)]
    pub retry_count: i32,
    /// 被中间件拦截时的规则标识（rule_type/规则名/id 拼接，空表示未被拦截）。C2 入站 block 写入。
    #[serde(default)]
    pub blocked_by: String,
    /// 拦截原因（命中模式 / 规则描述等人读说明，空表示未被拦截）。
    #[serde(default)]
    pub blocked_reason: String,
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
    /// 今日（本地 00:00 起）token 总量（input + output），按 eff_pid 聚合
    #[serde(default)]
    pub today_tokens: i64,
    /// 今日（本地 00:00 起）预估花费（$），基于 est_cost 聚合
    #[serde(default)]
    pub today_cost: f64,
}

/// 平台「最近一次测试结果」（来自 proxy_log 中 source_protocol='test' 的最新一条）。
/// 供 PlatformCard 常驻徽章消费：ok/fail + 耗时 + 时间。
#[derive(Debug, Clone, Serialize)]
pub struct LastTestResult {
    /// status_code ∈ [200, 300) → true
    pub success: bool,
    pub status_code: i32,
    pub duration_ms: i32,
    /// proxy_log.created_at（毫秒 epoch）
    pub created_at: i64,
    /// 失败时取 response_body 截断 ~200 字符；成功为空串
    pub error: String,
}

/// Summary row for list view (excludes large body fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLogSummary {
    pub id: String,
    pub group_key: String,
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
    /// 是否为流式（SSE）请求；列表展示流式标记
    #[serde(default)]
    pub is_stream: bool,
    /// 重试次数（retry_count>0 时列表显示重试徽标）
    #[serde(default)]
    pub retry_count: i32,
    pub created_at: i64,
}

/// 日志列表筛选条件
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProxyLogFilter {
    pub platform_id: Option<u64>,
    pub group_key: Option<String>,
    /// None=全部; Some(200)=仅成功; Some(-1)=仅失败
    pub status: Option<i32>,
    pub time_start: Option<i64>,
    pub time_end: Option<i64>,
    pub model: Option<String>,
    /// "original" = 按 model 列; "actual" = 按 actual_model 列
    pub model_type: Option<String>,
    /// 路径片段：对 request_url 做 LIKE %v% 模糊匹配
    #[serde(default)]
    pub path: Option<String>,
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
    pub filter_platform: Option<String>,
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
    /// 当前筛选范围（日期 + 分组 + 平台，不含 filter_model）内实际有记录的模型名，
    /// 供前端模型筛选下拉使用（避免列出配置过但无请求的模型）。
    pub available_models: Vec<String>,
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

/// 内置常识问答题库（问题, 期望关键词）。
/// 关键词须极短且为模型自然回答的开头，以便在 max_tokens=16 截断下仍可校验。
#[allow(dead_code)]
pub const TEST_TRIVIA: &[(&str, &str)] = &[
    ("中国的首都是哪个城市？", "北京"),
    ("一年有几个月？", "12"),
    ("水的化学式是什么？", "H2O"),
    ("地球有几个卫星（月亮）？", "1"),
    ("一周有几天？", "7"),
    ("彩虹有几种颜色？", "7"),
    ("太阳从哪个方向升起？", "东"),
    ("一个三角形有几条边？", "3"),
    ("人类有几只手？", "2"),
    ("英文字母表有几个字母？", "26"),
];

/// 生成一道随机可校验的测试题，返回 `(prompt, expected)`。
/// 两类轮换：算术（随机两位数 +/-/×）与常识问答。
/// prompt 每次随机 → 防指纹；expected 为归一化后用于子串校验的极短答案。
#[allow(dead_code)]
pub fn random_test_challenge() -> (String, String) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    if rng.gen_bool(0.5) {
        // 算术：两位数 10..=99
        let a: i64 = rng.gen_range(10..=99);
        let b: i64 = rng.gen_range(10..=99);
        match rng.gen_range(0..3) {
            0 => (format!("{} 加 {} 等于多少？", a, b), (a + b).to_string()),
            1 => {
                // 保证非负，便于关键词在开头
                let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
                (format!("{} 减 {} 等于多少？", hi, lo), (hi - lo).to_string())
            }
            _ => (format!("{} 乘以 {} 等于多少？", a, b), (a * b).to_string()),
        }
    } else {
        let (q, ans) = TEST_TRIVIA[rng.gen_range(0..TEST_TRIVIA.len())];
        (q.to_string(), ans.to_string())
    }
}

/// 归一化文本用于子串校验：转小写 + 去除空白/标点（仅保留字母数字与 CJK）。
/// 同一规则同时作用于响应文本与 expected，避免 "H2O"/"h2o"、"北京。"/"北京" 等差异。
#[allow(dead_code)]
pub fn normalize_for_match(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 测试响应内容校验（产线 model_test 复用，保证单测覆盖真实逻辑）。返回 true=通过。
///
/// - `expected = Some(exp)`（随机可校验题）：归一化后响应须含 expected 子串。
/// - `expected = None`（自定义 prompt）：跳过内容校验，仅要求响应非空。
#[allow(dead_code)]
pub fn verify_test_response(response_text: &str, expected: Option<&str>) -> bool {
    match expected {
        Some(exp) => {
            let norm_exp = normalize_for_match(exp);
            !norm_exp.is_empty() && normalize_for_match(response_text).contains(&norm_exp)
        }
        None => !response_text.trim().is_empty(),
    }
}

// ─── Model Price ───────────────────────────────────────────

/// 模型价格记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    pub id: u64,
    pub model_name: String,
    /// "github" | "manual"
    pub source: String,
    /// JSON: {input_cost_per_token, output_cost_per_token, cache_read_input_token_cost, pricing: {platform_type: {...}}, default_platform, ...}
    pub price_data: String,
    /// 最大输入 token（模型固有，平台无关）。NULL = 未知。
    #[serde(default)]
    pub max_input_tokens: Option<i64>,
    /// 最大输出 token（出站裁剪用）。NULL = 未知/无限制。
    #[serde(default)]
    pub max_output_tokens: Option<i64>,
    /// 上下文窗口。NULL = 未知。
    #[serde(default)]
    pub context_window: Option<i64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<i64>,
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

// ─── Middleware Rule Engine (C1 基座) ──────────────────────────
//
// 8 类请求/响应中间件规则的公共数据模型。表 `middleware_rule` 单表存储，
// 枚举全部 snake_case serde，与 src/services/api.ts 字面量联合类型一一对齐
// （契约见 .trellis/tasks/06-13-request-response-middleware/design.md）。
// 实际执行（入站/出站 apply）由 C2/C3 在 proxy.rs 落地；本文件只定义模型。

/// 规则类型（8 类中间件能力）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    /// 请求字段过滤（model 白/黑名单等）
    RequestFilter,
    /// 敏感词拦截（pattern 即词）
    SensitiveWord,
    /// 脱敏（字段值替换）
    Redaction,
    /// 内容过滤
    ContentFilter,
    /// 动态注入（system/header/body）
    DynamicInjection,
    /// 响应覆写（成功体改写）
    ResponseOverride,
    /// 矫正器（SSE/JSON/编码/字段缺省修复）
    Rectifier,
    /// 错误分类规则（重试/熔断/覆写状态码）
    ErrorRule,
}

impl RuleType {
    /// DB TEXT 列值（与 serde snake_case 一致）。
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleType::RequestFilter => "request_filter",
            RuleType::SensitiveWord => "sensitive_word",
            RuleType::Redaction => "redaction",
            RuleType::ContentFilter => "content_filter",
            RuleType::DynamicInjection => "dynamic_injection",
            RuleType::ResponseOverride => "response_override",
            RuleType::Rectifier => "rectifier",
            RuleType::ErrorRule => "error_rule",
        }
    }

    /// 从 DB TEXT 值解析；未知值返回 None（fail-open：调用方跳过该行）。
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "request_filter" => Some(RuleType::RequestFilter),
            "sensitive_word" => Some(RuleType::SensitiveWord),
            "redaction" => Some(RuleType::Redaction),
            "content_filter" => Some(RuleType::ContentFilter),
            "dynamic_injection" => Some(RuleType::DynamicInjection),
            "response_override" => Some(RuleType::ResponseOverride),
            "rectifier" => Some(RuleType::Rectifier),
            "error_rule" => Some(RuleType::ErrorRule),
            _ => None,
        }
    }
}

/// 规则作用域（三级，就近覆盖语义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleScope {
    /// 全局：所有请求
    Global,
    /// 分组：scope_ref = group_key
    Group,
    /// 平台：scope_ref = platform_id(字符串)
    Platform,
}

impl RuleScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleScope::Global => "global",
            RuleScope::Group => "group",
            RuleScope::Platform => "platform",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "group" => RuleScope::Group,
            "platform" => RuleScope::Platform,
            // 未知/空 → global（最安全的兜底层）
            _ => RuleScope::Global,
        }
    }
}

/// 匹配方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    /// 正则（regex crate，无回溯抗 ReDoS）
    Regex,
    /// 子串包含
    Contains,
    /// 完全相等
    Exact,
}

impl MatchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchType::Regex => "regex",
            MatchType::Contains => "contains",
            MatchType::Exact => "exact",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "regex" => MatchType::Regex,
            "exact" => MatchType::Exact,
            // 默认 contains（与表 DEFAULT 一致）
            _ => MatchType::Contains,
        }
    }
}

/// 命中动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    /// 脱敏遮罩
    Mask,
    /// 拦截（立即返回 4xx）
    Block,
    /// 仅告警
    Warn,
    /// 注入
    Inject,
    /// 覆写
    Override,
    /// 分类（error_rule）
    Classify,
}

impl RuleAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleAction::Mask => "mask",
            RuleAction::Block => "block",
            RuleAction::Warn => "warn",
            RuleAction::Inject => "inject",
            RuleAction::Override => "override",
            RuleAction::Classify => "classify",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "mask" => RuleAction::Mask,
            "block" => RuleAction::Block,
            "inject" => RuleAction::Inject,
            "override" => RuleAction::Override,
            "classify" => RuleAction::Classify,
            // 默认 warn（与表 DEFAULT 一致，最弱副作用）
            _ => RuleAction::Warn,
        }
    }
}

/// 单条中间件规则（对应 `middleware_rule` 表一行）。
///
/// `config` 是 type-specific JSON 字符串（设计文档列出每类形状），
/// 引擎层不强解析，由各执行器（C2/C3）按需 `serde_json::from_str`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareRule {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub rule_type: RuleType,
    #[serde(default = "default_rule_scope")]
    pub scope: RuleScope,
    /// group_key | platform_id(字符串) | ''(global)
    #[serde(default)]
    pub scope_ref: String,
    #[serde(default = "default_match_type")]
    pub match_type: MatchType,
    #[serde(default)]
    pub pattern: String,
    #[serde(default = "default_rule_action")]
    pub action: RuleAction,
    /// type-specific JSON（默认 "{}"）
    #[serde(default = "default_config_json")]
    pub config: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_builtin: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

fn default_rule_scope() -> RuleScope { RuleScope::Global }
fn default_match_type() -> MatchType { MatchType::Contains }
fn default_rule_action() -> RuleAction { RuleAction::Warn }
fn default_config_json() -> String { "{}".to_string() }

/// 创建规则入参（前端不传 id/时间戳）。
#[derive(Debug, Clone, Deserialize)]
pub struct CreateMiddlewareRule {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub rule_type: RuleType,
    #[serde(default = "default_rule_scope")]
    pub scope: RuleScope,
    #[serde(default)]
    pub scope_ref: String,
    #[serde(default = "default_match_type")]
    pub match_type: MatchType,
    #[serde(default)]
    pub pattern: String,
    #[serde(default = "default_rule_action")]
    pub action: RuleAction,
    #[serde(default = "default_config_json")]
    pub config: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_builtin: bool,
}

/// 更新规则入参（全量覆盖，id 必填）。
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMiddlewareRule {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub rule_type: RuleType,
    #[serde(default = "default_rule_scope")]
    pub scope: RuleScope,
    #[serde(default)]
    pub scope_ref: String,
    #[serde(default = "default_match_type")]
    pub match_type: MatchType,
    #[serde(default)]
    pub pattern: String,
    #[serde(default = "default_rule_action")]
    pub action: RuleAction,
    #[serde(default = "default_config_json")]
    pub config: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_builtin: bool,
}

/// 中间件总设置（settings KV：scope="middleware" key="settings"）。
///
/// `enabled` 为总开关（OFF = 全旁路）；`type_toggles` 按 rule_type 子开关
/// （缺省视为 true，即默认所有类型启用）。
/// 注：熔断器已移出中间件层，归 group 功能块独立 task 实现，本结构不含 breaker。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// key = rule_type snake_case 字面量；缺省键视为 true。
    #[serde(default)]
    pub type_toggles: std::collections::HashMap<String, bool>,
}

impl Default for MiddlewareSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            type_toggles: std::collections::HashMap::new(),
        }
    }
}

impl MiddlewareSettings {
    /// 指定 rule_type 是否启用：总开关关 → 全 false；否则查 type_toggles，缺省 true。
    /// C2/C3 执行层判定调用。
    pub fn type_enabled(&self, rule_type: RuleType) -> bool {
        if !self.enabled {
            return false;
        }
        self.type_toggles
            .get(rule_type.as_str())
            .copied()
            .unwrap_or(true)
    }
}

// ─── Scheduling & Breaker Settings ─────────────────────────

/// 全局调度 + 熔断默认设置（settings KV scope=`scheduling`, key=`settings`）。
/// Platform 的 `extra.breaker` 覆盖值为 0/缺省时继承本结构对应默认值。
/// `enabled=false` 时熔断总开关旁路（候选过滤不踢任何 Open 平台）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingBreakerSettings {
    /// 全局默认调度策略字面量（与 RoutingMode serde rename 对齐）；Group routing_mode 覆盖之。
    #[serde(default = "default_routing_mode_str")]
    pub default_routing_mode: String,
    /// 全局默认熔断失败阈值（连续失败达此数 → Open）。
    #[serde(default = "default_breaker_failure_threshold")]
    pub breaker_failure_threshold: u32,
    /// 全局默认 Open 持续秒数。
    #[serde(default = "default_breaker_open_secs")]
    pub breaker_open_secs: u64,
    /// 全局默认 HalfOpen 最大探测数。
    #[serde(default = "default_breaker_half_open_max")]
    pub breaker_half_open_max: u32,
    /// 熔断总开关。
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_routing_mode_str() -> String { "health_aware".to_string() }
fn default_breaker_failure_threshold() -> u32 { 5 }
fn default_breaker_open_secs() -> u64 { 60 }
fn default_breaker_half_open_max() -> u32 { 2 }

impl Default for SchedulingBreakerSettings {
    fn default() -> Self {
        Self {
            default_routing_mode: default_routing_mode_str(),
            breaker_failure_threshold: default_breaker_failure_threshold(),
            breaker_open_secs: default_breaker_open_secs(),
            breaker_half_open_max: default_breaker_half_open_max(),
            enabled: true,
        }
    }
}

impl SchedulingBreakerSettings {
    /// 解析某平台的有效熔断阈值：平台字段非 0 用之，否则全局默认。
    /// 返回 (failure_threshold, open_secs, half_open_max)。
    pub fn effective_thresholds(&self, platform: &Platform) -> (u32, u64, u32) {
        let b = platform.breaker();
        let ft = if b.failure_threshold > 0 {
            b.failure_threshold
        } else {
            self.breaker_failure_threshold
        };
        let os = if b.open_secs > 0 {
            b.open_secs
        } else {
            self.breaker_open_secs
        };
        let hom = if b.half_open_max > 0 {
            b.half_open_max
        } else {
            self.breaker_half_open_max
        };
        (ft.max(1), os.max(1), hom.max(1))
    }

    /// 全局默认调度策略解析为 RoutingMode（GB 创建 Group 时取初值）。
    #[allow(dead_code)]
    pub fn default_mode(&self) -> RoutingMode {
        RoutingMode::from_str_or_default(&self.default_routing_mode)
    }
}

// ─── Notification（系统通知模块 N1）───────────────────────────

/// 通知类型枚举（serde snake_case）。3 类型：task_complete / waiting_input / error。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotifType {
    TaskComplete,
    WaitingInput,
    Error,
}

impl NotifType {
    /// 用于 per_type HashMap key / DB notif_type 列的字面量（与 serde snake_case 对齐）。
    pub fn as_str(&self) -> &'static str {
        match self {
            NotifType::TaskComplete => "task_complete",
            NotifType::WaitingInput => "waiting_input",
            NotifType::Error => "error",
        }
    }

    /// 从字面量解析；未知/空 → TaskComplete（端点收到任意 type 字符串都可分发，通知不丢）。
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "task_complete" => NotifType::TaskComplete,
            "waiting_input" => NotifType::WaitingInput,
            "error" => NotifType::Error,
            _ => NotifType::TaskComplete,
        }
    }

    /// 内置默认模板（每类型均有；render 在 setting.template 空时兜底使用，无项目名时给
    /// `{project}` 注入品牌兜底名）。用户在设置页留空 template → 自动展示本默认。
    /// **跨层镜像**：前端逐字镜像于 `src/components/settings/NotificationSettings.tsx`
    /// 的 `NOTIF_DEFAULT_TEMPLATES`，改此处务必同步前端（zh 硬编码，非 i18n）。
    pub fn default_template(&self) -> &'static str {
        match self {
            NotifType::TaskComplete => "{project} 完成",
            NotifType::WaitingInput => "{project} 等待用户输入",
            NotifType::Error => "{project} 出错",
        }
    }
}

/// 呈现形态：完整播报 / 仅弹窗 / 仅收件箱 / 仅提示音。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NotifForm {
    PopupOnly,
    InboxOnly,
    SoundOnly,
    #[default]
    Full,
}

/// TTS 后端：跨平台 tts crate（默认）/ macOS `say` 命令 / 前端 WebSpeech。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TtsBackend {
    #[default]
    CrossPlatform,
    MacSay,
    WebSpeech,
}

/// 单类型通知配置（per_type 值）。template 含变量占位（{project}/{status}/...）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeSetting {
    /// 本类型是否 TTS 播报（与全局 tts_enabled 取与）。
    #[serde(default = "default_true")]
    pub tts: bool,
    /// 本类型是否弹窗。
    #[serde(default = "default_true")]
    pub popup: bool,
    /// 呈现形态。
    #[serde(default)]
    pub form: NotifForm,
    /// 模板（body 文本，含变量占位）。
    #[serde(default)]
    pub template: String,
}

impl Default for TypeSetting {
    fn default() -> Self {
        Self {
            tts: true,
            popup: true,
            form: NotifForm::Full,
            template: String::new(),
        }
    }
}

/// 单事件触发配置（per_event 值，N2 hook 事件通知 — 逐事件自含）。
///
/// key（在 per_event map 里）= Claude Code 官方 hook 事件名（如 `Stop`/`SubagentStop`），
/// 见 `CC_HOOK_EVENTS` 全量目录。`enabled` 决定该事件是否注入 hook + 触发通知；
/// `tts`/`popup` 为该事件独立通道开关（与全局 tts_enabled 取与决定 TTS）；
/// `template` 为可选 per-event 自定义文案（空则回退 `default_template_for_event(event)`，
/// 再回退类型 default_template 防空）。全字段 serde default → 向后兼容：
/// 旧 DB per_event 含 `notif_type`（serde 无 deny_unknown → 反序列化忽略多余字段）；
/// 旧缺 `tts`/`popup` → serde default true（用户启用事件时两通道默认都开）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSetting {
    /// 是否启用该事件（注入 hook + 触发通知）。
    #[serde(default)]
    pub enabled: bool,
    /// 该事件是否 TTS 播报（与全局 tts_enabled 取与）。
    #[serde(default = "default_true")]
    pub tts: bool,
    /// 该事件是否弹窗。
    #[serde(default = "default_true")]
    pub popup: bool,
    /// 该事件是否播提示音（独立通道 `play_beep`，不再跟随弹窗）。旧配置无 sound → 默认 true（向后兼容）。
    #[serde(default = "default_true")]
    pub sound: bool,
    /// 可选 per-event 自定义文案（空则回退 `default_template_for_event` / 类型 default_template）。
    #[serde(default)]
    pub template: String,
}

impl Default for EventSetting {
    fn default() -> Self {
        Self {
            enabled: false,
            tts: true,
            popup: true,
            sound: true,
            template: String::new(),
        }
    }
}

/// Claude Code 官方 hook 事件全量目录（约 30 个；来源 code.claude.com/docs/zh-CN/hooks）。
/// UI 列全量，默认仅 `DEFAULT_ON_EVENTS` 精选集 on，其余默认 off。
/// **跨层镜像**：前端 `src/components/settings/NotificationEventList.tsx` 的 `CC_HOOK_EVENTS`
/// 逐字镜像此表，改此处务必同步前端。事件名为 CC 官方英文原样，不翻译。
pub const CC_HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "Setup",
    "InstructionsLoaded",
    "UserPromptSubmit",
    "UserPromptExpansion",
    "MessageDisplay",
    "PreToolUse",
    "PermissionRequest",
    "PermissionDenied",
    "PostToolUse",
    "PostToolUseFailure",
    "PostToolBatch",
    "Notification",
    "SubagentStart",
    "SubagentStop",
    "Stop",
    "StopFailure",
    "TeammateIdle",
    "TaskCreated",
    "TaskCompleted",
    "ConfigChange",
    "CwdChanged",
    "FileChanged",
    "WorktreeCreate",
    "WorktreeRemove",
    "PreCompact",
    "PostCompact",
    "Elicitation",
    "ElicitationResult",
    "SessionEnd",
];

/// 默认 ON 精选集（2 个：任务完成 + 等待授权）。
/// 其余事件（含 SubagentStop/Notification/SessionEnd/PreCompact/SessionStart 等）默认 off，
/// 目录中可手动开。**跨层镜像**前端 `DEFAULT_ON_EVENTS`。
pub const DEFAULT_ON_EVENTS: &[&str] = &["Stop", "PermissionRequest"];

/// 事件名 → 该事件**专属独立默认模板**（zh 硬编码，非 i18n）。
///
/// 每事件一套模板、用各自专属入参（禁所有事件共用一个统一模板）。通用入参所有事件都有：
/// `{project}`(项目名)/`{session}`(会话id)。专属入参来源 code.claude.com/docs/zh-CN/hooks
/// 各事件 stdin 字段。为避免可选字段缺失残留裸 `{x}`，默认模板**只用高确定字段**
/// （脚本通用透传所有标量字段，确有则填，缺失字段通过 substitute_vars 的 fill_empty 选项
/// 在 event 路径替换为空串 → 见 notification.rs render_event；故默认模板可放心用专属入参）。
/// 未命中事件 → 空串（dispatch 兜底到类型 default_template）。
///
/// **跨层镜像**：前端 `src/components/settings/NotificationEventList.tsx` 的 `EVENT_CATALOG`
/// 逐字镜像本表的 defaultTemplate + 专属入参，改此处务必同步前端。
pub fn default_template_for_event(event: &str) -> &'static str {
    match event {
        "SessionStart" => "{project} 会话开始",
        "Setup" => "{project} 初始化（{trigger}）",
        "InstructionsLoaded" => "{project} 已加载 {memory_type}",
        "UserPromptSubmit" => "{project} 收到新指令",
        "UserPromptExpansion" => "{project} 展开命令 {command_name}",
        "MessageDisplay" => "{project} 消息更新",
        "PreToolUse" => "{project} 即将执行 {tool_name}",
        "PermissionRequest" => "{project} 请求授权：{tool_name}",
        "PermissionDenied" => "{project} 拒绝 {tool_name}：{reason}",
        "PostToolUse" => "{project} {tool_name} 完成（{duration_ms}ms）",
        "PostToolUseFailure" => "{project} {tool_name} 失败：{error}",
        "PostToolBatch" => "{project} 批量工具完成",
        "Notification" => "{project}：{message}",
        "SubagentStart" => "{project} 子代理 {agent_type} 启动",
        "SubagentStop" => "{project} 子代理 {agent_type} 完成",
        "Stop" => "{project} 任务完成",
        "StopFailure" => "{project} 中断：{error_message}",
        "TeammateIdle" => "{project} 队友 {teammate_id} 空闲",
        "TaskCreated" => "{project} 新建任务：{task_name}",
        "TaskCompleted" => "{project} 任务完成：{task_name}",
        "ConfigChange" => "{project} 配置变更（{config_source}）",
        "CwdChanged" => "{project} 切换目录：{new_cwd}",
        "FileChanged" => "{project} 文件变更：{file_path}",
        "WorktreeCreate" => "{project} 创建 worktree",
        "WorktreeRemove" => "{project} 移除 worktree",
        "PreCompact" => "{project} 即将压缩上下文（{compact_reason}）",
        "PostCompact" => "{project} 压缩完成",
        "Elicitation" => "{project} {server_name} 请求输入",
        "ElicitationResult" => "{project} {server_name} 已响应",
        "SessionEnd" => "{project} 会话结束（{end_reason}）",
        _ => "",
    }
}

/// 通知设置（settings KV scope=`notification`, key=`settings`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// 总开关（OFF 时全部分发旁路）。default true。
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// TTS 总开关。default true。
    #[serde(default = "default_true")]
    pub tts_enabled: bool,
    /// TTS 后端。default CrossPlatform。
    #[serde(default)]
    pub tts_backend: TtsBackend,
    /// 按类型配置（key = NotifType 字面量）。缺省键视为全 true + Full。
    #[serde(default)]
    pub per_type: std::collections::HashMap<String, TypeSetting>,
    /// 按事件配置（key = CC 事件名，见 CC_HOOK_EVENTS）。N2 hook 事件通知。
    /// 旧配置无此字段 → 空 map（serde default），前端按默认目录展示，用户开启才写入。
    #[serde(default)]
    pub per_event: std::collections::HashMap<String, EventSetting>,
    /// 收件箱历史自动清理保留天数。default 7。`0` = 不清理（永久保留）。
    /// 清理为硬删（参 proxy_log retention 模式），避 SQLite 体积单调增长。
    /// 旧配置无此字段 → serde default 回退 7。
    #[serde(default = "default_inbox_retention_days")]
    pub inbox_retention_days: u32,
}

/// 收件箱默认保留天数（7）。serde 缺省回退用。
fn default_inbox_retention_days() -> u32 {
    7
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            tts_enabled: true,
            tts_backend: TtsBackend::default(),
            per_type: std::collections::HashMap::new(),
            per_event: std::collections::HashMap::new(),
            inbox_retention_days: default_inbox_retention_days(),
        }
    }
}

impl NotificationSettings {
    /// 取某类型有效配置：per_type 缺省时返回默认（全 true + Full）。
    pub fn type_setting(&self, t: NotifType) -> TypeSetting {
        self.per_type.get(t.as_str()).cloned().unwrap_or_default()
    }

    /// 取某事件有效配置：per_event 命中且返回引用，否则 None。
    /// 注：未命中走「展示层默认」（前端兜底默认目录），DB 不硬写默认集。
    pub fn event_setting(&self, event: &str) -> Option<&EventSetting> {
        self.per_event.get(event)
    }
}

/// 收件箱通知项（notification 表行）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: i64,
    pub notif_type: String,
    pub title: String,
    pub body: String,
    pub created_at: i64,
}

#[cfg(test)]
mod notif_event_model_tests {
    use super::*;

    #[test]
    fn default_template_per_event_independent() {
        // 每事件模板各自独立、用其专属入参（抽样核对）。
        assert_eq!(default_template_for_event("Stop"), "{project} 任务完成");
        assert_eq!(default_template_for_event("SubagentStop"), "{project} 子代理 {agent_type} 完成");
        assert_eq!(default_template_for_event("Notification"), "{project}：{message}");
        assert_eq!(default_template_for_event("PostToolUseFailure"), "{project} {tool_name} 失败：{error}");
        assert_eq!(default_template_for_event("SessionEnd"), "{project} 会话结束（{end_reason}）");
        // 全量目录每事件都有非空专属默认模板。
        for e in CC_HOOK_EVENTS {
            assert!(!default_template_for_event(e).is_empty(), "event {e} missing default template");
        }
        // 各事件模板互不相同（无统一模板）。
        let mut seen = std::collections::HashSet::new();
        for e in CC_HOOK_EVENTS {
            let t = default_template_for_event(e);
            assert!(seen.insert(t), "duplicate default template across events: {t}");
        }
        // 未命中事件 → 空串（dispatch 兜底类型 default_template）。
        assert_eq!(default_template_for_event("UnknownEvent"), "");
    }

    #[test]
    fn default_on_set_subset_of_catalog() {
        for e in DEFAULT_ON_EVENTS {
            assert!(CC_HOOK_EVENTS.contains(e), "default-on event {e} not in catalog");
        }
        // 默认 ON 仅 Stop + PermissionRequest（用户指示精简）。
        assert_eq!(DEFAULT_ON_EVENTS, &["Stop", "PermissionRequest"]);
        // 这些事件在目录但默认 off（可手动开）。
        for e in [
            "SessionStart",
            "SubagentStop",
            "Notification",
            "SessionEnd",
            "PreCompact",
        ] {
            assert!(CC_HOOK_EVENTS.contains(&e), "event {e} should be in catalog");
            assert!(!DEFAULT_ON_EVENTS.contains(&e), "event {e} should default off");
        }
    }

    #[test]
    fn settings_backward_compat_without_per_event() {
        // 旧 JSON 无 per_event → 反序列化为空 map，不报错。
        let json = serde_json::json!({
            "enabled": true,
            "tts_enabled": true,
            "tts_backend": "cross_platform",
            "per_type": {}
        });
        let s: NotificationSettings = serde_json::from_value(json).unwrap();
        assert!(s.per_event.is_empty());
        assert!(s.event_setting("Stop").is_none());
    }

    #[test]
    fn event_setting_roundtrip() {
        let json = serde_json::json!({
            "per_event": {
                "Stop": { "enabled": true, "tts": false, "popup": true, "template": "{project} done" },
                "PostToolUse": { "enabled": false }
            }
        });
        let s: NotificationSettings = serde_json::from_value(json).unwrap();
        let stop = s.event_setting("Stop").unwrap();
        assert!(stop.enabled);
        assert!(!stop.tts);
        assert!(stop.popup);
        assert_eq!(stop.template, "{project} done");
        // tts/popup/template 缺省 → tts/popup default true、template 空串。
        let pt = s.event_setting("PostToolUse").unwrap();
        assert!(!pt.enabled);
        assert!(pt.tts);
        assert!(pt.popup);
        assert_eq!(pt.template, "");
    }

    #[test]
    fn event_setting_backward_compat_ignores_legacy_notif_type() {
        // 旧 DB per_event 含 notif_type（已删字段）→ serde 无 deny_unknown 忽略多余字段，
        // 旧缺 tts/popup → serde default true。不报错。
        let json = serde_json::json!({
            "per_event": {
                "SubagentStop": { "enabled": true, "notif_type": "error", "template": "x" }
            }
        });
        let s: NotificationSettings = serde_json::from_value(json).unwrap();
        let es = s.event_setting("SubagentStop").unwrap();
        assert!(es.enabled);
        assert!(es.tts); // 旧无 → default true
        assert!(es.popup); // 旧无 → default true
        assert_eq!(es.template, "x");
    }
}

#[cfg(test)]
mod middleware_model_tests {
    use super::*;

    #[test]
    fn rule_type_serde_snake_case_roundtrip() {
        for (variant, lit) in [
            (RuleType::RequestFilter, "\"request_filter\""),
            (RuleType::SensitiveWord, "\"sensitive_word\""),
            (RuleType::Redaction, "\"redaction\""),
            (RuleType::ContentFilter, "\"content_filter\""),
            (RuleType::DynamicInjection, "\"dynamic_injection\""),
            (RuleType::ResponseOverride, "\"response_override\""),
            (RuleType::Rectifier, "\"rectifier\""),
            (RuleType::ErrorRule, "\"error_rule\""),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, lit, "serialize {variant:?}");
            let back: RuleType = serde_json::from_str(lit).unwrap();
            assert_eq!(back, variant, "deserialize {lit}");
            // as_str / from_db_str 与 serde 字面量一致
            assert_eq!(format!("\"{}\"", variant.as_str()), lit);
            assert_eq!(RuleType::from_db_str(variant.as_str()), Some(variant));
        }
        assert_eq!(RuleType::from_db_str("nope"), None);
    }

    #[test]
    fn scope_match_action_serde_snake_case() {
        assert_eq!(serde_json::to_string(&RuleScope::Global).unwrap(), "\"global\"");
        assert_eq!(serde_json::to_string(&RuleScope::Group).unwrap(), "\"group\"");
        assert_eq!(serde_json::to_string(&RuleScope::Platform).unwrap(), "\"platform\"");
        assert_eq!(serde_json::to_string(&MatchType::Regex).unwrap(), "\"regex\"");
        assert_eq!(serde_json::to_string(&MatchType::Contains).unwrap(), "\"contains\"");
        assert_eq!(serde_json::to_string(&MatchType::Exact).unwrap(), "\"exact\"");
        assert_eq!(serde_json::to_string(&RuleAction::Mask).unwrap(), "\"mask\"");
        assert_eq!(serde_json::to_string(&RuleAction::Classify).unwrap(), "\"classify\"");
        // from_db_str 兜底
        assert_eq!(RuleScope::from_db_str("xxx"), RuleScope::Global);
        assert_eq!(MatchType::from_db_str("xxx"), MatchType::Contains);
        assert_eq!(RuleAction::from_db_str("xxx"), RuleAction::Warn);
    }

    #[test]
    fn middleware_rule_serde_roundtrip() {
        let rule = MiddlewareRule {
            id: 7,
            name: "mask-keys".into(),
            description: "redact api keys".into(),
            rule_type: RuleType::Redaction,
            scope: RuleScope::Group,
            scope_ref: "team-a".into(),
            match_type: MatchType::Regex,
            pattern: r"sk-\w+".into(),
            action: RuleAction::Mask,
            config: "{\"replacement\":\"****\"}".into(),
            priority: 3,
            enabled: true,
            is_builtin: false,
            created_at: 100,
            updated_at: 200,
        };
        let json = serde_json::to_string(&rule).unwrap();
        let back: MiddlewareRule = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, rule.id);
        assert_eq!(back.rule_type, rule.rule_type);
        assert_eq!(back.scope, rule.scope);
        assert_eq!(back.match_type, rule.match_type);
        assert_eq!(back.action, rule.action);
        assert_eq!(back.pattern, rule.pattern);
    }

    #[test]
    fn middleware_settings_default_and_type_enabled() {
        let s = MiddlewareSettings::default();
        assert!(s.enabled);
        assert!(s.type_toggles.is_empty());
        // 缺省键 → true
        assert!(s.type_enabled(RuleType::Redaction));

        // 显式关某类型
        let mut s2 = MiddlewareSettings::default();
        s2.type_toggles.insert("redaction".into(), false);
        assert!(!s2.type_enabled(RuleType::Redaction));
        assert!(s2.type_enabled(RuleType::SensitiveWord));

        // 总开关关 → 全 false
        let s3 = MiddlewareSettings { enabled: false, ..Default::default() };
        assert!(!s3.type_enabled(RuleType::SensitiveWord));
    }

    #[test]
    fn middleware_settings_serde_partial_fills_default() {
        // 旧/部分 JSON（无 type_toggles）→ default 填充
        let s: MiddlewareSettings = serde_json::from_str("{\"enabled\":true}").unwrap();
        assert!(s.enabled);
        assert!(s.type_toggles.is_empty());
        // 空对象 → enabled default true
        let s2: MiddlewareSettings = serde_json::from_str("{}").unwrap();
        assert!(s2.enabled);
    }

    #[test]
    fn notif_type_serde_snake_case_roundtrip() {
        for (variant, lit) in [
            (NotifType::TaskComplete, "\"task_complete\""),
            (NotifType::WaitingInput, "\"waiting_input\""),
            (NotifType::Error, "\"error\""),
        ] {
            assert_eq!(serde_json::to_string(&variant).unwrap(), lit);
            let back: NotifType = serde_json::from_str(lit).unwrap();
            assert_eq!(back, variant);
            assert_eq!(format!("\"{}\"", variant.as_str()), lit);
        }
        assert_eq!(NotifType::from_str_or_default("waiting_input"), NotifType::WaitingInput);
        assert_eq!(NotifType::from_str_or_default("unknown_xyz"), NotifType::TaskComplete);
    }

    #[test]
    fn notif_form_and_backend_serde() {
        assert_eq!(serde_json::to_string(&NotifForm::PopupOnly).unwrap(), "\"popup_only\"");
        assert_eq!(serde_json::to_string(&NotifForm::InboxOnly).unwrap(), "\"inbox_only\"");
        assert_eq!(serde_json::to_string(&NotifForm::SoundOnly).unwrap(), "\"sound_only\"");
        assert_eq!(serde_json::to_string(&NotifForm::Full).unwrap(), "\"full\"");
        assert_eq!(NotifForm::default(), NotifForm::Full);
        assert_eq!(serde_json::to_string(&TtsBackend::CrossPlatform).unwrap(), "\"cross_platform\"");
        assert_eq!(serde_json::to_string(&TtsBackend::MacSay).unwrap(), "\"mac_say\"");
        assert_eq!(serde_json::to_string(&TtsBackend::WebSpeech).unwrap(), "\"web_speech\"");
        assert_eq!(TtsBackend::default(), TtsBackend::CrossPlatform);
    }

    #[test]
    fn notification_settings_default_and_partial() {
        let s = NotificationSettings::default();
        assert!(s.enabled);
        assert!(s.tts_enabled);
        assert_eq!(s.tts_backend, TtsBackend::CrossPlatform);
        assert_eq!(s.inbox_retention_days, 7);
        assert!(s.per_type.is_empty());
        // 缺省类型 → 全 true + Full
        let ts = s.type_setting(NotifType::Error);
        assert!(ts.tts && ts.popup);
        assert_eq!(ts.form, NotifForm::Full);

        // 部分 JSON 填默认
        let p: NotificationSettings = serde_json::from_str("{\"enabled\":false}").unwrap();
        assert!(!p.enabled);
        assert!(p.tts_enabled);
        assert_eq!(p.tts_backend, TtsBackend::CrossPlatform);
        // 旧配置无 inbox_retention_days → serde default 回退 7
        assert_eq!(p.inbox_retention_days, 7);

        // per_type 显式覆盖往返
        let mut s2 = NotificationSettings::default();
        s2.per_type.insert(
            NotifType::TaskComplete.as_str().into(),
            TypeSetting { tts: false, popup: true, form: NotifForm::InboxOnly, template: "{project} done".into() },
        );
        let json = serde_json::to_string(&s2).unwrap();
        let back: NotificationSettings = serde_json::from_str(&json).unwrap();
        let got = back.type_setting(NotifType::TaskComplete);
        assert!(!got.tts);
        assert_eq!(got.form, NotifForm::InboxOnly);
        assert_eq!(got.template, "{project} done");
    }

    /// 最小 Platform，仅设 extra 用于 breaker 解析测试。
    fn platform_with_extra(extra: &str) -> Platform {
        Platform {
            id: 1,
            name: "p".into(),
            platform_type: Protocol::Anthropic,
            base_url: String::new(),
            api_key: String::new(),
            extra: extra.into(),
            models: PlatformModels::default(),
            available_models: vec![],
            endpoints: vec![],
            enabled: true,
            status: PlatformStatus::Enabled,
            auto_disabled_until: 0,
            auto_disable_strikes: 0,
            created_at: 0,
            updated_at: 0,
            deleted_at: 0,
            est_balance_remaining: 0.0,
            est_coding_plan: String::new(),
            last_real_query_at: 0,
            estimate_count: 0,
            show_in_tray: false,
            tray_display: String::new(),
            sort_order: 0,
            manual_budgets: vec![],
            balance_level: String::new(),
        }
    }

    #[test]
    fn parse_merge_breaker_roundtrip() {
        // 空 / 非法 / 无 breaker 键 → 全 0。
        assert_eq!(parse_breaker("").failure_threshold, 0);
        assert_eq!(parse_breaker("not json").open_secs, 0);
        assert_eq!(parse_breaker(r#"{"mock":{}}"#).half_open_max, 0);

        // merge 写入 → 再解析一致，且保留 extra 其余键。
        let merged = merge_breaker_into_extra(
            r#"{"mock":{"x":1}}"#,
            &PlatformBreaker { failure_threshold: 4, open_secs: 90, half_open_max: 2 },
        );
        let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(v["mock"]["x"], 1, "保留 extra 其余键");
        let b = parse_breaker(&merged);
        assert_eq!((b.failure_threshold, b.open_secs, b.half_open_max), (4, 90, 2));

        // 全 0 → 移除 breaker 键（无覆盖=继承全局）。
        let cleared = merge_breaker_into_extra(&merged, &PlatformBreaker::default());
        let v2: serde_json::Value = serde_json::from_str(&cleared).unwrap();
        assert!(v2.get("breaker").is_none(), "全 0 移除 breaker 键");
        assert_eq!(v2["mock"]["x"], 1, "清 breaker 不动其余键");
    }

    #[test]
    fn effective_thresholds_extra_override_and_inherit() {
        let global = SchedulingBreakerSettings::default(); // (5, 60, 2)

        // 缺 extra.breaker → 全继承全局默认。
        let p_none = platform_with_extra("{}");
        assert_eq!(global.effective_thresholds(&p_none), (5, 60, 2));

        // extra.breaker 全覆盖。
        let p_all = platform_with_extra(&merge_breaker_into_extra(
            "{}",
            &PlatformBreaker { failure_threshold: 9, open_secs: 120, half_open_max: 4 },
        ));
        assert_eq!(global.effective_thresholds(&p_all), (9, 120, 4));

        // 单键覆盖（failure_threshold），其余继承全局；open_secs/half_open_max=0 → 用全局。
        let p_partial = platform_with_extra(&merge_breaker_into_extra(
            "{}",
            &PlatformBreaker { failure_threshold: 8, open_secs: 0, half_open_max: 0 },
        ));
        assert_eq!(global.effective_thresholds(&p_partial), (8, 60, 2));
    }
}

#[cfg(test)]
mod model_test_challenge_tests {
    use super::*;

    #[test]
    fn random_challenge_prompt_varies_and_expected_nonempty() {
        // 多次生成应出现不止一个 prompt（防指纹）；每条 expected 非空且可在自身校验通过。
        let mut prompts = std::collections::HashSet::new();
        for _ in 0..200 {
            let (p, e) = random_test_challenge();
            assert!(!p.trim().is_empty(), "prompt 不应为空");
            assert!(!e.trim().is_empty(), "expected 不应为空");
            // expected 直接喂回校验必然通过（归一化自反）。
            assert!(verify_test_response(&e, Some(&e)), "expected 自校验应通过: {e}");
            prompts.insert(p);
        }
        assert!(prompts.len() > 1, "200 次生成应产生多种 prompt，实际 {}", prompts.len());
    }

    #[test]
    fn arithmetic_answers_are_correct() {
        // 算术题答案须为真实计算结果：采样直到覆盖一道加法验证语义。
        for _ in 0..500 {
            let (p, e) = random_test_challenge();
            if let Some(idx) = p.find(" 加 ") {
                let a: i64 = p[..idx].trim().parse().unwrap();
                let rest = &p[idx + " 加 ".len()..];
                let b: i64 = rest[..rest.find(' ').unwrap()].trim().parse().unwrap();
                assert_eq!(e, (a + b).to_string());
                return;
            }
        }
        panic!("500 次未抽到加法题");
    }

    #[test]
    fn verify_substring_match_tolerates_natural_answers() {
        // 含子串即通过：模型自然长答 + 标点 + 大小写均应匹配。
        assert!(verify_test_response("答案是 95。", Some("95")));
        assert!(verify_test_response("中国的首都是北京，是一座历史名城。", Some("北京")));
        assert!(verify_test_response("The formula is H2O.", Some("H2O")));
        assert!(verify_test_response("h2o", Some("H2O"))); // 大小写归一
        // 不含 expected → 失败。
        assert!(!verify_test_response("上海", Some("北京")));
        assert!(!verify_test_response("", Some("12")));
    }

    #[test]
    fn verify_custom_mode_skips_content_check() {
        // expected=None（自定义 prompt）：非空即通过，空白即失败，不做关键词比对。
        assert!(verify_test_response("任意非空回答", None));
        assert!(verify_test_response("anything goes here", None));
        assert!(!verify_test_response("   ", None));
        assert!(!verify_test_response("", None));
    }
}
