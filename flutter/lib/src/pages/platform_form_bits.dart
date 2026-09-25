/// 平台编辑表单的零件层。对应 React 的 `formSections.tsx` 里那几个共用小组件
/// （`FormSection` / `ApiKeyField`）与散在各分区的 input/select 写法。
///
/// 三条规矩与 `settings/bits.dart` 同源：
/// 1. 色值一律 `AidogTheme.of(context).c.*`，零硬编码；
/// 2. `onChanged == null` = 禁用态，就是 React `disabled=` 的直接投影；
/// 3. 受控文本框的 `TextEditingController` 必须活在 State 里（理由见 `groups.dart:727`）。
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../shell/theme.dart';
import '../shell/tiles.dart';
import '../utils/pinyin.dart';
import 'platform_logo.dart';
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
      // 分区卡之间 gap 16（`PlatformEditForm.tsx:138`）。
      padding: const EdgeInsets.only(bottom: 16),
      child: Tile(
        // React FormSection：padding 16、标题→内容 gap 12（formSections.tsx:58-65）。
        padding: const EdgeInsets.all(16),
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
                        // 分区标题 13 w700（`formSections.tsx:62`）。
                        style: AidogType.tile.copyWith(
                          fontSize: 13,
                          fontWeight: FontWeight.w700,
                          color: theme.c.fg,
                        ),
                      ),
                      if (desc != null && desc!.isNotEmpty)
                        Padding(
                          padding: const EdgeInsets.only(top: 2),
                          child: Text(
                            desc!,
                            // 分区描述 11 行高 1.4（`formSections.tsx:64`）。
                            style: AidogType.caption.copyWith(
                              fontSize: 11,
                              height: 1.4,
                              color: theme.c.fg3,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
                if (action != null) ...[
                  // 标题↔右上操作区 gap 8（`formSections.tsx:60`）。
                  const SizedBox(width: 8),
                  action!,
                ],
              ],
            ),
            // React FormSection 的 `gap: 12` 是 flex gap：它作用在标题行与
            // **每一个**子元素之间（`formSections.tsx:58`），不是只插一次。
            //
            // 纯 [SizedBox] 子元素是历史遗留的手插间距（4 / 6 各处不一），
            // 这里一并滤掉 —— 间距由本组件统一给，调用点不再自己补。
            for (final child in children)
              if (child is! SizedBox) ...[
                const SizedBox(height: 12),
                child,
              ],
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
    this.fontSize,
    this.contentPadding,
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

  /// 字号覆盖。null = 主题默认（label 13.5）。协议下拉的搜索框 React 写的是 12。
  final double? fontSize;

  /// 内边距覆盖。null = 主题默认（`.input` 的 8/12）。
  final EdgeInsets? contentPadding;

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
      fontSize: widget.fontSize,
      color: on ? theme.c.fg : theme.c.fg3,
    );
    final field = TextField(
      controller: _ctrl,
      focusNode: widget.focusNode,
      enabled: on,
      keyboardType: widget.numeric
          ? const TextInputType.numberWithOptions(decimal: true)
          : null,
      // 数字键盘在桌面端接了实体键盘就是摆设：照样能敲进字母，然后被上层的
      // `tryParse ?? 0` 静默变成 0。过滤放在这里，`NumberField` / `DecimalField`
      // 以及所有 `numeric: true` 的调用点一次全保住。
      inputFormatters: widget.numeric
          ? [FilteringTextInputFormatter.allow(RegExp(r'[0-9.]'))]
          : null,
      maxLines: widget.obscure ? 1 : widget.maxLines,
      obscureText: widget.obscure,
      style: style,
      decoration: InputDecoration(
        isDense: true,
        contentPadding: widget.contentPadding,
        hintText: widget.hint,
        hintStyle: AidogType.label.copyWith(
          fontSize: widget.fontSize,
          color: theme.c.fg3,
        ),
      ),
      onChanged: widget.onChanged,
    );
    if (widget.label == null) return field;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [FieldLabel(widget.label!), field],
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
      children: [FieldLabel(label!), sized],
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
        // 表单 hint 11、行高 1.5（`formSections.tsx:194,214,234,347,355`）。
        style: AidogType.caption.copyWith(
          fontSize: 11,
          height: 1.5,
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
    this.colors = const {},
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

  /// 协议 → 品牌色（`PlatformDefaults.protocolColorMap`）。logo 缺图时的圆圈底色。
  final Map<String, Color> colors;

  @override
  State<ProtocolPicker> createState() => _ProtocolPickerState();
}

