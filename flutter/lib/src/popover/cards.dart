/// 托盘小窗的 14 种卡片 + 二维网格（票 I11）—— 对齐
/// `src/components/PopoverCards.tsx` 的 `renderItem` / `renderGrid`。
///
/// **这一份被用两处**（与 React 同构，单一事实源，避免预览与实际渲染漂移）：
/// 1. 小窗本体：`lib/src/popover/app.dart`
/// 2. 设置页实时预览：`lib/src/pages/settings/tray_pages.dart`
///
/// 分行 / 查询 / 颜色的规则全在 `model.dart`（纯函数，单测钉住）；本文件只画。
library;

import 'package:flutter/material.dart';

import '../../charts.dart';
import '../../i18n.dart';
import '../../utils/formatters.dart';
import '../shell/theme.dart';
import 'model.dart';

/// CSS grid 的 `gap: 6px`（`src/styles/popover.css:103`）。
const double _gridGap = 6;

/// `.popover-root`：底色 var(--popover)=surface、圆角 --radius-lg 16、
/// 内边距上下 10 左右 14、宽 280..480、**无边框**
/// （popover.css:27-43 + `src/themes/mono.ts:25,54`；那句
/// `border: var(--glass-border)` 少了 border-style，浏览器按 none 处理）。
///
/// 小窗本体（`app.dart`）与设置页实时预览（`tray_pages.dart`）共用这一层，
/// 与 React 两处都套 `.popover-root` 对齐。
class PopoverRoot extends StatelessWidget {
  const PopoverRoot({super.key, required this.child});

  static const double padY = 10;
  static const double padX = 14;
  static const double minW = 280;
  static const double maxW = 480;

  final Widget child;

  @override
  Widget build(BuildContext context) => Container(
    constraints: const BoxConstraints(minWidth: minW, maxWidth: maxW),
    padding: const EdgeInsets.symmetric(vertical: padY, horizontal: padX),
    decoration: BoxDecoration(
      color: AidogTheme.of(context).c.surface,
      borderRadius: BorderRadius.circular(AidogRadius.lg),
    ),
    child: child,
  );
}

/// 小窗专用字阶：**不走主界面的 `AidogType`**。
///
/// React 小窗是独立 bundle，根字体就是系统 sans 13px（`src/styles/popover.css:34-35`），
/// 数字靠 `font-variant-numeric: tabular-nums` 等宽，**不换等宽字族**。行高不设，
/// 走浏览器 `normal`（字体自然行高）—— 所以这里 `height` 留空，不写 1.55。
TextStyle _pt(
  double size, {
  FontWeight weight = FontWeight.w400,
  Color? color,
  double? letterSpacing,
  bool tabular = false,
  FontStyle? fontStyle,
}) => TextStyle(
  fontFamily: AidogType.familySans,
  fontSize: size,
  fontWeight: weight,
  color: color,
  letterSpacing: letterSpacing,
  fontStyle: fontStyle,
  fontFeatures: tabular ? const [FontFeature.tabularFigures()] : null,
);

/// `.popover-section` 的上下内边距：m 6 / s 3 / l 8（popover.css:92,124,138）。
double _sectionPadY(PopoverSize size) => switch (size) {
  PopoverSize.s => 3,
  PopoverSize.m => 6,
  PopoverSize.l => 8,
};

/// `.popover-metric-value` / `.popover-platform-value` 的字号随 s/m/l 变
/// （popover.css:126-129,140-143）。基准 14 / 12 由调用方给。
double _valueSize(PopoverSize size, double base) => switch (size) {
  PopoverSize.s => 13,
  PopoverSize.m => base,
  PopoverSize.l => 16,
};

/// 小窗一帧的全部数据。整份读整份用，不拆成 14 个 model 类。
@immutable
class PopoverFrame {
  const PopoverFrame({
    required this.data,
    this.groups = const [],
    this.groupDetails,
    this.stats = const {},
    this.statsLoaded = false,
  });

