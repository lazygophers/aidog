//! 代理日志模型：完整日志行 / 平台用量统计 / 测试结果 / 列表摘要 / 筛选 / 日志设置。

use super::{ProxyAttempt, RoutingMode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// 上游返回的响应体（非流式完整 JSON；流式在终态 flush 后为聚合 SSE，中间态为空串，票 06 哨兵已废）
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
    /// 代理返回给用户的响应体（非流式含模型名替换；流式在终态 flush 后为聚合内容，中间态为空串）
    #[serde(default)]
    pub user_response_body: String,
    pub status_code: i32,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    /// 缓存写入 token（Anthropic `cache_creation_input_tokens`；计价 1.25×/2× input 价）。
    /// 此前未采集（2026-09-28 列），存量行 0。
    #[serde(default)]
    pub cache_write_tokens: i32,
    /// 缓存写入 1h TTL token（Anthropic `cache_creation.ephemeral_1h_input_tokens`；
    /// 计价走 `price.cache_write_1h` 档，spec B3）。2026-10-28 列，存量行 0。
    #[serde(default)]
    pub cache_write_1h_tokens: i32,
    /// 预估花费（$），基于 model_entry 定价
    #[serde(default)]
    pub est_cost: f64,
    /// 按次工具调用计数（spec-按次计费 B2）：(工具键, 成功次数)，键从 anthropic
    /// `usage.server_tool_use` 子段派生（`web_search_requests` → `web_search`）。
    /// 内存透传到 process_upsert 计价并加进 est_cost（B1 决策：不进
    /// `PROXY_LOG_COLUMNS` / 不落列，落库列集零变化）。
    #[serde(default)]
    pub tool_call_counts: Vec<(String, i64)>,
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
    /// 终态标记（stream-full-log 票 06）：请求日志生命周期完结（非流式终态 / 流式 flush /
    /// 断连兜底 / 中断补写）时置位。取代旧 `response_body == "[stream]"` 哨兵的终态判定
    /// （背压分支、聚合去重 gate、快照移除条件）。中间态（status=0 或流式聚合中）恒 false。
    #[serde(default)]
    pub done: bool,
    /// 出站 body 构造过程中被丢弃 / 被改写的**字段名**留痕（票 10），逗号分隔的 token 串，空表示无改动。
    /// token 形态：`drop:<字段名>`（客户端发了但目标协议允许集合不收，见 `apply_field_passthrough`）、
    /// `cap:max_tokens`（透传路径出站裁剪）、`rewrite:middleware`（透传路径 middleware 改写 body）。
    /// **只记名不记值**：字段值可能含敏感内容，任何情况下不进本列。
    /// 归上游侧「原始信息」：受 `log_upstream_request` 开关约束，按 `upstream_request_retention_days` 清理。
    #[serde(default)]
    pub field_trace: String,
    /// 「正文已省略」标记（perf-backend O6，2026-10-01 列）：该行的部分正文因日志写入队列
    /// 字节预算超限被降级丢弃（只落了元数据）。置位后本行生命周期内单调不回落（sticky）；
    /// 语义是「发生过降级」——极小概率后续节点预算恢复又把正文补写进去，标记保持 true
    /// （宁可多报不可漏报：反向「标 false 但正文缺」会误导排查）。正文列本身仍空串
    /// （不写占位文字，CLAUDE.md「Proxy 日志」段）。存量行 0 = 历史无降级。
    #[serde(default)]
    pub body_omitted: bool,
    /// 路由决策快照（代理日志-路由决策直观化 票 01）：本次请求的路由评估全程。
    /// 元数据：恒落库，不受 log_user_request / log_upstream_request 开关控制，
    /// 不进 strip_raw_sides / ProxyLogColumns::from_log 的两侧「原始信息」列集。
    #[serde(default)]
    pub routing: Option<RoutingSnapshot>,
    /// 客户端代码（请求进入时由 UA 归类，词表变体码或大类码（`aidog_core::client_vocab`），认不出 `other`，旧行可为旧 7 码；
    /// 空串 = 未知：迁移前存量行 / 带外日志）。元数据：不受 log_user_request 开关控制，不存原始 UA。
    #[serde(default)]
    pub client: String,
    /// 入站原始 User-Agent（`handler.rs` 入口直读请求头，截断 `USER_AGENT_MAX_CHARS`）。
    /// 空串 = 无 UA 头 / 存量行 / 带外日志。元数据：不受 log_user_request 开关控制，仅 proxy_log
    /// 单行存储，不进 stats_agg_hourly（聚合维度仍只用 `client` 代码，防基数失控）。
    #[serde(default)]
    pub user_agent: String,
    /// 会话归组键（magpie 票 08，migration 20261107-01）：入站请求派生（`gateway::proxy::derive_session_key`）
    /// ——anthropic `metadata.user_id` 的 session UUID / openai 系 `session_id` / header `x-session-id`；
    /// 取不到 = 空串（不参与聚合）。元数据：不受 log_user_request / log_upstream_request 开关控制。
    /// 存量行 '' = 未归组（D4a 不回填）。
    #[serde(default)]
    pub session_key: String,
    /// Claude Code 网关提示头 `x-claude-code-request-class`（main / subagent / workflow / compaction /
    /// auxiliary / side_reply …，未知值原样存）；空串 = 无该头 / 存量行。元数据：不受日志开关控制。
    #[serde(default)]
    pub request_class: String,
    /// Claude Code 网关提示头 `x-claude-code-agent-type`（子代理自己的回合才有）；空串 = 无。
    #[serde(default)]
    pub agent_type: String,
    /// Claude Code 网关提示头 `x-claude-code-prompt-id`（同一用户提示词派生的请求共享的 UUID）；空串 = 无。
    #[serde(default)]
    pub prompt_id: String,
    /// `x-claude-code-session-id`（Claude Code 会话 ID，区别于派生的 `session_key`）；空串 = 无。
    #[serde(default)]
    pub session_id: String,
    /// `x-claude-code-agent-id`（子代理 ID）；空串 = 无。
    #[serde(default)]
    pub agent_id: String,
    /// `x-claude-code-parent-agent-id`（嵌套 Agent 的父 Agent ID）；空串 = 无。
    #[serde(default)]
    pub parent_agent_id: String,
    /// `x-claude-code-compaction`（auto / manual / reactive，未知值原样存）；空串 = 无。
    #[serde(default)]
    pub compaction: String,
    /// `x-claude-code-context-compacted`（压缩后首个主对话请求，取值同 compaction）；空串 = 无。
    #[serde(default)]
    pub context_compacted: String,
    /// `x-claude-code-prev-tool-durations`：百分号解码后的 `Name=ms;Name=ms`（≤4096 字节）；空串 = 无。
    #[serde(default)]
    pub prev_tool_durations: String,
}

