/// Claude 设置页的 **hooks 构建器**（I17）—— 对齐 React
/// `src/components/settings/editors/HooksSectionInline.tsx` +
/// `hooks-types.tsx`（HOOK_EVENTS / HANDLER_TYPES / notify 快捷条）。
///
/// 结构：事件选择器加 matcher 组 → 组内加处理器（command / http / mcp_tool /
/// prompt / agent 五类，按类型出参数表单）。写回同一个 `hooks` 对象：
/// 空 handler 组 / 空事件一并清掉，全空 → null（`syncHooks` 同语义）。
///
/// notify 快捷条对齐 `NotifyHookQuickBar`：`build_notify_hooks_fragment`（只读式，
/// 不写 DB）→ 并入草稿 + `_aidog_hooks.enabled=true`；移除 = 剥 aidog 项 + false。
/// 与后端 `gateway::hooks` 的识别约定一致：按命令串含脚本文件名识别。
library;

import '../../../i18n.dart';
import '../../shell/theme.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';

import 'package:flutter/material.dart';

/// 事件元数据（`HOOK_EVENTS` 逐条镜像）。hasMatcher 决定处理器里有没有「条件 if」。
class HookEvent {
  const HookEvent(
    this.id,
    this.desc, {
    required this.hasMatcher,
    this.matcherOptions = const [],
    required this.matcherFreeform,
  });

  final String id;
  final String desc;
  final bool hasMatcher;
  final List<String> matcherOptions;
  final bool matcherFreeform;
}

const List<HookEvent> kHookEvents = [
  HookEvent(
    'SessionStart',
    '会话启动或恢复时触发',
    hasMatcher: true,
    matcherOptions: ['startup', 'resume', 'clear', 'compact'],
    matcherFreeform: false,
  ),
  HookEvent(
    'UserPromptSubmit',
    '用户提交提示时触发',
    hasMatcher: false,
    matcherFreeform: false,
  ),
  HookEvent(
    'PreToolUse',
    '工具调用前触发，可阻止',
    hasMatcher: true,
    matcherOptions: [
      'Bash',
      'Edit',
      'Write',
      'Read',
      'Glob',
      'Grep',
      'WebFetch',
      'Agent',
    ],
    matcherFreeform: true,
  ),
  HookEvent(
    'PostToolUse',
    '工具调用成功后触发',
    hasMatcher: true,
    matcherOptions: [
      'Bash',
      'Edit',
      'Write',
      'Read',
      'Glob',
      'Grep',
      'WebFetch',
      'Agent',
    ],
    matcherFreeform: true,
  ),
  HookEvent(
    'Notification',
    '发送通知时触发',
    hasMatcher: true,
    matcherOptions: [
      'permission_prompt',
      'idle_prompt',
      'auth_success',
      'elicitation_dialog',
    ],
    matcherFreeform: false,
  ),
  HookEvent(
    'Stop',
    'Claude 完成响应时触发',
    hasMatcher: false,
    matcherFreeform: false,
  ),
  HookEvent(
    'SubagentStop',
    '子代理完成时触发',
    hasMatcher: true,
    matcherOptions: ['general-purpose', 'Explore', 'Plan'],
    matcherFreeform: true,
  ),
  HookEvent(
    'ConfigChange',
    '配置文件变更时触发',
    hasMatcher: true,
    matcherOptions: [
      'user_settings',
      'project_settings',
      'local_settings',
      'policy_settings',
      'skills',
    ],
    matcherFreeform: false,
  ),
  HookEvent(
    'FileChanged',
    '监视文件变更时触发',
    hasMatcher: true,
    matcherFreeform: true,
  ),
  HookEvent(
    'CwdChanged',
    '工作目录切换时触发',
    hasMatcher: false,
    matcherFreeform: false,
  ),
  HookEvent(
    'PreCompact',
    '上下文压缩前触发',
    hasMatcher: true,
    matcherOptions: ['manual', 'auto'],
    matcherFreeform: false,
  ),
  HookEvent(
    'SessionEnd',
    '会话结束时触发',
    hasMatcher: true,
    matcherOptions: ['clear', 'resume', 'logout', 'prompt_input_exit', 'other'],
    matcherFreeform: false,
  ),
];

HookEvent? _eventMeta(String id) {
  for (final e in kHookEvents) {
    if (e.id == id) return e;
  }
  return null;
}