  /// `popover_data` 的返回值。
  final Map<String, Object?> data;

  /// `group_list`（group_* 卡按 group_key 反查分组名）。
  final List<Map<String, Object?>> groups;

  /// `group_detail_list`；**null = 还在加载**（group_balance 据此显示加载态）。
  final List<Map<String, Object?>>? groupDetails;

  /// `stats_query_batch` 结果，item.id → StatsResult。
  final Map<String, Map<String, Object?>> stats;

  /// 首次批量统计是否已完成（已完成但本卡缺结果 = 该卡查询失败）。
  final bool statsLoaded;

  Map<String, Object?> get config => _map(data['config']);
  Map<String, Object?> get todayStats => _map(data['today_stats']);
  List<Map<String, Object?>> get entries => _list(data['entries']);
  List<Map<String, Object?>> get platformToday => _list(data['platform_today']);
  bool get proxyRunning => data['proxy_running'] == true;
  int get proxyPort => (data['proxy_port'] as num?)?.toInt() ?? 0;

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
  static List<Map<String, Object?>> _list(Object? v) => (v as List? ?? const [])
      .whereType<Map>()
      .map(Map<String, Object?>.from)
      .toList(growable: false);
}

/// 按 row 分组的二维网格。行内按 `cols` 切块（对齐 CSS grid 的 auto-flow：
/// 一行里的卡多于列数时自动折到下一排，不挤成一排）。
class PopoverGrid extends StatelessWidget {
  const PopoverGrid({super.key, required this.frame});

  final PopoverFrame frame;

  @override
  Widget build(BuildContext context) {
    final rows = popoverRows(frame.config);
    if (rows.isEmpty) {
      return _Empty(AidogI18n.of(context).t('popover.empty'));
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final r in rows)
          for (var i = 0; i < r.items.length; i += r.cols)
            Padding(
              // CSS grid 的 gap 6px 同时管行列：**同一 row 折下来的隐式行**之间有
              // 6px，不同 `.popover-grid-row` 之间没有（它们是普通兄弟 div，无
              // margin）—— `src/styles/popover.css:101-105`。
              padding: EdgeInsets.only(bottom: i + r.cols < r.items.length ? _gridGap : 0),
              // `align-items: stretch`（popover.css:104）：同一行的卡等高。
              // Row 在 Column 里高度无约束，stretch 需要 IntrinsicHeight 定高。
              child: IntrinsicHeight(
                child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  for (var j = 0; j < r.cols; j++)
                    Expanded(
                      child: Padding(
                        padding: EdgeInsets.only(left: j == 0 ? 0 : _gridGap),
                        child: i + j < r.items.length
                            ? PopoverCard(
                                key: ValueKey('${r.items[i + j]['id']}'),
                                item: r.items[i + j],
                                frame: frame,
                              )
                            : const SizedBox.shrink(),
                      ),
                    ),
                ],
                ),
              ),
            ),
      ],
    );
  }
}

/// 单张卡：按 `item_type` 分发。未知类型 → 空盒（与 React `default: null` 一致）。
class PopoverCard extends StatelessWidget {
  const PopoverCard({super.key, required this.item, required this.frame});

  final Map<String, Object?> item;
  final PopoverFrame frame;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final size = normPopoverSize(item['size']);
    final color = popoverValueColor(item['color'], t.c);
    final id = '${item['id'] ?? ''}';
    final stats = frame.stats[id];
    final type = '${item['item_type'] ?? ''}';