/// `ProxyLog.user_agent` 字符上限（防畸形超长 UA 撑大行）。
pub const USER_AGENT_MAX_CHARS: usize = 256;

/// 会话统计聚合行（magpie 票 08）：`get_session_stats` 按 `proxy_log.session_key`
/// 聚合近 N 天的请求级数据。空 session_key 不产生行。
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct SessionStats {
    /// 会话归组键（派生规则见 `ProxyLog.session_key`）
    pub session_key: String,
    /// 标题 = 该会话首条有 request_body 的请求前 60 字符；log_user_request 关着取不到 → 空串
    /// （UI 显示「未命名会话」）
    pub title: String,
    /// 窗口内请求数
    #[ts(type = "number")]
    pub requests: i64,
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
    #[ts(type = "number")]
    pub cache_tokens: i64,
    /// 窗口内预估花费合计（$）
    pub est_cost: f64,
    /// 窗口内首条请求 created_at（毫秒 epoch）
    #[ts(type = "number")]
    pub first_created_at: i64,
    /// 窗口内末条请求 created_at（毫秒 epoch）
    #[ts(type = "number")]
    pub last_created_at: i64,
}

/// 会话标题截断长度（magpie 票 08 / spec D4）
pub const SESSION_TITLE_MAX_CHARS: usize = 60;

impl ProxyLog {
    /// 带外请求日志（不经代理转发路径的出站记录：Quota Script 查询 / 拉模型列表 / 模型测试）
    /// 的公共约定集中处，替代三份手写约定：marker group_key + marker source_protocol、
    /// done 终态、user_response_headers 恒 application/json；token / est_cost 零值由 Default 承担。
    /// 用法：`ProxyLog { 各自有逻辑的字段, ..Default::default() }.out_of_band(marker, marker_proto)`
    pub fn out_of_band(mut self, group_key: &str, source_protocol: &str) -> Self {
        self.group_key = group_key.into();
        self.source_protocol = source_protocol.into();
        self.user_response_headers = r#"{"content-type":"application/json"}"#.into();
        self.done = true;
        self
    }
}

// ── 路由决策快照（代理日志-路由决策直观化 票 01）──
//
// proxy_log.routing JSON 列的形状，与 attempts 列同模式。TS 侧由 ts-rs 生成到
// src/services/api/types/generated/（同 ProxyAttempt 模式）；RoutingMode 本身是
// manual.ts 锁定 enum（无 TS derive），故 mode 字段用 ts(type) 覆写为宽类型。

/// 候选条目的原因标签：`selected` = 进入最终尝试序列；其余为排除 / 跳过原因。
/// serde rename（kebab-case）即 wire 值，与 TS 侧字面量逐字一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub enum RouteReason {
    /// 选中（进入最终尝试序列）
    Selected,
    /// 手动禁用（status=Disabled）
    DisabledManual,
    /// 自动禁用未到期（AutoDisabled 未到退避试探时间）/ 已过期（expires_at，等效自动禁用）
    DisabledAuto,
    /// 高峰期禁用（disable_during_peak 开 + 命中 peak 窗口）
    PeakDisabled,
    /// 熔断（准入门 Reject；整组熔断回退时回填为 selected）
    Breaker,
    /// 配额冷却（429 重置前不调度，内存态）
    QuotaCooldown,
    /// 认证冷却（401 权限 / 402 余额，内存态）
    AuthCooldown,
    /// 协议未知（DB platform_type 枚举外，票 06）
    UnknownProtocol,
    /// 请求类型不支持（Chat/Decision 维度剔除，jev-decision-proxy R2/R3）
    KindUnsupported,
    /// 客户端白名单未命中（platform.extra.allowed_clients）
    ClientNotAllowed,
    /// 平台限频超限（platform.extra.rate_limits，内存计数）
    RateLimited,
    /// 命中的模型槽位被平台默认开关关闭（platform.extra.disabled_slots）：平台被排除，不计平台错误
    ModelNotOnPlatform,
    /// 命中的模型槽位被「时段档」自己的开关关闭（time_windows[].disabled_slots）：与 `ModelNotOnPlatform`（平台默认开关）区分
    TimeRuleSlotDisabled,
    /// 死站跳过（重试循环内同 base_url 已 connect 失败，handler.rs R3 补记）
    SkippedDeadUpstream,
    /// 直通模型链：平台未声明该直通模型，被丢弃（路由画布 01）
    NotDeclared,
    /// 入选但因最大尝试次数用尽从未尝试（路由画布 01；上限见 `RoutingSnapshot.max_retries`）
    RetryLimit,
}

