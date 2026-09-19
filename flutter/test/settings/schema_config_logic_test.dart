/// `SchemaConfigController` —— claude / codex / pi 三页共用的双模式状态机。
///
/// React 侧这三页没有测试，所以没有可照抄的断言。用例挑的是「搬丢了用户会直接丢数据」
/// 的那几条：脏状态判定、保存失败不清脏、离页拦截的三条出口、以及三页之间**故意保留**
/// 的行为差异（codex / pi 无 guard、无差异弹窗）。
library;

import 'package:aidog_flutter/src/pages/settings/schema_config_logic.dart';
import 'package:aidog_flutter/src/shell/nav_guard.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fake_invoke.dart';

SchemaConfigController claudeCtl(
  FakeInvoke fake, {
  Map<String, Object?>? recommended,
}) =>
    SchemaConfigController(
      wiring: SchemaConfigWiring.claude,
      recommendedConfig: recommended ?? {'theme': 'dark'},
      invoke: fake.fn,
    );

void main() {
  setUp(resetNavGuardForTest);
  tearDown(resetNavGuardForTest);

  group('load', () {
    test('读到非空配置就用它，基线随之建立 → 初始不脏', () async {
      final fake = FakeInvoke({
        'settings_get': {'model': 'opus'},
      });
      final c = claudeCtl(fake);
      await c.load();

      expect(c.config, {'model': 'opus'});
      expect(c.dirty, isFalse);
      expect(c.editJson, '{\n  "model": "opus"\n}');
      expect(fake.lastCallTo('settings_get')?.args, {'scope': 'global', 'key': 'claude_code'});
    });

    test('从未配置过（空对象 / null）→ 填入推荐配置', () async {
      for (final stored in <Object?>[<String, Object?>{}, null]) {
        final fake = FakeInvoke({'settings_get': stored});
        final c = claudeCtl(fake, recommended: {'theme': 'dark'});
        await c.load();
        expect(c.config, {'theme': 'dark'}, reason: 'stored=$stored');
        expect(c.dirty, isFalse, reason: '填了推荐值也算干净，不该一进页面就是脏的');
      }
    });

    test('读失败时保持空表单、不脏、不显示错误（与 React 的 console.error 一致）', () async {
      final fake = FakeInvoke();
      fake.errors['settings_get'] = StateError('no db');
      final c = claudeCtl(fake);
      await c.load();

      expect(c.config, isEmpty);
      expect(c.saveError, '');
      expect(c.dirty, isFalse, reason: 'baseline 仍是空串 → 一律不脏');
    });

    test('baseline 为空时改了也不脏（首屏加载中点走人不该弹未保存）', () {
      final c = claudeCtl(FakeInvoke());
      c.updateField('model', 'opus');
      expect(c.dirty, isFalse);
    });

    test('codex / pi 用各自的读写命令与入参形状', () async {
      final codex = FakeInvoke({'codex_config_read': {'a': 1}});
      final cc = SchemaConfigController(
        wiring: SchemaConfigWiring.codex,
        recommendedConfig: const {},
        invoke: codex.fn,
      );
      await cc.load();
      cc.updateField('a', 2);
      await cc.save();
      expect(codex.lastCallTo('codex_config_write')?.args, {
        'value': {'a': 2},
      });

      final pi = FakeInvoke({'pi_settings_read': {'a': 1}});
      final pc = SchemaConfigController(
        wiring: SchemaConfigWiring.pi,
        recommendedConfig: const {},
        invoke: pi.fn,
      );
      await pc.load();
      pc.updateField('a', 2);
      await pc.save();
      expect(pi.lastCallTo('pi_settings_write')?.args, {
        'config': {'a': 2},
      });
    });
  });

  group('脏状态', () {
    test('改字段 → 脏；改回去 → 不脏', () async {
      // 用未配对的键。配对键（model / fastMode …）改回原值也不会变干净 ——
      // 见下面那条专门的用例。
      final fake = FakeInvoke({'settings_get': {'outputStyle': 'Concise'}});
      final c = claudeCtl(fake);
      await c.load();

      c.updateField('outputStyle', 'Explanatory');
      expect(c.dirty, isTrue);
      c.updateField('outputStyle', 'Concise');
      expect(c.dirty, isFalse, reason: '签名回到基线就该恢复干净');
    });

    test('配对键改回原值仍然是脏的（配对的环境变量被顺带写出来了）', () async {
      // `model` ↔ `ANTHROPIC_MODEL` 是配对键：改一次就会生出 env.ANTHROPIC_MODEL，
      // 改回原值只是把环境变量也写回原值，那一项本来并不存在 → 配置确实变了。
      // React 版同样如此，这里把它钉住，免得以后有人「顺手修好」。
      final fake = FakeInvoke({'settings_get': {'model': 'opus'}});
      final c = claudeCtl(fake);
      await c.load();

      c.updateField('model', 'sonnet');
      c.updateField('model', 'opus');
      expect(c.dirty, isTrue);
      expect(c.config['env'], {'ANTHROPIC_MODEL': 'opus'});
    });

    test('JSON 模式下键序变了但内容相同 → 不脏（签名是排序过的）', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1, 'b': 2}});
      final c = claudeCtl(fake);
      await c.load();
      c.setMode(EditorMode.json);
      c.setEditJson('{"b":2,"a":1}');
      expect(c.dirty, isFalse);
    });

    test('JSON 模式下半截非法 JSON 视为脏', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.setMode(EditorMode.json);
      c.setEditJson('{"a": ');
      expect(c.dirty, isTrue);
    });

    test('canSave：不脏不能存，保存中不能存', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      expect(c.canSave, isFalse);
      c.updateField('a', 2);
      expect(c.canSave, isTrue);
      c.saving = true;
      expect(c.canSave, isFalse);
    });
  });

  group('save', () {
    test('成功：写回、刷新基线变干净、best-effort 同步分组配置', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);

      expect(await c.save(savedText: '已保存'), isTrue);
      expect(c.dirty, isFalse);
      expect(c.toast, '已保存');
      expect(c.saving, isFalse);
      expect(fake.lastCallTo('settings_set')?.args, {
        'input': {
          'scope': 'global',
          'key': 'claude_code',
          'value': {'a': 2},
        },
      });
      expect(fake.commands, contains('sync_group_settings'));
    });

    test('同步分组配置失败不影响保存成功', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      fake.errors['sync_group_settings'] = StateError('sync down');
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);

      expect(await c.save(), isTrue);
      expect(c.dirty, isFalse);
      expect(c.saveError, '');
    });

    test('写失败：返回 false、保留脏状态、错误就地显示', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      fake.errors['settings_set'] = StateError('disk full');
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);

      expect(await c.save(), isFalse);
      expect(c.dirty, isTrue, reason: '没存进去就不能变干净，否则用户以为存好了');
      expect(c.saveError, contains('disk full'));
      expect(c.saving, isFalse);
    });

    test('JSON 模式保存的是文本框里的内容，不是结构化草稿', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.setMode(EditorMode.json);
      c.setEditJson('{"fromJson": true}');
      await c.save();
      expect(c.config, {'fromJson': true});
    });

    test('JSON 模式下非法 JSON 保存失败并报错', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.setMode(EditorMode.json);
      c.setEditJson('{oops');
      expect(await c.save(), isFalse);
      expect(c.saveError, isNotEmpty);
      expect(fake.callsTo('settings_set'), isEmpty, reason: '解析都没过，不该发写命令');
    });

    test('codex / pi 保存后不调 sync_group_settings', () async {
      final fake = FakeInvoke({'codex_config_read': {'a': 1}});
      final c = SchemaConfigController(
        wiring: SchemaConfigWiring.codex,
        recommendedConfig: const {},
        invoke: fake.fn,
      );
      await c.load();
      c.updateField('a', 2);
      await c.save();
      expect(fake.commands, isNot(contains('sync_group_settings')));
    });
  });

  group('updateField 的空值语义', () {
    test('claude 页：走环境变量配对同步', () async {
      final fake = FakeInvoke({'settings_get': {'x': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('fastMode', false);
      expect(c.config['env'], {'CLAUDE_CODE_DISABLE_FAST_MODE': '1'});
    });

    test('codex / pi 页：null 与空串都删键，false 与 0 保留', () async {
      final fake = FakeInvoke({'codex_config_read': {'a': 1, 'b': 2, 'c': 3, 'd': 4}});
      final c = SchemaConfigController(
        wiring: SchemaConfigWiring.codex,
        recommendedConfig: const {},
        invoke: fake.fn,
      );
      await c.load();
      c.updateField('a', null);
      c.updateField('b', '');
      c.updateField('c', false);
      c.updateField('d', 0);
      expect(c.config, {'c': false, 'd': 0});
    });

    test('codex / pi 页不碰环境变量配对（改 fastMode 不生出 env）', () async {
      final fake = FakeInvoke({'codex_config_read': {'x': 1}});
      final c = SchemaConfigController(
        wiring: SchemaConfigWiring.codex,
        recommendedConfig: const {},
        invoke: fake.fn,
      );
      await c.load();
      c.updateField('fastMode', false);
      expect(c.config.containsKey('env'), isFalse);
    });
  });

  group('加载推荐配置', () {
    test('claude：有差异 → 开弹窗，不直接改配置', () async {
      final fake = FakeInvoke({'settings_get': {'theme': 'light'}});
      final c = claudeCtl(fake, recommended: {'theme': 'dark'});
      await c.load();
      c.loadRecommended(noDiffText: '已包含全部推荐项');

      expect(c.importDiff, isNotNull);
      expect(c.importDiff!.recommended, isTrue);
      expect(c.config, {'theme': 'light'}, reason: '没勾选之前不许动配置');
    });

    test('claude：无差异 → 只出提示，不开弹窗', () async {
      final fake = FakeInvoke({'settings_get': {'theme': 'dark'}});
      final c = claudeCtl(fake, recommended: {'theme': 'dark'});
      await c.load();
      c.loadRecommended(noDiffText: '已包含全部推荐项');

      expect(c.importDiff, isNull);
      expect(c.toast, '已包含全部推荐项');
    });

    test('codex / pi：直接深合并，没有弹窗', () async {
      final fake = FakeInvoke({'codex_config_read': {'keep': 1}});
      final c = SchemaConfigController(
        wiring: SchemaConfigWiring.codex,
        recommendedConfig: const {'added': 2},
        invoke: fake.fn,
      );
      await c.load();
      c.loadRecommended(loadedText: '已加载推荐配置');

      expect(c.importDiff, isNull);
      expect(c.config, {'keep': 1, 'added': 2});
      expect(c.toast, '已加载推荐配置');
    });

    test('应用勾选：只写勾中的路径，并带上推荐配置的 _aidog_* 内部键', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1, 'b': 1}});
      final c = claudeCtl(fake, recommended: {
        'a': 2,
        'b': 2,
        '_aidog_statusline': {'enabled': true},
      });
      await c.load();
      c.loadRecommended();
      c.applyImport({'a'}, appliedText: '已应用');

      expect(c.config['a'], 2);
      expect(c.config['b'], 1, reason: '没勾的保持原值');
      expect(c.config['_aidog_statusline'], {'enabled': true}, reason: '内部键随推荐一起并进来');
      expect(c.importDiff, isNull);
      expect(c.toast, '已应用');
      expect(c.dirty, isTrue);
    });

    test('取消弹窗 → 配置原样不动', () async {
      final fake = FakeInvoke({'settings_get': {'theme': 'light'}});
      final c = claudeCtl(fake, recommended: {'theme': 'dark'});
      await c.load();
      c.loadRecommended();
      c.cancelImport();
      expect(c.importDiff, isNull);
      expect(c.config, {'theme': 'light'});
      expect(c.dirty, isFalse);
    });

    test('没有待处理弹窗时 applyImport 是空操作', () {
      final c = claudeCtl(FakeInvoke());
      c.applyImport({'a'});
      expect(c.config, isEmpty);
    });
  });

  group('从 Claude Code 导入', () {
    test('扣掉 aidog 自己注入的字段后才算差异', () async {
      // 两边都有 `keep`，所以它不进 diff；差异只在 env 下面。
      final fake = FakeInvoke({
        'settings_get': {'keep': 1},
        'read_claude_code_settings': {
          'keep': 1,
          'env': {'ANTHROPIC_BASE_URL': 'http://x', 'MINE': 'y'},
        },
        'get_managed_paths': ['env.ANTHROPIC_BASE_URL'],
      });
      final c = claudeCtl(fake);
      await c.load();
      await c.importFromClaudeCode();

      expect(c.importDiff, isNotNull);
      expect(c.importDiff!.recommended, isFalse);
      final leaves = <String>[];
      for (final n in c.importDiff!.diff) {
        n.collectLeafPaths(leaves);
      }
      expect(leaves, ['env.MINE']);
    });

    test('get_managed_paths 失败 → 退回不过滤，导入仍可用', () async {
      final fake = FakeInvoke({
        'settings_get': {'keep': 1},
        'read_claude_code_settings': {'newKey': 1},
      });
      fake.errors['get_managed_paths'] = StateError('no marker');
      final c = claudeCtl(fake);
      await c.load();
      await c.importFromClaudeCode();
      expect(c.importDiff, isNotNull);
    });

    test('无差异 → 只出提示', () async {
      final fake = FakeInvoke({
        'settings_get': {'a': 1},
        'read_claude_code_settings': {'a': 1},
        'get_managed_paths': <String>[],
      });
      final c = claudeCtl(fake);
      await c.load();
      await c.importFromClaudeCode(noDiffText: '无差异，无需导入');
      expect(c.importDiff, isNull);
      expect(c.toast, '无差异，无需导入');
    });

    test('读文件失败 → toast 用「导入失败」模板，不开弹窗', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      fake.errors['read_claude_code_settings'] = StateError('ENOENT');
      final c = claudeCtl(fake);
      await c.load();
      await c.importFromClaudeCode(failedText: (e) => '导入失败：$e');

      expect(c.importDiff, isNull);
      expect(c.toast, startsWith('导入失败：'));
      expect(c.toast, contains('ENOENT'));
    });
  });

  group('离页拦截', () {
    test('干净时导航畅通，不注册 guard', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();

      expect(hasNavGuard, isFalse);
      var went = false;
      requestNavigation(() => went = true);
      expect(went, isTrue);
    });

    test('脏了就拦下导航，把 proceed 记下来等裁决', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);

      expect(hasNavGuard, isTrue);
      var went = false;
      requestNavigation(() => went = true);
      expect(went, isFalse, reason: 'guard 不自己放行');
      expect(c.pendingNav, isNotNull);
    });

    test('保存并离开：保存成功才放行', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);
      var went = false;
      requestNavigation(() => went = true);

      await c.saveAndLeave(savedText: '已保存');
      expect(went, isTrue);
      expect(c.pendingNav, isNull);
      expect(c.dirty, isFalse);
    });

    test('保存并离开：保存失败就留在原地，pendingNav 保住等重试', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      fake.errors['settings_set'] = StateError('disk full');
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);
      var went = false;
      requestNavigation(() => went = true);

      await c.saveAndLeave();
      expect(went, isFalse);
      expect(c.pendingNav, isNotNull, reason: '错误已就地显示，用户改完还能再点一次');
      expect(c.dirty, isTrue);
    });

    test('不保存直接离开：放行，草稿丢弃', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);
      var went = false;
      requestNavigation(() => went = true);

      c.discardAndLeave();
      expect(went, isTrue);
      expect(c.pendingNav, isNull);
    });

    test('取消：留在本页，草稿保留，guard 仍然挂着', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);
      var went = false;
      requestNavigation(() => went = true);

      c.cancelLeave();
      expect(went, isFalse);
      expect(c.pendingNav, isNull);
      expect(c.config['a'], 2, reason: '草稿不能被清掉');
      expect(hasNavGuard, isTrue, reason: '还是脏的，下次导航仍要拦');
    });

    test('改回干净后 guard 自动注销', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);
      expect(hasNavGuard, isTrue);
      c.updateField('a', 1);
      expect(hasNavGuard, isFalse);
    });

    test('保存成功后 guard 自动注销', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);
      await c.save();
      expect(hasNavGuard, isFalse);
    });

    test('dispose 必须摘掉 guard，否则离开设置页后每次导航都被拦', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);
      expect(hasNavGuard, isTrue);
      c.dispose();
      expect(hasNavGuard, isFalse);
    });

    test('codex / pi 故意不拦：React 版这两页就没有 guard', () async {
      final fake = FakeInvoke({'codex_config_read': {'a': 1}});
      final c = SchemaConfigController(
        wiring: SchemaConfigWiring.codex,
        recommendedConfig: const {},
        invoke: fake.fn,
      );
      await c.load();
      c.updateField('a', 2);
      expect(c.dirty, isTrue);
      expect(hasNavGuard, isFalse, reason: '行为与 React 一致；要加得先改 React 侧');
    });

    test('反复改动不会把 guard 注册成一串（注销闭包不丢）', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.updateField('a', 2);
      c.updateField('a', 3);
      c.updateField('a', 4);
      c.dispose();
      expect(hasNavGuard, isFalse);
    });
  });

  group('模式切换', () {
    test('切到 JSON 会用当前结构化草稿重刷文本', () async {
      final fake = FakeInvoke({'settings_get': {'a': 1}});
      final c = claudeCtl(fake);
      await c.load();
      c.setMode(EditorMode.json);
      c.setEditJson('{"hand":"edited"}');
      c.setMode(EditorMode.gui);
      c.setMode(EditorMode.json);
      expect(c.editJson, '{\n  "a": 1\n}', reason: 'GUI→JSON 重新生成，与 React 一致');
    });
  });
}
