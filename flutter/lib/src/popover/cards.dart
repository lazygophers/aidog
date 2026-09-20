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
      return _Empty(i18n.t('popover.empty'));
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final r in rows)
          for (var i = 0; i < r.items.length; i += r.cols)
            Padding(
              padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  for (var j = 0; j < r.cols; j++)
                    Expanded(
                      child: Padding(
                        padding: EdgeInsets.only(
                          left: j == 0 ? 0 : AidogSpace.sxs,
                        ),
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
        label: i18n.t('popover.todayCost'),
        value: formatCostUsd(_num(frame.todayStats['cost'])),
        sub: i18n.t('popover.todayCostSub'),
        size: size,
        color: color,
      ),
      'today_cache_rate' => _MetricRow(
        label: i18n.t('popover.todayCacheRate'),
        value: formatPercent(_num(frame.todayStats['cache_rate']), 0),
        sub: i18n.t('popover.todayCacheRateSub'),
        size: size,
        color: color,
      ),
      'today_tokens' => _MetricRow(
        label: i18n.t('popover.todayTokens'),
        value: formatNumber(_num(frame.todayStats['tokens'])),
        sub: i18n.t('popover.todayTokensSub'),
        size: size,
        color: color,
      ),
      'platform_today' => _PlatformToday(
        frame: frame,
        size: size,
        color: color,
      ),
      'cost_trend' => _StatsCard(
        title: size == PopoverSize.s ? null : _trendTitle(item, frame),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _trendBody(context, s, size, color),
      ),
      'platform_share' => _StatsCard(
        title: size == PopoverSize.s
            ? null
            : i18n.t('popover.itemPlatformShare'),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _shareBody(s, size),
      ),
      'hour_heatbar' => _StatsCard(
        title: size == PopoverSize.s ? null : i18n.t('popover.itemHourHeat'),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _heatBody(s),
      ),
      'platform_metric' => _StatsCard(
        title: size == PopoverSize.s
            ? null
            : i18n.t('popover.platformMetricTitle', {
                'name': _platformName(item, frame),
              }),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _platformMetricBody(s, size, color),
      ),
      'group_cost' => _StatsCard(
        title: size == PopoverSize.s
            ? null
            : i18n.t('popover.groupCostTitle', {'name': _groupName(item)}),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _groupCostBody(s, size, color),
      ),
      'group_tokens' => _StatsCard(
        title: size == PopoverSize.s
            ? null
            : i18n.t('popover.groupTokensTitle', {'name': _groupName(item)}),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _groupTokensBody(s, size, color),
      ),
      'group_requests' => _StatsCard(
        title: size == PopoverSize.s
            ? null
            : i18n.t('popover.groupRequestsTitle', {'name': _groupName(item)}),
        stats: stats,
        loaded: frame.statsLoaded,
        builder: (s) => _groupRequestsBody(s, size, color),
      ),
      'group_balance' => _GroupBalance(
        item: item,
        frame: frame,
        size: size,
        color: color,
        name: _groupName(item),
      ),
      _ => const SizedBox.shrink(),
    };
    return child;
  }

  /// cost_trend 标题（体现 scope）。对齐 `trendTitle`。
  String _trendTitle(Map<String, Object?> item, PopoverFrame frame) {
    final scope = '${item['scope'] ?? 'overall'}';
    if (scope == 'platform' && '${item['scope_ref'] ?? ''}'.isNotEmpty) {
      return i18n.t('popover.trendPlatformTitle', {
        'name': _platformName(item, frame),
      });
    }
    if (scope == 'group') return i18n.t('popover.trendGroupTitle');
    return i18n.t('popover.trendOverallTitle');
  }

  /// 平台名（按 platform_id 查 platform_today，兜底 scope_ref → 未知平台）。
  String _platformName(Map<String, Object?> item, PopoverFrame frame) {
    final ref = '${item['scope_ref'] ?? ''}';
    for (final p in frame.platformToday) {
      if ('${(p['platform_id'] as num?)?.toInt()}' == ref) {
        final n = '${p['platform_name'] ?? ''}';
        if (n.isNotEmpty) return n;
      }
    }
    return ref.isNotEmpty ? ref : i18n.t('popover.unknownPlatform');
  }

  /// 分组名（按 group_key 查 groups，兜底 scope_ref → 「分组」）。
  String _groupName(Map<String, Object?> item) {
    final ref = '${item['scope_ref'] ?? ''}';
    for (final g in frame.groups) {
      if ('${g['group_key'] ?? ''}' == ref) {
        final n = '${g['name'] ?? ''}';
        if (n.isNotEmpty) return n;
      }
    }
    return ref.isNotEmpty ? ref : i18n.t('popover.trendScopeGroup');
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
  if (buckets.isEmpty) return _Empty(i18n.t('popover.noUsageToday'));
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
                label: i18n.t('popover.trendCostSeries'),
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
        _Sub('${i18n.t('popover.trendTotal')} ${formatCostUsd(total)}', color),
    ],
  );
}