    final child = switch (type) {
      'proxy_status' => _ProxyStatus(frame: frame),
      'platform_balance' => _PlatformBalance(frame: frame, size: size),
      'today_cost' => _MetricRow(
        label: AidogI18n.of(context).t('popover.todayCost'),
        value: formatCostUsd(_num(frame.todayStats['cost'])),
        sub: AidogI18n.of(context).t('popover.todayCostSub'),
        size: size,
        color: color,
      ),
      'today_cache_rate' => _MetricRow(
        label: AidogI18n.of(context).t('popover.todayCacheRate'),
        value: formatPercent(_num(frame.todayStats['cache_rate']), 0),
        sub: AidogI18n.of(context).t('popover.todayCacheRateSub'),
        size: size,
        color: color,
      ),
      'today_tokens' => _MetricRow(
        label: AidogI18n.of(context).t('popover.todayTokens'),
        value: formatNumber(_num(frame.todayStats['tokens'])),
        sub: AidogI18n.of(context).t('popover.todayTokensSub'),
        size: size,
        color: color,
      ),
      'platform_today' => _PlatformToday(
        frame: frame,
        size: size,
        color: color,
      ),
      'cost_trend' => _StatsCard(
        size: size,
        title: size == PopoverSize.s ? null : _trendTitle(context, item, frame),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _trendBody(context, s, size, color),
      ),
      'platform_share' => _StatsCard(
        size: size,
        title: size == PopoverSize.s
            ? null
            : AidogI18n.of(context).t('popover.itemPlatformShare'),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _shareBody(context, s, size),
      ),
      'hour_heatbar' => _StatsCard(
        size: size,
        title: size == PopoverSize.s ? null : AidogI18n.of(context).t('popover.itemHourHeat'),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _heatBody(context, s),
      ),
      'platform_metric' => _StatsCard(
        size: size,
        title: size == PopoverSize.s
            ? null
            : AidogI18n.of(context).t('popover.platformMetricTitle', {
                'name': _platformName(context, item, frame),
              }),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _platformMetricBody(context, s, size, color),
      ),
      'group_cost' => _StatsCard(
        size: size,
        title: size == PopoverSize.s
            ? null
            : AidogI18n.of(context).t('popover.groupCostTitle', {'name': _groupName(context, item)}),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _groupCostBody(context, s, size, color),
      ),
      'group_tokens' => _StatsCard(
        size: size,
        title: size == PopoverSize.s
            ? null
            : AidogI18n.of(context).t('popover.groupTokensTitle', {'name': _groupName(context, item)}),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _groupTokensBody(context, s, size, color),
      ),
      'group_requests' => _StatsCard(
        size: size,
        title: size == PopoverSize.s
            ? null
            : AidogI18n.of(context).t('popover.groupRequestsTitle', {'name': _groupName(context, item)}),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _groupRequestsBody(context, s, size, color),
      ),
      'group_balance' => _GroupBalance(
        item: item,
        frame: frame,
        size: size,
        color: color,
        name: _groupName(context, item),
      ),
      _ => const SizedBox.shrink(),
    };
    return child;
  }

  /// cost_trend 标题（体现 scope）。对齐 `trendTitle`。
  String _trendTitle(BuildContext context, Map<String, Object?> item, PopoverFrame frame) {
    final scope = '${item['scope'] ?? 'overall'}';
    if (scope == 'platform' && '${item['scope_ref'] ?? ''}'.isNotEmpty) {
      return AidogI18n.of(context).t('popover.trendPlatformTitle', {
        'name': _platformName(context, item, frame),
      });
    }
    if (scope == 'group') return AidogI18n.of(context).t('popover.trendGroupTitle');
    return AidogI18n.of(context).t('popover.trendOverallTitle');
  }

  /// 平台名（按 platform_id 查 platform_today，兜底 scope_ref → 未知平台）。
  String _platformName(BuildContext context, Map<String, Object?> item, PopoverFrame frame) {
    final ref = '${item['scope_ref'] ?? ''}';
    for (final p in frame.platformToday) {
      if ('${(p['platform_id'] as num?)?.toInt()}' == ref) {
        final n = '${p['platform_name'] ?? ''}';
        if (n.isNotEmpty) return n;
      }
    }
    return ref.isNotEmpty ? ref : AidogI18n.of(context).t('popover.unknownPlatform');
  }

  /// 分组名（按 group_key 查 groups，兜底 scope_ref → 「分组」）。
  String _groupName(BuildContext context, Map<String, Object?> item) {
    final ref = '${item['scope_ref'] ?? ''}';
    for (final g in frame.groups) {
      if ('${g['group_key'] ?? ''}' == ref) {
        final n = '${g['name'] ?? ''}';
        if (n.isNotEmpty) return n;
      }
    }
    return ref.isNotEmpty ? ref : AidogI18n.of(context).t('popover.trendScopeGroup');
  }
}

