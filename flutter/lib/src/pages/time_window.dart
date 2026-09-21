/// 时段窗口内核，逐行对齐 `src/utils/timeWindow.ts` 与
/// `src/domains/platforms/defaults.ts::TimeWindow`。
///
/// 两条铁律（跨层一致，与 Rust `gateway::peak` 对称）：
/// 1. **存储恒 UTC+0**（窗口不带 `timezone` 时），展示/输入按 [TzMode] 换算；
///    带 `timezone` 的窗口存值即该时区本地值，读写都不换算。
/// 2. 半开区间 `[start, end)`，`end <= start` 视为跨天。
///
/// `wallTimeInTz` 用 `package:timezone`（IANA 数据库）对标 TS 侧的
/// `Intl.DateTimeFormat`。这个包本来就是 `flutter_local_notifications` 的传递依赖，
/// 提直接依赖零新增下载。
library;

import 'package:timezone/data/latest.dart' as tzdata;
import 'package:timezone/timezone.dart' as tz;

/// 时区展示模式：本地 or UTC+0。存储永远 UTC+0，仅展示/输入层换算。
enum TzMode { local, utc }

/// 窗口时区下拉候选（IANA 名）。`__utc__` 哨兵 = 无 timezone 字段（= UTC，向后兼容）。
/// 与 `timeWindow.ts::WINDOW_TIMEZONES` 逐条同序。
const List<String> kWindowTimezones = [
  '__utc__',
  'Asia/Shanghai',
  'Asia/Tokyo',
  'Asia/Singapore',
  'Asia/Kolkata',
  'America/New_York',
  'America/Chicago',
  'America/Los_Angeles',
  'Europe/London',
  'Europe/Berlin',
  'Europe/Zurich',
  'Europe/Moscow',
];

/// 本地时区相对 UTC 的分钟偏移（东区为正）。模块加载时取值，
/// 沿用 TS 侧 `LOCAL_OFFSET_MINUTES` 的既有时机（不做 DST 跨时刻重算）。
final int kLocalOffsetMinutes = DateTime.now().timeZoneOffset.inMinutes;

/// 选中时区模式对应的分钟偏移（UTC = 0 / 本地 = [kLocalOffsetMinutes]）。
int tzOffsetMinutes(TzMode mode) =>
    mode == TzMode.local ? kLocalOffsetMinutes : 0;

/// 时钟平移的纯函数内核 —— offset 显式入参，单测可覆盖任意时区（含 +5:30 / 负偏移）。
({int hour, int minute}) shiftClock(int hour, int minute, int offsetMinutes) {
  final m = (((hour * 60 + minute + offsetMinutes) % 1440) + 1440) % 1440;
  return (hour: m ~/ 60, minute: m % 60);
}

/// UTC 存值 → 选中时区显示值。按绝对分钟换算，半时区（+5:30）精确到分钟。
({int hour, int minute}) utcToDisplay(int hour, int minute, TzMode mode) =>
    shiftClock(hour, minute, tzOffsetMinutes(mode));

/// 选中时区输入值 → UTC 存值。
({int hour, int minute}) displayToUtc(int hour, int minute, TzMode mode) =>
    shiftClock(hour, minute, -tzOffsetMinutes(mode));

/// 高峰/低峰时段倍率窗口（`domains/platforms/defaults.ts::TimeWindow`）。
///
/// 字段名一律用 wire 上的 snake_case 名对应（`toJson` 里写回去），缺省的可选字段
/// **不写进 JSON**（与 TS 的 `undefined` 在 `JSON.stringify` 下被丢掉一致）。
class TimeWindow {
  const TimeWindow({
    required this.startHour,
    required this.endHour,
    required this.multiplier,
    this.timezone,
    this.daysOfWeek,
    this.startMinute,
    this.endMinute,
    this.daysOfMonth,
    this.models,
    this.startAt,
    this.endAt,
    this.unknown = const {},
  });

  factory TimeWindow.fromJson(Map<String, dynamic> j) {
    const known = {
      'start_hour',
      'end_hour',
      'multiplier',
      'timezone',
      'days_of_week',
      'start_minute',
      'end_minute',
      'days_of_month',
      'models',
      'start_at',
      'end_at',
    };
    return TimeWindow(
      // 存量数据里 start_hour 可能是非整数（半时区旧逻辑产物，如 8.5），
      // 这里原样保留成 double 再由 [normalizeWindow] 拆成 hour+minute。
      startHour: (j['start_hour'] as num?)?.toInt() ?? 0,
      endHour: (j['end_hour'] as num?)?.toInt() ?? 0,
      multiplier: (j['multiplier'] as num?)?.toDouble() ?? 1.0,
      timezone: j['timezone'] as String?,
      daysOfWeek: _intList(j['days_of_week']),
      startMinute: (j['start_minute'] as num?)?.toInt(),
      endMinute: (j['end_minute'] as num?)?.toInt(),
      daysOfMonth: _intList(j['days_of_month']),
      models: j['models'] is List
          ? [for (final m in j['models'] as List) '$m']
          : null,
      startAt: (j['start_at'] as num?)?.toInt(),
      endAt: (j['end_at'] as num?)?.toInt(),
      unknown: {
        for (final e in j.entries)
          if (!known.contains(e.key)) e.key: e.value,
      },
    );
  }

