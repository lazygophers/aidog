/// 平台卡片的**展示层零件**（票 I18b），对应 React 的
/// `src/components/platforms/PlatformCard.tsx` + 它依赖的四个派生模块：
/// `domains/platforms/health.ts`、`domains/platforms/autoCategorize.ts`、
/// `components/shared/usageColor.ts`、`utils/timeWindow.ts`。
///
/// 放独立文件而不是塞进 `platforms.dart`：这些是**纯函数 + 无状态 widget**，
/// 单测可以直接喂数据断言，不必挂整页。页面那边只负责把数据接进来。
///
/// 色值一律 `AidogTheme.of(context).c.*`，本文件零硬编码颜色。
library;

import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../utils/color_level.dart';
import '../utils/contrast.dart';
import '../../utils/formatters.dart';
import '../../utils/hex_color.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart' show counterStyle;
import 'models.dart';
import 'platform_paste_logic.dart';
import 'time_window.dart';

// 卡片侧原先自带一份 TimeWindow / isCurrentlyPeak，与表单侧重复（见下方「高峰时段」处的
// 说明）。删掉之后，原来 `import 'platform_card_bits.dart'` 的调用方仍需要这两个名字，
// 在这里转发，避免每个调用点各加一行 import。
export 'time_window.dart'
    show
        TimeWindow,
        isCurrentlyPeak,
        modelMatch,
        normalizeWindow,
        timeWindowFromJsonNormalized,
        wallTimeInTz;

// ══ 健康态（health.ts）══════════════════════════════════════════════

/// `constants.ts::HealthStatus`。
enum HealthStatus { healthy, warning, error, unknown }

/// `health.ts:10::healthStatus`：「成功即绿」——最近 N 次里有一次成功就算健康，
/// 全失败才红，无请求灰。**不返回 warning 中间态**。
HealthStatus healthStatusOf(int recentTotal, int recentFailures) {
  if (recentTotal == 0) return HealthStatus.unknown;
  if (recentFailures >= recentTotal) return HealthStatus.error;
  return HealthStatus.healthy;
}

/// `health.ts:19::keyInvalidFromStatus`：auto_disabled 且 last_error 是 401/403。
bool keyInvalidFromStatus(String status, String? lastError) =>
    status == 'auto_disabled' &&
    ((lastError?.startsWith('HTTP 401') ?? false) ||
        (lastError?.startsWith('HTTP 403') ?? false));

/// `health.ts:37::deriveHealth`：红=key 失效 / 黄=有 last_error 但可恢复 /
/// 绿=enabled 且无错；其余回落 manual → 成功率 → unknown。
HealthStatus deriveHealth({
  required String status,
  String? lastError,
  String? manual,
  int? recentTotal,
  int? recentFailures,
}) {
  if (keyInvalidFromStatus(status, lastError)) return HealthStatus.error;
  if (lastError != null && lastError.isNotEmpty) return HealthStatus.warning;
  if (status == 'enabled') return HealthStatus.healthy;
  if (manual != null) {
    return manual == 'ok' ? HealthStatus.healthy : HealthStatus.error;
  }
  if (recentTotal != null && recentFailures != null) {
    return healthStatusOf(recentTotal, recentFailures);
  }
  return HealthStatus.unknown;
}

/// `constants.ts::HEALTH_COLORS` 的主题投影。
Color healthColor(HealthStatus h, AidogColors c) => switch (h) {
  HealthStatus.healthy => c.ok,
  HealthStatus.warning => c.peak,
  HealthStatus.error => c.bad,
  HealthStatus.unknown => c.fg3,
};

/// `health.ts:56::allModelValues`：五个槽位取非空值并去重，保持槽位顺序。
List<String> allModelValues(PlatformModels m) {
  final seen = <String>{};
  final out = <String>[];
  for (final v in [m.defaultModel, m.sonnet, m.opus, m.haiku, m.gpt]) {
    if (v != null && v.isNotEmpty && seen.add(v)) out.add(v);
  }
  return out;
}

// ══ est_coding_plan / rate_limit（autoCategorize.ts）══════════════════

/// `autoCategorize.ts:4::EstCodingTier`。
class EstCodingTier {
  const EstCodingTier({
    required this.name,
    required this.estUtilization,
    this.limit,
    this.windowStart,
  });

  final String name;
  final double estUtilization;
  final int? limit;
  final int? windowStart;
}

/// `autoCategorize.ts:21::parseEstCodingPlan`：非法 / 空串 / 缺 tiers 数组 → null。
List<EstCodingTier>? parseEstCodingPlan(String raw) {
  if (raw.trim().isEmpty) return null;
  final Object? doc;
  try {
    doc = jsonDecode(raw);
  } catch (_) {
    return null;
  }
  final tiers = (doc as Map?)?['tiers'];
  if (tiers is! List) return null;
  return [
    for (final e in tiers)
      if (e is Map)
        EstCodingTier(
          name: (e['name'] as String?) ?? '',
          estUtilization: (e['est_utilization'] as num?)?.toDouble() ?? 0,
          limit: (e['limit'] as num?)?.toInt(),
          windowStart: (e['window_start'] as num?)?.toInt(),
        ),
  ];
}

/// `autoCategorize.ts:34::RateLimitSnapshot`。
class RateLimitSnapshot {
  const RateLimitSnapshot({
    required this.vendor,
    required this.observedAt,
    this.requestsRemaining,
    this.requestsLimit,
    this.tokensRemaining,
    this.tokensLimit,
    this.resetsAt,
  });

  final String vendor;
  final int observedAt;
  final int? requestsRemaining;
  final int? requestsLimit;
  final int? tokensRemaining;
  final int? tokensLimit;
  final int? resetsAt;
}

/// `autoCategorize.ts:48::parseRateLimit`：非法 / 空串 / vendor 非字符串 → null。
RateLimitSnapshot? parseRateLimit(String raw) {
  if (raw.trim().isEmpty) return null;
  final Object? doc;
  try {
    doc = jsonDecode(raw);
  } catch (_) {
    return null;
  }
  if (doc is! Map) return null;
  final m = doc;
  final vendor = m['vendor'];
  if (vendor is! String) return null;
  int? n(String k) => (m[k] as num?)?.toInt();
  return RateLimitSnapshot(
    vendor: vendor,
    observedAt: n('observed_at') ?? 0,
    requestsRemaining: n('requests_remaining'),
    requestsLimit: n('requests_limit'),
    tokensRemaining: n('tokens_remaining'),
    tokensLimit: n('tokens_limit'),
    resetsAt: n('resets_at'),
  );
}

/// `autoCategorize.ts:60::rateLimitRatio`：取「请求数」「token」两维里**最紧**的那个；
/// 厂商没给 limit → null（不拿 0 冒充）。
double? rateLimitRatio(RateLimitSnapshot rl) {
  final ratios = <double>[];
  void add(int? rem, int? lim) {
    if (rem != null && lim != null && lim > 0) ratios.add(rem / lim);
  }

  add(rl.requestsRemaining, rl.requestsLimit);
  add(rl.tokensRemaining, rl.tokensLimit);
  if (ratios.isEmpty) return null;
  return ratios.reduce(math.min);
}

