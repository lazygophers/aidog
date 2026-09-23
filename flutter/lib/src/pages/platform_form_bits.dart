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
    this.focusNode,
    this.numeric = false,
  });

  /// 数字输入：弹数字键盘，与 React 的 `<Input type="number">` 同口径。
  /// 值仍是字符串（调用方自己 clamp），只影响输入法。
  final bool numeric;

  /// 调用方要监听聚焦时传（模型单元格靠它做「聚焦即弹候选」）。
  final FocusNode? focusNode;

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
      focusNode: widget.focusNode,
      enabled: on,
      keyboardType: widget.numeric
          ? const TextInputType.numberWithOptions(decimal: true)
          : null,
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
      // Material 默认在下拉底下画一条横线，与本项目的描边风格冲突。
      underline: const SizedBox.shrink(),
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
            child: Text(labelOf?.call(o) ?? o, overflow: TextOverflow.ellipsis),
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
  /// 候选是**浮层**，盖在下方内容上（`ModelsMatrixSection.tsx:195-262` 的
  /// Radix Popover）。原先是行内展开的容器，一打开就把下面的行整体顶下去，
  /// 整张表单跟着跳。浮层与锚点的对齐走 `filter_dropdown.dart` 同一套
  /// `LayerLink` + `OverlayEntry`。
  final _link = LayerLink();
  OverlayEntry? _entry;
  bool _open = false;

  /// 聚焦即弹候选（票 31 ⑥，对齐 `ModelsMatrixSection.tsx:207` 的 `onFocus`）。
  /// 改造前必须点右边那颗箭头，不点就不知道有候选可选。
  final FocusNode _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      if (_focus.hasFocus) _show();
    });
  }

  @override
  void dispose() {
    _entry?.remove();
    _entry = null;
    _focus.dispose();
    super.dispose();
  }

  @override
  void didUpdateWidget(ModelCell old) {
    super.didUpdateWidget(old);
    // 值 / 候选变了，浮层里的清单要跟着重算（候选被过滤空了就收起来）。
    if (_entry != null) _scheduleSync();
  }

  void _show() => _setOpen(true);

  void _hide() => _setOpen(false);

  /// 开合只记状态，真正动 Overlay 延到帧末。
  /// 焦点回调可能落在 build 期间，当场 insert / setState 会直接抛
  /// 「setState() called during build」。
  void _setOpen(bool v) {
    if (_open == v) return;
    _open = v;
    _scheduleSync();
  }

  void _scheduleSync() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _sync();
    });
  }

  void _sync() {
    final want = _open && _filtered.isNotEmpty;
    if (want && _entry == null) {
      _entry = OverlayEntry(builder: _panel);
      Overlay.of(context).insert(_entry!);
      setState(() {});
    } else if (!want && _entry != null) {
      _entry!.remove();
      _entry = null;
      setState(() {});
    } else {
      _entry?.markNeedsBuild();
    }
  }

  List<String> get _filtered {
    final q = widget.value.trim().toLowerCase();
    if (q.isEmpty) return widget.candidates;
    return [
      for (final m in widget.candidates)
        if (pinyinMatch(q, m)) m,
    ];
  }

  Widget _panel(BuildContext overlayContext) {
    final theme = AidogTheme.of(context);
    final filtered = _filtered;
    if (filtered.isEmpty) return const SizedBox.shrink();
    final box = context.findRenderObject() as RenderBox?;
    final width = box?.size.width ?? 200;
    return Stack(
      children: [
        // 浮层外任意点击即关。用 Listener 不用 GestureDetector：
        // 后者会把这一下点击吃掉，用户得点两次才能按到下面的按钮。
        // React 的 Popover 是 `modal=false`，点外面既关浮层又照常触发下面的控件。
        Positioned.fill(
          child: Listener(
            behavior: HitTestBehavior.translucent,
            onPointerDown: (_) => _hide(),
          ),
        ),
        CompositedTransformFollower(
          link: _link,
          targetAnchor: Alignment.bottomLeft,
          followerAnchor: Alignment.topLeft,
          offset: const Offset(0, 2),
          child: Align(
            alignment: AlignmentDirectional.topStart,
            child: Material(
              type: MaterialType.transparency,
              child: Container(
                width: width,
                constraints: const BoxConstraints(maxHeight: 200),
                decoration: BoxDecoration(
                  color: theme.c.surface2,
                  border: Border.all(color: theme.c.line),
                  borderRadius: BorderRadius.circular(AidogRadius.sm),
                  boxShadow: theme.shadowFloat,
                ),
                child: ListView(
                  shrinkWrap: true,
                  padding: const EdgeInsets.all(2),
                  children: [
                    for (final m in filtered)
                      InkWell(
                        onTap: () {
                          widget.onChanged(m);
                          _hide();
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
            ),
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final hasDropdown = widget.candidates.isNotEmpty;
    return CompositedTransformTarget(
      link: _link,
      child: Row(
        children: [
          Expanded(
            child: PlatformField(
              focusNode: _focus,
              value: widget.value,
              hint: widget.hint,
              // 输入即弹（`ModelsMatrixSection.tsx:203` 的 onChange 同款）。
              onChanged: (v) {
                widget.onChanged(v);
                if (_open) {
                  _scheduleSync();
                } else {
                  _show();
                }
              },
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
                  _open ? Icons.arrow_drop_up : Icons.arrow_drop_down,
                ),
                onPressed: () => _open ? _hide() : _show(),
              ),
            ),
        ],
      ),
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
  final List<
    ({String value, String label, bool codingPlan, List<String> terms})
  >
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
      cp && !base.toLowerCase().contains('coding') ? '$base Coding Plan' : base;

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

/// 日期时间输入（票 31 ②③）：**手打仍然是主入口**（React 的
/// `datetime-local` 本来就能手打），旁边多一颗按钮开系统选择器。
///
/// 改造前两处都是裸文本框，要用户自己敲 `YYYY-MM-DDTHH:MM`：
/// 过期时间那处敲错**静默不生效**（用户以为设好了），高峰窗口那两处
/// 连格式提示都没有。所以这里三件事一起给：选择器、占位提示、**手打非法格式
/// 当场红字**——不再默默丢弃。
///
/// 两处共用这一个组件（票面硬要求：别各写一套）。
class DateTimeField extends StatefulWidget {
  const DateTimeField({
    super.key,
    required this.value,
    required this.onChanged,
    required this.invalidText,
    required this.pickTooltip,
    this.label,
    this.idPrefix = 'dt',
  });

  /// `YYYY-MM-DDTHH:MM`；空串 = 未设置。
  final String value;

  /// 合法值或空串。**非法值不回调** —— 但会在下面显示红字，不是静默吞掉。
  final ValueChanged<String> onChanged;

  /// 手打非法格式时的提示文案。
  final String invalidText;
  final String pickTooltip;
  final String? label;
  final String idPrefix;

  @override
  State<DateTimeField> createState() => _DateTimeFieldState();
}

class _DateTimeFieldState extends State<DateTimeField> {
  /// 手打出来的非法串；null = 当前没有格式问题。
  String? _invalid;

  /// `YYYY-MM-DDTHH:MM` → DateTime；解不出来返回 null。
  static DateTime? parse(String v) =>
      v.trim().isEmpty ? null : DateTime.tryParse(v.trim());

  static String format(DateTime d) {
    String two(int n) => n.toString().padLeft(2, '0');
    return '${d.year}-${two(d.month)}-${two(d.day)}'
        'T${two(d.hour)}:${two(d.minute)}';
  }

  Future<void> _pick() async {
    final now = DateTime.now();
    final seed = parse(widget.value) ?? now;
    final date = await showDatePicker(
      context: context,
      initialDate: seed,
      // 过期时间与高峰窗口都可能设在过去（补登记）或较远的未来，窗口给宽些。
      firstDate: DateTime(now.year - 5),
      lastDate: DateTime(now.year + 10),
    );
    if (date == null || !mounted) return;
    final time = await showTimePicker(
      context: context,
      initialTime: TimeOfDay(hour: seed.hour, minute: seed.minute),
    );
    if (!mounted) return;
    final picked = DateTime(
      date.year,
      date.month,
      date.day,
      time?.hour ?? seed.hour,
      time?.minute ?? seed.minute,
    );
    setState(() => _invalid = null);
    widget.onChanged(format(picked));
  }

  void _onTyped(String v) {
    if (v.trim().isEmpty) {
      setState(() => _invalid = null);
      widget.onChanged('');
      return;
    }
    if (parse(v) == null) {
      // 关键差别：改造前这里直接 return，值被默默丢掉。
      setState(() => _invalid = v);
      return;
    }
    setState(() => _invalid = null);
    widget.onChanged(v.trim());
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            Expanded(
              child: PlatformField(
                key: ValueKey('${widget.idPrefix}-input'),
                label: widget.label,
                value: widget.value,
                hint: 'YYYY-MM-DDTHH:MM',
                onChanged: _onTyped,
              ),
            ),
            Tooltip(
              message: widget.pickTooltip,
              child: IconButton(
                key: ValueKey('${widget.idPrefix}-pick'),
                iconSize: 14,
                visualDensity: VisualDensity.compact,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
                color: theme.c.fg3,
                icon: const Icon(Icons.event),
                onPressed: _pick,
              ),
            ),
          ],
        ),
        if (_invalid != null)
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Text(
              widget.invalidText,
              key: ValueKey('${widget.idPrefix}-invalid'),
              style: AidogType.caption.copyWith(color: theme.c.bad),
            ),
          ),
      ],
    );
  }
}

/// 数字输入 + 上下步进。对齐 React 的 `<Input type="number" min max step>`
/// （`formSections.tsx:570-588,813-860`）：Flutter 没有原生 spinner，
/// 这里自己画两颗箭头，键盘也切成数字键盘。
///
/// 值进出都是字符串：空串 = 未填（走继承默认值那条路），调用方照旧自己 clamp。
class NumberField extends StatelessWidget {
  const NumberField({
    super.key,
    required this.value,
    required this.onChanged,
    this.min,
    this.max,
    this.step = 1,
    this.hint,
    this.label,
    this.width,
  });

  final String value;
  final ValueChanged<String> onChanged;
  final num? min;
  final num? max;
  final num step;
  final String? hint;
  final String? label;
  final double? width;

  /// 步进一格。空串从 [min]（没有就 0）起步，结果夹在 [min]..[max] 内。
  void _bump(num delta) {
    final cur = num.tryParse(value.trim()) ?? min ?? 0;
    var next = cur + delta;
    if (min != null && next < min!) next = min!;
    if (max != null && next > max!) next = max!;
    final isInt = step is int && next == next.roundToDouble();
    onChanged(isInt ? '${next.round()}' : '$next');
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final field = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Expanded(
          child: PlatformField(
            value: value,
            hint: hint,
            numeric: true,
            onChanged: onChanged,
          ),
        ),
        Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            _Step(
              icon: Icons.keyboard_arrow_up,
              color: theme.c.fg3,
              onTap: () => _bump(step),
            ),
            _Step(
              icon: Icons.keyboard_arrow_down,
              color: theme.c.fg3,
              onTap: () => _bump(-step),
            ),
          ],
        ),
      ],
    );
    final sized = width == null ? field : SizedBox(width: width, child: field);
    if (label == null) return sized;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [TileMeta(label!), sized],
    );
  }
}

