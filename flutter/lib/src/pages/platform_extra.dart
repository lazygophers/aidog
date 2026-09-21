/// `platform.extra`（一个 JSON 字符串列）的读写层，逐个对齐
/// `src/services/api/platforms.ts` 的 parse/serialize 对，外加
/// `domains/platforms/autoCategorize.ts` 与 `utils/platformPaste.ts` 的两个纯函数。
///
/// 共同规矩（照抄 TS 侧）：
/// - 解析失败 / 非对象 / 空串 → 回落默认值，**不抛**；
/// - 序列化一律「读出整个 extra 对象 → 改自己那个键 → 写回」，保留别人的键；
/// - 「等于默认值」的配置**删键**而不是写 0/false，这样后端才知道是「继承」。
library;

import 'dart:convert';
import 'dart:math';

import 'time_window.dart';

/// 安全地把 extra 读成一个 JSON 对象；空串 / 非法 / 非对象 → 空 map。
Map<String, Object?> decodeExtraObject(String extra) {
  if (extra.trim().isEmpty) return {};
  try {
    final parsed = jsonDecode(extra);
    if (parsed is Map) return Map<String, Object?>.from(parsed);
  } catch (_) {
    /* 非法 JSON → 重建 */
  }
  return {};
}

Map<String, Object?>? _nestOf(Map<String, Object?> obj, String nest) {
  final home = obj[nest];
  if (home is Map) return Map<String, Object?>.from(home);
  return null;
}

// ─── Mock 平台配置（extra.mock）───────────────────────────────────────

/// `manual.ts::MockErrorMode`。
const List<String> kMockErrorModes = [
  'none',
  'http_error',
  'rate_limit_429',
  'timeout',
];

/// `manual.ts::MockErrorMode` 的 i18n key（`domains/platforms/constants.ts::MOCK_ERROR_MODES`）。
const Map<String, String> kMockErrorModeLabelKeys = {
  'none': 'platform.mockErrorNone',
  'http_error': 'platform.mockErrorHttp',
  'rate_limit_429': 'platform.mockErrorRateLimit',
  'timeout': 'platform.mockErrorTimeout',
};

/// `manual.ts::MockConfig`。三个可选字段（ttft_ms / inter_chunk_ms / error_rate）
/// 留 null = 不写进 JSON，让 Rust 的 `Option` 收到 `None` 走默认，**禁塞 0 假装默认**。
class MockConfig {
  const MockConfig({
    required this.statusCode,
    required this.delayMs,
    this.ttftMs,
    this.interChunkMs,
    required this.streamOverride,
    required this.responseText,
    required this.finishReason,
    required this.inputTokens,
    required this.outputTokens,
    required this.cacheTokens,
    required this.errorMode,
    this.errorRate,
    required this.chunkCount,
  });

  final int statusCode;
  final int delayMs;
  final int? ttftMs;
  final int? interChunkMs;

  /// null = 跟随请求的 stream；true/false = 强制流式/非流式。
  final bool? streamOverride;
  final String responseText;
  final String finishReason;
  final int inputTokens;
  final int outputTokens;
  final int cacheTokens;
  final String errorMode;
  final double? errorRate;
  final int chunkCount;

  MockConfig copyWith({
    int? statusCode,
    int? delayMs,
    int? ttftMs,
    int? interChunkMs,
    bool? streamOverride,
    String? responseText,
    String? finishReason,
    int? inputTokens,
    int? outputTokens,
    int? cacheTokens,
    String? errorMode,
    double? errorRate,
    int? chunkCount,
    bool clearTtftMs = false,
    bool clearInterChunkMs = false,
    bool clearStreamOverride = false,
    bool clearErrorRate = false,
  }) => MockConfig(
    statusCode: statusCode ?? this.statusCode,
    delayMs: delayMs ?? this.delayMs,
    ttftMs: clearTtftMs ? null : (ttftMs ?? this.ttftMs),
    interChunkMs: clearInterChunkMs ? null : (interChunkMs ?? this.interChunkMs),
    streamOverride: clearStreamOverride
        ? null
        : (streamOverride ?? this.streamOverride),
    responseText: responseText ?? this.responseText,
    finishReason: finishReason ?? this.finishReason,
    inputTokens: inputTokens ?? this.inputTokens,
    outputTokens: outputTokens ?? this.outputTokens,
    cacheTokens: cacheTokens ?? this.cacheTokens,
    errorMode: errorMode ?? this.errorMode,
    errorRate: clearErrorRate ? null : (errorRate ?? this.errorRate),
    chunkCount: chunkCount ?? this.chunkCount,
  );

