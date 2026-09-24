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
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../platform_card_bits.dart' show MiniBadge;
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
  State<SchedulingSettingsPage> createState() => _SchedulingSettingsPageState();
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
        ToggleCard(
          key: const ValueKey('breaker-master'),
          label: t.t('scheduling.masterToggle'),
          descriptions: [t.t('scheduling.masterToggleDesc')],
          value: s.enabled,
          onChanged: (_) => _c.toggleEnabled(),
        ),
        HeaderCard(
          title: t.t('scheduling.defaultRoutingMode'),
          descriptions: [t.t('scheduling.defaultRoutingModeDesc')],
          child: Padding(
            padding: const EdgeInsets.only(top: AidogSpace.smd),
            child: Align(
              alignment: AlignmentDirectional.centerStart,
              // React Select maxWidth 240（`SchedulingSettings.tsx:124-136`）。
              child: InlineSelect<String>(
                key: const ValueKey('routing-mode'),
                value: s.defaultRoutingMode,
                options: kRoutingModes,
                width: 240,
                labelOf: (m) {
                  final e = kRoutingModeLabels[m];
                  return e == null ? m : tOr(t, e.$1, e.$2);
                },
                onChanged: (v) => _c.setRoutingMode(v!),
              ),
            ),
          ),
        ),
        // 总开关关掉后整块压暗（`SchedulingSettings.tsx:142`）。
        HeaderCard(
          dimmed: !s.enabled,
          title: t.t('scheduling.breakerDefaults'),
          descriptions: [t.t('scheduling.breakerDefaultsDesc')],
          child: Padding(
            padding: const EdgeInsets.only(top: AidogSpace.smd),
            // React 是 2 列 grid（auto 1fr、行列距 10/12）；这里 Wrap 两枚一组
            // 换行，等价排布。
            child: Wrap(
              spacing: 12,
              runSpacing: AidogSpace.smd,
              children: [
                InlineRow(
                  label: t.t('platform.breakerFailureThreshold'),
                  child: NumberInput(
                    key: const ValueKey('breaker-failure-threshold'),
                    value: '${s.breakerFailureThreshold}',
                    width: 140,
                    onChanged: (v) => _c.setFailureThreshold(v),
                  ),
                ),
                InlineRow(
                  label: t.t('platform.breakerOpenSecs'),
                  child: NumberInput(
                    key: const ValueKey('breaker-open-secs'),
                    value: '${s.breakerOpenSecs}',
                    width: 140,
                    onChanged: (v) => _c.setOpenSecs(v),
                  ),
                ),
                InlineRow(
                  label: t.t('platform.breakerHalfOpenMax'),
                  child: NumberInput(
                    key: const ValueKey('breaker-half-open-max'),
                    value: '${s.breakerHalfOpenMax}',
                    width: 140,
                    onChanged: (v) => _c.setHalfOpenMax(v),
                  ),
                ),
              ],
            ),
          ),
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
  State<MiddlewareSettingsPage> createState() => _MiddlewareSettingsPageState();
}

class _MiddlewareSettingsPageState extends State<MiddlewareSettingsPage> {
  late final MiddlewareController _c;

  /// 待删除的规则 id（确认卡的数据源）。
  int? _deleteTarget;

  /// 表单草稿。表单没开时为 null。
  _RuleDraft? _draft;

  /// 只读态（内置规则）：表单照常渲染，但控件点不动、不给保存。
  bool _readOnly = false;

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

  /// [readOnly] = 内置规则：表单照开，但所有控件点不动，顶上说明为什么
  ///（`MiddlewareRules.tsx:721-727`）。原先内置规则的「查看规则」直接禁用，
  /// 它的条件和动作在界面上根本打不开看。
  void _openForm(_RuleDraft d, {bool readOnly = false}) {
    setState(() {
      _readOnly = readOnly;
      _draft = d;
      _condMode = 'cards';
      _actionsMode = 'cards';
      _dslError = null;
    });
    _syncGuard();
  }