/// 候选条目的位置标签（排序阶段的位置变化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub enum RouteTag {
    /// 分组 model_mappings 显式指定，提到最前
    MappingPromoted,
    /// coding plan 上浮
    CodingPlan,
    /// 失败降权沉底（health-aware penalty 窗口）
    Demoted,
    /// sticky 会话绑定命中，提到首位
    Sticky,
    /// auto_disabled 退避期满，作为探测放行（排在 active 之后，路由画布 01）
    Probe,
    /// 整组熔断踢空后，被熔断拒绝的候选回填为选中（路由画布 01）
    BreakerBackfill,
}

/// 单平台短路时被绕过的状态（路由画布 01）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub enum ShortcutBypass {
    /// auto_disabled 未到期
    AutoDisabled,
    /// expires_at 已过期
    Expired,
    /// 熔断 Open
    Breaker,
    /// 配额冷却（429）
    QuotaCooldown,
    /// 认证冷却（401/402）
    AuthCooldown,
}

/// 单平台短路标记（路由画布 01）：`literal` = 组内仅 1 个平台；否则为「恰好 1 个 Enabled」的等效单平台。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct SingleShortcut {
    pub literal: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<ShortcutBypass>>")]
    pub bypassed: Vec<ShortcutBypass>,
}

/// 目标模型的选择来源（可叠加：如 `mapped` + `slot-opus`）。
///
/// 线格式是单个字符串：固定词用 kebab-case（`slot-opus`），自定义槽位为 `slot-custom:<槽名>`。
/// 手写 serde 而非 derive：带名字的变体用 derive 会成 `{"slot-custom":"x"}` 对象，破坏既有字符串数组形态。
#[derive(Debug, Clone, PartialEq, Eq, TS)]
#[ts(
    export,
    export_to = "../../../../src/services/api/types/generated/",
    type = r#""mapped" | "time-window" | "preset-peak" | "slot-opus" | "slot-sonnet" | "slot-haiku" | "slot-gpt" | "slot-default" | "dedicated-slot" | "passthrough" | `slot-custom:${string}`"#
)]
pub enum ModelVia {
    /// 分组 model_mappings 先把请求模型名改成映射目标名
    Mapped,
    /// 平台 time_windows 命中，用该时段的 models
    TimeWindow,
    /// preset `models.peak` 高峰分支替换了平台 models
    PresetPeak,
    /// 命中平台 opus 槽位
    SlotOpus,
    /// 命中平台 sonnet 槽位
    SlotSonnet,
    /// 命中平台 haiku 槽位
    SlotHaiku,
    /// 命中平台 gpt 槽位
    SlotGpt,
    /// 命中自定义槽位（槽名）
    SlotCustom(String),
    /// 无档位词命中，落平台 default 槽位
    SlotDefault,
    /// 专用槽位（embedding / rerank / 决策 jev）
    DedicatedSlot,
    /// 无槽位可用 / 透传平台 / Tools：沿用（映射后的）请求模型名
    Passthrough,
}

const SLOT_CUSTOM_PREFIX: &str = "slot-custom:";

impl ModelVia {
    fn wire(&self) -> std::borrow::Cow<'static, str> {
        use std::borrow::Cow::{Borrowed, Owned};
        match self {
            Self::Mapped => Borrowed("mapped"),
            Self::TimeWindow => Borrowed("time-window"),
            Self::PresetPeak => Borrowed("preset-peak"),
            Self::SlotOpus => Borrowed("slot-opus"),
            Self::SlotSonnet => Borrowed("slot-sonnet"),
            Self::SlotHaiku => Borrowed("slot-haiku"),
            Self::SlotGpt => Borrowed("slot-gpt"),
            Self::SlotCustom(name) => Owned(format!("{SLOT_CUSTOM_PREFIX}{name}")),
            Self::SlotDefault => Borrowed("slot-default"),
            Self::DedicatedSlot => Borrowed("dedicated-slot"),
            Self::Passthrough => Borrowed("passthrough"),
        }
    }
}

impl Serialize for ModelVia {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.wire())
    }
}

