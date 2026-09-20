/// 托盘展示配置（`settings/tray`）—— 对齐 `src/pages/TrayConfigTab.tsx`。
///
/// 要点：
/// - **即时保存**：每次增删改都整份写回 `tray_config_set`，没有保存按钮。
/// - **order 每次重排**：`withOrders` 按数组下标重写每一项的 order，
///   不这么做，拖拽后顺序在后端就是乱的。
/// - 今日统计随 `proxy-log-updated` 事件刷新（1000 ms 防抖），不是 30 s 轮询。
/// - 平台列表**只取 enabled 的**。
library;

import 'dart:convert';

import '../invoke.dart';

const int kTrayDefaultFontSize = 9;

// 票 I15：预置色 / 预置分隔符 / 对齐选项随二维网格编辑器一起删除
// （菜单栏只画固定三段，这些都不再可选；存量配置里的对应字段原样留在 DB 里）。
const List<String> kTrayTodayMetrics = ['tokens', 'cache_rate', 'cost', 'requests'];

/// 去掉小数点后多余的零：`0.111000` → `0.111`，`10.10100` → `10.101`，`0.000` → `0`。
String trimZeros(String s) {
  if (!s.contains('.')) return s;
  return s.replaceFirst(RegExp(r'\.?0+$'), '');
}

/// 一个托盘展示项。字段名照 `src/services/api/tray.ts::TrayItem`
/// （后端 serde 契约里**没有 id 字段**，所以列表的拖拽 id 由下标派生）。
class TrayItem {
  TrayItem({
    required this.itemType,
    this.platformId,
    this.display = '',
    this.metric,
    this.label,
    this.decimals,
    Map<String, Object?>? color,
    this.fontSize = kTrayDefaultFontSize,
    this.lineMode = 'two',
    this.align = 'left',
    this.alignRow2,
    this.enabled = true,
    required this.order,
  }) : color = color ?? defaultColor();

  final String itemType; // platform | today_usage | separator
  final int? platformId;
  final String display;
  final String? metric;
  final String? label;
  final int? decimals;
  final Map<String, Object?> color;
  final int fontSize;
  final String lineMode;
  final String align;
  final String? alignRow2;
  final bool enabled;
  final int order;

  static Map<String, Object?> defaultColor() => {'mode': 'follow', 'value': ''};

  factory TrayItem.platform(int platformId, String display, int order) => TrayItem(
        itemType: 'platform',
        platformId: platformId,
        display: display,
        order: order,
      );

  factory TrayItem.todayUsage(String metric, int order) => TrayItem(
        itemType: 'today_usage',
        display: '',
        metric: metric,
        order: order,
      );

  /// 票 I15 的无参段：`routed_platform`（当前命中平台）/ `peak`（高峰指示）。
  factory TrayItem.simple(String itemType, int order) => TrayItem(
        itemType: itemType,
        display: '',
        lineMode: 'single',
        order: order,
      );

  factory TrayItem.fromJson(Map<String, Object?> j) => TrayItem(
        itemType: j['item_type'] as String? ?? 'platform',
        platformId: (j['platform_id'] as num?)?.toInt(),
        display: j['display'] as String? ?? '',
        metric: j['metric'] as String?,
        label: j['label'] as String?,
        decimals: (j['decimals'] as num?)?.toInt(),
        color: Map<String, Object?>.from((j['color'] as Map?) ?? defaultColor()),
        fontSize: (j['font_size'] as num?)?.toInt() ?? kTrayDefaultFontSize,
        lineMode: j['line_mode'] as String? ?? 'two',
        align: j['align'] as String? ?? 'left',
        alignRow2: j['align_row2'] as String?,
        enabled: j['enabled'] as bool? ?? true,
        order: (j['order'] as num?)?.toInt() ?? 0,
      );

  Map<String, Object?> toJson() => {
        'item_type': itemType,
        'platform_id': platformId,
        'display': display,
        'metric': metric,
        'label': label,
        'decimals': decimals,
        'color': color,
        'font_size': fontSize,
        'line_mode': lineMode,
        'align': align,
        'align_row2': alignRow2,
        'enabled': enabled,
        'order': order,
      };

  TrayItem copyWith({
    String? display,
    String? metric,
    String? label,
    int? decimals,
    Map<String, Object?>? color,
    int? fontSize,
    String? lineMode,
    String? align,
    String? alignRow2,
    bool? enabled,
    int? order,
  }) =>
      TrayItem(
        itemType: itemType,
        platformId: platformId,
        display: display ?? this.display,
        metric: metric ?? this.metric,
        label: label ?? this.label,
        decimals: decimals ?? this.decimals,
        color: color ?? this.color,
        fontSize: fontSize ?? this.fontSize,
        lineMode: lineMode ?? this.lineMode,
        align: align ?? this.align,
        alignRow2: alignRow2 ?? this.alignRow2,
        enabled: enabled ?? this.enabled,
        order: order ?? this.order,
      );
}

