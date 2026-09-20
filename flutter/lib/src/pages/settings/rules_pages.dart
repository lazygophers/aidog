/// 「规则」两页的 widget 层：
/// - `settings/scheduling` → `src/components/settings/SchedulingSettings.tsx`
/// - `settings/middleware`  → `src/components/settings/MiddlewareRules.tsx`
///
/// 校验（`clampNonNegativeInt`）、全量覆盖更新、silent 刷新全在
/// [SchedulingController] / [MiddlewareController]（票 I08 已测）。
///
/// 🔴 调度页「写失败不回滚本地值」是照搬 React 的（`scheduling_logic.dart:123`）。
///
/// **规则编辑表单与 React 的差异**：React 侧是卡片 / DSL 双模式编辑器（48 KB），
/// 这里是「条件 + 动作两个 JSON 编辑框」。功能上能建 / 能改 / 能删 / 能启停，
/// 但没有可视化条件树。列在 flutter/README.md 的未对齐清单里。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../../utils/formatters.dart';
import '../../shell/nav_guard.dart';
import '../../shell/theme.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'middleware_logic.dart';
import 'scheduling_logic.dart';

// ── 调度与熔断 ────────────────────────────────────────────

class SchedulingSettingsPage extends StatefulWidget {
  const SchedulingSettingsPage({super.key, this.invoke = kernelInvoke});

  final InvokeFn invoke;

  @override
  State<SchedulingSettingsPage> createState() =>
      _SchedulingSettingsPageState();
}

class _SchedulingSettingsPageState extends State<SchedulingSettingsPage> {
  late final SchedulingController _c;