impl<'de> Deserialize<'de> for ModelVia {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Ok(match raw.as_str() {
            "mapped" => Self::Mapped,
            "time-window" => Self::TimeWindow,
            "preset-peak" => Self::PresetPeak,
            "slot-opus" => Self::SlotOpus,
            "slot-sonnet" => Self::SlotSonnet,
            "slot-haiku" => Self::SlotHaiku,
            "slot-gpt" => Self::SlotGpt,
            "slot-default" => Self::SlotDefault,
            "dedicated-slot" => Self::DedicatedSlot,
            "passthrough" => Self::Passthrough,
            other => match other.strip_prefix(SLOT_CUSTOM_PREFIX) {
                Some(name) if !name.is_empty() => Self::SlotCustom(name.to_string()),
                _ => return Err(serde::de::Error::custom(format!("unknown ModelVia: {other}"))),
            },
        })
    }
}

/// 单个候选平台的路由评估结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct RouteCandidate {
    /// 排列序号（1 起；选中者按最终尝试顺序在前，排除者附后）
    #[ts(type = "number")]
    pub order: u32,
    pub platform: String,
    #[ts(type = "number")]
    pub platform_id: u64,
    /// per-group 平台优先级（1~10，10 最高）
    #[ts(type = "number")]
    pub level_priority: i32,
    /// 同级内优先级（小者先）
    #[ts(type = "number")]
    pub priority: i32,
    /// 原因标签
    pub reason: RouteReason,
    /// 路由目标模型（build_route_results 解析的 target_model；排除候选算不到时为 None）。
    /// 旧日志无此字段，缺省 None，序列化时省略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_model: Option<String>,
    /// 位置标签（仅选中者携带）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<RouteTag>>")]
    pub tags: Vec<RouteTag>,
    /// 目标模型选择来源（仅选中者携带；旧日志无此字段，缺省空）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<ModelVia>>")]
    pub model_via: Vec<ModelVia>,
    /// 命中槽位的完整有序模型链（链首 = target_model）；仅链长 > 1 时携带，旧日志缺省空。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub model_chain: Vec<String>,
}

/// 路由决策快照：单次请求的路由评估全程（proxy_log.routing 列）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct RoutingSnapshot {
    /// 本次路由模式（load_balance / failover / health_aware / sticky）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "string | null")]
    pub mode: Option<RoutingMode>,
    /// 候选平台
    #[serde(default)]
    pub entries: Vec<RouteCandidate>,
    /// 路由评估耗时（毫秒）：内核收到请求 → 候选平台评估完成。旧日志无此字段，缺省 None，序列化时省略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub eval_ms: Option<i64>,
    /// 单平台短路触发标记及被绕过的状态（路由画布 01）。旧日志缺省 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shortcut: Option<SingleShortcut>,
    /// 分组 max_retries（路由画布 01）。旧日志 / 未进重试循环缺省 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub max_retries: Option<u32>,
    /// 尝试轮（候选全排除后间隔 1 秒从头重来）的起点：每个元素 = 该轮开始时 `attempts` 已有的条数
    /// （第 N+1 轮的起点是第 N 个元素；空轮会产生相同的值）。缺省 None = 只有第 1 轮（含旧日志）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Array<number>")]
    // Box：RoutingSnapshot 随 RouteFail 走 Err 路径，多 24 字节会越过 clippy::result_large_err 的 128 字节线
    pub round_starts: Option<Box<Vec<u32>>>,
}

impl RoutingSnapshot {
    /// 最大尝试次数用尽补记（路由画布 01 / 请求尝试 01）：上限那次调用落在 `after_platform_id`，
    /// 其后的入选候选永不会被尝试 → 标 retry-limit。
    pub fn mark_retry_limit(&mut self, max_attempts: u32, after_platform_id: u64) {
        self.max_retries = Some(max_attempts);
        let Some(order) = self.entries.iter().find(|e| e.platform_id == after_platform_id).map(|e| e.order) else {
            return;
        };
        for e in &mut self.entries {
            if e.reason == RouteReason::Selected && e.order > order {
                e.reason = RouteReason::RetryLimit;
            }
        }
    }

    /// 逐次重算版（请求尝试-逐次重算）：重排后候选顺序随状态变化，「其后」不再有意义——
    /// 凡仍是 selected 且从未尝试过（`tried` 之外）的候选一律标 retry-limit。
    pub fn mark_retry_limit_untried(&mut self, max_attempts: u32, tried: &[u64]) {
        self.max_retries = Some(max_attempts);
        for e in &mut self.entries {
            if e.reason == RouteReason::Selected && !tried.contains(&e.platform_id) {
                e.reason = RouteReason::RetryLimit;
            }
        }
    }

    /// 死站跳过补记（handler.rs 重试循环 R3）：把该候选标为 skipped-dead-upstream。
    pub fn mark_dead_upstream(&mut self, platform_id: u64) {
        for e in &mut self.entries {
            if e.platform_id == platform_id && e.reason == RouteReason::Selected {
                e.reason = RouteReason::SkippedDeadUpstream;
            }
        }
    }
}

/// 序列化 routing 列（None / 序列化失败 → 空串）
pub fn serialize_routing(r: &Option<RoutingSnapshot>) -> String {
    r.as_ref()
        .and_then(|s| serde_json::to_string(s).ok())
        .unwrap_or_default()
}

/// 解析 routing 列（空串 / 坏 JSON → None）
pub fn parse_routing(s: &str) -> Option<RoutingSnapshot> {
    serde_json::from_str(s).ok()
}