// ══ coding tier 配色（usageColor.ts）═════════════════════════════════

const int _hourMs = 3600 * 1000;
const int _dayMs = 24 * _hourMs;

/// `usageColor.ts:29::usageLevelToColor`：后端 balance_level 串 → 分级。
ColorLevel usageLevelToColor(String? level) => switch (level) {
  'red' => ColorLevel.danger,
  'yellow' => ColorLevel.warning,
  'green' => ColorLevel.success,
  _ => ColorLevel.neutral,
};

/// `usageColor.ts:43::cycleMsForTier`：未知 name → null（无周期概念 → 中性）。
int? cycleMsForTier(String name) => switch (name) {
  'five_hour' => 5 * _hourMs,
  'weekly_limit' || 'seven_day' => 7 * _dayMs,
  'mcp_monthly' => 30 * _dayMs,
  _ => null,
};

/// `usageColor.ts:58::codingPaceDelta`：额度已用% − 时间已过%。
double codingPaceDelta(double utilization, int remainMs, int cycleMs) {
  final usedPct = clamp(utilization, 0, 100);
  final elapsedPct = clamp(((cycleMs - remainMs) / cycleMs) * 100, 0, 100);
  return usedPct - elapsedPct;
}

/// `usageColor.ts:65::colorFromPaceDelta`：`> 3` 红 / `-3..3` 黄 / `< -3` 绿。
ColorLevel colorFromPaceDelta(double deltaPp) {
  if (!deltaPp.isFinite) return ColorLevel.neutral;
  if (deltaPp > 3) return ColorLevel.danger;
  if (deltaPp >= -3) return ColorLevel.warning;
  return ColorLevel.success;
}

/// `usageColor.ts:79::codingTierLevel`：缺 remain / cycle / 非法 util → neutral；
/// util ≥ 100 → danger（耗尽后差额算法无意义）。
ColorLevel codingTierLevel(double utilization, int? remainMs, int? cycleMs) {
  if (!utilization.isFinite || utilization < 0) return ColorLevel.neutral;
  if (remainMs == null || cycleMs == null || cycleMs <= 0) {
    return ColorLevel.neutral;
  }
  if (utilization >= 100) return ColorLevel.danger;
  return colorFromPaceDelta(codingPaceDelta(utilization, remainMs, cycleMs));
}

// ══ 配额展示（health.ts::computeQuotaDisplay）════════════════════════

/// `health.ts:78` 的 tier 展示行。
class QuotaTierDisplay {
  const QuotaTierDisplay({
    required this.name,
    required this.remainPct,
    required this.utilization,
    required this.resetsAt,
    required this.limit,
    required this.remaining,
    required this.level,
  });

  final String name;
  final double remainPct;
  final double utilization;

  /// ISO 8601 串（真查侧原样、预估侧由 remainMs 现算）；null = 无周期信息。
  final String? resetsAt;
  final int? limit;
  final int? remaining;
  final ColorLevel level;
}

/// `health.ts:71::QuotaDisplay`。
class QuotaDisplay {
  const QuotaDisplay({
    required this.estimated,
    required this.balanceRemaining,
    required this.balanceTotal,
    required this.currency,
    required this.tiers,
    required this.hasData,
  });

  final bool estimated;
  final double? balanceRemaining;
  final double? balanceTotal;
  final String currency;
  final List<QuotaTierDisplay> tiers;
  final bool hasData;
}

double _tierRemain(double utilization) => clamp(100 - utilization, 0, 100);

/// `health.ts:83::computeQuotaDisplay`：预估(est_*) 与真查(quota) 的合并，
/// 优先级逐行照抄 —— 手动刷新校准过（[preferRealCalibrated]）优先真值，
/// 否则有预估用预估，冷启动回退真查。
QuotaDisplay computeQuotaDisplay(
  PlatformRow p,
  PlatformQuota? q,
  bool preferRealCalibrated, {
  int? nowMs,
}) {
  final now = nowMs ?? DateTime.now().millisecondsSinceEpoch;
  final preferReal = preferRealCalibrated && q != null;
  final estCoding = parseEstCodingPlan(p.estCodingPlan);
  final hasEstBalance = p.estBalanceRemaining > 0;
  final hasEst = hasEstBalance || (estCoding != null && estCoding.isNotEmpty);

  if (hasEst && !preferReal) {
    final tiers = <QuotaTierDisplay>[
      for (final tier in estCoding ?? const <EstCodingTier>[])
        () {
          final limit = tier.limit;
          final remainPct = _tierRemain(tier.estUtilization);
          final remaining = limit != null
              ? (limit * remainPct / 100).round()
              : null;
          final cycleMs = cycleMsForTier(tier.name);
          final ws = tier.windowStart;
          final remainMs = (ws != null && ws > 0 && cycleMs != null)
              ? ws + cycleMs - now
              : null;
          return QuotaTierDisplay(
            name: tier.name,
            remainPct: remainPct,
            utilization: tier.estUtilization,
            resetsAt: remainMs != null
                ? DateTime.fromMillisecondsSinceEpoch(now + remainMs)
                      .toUtc()
                      .toIso8601String()
                : null,
            limit: limit,
            remaining: remaining,
            level: codingTierLevel(tier.estUtilization, remainMs, cycleMs),
          );
        }(),
    ];
    return QuotaDisplay(
      estimated: true,
      balanceRemaining: hasEstBalance ? p.estBalanceRemaining : null,
      balanceTotal: null,
      currency: (q?.balanceCurrency?.isNotEmpty ?? false)
          ? q!.balanceCurrency!
          : 'USD',
      tiers: tiers,
      hasData: hasEstBalance || tiers.isNotEmpty,
    );
  }
  if (q != null) {
    final tiers = <QuotaTierDisplay>[
      for (final tier in q.codingPlanTiers ?? const <QuotaTier>[])
        () {
          final cycleMs = cycleMsForTier(tier.name);
          final resetsMs = tier.resetsAt == null
              ? null
              : DateTime.tryParse(tier.resetsAt!)?.millisecondsSinceEpoch;
          final remainMs = (resetsMs != null && cycleMs != null)
              ? resetsMs - now
              : null;
          return QuotaTierDisplay(
            name: tier.name,
            remainPct: _tierRemain(tier.utilization),
            utilization: tier.utilization,
            resetsAt: tier.resetsAt,
            limit: tier.limit,
            remaining: tier.remaining,
            level: codingTierLevel(tier.utilization, remainMs, cycleMs),
          );
        }(),
    ];
    final isAcu = q.balanceCurrency == 'ACU';
    return QuotaDisplay(
      estimated: false,
      // ACU（Devin）无余额端点，remaining 恒 0 → 改用 used 作展示值，total=null 抑制进度条。
      balanceRemaining: q.hasBalance
          ? (isAcu ? (q.balanceUsed ?? 0) : q.balanceRemaining)
          : null,
      balanceTotal: isAcu ? null : q.balanceTotal,
      currency: (q.balanceCurrency?.isNotEmpty ?? false)
          ? q.balanceCurrency!
          : 'USD',
      tiers: tiers,
      hasData: q.hasBalance || tiers.isNotEmpty,
    );
  }
  return const QuotaDisplay(
    estimated: false,
    balanceRemaining: null,
    balanceTotal: null,
    currency: 'USD',
    tiers: [],
    hasData: false,
  );
}

