/// 操作提示必须浮在窗口顶部居中，不跟着页面滚。
///
/// 回归 2026-09-22：原先 `ToastBar` 是页面内容里的一条整宽条，排在最下面
/// （`platforms.dart` 的 children 末尾）。平台一多，「已保存」「测试失败」
/// 这类提示直接落在屏幕外 —— 不是「位置不一样」，是**看不到**。
/// React 那边是 `createPortal` 到 body 的固定定位胶囊
/// （`PlatformListView.tsx:259-273`）。
library;

import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';


void main() {
  testWidgets('提示浮在窗口顶部居中，不在页面流里', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1000, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      wrapPage(
        Column(
          children: [
            const SizedBox(height: 700),
            const ToastBar(text: '已保存', ok: true),
          ],
        ),
        await makeI18n(tester),
      ),
    );
    await settle(tester);

    final box = tester.getRect(find.text('已保存'));
    // 页内渲染的话 y 会在 700 往下（屏幕外）；浮层渲染则贴在顶部 24 附近。
    expect(box.top, lessThan(80), reason: '必须贴窗口顶部，不跟着页面走');
    expect(box.center.dx, closeTo(500, 60), reason: '窗口宽 1000，要居中');
  });

  testWidgets('成功显 ✓，失败显 ✕（React 有图标，原先 Flutter 没有）', (tester) async {
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(const ToastBar(text: '成功了', ok: true), i18n),
    );
    await settle(tester);
    expect(find.byIcon(Icons.check), findsOneWidget);

    await tester.pumpWidget(
      wrapPage(const ToastBar(text: '失败了', ok: false), i18n),
    );
    await settle(tester);
    expect(find.byIcon(Icons.close), findsOneWidget);
  });
}