class _ProtocolPickerState extends State<ProtocolPicker> {
  /// 下拉是**浮层**（`SearchableProtocolSelect.tsx:191-198` 的
  /// `position:absolute; top:100%; zIndex:100`）。行内展开会把整张表单往下顶。
  final _link = LayerLink();
  OverlayEntry? _entry;
  bool _open = false;
  String _query = '';

  /// 键盘高亮项下标（`:63` 的 `highlightedIndex`）。
  int _highlight = 0;

  final FocusNode _triggerFocus = FocusNode();
  final FocusNode _searchFocus = FocusNode();
  final ScrollController _listScroll = ScrollController();

  @override
  void dispose() {
    _entry?.remove();
    _entry = null;
    _triggerFocus.dispose();
    _searchFocus.dispose();
    _listScroll.dispose();
    super.dispose();
  }

  /// `SearchableProtocolSelect.tsx:61::labelOf` —— label 不含 Coding 字样时补后缀。
  String _labelOf(String base, bool cp) =>
      cp && !base.toLowerCase().contains('coding') ? '$base Coding Plan' : base;

  List<({String value, String label, bool codingPlan, List<String> terms})>
  get _filtered {
    final q = _query.trim().toLowerCase();
    return [
      for (final p in widget.options)
        if (q.isEmpty ||
            p.terms.any((t) => t.toLowerCase().contains(q)) ||
            p.value.toLowerCase().contains(q))
          p,
    ];
  }

  int get _selectedIndexIn => _filtered.indexWhere(
    (p) => p.value == widget.value && p.codingPlan == widget.codingPlan,
  );

