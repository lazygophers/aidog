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
  'request_body',
  'request_headers',
  'response_body',
  'response_headers',
  'status',
  'model',
];
const List<String> kMwMatchTypes = ['contains', 'regex', 'exact'];
const List<String> kMwActionKinds = [
  'mask',
  'block',
  'warn',
  'inject',
  'override',
  'classify',
  'budget_gate',
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
bool _leafHasField(String target) =>
    target.contains('body') || target.contains('headers');

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
    if ('${node['kind']}' == 'leaf') {
      return _LeafEditor(
        node: node,
        onChanged: onChanged,
        onRemove: onRemove,
        removeLabel: removeLabel,
      );
    }
    final theme = AidogTheme.of(context);
    final t = AidogI18n.of(context);
    final isNot = '${node['kind']}' == 'not';
    final children = isNot
        ? [Map<String, Object?>.from(node['child'] as Map)]
        : [
            for (final c in (node['children'] as List? ?? const []))
              Map<String, Object?>.from(c as Map),
          ];
    void write(List<Map<String, Object?>> cs) => onChanged(
      isNot
          ? {'kind': 'not', 'child': cs.isEmpty ? emptyLeaf : cs[0]}
          : {...node, 'children': cs},
    );
    return Container(
      margin: EdgeInsets.only(left: depth * 12.0),
      // 组卡：padding 8、底色 `--bg-glass`（= surface，与外层卡同底，只靠 1px 边
      // 分层）、子节点之间 gap 6（`MiddlewareRules.tsx:346-356`）。
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        color: theme.c.surface,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 组头行 `gap: 8`（`MiddlewareRules.tsx:358`）。
          Row(
            spacing: 8,
            children: [
              _smallDropdown(
                context,
                // kind 下拉 `minWidth: 92`（`MiddlewareRules.tsx:360`）。
                minWidth: 92,
                value: '${node['kind']}',
                options: const ['all', 'any', 'not'],
                labelOf: (v) => switch (v) {
                  'all' => tOr(t, 'middleware.node.all', 'AND (全部满足)'),
                  'any' => tOr(t, 'middleware.node.any', 'OR (任一满足)'),
                  _ => tOr(t, 'middleware.node.not', 'NOT (取反)'),
                },
                onChanged: (v) => onChanged(
                  v == 'not'
                      ? {
                          'kind': 'not',
                          'child': children.isEmpty ? emptyLeaf : children[0],
                        }
                      : {'kind': v, 'children': children},
                ),
              ),
              const Spacer(),
              if (!isNot) ...[
                // `+ 条件` / `+ 子组` 是 ghost（无描边）11px
                //（`MiddlewareRules.tsx:370-375`）。
                SmallButton(
                  ghost: true,
                  fontSize: 11,
                  label: '+ ${tOr(t, 'middleware.addLeaf', '条件')}',
                  onTap: () => write([...children, emptyLeaf]),
                ),
                SmallButton(
                  ghost: true,
                  fontSize: 11,
                  label: '+ ${tOr(t, 'middleware.addGroup', '子组')}',
                  onTap: () => write([
                    ...children,
                    {
                      'kind': 'any',
                      'children': [emptyLeaf],
                    },
                  ]),
                ),
              ],
              // 删除是 ghost 图标键 `<IconClose size={12}>` + tertiary 色，
              // 不是文字按钮（`MiddlewareRules.tsx:378-382`）。
              if (onRemove != null)
                IconGhostButton(
                  icon: Icons.close,
                  size: 12,
                  color: theme.c.fg3,
                  tooltip: removeLabel ?? t.t('action.delete'),
                  onTap: onRemove,
                ),
            ],
          ),
          for (var i = 0; i < children.length; i++)
            Padding(
              // 组内 `gap: 6`（`MiddlewareRules.tsx:352`）。
              padding: const EdgeInsets.only(top: AidogSpace.ssm),
              child: ConditionTreeEditor(
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
    final theme = AidogTheme.of(context);
    final target = '${node['target']}';
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AidogSpace.sxs),
      // 叶子行 `gap: 6`（`MiddlewareRules.tsx:262`）。
      child: Row(
        spacing: AidogSpace.ssm,
        children: [
          _smallDropdown(
            context,
            // target 下拉 `minWidth: 140`（`MiddlewareRules.tsx:264`）。
            minWidth: 140,
            value: target,
            options: kMwTargets,
            labelOf: (v) => _targetLabel(t, v),
            onChanged: (v) => onChanged({...node, 'target': v, 'field': ''}),
          ),
          if (_leafHasField(target))
            Expanded(
              child: PlainTextField(
                // 对齐 React 的 AutoTextarea：长正则 / 多行值要看得全。
                maxLines: null,
                // React 这两格是 `AutoTextarea mono`
                //（`MiddlewareRules.tsx:273,290`）。
                mono: true,
                key: const ValueKey('cond-leaf-field'),
                value: '${node['field'] ?? ''}',
                hint: tOr(
                  t,
                  'middleware.fieldHint',
                  '字段（空=整体 / JSON path / header 名）',
                ),
                onSubmitted: (v) => onChanged({...node, 'field': v}),
              ),
            ),
          _smallDropdown(
            context,
            // match 下拉 `minWidth: 100`（`MiddlewareRules.tsx:282`）。
            minWidth: 100,
            value: '${node['match_type']}',
            options: kMwMatchTypes,
            onChanged: (v) => onChanged({...node, 'match_type': v}),
          ),
          Expanded(
            flex: 2,
            child: PlainTextField(
              // 对齐 React 的 AutoTextarea：长正则 / 多行值要看得全。
              maxLines: null,
              mono: true,
              key: const ValueKey('cond-leaf-pattern'),
              value: '${node['pattern'] ?? ''}',
              hint: tOr(t, 'middleware.pattern', '匹配模式'),
              onSubmitted: (v) => onChanged({...node, 'pattern': v}),
            ),
          ),
          if ('${node['match_type']}' == 'regex')
            // 「校验位」这一列光看下拉看不出是干嘛的，解释写在悬浮提示里
            //（`MiddlewareRules.tsx:299` 的 `title=`）。
            Tooltip(
              message: tOr(
                t,
                'middleware.checksumHint',
                '正则命中后再跑校验位，校验不过的不算命中',
              ),
              child: _smallDropdown(
                context,
                // validator 下拉 `minWidth: 110`（`MiddlewareRules.tsx:299`）。
                minWidth: 110,
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
                onChanged: (v) =>
                    onChanged({...node, 'validator': v == 'none' ? '' : v}),
              ),
            ),
          // 同组卡：ghost 图标键 12px tertiary（`MiddlewareRules.tsx:308-312`）。
          if (onRemove != null)
            IconGhostButton(
              icon: Icons.close,
              size: 12,
              color: theme.c.fg3,
              tooltip: removeLabel ?? AidogI18n.of(context).t('action.delete'),
              onTap: onRemove,
            ),
        ],
      ),
    );
  }
}

