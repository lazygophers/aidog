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

// ═══════════════════════════════════════════════════════════════════════════
// 票 I07（平台 / 分组 / 日志）用到的载荷。字段名同样逐个抄自
// `src/services/api/types/generated/*.ts` 与 `types/manual.ts`，出处标在每个类上。
// ═══════════════════════════════════════════════════════════════════════════

/// `generated/PlatformModels.ts`：五个可选槽位。
class PlatformModels {
  const PlatformModels({
    this.defaultModel,
    this.sonnet,
    this.opus,
    this.haiku,
    this.gpt,
  });

  factory PlatformModels.fromJson(Map<String, dynamic>? j) => PlatformModels(
    defaultModel: j?['default'] as String?,
    sonnet: j?['sonnet'] as String?,
    opus: j?['opus'] as String?,
    haiku: j?['haiku'] as String?,
    gpt: j?['gpt'] as String?,
  );

  /// wire 上的键名是 `default`（Dart 保留字，故字段改名，序列化时写回 `default`）。
  final String? defaultModel;
  final String? sonnet;
  final String? opus;
  final String? haiku;
  final String? gpt;

  Map<String, Object?> toJson() => {
    'default': defaultModel,
    'sonnet': sonnet,
    'opus': opus,
    'haiku': haiku,
    'gpt': gpt,
  };
}

/// `generated/PlatformEndpoint.ts`。
class PlatformEndpoint {
  const PlatformEndpoint({
    required this.protocol,
    required this.baseUrl,
    required this.clientType,
    required this.codingPlan,
  });

  factory PlatformEndpoint.fromJson(Map<String, dynamic> j) => PlatformEndpoint(
    protocol: (j['protocol'] as String?) ?? '',
    baseUrl: (j['base_url'] as String?) ?? '',
    clientType: (j['client_type'] as String?) ?? '',
    codingPlan: (j['coding_plan'] as bool?) ?? false,
  );

  final String protocol;
  final String baseUrl;
  final String clientType;
  final bool codingPlan;

  Map<String, Object?> toJson() => {
    'protocol': protocol,
    'base_url': baseUrl,
    'client_type': clientType,
    'coding_plan': codingPlan,
  };
}

/// `generated/Platform.ts`。页面真的读/写的字段全在这里；`platform_update` /
/// `platform_create` 的入参形状见 `platforms.ts:466/492`（`{input: {...}}`）。
class PlatformRow {
  const PlatformRow({
    required this.id,
    required this.name,
    required this.platformType,
    required this.baseUrl,
    required this.apiKey,
    required this.extra,
    required this.models,
    required this.availableModels,
    required this.endpoints,
    required this.enabled,
    required this.status,
    required this.expiresAt,
    required this.estBalanceRemaining,
    required this.estCodingPlan,
    required this.rateLimit,
    required this.lastRealQueryAt,
    required this.estimateCount,
    required this.lastError,
    required this.lastErrorAt,
    required this.sortOrder,
  });

  factory PlatformRow.fromJson(Map<String, dynamic> j) => PlatformRow(
    id: (j['id'] as num?)?.toInt() ?? 0,
    name: (j['name'] as String?) ?? '',
    platformType: (j['platform_type'] as String?) ?? '',
    baseUrl: (j['base_url'] as String?) ?? '',
    apiKey: (j['api_key'] as String?) ?? '',
    extra: (j['extra'] as String?) ?? '',
    models: PlatformModels.fromJson(j['models'] as Map<String, dynamic>?),
    availableModels: [
      for (final m in (j['available_models'] as List?) ?? const []) m as String,
    ],
    endpoints: [
      for (final e in (j['endpoints'] as List?) ?? const [])
        PlatformEndpoint.fromJson(e as Map<String, dynamic>),
    ],
    enabled: (j['enabled'] as bool?) ?? false,
    status: (j['status'] as String?) ?? 'disabled',
    expiresAt: (j['expires_at'] as num?)?.toInt() ?? 0,
    estBalanceRemaining: (j['est_balance_remaining'] as num?)?.toDouble() ?? 0,
    estCodingPlan: (j['est_coding_plan'] as String?) ?? '',
    rateLimit: (j['rate_limit'] as String?) ?? '',
    lastRealQueryAt: (j['last_real_query_at'] as num?)?.toInt() ?? 0,
    estimateCount: (j['estimate_count'] as num?)?.toInt() ?? 0,
    lastError: (j['last_error'] as String?) ?? '',
    lastErrorAt: (j['last_error_at'] as num?)?.toInt() ?? 0,
    sortOrder: (j['sort_order'] as num?)?.toInt() ?? 0,
  );

