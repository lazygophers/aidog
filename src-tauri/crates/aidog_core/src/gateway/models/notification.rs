//! 通知模块模型（系统通知 N1/N2）：类型/形态/TTS 后端 + 类型/事件配置 + 总设置 + 收件箱项。

use super::default_true;
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "test_notification.rs"]
mod test_notification;

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

/// Claude Code 官方 hook 事件全量目录（约 30 个；来源 code.claude.com/docs/zh-Hans/hooks）。
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
/// `{project}`(项目名)/`{session}`(会话id)。专属入参来源 code.claude.com/docs/zh-Hans/hooks
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