  Map<String, Object?> toJson() => {
    'status_code': statusCode,
    'delay_ms': delayMs,
    if (ttftMs != null) 'ttft_ms': ttftMs,
    if (interChunkMs != null) 'inter_chunk_ms': interChunkMs,
    'stream_override': streamOverride,
    'response_text': responseText,
    'finish_reason': finishReason,
    'input_tokens': inputTokens,
    'output_tokens': outputTokens,
    'cache_tokens': cacheTokens,
    'error_mode': errorMode,
    if (errorRate != null) 'error_rate': errorRate,
    'chunk_count': chunkCount,
  };
}

/// `platforms.ts::DEFAULT_MOCK_CONFIG`。
const MockConfig kDefaultMockConfig = MockConfig(
  statusCode: 200,
  delayMs: 0,
  streamOverride: null,
  responseText: 'Hello from mock',
  finishReason: 'end_turn',
  inputTokens: 100,
  outputTokens: 50,
  cacheTokens: 0,
  errorMode: 'none',
  chunkCount: 5,
);

/// `platforms.ts:262::parseMockConfig`。
MockConfig parseMockConfig(String extra) {
  if (extra.trim().isEmpty) return kDefaultMockConfig;
  try {
    final parsed = jsonDecode(extra);
    if (parsed is Map && parsed.containsKey('mock')) {
      final mock = parsed['mock'];
      if (mock is Map) {
        const d = kDefaultMockConfig;
        return MockConfig(
          statusCode: (mock['status_code'] as num?)?.toInt() ?? d.statusCode,
          delayMs: (mock['delay_ms'] as num?)?.toInt() ?? d.delayMs,
          ttftMs: (mock['ttft_ms'] as num?)?.toInt(),
          interChunkMs: (mock['inter_chunk_ms'] as num?)?.toInt(),
          streamOverride: mock['stream_override'] as bool?,
          responseText: (mock['response_text'] as String?) ?? d.responseText,
          finishReason: (mock['finish_reason'] as String?) ?? d.finishReason,
          inputTokens: (mock['input_tokens'] as num?)?.toInt() ?? d.inputTokens,
          outputTokens:
              (mock['output_tokens'] as num?)?.toInt() ?? d.outputTokens,
          cacheTokens: (mock['cache_tokens'] as num?)?.toInt() ?? d.cacheTokens,
          errorMode: (mock['error_mode'] as String?) ?? d.errorMode,
          errorRate: (mock['error_rate'] as num?)?.toDouble(),
          chunkCount: (mock['chunk_count'] as num?)?.toInt() ?? d.chunkCount,
        );
      }
    }
  } catch (_) {
    /* 非法 JSON → 回退默认 */
  }
  return kDefaultMockConfig;
}

/// `platforms.ts:281::serializeMockConfig`。
String serializeMockConfig(String extra, MockConfig mock) {
  final obj = decodeExtraObject(extra);
  obj['mock'] = mock.toJson();
  return jsonEncode(obj);
}

// ─── Devin 配置（extra.devin）─────────────────────────────────────────

/// `manual.ts::DevinConfig`。timeout 用 String 与 number input 兼容。
class DevinConfig {
  const DevinConfig({required this.devinTimeout, required this.devinMode});

  final String devinTimeout;
  final String devinMode;

  DevinConfig copyWith({String? devinTimeout, String? devinMode}) => DevinConfig(
    devinTimeout: devinTimeout ?? this.devinTimeout,
    devinMode: devinMode ?? this.devinMode,
  );
}

/// `platforms.ts::DEFAULT_DEVIN_CONFIG`。
const DevinConfig kDefaultDevinConfig = DevinConfig(
  devinTimeout: '',
  devinMode: '',
);

/// `formSections.tsx:277` 的 devin 模式候选。
const List<String> kDevinModes = ['normal', 'fast', 'lite', 'ultra', 'fusion'];

/// `platforms.ts:75::parseDevinConfig`。
DevinConfig parseDevinConfig(String extra) {
  if (extra.trim().isEmpty) return kDefaultDevinConfig;
  try {
    final parsed = jsonDecode(extra);
    if (parsed is Map && parsed.containsKey('devin')) {
      final d = parsed['devin'];
      if (d is Map) {
        final to = d['devin_timeout'];
        return DevinConfig(
          devinTimeout: to is num
              ? '${to.toInt()}'
              : (to is String ? to : ''),
          devinMode: d['devin_mode'] is String ? d['devin_mode'] as String : '',
        );
      }
    }
  } catch (_) {
    /* ignore */
  }
  return kDefaultDevinConfig;
}