  final int id;
  final String name;
  final String platformType;
  final String baseUrl;
  final String apiKey;
  final String extra;
  final PlatformModels models;
  final List<String> availableModels;
  final List<PlatformEndpoint> endpoints;
  final bool enabled;

  /// 三态：`enabled` / `disabled` / `auto_disabled`（`manual.ts::PlatformStatus`）。
  final String status;
  final int expiresAt;
  final double estBalanceRemaining;
  final String estCodingPlan;
  final String rateLimit;
  final int lastRealQueryAt;
  final int estimateCount;
  final String lastError;
  final int lastErrorAt;
  final int sortOrder;

  /// 启停用：只换 status + enabled（React `usePlatformsState.ts:519`）。
  PlatformRow withStatus(String next) =>
      copyWith(status: next, enabled: next == 'enabled');

  /// `refreshStats` 的字段级 merge（React `usePlatformsState.ts:456-465`）：
  /// 只取后台派生的七个统计字段，保留前端排序与乐观态。
  PlatformRow mergeStats(PlatformRow fresh) => copyWith(
    estBalanceRemaining: fresh.estBalanceRemaining,
    estCodingPlan: fresh.estCodingPlan,
    lastRealQueryAt: fresh.lastRealQueryAt,
    estimateCount: fresh.estimateCount,
    lastError: fresh.lastError,
    lastErrorAt: fresh.lastErrorAt,
    rateLimit: fresh.rateLimit,
  );

  /// 七个统计字段全等 → `refreshStats` 保留原对象引用（React `:446-454` 的同一判据）。
  bool statsEqual(PlatformRow other) =>
      estBalanceRemaining == other.estBalanceRemaining &&
      estCodingPlan == other.estCodingPlan &&
      lastRealQueryAt == other.lastRealQueryAt &&
      estimateCount == other.estimateCount &&
      lastError == other.lastError &&
      lastErrorAt == other.lastErrorAt &&
      rateLimit == other.rateLimit;

  PlatformRow copyWith({
    String? name,
    String? platformType,
    String? baseUrl,
    String? apiKey,
    String? extra,
    PlatformModels? models,
    List<String>? availableModels,
    List<PlatformEndpoint>? endpoints,
    bool? enabled,
    String? status,
    int? expiresAt,
    double? estBalanceRemaining,
    String? estCodingPlan,
    String? rateLimit,
    int? lastRealQueryAt,
    int? estimateCount,
    String? lastError,
    int? lastErrorAt,
    int? sortOrder,
  }) => PlatformRow(
    id: id,
    name: name ?? this.name,
    platformType: platformType ?? this.platformType,
    baseUrl: baseUrl ?? this.baseUrl,
    apiKey: apiKey ?? this.apiKey,
    extra: extra ?? this.extra,
    models: models ?? this.models,
    availableModels: availableModels ?? this.availableModels,
    endpoints: endpoints ?? this.endpoints,
    enabled: enabled ?? this.enabled,
    status: status ?? this.status,
    expiresAt: expiresAt ?? this.expiresAt,
    estBalanceRemaining: estBalanceRemaining ?? this.estBalanceRemaining,
    estCodingPlan: estCodingPlan ?? this.estCodingPlan,
    rateLimit: rateLimit ?? this.rateLimit,
    lastRealQueryAt: lastRealQueryAt ?? this.lastRealQueryAt,
    estimateCount: estimateCount ?? this.estimateCount,
    lastError: lastError ?? this.lastError,
    lastErrorAt: lastErrorAt ?? this.lastErrorAt,
    sortOrder: sortOrder ?? this.sortOrder,
  );
}

