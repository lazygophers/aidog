/// 三张热力图（对应 React 版 `HourHeatmap` / `HourHeatBar` / `DimensionHeatmap`）。
///
/// **这三个没有用过图表库**，React 那边自己就是 CSS grid 的 div 格子
/// （文件头写着「零 canvas 零 Recharts」）。Flutter 这边同理：`GridView` 而不是 fl_chart。
/// 色带复用 [ChartPalette.heat]（固定琥珀 alpha 阶梯，端点与 React 版一字不差）。
library;

import 'package:flutter/material.dart';

import '../../utils/formatters.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'empty.dart';
import 'palette.dart';

/// 行序：周一…周日（输入 day 仍按 JS `getDay` 约定，0 = 周日）。
const List<int> kDayRows = [1, 2, 3, 4, 5, 6, 0];

/// 顶部小时刻度只标 0/6/12/18，其余留空防挤。
const Set<int> kHourLabelAt = {0, 6, 12, 18};

/// 热力图轴 / 行名字样：React 两张图的容器都是 `fontSize: 9` + `--text-tertiary`
/// + `tabular-nums`（`HourHeatmap.tsx:57-59`、`DimensionHeatmap.tsx:64-66`），
/// 字体族是系统 sans，字距 0 —— `AidogType.micro` 的 11 + ls0.66 在 React 侧无对应物。
TextStyle heatAxisStyle(AidogColors c) =>
    counterStyle(fontSize: 9, color: c.fg3, fontWeight: FontWeight.w400);

/// 把格子压成正方形（React 的 `aspectRatio: "1"`，
/// `HourHeatmap.tsx:105` / `DimensionHeatmap.tsx:129`）。
///
/// `Align` 先把紧约束放松，`AspectRatio` 才有取边长的自由 —— 直接套在
/// `Expanded` 下会被紧约束原样顶回去。
class _SquareCell extends StatelessWidget {
  const _SquareCell({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) => Align(
    alignment: AlignmentDirectional.topStart,
    child: AspectRatio(aspectRatio: 1, child: child),
  );
}

/// 单个热力格。`tooltip` 走 Flutter 的 [Tooltip]（对应 React 的 `title` 属性）。
class HeatCell extends StatelessWidget {
  const HeatCell({super.key, required this.t, required this.tooltip});

  /// 归一化热度 [0,1]。
  final double t;
  final String tooltip;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      waitDuration: const Duration(milliseconds: 300),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: ChartPalette.of(context).heat(t),
          borderRadius: BorderRadius.circular(2),
        ),
      ),
    );
  }
}

/// 时刻热力图：24 小时 × 7 天 = 168 格。
class HourHeatmap extends StatelessWidget {
  const HourHeatmap({
    super.key,
    required this.data,
    required this.dayLabel,
    this.formatValue = _plain,
    this.emptyText = '',
    this.emptyHint,
  });

  /// 格子数据：day 0-6（JS getDay，0 = 周日）、hour 0-23、value 非负。缺格按 0。
  final List<({int day, int hour, double value})> data;

  /// 星期名（本地化归票 I03，这里只收 `0-6 → 串`）。
  final String Function(int day) dayLabel;
  final String Function(double) formatValue;
  final String emptyText;
  final String? emptyHint;

  @override
  Widget build(BuildContext context) {
    if (data.isEmpty) return ChartEmpty(emptyText, hint: emptyHint);
    final t = AidogTheme.of(context);
    final cells = {for (final d in data) d.day * 24 + d.hour: d.value};
    final max = data.fold<double>(0, (m, d) => d.value > m ? d.value : m);
    final axis = heatAxisStyle(t.c);

    Widget row(List<Widget> children) => Expanded(
      child: Padding(
        padding: const EdgeInsets.only(bottom: 2),
        child: Row(
          children: [
            SizedBox(
              // 星期名列 `3.5em` @9px = 31.5（`HourHeatmap.tsx:55`）。
              width: 31.5,
              child: Align(
                alignment: Alignment.centerLeft,
                child: children.first,
              ),
            ),
            for (final c in children.skip(1))
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(left: 2),
                  child: c,
                ),
              ),
          ],
        ),
      ),
    );

    return Column(
      children: [
        row([
          const SizedBox.shrink(),
          for (var h = 0; h < 24; h++)
            Center(
              child: Text(kHourLabelAt.contains(h) ? pad(h) : '', style: axis),
            ),
        ]),
        for (final d in kDayRows)
          row([
            Text(dayLabel(d), style: axis),
            for (var h = 0; h < 24; h++)
              _SquareCell(
                child: HeatCell(
                  t: max > 0 ? (cells[d * 24 + h] ?? 0) / max : 0,
                  tooltip:
                      '${dayLabel(d)} ${pad(h)}:00 · ${formatValue(cells[d * 24 + h] ?? 0)}',
                ),
              ),
          ]),
      ],
    );
  }
}

