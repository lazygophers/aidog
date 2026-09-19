// ── 统计 RPC 载荷模型（对应 Rust ts-rs 生成的 StatsBucket / StatsSeries /
// QuotaSnapshot / ScatterHistogram，字段名按 wire 的 snake_case 解码）──
// 后端聚合仍在 Rust（aidog_stats），这里只是二次聚合层的输入形状。

/// 一个时间桶的统计量。`timeBucket` 串形如 `YYYY-MM-DD` / `YYYY-MM-DD HH:MM` /
/// `YYYY-MM-DD HH:00:00`，解析走 `charts/ticks.dart::bucketMs`。
class StatsBucket {
  const StatsBucket({
    required this.timeBucket,
    required this.totalRequests,
    this.successCount = 0,
    this.errorCount = 0,
    this.inputTokens = 0,
    this.outputTokens = 0,
    this.cacheTokens = 0,
    this.avgDurationMs = 0,
    this.totalCost = 0,
  });

  factory StatsBucket.fromJson(Map<String, dynamic> j) => StatsBucket(
    timeBucket: j['time_bucket'] as String,
    totalRequests: (j['total_requests'] as num).toInt(),
    successCount: (j['success_count'] as num?)?.toInt() ?? 0,
    errorCount: (j['error_count'] as num?)?.toInt() ?? 0,
    inputTokens: (j['input_tokens'] as num?)?.toInt() ?? 0,
    outputTokens: (j['output_tokens'] as num?)?.toInt() ?? 0,
    cacheTokens: (j['cache_tokens'] as num?)?.toInt() ?? 0,
    avgDurationMs: (j['avg_duration_ms'] as num?)?.toDouble() ?? 0,
    totalCost: (j['total_cost'] as num?)?.toDouble() ?? 0,
  );

  final String timeBucket;
  final int totalRequests;
  final int successCount;
  final int errorCount;
  final int inputTokens;
  final int outputTokens;
  final int cacheTokens;
  final double avgDurationMs;
  final double totalCost;
}

/// 一个维度值的时间序列（platform 维度 = 平台名；model / group 维度 = 列值）。
class StatsSeries {
  const StatsSeries({required this.name, required this.buckets});

  factory StatsSeries.fromJson(Map<String, dynamic> j) => StatsSeries(
    name: j['name'] as String,
    buckets: [
      for (final b in j['buckets'] as List)
        StatsBucket.fromJson(b as Map<String, dynamic>),
    ],
  );

  final String name;
  final List<StatsBucket> buckets;
}

/// `generated/StatsOverview.ts`：整窗口的汇总量（Overview 卡 + 环比基准）。
class StatsOverview {
  const StatsOverview({
    required this.totalRequests,
    required this.successRate,
    required this.totalInputTokens,
    required this.totalOutputTokens,
    required this.totalCacheTokens,
    required this.cacheRate,
    required this.avgDurationMs,
    required this.totalCost,
  });

  factory StatsOverview.fromJson(Map<String, dynamic> j) => StatsOverview(
    totalRequests: (j['total_requests'] as num?)?.toInt() ?? 0,
    successRate: (j['success_rate'] as num?)?.toDouble() ?? 0,
    totalInputTokens: (j['total_input_tokens'] as num?)?.toInt() ?? 0,
    totalOutputTokens: (j['total_output_tokens'] as num?)?.toInt() ?? 0,
    totalCacheTokens: (j['total_cache_tokens'] as num?)?.toInt() ?? 0,
    cacheRate: (j['cache_rate'] as num?)?.toDouble() ?? 0,
    avgDurationMs: (j['avg_duration_ms'] as num?)?.toDouble() ?? 0,
    totalCost: (j['total_cost'] as num?)?.toDouble() ?? 0,
  );

  final int totalRequests;
  final double successRate;
  final int totalInputTokens;
  final int totalOutputTokens;
  final int totalCacheTokens;
  final double cacheRate;
  final double avgDurationMs;
  final double totalCost;
}

/// `generated/DimensionEntry.ts`：维度排行表的一行。
class DimensionEntry {
  const DimensionEntry({
    required this.name,
    required this.totalRequests,
    required this.successCount,
    required this.inputTokens,
    required this.outputTokens,
    required this.cacheTokens,
    required this.cacheRate,
    required this.avgDurationMs,
    required this.totalCost,
  });

