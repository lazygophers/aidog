/// 使用统计页的纯逻辑（对应 React 版 `src/pages/Stats.tsx` 里不碰 DOM 的那部分）。
/// 四个二次聚合函数不在这里 —— 它们是票 I05 的 `stats/aggregation.dart`，本文件不重造。
library;

import 'package:flutter/material.dart';

import '../../stats/models.dart';

/// 本地化的星期名，[day] 按 JS `getDay()` 约定 0 = 周日（`narrowWeekdays` 的下标正是这个）。
///
/// **与 React 版的一处差异**：那边是 `Intl.DateTimeFormat(lang, {weekday:'short'})`，
/// 出「周日 / Sun」这种缩写；Flutter 的 `MaterialLocalizations` 只提供 narrow（单字）。
/// 走它是因为 8 种语言（含阿拉伯语）的词条由 `flutter_localizations` 自带、无需任何
/// 运行时初始化；改用 `intl` 的 `DateFormat.E` 得先 `await initializeDateFormatting()`，
/// 漏调就在非英文 locale 下抛 `LocaleDataException`（本票实测踩到）。
/// 热力图行标只有 34px 宽，单字反而更合适。
String weekdayShort(BuildContext context, int day) =>
    MaterialLocalizations.of(context).narrowWeekdays[day];

/// 时间预设（React 版 `TimePreset`）。
enum TimePreset { today, sevenDays, thirtyDays }

/// 预设 → i18n key 后缀（`stats.today` / `stats.7d` / `stats.30d`）。
String presetKey(TimePreset p) => switch (p) {
  TimePreset.today => 'stats.today',
  TimePreset.sevenDays => 'stats.7d',
  TimePreset.thirtyDays => 'stats.30d',
};

/// 预设 → `[start, end]` 毫秒窗口。
/// today = 本地当日零点 → 现在；7d / 30d = 现在往前推 N 天 → 现在（**不**对齐到零点，
/// 与 React 的 `setDate(getDate() - 7)` 同语义：保留时分秒）。
({int start, int end}) getTimeRange(TimePreset preset, [DateTime? now]) {
  final n = now ?? DateTime.now();
  final end = n.millisecondsSinceEpoch;
  return switch (preset) {
    TimePreset.today => (
      start: DateTime(n.year, n.month, n.day).millisecondsSinceEpoch,
      end: end,
    ),
    // `DateTime(y, m, d - 7, ...)` 会自动跨月/跨年归一，等价 JS 的 setDate。
    TimePreset.sevenDays => (
      start: DateTime(
        n.year,
        n.month,
        n.day - 7,
        n.hour,
        n.minute,
        n.second,
        n.millisecond,
      ).millisecondsSinceEpoch,
      end: end,
    ),
    TimePreset.thirtyDays => (
      start: DateTime(
        n.year,
        n.month,
        n.day - 30,
        n.hour,
        n.minute,
        n.second,
        n.millisecond,
      ).millisecondsSinceEpoch,
      end: end,
    ),
  };
}

/// 上一等长周期：把当前 `[start, end]` 整体往前平移一个窗口长度。
({int start, int end}) previousRange(int start, int end) =>
    (start: start - (end - start), end: start);

/// 环比增减：`(cur - prev) / prev × 100`。prev ≤ 0 → null（无对比基准，隐藏 delta）。
double? delta(double cur, double prev) =>
    prev > 0 ? ((cur - prev) / prev) * 100 : null;

/// 切 preset 联动的粒度：today → hourly，7d / 30d → daily（手动选仍可覆盖）。
String granularityForPreset(TimePreset p) =>
    p == TimePreset.today ? 'hourly' : 'daily';

/// 主图区四 tab（spec C1）。
enum StatsTab { trend, share, density, quota }

String statsTabKey(StatsTab tab) => switch (tab) {
  StatsTab.trend => 'stats.tabTrend',
  StatsTab.share => 'stats.tabShare',
  StatsTab.density => 'stats.tabDensity',
  StatsTab.quota => 'stats.tabQuota',
};

/// 粒度 → i18n key。auto 降级时调用方再套 `stats.granAuto`。
String granularityKey(String g) => switch (g) {
  'minute' => 'stats.granMinute',
  '5min' => 'stats.gran5min',
  'hourly' => 'stats.granHourly',
  _ => 'stats.granDaily',
};

/// 分钟级数据来自 proxy_log，只短期可用 —— 要不要挂那条提示。
bool isFineGranularity(String g) => g == 'minute' || g == '5min';

/// 「无分组」sentinel：下拉选它 → `filter_group=''`（隧道请求 group_key 为空）。
/// `"0"` 在平台筛选是 truthy，直接透传后端 `CAST AS INTEGER = 0`（无平台）。
const String kNoGroupSentinel = '__none__';

