/// 浮窗二维布局的纯函数层（I17）—— 对齐
/// `src/pages/PopoverConfigTab/utils.ts`（normalizeConfig / effRow / makeItem）
/// 与 `usePopoverConfig.ts::moveItemToRow`。
///
/// 全部纯函数：布局编辑器（卡片拖拽 / 行列数）的语义在这里，widget 只画；
/// 单测直接钉行为（`test/settings/popover_layout_test.dart`）。
/// config 整份读整份写（`items` + `rows[{cols}]`），字段名照
/// `src/services/api/types/generated/` 的 serde 投影。
library;

/// 每行列数上限（React `constants.ts::MAX_COLS`）。
const int kPopoverMaxCols = 3;

/// `utils.ts::effRow`：row 缺省回退 order（与渲染层一致）。
int effRow(Map<String, Object?> item) =>
    (item['row'] as num?)?.toInt() ?? (item['order'] as num?)?.toInt() ?? 0;

/// `utils.ts::makeItem`：按类型带缺省值的新卡片。
Map<String, Object?> makePopoverItem(
  String type,
  int order,
  int row, {
  List<({int id, String name})> platforms = const [],
  List<({int id, String name, String groupKey})> groups = const [],
}) {
  final base = <String, Object?>{
    'id': 'popover-$type-${DateTime.now().microsecondsSinceEpoch}',
    'item_type': type,
    'visible': true,
    'order': order,
    'row': row,
    'size': 'm',
    'color': {'mode': 'follow', 'value': ''},
  };
  switch (type) {
    case 'cost_trend':
      return {
        ...base,
        'scope': 'overall',
        'scope_ref': null,
        'time_window': '7d',
      };
    case 'platform_share':
      return {...base, 'time_window': '7d'};
    case 'hour_heatbar':
      return {...base, 'time_window': 'today'};
    case 'platform_metric':
      return {
        ...base,
        'scope': 'platform',
        'scope_ref': platforms.isEmpty ? null : '${platforms.first.id}',
        'time_window': 'today',
      };
  }
  const groupTypes = {
    'group_cost',
    'group_tokens',
    'group_requests',
    'group_balance',
  };
  if (groupTypes.contains(type)) {
    final ref = groups.isEmpty ? null : groups.first.groupKey;
    return {
      ...base,
      'scope': 'group',
      'scope_ref': ref,
      if (type == 'group_cost') 'time_window': '7d',
      if (type == 'group_tokens' || type == 'group_requests')
        'time_window': 'today',
    };
  }
  return base;
}

/// `utils.ts::normalizeConfig`：items 按 row 升序分组、行号连续从 0、行内按
/// order 排，row/order 重写为规范值；`rows[]` 与之对齐（已有列数保留，缺省 1）。
/// 幂等。
Map<String, Object?> normalizePopoverConfig(
  List<Map<String, Object?>> items,
  List<Map<String, Object?>>? rows,
) {
  final byRow = <int, List<Map<String, Object?>>>{};
  for (final it in items) {
    (byRow[effRow(it)] ??= []).add(it);
  }
  final rowNums = byRow.keys.toList()..sort();
  final nextItems = <Map<String, Object?>>[];
  final nextRows = <Map<String, Object?>>[];
  final meta = rows ?? const [];
  for (var newRow = 0; newRow < rowNums.length; newRow++) {
    final list = byRow[rowNums[newRow]]!
      ..sort(
        (a, b) => ((a['order'] as num?)?.toInt() ?? 0).compareTo(
          (b['order'] as num?)?.toInt() ?? 0,
        ),
      );
    for (var idx = 0; idx < list.length; idx++) {
      nextItems.add({...list[idx], 'row': newRow, 'order': idx});
    }
    final cols = newRow < meta.length
        ? ((meta[newRow]['cols'] as num?)?.toInt() ?? 1)
        : 1;
    nextRows.add({'cols': cols});
  }
  return {'items': nextItems, 'rows': nextRows};
}

/// `usePopoverConfig.ts::moveItemToRow`：把 [activeId] 那张卡挪到 [targetRow]
/// 的 [beforeId] 之前（null = 行尾），order 在目标行内重写。
/// 同行同位（beforeId 是自己或 null 且本来就在行尾位置语义不变）时原样返回。
List<Map<String, Object?>> movePopoverItemToRow(
  List<Map<String, Object?>> items,
  String activeId,
  int targetRow,
  String? beforeId,
) {
  final activeIdx = items.indexWhere((i) => '${i['id']}' == activeId);
  if (activeIdx < 0) return items;
  final active = items[activeIdx];
  if (effRow(active) == targetRow &&
      (beforeId == null || beforeId == activeId)) {
    return items;
  }
  final rest = items.where((i) => '${i['id']}' != activeId).toList()
    ..sort(
      (a, b) => ((a['order'] as num?)?.toInt() ?? 0).compareTo(
        (b['order'] as num?)?.toInt() ?? 0,
      ),
    );
  final targetItems = rest.where((i) => effRow(i) == targetRow).toList();
  var insertIdx = targetItems.length;
  if (beforeId != null) {
    final bi = targetItems.indexWhere((i) => '${i['id']}' == beforeId);
    if (bi >= 0) insertIdx = bi;
  }
  targetItems.insert(insertIdx, {...active, 'row': targetRow});
  final updated = <String, Map<String, Object?>>{};
  for (var idx = 0; idx < targetItems.length; idx++) {
    updated['${targetItems[idx]['id']}'] = {
      ...targetItems[idx],
      'row': targetRow,
      'order': idx,
    };
  }
  return [
    for (final i in items)
      updated['${i['id']}'] ??
          ('${i['id']}' == activeId ? {...i, 'row': targetRow} : i),
  ];
}
