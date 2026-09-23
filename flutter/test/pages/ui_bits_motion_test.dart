/// 入场错峰与悬停抬升（第三梯队 / 票 29，2026-09-22）。
///
/// React 侧是 `useReveal(delayMs)` + `.reveal` / `.hover-lift` 两条 CSS
/// （`globals.css:969-975,1004-1008`）；这里是两个包装 widget。
/// 关键是**系统开了「减少动态效果」时直接到位不动画** —— 与 React 的
/// `@media (prefers-reduced-motion)`（`globals.css:1033,1051`）同一条判据。
library;

import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/shell.dart';

import 'dart:ui' show Canvas, PictureRecorder;

import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('Reveal：延迟到点前透明，到点后淡入到位', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(home: Reveal(delayMs: 120, child: Text('x'))),
    );
    expect(
      tester.widget<AnimatedOpacity>(find.byType(AnimatedOpacity)).opacity,
      0,
    );

    await tester.pump(const Duration(milliseconds: 200));
    expect(
      tester.widget<AnimatedOpacity>(find.byType(AnimatedOpacity)).opacity,
      1,
    );
    await tester.pumpAndSettle();
  });

  testWidgets('减少动态效果时 Reveal 不套动画，直接给 child', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: MediaQuery(
          data: MediaQueryData(disableAnimations: true),
          child: Reveal(delayMs: 500, child: Text('x')),
        ),
      ),
    );
    expect(find.byType(AnimatedOpacity), findsNothing);
    expect(find.text('x'), findsOneWidget);
  });

  testWidgets('HoverLift：指针进入上移，移出复位', (tester) async {
    // 用 Center 把 child 固定成一小块，好让指针能真的移到它外面。
    await tester.pumpWidget(
      const MaterialApp(
        home: Center(child: HoverLift(child: Text('x'))),
      ),
    );
    Offset offsetNow() =>
        tester.widget<AnimatedSlide>(find.byType(AnimatedSlide)).offset;
    expect(offsetNow().dy, 0);

    final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await gesture.addPointer(location: Offset.zero);
    addTearDown(gesture.removePointer);
    await gesture.moveTo(tester.getCenter(find.text('x')));
    await tester.pump();
    expect(offsetNow().dy, lessThan(0));

    await gesture.moveTo(const Offset(1, 1));
    await tester.pump();
    // -0.0 == 0.0 为真，但 Offset 的 == 按位比较认不出来，所以比 dy。
    expect(offsetNow().dy, 0);
    await tester.pumpAndSettle();
  });

  testWidgets('减少动态效果时 HoverLift 不套动画', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: MediaQuery(
          data: MediaQueryData(disableAnimations: true),
          child: HoverLift(child: Text('x')),
        ),
      ),
    );
    expect(find.byType(AnimatedSlide), findsNothing);
  });

  // ── 票 29 第三梯队：平台卡 / 分组页的动效 ──

  testWidgets('SmallButton：水波画在同一张 Material 上，不被底色盖住', (tester) async {
    // SmallButton 要读主题 token，所以得带上 aidog 的 ThemeExtension。
    await tester.pumpWidget(
      MaterialApp(
        theme: aidogThemeData(AidogMode.dark),
        home: Scaffold(
          body: Center(
            child: SmallButton(label: 'x', active: true, onTap: () {}),
          ),
        ),
      ),
    );
    // 底色由 Material 自己画（而不是外面再套一层不透明 Container），
    // 这是水波能被看见的前提 —— 回归的就是「按钮没有水波」那条。
    final m = tester.widget<Material>(
      find.ancestor(of: find.text('x'), matching: find.byType(Material)).first,
    );
    expect(m.color, isNot(Colors.transparent));
    expect(
      find.descendant(
        of: find.byType(Material),
        matching: find.byType(InkWell),
      ),
      findsWidgets,
    );

    // 按下去要真的出现一层水波（InkWell 的 splash 挂在 Material 上）。
    final gesture = await tester.startGesture(tester.getCenter(find.text('x')));
    await tester.pump(const Duration(milliseconds: 60));
    // 按下时 InkWell 会往 Material 上加一层 ink feature；这里只验按下 / 抬起
    // 不抛异常且状态能回到静止（水波本身是绘制层的东西，不适合在这里比像素）。
    await gesture.up();
    await tester.pumpAndSettle();
  });

  // React 的 `<Button>` 默认变体是实心的（`ui/button.tsx:14-16`）。Flutter 这边
  // 原先只有描边一种，主动作（「+ 添加平台」这类）两版长得完全不是一个东西。
  testWidgets('SmallButton filled：底色是 accent，文字是浅色，边是 accentEdge', (
    tester,
  ) async {
    Widget button({required bool filled, bool danger = false}) => MaterialApp(
      theme: aidogThemeData(AidogMode.dark),
      home: Scaffold(
        body: Center(
          child: SmallButton(
            label: 'x',
            filled: filled,
            danger: danger,
            onTap: () {},
          ),
        ),
      ),
    );
    Material materialOf(WidgetTester t) => t.widget<Material>(
      find.ancestor(of: find.text('x'), matching: find.byType(Material)).first,
    );

    await tester.pumpWidget(button(filled: false));
    expect(materialOf(tester).color, Colors.transparent);

    await tester.pumpWidget(button(filled: true));
    final filledMaterial = materialOf(tester);
    expect(filledMaterial.color, AidogColors.dark.accent);
    // 深色下 accent 是近黑（`#101012`，用户 2026-09-23 定），面对窗口底只有
    // 1.05:1 —— 轮廓全靠这圈亮边（压在 accent 上 3.22:1，刚过 3:1）。
    expect(
      (filledMaterial.shape! as RoundedRectangleBorder).side.color,
      AidogColors.dark.accentEdge,
      reason: '实心态的轮廓靠 accentEdge，不能没有边',
    );
    expect(
      tester.widget<Text>(find.text('x')).style!.color,
      AidogColors.light.surface,
    );

    // 破坏性动作的实心态用 bad，对齐 React 的 `variant="destructive"`。
    await tester.pumpWidget(button(filled: true, danger: true));
    expect(materialOf(tester).color, AidogColors.dark.bad);
  });

  // 胶囊多选（分组归属那种）选中与未选中只差一圈边和一点底色；深色下 accent 是
  // 近黑，边用 accent 等于没画，底色白 6% 又到不了 3:1 —— 两者都失效就分不出选没选。
  testWidgets('pill 选中态的边是 accentEdge，不是近黑的 accent', (tester) async {
    Widget button({required bool active}) => MaterialApp(
      theme: aidogThemeData(AidogMode.dark),
      home: Scaffold(
        body: Center(
          child: SmallButton(
            label: 'x',
            pill: true,
            active: active,
            onTap: () {},
          ),
        ),
      ),
    );
    Color edgeOf(WidgetTester t) =>
        ((t
                    .widget<Material>(
                      find
                          .ancestor(
                            of: find.text('x'),
                            matching: find.byType(Material),
                          )
                          .first,
                    )
                    .shape!
                as RoundedRectangleBorder)
            .side
            .color);

    await tester.pumpWidget(button(active: false));
    expect(edgeOf(tester), AidogColors.dark.line);

    await tester.pumpWidget(button(active: true));
    await tester.pumpAndSettle();
    expect(edgeOf(tester), AidogColors.dark.accentEdge);
  });

  testWidgets('pill 态切换走 200ms 过渡，普通态不拖泥带水', (tester) async {
    Widget button({required bool pill}) => MaterialApp(
      theme: aidogThemeData(AidogMode.dark),
      home: Scaffold(
        body: Center(
          child: SmallButton(label: 'x', pill: pill, onTap: () {}),
        ),
      ),
    );
    await tester.pumpWidget(button(pill: true));
    final animated = tester.widget<Material>(
      find.ancestor(of: find.text('x'), matching: find.byType(Material)).first,
    );
    expect(animated.animationDuration, const Duration(milliseconds: 200));

    await tester.pumpWidget(button(pill: false));
    final plain = tester.widget<Material>(
      find.ancestor(of: find.text('x'), matching: find.byType(Material)).first,
    );
    expect(plain.animationDuration, Duration.zero);
  });

  testWidgets('DashedBorder：画的是断续线段，不是整圈实线', (tester) async {
    // 只验它确实按 dash/gap 切段（段数 > 1），不验像素。
    const painter = DashedBorder(color: Color(0xFF000000), radius: 8);
    final recorder = PictureRecorder();
    final canvas = Canvas(recorder);
    var segments = 0;
    painter.paint(
      _CountingCanvas(canvas, () => segments++),
      const Size(100, 40),
    );
    expect(segments, greaterThan(1));
  });
}

/// 数一数 `drawPath` 被调了几次 —— 虚线是多段，实线只有一段。
class _CountingCanvas implements Canvas {
  _CountingCanvas(this._inner, this._onDrawPath);

  final Canvas _inner;
  final void Function() _onDrawPath;

  @override
  void drawPath(Path path, Paint paint) {
    _onDrawPath();
    _inner.drawPath(path, paint);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnsupportedError('只用到 drawPath');
}