/// 请求流向聚合行（请求流向票 03）：stats_agg_hourly 按 (client, group_key, req_model, platform_id, model) GROUP BY。
/// 旧数据 client / req_model 为空串，前端显示「未知」。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct FlowRow {
    pub client: String,
    pub group_key: String,
    pub req_model: String,
    /// Claude Code 请求类型（main / subagent ...）；旧数据为空串。
    pub request_class: String,
    #[ts(type = "number")]
    pub platform_id: i64,
    pub model: String,
    #[ts(type = "number")]
    pub requests: i64,
    /// input + output + cache
    #[ts(type = "number")]
    pub tokens: i64,
    pub est_cost: f64,
}

/// 分组页 7 天趋势行（详情页右栏趋势卡）：stats_agg_hourly 按天（本地日期）× 平台 GROUP BY。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct GroupDailyBreakdown {
    /// 本地日期（time_hour 前 10 位，YYYY-MM-DD）。
    pub day: String,
    #[ts(type = "number")]
    pub platform_id: i64,
    #[ts(type = "number")]
    pub requests: i64,
    pub est_cost: f64,
}

/// 最近一次路由快照（G3 票 03 双侧边「最近路由」常驻区）：proxy_log 最新一条带 routing 的行。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct LatestRouting {
    #[ts(type = "number")]
    pub platform_id: i64,
    pub source_model: String,
    /// 实际路由到的上游模型（proxy_log.actual_model）。
    pub actual_model: String,
    #[ts(type = "number")]
    pub created_at: i64,
    pub snapshot: RoutingSnapshot,
}

/// 分组「最后一次请求」摘要（分组列表卡片 / 详情页）：proxy_log 该 group_key 最新一条
/// （排除 test/quota/fetch-models，与 Logs 主页同口径）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct GroupLastRequest {
    /// proxy_log.id，供跳转日志详情。
    pub id: String,
    pub status_code: i32,
    #[ts(type = "number")]
    pub created_at: i64,
    pub duration_ms: i32,
    /// 拦截原因（proxy_log.blocked_reason，空 = 未被拦截）。
    pub blocked_reason: String,
    /// 失败摘要：最后一次带 error 的 attempt.error，否则响应体前 200 字符；成功为空。
    pub error: String,
}

/// 平台使用统计（从 proxy_logs 聚合）
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct PlatformUsageStats {
    #[ts(type = "number")]
    pub total_requests: i64,
    #[ts(type = "number")]
    pub success_count: i64,
    #[ts(type = "number")]
    pub total_input_tokens: i64,
    #[ts(type = "number")]
    pub total_output_tokens: i64,
    #[ts(type = "number")]
    pub total_cache_tokens: i64,
    pub cache_rate: f64,
    /// 最近 N 次请求中失败的次数（用于可用性判断）
    #[ts(type = "number")]
    pub recent_failures: i64,
    /// 最近 N 次请求的总数
    #[ts(type = "number")]
    pub recent_total: i64,
    /// 累计预估花费（$），基于 est_cost 聚合
    #[serde(default)]
    pub total_cost: f64,
    /// 今日（本地 00:00 起）token 总量（input + output），按 eff_pid 聚合
    #[serde(default)]
    #[ts(type = "number")]
    pub today_tokens: i64,
    /// 今日（本地 00:00 起）预估花费（$），基于 est_cost 聚合
    #[serde(default)]
    pub today_cost: f64,
    /// 今日（本地 00:00 起）请求数（分组列表卡片「今日请求」展示口径）
    #[serde(default)]
    #[ts(type = "number")]
    pub today_requests: i64,
    /// 今日（本地 00:00 起）成功数（与 today_requests 配对算成功率）
    #[serde(default)]
    #[ts(type = "number")]
    pub today_success: i64,
}

/// 平台「最近一次测试结果」（来自 proxy_log 中 source_protocol='test' 的最新一条）。
/// 供 PlatformCard 常驻徽章消费：ok/fail + 耗时 + 时间。
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct LastTestResult {
    /// status_code ∈ [200, 300) → true
    pub success: bool,
    pub status_code: i32,
    pub duration_ms: i32,
    /// proxy_log.created_at（毫秒 epoch）
    #[ts(type = "number")]
    pub created_at: i64,
    /// 失败时取 response_body 截断 ~200 字符；成功为空串（徽章 title 短摘要用）
    pub error: String,
    /// 测试响应正文（成功/失败均带），截断 ~4000 字符；供前端 JSON 解析结构化展示。
    pub response_body: String,
}

