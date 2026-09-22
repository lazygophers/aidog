/// 使用统计页（对应 React 版 `src/pages/Stats.tsx`，45.8 KB，全仓最大的一页）。
///
/// 结构与 React 版一一对应：筛选条（时间预设 / 粒度 / 维度 / 分组 / 模型 / 平台 /
/// 计费类型）→ 8 张 Overview 读数格（带环比）→ 主图区四 tab（时间序列 / 占比 /
/// 密度 / 配额）→ 维度排行表（列排序 + 分页）。
///
/// 二次聚合不在这里：`buildTrendChartData` / `buildHeatCells` /
/// `buildDimensionDayCells` / `buildQuotaGauges` 是票 I05 的 `stats/aggregation.dart`。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../charts.dart';
import '../../i18n.dart';
// 聚合层与图表层各有一个 `GaugeTrendPoint`（前者是记录、后者是图表入参类）：
// 本页只构造图表那个，聚合那个藏掉。
import '../../stats/aggregation.dart' hide GaugeTrendPoint;
import '../../stats/models.dart';
import '../../utils/color_level.dart';
import '../../utils/formatters.dart';
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'filter_dropdown.dart';
import 'invoke.dart';
import 'models.dart';
import 'ui_bits.dart' show HoverLift;
import 'stats_logic.dart';

class StatsPage extends StatefulWidget {
  const StatsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.logUpdates,
    this.now = DateTime.now,
    this.initialPlatformId,
    this.initialGroupKey,
  });

  final InvokeFn invoke;
  final Stream<void>? logUpdates;
  final DateTime Function() now;

  /// 从平台卡 / 分组卡跳进来时带的初始筛选（React 版 `initialFilter`）。
  final int? initialPlatformId;
  final String? initialGroupKey;

  @override
  State<StatsPage> createState() => _StatsPageState();
}

class _StatsPageState extends State<StatsPage> {
  StatsResult? _data;
  StatsOverview? _prevOverview;
  bool _loading = true;

  TimePreset _preset = TimePreset.today;
  String _granularity = 'hourly';

  /// 自动降级后**实际生效**的粒度（auto 时 ≠ 用户所选），趋势图标注用。
  String _effectiveGran = 'hourly';

  String _groupBy = 'platform';
  late String _filterGroup = widget.initialGroupKey ?? '';
  String _filterModel = '';
  late String _filterPlatform = widget.initialPlatformId?.toString() ?? '';
  bool _filterCodingPlan = false;

  List<GroupSummary> _groups = const [];
  List<PlatformSummary> _platforms = const [];
  Map<String, List<String>> _protocolTerms = const {};

  SortKey _sortKey = SortKey.totalRequests;
  SortDir _sortDir = SortDir.desc;
  int _page = 0;

  StatsTab _tab = StatsTab.trend;
  bool _trendStacked = false;
  String _densityView = 'heat';
  List<StatsBucket>? _heatBuckets;
  ScatterHistogram? _scatterHist;
  List<QuotaSnapshot>? _quotaSnaps;

  StreamSubscription<void>? _logSub;

  /// 并发刷新去重：上一轮没回来就不发第二轮，避免慢响应乱序覆盖新数据。
  bool _inFlight = false;

  /// 按需查询的世代号：筛选变了就作废在途结果（对应 React effect 的 `cancelled` 标志）。
  int _gen = 0;

  @override
  void initState() {
    super.initState();
    _load();
    _loadFilterOptions();
    _loadProtocolTerms();
    _logSub = (widget.logUpdates ?? debounceStream(kernelProxyLogUpdated()))
        .listen((_) {
          _load(silent: true);
        });
  }

  @override
  void dispose() {
    _logSub?.cancel();
    super.dispose();
  }

  Map<String, Object?> _baseQuery() => baseQuery(
    granularity: _granularity,
    groupBy: _groupBy,
    filterGroup: _filterGroup,
    filterModel: _filterModel,
    filterPlatform: _filterPlatform,
    filterCodingPlan: _filterCodingPlan,
  );

