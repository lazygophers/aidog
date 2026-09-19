/// 页面用到的 RPC 载荷模型。字段名一律从 `src/services/api/types/generated/<Type>.ts`
/// （ts-rs 生成，Rust serde 的真值投影）或 `types/manual.ts` 抄，**不凭记忆写** ——
/// I01 的泛型 invoke 不校验字段类型，写错一个字母就是运行时静默 null。
library;

/// `types/manual.ts::TodayStats`（`tray_today_stats` 返回）。
class TodayStats {
  const TodayStats({
    required this.tokens,
    required this.inputTokens,
    required this.outputTokens,
    required this.cacheTokens,
    required this.cacheRate,
    required this.cost,
    required this.totalRequests,
  });

  factory TodayStats.fromJson(Map<String, dynamic> j) => TodayStats(
    tokens: (j['tokens'] as num?)?.toInt() ?? 0,
    inputTokens: (j['input_tokens'] as num?)?.toInt() ?? 0,
    outputTokens: (j['output_tokens'] as num?)?.toInt() ?? 0,
    cacheTokens: (j['cache_tokens'] as num?)?.toInt() ?? 0,
    cacheRate: (j['cache_rate'] as num?)?.toDouble() ?? 0,
    cost: (j['cost'] as num?)?.toDouble() ?? 0,
    totalRequests: (j['total_requests'] as num?)?.toInt() ?? 0,
  );

  final int tokens;
  final int inputTokens;
  final int outputTokens;
  final int cacheTokens;
  final double cacheRate;
  final double cost;
  final int totalRequests;
}

/// `types/manual.ts::TodayPlatformStat`（`popover_platform_today` 返回）。
class TodayPlatformStat {
  const TodayPlatformStat({
    required this.platformId,
    required this.platformName,
    required this.tokens,
    required this.cost,
    required this.requests,
  });

  factory TodayPlatformStat.fromJson(Map<String, dynamic> j) =>
      TodayPlatformStat(
        platformId: (j['platform_id'] as num?)?.toInt() ?? 0,
        platformName: (j['platform_name'] as String?) ?? '',
        tokens: (j['tokens'] as num?)?.toInt() ?? 0,
        cost: (j['cost'] as num?)?.toDouble() ?? 0,
        requests: (j['requests'] as num?)?.toInt() ?? 0,
      );

  final int platformId;
  final String platformName;
  final int tokens;
  final double cost;
  final int requests;
}

/// `generated/Platform.ts` 的页面子集：首页求总余额、统计页做平台筛选 / 配额卡标题。
/// 只取用得到的四个字段（整条 Platform 有 20+ 字段，页面一个也不碰）。
class PlatformSummary {
  const PlatformSummary({
    required this.id,
    required this.name,
    required this.platformType,
    required this.estBalanceRemaining,
  });

  factory PlatformSummary.fromJson(Map<String, dynamic> j) => PlatformSummary(
    id: (j['id'] as num?)?.toInt() ?? 0,
    name: (j['name'] as String?) ?? '',
    platformType: (j['platform_type'] as String?) ?? '',
    estBalanceRemaining: (j['est_balance_remaining'] as num?)?.toDouble() ?? 0,
  );

  final int id;
  final String name;

  /// 协议 code（`generated/Protocol` 的字面值），统计页按它取跨语言搜索词。
  final String platformType;
  final double estBalanceRemaining;
}

/// `generated/GroupDetail.ts` 的页面子集：统计页分组筛选只要 `group.group_key` + `group.name`。
class GroupSummary {
  const GroupSummary({required this.groupKey, required this.name});

  /// 入参是 `GroupDetail` 整条，内部取 `group` 子对象。
  factory GroupSummary.fromJson(Map<String, dynamic> j) {
    final g = (j['group'] as Map<String, dynamic>?) ?? const {};
    return GroupSummary(
      groupKey: (g['group_key'] as String?) ?? '',
      name: (g['name'] as String?) ?? '',
    );
  }

  final String groupKey;
  final String name;
}

/// `types/manual.ts::ProxySettings` 的页面子集：首页只要端口。
class ProxySettingsSummary {
  const ProxySettingsSummary({required this.port});

  factory ProxySettingsSummary.fromJson(Map<String, dynamic> j) =>
      ProxySettingsSummary(port: (j['port'] as num?)?.toInt() ?? 0);

  final int port;
}