/// `health.ts:142::tierLabel`。
String tierLabel(String name) => switch (name) {
  'five_hour' => '5h',
  'weekly_limit' => 'week',
  'mcp_monthly' => 'MCP',
  _ => name,
};

/// `health.ts:150::formatResetCountdown`：ISO 串 → `3d 4h` / `4h 5m` / `5m`；
/// null / 非法 / 已过期 → 空串。
String formatResetCountdown(String? resetsAt, {int? nowMs}) {
  if (resetsAt == null || resetsAt.isEmpty) return '';
  final ts = DateTime.tryParse(resetsAt)?.millisecondsSinceEpoch;
  if (ts == null) return '';
  final now = nowMs ?? DateTime.now().millisecondsSinceEpoch;
  final diffMs = ts - now;
  if (diffMs <= 0) return '';
  final diffMin = (diffMs / 60000).ceil();
  final diffHours = diffMin ~/ 60;
  final diffDays = diffHours ~/ 24;
  if (diffDays > 0) return '${diffDays}d ${diffHours % 24}h';
  if (diffHours > 0) return '${diffHours}h ${diffMin % 60}m';
  return '${diffMin}m';
}

/// `health.ts:166::formatResetClock`：当天 `HH:mm`，跨天 `M/D HH:mm`；
/// null / 非法 / 已过期 → 空串。
String formatResetClock(String? resetsAt, {int? nowMs}) {
  if (resetsAt == null || resetsAt.isEmpty) return '';
  final ts = DateTime.tryParse(resetsAt)?.millisecondsSinceEpoch;
  if (ts == null) return '';
  final nowTs = nowMs ?? DateTime.now().millisecondsSinceEpoch;
  if (ts - nowTs <= 0) return '';
  final d = DateTime.fromMillisecondsSinceEpoch(ts);
  final now = DateTime.fromMillisecondsSinceEpoch(nowTs);
  final clock = '${pad(d.hour)}:${pad(d.minute)}';
  final sameDay =
      d.year == now.year && d.month == now.month && d.day == now.day;
  return sameDay ? clock : '${d.month}/${d.day} $clock';
}

// ══ 手动预算（health.ts::computeManualBudgetDisplay）══════════════════

/// `health.ts:191::ManualBudgetDisplay`。
class ManualBudgetDisplay {
  const ManualBudgetDisplay({
    required this.remaining,
    required this.amount,
    required this.unit,
    required this.kind,
    required this.ratio,
    required this.depleted,
  });

  final double remaining;
  final double amount;
  final String unit;
  final String kind;

  /// 剩余占比 0–1，越低越紧。
  final double ratio;
  final bool depleted;
}

/// `health.ts:204::computeManualBudgetDisplay`：只看 enabled 且 amount>0 的，
/// 取**剩余比例最低**那条；一条都没有 → null。
ManualBudgetDisplay? computeManualBudgetDisplay(List<ManualBudget>? budgets) {
  final enabled = [
    for (final b in budgets ?? const <ManualBudget>[])
      if (b.enabled && b.amount > 0) b,
  ];
  if (enabled.isEmpty) return null;
  ManualBudget? tightest;
  var minRatio = double.infinity;
  for (final b in enabled) {
    final ratio = (b.amount - b.consumed) / b.amount;
    if (ratio < minRatio) {
      minRatio = ratio;
      tightest = b;
    }
  }
  if (tightest == null) return null;
  final rem = tightest.amount - tightest.consumed;
  return ManualBudgetDisplay(
    remaining: rem,
    amount: tightest.amount,
    unit: tightest.unit,
    kind: tightest.kind,
    ratio: clamp(minRatio.isFinite ? minRatio : 0, 0, 1),
    depleted: rem <= 0,
  );
}

// 高峰时段那一整套（TimeWindow / _splitFraction / wallTimeInTz / modelMatch /
// isCurrentlyPeak）的唯一定义在 `time_window.dart`，本文件直接 import。
// 这里原本有一份逐字相同的拷贝 —— 两条并行开发的分支各自从 `utils/timeWindow.ts`
// 翻了一遍，合并时撞名。留下的那份多一个 `unknown` 字段（保留未识别的键，
// 保存时不把用户手写的配置吃掉），写入侧要用它。

/// `extra` 字符串的进程级解析缓存（LRU，按内容寻址）：每张卡的 build 里
/// peak / disable_during_peak / plan_price / quota_custom_script 各 parse 一遍，
/// 每次重建全部重来；全走这里合并成每个字符串一次。
const int _kExtraCacheCapacity = 32;
final Map<String, Map<String, dynamic>?> _extraCache =
    <String, Map<String, dynamic>?>{};

Map<String, dynamic>? _extraMap(String extra) {
  if (extra.trim().isEmpty) return null;
  if (_extraCache.containsKey(extra)) {
    final hit = _extraCache.remove(extra)!;
    _extraCache[extra] = hit;
    return hit;
  }
  Map<String, dynamic>? parsed;
  try {
    final v = jsonDecode(extra);
    if (v is Map) parsed = v.cast<String, dynamic>();
  } catch (_) {
    /* ignore */
  }
  _extraCache[extra] = parsed;
  if (_extraCache.length > _kExtraCacheCapacity) {
    _extraCache.remove(_extraCache.keys.first);
  }
  return parsed;
}

/// `platforms.ts:348::parsePlatformPeak`：`extra.peak` 数组；缺失 / 非法 → 空。
List<TimeWindow> parsePlatformPeak(String extra) {
  final parsed = _extraMap(extra);
  if (parsed != null && parsed['peak'] is List) {
    return [
      for (final e in parsed['peak'] as List)
        if (e is Map) TimeWindow.fromJson(e.cast<String, dynamic>()),
    ];
  }
  return const [];
}

/// `platforms.ts:381::parseDisableDuringPeak`：**严格布尔**（数字 / 字符串不误判）。
bool parseDisableDuringPeak(String extra) {
  return _extraMap(extra)?['disable_during_peak'] == true;
}

/// `PlatformCard.tsx:134`：手填套餐月价（`extra.plan_price`，¥/月）；非数字 → null。
double? parsePlanPrice(String extra) {
  final v = _extraMap(extra)?['plan_price'];
  return v is num ? v.toDouble() : null;
}

/// `platforms.ts:160::hasCustomQuotaScript`：`extra.quota_custom_script` 非空。
bool hasCustomQuotaScript(String extra) {
  final v = _extraMap(extra)?['quota_custom_script'];
  return v is String && v.trim().isNotEmpty;
}

// ══ 协议元数据（useProtocolMeta.ts + defaults.ts）════════════════════