// ── 各卡的正文 ────────────────────────────────────────────

Widget _trendBody(
  BuildContext context,
  Map<String, Object?> s,
  PopoverSize size,
  Color? color,
) {
  final buckets = (s['buckets'] as List? ?? const []).whereType<Map>().toList();
  if (buckets.isEmpty) return _Empty(AidogI18n.of(context).t('popover.noUsageToday'));
  final total = buckets.fold<double>(
    0,
    (sum, b) => sum + _num(b['total_cost']),
  );
  final p = ChartPalette(AidogTheme.of(context).c);
  return Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      SizedBox(
        height: switch (size) {
          PopoverSize.s => 44,
          PopoverSize.m => 56,
          PopoverSize.l => 72,
        },
        child: AlwaysLtr(
          child: AidogLineChart(
            mini: true,
            series: [
              ChartSeries(
                key: 'v',
                label: AidogI18n.of(context).t('popover.trendCostSeries'),
                color: p.series(0),
                points: [
                  for (final b in buckets)
                    ChartPoint(
                      bucketMs('${b['time_bucket'] ?? ''}'),
                      _num(b['total_cost']),
                    ),
                ],
                format: formatCostUsd,
              ),
            ],
          ),
        ),
      ),
      if (size == PopoverSize.l)
        _Sub('${AidogI18n.of(context).t('popover.trendTotal')} ${formatCostUsd(total)}', color),
    ],
  );
}

Widget _shareBody(BuildContext context, Map<String, Object?> s, PopoverSize size) {
  final entries = popoverShareEntries(s);
  // 不足两个有效扇区构不成占比（与 DonutChart 空态判据一致），诚实空态不画假环。
  if (entries.length < 2) return _Empty(AidogI18n.of(context).t('charts.noData'));
  return Center(
    child: AidogDonutChart(
      mini: true,
      data: entries,
      restLabel: AidogI18n.of(context).t('stats.donutRest'),
      topN: size == PopoverSize.s ? 3 : 4,
      size: switch (size) {
        PopoverSize.s => 64,
        PopoverSize.m => 88,
        PopoverSize.l => 104,
      },
      showLegend: size != PopoverSize.s,
      formatValue: formatCostUsd,
      centerLabel: AidogI18n.of(context).t('popover.trendTotal'),
    ),
  );
}

Widget _heatBody(BuildContext context, Map<String, Object?> s) {
  final buckets = (s['buckets'] as List? ?? const []).whereType<Map>().toList();
  if (buckets.isEmpty) return _Empty(AidogI18n.of(context).t('popover.noUsageToday'));
  return AlwaysLtr(
    child: HourHeatBar(
      semanticLabel: AidogI18n.of(context).t('popover.itemHourHeat'),
      formatValue: formatNumber,
      data: [
        for (final b in buckets)
          (
            hour: DateTime.fromMillisecondsSinceEpoch(
              bucketMs('${b['time_bucket'] ?? ''}').isFinite
                  ? bucketMs('${b['time_bucket'] ?? ''}').toInt()
                  : 0,
            ).hour,
            value: _num(b['total_requests']),
          ),
      ],
    ),
  );
}