class _Step extends StatelessWidget {
  const _Step({required this.icon, required this.color, required this.onTap});

  final IconData icon;
  final Color color;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => InkWell(
    onTap: onTap,
    child: SizedBox(
      height: 12,
      width: 16,
      child: Icon(icon, size: 12, color: color),
    ),
  );
}

/// 小数输入框：数字键盘 + 解析失败时**保留原文并报错**。
///
/// 存在的理由是一类静默丢数据：调用方把值存成 `double`，输入框又直接写
/// `double.tryParse(v) ?? 0`，于是打到一半的 `10.`、打错的 `10.5.3` 全部变 0，
/// 而且外层一重建还会把输入框里的字换成那个 0 —— 用户看到的是自己打的东西
/// 凭空消失，没有任何提示。
///
/// 这里的做法：正在编辑的原文归本 State 管，只有**解析得出来**才往上报；
/// 解析不出来就停在那儿，下面显示一行红字。外部值真的变了（预设填入之类）
/// 才把原文换掉。
class DecimalField extends StatefulWidget {
  const DecimalField({
    super.key,
    required this.value,
    required this.onParsed,
    required this.invalidText,
    this.hint,
    this.width,
  });

  /// 当前值的显示文本（空串 = 未设置）。
  final String value;

  /// 解析成功才调用；null = 用户清空了。
  final ValueChanged<double?> onParsed;

