/// 四类结构化字段的可视化编辑器（票 27 第 1 步），对齐
/// `src/components/settings/editors/_shared.tsx` 的 `KvEditor` / `KvSelectEditor` /
/// `ObjectEditor` / `StringListEditor`。
///
/// 在这之前这四类字段在 Flutter 侧全都退回手写 JSON 或裸多行文本 ——
/// 改个模型别名要自己敲花括号，少一个逗号当场看不出来。
///
/// 一条共同的写回约定（照抄 React）：**编辑到空就把整个键删掉**（回传 `null`），
/// 不留 `{}` / `[]` 这种空壳 —— 空壳会被写进 settings.json，看起来像「配过」。
library;

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../ui_bits.dart' show AidogSwitch, SmallButton;
import 'bits.dart';

/// 行尾那颗「移除」×。四个编辑器共用一个长相。
class _RemoveButton extends StatelessWidget {
  const _RemoveButton({super.key, required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return IconButton(
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
      iconSize: 14,
      visualDensity: VisualDensity.compact,
      tooltip: t.t('action.remove'),
      icon: Icon(Icons.close, color: theme.c.fg3),
      onPressed: onTap,
    );
  }
}

/// 不带标签的下拉，给 kv-select / object 的行内用（[SelectRow] 自带标签，塞不进行里）。
class _InlineSelect extends StatelessWidget {
  const _InlineSelect({
    super.key,
    required this.value,
    required this.options,
    required this.onChanged,
    this.allowNone = false,
  });

  final String value;
  final List<String> options;
  final ValueChanged<String?> onChanged;

  /// 允许「不设置」（object 的 select 子字段要，kv-select 的值不要）。
  final bool allowNone;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    // 当前值不在候选里（手写过的自定义值）也要显示，否则 Dropdown 会断言失败。
    final items = <String>[
      if (value.isNotEmpty && !options.contains(value)) value,
      ...options,
    ];
    return DropdownButton<String>(
      value: value.isEmpty ? null : value,
      underline: const SizedBox.shrink(),
      isDense: true,
      isExpanded: true,
      dropdownColor: theme.c.surface2,
      style: AidogType.micro.copyWith(color: theme.c.fg),
      hint: Text('—', style: AidogType.micro.copyWith(color: theme.c.fg3)),
      onChanged: onChanged,
      items: [
        if (allowNone)
          DropdownMenuItem<String>(
            value: null,
            child: Text(
              '—',
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
        for (final o in items)
          DropdownMenuItem<String>(value: o, child: Text(o)),
      ],
    );
  }
}

/// 键值对编辑器（`_shared.tsx:234::KvEditor`）。
///
/// 已有行的 **key 只读**：改键名等于删一条加一条，React 也是这个约定 ——
/// 允许就地改键会让「改到一半」的中间态（键重复 / 键为空）需要另一套校验。
class KvEditor extends StatefulWidget {
  const KvEditor({
    super.key,
    required this.items,
    required this.onChanged,
    this.keyPlaceholder = 'KEY',
    this.idPrefix = 'kv',
  });

  final Map<String, String> items;

  /// 空表回传 `null`（= 删掉这个字段）。
  final ValueChanged<Map<String, String>?> onChanged;
  final String keyPlaceholder;
  final String idPrefix;

  @override
  State<KvEditor> createState() => _KvEditorState();
}

class _KvEditorState extends State<KvEditor> {
  final TextEditingController _newKey = TextEditingController();
  final TextEditingController _newVal = TextEditingController();

  @override
  void dispose() {
    _newKey.dispose();
    _newVal.dispose();
    super.dispose();
  }

  void _write(Map<String, String> next) =>
      widget.onChanged(next.isEmpty ? null : next);

