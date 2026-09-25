/// 带搜索的筛选下拉（对应 React 版 `src/components/shared/FilterDropdown.tsx`）。
///
/// 行为逐条对齐：触发钮显示当前项的 label（无值显示「全部…」）；展开后第一行永远是
/// 「全部…」（不参与过滤）；搜索框自动聚焦；过滤后为空显示 `emptyLabel`；
/// 选中或关闭时清空搜索词。
///
/// label 匹配走 [pinyinMatch]（自建 3500 常用字词典，语义同 React 的
/// `pinyin-pro` 版）：拼音 / 首字母 / 中文 / 中英混合都能搜「分组」这类用户自起中文名。
/// [searchTerms]（registry 协议词条）仍是纯子串 —— 拼音与首字母已作为字面数据存着。
library;

import 'dart:convert';

import 'package:flutter/material.dart';

import '../shell/theme.dart';
import '../utils/pinyin.dart';
import 'invoke.dart';

/// 一个可选项。[searchTerms] 是 label 之外的跨语言搜索词（registry 协议词条）。
@immutable
class FilterOption {
  const FilterOption({
    required this.value,
    required this.label,
    this.searchTerms = const [],
  });

  final String value;
  final String label;
  final List<String> searchTerms;
}

/// 搜索过滤。空查询（含全空格）返回全部；否则 label 拼音模糊命中、或任一
/// searchTerm 子串命中即保留。
List<FilterOption> filterOptions(String query, List<FilterOption> options) {
  final q = query.trim();
  if (q.isEmpty) return options;
  return [
    for (final o in options)
      if (pinyinMatch(q, o.label) ||
          o.searchTerms.any((t) => t.toLowerCase().contains(q.toLowerCase())))
        o,
  ];
}

class FilterDropdown extends StatefulWidget {
  const FilterDropdown({
    super.key,
    required this.width,
    required this.value,
    required this.onChanged,
    required this.allLabel,
    required this.searchPlaceholder,
    required this.options,
    required this.emptyLabel,
    this.height = 36,
    this.padX = 16,
    this.fontSize = 14,
  });

  /// 触发钮高度 / 水平内衬 / 字号。缺省 36 / 16 / 14 —— React 的
  /// `style={{ fontSize: 14, lineHeight: 1.5, height: 36 }}` + shadcn `<Button>`
  /// 缺省 `px-4`（`src/components/shared/FilterDropdown.tsx:50`、`ui/button.tsx`）。
  /// 模型信息页的 `SelectTrigger` 是 `height 32, padding "6px 8px", fontSize 12`
  /// （`src/pages/ModelInfo/ModelInfoTab.tsx:225,236`），那边逐项覆盖。
  final double height;
  final double padX;
  final double? fontSize;

  final double width;

  /// 当前选中值；空串 = 「全部」。
  final String value;
  final ValueChanged<String> onChanged;
  final String allLabel;
  final String searchPlaceholder;
  final List<FilterOption> options;

  /// 搜索无匹配时的空态文案。
  final String emptyLabel;

  @override
  State<FilterDropdown> createState() => _FilterDropdownState();
}

class _FilterDropdownState extends State<FilterDropdown> {
  final _link = LayerLink();
  final _search = TextEditingController();
  OverlayEntry? _entry;

  @override
  void dispose() {
    _close();
    _search.dispose();
    super.dispose();
  }

  void _close() {
    _entry?.remove();
    _entry = null;
    _search.text = '';
  }

  void _toggle() {
    if (_entry != null) {
      setState(_close);
      return;
    }
    _entry = OverlayEntry(builder: _panel);
    Overlay.of(context).insert(_entry!);
    setState(() {});
  }

  void _pick(String v) {
    widget.onChanged(v);
    setState(_close);
  }

