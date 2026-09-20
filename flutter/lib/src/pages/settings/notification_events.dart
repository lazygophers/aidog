/// 逐 hook 事件通知配置的**数据表**（票 I16）—— 逐字镜像
/// `src/components/settings/NotificationEventList.tsx` 的 `CC_HOOK_EVENTS` /
/// `DEFAULT_ON_EVENTS` / `EVENT_CATALOG`，而那一份又是后端
/// `src-tauri/src/gateway/models.rs` 的镜像。
///
/// 🔴 三份是同一张表的三个投影，**改一侧必须逐字同步另外两侧**。
/// 模板串是中文硬编码（非 i18n），与 React 侧一致 —— 它是后端默认值的回显，
/// 翻译了反而与实际发出的通知不符。
library;

/// Claude Code 官方 hook 事件全量目录（30 个，顺序即 React 侧顺序）。
const List<String> kCcHookEvents = [
  'SessionStart',
  'Setup',
  'InstructionsLoaded',
  'UserPromptSubmit',
  'UserPromptExpansion',
  'MessageDisplay',
  'PreToolUse',
  'PermissionRequest',
  'PermissionDenied',
  'PostToolUse',
  'PostToolUseFailure',
  'PostToolBatch',
  'Notification',
  'SubagentStart',
  'SubagentStop',
  'Stop',
  'StopFailure',
  'TeammateIdle',
  'TaskCreated',
  'TaskCompleted',
  'ConfigChange',
  'CwdChanged',
  'FileChanged',
  'WorktreeCreate',
  'WorktreeRemove',
  'PreCompact',
  'PostCompact',
  'Elicitation',
  'ElicitationResult',
  'SessionEnd',
];

/// 默认 ON 的精选集（任务完成 + 等待授权），其余默认 off。
const List<String> kDefaultOnEvents = ['Stop', 'PermissionRequest'];

/// 通用入参（所有事件都有）。
const List<String> kCommonEventVars = ['{project}', '{session}'];

/// 每事件的专属默认模板 + 专属入参（不含 [kCommonEventVars]）。
const Map<String, ({String defaultTemplate, List<String> vars})>
kEventCatalog = {
  'SessionStart': (
    defaultTemplate: '{project} 会话开始',
    vars: ['{source}', '{model}', '{agent_type}', '{session_title}'],
  ),
  'Setup': (defaultTemplate: '{project} 初始化（{trigger}）', vars: ['{trigger}']),
  'InstructionsLoaded': (
    defaultTemplate: '{project} 已加载 {memory_type}',
    vars: ['{file_path}', '{memory_type}', '{load_reason}'],
  ),
  'UserPromptSubmit': (
    defaultTemplate: '{project} 收到新指令',
    vars: ['{prompt}'],
  ),
  'UserPromptExpansion': (
    defaultTemplate: '{project} 展开命令 {command_name}',
    vars: ['{command_name}', '{command_args}'],
  ),
  'MessageDisplay': (
    defaultTemplate: '{project} 消息更新',
    vars: ['{turn_id}', '{final}'],
  ),
  'PreToolUse': (
    defaultTemplate: '{project} 即将执行 {tool_name}',
    vars: ['{tool_name}'],
  ),
  'PermissionRequest': (
    defaultTemplate: '{project} 请求授权：{tool_name}',
    vars: ['{tool_name}'],
  ),
  'PermissionDenied': (
    defaultTemplate: '{project} 拒绝 {tool_name}：{reason}',
    vars: ['{tool_name}', '{reason}'],
  ),
  'PostToolUse': (
    defaultTemplate: '{project} {tool_name} 完成（{duration_ms}ms）',
    vars: ['{tool_name}', '{duration_ms}'],
  ),
  'PostToolUseFailure': (
    defaultTemplate: '{project} {tool_name} 失败：{error}',
    vars: ['{tool_name}', '{error}'],
  ),
  'PostToolBatch': (defaultTemplate: '{project} 批量工具完成', vars: []),
  'Notification': (
    defaultTemplate: '{project}：{message}',
    vars: ['{message}', '{type}'],
  ),
  'SubagentStart': (
    defaultTemplate: '{project} 子代理 {agent_type} 启动',
    vars: ['{agent_type}'],
  ),
  'SubagentStop': (
    defaultTemplate: '{project} 子代理 {agent_type} 完成',
    vars: ['{agent_type}'],
  ),
  'Stop': (defaultTemplate: '{project} 任务完成', vars: []),
  'StopFailure': (
    defaultTemplate: '{project} 中断：{error_message}',
    vars: ['{error_code}', '{error_message}'],
  ),
  'TeammateIdle': (
    defaultTemplate: '{project} 队友 {teammate_id} 空闲',
    vars: ['{teammate_id}', '{status}'],
  ),
  'TaskCreated': (
    defaultTemplate: '{project} 新建任务：{task_name}',
    vars: ['{task_id}', '{task_name}'],
  ),
  'TaskCompleted': (
    defaultTemplate: '{project} 任务完成：{task_name}',
    vars: ['{task_id}', '{task_name}'],
  ),
  'ConfigChange': (
    defaultTemplate: '{project} 配置变更（{config_source}）',
    vars: ['{config_source}'],
  ),
  'CwdChanged': (
    defaultTemplate: '{project} 切换目录：{new_cwd}',
    vars: ['{old_cwd}', '{new_cwd}'],
  ),
  'FileChanged': (
    defaultTemplate: '{project} 文件变更：{file_path}',
    vars: ['{file_path}', '{change_type}'],
  ),
  'WorktreeCreate': (
    defaultTemplate: '{project} 创建 worktree',
    vars: ['{worktree_path}'],
  ),
  'WorktreeRemove': (
    defaultTemplate: '{project} 移除 worktree',
    vars: ['{worktree_path}'],
  ),
  'PreCompact': (
    defaultTemplate: '{project} 即将压缩上下文（{compact_reason}）',
    vars: ['{compact_reason}', '{context_size}'],
  ),
  'PostCompact': (
    defaultTemplate: '{project} 压缩完成',
    vars: ['{context_reduction_ratio}'],
  ),
  'Elicitation': (
    defaultTemplate: '{project} {server_name} 请求输入',
    vars: ['{server_name}', '{tool_name}'],
  ),
  'ElicitationResult': (
    defaultTemplate: '{project} {server_name} 已响应',
    vars: ['{server_name}'],
  ),
  'SessionEnd': (
    defaultTemplate: '{project} 会话结束（{end_reason}）',
    vars: ['{end_reason}', '{duration_ms}'],
  ),
};