/// 今日统计摘要（`types/manual.ts::TodayStats`）。
class TodayStats {
  const TodayStats({
    required this.tokens,
    required this.cacheRate,
    required this.cost,
    required this.totalRequests,
  });

  final int tokens;
  final double cacheRate;
  final double cost;
  final int totalRequests;

  /// React 的 `todayStats ?? { tokens: 0, cache_rate: 0, cost: 0, total_requests: 0 }`。
  static const zero = TodayStats(tokens: 0, cacheRate: 0, cost: 0, totalRequests: 0);

  factory TodayStats.fromJson(Map<String, Object?> j) => TodayStats(
        tokens: (j['tokens'] as num?)?.toInt() ?? 0,
        cacheRate: (j['cache_rate'] as num?)?.toDouble() ?? 0,
        cost: (j['cost'] as num?)?.toDouble() ?? 0,
        totalRequests: (j['total_requests'] as num?)?.toInt() ?? 0,
      );
}

/// 展示项的预览文本。标签留空时用自动标签。
({String label, String value}) computeItemText(
  TrayItem item,
  ({String name, double balance, String? codingPlan})? platform,
  TodayStats? todayStats,
  String Function(String key, String fallback) t,
) {
  // 票 I15 的两段由后端按实时路由算（当前命中平台 / 该平台是否在高峰窗口），
  // 前端拿不到这两个值，预览里只占位。
  if (item.itemType == 'routed_platform' || item.itemType == 'peak') {
    final auto = item.itemType == 'peak'
        ? t('tray.segment.peak', '高峰指示')
        : t('tray.segment.routed', '当前命中平台');
    final lbl = item.label;
    return (label: lbl != null && lbl.isNotEmpty ? lbl : auto, value: '—');
  }
  if (item.itemType == 'today_usage') {
    final s = todayStats ?? TodayStats.zero;
    final auto = switch (item.metric ?? 'tokens') {
      'cache_rate' => (
          label: t('tray.metric.cache_rate', 'Cache'),
          value: '${s.cacheRate.toStringAsFixed(0)}%'
        ),
      'cost' => (
          label: t('tray.metric.cost', '花费'),
          value: '\$${trimZeros(s.cost.toStringAsFixed(item.decimals ?? 5))}'
        ),
      'requests' => (label: t('tray.metric.requests', '请求'), value: '${s.totalRequests}'),
      _ => (label: t('tray.metric.today', '今日'), value: '${s.tokens} tok'),
    };
    final lbl = item.label;
    return (label: lbl != null && lbl.isNotEmpty ? lbl : auto.label, value: auto.value);
  }
  if (platform == null) {
    final lbl = item.label;
    return (
      label: lbl != null && lbl.isNotEmpty ? lbl : '#${item.platformId}',
      value: '--.--',
    );
  }
  var isCoding = item.display == 'coding';
  var util = 0.0;
  final cp = platform.codingPlan;
  if (cp != null && cp.isNotEmpty) {
    // 解析失败就当没有 coding plan（React 里是 `catch { /* */ }`）。
    final tiers = _firstTierUtilization(cp);
    if (tiers != null) {
      isCoding = true;
      util = tiers;
    }
  }
  final autoValue = isCoding
      ? '${(100 - util).clamp(0, double.infinity).toStringAsFixed(0)}%'
      : '\$${trimZeros(platform.balance.toStringAsFixed(2))}';
  final lbl = item.label;
  return (label: lbl != null && lbl.isNotEmpty ? lbl : platform.name, value: autoValue);
}

/// 取 coding plan 的首档使用率。解析不出来就当没有 coding plan
/// （React 那边是 `try { ... } catch { /* */ }`，同样静默）。
double? _firstTierUtilization(String json) {
  try {
    final p = jsonDecode(json);
    if (p is! Map) return null;
    final tiers = p['tiers'];
    if (tiers is! List || tiers.isEmpty) return null;
    final first = tiers.first;
    if (first is! Map) return 0;
    return (first['est_utilization'] as num?)?.toDouble() ?? 0;
  } catch (_) {
    return null;
  }
}

// ── 票 I15：菜单栏固定三段，配置降成「最多挑 3 项」 ──────────
//
// 与 React `src/pages/TrayConfigTab.tsx` 和 Rust `TRAY_MAX_SEGMENTS` 同一套数据模型。

/// 菜单栏最多画几段。
const int kTrayMaxSegments = 3;

/// 一个可挑选的展示段。key 只在 UI 侧用来对号，入库的仍是 [TrayItem]。
typedef TraySegmentOption = ({String key, String label, TrayItem item});

/// 某个已存配置项对应的段 key。
String traySegmentKey(TrayItem item) => switch (item.itemType) {
      'today_usage' => 'today_usage:${item.metric ?? 'tokens'}',
      'platform' => 'platform:${item.platformId}',
      _ => item.itemType,
    };