/// `generated/EnvVar.ts`。
class EnvVar {
  const EnvVar({required this.key, required this.value});

  factory EnvVar.fromJson(Map<String, dynamic> j) => EnvVar(
    key: (j['key'] as String?) ?? '',
    value: (j['value'] as String?) ?? '',
  );

  final String key;
  final String value;

  Map<String, Object?> toJson() => {'key': key, 'value': value};
}

/// `generated/ModelMapping.ts`。
class ModelMapping {
  const ModelMapping({
    required this.sourceModel,
    required this.targetPlatformId,
    required this.targetModel,
    required this.requestTimeoutSecs,
    required this.connectTimeoutSecs,
  });

  factory ModelMapping.fromJson(Map<String, dynamic> j) => ModelMapping(
    sourceModel: (j['source_model'] as String?) ?? '',
    targetPlatformId: (j['target_platform_id'] as num?)?.toInt() ?? 0,
    targetModel: (j['target_model'] as String?) ?? '',
    requestTimeoutSecs: (j['request_timeout_secs'] as num?)?.toInt() ?? 0,
    connectTimeoutSecs: (j['connect_timeout_secs'] as num?)?.toInt() ?? 0,
  );

  final String sourceModel;
  final int targetPlatformId;
  final String targetModel;
  final int requestTimeoutSecs;
  final int connectTimeoutSecs;

  Map<String, Object?> toJson() => {
    'source_model': sourceModel,
    'target_platform_id': targetPlatformId,
    'target_model': targetModel,
    'request_timeout_secs': requestTimeoutSecs,
    'connect_timeout_secs': connectTimeoutSecs,
  };
}

/// `generated/Group.ts`。
class GroupRow {
  const GroupRow({
    required this.id,
    required this.name,
    required this.groupKey,
    required this.routingMode,
    required this.autoFromPlatform,
    required this.requestTimeoutSecs,
    required this.connectTimeoutSecs,
    required this.maxRetries,
    required this.modelMappings,
    required this.envVars,
    required this.isDefault,
    required this.extra,
  });

  factory GroupRow.fromJson(Map<String, dynamic> j) => GroupRow(
    id: (j['id'] as num?)?.toInt() ?? 0,
    name: (j['name'] as String?) ?? '',
    groupKey: (j['group_key'] as String?) ?? '',
    routingMode: (j['routing_mode'] as String?) ?? 'failover',
    autoFromPlatform: (j['auto_from_platform'] as String?) ?? '',
    requestTimeoutSecs: (j['request_timeout_secs'] as num?)?.toInt() ?? 0,
    connectTimeoutSecs: (j['connect_timeout_secs'] as num?)?.toInt() ?? 0,
    maxRetries: (j['max_retries'] as num?)?.toInt() ?? 0,
    modelMappings: [
      for (final m in (j['model_mappings'] as List?) ?? const [])
        ModelMapping.fromJson(m as Map<String, dynamic>),
    ],
    envVars: [
      for (final e in (j['env_vars'] as List?) ?? const [])
        EnvVar.fromJson(e as Map<String, dynamic>),
    ],
    isDefault: (j['is_default'] as bool?) ?? false,
    extra: (j['extra'] as String?) ?? '',
  );

  final int id;
  final String name;
  final String groupKey;
  final String routingMode;
  final String autoFromPlatform;
  final int requestTimeoutSecs;
  final int connectTimeoutSecs;
  final int maxRetries;
  final List<ModelMapping> modelMappings;
  final List<EnvVar> envVars;
  final bool isDefault;
  final String extra;
}

/// `generated/GroupPlatformDetail.ts`。
class GroupPlatform {
  const GroupPlatform({
    required this.platform,
    required this.priority,
    required this.weight,
    required this.levelPriority,
  });

  factory GroupPlatform.fromJson(Map<String, dynamic> j) => GroupPlatform(
    platform: PlatformRow.fromJson(
      (j['platform'] as Map<String, dynamic>?) ?? const {},
    ),
    priority: (j['priority'] as num?)?.toInt() ?? 0,
    weight: (j['weight'] as num?)?.toInt() ?? 1,
    levelPriority: (j['level_priority'] as num?)?.toInt() ?? 5,
  );