/// `platforms.ts:95::serializeDevinConfig`。存量嵌套 `org_id` 原样保留
/// （本表单不再编辑它，编辑入口在配额脚本 requires）；三样全空 → 删整个 `devin` 键。
String serializeDevinConfig(String extra, DevinConfig cfg) {
  final obj = decodeExtraObject(extra);
  final prev = _nestOf(obj, 'devin');
  final orgId = prev != null && prev['org_id'] is String
      ? prev['org_id'] as String
      : '';
  final timeoutNum = max(0, (num.tryParse(cfg.devinTimeout) ?? 0).floor());
  final mode = cfg.devinMode.trim();
  if (orgId.trim().isEmpty && timeoutNum == 0 && mode.isEmpty) {
    obj.remove('devin');
  } else {
    final devin = <String, Object?>{};
    if (orgId.trim().isNotEmpty) devin['org_id'] = orgId;
    if (timeoutNum > 0) devin['devin_timeout'] = timeoutNum;
    if (mode.isNotEmpty) devin['devin_mode'] = mode;
    obj['devin'] = devin;
  }
  return jsonEncode(obj);
}

// ─── 配额查询脚本（extra.quota_script_id / quota_custom_script / requires）───

/// `formSections.tsx:156`：自定义伪变体哨兵值。
const String kQuotaCustomVariant = '__custom__';

/// `platforms.ts:127::LEGACY_REQUIRES_NEST` —— requires 参数的旧嵌套家。
const Map<String, String> kLegacyRequiresNest = {
  'org_id': 'devin',
  'balance_base_url': 'newapi',
  'balance_api_key': 'newapi',
};

/// `platforms.ts:144::parseQuotaScriptConfig`。
({String variantId, String customScript}) parseQuotaScriptConfig(String extra) {
  if (extra.trim().isEmpty) return (variantId: '', customScript: '');
  try {
    final parsed = jsonDecode(extra);
    if (parsed is Map) {
      return (
        variantId: parsed['quota_script_id'] is String
            ? parsed['quota_script_id'] as String
            : '',
        customScript: parsed['quota_custom_script'] is String
            ? parsed['quota_custom_script'] as String
            : '',
      );
    }
  } catch (_) {
    /* ignore */
  }
  return (variantId: '', customScript: '');
}

/// `platforms.ts:160::hasCustomQuotaScript`。
bool hasCustomQuotaScript(String extra) =>
    parseQuotaScriptConfig(extra).customScript.trim().isNotEmpty;

/// `platforms.ts:167::readRequiresValue`：嵌套优先（newapi / devin）→ 顶层兜底。
String readRequiresValue(String extra, String key) {
  if (extra.trim().isEmpty) return '';
  try {
    final parsed = jsonDecode(extra);
    if (parsed is! Map) return '';
    for (final nest in const ['newapi', 'devin']) {
      final home = parsed[nest];
      if (home is Map) {
        final v = home[key];
        if (v is String && v.isNotEmpty) return v;
      }
    }
    final top = parsed[key];
    return top is String ? top : '';
  } catch (_) {
    /* ignore */
  }
  return '';
}

