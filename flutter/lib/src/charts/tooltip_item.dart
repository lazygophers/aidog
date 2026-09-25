/// [TooltipRow] → fl_chart 的文本模型。
///
/// 单独一个文件是因为 `tooltip.dart` 刻意不依赖 fl_chart（行模型要能脱离图表单测，
/// 序列身份的断言都写在那一层）。这里只管排版：`■ 名字  值`。
library;

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';

import '../shell/theme.dart';
import 'tooltip.dart';

/// [header] 只挂第一行 —— fl_chart 每个触点一行，没有独立表头槽。
LineTooltipItem chartTooltipItem(
  TooltipRow row, {
  String? header,
  required AidogColors c,
}) => LineTooltipItem(
  header == null ? '' : '$header\n',
  // 盒内一律 `text-xs` = 12（`ui/chart.tsx:186`），不是 caption 的 12.5。
  AidogType.caption.copyWith(fontSize: 12, color: c.fg3),
  textAlign: TextAlign.left,
  children: [
    TextSpan(
      text: '$kTooltipSwatch ',
      style: AidogType.caption.copyWith(fontSize: 12, color: row.color),
    ),
    TextSpan(
      text: '${row.label}  ',
      style: AidogType.caption.copyWith(fontSize: 12, color: c.fg2),
    ),
    TextSpan(
      // 数值行 `font-mono font-medium tabular-nums`（`charts/tooltip.tsx:55`）：
      // 等宽 + **w500**，numSm 缺省是 w400。
      text: row.value,
      style: AidogType.numSm.copyWith(
        fontSize: 12,
        fontWeight: FontWeight.w500,
        color: c.fg,
        fontFeatures: const [FontFeature.tabularFigures()],
      ),
    ),
  ],
);