/// 从 `get_defaults_json` 一次性抠出平台卡需要的全部协议派生值。
///
/// React 那边是 `useProtocolMeta` 聚合 7 个 async getter（都走同一个 docPromise
/// 单例缓存）。这里不需要缓存机制：整份文档在控制器里解析一次，存成这张表。
class ProtocolMetaTable {
  const ProtocolMetaTable({
    this.labels = const {},
    this.homepages = const {},
    this.docsUrls = const {},
    this.pricingUrls = const {},
    this.codingPlanProtocols = const {},
    this.pasteFallbackProtocols = const {},
    this.quotaScriptProtocols = const {},
    this.defaultModels = const {},
    this.peakModels = const {},
    this.presetPeak = const {},
    this.keywords = const {},
    this.hosts = const {},
    this.keyPrefixes = const {},
    this.colors = const {},
    this.loaded = false,
  });

  /// `get_defaults_json` 的整份文档 → 本表。[locale] 决定 label 取哪个 name。
  ///
  /// 文档 219KB，platforms 与 importexport 两页各 parse 一遍（每遍含一次完整
  /// jsonDecode）——按原文缓存，同一份文档只 parse 一次（性能审计 #7②）。
  static String? _cachedRaw;
  static String? _cachedLocale;
  static ProtocolMetaTable? _cached;

  factory ProtocolMetaTable.parse(String rawJson, String locale) {
    if (rawJson.isEmpty) return const ProtocolMetaTable();
    if (_cached != null && rawJson == _cachedRaw && locale == _cachedLocale) {
      return _cached!;
    }
    final table = _parseUncached(rawJson, locale);
    _cachedRaw = rawJson;
    _cachedLocale = locale;
    _cached = table;
    return table;
  }

  static ProtocolMetaTable _parseUncached(String rawJson, String locale) {
    final Object? doc;
    try {
      doc = jsonDecode(rawJson);
    } catch (_) {
      return const ProtocolMetaTable();
    }
    final protocols = (doc as Map?)?['protocols'];
    if (protocols is! Map) return const ProtocolMetaTable();
    final labels = <String, String>{};
    final homepages = <String, String>{};
    final docsUrls = <String, String>{};
    final pricingUrls = <String, String>{};
    final cp = <String>{};
    final pf = <String>{};
    final qs = <String>{};
    final defModels = <String, List<String>>{};
    final peakModels = <String, List<String>>{};
    final presetPeak = <String, List<TimeWindow>>{};
    final keywords = <String, List<String>>{};
    final hosts = <String, List<String>>{};
    final keyPrefixes = <String, List<String>>{};
    final colors = <String, Color>{};
    protocols.forEach((key, entry) {
      final code = '$key';
      if (entry is! Map) return;
      // 智能识别的三样数据源（`defaults.ts:478::buildProtocolsFromPresets`）：
      // keywords / key_prefixes 直取，hosts 由 default 端点派生。
      final kw = entry['keywords'];
      if (kw is List) {
        final v = [
          for (final e in kw)
            if (e is String && e.isNotEmpty) e,
        ];
        if (v.isNotEmpty) keywords[code] = v;
      }
      final kp = entry['key_prefixes'];
      if (kp is List) {
        final v = [
          for (final e in kp)
            if (e is String && e.isNotEmpty) e,
        ];
        if (v.isNotEmpty) keyPrefixes[code] = v;
      }
      final eps = entry['endpoints'];
      if (eps is Map && eps['default'] is List) {
        final v = deriveProtocolHosts([
          for (final e in eps['default'] as List)
            if (e is Map && e['base_url'] is String) e['base_url'] as String,
        ]);
        if (v.isNotEmpty) hosts[code] = v;
      }
      final name = entry['name'];
      if (name is Map) {
        final v = name[locale] ?? name['en-US'];
        if (v is String && v.isNotEmpty) labels[code] = v;
      }
      final col = entry['color'];
      if (col is String) {
        final c = parseHexColor(col);
        if (c != null) colors[code] = c;
      }
      final hp = entry['homepage'];
      if (hp is String && hp.isNotEmpty) homepages[code] = hp;
      final su = entry['source_urls'];
      if (su is Map) {
        final d = su['docs'];
        if (d is String && d.isNotEmpty) docsUrls[code] = d;
        final pr = su['pricing'];
        if (pr is String && pr.isNotEmpty) pricingUrls[code] = pr;
      }
      if (entry['is_coding_plan'] == true) cp.add(code);
      if (entry['paste_fallback'] == true) pf.add(code);
      final scripts = entry['quota_scripts'];
      if (scripts is List && scripts.isNotEmpty) qs.add(code);
      final models = entry['models'];
      if (models is Map) {
        List<String> slots(Object? branch) => branch is Map
            ? allModelValues(
                PlatformModels.fromJson(branch.cast<String, dynamic>()),
              )
            : const [];
        defModels[code] = slots(models['default']);
        final pk = slots(models['peak']);
        if (pk.isNotEmpty) peakModels[code] = pk;
      }
      final peak = entry['peak'];
      if (peak is List && peak.isNotEmpty) {
        presetPeak[code] = [
          for (final e in peak)
            if (e is Map) TimeWindow.fromJson(e.cast<String, dynamic>()),
        ];
      }
    });
    return ProtocolMetaTable(
      labels: labels,
      homepages: homepages,
      docsUrls: docsUrls,
      pricingUrls: pricingUrls,
      codingPlanProtocols: cp,
      pasteFallbackProtocols: pf,
      quotaScriptProtocols: qs,
      defaultModels: defModels,
      peakModels: peakModels,
      presetPeak: presetPeak,
      keywords: keywords,
      hosts: hosts,
      keyPrefixes: keyPrefixes,
      colors: colors,
      loaded: true,
    );
  }

  final Map<String, String> labels;
  final Map<String, String> homepages;
  final Map<String, String> docsUrls;
  final Map<String, String> pricingUrls;
  final Set<String> codingPlanProtocols;

  /// registry `platform.json` 的 `paste_fallback`（智能识别优先级 2 兜底，如 newapi）。
  final Set<String> pasteFallbackProtocols;
  final Set<String> quotaScriptProtocols;
  final Map<String, List<String>> defaultModels;
  final Map<String, List<String>> peakModels;
  final Map<String, List<TimeWindow>> presetPeak;

  /// registry `platform.json` 的 `keywords`（智能识别的 keyword 扫描用）。
  final Map<String, List<String>> keywords;

  /// 由 default 端点派生的 host（或 host+path）子串（智能识别的优先级 1）。
  final Map<String, List<String>> hosts;

  /// registry `platform.json` 的 `key_prefixes`（智能识别的优先级 2）。
  final Map<String, List<String>> keyPrefixes;

  /// 协议品牌色（registry `platform.json` 的 `color`）。
  /// 缺省 = 该协议没登记颜色，调用方回落主题的 accent（React 同口径，
  /// `useProtocolMeta.ts:94` 的 `?? var(--accent)`）。
  final Map<String, Color> colors;

  /// 文档是否已到手。false = registry 未就绪，调用方按旧启发式回落。
  final bool loaded;

