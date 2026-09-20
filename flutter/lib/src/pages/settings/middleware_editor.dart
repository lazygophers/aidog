/// 中间件规则的可视化编辑器（I17）—— 对齐 React
/// `src/components/settings/MiddlewareRules.tsx` 的三块编辑器：
/// 条件树（递归组卡片）、动作链（有序表单）、应用范围（三维多选）。
/// DSL 源码模式与 JSON 高级模式在 `rules_pages.dart` 里挂接（本文件只做卡片态）。
///
/// 树 / 动作的数据形状与后端 serde 一致（Map / List），不建第二套模型。
library;

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../shell/theme.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'middleware_dsl.dart';

const List<String> kMwTargets = [
  'request_body', 'request_headers', 'response_body', 'response_headers',
  'status', 'model',
];
const List<String> kMwMatchTypes = ['contains', 'regex', 'exact'];
const List<String> kMwActionKinds = [
  'mask', 'block', 'warn', 'inject', 'override', 'classify', 'budget_gate',
];
/// mask 的 fields 是闭集：Rust 侧 inbound.rs 只认 messages / system。
const List<String> kMwMaskFields = ['messages', 'system'];
/// 终结性动作（block/classify/budget_gate 之后停止执行）。
const Set<String> kMwTerminalKinds = {'block', 'classify', 'budget_gate'};

Map<String, Object?> get emptyLeaf => {
      'kind': 'leaf',
      'target': 'request_body',
      'field': '',
      'match_type': 'contains',
      'pattern': '',
      'validator': '',
    };

/// ActionParams 前端默认值（与 Rust serde default 对齐）。
Map<String, Object?> defaultActionParams() => {
      'replacement': '****',
      'fields': <String>[],
      'inject_mode': '',
      'target': '',
      'value': '',
      'category': '',
      'retryable': true,
      'override_status': null,
      'override_body': null,
      'observe': false,
      'budget_usd': 0,
    };

/// 叶子的 field 输入只在四个 body/header 侧 target 有意义。
bool _leafHasField(String target) => target.contains('body') ||
    target.contains('headers');

// ── 条件树编辑器 ────────────────────────────────────────

class ConditionTreeEditor extends StatelessWidget {
  const ConditionTreeEditor({
    super.key,
    required this.node,
    required this.onChanged,
    this.onRemove,
    this.removeLabel,
    this.depth = 0,
  });

  final Map<String, Object?> node;
  final ValueChanged<Map<String, Object?>> onChanged;

  /// 删除本节点；顶层传「清空条件」（重置为空叶子），文案由 [removeLabel] 决定。
  final VoidCallback? onRemove;
  final String? removeLabel;
  final int depth;

