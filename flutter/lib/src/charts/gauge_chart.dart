/// 快照仪表盘（对应 React 版 `src/components/charts/GaugeChart.tsx`）。
///
/// React 那边是 `conic-gradient` + `radial mask` 的纯 CSS 环（零 canvas 零 Recharts）；
/// Flutter 没有 conic-gradient，一句 `canvas.drawArc` 就够，比拼渐变着色器简单得多 ——
/// 所以这两个（环 + 趋势 sparkline）走 [CustomPainter]，不引图表库。
library;

import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../../utils/formatters.dart';
import '../shell/theme.dart';
import 'empty.dart';
import 'palette.dart';

/// 趋势态数据：[at] = 事件 Unix 秒，[fraction] = 当时占比 [0,1]（quota_snapshots 喂）。
@immutable
class GaugeTrendPoint {
  const GaugeTrendPoint({required this.at, required this.fraction});
  final double at;
  final double fraction;
}

class GaugeChart extends StatelessWidget {
  const GaugeChart({
    super.key,
    required this.value,
    required this.formatValue,
    this.max = 1,
    this.title,
    this.label,
    this.size = 160,
    this.trend,
    this.emptyText = '',
    this.emptyHint,
  });

  final double value;

  /// 满值。`max <= 0` 视为无数据（诚实空态），与 React 版同判据。
  final double max;
  final String Function(double) formatValue;

  /// 标题（如平台名），画在**环的上方**。React 那边 `GaugeChart` 的 `title`
  /// 由 `ChartCard` 渲染在图上方（`Stats.tsx:853-860` 传的就是平台名）。
  final String? title;

  /// 说明字，画在环心。
  final String? label;
  final double size;

  /// 趋势态：非空时在仪表下方画 fraction sparkline（≥2 点画线，单点画点）。
  final List<GaugeTrendPoint>? trend;
  final String emptyText;
  final String? emptyHint;

  @override
  Widget build(BuildContext context) {
    if (!(max > 0)) return ChartEmpty(emptyText, hint: emptyHint);
    final t = AidogTheme.of(context);
    final palette = ChartPalette(t.c);
    final fraction = clamp(value / max, 0, 1);
    final ringWidth = math.max(10, (size / 14).round()).toDouble();

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (title case final tt?)
          SizedBox(
            width: size,
            child: Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: Text(
                tt,
                textAlign: TextAlign.center,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.label.copyWith(
                  color: t.c.fg,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ),
        Semantics(
          value: '${(fraction * 100).round()}%',
          child: SizedBox(
            width: size,
            height: size,
            child: CustomPaint(
              painter: GaugeRingPainter(
                fraction: fraction,
                ringWidth: ringWidth,
                arc: palette.primary,
                track: palette.series(2),
              ),
              child: Center(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      formatPercent(fraction * 100, 0),
                      style: TextStyle(
                        fontFamily: AidogType.familyMono,
                        fontSize: size / 7,
                        fontWeight: FontWeight.w600,
                        color: t.c.fg,
                        fontFeatures: const [FontFeature.tabularFigures()],
                      ),
                    ),
                    Text(
                      formatValue(value),
                      style: AidogType.numSm.copyWith(color: t.c.fg2),
                    ),
                    if (label != null)
                      SizedBox(
                        width: size * 0.7,
                        child: Text(
                          label!,
                          textAlign: TextAlign.center,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: AidogType.micro.copyWith(color: t.c.fg3),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
        if (trend != null && trend!.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.smd),
            child: SizedBox(
              width: size,
              height: kSparklineHeight,
              child: CustomPaint(
                painter: SparklinePainter(
                  points: trend!,
                  color: palette.primary,
                ),
              ),
            ),
          ),
      ],
    );
  }
}

/// 环高：与 React 版的 `H = 30` 一致。
const double kSparklineHeight = 30;
const double kSparklinePad = 2;

/// 琥珀弧 + 灰轨。12 点起画、顺时针，与 `conic-gradient` 的起点方向一致。
class GaugeRingPainter extends CustomPainter {
  const GaugeRingPainter({
    required this.fraction,
    required this.ringWidth,
    required this.arc,
    required this.track,
  });

  final double fraction;
  final double ringWidth;
  final Color arc;
  final Color track;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Rect.fromLTWH(
      0,
      0,
      size.width,
      size.height,
    ).deflate(ringWidth / 2);
    final paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = ringWidth
      ..color = track;
    canvas.drawArc(rect, 0, math.pi * 2, false, paint);
    if (fraction <= 0) return;
    canvas.drawArc(
      rect,
      -math.pi / 2,
      math.pi * 2 * fraction,
      false,
      paint..color = arc,
    );
  }

  @override
  bool shouldRepaint(GaugeRingPainter old) =>
      old.fraction != fraction ||
      old.ringWidth != ringWidth ||
      old.arc != arc ||
      old.track != track;
}

/// 趋势 sparkline：x 按 [GaugeTrendPoint.at] 归一，y 上 = 1 下 = 0。单点画圆点。
class SparklinePainter extends CustomPainter {
  const SparklinePainter({required this.points, required this.color});

  final List<GaugeTrendPoint> points;
  final Color color;

  /// 与 React 版同算式：`tSpan = max(1e-9, last.at - first.at)`（防单点除零）。
  Offset at(int i, Size size) {
    final t0 = points.first.at;
    final span = math.max(1e-9, points.last.at - t0);
    return Offset(
      kSparklinePad +
          (points[i].at - t0) / span * (size.width - kSparklinePad * 2),
      kSparklinePad +
          (1 - clamp(points[i].fraction, 0, 1)) *
              (size.height - kSparklinePad * 2),
    );
  }

  @override
  void paint(Canvas canvas, Size size) {
    if (points.isEmpty) return;
    final paint = Paint()..color = color;
    if (points.length == 1) {
      canvas.drawCircle(at(0, size), 2, paint);
      return;
    }
    final path = Path()..moveTo(at(0, size).dx, at(0, size).dy);
    for (var i = 1; i < points.length; i++) {
      final p = at(i, size);
      path.lineTo(p.dx, p.dy);
    }
    canvas.drawPath(
      path,
      paint
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5
        ..strokeJoin = StrokeJoin.round
        ..strokeCap = StrokeCap.round,
    );
  }

  @override
  bool shouldRepaint(SparklinePainter old) =>
      old.points != points || old.color != color;
}
