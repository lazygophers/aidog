/// 环形图（对应 React 版 `src/components/charts/DonutChart.tsx`）。
///
/// topN + 「其他」合并、首位主色、中央总值、侧列占比图例。
/// 单一有效扇区（或全空）构不成占比图 → 诚实空态，与 React 版同判据。
library;

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';

import '../../utils/formatters.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'empty.dart';
import 'palette.dart';

/// 一块扇区。[percent] 是 0–100 的百分数（与 React 版同量纲）。
@immutable
class DonutSlice {
  const DonutSlice({
    required this.name,
    required this.value,
    required this.color,
    required this.percent,
  });

  final String name;
  final double value;
  final Color color;
  final double percent;
}

/// 纯函数：原始占比数据 → 扇区列表（可脱离 widget 单测）。
/// - 只留 value > 0 的项，按 value 降序
/// - 前 [topN] 单列，其余合并成一块，名字用 [restLabel]
/// - 颜色按序走 [palette]：首位主色、其后灰阶
List<DonutSlice> donutSlices(
  List<({String name, double value})> data,
  ChartPalette palette, {
  int topN = 4,
  required String restLabel,
}) {
  final sorted = data.where((d) => d.value > 0).toList()
    ..sort((a, b) => b.value.compareTo(a.value));
  if (sorted.isEmpty) return const [];
  final total = sorted.fold<double>(0, (s, d) => s + d.value);
  final items = <({String name, double value})>[...sorted.take(topN)];
  final rest = sorted.skip(topN).fold<double>(0, (s, d) => s + d.value);
  if (rest > 0) items.add((name: restLabel, value: rest));
  return [
    for (var i = 0; i < items.length; i++)
      DonutSlice(
        name: items[i].name,
        value: items[i].value,
        color: palette.series(i),
        percent: items[i].value / total * 100,
      ),
  ];
}

class AidogDonutChart extends StatefulWidget {
  const AidogDonutChart({
    super.key,
    required this.data,
    required this.restLabel,
    required this.formatValue,
    this.topN = 4,
    this.centerLabel,
    this.size = 200,
    this.emptyText = '',
    this.emptyHint,
    this.mini = false,
    this.showLegend = true,
  });

  final List<({String name, double value})> data;

  /// 「其他」那块的文案（i18n 归票 I03）。
  final String restLabel;
  final String Function(double) formatValue;
  final int topN;

  /// 中央总值下的说明字。
  final String? centerLabel;
  final double size;
  final String emptyText;
  final String? emptyHint;

  /// 迷你模式：扇区不足两块时返回空盒（空态归调用方），与 React 版返回 null 同义。
  final bool mini;
  final bool showLegend;

  @override
  State<AidogDonutChart> createState() => _AidogDonutChartState();
}

