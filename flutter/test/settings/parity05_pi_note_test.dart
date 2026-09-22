/// `PiUnsupportedNote` 的 widget 测试 —— 逐条翻自 React 的
/// `src/components/shared/PiUnsupportedNote.test.tsx`，**期望值一字不改**：
///   1. 渲染标题 key 与调用方传入的原因 key
///   2. 原因 key 逐调用点独立（MCP / Hooks / cc-switch 三处不串味）
///
/// React 那份断言的是 **key 本身**（它的测试 i18n 实例缺 key 时回落到 key）。
/// Dart 侧 `t.t()` 缺 key 时同样返回 key（`translations.dart:75`），所以拿
/// 真资产里不存在的假 key 做断言，两边语义一致。
///
/// 一处形态差异：React 断言 `img[alt="pi"]`（pi.svg 资产）；Flutter 外壳没有
/// 这份资产，用同尺寸的 Icon 占位，故改断言图标在不在。
library;

import 'package:aidog_flutter/src/pages/ui_bits.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';

void main() {
  Future<void> mount(WidgetTester tester, String reasonKey) async {
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        PiUnsupportedNote(reasonKey: reasonKey, reasonFallback: 'pi 没有 MCP 协议'),
        i18n,
      ),
    );
    await settle(tester);
  }

  testWidgets('渲染 pi 图标、标题 key 与调用方传入的原因 key', (tester) async {
    await mount(tester, 'pi.noMcp');
    expect(find.byIcon(Icons.extension_outlined), findsOneWidget);
    final i18n = await makeI18n(tester);
    expect(
      find.textContaining(i18n.t('pi.unsupportedTitle'), findRichText: true),
      findsOneWidget,
    );
    // 假 key 取不到译文 → 回落到调用方给的 fallback
    expect(
      find.textContaining('pi 没有 MCP 协议', findRichText: true),
      findsOneWidget,
    );
  });

  testWidgets('原因 key 逐调用点独立（MCP / Hooks / cc-switch 三处不串味）', (tester) async {
    // 三处真实 key 各自取到自己的译文，互不相同。
    final i18n = await makeI18n(tester);
    final mcp = i18n.t('pi.unsupportedMcp');
    final hooks = i18n.t('pi.unsupportedHooks');
    final cc = i18n.t('pi.unsupportedCcSwitch');
    expect({mcp, hooks, cc}.length, 3, reason: '三处理由必须各说各的');

    await mount(tester, 'pi.unsupportedHooks');
    expect(find.textContaining(hooks, findRichText: true), findsOneWidget);
    expect(find.textContaining(mcp, findRichText: true), findsNothing);
  });
}