  /// 喂给智能识别解析器的 preset 视图（`defaults.ts:478::buildProtocolsFromPresets`）。
  ///
  /// 平台匹配词全部来自 registry，代码里不写任何平台名 —— 这是项目既定规矩。
  List<PastePresetRef> get pastePresets => [
    for (final code in labels.keys)
      PastePresetRef(
        value: code,
        label: labels[code] ?? code,
        keywords: keywords[code] ?? const [],
        hosts: hosts[code] ?? const [],
        keyPrefixes: keyPrefixes[code] ?? const [],
        codingPlan: codingPlanProtocols.contains(code),
        pasteFallback: pasteFallbackProtocols.contains(code),
      ),
  ];

  String label(String protocol) => labels[protocol] ?? protocol;

  bool isCodingPlan(String protocol) => codingPlanProtocols.contains(protocol);

  /// `defaults.ts:224::platformHasQuotaScript`：索引未就绪回落旧启发式
  /// （mock / claude_code 排除 —— 两协议本就无脚本，回落等价）。
  bool hasQuotaScript(String protocol, String extra) {
    if (!loaded) return protocol != 'mock' && protocol != 'claude_code';
    return quotaScriptProtocols.contains(protocol) ||
        hasCustomQuotaScript(extra);
  }

  /// `useProtocolMeta.ts:80-86`：用户 `extra.peak` 优先 → preset default；
  /// 命中就取 `models.peak` 分支，否则 `models.default`。
  List<String> modelsFor(String protocol, String extra, int nowMs) {
    final userPeak = parsePlatformPeak(extra);
    final windows = userPeak.isNotEmpty
        ? userPeak
        : (presetPeak[protocol] ?? const <TimeWindow>[]);
    final isPeak = isCurrentlyPeak(windows, nowMs);
    if (isPeak) {
      final pk = peakModels[protocol];
      if (pk != null && pk.isNotEmpty) return pk;
    }
    return defaultModels[protocol] ?? const [];
  }
}

// ══ 分享（ShareModal.tsx）══════════════════════════════════════════

/// `ShareModal.tsx:25::ShareFormat`。
enum ShareFormat { url, yaml, json, base64 }

/// `ShareModal.tsx:50::toBase64Utf8`：UTF-8 安全的 base64（中文 / emoji 不炸）。
String toBase64Utf8(String s) => base64Encode(utf8.encode(s));

/// 最小 YAML 生成器。
///
/// Dart 侧没有等价的 `yaml` 序列化实现（`package:yaml` 只解析不生成），而分享对象的形状
/// 是固定的（`SharePlatform`：字符串 / 数字 / 布尔 / 列表 / 嵌套 map），值得就地写一个，
/// 不值得为它引一个新依赖。标量一律按 YAML 规则决定要不要加引号。
String yamlStringify(Object? value) => _yamlNode(value, 0);

String _yamlNode(Object? v, int indent) {
  final pad = '  ' * indent;
  if (v is Map) {
    if (v.isEmpty) return '{}\n';
    final b = StringBuffer();
    v.forEach((k, val) {
      final key = _yamlScalar('$k');
      if (val is Map && val.isNotEmpty) {
        b.write('$pad$key:\n${_yamlNode(val, indent + 1)}');
      } else if (val is List && val.isNotEmpty) {
        b.write('$pad$key:\n${_yamlNode(val, indent + 1)}');
      } else {
        b.write('$pad$key: ${_yamlInline(val)}\n');
      }
    });
    return b.toString();
  }
  if (v is List) {
    if (v.isEmpty) return '$pad[]\n';
    final b = StringBuffer();
    for (final item in v) {
      if (item is Map && item.isNotEmpty) {
        // `- ` 起头后把第一行提上来，其余保持缩进。
        final body = _yamlNode(item, indent + 1);
        final lines = body.split('\n')..removeLast();
        b.write('$pad- ${lines.first.trimLeft()}\n');
        for (final l in lines.skip(1)) {
          b.write('$l\n');
        }
      } else {
        b.write('$pad- ${_yamlInline(item)}\n');
      }
    }
    return b.toString();
  }
  return '$pad${_yamlInline(v)}\n';
}

String _yamlInline(Object? v) {
  if (v == null) return 'null';
  if (v is num || v is bool) return '$v';
  if (v is Map) return '{}';
  if (v is List) return '[]';
  return _yamlScalar('$v');
}

/// 需要引号就加。判据按 YAML 的实际歧义点收窄，不是「见到特殊字符就引」：
/// 空串 / 前后有空白 / 以指示符开头 / 含 `: ` 或 ` #`（真正会改变解析的两处）/
/// 含换行 / 是 bool-null 字面量 / 看起来是数字。
/// `https://a/v1` 这种冒号后不跟空格的**不**加引号 —— 与 `yaml` 包的输出一致。
String _yamlScalar(String s) {
  const indicators = '-?:,[]{}#&*!|>\'"%@`';
  final needsQuote =
      s.isEmpty ||
      s != s.trim() ||
      indicators.contains(s[0]) ||
      s.contains(': ') ||
      s.contains(' #') ||
      s.contains('\n') ||
      const [
        'true',
        'false',
        'null',
        'yes',
        'no',
        '~',
      ].contains(s.toLowerCase()) ||
      double.tryParse(s) != null;
  if (!needsQuote) return s;
  return '"${s.replaceAll(r'\', r'\\').replaceAll('"', r'\"').replaceAll('\n', r'\n')}"';
}

/// `ShareModal.tsx:59::formatShare`：
/// url → `${urlScheme}?data=<base64(JSON)>`；base64 包的是 **YAML**（解析端 serde_yml 兼容）。
String formatShare(
  Map<String, Object?> share,
  ShareFormat fmt, {
  String? urlScheme,
}) {
  const encoder = JsonEncoder.withIndent('  ');
  if (fmt == ShareFormat.url) {
    return '$urlScheme?data=${toBase64Utf8(encoder.convert(share))}';
  }
  final yaml = yamlStringify(share);
  if (fmt == ShareFormat.yaml) return yaml;
  if (fmt == ShareFormat.json) return encoder.convert(share);
  return toBase64Utf8(yaml);
}

// ══ 展示零件 ═══════════════════════════════════════════════════════

/// `PlatformCard.tsx:867::relativeTime`：相对时间**简写**（`3m` / `5h` / `2d`），
/// 不足 1 分钟返空串。刻意不复用 `formatRelativeTime`（那个返完整文案）。
String relativeTimeShort(int createdMs, {int? nowMs}) {
  final now = nowMs ?? DateTime.now().millisecondsSinceEpoch;
  final diff = math.max(0, now - createdMs);
  final sec = diff ~/ 1000;
  if (sec < 60) return '';
  final min = sec ~/ 60;
  if (min < 60) return '${min}m';
  final hr = min ~/ 60;
  if (hr < 24) return '${hr}h';
  return '${hr ~/ 24}d';
}

/// 余额进度条（`components/shared/BalanceBar.tsx`）。
/// `remaining` 为 null / NaN → 整块不渲染；无 total → 只有数字没有条。
class BalanceBar extends StatelessWidget {
  const BalanceBar({
    super.key,
    required this.remaining,
    this.total,
    this.currency = '\$',
    this.level,
    this.showTotal = true,
    this.label,
  });