  /// silent=true：后台刷新（代理事件触发），不掀 loading —— 掀了整页会卸载重挂，
  /// 每个代理请求跑完，图就从零重播一遍生长动画（票 11 病灶 A）。
  Future<void> _load({bool silent = false}) async {
    if (_inFlight) return;
    _inFlight = true;
    if (!silent && mounted) setState(() => _loading = true);
    try {
      final range = getTimeRange(_preset, widget.now());
      final base = _baseQuery();
      final prevR = previousRange(range.start, range.end);
      final results = await Future.wait<Object?>([
        widget.invoke('stats_query', {
          'query': {
            ...base,
            'series_by': _groupBy,
            'start': range.start,
            'end': range.end,
          },
        }),
        widget
            .invoke('stats_query', {
              'query': {...base, 'start': prevR.start, 'end': prevR.end},
            })
            .catchError((Object _) => null),
      ]);
      final result = StatsResult.fromJson(results[0]! as Map<String, dynamic>);
      final prev = results[1] == null
          ? null
          : StatsResult.fromJson(results[1]! as Map<String, dynamic>);

      var finalResult = result;
      var finalGran = _granularity;
      if (shouldDowngradeToMinute(
        granularity: _granularity,
        spanMs: range.end - range.start,
        buckets: result.buckets,
      )) {
        final r = await widget
            .invoke('stats_query', {
              'query': {
                ...base,
                'granularity': 'minute',
                'start': range.start,
                'end': range.end,
              },
            })
            .catchError((Object _) => null);
        if (r != null) {
          finalResult = StatsResult.fromJson(r as Map<String, dynamic>);
          finalGran = 'minute';
        }
      }
      if (mounted) {
        setState(() {
          _prevOverview = prev?.overview;
          _data = finalResult;
          _effectiveGran = finalGran;
        });
      }
    } catch (e) {
      // React 版这里是 console.error：保留上一轮数据，不清空、不弹窗。
      debugPrint('stats_query failed: $e');
    }
    _inFlight = false;
    if (!silent && mounted) setState(() => _loading = false);
  }

  Future<void> _loadFilterOptions() async {
    // 注意：筛选选项**不订阅** proxy-log-updated —— 代理跑完一个请求不会改变分组或
    // 平台列表，订阅只会让一次代理事件触发两轮重查（票 11 病灶 B）。
    try {
      final v = await widget.invoke('group_detail_list');
      if (mounted) {
        setState(() {
          _groups = [
            for (final e in v! as List)
              GroupSummary.fromJson(e as Map<String, dynamic>),
          ];
        });
      }
    } catch (_) {
      // 拉不到就是空下拉，不阻断整页。
    }
    try {
      final v = await widget.invoke('platform_list');
      if (mounted) {
        setState(() {
          _platforms = [
            for (final e in v! as List)
              PlatformSummary.fromJson(e as Map<String, dynamic>),
          ];
        });
      }
    } catch (_) {
      // 同上。
    }
  }

  /// 协议跨语言搜索词。实现挪到 `filter_dropdown.dart`，日志页那边的平台下拉
  /// 要同一份（两处各抄一遍必然漂移）。
  Future<void> _loadProtocolTerms() async {
    final out = await loadProtocolTerms(widget.invoke);
    if (mounted) setState(() => _protocolTerms = out);
  }

  /// 密度 tab 按需拉 hourly 桶：粒度选择是主查询概念，热力图需要小时信息，固定 hourly。
  Future<void> _loadHeat(int gen) async {
    final range = getTimeRange(_preset, widget.now());
    try {
      final v = await widget.invoke('stats_query', {
        'query': {
          ..._baseQuery(),
          'granularity': 'hourly',
          'start': range.start,
          'end': range.end,
        },
      });
      final r = StatsResult.fromJson(v! as Map<String, dynamic>);
      if (mounted && gen == _gen) setState(() => _heatBuckets = r.buckets);
    } catch (e) {
      debugPrint('stats_query (heat) failed: $e');
    }
  }

  /// 散点矩阵（D2 bin 化）：filter 语义同主查询（granularity / group_by 不适用）。
  Future<void> _loadScatter(int gen) async {
    final range = getTimeRange(_preset, widget.now());
    final f = _baseQuery();
    try {
      final v = await widget.invoke('scatter_histogram', {
        'query': {
          'start': range.start,
          'end': range.end,
          if (f.containsKey('filter_group')) 'filter_group': f['filter_group'],
          if (f.containsKey('filter_model')) 'filter_model': f['filter_model'],
          if (f.containsKey('filter_platform'))
            'filter_platform': f['filter_platform'],
          if (f.containsKey('filter_coding_plan'))
            'filter_coding_plan': f['filter_coding_plan'],
        },
      });
      final h = ScatterHistogram.fromJson(v! as Map<String, dynamic>);
      if (mounted && gen == _gen) setState(() => _scatterHist = h);
    } catch (e) {
      debugPrint('scatter_histogram failed: $e');
    }
  }