/// 五类处理器（`HANDLER_TYPES` + `HANDLER_LABELS`）。
const List<(String, String)> kHandlerTypes = [
  ('command', '命令'),
  ('http', 'HTTP'),
  ('mcp_tool', 'MCP 工具'),
  ('prompt', 'LLM 提示'),
  ('agent', 'Agent 验证'),
];

String _handlerLabel(I18nController t, String ty) {
  final e = kHandlerTypes.where((h) => h.$1 == ty).firstOrNull;
  return e == null ? ty : tOr(t, 'settings.hooks.handler.${e.$1}', e.$2);
}

/// aidog notify 项的识别标记（后端 `gateway::hooks` 同约定）。
const List<String> _aidogNotifyMarkers = [
  'aidog-notify-complete',
  'aidog-notify-waiting',
];

bool _isAidogNotifyHandler(Map<String, Object?> h) {
  final cmd = '${h['command'] ?? ''}';
  return _aidogNotifyMarkers.any(cmd.contains);
}

bool _hasNotifyHooks(Map<String, Object?> hooks) {
  for (final groups in hooks.values.whereType<List>()) {
    for (final g in groups.whereType<Map>()) {
      for (final h in (g['hooks'] as List? ?? const []).whereType<Map>()) {
        if (_isAidogNotifyHandler(Map<String, Object?>.from(h))) return true;
      }
    }
  }
  return false;
}

/// hooks → 清洗后的 hooks；空 → null（`syncHooks`）。
Map<String, Object?>? _cleanHooks(Map<String, Object?> hooks) {
  final cleaned = <String, Object?>{};
  for (final e in hooks.entries) {
    final kept = (e.value as List? ?? const [])
        .whereType<Map>()
        .map(Map<String, Object?>.from)
        .where((g) => (g['hooks'] as List? ?? const []).isNotEmpty)
        .toList();
    if (kept.isNotEmpty) cleaned[e.key] = kept;
  }
  return cleaned.isEmpty ? null : cleaned;
}

/// 从 hooks 剥掉全部 aidog notify 项（`stripNotifyHooks`）；空 → null。
Map<String, Object?>? _stripNotifyHooks(Map<String, Object?>? hooks) {
  if (hooks == null) return null;
  final out = <String, Object?>{};
  for (final e in hooks.entries) {
    final kept = <Map<String, Object?>>[];
    for (final g in (e.value as List? ?? const []).whereType<Map>()) {
      final gm = Map<String, Object?>.from(g);
      final handlers = (gm['hooks'] as List? ?? const [])
          .whereType<Map>()
          .map(Map<String, Object?>.from)
          .where((h) => !_isAidogNotifyHandler(h))
          .toList();
      if (handlers.isNotEmpty) kept.add({...gm, 'hooks': handlers});
    }
    if (kept.isNotEmpty) out[e.key] = kept;
  }
  return out.isEmpty ? null : out;
}

/// 把后端 notify 片段并入当前 hooks（先剥旧 aidog 项，幂等；`mergeNotifyHooks`）。
Map<String, Object?> _mergeNotifyHooks(
  Map<String, Object?>? current,
  Map<String, Object?> fragment,
) {
  final merged = _stripNotifyHooks(current) ?? <String, Object?>{};
  for (final e in fragment.entries) {
    for (final g in (e.value as List? ?? const []).whereType<Map>()) {
      final handlers = (g['hooks'] as List? ?? const [])
          .whereType<Map>()
          .map(Map<String, Object?>.from)
          .where(_isAidogNotifyHandler)
          .map((h) => {...h, 'type': '${h['type'] ?? 'command'}'})
          .toList();
      if (handlers.isEmpty) continue;
      final existing = (merged[e.key] as List? ?? const [])
          .whereType<Map>()
          .toList();
      merged[e.key] = [
        ...existing,
        {'matcher': '', 'hooks': handlers},
      ];
    }
  }
  return merged;
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}

class HooksEditor extends StatefulWidget {
  const HooksEditor({
    super.key,
    required this.hooks,
    required this.onChanged,
    required this.updateField,
    required this.invoke,
  });

  /// `config.hooks ?? {}`（整份）。
  final Map<String, Object?> hooks;

  /// 整份写回 hooks；空 → null。
  final ValueChanged<Object?> onChanged;

  /// 写任意 config 键（notify 快捷条要写 `_aidog_hooks`）。
  final void Function(String field, Object? value) updateField;

  final InvokeFn invoke;

  @override
  State<HooksEditor> createState() => _HooksEditorState();
}

