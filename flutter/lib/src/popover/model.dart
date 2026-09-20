/// 托盘小窗的纯数据层（票 I11）—— 对齐 `src/components/PopoverCards.tsx` 里
/// 不含 JSX 的那一半：尺寸归一、颜色解析、统计查询构造、二维网格分行。
///
/// 全部是纯函数，不碰 widget，也不碰传输层 —— 卡片渲染与查询口径的对齐证据
/// 都落在这一层的单测上（`test/popover/model_test.dart`）。
///
/// **不建第二份 schema**：后端结构整份读整份用（与 `popover_logic.dart` 同 idiom），
/// 字段名从 `src/services/api/types/generated/` 抄。
library;

import 'dart:ui' show Color;

import '../shell/theme.dart' show AidogColors;

/// 卡片尺寸 / 内容密度。缺省与非法值 → [PopoverSize.m]（对齐 `normSize`）。
enum PopoverSize { s, m, l }

PopoverSize normPopoverSize(Object? raw) => switch ('${raw ?? ''}') {
  's' => PopoverSize.s,
  'l' => PopoverSize.l,
  _ => PopoverSize.m,
};

/// 一天的毫秒数（React `DAY_MS`）。
const int kDayMs = 86400000;

/// 需要发统计查询的卡片类型（其余卡走 `popover_data` / `group_detail_list`）。
/// 镜像 `PopoverCards.tsx::STATS_ITEM_TYPES`。
const Set<String> kPopoverStatsItemTypes = {
  'cost_trend',
  'platform_metric',
  'platform_share',
  'hour_heatbar',
  'group_cost',
  'group_tokens',
  'group_requests',
};

/// 强制「今日」窗的三类（`hour_heatbar` 固定 24 个 hourly 桶）。
const Set<String> kPopoverTodayOnlyTypes = {
  'group_tokens',
  'group_requests',
  'hour_heatbar',
};

/// `item.color` → 数值上色。`follow` / 缺省 / 非法 → null（继承主题前景色）。
///
/// React 的 preset 三色走 CSS 变量 `--status-{error,success,warning}`，
/// 本层换成同语义的 token（`bad` / `ok` / `peak`）—— 色值只来自主题，零字面量。
Color? popoverValueColor(Object? rawColor, AidogColors c) {
  if (rawColor is! Map) return null;
  final mode = '${rawColor['mode'] ?? ''}';
  final value = '${rawColor['value'] ?? ''}';
  if (mode == 'preset') {
    return switch (value) {
      'red' => c.bad,
      'green' => c.ok,
      'orange' => c.peak,
      _ => null,
    };
  }
  if (mode == 'custom') {
    final hex = value.trim().replaceFirst(RegExp('^#'), '');
    if (hex.length == 6) {
      final v = int.tryParse(hex, radix: 16);
      if (v != null) return Color(0xFF000000 | v);
    }
  }
  return null;
}

/// 平台余额条目（`entries`）的圆点 / 数值色。与 [popoverValueColor] 同规则，
/// 只是 follow / 非法时回落到主题前景色而不是 null（React `resolveColor` 的语义）。
Color popoverEntryColor(Object? rawColor, AidogColors c) =>
    popoverValueColor(rawColor, c) ?? c.fg;

/// scope / time_window → StatsQuery。对齐 `buildTrendQuery`：
/// today = 本地午夜起 + hourly；7d / 30d = now − N 天 + daily。
///
/// [now] 仅测试传，生产用当前时刻。
Map<String, Object?> buildPopoverTrendQuery(
  Map<String, Object?> item, {
  DateTime? now,
}) {
  final n = now ?? DateTime.now();
  final nowMs = n.millisecondsSinceEpoch;
  final window = '${item['time_window'] ?? '7d'}';
  final int start;
  final String granularity;
  if (window == 'today') {
    start = DateTime(n.year, n.month, n.day).millisecondsSinceEpoch;
    granularity = 'hourly';
  } else {
    start = nowMs - (window == '30d' ? 30 : 7) * kDayMs;
    granularity = 'daily';
  }
  final q = <String, Object?>{
    'start': start,
    'end': nowMs,
    'granularity': granularity,
  };
  final scope = '${item['scope'] ?? 'overall'}';
  final ref = item['scope_ref'];
  if (scope == 'group' && ref != null && '$ref'.isNotEmpty) {
    q['filter_group'] = '$ref';
  } else if (scope == 'platform' && ref != null && '$ref'.isNotEmpty) {
    q['filter_platform'] = '$ref';
  }
  return q;
}