/// 迷你热力条（托盘浮窗）：今日 0-23 时单行紧凑横条，恒 24 格。
/// 越界小时（<0 或 ≥24）直接忽略，不崩。
class HourHeatBar extends StatelessWidget {
  const HourHeatBar({
    super.key,
    required this.data,
    this.formatValue = _plain,
    this.semanticLabel,
  });

  final List<({int hour, double value})> data;
  final String Function(double) formatValue;
  final String? semanticLabel;

  @override
  Widget build(BuildContext context) {
    final values = List<double>.filled(24, 0);
    for (final d in data) {
      if (d.hour >= 0 && d.hour < 24) values[d.hour] = d.value;
    }
    final max = values.fold<double>(0, (m, v) => v > m ? v : m);
    return Semantics(
      label: semanticLabel,
      image: true,
      child: SizedBox(
        height: 12,
        child: Row(
          children: [
            for (var h = 0; h < 24; h++)
              Expanded(
                child: Padding(
                  padding: EdgeInsets.only(left: h == 0 ? 0 : 2),
                  child: HeatCell(
                    t: max > 0 ? values[h] / max : 0,
                    tooltip: '${pad(h)}:00 · ${formatValue(values[h])}',
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

/// 维度热力图：维度（行）× 时间（列）。行 = 首现序（调用方已按总量降序），列 = 时间升序。
class DimensionHeatmap extends StatelessWidget {
  const DimensionHeatmap({
    super.key,
    required this.data,
    this.formatValue = _plain,
    this.formatDay = defaultFormatDay,
    this.emptyText = '',
    this.emptyHint,
  });

  /// 格子数据：name 维度值（行）、day 列时间（ms 时间戳）、value 非负。缺格按 0。
  final List<({String name, double day, double value})> data;
  final String Function(double) formatValue;
  final String Function(double ms) formatDay;
  final String emptyText;
  final String? emptyHint;

  /// 缺省列标签：本地时区 `MM-DD`（与 React 版同）。
  static String defaultFormatDay(double ms) {
    final d = DateTime.fromMillisecondsSinceEpoch(ms.toInt());
    return '${pad(d.month)}-${pad(d.day)}';
  }

  @override
  Widget build(BuildContext context) {
    if (data.isEmpty) return ChartEmpty(emptyText, hint: emptyHint);
    final t = AidogTheme.of(context);
    final names = <String>[];
    for (final d in data) {
      if (!names.contains(d.name)) names.add(d.name);
    }
    final days = data.map((d) => d.day).toSet().toList()..sort();
    final cells = {for (final d in data) (d.name, d.day): d.value};
    final max = data.fold<double>(0, (m, d) => d.value > m ? d.value : m);
    // 列刻度稀疏标注：只标每 labelEvery 列，防 "MM-DD" 串挤叠。
    final labelEvery = (days.length / 10).ceil().clamp(1, 1 << 30);
    final axis = heatAxisStyle(t.c);

    Widget row(Widget head, List<Widget> cols) => Expanded(
      child: Padding(
        padding: const EdgeInsets.only(bottom: 2),
        child: Row(
          children: [
            // 维度名列 `minmax(6em, 12em)` @9px = 54–108 弹性
            //（`DimensionHeatmap.tsx:62`）。这里取弹性下限：Row 里
            // 没有 grid 的 minmax，固定 54 让其余宽度全给格子。
            SizedBox(width: 54, child: head),
            for (final c in cols)
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(left: 2),
                  child: c,
                ),
              ),
          ],
        ),
      ),
    );

    return Column(
      children: [
        row(const SizedBox.shrink(), [
          for (var i = 0; i < days.length; i++)
            Center(
              child: Text(
                i % labelEvery == 0 ? formatDay(days[i]) : '',
                style: axis,
                maxLines: 1,
                overflow: TextOverflow.clip,
              ),
            ),
        ]),
        for (final name in names)
          row(
            Tooltip(
              message: name,
              child: Align(
                alignment: AlignmentDirectional.centerStart,
                child: Text(
                  name,
                  style: axis,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
            [
              for (final d in days)
                _SquareCell(
                  child: HeatCell(
                    t: max > 0 ? (cells[(name, d)] ?? 0) / max : 0,
                    tooltip:
                        '$name ${formatDay(d)} · ${formatValue(cells[(name, d)] ?? 0)}',
                  ),
                ),
            ],
          ),
      ],
    );
  }
}

/// 缺省数值格式化：原值（对应 React 的 `String(n)`；整数不带 `.0`）。
String _plain(double n) => n == n.roundToDouble() ? '${n.toInt()}' : '$n';