String _targetLabel(I18nController t, String v) => switch (v) {
  'request_body' => tOr(t, 'middleware.target.request_body', '请求 body'),
  'request_headers' => tOr(t, 'middleware.target.request_headers', '请求 header'),
  'response_body' => tOr(t, 'middleware.target.response_body', '响应 body'),
  'response_headers' => tOr(
    t,
    'middleware.target.response_headers',
    '响应 header',
  ),
  'status' => tOr(t, 'middleware.target.status', '状态码'),
  'model' => tOr(t, 'middleware.target.model', '模型'),
  _ => v,
};

/// 条件 / 动作里的下拉。React 全是 shadcn `SelectTrigger`：`h-9` + 1px
/// `border-input` + `bg-card` + `rounded-md` + 13px（`ui/select.tsx` +
/// `MiddlewareRules.tsx:264,282,299,360,435,494` 的 inline `fontSize: F.hint`），
/// 不是裸下拉。[minWidth] 逐处照抄 React 的 `minWidth`。
Widget _smallDropdown(
  BuildContext context, {
  required String value,
  required List<String> options,
  required ValueChanged<String> onChanged,
  String Function(String)? labelOf,
  double minWidth = 92,
}) {
  final theme = AidogTheme.of(context);
  final items = <String>[
    if (value.isNotEmpty && !options.contains(value)) value,
    ...options,
  ];
  return Container(
    constraints: BoxConstraints(minWidth: minWidth),
    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
    decoration: BoxDecoration(
      color: theme.c.surface,
      border: Border.all(color: theme.c.line),
      borderRadius: BorderRadius.circular(AidogRadius.md),
    ),
    child: DropdownButton<String>(
      value: value.isEmpty ? null : value,
      isDense: true,
      items: [
        for (final o in items)
          DropdownMenuItem(value: o, child: Text(labelOf?.call(o) ?? o)),
      ],
      onChanged: (v) {
        if (v != null) onChanged(v);
      },
      style: AidogType.label.copyWith(fontSize: 13, color: theme.c.fg),
      dropdownColor: theme.c.surface2,
      underline: const SizedBox.shrink(),
      iconSize: 14,
    ),
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
          // `+ 动作` 是 ghost + `fontSize: F.hint` 13（`MiddlewareRules.tsx:570`）。
          child: SmallButton(
            key: const ValueKey('mw-add-action'),
            ghost: true,
            fontSize: 13,
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
    // 参数行之间 `gap: 6`（`MiddlewareRules.tsx:431`）。
    Widget row(List<Widget> children) => Padding(
      padding: const EdgeInsets.only(top: AidogSpace.ssm),
      child: Row(spacing: AidogSpace.ssm, children: children),
    );
    return Container(
      key: ValueKey('mw-action-$i'),
      // 步骤卡：卡间 gap 6、padding 8、底色 `--bg-glass`（= surface）
      //（`MiddlewareRules.tsx:429-431`）。
      margin: const EdgeInsets.only(bottom: AidogSpace.ssm),
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        color: theme.c.surface,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            spacing: AidogSpace.ssm,
            children: [
              // 序号列固定宽 18，多位数不把后面的下拉推挤走；11px 无字距
              //（`MiddlewareRules.tsx:433`）。
              SizedBox(
                width: 18,
                child: Text(
                  '${i + 1}',
                  style: AidogType.micro.copyWith(
                    letterSpacing: 0,
                    color: theme.c.fg3,
                  ),
                ),
              ),
              _smallDropdown(
                context,
                // action kind 下拉 `minWidth: 110`（`MiddlewareRules.tsx:435`）。
                minWidth: 110,
                value: kind,
                options: kMwActionKinds,
                labelOf: (v) =>
                    '${_actionLabel(t, v)}'
                    '${kMwTerminalKinds.contains(v) ? ' ⏹' : ''}',
                onChanged: (v) => setStep({...st, 'kind': v}),
              ),
              const Spacer(),
              // ↑ ↓ 是 ghost 11（`MiddlewareRules.tsx:443-444`）。
              SmallButton(
                ghost: true,
                fontSize: 11,
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
              SmallButton(
                ghost: true,
                fontSize: 11,
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
              // 删除是 ghost 图标键 12px tertiary（`MiddlewareRules.tsx:445-447`）。
              IconGhostButton(
                icon: Icons.close,
                size: 12,
                color: theme.c.fg3,
                tooltip: t.t('action.delete'),
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
                  // 对齐 React 的 AutoTextarea：长正则 / 多行值要看得全。
                  maxLines: null,
                  // React 这格是 `AutoTextarea mono`（`MiddlewareRules.tsx:453`）。
                  mono: true,
                  key: ValueKey('mw-action-$i-replacement'),
                  value: '${params['replacement'] ?? '****'}',
                  hint: 'replacement（默认 ****，regex 支持 \$1）',
                  onSubmitted: (v) => setParams({...params, 'replacement': v}),
                ),
              ),
              if (kind == 'mask')
                // React 是收起式多选：触发器拼已选项，空态显示「全部字段
                //（messages + system）」，展开才是带勾选框的清单
                //（`MiddlewareRules.tsx:460-473`）。原先是两颗平铺按钮 + 一行
                // 空态小字，与「应用范围」那三维不同构。
                Expanded(
                  child: MultiSelectRow(
                    options: [
                      for (final f in kMwMaskFields) (value: f, label: f),
                    ],
                    selected: [...(params['fields'] as List? ?? const [])],
                    emptyLabel: tOr(
                      t,
                      'middleware.maskFieldsAll',
                      '全部字段（messages + system）',
                    ),
                    itemKeyPrefix: 'mw-action-$i-field',
                    onToggle: (f) {
                      final cur = (params['fields'] as List? ?? const [])
                          .toList();
                      setParams({
                        ...params,
                        'fields': cur.contains(f)
                            ? cur.where((x) => x != f).toList()
                            : [...cur, f],
                      });
                    },
                  ),
                ),
            ]),
          if (kind == 'block')
            // React 是**行内**一行：`<label>` 里开关 + 11px 文字，后面接一段
            // 11px tertiary 提示，整体 `gap: 6`（`MiddlewareRules.tsx:477-490`）。
            // `SwitchRow` 是整行块（13 w600 标题在左、开关在右、提示另起一行），
            // 形态完全不同。
            row([
              AidogSwitch(
                key: ValueKey('mw-action-$i-observe'),
                compact: true,
                value: params['observe'] == true,
                onChanged: () => setParams({
                  ...params,
                  'observe': params['observe'] != true,
                }),
              ),
              Text(
                tOr(t, 'middleware.observe', '观察模式'),
                style: AidogType.micro.copyWith(
                  letterSpacing: 0,
                  color: theme.c.fg,
                ),
              ),
              Expanded(
                child: Text(
                  tOr(
                    t,
                    'middleware.observeHint',
                    '开启后命中不拦截：请求照常转发并计费，只在日志里记一笔',
                  ),
                  style: AidogType.micro.copyWith(
                    letterSpacing: 0,
                    color: theme.c.fg3,
                  ),
                ),
              ),
            ]),
          if (kind == 'inject') ...[
            row([
              _smallDropdown(
                context,
                // inject mode 下拉 `minWidth: 150`（`MiddlewareRules.tsx:494`）。
                minWidth: 150,
                value: '${params['inject_mode'] ?? ''}'.isEmpty
                    ? 'system_append'
                    : '${params['inject_mode']}',
                options: const ['system_append', 'body_set', 'header_set'],
                onChanged: (v) => setParams({...params, 'inject_mode': v}),
              ),
              if ('${params['inject_mode']}' == 'body_set')
                Expanded(
                  child: PlainTextField(
                    // 对齐 React 的 AutoTextarea：长正则 / 多行值要看得全。
                    maxLines: null,
                    // target 那格 React 带 mono（`MiddlewareRules.tsx:503-508`）；
                    // value / category / override body 三格不带。
                    mono: true,
                    value: '${params['target'] ?? ''}',
                    hint: 'target JSON key',
                    onSubmitted: (v) => setParams({...params, 'target': v}),
                  ),
                ),
              Expanded(
                flex: 2,
                child: PlainTextField(
                  // 对齐 React 的 AutoTextarea：长正则 / 多行值要看得全。
                  maxLines: null,
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
              // 辅助小字 11px tertiary **无字距**（`MiddlewareRules.tsx:522,534`）。
              Text(
                '\$',
                style: AidogType.micro.copyWith(
                  letterSpacing: 0,
                  color: theme.c.fg3,
                ),
              ),
              // 🔴 这里原先是纯文本框 + `double.tryParse(v) ?? 0`：手滑写成
              // 「10 usd」会**静默变成 0**，预算闸门当场失效而界面一声不吭。
              // [NumberInput] 从源头堵死 —— 非数字敲不进来，空串不上报、
              // 输入框恢复原值，越界才夹取并显示范围。
              NumberInput(
                key: ValueKey('mw-action-$i-budget'),
                // `minWidth: 110; flex: "0 1 160px"`（`MiddlewareRules.tsx:524`）。
                width: 160,
                decimal: true,
                min: 0,
                value: '${params['budget_usd'] ?? 0}',
                hint: tOr(t, 'middleware.budgetAmount', '本月预算上限（美元）'),
                onChanged: (v) {
                  final parsed = double.tryParse(v);
                  if (parsed == null) return;
                  setParams({...params, 'budget_usd': parsed});
                },
              ),
              Expanded(
                child: Text(
                  tOr(
                    t,
                    'middleware.budgetHint',
                    '本自然月内作用范围的累计花费达到该金额后，请求被拒绝（每月 1 号归零）',
                  ),
                  style: AidogType.micro.copyWith(
                    letterSpacing: 0,
                    color: theme.c.fg3,
                  ),
                ),
              ),
            ]),
          if (kind == 'classify')
            row([
              Expanded(
                child: PlainTextField(
                  // 对齐 React 的 AutoTextarea：长正则 / 多行值要看得全。
                  maxLines: null,
                  key: ValueKey('mw-action-$i-category'),
                  value: '${params['category'] ?? ''}',
                  hint: 'category',
                  onSubmitted: (v) => setParams({...params, 'category': v}),
                ),
              ),
              // React 是行内 `<label gap:4 fontSize:11>` + Switch
              //（`MiddlewareRules.tsx:548-551`），不是整行的 SwitchRow。
              Row(
                mainAxisSize: MainAxisSize.min,
                spacing: AidogSpace.sxs,
                children: [
                  AidogSwitch(
                    key: ValueKey('mw-action-$i-retryable'),
                    compact: true,
                    value: params['retryable'] != false,
                    onChanged: () => setParams({
                      ...params,
                      'retryable': params['retryable'] == false,
                    }),
                  ),
                  Text(
                    'retryable',
                    style: AidogType.micro.copyWith(
                      letterSpacing: 0,
                      color: theme.c.fg,
                    ),
                  ),
                ],
              ),
              // 同预算上限：原先 `int.tryParse(v) ?? 0` 会把打错的状态码变成
              // `0`（一个根本不存在的 HTTP 状态）。夹在 100–599 内，越界时
              // 输入框下面显示这个范围。
              NumberInput(
                key: ValueKey('mw-action-$i-override-status'),
                // `minWidth: 110; flex: "0 1 140px"`（`MiddlewareRules.tsx:553`）。
                width: 140,
                min: 100,
                max: 599,
                value: params['override_status'] == null
                    ? ''
                    : '${params['override_status']}',
                hint: 'override status',
                onChanged: (v) {
                  final parsed = int.tryParse(v);
                  if (parsed == null) return;
                  setParams({...params, 'override_status': parsed});
                },
              ),
              Expanded(
                child: PlainTextField(
                  // 对齐 React 的 AutoTextarea：长正则 / 多行值要看得全。
                  maxLines: null,
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
    // 🔴 `value` 是 `Object` 不是 `String`：`applies_to.platforms` 在后端是
    // `Vec<i64>`（`aidog_db/src/models/middleware.rs:250`），这里写成字符串
    // 会让整条规则存不进去（serde 反序列化失败），或者平台限定被 `#[serde(default)]`
    // 吞成空数组 —— 规则看着存住了，限定却不生效。React 传的就是数字
    // （`MiddlewareRules.tsx:605`）。groups / models 那两维在后端是 `Vec<String>`，
    // 继续传字符串是对的。
    // 平台 / 分组是**多到会撑爆**的两维：几十个平台一平铺，下面的「分组」
    // 和「模型」就被挤出屏幕。React 这两维用的是收起式 `MultiSelect`
    //（`MiddlewareRules.tsx:604-623`），这里同构。
    Widget chips(
      String key,
      String label,
      List<({Object value, String label})> opts,
    ) => MultiSelectRow(
      label: label,
      options: opts,
      selected: [...(value[key] as List? ?? const [])],
      emptyLabel: tOr(t, 'middleware.appliesAll', '全部'),
      itemKeyPrefix: key,
      onToggle: (v) => _toggle(key, v),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        chips(
          'platforms',
          tOr(t, 'middleware.appliesPlatforms', '平台（空 = 全部）'),
          [for (final p in platforms) (value: p.id, label: p.name)],
        ),
        chips('groups', tOr(t, 'middleware.appliesGroups', '分组（空 = 全部）'), [
          for (final g in groups) (value: g.groupKey, label: g.name),
        ]),
        // 三维标签一律 `F.hint` 13 + `--text-secondary`
        //（`MiddlewareRules.tsx:601,612,623`），不是 micro 11 ls .66 fg3。
        Text(
          tOr(t, 'middleware.appliesModels', '模型（逗号分隔，空 = 全部）'),
          style: AidogType.label.copyWith(fontSize: 13, color: theme.c.fg2),
        ),
        const SizedBox(height: AidogSpace.sxs),
        PlainTextField(
          // 对齐 React 的 AutoTextarea：长正则 / 多行值要看得全。
          maxLines: null,
          // 模型清单也是 `AutoTextarea mono`（`MiddlewareRules.tsx:626-630`）。
          mono: true,
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
          // 口径提示也是 `F.hint` 13 + tertiary（`MiddlewareRules.tsx:632`）。
          style: AidogType.label.copyWith(fontSize: 13, color: theme.c.fg3),
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
    final checksum = '${node['validator'] ?? ''}'.isEmpty
        ? ''
        : ' +${node['validator']}';
    return '$tgt$field ${node['match_type']} /${node['pattern']}/$checksum';
  }
  if (kind == 'not') {
    return 'NOT(${conditionsSummary(Map<String, Object?>.from(node['child'] as Map))})';
  }
  // 括号按**子节点个数**判，不是按拼接串长度；一个子节点都没有时给 `∅`
  //（`MiddlewareRules.tsx:213-214`）。原先「串长 > 1」会把单个子节点也套上
  // 括号，空树则渲染成 `()`。
  final children = node['children'] as List? ?? const [];
  final joined = [
    for (final c in children) conditionsSummary(Map<String, Object?>.from(c as Map)),
  ].join(kind == 'all' ? ' AND ' : ' OR ');
  if (children.length > 1) return '($joined)';
  return joined.isEmpty ? '∅' : joined;
}

/// 动作链摘要：`脱敏 → 拦截`。
String actionsSummary(I18nController t, List<Object?> steps) =>
    [for (final s in steps.whereType<Map>()) _actionLabel(t, '${s['kind']}')]
        .join(' → ');

/// 是否存在观察模式的 block 动作（列表徽标用）。
bool hasObserveAction(List<Object?> steps) => steps.whereType<Map>().any(
  (s) =>
      '${s['kind']}' == 'block' &&
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
