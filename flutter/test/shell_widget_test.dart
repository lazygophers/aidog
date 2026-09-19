// 骨架的交互：侧栏展开/折叠、设置 13 子页展开、主题切换、以及**离页拦截**。
//
// 离页拦截不是可选项：没有它，正在编辑的表单会在用户点别的导航项时静默丢失。

import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// 最小宿主：只装骨架，页面内容用一个可辨认的占位。
Widget host({
  required ShellController nav,
  required ThemeController theme,
  Widget Function(BuildContext, String)? page,
}) {
  return AidogShellApp(
    controller: nav,
    theme: theme,
    localeLabel: 'zh-Hans',
    pageBuilder: page ?? (context, id) => Text('page:$id'),
  );
}

/// 侧栏当前宽度（AnimatedContainer 停稳后的实际值）。
double railWidth(WidgetTester tester) =>
    tester.getSize(find.byType(Rail)).width;

void main() {
  setUp(resetNavGuardForTest);
  tearDown(resetNavGuardForTest);

  testWidgets('侧栏默认展开 200px，折叠态 56px，来回都对', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1440, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(host(nav: ShellController(), theme: ThemeController()));
    await tester.pumpAndSettle();
    expect(railWidth(tester), AidogLayout.railW);
    expect(AidogLayout.railW, 200);

    // 展开态能看到标签文本。
    expect(find.text('nav.home'), findsOneWidget);

    await tester.tap(find.byKey(const Key('rail-collapse')));
    await tester.pumpAndSettle();
    expect(railWidth(tester), AidogLayout.railWCollapsed);
    expect(AidogLayout.railWCollapsed, 56);
    // 折叠后标签与节头都藏起来，只剩图标。
    expect(find.text('nav.home'), findsNothing);
    expect(find.text('NAV.SECTION.OVERVIEW'), findsNothing);

    await tester.tap(find.byKey(const Key('rail-collapse')));
    await tester.pumpAndSettle();
    expect(railWidth(tester), AidogLayout.railW);
    expect(find.text('nav.home'), findsOneWidget);
  });

  testWidgets('侧栏是格子盘的第 0 列：无圆角、无阴影，只有一条 end 边', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1440, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(host(nav: ShellController(), theme: ThemeController()));
    await tester.pumpAndSettle();

    final box = tester.widget<AnimatedContainer>(
      find.descendant(of: find.byType(Rail), matching: find.byType(AnimatedContainer)).first,
    );
    final deco = box.decoration! as BoxDecoration;
    expect(deco.borderRadius, isNull, reason: '第 0 列不是独立面板，不能有自己的圆角');
    expect(deco.boxShadow, isNull, reason: '第 0 列不浮起来，不能有阴影');
    expect(deco.border, isA<BorderDirectional>());
    expect((deco.border! as BorderDirectional).start, BorderSide.none);
  });

  testWidgets('点设置展开 13 个子页，分 5 组；点子页切到对应 activeId', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1440, 1400));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final nav = ShellController();
    await tester.pumpWidget(host(nav: nav, theme: ThemeController()));
    await tester.pumpAndSettle();

    await tester.tap(find.text('nav.settings'));
    await tester.pumpAndSettle();

    // 13 个子页标签全在。
    final settings = kBaseNav.firstWhere((n) => n.id == 'settings');
    for (final c in settings.children) {
      expect(find.text(c.labelKey), findsOneWidget, reason: c.id);
    }
    // 5 个组头（micro 全大写）。
    for (final g in {for (final c in settings.children) c.group}) {
      expect(find.text(g.toUpperCase()), findsOneWidget, reason: g);
    }

    // 展开时自动跳首个子页。
    expect(nav.activeId, 'settings/system');

    await tester.tap(find.text('appSettings.claudeTab'));
    await tester.pumpAndSettle();
    expect(nav.activeId, 'settings/claude');
    expect(nav.settingsTab, 'claude');
  });

  testWidgets('底部主题按钮切深浅，界面底色跟着换', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1440, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final theme = ThemeController();
    await tester.pumpWidget(host(nav: ShellController(), theme: theme));
    await tester.pumpAndSettle();
    expect(theme.isDark, isTrue);
    expect(find.text('theme.dark'), findsOneWidget);

    await tester.tap(find.byKey(const Key('rail-theme-toggle')));
    await tester.pumpAndSettle();
    expect(theme.isDark, isFalse);
    expect(find.text('theme.light'), findsOneWidget);

    final shell = tester.widget<ColoredBox>(
      find.descendant(of: find.byType(AppShell), matching: find.byType(ColoredBox)).first,
    );
    expect(shell.color, AidogColors.light.bg);
  });

  group('离页拦截', () {
    testWidgets('表单脏时点别的导航项：不切页，交给 guard 裁决', (tester) async {
      await tester.binding.setSurfaceSize(const Size(1440, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      final nav = ShellController();
      await tester.pumpWidget(host(
        nav: nav,
        theme: ThemeController(),
        page: (context, id) => id == 'home'
            ? const _DirtyFormPage()
            : Text('page:$id'),
      ));
      await tester.pumpAndSettle();

      // 表单变脏 → 注册 guard。
      await tester.enterText(find.byType(TextField), '改了一半');
      await tester.pumpAndSettle();
      expect(hasNavGuard, isTrue);

      // 点「AI 平台」：被拦下，页面没动。
      await tester.tap(find.text('nav.platforms'));
      await tester.pumpAndSettle();
      expect(nav.activeId, 'home');
      expect(find.text('丢弃改动？'), findsOneWidget);
      expect(find.byType(TextField), findsOneWidget, reason: '编辑中的内容必须还在');

      // 取消 → 留在原页，输入还在。
      await tester.tap(find.text('取消'));
      await tester.pumpAndSettle();
      expect(nav.activeId, 'home');
      expect(find.text('丢弃改动？'), findsNothing);

      // 再点一次并确认 → guard 调 proceed，这才真的切页。
      await tester.tap(find.text('nav.platforms'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('丢弃'));
      await tester.pumpAndSettle();
      expect(nav.activeId, 'platforms');
      expect(find.text('page:platforms'), findsOneWidget);
    });

    testWidgets('表单干净时不拦：直接切页', (tester) async {
      await tester.binding.setSurfaceSize(const Size(1440, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      final nav = ShellController();
      await tester.pumpWidget(host(
        nav: nav,
        theme: ThemeController(),
        page: (context, id) =>
            id == 'home' ? const _DirtyFormPage() : Text('page:$id'),
      ));
      await tester.pumpAndSettle();
      expect(hasNavGuard, isFalse);

      await tester.tap(find.text('nav.platforms'));
      await tester.pumpAndSettle();
      expect(nav.activeId, 'platforms');
    });

    test('后注册者胜；注销只清掉自己那一个', () {
      var first = 0;
      var second = 0;
      final offFirst = registerNavGuard((p) => first++);
      final offSecond = registerNavGuard((p) => second++);

      requestNavigation(() {});
      expect((first, second), (0, 1));

      // 第一个的注销函数此刻不该清掉第二个。
      offFirst();
      requestNavigation(() {});
      expect((first, second), (0, 2));

      offSecond();
      var proceeded = false;
      requestNavigation(() => proceeded = true);
      expect(proceeded, isTrue);
    });
  });
}

/// 一个会变脏的表单页：脏了就注册 guard，弹自定义确认框（不是原生 confirm）。
class _DirtyFormPage extends StatefulWidget {
  const _DirtyFormPage();

  @override
  State<_DirtyFormPage> createState() => _DirtyFormPageState();
}

class _DirtyFormPageState extends State<_DirtyFormPage> {
  void Function()? _unregister;
  void Function()? _pending;

  @override
  void dispose() {
    _unregister?.call();
    super.dispose();
  }

  void _onChanged(String v) {
    if (v.isEmpty || _unregister != null) return;
    _unregister = registerNavGuard((proceed) {
      setState(() => _pending = proceed);
    });
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        TextField(onChanged: _onChanged),
        if (_pending != null) ...[
          const Text('丢弃改动？'),
          TextButton(
            onPressed: () => setState(() => _pending = null),
            child: const Text('取消'),
          ),
          TextButton(
            onPressed: () {
              final proceed = _pending!;
              _unregister?.call();
              _unregister = null;
              setState(() => _pending = null);
              proceed();
            },
            child: const Text('丢弃'),
          ),
        ],
      ],
    );
  }
}