  @override
  void initState() {
    super.initState();
    _c = SchedulingController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    unawaited(_c.load());
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    if (_c.loading) {
      return SettingsPageBody(
        title: t.t('appSettings.schedulingTab'),
        children: [CenteredNote(text: t.t('status.loading'))],
      );
    }
    final s = _c.settings;
    return SettingsPageBody(
      title: t.t('appSettings.schedulingTab'),
      children: [
        SettingsCard(
          children: [
            SwitchRow(
              key: const ValueKey('breaker-master'),
              label: t.t('scheduling.masterToggle'),
              description: t.t('scheduling.masterToggleDesc'),
              value: s.enabled,
              onChanged: (_) => _c.toggleEnabled(),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('scheduling.defaultRoutingMode'),
          description: t.t('scheduling.defaultRoutingModeDesc'),
          children: [
            ChoiceRow(
              key: const ValueKey('routing-mode'),
              label: t.t('scheduling.defaultRoutingMode'),
              options: kRoutingModes,
              value: s.defaultRoutingMode,
              labelOf: (m) {
                final e = kRoutingModeLabels[m];
                return e == null ? m : tOr(t, e.$1, e.$2);
              },
              onChanged: _c.setRoutingMode,
            ),
          ],
        ),
        SettingsCard(
          title: t.t('scheduling.breakerDefaults'),
          description: t.t('scheduling.breakerDefaultsDesc'),
          children: [
            NumberRow(
              key: const ValueKey('breaker-failure-threshold'),
              label: t.t('platform.breakerFailureThreshold'),
              value: s.breakerFailureThreshold,
              parse: clampNonNegativeInt,
              onChanged: (v) => _c.setFailureThreshold('$v'),
            ),
            NumberRow(
              key: const ValueKey('breaker-open-secs'),
              label: t.t('platform.breakerOpenSecs'),
              value: s.breakerOpenSecs,
              parse: clampNonNegativeInt,
              onChanged: (v) => _c.setOpenSecs('$v'),
            ),
            NumberRow(
              key: const ValueKey('breaker-half-open-max'),
              label: t.t('platform.breakerHalfOpenMax'),
              value: s.breakerHalfOpenMax,
              parse: clampNonNegativeInt,
              onChanged: (v) => _c.setHalfOpenMax('$v'),
            ),
          ],
        ),
        if (_c.error.isNotEmpty) ErrorNote(text: _c.error),
      ],
    );
  }
}

// ── 中间件规则 ────────────────────────────────────────────

class MiddlewareSettingsPage extends StatefulWidget {
  const MiddlewareSettingsPage({super.key, this.invoke = kernelInvoke});

  final InvokeFn invoke;

  @override
  State<MiddlewareSettingsPage> createState() =>
      _MiddlewareSettingsPageState();
}

class _MiddlewareSettingsPageState extends State<MiddlewareSettingsPage> {
  late final MiddlewareController _c;

  /// 待删除的规则 id（确认卡的数据源）。
  int? _deleteTarget;

  /// 表单草稿。表单没开时为 null。
  _RuleDraft? _draft;

  void Function()? _unregisterGuard;
  void Function()? _pendingNav;

  @override
  void initState() {
    super.initState();
    _c = MiddlewareController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    unawaited(_c.load());
    unawaited(_c.loadSettings());
    unawaited(_c.loadAppliesToOptions());
  }

  @override
  void dispose() {
    _unregisterGuard?.call();
    super.dispose();
  }

  /// 表单开着 = 有未保存编辑。React 侧没有这层守卫（它的表单是 modal），
  /// 这里加上是因为桌面壳里点侧栏会直接把整棵树拆掉、草稿静默丢失。
  void _syncGuard() {
    if (_draft != null) {
      _unregisterGuard ??= registerNavGuard((proceed) {
        setState(() => _pendingNav = proceed);
      });
    } else {
      _unregisterGuard?.call();
      _unregisterGuard = null;
    }
  }

  void _openForm(_RuleDraft d) {
    setState(() => _draft = d);
    _syncGuard();
  }

  void _closeForm() {
    setState(() => _draft = null);
    _c.closeForm();
    _syncGuard();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return SettingsPageBody(
      title: t.t('appSettings.middlewareTab'),
      subtitle: '${_c.rules.length}',
      trailing: SmallButton(
        key: const ValueKey('middleware-add'),
        label: t.t('middleware.addRule'),
        onTap: () {
          _c.openCreate();
          _openForm(_RuleDraft.empty());
        },
      ),
      children: [
        SettingsCard(
          children: [
            SwitchRow(
              key: const ValueKey('middleware-master'),
              label: t.t('middleware.masterToggle'),
              description: t.t('middleware.masterToggleDesc'),
              value: _c.settingsEnabled,
              onChanged: _c.setSettingsEnabled,
            ),
          ],
        ),
        SettingsCard(
          title: t.t('middleware.globalRules'),
          description: t.t('middleware.globalRulesHint'),
          children: [
            if (_c.loading)
              CenteredNote(text: t.t('status.loading'))
            else if (_c.rules.isEmpty)
              CenteredNote(text: t.t('middleware.noRules'))
            else
              for (final r in _c.rules) _ruleRow(t, r),
          ],
        ),
        if (_draft != null) _form(t, _draft!),
        if (_deleteTarget != null)
          ConfirmCard(
            title: t.t('action.delete'),
            body: t.t('middleware.name'),
            confirmLabel: t.t('action.delete'),
            onCancel: () => setState(() => _deleteTarget = null),
            onConfirm: () {
              final id = _deleteTarget!;
              setState(() => _deleteTarget = null);
              _c.deleteRule(id);
            },
          ),
        if (_pendingNav != null)
          UnsavedChangesCard(
            onSave: () async {
              final d = _draft;
              if (d == null || !d.valid) return;
              await _c.save(d.toInput());
              if (!mounted) return;
              final proceed = _pendingNav;
              setState(() {
                _draft = null;
                _pendingNav = null;
              });
              _syncGuard();
              proceed?.call();
            },
            onDiscard: () {
              final proceed = _pendingNav;
              setState(() {
                _draft = null;
                _pendingNav = null;
              });
              _syncGuard();
              proceed?.call();
            },
            onCancel: () => setState(() => _pendingNav = null),
          ),
        if (_c.error.isNotEmpty) ErrorNote(text: _c.error),
      ],
    );
  }

  Widget _ruleRow(I18nController t, MiddlewareRule r) {
    final theme = AidogTheme.of(context);
    final budget = _c.budgets[r.id];
    return Padding(
      key: ValueKey('rule-${r.id}'),
      padding: const EdgeInsets.symmetric(vertical: AidogSpace.sxs),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Row(
                  children: [
                    Flexible(
                      child: Text(
                        r.name,
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.label.copyWith(color: theme.c.fg),
                      ),
                    ),
                    if (r.isBuiltin)
                      Padding(
                        padding: const EdgeInsets.only(left: AidogSpace.sxs),
                        child: Text(
                          t.t('middleware.builtin'),
                          style: AidogType.micro.copyWith(color: theme.c.fg3),
                        ),
                      ),
                  ],
                ),
                if (r.description.isNotEmpty)
                  Text(
                    r.description,
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                if (budget != null)
                  Text(
                    '${t.t('middleware.budgetUsed')} '
                    '${formatCostUsd((budget['spent_usd'] as num?)?.toDouble() ?? 0)} · '
                    '${t.t('middleware.budgetRemaining')} '
                    '${formatCostUsd((budget['remaining_usd'] as num?)?.toDouble() ?? 0)}',
                    style: AidogType.micro.copyWith(
                      color:
                          ((budget['remaining_usd'] as num?)?.toDouble() ??
                                  0) <
                              0
                          ? theme.c.bad
                          : theme.c.fg3,
                    ),
                  ),
              ],
            ),
          ),
          SmallButton(
            label: r.enabled
                ? t.t('middleware.enabled')
                : t.t('settings.perm.disableAuto'),
            active: r.enabled,
            onTap: () => _c.toggleRule(r),
          ),
          const SizedBox(width: AidogSpace.sxs),
          SmallButton(
            // 内置规则只可启停，内容不可修改。
            label: r.isBuiltin
                ? t.t('middleware.viewRule')
                : t.t('action.edit'),
            onTap: r.isBuiltin
                ? null
                : () {
                    _c.openEdit(r);
                    _openForm(_RuleDraft.fromRule(r));
                  },
          ),
          const SizedBox(width: AidogSpace.sxs),
          SmallButton(
            label: t.t('action.delete'),
            danger: true,
            onTap: r.isBuiltin
                ? null
                : () => setState(() => _deleteTarget = r.id),
          ),
        ],
      ),
    );
  }

  Widget _form(I18nController t, _RuleDraft d) => SettingsCard(
    title: _c.editingRule != null
        ? t.t('middleware.editRule')
        : t.t('middleware.addRule'),
    children: [
      TextRow(
        key: const ValueKey('rule-name'),
        label: t.t('middleware.name'),
        value: d.name,
        onChanged: (v) => setState(() => d.name = v),
      ),
      TextRow(
        label: t.t('middleware.description'),
        value: d.description,
        onChanged: (v) => setState(() => d.description = v),
      ),
      NumberRow(
        label: t.t('middleware.priority'),
        value: d.priority,
        onChanged: (v) => setState(() => d.priority = v),
      ),
      SwitchRow(
        label: t.t('middleware.enabled'),
        value: d.enabled,
        onChanged: (v) => setState(() => d.enabled = v),
      ),
      TextRow(
        key: const ValueKey('rule-conditions'),
        label: t.t('middleware.conditions'),
        description: t.t('middleware.dslHint'),
        value: d.conditionsText,
        maxLines: 5,
        onChanged: (v) => setState(() => d.conditionsText = v),
      ),
      TextRow(
        key: const ValueKey('rule-actions'),
        label: t.t('middleware.actions'),
        value: d.actionsText,
        maxLines: 5,
        onChanged: (v) => setState(() => d.actionsText = v),
      ),
      TextRow(
        key: const ValueKey('rule-applies-to'),
        label: t.t('middleware.appliesTo'),
        description: [
          '${t.t('middleware.appliesPlatforms')}: '
              '${_c.platforms.map((p) => '${p.id}=${p.name}').join(', ')}',
          '${t.t('middleware.appliesGroups')}: '
              '${_c.groups.map((g) => '${g.groupKey}=${g.name}').join(', ')}',
        ].join('\n'),
        value: d.appliesToText,
        maxLines: 4,
        onChanged: (v) => setState(() => d.appliesToText = v),
      ),
      if (d.parseError(t) != null) ErrorNote(text: d.parseError(t)!),
      Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          SmallButton(label: t.t('action.cancel'), onTap: _closeForm),
          const SizedBox(width: AidogSpace.ssm),
          SmallButton(
            key: const ValueKey('rule-save'),
            label: t.t('action.save'),
            // 名字为空、或 JSON 解不开就点不动。
            onTap: d.valid
                ? () async {
                    await _c.save(d.toInput());
                    if (mounted) _closeForm();
                  }
                : null,
          ),
        ],
      ),
    ],
  );
}