  void _add() {
    final k = _newKey.text.trim();
    if (k.isEmpty) return;
    _write({...widget.items, k: _newVal.text});
    _newKey.clear();
    _newVal.clear();
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final entries = widget.items.entries.toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final e in entries)
          Padding(
            key: ValueKey('${widget.idPrefix}-row-${e.key}'),
            padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
            child: Row(
              children: [
                Expanded(
                  flex: 2,
                  child: PlainTextField(value: e.key, enabled: false),
                ),
                const SizedBox(width: AidogSpace.sxs),
                Expanded(
                  flex: 3,
                  child: PlainTextField(
                    key: ValueKey('${widget.idPrefix}-val-${e.key}'),
                    value: e.value,
                    onSubmitted: (v) => _write({...widget.items, e.key: v}),
                  ),
                ),
                _RemoveButton(
                  key: ValueKey('${widget.idPrefix}-del-${e.key}'),
                  onTap: () => _write({...widget.items}..remove(e.key)),
                ),
              ],
            ),
          ),
        Row(
          children: [
            Expanded(
              flex: 2,
              child: TextField(
                key: ValueKey('${widget.idPrefix}-new-key'),
                controller: _newKey,
                style: AidogType.label.copyWith(
                  color: AidogTheme.of(context).c.fg,
                ),
                decoration: InputDecoration(
                  isDense: true,
                  hintText: widget.keyPlaceholder,
                ),
              ),
            ),
            const SizedBox(width: AidogSpace.sxs),
            Expanded(
              flex: 3,
              child: TextField(
                key: ValueKey('${widget.idPrefix}-new-val'),
                controller: _newVal,
                style: AidogType.label.copyWith(
                  color: AidogTheme.of(context).c.fg,
                ),
                decoration: const InputDecoration(
                  isDense: true,
                  hintText: 'VALUE',
                ),
                onSubmitted: (_) => _add(),
              ),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              key: ValueKey('${widget.idPrefix}-add'),
              label: '+',
              onTap: _add,
            ),
          ],
        ),
      ],
    );
  }
}

/// 键 + 下拉值的编辑器（`_shared.tsx:380::KvSelectEditor`）。
/// 与 [KvEditor] 的唯一差别是值那一格是固定候选的下拉。
class KvSelectEditor extends StatefulWidget {
  const KvSelectEditor({
    super.key,
    required this.items,
    required this.onChanged,
    required this.valueOptions,
    this.keyPlaceholder = 'KEY',
    this.idPrefix = 'kvs',
  });

  final Map<String, String> items;
  final ValueChanged<Map<String, String>?> onChanged;
  final List<String> valueOptions;
  final String keyPlaceholder;
  final String idPrefix;

  @override
  State<KvSelectEditor> createState() => _KvSelectEditorState();
}

class _KvSelectEditorState extends State<KvSelectEditor> {
  final TextEditingController _newKey = TextEditingController();

  @override
  void dispose() {
    _newKey.dispose();
    super.dispose();
  }

  void _write(Map<String, String> next) =>
      widget.onChanged(next.isEmpty ? null : next);

  void _add() {
    final k = _newKey.text.trim();
    if (k.isEmpty) return;
    // 新行取第一个候选当初值；没有候选就空串（用户随后再选）。
    _write({
      ...widget.items,
      k: widget.valueOptions.isEmpty ? '' : widget.valueOptions.first,
    });
    _newKey.clear();
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final entries = widget.items.entries.toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final e in entries)
          Padding(
            key: ValueKey('${widget.idPrefix}-row-${e.key}'),
            padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
            child: Row(
              children: [
                Expanded(
                  flex: 2,
                  child: PlainTextField(value: e.key, enabled: false),
                ),
                const SizedBox(width: AidogSpace.sxs),
                Expanded(
                  flex: 3,
                  child: _InlineSelect(
                    key: ValueKey('${widget.idPrefix}-val-${e.key}'),
                    value: e.value,
                    options: widget.valueOptions,
                    onChanged: (v) =>
                        _write({...widget.items, e.key: v ?? ''}),
                  ),
                ),
                _RemoveButton(
                  key: ValueKey('${widget.idPrefix}-del-${e.key}'),
                  onTap: () => _write({...widget.items}..remove(e.key)),
                ),
              ],
            ),
          ),
        Row(
          children: [
            Expanded(
              child: TextField(
                key: ValueKey('${widget.idPrefix}-new-key'),
                controller: _newKey,
                style: AidogType.label.copyWith(
                  color: AidogTheme.of(context).c.fg,
                ),
                decoration: InputDecoration(
                  isDense: true,
                  hintText: widget.keyPlaceholder,
                ),
                onSubmitted: (_) => _add(),
              ),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              key: ValueKey('${widget.idPrefix}-add'),
              label: '+',
              onTap: _add,
            ),
          ],
        ),
      ],
    );
  }
}