  /// 配额快照序列（D4）：时间窗沿用页面 preset；平台筛选沿用筛选条。
  /// 「无平台」sentinel `"0"`：quota_snapshot 只记真实平台，直接置空走诚实空态，不发请求。
  Future<void> _loadQuota(int gen) async {
    if (_filterPlatform == '0') {
      setState(() => _quotaSnaps = const []);
      return;
    }
    final range = getTimeRange(_preset, widget.now());
    try {
      final v = await widget.invoke('quota_snapshots', {
        'query': {
          'start': range.start,
          'end': range.end,
          'platform_id': _filterPlatform.isEmpty
              ? null
              : int.tryParse(_filterPlatform),
        },
      });
      final list = [
        for (final e in v! as List)
          QuotaSnapshot.fromJson(e as Map<String, dynamic>),
      ];
      if (mounted && gen == _gen) setState(() => _quotaSnaps = list);
    } catch (e) {
      debugPrint('quota_snapshots failed: $e');
    }
  }

  /// tab / 视图 / 筛选变化后，把该 tab 需要的按需查询发一遍（对应 React 的三个 effect）。
  void _refreshTabData() {
    final gen = ++_gen;
    if (_tab == StatsTab.density) {
      _loadHeat(gen);
      if (_densityView == 'scatter') _loadScatter(gen);
    } else if (_tab == StatsTab.quota) {
      _loadQuota(gen);
    }
  }

  /// 筛选变化：重置分页 + 重查主数据 + 重查当前 tab 的按需数据。
  void _onFilterChanged(VoidCallback apply) {
    setState(() {
      apply();
      _page = 0;
    });
    _load();
    _refreshTabData();
  }

