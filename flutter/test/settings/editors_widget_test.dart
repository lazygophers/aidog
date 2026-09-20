/// I17 两个专用编辑器（权限矩阵 / hooks 构建器）接进 claude 设置页的 widget 测试。
/// 编辑器行为对齐 React 的 PermissionsSection / HooksSectionInline：
/// 规则增删改模式、defaultMode / 安全开关、JSON 回退、事件与处理器编辑、
/// notify 快捷条。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/src/pages/settings/schema_config_page.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';
import 'pages_c_widget_test.dart' show baseResponses, tapSwitch;

SchemaBundle _bundle() => SchemaBundle(
      sections: [
        SchemaSection({
          'id': 'permissions',
          'labelKey': 'settings.sectionPermissions',
          'fields': [
            {'key': 'permissions', 'label': 'Permissions', 'type': 'json'},
          ],
        }),
        SchemaSection({
          'id': 'hooks',
          'labelKey': 'settings.sectionHooks',
          'fields': [
            {'key': 'hooks', 'label': 'Hooks', 'type': 'json', 'skipGui': true},
          ],
        }),
      ],
      recommended: const {},
    );

void main() {
  Future<(FakeKernel, I18nController)> mount(
    WidgetTester tester, {
    Map<String, Object?> stored = const {},
    Map<String, Object?> notifyFragment = const {},
  }) async {
    await useBigSurface(tester);
    final k = FakeKernel({
      ...baseResponses(),
      'settings_get': (_) => stored,
      'settings_set': (_) => null,
      'build_notify_hooks_fragment': (_) => notifyFragment,
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

  /// 点开挂在 [key] 行里的下拉（DropdownButton 在 Padding 里，直接 tap 行打不开；
  /// `find.byType` 对泛型类比的是 runtimeType，得用 `is` 谓词匹配任意类型参数）。
  Future<void> openDropdown(WidgetTester tester, Key key) async {
    await tester.tap(
      find.descendant(
        of: find.byKey(key),
        matching: find.byWidgetPredicate((w) => w is DropdownButton),
      ),
    );
    await tester.pumpAndSettle();
  }

  /// 保存并取回写进 settings_set 的那份 value。
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

  // ── 权限矩阵 ─────────────────────────────────────────

  group('权限矩阵', () {
    testWidgets('加一条规则 → permissions.allow；模式切换 → ask；删除 → 键回收', (tester) async {
      final (k, t) = await mount(tester);
      // 加一条 allow 规则
      await tester.enterText(
        find.byKey(const ValueKey('perm-add-pattern')),
        'Bash(npm run *)',
      );
      await tester.tap(find.byKey(const ValueKey('perm-add-rule')));
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      expect(
        (v['permissions']! as Map)['allow'],
        ['Bash(npm run *)'],
      );

      // 模式切成 ask（规则行上的模式下拉）
      await openDropdown(tester, const ValueKey('perm-rule-0'));
      await tester.tap(find.text(t.t('settings.permissionsAsk')).last);
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      final perms = v['permissions']! as Map;
      expect(perms.containsKey('allow'), isFalse);
      expect(perms['ask'], ['Bash(npm run *)']);

      // 删除 → 空对象回写成删键
      await tester.tap(find.byKey(const ValueKey('perm-rule-0-del')));
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v.containsKey('permissions'), isFalse);
    });

    testWidgets('defaultMode + 两个安全开关', (tester) async {
      final (k, t) = await mount(tester);
      await tapSwitch(tester, 'perm-disable-bypass');
      await tapSwitch(tester, 'perm-disable-auto');
      final v = await saveAndRead(tester, k, t);
      final perms = v['permissions']! as Map;
      expect(perms['disableBypassPermissionsMode'], 'disable');
      expect(perms['disableAutoMode'], 'disable');
    });

    testWidgets('JSON 回退：可视化 → JSON 文本一致', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: {
          'permissions': {
            'allow': ['Bash(git *)'],
            'defaultMode': 'plan',
          },
        },
      );
      await tester.tap(find.byKey(const ValueKey('perm-view-json')));
      await settle(tester);
      final field = find.descendant(
        of: find.byKey(const ValueKey('perm-json')),
        matching: find.byType(TextField),
      );
      final ctrl = tester.widget<TextField>(field).controller!;
      expect(ctrl.text, contains('"Bash(git *)"'));
      expect(ctrl.text, contains('"plan"'));
      expect(t.t('settings.permissionsVisualView'), isNotEmpty);
    });
  });

  // ── hooks 构建器 ─────────────────────────────────────

  group('hooks 构建器', () {
    testWidgets('加事件 → 默认 command 处理器；写命令落 config.hooks', (tester) async {
      final (k, t) = await mount(tester);
      await openDropdown(tester, const ValueKey('hooks-add-event'));
      await tester.tap(find.textContaining('SessionStart').last);
      await settle(tester);
      expect(
        find.byKey(const ValueKey('hooks-event-SessionStart')),
        findsOneWidget,
      );

      await tester.enterText(
        find.byKey(const ValueKey('hooks-cmd-SessionStart-0-0')),
        './scripts/check.sh',
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);

      final v = await saveAndRead(tester, k, t);
      final hooks = v['hooks']! as Map;
      final groups = (hooks['SessionStart']! as List).cast<Map>();
      expect(groups[0]['matcher'], '');
      final handlers = (groups[0]['hooks']! as List).cast<Map>();
      expect(handlers[0]['type'], 'command');
      expect(handlers[0]['command'], './scripts/check.sh');
    });

    testWidgets('空 handler 组在写回时清掉（syncHooks）', (tester) async {
      final (k, t) = await mount(
        tester,
        stored: {
          'hooks': {
            'Stop': [
              {
                'matcher': '',
                'hooks': [
                  {'type': 'command', 'command': 'aidog-notify-complete'},
                ],
              },
              {'matcher': 'x', 'hooks': []},
            ],
            'Empty': [],
          },
        },
      );
      // 删掉唯一的 handler → Stop 的第一个组空了 → 事件整个回收。
      await tester.tap(
        find.byKey(const ValueKey('hooks-del-handler-Stop-0-0')),
      );
      await settle(tester);
      final v = await saveAndRead(tester, k, t);
      expect(v.containsKey('hooks'), isFalse);
    });

    testWidgets('notify 快捷条：注入片段 + _aidog_hooks；移除剥干净', (tester) async {
      final (k, t) = await mount(
        tester,
        notifyFragment: {
          'Stop': [
            {
              'matcher': '',
              'hooks': [
                {
                  'type': 'command',
                  'command': 'sh ~/.aidog/scripts/aidog-notify-complete',
                },
              ],
            },
          ],
        },
      );
      await tester.tap(find.byKey(const ValueKey('hooks-notify-inject')));
      await settle(tester);
      var v = await saveAndRead(tester, k, t);
      final hooks = v['hooks']! as Map;
      final groups = (hooks['Stop']! as List).cast<Map>();
      final handlers = (groups[0]['hooks']! as List).cast<Map>();
      expect('${handlers[0]['command']}', contains('aidog-notify-complete'));
      expect((v['_aidog_hooks']! as Map)['enabled'], true);

      // 移除：aidog 项剥掉后 hooks 全空 → 键回收，开关置 false。
      await tester.tap(find.byKey(const ValueKey('hooks-notify-remove')));
      await settle(tester);
      v = await saveAndRead(tester, k, t);
      expect(v.containsKey('hooks'), isFalse);
      expect((v['_aidog_hooks']! as Map)['enabled'], false);
    });
  });
}
