/// 调度与熔断全局默认（`settings/scheduling`）—— 对齐
/// `src/components/settings/SchedulingSettings.tsx`。
///
/// 一页只有 5 个字段，但有两条容易搬丢的规则：
/// ① 三个数字框的校验是 `max(0, floor(Number(v) || 0))` —— 空串 / 非数字 / 负数一律成 0；
/// ② 每改一次就整份写回（没有保存按钮），写失败**不回滚本地值**，只在底部挂一条错误。
library;

import '../invoke.dart';

/// 与 `src/domains/groups/routing.ts:5::ROUTING_MODES` 逐条同序。
/// 下拉里的顺序就是这个顺序，别按字母重排。
const List<String> kRoutingModes = [
  'failover',
  'load_balance',
  'health_aware',
  'least_latency',
  'sticky',
];

/// 策略短名的 i18n key + 默认中文，对齐 `routing.ts::routingModeLabel` 的映射表。
/// 取不到 key 时回落 mode 字面量本身（与 React 的 `?? mode` 一致）。
const Map<String, (String key, String fallback)> kRoutingModeLabels = {
  'failover': ('group.failover', '故障转移'),
  'load_balance': ('group.loadBalance', '负载均衡'),
  'health_aware': ('group.routingMode.health_aware', '健康感知'),
  'least_latency': ('group.routingMode.least_latency', '最低延迟'),
  'sticky': ('group.routingMode.sticky', '会话粘性'),
};

class SchedulingSettings {
  const SchedulingSettings({
    required this.defaultRoutingMode,
    required this.breakerFailureThreshold,
    required this.breakerOpenSecs,
    required this.breakerHalfOpenMax,
    required this.enabled,
  });

  /// `SchedulingSettings.tsx:43-49` 的 DEFAULT_SETTINGS，一字不改。
  static const defaults = SchedulingSettings(
    defaultRoutingMode: 'health_aware',
    breakerFailureThreshold: 5,
    breakerOpenSecs: 60,
    breakerHalfOpenMax: 2,
    enabled: true,
  );

  final String defaultRoutingMode;
  final int breakerFailureThreshold;
  final int breakerOpenSecs;
  final int breakerHalfOpenMax;
  final bool enabled;

  factory SchedulingSettings.fromJson(Map<String, Object?> j) =>
      SchedulingSettings(
        defaultRoutingMode:
            j['default_routing_mode'] as String? ?? 'health_aware',
        breakerFailureThreshold:
            (j['breaker_failure_threshold'] as num?)?.toInt() ?? 5,
        breakerOpenSecs: (j['breaker_open_secs'] as num?)?.toInt() ?? 60,
        breakerHalfOpenMax: (j['breaker_half_open_max'] as num?)?.toInt() ?? 2,
        enabled: j['enabled'] as bool? ?? true,
      );

  Map<String, Object?> toJson() => {
    'default_routing_mode': defaultRoutingMode,
    'breaker_failure_threshold': breakerFailureThreshold,
    'breaker_open_secs': breakerOpenSecs,
    'breaker_half_open_max': breakerHalfOpenMax,
    'enabled': enabled,
  };

  SchedulingSettings copyWith({
    String? defaultRoutingMode,
    int? breakerFailureThreshold,
    int? breakerOpenSecs,
    int? breakerHalfOpenMax,
    bool? enabled,
  }) => SchedulingSettings(
    defaultRoutingMode: defaultRoutingMode ?? this.defaultRoutingMode,
    breakerFailureThreshold:
        breakerFailureThreshold ?? this.breakerFailureThreshold,
    breakerOpenSecs: breakerOpenSecs ?? this.breakerOpenSecs,
    breakerHalfOpenMax: breakerHalfOpenMax ?? this.breakerHalfOpenMax,
    enabled: enabled ?? this.enabled,
  );
}

/// 数字框的校验：`Math.max(0, Math.floor(Number(v) || 0))`。
///
/// 逐条对应 JS 的怪脾气：空串 `Number("")` 是 0（不是 NaN）；非数字是 NaN，
/// `NaN || 0` 得 0；小数向下取整；负数钳到 0。
int clampNonNegativeInt(String raw) {
  final n = double.tryParse(raw.trim());
  if (n == null || n.isNaN) return 0;
  if (n <= 0) return 0;
  if (n.isInfinite) return 0x7fffffffffffffff;
  return n.floor();
}

class SchedulingController {
  SchedulingController({InvokeFn? invoke, this.onChanged})
    : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  SchedulingSettings settings = SchedulingSettings.defaults;
  bool loading = true;
  String error = '';

  void _notify() => onChanged?.call();

  Future<void> load() async {
    try {
      settings = SchedulingSettings.fromJson(
        _map(await _invoke('scheduling_settings_get')),
      );
    } catch (_) {
      // React：console.error 后退回默认值（不在界面上报错）。
      settings = SchedulingSettings.defaults;
    } finally {
      loading = false;
      _notify();
    }
  }

  /// 没有保存按钮：改一次就写一次。写失败不回滚本地值（与 React 一致）。
  Future<void> persist(SchedulingSettings next) async {
    settings = next;
    error = '';
    _notify();
    try {
      await _invoke('scheduling_settings_set', {'settings': next.toJson()});
    } catch (e) {
      error = '$e';
      _notify();
    }
  }

  Future<void> toggleEnabled() =>
      persist(settings.copyWith(enabled: !settings.enabled));

  Future<void> setRoutingMode(String mode) =>
      persist(settings.copyWith(defaultRoutingMode: mode));

  Future<void> setFailureThreshold(String raw) => persist(
    settings.copyWith(breakerFailureThreshold: clampNonNegativeInt(raw)),
  );

  Future<void> setOpenSecs(String raw) =>
      persist(settings.copyWith(breakerOpenSecs: clampNonNegativeInt(raw)));

  Future<void> setHalfOpenMax(String raw) =>
      persist(settings.copyWith(breakerHalfOpenMax: clampNonNegativeInt(raw)));

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}