/// 字符串清单编辑器（`_shared.tsx:312::StringListEditor`）。
///
/// 与旧实现（一行一项的多行文本框）的差别不只是长相：那种写法下**空行与前后空格
/// 都会被吃掉**，而且没法逐条删 —— 要删中间一条得自己数行。
class StringListEditor extends StatefulWidget {
  const StringListEditor({
    super.key,
    required this.items,
    required this.onChanged,
    required this.addLabel,
    this.idPrefix = 'list',
  });

  final List<String> items;

  /// 空清单回传 `null`（= 删掉这个字段）。
  final ValueChanged<List<String>?> onChanged;

  /// 新增那一格的引导语（React 的 `addLabel`）。
  final String addLabel;
  final String idPrefix;

  @override
  State<StringListEditor> createState() => _StringListEditorState();
}

class _StringListEditorState extends State<StringListEditor> {
  final TextEditingController _draft = TextEditingController();

  @override
  void dispose() {
    _draft.dispose();
    super.dispose();
  }

  void _write(List<String> next) =>
      widget.onChanged(next.isEmpty ? null : next);

  void _add() {
    final v = _draft.text.trim();
    if (v.isEmpty) return;
    _write([...widget.items, v]);
    _draft.clear();
    setState(() {});
  }

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      for (var i = 0; i < widget.items.length; i++)
        Padding(
          key: ValueKey('${widget.idPrefix}-row-$i'),
          padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
          child: Row(
            children: [
              Expanded(
                child: PlainTextField(
                  key: ValueKey('${widget.idPrefix}-item-$i'),
                  value: widget.items[i],
                  onSubmitted: (v) {
                    final next = [...widget.items];
                    next[i] = v;
                    _write(next);
                  },
                ),
              ),
              _RemoveButton(
                key: ValueKey('${widget.idPrefix}-del-$i'),
                onTap: () => _write([
                  for (var j = 0; j < widget.items.length; j++)
                    if (j != i) widget.items[j],
                ]),
              ),
            ],
          ),
        ),
      Row(
        children: [
          Expanded(
            child: TextField(
              key: ValueKey('${widget.idPrefix}-new'),
              controller: _draft,
              style: AidogType.label.copyWith(
                color: AidogTheme.of(context).c.fg,
              ),
              decoration: InputDecoration(
                isDense: true,
                hintText: widget.addLabel,
              ),
              // React 那边回车就加一条，这里同。
              onSubmitted: (_) => _add(),
            ),
          ),
          const SizedBox(width: AidogSpace.sxs),
          SmallButton(
            key: ValueKey('${widget.idPrefix}-add'),
            label: '+',
            onTap: _add,
          ),
        ],
      ),
    ],
  );
}

/// 固定子字段的对象编辑器（`_shared.tsx:443::ObjectEditor`）。
///
/// 子字段清单来自 schema 的 `objectFields`，四种类型：boolean / select /
/// string / string[]。**子字段清空即删键**，整棵树清空即删整个字段。
class ObjectEditor extends StatelessWidget {
  const ObjectEditor({
    super.key,
    required this.value,
    required this.onChanged,
    required this.fields,
    required this.addLabel,
    this.idPrefix = 'obj',
  });

  final Map<String, Object?> value;
  final ValueChanged<Map<String, Object?>?> onChanged;

  /// 每项形如 `{key, label, type, options?, placeholder?}`。
  final List<Map<String, Object?>> fields;
  final String addLabel;
  final String idPrefix;