Widget _platformMetricBody(
  BuildContext context,
  Map<String, Object?> s,
  PopoverSize size,
  Color? color,
) {
  final o = _mapOf(s['overview']);
  final tokens = popoverOverviewTokens(o);
  return Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      // React 这张卡用的是 `.popover-platform-row`（上下 3、间距 10、基线对齐、
      // 值走 platform-value 12 档），不是 metric-row。
      Padding(
        padding: const EdgeInsets.symmetric(vertical: 3),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.baseline,
          textBaseline: TextBaseline.alphabetic,
          children: [
            _Value(
              formatCostUsd(_num(o['total_cost'])),
              color,
              size: size,
              base: 12,
            ),
            if (size != PopoverSize.s) ...[
              const SizedBox(width: 10),
              _SubInline('${formatNumber(tokens)} tok'),
            ],
          ],
        ),
      ),
      if (size == PopoverSize.l)
        _Sub(
          '${AidogI18n.of(context).t('popover.tokenIn')} ${formatNumber(_num(o['total_input_tokens']))}'
          ' · ${AidogI18n.of(context).t('popover.tokenOut')} ${formatNumber(_num(o['total_output_tokens']))}',
          null,
        ),
    ],
  );
}

Widget _groupCostBody(
  BuildContext context,
  Map<String, Object?> s,
  PopoverSize size,
  Color? color,
) {
  final o = _mapOf(s['overview']);
  return Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _MetricValueRow(size: size, child: _Value(formatCostUsd(_num(o['total_cost'])), color, size: size)),
      if (size == PopoverSize.l)
        _Sub(
          '${formatNumber(_num(o['total_requests']))} ${AidogI18n.of(context).t('popover.reqUnit')}'
          ' · ${formatNumber(popoverOverviewTokens(o))} tok',
          null,
        ),
    ],
  );
}

Widget _groupTokensBody(
  BuildContext context,
  Map<String, Object?> s,
  PopoverSize size,
  Color? color,
) {
  final o = _mapOf(s['overview']);
  return Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _MetricValueRow(size: size, child: _Value('${formatNumber(popoverOverviewTokens(o))} tok', color, size: size)),
      if (size == PopoverSize.l)
        _Sub(
          '${AidogI18n.of(context).t('popover.tokenIn')} ${formatNumber(_num(o['total_input_tokens']))}'
          ' · ${AidogI18n.of(context).t('popover.tokenOut')} ${formatNumber(_num(o['total_output_tokens']))}',
          null,
        ),
    ],
  );
}

Widget _groupRequestsBody(
  BuildContext context,
  Map<String, Object?> s,
  PopoverSize size,
  Color? color,
) {
  final o = _mapOf(s['overview']);
  return Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _MetricValueRow(size: size, child: _Value(formatNumber(_num(o['total_requests'])), color, size: size)),
      if (size == PopoverSize.l)
        _Sub(
          '${AidogI18n.of(context).t('popover.successRate')} '
          '${formatPercent(_num(o['success_rate']), 0)}',
          null,
        ),
    ],
  );
}

// ── 不走统计查询的四张卡 ──────────────────────────────────

class _ProxyStatus extends StatelessWidget {
  const _ProxyStatus({required this.frame});

  final PopoverFrame frame;

  @override
  Widget build(BuildContext context) {
    final c = AidogTheme.of(context).c;
    final running = frame.proxyRunning;
    // `.popover-header`：不是 section，自带下边框 + 上下 8（popover.css:54-61）。
    return Container(
      padding: const EdgeInsets.only(bottom: 8),
      margin: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: c.line)),
      ),
      child: Row(
        children: [
          _StatusDot(running: running, color: running ? c.ok : c.fg3),
          const SizedBox(width: 6),
          Text(
            running ? ltr('Running :${frame.proxyPort}') : 'Stopped',
            style: _pt(12, weight: FontWeight.w500, color: c.fg2, letterSpacing: 0.24),
          ),
        ],
      ),
    );
  }
}

