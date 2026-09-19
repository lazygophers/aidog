/// 托盘小窗布局配置（`settings/popover`）—— 对齐
/// `src/pages/PopoverConfigTab/usePopoverConfig.ts`。
///
/// 预览数据四路并发：平台当日、今日汇总、分组明细、批量统计。
/// 任一路失败只让那一块空着，不整页报错（预览就是预览）。
library;

import '../invoke.dart';

/// 小窗里的一张卡。字段名照 `gateway/models.rs::PopoverItem` 的 serde。
class PopoverItem {
  const PopoverItem(this.raw);

  final Map<String, Object?> raw;

  String get itemType => '${raw['item_type'] ?? ''}';
  bool get enabled => raw['enabled'] == true;
  int get order => (raw['order'] as num?)?.toInt() ?? 0;

  Map<String, Object?> toJson() => raw;
}

class PopoverController {
  PopoverController({InvokeFn? invoke, this.onChanged}) : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  /// 整份配置。结构由后端定义，这里整份读整份写，不拆字段
  /// —— 拆了就要维护第二份 schema，且漏字段会把用户配置清掉。
  Map<String, Object?> config = const {};
  bool loading = true;
  String message = '';

  /// 预览数据。
  List<Map<String, Object?>> platformToday = const [];
  Map<String, Object?> trayToday = const {};
  List<Map<String, Object?>> groupDetails = const [];
  List<Object?> statsBatch = const [];
  List<({int id, String name})> platforms = const [];
  List<({int id, String name, String groupKey})> groups = const [];

  void _notify() => onChanged?.call();

  Future<void> load() async {
    try {
      config = _map(await _invoke('popover_config_get'));
    } catch (_) {/* 读失败留空配置，页面显示默认布局 */}
    loading = false;
    _notify();
  }

  /// 预览数据各自独立失败。
  Future<void> loadPreview({List<Map<String, Object?>> statsQueries = const []}) async {
    try {
      final r = await _invoke('popover_platform_today');
      platformToday =
          (r is List ? r : const []).whereType<Map>().map(Map<String, Object?>.from).toList();
    } catch (_) {/* */}
    try {
      trayToday = _map(await _invoke('tray_today_stats'));
    } catch (_) {/* */}
    try {
      final r = await _invoke('group_detail_list');
      groupDetails =
          (r is List ? r : const []).whereType<Map>().map(Map<String, Object?>.from).toList();
    } catch (_) {/* */}
    if (statsQueries.isNotEmpty) {
      try {
        // 批量查询：一次 IPC 拉多卡数据，结果顺序与 queries 一一对应（消除 fan-out）。
        final r = await _invoke('stats_query_batch', {'queries': statsQueries});
        statsBatch = r is List ? r : const [];
      } catch (_) {/* */}
    }
    _notify();
  }

  /// 选择器的数据源。
  Future<void> loadPickers() async {
    try {
      final ps = await _invoke('platform_list');
      platforms = (ps is List ? ps : const [])
          .whereType<Map>()
          .map((p) => (id: (p['id'] as num).toInt(), name: '${p['name']}'))
          .toList();
    } catch (_) {/* */}
    try {
      final gs = await _invoke('group_list');
      groups = (gs is List ? gs : const [])
          .whereType<Map>()
          .map((g) => (
                id: (g['id'] as num).toInt(),
                name: '${g['name']}',
                groupKey: '${g['group_key']}',
              ))
          .toList();
    } catch (_) {/* */}
    _notify();
  }

  /// 即时保存：整份写回。
  Future<void> persist(Map<String, Object?> next) async {
    config = next;
    _notify();
    try {
      await _invoke('popover_config_set', {'config': next});
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  /// 卡片列表的 order 按下标重排（与托盘同理，拖拽后不重排顺序在后端就是乱的）。
  static List<Map<String, Object?>> withOrders(List<Map<String, Object?>> items) =>
      [for (var i = 0; i < items.length; i++) {...items[i], 'order': i}];

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}