  void _closeForm() {
    setState(() {
      _draft = null;
      _readOnly = false;
    });
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
      children: [
        ToggleCard(
          key: const ValueKey('middleware-master'),
          label: t.t('middleware.masterToggle'),
          descriptions: [t.t('middleware.masterToggleDesc')],
          value: _c.settingsEnabled,
          onChanged: _c.setSettingsEnabled,
        ),
        Padding(
          padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
          child: Tile(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
            child: Opacity(
              // 同上（`MiddlewareRules.tsx:1246`）。
              opacity: _c.settingsEnabled ? 1 : 0.55,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    t.t('middleware.globalRulesHint'),
                    style: AidogType.label.copyWith(
                      fontSize: 13,
                      color: AidogTheme.of(context).c.fg3,
                    ),
                  ),
                  const SizedBox(height: AidogSpace.ssm),
                  if (_c.loading)
                    Padding(
                      padding: const EdgeInsets.all(8),
                      child: Text(
                        t.t('status.loading'),
                        style: AidogType.label.copyWith(
                          fontSize: 13,
                          color: AidogTheme.of(context).c.fg2,
                        ),
                      ),
                    )
                  else if (_c.rules.isEmpty)
                    Padding(
                      padding: const EdgeInsets.all(8),
                      child: Text(
                        t.t('middleware.noRules'),
                        style: AidogType.label.copyWith(
                          fontSize: 13,
                          color: AidogTheme.of(context).c.fg3,
                        ),
                      ),
                    )
                  else
                    for (final r in _c.rules) _ruleRow(t, r),
                  // 新增入口在列表底部（React「+ 新增规则」ghost 按钮，
                  // `MiddlewareRules.tsx:1151-1155`），不在页头。
                  const SizedBox(height: AidogSpace.ssm),
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: SmallButton(
                      key: const ValueKey('middleware-add'),
                      ghost: true,
                      label: '+ ${t.t('middleware.addRule')}',
                      onTap: () {
                        _c.openCreate();
                        _openForm(_RuleDraft.empty());
                      },
                    ),
                  ),
                ],
              ),
            ),
          ),
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
    final actions = r.raw['actions'] as List? ?? const [];
    final applies = r.raw['applies_to'] is Map
        ? Map<String, Object?>.from(r.raw['applies_to'] as Map)
        : null;
    return Padding(
      key: ValueKey('rule-${r.id}'),
      padding: const EdgeInsets.only(bottom: 6),
      // 停用的规则整行弱化（`MiddlewareRules.tsx:1007-1011` 的 opacity 0.55）：
      // 原先停用与启用长得一模一样，一屏规则里分不出哪几条其实没在跑。
      child: Opacity(
        opacity: r.enabled ? 1 : 0.55,
        child: Container(
          // 玻璃盒行：r-sm、padding 10/14、1px 边框（failed 红）
          //（`MiddlewareRules.tsx:905-914`）。
          padding: const EdgeInsets.symmetric(
            horizontal: 14,
            vertical: 10,
          ),
          decoration: BoxDecoration(
            color: theme.c.surface2,
            border: Border.all(color: r.failed ? theme.c.bad : theme.c.line),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    // 名称 + 徽标行（10px 徽标，`MiddlewareRules.tsx:921-966`）。
                    Wrap(
                      spacing: AidogSpace.ssm,
                      runSpacing: 4,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
                        Text(
                          r.name,
                          style: AidogType.label.copyWith(
                            fontSize: 13,
                            fontWeight: FontWeight.w600,
                            color: theme.c.fg,
                          ),
                        ),
                        if (r.isBuiltin)
                          _ruleBadge(
                            theme,
                            text: t.t('middleware.builtin'),
                            fg: theme.c.accentText,
                            bg: theme.c.accentWash,
                          ),
                        // 失效规则要一眼看得出来（`MiddlewareRules.tsx:930-933`）：
                        // 引擎跳过它，用户该做的是删掉重建，不是继续改。
                        if (r.failed)
                          _ruleBadge(
                            theme,
                            text: t.t('middleware.failed'),
                            fg: theme.c.bad,
                            bg: theme.c.bad.withValues(alpha: 0.12),
                          ),
                        _ruleBadge(
                          theme,
                          text: actionsSummary(t, actions),
                          fg: theme.c.fg2,
                          bg: theme.c.fg3.withValues(alpha: 0.12),
                        ),
                        if (hasObserveAction(actions))
                          Tooltip(
                            message: tOr(
                              t,
                              'middleware.observeHint',
                              '开启后命中不拦截：请求照常转发并计费，只在日志里记一笔，用来验证规则是否误伤',
                            ),
                            child: _ruleBadge(
                              theme,
                              text: tOr(t, 'middleware.observe', '观察模式'),
                              fg: theme.c.peak,
                              bg: theme.c.peak.withValues(alpha: 0.12),
                            ),
                          ),
                        if (appliesSummary(applies).isNotEmpty)
                          _ruleBadge(
                            theme,
                            text: appliesSummary(applies),
                            fg: theme.c.fg2,
                            bg: theme.c.fg3.withValues(alpha: 0.12),
                          ),
                      ],
                    ),
                    // 失效规则不显摘要（`MiddlewareRules.tsx:950`）：那份条件引擎
                    // 已经翻译不了，照着念只会误导。
                    if (!r.failed)
                      Padding(
                        padding: const EdgeInsets.only(top: 3),
                        child: Text(
                          conditionsSummary(
                            r.raw['conditions'] is Map
                                ? Map<String, Object?>.from(
                                    r.raw['conditions'] as Map,
                                  )
                                : emptyLeaf,
                          ),
                          style: AidogType.numSm.copyWith(
                            fontSize: 11,
                            color: theme.c.fg3,
                          ),
                        ),
                      ),
                    if (budget != null) _BudgetLine(budget: budget),
                  ],
                ),
              ),
              // 开关而不是按钮（`MiddlewareRules.tsx:917`）。
              AidogSwitch(
                compact: true,
                value: r.enabled,
                onChanged: () => _c.toggleRule(r),
              ),
              const SizedBox(width: AidogSpace.sxs),
              // 内置规则可点开查看详情（表单只读）；Failed 规则（含内置残留）
              // 只可删除（`MiddlewareRules.tsx:1013-1028`）。
              if (!r.failed)
                IconGhostButton(
                  key: ValueKey('rule-edit-${r.id}'),
                  icon: Icons.edit_outlined,
                  tooltip: t.t('action.edit'),
                  onTap: () {
                    // 内置规则只读打开：看得到条件 / 动作，改不动
                    //（React 同一颗按钮走的也是只读表单）。
                    if (!r.isBuiltin) _c.openEdit(r);
                    _openForm(_RuleDraft.fromRule(r), readOnly: r.isBuiltin);
                  },
                ),
              if (!r.isBuiltin || r.failed)
                IconGhostButton(
                  key: ValueKey('rule-del-${r.id}'),
                  icon: Icons.close,
                  tooltip: t.t('action.delete'),
                  onTap: () => setState(() => _deleteTarget = r.id),
                ),
            ],
          ),
        ),
      ),
    );
  }

  /// 10px 徽标（React `badge`，`MiddlewareRules.tsx:925-966`）。
  Widget _ruleBadge(
    AidogTheme theme, {
    required String text,
    required Color fg,
    required Color bg,
  }) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
    decoration: BoxDecoration(
      color: bg,
      borderRadius: BorderRadius.circular(4),
    ),
    child: Text(
      text,
      style: AidogType.caption.copyWith(fontSize: 10, color: fg),
    ),
  );

  /// 表单弹窗：对齐 React 的 `RuleFormDialog`（`MiddlewareRules.tsx:861-893`：
  /// 720 宽、内滚的 modal），不再是页内卡。只读时控件罩住、底部给「关闭」。
  Widget _form(I18nController t, _RuleDraft d) => AidogModal(
    key: ValueKey('rule-form-${_c.editingRule?['id'] ?? 'new'}'),
    maxWidth: 720,
    child: ModalCard(
      title: _readOnly
          ? t.t('middleware.viewRule')
          : _c.editingRule != null
          ? t.t('middleware.editRule')
          : t.t('middleware.addRule'),
      padding: const EdgeInsets.all(20),
      child: _readOnly
          ? Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                // 只读时先说清为什么点不动（`MiddlewareRules.tsx:721-727`）。
                Text(
                  tOr(
                    t,
                    'middleware.builtinReadonlyHint',
                    '内置规则只可启停，内容不可修改',
                  ),
                  style: AidogType.caption.copyWith(
                    fontSize: 11,
                    color: AidogTheme.of(context).c.fg3,
                  ),
                ),
                const SizedBox(height: AidogSpace.ssm),
                // 文本仍可选中复制，只是所有控件点不动（React 用
                // `pointerEvents: none` 达到同样效果）。
                IgnorePointer(child: _formFields(t, d)),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    SmallButton(
                      key: const ValueKey('rule-readonly-close'),
                      label: t.t('action.close'),
                      onTap: _closeForm,
                    ),
                  ],
                ),
              ],
            )
          : _formFields(t, d),
    ),
  );

  Widget _formFields(I18nController t, _RuleDraft d) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      // React：名称 / 描述是全宽 Input（placeholder 形态，无标签，
      // `MiddlewareRules.tsx:731-741`）。
      PlainTextField(
        key: const ValueKey('rule-name'),
        value: d.name,
        hint: t.t('middleware.name'),
        onChanged: (v) => setState(() => d.name = v),
      ),
      const SizedBox(height: AidogSpace.smd),
      PlainTextField(
        value: d.description,
        hint: tOr(t, 'middleware.description', '描述（可选）'),
        onChanged: (v) => setState(() => d.description = v),
      ),
      const SizedBox(height: AidogSpace.smd),
      Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Expanded(
            child: Text(
              t.t('middleware.conditions'),
              style: AidogType.label.copyWith(
                fontSize: 13,
                color: AidogTheme.of(context).c.fg2,
              ),
            ),
          ),
          SmallButton(
            key: const ValueKey('cond-mode-cards'),
            ghost: true,
            fontSize: 11,
            label: tOr(t, 'middleware.toCards', '卡片模式'),
            active: _condMode == 'cards',
            onTap: () => _switchCondMode('cards'),
          ),
          const SizedBox(width: AidogSpace.sxs),
          SmallButton(
            key: const ValueKey('cond-mode-dsl'),
            ghost: true,
            fontSize: 11,
            label: tOr(t, 'middleware.toDsl', 'DSL 源码'),
            active: _condMode == 'dsl',
            onTap: () => _switchCondMode('dsl'),
          ),
          const SizedBox(width: AidogSpace.sxs),
          SmallButton(
            key: const ValueKey('cond-mode-json'),
            ghost: true,
            fontSize: 11,
            label: t.t('settings.jsonMode'),
            active: _condMode == 'json',
            onTap: () => _switchCondMode('json'),
          ),
        ],
      ),
      const SizedBox(height: AidogSpace.ssm),
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
      ] else
        TextRow(
          key: const ValueKey('rule-conditions'),
          label: t.t('middleware.conditions'),
          value: _condJsonText,
          maxLines: 6,
          onChanged: (v) => setState(() => _condJsonText = v),
        ),
      if (d.phaseError != null) ErrorNote(text: d.phaseError!),
      const SizedBox(height: AidogSpace.smd),
      Row(
        children: [
          Expanded(
            child: Text(
              t.t('middleware.actions'),
              style: AidogType.label.copyWith(
                fontSize: 13,
                color: AidogTheme.of(context).c.fg2,
              ),
            ),
          ),
          SmallButton(
            key: const ValueKey('actions-mode-cards'),
            ghost: true,
            fontSize: 11,
            label: tOr(t, 'middleware.toCards', '卡片模式'),
            active: _actionsMode == 'cards',
            onTap: () => _switchActionsMode('cards'),
          ),
          const SizedBox(width: AidogSpace.sxs),
          SmallButton(
            key: const ValueKey('actions-mode-json'),
            ghost: true,
            fontSize: 11,
            label: t.t('settings.jsonMode'),
            active: _actionsMode == 'json',
            onTap: () => _switchActionsMode('json'),
          ),
        ],
      ),
      const SizedBox(height: AidogSpace.ssm),
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
      const SizedBox(height: AidogSpace.smd),
      Text(
        t.t('middleware.appliesTo'),
        style: AidogType.label.copyWith(
          fontSize: 13,
          color: AidogTheme.of(context).c.fg2,
        ),
      ),
      const SizedBox(height: AidogSpace.ssm),
      AppliesToEditor(
        value: _draft!.applies,
        onChanged: (a) => setState(() => _draft!.applies = a),
        platforms: _c.platforms,
        groups: _c.groups,
      ),
      const SizedBox(height: AidogSpace.smd),
      // 优先级：label 13 + 宽 120 的数字框（`MiddlewareRules.tsx:828-840`）。
      InlineRow(
        label: t.t('middleware.priority'),
        child: NumberInput(
          value: '${d.priority}',
          width: 120,
          onChanged: (v) =>
              setState(() => d.priority = int.tryParse(v) ?? 0),
        ),
      ),
      const SizedBox(height: AidogSpace.smd),
      SwitchRow(
        label: t.t('middleware.enabled'),
        value: d.enabled,
        onChanged: (v) => setState(() => d.enabled = v),
      ),
      if (!_readOnly) ...[
        const SizedBox(height: AidogSpace.ssm),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            SmallButton(label: t.t('action.cancel'), onTap: _closeForm),
            const SizedBox(width: AidogSpace.ssm),
            SmallButton(
              key: const ValueKey('rule-save'),
              filled: true,
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

/// 预算闸门的当前窗口状态（`MiddlewareRules.tsx:966-1005`）：
/// 进度条 + 「本月已用 X / 上限 Y」+ 超限时换成「已超预算，请求被拒绝」。
///
/// 原先只画「已用 X · 剩余 Y」，`budget_usd` 这个字段**后端一直在发、这边从没取用**
/// （`generated/MiddlewareBudgetStatus.ts:7-11`）。后果是看不到上限是多少，
/// 超限时也只是数字变红 —— 看不出请求已经被拦下了。
class _BudgetLine extends StatelessWidget {
  const _BudgetLine({required this.budget});

  final Map<String, Object?> budget;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final limit = (budget['budget_usd'] as num?)?.toDouble() ?? 0;
    final spent = (budget['spent_usd'] as num?)?.toDouble() ?? 0;
    final remaining = (budget['remaining_usd'] as num?)?.toDouble() ?? 0;
    final over = remaining <= 0;
    // 上限为 0（没配预算）时不画进度条，否则除零。
    final ratio = limit <= 0 ? 0.0 : (spent / limit).clamp(0.0, 1.0);
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.sxs),
      child: Wrap(
        spacing: AidogSpace.ssm,
        runSpacing: AidogSpace.sxs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          if (limit > 0)
            SizedBox(
              width: 140,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(2),
                child: LinearProgressIndicator(
                  value: ratio,
                  minHeight: 4,
                  backgroundColor: theme.c.line,
                  valueColor: AlwaysStoppedAnimation<Color>(
                    over ? theme.c.bad : theme.c.accentText,
                  ),
                ),
              ),
            ),
          Text(
            '${t.t('middleware.budgetUsed')} ${formatCostUsd(spent)}'
            '${limit > 0 ? ' / ${formatCostUsd(limit)}' : ''}',
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          Text(
            over
                ? t.t('middleware.budgetExceeded')
                : '${t.t('middleware.budgetRemaining')} ${formatCostUsd(remaining)}',
            style: AidogType.micro.copyWith(
              color: over ? theme.c.bad : theme.c.fg3,
            ),
          ),
        ],
      ),
    );
  }
}