  final PlatformRow platform;
  final int priority;
  final int weight;
  final int levelPriority;

  GroupPlatform copyWith({PlatformRow? platform, int? levelPriority}) =>
      GroupPlatform(
        platform: platform ?? this.platform,
        priority: priority,
        weight: weight,
        levelPriority: levelPriority ?? this.levelPriority,
      );
}

/// `generated/GroupDetail.ts`。
class GroupDetail {
  const GroupDetail({
    required this.group,
    required this.platforms,
    required this.modelMappings,
  });

  /// React `useGroupData.ts:130-134` 的填充语义：`model_mappings` 缺失时回落
  /// `group.model_mappings`，`platforms` 缺失时回落空列表。
  factory GroupDetail.fromJson(Map<String, dynamic> j) {
    final group = GroupRow.fromJson(
      (j['group'] as Map<String, dynamic>?) ?? const {},
    );
    final raw = j['model_mappings'] as List?;
    return GroupDetail(
      group: group,
      platforms: [
        for (final p in (j['platforms'] as List?) ?? const [])
          GroupPlatform.fromJson(p as Map<String, dynamic>),
      ],
      modelMappings: raw == null || raw.isEmpty
          ? group.modelMappings
          : [
              for (final m in raw)
                ModelMapping.fromJson(m as Map<String, dynamic>),
            ],
    );
  }

  final GroupRow group;
  final List<GroupPlatform> platforms;
  final List<ModelMapping> modelMappings;

  bool hasPlatform(int id) => platforms.any((gp) => gp.platform.id == id);

  GroupDetail copyWith({List<GroupPlatform>? platforms}) => GroupDetail(
    group: group,
    platforms: platforms ?? this.platforms,
    modelMappings: modelMappings,
  );
}

/// `generated/PlatformUsageStats.ts`。
class UsageStats {
  const UsageStats({
    required this.totalRequests,
    required this.successCount,
    required this.totalInputTokens,
    required this.totalOutputTokens,
    required this.totalCacheTokens,
    required this.cacheRate,
    required this.recentFailures,
    required this.recentTotal,
    required this.totalCost,
    required this.todayTokens,
    required this.todayCost,
  });

  factory UsageStats.fromJson(Map<String, dynamic> j) => UsageStats(
    totalRequests: (j['total_requests'] as num?)?.toInt() ?? 0,
    successCount: (j['success_count'] as num?)?.toInt() ?? 0,
    totalInputTokens: (j['total_input_tokens'] as num?)?.toInt() ?? 0,
    totalOutputTokens: (j['total_output_tokens'] as num?)?.toInt() ?? 0,
    totalCacheTokens: (j['total_cache_tokens'] as num?)?.toInt() ?? 0,
    cacheRate: (j['cache_rate'] as num?)?.toDouble() ?? 0,
    recentFailures: (j['recent_failures'] as num?)?.toInt() ?? 0,
    recentTotal: (j['recent_total'] as num?)?.toInt() ?? 0,
    totalCost: (j['total_cost'] as num?)?.toDouble() ?? 0,
    todayTokens: (j['today_tokens'] as num?)?.toInt() ?? 0,
    todayCost: (j['today_cost'] as num?)?.toDouble() ?? 0,
  );

  final int totalRequests;
  final int successCount;
  final int totalInputTokens;
  final int totalOutputTokens;
  final int totalCacheTokens;
  final double cacheRate;
  final int recentFailures;
  final int recentTotal;
  final double totalCost;
  final int todayTokens;
  final double todayCost;
}

/// `manual.ts::PlatformQuota`（`platform_query_quota*` 返回）。
class PlatformQuota {
  const PlatformQuota({
    required this.success,
    required this.error,
    required this.queriedAt,
    required this.newapiUserId,
    required this.balanceRemaining,
  });