  @override
  Widget build(BuildContext context) {
    if ('${node['kind']}' == 'leaf') return _LeafEditor(node: node, onChanged: onChanged, onRemove: onRemove, removeLabel: removeLabel);
    final theme = AidogTheme.of(context);
    final t = AidogI18n.of(context);
    final isNot = '${node['kind']}' == 'not';
    final children = isNot
        ? [Map<String, Object?>.from(node['child'] as Map)]
        : [
            for (final c in (node['children'] as List? ?? const []))
              Map<String, Object?>.from(c as Map),
          ];
    void write(List<Map<String, Object?>> cs) => onChanged(isNot
        ? {'kind': 'not', 'child': cs.isEmpty ? emptyLeaf : cs[0]}
        : {...node, 'children': cs});
    return Container(
      margin: EdgeInsets.only(left: depth * 12.0),
      padding: const EdgeInsets.all(AidogSpace.ssm),
      decoration: BoxDecoration(
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        color: theme.c.surface2,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              _smallDropdown(
                context,
                value: '${node['kind']}',
                options: const ['all', 'any', 'not'],
                labelOf: (v) => switch (v) {
                  'all' => tOr(t, 'middleware.node.all', 'AND (全部满足)'),
                  'any' => tOr(t, 'middleware.node.any', 'OR (任一满足)'),
                  _ => tOr(t, 'middleware.node.not', 'NOT (取反)'),
                },
                onChanged: (v) => onChanged(v == 'not'
                    ? {
                        'kind': 'not',
                        'child': children.isEmpty ? emptyLeaf : children[0],
                      }
                    : {'kind': v, 'children': children}),
              ),
              const Spacer(),
              if (!isNot) ...[
                SmallButton(
                  label: '+ ${tOr(t, 'middleware.addLeaf', '条件')}',
                  onTap: () => write([...children, emptyLeaf]),
                ),
                const SizedBox(width: AidogSpace.sxs),
                SmallButton(
                  label: '+ ${tOr(t, 'middleware.addGroup', '子组')}',
                  onTap: () => write([
                    ...children,
                    {'kind': 'any', 'children': [emptyLeaf]},
                  ]),
                ),
                const SizedBox(width: AidogSpace.sxs),
              ],
              if (onRemove != null)
                SmallButton(
                  label: removeLabel ?? t.t('action.delete'),
                  onTap: onRemove,
                ),
            ],
          ),
          for (var i = 0; i < children.length; i++)
            ConditionTreeEditor(
              key: ValueKey('cond-node-$depth-$i'),
              node: children[i],
              onChanged: (n) => write([
                for (var j = 0; j < children.length; j++)
                  j == i ? n : children[j],
              ]),
              onRemove: isNot
                  ? null
                  : () => write([
                        for (var j = 0; j < children.length; j++)
                          if (j != i) children[j],
                      ]),
              depth: depth + 1,
            ),
        ],
      ),
    );
  }
}

class _LeafEditor extends StatelessWidget {
  const _LeafEditor({
    required this.node,
    required this.onChanged,
    this.onRemove,
    this.removeLabel,
  });

  final Map<String, Object?> node;
  final ValueChanged<Map<String, Object?>> onChanged;
  final VoidCallback? onRemove;
  final String? removeLabel;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final target = '${node['target']}';
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AidogSpace.sxs),
      child: Row(
        children: [
          _smallDropdown(
            context,
            value: target,
            options: kMwTargets,
            labelOf: (v) => _targetLabel(t, v),
            onChanged: (v) => onChanged({...node, 'target': v, 'field': ''}),
          ),
          const SizedBox(width: AidogSpace.sxs),
          if (_leafHasField(target))
            Expanded(
              child: PlainTextField(
                key: const ValueKey('cond-leaf-field'),
                value: '${node['field'] ?? ''}',
                hint: tOr(t, 'middleware.fieldHint', '字段（空=整体 / JSON path / header 名）'),
                onSubmitted: (v) => onChanged({...node, 'field': v}),
              ),
            ),
          const SizedBox(width: AidogSpace.sxs),
          _smallDropdown(
            context,
            value: '${node['match_type']}',
            options: kMwMatchTypes,
            onChanged: (v) => onChanged({...node, 'match_type': v}),
          ),
          const SizedBox(width: AidogSpace.sxs),
          Expanded(
            flex: 2,
            child: PlainTextField(
              key: const ValueKey('cond-leaf-pattern'),
              value: '${node['pattern'] ?? ''}',
              hint: tOr(t, 'middleware.pattern', '匹配模式'),
              onSubmitted: (v) => onChanged({...node, 'pattern': v}),
            ),
          ),
          if ('${node['match_type']}' == 'regex') ...[
            const SizedBox(width: AidogSpace.sxs),
            _smallDropdown(
              context,
              value: '${node['validator'] ?? ''}'.isEmpty
                  ? 'none'
                  : '${node['validator']}',
              options: ['none', ...kDslValidators],
              labelOf: (v) => switch (v) {
                'luhn' => tOr(t, 'middleware.checksum.luhn', 'Luhn (银行卡)'),
                'iban' => tOr(t, 'middleware.checksum.iban', 'IBAN'),
                'cn_id' => tOr(t, 'middleware.checksum.cn_id', '中国身份证'),
                _ => tOr(t, 'middleware.checksum.none', '无校验位'),
              },
              onChanged: (v) => onChanged({
                ...node,
                'validator': v == 'none' ? '' : v,
              }),
            ),
          ],
          if (onRemove != null) ...[
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              label: removeLabel ?? AidogI18n.of(context).t('action.delete'),
              onTap: onRemove,
            ),
          ],
        ],
      ),
    );
  }
}