/// 筛选条公共查询体（主查询 / 环比查询 / 密度 hourly 查询共用）。
/// 空串筛选项**不进 map**（对齐 React 的 `|| undefined`：不传 ≠ 传空串）。
Map<String, Object?> baseQuery({
  required String granularity,
  required String groupBy,
  required String filterGroup,
  required String filterModel,
  required String filterPlatform,
  required bool filterCodingPlan,
}) => {
  'granularity': granularity,
  'group_by': groupBy,
  if (filterGroup.isNotEmpty)
    'filter_group': filterGroup == kNoGroupSentinel ? '' : filterGroup,
  if (filterModel.isNotEmpty) 'filter_model': filterModel,
  if (filterPlatform.isNotEmpty) 'filter_platform': filterPlatform,
  if (filterCodingPlan) 'filter_coding_plan': true,
};

/// 自动降级粒度的判据（React 版 `load()` 里那三个条件）：
/// 用户选「按小时」 + 窗口 ≤ 24h + 非空桶 < 4 → 改走 proxy_log 的 minute 查询。
/// 7d / 30d 长范围绝不降到 minute（防桶爆炸）。
bool shouldDowngradeToMinute({
  required String granularity,
  required int spanMs,
  required List<StatsBucket> buckets,
}) {
  if (granularity != 'hourly') return false;
  if (spanMs > 24 * 60 * 60 * 1000) return false;
  return buckets.where((b) => b.totalRequests > 0).length < 4;
}

/// 维度表可排序的列。
enum SortKey {
  name,
  totalRequests,
  successCount,
  inputTokens,
  outputTokens,
  cacheTokens,
  cacheRate,
  avgDurationMs,
  totalCost,
}

enum SortDir { asc, desc }

double _numOf(DimensionEntry d, SortKey k) => switch (k) {
  SortKey.name => 0,
  SortKey.totalRequests => d.totalRequests.toDouble(),
  SortKey.successCount => d.successCount.toDouble(),
  SortKey.inputTokens => d.inputTokens.toDouble(),
  SortKey.outputTokens => d.outputTokens.toDouble(),
  SortKey.cacheTokens => d.cacheTokens.toDouble(),
  SortKey.cacheRate => d.cacheRate,
  SortKey.avgDurationMs => d.avgDurationMs,
  SortKey.totalCost => d.totalCost,
};

/// 维度表排序。name 列按串比较（`localeCompare` 对应 Dart 的 `compareTo`），其余按数值。
/// 并列保持首现序（JS sort 稳定、Dart 不保证，按下标兜底）。
List<DimensionEntry> sortDimensions(
  List<DimensionEntry> dims,
  SortKey key,
  SortDir dir,
) {
  final indexed = [for (var (i, d) in dims.indexed) (i, d)];
  indexed.sort((a, b) {
    final cmp = key == SortKey.name
        ? a.$2.name.compareTo(b.$2.name)
        : _numOf(a.$2, key).compareTo(_numOf(b.$2, key));
    final signed = dir == SortDir.asc ? cmp : -cmp;
    return signed != 0 ? signed : a.$1.compareTo(b.$1);
  });
  return [for (final e in indexed) e.$2];
}

/// 点列头的下一个排序态：同列反向，换列则 name 升序、数值列降序。
({SortKey key, SortDir dir}) nextSort(
  SortKey current,
  SortDir dir,
  SortKey tapped,
) {
  if (current == tapped) {
    return (
      key: current,
      dir: dir == SortDir.asc ? SortDir.desc : SortDir.asc,
    );
  }
  return (
    key: tapped,
    dir: tapped == SortKey.name ? SortDir.asc : SortDir.desc,
  );
}

/// 每页行数（React 版 `PAGE_SIZE = 50`）。
const int kPageSize = 50;

/// 分页：页数至少 1；当前页越界时夹到最后一页（`safePage`）。
({int pageCount, int safePage, List<DimensionEntry> rows}) paginate(
  List<DimensionEntry> sorted,
  int page, [
  int pageSize = kPageSize,
]) {
  final pageCount = (sorted.length / pageSize).ceil().clamp(1, 1 << 30);
  final safePage = page < pageCount - 1 ? page : pageCount - 1;
  final from = safePage * pageSize;
  return (
    pageCount: pageCount,
    safePage: safePage,
    rows: sorted.sublist(
      from.clamp(0, sorted.length),
      (from + pageSize).clamp(0, sorted.length),
    ),
  );
}

/// 维度名空串归一化（后端回溯失败返空串）：donut / 维度表 / 趋势图例 / 维度热力四处共用。
List<DimensionEntry> normalizeDimNames(
  List<DimensionEntry> dims,
  String unknown,
) => [
  for (final d in dims)
    if (d.name.isEmpty) d.withName(unknown) else d,
];

/// series 的同一归一化。
List<StatsSeries> normalizeSeriesNames(
  List<StatsSeries> series,
  String unknown,
) => [
  for (final s in series)
    if (s.name.isEmpty) StatsSeries(name: unknown, buckets: s.buckets) else s,
];
