/// 行内窄下拉：与标签同排时用（`SelectRow` 是标签在上、独占一行的形态，
/// 这些位置在 `Wrap` 里跟别的控件挤同一行）。对齐 React 的窄 `Select`。
library;

import 'package:flutter/material.dart';

import '../shell/theme.dart';

class MiniSelect extends StatelessWidget {
  const MiniSelect({
    super.key,
    required this.value,
    required this.options,
    required this.onChanged,
    this.labelOf,
    this.fontSize,
  });

  final String value;
  final List<String> options;
  final ValueChanged<String?> onChanged;
  final String Function(String option)? labelOf;

  /// 字号覆盖。null = 缺省 micro 11；Skills / MCP 的 `SelectTrigger` React 显式
  /// 写 13（`SkillsView.tsx:264,299`、`SkillModals.tsx:146`、`McpModals.tsx:263`）。
  final double? fontSize;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    // 当前值不在选项里（用户手填的自定义值）也要能显示，否则 Dropdown 会断言失败。
    final items = <String>[
      if (value.isNotEmpty && !options.contains(value)) value,
      ...options,
    ];
    return DropdownButton<String>(
      value: value.isEmpty ? null : value,
      underline: const SizedBox.shrink(),
      isDense: true,
      dropdownColor: theme.c.surface2,
      style: AidogType.micro.copyWith(
        color: theme.c.fg,
        fontSize: fontSize,
        letterSpacing: fontSize == null ? null : 0,
      ),
      onChanged: onChanged,
      items: [
        for (final o in items)
          DropdownMenuItem<String>(
            value: o,
            child: Text(labelOf?.call(o) ?? o),
          ),
      ],
    );
  }
}
