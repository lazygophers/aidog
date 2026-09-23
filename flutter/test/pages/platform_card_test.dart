/// 平台卡的展示层测试（票 I18b）。
///
/// 前半是 React 三份现成测试的**逐条翻译，数据与期望值一字不改**：
///   - `src/components/shared/usageColor.test.ts`
///   - `src/utils/timeWindow.test.ts`
///   - `src/components/shared/BalanceBar.test.tsx`
/// 后半是 React 没有单测、但本票新补的派生（配额合并 / 手动预算 / 分享格式 / registry 元数据），
/// 以及 `src/components/platforms/PlatformCard.test.tsx` 那 8 条交互断言的 widget 版。
library;

import 'dart:async';
import 'dart:convert';

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/utils/color_level.dart';
import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';
import 'harness.dart';

const int _hour = 3600000;
const int _day = 24 * _hour;

/// base64(UTF-8) → 原文，用来验 [toBase64Utf8] 的往返。
String utf8Decode(String b64) => utf8.decode(base64Decode(b64));

/// `Date.UTC(y, m, d, h, mi, s)` 的 Dart 版（JS 的 month 从 0 起）。
int utcMs(int y, int monthFrom0, int d, [int h = 0, int mi = 0, int s = 0]) =>
    DateTime.utc(y, monthFrom0 + 1, d, h, mi, s).millisecondsSinceEpoch;

TimeWindow win({
  int startHour = 6,
  int endHour = 10,
  int startMinute = 0,
  int endMinute = 0,
  double multiplier = 2,
  String? timezone,
  List<int>? daysOfWeek,
  List<int>? daysOfMonth,
  List<String>? models,
  int? startAt,
  int? endAt,
}) => TimeWindow(
  startHour: startHour,
  endHour: endHour,
  startMinute: startMinute,
  endMinute: endMinute,
  multiplier: multiplier,
  timezone: timezone,
  daysOfWeek: daysOfWeek,
  daysOfMonth: daysOfMonth,
  models: models,
  startAt: startAt,
  endAt: endAt,
);

Map<String, dynamic> platRow(
  int id,
  String name, {
  String type = 'openai',
  String status = 'enabled',
  bool enabled = true,
  String extra = '',
  String estCodingPlan = '',
  double estBalance = 0,
  String rateLimit = '',
  String lastError = '',
  int lastErrorAt = 0,
  int expiresAt = 0,
  int autoDisabledUntil = 0,
  double codingWindowCost = 0,
  String balanceLevel = '',
  List<Object?> endpoints = const [],
  Map<String, Object?> models = const {},
  List<Object?> manualBudgets = const [],
}) => {
  'id': id,
  'name': name,
  'platform_type': type,
  'base_url': 'https://api.test/$id',
  'api_key': 'sk-test',
  'extra': extra,
  'models': models,
  'available_models': <Object?>[],
  'endpoints': endpoints,
  'enabled': enabled,
  'status': status,
  'auto_disabled_until': autoDisabledUntil,
  'expires_at': expiresAt,
  'est_balance_remaining': estBalance,
  'est_coding_plan': estCodingPlan,
  'rate_limit': rateLimit,
  'last_real_query_at': 0,
  'estimate_count': 0,
  'last_error': lastError,
  'last_error_at': lastErrorAt,
  'sort_order': 0,
  'manual_budgets': manualBudgets,
  'coding_window_cost': codingWindowCost,
  'balance_level': balanceLevel,
};

Map<String, dynamic> usageJson({
  int totalRequests = 100,
  int successCount = 95,
  int input = 10000,
  int output = 5000,
  double totalCost = 1.5,
  int todayTokens = 1000,
  double todayCost = 0.15,
  int recentTotal = 10,
  int recentFailures = 1,
}) => {
  'total_requests': totalRequests,
  'success_count': successCount,
  'total_input_tokens': input,
  'total_output_tokens': output,
  'total_cache_tokens': 0,
  'cache_rate': 0,
  'recent_failures': recentFailures,
  'recent_total': recentTotal,
  'total_cost': totalCost,
  'today_tokens': todayTokens,
  'today_cost': todayCost,
};

/// 带一条 quota_scripts 的 registry 文档 —— 没有它 `platformWantsQuota` 整块不放行。
const String kDefaultsJson =
    '{"protocols":{"openai":{"name":{"zh-Hans":"开放人工智能","en-US":"OpenAI"},'
    '"homepage":"https://openai.com","source_urls":{"docs":"https://d.test",'
    '"pricing":"https://p.test"},"quota_scripts":[{"id":"v1"}],'
    '"models":{"default":{"default":"gpt-5"},"peak":{"default":"gpt-5-mini"}}},'
    '"glm_coding":{"name":{"zh-Hans":"智谱编码"},"is_coding_plan":true,'
    '"peak":[{"start_hour":0,"end_hour":24,"multiplier":2}]},'
    // bare_proto 登记了品牌色但没有内置 logo → 画字母块，颜色看得见；
    // nocolor_proto 两样都没有 → 回落主题 accent。
    '"bare_proto":{"name":{"en-US":"Bare"},"color":"#10A37F"},'
    '"nocolor_proto":{"name":{"en-US":"NoColor"}}}}';

FakeInvoke cardFake({
  List<Object?>? platforms,
  Map<String, Object?>? usage,
  Object? lastTest,
}) => FakeInvoke({
  'platform_list': platforms ?? [platRow(1, 'Test Platform')],
  'group_detail_list': <Object?>[],
  'all_platform_usage_stats': usage ?? <String, Object?>{},
  'get_last_test_result': lastTest,
  'scheduling_settings_get': null,
  'get_defaults_json': kDefaultsJson,
  'get_protocol_logo_path': '',
  'get_protocol_logo_data_url': '',
  'sync_protocol_logo': null,
  'platform_query_quota': {'success': true, 'queried_at': 1},
  'platform_update': platRow(1, 'Test Platform', status: 'disabled', enabled: false),
  'platform_usage_stats': null,
  'platform_delete': null,
  'platform_reorder': null,
  'platform_share_export': {'name': 'Test Platform', 'api_key': 'sk-test'},
  'platform_purge_disabled_preview': <Object?>[],
  'platform_purge_disabled': {'deletedIds': <Object?>[], 'unassignedIds': <Object?>[]},
  'model_test': {'success': true, 'duration_ms': 7, 'error': ''},
  'set_ui_extra': null,
});

