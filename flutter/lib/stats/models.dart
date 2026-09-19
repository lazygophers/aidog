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