class _HooksEditorState extends State<HooksEditor> {
  /// 事件 → 是否展开（缺省展开，与 React `isExpanded` 一致）。
  final Map<String, bool> _collapsed = {};
  bool _notifyBusy = false;
  String _notifyError = '';

  List<Map<String, Object?>> _groupsOf(String eventId) =>
      (widget.hooks[eventId] as List? ?? const [])
          .whereType<Map>()
          .map(Map<String, Object?>.from)
          .toList();

  Map<String, Object?> _groupAt(String eventId, int gi) =>
      _groupsOf(eventId)[gi];

  List<Map<String, Object?>> _handlersOf(String eventId, int gi) =>
      (_groupAt(eventId, gi)['hooks'] as List? ?? const [])
          .whereType<Map>()
          .map(Map<String, Object?>.from)
          .toList();

  /// 改某事件的 matcher 组列表后写回（空组 / 空事件清掉）。
  void _writeGroups(String eventId, List<Map<String, Object?>> groups) {
    final next = {...widget.hooks};
    if (groups.isEmpty) {
      next.remove(eventId);
    } else {
      next[eventId] = groups;
    }
    widget.onChanged(_cleanHooks(next));
  }

  void _patchGroup(
    String eventId,
    int gi,
    Map<String, Object?> Function(Map<String, Object?> g) patch,
  ) {
    final groups = [..._groupsOf(eventId)];
    groups[gi] = patch(groups[gi]);
    _writeGroups(eventId, groups);
  }