/// 规则表单草稿。三个结构化字段以 JSON 文本承载（见文件头说明）。
class _RuleDraft {
  _RuleDraft({
    required this.name,
    required this.description,
    required this.priority,
    required this.enabled,
    required this.conditionsText,
    required this.actionsText,
    required this.appliesToText,
  });

  factory _RuleDraft.empty() => _RuleDraft(
    name: '',
    description: '',
    priority: 0,
    enabled: true,
    conditionsText: '{}',
    actionsText: '[]',
    appliesToText: '{}',
  );

  factory _RuleDraft.fromRule(MiddlewareRule r) => _RuleDraft(
    name: r.name,
    description: r.description,
    priority: r.priority,
    enabled: r.enabled,
    conditionsText: _pretty(r.raw['conditions'] ?? <String, Object?>{}),
    actionsText: _pretty(r.raw['actions'] ?? <Object?>[]),
    appliesToText: _pretty(r.raw['applies_to'] ?? <String, Object?>{}),
  );

  String name;
  String description;
  int priority;
  bool enabled;
  String conditionsText;
  String actionsText;
  String appliesToText;

  static String _pretty(Object? v) =>
      const JsonEncoder.withIndent('  ').convert(v);

  Object? _decode(String s) {
    try {
      return jsonDecode(s);
    } catch (_) {
      return null;
    }
  }

  /// 三个 JSON 框都解得开、名字非空才算可保存。
  bool get valid =>
      name.trim().isNotEmpty &&
      _decode(conditionsText) != null &&
      _decode(actionsText) != null &&
      _decode(appliesToText) != null;

  String? parseError(I18nController t) {
    if (_decode(conditionsText) == null) return t.t('middleware.conditions');
    if (_decode(actionsText) == null) return t.t('middleware.actions');
    if (_decode(appliesToText) == null) return t.t('middleware.appliesTo');
    return null;
  }

  Map<String, Object?> toInput() => {
    'name': name.trim(),
    'description': description,
    'conditions': _decode(conditionsText),
    'actions': _decode(actionsText),
    'applies_to': _decode(appliesToText),
    'priority': priority,
    'enabled': enabled,
    'is_builtin': false,
  };
}
