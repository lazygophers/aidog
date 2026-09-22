/// 票 28 ④ 的护栏：`settings` 类型的市场来源可以内联定义多个插件，
/// 每个插件自己又是一个完整的来源编辑器（递归嵌套）。
/// 对齐 `src/components/settings/editors/PluginsSection.tsx:139-176`。
///
/// 只测这一支：来源编辑器其余类型（github / url / npm…）由
/// `parity05_editors_widget_test.dart` 守。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/src/pages/settings/plugins_editor.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';

Future<(List<Map<String, Object?>>, I18nController)> mount(
  WidgetTester tester, {
  Map<String, Object?> source = const {'source': 'settings'},
}) async {
  await useBigSurface(tester);
  final writes = <Map<String, Object?>>[];
  final i18n = await makeI18n(tester);
  await tester.pumpWidget(
    wrapPage(
      MarketplaceSourceEditor(source: source, onChanged: writes.add),
      i18n,
    ),
  );
  await settle(tester);
  return (writes, i18n);
}

void main() {
  testWidgets('只有 settings 类型才出「+ Plugin」', (tester) async {
    await mount(tester, source: const {'source': 'github'});
    expect(find.byKey(const ValueKey('mkt-plugin-add')), findsNothing);

    await mount(tester);
    expect(find.byKey(const ValueKey('mkt-plugin-add')), findsOneWidget);
  });

  testWidgets('点「+ Plugin」加一条空插件，默认来源是 github', (tester) async {
    final (writes, _) = await mount(tester);
    await tester.tap(find.byKey(const ValueKey('mkt-plugin-add')));
    await settle(tester);
    expect(writes, hasLength(1));
    expect(writes.single['plugins'], [
      {
        'name': '',
        'source': {'source': 'github'},
      },
    ]);
  });

  testWidgets('内联插件能填名字，也能递归配自己的来源', (tester) async {
    final (writes, _) = await mount(
      tester,
      source: const {
        'source': 'settings',
        'plugins': [
          {
            'name': 'p1',
            'source': {'source': 'github'},
          },
        ],
      },
    );
    // 名字那格在。
    final nameField = find.descendant(
      of: find.byKey(const ValueKey('mkt-plugin-0-name')),
      matching: find.byType(TextField),
    );
    expect(nameField, findsOneWidget);
    await tester.enterText(nameField, 'renamed');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await settle(tester);
    expect((writes.last['plugins']! as List).first, {
      'name': 'renamed',
      'source': {'source': 'github'},
    });

    // 递归那层：插件自己的来源编辑器带 owner / repo 两格。
    final repo = find.descendant(
      of: find.byKey(const ValueKey('mkt-plugin-0-src-repo')),
      matching: find.byType(TextField),
    );
    expect(repo, findsOneWidget);
    await tester.enterText(repo, 'acme/tools');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await settle(tester);
    final plug = (writes.last['plugins']! as List).first as Map;
    expect((plug['source']! as Map)['repo'], 'acme/tools');
  });

  testWidgets('移除最后一条内联插件 → plugins 键回收，不留空数组', (tester) async {
    final (writes, _) = await mount(
      tester,
      source: const {
        'source': 'settings',
        'plugins': [
          {
            'name': 'p1',
            'source': {'source': 'github'},
          },
        ],
      },
    );
    await tester.tap(find.byKey(const ValueKey('mkt-plugin-0-remove')));
    await settle(tester);
    expect(writes.last.containsKey('plugins'), isFalse);
    // 来源本身还在，只是没有内联插件了。
    expect(writes.last['source'], 'settings');
  });

  testWidgets('两条插件各改各的，互不串写', (tester) async {
    final (writes, _) = await mount(
      tester,
      source: const {
        'source': 'settings',
        'plugins': [
          {
            'name': 'a',
            'source': {'source': 'github'},
          },
          {
            'name': 'b',
            'source': {'source': 'github'},
          },
        ],
      },
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('mkt-plugin-1-name')),
        matching: find.byType(TextField),
      ),
      'b2',
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await settle(tester);
    final plugs = writes.last['plugins']! as List;
    expect((plugs[0] as Map)['name'], 'a');
    expect((plugs[1] as Map)['name'], 'b2');
  });
}