  static List<int>? _intList(Object? v) => v is List
      ? [for (final x in v) (x as num).toInt()]
      : null;

  final int startHour;
  final int endHour;
  final double multiplier;
  final String? timezone;
  final List<int>? daysOfWeek;
  final int? startMinute;
  final int? endMinute;
  final List<int>? daysOfMonth;
  final List<String>? models;
  final int? startAt;
  final int? endAt;

  /// 后端/远端可能带上本层不认识的键；序列化时原样带回去，别把别人的数据吃掉。
  final Map<String, Object?> unknown;

  Map<String, Object?> toJson() => {
    ...unknown,
    'start_hour': startHour,
    'end_hour': endHour,
    'multiplier': multiplier,
    if (timezone != null) 'timezone': timezone,
    if (daysOfWeek != null) 'days_of_week': daysOfWeek,
    if (startMinute != null) 'start_minute': startMinute,
    if (endMinute != null) 'end_minute': endMinute,
    if (daysOfMonth != null) 'days_of_month': daysOfMonth,
    if (models != null) 'models': models,
    if (startAt != null) 'start_at': startAt,
    if (endAt != null) 'end_at': endAt,
  };

  /// `null` 入参 = 不改；要把一个可选字段**清掉**用 `clearXxx`（Dart 没有
  /// `undefined` 与 `null` 的二分，不给显式 clear 就没法表达「删掉这个键」）。
  TimeWindow copyWith({
    int? startHour,
    int? endHour,
    double? multiplier,
    String? timezone,
    List<int>? daysOfWeek,
    int? startMinute,
    int? endMinute,
    List<int>? daysOfMonth,
    List<String>? models,
    int? startAt,
    int? endAt,
    bool clearTimezone = false,
    bool clearDaysOfWeek = false,
    bool clearDaysOfMonth = false,
    bool clearModels = false,
    bool clearStartAt = false,
    bool clearEndAt = false,
  }) => TimeWindow(
    startHour: startHour ?? this.startHour,
    endHour: endHour ?? this.endHour,
    multiplier: multiplier ?? this.multiplier,
    timezone: clearTimezone ? null : (timezone ?? this.timezone),
    daysOfWeek: clearDaysOfWeek ? null : (daysOfWeek ?? this.daysOfWeek),
    startMinute: startMinute ?? this.startMinute,
    endMinute: endMinute ?? this.endMinute,
    daysOfMonth: clearDaysOfMonth ? null : (daysOfMonth ?? this.daysOfMonth),
    models: clearModels ? null : (models ?? this.models),
    startAt: clearStartAt ? null : (startAt ?? this.startAt),
    endAt: clearEndAt ? null : (endAt ?? this.endAt),
    unknown: unknown,
  );

  /// 深拷贝（列表也复制），对应 TS 侧 `{...w, days_of_week: [...]}` 的那几处。
  TimeWindow clone() => TimeWindow(
    startHour: startHour,
    endHour: endHour,
    multiplier: multiplier,
    timezone: timezone,
    daysOfWeek: daysOfWeek == null ? null : [...daysOfWeek!],
    startMinute: startMinute,
    endMinute: endMinute,
    daysOfMonth: daysOfMonth == null ? null : [...daysOfMonth!],
    models: models == null ? null : [...models!],
    startAt: startAt,
    endAt: endAt,
    unknown: unknown,
  );
}

/// 存量非整数 `start_hour`/`end_hour`（半时区旧逻辑产物，如 8.5）拆为 hour+minute。
///
/// Dart 这边 `TimeWindow.startHour` 已经是 int（`fromJson` 里 `toInt()` 截断），
/// 所以真正的拆分发生在这里的 [fromJsonNormalized]；本函数保留成恒等变换，
/// 供与 TS 侧同名调用点一一对应。
TimeWindow normalizeWindow(TimeWindow w) => w;

/// 从 JSON 读一个窗口并做 TS `normalizeWindow` 的小数拆分。
TimeWindow timeWindowFromJsonNormalized(Map<String, dynamic> j) {
  var w = TimeWindow.fromJson(j);
  final rawStart = (j['start_hour'] as num?)?.toDouble() ?? 0;
  final rawEnd = (j['end_hour'] as num?)?.toDouble() ?? 0;
  if (rawStart != rawStart.truncateToDouble()) {
    final s = _splitFraction(rawStart, w.startMinute);
    w = w.copyWith(startHour: s.hour, startMinute: s.minute);
  }
  if (rawEnd != rawEnd.truncateToDouble()) {
    final e = _splitFraction(rawEnd, w.endMinute);
    w = w.copyWith(endHour: e.hour, endMinute: e.minute);
  }
  return w;
}

