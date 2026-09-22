/// 入场错峰与悬停抬升（第三梯队 / 票 29，2026-09-22）。
///
/// React 侧是 `useReveal(delayMs)` + `.reveal` / `.hover-lift` 两条 CSS
/// （`globals.css:969-975,1004-1008`）；这里是两个包装 widget。
/// 关键是**系统开了「减少动态效果」时直接到位不动画** —— 与 React 的
/// `@media (prefers-reduced-motion)`（`globals.css:1033,1051`）同一条判据。
library;

import 'package:aidog_flutter/pages.dart';
import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('Reveal：延迟到点前透明，到点后淡入到位', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Reveal(delayMs: 120, child: Text('x')),
      ),
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
      const MaterialApp(home: Center(child: HoverLift(child: Text('x')))),
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
}