/// 某事件的默认模板；表里没有就只回 `{project}`（与后端兜底呼应）。
String defaultTemplateForEvent(String event) =>
    kEventCatalog[event]?.defaultTemplate ?? '{project}';

/// 某事件可用入参 = 通用 + 专属。
List<String> eventVars(String event) => [
  ...kCommonEventVars,
  ...(kEventCatalog[event]?.vars ?? const []),
];

/// 列表顺序：默认 ON 精选集在前，其余按目录顺序。
List<String> orderedHookEvents() => [
  ...kDefaultOnEvents.where(kCcHookEvents.contains),
  ...kCcHookEvents.where((e) => !kDefaultOnEvents.contains(e)),
];

/// 单事件配置。字段名照 `generated/EventSetting.ts`。
class EventSetting {
  const EventSetting({
    required this.enabled,
    required this.tts,
    required this.popup,
    required this.sound,
    required this.template,
  });

  final bool enabled;
  final bool tts;
  final bool popup;
  final bool sound;
  final String template;

  /// serde default：缺 `tts`/`popup`/`sound` 一律 true（向后兼容旧 DB 行）。
  factory EventSetting.fromJson(Map<String, Object?> j) => EventSetting(
    enabled: j['enabled'] as bool? ?? false,
    tts: j['tts'] as bool? ?? true,
    popup: j['popup'] as bool? ?? true,
    sound: j['sound'] as bool? ?? true,
    template: j['template'] as String? ?? '',
  );

  Map<String, Object?> toJson() => {
    'enabled': enabled,
    'tts': tts,
    'popup': popup,
    'sound': sound,
    'template': template,
  };

  EventSetting copyWith({
    bool? enabled,
    bool? tts,
    bool? popup,
    bool? sound,
    String? template,
  }) => EventSetting(
    enabled: enabled ?? this.enabled,
    tts: tts ?? this.tts,
    popup: popup ?? this.popup,
    sound: sound ?? this.sound,
    template: template ?? this.template,
  );
}

/// 有效展示态：`per_event` 命中用存储值，否则按默认目录兜底
/// （精选集 on + 三通道默认开），与 `effectiveSetting` 逐条一致。
EventSetting effectiveEventSetting(
  Map<String, Object?> perEvent,
  String event,
) {
  final stored = perEvent[event];
  if (stored is Map) {
    return EventSetting.fromJson(Map<String, Object?>.from(stored));
  }
  return EventSetting(
    enabled: kDefaultOnEvents.contains(event),
    tts: true,
    popup: true,
    sound: true,
    template: '',
  );
}
