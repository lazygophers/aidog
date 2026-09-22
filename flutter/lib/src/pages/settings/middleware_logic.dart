/// 中间件规则（`settings/middleware`）—— 对齐
/// `src/components/settings/MiddlewareRules.tsx`。
///
/// 两条容易搬丢的规则：
/// ① **`silent` 刷新**：写操作（启停 / 删除 / 保存）之后重新拉列表时**不置 loading**，
///    否则整张规则列表被「加载中」占位替换一帧，改一条规则的体感变成整页重来。
///    只有首屏才走 loading。
/// ② **`middleware_update_rule` 是全量覆盖**：只翻 enabled 也必须把原规则每个字段
///    带上，漏一个就把它清成默认值。
library;

import '../invoke.dart';

/// 规则列表里的一条。字段名照 `gateway/models.rs::MiddlewareRule` 的 serde（snake_case）。
class MiddlewareRule {
  const MiddlewareRule(this.raw);

  /// 原始 map —— 更新时要整份回传，所以保留全字段而不是挑几个解出来。
  final Map<String, Object?> raw;

  int get id => (raw['id'] as num).toInt();
  String get name => '${raw['name'] ?? ''}';
  String get description => '${raw['description'] ?? ''}';
  bool get enabled => raw['enabled'] == true;
  bool get isBuiltin => raw['is_builtin'] == true;
  int get priority => (raw['priority'] as num?)?.toInt() ?? 0;

  /// 旧模型残留、引擎**翻译不了也不执行**的规则
  /// （`generated/MiddlewareRule.ts:12-16`：「前端展示失败态引导手删」）。
  ///
  /// 后端一直在发这个字段，Flutter 这边原先没解析 —— 后果是失效规则长得和正常规则
  /// 一模一样，用户还能点进一个引擎根本不跑的编辑表单去改，改完当然也不生效。
  bool get failed => raw['failed'] == true;

  /// 全量覆盖用的入参（字段集与 React `handleToggle` 逐条一致）。
  Map<String, Object?> toUpdateInput({bool? enabled}) => {
        'id': raw['id'],
        'name': raw['name'],
        'description': raw['description'],
        'conditions': raw['conditions'],
        'actions': raw['actions'],
        'applies_to': raw['applies_to'],
        'priority': raw['priority'],
        'enabled': enabled ?? raw['enabled'],
      };
}

class MiddlewareController {
  MiddlewareController({InvokeFn? invoke, this.onChanged}) : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  List<MiddlewareRule> rules = const [];

  /// rule_id → 预算窗口状态。
  Map<int, Map<String, Object?>> budgets = const {};

  /// 中间件总开关（`MiddlewareSettings { enabled }`）。
  bool settingsEnabled = true;

  bool loading = true;
  String error = '';

  /// 规则表单：null = 没开；空 map = 新建；有 id = 编辑。
  Map<String, Object?>? editingRule;
  bool showForm = false;

  /// 「适用范围」编辑器的下拉数据源。
  List<({int id, String name})> platforms = const [];
  List<({int id, String name, String groupKey})> groups = const [];

  void _notify() => onChanged?.call();

  /// [silent] = 写操作后的刷新，不置 loading。
  Future<void> load({bool silent = false}) async {
    if (!silent) {
      loading = true;
      _notify();
    }
    try {
      final all = await _invoke('middleware_list_rules');
      rules = (all is List ? all : const [])
          .whereType<Map>()
          .map((e) => MiddlewareRule(Map<String, Object?>.from(e)))
          .toList();
      // 预算状态与规则列表同批刷新；失败不影响列表渲染。
      try {
        final st = await _invoke('middleware_budget_status');
        budgets = {
          for (final b in (st is List ? st : const []).whereType<Map>())
            (b['rule_id'] as num).toInt(): Map<String, Object?>.from(b),
        };
      } catch (_) {/* console.error；列表照常渲染 */}
    } catch (e) {
      error = '$e';
    } finally {
      loading = false;
      _notify();
    }
  }

  Future<void> loadSettings() async {
    try {
      final s = await _invoke('middleware_settings_get');
      settingsEnabled = s is Map ? (s['enabled'] as bool? ?? true) : true;
    } catch (_) {
      settingsEnabled = true;
    }
    _notify();
  }

  /// 总开关：先改本地再写回，失败不回滚（与 React 的 `persist` 同形）。
  Future<void> setSettingsEnabled(bool v) async {
    settingsEnabled = v;
    _notify();
    try {
      await _invoke('middleware_settings_set', {
        'settings': {'enabled': v},
      });
    } catch (e) {
      error = '$e';
      _notify();
    }
  }

  /// 「适用范围」下拉的数据源。两个都失败静默（React 里都是 `.catch(() => {})`）。
  Future<void> loadAppliesToOptions() async {
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

  void openCreate() {
    editingRule = null;
    showForm = true;
    _notify();
  }

  void openEdit(MiddlewareRule r) {
    editingRule = r.raw;
    showForm = true;
    _notify();
  }

  void closeForm() {
    showForm = false;
    editingRule = null;
    _notify();
  }

  /// 保存草稿：有 editingRule 就更新，否则新建。成功后 silent 刷新。
  Future<void> save(Map<String, Object?> draft) async {
    final editing = editingRule;
    if (editing != null) {
      await _invoke('middleware_update_rule', {
        'input': {...draft, 'id': editing['id']},
      });
    } else {
      await _invoke('middleware_create_rule', {'input': draft});
    }
    showForm = false;
    editingRule = null;
    await load(silent: true);
  }

  /// 启停一条规则。整份回传，只翻 enabled。
  Future<void> toggleRule(MiddlewareRule r) async {
    try {
      await _invoke('middleware_update_rule', {
        'input': r.toUpdateInput(enabled: !r.enabled),
      });
      await load(silent: true);
    } catch (e) {
      error = '$e';
      _notify();
    }
  }

  /// **破坏性**：删一条规则，调用方先弹确认框。
  Future<void> deleteRule(int id) async {
    try {
      await _invoke('middleware_delete_rule', {'id': id});
      await load(silent: true);
    } catch (e) {
      error = '$e';
      _notify();
    }
  }
}
