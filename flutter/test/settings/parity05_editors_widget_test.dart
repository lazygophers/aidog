/// 票 parity-05：settings 界面对齐补齐的三个专用编辑器（沙箱 / 插件 / 环境变量）
/// 与两处新交互（字段重置徽标、全局搜索）的 widget 测试。
///
/// 写回形状全部对着 React 的真值源断言，期望值一字不改：
///   - `SandboxSection.tsx::sync`（空数组·false·null 删键，子对象清空即删）
///   - `PluginsSection.tsx`（enabledPlugins / extraKnownMarketplaces 的增删）
///   - `EnvEditor.tsx`（已知变量分组、自定义变量、搜索过滤、空值删键）
///   - `FieldRenderer.tsx`（R10 重置徽标只在偏离推荐默认时出现）
///   - `Settings.tsx::search`（section 标签 / 字段命中，都不中 → searchNoMatch）
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/src/pages/settings/env_editor.dart';
import 'package:aidog_flutter/src/pages/settings/schema_config_page.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';
import 'pages_c_widget_test.dart' show baseResponses, tapSwitch;

/// 一份够小的 schema：沙箱 / 插件 / 环境变量三节 + 一个带推荐默认的普通字段。
SchemaBundle _bundle() => SchemaBundle(
  sections: [
    SchemaSection({
      'id': 'core',
      'labelKey': 'settings.sectionCore',
      'fields': [
        {'key': 'model', 'label': 'Model', 'type': 'string'},
      ],
    }),
    SchemaSection({
      'id': 'env',
      'labelKey': 'settings.sectionEnv',
      'fields': [
        {'key': 'env', 'label': 'Environment Variables', 'type': 'json'},
      ],
    }),
    SchemaSection({
      'id': 'plugins',
      'labelKey': 'settings.sectionPlugins',
      'fields': [
        {
          'key': 'enabledPlugins',
          'label': 'Enabled Plugins',
          'type': 'kv',
          'skipGui': true,
        },
      ],
    }),
    SchemaSection({
      'id': 'sandbox',
      'labelKey': 'settings.sectionSandbox',
      'fields': [
        {'key': 'sandbox', 'label': 'Sandbox', 'type': 'json', 'skipGui': true},
      ],
    }),
  ],
  recommended: const {'model': 'opus'},
  envCatalog: const EnvVarCatalog(
    defs: [
      EnvVarDef({
        'key': 'CLAUDE_CODE_MAX_OUTPUT_TOKENS',
        'label': 'Max Output Tokens',
        'type': 'number',
        'group': 'performance',
      }),
      EnvVarDef({
        'key': 'ANTHROPIC_AUTH_TOKEN',
        'label': 'Auth Token',
        'type': 'password',
        'group': 'performance',
      }),
      EnvVarDef({
        'key': 'DISABLE_TELEMETRY',
        'label': 'Disable Telemetry',
        'type': 'boolean',
        'group': 'toggles',
      }),
    ],
    groupOrder: ['performance', 'toggles'],
    groupLabelKeys: {
      'performance': 'env.group.performance',
      'toggles': 'env.group.toggles',
    },
  ),
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
          bundleLoader: (_) async => _bundle(),
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

  // ── 沙箱 ─────────────────────────────────────────────

  group('沙箱编辑器', () {
    testWidgets('关着时只有开关与说明；打开后四块都出来', (tester) async {
      final (_, t) = await mount(tester);
      expect(find.byKey(const ValueKey('sandbox-enabled')), findsOneWidget);
      expect(find.text(t.t('settings.sandbox.disabledHint')), findsOneWidget);
      // 关着时不该有文件系统/网络/策略那三块
      expect(find.byKey(const ValueKey('sandbox-fs-allowWrite')), findsNothing);

      await tapSwitch(tester, 'sandbox-enabled');
      await settle(tester);
      expect(
        find.byKey(const ValueKey('sandbox-fs-allowWrite')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('sandbox-net-allowedDomains')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('sandbox-unix-sockets')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('sandbox-excluded-commands')),
        findsOneWidget,
      );
    });

    testWidgets('开开关 → {enabled:true}；再关 → 整个 sandbox 键删掉', (tester) async {
      final (k, t) = await mount(tester);
      await tapSwitch(tester, 'sandbox-enabled');
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(v['sandbox'], {'enabled': true});

      // 关掉：enabled=false 被 sync 删掉 → 整棵树空 → sandbox 键消失
      await tapSwitch(tester, 'sandbox-enabled');
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v.containsKey('sandbox'), isFalse);
    });

    testWidgets('排除命令：加一条进 excludedCommands，重复项不加，删掉后键回收', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'sandbox': {'enabled': true},
        },
      );
      final field = find.descendant(
        of: find.byKey(const ValueKey('sandbox-excluded-commands')),
        matching: find.byType(TextField),
      );
      await tester.enterText(field, 'docker');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect((v['sandbox']! as Map)['excludedCommands'], ['docker']);

      // 重复项：不该变成两条
      await tester.enterText(field, 'docker');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect((v['sandbox']! as Map)['excludedCommands'], ['docker']);

      await tester.tap(find.byKey(const ValueKey('sandbox-tag-del-0')));
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect((v['sandbox']! as Map).containsKey('excludedCommands'), isFalse);
    });

    testWidgets('「禁止逃逸」是反向开关，存得进去', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'sandbox': {'enabled': true},
        },
      );
      await tapSwitch(tester, 'sandbox-no-escape');
      await settle(tester);
      // 开关的 ON 态 = `allowUnsandboxedCommands == false`。这个 false 是用户的
      // 选择（该字段默认 true），不是「与默认值相同」，所以两侧的 sync 都把它
      // 排除在删除之外（`SandboxSection.tsx:128` / `sandbox_editor.dart:248`）。
      //
      // 2026-09-22 之前这条断言写的是「没有变化」——那时两侧都会把 false 删掉，
      // 开关点完自己弹回去。用户在 ask-ui 里定了两侧一起修，所以改成断言存得进去。
      final v = await saveAndRead(tester, k, t);
      expect((v['sandbox']! as Map)['allowUnsandboxedCommands'], isFalse);
    });

    testWidgets('端口：合法值写进 network，非法值原样不写', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'sandbox': {'enabled': true},
        },
      );
      final port = find.descendant(
        of: find.byKey(const ValueKey('sandbox-http-proxy')),
        matching: find.byType(TextField),
      );
      await tester.enterText(port, '8080');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(
        ((v['sandbox']! as Map)['network']! as Map)['httpProxyPort'],
        8080,
      );

      // 越界：React 的 setNetPort 直接 return，不写也不清
      await tester.enterText(port, '70000');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(
        ((v['sandbox']! as Map)['network']! as Map)['httpProxyPort'],
        8080,
      );
    });
  });

  // ── 插件 ─────────────────────────────────────────────

  group('插件编辑器', () {
    testWidgets('加插件 → enabledPlugins；关开关 → false；删除 → 键回收', (tester) async {
      final (k, t) = await mount(tester);
      await tester.enterText(
        find.byKey(const ValueKey('plugin-new')),
        'fmt@core',
      );
      await tester.tap(find.byKey(const ValueKey('plugin-add')));
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(v['enabledPlugins'], {'fmt@core': true});

      // key 挂在 Switch 本身上（不是外层行），直接按 key 点。
      await tester.tap(find.byKey(const ValueKey('plugin-on-fmt@core')));
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v['enabledPlugins'], {'fmt@core': false});

      await tester.tap(find.byKey(const ValueKey('plugin-del-fmt@core')));
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v.containsKey('enabledPlugins'), isFalse);
    });

    testWidgets('加市场源 → 默认 github 来源；删除 → 键回收', (tester) async {
      final (k, t) = await mount(tester);
      await tester.enterText(find.byKey(const ValueKey('market-new')), 'team');
      await tester.tap(find.byKey(const ValueKey('market-add')));
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(v['extraKnownMarketplaces'], {
        'team': {
          'source': {'source': 'github'},
        },
      });

      await tester.tap(find.byKey(const ValueKey('market-del-team')));
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v.containsKey('extraKnownMarketplaces'), isFalse);
    });
  });

  // ── 环境变量 ─────────────────────────────────────────

  group('环境变量编辑器', () {
    testWidgets('已设的已知变量按组显示；改值写回；清空删键', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'env': {'CLAUDE_CODE_MAX_OUTPUT_TOKENS': '16384'},
        },
      );
      expect(find.text(t.t('env.group.performance')), findsOneWidget);
      final row = find.descendant(
        of: find.byKey(const ValueKey('env-CLAUDE_CODE_MAX_OUTPUT_TOKENS')),
        matching: find.byType(TextField),
      );
      await tester.enterText(row, '32768');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(v['env'], {'CLAUDE_CODE_MAX_OUTPUT_TOKENS': '32768'});

      // 清空 = 删这个键 → env 整棵空 → env 键消失
      await tester.enterText(row, '');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v.containsKey('env'), isFalse);
    });

    testWidgets('密码型变量能看一眼明文，再切回密文；切换态不进落盘结果', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'env': {'ANTHROPIC_AUTH_TOKEN': 'sk-ant-secret'},
        },
      );
      bool obscured() => tester
          .widget<TextField>(
            find.descendant(
              of: find.byKey(const ValueKey('env-ANTHROPIC_AUTH_TOKEN')),
              matching: find.byType(TextField),
            ),
          )
          .obscureText;

      // 默认密文：粘错一个字符看不出来，所以要有得看。
      expect(obscured(), isTrue);
      await tester.tap(
        find.byKey(const ValueKey('env-reveal-ANTHROPIC_AUTH_TOKEN')),
      );
      await settle(tester);
      expect(obscured(), isFalse);

      // 再切回去。
      await tester.tap(
        find.byKey(const ValueKey('env-reveal-ANTHROPIC_AUTH_TOKEN')),
      );
      await settle(tester);
      expect(obscured(), isTrue);

      // 切换只是看一眼：不写设置、不落盘。连「有改动待保存」都不该算。
      await tester.tap(
        find.byKey(const ValueKey('env-reveal-ANTHROPIC_AUTH_TOKEN')),
      );
      await settle(tester);
      await tester.tap(find.text(t.t('action.save')));
      await settle(tester);
      expect(
        k.countOf('settings_set'),
        0,
        reason: '只是切了明文，没有任何值变过，不该有写入',
      );
    });

    testWidgets('已知变量有「移除」×，点了键就不在落盘结果里（票 28 ①）', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'env': {
            'CLAUDE_CODE_MAX_OUTPUT_TOKENS': '16384',
            'DISABLE_TELEMETRY': '0',
          },
        },
      );
      // 已设置的变量才有 ×（React `EnvEditor.tsx:34` 的 isSet 同判据）。
      expect(
        find.byKey(const ValueKey('env-remove-CLAUDE_CODE_MAX_OUTPUT_TOKENS')),
        findsOneWidget,
      );
      await tester.tap(
        find.byKey(const ValueKey('env-remove-CLAUDE_CODE_MAX_OUTPUT_TOKENS')),
      );
      await settle(tester);
      final v = await saveAndRead(tester, k, t);
      expect(v['env'], {'DISABLE_TELEMETRY': '0'});
    });

    testWidgets('关成 "0" 的开关型变量也删得掉 —— 清空串那条路删不掉它（票 28 ①）', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'env': {'DISABLE_TELEMETRY': '0'},
        },
      );
      // 开关关着仍是「已设置」，所以 × 在；而它压根没有空串可清。
      expect(
        find.byKey(const ValueKey('env-remove-DISABLE_TELEMETRY')),
        findsOneWidget,
      );
      await tester.tap(
        find.byKey(const ValueKey('env-remove-DISABLE_TELEMETRY')),
      );
      await settle(tester);
      final v = await saveAndRead(tester, k, t);
      expect(v.containsKey('env'), isFalse);
    });

    testWidgets('自定义变量也有「移除」×（票 28 ①）', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'env': {'MY_OWN': 'x', 'OTHER': 'y'},
        },
      );
      await tester.tap(find.byKey(const ValueKey('env-remove-MY_OWN')));
      await settle(tester);
      final v = await saveAndRead(tester, k, t);
      expect(v['env'], {'OTHER': 'y'});
    });

    testWidgets('未知键归到自定义组；「+ 自定义」加一条', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: const {
          'env': {'MY_OWN': 'x'},
        },
      );
      expect(find.text(t.t('env.group.custom')), findsOneWidget);
      expect(find.byKey(const ValueKey('env-custom-MY_OWN')), findsOneWidget);

      await tester.enterText(
        find.byKey(const ValueKey('env-custom-key')),
        'FOO',
      );
      await tester.enterText(
        find.byKey(const ValueKey('env-custom-value')),
        'bar',
      );
      await tester.tap(find.byKey(const ValueKey('env-add-custom')));
      await settle(tester);
      final v = await saveAndRead(tester, k, t);
      expect(v['env'], {'MY_OWN': 'x', 'FOO': 'bar'});
    });

    testWidgets('搜索：命中留下、不命中出 noResults；搜索态下不显示添加入口', (tester) async {
      final (_, t) = await mount(
        tester,
        stored: const {
          'env': {'CLAUDE_CODE_MAX_OUTPUT_TOKENS': '1'},
        },
      );
      await tester.enterText(
        find.byKey(const ValueKey('env-search')),
        'output',
      );
      await settle(tester);
      expect(
        find.byKey(const ValueKey('env-CLAUDE_CODE_MAX_OUTPUT_TOKENS')),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('env-no-results')), findsNothing);
      // 搜索态下不显示「+ 自定义 / + 添加已知变量」
      expect(find.byKey(const ValueKey('env-add-custom')), findsNothing);

      await tester.enterText(
        find.byKey(const ValueKey('env-search')),
        'zzz-nope',
      );
      await settle(tester);
      expect(find.byKey(const ValueKey('env-no-results')), findsOneWidget);
      expect(find.text(t.t('env.noResults')), findsOneWidget);
    });

    testWidgets('「+ 添加已知变量」按类型填默认值：boolean → "1"', (tester) async {
      final (k, t) = await mount(tester);
      await tester.tap(find.byKey(const ValueKey('env-add-known')));
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('env-add-DISABLE_TELEMETRY')));
      await settle(tester);
      final v = await saveAndRead(tester, k, t);
      expect(v['env'], {'DISABLE_TELEMETRY': '1'});
    });
  });

  // ── 重置徽标 + 全局搜索 ────────────────────────────────

  group('字段重置与全局搜索', () {
    testWidgets('值等于推荐默认 → 无徽标；改掉 → 出徽标；点它还原', (tester) async {
      final (k, t) = await mount(tester, stored: const {'model': 'opus'});
      expect(find.byKey(const ValueKey('field-reset-model')), findsNothing);

      final field = find.descendant(
        of: find.byKey(const ValueKey('field-model')),
        matching: find.byType(TextField),
      );
      await tester.enterText(field, 'sonnet');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      expect(find.byKey(const ValueKey('field-reset-model')), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('field-reset-model')));
      await settle(tester);
      final v = await saveAndRead(tester, k, t);
      expect(v['model'], 'opus');
      expect(find.byKey(const ValueKey('field-reset-model')), findsNothing);
    });

    testWidgets('搜索按字段名过滤；全不中 → searchNoMatch', (tester) async {
      final (_, t) = await mount(tester);
      await tester.enterText(
        find.byKey(const ValueKey('settings-search')),
        'model',
      );
      await settle(tester);
      expect(find.byKey(const ValueKey('field-model')), findsOneWidget);
      // sandbox 节不含 model 字段，应当被滤掉
      expect(find.byKey(const ValueKey('sandbox-enabled')), findsNothing);

      await tester.enterText(
        find.byKey(const ValueKey('settings-search')),
        'zzz-no-such-setting',
      );
      await settle(tester);
      expect(
        find.byKey(const ValueKey('settings-search-no-match')),
        findsOneWidget,
      );
      expect(find.text(t.t('settings.searchNoMatch')), findsOneWidget);
    });
  });
}