/// Summary row for list view (excludes large body fields)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ProxyLogSummary {
    pub id: String,
    pub group_key: String,
    pub model: String,
    pub actual_model: String,
    pub source_protocol: String,
    pub target_protocol: String,
    #[ts(type = "number")]
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
    /// 被拦截的环节（rate-limit-aware 票 04：'router' 等；空串 = 未被拦截）
    #[serde(default)]
    pub blocked_by: String,
    /// 拦截原因（'rate_limit' / 'peak' / 'observe' / …；空串 = 未被拦截）
    #[serde(default)]
    pub blocked_reason: String,
    /// 客户端代码（见 `ProxyLog.client`）；空串 = 未知
    #[serde(default)]
    pub client: String,
    /// 入站原始 User-Agent（见 `ProxyLog.user_agent`）；空串 = 无
    #[serde(default)]
    pub user_agent: String,
    /// 请求 URL；CONNECT 隧道行 = 目标 `host:port`（列表直接展示，免逐行取详情）
    #[serde(default)]
    pub request_url: String,
    /// 隧道上行字节数（migration 20261109-02，仅 http-connect 行有值，其余 0）
    #[serde(default)]
    #[ts(type = "number")]
    pub up_bytes: i64,
    /// 隧道下行字节数（同上）
    #[serde(default)]
    #[ts(type = "number")]
    pub down_bytes: i64,
    #[ts(type = "number")]
    pub created_at: i64,
    /// Claude Code 请求类型提示头（见 `ProxyLog.request_class`）；空串 = 无
    #[serde(default)]
    pub request_class: String,
    /// Claude Code 子代理类型提示头（见 `ProxyLog.agent_type`）；空串 = 无
    #[serde(default)]
    pub agent_type: String,
}

/// 分页探测结果（logs-query-ipc-slimming s2）：`items` 恒为 `limit` 行，`has_more` 由
/// `LIMIT limit+1` 探测出的第 limit+1 行是否存在决定（该行本身不下发，仅用于置位）。
/// 替代精确 `COUNT(*)` 全表扫，前端分页 UI 退化为「有更多/没有更多」，不再展示精确总数/总页数。
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ProxyLogPage {
    pub items: Vec<ProxyLogSummary>,
    pub has_more: bool,
}

/// 日志列表筛选条件
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ProxyLogFilter {
    #[ts(optional, type = "number | null")]
    pub platform_id: Option<u64>,
    #[ts(optional)]
    pub group_key: Option<String>,
    /// None=全部; Some(200)=仅成功; Some(-1)=仅失败
    #[ts(optional)]
    pub status: Option<i32>,
    #[ts(optional, type = "number | null")]
    pub time_start: Option<i64>,
    #[ts(optional, type = "number | null")]
    pub time_end: Option<i64>,
    /// 请求模型：按 model 列精确匹配
    #[ts(optional)]
    pub model: Option<String>,
    /// 实际模型：按 actual_model 列精确匹配，与 model 同时给出时 AND
    #[ts(optional)]
    pub actual_model: Option<String>,
    /// 路径片段：对 request_url 做 LIKE %v% 模糊匹配
    #[serde(default)]
    #[ts(optional)]
    pub path: Option<String>,
    /// None=全部; Some(non-empty)=source_protocol IN (...) 包含筛选（如 ["test","quota"]）
    #[serde(default)]
    #[ts(optional)]
    pub sources: Option<Vec<String>>,
    /// None=不排; Some(non-empty)=source_protocol NOT IN (...) 排除筛选。
    /// Logs 主页传 ["test","quota"] → 仅留纯代理转发，test/quota 仅请求日志页可见。
    /// 该谓词非恒真：test/quota 行真实写入 proxy_log（model_test.rs / quota/http.rs），不可跳过。
    /// source_protocol 列 `NOT NULL DEFAULT ''`（schema_early.rs:119），故 SQL 侧无需
    /// `OR source_protocol IS NULL` 防御（logs-query-ipc-slimming s3 已删该死分支）。
    #[serde(default)]
    #[ts(optional)]
    pub exclude_sources: Option<Vec<String>>,
    /// 中间件观察模式命中筛选（票 04）：Some(true) = 仅 `blocked_reason='observe'` 的行；
    /// None / Some(false) = 不加此谓词（全部）。
    #[serde(default)]
    #[ts(optional, type = "boolean | null")]
    pub observed: Option<bool>,
    /// 会话归组键：按 session_key 列精确匹配（magpie 票 09 会话统计下钻）
    #[serde(default)]
    #[ts(optional)]
    pub session_key: Option<String>,
    /// Claude Code `x-claude-code-prompt-id` 精确匹配
    #[serde(default)]
    #[ts(optional)]
    pub prompt_id: Option<String>,
    /// Claude Code `x-claude-code-session-id` 精确匹配
    #[serde(default)]
    #[ts(optional)]
    pub session_id: Option<String>,
}

/// 手动「立即清理」前的只读预估（三个数）。
///
/// 口径：`overdue_rows` / `overdue_body_bytes` 只按**整行保留期**（`retention_days` +
/// `retention_unit`）算，与 `cleanup_proxy_logs` 的删除谓词逐字同源，故预估行数 = 随后
/// 实际删除的行数。`db_size_bytes` 取 `page_count × page_size`（**不含 WAL 文件**，
/// WAL 在 checkpoint 后并入主文件，不是独立的可回收量）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct CleanupEstimate {
    /// 超过整行保留期、下次清理会被物理删除的行数（保留期=0 时恒为 0）
    #[ts(type = "number")]
    pub overdue_rows: i64,
    /// 上述行的四个 body 列字节总和（request/upstream_request/response/user_response）
    #[ts(type = "number")]
    pub overdue_body_bytes: i64,
    /// log.db 当前大小（page_count × page_size，不含 WAL）
    #[ts(type = "number")]
    pub db_size_bytes: i64,
}

