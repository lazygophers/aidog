/// 「规则」两页的 widget 层：
/// - `settings/scheduling` → `src/components/settings/SchedulingSettings.tsx`
/// - `settings/middleware`  → `src/components/settings/MiddlewareRules.tsx`
///
/// 校验（`clampNonNegativeInt`）、全量覆盖更新、silent 刷新全在
/// [SchedulingController] / [MiddlewareController]（票 I08 已测）。
///
/// 🔴 调度页「写失败不回滚本地值」是照搬 React 的（`scheduling_logic.dart:123`）。
///
/// 中间件规则表单（I17）对齐 React 的卡片 ↔ DSL 双模式：条件树 / 动作链 /
/// 应用范围走 `middleware_editor.dart` 的表单编辑器，DSL 源码与裸 JSON 保留为
/// 高级模式（React 只有前两种，JSON 是 Flutter 侧多给的一层回退）。
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
import 'middleware_dsl.dart';
import 'middleware_editor.dart';
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

  /// 条件区模式：cards（默认）/ dsl / json；动作区：cards / json。
  /// DSL 与 JSON 只是同一份结构化草稿的文本视图，切换时互转。
  String _condMode = 'cards';
  String _dslText = '';
  String? _dslError;
  String _condJsonText = '';
  String _actionsMode = 'cards';
  String _actionsJsonText = '';

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
    setState(() {
      _draft = d;
      _condMode = 'cards';
      _actionsMode = 'cards';
      _dslError = null;
    });
    _syncGuard();
  }

  void _closeForm() {
    setState(() => _draft = null);
    _c.closeForm();
    _syncGuard();
  }

  /// DSL 文本解析；失败返回错误文案，成功返回 null。
  String? _tryParseDsl(String src) {
    try {
      parseDsl(src);
      return null;
    } on DslException catch (e) {
      return '$e';
    }
  }

  /// 切条件区模式：离开文本模式时把文本解析回结构化草稿（解析失败留在原模式）。
  void _switchCondMode(String target) {
    final d = _draft;
    if (d == null) return;
    if (target == _condMode) return;
    if (_condMode == 'dsl') {
      final err = _tryParseDsl(_dslText);
      if (err != null) return; // React：切回卡片按钮在 DSL 报错时禁用
      d.conditions = parseDsl(_dslText);
    } else if (_condMode == 'json') {
      try {
        final v = jsonDecode(_condJsonText);
        if (v is! Map) return;
        d.conditions = Map<String, Object?>.from(v);
      } catch (_) {
        return;
      }
    }
    setState(() {
      _condMode = target;
      if (target == 'dsl') {
        _dslText = treeToDsl(d.conditions);
        _dslError = null;
      } else if (target == 'json') {
        _condJsonText = _pretty(d.conditions);
      }
    });
  }

  void _switchActionsMode(String target) {
    final d = _draft;
    if (d == null || target == _actionsMode) return;
    if (_actionsMode == 'json') {
      try {
        final v = jsonDecode(_actionsJsonText);
        if (v is! List) return;
        d.actions = [
          for (final a in v)
            if (a is Map) Map<String, Object?>.from(a),
        ];
      } catch (_) {
        return;
      }
    }
    setState(() {
      _actionsMode = target;
      if (target == 'json') _actionsJsonText = _pretty(d.actions);
    });
  }

  static String _pretty(Object? v) =>
      const JsonEncoder.withIndent('  ').convert(v);

  static const _tJsonErr = 'JSON 解析失败';

  /// 保存前把文本模式下的内容解析回草稿（React `handleSave` 同兜底）。
  /// 返回 null = 可保存；否则是错误文案。
  String? _syncDraftFromText() {
    final d = _draft;
    if (d == null) return 'no draft';
    if (_condMode == 'dsl') {
      final err = _tryParseDsl(_dslText);
      if (err != null) return err;
      d.conditions = parseDsl(_dslText);
    } else if (_condMode == 'json') {
      try {
        final v = jsonDecode(_condJsonText);
        if (v is! Map) return _tJsonErr;
        d.conditions = Map<String, Object?>.from(v);
      } catch (_) {
        return _tJsonErr;
      }
    }
    if (_actionsMode == 'json') {
      try {
        final v = jsonDecode(_actionsJsonText);
        if (v is! List) return _tJsonErr;
        d.actions = [
          for (final a in v)
            if (a is Map) Map<String, Object?>.from(a),
        ];
      } catch (_) {
        return _tJsonErr;
      }
    }
    return null;
  }

  bool _draftValid() {
    final d = _draft;
    if (d == null || !d.valid) return false;
    if (_condMode == 'dsl' && _dslError != null) return false;
    return _syncDraftFromText() == null;
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
              if (d == null || !_draftValid()) return;
              _syncDraftFromText();
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
                // 条件 / 动作 / 应用范围摘要（React RuleRow 的徽标行）。
                Text(
                  conditionsSummary(
                    r.raw['conditions'] is Map
                        ? Map<String, Object?>.from(r.raw['conditions'] as Map)
                        : emptyLeaf,
                  ),
                  style: AidogType.micro.copyWith(color: theme.c.fg3),
                ),
                Text(
                  actionsSummary(
                    t,
                    r.raw['actions'] as List? ?? const [],
                  ),
                  style: AidogType.micro.copyWith(color: theme.c.accent),
                ),
                if (hasObserveAction(r.raw['actions'] as List? ?? const []))
                  Text(
                    '${tOr(t, 'middleware.observe', '观察模式')} · '
                    '${appliesSummary(r.raw['applies_to'] is Map ? Map<String, Object?>.from(r.raw['applies_to'] as Map) : null)}',
                    style: AidogType.micro.copyWith(color: theme.c.peak),
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
      // ── 条件：卡片 / DSL / JSON 三模式 ──
      Row(
        children: [
          Text(
            t.t('middleware.conditions'),
            style: AidogType.label.copyWith(
              color: AidogTheme.of(context).c.fg,
            ),
          ),
          const Spacer(),
          SmallButton(
            key: const ValueKey('cond-mode-cards'),
            label: tOr(t, 'middleware.toCards', '卡片模式'),
            active: _condMode == 'cards',
            onTap: () => _switchCondMode('cards'),
          ),
          const SizedBox(width: AidogSpace.sxs),
          SmallButton(
            key: const ValueKey('cond-mode-dsl'),
            label: tOr(t, 'middleware.toDsl', 'DSL 源码'),
            active: _condMode == 'dsl',
            onTap: () => _switchCondMode('dsl'),
          ),
          const SizedBox(width: AidogSpace.sxs),
          SmallButton(
            key: const ValueKey('cond-mode-json'),
            label: t.t('settings.jsonMode'),
            active: _condMode == 'json',
            onTap: () => _switchCondMode('json'),
          ),
        ],
      ),
      if (_condMode == 'cards')
        ConditionTreeEditor(
          node: _draft!.conditions,
          onChanged: (n) => setState(() => _draft!.conditions = n),
          onRemove: () => setState(() => _draft!.conditions = emptyLeaf),
          removeLabel: tOr(t, 'middleware.clearConditions', '清空条件'),
        )
      else if (_condMode == 'dsl') ...[
        TextRow(
          key: const ValueKey('rule-conditions-dsl'),
          label: t.t('middleware.conditions'),
          description: t.t('middleware.dslHint'),
          value: _dslText,
          maxLines: 6,
          onChanged: (v) {
            setState(() {
              _dslText = v;
              _dslError = _tryParseDsl(v);
            });
          },
        ),
        if (_dslError != null) ErrorNote(text: _dslError!),
      ]
      else
        TextRow(
          key: const ValueKey('rule-conditions'),
          label: t.t('middleware.conditions'),
          value: _condJsonText,
          maxLines: 6,
          onChanged: (v) => setState(() => _condJsonText = v),
        ),
      if (d.phaseError != null) ErrorNote(text: d.phaseError!),
      // ── 动作链：卡片 / JSON ──
      Row(
        children: [
          Expanded(
            child: Text(
              t.t('middleware.actions'),
              style: AidogType.label.copyWith(
                color: AidogTheme.of(context).c.fg,
              ),
            ),
          ),
          SmallButton(
            key: const ValueKey('actions-mode-cards'),
            label: tOr(t, 'middleware.toCards', '卡片模式'),
            active: _actionsMode == 'cards',
            onTap: () => _switchActionsMode('cards'),
          ),
          const SizedBox(width: AidogSpace.sxs),
          SmallButton(
            key: const ValueKey('actions-mode-json'),
            label: t.t('settings.jsonMode'),
            active: _actionsMode == 'json',
            onTap: () => _switchActionsMode('json'),
          ),
        ],
      ),
      if (_actionsMode == 'cards')
        ActionChainEditor(
          steps: _draft!.actions,
          onChanged: (s) => setState(() => _draft!.actions = s),
        )
      else
        TextRow(
          key: const ValueKey('rule-actions'),
          label: t.t('middleware.actions'),
          value: _actionsJsonText,
          maxLines: 6,
          onChanged: (v) => setState(() => _actionsJsonText = v),
        ),
      // ── 应用范围 ──
      Text(
        t.t('middleware.appliesTo'),
        style: AidogType.label.copyWith(color: AidogTheme.of(context).c.fg),
      ),
      AppliesToEditor(
        value: _draft!.applies,
        onChanged: (a) => setState(() => _draft!.applies = a),
        platforms: _c.platforms,
        groups: _c.groups,
      ),
      Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          SmallButton(label: t.t('action.cancel'), onTap: _closeForm),
          const SizedBox(width: AidogSpace.ssm),
          SmallButton(
            key: const ValueKey('rule-save'),
            label: t.t('action.save'),
            // 名字为空、条件 / 动作在当前模式下解析不了、或混阶段，就点不动。
            onTap: _draftValid()
                ? () async {
                    _syncDraftFromText();
                    await _c.save(_draft!.toInput());
                    if (mounted) _closeForm();
                  }
                : null,
          ),
        ],
      ),
    ],
  );
}