  factory DimensionEntry.fromJson(Map<String, dynamic> j) => DimensionEntry(
    name: (j['name'] as String?) ?? '',
    totalRequests: (j['total_requests'] as num?)?.toInt() ?? 0,
    successCount: (j['success_count'] as num?)?.toInt() ?? 0,
    inputTokens: (j['input_tokens'] as num?)?.toInt() ?? 0,
    outputTokens: (j['output_tokens'] as num?)?.toInt() ?? 0,
    cacheTokens: (j['cache_tokens'] as num?)?.toInt() ?? 0,
    cacheRate: (j['cache_rate'] as num?)?.toDouble() ?? 0,
    avgDurationMs: (j['avg_duration_ms'] as num?)?.toDouble() ?? 0,
    totalCost: (j['total_cost'] as num?)?.toDouble() ?? 0,
  );

  /// 维度名。后端回溯失败会返空串，归「未知平台」由页面做（单点归一化）。
  final String name;
  final int totalRequests;
  final int successCount;
  final int inputTokens;
  final int outputTokens;
  final int cacheTokens;
  final double cacheRate;
  final double avgDurationMs;
  final double totalCost;

  /// 换掉名字（空名归一化用），其余字段照抄。
  DimensionEntry withName(String next) => DimensionEntry(
    name: next,
    totalRequests: totalRequests,
    successCount: successCount,
    inputTokens: inputTokens,
    outputTokens: outputTokens,
    cacheTokens: cacheTokens,
    cacheRate: cacheRate,
    avgDurationMs: avgDurationMs,
    totalCost: totalCost,
  );
}

/// `generated/StatsResult.ts`：`stats_query` 的返回体。
class StatsResult {
  const StatsResult({
    required this.overview,
    required this.buckets,
    required this.dimensionData,
    required this.availableModels,
    required this.series,
  });

  factory StatsResult.fromJson(Map<String, dynamic> j) => StatsResult(
    overview: StatsOverview.fromJson(
      (j['overview'] as Map<String, dynamic>?) ?? const {},
    ),
    buckets: [
      for (final b in (j['buckets'] as List?) ?? const [])
        StatsBucket.fromJson(b as Map<String, dynamic>),
    ],
    dimensionData: [
      for (final d in (j['dimension_data'] as List?) ?? const [])
        DimensionEntry.fromJson(d as Map<String, dynamic>),
    ],
    availableModels: [
      for (final m in (j['available_models'] as List?) ?? const [])
        m as String,
    ],
    series: [
      for (final s in (j['series'] as List?) ?? const [])
        StatsSeries.fromJson(s as Map<String, dynamic>),
    ],
  );

  final StatsOverview overview;
  final List<StatsBucket> buckets;
  final List<DimensionEntry> dimensionData;

  /// 当前筛选范围内实际有记录的模型名（模型筛选下拉的数据源，非配置列表）。
  final List<String> availableModels;
  final List<StatsSeries> series;
}

/// 单条配额快照：余额查询成功时的 `est_balance_remaining` 落库值。
/// `createdAt` 是**毫秒** Unix 时间戳。
class QuotaSnapshot {
  const QuotaSnapshot({
    required this.platformId,
    required this.estBalanceRemaining,
    required this.createdAt,
  });

  factory QuotaSnapshot.fromJson(Map<String, dynamic> j) => QuotaSnapshot(
    platformId: (j['platform_id'] as num).toInt(),
    estBalanceRemaining: (j['est_balance_remaining'] as num).toDouble(),
    createdAt: (j['created_at'] as num).toInt(),
  );

  final int platformId;
  final double estBalanceRemaining;
  final int createdAt;
}

/// 服务端 bin 化的 `(duration_bin × cost_bin)` 计数矩阵。
/// `counts[i][j]` = duration 落第 i 个 bin 且 est_cost 落第 j 个 bin 的请求数；
/// 边界数组长度 = bin 数 + 1。空窗口 → 三个数组全空（不报错）。
class ScatterHistogram {
  const ScatterHistogram({
    required this.durationBins,
    required this.costBins,
    required this.counts,
  });

  factory ScatterHistogram.fromJson(Map<String, dynamic> j) => ScatterHistogram(
    durationBins: [for (final v in j['duration_bins'] as List) (v as num).toDouble()],
    costBins: [for (final v in j['cost_bins'] as List) (v as num).toDouble()],
    counts: [
      for (final row in j['counts'] as List)
        [for (final v in row as List) (v as num).toInt()],
    ],
  );

  final List<double> durationBins;
  final List<double> costBins;
  final List<List<int>> counts;
}
