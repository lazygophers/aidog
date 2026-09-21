/// 平台编辑表单的零件层。对应 React 的 `formSections.tsx` 里那几个共用小组件
/// （`FormSection` / `ApiKeyField`）与散在各分区的 input/select 写法。
///
/// 三条规矩与 `settings/bits.dart` 同源：
/// 1. 色值一律 `AidogTheme.of(context).c.*`，零硬编码；
/// 2. `onChanged == null` = 禁用态，就是 React `disabled=` 的直接投影；
/// 3. 受控文本框的 `TextEditingController` 必须活在 State 里（理由见 `groups.dart:727`）。
library;

import 'package:flutter/material.dart';

import '../shell/theme.dart';
import '../shell/tiles.dart';
import '../utils/pinyin.dart';
import 'ui_bits.dart';

/// 编辑页的一块分区卡：标题 + 可选说明 + 可选右上角操作区 + 内容。
/// 对应 `formSections.tsx:54::FormSection`。
class FormSection extends StatelessWidget {
  const FormSection({
    super.key,
    required this.title,
    this.desc,
    this.action,
    required this.children,
  });

  final String title;
  final String? desc;
  final Widget? action;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.smd),
      child: Tile(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        title,
                        style: AidogType.tile.copyWith(color: theme.c.fg),
                      ),
                      if (desc != null && desc!.isNotEmpty)
                        Padding(
                          padding: const EdgeInsets.only(top: 2),
                          child: Text(
                            desc!,
                            style: AidogType.caption.copyWith(
                              color: theme.c.fg3,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
                if (action != null) ...[
                  const SizedBox(width: AidogSpace.ssm),
                  action!,
                ],
              ],
            ),
            const SizedBox(height: AidogSpace.ssm),
            ...children,
          ],
        ),
      ),
    );
  }
}

/// 受控单行 / 多行文本框。外部值变了才回灌输入框，免得打断用户正在选的那一段。
class PlatformField extends StatefulWidget {
  const PlatformField({
    super.key,
    required this.value,
    required this.onChanged,
    this.hint,
    this.label,
    this.maxLines = 1,
    this.obscure = false,
    this.enabled = true,
    this.mono = false,
  });

  final String value;
  final ValueChanged<String>? onChanged;
  final String? hint;
  final String? label;
  final int maxLines;
  final bool obscure;
  final bool enabled;

  /// 脚本正文 / token 之类用等宽字体。
  final bool mono;

  @override
  State<PlatformField> createState() => _PlatformFieldState();
}

class _PlatformFieldState extends State<PlatformField> {
  late final TextEditingController _ctrl = TextEditingController(
    text: widget.value,
  );

  @override
  void didUpdateWidget(PlatformField old) {
    super.didUpdateWidget(old);
    if (widget.value != _ctrl.text) {
      _ctrl.value = TextEditingValue(
        text: widget.value,
        selection: TextSelection.collapsed(offset: widget.value.length),
      );
    }
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final on = widget.enabled && widget.onChanged != null;
    final style = (widget.mono ? AidogType.numSm : AidogType.label).copyWith(
      color: on ? theme.c.fg : theme.c.fg3,
    );
    final field = TextField(
      controller: _ctrl,
      enabled: on,
      maxLines: widget.obscure ? 1 : widget.maxLines,
      obscureText: widget.obscure,
      style: style,
      decoration: InputDecoration(
        isDense: true,
        hintText: widget.hint,
        hintStyle: AidogType.label.copyWith(color: theme.c.fg3),
      ),
      onChanged: widget.onChanged,
    );
    if (widget.label == null) return field;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [TileMeta(widget.label!), field],
    );
  }
}

/// 下拉选择。选项少的地方也用它（一排按钮在这一页会把行撑爆）。
class FormDropdown extends StatelessWidget {
  const FormDropdown({
    super.key,
    required this.value,
    required this.options,
    required this.onChanged,
    this.labelOf,
    this.label,
    this.width,
  });