  Widget _panel(BuildContext overlayContext) {
    final t = AidogTheme.of(context);
    final width = widget.width < 320 ? 320.0 : widget.width;
    return Stack(
      children: [
        // 面板外任意点击即关闭（对应 Radix Popover 的 modal 行为）。
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.translucent,
            onTap: () => setState(_close),
          ),
        ),
        CompositedTransformFollower(
          link: _link,
          targetAnchor: Alignment.bottomLeft,
          followerAnchor: Alignment.topLeft,
          offset: const Offset(0, AidogSpace.sxs),
          child: Align(
            alignment: AlignmentDirectional.topStart,
            child: Material(
              // 只为 InkWell 提供墨层，底色仍由下面 Container 的 token 决定。
              type: MaterialType.transparency,
              child: Container(
                width: width,
                constraints: const BoxConstraints(maxHeight: 320),
                // 浮层 `padding: 8` + `bg-popover`（`FilterDropdown.tsx:78-79`）；
                // `--bg-glass` / `--card` / `bg-popover` 在 Flutter 一律是 surface。
                padding: const EdgeInsets.all(AidogSpace.s_8),
                decoration: BoxDecoration(
                  color: t.c.surface,
                  border: Border.all(color: t.c.line),
                  borderRadius: BorderRadius.circular(AidogRadius.md),
                  boxShadow: t.shadowFloat,
                ),
                child: ValueListenableBuilder<TextEditingValue>(
                  valueListenable: _search,
                  builder: (context, value, _) {
                    final hits = filterOptions(value.text, widget.options);
                    return Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        TextField(
                          controller: _search,
                          autofocus: true,
                          // 搜索框 `<Input style={{ fontSize: 14 }}>`，内衬走
                          // `.input` 的 `padding: 8px 12px`
                          //（`FilterDropdown.tsx:81-87` + `globals.css:433-450`）。
                          style: AidogType.label.copyWith(
                            fontSize: 14,
                            color: t.c.fg,
                          ),
                          decoration: InputDecoration(
                            isDense: true,
                            hintText: widget.searchPlaceholder,
                            hintStyle: AidogType.label.copyWith(
                              fontSize: 14,
                              color: t.c.fg3,
                            ),
                            contentPadding: const EdgeInsets.symmetric(
                              horizontal: 12,
                              vertical: AidogSpace.s_8,
                            ),
                            enabledBorder: OutlineInputBorder(
                              borderSide: BorderSide(color: t.c.line),
                              borderRadius: BorderRadius.circular(
                                AidogRadius.sm,
                              ),
                            ),
                            focusedBorder: OutlineInputBorder(
                              borderSide: BorderSide(color: t.c.accentEdge),
                              borderRadius: BorderRadius.circular(
                                AidogRadius.sm,
                              ),
                            ),
                          ),
                        ),
                        // 搜索框↔列表 `gap: 6`（`FilterDropdown.tsx:79`）。
                        const SizedBox(height: AidogSpace.ssm),
                        Flexible(
                          // 列表自身 `maxHeight: 250`（`FilterDropdown.tsx:88`），
                          // 与浮层的 320 上限是两道，不是一道。
                          child: ConstrainedBox(
                            constraints: const BoxConstraints(maxHeight: 250),
                            child: ListView(
                              shrinkWrap: true,
                              children: [
                                _row(
                                  t,
                                  widget.allLabel,
                                  widget.value.isEmpty,
                                  '',
                                ),
                                if (hits.isEmpty)
                                  Padding(
                                    // 空态 `fontSize: 12, padding: "6px 8px"`
                                    //（`FilterDropdown.tsx:91`）。
                                    padding: const EdgeInsets.symmetric(
                                      horizontal: AidogSpace.s_8,
                                      vertical: AidogSpace.ssm,
                                    ),
                                    child: Text(
                                      widget.emptyLabel,
                                      style: AidogType.caption.copyWith(
                                        fontSize: 12,
                                        color: t.c.fg3,
                                      ),
                                    ),
                                  )
                                else
                                  for (final o in hits)
                                    _row(
                                      t,
                                      o.label,
                                      widget.value == o.value,
                                      o.value,
                                    ),
                              ],
                            ),
                          ),
                        ),
                      ],
                    );
                  },
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _row(AidogTheme t, String label, bool active, String value) {
    return Padding(
      // 选项之间 `gap: 2`（`FilterDropdown.tsx:88`）。
      padding: const EdgeInsets.only(bottom: 2),
      child: InkWell(
        onTap: () => _pick(value),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        child: Container(
          padding: const EdgeInsets.symmetric(
            horizontal: AidogSpace.smd,
            vertical: AidogSpace.ssm,
          ),
          decoration: BoxDecoration(
            color: active ? t.c.accentWash : null,
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Text(
            label,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            // 选项 `fontSize: 14, lineHeight: 1.5`（`FilterDropdown.tsx:129`）。
            style: AidogType.label.copyWith(
              fontSize: 14,
              color: active ? t.c.accentText : t.c.fg,
              fontWeight: active ? FontWeight.w500 : FontWeight.w400,
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final current = widget.options
        .where((o) => o.value == widget.value)
        .firstOrNull;
    final open = _entry != null;
    return CompositedTransformTarget(
      link: _link,
      child: InkWell(
        onTap: _toggle,
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        child: Container(
          width: widget.width,
          height: widget.height,
          padding: EdgeInsets.symmetric(horizontal: widget.padX),
          decoration: BoxDecoration(
            // `bg-card`（`FilterDropdown.tsx:48`）= Flutter 的 surface。
            color: t.c.surface,
            border: Border.all(color: open ? t.c.accentEdge : t.c.line),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
            // 展开时 `shadow-[0_0_0_3px_var(--accent-subtle)]` 外发光（同上 :48）：
            // 零模糊、零偏移、外扩 3 —— 是光环不是投影。
            boxShadow: open
                ? [
                    BoxShadow(
                      color: t.c.accentWash,
                      spreadRadius: 3,
                      blurRadius: 0,
                    ),
                  ]
                : null,
          ),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  current?.label ?? widget.allLabel,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AidogType.caption.copyWith(
                    fontSize: widget.fontSize,
                    color: t.c.fg,
                  ),
                ),
              ),
              // 箭头是 8 宽 × 5 高的 CSS 三角 + `marginLeft: 8`，展开时
              // `rotate(180deg)` 200ms 过渡（`FilterDropdown.tsx:57-69`）——
              // Material 的 arrow_drop_down 是另一个形状，且切图标没有过渡。
              const SizedBox(width: AidogSpace.s_8),
              AnimatedRotation(
                turns: open ? 0.5 : 0,
                duration: const Duration(milliseconds: 200),
                child: CustomPaint(
                  size: const Size(8, 5),
                  painter: _CaretPainter(t.c.fg3),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// 下三角箭头：8 宽 × 5 高实心三角，对应 React 用 border 拼出来的 CSS 三角
/// （`src/components/shared/FilterDropdown.tsx:57-69`）。
class _CaretPainter extends CustomPainter {
  const _CaretPainter(this.color);

  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final path = Path()
      ..moveTo(0, 0)
      ..lineTo(size.width, 0)
      ..lineTo(size.width / 2, size.height)
      ..close();
    canvas.drawPath(path, Paint()..color = color);
  }

  @override
  bool shouldRepaint(_CaretPainter old) => old.color != color;
}

/// 协议跨语言搜索词（registry 的 8 locale name + keywords）。
/// 对应 React 版 `defaults.ts::getProtocolSearchTermsMap`；统计页与日志页共用一份，
/// 拉不到就返回空表（下拉退回只按 label 搜，不该连页面一起垮）。
Future<Map<String, List<String>>> loadProtocolTerms(InvokeFn invoke) async {
  try {
    final raw = await invoke('get_defaults_json');
    final doc = jsonDecode(raw! as String) as Map<String, dynamic>;
    final protocols = (doc['protocols'] as Map<String, dynamic>?) ?? const {};
    final out = <String, List<String>>{};
    for (final entry in protocols.entries) {
      final e = entry.value as Map<String, dynamic>?;
      if (e == null) continue;
      final names = (e['name'] as Map<String, dynamic>?) ?? const {};
      final terms = <String>{
        for (final v in names.values)
          if (v is String && v.trim().isNotEmpty) v,
        for (final k in (e['keywords'] as List?) ?? const [])
          if (k is String) k,
      };
      // resolveName 的三层回落：没有任何可用名字时落到协议 code 本身。
      if (names.values.whereType<String>().every((v) => v.trim().isEmpty)) {
        terms.add(entry.key);
      }
      out[entry.key] = terms.toList(growable: false);
    }
    return out;
  } catch (e) {
    debugPrint('get_defaults_json failed: $e');
    return const {};
  }
}