/// Proxy logging settings stored in settings table (scope=proxy, key=logging)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ProxyLogSettings {
    /// Whether to record user-side raw data (request headers + request body +
    /// response headers + response body to client + request_url). 关闭后这些列入库即清空，只留解析后元数据。
    /// 元数据（token / cost / status / model / duration 等）恒记录，无总开关（2026-10-22 移除 enabled）。
    #[serde(default)]
    pub log_user_request: bool,

    /// Whether to record upstream-side raw data (upstream request headers + body +
    /// upstream response headers + upstream response body). 关闭后这些列入库即清空，只留解析后元数据。
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

    /// 单位（小时/天/周）—— 与同名 `*_retention_days` 字段值配对。
    /// serde default = Day：老 settings.json 缺此字段时按天解，配老 value 7 → 7 天不变（零迁移）。
    /// 新装 Default::default() 三项均改 6 + Hour = 6 小时。
    /// value=0 永久保留（不看单位，0 小时 = 0 天 = 永久）。
    #[serde(default)]
    pub user_request_retention_unit: RetentionUnit,
    #[serde(default)]
    pub upstream_request_retention_unit: RetentionUnit,
    #[serde(default)]
    pub retention_unit: RetentionUnit,
}

/// 保留期单位。serde lowercase（hour/day/week），Default = Day（老配置兼容）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum RetentionUnit {
    Hour,
    #[default]
    Day,
    Week,
}

impl RetentionUnit {
    /// 与数值配对换算成秒。Hour=3600 / Day=86400 / Week=604800。
    pub fn secs(self, value: u32) -> u64 {
        match self {
            Self::Hour => 3600 * value as u64,
            Self::Day => 86400 * value as u64,
            Self::Week => 604800 * value as u64,
        }
    }
}

impl ProxyLogSettings {
    /// 调度周期下限：1 分钟（再短的唤醒对磁盘没意义，只是空转）。
    pub const CLEANUP_INTERVAL_MIN_SECS: u64 = 60;
    /// 调度周期上限：24 小时（长保留期不需要更频繁的唤醒）。
    pub const CLEANUP_INTERVAL_MAX_SECS: u64 = 24 * 3600;

    /// 清理调度周期（秒）：三档保留期（user / upstream / 整行）取**最短的非零值 ÷ 4**，
    /// 夹在 [1 分钟, 24 小时]。三档全 0（永久保留）退化为 24 小时。
    ///
    /// ÷4 保证任一档保留期内至少被扫 4 次，超期数据最多多活 1/4 个保留期。
    /// 放在此处（与 `RetentionUnit::secs` 同层）是为了随内核走：无界面二进制复用同一份，
    /// 不在 root bin 里重写一遍。
    pub fn cleanup_interval_secs(&self) -> u64 {
        let shortest = [
            self.user_request_retention_unit
                .secs(self.user_request_retention_days),
            self.upstream_request_retention_unit
                .secs(self.upstream_request_retention_days),
            self.retention_unit.secs(self.retention_days),
        ]
        .into_iter()
        .filter(|&s| s > 0)
        .min();
        match shortest {
            // 三档全 0 = 永久保留：无超期数据可清，退化为最长周期。
            None => Self::CLEANUP_INTERVAL_MAX_SECS,
            Some(s) => (s / 4).clamp(
                Self::CLEANUP_INTERVAL_MIN_SECS,
                Self::CLEANUP_INTERVAL_MAX_SECS,
            ),
        }
    }
}

fn default_user_req_retention() -> u32 {
    7
}
fn default_upstream_req_retention() -> u32 {
    7
}
fn default_retention_days() -> u32 {
    90
}