class _AidogDonutChartState extends State<AidogDonutChart> {
  /// 指针所在扇区。-1 = 没碰到。碰到时中央的总值换成那块的名称 / 值 / 占比 ——
  /// React 那边是跟随指针的 tooltip（`DonutChart.tsx`），信息一样，
  /// 落点换成环心：Flutter 这版是桌面 + 触屏通用，跟随指针的浮层在触屏上够不着。
  int _touched = -1;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final data = widget.data;
    final restLabel = widget.restLabel;
    final formatValue = widget.formatValue;
    final topN = widget.topN;
    final centerLabel = widget.centerLabel;
    final size = widget.size;
    final emptyText = widget.emptyText;
    final emptyHint = widget.emptyHint;
    final mini = widget.mini;
    final showLegend = widget.showLegend;
    final slices = donutSlices(
      data,
      ChartPalette(t.c),
      topN: topN,
      restLabel: restLabel,
    );
    if (slices.length < 2) {
      return mini
          ? const SizedBox.shrink()
          : ChartEmpty(emptyText, hint: emptyHint);
    }
    final total = slices.fold<double>(0, (s, d) => s + d.value);

    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        SizedBox(
          width: size,
          height: size,
          child: Stack(
            alignment: Alignment.center,
            children: [
              PieChart(
                PieChartData(
                  sectionsSpace: 2,
                  centerSpaceRadius: size * 0.31, // 内径 62% 直径的一半
                  // React `<Pie>` 不传 `startAngle`，recharts 缺省 0° =
                  // **3 点方向**起画（`DonutChart.tsx:101-110`）；fl_chart 的
                  // `startDegreeOffset` 缺省 0 同样是 3 点，所以这里不再偏移。
                  // 旋向（fl_chart 恒顺时针 / recharts 逆时针）框架不可调。
                  // 原先只写了 `enabled: true` 却没给 touchCallback ——
                  // 等于开了个没人接的开关，碰扇区什么都不出。
                  pieTouchData: PieTouchData(
                    enabled: true,
                    touchCallback: (event, response) {
                      final i =
                          response?.touchedSection?.touchedSectionIndex ?? -1;
                      // 抬手 / 移出即复位，否则中央会一直停在最后碰过的那块。
                      final next =
                          event is FlPointerExitEvent ||
                              event is FlTapUpEvent ||
                              event is FlLongPressEnd ||
                              event is FlPanEndEvent
                          ? -1
                          : i;
                      if (next != _touched) setState(() => _touched = next);
                    },
                  ),
                  sections: [
                    for (var i = 0; i < slices.length; i++)
                      PieChartSectionData(
                        value: slices[i].value,
                        color: slices[i].color,
                        // 碰到的那块鼓出来一点，指哪块一眼可见。
                        radius: size * (i == _touched ? 0.155 : 0.13),
                        showTitle: false,
                      ),
                  ],
                ),
              ),
              // 碰到某块时中央换成「名称 / 值 / 占比」，松手回到总值。
              Builder(
                builder: (context) {
                  final hit = _touched >= 0 && _touched < slices.length
                      ? slices[_touched]
                      : null;
                  final pct = hit == null || total <= 0
                      ? ''
                      : '${(hit.value / total * 100).toStringAsFixed(1)}%';
                  return Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      // 中央总值：React `fontSize: mini ? 12 : 15, fontWeight: 600`
                      // + `tabular-nums`，字体族是系统 sans（`DonutChart.tsx:128`），
                      // 不是等宽 —— 所以走 counterStyle 而不是 numMd/numSm。
                      Text(
                        formatValue(hit?.value ?? total),
                        style: counterStyle(
                          fontSize: mini ? 12 : 15,
                          color: t.c.fg,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      if (hit != null) ...[
                        SizedBox(
                          width: size * 0.5,
                          child: Text(
                            hit.name,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            textAlign: TextAlign.center,
                            style: _centerHintStyle(mini, t.c.fg2),
                          ),
                        ),
                        Text(pct, style: _centerHintStyle(mini, t.c.fg3)),
                      ] else if (centerLabel != null)
                        Text(
                          centerLabel,
                          style: _centerHintStyle(mini, t.c.fg3),
                        ),
                    ],
                  );
                },
              ),
            ],
          ),
        ),
        if (showLegend) ...[
          // 图例↔环 `gap: mini ? 12 : 24`（`DonutChart.tsx:88`）。
          SizedBox(width: mini ? 12 : AidogSpace.s_2xl),
          // React：`flex: 1; minWidth: mini ? 120 : 180`（`DonutChart.tsx:137`）。
          //
          // 外层的 `flexWrap: "wrap"`（同上 :88，窄容器时图例掉到环下方）**没做**：
          // 要知道剩余宽度只能上 `LayoutBuilder`，而它不支持 dry layout ——
          // 托盘浮窗的卡片外面是 `IntrinsicHeight`（`popover/cards.dart:154`），
          // 一放进去整张卡就崩。`Wrap` 也不行：它给子项的是松约束，
          // 里面的 `Expanded`（百分比靠它右对齐）没有确定宽度可用。
          Expanded(
            child: ConstrainedBox(
              constraints: BoxConstraints(minWidth: mini ? 120 : 180),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (final d in slices)
                    Container(
                      // React `padding: mini ? "3px 0" : "5px 0"`
                      //（`DonutChart.tsx:145`）。
                      padding: EdgeInsets.symmetric(vertical: mini ? 3 : 5),
                      decoration: BoxDecoration(
                        border: Border(bottom: BorderSide(color: t.c.line)),
                      ),
                      child: Row(
                        children: [
                          Container(
                            width: 8,
                            height: 8,
                            decoration: BoxDecoration(
                              color: d.color,
                              borderRadius: BorderRadius.circular(2),
                            ),
                          ),
                          // React 行内 `gap: 8`（`DonutChart.tsx:144`）。
                          const SizedBox(width: AidogSpace.s_8),
                          Expanded(
                            child: Text(
                              d.name,
                              overflow: TextOverflow.ellipsis,
                              maxLines: 1,
                              // 图例容器 `fontSize: mini ? 11 : 12`
                              //（`DonutChart.tsx:137`）。
                              style: AidogType.caption.copyWith(
                                fontSize: mini ? 11 : 12,
                                color: t.c.fg,
                              ),
                            ),
                          ),
                          Text(
                            formatPercent(d.percent),
                            // 百分比继承同一档，只多 tabular-nums（系统 sans）。
                            style: counterStyle(
                              fontSize: mini ? 11 : 12,
                              color: t.c.fg2,
                              fontWeight: FontWeight.w400,
                            ),
                          ),
                        ],
                      ),
                    ),
                ],
              ),
            ),
          ),
        ],
      ],
    );
  }
}

/// 环心说明字：React `fontSize: mini ? 9 : 10` + `--text-tertiary`
/// （`DonutChart.tsx:132`），字距 0 —— `AidogType.micro` 的 11 + ls0.66 在
/// React 侧没有对应物。
TextStyle _centerHintStyle(bool mini, Color color) => AidogType.micro.copyWith(
  fontSize: mini ? 9 : 10,
  letterSpacing: 0,
  color: color,
);