  void _patchHandler(
    String eventId,
    int gi,
    int hi,
    Map<String, Object?> patch,
  ) {
    _patchGroup(eventId, gi, (g) {
      final handlers = [..._handlersOf(eventId, gi)];
      final cur = handlers[hi];
      final next = {...cur, ...patch};
      for (final e in patch.entries) {
        if (e.value == null) next.remove(e.key);
      }
      handlers[hi] = next;
      return {...g, 'hooks': handlers};
    });
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final events = widget.hooks.keys.where((e) => _groupsOf(e).isNotEmpty);
    var totalHooks = 0;
    for (final e in events) {
      totalHooks += _groupsOf(e)
          .fold(0, (s, g) => s + (g['hooks'] as List? ?? const []).length);
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        _notifyQuickBar(t),
        // 事件选择器：选一个就给该事件加一个 matcher 组。
        SelectRow(
          key: const ValueKey('hooks-add-event'),
          label: tOr(t, 'settings.hooks.addEvent', '+ 添加 Hook 事件…'),
          options: [for (final e in kHookEvents) e.id],
          value: '',
          onChanged: (v) {
            if (v == null || v.isEmpty) return;
            _writeGroups(v, [
              ..._groupsOf(v),
              {
                'matcher': '',
                'hooks': [
                  {'type': 'command', 'command': ''},
                ],
              },
            ]);
            setState(() => _collapsed[v] = false);
          },
          labelOf: (id) {
            final meta = _eventMeta(id);
            return meta == null
                ? id
                : '$id — ${tOr(t, 'settings.hooks.event.$id.desc', meta.desc)}';
          },
        ),
        if (totalHooks == 0)
          Text(
            '${tOr(t, 'settings.hooks.introLine1', 'Hooks 在 Claude Code 生命周期的特定点自动执行命令/HTTP请求/LLM提示。')}\n'
            '${tOr(t, 'settings.hooks.introLine2', '选择事件类型开始配置。')}',
            style: AidogType.micro.copyWith(
              color: AidogTheme.of(context).c.fg3,
            ),
          ),
        for (final eventId in events) _eventCard(t, eventId),
      ],
    );
  }

  /// 通知 hook 快捷注入 / 移除（`NotifyHookQuickBar`）。
  Widget _notifyQuickBar(I18nController t) {
    final theme = AidogTheme.of(context);
    final injected = _hasNotifyHooks(widget.hooks);
    return Container(
      key: const ValueKey('hooks-notify-bar'),
      margin: const EdgeInsets.only(bottom: AidogSpace.ssm),
      padding: const EdgeInsets.all(AidogSpace.ssm),
      decoration: BoxDecoration(
        color: theme.c.surface2,
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.md),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  tOr(t, 'settings.hooksQuickTitle', '通知 hook'),
                  style: AidogType.label.copyWith(
                    color: theme.c.fg,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                Text(
                  injected
                      ? tOr(
                          t,
                          'settings.hooksNotifyInjected',
                          '已注入 Claude Code 完成/等待通知 hook，保存后对全部分组生效。',
                        )
                      : tOr(
                          t,
                          'settings.hooksQuickDesc',
                          '一键填入 Claude Code 完成/等待通知 hook，保存后对全部分组与 Codex 生效。',
                        ),
                  style: AidogType.micro.copyWith(color: theme.c.fg2),
                ),
              ],
            ),
          ),
          const SizedBox(width: AidogSpace.ssm),
          if (injected)
            SmallButton(
              key: const ValueKey('hooks-notify-remove'),
              label: tOr(t, 'notif.hookRemove', '移除'),
              onTap: _notifyBusy
                  ? null
                  : () {
                      widget.onChanged(_stripNotifyHooks(widget.hooks));
                      widget.updateField('_aidog_hooks', {'enabled': false});
                    },
            )
          else
            SmallButton(
              key: const ValueKey('hooks-notify-inject'),
              label: _notifyBusy
                  ? tOr(t, 'settings.hooksNotifyBusy', '处理中…')
                  : tOr(t, 'settings.hooksNotifyInject', '注入通知 hook'),
              onTap: _notifyBusy ? null : _injectNotify,
            ),
          if (_notifyError.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(left: AidogSpace.ssm),
              child: Text(
                tOr(t, 'settings.hooksNotifyError', '操作失败，请重试'),
                style: AidogType.micro.copyWith(color: theme.c.bad),
              ),
            ),
        ],
      ),
    );
  }

  Future<void> _injectNotify() async {
    setState(() {
      _notifyBusy = true;
      _notifyError = '';
    });
    try {
      final fragment = await widget.invoke('build_notify_hooks_fragment');
      final merged = _mergeNotifyHooks(
        widget.hooks,
        fragment is Map ? Map<String, Object?>.from(fragment) : const {},
      );
      widget.onChanged(merged);
      widget.updateField('_aidog_hooks', {'enabled': true});
    } catch (_) {
      setState(() => _notifyError = 'failed');
    } finally {
      if (mounted) setState(() => _notifyBusy = false);
    }
  }

  Widget _eventCard(I18nController t, String eventId) {
    final theme = AidogTheme.of(context);
    final meta = _eventMeta(eventId);
    final groups = _groupsOf(eventId);
    final count = groups.fold(
      0,
      (s, g) => s + (g['hooks'] as List? ?? const []).length,
    );
    final expanded = !(_collapsed[eventId] ?? false);
    return Container(
      key: ValueKey('hooks-event-$eventId'),
      margin: const EdgeInsets.only(bottom: AidogSpace.ssm),
      padding: const EdgeInsets.all(AidogSpace.ssm),
      decoration: BoxDecoration(
        color: theme.c.surface2,
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.md),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              InkWell(
                onTap: () => setState(() => _collapsed[eventId] = expanded),
                child: Icon(
                  expanded ? Icons.expand_less : Icons.expand_more,
                  size: 16,
                  color: theme.c.fg3,
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              Text(
                eventId,
                style: AidogType.label.copyWith(color: theme.c.accent),
              ),
              if (meta != null)
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.only(left: AidogSpace.sxs),
                    child: Text(
                      tOr(t, 'settings.hooks.event.$eventId.desc', meta.desc),
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                  ),
                )
              else
                const Spacer(),
              Text(
                '$count',
                key: ValueKey('hooks-count-$eventId'),
                style: AidogType.micro.copyWith(color: theme.c.accent),
              ),
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(
                key: ValueKey('hooks-del-event-$eventId'),
                label: '×',
                onTap: () => _writeGroups(eventId, const []),
              ),
            ],
          ),
          if (expanded) ...[
            for (var gi = 0; gi < groups.length; gi++)
              _matcherGroup(t, eventId, gi),
            SmallButton(
              key: ValueKey('hooks-add-group-$eventId'),
              label: tOr(t, 'settings.hooks.addMatcherGroup', '+ 匹配器组'),
              onTap: () => _writeGroups(eventId, [
                ...groups,
                {
                  'matcher': '',
                  'hooks': [
                    {'type': 'command', 'command': ''},
                  ],
                },
              ]),
            ),
          ],
        ],
      ),
    );
  }

  Widget _matcherGroup(I18nController t, String eventId, int gi) {
    final theme = AidogTheme.of(context);
    final group = _groupAt(eventId, gi);
    final meta = _eventMeta(eventId);
    final matcher = '${group['matcher'] ?? ''}';
    final tags = matcher
        .split('|')
        .map((s) => s.trim())
        .where((s) => s.isNotEmpty)
        .toList();
    final handlers = _handlersOf(eventId, gi);
    return Container(
      key: ValueKey('hooks-group-$eventId-$gi'),
      margin: const EdgeInsets.symmetric(vertical: AidogSpace.sxs),
      padding: const EdgeInsets.only(left: AidogSpace.smd),
      decoration: BoxDecoration(
        border: Border(left: BorderSide(color: theme.c.accent, width: 2)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Text(
                tOr(t, 'settings.hooks.matcher', '匹配器'),
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              ),
              const SizedBox(width: AidogSpace.ssm),
              Expanded(
                child: Wrap(
                  spacing: AidogSpace.sxs,
                  runSpacing: AidogSpace.sxs,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    if (meta != null && meta.matcherOptions.isNotEmpty)
                      for (final opt in meta.matcherOptions)
                        SmallButton(
                          key: ValueKey('hooks-matcher-$eventId-$gi-$opt'),
                          label: opt,
                          active: tags.contains(opt),
                          onTap: () {
                            final next = tags.contains(opt)
                                ? tags.where((x) => x != opt).toList()
                                : [...tags, opt];
                            _patchGroup(
                              eventId,
                              gi,
                              (g) => {...g, 'matcher': next.join('|')},
                            );
                          },
                        )
                    else if (meta != null && meta.matcherFreeform)
                      SizedBox(
                        width: 260,
                        child: PlainTextField(
                          value: matcher,
                          hint: eventId == 'FileChanged'
                              ? tOr(
                                  t,
                                  'settings.hooks.matcherFilePh',
                                  '文件名，如 .envrc|.env',
                                )
                              : tOr(
                                  t,
                                  'settings.hooks.matcherToolPh',
                                  '工具名称或正则，多个用 | 分隔',
                                ),
                          onSubmitted: (v) => _patchGroup(
                            eventId,
                            gi,
                            (g) => {...g, 'matcher': v},
                          ),
                        ),
                      )
                    else
                      Text(
                        tOr(t, 'settings.hooks.matchAll', '匹配所有'),
                        style: AidogType.micro.copyWith(color: theme.c.fg3),
                      ),
                  ],
                ),
              ),
              SmallButton(
                key: ValueKey('hooks-del-group-$eventId-$gi'),
                label: '×',
                onTap: () => _writeGroups(
                  eventId,
                  [..._groupsOf(eventId)]..removeAt(gi),
                ),
              ),
            ],
          ),
          for (var hi = 0; hi < handlers.length; hi++)
            _handler(t, eventId, gi, hi),
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: SmallButton(
              key: ValueKey('hooks-add-handler-$eventId-$gi'),
              label: tOr(t, 'settings.hooks.addHandler', '+ 处理器'),
              onTap: () => _patchGroup(
                eventId,
                gi,
                (g) => {
                  ...g,
                  'hooks': [
                    ..._handlersOf(eventId, gi),
                    {'type': 'command', 'command': ''},
                  ],
                },
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _handler(I18nController t, String eventId, int gi, int hi) {
    final theme = AidogTheme.of(context);
    final h = _handlersOf(eventId, gi)[hi];
    final type = '${h['type'] ?? 'command'}';
    final meta = _eventMeta(eventId);
    return Container(
      key: ValueKey('hooks-handler-$eventId-$gi-$hi'),
      margin: const EdgeInsets.symmetric(vertical: AidogSpace.sxs),
      padding: const EdgeInsets.all(AidogSpace.ssm),
      decoration: BoxDecoration(
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Text(
                _handlerLabel(t, type),
                style: AidogType.micro.copyWith(color: theme.c.accent),
              ),
              const SizedBox(width: AidogSpace.ssm),
              DropdownButton<String>(
                value: kHandlerTypes.any((x) => x.$1 == type) ? type : null,
                items: [
                  for (final ht in kHandlerTypes)
                    DropdownMenuItem(
                      value: ht.$1,
                      child: Text(_handlerLabel(t, ht.$1)),
                    ),
                ],
                onChanged: (v) {
                  if (v == null) return;
                  // 换类型时丢掉旧类型的参数（React 只保留公共字段，照搬）。
                  _patchHandler(eventId, gi, hi, {
                    'type': v,
                    'command': null,
                    'url': null,
                    'server': null,
                    'tool': null,
                    'prompt': null,
                    'shell': null,
                  });
                },
                style: AidogType.micro.copyWith(color: theme.c.fg),
                underline: const SizedBox.shrink(),
                iconSize: 16,
              ),
              const Spacer(),
              SmallButton(
                key: ValueKey('hooks-del-handler-$eventId-$gi-$hi'),
                label: '×',
                onTap: () => _patchGroup(
                  eventId,
                  gi,
                  (g) => {
                    ...g,
                    'hooks': [..._handlersOf(eventId, gi)]..removeAt(hi),
                  },
                ),
              ),
            ],
          ),
          if (type == 'command') ...[
            TextRow(
              key: ValueKey('hooks-cmd-$eventId-$gi-$hi'),
              label: tOr(t, 'settings.hooks.fieldCommand', '命令'),
              value: '${h['command'] ?? ''}',
              maxLines: 3,
              hint: tOr(t, 'settings.hooks.commandPh', '命令或脚本路径'),
              onSubmitted: (v) => _patchHandler(eventId, gi, hi, {
                'command': v.isEmpty ? null : v,
              }),
            ),
            SelectRow(
              label: 'Shell',
              options: const ['', 'powershell'],
              value: '${h['shell'] ?? ''}',
              onChanged: (v) => _patchHandler(eventId, gi, hi, {
                'shell': v == null || v.isEmpty ? null : v,
              }),
              labelOf: (v) => v.isEmpty ? 'Bash' : 'PowerShell',
            ),
          ],
          if (type == 'http')
            TextRow(
              label: 'URL',
              value: '${h['url'] ?? ''}',
              hint: tOr(t, 'settings.hooks.urlPh', 'HTTP URL'),
              onSubmitted: (v) =>
                  _patchHandler(eventId, gi, hi, {'url': v.isEmpty ? null : v}),
            ),
          if (type == 'mcp_tool') ...[
            TextRow(
              label: tOr(t, 'settings.hooks.fieldServer', '服务器'),
              value: '${h['server'] ?? ''}',
              hint: tOr(t, 'settings.hooks.serverPh', 'MCP 服务器名称'),
              onSubmitted: (v) => _patchHandler(eventId, gi, hi, {
                'server': v.isEmpty ? null : v,
              }),
            ),
            TextRow(
              label: tOr(t, 'settings.hooks.fieldTool', '工具'),
              value: '${h['tool'] ?? ''}',
              hint: tOr(t, 'settings.hooks.toolPh', '工具名称'),
              onSubmitted: (v) => _patchHandler(eventId, gi, hi, {
                'tool': v.isEmpty ? null : v,
              }),
            ),
          ],
          if (type == 'prompt' || type == 'agent')
            TextRow(
              label: tOr(t, 'settings.hooks.fieldPrompt', '提示'),
              value: '${h['prompt'] ?? ''}',
              maxLines: 3,
              hint: tOr(
                t,
                'settings.hooks.promptPh',
                '提示文本，用 \$ARGUMENTS 插入 hook 输入数据',
              ),
              onSubmitted: (v) => _patchHandler(eventId, gi, hi, {
                'prompt': v.isEmpty ? null : v,
              }),
            ),
          if (meta != null && meta.hasMatcher)
            TextRow(
              label: tOr(t, 'settings.hooks.fieldIf', '条件 if'),
              value: '${h['if'] ?? ''}',
              hint: tOr(t, 'settings.hooks.ifPh', '匹配条件，如 Bash(rm *)'),
              onSubmitted: (v) =>
                  _patchHandler(eventId, gi, hi, {'if': v.isEmpty ? null : v}),
            ),
          NumberRow(
            label: tOr(t, 'settings.hooks.fieldTimeout', '超时'),
            value: (h['timeout'] as num?)?.toInt() ?? 0,
            onChanged: (v) =>
                _patchHandler(eventId, gi, hi, {'timeout': v == 0 ? null : v}),
            description: tOr(t, 'settings.hooks.seconds', '秒'),
          ),
          if (type == 'command')
            SwitchRow(
              label: 'async',
              description: tOr(t, 'settings.hooks.asyncDesc', '后台运行（不阻塞主流程）'),
              value: h['async'] == true,
              onChanged: (v) =>
                  _patchHandler(eventId, gi, hi, {'async': v ? true : null}),
            ),
          TextRow(
            label: tOr(t, 'settings.hooks.fieldStatus', '状态'),
            value: '${h['statusMessage'] ?? ''}',
            hint: tOr(t, 'settings.hooks.statusPh', '运行时显示的状态消息'),
            onSubmitted: (v) => _patchHandler(eventId, gi, hi, {
              'statusMessage': v.isEmpty ? null : v,
            }),
          ),
        ],
      ),
    );
  }
}