String _targetLabel(I18nController t, String v) => switch (v) {
      'request_body' => tOr(t, 'middleware.target.request_body', '请求 body'),
      'request_headers' =>
        tOr(t, 'middleware.target.request_headers', '请求 header'),
      'response_body' => tOr(t, 'middleware.target.response_body', '响应 body'),
      'response_headers' =>
        tOr(t, 'middleware.target.response_headers', '响应 header'),
      'status' => tOr(t, 'middleware.target.status', '状态码'),
      'model' => tOr(t, 'middleware.target.model', '模型'),
      _ => v,
    };

Widget _smallDropdown(
  BuildContext context, {
  required String value,
  required List<String> options,
  required ValueChanged<String> onChanged,
  String Function(String)? labelOf,
}) {
  final theme = AidogTheme.of(context);
  final items = <String>[
    if (value.isNotEmpty && !options.contains(value)) value,
    ...options,
  ];
  return DropdownButton<String>(
    value: value.isEmpty ? null : value,
    isDense: true,
    items: [
      for (final o in items)
        DropdownMenuItem(value: o, child: Text(labelOf?.call(o) ?? o)),
    ],
    onChanged: (v) {
      if (v != null) onChanged(v);
    },
    style: AidogType.micro.copyWith(color: theme.c.fg),
    dropdownColor: theme.c.surface2,
    underline: const SizedBox.shrink(),
    iconSize: 14,
  );
}

// ── 动作链编辑器 ────────────────────────────────────────

class ActionChainEditor extends StatelessWidget {
  const ActionChainEditor({
    super.key,
    required this.steps,
    required this.onChanged,
  });