/// 运行中的状态点会呼吸（`statusPulse` 2s，opacity 1 → 0.5）并带一圈辉光
/// （popover.css:63-82 + PopoverCards.tsx:88-92）。系统「减少动态效果」时不动画。
class _StatusDot extends StatefulWidget {
  const _StatusDot({required this.running, required this.color});

  final bool running;
  final Color color;

  @override
  State<_StatusDot> createState() => _StatusDotState();
}

class _StatusDotState extends State<_StatusDot> with SingleTickerProviderStateMixin {
  late final AnimationController _ctrl = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 2),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final reduce = MediaQuery.maybeDisableAnimationsOf(context) ?? false;
    if (widget.running && !reduce) {
      if (!_ctrl.isAnimating) _ctrl.repeat(reverse: true);
    } else {
      _ctrl.stop();
      _ctrl.value = 0;
    }
  }

  @override
  void didUpdateWidget(_StatusDot old) {
    super.didUpdateWidget(old);
    if (old.running != widget.running) didChangeDependencies();
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final dot = Container(
      width: 7,
      height: 7,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: widget.color,
        boxShadow: widget.running
            ? [BoxShadow(color: widget.color.withValues(alpha: 0.5), blurRadius: 8)]
            : null,
      ),
    );
    if (!widget.running) return dot;
    return FadeTransition(
      opacity: Tween<double>(begin: 1, end: 0.5).animate(
        CurvedAnimation(parent: _ctrl, curve: Curves.easeInOut),
      ),
      child: dot,
    );
  }
}

class _PlatformBalance extends StatelessWidget {
  const _PlatformBalance({required this.frame, required this.size});

  final PopoverFrame frame;
  final PopoverSize size;

  @override
  Widget build(BuildContext context) {
    final c = AidogTheme.of(context).c;
    if (frame.entries.isEmpty) return const SizedBox.shrink();
    return _Section(
      size: size,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // `.popover-entry`：上下 4、间距 6（popover.css:156-161）。
          for (final e in frame.entries)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Row(
                children: [
                  Container(
                    width: 6,
                    height: 6,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: popoverEntryColor(e['color'], c),
                    ),
                  ),
                  const SizedBox(width: 6),
                  if (size != PopoverSize.s)
                    Expanded(
                      child: Text(
                        '${e['name'] ?? ''}',
                        overflow: TextOverflow.ellipsis,
                        style: _pt(12, color: c.fg2),
                      ),
                    )
                  else
                    const Spacer(),
                  Text(
                    ltr('${e['value'] ?? ''}'),
                    style: _pt(
                      13,
                      weight: FontWeight.w600,
                      color: popoverEntryColor(e['color'], c),
                      tabular: true,
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _PlatformToday extends StatelessWidget {
  const _PlatformToday({
    required this.frame,
    required this.size,
    required this.color,
  });

  final PopoverFrame frame;
  final PopoverSize size;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final c = AidogTheme.of(context).c;
    return _Section(
      size: size,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _Title(AidogI18n.of(context).t('popover.platformToday')),
          if (frame.platformToday.isEmpty)
            _Empty(AidogI18n.of(context).t('popover.noUsageToday'))
          else
            // `.popover-platform-row`：上下 3、间距 10（popover.css:247-253）。
            for (final p in frame.platformToday)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 3),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.baseline,
                  textBaseline: TextBaseline.alphabetic,
                  children: [
                    Expanded(
                      child: Text(
                        '${p['platform_name'] ?? ''}'.isEmpty
                            ? AidogI18n.of(context).t('popover.unknownPlatform')
                            : '${p['platform_name']}',
                        overflow: TextOverflow.ellipsis,
                        style: _pt(12, color: c.fg2),
                      ),
                    ),
                    const SizedBox(width: 10),
                    _Value(
                      formatCostUsd(_num(p['cost'])),
                      color,
                      size: size,
                      base: 12,
                    ),
                    if (size != PopoverSize.s) ...[
                      const SizedBox(width: 10),
                      _SubInline('${formatNumber(_num(p['tokens']))} tok'),
                    ],
                    if (size == PopoverSize.l) ...[
                      const SizedBox(width: 10),
                      _SubInline(
                        '${formatNumber(_num(p['requests']))} '
                        '${AidogI18n.of(context).t('popover.reqUnit')}',
                      ),
                    ],
                  ],
                ),
              ),
        ],
      ),
    );
  }
}