  void _setKey(String k, Object? v) {
    final next = {...value};
    final empty =
        v == null || v == '' || (v is List && v.isEmpty) || v == false;
    if (empty) {
      next.remove(k);
    } else {
      next[k] = v;
    }
    onChanged(next.isEmpty ? null : next);
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Container(
      padding: const EdgeInsets.only(left: AidogSpace.ssm),
      decoration: BoxDecoration(
        border: BorderDirectional(
          start: BorderSide(color: theme.c.line, width: 2),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final f in fields) _sub(context, theme, f),
        ],
      ),
    );
  }

  Widget _sub(
    BuildContext context,
    AidogTheme theme,
    Map<String, Object?> f,
  ) {
    final key = '${f['key']}';
    final type = '${f['type']}';
    final label = '${f['label'] ?? key}';
    final options = (f['options'] as List? ?? const [])
        .map((e) => '$e')
        .toList();
    final v = value[key];
    final control = switch (type) {
      'boolean' => Align(
        alignment: AlignmentDirectional.centerStart,
        child: AidogSwitch(
          key: ValueKey('$idPrefix-$key'),
          value: v == true,
          compact: true,
          onChanged: () => _setKey(key, v != true),
        ),
      ),
      'select' => _InlineSelect(
        key: ValueKey('$idPrefix-$key'),
        value: v == null ? '' : '$v',
        options: options,
        allowNone: true,
        onChanged: (nv) => _setKey(key, nv),
      ),
      'string[]' => StringListEditor(
        key: ValueKey('$idPrefix-$key'),
        idPrefix: '$idPrefix-$key',
        items: v is List ? v.map((e) => '$e').toList() : const [],
        addLabel: addLabel,
        onChanged: (list) => _setKey(key, list),
      ),
      _ => PlainTextField(
        key: ValueKey('$idPrefix-$key'),
        value: v == null ? '' : '$v',
        hint: f['placeholder'] as String?,
        onSubmitted: (nv) => _setKey(key, nv.trim()),
      ),
    };
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Row(
        crossAxisAlignment: type == 'string[]'
            ? CrossAxisAlignment.start
            : CrossAxisAlignment.center,
        children: [
          SizedBox(
            width: 110,
            child: Padding(
              padding: EdgeInsets.only(top: type == 'string[]' ? 6 : 0),
              child: Text(
                label,
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
            ),
          ),
          const SizedBox(width: AidogSpace.ssm),
          Expanded(child: control),
        ],
      ),
    );
  }
}

/// 带标签列的字段行外壳：**左右分栏**（React `FieldLabel`，editors/_shared.tsx:117-171）——
/// 左列固定 200（S.labelW）：label 15 w500 + 重置 pill + key mono 13 + desc 13；
/// 右列是控件本体（flex 1）。原先「标签在上 + 控件全宽在下」的纵向堆叠是
/// settings 域最大的结构差（审计 batch3 #1）。
class FieldShell extends StatelessWidget {
  const FieldShell({
    super.key,
    required this.label,
    this.fieldKey,
    required this.child,
    this.description,
    this.reset,
    this.crossAxisAlignment = CrossAxisAlignment.start,
  });

  final String label;

  /// schema 键名（左列第二行的 mono 小字）。
  final String? fieldKey;
  final String? description;

  /// 行内重置 pill（值 ≠ 推荐默认时才有，`_shared.tsx:144-160`）。
  final Widget? reset;
  final Widget child;

  /// 长控件（kv / 清单 / json）顶对齐，短控件（开关 / 下拉）居中。
  final CrossAxisAlignment crossAxisAlignment;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.slg),
      child: Row(
        crossAxisAlignment: crossAxisAlignment,
        children: [
          SizedBox(
            width: 200,
            child: Padding(
              padding: const EdgeInsets.only(top: 6),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Wrap(
                    spacing: 6,
                    runSpacing: 4,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      HighlightedText(
                        label,
                        style: AidogType.body.copyWith(
                          fontWeight: FontWeight.w500,
                          color: theme.c.fg,
                        ),
                      ),
                      ?reset,
                    ],
                  ),
                  if (fieldKey != null && fieldKey!.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.only(top: 3),
                      child: Text(
                        ltr(fieldKey!),
                        style: AidogType.numSm.copyWith(
                          fontSize: 13,
                          color: theme.c.fg3,
                        ),
                      ),
                    ),
                  if (description != null && description!.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.only(top: 3),
                      child: Text(
                        description!,
                        style: AidogType.label.copyWith(
                          fontSize: 13,
                          color: theme.c.fg3,
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(child: child),
        ],
      ),
    );
  }
}