  factory PlatformQuota.fromJson(Map<String, dynamic> j) => PlatformQuota(
    success: (j['success'] as bool?) ?? false,
    error: j['error'] as String?,
    queriedAt: (j['queried_at'] as num?)?.toInt() ?? 0,
    newapiUserId: j['newapi_user_id'] as String?,
    balanceRemaining:
        ((j['balance'] as Map<String, dynamic>?)?['remaining'] as num?)
            ?.toDouble(),
  );

  final bool success;
  final String? error;
  final int queriedAt;
  final String? newapiUserId;

  /// `manual.ts::BalanceInfo.remaining`；balance 为 null（coding plan 平台）时留空。
  final double? balanceRemaining;
}

/// `generated/LastTestResult.ts`（`get_last_test_result` 返回，无记录是 null）。
class LastTestResult {
  const LastTestResult({
    required this.success,
    required this.statusCode,
    required this.durationMs,
    required this.createdAt,
    required this.error,
  });

  factory LastTestResult.fromJson(Map<String, dynamic> j) => LastTestResult(
    success: (j['success'] as bool?) ?? false,
    statusCode: (j['status_code'] as num?)?.toInt() ?? 0,
    durationMs: (j['duration_ms'] as num?)?.toInt() ?? 0,
    createdAt: (j['created_at'] as num?)?.toInt() ?? 0,
    error: (j['error'] as String?) ?? '',
  );

  final bool success;
  final int statusCode;
  final int durationMs;
  final int createdAt;
  final String error;
}

/// `platforms.ts:583` 的 `model_test` 返回（`manual.ts::ModelTestResult`）。
class ModelTestResult {
  const ModelTestResult({
    required this.success,
    required this.durationMs,
    required this.error,
  });

  factory ModelTestResult.fromJson(Map<String, dynamic> j) => ModelTestResult(
    success: (j['success'] as bool?) ?? false,
    durationMs: (j['duration_ms'] as num?)?.toInt() ?? 0,
    error: (j['error'] as String?) ?? '',
  );

  final bool success;
  final int durationMs;
  final String error;
}

/// `platforms.ts:500` 的 `platform_purge_disabled` 返回。
/// 键是 camelCase（这两个字段是命令自己拼的对象，不是 serde 结构体）。
class PurgeReport {
  const PurgeReport({required this.deletedIds, required this.unassignedIds});

  factory PurgeReport.fromJson(Map<String, dynamic> j) => PurgeReport(
    deletedIds: [
      for (final i in (j['deletedIds'] as List?) ?? const []) (i as num).toInt(),
    ],
    unassignedIds: [
      for (final i in (j['unassignedIds'] as List?) ?? const [])
        (i as num).toInt(),
    ],
  );

  final List<int> deletedIds;
  final List<int> unassignedIds;
}

/// `platforms.ts:548` 系列批量命令的返回（`manual.ts::BatchReport`）。
class BatchReport {
  const BatchReport({required this.applied});

  factory BatchReport.fromJson(Map<String, dynamic> j) =>
      BatchReport(applied: (j['applied'] as num?)?.toInt() ?? 0);

  final int applied;
}

/// `generated/ProxyLogSummary.ts`。
class ProxyLogSummary {
  const ProxyLogSummary({
    required this.id,
    required this.groupKey,
    required this.model,
    required this.actualModel,
    required this.platformId,
    required this.statusCode,
    required this.durationMs,
    required this.inputTokens,
    required this.outputTokens,
    required this.cacheTokens,
    required this.isStream,
    required this.retryCount,
    required this.createdAt,
  });

  factory ProxyLogSummary.fromJson(Map<String, dynamic> j) => ProxyLogSummary(
    id: (j['id'] as String?) ?? '',
    groupKey: (j['group_key'] as String?) ?? '',
    model: (j['model'] as String?) ?? '',
    actualModel: (j['actual_model'] as String?) ?? '',
    platformId: (j['platform_id'] as num?)?.toInt() ?? 0,
    statusCode: (j['status_code'] as num?)?.toInt() ?? 0,
    durationMs: (j['duration_ms'] as num?)?.toInt() ?? 0,
    inputTokens: (j['input_tokens'] as num?)?.toInt() ?? 0,
    outputTokens: (j['output_tokens'] as num?)?.toInt() ?? 0,
    cacheTokens: (j['cache_tokens'] as num?)?.toInt() ?? 0,
    isStream: (j['is_stream'] as bool?) ?? false,
    retryCount: (j['retry_count'] as num?)?.toInt() ?? 0,
    createdAt: (j['created_at'] as num?)?.toInt() ?? 0,
  );