  /// 解析失败时显示的一行提示。
  final String invalidText;

  final String? hint;
  final double? width;

  @override
  State<DecimalField> createState() => _DecimalFieldState();
}

class _DecimalFieldState extends State<DecimalField> {
  late String _text = widget.value;
  bool _invalid = false;

  @override
  void didUpdateWidget(DecimalField old) {
    super.didUpdateWidget(old);
    // 外部值变了才覆盖原文；用户正打着的半截数字不受影响（那时 value 没动）。
    if (widget.value != old.value && widget.value != _text) {
      setState(() {
        _text = widget.value;
        _invalid = false;
      });
    }
  }

  void _onTyped(String v) {
    final trimmed = v.trim();
    if (trimmed.isEmpty) {
      setState(() {
        _text = v;
        _invalid = false;
      });
      widget.onParsed(null);
      return;
    }
    final parsed = double.tryParse(trimmed);
    setState(() {
      _text = v;
      _invalid = parsed == null;
    });
    if (parsed != null) widget.onParsed(parsed);
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final field = PlatformField(
      value: _text,
      hint: widget.hint,
      numeric: true,
      onChanged: _onTyped,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        widget.width == null
            ? field
            : SizedBox(width: widget.width, child: field),
        if (_invalid)
          Text(
            widget.invalidText,
            style: AidogType.caption.copyWith(color: theme.c.bad),
          ),
      ],
    );
  }
}