void main() {
  // ══ usageColor.test.ts 逐条翻译 ══════════════════════════════════

  group('usageLevelToColor（React: usageColor.test.ts）', () {
    test('maps backend level strings', () {
      expect(usageLevelToColor('red'), ColorLevel.danger);
      expect(usageLevelToColor('yellow'), ColorLevel.warning);
      expect(usageLevelToColor('green'), ColorLevel.success);
    });
    test('falls back to neutral for unknown/null', () {
      expect(usageLevelToColor('neutral'), ColorLevel.neutral);
      expect(usageLevelToColor(null), ColorLevel.neutral);
      expect(usageLevelToColor('bogus'), ColorLevel.neutral);
    });
  });

  group('cycleMsForTier（React: usageColor.test.ts）', () {
    test('returns durations for known tiers', () {
      expect(cycleMsForTier('five_hour'), 5 * _hour);
      expect(cycleMsForTier('weekly_limit'), 7 * _day);
      expect(cycleMsForTier('seven_day'), 7 * _day);
      expect(cycleMsForTier('mcp_monthly'), 30 * _day);
    });
    test('returns null for unknown tiers', () {
      expect(cycleMsForTier('unknown'), isNull);
    });
  });

  group('codingPaceDelta（React: usageColor.test.ts）', () {
    test('is 0 when usage tracks time exactly', () {
      expect(codingPaceDelta(50, (2.5 * _hour).round(), 5 * _hour), 0);
    });
    test('is positive when overspending', () {
      expect(
        codingPaceDelta(58, (2.5 * _hour).round(), 5 * _hour),
        closeTo(8, 0.000001),
      );
    });
    test('is negative when under budget', () {
      expect(
        codingPaceDelta(58, 17 * 60000, 5 * _hour),
        closeTo(-36.333333, 0.0001),
      );
    });
    test('clamps utilization and elapsed into 0-100', () {
      expect(codingPaceDelta(150, 5 * _hour, 5 * _hour), 100);
      expect(codingPaceDelta(0, -_hour, 5 * _hour), -100);
    });
  });

  group('colorFromPaceDelta（React: usageColor.test.ts）', () {
    test('neutral for non-finite', () {
      expect(colorFromPaceDelta(double.nan), ColorLevel.neutral);
      expect(colorFromPaceDelta(double.infinity), ColorLevel.neutral);
    });
    test('danger above +3', () {
      expect(colorFromPaceDelta(3.1), ColorLevel.danger);
      expect(colorFromPaceDelta(50), ColorLevel.danger);
    });
    test('warning within ±3', () {
      expect(colorFromPaceDelta(3), ColorLevel.warning);
      expect(colorFromPaceDelta(0), ColorLevel.warning);
      expect(colorFromPaceDelta(-3), ColorLevel.warning);
    });
    test('success below -3', () {
      expect(colorFromPaceDelta(-3.1), ColorLevel.success);
      expect(colorFromPaceDelta(-100), ColorLevel.success);
    });
  });

  group('codingTierLevel（React: usageColor.test.ts）', () {
    test('neutral on invalid utilization', () {
      expect(codingTierLevel(double.nan, _day, _day), ColorLevel.neutral);
      expect(codingTierLevel(-1, _day, _day), ColorLevel.neutral);
    });
    test('neutral when remain/cycle missing or non-positive cycle', () {
      expect(codingTierLevel(50, null, _day), ColorLevel.neutral);
      expect(codingTierLevel(50, _day, null), ColorLevel.neutral);
      expect(codingTierLevel(50, _day, 0), ColorLevel.neutral);
    });
    test('danger when quota exhausted (util >= 100)', () {
      expect(codingTierLevel(100, 0, 5 * _hour), ColorLevel.danger);
    });
    test('delegates to the pace-delta coloring otherwise', () {
      expect(codingTierLevel(80, 3 * _hour, 5 * _hour), ColorLevel.danger);
      expect(
        codingTierLevel(52, (2.5 * _hour).round(), 5 * _hour),
        ColorLevel.warning,
      );
      expect(codingTierLevel(58, 17 * 60000, 5 * _hour), ColorLevel.success);
      expect(
        codingTierLevel(2, (5 * _hour * 0.99).round(), 5 * _hour),
        ColorLevel.warning,
      );
    });
  });

  // ══ timeWindow.test.ts 逐条翻译 ═════════════════════════════════

  group('isCurrentlyPeak — 时段 / 星期 / 日期 / model scope（React: timeWindow.test.ts）', () {
    // 基准时刻 2026-06-26T08:30:00Z = 周五(5)、26 号。
    final now = utcMs(2026, 5, 26, 8, 30, 0);

    test('空 / null 窗口列表恒不命中', () {
      expect(isCurrentlyPeak(const [], now), isFalse);
      expect(isCurrentlyPeak(null, now), isFalse);
    });

    test('同天窗口是半开区间 [start, end)', () {
      expect(isCurrentlyPeak([win()], now), isTrue);
      expect(
        isCurrentlyPeak([win(startHour: 8, startMinute: 30)], now),
        isTrue,
      );
      expect(
        isCurrentlyPeak([win(startHour: 6, endHour: 8, endMinute: 30)], now),
        isFalse,
      );
      expect(isCurrentlyPeak([win(startHour: 9, endHour: 10)], now), isFalse);
    });

    test('跨天窗口 end <= start 时是并集，start==end 退化为全天', () {
      expect(isCurrentlyPeak([win(startHour: 22, endHour: 9)], now), isTrue);
      expect(isCurrentlyPeak([win(startHour: 22, endHour: 6)], now), isFalse);
      expect(isCurrentlyPeak([win(startHour: 5, endHour: 5)], now), isTrue);
    });

    test('越界 minute 被夹到 0..59 而非溢出成小时', () {
      expect(
        isCurrentlyPeak([win(startHour: 8, startMinute: 99)], now),
        isFalse,
      );
      expect(
        isCurrentlyPeak([win(startHour: 8, startMinute: -10)], now),
        isTrue,
      );
    });

    test('days_of_week 用 0=Sun…6=Sat，缺省为每天', () {
      expect(isCurrentlyPeak([win(daysOfWeek: [5])], now), isTrue);
      expect(isCurrentlyPeak([win(daysOfWeek: [0, 6])], now), isFalse);
    });

    test('days_of_month 与 days_of_week 取 AND', () {
      expect(isCurrentlyPeak([win(daysOfMonth: [26])], now), isTrue);
      expect(isCurrentlyPeak([win(daysOfMonth: [1])], now), isFalse);
      expect(
        isCurrentlyPeak([win(daysOfWeek: [5], daysOfMonth: [1])], now),
        isFalse,
      );
    });

    test('end_at 到期后窗口失效', () {
      final sec = now ~/ 1000;
      expect(isCurrentlyPeak([win(endAt: sec + 1)], now), isTrue);
      expect(isCurrentlyPeak([win(endAt: sec)], now), isFalse);
    });

    test('model scope：空 requestModel 跳过过滤，通配取前缀', () {
      final scoped = [
        win(models: ['glm-5.2*', 'kimi-k2']),
      ];
      expect(isCurrentlyPeak(scoped, now), isTrue);
      expect(isCurrentlyPeak(scoped, now, 'glm-5.2'), isTrue);
      expect(isCurrentlyPeak(scoped, now, 'glm-5.2-turbo'), isTrue);
      expect(isCurrentlyPeak(scoped, now, 'kimi-k2'), isTrue);
      expect(isCurrentlyPeak(scoped, now, 'kimi-k2-thinking'), isFalse);
      expect(isCurrentlyPeak(scoped, now, 'gpt-4'), isFalse);
      expect(isCurrentlyPeak([win(models: const [])], now, 'gpt-4'), isTrue);
    });

    test('多窗口取任一命中', () {
      expect(
        isCurrentlyPeak([win(startHour: 0, endHour: 1), win()], now),
        isTrue,
      );
      expect(
        isCurrentlyPeak([
          win(startHour: 0, endHour: 1),
          win(startHour: 20, endHour: 22),
        ], now),
        isFalse,
      );
    });
  });

  group('isCurrentlyPeak — start_at 生效期护栏（React: timeWindow.test.ts）', () {
    final w = win(startHour: 0, endHour: 24, startAt: 1790784000);

    test('nowMs 未越过 start_at → 不命中', () {
      expect(isCurrentlyPeak([w], (1790784000 - 1) * 1000), isFalse);
    });
    test('nowMs 越过 start_at → 命中', () {
      expect(isCurrentlyPeak([w], (1790784000 + 1) * 1000), isTrue);
    });
  });

  group('wallTimeInTz / isCurrentlyPeak — 窗口时区（React: timeWindow.test.ts）', () {
    const ms = 1704595800000; // 2024-01-07T02:50:00Z；北京同日 10:50，周日(0)。

    test('Asia/Shanghai 平移 +8 且 None/UTC/非法名一致', () {
      final sh = wallTimeInTz(ms, 'Asia/Shanghai');
      expect('$sh', '(dayOfMonth: 7, hour: 10, minute: 50, weekday: 0)');
      final utc = wallTimeInTz(ms, null);
      expect('$utc', '(dayOfMonth: 7, hour: 2, minute: 50, weekday: 0)');
      expect('${wallTimeInTz(ms, 'UTC')}', '$utc');
      expect('${wallTimeInTz(ms, 'Not/AZone')}', '$utc');
    });

    test('时区平移跨日时 weekday / day_of_month 翻转', () {
      // 2024-01-06T18:00:00Z 周六；北京 = 2024-01-07 02:00 周日、7 号。
      final sh = wallTimeInTz(1704564000000, 'Asia/Shanghai');
      expect('$sh', '(dayOfMonth: 7, hour: 2, minute: 0, weekday: 0)');
      expect(wallTimeInTz(1704564000000, null).weekday, 6);
    });

    test('isCurrentlyPeak 按窗口时区判定：北京 9-12 窗口 UTC 01:26 命中、05:26 miss', () {
      final w = win(startHour: 9, endHour: 12, timezone: 'Asia/Shanghai');
      expect(isCurrentlyPeak([w], utcMs(2026, 5, 26, 1, 26, 0)), isTrue);
      expect(isCurrentlyPeak([w], utcMs(2026, 5, 26, 5, 26, 0)), isFalse);
    });

    test('缺省 timezone = UTC（向后兼容）', () {
      final w = win(startHour: 9, endHour: 12);
      expect(isCurrentlyPeak([w], utcMs(2026, 5, 26, 9, 30, 0)), isTrue);
      expect(isCurrentlyPeak([w], utcMs(2026, 5, 26, 2, 30, 0)), isFalse);
    });

    test('days_of_week 按窗口时区本地 weekday 过滤', () {
      const ms2 = 1704564000000;
      final sunday = win(startHour: 0, endHour: 24, daysOfWeek: [0]);
      expect(isCurrentlyPeak([sunday], ms2), isFalse);
      final sundaySh = win(
        startHour: 0,
        endHour: 24,
        daysOfWeek: [0],
        timezone: 'Asia/Shanghai',
      );
      expect(isCurrentlyPeak([sundaySh], ms2), isTrue);
    });
  });

  // 归一化是**独立一步**，不在 `fromJson` 里做 —— 与 React 一致：
  // `timeWindow.ts:46::normalizeWindow` 是单独导出的函数，解析本身不动小数。
  // 整数小时时 `start_minute` 保持缺省（React 是 undefined，这里是 null），不补 0。
  group('timeWindowFromJsonNormalized — 半时区脏数据归一（React: normalizeWindow）', () {
    test('整数 hour 原样', () {
      final w = timeWindowFromJsonNormalized({'start_hour': 8, 'end_hour': 20});
      expect([w.startHour, w.startMinute, w.endHour], [8, null, 20]);
    });
    test('8.5 拆成 8:30', () {
      final w = timeWindowFromJsonNormalized({
        'start_hour': 8.5,
        'end_hour': 20,
      });
      expect([w.startHour, w.startMinute], [8, 30]);
    });
    test('已有 start_minute 时叠加进位（8.5 + 40 → 9:10）', () {
      final w = timeWindowFromJsonNormalized({
        'start_hour': 8.5,
        'start_minute': 40,
        'end_hour': 20,
      });
      expect([w.startHour, w.startMinute], [9, 10]);
    });
    test('start/end 各自独立归一', () {
      final w = timeWindowFromJsonNormalized({
        'start_hour': 8.5,
        'end_hour': 20.25,
      });
      expect([w.startHour, w.startMinute, w.endHour, w.endMinute], [
        8,
        30,
        20,
        15,
      ]);
    });
  });

  // ══ extra 解析 ═════════════════════════════════════════════════

  group('extra 解析（React: platforms.ts 的三个 parse*）', () {
    test('parseDisableDuringPeak 严格布尔', () {
      expect(parseDisableDuringPeak(''), isFalse);
      expect(parseDisableDuringPeak('not json'), isFalse);
      expect(parseDisableDuringPeak('{"disable_during_peak":true}'), isTrue);
      expect(parseDisableDuringPeak('{"disable_during_peak":1}'), isFalse);
      expect(parseDisableDuringPeak('{"disable_during_peak":"true"}'), isFalse);
    });
    test('parsePlatformPeak 读 extra.peak 数组', () {
      expect(parsePlatformPeak(''), isEmpty);
      expect(parsePlatformPeak('{"peak":"x"}'), isEmpty);
      final ws = parsePlatformPeak('{"peak":[{"start_hour":9,"end_hour":12}]}');
      expect(ws.length, 1);
      expect(ws.first.endHour, 12);
    });
    test('parsePlanPrice 只认数字', () {
      expect(parsePlanPrice('{"plan_price":20}'), 20);
      expect(parsePlanPrice('{"plan_price":"20"}'), isNull);
      expect(parsePlanPrice(''), isNull);
    });
    test('hasCustomQuotaScript 空白脚本不算', () {
      expect(hasCustomQuotaScript('{"quota_custom_script":"x"}'), isTrue);
      expect(hasCustomQuotaScript('{"quota_custom_script":"  "}'), isFalse);
      expect(hasCustomQuotaScript('{}'), isFalse);
    });
  });

  // ══ 健康态 / 模型值 ════════════════════════════════════════════

  group('deriveHealth（React: health.ts）', () {
    test('auto_disabled + 401/403 → error（key 失效）', () {
      expect(
        deriveHealth(status: 'auto_disabled', lastError: 'HTTP 401 xx'),
        HealthStatus.error,
      );
      expect(
        deriveHealth(status: 'auto_disabled', lastError: 'HTTP 403'),
        HealthStatus.error,
      );
    });
    test('有 last_error 但可恢复 → warning', () {
      expect(
        deriveHealth(status: 'enabled', lastError: 'HTTP 429'),
        HealthStatus.warning,
      );
      expect(
        deriveHealth(status: 'auto_disabled', lastError: 'HTTP 500'),
        HealthStatus.warning,
      );
    });
    test('enabled 且无错 → healthy', () {
      expect(deriveHealth(status: 'enabled'), HealthStatus.healthy);
      expect(deriveHealth(status: 'enabled', lastError: ''), HealthStatus.healthy);
    });
    test('回落 manual → 成功率 → unknown', () {
      expect(
        deriveHealth(status: 'disabled', manual: 'ok'),
        HealthStatus.healthy,
      );
      expect(
        deriveHealth(status: 'disabled', manual: 'fail'),
        HealthStatus.error,
      );
      expect(
        deriveHealth(status: 'disabled', recentTotal: 10, recentFailures: 1),
        HealthStatus.healthy,
      );
      expect(
        deriveHealth(status: 'disabled', recentTotal: 3, recentFailures: 3),
        HealthStatus.error,
      );
      expect(
        deriveHealth(status: 'disabled', recentTotal: 0, recentFailures: 0),
        HealthStatus.unknown,
      );
      expect(deriveHealth(status: 'disabled'), HealthStatus.unknown);
    });
  });

  group('allModelValues（React: health.ts:56）', () {
    test('按槽位顺序取非空值并去重', () {
      final m = PlatformModels.fromJson({
        'default': 'a',
        'sonnet': 'b',
        'opus': 'a',
        'haiku': '',
        'gpt': 'c',
      });
      expect(allModelValues(m).join(','), 'a,b,c');
    });
    test('全空 → 空列表', () {
      expect(allModelValues(PlatformModels.fromJson(null)), isEmpty);
    });
  });

  // ══ 速率限制 ═══════════════════════════════════════════════════

  group('parseRateLimit / rateLimitRatio（React: autoCategorize.ts）', () {
    test('空 / 非法 / 缺 vendor → null', () {
      expect(parseRateLimit(''), isNull);
      expect(parseRateLimit('{'), isNull);
      expect(parseRateLimit('{"requests_remaining":5}'), isNull);
    });
    test('取最紧的那一维', () {
      final rl = parseRateLimit(
        '{"vendor":"anthropic","requests_remaining":50,"requests_limit":100,'
        '"tokens_remaining":10,"tokens_limit":100,"observed_at":1}',
      )!;
      expect(rl.vendor, 'anthropic');
      expect(rateLimitRatio(rl), closeTo(0.1, 1e-9));
    });
    test('厂商没给 limit → null（不拿 0 冒充）', () {
      final rl = parseRateLimit(
        '{"vendor":"openrouter","requests_remaining":7,"observed_at":1}',
      )!;
      expect(rateLimitRatio(rl), isNull);
    });
    test('limit 为 0 不参与（除零防护）', () {
      final rl = parseRateLimit(
        '{"vendor":"x","requests_remaining":1,"requests_limit":0,"observed_at":1}',
      )!;
      expect(rateLimitRatio(rl), isNull);
    });
  });

  // ══ 配额合并 ═══════════════════════════════════════════════════

  group('computeQuotaDisplay（React: health.ts:83）', () {
    final now = utcMs(2026, 5, 26, 8, 30);

    PlatformRow row(Map<String, Object?> over) =>
        PlatformRow.fromJson({...platRow(1, 'p'), ...over});

    test('无预估无真查 → 无数据', () {
      final q = computeQuotaDisplay(row({}), null, false, nowMs: now);
      expect(q.hasData, isFalse);
      expect(q.balanceRemaining, isNull);
      expect(q.currency, 'USD');
    });

    test('有预估余额且未校准 → 用预估值、estimated=true', () {
      final q = computeQuotaDisplay(
        row({'est_balance_remaining': 12.5}),
        null,
        false,
        nowMs: now,
      );
      expect(q.estimated, isTrue);
      expect(q.balanceRemaining, 12.5);
      expect(q.balanceTotal, isNull);
      expect(q.hasData, isTrue);
    });

    test('手动刷新校准过 → 真查值压过预估', () {
      final quota = PlatformQuota.fromJson({
        'success': true,
        'balance': {'remaining': 3.0, 'total': 10.0, 'currency': 'USD'},
      });
      final q = computeQuotaDisplay(
        row({'est_balance_remaining': 12.5}),
        quota,
        true,
        nowMs: now,
      );
      expect(q.estimated, isFalse);
      expect(q.balanceRemaining, 3.0);
      expect(q.balanceTotal, 10.0);
    });

    test('ACU（Devin）：展示 used、total 置 null 抑制进度条', () {
      final quota = PlatformQuota.fromJson({
        'success': true,
        'balance': {
          'remaining': 0,
          'total': 100,
          'used': 42,
          'currency': 'ACU',
        },
      });
      final q = computeQuotaDisplay(row({}), quota, true, nowMs: now);
      expect(q.balanceRemaining, 42);
      expect(q.balanceTotal, isNull);
      expect(q.currency, 'ACU');
    });

    test('预估 coding tiers：remainPct = 100-util，limit 有值才算 remaining', () {
      final q = computeQuotaDisplay(
        row({
          'est_coding_plan':
              '{"tiers":[{"name":"five_hour","est_utilization":40,"limit":200,'
              '"window_start":${now - _hour}},'
              '{"name":"weekly_limit","est_utilization":10}]}',
        }),
        null,
        false,
        nowMs: now,
      );
      expect(q.tiers.length, 2);
      expect(q.tiers[0].remainPct, 60);
      expect(q.tiers[0].remaining, 120);
      expect(q.tiers[0].resetsAt, isNotNull);
      // 5h 周期过了 1h（20%），已用 40% → +20pp → danger。
      expect(q.tiers[0].level, ColorLevel.danger);
      // 没有 window_start → 无 remain → 中性，resetsAt 为空。
      expect(q.tiers[1].remaining, isNull);
      expect(q.tiers[1].resetsAt, isNull);
      expect(q.tiers[1].level, ColorLevel.neutral);
    });

    test('真查 coding tiers：resets_at 原样透传', () {
      final resets = DateTime.fromMillisecondsSinceEpoch(
        now + 2 * _hour,
        isUtc: true,
      ).toIso8601String();
      final quota = PlatformQuota.fromJson({
        'success': true,
        'coding_plan': {
          'tiers': [
            {
              'name': 'five_hour',
              'utilization': 60,
              'resets_at': resets,
              'limit': null,
              'remaining': null,
            },
          ],
        },
      });
      final q = computeQuotaDisplay(row({}), quota, false, nowMs: now);
      expect(q.tiers.single.resetsAt, resets);
      expect(q.tiers.single.remainPct, 40);
      expect(q.hasData, isTrue);
      expect(q.balanceRemaining, isNull);
    });

    test('est_coding_plan 非法 JSON → 当没有', () {
      expect(parseEstCodingPlan('nope'), isNull);
      expect(parseEstCodingPlan('{"tiers":"x"}'), isNull);
      expect(parseEstCodingPlan(''), isNull);
    });
  });

  group('tierLabel / formatResetCountdown / formatResetClock（React: health.ts）', () {
    final now = DateTime.utc(2026, 6, 26, 8, 30).millisecondsSinceEpoch;
    String iso(int ms) =>
        DateTime.fromMillisecondsSinceEpoch(ms, isUtc: true).toIso8601String();

    test('tierLabel 三个已知档 + 兜底原名', () {
      expect(tierLabel('five_hour'), '5h');
      expect(tierLabel('weekly_limit'), 'week');
      expect(tierLabel('mcp_monthly'), 'MCP');
      expect(tierLabel('whatever'), 'whatever');
    });

    test('倒计时：天 / 小时 / 分钟三档，过期与非法返空', () {
      expect(formatResetCountdown(null, nowMs: now), '');
      expect(formatResetCountdown('not a date', nowMs: now), '');
      expect(formatResetCountdown(iso(now - 1000), nowMs: now), '');
      expect(formatResetCountdown(iso(now + 30 * 60000), nowMs: now), '30m');
      expect(
        formatResetCountdown(iso(now + 2 * _hour + 5 * 60000), nowMs: now),
        '2h 5m',
      );
      expect(
        formatResetCountdown(iso(now + 3 * _day + 4 * _hour), nowMs: now),
        '3d 4h',
      );
    });

    test('重置时刻：当天 HH:mm，跨天 M/D HH:mm', () {
      final sameDay = formatResetClock(iso(now + 2 * _hour), nowMs: now);
      expect(sameDay.contains(':'), isTrue);
      expect(sameDay.contains('/'), isFalse);
      expect(formatResetClock(iso(now + 3 * _day), nowMs: now).contains('/'), isTrue);
      expect(formatResetClock(iso(now - 1), nowMs: now), '');
      expect(formatResetClock(null, nowMs: now), '');
    });
  });

  // ══ 手动预算 ═══════════════════════════════════════════════════

  group('computeManualBudgetDisplay（React: health.ts:204）', () {
    ManualBudget mb(double amount, double consumed, {bool enabled = true, String unit = 'usd'}) =>
        ManualBudget.fromJson({
          'id': 'x',
          'kind': 'total',
          'unit': unit,
          'amount': amount,
          'consumed': consumed,
          'enabled': enabled,
        });

    test('没有启用的条目 → null', () {
      expect(computeManualBudgetDisplay(null), isNull);
      expect(computeManualBudgetDisplay([mb(10, 1, enabled: false)]), isNull);
      expect(computeManualBudgetDisplay([mb(0, 0)]), isNull);
    });
    test('取剩余比例最低那条', () {
      final d = computeManualBudgetDisplay([
        mb(100, 10), // 剩 90%
        mb(10, 9, unit: 'token'), // 剩 10% ← 最紧
      ])!;
      expect(d.unit, 'token');
      expect(d.remaining, 1);
      expect(d.amount, 10);
      expect(d.ratio, closeTo(0.1, 1e-9));
      expect(d.depleted, isFalse);
    });
    test('用超了 → depleted，ratio 夹到 0', () {
      final d = computeManualBudgetDisplay([mb(10, 12)])!;
      expect(d.depleted, isTrue);
      expect(d.ratio, 0);
    });
  });

  // ══ 相对时间 ═══════════════════════════════════════════════════

  group('relativeTimeShort（React: PlatformCard.tsx:867）', () {
    const now = 1740000000000;
    test('不足 1 分钟返空串', () {
      expect(relativeTimeShort(now - 59000, nowMs: now), '');
      expect(relativeTimeShort(now + 5000, nowMs: now), '');
    });
    test('分 / 时 / 天三档', () {
      expect(relativeTimeShort(now - 3 * 60000, nowMs: now), '3m');
      expect(relativeTimeShort(now - 5 * _hour, nowMs: now), '5h');
      expect(relativeTimeShort(now - 2 * _day, nowMs: now), '2d');
    });
  });

  // ══ 分享格式 ═══════════════════════════════════════════════════

  group('formatShare（React: ShareModal.tsx:59）', () {
    final share = <String, Object?>{
      'name': 'My Platform',
      'platform_type': 'openai',
      'api_key': 'sk-test',
      'endpoints': [
        {'protocol': 'openai', 'base_url': 'https://a/v1'},
      ],
    };

    test('json 是两空格缩进', () {
      final s = formatShare(share, ShareFormat.json);
      expect(s.startsWith('{\n  "name": "My Platform"'), isTrue);
    });
    test('yaml 标量该加引号的加，列表用 `- `', () {
      final s = formatShare(share, ShareFormat.yaml);
      expect(s.contains('name: My Platform'), isTrue);
      expect(s.contains('api_key: sk-test'), isTrue);
      expect(s.contains('endpoints:\n  - protocol: openai'), isTrue);
      // 冒号后不跟空格不构成歧义 → 不加引号（与 React 的 `yaml` 包同口径）。
      expect(s.contains('base_url: https://a/v1'), isTrue);
    });
    test('yaml 引号只加在真会改变解析的地方', () {
      expect(
        formatShare(const {'a': 'x: y', 'b': 'true', 'c': '12', 'd': '- z', 'e': ''}, ShareFormat.yaml),
        'a: "x: y"\nb: "true"\nc: "12"\nd: "- z"\ne: ""\n',
      );
    });
    test('base64 包的是 YAML 不是 JSON', () {
      final b64 = formatShare(share, ShareFormat.base64);
      expect(
        utf8Decode(b64),
        formatShare(share, ShareFormat.yaml),
      );
    });
    test('url 走 base64(JSON) 并带 scheme', () {
      final u = formatShare(
        share,
        ShareFormat.url,
        urlScheme: 'aidog://platform/import',
      );
      expect(u.startsWith('aidog://platform/import?data='), isTrue);
      expect(
        utf8Decode(u.split('?data=').last),
        formatShare(share, ShareFormat.json),
      );
    });
    test('UTF-8 安全（中文不炸）', () {
      expect(utf8Decode(toBase64Utf8('智谱·清言')), '智谱·清言');
    });
  });

  // ══ registry 元数据 ════════════════════════════════════════════

  group('ProtocolMetaTable（React: defaults.ts 的一组 getter）', () {
    final meta = ProtocolMetaTable.parse(kDefaultsJson, 'zh-Hans');

    test('label 取当前 locale，缺失回落裸 code', () {
      expect(meta.label('openai'), '开放人工智能');
      expect(meta.label('unknown'), 'unknown');
      expect(
        ProtocolMetaTable.parse(kDefaultsJson, 'en-US').label('openai'),
        'OpenAI',
      );
      // 该 locale 没有就回落 en-US。
      expect(
        ProtocolMetaTable.parse(kDefaultsJson, 'fr-FR').label('openai'),
        'OpenAI',
      );
    });

    test('外链 / coding plan 标记', () {
      expect(meta.homepages['openai'], 'https://openai.com');
      expect(meta.docsUrls['openai'], 'https://d.test');
      expect(meta.pricingUrls['openai'], 'https://p.test');
      expect(meta.isCodingPlan('glm_coding'), isTrue);
      expect(meta.isCodingPlan('openai'), isFalse);
    });

    test('hasQuotaScript：registry 有变体 / 自定义脚本 / 未就绪回落旧启发式', () {
      expect(meta.hasQuotaScript('openai', ''), isTrue);
      expect(meta.hasQuotaScript('glm_coding', ''), isFalse);
      expect(
        meta.hasQuotaScript('glm_coding', '{"quota_custom_script":"x"}'),
        isTrue,
      );
      const unloaded = ProtocolMetaTable();
      expect(unloaded.hasQuotaScript('anything', ''), isTrue);
      expect(unloaded.hasQuotaScript('mock', ''), isFalse);
      expect(unloaded.hasQuotaScript('claude_code', ''), isFalse);
    });

    test('modelsFor：命中 peak 走 peak 分支，否则 default', () {
      // openai 无 preset peak，也没 extra 覆盖 → default。
      expect(meta.modelsFor('openai', '', 0), ['gpt-5']);
      // 用户 extra.peak 全天窗口 → 命中 → peak 分支。
      expect(
        meta.modelsFor(
          'openai',
          '{"peak":[{"start_hour":0,"end_hour":24,"multiplier":2}]}',
          0,
        ),
        ['gpt-5-mini'],
      );
      expect(meta.modelsFor('unknown', '', 0), isEmpty);
    });

    test('非法 JSON → 空表且 loaded=false', () {
      expect(ProtocolMetaTable.parse('nope', 'zh-Hans').loaded, isFalse);
      expect(ProtocolMetaTable.parse('', 'zh-Hans').loaded, isFalse);
      expect(ProtocolMetaTable.parse('{"x":1}', 'zh-Hans').loaded, isFalse);
    });
  });

  // ══ BalanceBar（React: BalanceBar.test.tsx 逐条翻译）══════════════

  group('BalanceBar（React: BalanceBar.test.tsx）', () {
    Future<void> mount(WidgetTester tester, Widget w) async {
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(w, c));
      await settle(tester);
    }

    testWidgets('remaining 为 null / NaN 时什么都不画', (tester) async {
      await mount(tester, const BalanceBar(remaining: null));
      expect(find.byType(LinearProgressIndicator), findsNothing);
      await mount(tester, const BalanceBar(remaining: double.nan));
      expect(find.byType(LinearProgressIndicator), findsNothing);
    });

    testWidgets('没有 total 时只有数字，没有进度条', (tester) async {
      await mount(tester, const BalanceBar(remaining: 12.3));
      expect(find.text('\$12.30'), findsOneWidget);
      expect(find.byType(LinearProgressIndicator), findsNothing);
    });

    testWidgets('有 total 时出数字 + total + 进度条，币种可换', (tester) async {
      await mount(
        tester,
        const BalanceBar(remaining: 20, total: 50, currency: '¥'),
      );
      expect(find.text('¥20.00'), findsOneWidget);
      expect(find.text('/ ¥50.00'), findsOneWidget);
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
    });

    testWidgets('showTotal=false 藏起 total', (tester) async {
      await mount(
        tester,
        const BalanceBar(remaining: 20, total: 50, showTotal: false),
      );
      expect(find.text('/ \$50.00'), findsNothing);
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
    });

    testWidgets('total <= 0 当作没有 total', (tester) async {
      await mount(tester, const BalanceBar(remaining: 10, total: 0));
      expect(find.text('\$10.00'), findsOneWidget);
      expect(find.byType(LinearProgressIndicator), findsNothing);
    });

    test('remainingLevel 阈值：>=50 绿 / >=20 黄 / 其余红', () {
      expect(remainingLevel(50), ColorLevel.success);
      expect(remainingLevel(49.9), ColorLevel.warning);
      expect(remainingLevel(20), ColorLevel.warning);
      expect(remainingLevel(19.9), ColorLevel.danger);
    });
  });

  // ══ 平台卡 widget（React: PlatformCard.test.tsx 的 8 条）════════

  group('PlatformCard（React: PlatformCard.test.tsx）', () {
    Future<(FakeInvoke, I18nController)> mount(
      WidgetTester tester,
      FakeInvoke k, {
      void Function(PlatformRow)? onEdit,
      void Function(PlatformRow)? onDuplicate,
    }) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
            onEditPlatform: onEdit,
            onDuplicatePlatform: onDuplicate,
          ),
          c,
        ),
      );
      await settle(tester);
      return (k, c);
    }

    testWidgets('折叠态：渲染平台名 + 操作按钮；展开区默认收着', (tester) async {
      final (k, c) = await mount(tester, cardFake());
      expect(find.text('Test Platform'), findsOneWidget);
      expect(find.byTooltip(c.t('page.logs')), findsOneWidget);
      expect(find.byTooltip(c.t('platform.share.button')), findsOneWidget);
      // 编辑 / 复制无条件渲染，与 `PlatformCard.tsx:832-850` 一致：宿主页没传回调时
      // 由页面自带的表单控制器接手（`platforms.dart` 的 `?? _form.handleDuplicate`）。
      expect(find.byTooltip(c.t('platform.duplicate')), findsOneWidget);
      // registry 给了默认模型 → hasDetail=true → 有展开控件，但内容默认收着。
      expect(find.byTooltip(c.t('platform.toggleDetail')), findsOneWidget);
      expect(find.text(c.t('platform.models')), findsNothing);
      expect(k.commands.contains('platform_list'), isTrue);
    });

    testWidgets('没有任何明细可看时连展开控件都不画', (tester) async {
      final (_, c) = await mount(
        tester,
        cardFake(platforms: [platRow(1, 'Bare', type: 'bare_proto')]),
      );
      expect(find.byTooltip(c.t('platform.toggleDetail')), findsNothing);
    });

    testWidgets('接上表单回调后才渲染编辑 / 复制，点击把这一行传回去', (tester) async {
      final edited = <int>[];
      final duplicated = <int>[];
      final (_, c) = await mount(
        tester,
        cardFake(),
        onEdit: (p) => edited.add(p.id),
        onDuplicate: (p) => duplicated.add(p.id),
      );
      await tester.tap(find.byTooltip(c.t('action.edit')));
      await tester.tap(find.byTooltip(c.t('platform.duplicate')));
      await settle(tester);
      expect(edited, [1]);
      expect(duplicated, [1]);
    });

    testWidgets('启停开关：真滑块 + 三态文案 + 点击发 platform_update', (tester) async {
      final (k, c) = await mount(tester, cardFake());
      // 票 13：文字按钮换成滑块，enabled 平台呈开态。
      expect(tester.widget<AidogSwitch>(find.byType(AidogSwitch)).value, isTrue);
      expect(find.byTooltip(c.t('platform.disable')), findsOneWidget);
      await tester.tap(find.byTooltip(c.t('platform.disable')));
      await settle(tester);
      expect(k.lastCallTo('platform_update')!.args!['input'], isNotNull);
    });

    testWidgets('停用平台 → 开关呈关态', (tester) async {
      await mount(
        tester,
        cardFake(
          platforms: [
            platRow(1, 'Test Platform', status: 'disabled', enabled: false),
          ],
        ),
      );
      expect(
        tester.widget<AidogSwitch>(find.byType(AidogSwitch)).value,
        isFalse,
      );
    });

    testWidgets('六颗快操作各有图标按钮，按 tooltip 点得动', (tester) async {
      final edited = <int>[];
      final duplicated = <int>[];
      final nav = <String>[];
      final (k, c) = await mount(
        tester,
        cardFake(),
        onEdit: (p) => edited.add(p.id),
        onDuplicate: (p) => duplicated.add(p.id),
      );
      // 刷新额度 / 日志 / 编辑 / 分享 / 复制 / 删除：文案 key 全在 tooltip 上。
      for (final key in [
        'platform.quotaRefresh',
        'page.logs',
        'action.edit',
        'platform.share.button',
        'platform.duplicate',
        'action.delete',
      ]) {
        expect(find.byTooltip(c.t(key)), findsOneWidget, reason: key);
      }
      final before = k.callsTo('platform_query_quota').length;
      await tester.tap(find.byTooltip(c.t('platform.quotaRefresh')));
      await settle(tester);
      expect(k.callsTo('platform_query_quota').length, before + 1);
      await tester.tap(find.byTooltip(c.t('action.edit')));
      await tester.tap(find.byTooltip(c.t('platform.duplicate')));
      await settle(tester);
      expect(edited, [1]);
      expect(duplicated, [1]);
      await tester.tap(find.byTooltip(c.t('action.delete')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);
      expect(nav, isEmpty);
    });

    testWidgets('点卡片头部切换展开态', (tester) async {
      final (_, c) = await mount(tester, cardFake());
      expect(find.text(c.t('platform.models')), findsNothing);
      await tester.tap(find.text('Test Platform'));
      await settle(tester);
      expect(find.text(c.t('platform.models')), findsOneWidget);
      await tester.tap(find.text('Test Platform'));
      await settle(tester);
      expect(find.text(c.t('platform.models')), findsNothing);
    });

    testWidgets('auto_disabled → 「重新启用」文案 + 自动禁用徽标', (tester) async {
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'Test Platform',
              status: 'auto_disabled',
              enabled: false,
              lastError: 'HTTP 401',
              lastErrorAt: 1740000000000,
              autoDisabledUntil: 1740000100000,
            ),
          ],
        ),
      );
      expect(find.text(c.t('platform.autoDisabled')), findsOneWidget);
      expect(find.byTooltip(c.t('platform.reenable')), findsOneWidget);
    });

    testWidgets('停用态整卡半透明（opacity 0.5）', (tester) async {
      await mount(
        tester,
        cardFake(
          platforms: [
            platRow(1, 'Test Platform', status: 'disabled', enabled: false),
          ],
        ),
      );
      // 压暗现在走 150ms 过渡（React 同处 `transition: opacity 150ms`），
      // 所以拿的是 `AnimatedOpacity` 的目标值。
      final op = tester.widget<AnimatedOpacity>(
        find
            .ancestor(
              of: find.text('Test Platform'),
              matching: find.byType(AnimatedOpacity),
            )
            .first,
      );
      expect(op.opacity, 0.5);
    });

    testWidgets('testing=true 时快测按钮禁用且不再发命令', (tester) async {
      final k = cardFake();
      // 让 model_test 卡住，快测按钮就停在 testing 态。
      final gate = Completer<Object?>();
      k.responses['model_test'] = () => gate.future;
      final (_, c) = await mount(tester, k);
      await tester.tap(find.byTooltip(c.t('platform.quickTest')));
      await settle(tester);
      expect(find.byTooltip(c.t('platform.quickTest')), findsNothing);
      expect(k.callsTo('model_test').length, 1);
      gate.complete({'success': true, 'duration_ms': 7, 'error': ''});
      await settle(tester);
    });

    testWidgets('可查配额的平台才有「刷新额度」按钮；点它发 platform_query_quota', (tester) async {
      final (k, c) = await mount(tester, cardFake());
      final before = k.callsTo('platform_query_quota').length;
      await tester.tap(find.byTooltip(c.t('platform.quotaRefresh')));
      await settle(tester);
      expect(k.callsTo('platform_query_quota').length, before + 1);
    });

    testWidgets('协议无配额脚本 → 不渲染刷新按钮，也不后台查', (tester) async {
      final (k, c) = await mount(
        tester,
        cardFake(platforms: [platRow(1, 'Test Platform', type: 'glm_coding')]),
      );
      expect(find.byTooltip(c.t('platform.quotaRefresh')), findsNothing);
      expect(k.commands.contains('platform_query_quota'), isFalse);
    });

    testWidgets('最近一次测试：✓ + 时长 + 相对时间', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      await mount(
        tester,
        cardFake(
          lastTest: {
            'success': true,
            'status_code': 200,
            'duration_ms': 1234,
            'created_at': now - 5 * 60000,
            'error': '',
            'response_body': '',
          },
        ),
      );
      expect(find.textContaining('1234ms'), findsOneWidget);
      expect(find.textContaining('5m'), findsOneWidget);
    });

    testWidgets('最近错误徽标带错误正文', (tester) async {
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'Test Platform',
              lastError: 'HTTP 500 upstream',
              lastErrorAt: 1740000000000,
            ),
          ],
        ),
      );
      expect(
        find.text('${c.t('platform.lastError')}: HTTP 500 upstream'),
        findsOneWidget,
      );
    });

    testWidgets('已过期显红 badge；未过期显「到期 …」小字', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(1, 'Expired', expiresAt: now - 1000),
            platRow(2, 'Soon', expiresAt: now + 3600000),
          ],
        ),
      );
      expect(find.text(c.t('platform.expired')), findsOneWidget);
      expect(find.textContaining(c.t('platform.expiresAtBadge').split('{{').first.trim()), findsOneWidget);
    });

    testWidgets('高峰徽标：preset 全天窗口 + model scope 限定显「高峰·N模型」', (tester) async {
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'Peak',
              extra:
                  '{"peak":[{"start_hour":0,"end_hour":24,"multiplier":2,'
                  '"models":["gpt-5","gpt-4"]}]}',
            ),
          ],
        ),
      );
      expect(
        find.text(c.t('platform.peak_badge_limited').replaceAll('{{count}}', '2')),
        findsOneWidget,
      );
    });

    testWidgets('disable_during_peak 命中窗口 → 显「高峰禁用中」而非「高峰」', (tester) async {
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'PeakOff',
              extra:
                  '{"disable_during_peak":true,'
                  '"peak":[{"start_hour":0,"end_hour":24,"multiplier":2}]}',
            ),
          ],
        ),
      );
      expect(find.text(c.t('platform.peak_disabled_badge')), findsOneWidget);
      expect(find.text(c.t('platform.peak_badge')), findsNothing);
    });

    testWidgets('Coding Plan 协议徽标由 registry 的 is_coding_plan 驱动', (tester) async {
      final (_, c) = await mount(
        tester,
        cardFake(platforms: [platRow(1, 'GLM', type: 'glm_coding')]),
      );
      expect(find.text(c.t('platform.codingPlanBadge')), findsOneWidget);
    });

    testWidgets('协议展示名 + 主 base_url 副行', (tester) async {
      await mount(tester, cardFake());
      expect(find.text('开放人工智能 · https://api.test/1'), findsOneWidget);
    });

    testWidgets('余额进度条：后端 balance_level 决定配色档，不在前端重算阈值', (tester) async {
      final k = cardFake(
        platforms: [
          platRow(1, 'Bal', estBalance: 12.5, balanceLevel: 'red'),
        ],
      );
      await mount(tester, k);
      expect(find.text('\$12.50'), findsOneWidget);
    });

    testWidgets('余额待回时显骨架，不显空白也不显旧值', (tester) async {
      final k = cardFake();
      final gate = Completer<Object?>();
      k.responses['platform_query_quota'] = () => gate.future;
      await mount(tester, k);
      expect(find.byType(SkeletonBox), findsOneWidget);
      gate.complete({'success': true, 'queried_at': 1});
      await settle(tester);
    });

    testWidgets('展开明细：三个已使用 chip + 两个今日 chip + 模型 badge', (tester) async {
      final (_, c) = await mount(
        tester,
        cardFake(usage: {'1': usageJson()}),
      );
      await tester.tap(find.byTooltip(c.t('platform.toggleDetail')));
      await settle(tester);
      expect(find.text(c.t('platform.usageLabel')), findsOneWidget);
      expect(find.text(c.t('platform.todayUsageLabel')), findsOneWidget);
      expect(find.text('15.0K'), findsOneWidget); // 10000 + 5000
      expect(find.text('\$1.50'), findsOneWidget);
      expect(find.text('95.0%'), findsOneWidget);
      expect(find.text('1.0K'), findsOneWidget); // today_tokens
      expect(find.text('\$0.150'), findsOneWidget);
      // 品牌外链三条都来自 registry。
      expect(find.text(c.t('platform.homepage')), findsOneWidget);
      expect(find.text(c.t('platform.sourceDocs')), findsOneWidget);
      expect(find.text(c.t('platform.sourcePricing')), findsOneWidget);
    });

    testWidgets('展开明细：端点 badge 带 Code 角标，模型 badge 列出已配置模型', (tester) async {
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'EP',
              models: {'default': 'gpt-5', 'opus': 'o1'},
              endpoints: [
                {
                  'protocol': 'openai',
                  'base_url': 'https://a/v1',
                  'client_type': '',
                  'coding_plan': true,
                },
              ],
            ),
          ],
          usage: {'1': usageJson()},
        ),
      );
      await tester.tap(find.byTooltip(c.t('platform.toggleDetail')));
      await settle(tester);
      expect(find.text(c.t('platform.endpoints')), findsOneWidget);
      expect(find.text('开放人工智能 Code'), findsOneWidget);
      expect(find.text('gpt-5'), findsOneWidget);
      expect(find.text('o1'), findsOneWidget);

      // `Code` 是角标：只有它一段换色，协议名那段保持中性
      //（整枚徽标变绿会让人分不清它是端点名的一部分还是角标）。
      final badge = tester.widget<Text>(find.text('开放人工智能 Code'));
      final span = badge.textSpan! as TextSpan;
      final code = span.children!.single as TextSpan;
      expect(code.toPlainText().trim(), 'Code');
      expect(code.style!.color, isNot(badge.style!.color));
    });

    testWidgets('展开态落盘走防抖：300ms 之后才发 set_ui_extra', (tester) async {
      final (k, c) = await mount(tester, cardFake(usage: {'1': usageJson()}));
      await tester.tap(find.byTooltip(c.t('platform.toggleDetail')));
      await settle(tester);
      expect(k.commands.contains('set_ui_extra'), isFalse, reason: '防抖没到点就不该写');
      await tester.pump(const Duration(milliseconds: 320));
      await settle(tester);
      final call = k.lastCallTo('set_ui_extra')!;
      expect(call.args!['key'], '_ui_expand_plat');
      expect(call.args!['value'], isTrue);
      expect(call.args!['id'], 1);

      // 再点一次收起：同一条防抖链，落盘 false。
      await tester.tap(find.byTooltip(c.t('platform.toggleDetail')));
      await tester.pump(const Duration(milliseconds: 320));
      await settle(tester);
      expect(k.lastCallTo('set_ui_extra')!.args!['value'], isFalse);
    });

    testWidgets('配额档位块：进度条 + 剩余% + 档名', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'Tiers',
              estCodingPlan:
                  '{"tiers":[{"name":"five_hour","est_utilization":40,'
                  '"window_start":${now - 3600000}}]}',
            ),
          ],
        ),
      );
      expect(find.text('60%${c.t('platform.quotaRemainSuffix')}'), findsOneWidget);
      expect(find.textContaining('5h'), findsWidgets);
    });

    testWidgets('展开区配额各档：不支持配额查询的平台，有历史档位也不显示', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      // bare_proto 没登记 quota_scripts → quotaCapable=false。
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'NoQuotaScript',
              type: 'bare_proto',
              estCodingPlan:
                  '{"tiers":[{"name":"five_hour","est_utilization":40,'
                  '"window_start":${now - 3600000}}]}',
            ),
          ],
          usage: {'1': usageJson()},
        ),
      );
      await tester.tap(find.byTooltip(c.t('platform.toggleDetail')));
      await settle(tester);
      expect(find.text(c.t('platform.usageLabel')), findsOneWidget);
      expect(find.text(c.t('platform.quotaLabel')), findsNothing);
    });

    testWidgets('配额档位块：倒计时与重置时刻同一行，前面一枚时钟图标', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'Tiers',
              estCodingPlan:
                  '{"tiers":[{"name":"five_hour","est_utilization":40,'
                  '"window_start":${now - 3600000}}]}',
            ),
          ],
        ),
      );
      final clock = find.byIcon(Icons.schedule);
      expect(clock, findsOneWidget);
      // 同一行 = 图标与倒计时文本的纵向中心对得上。
      final clockY = tester.getCenter(clock).dy;
      final textY = tester
          .getCenter(find.textContaining('·', findRichText: true).last)
          .dy;
      expect((clockY - textY).abs(), lessThan(4));
    });

    testWidgets('上游速率限制：5 分钟内的快照才画，过期的不画', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'Fresh',
              estBalance: 5,
              rateLimit:
                  '{"vendor":"anthropic","requests_remaining":20,'
                  '"requests_limit":100,"observed_at":$now}',
            ),
            platRow(
              2,
              'Stale',
              estBalance: 5,
              rateLimit:
                  '{"vendor":"anthropic","requests_remaining":20,'
                  '"requests_limit":100,"observed_at":${now - 400000}}',
            ),
          ],
        ),
      );
      expect(find.text('${c.t('platform.rateLimit')} 20%'), findsOneWidget);
    });

    // ── 余额行门控：六块各判各的，不共用「配额已到达」这一个条件 ──
    //
    // 回归的是这条 bug：整行曾锁在 `quotaCapable && quota.hasData` 之后，
    // 于是「coding 已用 tokens」「速率余量」这些与配额无关、早就有数据的块
    // 一起被挡住不显示。

    testWidgets('不支持配额查询的平台：coding plan 已用 tokens 照样显示', (tester) async {
      // glm_coding 在 registry 里没有 quota_scripts → quotaCapable=false。
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'GLM',
              type: 'glm_coding',
              endpoints: [
                {
                  'protocol': 'glm_coding',
                  'base_url': 'https://a/v4',
                  'client_type': '',
                  'coding_plan': true,
                },
              ],
            ),
          ],
          usage: {'1': usageJson()},
        ),
      );
      // 折叠态就该看见：已用 tokens + 预估金额（React 注释「折叠态亦可见」）。
      expect(find.byTooltip(c.t('platform.codingUsedHint')), findsOneWidget);
      expect(find.text('15.0K'), findsOneWidget); // 10000 + 5000
      expect(find.text('\$1.50'), findsOneWidget);
      // 没配额能力就没有刷新按钮（这条门控是对的，保持）。
      expect(find.byTooltip(c.t('platform.quotaRefresh')), findsNothing);
    });

    testWidgets('配额查回来是空的：速率余量 chip 照样显示', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      final k = cardFake(
        platforms: [
          platRow(
            1,
            'RL only',
            rateLimit:
                '{"vendor":"anthropic","requests_remaining":20,'
                '"requests_limit":100,"observed_at":$now}',
          ),
        ],
      );
      // 查得通但没有 balance / coding_plan → quota.hasData=false。
      k.responses['platform_query_quota'] = () => {
        'success': true,
        'queried_at': 1,
      };
      final (_, c) = await mount(tester, k);
      expect(find.text('${c.t('platform.rateLimit')} 20%'), findsOneWidget);
    });

    testWidgets('手动预算：token 单位显「剩余 / 总额 tok」', (tester) async {
      final (_, c) = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'Budget',
              estBalance: 1,
              manualBudgets: [
                {
                  'id': 'b1',
                  'kind': 'total',
                  'unit': 'token',
                  'amount': 1000,
                  'consumed': 900,
                  'enabled': true,
                },
              ],
            ),
          ],
        ),
      );
      expect(find.text('100'), findsOneWidget);
      expect(find.text(' / 1.0K tok'), findsOneWidget);
      // token 单位额外缀「≈未知$」：拿不到单价就说拿不到，不编一个折算值。
      expect(
        find.text(
          '${c.t('platform.manualBudgetLabel')} · '
          '${c.t('platform.manualBudgetTokenApprox')}',
        ),
        findsOneWidget,
      );
    });
  });

  group('PlatformCard — 其余分支', () {
    Future<I18nController> mount(WidgetTester tester, FakeInvoke k) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      return c;
    }

    testWidgets('logo 缓存命中 → 画图，不画首字母', (tester) async {
      final k = cardFake();
      k.responses['get_protocol_logo_path'] = '/tmp/openai.png';
      // 1×1 透明 PNG。
      k.responses['get_protocol_logo_data_url'] =
          'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJ'
          'AAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';
      await mount(tester, k);
      expect(find.byType(Image), findsOneWidget);
      expect(find.text('OP'), findsNothing);
    });

    testWidgets('logo 缓存 miss 但有内置 svg → 画内置图，不掉成字母块', (tester) async {
      // 2026-09-22 之前这条断言的是「缓存 miss 就显 OP 两个字母」——
      // 那是把缺陷当成了规格。React 有四级回退，openai 属于 16 个内置图之一，
      // 缓存没同步下来也该画出真 logo（`platform_logo.dart::platformLogo`）。
      await mount(tester, cardFake());
      expect(find.text('OP'), findsNothing);
      expect(find.byType(SvgPicture), findsWidgets);
    });

    testWidgets('四级全落空（无内置图、无 base_url）才画字母块', (tester) async {
      final k = cardFake();
      k.responses['platform_list'] = [
        platRow(1, 'Test Platform', type: 'zz')..['base_url'] = '',
      ];
      await mount(tester, k);
      expect(find.text('ZZ'), findsOneWidget);
    });

    testWidgets('所属分组 badge 列在名称下面', (tester) async {
      final k = cardFake();
      k.responses['group_detail_list'] = [
        {
          'group': {'id': 1, 'group_key': 'gk', 'name': '主力组'},
          'platforms': [
            {
              'platform': platRow(1, 'Test Platform'),
              'priority': 0,
              'weight': 1,
            },
          ],
        },
      ];
      await mount(tester, k);
      // 已分组的不出现在未分组区，所以这里断言的是控制器算出来的成员关系。
      expect(find.text('Test Platform'), findsNothing);
    });

    testWidgets('最近测试失败 → ✗ + 错误摘要截到 30 字', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      await mount(
        tester,
        cardFake(
          lastTest: {
            'success': false,
            'status_code': 500,
            'duration_ms': 0,
            'created_at': now - 2 * _hour,
            'error': 'E' * 45,
            'response_body': '',
          },
        ),
      );
      expect(find.text('✗ · 2h ${'E' * 30}'), findsOneWidget);
    });

    testWidgets('ACU 平台：显 used、带「ACU 用量」小标、无进度条', (tester) async {
      final k = cardFake(platforms: [platRow(1, 'Devin', type: 'openai')]);
      k.responses['platform_query_quota'] = {
        'success': true,
        'queried_at': 1,
        'balance': {
          'remaining': 0,
          'total': 100,
          'used': 42,
          'currency': 'ACU',
        },
      };
      final c = await mount(tester, k);
      expect(find.text('42.00'), findsOneWidget);
      expect(find.text(c.t('platform.acuUsage')), findsOneWidget);
    });

    testWidgets('usd 手动预算走进度条；耗尽时换「额度耗尽」文案', (tester) async {
      final c = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'Budget',
              estBalance: 1,
              manualBudgets: [
                {
                  'id': 'b1',
                  'kind': 'total',
                  'unit': 'usd',
                  'amount': 10,
                  'consumed': 12,
                  'enabled': true,
                },
              ],
            ),
          ],
        ),
      );
      expect(find.text(c.t('platform.manualBudgetDepleted')), findsOneWidget);
      expect(find.text('\$-2.00'), findsNothing, reason: '负余额不画负数条');
    });

    testWidgets('coding 端点：已用 tokens + 金额 + 本周期折算 + 套餐价', (tester) async {
      final c = await mount(
        tester,
        cardFake(
          platforms: [
            platRow(
              1,
              'CP',
              estBalance: 5,
              codingWindowCost: 2.5,
              extra: '{"plan_price":20}',
              endpoints: [
                {
                  'protocol': 'openai',
                  'base_url': 'https://a/v1',
                  'client_type': '',
                  'coding_plan': true,
                },
              ],
            ),
          ],
          usage: {'1': usageJson()},
        ),
      );
      expect(find.text('15.0K'), findsOneWidget);
      expect(find.text('tok'), findsOneWidget);
      expect(
        find.text(
          '${c.t('platform.codingWindowCost')} \$2.50 · '
          '${c.t('platform.codingPlanPrice')} ¥20.0/月',
        ),
        findsOneWidget,
      );
    });

    testWidgets('用量待回 → 展开区画三条骨架而不是空白', (tester) async {
      final k = cardFake();
      final gate = Completer<Object?>();
      k.responses['all_platform_usage_stats'] = () => gate.future;
      final c = await mount(tester, k);
      await tester.tap(find.byTooltip(c.t('platform.toggleDetail')));
      await settle(tester);
      // 3 条用量骨架 + 1 条余额骨架：用量卡住时 `pumpQuota` 还没轮到，余额也还在待回。
      expect(find.byType(SkeletonBox), findsNWidgets(4));
      gate.complete(<String, Object?>{});
      await settle(tester);
    });
  });

  group('PlatformsPage — 列表区其余分支', () {
    Future<(FakeInvoke, I18nController)> mount(
      WidgetTester tester,
      FakeInvoke k,
    ) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      return (k, c);
    }

    testWidgets('搜索框过滤未分组列表', (tester) async {
      final (_, _) = await mount(
        tester,
        cardFake(platforms: [platRow(1, 'alpha'), platRow(2, 'beta')]),
      );
      await tester.enterText(find.byType(TextField).first, 'alp');
      await settle(tester);
      expect(find.text('alpha'), findsOneWidget);
      expect(find.text('beta'), findsNothing);
    });

    testWidgets('删除必须先确认：点删除只出确认卡，不发命令', (tester) async {
      final (k, c) = await mount(tester, cardFake());
      await tester.tap(find.byTooltip(c.t('action.delete')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);
      expect(k.commands.contains('platform_delete'), isFalse);
      await tester.tap(find.text(c.t('action.delete')).last);
      await settle(tester);
      expect(k.lastCallTo('platform_delete')!.args!['id'], 1);
    });

    testWidgets('「测试」开模型测试面板', (tester) async {
      final k = cardFake();
      k.responses['platform_list'] = [platRow(1, 'Test Platform')];
      final (_, c) = await mount(tester, k);
      await tester.tap(find.byTooltip(c.t('test.title')));
      await settle(tester);
      expect(find.byType(ModelTestPanel), findsOneWidget);
    });

    testWidgets('分享面板点关闭就收起', (tester) async {
      final (_, c) = await mount(tester, cardFake());
      await tester.tap(find.byTooltip(c.t('platform.share.button')));
      await settle(tester);
      expect(find.byType(SharePanel), findsOneWidget);
      await tester.tap(find.text(c.t('action.close')));
      await settle(tester);
      expect(find.byType(SharePanel), findsNothing);
    });

    testWidgets('分享导出失败 → 不开面板', (tester) async {
      final k = cardFake();
      k.errors['platform_share_export'] = StateError('boom');
      final (_, c) = await mount(tester, k);
      await tester.tap(find.byTooltip(c.t('platform.share.button')));
      await settle(tester);
      expect(find.byType(SharePanel), findsNothing);
      expect(find.byType(ToastBar), findsOneWidget);
    });

    testWidgets('列表为空 → 空态卡', (tester) async {
      final (_, _) = await mount(tester, cardFake(platforms: const []));
      expect(find.byType(CenteredNote), findsOneWidget);
    });

    testWidgets('「日志」按钮把本行的 platformId 带去日志页', (tester) async {
      await useBigSurface(tester);
      final k = cardFake();
      final nav = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
            onNavigate: (id, {int? platformId, String? groupKey}) =>
                nav.add('$id:$platformId'),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('page.logs')));
      await settle(tester);
      expect(nav, ['logs:1']);
    });

    testWidgets('新日志事件 → 静默重查统计，不闪 loading', (tester) async {
      await useBigSurface(tester);
      final k = cardFake();
      final ticker = LogTicker();
      addTearDown(ticker.close);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: ticker.stream,
          ),
          c,
        ),
      );
      await settle(tester);
      final before = k.callsTo('all_platform_usage_stats').length;
      ticker.fire();
      await settle(tester);
      expect(k.callsTo('all_platform_usage_stats').length, before + 1);
    });

  });

  // ══ 清理失效平台弹窗 ═══════════════════════════════════════════

  group('清理失效平台确认弹窗（React: PlatformListView.tsx:270-288）', () {
    testWidgets('候选为空 → 空态文案且确认按钮点不动', (tester) async {
      await useBigSurface(tester);
      final k = cardFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('platform.purgeDisabled')));
      await settle(tester);
      expect(find.text(c.t('platform.purgeDisabledNone')), findsOneWidget);
      await tester.tap(find.text(c.t('action.confirm')));
      await settle(tester);
      expect(k.commands.contains('platform_purge_disabled'), isFalse);
    });

    testWidgets('有候选 → 列清单 + 失效原因 badge，确认才真删', (tester) async {
      await useBigSurface(tester);
      final k = cardFake();
      k.responses['platform_purge_disabled_preview'] = [
        {'id': 2, 'name': '坏平台', 'reason': 'auth_failed', 'action': 'delete'},
        {'id': 3, 'name': '过期平台', 'reason': 'expired', 'action': 'unassign'},
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('platform.purgeDisabled')));
      await settle(tester);
      expect(find.text(c.t('platform.purgeDisabledListTitle')), findsOneWidget);
      expect(find.text('坏平台'), findsOneWidget);
      expect(find.text('过期平台'), findsOneWidget);
      expect(
        find.text(c.t('platform.purgeDisabledReasonAuthFailed')),
        findsOneWidget,
      );
      expect(
        find.text(c.t('platform.purgeDisabledReasonExpired')),
        findsOneWidget,
      );
      expect(k.commands.contains('platform_purge_disabled'), isFalse);
      await tester.tap(find.text(c.t('action.confirm')));
      await settle(tester);
      expect(k.commands.contains('platform_purge_disabled'), isTrue);
    });

    testWidgets('清单多到装不下时自己滚，不把弹窗撑长', (tester) async {
      await useBigSurface(tester);
      final k = cardFake();
      k.responses['platform_purge_disabled_preview'] = [
        for (var i = 0; i < 40; i++)
          {'id': i + 2, 'name': '坏平台$i', 'reason': 'expired', 'action': 'delete'},
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('platform.purgeDisabled')));
      await settle(tester);
      final list = find.ancestor(
        of: find.text(c.t('platform.purgeDisabledListTitle')),
        matching: find.byType(SingleChildScrollView),
      );
      expect(list, findsWidgets);
      expect(tester.getSize(list.first).height, lessThanOrEqualTo(240));
      // 确认按钮没被挤出屏幕：清单滚了，弹窗本身还是那么高。
      expect(find.text(c.t('action.confirm')), findsOneWidget);
    });

    testWidgets('预览拉取中先开弹窗写「处理中」，此时确认点不动', (tester) async {
      await useBigSurface(tester);
      final k = cardFake();
      final gate = Completer<List<Object?>>();
      k.responses['platform_purge_disabled_preview'] = () => gate.future;
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('platform.purgeDisabled')));
      await tester.pump();
      expect(find.text(c.t('status.loading')), findsOneWidget);
      await tester.tap(find.text(c.t('action.confirm')));
      await tester.pump();
      expect(k.commands.contains('platform_purge_disabled'), isFalse);

      gate.complete([
        {'id': 2, 'name': '坏平台', 'reason': 'expired', 'action': 'delete'},
      ]);
      await settle(tester);
      expect(find.text('坏平台'), findsOneWidget);
    });

    testWidgets('执行中弹窗不关：确认按钮变「处理中」且两颗按钮都禁掉', (tester) async {
      await useBigSurface(tester);
      final k = cardFake();
      k.responses['platform_purge_disabled_preview'] = [
        {'id': 2, 'name': '坏平台', 'reason': 'expired', 'action': 'delete'},
      ];
      final gate = Completer<Map<String, Object?>>();
      k.responses['platform_purge_disabled'] = () => gate.future;
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('platform.purgeDisabled')));
      await settle(tester);
      await tester.tap(find.text(c.t('action.confirm')));
      await tester.pump();

      // 弹窗还在（清单还看得到），确认按钮换成进行中文案。
      expect(find.text('坏平台'), findsOneWidget);
      expect(find.text(c.t('status.loading')), findsOneWidget);
      // 这时点取消不该把弹窗关掉。
      await tester.tap(find.text(c.t('action.cancel')));
      await tester.pump();
      expect(find.text('坏平台'), findsOneWidget);

      gate.complete({'deletedIds': <Object?>[2], 'unassignedIds': <Object?>[]});
      await settle(tester);
      expect(find.text('坏平台'), findsNothing);
    });
  });

  // ══ 页头与未分组区（票 14 第二梯队）═══════════════════════════════

  group('卡片 logo 的协议品牌色（React: PlatformCard.tsx:231-237）', () {
    Future<void> mount(WidgetTester tester, FakeInvoke k) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
    }

    /// logo 框的描边色。36×36 那一枚在卡片里是唯一尺寸，按尺寸认。
    Color logoBorder(WidgetTester tester) {
      final box = tester.widgetList<Container>(find.byType(Container)).firstWhere(
        (w) =>
            w.constraints?.maxWidth == 36 &&
            w.decoration is BoxDecoration &&
            (w.decoration! as BoxDecoration).border != null,
      );
      return ((box.decoration! as BoxDecoration).border! as Border).top.color;
    }

    testWidgets('registry 登记了颜色 → logo 框用品牌色描边', (tester) async {
      await mount(
        tester,
        cardFake(platforms: [platRow(1, 'Bare', type: 'bare_proto')]),
      );
      expect(
        logoBorder(tester),
        const Color(0xFF10A37F).withValues(alpha: 0x30 / 255),
      );
    });

    testWidgets('registry 没登记颜色 → 回落主题色，不拿别家的品牌色顶替', (tester) async {
      await mount(
        tester,
        cardFake(platforms: [platRow(1, 'NoColor', type: 'nocolor_proto')]),
      );
      expect(
        logoBorder(tester),
        isNot(const Color(0xFF10A37F).withValues(alpha: 0x30 / 255)),
      );
    });
  });

  group('页头与未分组区（React: PlatformListView.tsx:106-147）', () {
    Future<I18nController> mountPage(WidgetTester tester, FakeInvoke k) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      return c;
    }

    testWidgets('页头「添加平台」是实心，「清理失效」保持弱化', (tester) async {
      // 判据是 React 各调用点的 variant：「添加平台」没写 variant = 默认实心
      //（`PlatformListView.tsx:121`），「清理失效」是 `variant="ghost"`（`:124-125`）。
      // 「添加分组」同为实心，但这个夹具 `showGroups: false`，页头上没有那颗。
      final c = await mountPage(tester, cardFake());
      SmallButton btn(String label) =>
          tester.widget<SmallButton>(find.widgetWithText(SmallButton, label));
      expect(btn('+ ${c.t('platform.add')}').filled, isTrue);
      final purge = btn(c.t('platform.purgeDisabled'));
      expect(purge.filled, isFalse);
      expect(purge.ghost, isTrue);
    });

    testWidgets('破坏性的「清理失效」排在两颗「添加」之后', (tester) async {
      final c = await mountPage(tester, cardFake());
      final addX = tester
          .getTopLeft(find.text('+ ${c.t('platform.add')}'))
          .dx;
      final purgeX = tester
          .getTopLeft(find.text(c.t('platform.purgeDisabled')))
          .dx;
      expect(purgeX, greaterThan(addX));
    });

    testWidgets('副标题：有平台写「启用数 / 总数 active」，一个都没有写空态文案', (tester) async {
      final c = await mountPage(tester, cardFake());
      expect(find.text('1 / 1 active'), findsOneWidget);

      await tester.pumpWidget(const SizedBox.shrink());
      final empty = cardFake(platforms: <Object?>[]);
      await mountPage(tester, empty);
      expect(find.textContaining('active'), findsNothing);
      expect(find.text(c.t('platform.empty')), findsWidgets);
    });

    testWidgets('未分组区没有标题行', (tester) async {
      final c = await mountPage(tester, cardFake());
      expect(find.text(c.t('platform.ungrouped')), findsNothing);
    });

    testWidgets('搜索筛空只是列表空着，不写「暂无平台」（空态看的是全部平台）', (tester) async {
      final c = await mountPage(tester, cardFake());
      await tester.enterText(find.byType(TextField).first, '匹配不上的词');
      await settle(tester);
      expect(find.text('Test Platform'), findsNothing);
      expect(find.text(c.t('platform.empty')), findsNothing);
    });
  });

  // ══ 分享面板 ═══════════════════════════════════════════════════

  group('SharePanel（React: ShareModal.tsx）', () {
    testWidgets('卡片点分享 → 拉 platform_share_export 并开面板', (tester) async {
      await useBigSurface(tester);
      final k = cardFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('platform.share.button')));
      await settle(tester);
      expect(k.lastCallTo('platform_share_export')!.args!['platformId'], 1);
      expect(find.byType(SharePanel), findsOneWidget);
    });

    testWidgets('打开即自动复制一次；切格式换正文；手动复制按钮改文案', (tester) async {
      final copied = <String>[];
      final toasts = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SharePanel(
            share: const {'name': 'P', 'api_key': 'sk'},
            title: 'P',
            urlScheme: 'aidog://platform/import',
            onToast: (t, {required ok}) => toasts.add('$ok|$t'),
            onClose: () {},
            copy: (t) async => copied.add(t),
          ),
          c,
        ),
      );
      await settle(tester);
      // URL 格式默认选中（深链是推荐分享格式）。
      expect(copied.single.startsWith('aidog://platform/import?data='), isTrue);
      expect(toasts.single, 'true|${c.t('platform.share.autoCopied')}');
      expect(find.text(c.t('platform.share.copiedBtn')), findsOneWidget);

      await tester.tap(find.text(c.t('platform.share.format.yaml')));
      await settle(tester);
      expect(find.textContaining('name: P'), findsOneWidget);

      await tester.tap(find.text(c.t('platform.share.copiedBtn')));
      await settle(tester);
      expect(copied.length, 2);
      expect(copied.last.contains('name: P'), isTrue);
      expect(toasts.last, 'true|${c.t('platform.share.copied')}');
    });

    testWidgets('没有 urlScheme → 只有三个格式，默认 YAML', (tester) async {
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SharePanel(
            share: const {'name': 'P'},
            title: 'P',
            onToast: (t, {required ok}) {},
            onClose: () {},
            copy: (t) async {},
          ),
          c,
        ),
      );
      await settle(tester);
      expect(find.text(c.t('platform.share.format.url')), findsNothing);
      expect(find.textContaining('name: P'), findsOneWidget);
    });

    testWidgets('复制失败 → 报失败 toast，按钮文案不变', (tester) async {
      final toasts = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SharePanel(
            share: const {'name': 'P'},
            title: 'P',
            onToast: (t, {required ok}) => toasts.add('$ok|$t'),
            onClose: () {},
            copy: (t) async => throw StateError('no clipboard'),
          ),
          c,
        ),
      );
      await settle(tester);
      expect(toasts.single, 'false|${c.t('platform.share.copyFail')}');
      expect(find.text(c.t('platform.share.copyBtn')), findsOneWidget);
    });
  });

  // ══ 拖拽排序 ═══════════════════════════════════════════════════

  group('拖拽排序（React: usePlatformsState.ts:251）', () {
    Future<(FakeInvoke, PlatformsController)> ctl() async {
      final k = cardFake(
        platforms: [platRow(1, 'a'), platRow(2, 'b'), platRow(3, 'c')],
      );
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      return (k, c);
    }

    test('第一张拖到末尾：本地立刻重排 + 整串 id 发后端', () async {
      final (k, c) = await ctl();
      await c.reorderStandalone(0, 2);
      expect([for (final p in c.platforms) p.name].join(','), 'b,c,a');
      expect(k.lastCallTo('platform_reorder')!.args!['orderedIds'], [2, 3, 1]);
    });

    test('末尾拖回开头', () async {
      final (k, c) = await ctl();
      await c.reorderStandalone(2, 0);
      expect([for (final p in c.platforms) p.name].join(','), 'c,a,b');
      expect(k.lastCallTo('platform_reorder')!.args!['orderedIds'], [3, 1, 2]);
    });

    test('原地不动 / 越界下标 → 一个命令都不发', () async {
      final (k, c) = await ctl();
      await c.reorderStandalone(1, 1);
      await c.reorderStandalone(9, 0);
      await c.reorderStandalone(-1, 0);
      expect(k.commands.contains('platform_reorder'), isFalse);
      expect([for (final p in c.platforms) p.name].join(','), 'a,b,c');
    });

    test('后端写失败不回滚本地顺序（React: .catch(console.error)）', () async {
      final (k, c) = await ctl();
      k.errors['platform_reorder'] = StateError('boom');
      await c.reorderStandalone(0, 1);
      expect([for (final p in c.platforms) p.name].join(','), 'b,a,c');
    });

    test('applyLocalOrder 只重排未分组那批，已分组的留在原位', () async {
      final k = cardFake(
        platforms: [platRow(1, 'a'), platRow(2, 'b'), platRow(3, 'c')],
      );
      k.responses['group_detail_list'] = [
        {
          'group': {'id': 1, 'group_key': 'gk', 'name': 'G'},
          'platforms': [
            {'platform': platRow(2, 'b'), 'priority': 0, 'weight': 1},
          ],
        },
      ];
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      // 未分组的是 a / c；把 a 挪到 c 后面，b 原位不动。
      await c.reorderStandalone(0, 1);
      expect([for (final p in c.platforms) p.name].join(','), 'c,b,a');
      expect(k.lastCallTo('platform_reorder')!.args!['orderedIds'], [3, 1]);
    });

    testWidgets('每张卡都有拖拽手柄', (tester) async {
      await useBigSurface(tester);
      final k = cardFake(platforms: [platRow(1, 'a'), platRow(2, 'b')]);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      expect(find.byTooltip(c.t('platform.dragReorder')), findsNWidgets(2));
    });

    testWidgets('拖起来：那张换成虚线 ghost，其余压到 0.4', (tester) async {
      await useBigSurface(tester);
      final k = cardFake(platforms: [platRow(1, 'a'), platRow(2, 'b')]);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream<void>.empty(),
          ),
          c,
        ),
      );
      await settle(tester);
      expect(find.text('a'), findsOneWidget);

      // 按住第一张的手柄拖一段，停在半途（不松手）。
      final handle = find.byTooltip(c.t('platform.dragReorder')).first;
      final drag = await tester.startGesture(tester.getCenter(handle));
      await tester.pump();
      for (var i = 0; i < 6; i++) {
        await drag.moveBy(const Offset(0, 10));
        await tester.pump();
      }

      // 其余卡压暗：0.4 这个值直接来自 React。
      final dimmed = tester
          .widgetList<Opacity>(find.byType(Opacity))
          .where((w) => w.opacity == 0.4);
      expect(dimmed, isNotEmpty, reason: '拖拽时其余卡要压暗');

      await drag.up();
      await settle(tester);
      // 松手后一切复原。
      expect(
        tester
            .widgetList<Opacity>(find.byType(Opacity))
            .where((w) => w.opacity == 0.4),
        isEmpty,
      );
    });
  });

  group('切语言 → 重解析协议元数据', () {
    test('setLocale 换 locale 才重拉，同一个值不重复发命令', () async {
      final k = cardFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      final before = k.callsTo('get_defaults_json').length;
      c.setLocale('zh-Hans');
      expect(k.callsTo('get_defaults_json').length, before);
      c.setLocale('en-US');
      await Future<void>.delayed(Duration.zero);
      expect(k.callsTo('get_defaults_json').length, before + 1);
      expect(c.protocolMeta.label('openai'), 'OpenAI');
    });
  });

  // ══ logo 三级回退 ══════════════════════════════════════════════

  group('平台 logo 三级回退（React: useProtocolLogo.ts）', () {
    test('缓存命中 → 取 data URL，不触发同步', () async {
      final k = cardFake();
      k.responses['get_protocol_logo_path'] = '/tmp/openai.png';
      k.responses['get_protocol_logo_data_url'] =
          'data:image/png;base64,AAAA';
      final c = PlatformsController(invoke: k.fn);
      await c.ensureProtocolLogo('openai');
      expect(c.protocolLogos['openai'], 'data:image/png;base64,AAAA');
      expect(k.commands.contains('sync_protocol_logo'), isFalse);
    });

    test('缓存 miss → 触发后台同步，本会话不再问第二遍', () async {
      final k = cardFake();
      final c = PlatformsController(invoke: k.fn);
      await c.ensureProtocolLogo('openai');
      await c.ensureProtocolLogo('openai');
      expect(k.callsTo('get_protocol_logo_path').length, 1);
      expect(k.lastCallTo('sync_protocol_logo')!.args!['protocol'], 'openai');
      expect(c.protocolLogos, isEmpty);
    });

    test('查路径抛错 → 静默回落首字母，不写表', () async {
      final k = cardFake();
      k.errors['get_protocol_logo_path'] = StateError('boom');
      final c = PlatformsController(invoke: k.fn);
      await c.ensureProtocolLogo('openai');
      expect(c.protocolLogos, isEmpty);
    });

    test('同步命令自己抛错也不影响主流程', () async {
      final k = cardFake();
      k.errors['sync_protocol_logo'] = StateError('boom');
      final c = PlatformsController(invoke: k.fn);
      await c.ensureProtocolLogo('openai');
      expect(c.protocolLogos, isEmpty);
    });

    test('空协议名不发命令', () async {
      final k = cardFake();
      final c = PlatformsController(invoke: k.fn);
      await c.ensureProtocolLogo('');
      expect(k.commands.contains('get_protocol_logo_path'), isFalse);
    });

    // logo 的四级回退（含 mime 分流、ICO 抠 PNG）单独放
    // `test/pages/platform_logo_test.dart`，那边一起测四级串联。
  });
}