  final List<Map<String, Object?>> steps;
  final ValueChanged<List<Map<String, Object?>>> onChanged;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (var i = 0; i < steps.length; i++)
          _stepCard(context, t, i, steps[i]),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: SmallButton(
            key: const ValueKey('mw-add-action'),
            label: '+ ${tOr(t, 'middleware.addAction', '动作')}',
            onTap: () => onChanged([
              ...steps,
              {'kind': 'warn', 'params': defaultActionParams()},
            ]),
          ),
        ),
      ],
    );
  }

  Widget _stepCard(
    BuildContext context,
    I18nController t,
    int i,
    Map<String, Object?> st,
  ) {
    final theme = AidogTheme.of(context);
    final kind = '${st['kind']}';
    final params = st['params'] is Map
        ? Map<String, Object?>.from(st['params'] as Map)
        : <String, Object?>{};
    void setStep(Map<String, Object?> next) => onChanged([
          for (var j = 0; j < steps.length; j++) j == i ? next : steps[j],
        ]);
    void setParams(Map<String, Object?> p) => setStep({...st, 'params': p});
    Widget row(List<Widget> children) => Padding(
          padding: const EdgeInsets.symmetric(vertical: AidogSpace.sxs),
          child: Row(children: children),
        );
    return Container(
      key: ValueKey('mw-action-$i'),
      margin: const EdgeInsets.only(bottom: AidogSpace.sxs),
      padding: const EdgeInsets.all(AidogSpace.ssm),
      decoration: BoxDecoration(
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        color: theme.c.surface2,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Text('${i + 1}',
                  style: AidogType.micro.copyWith(color: theme.c.fg3)),
              const SizedBox(width: AidogSpace.ssm),
              _smallDropdown(
                context,
                value: kind,
                options: kMwActionKinds,
                labelOf: (v) => '${_actionLabel(t, v)}'
                    '${kMwTerminalKinds.contains(v) ? ' ⏹' : ''}',
                onChanged: (v) => setStep({...st, 'kind': v}),
              ),
              const Spacer(),
              SmallButton(
                label: '↑',
                onTap: i == 0
                    ? null
                    : () {
                        final next = [...steps];
                        final tmp = next[i - 1];
                        next[i - 1] = next[i];
                        next[i] = tmp;
                        onChanged(next);
                      },
              ),
              const SizedBox(width: AidogSpace.sxs),
              SmallButton(
                label: '↓',
                onTap: i == steps.length - 1
                    ? null
                    : () {
                        final next = [...steps];
                        final tmp = next[i + 1];
                        next[i + 1] = next[i];
                        next[i] = tmp;
                        onChanged(next);
                      },
              ),
              const SizedBox(width: AidogSpace.sxs),
              SmallButton(
                label: t.t('action.delete'),
                onTap: () => onChanged([
                  for (var j = 0; j < steps.length; j++)
                    if (j != i) steps[j],
                ]),
              ),
            ],
          ),
          if (kind == 'mask' || kind == 'override')
            row([
              Expanded(
                child: PlainTextField(
                  key: ValueKey('mw-action-$i-replacement'),
                  value: '${params['replacement'] ?? '****'}',
                  hint: 'replacement（默认 ****，regex 支持 \$1）',
                  onSubmitted: (v) => setParams({...params, 'replacement': v}),
                ),
              ),
              if (kind == 'mask') ...[
                const SizedBox(width: AidogSpace.sxs),
                for (final f in kMwMaskFields)
                  Padding(
                    padding: const EdgeInsets.only(left: AidogSpace.sxs),
                    child: SmallButton(
                      key: ValueKey('mw-action-$i-field-$f'),
                      label: f,
                      active: (params['fields'] as List? ?? const []).contains(f),
                      onTap: () {
                        final cur = (params['fields'] as List? ?? const []).toList();
                        setParams({
                          ...params,
                          'fields': cur.contains(f)
                              ? cur.where((x) => x != f).toList()
                              : [...cur, f],
                        });
                      },
                    ),
                  ),
              ],
            ]),
          if (kind == 'block')
            SwitchRow(
              key: ValueKey('mw-action-$i-observe'),
              label: tOr(t, 'middleware.observe', '观察模式'),
              description: tOr(
                t,
                'middleware.observeHint',
                '开启后命中不拦截：请求照常转发并计费，只在日志里记一笔',
              ),
              value: params['observe'] == true,
              onChanged: (v) => setParams({...params, 'observe': v}),
            ),
          if (kind == 'inject') ...[
            row([
              _smallDropdown(
                context,
                value: '${params['inject_mode'] ?? ''}'.isEmpty
                    ? 'system_append'
                    : '${params['inject_mode']}',
                options: const ['system_append', 'body_set', 'header_set'],
                onChanged: (v) => setParams({...params, 'inject_mode': v}),
              ),
              if ('${params['inject_mode']}' == 'body_set') ...[
                const SizedBox(width: AidogSpace.sxs),
                Expanded(
                  child: PlainTextField(
                    value: '${params['target'] ?? ''}',
                    hint: 'target JSON key',
                    onSubmitted: (v) => setParams({...params, 'target': v}),
                  ),
                ),
              ],
              const SizedBox(width: AidogSpace.sxs),
              Expanded(
                flex: 2,
                child: PlainTextField(
                  key: ValueKey('mw-action-$i-value'),
                  value: '${params['value'] ?? ''}',
                  hint: 'value',
                  onSubmitted: (v) => setParams({...params, 'value': v}),
                ),
              ),
            ]),
          ],
          if (kind == 'budget_gate')
            row([
              Text('\$', style: AidogType.micro.copyWith(color: theme.c.fg3)),
              const SizedBox(width: AidogSpace.sxs),
              SizedBox(
                width: 140,
                child: PlainTextField(
                  key: ValueKey('mw-action-$i-budget'),
                  value: '${params['budget_usd'] ?? 0}',
                  hint: tOr(t, 'middleware.budgetAmount', '本月预算上限（美元）'),
                  onSubmitted: (v) => setParams({
                    ...params,
                    'budget_usd': double.tryParse(v) ?? 0,
                  }),
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              Expanded(
                child: Text(
                  tOr(
                    t,
                    'middleware.budgetHint',
                    '本自然月内作用范围的累计花费达到该金额后，请求被拒绝（每月 1 号归零）',
                  ),
                  style: AidogType.micro.copyWith(color: theme.c.fg3),
                ),
              ),
            ]),
          if (kind == 'classify')
            row([
              Expanded(
                child: PlainTextField(
                  key: ValueKey('mw-action-$i-category'),
                  value: '${params['category'] ?? ''}',
                  hint: 'category',
                  onSubmitted: (v) => setParams({...params, 'category': v}),
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              SwitchRow(
                label: 'retryable',
                value: params['retryable'] != false,
                onChanged: (v) => setParams({...params, 'retryable': v}),
              ),
              const SizedBox(width: AidogSpace.sxs),
              SizedBox(
                width: 120,
                child: PlainTextField(
                  value: params['override_status'] == null
                      ? ''
                      : '${params['override_status']}',
                  hint: 'override status',
                  onSubmitted: (v) => setParams({
                    ...params,
                    'override_status': v.isEmpty ? null : (int.tryParse(v) ?? 0),
                  }),
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              Expanded(
                child: PlainTextField(
                  value: params['override_body'] == null
                      ? ''
                      : '${params['override_body']}',
                  hint: 'override body',
                  onSubmitted: (v) => setParams({
                    ...params,
                    'override_body': v.isEmpty ? null : v,
                  }),
                ),
              ),
            ]),
        ],
      ),
    );
  }
}

String _actionLabel(I18nController t, String k) => switch (k) {
      'mask' => tOr(t, 'middleware.action.mask', '脱敏'),
      'block' => tOr(t, 'middleware.action.block', '拦截'),
      'warn' => tOr(t, 'middleware.action.warn', '告警'),
      'inject' => tOr(t, 'middleware.action.inject', '注入'),
      'override' => tOr(t, 'middleware.action.override', '改写'),
      'classify' => tOr(t, 'middleware.action.classify', '分类'),
      'budget_gate' => tOr(t, 'middleware.action.budgetGate', '预算闸门'),
      _ => k,
    };

// ── 应用范围编辑器 ──────────────────────────────────────

class AppliesToEditor extends StatelessWidget {
  const AppliesToEditor({
    super.key,
    required this.value,
    required this.onChanged,
    required this.platforms,
    required this.groups,
  });

  /// `{platforms:[id…], groups:[group_key…], models:[…]}`。
  final Map<String, Object?> value;
  final ValueChanged<Map<String, Object?>> onChanged;
  final List<({int id, String name})> platforms;
  final List<({int id, String name, String groupKey})> groups;

  void _toggle(String dim, Object v) {
    final cur = (value[dim] as List? ?? const []).toList();
    onChanged({
      ...value,
      dim: cur.contains(v) ? cur.where((x) => x != v).toList() : [...cur, v],
    });
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    Widget chips(
      String key,
      String label,
      List<({String value, String label})> opts,
    ) =>
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(label, style: AidogType.micro.copyWith(color: theme.c.fg3)),
            const SizedBox(height: AidogSpace.sxs),
            if (opts.isEmpty)
              Text(tOr(t, 'middleware.appliesAll', '全部'),
                  style: AidogType.micro.copyWith(color: theme.c.fg3))
            else
              Wrap(
                spacing: AidogSpace.sxs,
                runSpacing: AidogSpace.sxs,
                children: [
                  for (final o in opts)
                    SmallButton(
                      key: ValueKey('$key-${o.value}'),
                      label: o.label,
                      active: (value[key] as List? ?? const []).contains(o.value),
                      onTap: () => _toggle(key, o.value),
                    ),
                ],
              ),
            const SizedBox(height: AidogSpace.ssm),
          ],
        );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        chips(
          'platforms',
          tOr(t, 'middleware.appliesPlatforms', '平台（空 = 全部）'),
          [
            for (final p in platforms) (value: '${p.id}', label: p.name),
          ],
        ),
        chips(
          'groups',
          tOr(t, 'middleware.appliesGroups', '分组（空 = 全部）'),
          [
            for (final g in groups) (value: g.groupKey, label: g.name),
          ],
        ),
        Text(
          tOr(t, 'middleware.appliesModels', '模型（逗号分隔，空 = 全部）'),
          style: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
        const SizedBox(height: AidogSpace.sxs),
        PlainTextField(
          key: const ValueKey('mw-applies-models'),
          value: ((value['models'] as List? ?? const [])).join(','),
          onSubmitted: (v) => onChanged({
            ...value,
            'models': v
                .split(',')
                .map((s) => s.trim())
                .where((s) => s.isNotEmpty)
                .toList(),
          }),
        ),
        Text(
          tOr(
            t,
            'middleware.appliesModelsHint',
            '填上游实际模型名（配了模型重映射就填映射之后的那个）。预算闸门按这个名字统计花费。',
          ),
          style: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
      ],
    );
  }
}