Widget _shareBody(Map<String, Object?> s, PopoverSize size) {
  final entries = popoverShareEntries(s);
  // 不足两个有效扇区构不成占比（与 DonutChart 空态判据一致），诚实空态不画假环。
  if (entries.length < 2) return _Empty(i18n.t('charts.noData'));
  return Center(
    child: AidogDonutChart(
      mini: true,
      data: entries,
      restLabel: i18n.t('stats.donutRest'),
      topN: size == PopoverSize.s ? 3 : 4,
      size: switch (size) {
        PopoverSize.s => 64,
        PopoverSize.m => 88,
        PopoverSize.l => 104,
      },
      showLegend: size != PopoverSize.s,
      formatValue: formatCostUsd,
      centerLabel: i18n.t('popover.trendTotal'),
    ),
  );
}

Widget _heatBody(Map<String, Object?> s) {
  final buckets = (s['buckets'] as List? ?? const []).whereType<Map>().toList();
  if (buckets.isEmpty) return _Empty(i18n.t('popover.noUsageToday'));
  return AlwaysLtr(
    child: HourHeatBar(
      semanticLabel: i18n.t('popover.itemHourHeat'),
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
  Map<String, Object?> s,
  PopoverSize size,
  Color? color,
) {
  final o = _mapOf(s['overview']);
  final tokens = popoverOverviewTokens(o);
  return Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Row(
        children: [
          _Value(formatCostUsd(_num(o['total_cost'])), color),
          if (size != PopoverSize.s) ...[
            const SizedBox(width: AidogSpace.sxs),
            _SubInline('${formatNumber(tokens)} tok'),
          ],
        ],
      ),
      if (size == PopoverSize.l)
        _Sub(
          '${i18n.t('popover.tokenIn')} ${formatNumber(_num(o['total_input_tokens']))}'
          ' · ${i18n.t('popover.tokenOut')} ${formatNumber(_num(o['total_output_tokens']))}',
          null,
        ),
    ],
  );
}

Widget _groupCostBody(Map<String, Object?> s, PopoverSize size, Color? color) {
  final o = _mapOf(s['overview']);
  return Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _Value(formatCostUsd(_num(o['total_cost'])), color),
      if (size == PopoverSize.l)
        _Sub(
          '${formatNumber(_num(o['total_requests']))} ${i18n.t('popover.reqUnit')}'
          ' · ${formatNumber(popoverOverviewTokens(o))} tok',
          null,
        ),
    ],
  );
}

Widget _groupTokensBody(
  Map<String, Object?> s,
  PopoverSize size,
  Color? color,
) {
  final o = _mapOf(s['overview']);
  return Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _Value('${formatNumber(popoverOverviewTokens(o))} tok', color),
      if (size == PopoverSize.l)
        _Sub(
          '${i18n.t('popover.tokenIn')} ${formatNumber(_num(o['total_input_tokens']))}'
          ' · ${i18n.t('popover.tokenOut')} ${formatNumber(_num(o['total_output_tokens']))}',
          null,
        ),
    ],
  );
}