/// `platforms.ts:193::serializeQuotaScriptConfig`。互斥规则与后端物化对齐：
/// custom 非空 → 写 `quota_custom_script` 删 `quota_script_id`；
/// 否则删 custom，id 命中 variants 才写，缺省 / 失效不写（后端回落首条）。
String serializeQuotaScriptConfig(
  String extra, {
  required String variantId,
  required String customScript,
  required Map<String, String> requires,
  required List<({String id, List<String> requires})> variants,
  required String protocol,
}) {
  final obj = decodeExtraObject(extra);
  final custom = customScript.trim();
  if (custom.isNotEmpty) {
    obj['quota_custom_script'] = customScript;
    obj.remove('quota_script_id');
  } else {
    obj.remove('quota_custom_script');
    final matched = variants.where((v) => v.id == variantId);
    final sel = matched.isNotEmpty
        ? matched.first
        : (variants.isNotEmpty ? variants.first : null);
    if (variantId.isNotEmpty && matched.isNotEmpty) {
      obj['quota_script_id'] = variantId;
    } else {
      obj.remove('quota_script_id');
    }
    if (sel != null) {
      for (final key in sel.requires) {
        final val = (requires[key] ?? '').trim();
        final nest = kLegacyRequiresNest[key];
        if (val.isNotEmpty) {
          obj[key] = val;
          if (nest != null) {
            obj[nest] = {..._nestOf(obj, nest) ?? {}, key: val};
          }
        } else {
          obj.remove(key);
          if (nest != null) {
            final home = _nestOf(obj, nest);
            if (home != null) {
              home.remove(key);
              obj[nest] = home;
            }
          }
        }
      }
    }
  }
  // newapi user_id：旧表单字段保留（查询结果回填目标）。
  if (protocol == 'newapi') {
    final uid = (requires['user_id'] ?? '').trim();
    if (uid.isNotEmpty) {
      obj['newapi'] = {..._nestOf(obj, 'newapi') ?? {}, 'user_id': uid};
    } else {
      final home = _nestOf(obj, 'newapi');
      if (home != null) {
        home.remove('user_id');
        obj['newapi'] = home;
      }
    }
  }
  return jsonEncode(obj);
}

// ─── 熔断覆盖（extra.breaker）─────────────────────────────────────────

/// `generated/PlatformBreaker.ts`。每字段 0/缺省 = 继承全局默认。
class PlatformBreaker {
  const PlatformBreaker({
    required this.failureThreshold,
    required this.openSecs,
    required this.halfOpenMax,
  });

  final int failureThreshold;
  final int openSecs;
  final int halfOpenMax;
}

/// `platforms.ts:301::parsePlatformBreaker`。
PlatformBreaker parsePlatformBreaker(String extra) {
  const zero = PlatformBreaker(
    failureThreshold: 0,
    openSecs: 0,
    halfOpenMax: 0,
  );
  if (extra.trim().isEmpty) return zero;
  try {
    final parsed = jsonDecode(extra);
    if (parsed is Map && parsed.containsKey('breaker')) {
      final b = parsed['breaker'];
      if (b is Map) {
        return PlatformBreaker(
          failureThreshold: b['failure_threshold'] is num
              ? (b['failure_threshold'] as num).toInt()
              : 0,
          openSecs: b['open_secs'] is num ? (b['open_secs'] as num).toInt() : 0,
          halfOpenMax: b['half_open_max'] is num
              ? (b['half_open_max'] as num).toInt()
              : 0,
        );
      }
    }
  } catch (_) {
    /* 非法 JSON → 回退全 0 */
  }
  return zero;
}

/// `platforms.ts:326::serializePlatformBreaker`。三值全 0 → 删 breaker 键。
String serializePlatformBreaker(String extra, PlatformBreaker b) {
  final obj = decodeExtraObject(extra);
  if (b.failureThreshold == 0 && b.openSecs == 0 && b.halfOpenMax == 0) {
    obj.remove('breaker');
  } else {
    obj['breaker'] = {
      'failure_threshold': b.failureThreshold,
      'open_secs': b.openSecs,
      'half_open_max': b.halfOpenMax,
    };
  }
  return jsonEncode(obj);
}

// ─── peak / disable_during_peak / time_windows ───────────────────────

/// `platforms.ts:348::parsePlatformPeak`。
List<TimeWindow> parsePlatformPeak(String extra) {
  if (extra.trim().isEmpty) return const [];
  try {
    final parsed = jsonDecode(extra);
    if (parsed is Map && parsed.containsKey('peak')) {
      final arr = parsed['peak'];
      if (arr is List) {
        return [
          for (final w in arr)
            timeWindowFromJsonNormalized(Map<String, dynamic>.from(w as Map)),
        ];
      }
    }
  } catch (_) {
    /* ignore */
  }
  return const [];
}

/// `platforms.ts:361::serializePlatformPeak`。空数组 → 删 peak 键。
String serializePlatformPeak(String extra, List<TimeWindow> windows) {
  final obj = decodeExtraObject(extra);
  if (windows.isEmpty) {
    obj.remove('peak');
  } else {
    obj['peak'] = [for (final w in windows) w.toJson()];
  }
  return jsonEncode(obj);
}