/// 单卡的查询参数；不需要查询 → null。对齐 `buildItemQuery`。
Map<String, Object?>? buildPopoverItemQuery(
  Map<String, Object?> item, {
  DateTime? now,
}) {
  final type = '${item['item_type'] ?? ''}';
  if (!kPopoverStatsItemTypes.contains(type)) return null;
  final eff = kPopoverTodayOnlyTypes.contains(type)
      ? {...item, 'time_window': 'today'}
      : item;
  final q = buildPopoverTrendQuery(eff, now: now);
  // 环形卡按平台拆序列，前端再逐序列聚合成占比。
  if (type == 'platform_share') q['series_by'] = 'platform';
  return q;
}

/// 一帧所需的全部统计查询，`itemIds` 与 `queries` 平行（顺序对齐，供批量 IPC）。
/// 对齐 `collectStatsQueries`：**只收 visible 的卡**。
({List<String> itemIds, List<Map<String, Object?>> queries})
popoverStatsQueries(Map<String, Object?> config, {DateTime? now}) {
  final itemIds = <String>[];
  final queries = <Map<String, Object?>>[];
  for (final item in popoverItems(config)) {
    if (item['visible'] != true) continue;
    final q = buildPopoverItemQuery(item, now: now);
    if (q == null) continue;
    itemIds.add('${item['id'] ?? ''}');
    queries.add(q);
  }
  return (itemIds: itemIds, queries: queries);
}

/// `config.items` 取成 Map 列表（缺省 / 类型不符 → 空）。
List<Map<String, Object?>> popoverItems(Map<String, Object?> config) =>
    (config['items'] as List? ?? const [])
        .whereType<Map>()
        .map(Map<String, Object?>.from)
        .toList(growable: false);

/// 二维网格分行。对齐 `renderGrid`：
/// - 只取 visible 的卡；
/// - 行号 = `row ?? order`（老配置各占一行）；
/// - 行内按 `order` 升序，行间按行号升序；
/// - 每行列数 = `config.rows[row].cols`，缺省 / 越界 = 1。
List<({int row, int cols, List<Map<String, Object?>> items})> popoverRows(
  Map<String, Object?> config,
) {
  final rowMap = <int, List<Map<String, Object?>>>{};
  for (final item in popoverItems(config)) {
    if (item['visible'] != true) continue;
    final row = (item['row'] as num?)?.toInt() ?? (item['order'] as num?)?.toInt() ?? 0;
    (rowMap[row] ??= <Map<String, Object?>>[]).add(item);
  }
  final rows = rowMap.keys.toList()..sort();
  final meta = (config['rows'] as List? ?? const []).whereType<Map>().toList();
  return [
    for (final row in rows)
      (
        row: row,
        cols: row < meta.length
            ? ((meta[row]['cols'] as num?)?.toInt() ?? 1)
            : 1,
        items: rowMap[row]!
          ..sort(
            (a, b) => ((a['order'] as num?)?.toInt() ?? 0)
                .compareTo((b['order'] as num?)?.toInt() ?? 0),
          ),
      ),
  ];
}

/// `StatsResult.overview` 的 token 口径：input + output（不含 cache），
/// 与 `today_stats.tokens` 一致。
double popoverOverviewTokens(Map<String, Object?> overview) =>
    ((overview['total_input_tokens'] as num?)?.toDouble() ?? 0) +
    ((overview['total_output_tokens'] as num?)?.toDouble() ?? 0);

/// 逐序列聚合窗口总花费 → 环形图条目（value ≤ 0 的序列滤掉，与 React 一致）。
List<({String name, double value})> popoverShareEntries(
  Map<String, Object?> stats,
) {
  final series = (stats['series'] as List? ?? const []).whereType<Map>();
  final out = <({String name, double value})>[];
  for (final s in series) {
    final total = (s['buckets'] as List? ?? const [])
        .whereType<Map>()
        .fold<double>(
          0,
          (sum, b) => sum + ((b['total_cost'] as num?)?.toDouble() ?? 0),
        );
    if (total > 0) out.add((name: '${s['name'] ?? ''}', value: total));
  }
  return out;
}