  final double? remaining;
  final double? total;
  final String currency;
  final ColorLevel? level;
  final bool showTotal;
  final String? label;

  @override
  Widget build(BuildContext context) {
    final rem = remaining;
    if (rem == null || rem.isNaN) return const SizedBox.shrink();
    final theme = AidogTheme.of(context);
    final hasTotal = total != null && total! > 0;
    final pct = hasTotal ? clamp(rem / total! * 100, 0, 100) : null;
    final barLevel =
        level ?? (pct != null ? remainingLevel(pct) : ColorLevel.neutral);
    final color = levelColor(barLevel, theme.c);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        // 卡片可以很窄，而金额串长度不可控（大额 + 长币种符号）。主数字保持完整，
        // 「/ 总额」那截可收缩并省略——否则窄卡上会直接撑出黄黑条纹的溢出警告。
        Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Text(
              '$currency${formatCost(rem)}',
              // `.counter`：sans + tabular（`BalanceBar.tsx:60`）。
              style: counterStyle(fontSize: 12, color: color),
            ),
            if (hasTotal && showTotal)
              Flexible(
                child: Padding(
                  padding: const EdgeInsets.only(left: AidogSpace.sxs),
                  child: Text(
                    '/ $currency${formatCost(total!)}',
                    style: AidogType.micro.copyWith(
                      fontSize: 10,
                      color: theme.c.fg3,
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
          ],
        ),
        if (pct != null) ...[
          const SizedBox(height: AidogSpace.sxs),
          _Bar(
            ratio: pct / 100,
            color: color,
            height: 6,
            // 槽色是 --bg-glass（= surface），不是 surface2（`BalanceBar.tsx:69`）。
            track: theme.c.surface,
          ),
        ],
        if (label != null)
          Text(
            label!,
            style: AidogType.micro.copyWith(
              fontSize: 9,
              color: theme.c.fg3,
              fontWeight: FontWeight.w700,
            ),
          ),
      ],
    );
  }
}

/// `BalanceBar.tsx:27::remainingLevel`：剩余占比 ≥50 绿 / ≥20 黄 / 其余红。
ColorLevel remainingLevel(double pct) {
  if (pct >= 50) return ColorLevel.success;
  if (pct >= 20) return ColorLevel.warning;
  return ColorLevel.danger;
}

class _Bar extends StatelessWidget {
  const _Bar({
    required this.ratio,
    required this.color,
    required this.height,
    required this.track,
  });

  final double ratio;
  final Color color;
  final double height;
  final Color track;

  @override
  Widget build(BuildContext context) => ClipRRect(
    borderRadius: BorderRadius.circular(AidogRadius.sm),
    child: SizedBox(
      height: height,
      child: LinearProgressIndicator(
        value: clamp(ratio, 0, 1),
        minHeight: height,
        backgroundColor: track,
        valueColor: AlwaysStoppedAnimation<Color>(color),
      ),
    ),
  );
}

/// 统计 chip（`components/shared/StatChip.tsx`）：图标 + 值 + 单位标签。
class StatChip extends StatelessWidget {
  const StatChip({
    super.key,
    required this.icon,
    required this.value,
    required this.label,
    this.level,
    this.labelGap = AidogSpace.sxs,
  });

  final IconData icon;
  final String value;
  final String label;
  final ColorLevel? level;

  /// 值与标签之间的间距。React chip 内部一律 `gap: 6`（`StatChip.tsx:43-44`），
  /// 分组页按该真值传 6；其余调用点保持原值，等各自的对齐批次再收。
  final double labelGap;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final valueColor = level != null ? levelColor(level!, theme.c) : theme.c.fg;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
      decoration: BoxDecoration(
        // React 中性档底是 --bg-glass（= card = surface），不是 surface2；
        // 带 level 的档走 `--color-*-bg`（浅色 0.10 / 深色 0.12，
        // `globals.css:77-81` 与 `:830-834`）—— `StatChip.tsx:46` 的 levelBg。
        color: level == null || level == ColorLevel.neutral
            ? theme.c.surface
            : levelColor(level!, theme.c).withValues(
                alpha: theme.mode == AidogMode.dark ? 0.12 : 0.10,
              ),
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.pill),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 13, color: valueColor),
          const SizedBox(width: AidogSpace.ssm),
          Text(
            value,
            // React 挂 `.counter`：sans + tabular，不换等宽族（`StatChip.tsx:60`）。
            style: counterStyle(fontSize: 12, color: valueColor),
          ),
          SizedBox(width: labelGap),
          Text(
            label,
            style: AidogType.micro.copyWith(
              fontSize: 10,
              color: theme.c.fg3,
            ),
          ),
        ],
      ),
    );
  }
}

/// 小徽标（自动禁用 / 高峰 / 已过期 / 最近错误 / Coding Plan / 分组名 …）。
class MiniBadge extends StatelessWidget {
  const MiniBadge({
    super.key,
    required this.text,
    required this.color,
    this.tooltip,
    this.icon,
    this.onTap,
    this.accentText,
    this.accentColor,
    this.solid = false,
    this.borderColor,
    this.fontSize = 10,
    this.padY = 1,
    this.padX = AidogSpace.ssm,
    this.radius = 5,
    this.background,
    this.letterSpacing = 0,
    this.fontWeight = FontWeight.w600,
  });

  /// 字重。缺省 w600（shadcn `Badge` 的 `font-semibold`）；多 key 预览的协议
  /// 徽标 React 写的是 700（`MultiKeyPreview.tsx:69`）。
  final FontWeight fontWeight;

  /// 圆角。缺省 5；Skills / MCP 两页的中性 chip React 写的是 4 或 3
  /// （`SkillDetailView.tsx:138,152`、`McpModals.tsx:128`）。
  final double radius;

  /// 底色覆盖。null = 语义底（实心取 [color]，否则 [color] 12%）。
  /// React 这两页的 chip 多是 `--bg-floating` / `--bg-elevated` / `--accent-subtle`
  /// 这类**平底**，不是语义色兑水（`SkillsView.tsx:440`、`McpModals.tsx:129`）。
  final Color? background;

  /// 字距。缺省 0；sourceType 徽标 React 写的是 0.3（`SkillsView.tsx:442`）。
  final double letterSpacing;

  /// 字号。缺省 10；模型 badge React 写的是 11（`PlatformCard.tsx:710`）。
  final double fontSize;

  /// 左右内边距。缺省 6；shadcn `Badge` 在分组卡上被逐处覆盖成
  /// `padding: "0 5px"`（auto 徽标，`GroupListItem.tsx:211`）或 `"0 6px"`
  /// （路由模式徽标，`GroupListItem.tsx:215`）。
  final double padX;

  /// 上下内边距。缺省 1；模型 badge React 是 2。
  final double padY;

  final String text;
  final Color color;

  /// 跟在 [text] 后面、用 [accentColor] 单独上色的一小段（端点徽标的 `Code` 角标）。
  /// 整枚徽标不变色——变色会让人分不清它是端点名的一部分还是角标
  ///（`PlatformCard.tsx:697-700` 同形态）。
  final String? accentText;
  final Color? accentColor;