impl Default for ProxyLogSettings {
    fn default() -> Self {
        // 新装默认：三项均 6h（value=6 + Hour）。
        // serde 反序列化路径独立：老 settings.json 走 serde field default（days 7/7/90 + unit Day），
        // 此处仅新装首次落库走 Default impl。
        Self {
            log_user_request: false,
            log_upstream_request: false,
            user_request_retention_days: 6,
            upstream_request_retention_days: 6,
            retention_days: 6,
            user_request_retention_unit: RetentionUnit::Hour,
            upstream_request_retention_unit: RetentionUnit::Hour,
            retention_unit: RetentionUnit::Hour,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ponytail: serde field default vs struct Default impl 是本任务核心契约，留一个可运行检查。
    /// 老配置 JSON（无 unit 字段）反序列化：serde field default 路径
    /// → days 7/7/90（fn 默认）+ unit Day（enum #[default]）→ 行为零迁移。
    #[test]
    fn old_config_without_unit_fields_defaults_to_day() {
        let json = r#"{"enabled":true}"#;
        let s: ProxyLogSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s.user_request_retention_days, 7);
        assert_eq!(s.upstream_request_retention_days, 7);
        assert_eq!(s.retention_days, 90);
        assert_eq!(s.user_request_retention_unit, RetentionUnit::Day);
        assert_eq!(s.upstream_request_retention_unit, RetentionUnit::Day);
        assert_eq!(s.retention_unit, RetentionUnit::Day);
        // 7 Day = 604800s（与改动前 7d 一致）
        assert_eq!(RetentionUnit::Day.secs(7), 604_800);
    }

    /// 新装 Default::default()：三项 value=6 + Hour → 6h（21600s）。
    #[test]
    fn default_impl_uses_hour_and_six() {
        let s = ProxyLogSettings::default();
        assert_eq!(s.user_request_retention_days, 6);
        assert_eq!(s.upstream_request_retention_days, 6);
        assert_eq!(s.retention_days, 6);
        assert_eq!(s.user_request_retention_unit, RetentionUnit::Hour);
        assert_eq!(s.retention_unit, RetentionUnit::Hour);
        assert_eq!(RetentionUnit::Hour.secs(6), 21_600);
    }

    /// value=0 永久保留：单位无关（Hour/Day/Week × 0 均为 0）。
    #[test]
    fn zero_value_is_forever_regardless_of_unit() {
        assert_eq!(RetentionUnit::Hour.secs(0), 0);
        assert_eq!(RetentionUnit::Day.secs(0), 0);
        assert_eq!(RetentionUnit::Week.secs(0), 0);
    }

    /// 构造三档保留期（value + unit 相同的简写），只为周期派生测试服务。
    fn settings_with(
        user: (u32, RetentionUnit),
        up: (u32, RetentionUnit),
        row: (u32, RetentionUnit),
    ) -> ProxyLogSettings {
        ProxyLogSettings {
            user_request_retention_days: user.0,
            user_request_retention_unit: user.1,
            upstream_request_retention_days: up.0,
            upstream_request_retention_unit: up.1,
            retention_days: row.0,
            retention_unit: row.1,
            ..Default::default()
        }
    }

    /// 周期派生①取最短：三档不同（1 天 / 12 小时 / 4 小时）→ 取 4 小时那档。
    #[test]
    fn cleanup_interval_takes_shortest_of_three() {
        let s = settings_with(
            (1, RetentionUnit::Day),
            (12, RetentionUnit::Hour),
            (4, RetentionUnit::Hour),
        );
        // 最短 = 4h = 14400s → ÷4 = 3600s
        assert_eq!(s.cleanup_interval_secs(), 3600);
    }

    /// 周期派生②÷4：零档不参与取最短（0 = 永久保留，不是「最短」）。
    #[test]
    fn cleanup_interval_divides_by_four_and_ignores_zero() {
        let s = settings_with(
            (0, RetentionUnit::Hour),
            (0, RetentionUnit::Hour),
            (8, RetentionUnit::Hour),
        );
        // 唯一非零档 8h = 28800s → ÷4 = 7200s
        assert_eq!(s.cleanup_interval_secs(), 7200);
    }

    /// 周期派生③上下夹紧：1 小时保留期 → 15 分钟；90 天 → 夹到 24 小时下限侧同理。
    #[test]
    fn cleanup_interval_clamps_both_ends() {
        // 1h ÷ 4 = 900s（区间内，不夹）
        let hourly = settings_with(
            (1, RetentionUnit::Hour),
            (1, RetentionUnit::Hour),
            (1, RetentionUnit::Hour),
        );
        assert_eq!(hourly.cleanup_interval_secs(), 900);

        // 下限：1 分钟保留期不存在（单位最小 Hour），构造 1h 的 1/4 已是 900s；
        // 用 Hour 档最小值 1 与更长档共存仍取最短 → 验证下限用极端值 0 不可行，
        // 故直接验证常量语义：任何派生值不得低于 60s。
        assert!(hourly.cleanup_interval_secs() >= ProxyLogSettings::CLEANUP_INTERVAL_MIN_SECS);

        // 上限：90 天 ÷ 4 = 22.5 天 → 夹到 24 小时
        let long = settings_with(
            (90, RetentionUnit::Day),
            (90, RetentionUnit::Day),
            (90, RetentionUnit::Day),
        );
        assert_eq!(
            long.cleanup_interval_secs(),
            ProxyLogSettings::CLEANUP_INTERVAL_MAX_SECS
        );
    }

    /// 周期派生④全零退化：三档全 0（永久保留）→ 24 小时。
    #[test]
    fn cleanup_interval_all_zero_falls_back_to_daily() {
        let s = settings_with(
            (0, RetentionUnit::Hour),
            (0, RetentionUnit::Day),
            (0, RetentionUnit::Week),
        );
        assert_eq!(s.cleanup_interval_secs(), 24 * 3600);
    }

    /// 旧 routing JSON（无 target_model 字段）可反序列化；新序列化 None 省略字段。
    #[test]
    fn route_candidate_target_model_backward_compatible() {
        let old = r#"{"order":1,"platform":"p","platform_id":1,"level_priority":5,"priority":0,"reason":"selected","tags":[]}"#;
        let parsed: RouteCandidate = serde_json::from_str(old).unwrap();
        assert_eq!(parsed.target_model, None);
        assert!(parsed.model_via.is_empty());

        let with_model = RouteCandidate {
            order: 1,
            platform: "p".into(),
            platform_id: 1,
            level_priority: 5,
            priority: 0,
            reason: RouteReason::Selected,
            target_model: Some("m-1".into()),
            model_via: Vec::new(),
            model_chain: Vec::new(),
            tags: Vec::new(),
        };
        let json = serde_json::to_string(&with_model).unwrap();
        assert!(json.contains(r#""target_model":"m-1""#));

        let none = RouteCandidate { target_model: None, ..with_model.clone() };
        let json_none = serde_json::to_string(&none).unwrap();
        assert!(!json_none.contains("target_model"));
        assert_eq!(serde_json::from_str::<RouteCandidate>(&json).unwrap(), with_model);
    }
}