  final String value;
  final List<String> options;
  final ValueChanged<String>? onChanged;
  final String Function(String option)? labelOf;
  final String? label;
  final double? width;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    // 当前值不在候选里（远端改名 / 用户手填）也要能显示，否则 Dropdown 会断言失败。
    final items = <String>[
      if (value.isNotEmpty && !options.contains(value)) value,
      ...options,
    ];
    final dd = DropdownButton<String>(
      value: value.isEmpty ? null : value,
      isDense: true,
      // 恒 true：给了 width 还让它按最宽选项撑开，IANA 时区名那一排必溢出。
      isExpanded: true,
      dropdownColor: theme.c.surface2,
      style: AidogType.label.copyWith(color: theme.c.fg),
      hint: Text('—', style: AidogType.label.copyWith(color: theme.c.fg3)),
      onChanged: onChanged == null
          ? null
          : (v) {
              if (v != null) onChanged!(v);
            },
      items: [
        for (final o in items)
          DropdownMenuItem<String>(
            value: o,
            child: Text(
              labelOf?.call(o) ?? o,
              overflow: TextOverflow.ellipsis,
            ),
          ),
      ],
    );
    final sized = width == null ? dd : SizedBox(width: width, child: dd);
    if (label == null) return sized;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [TileMeta(label!), sized],
    );
  }
}

/// 一行「说明文字」（分区里的 hint / 空态）。
class FormHint extends StatelessWidget {
  const FormHint(this.text, {super.key, this.danger = false});

  final String text;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Text(
        text,
        style: AidogType.caption.copyWith(
          color: danger ? theme.c.bad : theme.c.fg3,
        ),
      ),
    );
  }
}

/// 模型单元格：输入框 + 候选下拉（按 [pinyinMatch] 过滤）。
///
/// 对应 `ModelsMatrixSection.tsx:183::renderCell`。候选为空时退化成纯输入框
/// （React 那边 `hasDropdown=false` 时不画箭头，同一条）。
class ModelCell extends StatefulWidget {
  const ModelCell({
    super.key,
    required this.value,
    required this.candidates,
    required this.onChanged,
    required this.hint,
    required this.pickTooltip,
  });

  final String value;
  final List<String> candidates;
  final ValueChanged<String> onChanged;
  final String hint;
  final String pickTooltip;

  @override
  State<ModelCell> createState() => _ModelCellState();
}

class _ModelCellState extends State<ModelCell> {
  bool _open = false;