  /// 实心徽标：[color] 当底色，文字用页面底色反白。
  /// 用在「这是当前生效的那一个」这种需要一眼认出的标记上（分组页的默认分组徽标，
  /// `GroupListItem.tsx:207-209` 的 primary 底 + 反色字）。
  final bool solid;

  /// 描边覆盖。null = 默认（实心取 [color] / needsEdge，非实心取 [color] 30%）。
  /// 默认分组徽标用：React 那圈是 accent-edge（近黑 accent 需要亮边发声）。
  final Color? borderColor;
  final String? tooltip;
  final IconData? icon;

  /// 给了才可点（「最近测试」徽章展开响应正文用）。null = 纯展示。
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final theme0 = AidogTheme.of(context);
    // 判断一律拿**合成后的实色**算：[color] 允许是半透明 token（`peakIdle`
    // 是白 5%、`accentWash` 是白 6%），直接算得到的是「它自己多亮」，
    // 不是「它压在卡面上多亮」——那个数是假的。绘制仍用原 [color]，
    // 合成交给 Flutter。
    final fill = compositeOver(color, theme0.c.surface);
    // 实心徽标的文字色按**底色亮度**挑，不能写死成页面底色：
    // 深色主题的 accent 是近黑（#101012），原先的「文字用 theme.c.bg」
    // 会得到近黑字 + 近黑底，整枚徽标读不出来。
    final onSolid = fill.computeLuminance() > 0.5
        ? AidogColors.dark.bg
        : AidogColors.light.surface;
    // 底色与卡片表面差得太近时（同上那种近黑 accent），再用底色当描边等于没有
    // 描边，徽标与背景糊成一片 —— 这时借一条 lineStrong 把边缘划出来。
    final needsEdge = solid && contrastRatio(fill, theme0.c.surface) < 1.5;
    final body = Container(
      padding: EdgeInsets.symmetric(horizontal: padX, vertical: padY),
      decoration: BoxDecoration(
        color: background ?? (solid ? color : color.withValues(alpha: 0.12)),
        border: Border.all(
          color: borderColor ??
              (needsEdge
                  ? theme0.c.lineStrong
                  : solid
                  ? color
                  : color.withValues(alpha: 0.30)),
        ),
        borderRadius: BorderRadius.circular(radius),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (icon != null) ...[
            Icon(icon, size: 10, color: solid ? onSolid : color),
            // icon↔文字 gap 4（`PlatformCard.tsx:292,312,342`）。
            const SizedBox(width: 4),
          ],
          Flexible(
            child: Text.rich(
              TextSpan(
                text: text,
                children: accentText == null
                    ? null
                    : [
                        TextSpan(
                          text: ' $accentText',
                          style: TextStyle(
                            color: accentColor ?? color,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ],
              ),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AidogType.micro.copyWith(
                fontSize: fontSize,
                letterSpacing: letterSpacing,
                color: solid ? onSolid : color,
                fontWeight: fontWeight,
              ),
            ),
          ),
        ],
      ),
    );
    final tappable = onTap == null
        ? body
        : MouseRegion(
            cursor: SystemMouseCursors.click,
            child: GestureDetector(onTap: onTap, child: body),
          );
    return tooltip == null || tooltip!.isEmpty
        ? tappable
        : Tooltip(message: tooltip!, child: tappable);
  }
}

// ── 测试响应正文解析（`src/components/shared/TestResultBody.tsx`）───────

/// 结构化视图的一行。
typedef TestBodyRow = ({String label, String value});

/// [parseTestBody] 的结果：`rows` 非空 = 命中已知结构（渲染结构化）；
/// 否则回退 `text` 原文（两者互斥，与 React 的 `kind: known | raw` 等价）。
typedef ParsedTestBody = ({List<TestBodyRow> rows, String text});

/// 安全取字符串：基础类型转字符串，对象 / 数组 JSON 序列化
/// （`TestResultBody.tsx:17::toDisplay`）。
String testBodyDisplay(Object? v) {
  if (v == null) return '';
  if (v is String) return v;
  if (v is num || v is bool) return '$v';
  try {
    return jsonEncode(v);
  } catch (_) {
    return '';
  }
}

/// 解析测试响应正文（`TestResultBody.tsx:35::parseTestBody`）。
/// `t` 取文案，按仿函数模式由调用方注入。
ParsedTestBody parseTestBody(String raw, String Function(String key) t) {
  final text = raw.trim();
  if (text.isEmpty) return (rows: const <TestBodyRow>[], text: '');

  Object? parsed;
  try {
    parsed = jsonDecode(text);
  } catch (_) {
    return (rows: const <TestBodyRow>[], text: text);
  }
  if (parsed is! Map) return (rows: const <TestBodyRow>[], text: text);
  final obj = parsed.cast<String, Object?>();
  final rows = <TestBodyRow>[];

  // error 体
  final err = obj['error'];
  if (err != null) {
    if (err is Map) {
      final e = err.cast<String, Object?>();
      final msg = testBodyDisplay(e['message']);
      if (msg.isNotEmpty) {
        rows.add((label: t('testBody.errorMessage'), value: msg));
      }
      final type = testBodyDisplay(e['type']);
      if (type.isNotEmpty) {
        rows.add((label: t('testBody.errorType'), value: type));
      }
      final code = testBodyDisplay(e['code']);
      if (code.isNotEmpty) {
        rows.add((label: t('testBody.errorCode'), value: code));
      }
      // error 对象存在但无可识别子字段 → 整体序列化兜底
      if (msg.isEmpty && type.isEmpty && code.isEmpty) {
        rows.add((label: t('testBody.error'), value: testBodyDisplay(err)));
      }
    } else {
      rows.add((label: t('testBody.error'), value: testBodyDisplay(err)));
    }
  }

  // usage（Anthropic: input_tokens/output_tokens；OpenAI: prompt_tokens/completion_tokens）
  final usage = obj['usage'];
  if (usage is Map) {
    final u = usage.cast<String, Object?>();
    final input = testBodyDisplay(u['input_tokens'] ?? u['prompt_tokens']);
    if (input.isNotEmpty) {
      rows.add((label: t('testBody.inputTokens'), value: input));
    }
    final output = testBodyDisplay(
      u['output_tokens'] ?? u['completion_tokens'],
    );
    if (output.isNotEmpty) {
      rows.add((label: t('testBody.outputTokens'), value: output));
    }
  }

  // model
  final model = testBodyDisplay(obj['model']);
  if (model.isNotEmpty) rows.add((label: t('testBody.model'), value: model));

  // 文本内容：Anthropic content[].text / OpenAI choices[].message.content
  final content = _extractContentText(obj);
  if (content.isNotEmpty) {
    rows.add((label: t('testBody.content'), value: content));
  }

  if (rows.isNotEmpty) return (rows: rows, text: '');
  return (rows: const <TestBodyRow>[], text: text);
}