Widget _groupRequestsBody(
  Map<String, Object?> s,
  PopoverSize size,
  Color? color,
) {
  final o = _mapOf(s['overview']);
  return Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _Value(formatNumber(_num(o['total_requests'])), color),
      if (size == PopoverSize.l)
        _Sub(
          '${i18n.t('popover.successRate')} '
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
    return _Section(
      child: Row(
        children: [
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: running ? c.ok : c.fg3,
            ),
          ),
          const SizedBox(width: AidogSpace.sxs),
          Text(
            running ? ltr('Running :${frame.proxyPort}') : 'Stopped',
            style: AidogType.caption.copyWith(color: c.fg2),
          ),
        ],
      ),
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
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final e in frame.entries)
            Padding(
              padding: const EdgeInsets.only(bottom: 2),
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
                  const SizedBox(width: AidogSpace.sxs),
                  if (size != PopoverSize.s)
                    Expanded(
                      child: Text(
                        '${e['name'] ?? ''}',
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.caption.copyWith(color: c.fg3),
                      ),
                    )
                  else
                    const Spacer(),
                  Text(
                    ltr('${e['value'] ?? ''}'),
                    style: numStyle(AidogType.numSm, popoverEntryColor(e['color'], c)),
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
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _Title(i18n.t('popover.platformToday')),
          if (frame.platformToday.isEmpty)
            _Empty(i18n.t('popover.noUsageToday'))
          else
            for (final p in frame.platformToday)
              Padding(
                padding: const EdgeInsets.only(bottom: 2),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        '${p['platform_name'] ?? ''}'.isEmpty
                            ? i18n.t('popover.unknownPlatform')
                            : '${p['platform_name']}',
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.caption.copyWith(color: c.fg3),
                      ),
                    ),
                    Text(
                      ltr(formatCostUsd(_num(p['cost']))),
                      style: numStyle(AidogType.numSm, color ?? c.fg),
                    ),
                    if (size != PopoverSize.s) ...[
                      const SizedBox(width: AidogSpace.sxs),
                      _SubInline('${formatNumber(_num(p['tokens']))} tok'),
                    ],
                    if (size == PopoverSize.l) ...[
                      const SizedBox(width: AidogSpace.sxs),
                      _SubInline(
                        '${formatNumber(_num(p['requests']))} '
                        '${i18n.t('popover.reqUnit')}',
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
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (size != PopoverSize.s)
            _Title(i18n.t('popover.groupBalanceTitle', {'name': name})),
          if (details == null)
            _Empty(i18n.t('common.loading'))
          else if (detail == null)
            _Empty(i18n.t('popover.trendNoGroup'))
          else ...[
            _Value(formatCostUsd(balance), color),
            if (size == PopoverSize.l)
              _Sub(
                '${platforms.length} ${i18n.t('popover.platformsUnit')}',
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
  });

  final String? title;
  final Map<String, Object?>? stats;
  final bool loaded;
  final Widget Function(Map<String, Object?>) builder;

  @override
  Widget build(BuildContext context) {
    final s = stats;
    return _Section(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (title != null) _Title(title!),
          if (loaded && s == null)
            _Empty(i18n.t('popover.trendLoadError'))
          else if (s == null)
            _Empty(i18n.t('common.loading'))
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
    if (size == PopoverSize.s) {
      return _Section(child: _Value(value, color));
    }
    return _Section(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  label,
                  overflow: TextOverflow.ellipsis,
                  style: AidogType.caption.copyWith(color: c.fg3),
                ),
              ),
              _Value(value, color),
            ],
          ),
          if (size == PopoverSize.l) _Sub(sub, null),
        ],
      ),
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Container(
      padding: const EdgeInsets.all(AidogSpace.ssm),
      decoration: BoxDecoration(
        color: t.c.surface,
        borderRadius: BorderRadius.circular(AidogRadius.md),
        border: Border.all(color: t.c.line),
      ),
      child: child,
    );
  }
}

class _Title extends StatelessWidget {
  const _Title(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 4),
    child: Text(
      text,
      overflow: TextOverflow.ellipsis,
      style: AidogType.caption.copyWith(color: AidogTheme.of(context).c.fg3),
    ),
  );
}

class _Value extends StatelessWidget {
  const _Value(this.text, this.color);

  final String text;
  final Color? color;

  @override
  Widget build(BuildContext context) => Text(
    ltr(text),
    overflow: TextOverflow.ellipsis,
    style: numStyle(
      AidogType.numMd,
      color ?? AidogTheme.of(context).c.fg,
    ),
  );
}

class _Sub extends StatelessWidget {
  const _Sub(this.text, this.color);

  final String text;
  final Color? color;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 2),
    child: Text(
      text,
      overflow: TextOverflow.ellipsis,
      style: AidogType.caption.copyWith(
        color: color ?? AidogTheme.of(context).c.fg3,
      ),
    ),
  );
}

class _SubInline extends StatelessWidget {
  const _SubInline(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Text(
    ltr(text),
    style: AidogType.caption.copyWith(color: AidogTheme.of(context).c.fg3),
  );
}

class _Empty extends StatelessWidget {
  const _Empty(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Text(
    text,
    style: AidogType.caption.copyWith(color: AidogTheme.of(context).c.fg3),
  );
}

double _num(Object? v) => (v as num?)?.toDouble() ?? 0;

Map<String, Object?> _mapOf(Object? v) =>
    v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