  List<String> get _filtered {
    final q = widget.value.trim().toLowerCase();
    if (q.isEmpty) return widget.candidates;
    return [
      for (final m in widget.candidates)
        if (pinyinMatch(q, m)) m,
    ];
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final hasDropdown = widget.candidates.isNotEmpty;
    final filtered = _filtered;
    final open = _open && filtered.isNotEmpty;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            Expanded(
              child: PlatformField(
                value: widget.value,
                hint: widget.hint,
                onChanged: widget.onChanged,
              ),
            ),
            if (hasDropdown)
              Tooltip(
                message: widget.pickTooltip,
                child: IconButton(
                  iconSize: 14,
                  visualDensity: VisualDensity.compact,
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(
                    minWidth: 22,
                    minHeight: 22,
                  ),
                  color: theme.c.fg3,
                  icon: Icon(
                    open ? Icons.arrow_drop_up : Icons.arrow_drop_down,
                  ),
                  onPressed: () => setState(() => _open = !_open),
                ),
              ),
          ],
        ),
        if (open)
          Container(
            constraints: const BoxConstraints(maxHeight: 200),
            margin: const EdgeInsets.only(top: 2),
            decoration: BoxDecoration(
              color: theme.c.surface2,
              border: Border.all(color: theme.c.line),
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: ListView(
              shrinkWrap: true,
              padding: const EdgeInsets.all(2),
              children: [
                for (final m in filtered)
                  InkWell(
                    onTap: () {
                      widget.onChanged(m);
                      setState(() => _open = false);
                    },
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: AidogSpace.ssm,
                        vertical: AidogSpace.sxs,
                      ),
                      child: Text(
                        m,
                        style: AidogType.label.copyWith(
                          color: m == widget.value
                              ? theme.c.accentText
                              : theme.c.fg,
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ),
      ],
    );
  }
}

/// 可搜索的协议选择器（`SearchableProtocolSelect.tsx` 的 Flutter 版）。
///
/// 只做「点开 → 搜 → 选」这条主路径：搜索走 `searchTerms` 纯子串 + `value` 子串，
/// 与 React 的过滤条件逐字一致（拼音等形式是 registry 的数据，不在代码里推导）。
class ProtocolPicker extends StatefulWidget {
  const ProtocolPicker({
    super.key,
    required this.value,
    required this.codingPlan,
    required this.options,
    required this.onChanged,
    required this.searchHint,
    required this.noMatchText,
  });

  final String value;
  final bool codingPlan;

  /// (value, label, codingPlan, searchTerms) 四元组，来自 registry 派生层。
  final List<({String value, String label, bool codingPlan, List<String> terms})>
      options;
  final void Function(String protocol, bool codingPlan) onChanged;
  final String searchHint;
  final String noMatchText;

  @override
  State<ProtocolPicker> createState() => _ProtocolPickerState();
}

class _ProtocolPickerState extends State<ProtocolPicker> {
  bool _open = false;
  String _query = '';

  /// `SearchableProtocolSelect.tsx:61::labelOf` —— label 不含 Coding 字样时补后缀。
  String _labelOf(String base, bool cp) =>
      cp && !base.toLowerCase().contains('coding')
          ? '$base Coding Plan'
          : base;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final q = _query.trim().toLowerCase();
    final filtered = [
      for (final p in widget.options)
        if (q.isEmpty ||
            p.terms.any((t) => t.toLowerCase().contains(q)) ||
            p.value.toLowerCase().contains(q))
          p,
    ];
    String selectedLabel = widget.value;
    for (final p in widget.options) {
      if (p.value == widget.value) selectedLabel = p.label;
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        InkWell(
          onTap: () => setState(() {
            _open = !_open;
            _query = '';
          }),
          child: Container(
            padding: const EdgeInsets.symmetric(
              horizontal: AidogSpace.smd,
              vertical: AidogSpace.ssm,
            ),
            decoration: BoxDecoration(
              color: theme.c.surface2,
              border: Border.all(color: _open ? theme.c.accent : theme.c.line),
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    _labelOf(selectedLabel, widget.codingPlan),
                    overflow: TextOverflow.ellipsis,
                    style: AidogType.label.copyWith(color: theme.c.fg),
                  ),
                ),
                Icon(
                  _open ? Icons.expand_less : Icons.expand_more,
                  size: 14,
                  color: theme.c.fg3,
                ),
              ],
            ),
          ),
        ),
        if (_open) ...[
          const SizedBox(height: AidogSpace.sxs),
          PlatformField(
            value: _query,
            hint: widget.searchHint,
            onChanged: (v) => setState(() => _query = v),
          ),
          Container(
            constraints: const BoxConstraints(maxHeight: 256),
            margin: const EdgeInsets.only(top: 2),
            decoration: BoxDecoration(
              color: theme.c.surface2,
              border: Border.all(color: theme.c.line),
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: filtered.isEmpty
                ? Padding(
                    padding: const EdgeInsets.all(AidogSpace.ssm),
                    child: Text(
                      widget.noMatchText,
                      style: AidogType.caption.copyWith(color: theme.c.fg3),
                    ),
                  )
                : ListView(
                    shrinkWrap: true,
                    padding: const EdgeInsets.all(2),
                    children: [
                      for (final p in filtered)
                        InkWell(
                          onTap: () {
                            widget.onChanged(p.value, p.codingPlan);
                            setState(() {
                              _open = false;
                              _query = '';
                            });
                          },
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: AidogSpace.ssm,
                              vertical: AidogSpace.sxs,
                            ),
                            child: Row(
                              children: [
                                Expanded(
                                  child: Text(
                                    _labelOf(p.label, p.codingPlan),
                                    overflow: TextOverflow.ellipsis,
                                    style: AidogType.label.copyWith(
                                      color: p.value == widget.value
                                          ? theme.c.accentText
                                          : theme.c.fg,
                                    ),
                                  ),
                                ),
                                if (p.codingPlan)
                                  Text(
                                    'Code',
                                    style: AidogType.micro.copyWith(
                                      color: theme.c.ok,
                                    ),
                                  ),
                              ],
                            ),
                          ),
                        ),
                    ],
                  ),
          ),
        ],
      ],
    );
  }
}

/// 一排 0-6 的周几 toggle（`WEEKDAY_LABELS = S M T W T F S`，0=Sunday）。
class WeekdayToggles extends StatelessWidget {
  const WeekdayToggles({
    super.key,
    required this.selected,
    required this.onToggle,
    required this.tooltipOf,
  });

  static const List<String> labels = ['S', 'M', 'T', 'W', 'T', 'F', 'S'];

  final List<int> selected;
  final void Function(int day) onToggle;
  final String Function(int day) tooltipOf;

  @override
  Widget build(BuildContext context) => Wrap(
    spacing: 2,
    children: [
      for (var d = 0; d < 7; d++)
        Tooltip(
          message: tooltipOf(d),
          child: SmallButton(
            label: labels[d],
            active: selected.contains(d),
            onTap: () => onToggle(d),
          ),
        ),
    ],
  );
}