/// 从 Anthropic content 数组或 OpenAI choices 数组抽取文本内容（取第一段非空）。
String _extractContentText(Map<String, Object?> obj) {
  final content = obj['content'];
  if (content is List) {
    for (final block in content) {
      if (block is Map) {
        final txt = testBodyDisplay(block['text']);
        if (txt.isNotEmpty) return txt;
      }
    }
  }
  final choices = obj['choices'];
  if (choices is List) {
    for (final choice in choices) {
      if (choice is Map) {
        final message = choice['message'];
        if (message is Map) {
          final txt = testBodyDisplay(message['content']);
          if (txt.isNotEmpty) return txt;
        }
      }
    }
  }
  return '';
}

/// 测试响应正文的渲染（`TestResultBody.tsx:127`）。命中已知结构 → key-value 视图，
/// 否则原文；两者都空 → 什么也不画。
class TestResultBody extends StatelessWidget {
  const TestResultBody({super.key, required this.body});

  final String body;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final parsed = parseTestBody(body, t.t);
    if (parsed.rows.isEmpty) {
      if (parsed.text.isEmpty) return const SizedBox.shrink();
      return Text(
        parsed.text,
        style: AidogType.numSm.copyWith(color: theme.c.fg2),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final r in parsed.rows)
          Padding(
            padding: const EdgeInsets.only(bottom: 2),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  r.label,
                  style: AidogType.micro.copyWith(
                    color: theme.c.fg3,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(width: AidogSpace.sxs),
                Flexible(
                  child: Text(
                    r.value,
                    style: AidogType.micro.copyWith(color: theme.c.fg2),
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

/// 配额档位块（`PlatformCard.tsx:496` 紧凑态 / `:604` 展开态两版，同一份数据两种尺寸）。
class QuotaTierBlock extends StatelessWidget {
  const QuotaTierBlock({
    super.key,
    required this.tier,
    required this.remainSuffix,
    this.expanded = false,
    this.nowMs,
  });

  final QuotaTierDisplay tier;
  final String remainSuffix;
  final bool expanded;
  final int? nowMs;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final isMcp = tier.name == 'mcp_monthly';
    final value = isMcp && tier.limit != null
        ? '${tier.remaining ?? 0}/${tier.limit}'
        : '${tier.remainPct.toStringAsFixed(0)}%';
    final color = tier.level == ColorLevel.neutral
        ? theme.c.fg2
        : levelColor(tier.level, theme.c);
    final countdown = formatResetCountdown(tier.resetsAt, nowMs: nowMs);
    final resetClock = formatResetClock(tier.resetsAt, nowMs: nowMs);
    return Container(
      constraints: BoxConstraints(
        minWidth: expanded ? 96 : 64,
        maxWidth: expanded ? 150 : 120,
      ),
      padding: EdgeInsets.symmetric(
        horizontal: expanded ? AidogSpace.smd : AidogSpace.ssm,
        vertical: expanded ? AidogSpace.ssm : 3,
      ),
      decoration: BoxDecoration(
        // 底是 --bg-glass（= surface），不是 surface2（`PlatformCard.tsx:527,637`）。
        color: theme.c.surface,
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          _Bar(
            ratio: clamp(tier.remainPct, 0, 100) / 100,
            color: color,
            height: expanded ? 5 : 4,
            track: theme.c.bg,
          ),
          // 展开态块内竖向 gap 4（`PlatformCard.tsx:634`）；紧凑态仍是 2。
          SizedBox(height: expanded ? 4 : 2),
          // 主数 11 w700 + 后缀独立 8px w600 半透明（PlatformCard.tsx:537-539；
          // 展开态主数 13 = numMd）。原先后缀与主数同号同阶。
          Text.rich(
            TextSpan(
              text: value,
              children: [
                TextSpan(
                  // 展开态后缀 9px + 左间距 2（`PlatformCard.tsx:648`）；
                  // 紧凑态仍是 8px 无间距（`:539`）。
                  text: expanded ? ' $remainSuffix' : remainSuffix,
                  style: TextStyle(
                    fontSize: expanded ? 9 : 8,
                    fontWeight: FontWeight.w600,
                    color: theme.c.fg.withValues(alpha: 0.65),
                  ),
                ),
              ],
            ),
            // 展开态主数 13 w700 行高 1.1（`PlatformCard.tsx:647`）。
            style: (expanded ? AidogType.numMd : AidogType.numSm).copyWith(
              fontSize: expanded ? 13 : 11,
              height: 1.1,
              color: theme.c.fg,
              fontWeight: FontWeight.w700,
            ),
          ),
          // 档位名 · 倒计时：紧凑态 9px 无图标、重置时刻另起一行 8px（:540-547）；
          // 展开态 11px + 时钟图标同排（:650-657）。
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Flexible(
                child: Text(
                  tierLabel(tier.name),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AidogType.micro.copyWith(
                    fontSize: expanded ? 11 : 9,
                    letterSpacing: 0,
                    color: theme.c.fg3,
                  ),
                ),
              ),
              if (countdown.isNotEmpty) ...[
                if (expanded) ...[
                  const SizedBox(width: 3),
                  Icon(Icons.schedule, size: 11, color: theme.c.fg3),
                  const SizedBox(width: 2),
                ],
                Flexible(
                  child: Text(
                    // 紧凑态档名与倒计时之间有一个 ` ·`（`PlatformCard.tsx:541`）；
                    // 展开态是图标分隔，倒计时后再接重置时刻。
                    expanded
                        ? (resetClock.isNotEmpty
                              ? '$countdown · $resetClock'
                              : countdown)
                        : ' ·$countdown',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    // 展开态 11 / 紧凑态 9，字距 0（`PlatformCard.tsx:650`）。
                    style: AidogType.micro.copyWith(
                      fontSize: expanded ? 11 : 9,
                      letterSpacing: 0,
                      color: theme.c.fg3,
                    ),
                  ),
                ),
              ],
            ],
          ),
          if (!expanded && resetClock.isNotEmpty)
            Text(
              resetClock,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AidogType.micro.copyWith(
                fontSize: 8,
                color: theme.c.fg3,
              ),
            ),
        ],
      ),
    );
  }
}

/// 骨架占位（外部 HTTP 待回）。**不画零值假数**：给一块灰条，不给 `0`。
class SkeletonBox extends StatefulWidget {
  const SkeletonBox({
    super.key,
    required this.width,
    required this.height,
    this.semanticLabel,
  });

  final double width;
  final double height;
  final String? semanticLabel;

  @override
  State<SkeletonBox> createState() => _SkeletonBoxState();
}

/// 带流光的骨架块（React `.skeleton`：1.4s ease-in-out 无限往复的 shimmer，
/// `globals.css:206-216`）—— 纯灰块和「加载完了但没数据」长得一样。
class _SkeletonBoxState extends State<SkeletonBox>
    with SingleTickerProviderStateMixin {
  late final AnimationController _ctrl = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1400),
  )..repeat(reverse: true);

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Semantics(
      label: widget.semanticLabel,
      child: AnimatedBuilder(
        animation: _ctrl,
        builder: (context, _) => Container(
          width: widget.width,
          height: widget.height,
          decoration: BoxDecoration(
            color: Color.lerp(
              theme.c.surface2,
              theme.c.line,
              Curves.easeInOut.transform(_ctrl.value) * 0.6,
            ),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
        ),
      ),
    );
  }

}
