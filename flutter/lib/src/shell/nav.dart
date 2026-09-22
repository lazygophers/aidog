/// 导航信息结构 —— 与 `src/App.tsx` 的 `BASE_NAV`（第 34-65 行）逐条对齐：
/// 10 个顶级项 / 5 个 section / 设置 13 个子页分 5 组 / badge。
/// 结构沿用 React 版，实现不沿用（见票 I02）。
///
/// `test/nav_structure_test.dart` 直接解析 `src/App.tsx` 做零差集比对 ——
/// React 侧加一个设置子页而这里没跟，测试当场红。
library;

import 'package:flutter/widgets.dart';

import 'nav_guard.dart';

/// 折叠子菜单项（设置的子页）。id 用 `<parent>/<sub>` 复合形式。
@immutable
class NavChild {
  const NavChild({required this.id, required this.labelKey, required this.group});

  final String id;
  final String labelKey;

  /// 分组标题 i18n key；**相邻**同 group 的子项归为一节。
  final String group;
}

@immutable
class NavItem {
  const NavItem({
    required this.id,
    required this.icon,
    required this.labelKey,
    this.section,
    this.badge,
    this.children = const [],
  });

  final String id;
  final String icon;
  final String labelKey;

  /// 所属 section（顶级分组）i18n key；相邻同 section 归为一节，节头可折叠。
  final String? section;

  /// 可选未读 badge 计数（> 0 时显示）。
  final int? badge;
  final List<NavChild> children;

  bool get hasChildren => children.isNotEmpty;
}

/// 跨页快捷跳转携带的筛选上下文（平台→日志 / 分组→统计 等）。
/// 字段照抄 `src/components/Sidebar.tsx` 的 `NavContext`。
@immutable
class NavContext {
  const NavContext({
    this.platformId,
    this.platformName,
    this.groupId,
    this.groupKey,
    this.model,
    this.duplicate,
  });

  final int? platformId;
  final String? platformName;
  final String? groupId;
  final String? groupKey;
  final String? model;

  /// 经导航进入平台页时以「复制」（新建态）而非「编辑」打开目标平台。
  final bool? duplicate;

  bool get isEmpty =>
      platformId == null &&
      platformName == null &&
      groupId == null &&
      groupKey == null &&
      model == null &&
      duplicate == null;
}

/// 与 `src/App.tsx::BASE_NAV` 一一对应。顺序也一致（section 靠相邻聚合，换顺序就换分节）。
const List<NavItem> kBaseNav = [
  NavItem(id: 'home', icon: 'home', labelKey: 'nav.home', section: 'nav.section.overview'),
  NavItem(id: 'platforms', icon: 'platforms', labelKey: 'nav.platforms', section: 'nav.section.platform'),
  NavItem(id: 'stats', icon: 'stats', labelKey: 'nav.stats', section: 'nav.section.logStats'),
  NavItem(id: 'logs', icon: 'logs', labelKey: 'nav.logs', section: 'nav.section.logStats'),
  NavItem(id: 'request-log', icon: 'logs', labelKey: 'nav.requestLog', section: 'nav.section.logStats'),
  NavItem(id: 'notifications', icon: 'notifications', labelKey: 'nav.notifications', section: 'nav.section.logStats'),
  NavItem(id: 'skills', icon: 'skills', labelKey: 'nav.skills', section: 'nav.section.extension'),
  NavItem(id: 'mcp', icon: 'mcp', labelKey: 'nav.mcp', section: 'nav.section.extension'),
  NavItem(
    id: 'settings',
    icon: 'settings',
    labelKey: 'nav.settings',
    section: 'nav.section.system',
    children: [
      NavChild(id: 'settings/system', labelKey: 'appSettings.systemTab', group: 'nav.settingsGroup.general'),
      NavChild(id: 'settings/coding_tools', labelKey: 'appSettings.cliIntegrationTab', group: 'nav.settingsGroup.integration'),
      NavChild(id: 'settings/claude', labelKey: 'appSettings.claudeTab', group: 'nav.settingsGroup.integration'),
      NavChild(id: 'settings/codex', labelKey: 'appSettings.codexTab', group: 'nav.settingsGroup.integration'),
      NavChild(id: 'settings/pi', labelKey: 'appSettings.piTab', group: 'nav.settingsGroup.integration'),
      NavChild(id: 'settings/middleware', labelKey: 'appSettings.middlewareTab', group: 'nav.settingsGroup.rules'),
      NavChild(id: 'settings/scheduling', labelKey: 'appSettings.schedulingTab', group: 'nav.settingsGroup.rules'),
      NavChild(id: 'settings/notifications', labelKey: 'appSettings.notificationsTab', group: 'nav.settingsGroup.notification'),
      NavChild(id: 'settings/pricing', labelKey: 'appSettings.modelInfoTab', group: 'nav.settingsGroup.config'),
      NavChild(id: 'settings/tray', labelKey: 'appSettings.trayTab', group: 'nav.settingsGroup.config'),
      NavChild(id: 'settings/popover', labelKey: 'appSettings.popoverTab', group: 'nav.settingsGroup.config'),
      NavChild(id: 'settings/importexport', labelKey: 'appSettings.importExportTab', group: 'nav.settingsGroup.config'),
      NavChild(id: 'settings/mitm', labelKey: 'appSettings.mitmTab', group: 'nav.settingsGroup.config'),
    ],
  ),
  NavItem(id: 'about', icon: 'about', labelKey: 'nav.about', section: 'nav.section.system'),
];

