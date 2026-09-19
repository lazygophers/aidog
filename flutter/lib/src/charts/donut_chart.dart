/// 环形图（对应 React 版 `src/components/charts/DonutChart.tsx`）。
///
/// topN + 「其他」合并、首位主色、中央总值、侧列占比图例。
/// 单一有效扇区（或全空）构不成占比图 → 诚实空态，与 React 版同判据。
library;

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';

import '../../utils/formatters.dart';
import '../shell/theme.dart';
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

class AidogDonutChart extends StatelessWidget {
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
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
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
                  startDegreeOffset: -90,
                  pieTouchData: PieTouchData(enabled: true),
                  sections: [
                    for (final d in slices)
                      PieChartSectionData(
                        value: d.value,
                        color: d.color,
                        radius: size * 0.13, // 外径 88% → 环宽 (88-62)/2 = 13%
                        showTitle: false,
                      ),
                  ],
                ),
              ),
              Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    formatValue(total),
                    style: (mini ? AidogType.numSm : AidogType.numMd).copyWith(
                      color: t.c.fg,
                      fontWeight: FontWeight.w600,
                      fontFeatures: const [FontFeature.tabularFigures()],
                    ),
                  ),
                  if (centerLabel != null)
                    Text(
                      centerLabel!,
                      style: AidogType.micro.copyWith(color: t.c.fg3),
                    ),
                ],
              ),
            ],
          ),
        ),
        if (showLegend) ...[
          const SizedBox(width: AidogSpace.s_2xl),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final d in slices)
                  Container(
                    padding: const EdgeInsets.symmetric(vertical: 5),
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
                        const SizedBox(width: AidogSpace.ssm),
                        Expanded(
                          child: Text(
                            d.name,
                            overflow: TextOverflow.ellipsis,
                            maxLines: 1,
                            style: AidogType.caption.copyWith(color: t.c.fg),
                          ),
                        ),
                        Text(
                          formatPercent(d.percent),
                          style: AidogType.numSm.copyWith(
                            color: t.c.fg2,
                            fontFeatures: const [FontFeature.tabularFigures()],
                          ),
                        ),
                      ],
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