class _GroupBalance extends StatelessWidget {
  const _GroupBalance({
    required this.item,
    required this.frame,
    required this.size,
    required this.color,
    required this.name,
  });

  final Map<String, Object?> item;
  final PopoverFrame frame;
  final PopoverSize size;
  final Color? color;
  final String name;

  @override
  Widget build(BuildContext context) {
    final details = frame.groupDetails;
    final ref = '${item['scope_ref'] ?? ''}';
    Map<String, Object?>? detail;
    for (final d in details ?? const <Map<String, Object?>>[]) {
      if ('${_mapOf(d['group'])['group_key'] ?? ''}' == ref) detail = d;
    }
    final platforms = (detail?['platforms'] as List? ?? const [])
        .whereType<Map>()
        .toList();
    final balance = platforms.fold<double>(
      0,
      (sum, p) => sum + _num(_mapOf(p['platform'])['est_balance_remaining']),
    );
    return _Section(
      size: size,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (size != PopoverSize.s)
            _Title(AidogI18n.of(context).t('popover.groupBalanceTitle', {'name': name})),
          if (details == null)
            _Empty(AidogI18n.of(context).t('common.loading'))
          else if (detail == null)
            _Empty(AidogI18n.of(context).t('popover.trendNoGroup'))
          else ...[
            _MetricValueRow(child: _Value(formatCostUsd(balance), color, size: size)),
            if (size == PopoverSize.l)
              _Sub(
                '${platforms.length} ${AidogI18n.of(context).t('popover.platformsUnit')}',
                null,
              ),
          ],
        ],
      ),
    );
  }
}

// ── 公共外壳 ──────────────────────────────────────────────

/// 走统计查询的卡共用的三态壳：未加载完 → loading；加载完但缺结果 → 失败；
/// 有结果 → [builder]。对齐 React 每张卡里那段 `failed / null / empty` 三分支。
class _StatsCard extends StatelessWidget {
  const _StatsCard({
    required this.title,
    required this.stats,
    required this.loaded,
    required this.builder,
    required this.size,
  });

  final String? title;
  final Map<String, Object?>? stats;
  final bool loaded;
  final Widget Function(Map<String, Object?>) builder;
  final PopoverSize size;

  @override
  Widget build(BuildContext context) {
    final s = stats;
    return _Section(
      size: size,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (title != null) _Title(title!),
          if (loaded && s == null)
            _Empty(AidogI18n.of(context).t('popover.trendLoadError'))
          else if (s == null)
            _Empty(AidogI18n.of(context).t('common.loading'))
          else
            builder(s),
        ],
      ),
    );
  }
}

class _MetricRow extends StatelessWidget {
  const _MetricRow({
    required this.label,
    required this.value,
    required this.sub,
    required this.size,
    required this.color,
  });

  final String label;
  final String value;
  final String sub;
  final PopoverSize size;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final c = AidogTheme.of(context).c;
    // s: 仅大数值（无标签）；m: 标签 + 值；l: 标签 + 值 + 副标。
    // s 档的 section 不带 pc-s（React 只把 pc-s 给了里面的 metric-row，
    // `PopoverCards.tsx:132-139`），所以外壳按 m 的 6px 走。
    if (size == PopoverSize.s) {
      return _Section(
        size: PopoverSize.m,
        child: _MetricValueRow(
          size: size,
          child: _Value(value, color, size: size),
        ),
      );
    }
    return _Section(
      size: size,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _MetricValueRow(
            size: size,
            label: Text(
              label,
              overflow: TextOverflow.ellipsis,
              style: _pt(12, color: c.fg2),
            ),
            child: _Value(value, color, size: size),
          ),
          if (size == PopoverSize.l) _Sub(sub, null),
        ],
      ),
    );
  }
}