  final String id;
  final String groupKey;
  final String model;
  final String actualModel;
  final int platformId;
  final int statusCode;
  final int durationMs;
  final int inputTokens;
  final int outputTokens;
  final int cacheTokens;
  final bool isStream;
  final int retryCount;
  final int createdAt;
}

/// `manual.ts::ProxyLogDetail`（`proxy_log_get` 返回，未命中是 null）。
class ProxyLogDetail {
  const ProxyLogDetail({
    required this.id,
    required this.groupKey,
    required this.model,
    required this.actualModel,
    required this.sourceProtocol,
    required this.targetProtocol,
    required this.platformId,
    required this.requestHeaders,
    required this.requestBody,
    required this.upstreamRequestHeaders,
    required this.upstreamRequestBody,
    required this.responseBody,
    required this.requestUrl,
    required this.upstreamRequestUrl,
    required this.upstreamResponseHeaders,
    required this.upstreamStatusCode,
    required this.userResponseHeaders,
    required this.userResponseBody,
    required this.statusCode,
    required this.durationMs,
    required this.inputTokens,
    required this.outputTokens,
    required this.cacheTokens,
    required this.createdAt,
  });

  factory ProxyLogDetail.fromJson(Map<String, dynamic> j) => ProxyLogDetail(
    id: (j['id'] as String?) ?? '',
    groupKey: (j['group_key'] as String?) ?? '',
    model: (j['model'] as String?) ?? '',
    actualModel: (j['actual_model'] as String?) ?? '',
    sourceProtocol: (j['source_protocol'] as String?) ?? '',
    targetProtocol: (j['target_protocol'] as String?) ?? '',
    platformId: (j['platform_id'] as num?)?.toInt() ?? 0,
    requestHeaders: (j['request_headers'] as String?) ?? '',
    requestBody: (j['request_body'] as String?) ?? '',
    upstreamRequestHeaders: (j['upstream_request_headers'] as String?) ?? '',
    upstreamRequestBody: (j['upstream_request_body'] as String?) ?? '',
    responseBody: (j['response_body'] as String?) ?? '',
    requestUrl: (j['request_url'] as String?) ?? '',
    upstreamRequestUrl: (j['upstream_request_url'] as String?) ?? '',
    upstreamResponseHeaders: (j['upstream_response_headers'] as String?) ?? '',
    upstreamStatusCode: (j['upstream_status_code'] as num?)?.toInt() ?? 0,
    userResponseHeaders: (j['user_response_headers'] as String?) ?? '',
    userResponseBody: (j['user_response_body'] as String?) ?? '',
    statusCode: (j['status_code'] as num?)?.toInt() ?? 0,
    durationMs: (j['duration_ms'] as num?)?.toInt() ?? 0,
    inputTokens: (j['input_tokens'] as num?)?.toInt() ?? 0,
    outputTokens: (j['output_tokens'] as num?)?.toInt() ?? 0,
    cacheTokens: (j['cache_tokens'] as num?)?.toInt() ?? 0,
    createdAt: (j['created_at'] as num?)?.toInt() ?? 0,
  );

  final String id;
  final String groupKey;
  final String model;
  final String actualModel;
  final String sourceProtocol;
  final String targetProtocol;
  final int platformId;
  final String requestHeaders;
  final String requestBody;
  final String upstreamRequestHeaders;
  final String upstreamRequestBody;
  final String responseBody;
  final String requestUrl;
  final String upstreamRequestUrl;
  final String upstreamResponseHeaders;
  final int upstreamStatusCode;
  final String userResponseHeaders;
  final String userResponseBody;
  final int statusCode;
  final int durationMs;
  final int inputTokens;
  final int outputTokens;
  final int cacheTokens;
  final int createdAt;
}
