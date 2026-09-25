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
    this.height = 30,
    this.padX = AidogSpace.smd,
    this.fontSize,
  });

  /// 触发钮高度 / 水平内衬 / 字号。缺省 30 / 10 / caption 12.5；模型信息页的
  /// `SelectTrigger` 是 `height 32, padding "6px 8px", fontSize 12`
  /// （`src/pages/ModelInfo/ModelInfoTab.tsx:225,236`）。
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
                padding: const EdgeInsets.all(AidogSpace.ssm),
                decoration: BoxDecoration(
                  color: t.c.surface2,
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
                          style: AidogType.label.copyWith(color: t.c.fg),
                          decoration: InputDecoration(
                            isDense: true,
                            hintText: widget.searchPlaceholder,
                            hintStyle: AidogType.label.copyWith(color: t.c.fg3),
                            contentPadding: const EdgeInsets.symmetric(
                              horizontal: AidogSpace.ssm,
                              vertical: AidogSpace.ssm,
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
                        const SizedBox(height: AidogSpace.ssm),
                        Flexible(
                          child: ListView(
                            shrinkWrap: true,
                            children: [
                              _row(t, widget.allLabel, widget.value.isEmpty, ''),
                              if (hits.isEmpty)
                                Padding(
                                  padding: const EdgeInsets.symmetric(
                                    horizontal: AidogSpace.smd,
                                    vertical: AidogSpace.ssm,
                                  ),
                                  child: Text(
                                    widget.emptyLabel,
                                    style: AidogType.caption.copyWith(
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
    return InkWell(
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
          style: AidogType.label.copyWith(
            color: active ? t.c.accentText : t.c.fg,
            fontWeight: active ? FontWeight.w500 : FontWeight.w400,
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
            color: t.c.surface2,
            border: Border.all(color: open ? t.c.accentEdge : t.c.line),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
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
              const SizedBox(width: AidogSpace.ssm),
              Icon(
                open ? Icons.arrow_drop_up : Icons.arrow_drop_down,
                size: 16,
                color: t.c.fg3,
              ),
            ],
          ),
        ),
      ),
    );
  }
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