  /// `:86::openDropdown` —— 打开时清空搜索、高亮定位到当前选中项、焦点给搜索框。
  void _openDropdown() {
    if (_open) return;
    _query = '';
    _open = true;
    final idx = _selectedIndexIn;
    _highlight = idx >= 0 ? idx : 0;
    _entry = OverlayEntry(builder: _panel);
    Overlay.of(context).insert(_entry!);
    setState(() {});
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && _open) _searchFocus.requestFocus();
    });
  }

  void _close() {
    if (!_open) return;
    _open = false;
    _query = '';
    _entry?.remove();
    _entry = null;
    if (mounted) setState(() {});
  }

  void _rebuildPanel() => _entry?.markNeedsBuild();

  /// `:75::tabCycle` —— 在**完整列表**里循环切到上 / 下一个协议。
  void _tabCycle(bool shift) {
    final all = widget.options;
    final idx = all.indexWhere(
      (p) => p.value == widget.value && p.codingPlan == widget.codingPlan,
    );
    if (idx < 0 || all.isEmpty) return;
    final next = shift
        ? (idx - 1 + all.length) % all.length
        : (idx + 1) % all.length;
    widget.onChanged(all[next].value, all[next].codingPlan);
  }

  void _pick(({String value, String label, bool codingPlan, List<String> terms}) p) {
    widget.onChanged(p.value, p.codingPlan);
    _close();
  }

  /// 触发器键盘（关闭态）：Tab / Shift+Tab 循环切，↓ / Enter / 空格 打开（`:107-121`）。
  KeyEventResult _onTriggerKey(FocusNode node, KeyEvent e) {
    if (_open || e is! KeyDownEvent) return KeyEventResult.ignored;
    final k = e.logicalKey;
    if (k == LogicalKeyboardKey.tab) {
      _tabCycle(HardwareKeyboard.instance.isShiftPressed);
      return KeyEventResult.handled;
    }
    if (k == LogicalKeyboardKey.arrowDown ||
        k == LogicalKeyboardKey.enter ||
        k == LogicalKeyboardKey.space) {
      _openDropdown();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  /// 搜索框键盘（打开态）：↑↓ 移高亮、Enter 选中、Esc 关（`:124-151`）。
  KeyEventResult _onSearchKey(FocusNode node, KeyEvent e) {
    if (e is! KeyDownEvent) return KeyEventResult.ignored;
    final k = e.logicalKey;
    final list = _filtered;
    if (k == LogicalKeyboardKey.arrowDown) {
      _highlight = (_highlight + 1).clamp(0, list.isEmpty ? 0 : list.length - 1);
      _rebuildPanel();
      return KeyEventResult.handled;
    }
    if (k == LogicalKeyboardKey.arrowUp) {
      _highlight = (_highlight - 1).clamp(0, list.isEmpty ? 0 : list.length - 1);
      _rebuildPanel();
      return KeyEventResult.handled;
    }
    if (k == LogicalKeyboardKey.enter) {
      if (_highlight >= 0 && _highlight < list.length) _pick(list[_highlight]);
      return KeyEventResult.handled;
    }
    if (k == LogicalKeyboardKey.escape) {
      _close();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  Color? _colorOf(String protocol) => widget.colors[protocol];

  Widget _panel(BuildContext overlayContext) {
    final theme = AidogTheme.of(context);
    final filtered = _filtered;
    final box = context.findRenderObject() as RenderBox?;
    final width = box?.size.width ?? 240;
    return Stack(
      children: [
        Positioned.fill(
          child: Listener(
            behavior: HitTestBehavior.translucent,
            onPointerDown: (_) => _close(),
          ),
        ),
        CompositedTransformFollower(
          link: _link,
          targetAnchor: Alignment.bottomLeft,
          followerAnchor: Alignment.topLeft,
          // `marginTop: 4`（`SearchableProtocolSelect.tsx:195`）。
          offset: const Offset(0, 4),
          child: Align(
            alignment: AlignmentDirectional.topStart,
            child: Material(
              type: MaterialType.transparency,
              child: Container(
                width: width,
                // 面板 `padding: 4`、底 var(--popover)=surface（`:195-197`）。
                padding: const EdgeInsets.all(4),
                decoration: BoxDecoration(
                  color: theme.c.surface,
                  border: Border.all(color: theme.c.line),
                  borderRadius: BorderRadius.circular(AidogRadius.sm),
                  boxShadow: theme.shadowFloat,
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    // 搜索框 12 / `6px 10px` / 下距 4（`:208`）。
                    Focus(
                      focusNode: _searchFocus,
                      onKeyEvent: _onSearchKey,
                      child: PlatformField(
                        value: _query,
                        hint: widget.searchHint,
                        fontSize: 12,
                        contentPadding: const EdgeInsets.symmetric(
                          horizontal: 10,
                          vertical: 6,
                        ),
                        onChanged: (v) {
                          _query = v;
                          // 搜索内容变了高亮回第一项（`:104`）。
                          _highlight = 0;
                          _rebuildPanel();
                        },
                      ),
                    ),
                    const SizedBox(height: 4),
                    ConstrainedBox(
                      constraints: const BoxConstraints(maxHeight: 256),
                      child: filtered.isEmpty
                          ? Padding(
                              // 空态 `padding: 8px 12px`、12 tertiary（`:216`）。
                              padding: const EdgeInsets.symmetric(
                                horizontal: 12,
                                vertical: 8,
                              ),
                              child: Text(
                                widget.noMatchText,
                                style: AidogType.caption.copyWith(
                                  fontSize: 12,
                                  color: theme.c.fg3,
                                ),
                              ),
                            )
                          : ListView.builder(
                              controller: _listScroll,
                              shrinkWrap: true,
                              padding: EdgeInsets.zero,
                              itemCount: filtered.length,
                              itemBuilder: (context, i) =>
                                  _option(theme, filtered[i], i),
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

  Widget _option(
    AidogTheme theme,
    ({String value, String label, bool codingPlan, List<String> terms}) p,
    int idx,
  ) {
    final active = p.value == widget.value && p.codingPlan == widget.codingPlan;
    final highlighted = idx == _highlight;
    return InkWell(
      onTap: () => _pick(p),
      onHover: (v) {
        if (!v || _highlight == idx) return;
        _highlight = idx;
        _rebuildPanel();
      },
      borderRadius: BorderRadius.circular(AidogRadius.sm),
      child: Container(
        // 选项 `padding: 7px 12px`、字 13；选中 accent-subtle 底 + accent 字 + w500
        //（`SearchableProtocolSelect.tsx:232-238`）。
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 7),
        decoration: BoxDecoration(
          color: active
              ? theme.c.accentWash
              : highlighted
              ? theme.c.surface2
              : Colors.transparent,
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: Row(
          children: [
            ProtocolLogo(
              protocol: p.value,
              size: 20,
              color: _colorOf(p.value),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                _labelOf(p.label, p.codingPlan),
                overflow: TextOverflow.ellipsis,
                style: AidogType.label.copyWith(
                  fontSize: 13,
                  fontWeight: active ? FontWeight.w500 : FontWeight.w400,
                  color: active ? theme.c.accentText : theme.c.fg,
                ),
              ),
            ),
            if (p.codingPlan) ...[
              const SizedBox(width: 6),
              // Code 角标：`1px 5px`、radius sm、success 20% 底、10 w600（`:256-263`）。
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                decoration: BoxDecoration(
                  color: theme.c.ok.withValues(alpha: 0.20),
                  borderRadius: BorderRadius.circular(AidogRadius.sm),
                ),
                child: Text(
                  'Code',
                  style: AidogType.micro.copyWith(
                    fontSize: 10,
                    letterSpacing: 0,
                    fontWeight: FontWeight.w600,
                    color: theme.c.ok,
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    String selectedLabel = widget.value;
    for (final p in widget.options) {
      if (p.value == widget.value) selectedLabel = p.label;
    }
    return CompositedTransformTarget(
      link: _link,
      child: Focus(
        focusNode: _triggerFocus,
        onKeyEvent: _onTriggerKey,
        child: InkWell(
          onTap: () {
            if (_open) {
              _close();
            } else {
              _triggerFocus.requestFocus();
              _openDropdown();
            }
          },
          child: Container(
            // 触发器是 `.input`：`padding 8px 12px`、圆角 8、底 var(--card)=surface、
            // 字 13（`globals.css:433-450` + `SearchableProtocolSelect.tsx:158-185`）。
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: BoxDecoration(
              color: theme.c.surface,
              border: Border.all(
                color: _open ? theme.c.accentEdge : theme.c.line,
              ),
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: Row(
              children: [
                ProtocolLogo(
                  protocol: widget.value,
                  size: 20,
                  color: _colorOf(widget.value),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    _labelOf(selectedLabel, widget.codingPlan),
                    overflow: TextOverflow.ellipsis,
                    style: AidogType.label.copyWith(
                      fontSize: 13,
                      color: theme.c.fg,
                    ),
                  ),
                ),
                // 箭头是 10px 的 `▼`，展开时转 180°（`:180-185`）。
                AnimatedRotation(
                  turns: _open ? 0.5 : 0,
                  duration: const Duration(milliseconds: 200),
                  child: Text(
                    '▼',
                    style: TextStyle(fontSize: 10, color: theme.c.fg3),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
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
          // 22×22 方块、`padding 0`、11（`formSections.tsx:893`）。
          child: SizedBox(
            width: 22,
            height: 22,
            child: SmallButton(
              label: labels[d],
              padding: (0, 0),
              active: selected.contains(d),
              onTap: () => onToggle(d),
            ),
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
      children: [FieldLabel(label!), sized],
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
///
/// **为什么这里有红线，而 [NumberInput]（`ui_bits.dart:707-725`）没有**：
/// 两者挡的是同一个坑（`tryParse ?? 0` 静默吞数据），但挡在不同位置。
/// - [NumberInput] 在**入口**挡：输入过滤只放数字与一个小数点进来，
///   「10 usd」根本敲不进去，于是「非法值」这个状态压根不存在，也就没什么
///   可提示的。
/// - [DecimalField] 挡不了入口：它收的是小数，而**打到一半的 `10.` 必须放行**
///   （不放行就打不出 `10.5`），所以它一定会经过一段既不合法、也不能丢的
///   中间态。这段中间态要么显示出来，要么就得偷偷替用户决定 —— 后者正是
///   那个 bug。红线就是「这一刻我没有采信你输入的值」的唯一告知。
///
/// 换句话说：能在入口过滤干净的用 [NumberInput]，过滤不干净、必须容忍中间态的
/// 用这个。别给 [NumberInput] 加红线（它永远不会红），也别把这里的红线省掉。
///
/// 对侧那半写在 `ui_bits.dart:720-725`（`NumberInput` 文档注释末尾）——
/// 那句话是「差别是故意的，不是漂移」。**改动任一侧前先读另一侧。**
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
