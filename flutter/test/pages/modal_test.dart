// 票 11 的护栏：确认弹窗必须是**真浮层**，不是页面里的一张格子。
//
// 判据不看实现（`OverlayPortal` 是手段），只看可观察的两条性质：
//   1. 面板按**整窗**居中 —— 页内卡片跟着内容流走，横向中心不会正好落在窗口中心；
//   2. 面板底下铺着一层全窗遮罩，点它的行为与 React 那边逐个对齐。
import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';

/// 把确认卡放进一张**偏左的窄栏**里：页内渲染时它的中心在左侧，
/// 浮层渲染时它的中心必须回到窗口正中。
Widget _offsetHost(Widget confirm, I18nController c) => MaterialApp(
  theme: aidogThemeData(AidogMode.dark),
  home: AidogI18n(
    controller: c,
    child: Scaffold(
      body: Align(
        alignment: Alignment.topLeft,
        child: SizedBox(width: 200, child: confirm),
      ),
    ),
  ),
);

void main() {
  testWidgets('确认弹窗按整窗居中，不跟着页内位置走', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1000, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final c = await makeI18n(tester);

    await tester.pumpWidget(
      _offsetHost(
        ConfirmCard(
          title: '删除',
          body: '确定删除？',
          confirmLabel: '删除',
          onCancel: () {},
          onConfirm: () {},
        ),
        c,
      ),
    );
    await settle(tester);

    expect(find.byType(ConfirmCard), findsOneWidget);
    expect(find.text('确定删除？'), findsOneWidget);
    // 窗口宽 1000，宿主栏只有 200 宽且贴左；居中说明画的是浮层不是页内格子。
    expect(tester.getCenter(find.text('确定删除？')).dx, closeTo(500, 1));
  });

  testWidgets('破坏性确认点遮罩不关（对齐 React 的 AlertDialog）', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1000, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final c = await makeI18n(tester);
    var cancelled = 0;

    await tester.pumpWidget(
      _offsetHost(
        ConfirmCard(
          title: '删除',
          body: '确定删除？',
          confirmLabel: '删除',
          onCancel: () => cancelled++,
          onConfirm: () {},
        ),
        c,
      ),
    );
    await settle(tester);

    await tester.tapAt(const Offset(20, 20));
    await settle(tester);
    expect(cancelled, 0);
  });

  testWidgets('dismissOnBarrier 的弹窗点遮罩即取消，busy 时不取消', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1000, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final c = await makeI18n(tester);
    var cancelled = 0;

    Widget card({required bool busy}) => ConfirmCard(
      title: '清空',
      body: '确定清空？',
      confirmLabel: '清空',
      busy: busy,
      dismissOnBarrier: true,
      onCancel: () => cancelled++,
      onConfirm: () {},
    );

    await tester.pumpWidget(_offsetHost(card(busy: false), c));
    await settle(tester);
    await tester.tapAt(const Offset(20, 20));
    await settle(tester);
    expect(cancelled, 1);

    await tester.pumpWidget(_offsetHost(card(busy: true), c));
    await settle(tester);
    await tester.tapAt(const Offset(20, 20));
    await settle(tester);
    expect(cancelled, 1, reason: '执行中点遮罩不许关');
  });

  testWidgets('破坏性确认点遮罩不关，但按 Esc 关（Radix AlertDialog 口径）', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1000, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final c = await makeI18n(tester);
    var cancelled = 0;

    await tester.pumpWidget(
      _offsetHost(
        ConfirmCard(
          title: '删除',
          body: '确定删除？',
          confirmLabel: '删除',
          onCancel: () => cancelled++,
          onConfirm: () {},
        ),
        c,
      ),
    );
    await settle(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(cancelled, 1);
  });

  testWidgets('焦点在浮层里的输入框上，按 Esc 照样关得掉', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1000, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final c = await makeI18n(tester);
    var closed = 0;

    await tester.pumpWidget(
      _offsetHost(
        AidogModal(
          onBarrierTap: () => closed++,
          child: const Material(child: TextField()),
        ),
        c,
      ),
    );
    await settle(tester);

    // 先把焦点交给输入框 —— 这正是「加一行绑 Esc」会失手的情形：
    // 键盘事件先到 EditableText，没冒泡上来就关不掉。
    await tester.tap(find.byType(TextField));
    await settle(tester);
    expect(
      tester.widget<TextField>(find.byType(TextField)),
      isNotNull,
      reason: '前置条件：输入框在浮层里',
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(closed, 1);
  });

  testWidgets('两层浮层叠着，Esc 只关最上面那层', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1000, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final c = await makeI18n(tester);
    var outer = 0;
    var inner = 0;

    await tester.pumpWidget(
      _offsetHost(
        AidogModal(
          onBarrierTap: () => outer++,
          child: Material(
            child: AidogModal(
              onBarrierTap: () => inner++,
              child: const Material(child: Text('里层')),
            ),
          ),
        ),
        c,
      ),
    );
    await settle(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(inner, 1);
    expect(outer, 0, reason: '一次 Esc 不许把两层一起关掉');
  });
}