  void _setTab(StatsTab tab) {
    setState(() => _tab = tab);
    _refreshTabData();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final tr = AidogI18n.of(context);
    final data = _data;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHead(title: tr.t('page.stats'), subtitle: tr.t('stats.desc')),
        Bento(
          children: [
            BentoCell(span: 12, child: _filters(t, tr)),
            if (_loading)
              BentoCell(
                span: 12,
                child: Tile(
                  child: Center(
                    child: Text(
                      tr.t('stats.loading'),
                      style: AidogType.caption.copyWith(color: t.c.fg2),
                    ),
                  ),
                ),
              )
            else if (data == null)
              BentoCell(
                span: 12,
                child: Tile(
                  child: Center(
                    child: Text(
                      tr.t('stats.noData'),
                      style: AidogType.caption.copyWith(color: t.c.fg2),
                    ),
                  ),
                ),
              )
            else ...[
              ..._overviewCells(t, tr, data.overview),
              BentoCell(span: 12, child: _tabBar(t, tr)),
              ..._tabContent(t, tr, data),
              ..._dimensionTable(t, tr, data),
            ],
          ],
        ),
      ],
    );
  }

  // ── 筛选条 ────────────────────────────────────────────────

  Widget _filters(AidogTheme t, I18nController tr) {
    return Tile(
      child: Wrap(
        spacing: AidogSpace.smd,
        runSpacing: AidogSpace.smd,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final p in TimePreset.values)
                Padding(
                  padding: const EdgeInsets.only(right: AidogSpace.sxs),
                  child: _Pill(
                    label: tr.t(presetKey(p)),
                    active: _preset == p,
                    // 切 preset 联动粒度：today→hourly（24 点），7d/30d→daily；
                    // 手动选粒度仍可覆盖。
                    onTap: () => _onFilterChanged(() {
                      _preset = p;
                      _granularity = granularityForPreset(p);
                    }),
                  ),
                ),
            ],
          ),
          _Select(
            width: 90,
            value: _granularity,
            items: [
              (value: 'daily', label: tr.t('stats.daily')),
              (value: 'hourly', label: tr.t('stats.hourly')),
            ],
            onChanged: (v) => _onFilterChanged(() => _granularity = v),
          ),
          _Select(
            width: 110,
            value: _groupBy,
            items: [
              (value: 'platform', label: tr.t('stats.byPlatform')),
              (value: 'model', label: tr.t('stats.byModel')),
              (value: 'group', label: tr.t('stats.byGroup')),
            ],
            onChanged: (v) => _onFilterChanged(() => _groupBy = v),
          ),
          FilterDropdown(
            width: 140,
            value: _filterGroup,
            onChanged: (v) => _onFilterChanged(() => _filterGroup = v),
            allLabel: tr.t('stats.allGroups'),
            searchPlaceholder: tr.t('stats.searchGroup'),
            emptyLabel: tr.t('stats.noMatch'),
            options: [
              for (final g in _groups)
                FilterOption(value: g.groupKey, label: g.name),
              // 隧道请求无 apikey → group_key=''（sentinel 映射见 baseQuery）
              FilterOption(
                value: kNoGroupSentinel,
                label: tr.t('stats.noGroup'),
              ),
            ],
          ),
          FilterDropdown(
            width: 170,
            value: _filterModel,
            onChanged: (v) => _onFilterChanged(() => _filterModel = v),
            allLabel: tr.t('stats.allModels'),
            searchPlaceholder: tr.t('stats.searchModel'),
            emptyLabel: tr.t('stats.noMatch'),
            options: [
              // 模型筛选项来自实际 proxy_log 记录（后端 available_models），非配置列表。
              for (final m
                  in (_data?.availableModels.toList(growable: false) ?? [])
                    ..sort())
                FilterOption(value: m, label: m),
            ],
          ),
          FilterDropdown(
            width: 140,
            value: _filterPlatform,
            onChanged: (v) => _onFilterChanged(() => _filterPlatform = v),
            allLabel: tr.t('stats.allPlatforms'),
            searchPlaceholder: tr.t('stats.searchPlatform'),
            emptyLabel: tr.t('stats.noMatch'),
            options: [
              for (final p in _platforms)
                FilterOption(
                  value: p.id.toString(),
                  label: p.name,
                  searchTerms: _protocolTerms[p.platformType] ?? const [],
                ),
              // 「无平台」(platform_id=0)：隧道请求 host 未命中任何平台。
              FilterOption(value: '0', label: tr.t('stats.noPlatform')),
            ],
          ),
          _Select(
            width: 130,
            value: _filterCodingPlan ? 'coding' : 'all',
            items: [
              (value: 'all', label: tr.t('stats.planAll')),
              (value: 'coding', label: tr.t('stats.codingPlanOnly')),
            ],
            onChanged: (v) =>
                _onFilterChanged(() => _filterCodingPlan = v == 'coding'),
          ),
        ],
      ),
    );
  }

  // ── Overview 八卡 ─────────────────────────────────────────

  List<BentoCell> _overviewCells(
    AidogTheme t,
    I18nController tr,
    StatsOverview o,
  ) {
    final p = _prevOverview;
    Widget card(
      String label,
      String value, {
      String? unit,
      ColorLevel? level,
      double? deltaPct,
      bool inverse = false,
    }) => _OverviewCard(
      label: label,
      value: value,
      unit: unit,
      level: level,
      delta: deltaPct,
      deltaInverse: inverse,
      deltaNote: tr.t('stats.vsPrevPeriod'),
    );

    return [
      BentoCell(
        span: 3,
        child: card(
          tr.t('stats.totalRequests'),
          formatNumber(o.totalRequests),
          deltaPct: delta(
            o.totalRequests.toDouble(),
            (p?.totalRequests ?? 0).toDouble(),
          ),
        ),
      ),
      BentoCell(
        span: 3,
        child: card(
          tr.t('stats.successRate'),
          o.successRate.toStringAsFixed(1),
          unit: '%',
          level: successRateLevel(o.successRate, o.totalRequests),
          deltaPct: delta(o.successRate, p?.successRate ?? 0),
        ),
      ),
      BentoCell(
        span: 3,
        child: card(
          tr.t('stats.inputTokens'),
          formatNumber(o.totalInputTokens),
          deltaPct: delta(
            o.totalInputTokens.toDouble(),
            (p?.totalInputTokens ?? 0).toDouble(),
          ),
        ),
      ),
      BentoCell(
        span: 3,
        child: card(
          tr.t('stats.outputTokens'),
          formatNumber(o.totalOutputTokens),
          deltaPct: delta(
            o.totalOutputTokens.toDouble(),
            (p?.totalOutputTokens ?? 0).toDouble(),
          ),
        ),
      ),
      BentoCell(
        span: 3,
        child: card(
          tr.t('stats.cacheTokens'),
          formatNumber(o.totalCacheTokens),
          deltaPct: delta(
            o.totalCacheTokens.toDouble(),
            (p?.totalCacheTokens ?? 0).toDouble(),
          ),
        ),
      ),
      BentoCell(
        span: 3,
        child: card(
          tr.t('stats.cacheRate'),
          o.cacheRate.toStringAsFixed(1),
          unit: '%',
          deltaPct: delta(o.cacheRate, p?.cacheRate ?? 0),
        ),
      ),
      BentoCell(
        span: 3,
        child: card(
          tr.t('stats.avgLatency'),
          o.avgDurationMs.toStringAsFixed(0),
          unit: 'ms',
          deltaPct: delta(o.avgDurationMs, p?.avgDurationMs ?? 0),
          inverse: true,
        ),
      ),
      BentoCell(
        span: 3,
        child: card(
          tr.t('stats.totalCost'),
          '\$${formatCost(o.totalCost)}',
          level: costLevel(o.totalCost),
          deltaPct: delta(o.totalCost, p?.totalCost ?? 0),
          inverse: true,
        ),
      ),
    ];
  }

  // ── 主图区四 tab ──────────────────────────────────────────

  Widget _tabBar(AidogTheme t, I18nController tr) => Tile(
    child: Wrap(
      spacing: AidogSpace.sxs,
      runSpacing: AidogSpace.sxs,
      children: [
        for (final tab in StatsTab.values)
          Semantics(
            selected: _tab == tab,
            button: true,
            child: _Pill(
              label: tr.t(statsTabKey(tab)),
              active: _tab == tab,
              onTap: () => _setTab(tab),
            ),
          ),
      ],
    ),
  );

  String _granLabel(I18nController tr) {
    final base = tr.t(granularityKey(_effectiveGran));
    return _effectiveGran != _granularity
        ? tr.t('stats.granAuto', {'g': base})
        : base;
  }

  String _byLabel(I18nController tr) =>
      tr.t('stats.by${_groupBy[0].toUpperCase()}${_groupBy.substring(1)}');

  List<BentoCell> _tabContent(
    AidogTheme t,
    I18nController tr,
    StatsResult data,
  ) {
    final unknown = tr.t('popover.unknownPlatform');
    final dims = normalizeDimNames(data.dimensionData, unknown);
    final series = normalizeSeriesNames(data.series, unknown);
    final palette = ChartPalette.of(context);

    switch (_tab) {
      case StatsTab.trend:
        final trend = buildTrendChartData(
          data.buckets,
          series,
          tr.t('stats.requests'),
        );
        final keys = trend.config.keys.toList(growable: false);
        final chartSeries = [
          for (var i = 0; i < keys.length; i++)
            ChartSeries(
              key: keys[i],
              label: trend.config[keys[i]]!,
              color: palette.series(i),
              // 宽表是**稀疏**的：某个平台在某个时段没有流量，那一行就没有它的键
              //（`buildTrendChartData` 照抄 React 的构表方式）。这里必须**每行都出一个点**，
              // 否则各系列点数不等，`downsampleAligned` 会按契约抛
              //「宽表语义要求各系列共用同一批 x」——真实数据一进来就崩。
              //
              // 缺口画成**断线**，不是落到 0：这一列是请求数，
              // 「那个时段没有这个平台的数据」与「那个时段是 0 次请求」不是一回事。
              // recharts 默认 `connectNulls={false}`（`LineChart.tsx:201`）就是断线，
              // 这里用 `ChartPoint.missing` 表达同一件事（堆叠图仍按 0 堆，同 React）。
              points: [
                for (final row in trend.rows)
                  if (row[keys[i]] == null)
                    ChartPoint.missing(row['x']!.toDouble())
                  else
                    ChartPoint(row['x']!.toDouble(), row[keys[i]]!.toDouble()),
              ],
              format: formatNumber,
            ),
        ];
        var meta = '${tr.t('stats.granularityLabel')}: ${_granLabel(tr)}';
        if (isFineGranularity(_effectiveGran)) {
          meta = '$meta ${tr.t('stats.fineGranBadge')}';
        }
        // 点太多时图会降采样。不写出来的话，用户看到的是抽样曲线而不自知
        //（`LineChart.tsx:111-115` 把这句写在副标题里）。
        final kept = downsampledPointCount(
          chartSeries,
          yOf: _trendStacked && trend.multi ? rowTotalOf(chartSeries) : null,
        );
        if (kept != null) {
          meta = '$meta · ${tr.t('charts.downsampled', {'count': '$kept'})}';
        }
        return [
          if (trend.multi)
            BentoCell(
              span: 12,
              child: Tile(
                // 这两颗按钮是一组「视图」开关，读屏要读得出这一组是什么
                //（React 挂在 `role="group"` 上的 aria-label，`Stats.tsx:688`）。
                child: Semantics(
                  label: tr.t('stats.viewMode'),
                  container: true,
                  child: Wrap(
                    spacing: AidogSpace.sxs,
                    children: [
                      _Pill(
                        label: tr.t('stats.viewLine'),
                        active: !_trendStacked,
                        onTap: () => setState(() => _trendStacked = false),
                      ),
                      _Pill(
                        label: tr.t('stats.viewStacked'),
                        active: _trendStacked,
                        onTap: () => setState(() => _trendStacked = true),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          BentoCell(
            span: 12,
            child: Tooltip(
              message: isFineGranularity(_effectiveGran)
                  ? tr.t('stats.fineGranHint')
                  : '',
              child: SeriesTile(
                title: tr.t('stats.requestTrend'),
                meta: meta,
                chartHeight: 260,
                legend: [
                  for (final s in chartSeries) (color: s.color, label: s.label),
                ],
                chart: _trendStacked && trend.multi
                    ? AidogStackedAreaChart(
                        series: chartSeries,
                        emptyText: tr.t('stats.noData'),
                      )
                    : AidogLineChart(
                        series: chartSeries,
                        emptyText: tr.t('stats.noData'),
                      ),
              ),
            ),
          ),
        ];

      case StatsTab.share:
        return [
          BentoCell(
            span: 6,
            child: SeriesTile(
              title: '${tr.t('stats.costShare')} — ${_byLabel(tr)}',
              chartHeight: 260,
              chart: AidogDonutChart(
                data: [
                  for (final d in dims)
                    if (d.totalCost > 0) (name: d.name, value: d.totalCost),
                ],
                restLabel: tr.t('stats.donutRest'),
                formatValue: formatCostUsd,
                centerLabel: tr.t('stats.totalCost'),
                emptyText: tr.t('stats.noData'),
              ),
            ),
          ),
          BentoCell(
            span: 6,
            child: SeriesTile(
              title: tr.t('stats.dimHeatTitle'),
              chartHeight: 260,
              chart: DimensionHeatmap(
                data: [
                  for (final c in buildDimensionDayCells(series))
                    (
                      name: c.name,
                      day: c.day.toDouble(),
                      value: c.value.toDouble(),
                    ),
                ],
                formatValue: formatNumber,
                emptyText: tr.t('stats.noData'),
              ),
            ),
          ),
        ];

      case StatsTab.density:
        return [
          BentoCell(
            span: 12,
            child: Tile(
              child: Wrap(
                spacing: AidogSpace.sxs,
                children: [
                  _Pill(
                    label: tr.t('stats.densityViewHeat'),
                    active: _densityView == 'heat',
                    onTap: () {
                      setState(() => _densityView = 'heat');
                      _refreshTabData();
                    },
                  ),
                  _Pill(
                    label: tr.t('stats.densityViewScatter'),
                    active: _densityView == 'scatter',
                    onTap: () {
                      setState(() => _densityView = 'scatter');
                      _refreshTabData();
                    },
                  ),
                ],
              ),
            ),
          ),
          BentoCell(
            span: 12,
            child: _densityView == 'heat'
                ? SeriesTile(
                    title: tr.t('stats.heatTitle'),
                    chartHeight: 260,
                    chart: HourHeatmap(
                      data: [
                        for (final c in buildHeatCells(
                          _heatBuckets ?? const [],
                        ))
                          (day: c.day, hour: c.hour, value: c.value.toDouble()),
                      ],
                      dayLabel: (d) => weekdayShort(context, d),
                      formatValue: formatNumber,
                      emptyText: tr.t('stats.noData'),
                    ),
                  )
                : SeriesTile(
                    title: tr.t('stats.scatterTitle'),
                    chartHeight: 300,
                    chart: AidogScatterChart(
                      histogram:
                          _scatterHist ??
                          const ScatterHistogram(
                            durationBins: [],
                            costBins: [],
                            counts: [],
                          ),
                      xLabel: tr.t('charts.scatterX'),
                      yLabel: tr.t('charts.scatterY'),
                      countLabel: (n) =>
                          tr.t('charts.scatterCount', {'count': n}),
                      emptyText: tr.t('stats.noData'),
                    ),
                  ),
          ),
        ];

      case StatsTab.quota:
        final gauges = buildQuotaGauges(_quotaSnaps ?? const []);
        if (gauges.isEmpty) {
          return [
            BentoCell(
              span: 12,
              child: SeriesTile(
                title: tr.t('stats.quotaTitle'),
                chartHeight: 200,
                chart: GaugeChart(
                  value: 0,
                  max: 0,
                  formatValue: formatCostUsd,
                  emptyText: tr.t('stats.quotaTitle'),
                  emptyHint: tr.t('stats.quotaEmptyHint'),
                ),
              ),
            ),
          ];
        }
        return [
          BentoCell(
            span: 12,
            child: Tile(
              title: tr.t('stats.quotaTitle'),
              meta: tr.t('stats.quotaPeakNote'),
              child: Wrap(
                spacing: AidogSpace.smd,
                runSpacing: AidogSpace.smd,
                children: [
                  for (final g in gauges)
                    // 平台名画在环**上方**（`Stats.tsx:853-860` 的 `title`），
                    // 不是塞进环心；整块带悬停抬升（同处 `className="hover-lift"`）。
                    HoverLift(
                      child: GaugeChart(
                        value: g.current,
                        max: g.peak,
                        formatValue: formatCostUsd,
                        title:
                            _platforms
                                .where((p) => p.id == g.platformId)
                                .firstOrNull
                                ?.name ??
                            '#${g.platformId}',
                        trend: [
                          for (final p in g.trend)
                            GaugeTrendPoint(
                              at: p.at.toDouble(),
                              fraction: p.fraction,
                            ),
                        ],
                      ),
                    ),
                ],
              ),
            ),
          ),
        ];
    }
  }

  // ── 维度排行表 ────────────────────────────────────────────

  List<BentoCell> _dimensionTable(
    AidogTheme t,
    I18nController tr,
    StatsResult data,
  ) {
    final dims = normalizeDimNames(
      data.dimensionData,
      tr.t('popover.unknownPlatform'),
    );
    if (dims.isEmpty) return const [];
    final sorted = sortDimensions(dims, _sortKey, _sortDir);
    final pg = paginate(sorted, _page);

    Widget head(String label, SortKey col) => _SortHead(
      label: label,
      active: _sortKey == col,
      dir: _sortDir,
      onTap: () {
        final next = nextSort(_sortKey, _sortDir, col);
        setState(() {
          _sortKey = next.key;
          _sortDir = next.dir;
          _page = 0;
        });
      },
    );

    // 表头是可点的排序控件，所以用 ListingTile.table 的 columns（纯文本）放不下 ——
    // 标题行自己排一行 widget，正文仍走 ListingTile.table。
    return [
      BentoCell(
        span: 12,
        child: ListingTile(
          title: '${tr.t('stats.dimensionRank')} — ${_byLabel(tr)}',
          meta: tr.t('stats.totalRows', {'count': sorted.length}),
          rows: [
            Row(
              children: [
                Expanded(
                  flex: 3,
                  child: head(tr.t('stats.dimName'), SortKey.name),
                ),
                Expanded(
                  child: head(tr.t('stats.requests'), SortKey.totalRequests),
                ),
                Expanded(
                  child: head(tr.t('stats.success'), SortKey.successCount),
                ),
                Expanded(
                  child: head(tr.t('stats.inputTokens'), SortKey.inputTokens),
                ),
                Expanded(
                  child: head(tr.t('stats.outputTokens'), SortKey.outputTokens),
                ),
                Expanded(
                  child: head(tr.t('stats.cacheTokens'), SortKey.cacheTokens),
                ),
                Expanded(
                  child: head(tr.t('stats.cacheRate'), SortKey.cacheRate),
                ),
                Expanded(
                  child: head(tr.t('stats.avgMs'), SortKey.avgDurationMs),
                ),
                Expanded(
                  child: head(tr.t('stats.totalCost'), SortKey.totalCost),
                ),
              ],
            ),
            for (final d in pg.rows) _dimRow(t, d),
          ],
          footer: pg.pageCount > 1
              ? Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    _Pill(
                      label: tr.t('stats.prevPage'),
                      active: false,
                      onTap: pg.safePage <= 0
                          ? null
                          : () => setState(() => _page = pg.safePage - 1),
                    ),
                    const SizedBox(width: AidogSpace.ssm),
                    Ltr(
                      child: Text(
                        tr.t('stats.pageOf', {
                          'current': pg.safePage + 1,
                          'total': pg.pageCount,
                        }),
                        style: numStyle(AidogType.numSm, t.c.fg2),
                      ),
                    ),
                    const SizedBox(width: AidogSpace.ssm),
                    _Pill(
                      label: tr.t('stats.nextPage'),
                      active: false,
                      onTap: pg.safePage >= pg.pageCount - 1
                          ? null
                          : () => setState(() => _page = pg.safePage + 1),
                    ),
                  ],
                )
              : null,
        ),
      ),
    ];
  }

  Widget _dimRow(AidogTheme t, DimensionEntry d) {
    final rate = successRate(d.successCount, d.totalRequests);
    Widget num(String s, [Color? color]) => Align(
      alignment: AlignmentDirectional.centerEnd,
      child: Ltr(
        child: Text(s, style: numStyle(AidogType.numSm, color ?? t.c.fg)),
      ),
    );
    return Row(
      children: [
        Expanded(
          flex: 3,
          child: Text(
            d.name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: AidogType.caption.copyWith(
              color: t.c.fg,
              fontWeight: FontWeight.w500,
            ),
          ),
        ),
        Expanded(child: num(formatNumber(d.totalRequests))),
        Expanded(
          child: num(
            formatNumber(d.successCount),
            levelColor(successRateLevel(rate, d.totalRequests), t.c),
          ),
        ),
        Expanded(child: num(formatNumber(d.inputTokens))),
        Expanded(child: num(formatNumber(d.outputTokens))),
        Expanded(child: num(formatNumber(d.cacheTokens))),
        Expanded(child: num('${d.cacheRate.toStringAsFixed(1)}%')),
        Expanded(child: num('${d.avgDurationMs.toStringAsFixed(0)} ms')),
        Expanded(
          child: num(
            '\$${formatCost(d.totalCost)}',
            levelColor(costLevel(d.totalCost), t.c),
          ),
        ),
      ],
    );
  }
}

// ── 小部件 ────────────────────────────────────────────────

/// Overview 读数卡：值 + 可选单位 + 色编码 + 环比 delta。
/// delta 绝对值 < 0.05 不显示（对齐 React 的 `Math.abs(delta) >= 0.05`）。
class _OverviewCard extends StatelessWidget {
  const _OverviewCard({
    required this.label,
    required this.value,
    required this.deltaNote,
    this.unit,
    this.level,
    this.delta,
    this.deltaInverse = false,
  });

  final String label;
  final String value;
  final String? unit;
  final ColorLevel? level;
  final double? delta;
  final bool deltaInverse;
  final String deltaNote;

  @override
  Widget build(BuildContext context) {
    final d = delta;
    final show = d != null && d.abs() >= 0.05;
    // 正常指标：上升 = 好（ok）；反向指标（成本 / 延迟）：上升 = 差（bad）。
    final up = show && d > 0;
    final good = deltaInverse ? !up : up;
    return ReadoutTile(
      label: label,
      value: unit == null ? value : '$value$unit',
      delta: show ? '${up ? '+' : '-'}${d.abs().toStringAsFixed(1)}%' : null,
      trend: show ? (good ? Trend.up : Trend.down) : Trend.flat,
      deltaNote: show ? deltaNote : null,
    );
  }
}

/// 胶囊按钮（时间预设 / tab / 视图切换 / 翻页）。[onTap] 为 null = 禁用态。
class _Pill extends StatelessWidget {
  const _Pill({required this.label, required this.active, required this.onTap});

  final String label;
  final bool active;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final disabled = onTap == null;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(AidogRadius.sm),
      child: Container(
        padding: const EdgeInsets.symmetric(
          horizontal: AidogSpace.smd,
          vertical: AidogSpace.sxs,
        ),
        decoration: BoxDecoration(
          color: active ? t.c.accentWash : null,
          border: Border.all(color: active ? t.c.accent : t.c.line),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: Text(
          label,
          style: AidogType.caption.copyWith(
            color: disabled
                ? t.c.fg3
                : active
                ? t.c.accent
                : t.c.fg2,
          ),
        ),
      ),
    );
  }
}

/// 无搜索的下拉（粒度 / 维度 / 计费类型）。走 Material 的 `DropdownButton`，
/// 不自造第二套弹层。
class _Select extends StatelessWidget {
  const _Select({
    required this.width,
    required this.value,
    required this.items,
    required this.onChanged,
  });

  final double width;
  final String value;
  final List<({String value, String label})> items;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Container(
      width: width,
      height: 30,
      padding: const EdgeInsets.symmetric(horizontal: AidogSpace.ssm),
      decoration: BoxDecoration(
        color: t.c.surface2,
        border: Border.all(color: t.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: DropdownButtonHideUnderline(
        child: DropdownButton<String>(
          value: value,
          isExpanded: true,
          isDense: true,
          dropdownColor: t.c.surface2,
          iconSize: 16,
          iconEnabledColor: t.c.fg3,
          style: AidogType.caption.copyWith(color: t.c.fg),
          items: [
            for (final i in items)
              DropdownMenuItem<String>(
                value: i.value,
                child: Text(i.label, overflow: TextOverflow.ellipsis),
              ),
          ],
          onChanged: (v) {
            if (v != null) onChanged(v);
          },
        ),
      ),
    );
  }
}

/// 可排序表头：点一下换列 / 反向，当前列带方向箭头。
class _SortHead extends StatelessWidget {
  const _SortHead({
    required this.label,
    required this.active,
    required this.dir,
    required this.onTap,
  });

  final String label;
  final bool active;
  final SortDir dir;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return InkWell(
      onTap: onTap,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Flexible(
            child: Text(
              label.toUpperCase(),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AidogType.micro.copyWith(
                color: active ? t.c.accent : t.c.fg3,
              ),
            ),
          ),
          if (active)
            Icon(
              dir == SortDir.asc ? Icons.arrow_upward : Icons.arrow_downward,
              size: 11,
              color: t.c.accent,
            ),
        ],
      ),
    );
  }
}