/// 候选段清单：今日统计 4 项 + 当前命中平台 + 高峰指示 + 每个已启用平台。
List<TraySegmentOption> traySegmentOptions(
  List<({int id, String name, double balance, String? codingPlan})> platforms,
  String Function(String key, String fallback) t,
) =>
    [
      for (final m in kTrayTodayMetrics)
        (
          key: 'today_usage:$m',
          label: '${t('tray.todayUsage', '今日消耗')} — ${t('tray.metric.$m', m)}',
          item: TrayItem.todayUsage(m, 0),
        ),
      (
        key: 'routed_platform',
        label: t('tray.segment.routed', '当前命中平台'),
        item: TrayItem.simple('routed_platform', 0),
      ),
      (
        key: 'peak',
        label: t('tray.segment.peak', '高峰指示'),
        item: TrayItem.simple('peak', 0),
      ),
      for (final p in platforms)
        (
          key: 'platform:${p.id}',
          label: p.name,
          item: TrayItem.platform(p.id, 'balance', 0),
        ),
    ];

/// 勾选 / 取消一个段。**只翻 enabled，不删项**（票 I15 的迁移规则）：
/// 取消勾选的项留在 items 里，标签 / 颜色 / 行模式原样保留，勾回来还是老样子。
/// 已满 3 段时继续勾 → 原样返回（调用方把按钮置灰）。
List<TrayItem> toggleTraySegment(List<TrayItem> items, TraySegmentOption opt) {
  final idx = items.indexWhere((it) => traySegmentKey(it) == opt.key);
  final enabledCount = items.where((it) => it.enabled).length;
  List<TrayItem> next;
  if (idx >= 0 && items[idx].enabled) {
    next = [
      for (var i = 0; i < items.length; i++)
        i == idx ? items[i].copyWith(enabled: false) : items[i],
    ];
  } else if (enabledCount >= kTrayMaxSegments) {
    return items;
  } else if (idx >= 0) {
    next = [
      for (var i = 0; i < items.length; i++)
        i == idx ? items[i].copyWith(enabled: true) : items[i],
    ];
  } else {
    next = [...items, opt.item];
  }
  // enabled 的排在前面（保持原相对顺序），order 交给 withOrders 按下标重写。
  return [
    ...next.where((it) => it.enabled),
    ...next.where((it) => !it.enabled),
  ];
}

class TrayController {
  TrayController({InvokeFn? invoke, this.onChanged}) : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  /// 只保留 enabled 的平台。
  List<({int id, String name, double balance, String? codingPlan})> platforms = const [];
  String separator = '  ';
  List<TrayItem> items = const [];
  TodayStats? todayStats;
  bool loading = true;
  String message = '';

  void _notify() => onChanged?.call();

  Future<void> load() async {
    try {
      final results = await Future.wait([
        _invoke('platform_list'),
        _invoke('tray_config_get'),
        _invoke('tray_today_stats'),
      ]);
      final list = results[0];
      platforms = (list is List ? list : const [])
          .whereType<Map>()
          .where((p) => p['enabled'] == true)
          .map((p) => (
                id: (p['id'] as num).toInt(),
                name: '${p['name']}',
                balance: (p['est_balance_remaining'] as num?)?.toDouble() ?? 0,
                codingPlan: p['est_coding_plan'] as String?,
              ))
          .toList();
      final cfg = _map(results[1]);
      separator = cfg['separator'] as String? ?? '  ';
      items = (cfg['items'] as List? ?? const [])
          .map((e) => TrayItem.fromJson(Map<String, Object?>.from(e as Map)))
          .toList();
      todayStats = TodayStats.fromJson(_map(results[2]));
    } catch (_) {/* console.error；页面留空 */}
    loading = false;
    _notify();
  }

  /// 今日统计随 proxy-log 事件刷新。失败静默（刷新失败不该把旧数字换成错误）。
  Future<void> refreshStats() async {
    try {
      todayStats = TodayStats.fromJson(_map(await _invoke('tray_today_stats')));
      _notify();
    } catch (_) {/* */}
  }

  /// order 按下标重写 —— 不这么做，拖拽后的顺序在后端就是乱的。
  static List<TrayItem> withOrders(List<TrayItem> src) =>
      [for (var i = 0; i < src.length; i++) src[i].copyWith(order: i)];

  Future<void> persist(List<TrayItem> next, {String? sep}) async {
    items = withOrders(next);
    if (sep != null) separator = sep;
    _notify();
    try {
      await _invoke('tray_config_set', {
        'config': {'separator': separator, 'items': items.map((e) => e.toJson()).toList()},
      });
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  /// 勾选 / 取消一个段（票 I15 起这是托盘页唯一的编辑动作）。
  Future<void> toggleSegment(TraySegmentOption opt) =>
      persist(toggleTraySegment(items, opt));

  /// 当前启用的段，按 order 排。
  List<TrayItem> get selected =>
      [...items.where((i) => i.enabled)]..sort((a, b) => a.order - b.order);

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}