({int hour, int minute}) _splitFraction(double h, int? existingMinute) {
  final hour = h.floor();
  final extraMinutes = ((h - hour) * 60).round();
  return shiftClock(hour, (existingMinute ?? 0) + extraMinutes, 0);
}

bool _tzInited = false;

void _ensureTz() {
  if (_tzInited) return;
  tzdata.initializeTimeZones();
  _tzInited = true;
}

/// `ms` 在 `timeZone`（IANA 名，缺省 / 非法 = UTC）的**本地**全时间分量。
/// 与 TS `wallTimeInTz` / Rust `peak::wall_time` 对称。
///
/// weekday 用 JS 口径 **0=Sunday…6=Saturday**（Dart 的 `DateTime.weekday`
/// 是 1=Mon…7=Sun，这里 `% 7` 折过来）。
({int hour, int minute, int weekday, int dayOfMonth}) wallTimeInTz(
  int ms,
  String? timeZone,
) {
  DateTime d;
  if (timeZone == null || timeZone.isEmpty || timeZone == 'UTC') {
    // UTC 不查数据库：`data/latest.dart` 的精简集里根本没有 `UTC` 这个 Location，
    // 而 UTC 本来就是 `DateTime` 自带的能力。
    d = DateTime.fromMillisecondsSinceEpoch(ms, isUtc: true);
  } else {
    _ensureTz();
    try {
      d = tz.TZDateTime.fromMillisecondsSinceEpoch(
        tz.getLocation(timeZone),
        ms,
      );
    } catch (_) {
      // 非法时区名 → 回落 UTC（数据脏不炸 UI）。
      d = DateTime.fromMillisecondsSinceEpoch(ms, isUtc: true);
    }
  }
  return (
    hour: d.hour,
    minute: d.minute,
    weekday: d.weekday % 7,
    dayOfMonth: d.day,
  );
}

int _clampMinute(int m) => m < 0 ? 0 : (m > 59 ? 59 : m);

/// 单 pattern 与请求模型匹配（与 Rust `peak::model_match` 对称）。
bool modelMatch(String pattern, String requestModel) {
  if (pattern.endsWith('*')) {
    final prefix = pattern.substring(0, pattern.length - 1);
    return requestModel == prefix || requestModel.startsWith(prefix);
  }
  return requestModel == pattern;
}

/// 窗口 model scope 是否覆盖 requestModel（与 Rust `peak::window_models_hit` 对称）。
bool windowModelsHit(TimeWindow w, String requestModel) {
  if (requestModel.isEmpty) return true;
  final ms = w.models;
  if (ms == null || ms.isEmpty) return true;
  return ms.any((p) => modelMatch(p, requestModel));
}

bool _hit(
  TimeWindow w,
  int hour,
  int minute,
  int weekday,
  int dayOfMonth,
  String requestModel,
  int epochSec,
) {
  // 生效期判定（与 Rust period_active 对称，优先级最高）
  if (w.startAt != null && epochSec < w.startAt!) return false;
  if (w.endAt != null && epochSec >= w.endAt!) return false;
  if (w.daysOfWeek != null && !w.daysOfWeek!.contains(weekday)) return false;
  if (w.daysOfMonth != null && !w.daysOfMonth!.contains(dayOfMonth)) {
    return false;
  }
  final tMin = hour * 60 + minute;
  final startMin = w.startHour * 60 + _clampMinute(w.startMinute ?? 0);
  final endMin = w.endHour * 60 + _clampMinute(w.endMinute ?? 0);
  final bool timeHit;
  if (endMin > startMin) {
    timeHit = tMin >= startMin && tMin < endMin;
  } else {
    // 跨天（含 start==end 的退化情况，按全天命中处理）
    timeHit = tMin >= startMin || tMin < endMin;
  }
  if (!timeHit) return false;
  return windowModelsHit(w, requestModel);
}

/// first-match 命中任一窗口 → true（不关心 multiplier 值）；空/无命中 → false。
bool isCurrentlyPeak(
  List<TimeWindow>? windows,
  int nowMs, [
  String requestModel = '',
]) {
  if (windows == null || windows.isEmpty) return false;
  final epochSec = nowMs ~/ 1000;
  return windows.any((w) {
    final t = wallTimeInTz(nowMs, w.timezone);
    return _hit(
      w,
      t.hour,
      t.minute,
      t.weekday,
      t.dayOfMonth,
      requestModel,
      epochSec,
    );
  });
}