// ── 摘要（规则列表行 + 表单标题用）──────────────────────

/// 条件树摘要：递归渲染为 `a AND (b OR c)`（`conditionsSummary`）。
String conditionsSummary(Map<String, Object?> node) {
  final kind = '${node['kind']}';
  if (kind == 'leaf') {
    final tgt = switch ('${node['target']}') {
      'request_body' => 'req.body',
      'request_headers' => 'req.headers',
      'response_body' => 'resp.body',
      'response_headers' => 'resp.headers',
      _ => '${node['target']}',
    };
    final field = '${node['field'] ?? ''}'.isEmpty ? '' : '.${node['field']}';
    final checksum =
        '${node['validator'] ?? ''}'.isEmpty ? '' : ' +${node['validator']}';
    return '$tgt$field ${node['match_type']} /${node['pattern']}/$checksum';
  }
  if (kind == 'not') {
    return 'NOT(${conditionsSummary(Map<String, Object?>.from(node['child'] as Map))})';
  }
  final joined = [
    for (final c in (node['children'] as List? ?? const []))
      conditionsSummary(Map<String, Object?>.from(c as Map)),
  ].join(kind == 'all' ? ' AND ' : ' OR ');
  return joined.length > 1 || joined.isEmpty ? '($joined)' : joined;
}

/// 动作链摘要：`脱敏 → 拦截`。
String actionsSummary(I18nController t, List<Object?> steps) => [
      for (final s in steps.whereType<Map>())
        _actionLabel(t, '${s['kind']}'),
    ].join(' → ');

/// 是否存在观察模式的 block 动作（列表徽标用）。
bool hasObserveAction(List<Object?> steps) => steps.whereType<Map>().any(
      (s) => '${s['kind']}' == 'block' &&
          s['params'] is Map &&
          (s['params'] as Map)['observe'] == true,
    );

/// 应用范围摘要：`p:1,2 g:dev m:glm-4`。
String appliesSummary(Map<String, Object?>? at) {
  if (at == null) return '';
  String join(String k) =>
      ((at[k] as List? ?? const [])).map((e) => '$e').join(',');
  final parts = <String>[];
  final p = join('platforms');
  final g = join('groups');
  final m = join('models');
  if (p.isNotEmpty) parts.add('p:$p');
  if (g.isNotEmpty) parts.add('g:$g');
  if (m.isNotEmpty) parts.add('m:$m');
  return parts.join(' ');
}
