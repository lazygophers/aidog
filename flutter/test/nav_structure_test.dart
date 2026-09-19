// 导航结构漂移护栏。
//
// 票 I02 要「沿用 src/components/Sidebar.tsx 的信息结构」：5 个 section、13 个设置子页
// 分 5 组、badge。「沿用」不能靠人肉核对 —— 这条测试直接解析 `src/App.tsx` 的 BASE_NAV
// 做零差集比对。React 侧加一个设置子页而 Dart 侧没跟，测试当场红。
//
// 同 idiom：test/command_names_test.dart（从 startup.rs 取命令名真值源）。

import 'dart:io';

import 'package:aidog_flutter/shell.dart';
import 'package:flutter_test/flutter_test.dart';

/// 从 flutter/ 往上找仓库根（有 src-tauri/ 的那一层）。
Directory repoRoot() {
  var dir = Directory.current.absolute;
  while (true) {
    if (Directory('${dir.path}/src-tauri').existsSync()) return dir;
    final parent = dir.parent;
    if (parent.path == dir.path) {
      throw StateError('找不到仓库根（从 ${Directory.current.path} 往上找 src-tauri/）');
    }
    dir = parent;
  }
}

/// `src/App.tsx` 里 `const BASE_NAV: NavItem[] = [ ... ];` 那一整块。
String baseNavSource() {
  final src = File('${repoRoot().path}/src/App.tsx').readAsStringSync();
  // 这里不能用 expect：本函数在 main() 里、任何 test() 体之外被调用，
  // flutter_test 会抛 OutsideTestException。解析失败直接抛，堆栈一样指到这行。
  final start = src.indexOf('const BASE_NAV');
  if (start == -1) throw StateError('src/App.tsx 里找不到 BASE_NAV');
  final end = src.indexOf('\n];', start);
  if (end == -1) throw StateError('BASE_NAV 没有以 `\n];` 收尾');
  return src.substring(start, end);
}

void main() {
  final tsx = baseNavSource();

  /// React 侧的顶级项 id，按出现顺序。子页 id 一律带 `/`，用这一条就能把两层分开。
  final tsTop = RegExp(r'id: "([a-z][a-z_-]*)"')
      .allMatches(tsx)
      .map((m) => m.group(1)!)
      .toList();

  /// React 侧的设置子页：`{ id: "settings/xxx", labelKey: "...", group: "..." }`。
  final tsChildren = RegExp(
    r'\{ id: "(settings/[a-z_]+)", labelKey: "([\w.]+)", group: "([\w.]+)" \}',
  ).allMatches(tsx).map((m) => (
        id: m.group(1)!,
        labelKey: m.group(2)!,
        group: m.group(3)!,
      )).toList();

  final dartSettings = kBaseNav.firstWhere((n) => n.id == 'settings');

  test('顶级项 id 与 src/App.tsx 零差集（含顺序）', () {
    expect(tsTop, hasLength(10));
    expect(kBaseNav.map((n) => n.id).toList(), tsTop);
  });

  test('设置子页 13 个，id / labelKey / group 与 React 侧逐条一致', () {
    expect(tsChildren, hasLength(13));
    expect(dartSettings.children, hasLength(13));
    for (var i = 0; i < 13; i++) {
      final ts = tsChildren[i];
      final dart = dartSettings.children[i];
      expect(dart.id, ts.id);
      expect(dart.labelKey, ts.labelKey);
      expect(dart.group, ts.group);
    }
  });

  test('13 个子页分 5 组，组名与组内条数与现状一致', () {
    final groups = groupAdjacent(dartSettings.children, (NavChild c) => c.group);
    expect(groups.map((g) => g.key).toList(), [
      'nav.settingsGroup.general',
      'nav.settingsGroup.integration',
      'nav.settingsGroup.rules',
      'nav.settingsGroup.notification',
      'nav.settingsGroup.config',
    ]);
    expect(groups.map((g) => g.items.length).toList(), [1, 4, 2, 1, 5]);
  });

  test('顶级项按相邻聚合成 5 个 section', () {
    final sections = groupAdjacent(kBaseNav, (NavItem i) => i.section ?? '');
    expect(sections.map((s) => s.key).toList(), [
      'nav.section.overview',
      'nav.section.platform',
      'nav.section.logStats',
      'nav.section.extension',
      'nav.section.system',
    ]);
    expect(sections.map((s) => s.items.length).toList(), [1, 1, 4, 2, 2]);
  });

  test('每个顶级项的 section / labelKey 也和 React 侧对得上', () {
    for (final item in kBaseNav) {
      expect(
        tsx,
        contains('id: "${item.id}"'),
        reason: '${item.id} 在 src/App.tsx 里不存在',
      );
      expect(tsx, contains('labelKey: "${item.labelKey}"'));
      expect(tsx, contains('section: "${item.section}"'));
    }
  });

  test('图标 key 全部有对应的 IconData（不留空图标）', () {
    for (final item in kBaseNav) {
      expect(kNavIcons.containsKey(item.icon), isTrue, reason: item.icon);
    }
  });

  group('ShellController', () {
    test('settingsTab 与 App.tsx:196 同规则：裸 settings 回退 system', () {
      expect(ShellController(initial: 'settings').settingsTab, 'system');
      expect(ShellController(initial: 'settings/claude').settingsTab, 'claude');
      expect(ShellController(initial: 'settings/claude').activeTopId, 'settings');
    });

    test('日志/通知关掉时该项从侧栏消失，且当前页会被赶到 platforms', () {
      final c = ShellController(initial: 'logs');
      c.setVisibility(logEnabled: false);
      expect(c.items.map((n) => n.id), isNot(contains('logs')));
      expect(c.activeId, 'platforms');

      final d = ShellController(initial: 'home');
      d.setVisibility(notifEnabled: false);
      expect(d.items.map((n) => n.id), isNot(contains('notifications')));
      expect(d.activeId, 'home');
    });
  });
}