/// `platforms.ts:381::parseDisableDuringPeak`。严格布尔：数字/字符串不误判。
bool parseDisableDuringPeak(String extra) {
  if (extra.trim().isEmpty) return false;
  try {
    final parsed = jsonDecode(extra);
    if (parsed is Map && parsed.containsKey('disable_during_peak')) {
      return parsed['disable_during_peak'] == true;
    }
  } catch (_) {
    /* ignore */
  }
  return false;
}

/// `platforms.ts:394::serializeDisableDuringPeak`。false → 删键。
String serializeDisableDuringPeak(String extra, bool enabled) {
  final obj = decodeExtraObject(extra);
  if (enabled) {
    obj['disable_during_peak'] = true;
  } else {
    obj.remove('disable_during_peak');
  }
  return jsonEncode(obj);
}

/// `manual.ts::TimeModelRule`：按时段窗口切换主力模型档。
class TimeModelRule {
  const TimeModelRule({required this.windows, required this.models});

  factory TimeModelRule.fromJson(Map<String, dynamic> j) => TimeModelRule(
    windows: [
      for (final w in (j['windows'] as List?) ?? const [])
        timeWindowFromJsonNormalized(Map<String, dynamic>.from(w as Map)),
    ],
    models: {
      for (final e in ((j['models'] as Map?) ?? const {}).entries)
        if (e.value is String) '${e.key}': e.value as String,
    },
  );

  final List<TimeWindow> windows;

  /// 5 槽映射（`default`/`sonnet`/`opus`/`haiku`/`gpt`），只放非空槽。
  final Map<String, String> models;

  TimeModelRule copyWith({
    List<TimeWindow>? windows,
    Map<String, String>? models,
  }) => TimeModelRule(
    windows: windows ?? this.windows,
    models: models ?? this.models,
  );

  Map<String, Object?> toJson() => {
    'windows': [for (final w in windows) w.toJson()],
    'models': models,
  };
}

/// `platforms.ts:415::parsePlatformTimeWindows`。
List<TimeModelRule> parsePlatformTimeWindows(String extra) {
  if (extra.trim().isEmpty) return const [];
  try {
    final parsed = jsonDecode(extra);
    if (parsed is Map && parsed.containsKey('time_windows')) {
      final arr = parsed['time_windows'];
      if (arr is List) {
        return [
          for (final r in arr)
            TimeModelRule.fromJson(Map<String, dynamic>.from(r as Map)),
        ];
      }
    }
  } catch (_) {
    /* ignore */
  }
  return const [];
}

/// `platforms.ts:430::serializePlatformTimeWindows`。空数组 → 删 time_windows 键。
String serializePlatformTimeWindows(String extra, List<TimeModelRule> rules) {
  final obj = decodeExtraObject(extra);
  if (rules.isEmpty) {
    obj.remove('time_windows');
  } else {
    obj['time_windows'] = [for (final r in rules) r.toJson()];
  }
  return jsonEncode(obj);
}

// ─── 手动预算 ────────────────────────────────────────────────────────

/// `generated/ManualBudget.ts`。
class ManualBudget {
  const ManualBudget({
    required this.id,
    required this.kind,
    required this.unit,
    required this.amount,
    required this.windowHours,
    required this.windowUnit,
    required this.consumed,
    required this.windowStartAt,
    required this.enabled,
  });

  factory ManualBudget.fromJson(Map<String, dynamic> j) => ManualBudget(
    id: (j['id'] as String?) ?? '',
    kind: (j['kind'] as String?) ?? 'total',
    unit: (j['unit'] as String?) ?? 'usd',
    amount: (j['amount'] as num?)?.toDouble() ?? 0,
    windowHours: (j['window_hours'] as num?)?.toDouble(),
    windowUnit: (j['window_unit'] as String?) ?? 'hour',
    consumed: (j['consumed'] as num?)?.toDouble() ?? 0,
    windowStartAt: (j['window_start_at'] as num?)?.toInt(),
    enabled: (j['enabled'] as bool?) ?? true,
  );

  final String id;

  /// `total` / `rolling` / `fixed` / `daily`。
  final String kind;

  /// `usd` / `token` / `count`。
  final String unit;
  final double amount;
  final double? windowHours;

  /// `minute` / `hour` / `day` / `week` / `month`。
  final String windowUnit;
  final double consumed;
  final int? windowStartAt;
  final bool enabled;

