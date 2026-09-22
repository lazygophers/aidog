/// 票 27 第 1 步的护栏：四类结构化字段（kv / kv-select / object / string[]）
/// 用可视化编辑器改一次，落盘的 JSON 结构必须正确。
///
/// 挂的是整个 `SchemaConfigPage`，一路走到 `settings_set` 的入参 ——
/// 单挂编辑器只能证明回调吐了什么，证明不了它最终写成了什么形状。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/src/pages/settings/bits.dart' show PlainTextField;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';
import 'pages_c_widget_test.dart' show baseResponses;

/// 一份只含这四类字段的小 schema。
SchemaBundle bundle() => SchemaBundle(
  sections: [
    SchemaSection({
      'id': 'core',
      'labelKey': 'settings.sectionCore',
      'fields': [
        {
          'key': 'modelOverrides',
          'label': 'Model Overrides',
          'type': 'kv',
          'keyPlaceholder': 'model',
        },
        {
          'key': 'skillOverrides',
          'label': 'Skill Overrides',
          'type': 'kv-select',
          'keyPlaceholder': 'skill-name',
          'valueOptions': ['on', 'off'],
        },
        {
          'key': 'availableModels',
          'label': 'Available Models',
          'type': 'string[]',
        },
        {
          'key': 'spinnerVerbs',
          'label': 'Spinner Verbs',
          'type': 'object',
          'objectFields': [
            {
              'key': 'mode',
              'label': 'Mode',
              'type': 'select',
              'options': ['replace', 'append'],
            },
            {'key': 'verbs', 'label': 'Verbs', 'type': 'string[]'},
            {'key': 'enabled', 'label': 'Enabled', 'type': 'boolean'},
          ],
        },
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

  Future<Map<String, Object?>> saveAndRead(
    WidgetTester tester,
    FakeKernel k,
    I18nController t,
  ) async {
    await tester.tap(find.text(t.t('action.save')));
    await settle(tester);
    final input = k.lastArgsOf('settings_set')!['input']! as Map;
    return Map<String, Object?>.from(input['value']! as Map);
  }

  /// 往某个 key 下的输入框里填字并提交。
  Future<void> type(WidgetTester tester, String key, String text) async {
    await tester.enterText(find.byKey(ValueKey(key)), text);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await settle(tester);
  }

  group('kv', () {
    testWidgets('加一条 → 写成对象；改值 → 只动那一条；删空 → 整个键回收', (tester) async {
      final (k, t) = await mount(tester);
      // 改造前这里是一个手写 JSON 的文本框，现在是 key / value 两格 + 加号。
      await type(tester, 'field-modelOverrides-new-key', 'opus');
      await type(tester, 'field-modelOverrides-new-val', 'sonnet');
      await tester.tap(find.byKey(const ValueKey('field-modelOverrides-add')));
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(v['modelOverrides'], {'opus': 'sonnet'});

      await type(tester, 'field-modelOverrides-val-opus', 'haiku');
      v = await saveAndRead(tester, k, t);
      expect(v['modelOverrides'], {'opus': 'haiku'});

      await tester.tap(
        find.byKey(const ValueKey('field-modelOverrides-del-opus')),
      );
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      // 空表不留 `{}` 空壳，整个键删掉。
      expect(v.containsKey('modelOverrides'), isFalse);
    });

    testWidgets('已有行的 key 只读（改键名 = 删一条加一条）', (tester) async {
      await mount(
        tester,
        stored: const {
          'modelOverrides': {'a': 'b'},
        },
      );
      final keyCell = tester.widget<PlainTextField>(
        find
            .descendant(
              of: find.byKey(const ValueKey('field-modelOverrides-row-a')),
              matching: find.byType(PlainTextField),
            )
            .first,
      );
      expect(keyCell.enabled, isFalse);
    });
  });

  group('kv-select', () {
    testWidgets('加一条取第一个候选；换下拉写回新值', (tester) async {
      final (k, t) = await mount(tester);
      await type(tester, 'field-skillOverrides-new-key', 'my-skill');
      await tester.tap(find.byKey(const ValueKey('field-skillOverrides-add')));
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(v['skillOverrides'], {'my-skill': 'on'});

      await tester.tap(
        find.byKey(const ValueKey('field-skillOverrides-val-my-skill')),
      );
      await settle(tester);
      await tester.tap(find.text('off').last);
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v['skillOverrides'], {'my-skill': 'off'});
    });
  });

  group('string[]', () {
    testWidgets('逐条加、逐条改、逐条删；清空即删键', (tester) async {
      final (k, t) = await mount(tester);
      await type(tester, 'field-availableModels-new', 'opus');
      await tester.tap(find.byKey(const ValueKey('field-availableModels-add')));
      await settle(tester);
      await type(tester, 'field-availableModels-new', 'sonnet');
      await tester.tap(find.byKey(const ValueKey('field-availableModels-add')));
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(v['availableModels'], ['opus', 'sonnet']);

      // 改中间一条不影响另一条。
      await type(tester, 'field-availableModels-item-0', 'haiku');
      v = await saveAndRead(tester, k, t);
      expect(v['availableModels'], ['haiku', 'sonnet']);

      // 删第一条：剩下的那条原样留着（旧实现是多行文本框，删中间一条得自己数行）。
      await tester.tap(
        find.byKey(const ValueKey('field-availableModels-del-0')),
      );
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v['availableModels'], ['sonnet']);

      await tester.tap(
        find.byKey(const ValueKey('field-availableModels-del-0')),
      );
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v.containsKey('availableModels'), isFalse);
    });

    testWidgets('值里的空格与空白项原样保留（旧的多行文本框会 trim 掉、吃掉空行）', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'availableModels': ['a b'],
        },
      );
      // 存量那条带空格；再加一条同样带空格的，两条都要原样落盘。
      await type(tester, 'field-availableModels-new', 'c d');
      await tester.tap(find.byKey(const ValueKey('field-availableModels-add')));
      await settle(tester);
      final v = await saveAndRead(tester, k, t);
      expect(v['availableModels'], ['a b', 'c d']);
    });
  });

  group('object', () {
    testWidgets('三种子字段各写各的，嵌套 string[] 也能加', (tester) async {
      final (k, t) = await mount(tester);

      // select 子字段
      await tester.tap(find.byKey(const ValueKey('field-spinnerVerbs-mode')));
      await settle(tester);
      await tester.tap(find.text('append').last);
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(v['spinnerVerbs'], {'mode': 'append'});

      // 嵌套的 string[] 子字段
      await type(tester, 'field-spinnerVerbs-verbs-new', 'thinking');
      await tester.tap(
        find.byKey(const ValueKey('field-spinnerVerbs-verbs-add')),
      );
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v['spinnerVerbs'], {
        'mode': 'append',
        'verbs': ['thinking'],
      });

      // boolean 子字段
      await tester.tap(
        find.byKey(const ValueKey('field-spinnerVerbs-enabled')),
      );
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect((v['spinnerVerbs']! as Map)['enabled'], isTrue);
    });

    testWidgets('子字段清空即删子键，整棵清空即删整个字段', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'spinnerVerbs': {'mode': 'append', 'enabled': true},
        },
      );
      // 关掉 boolean → 子键删掉（false 不写进去）。
      await tester.tap(
        find.byKey(const ValueKey('field-spinnerVerbs-enabled')),
      );
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(v['spinnerVerbs'], {'mode': 'append'});

      // select 选回「—」→ 整棵树空 → 整个字段删掉。
      await tester.tap(find.byKey(const ValueKey('field-spinnerVerbs-mode')));
      await settle(tester);
      await tester.tap(find.text('—').last);
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v.containsKey('spinnerVerbs'), isFalse);
    });
  });
}