/// `.popover-section`：只有上下内边距，**没有底色 / 边框 / 圆角**
/// （`src/styles/popover.css:91-93`；grid 模式下相邻 section 的分隔线也被
/// `:116-119` 去掉了，所以这里一条线都不画）。
class _Section extends StatelessWidget {
  const _Section({required this.size, required this.child});

  final PopoverSize size;
  final Widget child;

  @override
  Widget build(BuildContext context) => Padding(
    padding: EdgeInsets.symmetric(vertical: _sectionPadY(size)),
    child: child,
  );
}

/// `.popover-metric-row`：上下 4（s 档 2），基线对齐，标签与值之间 12
/// （popover.css:130-132,221-227）。
class _MetricValueRow extends StatelessWidget {
  const _MetricValueRow({required this.child, this.size = PopoverSize.m, this.label});

  final Widget child;
  final PopoverSize size;
  final Widget? label;

  @override
  Widget build(BuildContext context) {
    final l = label;
    return Padding(
      padding: EdgeInsets.symmetric(vertical: size == PopoverSize.s ? 2 : 4),
      child: l == null
          ? child
          : Row(
              crossAxisAlignment: CrossAxisAlignment.baseline,
              textBaseline: TextBaseline.alphabetic,
              children: [Expanded(child: l), const SizedBox(width: 12), child],
            ),
    );
  }
}

/// `.popover-stats-title`：11 w600 全大写 ls 0.06em fg3，下距 6。
class _Title extends StatelessWidget {
  const _Title(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 6),
    child: Text(
      text.toUpperCase(),
      overflow: TextOverflow.ellipsis,
      style: _pt(
        11,
        weight: FontWeight.w600,
        color: AidogTheme.of(context).c.fg3,
        letterSpacing: 0.66,
      ),
    ),
  );
}

/// `.popover-metric-value`：w600 tabular，字号随 s/m/l 为 13/14/16。
class _Value extends StatelessWidget {
  const _Value(this.text, this.color, {required this.size, this.base = 14});

  final String text;
  final Color? color;
  final PopoverSize size;

  /// m 档字号：指标值 14，平台行金额 12（popover.css:238,265）。
  final double base;

  @override
  Widget build(BuildContext context) => Text(
    ltr(text),
    overflow: TextOverflow.ellipsis,
    style: _pt(
      _valueSize(size, base),
      weight: FontWeight.w600,
      color: color ?? AidogTheme.of(context).c.fg,
      tabular: true,
    ),
  );
}

/// `.popover-metric-sub`：11 fg3 tabular，上距 3。
class _Sub extends StatelessWidget {
  const _Sub(this.text, this.color);

  final String text;
  final Color? color;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 3),
    child: Text(
      text,
      overflow: TextOverflow.ellipsis,
      style: _pt(
        11,
        color: color ?? AidogTheme.of(context).c.fg3,
        tabular: true,
      ),
    ),
  );
}

/// `.popover-platform-sub`：同 11 fg3 tabular，行内不带上距。
class _SubInline extends StatelessWidget {
  const _SubInline(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Text(
    ltr(text),
    style: _pt(11, color: AidogTheme.of(context).c.fg3, tabular: true),
  );
}

/// `.popover-empty`：11 斜体 fg3，上下内边距 4。
class _Empty extends StatelessWidget {
  const _Empty(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 4),
    child: Text(
      text,
      style: _pt(
        11,
        color: AidogTheme.of(context).c.fg3,
        fontStyle: FontStyle.italic,
      ),
    ),
  );
}

double _num(Object? v) => (v as num?)?.toDouble() ?? 0;

Map<String, Object?> _mapOf(Object? v) =>
    v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
