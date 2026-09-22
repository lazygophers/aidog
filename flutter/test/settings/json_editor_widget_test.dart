/// 票 27 第 2 步（折叠）：JSON 字段换成 `re_editor` 的 `CodeEditor` 之后，
/// 行号 / 折叠标记 / 语法高亮由包提供，**行内报错仍是自己的**（包只高亮不校验）。
///
/// 输入法预编辑（composing）这一类只有真跑 app 才看得出来，widget 测试照不出，
/// 所以那部分是人工验的，见票 27 的交付说明。这里守的是「装没装上、错没错报」。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:re_editor/re_editor.dart';

import '../pages/harness.dart';
import 'pages_c_widget_test.dart' show baseResponses;

SchemaBundle bundle() => SchemaBundle(
  sections: [
    SchemaSection({
      'id': 'core',
      'labelKey': 'settings.sectionCore',
      'fields': [
        {'key': 'hooks', 'label': 'Hooks', 'type': 'json'},
      ],
    }),
  ],
  recommended: const {},
);

void main() {
  Future<(FakeKernel, I18nController)> mount(
    WidgetTester tester, {
    Map<String, Object?> stored = const {},
  }) async {
    await useBigSurface(tester);
    final k = FakeKernel({
      ...baseResponses(),
      'settings_get': (_) => stored,
      'settings_set': (_) => null,
      'fs_autocomplete': (_) => const [],
    });
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        SchemaConfigPage(
          kind: SchemaConfigKind.claude,
          invoke: k.invoke,
          bundleLoader: (_) async => bundle(),
        ),
        i18n,
      ),
    );
    await settle(tester);
    return (k, i18n);
  }

  testWidgets('json 字段用的是 CodeEditor，带行号与折叠标记', (tester) async {
    await mount(
      tester,
      stored: const {
        'hooks': {
          'a': [1, 2],
        },
      },
    );
    expect(find.byKey(const ValueKey('json-code-editor')), findsOneWidget);
    expect(find.byType(CodeEditor), findsOneWidget);
    // 行号与折叠标记来自 indicatorBuilder —— 这是换包换来的东西，钉住别被改没。
    expect(find.byType(DefaultCodeLineNumber), findsOneWidget);
    expect(find.byType(DefaultCodeChunkIndicator), findsOneWidget);
  });

  testWidgets('存量值以两空格缩进铺进编辑器', (tester) async {
    await mount(
      tester,
      stored: const {
        'hooks': {'a': 1},
      },
    );
    final editor = tester.widget<CodeEditor>(find.byType(CodeEditor));
    expect(editor.controller!.text, '{\n  "a": 1\n}');
  });

  testWidgets('非法 JSON → 报错带行列，不是只说「格式错误」', (tester) async {
    final (_, t) = await mount(tester);
    final editor = tester.widget<CodeEditor>(find.byType(CodeEditor));
    // 第 3 行少个逗号。
    editor.focusNode!.requestFocus();
    await settle(tester);
    editor.controller!.text = '{\n  "a": 1\n  "b": 2\n}';
    await settle(tester);
    // 失焦提交（与手敲完点别处同一条路径）。
    editor.focusNode!.unfocus();
    await settle(tester);

    expect(find.textContaining('L3:'), findsOneWidget);
    // 旧实现只吐一句 Dart 异常串，里面带的是字符偏移不是行号。
    expect(find.textContaining('at character'), findsNothing);
    // 报错时不写回配置：保存按钮不该把半成品灌进去。
    expect(find.text(t.t('action.save')), findsOneWidget);
  });

  testWidgets('改成合法 JSON → 错误消失并写回', (tester) async {
    final (k, t) = await mount(tester);
    final editor = tester.widget<CodeEditor>(find.byType(CodeEditor));
    editor.focusNode!.requestFocus();
    await settle(tester);
    editor.controller!.text = '{\n  "a": 1\n}';
    await settle(tester);
    editor.focusNode!.unfocus();
    await settle(tester);

    expect(find.textContaining('L'), findsNothing);
    await tester.tap(find.text(t.t('action.save')));
    await settle(tester);
    final input = k.lastArgsOf('settings_set')!['input']! as Map;
    final saved = Map<String, Object?>.from(input['value']! as Map);
    expect(saved['hooks'], {'a': 1});
  });
}