/// 规则表单草稿。conditions / actions / applies_to 是结构化对象（表单直接改），
/// DSL / JSON 只是同一份数据的文本视图（切换模式时互转）。
class _RuleDraft {
  _RuleDraft({
    required this.name,
    required this.description,
    required this.priority,
    required this.enabled,
    required this.conditions,
    required this.actions,
    required this.applies,
  });

  factory _RuleDraft.empty() => _RuleDraft(
    name: '',
    description: '',
    priority: 0,
    enabled: true,
    conditions: emptyLeaf,
    actions: [
      {
        'kind': 'mask',
        'params': {...defaultActionParams(), 'replacement': '****'},
      },
    ],
    applies: {'platforms': [], 'groups': [], 'models': []},
  );

  factory _RuleDraft.fromRule(MiddlewareRule r) {
    Map<String, Object?> asMap(Object? v) =>
        v is Map ? Map<String, Object?>.from(v) : emptyLeaf;
    return _RuleDraft(
      name: r.name,
      description: r.description,
      priority: r.priority,
      enabled: r.enabled,
      conditions: asMap(r.raw['conditions']),
      actions: [
        for (final a in (r.raw['actions'] as List? ?? const []))
          if (a is Map) Map<String, Object?>.from(a),
      ],
      applies: r.raw['applies_to'] is Map
          ? Map<String, Object?>.from(r.raw['applies_to'] as Map)
          : {'platforms': [], 'groups': [], 'models': []},
    );
  }

  String name;
  String description;
  int priority;
  bool enabled;

  /// 条件树（serde 形状，见 `middleware_dsl.dart` 的文件头）。
  Map<String, Object?> conditions;

  /// 动作链（有序）。
  List<Map<String, Object?>> actions;

  /// `{platforms, groups, models}`。
  Map<String, Object?> applies;

  /// 混阶段检查（与 Rust validate_rule_phases 对称，保存前提前提示）。
  String? get phaseError => mixedPhase(conditions);

  /// 名字非空、叶子不混阶段才算可保存。
  bool get valid => name.trim().isNotEmpty && phaseError == null;

  Map<String, Object?> toInput() => {
    'name': name.trim(),
    'description': description,
    'conditions': conditions,
    'actions': actions,
    'applies_to': applies,
    'priority': priority,
    'enabled': enabled,
    'is_builtin': false,
  };
}