  ManualBudget copyWith({
    String? kind,
    String? unit,
    double? amount,
    double? windowHours,
    String? windowUnit,
    bool? enabled,
    bool clearWindowHours = false,
  }) => ManualBudget(
    id: id,
    kind: kind ?? this.kind,
    unit: unit ?? this.unit,
    amount: amount ?? this.amount,
    windowHours: clearWindowHours ? null : (windowHours ?? this.windowHours),
    windowUnit: windowUnit ?? this.windowUnit,
    consumed: consumed,
    windowStartAt: windowStartAt,
    enabled: enabled ?? this.enabled,
  );

  Map<String, Object?> toJson() => {
    'id': id,
    'kind': kind,
    'unit': unit,
    'amount': amount,
    'window_hours': windowHours,
    'window_unit': windowUnit,
    'consumed': consumed,
    'window_start_at': windowStartAt,
    'enabled': enabled,
  };
}

const List<String> kManualBudgetKinds = ['total', 'rolling', 'fixed', 'daily'];
const List<String> kManualBudgetUnits = ['usd', 'token', 'count'];
const List<String> kWindowUnits = ['minute', 'hour', 'day', 'week', 'month'];

final Random _rng = Random();

/// `health.ts:183::newManualBudget`：随机 id + total/usd，consumed 从 0 起算。
ManualBudget newManualBudget() {
  final id = List.generate(
    32,
    (_) => '0123456789abcdef'[_rng.nextInt(16)],
  ).join();
  return ManualBudget(
    id: id,
    kind: 'total',
    unit: 'usd',
    amount: 0,
    windowHours: null,
    windowUnit: 'hour',
    consumed: 0,
    windowStartAt: null,
    enabled: true,
  );
}

// ─── 纯函数：模型自动归类 / key 拆分 / 批量命名预览 ───────────────────

/// `autoCategorize.ts:73::autoCategorize`：按模型名模式自动分配到 5 个槽位。
Map<String, String> autoCategorize(List<String> modelIds) {
  final result = <String, String>{
    'default': '',
    'sonnet': '',
    'opus': '',
    'haiku': '',
    'gpt': '',
  };
  final patterns = <({String slot, bool Function(String) test})>[
    (slot: 'opus', test: (id) => RegExp('opus', caseSensitive: false).hasMatch(id)),
    (
      slot: 'sonnet',
      test: (id) => RegExp('sonnet', caseSensitive: false).hasMatch(id),
    ),
    (
      slot: 'haiku',
      test: (id) => RegExp('haiku', caseSensitive: false).hasMatch(id),
    ),
    (
      slot: 'gpt',
      test: (id) =>
          RegExp('gpt', caseSensitive: false).hasMatch(id) &&
          !RegExp('mini', caseSensitive: false).hasMatch(id),
    ),
  ];
  final assigned = <String>{};
  for (final p in patterns) {
    for (final id in modelIds) {
      if (p.test(id) && !assigned.contains(id)) {
        result[p.slot] = id;
        assigned.add(id);
      }
    }
  }
  final firstIdx = modelIds.indexWhere((id) => !assigned.contains(id));
  final first = firstIdx >= 0
      ? modelIds[firstIdx]
      : (modelIds.isNotEmpty ? modelIds.first : '');
  if (first.isNotEmpty && result['default']!.isEmpty) result['default'] = first;
  return result;
}

/// `platformPaste.ts:239::splitApiKeys`：按空白 / 逗号 / 分号拆，去重保序。
List<String> splitApiKeys(String raw) {
  if (raw.isEmpty) return const [];
  final out = <String>[];
  for (final part in raw.split(RegExp(r'[\s,;]+'))) {
    final v = part.trim();
    if (v.isNotEmpty && !out.contains(v)) out.add(v);
  }
  return out;
}

/// `platformPasteApply.ts:254::previewBatchNames`：`{base}-{key尾4位}`，撞名追号 `-2`。
List<String> previewBatchNames(
  List<String> keys,
  String baseName,
  Set<String> usedNames,
) {
  final prefix = (baseName.isEmpty ? 'Platform' : baseName).trim();
  // 复制一份避免污染调用方传入的 Set（预览不写回）。
  final used = {...usedNames};
  final out = <String>[];
  for (final k in keys) {
    final tail = k.length >= 4 ? k.substring(k.length - 4) : k;
    var pname = '$prefix-$tail';
    if (used.contains(pname)) {
      var seq = 2;
      while (used.contains('$pname-$seq')) {
        seq++;
      }
      pname = '$pname-$seq';
    }
    used.add(pname);
    out.add(pname);
  }
  return out;
}