/// 相邻同 key 聚为一节（与 `Sidebar.tsx:237-243` 的聚合规则相同：相邻，不是 groupBy）。
List<({String key, List<T> items})> groupAdjacent<T>(
  List<T> items,
  String Function(T) keyOf,
) {
  final out = <({String key, List<T> items})>[];
  for (final it in items) {
    final k = keyOf(it);
    if (out.isNotEmpty && out.last.key == k) {
      out.last.items.add(it);
    } else {
      out.add((key: k, items: <T>[it]));
    }
  }
  return out;
}

/// 导航状态。React 版是 `App.tsx` 里的两个 useState + `AppSettings.tsx` 的 tab；
/// 这里合成一个 controller，子页后缀保留在 [activeId] 里（如 `settings/claude`）。
class ShellController extends ChangeNotifier {
  ShellController({String initial = 'home', List<NavItem> items = kBaseNav})
      : _activeId = initial,
        // ignore: prefer_initializing_formals -- 私有字段名与参数名不同
        _items = items;

  String _activeId;
  NavContext _context = const NavContext();
  List<NavItem> _items;

  /// 形如 `home` 或 `settings/claude`。
  String get activeId => _activeId;

  /// 顶级页 id（`settings/claude` → `settings`）。
  String get activeTopId => _activeId.split('/').first;

  /// 设置子页 tab（`settings/claude` → `claude`）；裸 `settings` 回退 `system`，
  /// 与 `App.tsx:196` 的 `settingsTab` 同规则。
  String get settingsTab =>
      _activeId.startsWith('settings/') ? _activeId.substring(9) : 'system';

  NavContext get context => _context;
  List<NavItem> get items => _items;

  /// 隐藏菜单：日志关闭去 logs，通知关闭去 notifications（`App.tsx:180-184`）。
  void setVisibility({bool? logEnabled, bool? notifEnabled}) {
    final hidden = <String>{
      if (logEnabled == false) 'logs',
      if (notifEnabled == false) 'notifications',
    };
    _items = kBaseNav.where((n) => !hidden.contains(n.id)).toList(growable: false);
    if (hidden.contains(activeTopId)) _activeId = 'platforms';
    notifyListeners();
  }

  /// 切页。**一律经 [requestNavigation]** —— 脏表单在这里被拦下。
  void navigate(String id, [NavContext? context]) {
    if (id == _activeId && context == null) return;
    // 切页前先收掉键盘焦点。
    //
    // 设置页的输入框是**失焦即提交**（`settings/bits.dart:171-173` 的
    // TextRow / NumberRow，全仓 60 处）。点侧栏切页时输入框不会自己失焦，
    // 而是连着整页一起被销毁 —— 用户刚改的值就这么静默没了，没有任何提示。
    //
    // 在这里收一次焦点，失焦回调照常跑，值落盘之后再换页。改这一处盖住全部 60 个框，
    // 不用改提交时机（改成逐字符提交会让「3」改成「30」的中间态也真的落库），
    // 也不用动 navGuard（同一时刻只允许一个 guard，输入框去抢会把页面自己的挤掉）。
    FocusManager.instance.primaryFocus?.unfocus();
    requestNavigation(() {
      _activeId = id;
      _context = context ?? const NavContext();
      notifyListeners();
    });
  }
}
