/// 技能页逻辑层的单测（票 I09）。
///
/// React 侧这一批**没有现成测试**（`src/pages/Skills/` 下无 `.test.`），所以这里逐条
/// 对着 `useSkillsData.ts` / `SkillInstallView.tsx` / `SkillDetailView.tsx` 的行为写：
/// 每条断言的旁边写清楚对应的是源文件哪一行的哪个分支。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart' show VoidCallback;

import 'package:aidog_flutter/pages.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';

Map<String, Object?> skillJson(
  String name, {
  List<String> agents = const [],
  String? path = '/p/skill',
  String? description,
  String? source,
}) => {
  'name': name,
  'enabled_agents': agents,
  'scope': const {'kind': 'global'},
  'installed_path': path,
  'description': description,
  'source': source,
  'source_type': null,
  'source_url': null,
  'skill_folder_hash': null,
  'plugin_name': null,
  'installed_at': null,
  'updated_at': null,
};

Map<String, Object?> cached(
  List<Map<String, Object?>> items, {
  bool stale = false,
  bool loadFailed = false,
}) => {'items': items, 'stale': stale, 'load_failed': loadFailed};

Map<String, Object?> opResult({
  bool success = true,
  String stdout = '',
  String stderr = '',
}) => {'success': success, 'stdout': stdout, 'stderr': stderr};

/// 默认夹具：npx 可用、缓存命中两条 skill。
FakeKernel fakeSkills({
  bool npx = true,
  Map<String, Object? Function(Map<String, Object?>?)> overrides = const {},
}) {
  final base = <String, Object? Function(Map<String, Object?>?)>{
    'skills_check_env': (_) => {'npx_available': npx, 'node_version': 'v22.0.0'},
    'skills_list_installed': (_) => cached([
      skillJson('git-flow', agents: ['claude'], source: 'acme/skills'),
      skillJson('docs-writer'),
    ]),
    'skills_list_refresh': (_) => cached([
      skillJson('git-flow', agents: ['claude'], source: 'acme/skills'),
      skillJson('docs-writer'),
    ]),
  };
  return FakeKernel({...base, ...overrides});
}

Future<SkillsController> bootedSkills(
  FakeKernel k, {
  int Function()? now,
}) async {
  final c = SkillsController(
    invoke: k.invoke,
    t: (key, [args]) => key,
    onChanged: () {},
    now: now ?? () => 0,
  );
  await c.init();
  return c;
}

void main() {
  group('share.ts 的编解码（逐条对着 share.ts 写）', () {
    test('skillCatalogId: 有 source 才能分享', () {
      expect(
        skillCatalogId(
          SkillInfo.fromJson(skillJson('a', source: 'owner/repo')),
        ),
        'owner/repo@a',
      );
      // 手动 symlink（锁文件无条目）→ null，不可分享
      expect(skillCatalogId(SkillInfo.fromJson(skillJson('a'))), isNull);
    });

    test('decodeSkillShare: 裸数组 / {skills:[...]} 两种都收', () {
      expect(decodeSkillShare('["o/r@a","o/r@b"]'), ['o/r@a', 'o/r@b']);
      expect(decodeSkillShare('{"skills":["o/r@a"]}'), ['o/r@a']);
    });

    test('decodeSkillShare: base64 包裹也认（长度 > 16 才尝试解码）', () {
      final payload = jsonEncode({
        'skills': ['owner/repo@some-skill'],
      });
      final b64 = base64.encode(utf8.encode(payload));
      expect(b64.length, greaterThan(16));
      expect(decodeSkillShare(b64), ['owner/repo@some-skill']);
    });

    test('decodeSkillShare: 非法一律 null，不抛', () {
      expect(decodeSkillShare(''), isNull);
      expect(decodeSkillShare('   '), isNull);
      expect(decodeSkillShare('not json'), isNull);
      // 每项必须含 @（owner/repo@skill 形态）
      expect(decodeSkillShare('["no-at-sign"]'), isNull);
      expect(decodeSkillShare('[1,2]'), isNull);
      // 对象但没有 skills 数组
      expect(decodeSkillShare('{"foo":1}'), isNull);
    });

    test('stdout 计数正则：解析不出来按 0 算', () {
      expect(parseAlignedCount('aligned 3 changes (x)'), 3);
      expect(parseAlignedCount('nothing'), 0);
      expect(parseEnabledCount('enabled 12 skills'), 12);
      expect(parseEnabledCount(''), 0);
    });

    test('formatSkillFileSize: 三档阈值与小数位照搬', () {
      expect(formatSkillFileSize(0), '0 B');
      expect(formatSkillFileSize(1023), '1023 B');
      expect(formatSkillFileSize(1024), '1.0 KB');
      expect(formatSkillFileSize(1024 * 1024), '1.0 MB');
      expect(formatSkillFileSize(1536), '1.5 KB');
    });
  });

  group('SWR 缓存 / 刷新（useSkillsData.ts:102-162）', () {
    test('命中缓存: 先渲染缓存，再后台 revalidate（两条命令都发）', () async {
      final k = fakeSkills();
      final c = await bootedSkills(k);
      expect(c.installed, hasLength(2));
      expect(k.countOf('skills_list_installed'), 1);
      // 缓存命中后仍必须跑一次 refresh —— 空缓存粘住那个 bug 就是漏了这一步
      expect(k.countOf('skills_list_refresh'), 1);
      expect(c.installedLoading, isFalse);
    });

    test('冷启动（stale=true）: 显整页加载态并强制 refresh', () async {
      final k = fakeSkills(
        overrides: {'skills_list_installed': (_) => cached([], stale: true)},
      );
      final c = await bootedSkills(k);
      expect(c.installedLoading, isFalse); // 跑完落回
      expect(c.installed, hasLength(2));
      expect(k.countOf('skills_list_refresh'), 1);
    });

    test('缓存读取失败也兜底走 refresh，不炸页', () async {
      final k = fakeSkills(
        overrides: {
          'skills_list_installed': (_) => throw StateError('cache broken'),
        },
      );
      final c = await bootedSkills(k);
      expect(c.installed, hasLength(2));
      expect(k.countOf('skills_list_refresh'), 1);
    });

    test('load_failed=true: 提示「显示上次缓存」，列表不清空', () async {
      final k = fakeSkills(
        overrides: {
          'skills_list_installed': (_) => cached([], stale: true),
          'skills_list_refresh': (_) => cached([
            skillJson('kept'),
          ], loadFailed: true),
        },
      );
      final c = await bootedSkills(k);
      expect(c.message, 'skills.loadFailed');
      expect(c.installed, hasLength(1));
    });

    test('refresh 抛错: 保留上一次的列表，不清空', () async {
      var boom = false;
      final k = fakeSkills(
        overrides: {
          'skills_list_refresh': (_) => boom
              ? throw StateError('npx gone')
              : cached([
                  skillJson('git-flow', agents: ['claude']),
                  skillJson('docs-writer'),
                ]),
        },
      );
      final c = await bootedSkills(k);
      expect(c.installed, hasLength(2)); // 缓存那两条
      boom = true;
      await c.refreshInstalled();
      expect(c.installed, hasLength(2));
      expect(c.refreshing, isFalse);
    });

    test('项目 scope 但路径为空: 列表清空且一条命令都不发', () async {
      final k = fakeSkills();
      final c = await bootedSkills(k);
      final before = k.calls.length;
      c.setScopeKind('project');
      await Future<void>.delayed(Duration.zero);
      expect(c.scopeInvalid, isTrue);
      expect(c.installed, isEmpty);
      expect(k.calls.length, before);
    });

    test('scope 载荷: global 是 {kind:global}，project 带 path', () async {
      final k = fakeSkills();
      final c = await bootedSkills(k);
      expect(c.scope, {'kind': 'global'});
      c.projectPath = '/work/proj';
      c.scopeKind = 'project';
      expect(c.scope, {'kind': 'project', 'path': '/work/proj'});
    });

    test('获焦重查: 10 秒内不重复跑 npx', () async {
      var clock = 0;
      final k = fakeSkills();
      final c = await bootedSkills(k, now: () => clock);
      final after = k.countOf('skills_list_refresh');
      // 刚刷过（lastRefreshAt = 0，now = 0）→ 差 0 ms，节流拦住
      c.maybeRevalidate();
      await Future<void>.delayed(Duration.zero);
      expect(k.countOf('skills_list_refresh'), after);
      // 推过 10 秒窗口 → 放行
      clock = kRevalidateThrottleMs;
      c.maybeRevalidate();
      await Future<void>.delayed(Duration.zero);
      expect(k.countOf('skills_list_refresh'), after + 1);
    });

    test('获焦重查: 不在列表视图 / 正在刷新时不触发', () async {
      var clock = kRevalidateThrottleMs * 10;
      final k = fakeSkills();
      final c = await bootedSkills(k, now: () => clock);
      final after = k.countOf('skills_list_refresh');
      c.setSubView('install');
      c.maybeRevalidate();
      await Future<void>.delayed(Duration.zero);
      expect(k.countOf('skills_list_refresh'), after);
      c.setSubView('list');
      c.refreshing = true;
      c.maybeRevalidate();
      await Future<void>.delayed(Duration.zero);
      expect(k.countOf('skills_list_refresh'), after);
    });
  });

  group('派生量与筛选', () {
    test('统计取全量，筛选只影响展示', () async {
      final c = await bootedSkills(fakeSkills());
      expect(c.total, 2);
      expect(c.agentCounts, {'claude': 1, 'codex': 0});
      c.setEnabledFilter('enabled');
      expect(c.filteredInstalled, hasLength(1));
      // 总数不跟着筛选走
      expect(c.total, 2);
      c.setEnabledFilter('disabled');
      expect([for (final s in c.filteredInstalled) s.name], ['docs-writer']);
    });

    test('搜索匹配 name / description / source，且不分大小写', () async {
      final k = fakeSkills(
        overrides: {
          'skills_list_installed': (_) => cached([
            skillJson('alpha', description: '部署助手'),
            skillJson('beta', source: 'ACME/tools'),
          ]),
          'skills_list_refresh': (_) => cached([
            skillJson('alpha', description: '部署助手'),
            skillJson('beta', source: 'ACME/tools'),
          ]),
        },
      );
      final c = await bootedSkills(k);
      c.setSearchQuery('ALPHA');
      expect([for (final s in c.filteredInstalled) s.name], ['alpha']);
      c.setSearchQuery('部署');
      expect([for (final s in c.filteredInstalled) s.name], ['alpha']);
      c.setSearchQuery('acme');
      expect([for (final s in c.filteredInstalled) s.name], ['beta']);
      // 中文描述打拼音 / 首字母也要搜得到（`useSkillsData.ts:88-90` 走 pinyinMatch）。
      c.setSearchQuery('bushu');
      expect([for (final s in c.filteredInstalled) s.name], ['alpha']);
      c.setSearchQuery('bsz');
      expect([for (final s in c.filteredInstalled) s.name], ['alpha']);
      c.setSearchQuery('没这个词');
      expect(c.filteredInstalled, isEmpty);
    });

    test('筛选 + 搜索叠加', () async {
      final c = await bootedSkills(fakeSkills());
      c.setEnabledFilter('enabled');
      c.setSearchQuery('docs');
      expect(c.filteredInstalled, isEmpty);
    });
  });

  group('启停（乐观更新 + 回滚）', () {
    test('未启用 → enable：带 path，成功后保留乐观态不重拉', () async {
      final k = fakeSkills(
        overrides: {'skills_enable': (_) => opResult(stdout: 'ok')},
      );
      final c = await bootedSkills(k);
      final before = k.countOf('skills_list_refresh');
      await c.toggle(c.installed[1], 'codex');
      expect(k.lastArgsOf('skills_enable'), {
        'name': 'docs-writer',
        'path': '/p/skill',
        'agent': 'codex',
        'scope': {'kind': 'global'},
      });
      expect(c.installed[1].enabledAgents, ['codex']);
      expect(c.message, 'skills.enabled');
      // 成功不整表重载
      expect(k.countOf('skills_list_refresh'), before);
    });

    test('已启用 → disable：不带 path', () async {
      final k = fakeSkills(
        overrides: {'skills_disable': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      await c.toggle(c.installed[0], 'claude');
      expect(k.lastArgsOf('skills_disable')!.containsKey('path'), isFalse);
      expect(c.installed[0].enabledAgents, isEmpty);
      expect(c.message, 'skills.disabled');
    });

    test('后端返 success:false → 回滚 + 弹 stderr', () async {
      final k = fakeSkills(
        overrides: {
          'skills_enable': (_) =>
              opResult(success: false, stderr: '  npx boom  '),
        },
      );
      final c = await bootedSkills(k);
      await c.toggle(c.installed[1], 'codex');
      expect(c.installed[1].enabledAgents, isEmpty, reason: '必须回滚');
      expect(c.message, 'npx boom', reason: 'stderr 要 trim');
    });

    test('stderr 空则退 stdout，都空才用兜底文案', () async {
      final k = fakeSkills(
        overrides: {
          'skills_enable': (_) => opResult(success: false, stdout: 'out msg'),
        },
      );
      final c = await bootedSkills(k);
      await c.toggle(c.installed[1], 'codex');
      expect(c.message, 'out msg');

      final k2 = fakeSkills(
        overrides: {'skills_enable': (_) => opResult(success: false)},
      );
      final c2 = await bootedSkills(k2);
      await c2.toggle(c2.installed[1], 'codex');
      expect(c2.message, 'skills.opFailed');
    });

    test('抛异常也回滚', () async {
      final k = fakeSkills(
        overrides: {'skills_enable': (_) => throw StateError('io')},
      );
      final c = await bootedSkills(k);
      await c.toggle(c.installed[1], 'codex');
      expect(c.installed[1].enabledAgents, isEmpty);
      expect(c.message, contains('io'));
    });

    test('npx 不可用 / 有任务在跑时一条命令都不发', () async {
      final k = fakeSkills(npx: false);
      final c = await bootedSkills(k);
      expect(c.writeReady, isFalse);
      await c.toggle(c.installed[0], 'claude');
      expect(k.countOf('skills_enable'), 0);
      expect(k.countOf('skills_disable'), 0);

      final k2 = fakeSkills(
        overrides: {'skills_disable': (_) => opResult()},
      );
      final c2 = await bootedSkills(k2);
      c2.busyKey = '__something__';
      await c2.toggle(c2.installed[0], 'claude');
      expect(k2.countOf('skills_disable'), 0);
    });
  });

  group('破坏性操作：确认之前一个命令都不发', () {
    test('一键卸载：askUninstallAll 只开确认框', () async {
      final k = fakeSkills(
        overrides: {'skills_uninstall_all': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      c.askUninstallAll();
      expect(c.confirmUninstall, isTrue);
      expect(k.countOf('skills_uninstall_all'), 0);
      await c.uninstallAll();
      expect(k.countOf('skills_uninstall_all'), 1);
      expect(c.confirmUninstall, isFalse);
      expect(c.message, 'skills.uninstallAllDone');
    });

    test('单条卸载：取消后目标清空，不发命令', () async {
      final k = fakeSkills(
        overrides: {'skills_uninstall': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      c.askUninstall(c.installed[0]);
      expect(c.uninstallTarget, isNotNull);
      c.cancelUninstall();
      expect(c.uninstallTarget, isNull);
      expect(k.countOf('skills_uninstall'), 0);
    });

    test('单条卸载：确认后按名字发命令', () async {
      final k = fakeSkills(
        overrides: {'skills_uninstall': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      c.askUninstall(c.installed[0]);
      await c.uninstallSingle();
      expect(k.lastArgsOf('skills_uninstall'), {
        'name': 'git-flow',
        'scope': {'kind': 'global'},
      });
      expect(c.message, 'skills.uninstallDone');
    });

    test('批量卸载：勾选集合发出后被清空（React 同序）', () async {
      final k = fakeSkills(
        overrides: {'skills_uninstall_batch': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      c.toggleSelected('git-flow');
      c.toggleSelected('docs-writer');
      c.toggleSelected('git-flow'); // 再点一次 = 取消
      expect(c.selectedNames, {'docs-writer'});
      c.askUninstallBatch();
      await c.uninstallBatch();
      expect(k.lastArgsOf('skills_uninstall_batch')!['names'], ['docs-writer']);
      expect(c.selectedNames, isEmpty);
    });

    test('批量卸载：一条都没勾就不发命令', () async {
      final k = fakeSkills(
        overrides: {'skills_uninstall_batch': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      await c.uninstallBatch();
      expect(k.countOf('skills_uninstall_batch'), 0);
    });
  });

  group('对齐 / 全部启用（stdout 计数决定文案）', () {
    test('对齐：同 agent 直接 return，连弹窗都不关', () async {
      final k = fakeSkills(
        overrides: {'skills_align_agents': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      c.openAlign();
      c.setAlignTo('claude'); // from 也是 claude
      await c.align();
      expect(k.countOf('skills_align_agents'), 0);
      expect(c.alignOpen, isTrue, reason: 'React 在 return 之前没 setAlignOpen(false)');
    });

    test('对齐：N=0 报 noop，N>0 报变更数', () async {
      final k = fakeSkills(
        overrides: {
          'skills_align_agents': (_) => opResult(stdout: 'aligned 0 changes'),
        },
      );
      final c = await bootedSkills(k);
      await c.align();
      expect(c.message, 'skills.alignNoop');

      final k2 = fakeSkills(
        overrides: {
          'skills_align_agents': (_) =>
              opResult(stdout: 'aligned 4 changes (a,b)'),
        },
      );
      final c2 = await bootedSkills(k2);
      await c2.align();
      expect(c2.message, 'skills.alignDone');
      expect(k2.lastArgsOf('skills_align_agents'), {
        'from': 'claude',
        'to': 'codex',
        'scope': {'kind': 'global'},
      });
    });

    test('全部启用：非破坏性，不需确认；N=0 报 noop', () async {
      final k = fakeSkills(
        overrides: {
          'skills_enable_all': (_) => opResult(stdout: 'enabled 0 skills'),
        },
      );
      final c = await bootedSkills(k);
      await c.enableAll('codex');
      expect(k.lastArgsOf('skills_enable_all'), {
        'agent': 'codex',
        'scope': {'kind': 'global'},
      });
      expect(c.message, 'skills.enableAllNoop');
    });

    test('全部启用：成功后强制重查一次', () async {
      final k = fakeSkills(
        overrides: {
          'skills_enable_all': (_) => opResult(stdout: 'enabled 7 skills'),
        },
      );
      final c = await bootedSkills(k);
      final before = k.countOf('skills_list_refresh');
      await c.enableAll('claude');
      expect(k.countOf('skills_list_refresh'), before + 1);
      expect(c.message, 'skills.enableAllDone');
    });
  });

  group('分享 / 导入', () {
    test('分享：非 catalog 来源只提示，不开弹窗', () async {
      final c = await bootedSkills(fakeSkills());
      c.share(c.installed[1]); // docs-writer 无 source
      expect(c.shareData, isNull);
      expect(c.message, 'skills.share.noSource');
      c.share(c.installed[0]);
      expect(c.shareData!.skills, ['acme/skills@git-flow']);
      expect(c.shareData!.name, 'git-flow');
    });

    test('粘贴导入：非法只提示且不关弹窗；合法则转确认框', () async {
      final c = await bootedSkills(fakeSkills());
      c.setPasteOpen(true);
      c.setPasteText('garbage');
      c.pasteImport();
      expect(c.message, 'skills.importInvalid');
      expect(c.pasteOpen, isTrue);
      expect(c.importIds, isNull);

      c.setPasteText('["o/r@x"]');
      c.pasteImport();
      expect(c.pasteOpen, isFalse);
      expect(c.pasteText, '');
      expect(c.importIds, ['o/r@x']);
      // 默认全选 agent、scope 回到 global
      expect(c.importAgents, {'claude', 'codex'});
      expect(c.importScopeKind, 'global');
    });

    test('deep-link 导入：与粘贴同一条路径', () async {
      final c = await bootedSkills(fakeSkills());
      c.openDeepLinkImport(
        base64.encode(
          utf8.encode(
            jsonEncode({
              'skills': ['o/r@deep'],
            }),
          ),
        ),
      );
      expect(c.importIds, ['o/r@deep']);
      // 空 data 什么都不做
      c.cancelImport();
      c.openDeepLinkImport('');
      expect(c.importIds, isNull);
    });

    test('导入：逐条 skills_install，全成功报 importOk', () async {
      final k = fakeSkills(
        overrides: {'skills_install': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      c.openImportConfirm(['o/r@a', 'o/r@b']);
      await c.runImport();
      expect(k.countOf('skills_install'), 2);
      expect(c.message, 'skills.importOk');
      expect(c.importIds, isNull);
      expect(c.importBusy, isFalse);
    });

    test('导入：全失败报 importFail，部分成功报 importPartial + 失败清单', () async {
      final k = fakeSkills(
        overrides: {'skills_install': (_) => opResult(success: false)},
      );
      final c = await bootedSkills(k);
      c.openImportConfirm(['o/r@a']);
      await c.runImport();
      expect(c.message, 'skills.importFail');

      var n = 0;
      final k2 = fakeSkills(
        overrides: {
          'skills_install': (_) => opResult(success: n++ == 0),
        },
      );
      final c2 = await bootedSkills(k2);
      c2.openImportConfirm(['o/r@ok', 'o/r@bad']);
      await c2.runImport();
      expect(c2.message, startsWith('skills.importPartial'));
      expect(c2.message, contains('o/r@bad'), reason: '失败清单要跟在后面');
    });

    test('导入：npx 不可用时只提示，不发命令', () async {
      final k = fakeSkills(
        npx: false,
        overrides: {'skills_install': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      c.openImportConfirm(['o/r@a']);
      await c.runImport();
      expect(k.countOf('skills_install'), 0);
      expect(c.message, 'skills.envMissing');
    });

    test('导入：一个 agent 都没选 / 项目路径为空 → 不发命令', () async {
      final k = fakeSkills(
        overrides: {'skills_install': (_) => opResult()},
      );
      final c = await bootedSkills(k);
      c.openImportConfirm(['o/r@a']);
      c.toggleImportAgent('claude');
      c.toggleImportAgent('codex');
      expect(c.importAgents, isEmpty);
      await c.runImport();
      expect(k.countOf('skills_install'), 0);

      c.toggleImportAgent('claude');
      c.setImportScopeKind('project');
      await c.runImport();
      expect(k.countOf('skills_install'), 0);

      c.setImportProjectPath('/proj');
      await c.runImport();
      expect(k.countOf('skills_install'), 1);
      expect(k.lastArgsOf('skills_install')!['scope'], {
        'kind': 'project',
        'path': '/proj',
      });
    });

    test('openImportConfirm: 空清单不开框', () async {
      final c = await bootedSkills(fakeSkills());
      c.openImportConfirm(const []);
      expect(c.importIds, isNull);
    });
  });

  group('搜索安装子视图（SkillInstallView.tsx）', () {
    SkillInstallController install(
      FakeKernel k, {
      VoidCallback? onInstalled,
      Duration debounce = Duration.zero,
    }) => SkillInstallController(
      invoke: k.invoke,
      t: (key, [args]) => key,
      onChanged: () {},
      scope: const {'kind': 'global'},
      onInstalled: onInstalled ?? () {},
      debounce: debounce,
    );

    test('空关键字：清空结果且不发命令', () async {
      final k = FakeKernel({'skills_search': (_) => <Object?>[]});
      final c = install(k);
      await c.search('   ');
      expect(k.countOf('skills_search'), 0);
      expect(c.results, isEmpty);
      expect(c.loading, isFalse);
    });

    test('搜索：结果每条默认全选 agent', () async {
      final k = FakeKernel({
        'skills_search': (_) => [
          {'id': 'o/r@a', 'name': 'a', 'description': 'd', 'repo_url': 'u'},
        ],
      });
      final c = install(k);
      await c.search('git');
      expect(k.lastArgsOf('skills_search'), {'keyword': 'git'});
      expect(c.results, hasLength(1));
      expect(c.selected['o/r@a'], {'claude', 'codex'});
    });

    test('搜索失败：记 error 并清空结果', () async {
      final k = FakeKernel({
        'skills_search': (_) => throw StateError('offline'),
      });
      final c = install(k);
      await c.search('git');
      expect(c.error, contains('offline'));
      expect(c.results, isEmpty);
      expect(c.loading, isFalse);
    });

    test('防抖：连打三次只跑最后一次', () {
      fakeAsync((clock) {
        final k = FakeKernel({'skills_search': (_) => <Object?>[]});
        final c = install(
          k,
          debounce: kSkillSearchDebounce,
        );
        c.setKeyword('g');
        c.setKeyword('gi');
        c.setKeyword('git');
        clock(const Duration(milliseconds: 349));
        expect(k.countOf('skills_search'), 0);
        clock(const Duration(milliseconds: 2));
        expect(k.countOf('skills_search'), 1);
        expect(k.lastArgsOf('skills_search'), {'keyword': 'git'});
        c.dispose();
      });
    });

    test('逐条安装：成功回调父级刷新', () async {
      var refreshed = 0;
      final k = FakeKernel({
        'skills_search': (_) => [
          {'id': 'o/r@a', 'name': 'a', 'description': null, 'repo_url': null},
        ],
        'skills_install': (_) => opResult(),
      });
      final c = install(k, onInstalled: () => refreshed++);
      await c.search('a');
      await c.install(c.results.first);
      expect(k.lastArgsOf('skills_install'), {
        'id': 'o/r@a',
        'agents': ['claude', 'codex'],
        'scope': {'kind': 'global'},
      });
      expect(refreshed, 1);
      expect(c.message, 'skills.install.installSuccess');
      expect(c.busyId, isNull);
    });

    test('逐条安装：一个 agent 都没选就不发命令', () async {
      final k = FakeKernel({
        'skills_search': (_) => [
          {'id': 'o/r@a', 'name': 'a', 'description': null, 'repo_url': null},
        ],
        'skills_install': (_) => opResult(),
      });
      final c = install(k);
      await c.search('a');
      c.toggleAgent('o/r@a', 'claude');
      c.toggleAgent('o/r@a', 'codex');
      await c.install(c.results.first);
      expect(k.countOf('skills_install'), 0);
    });

    test('批量安装：按 agent 集合分组，同组一次 skills_install_batch', () async {
      final k = FakeKernel({
        'skills_search': (_) => [
          for (final id in ['o/r@a', 'o/r@b', 'o/r@c'])
            {'id': id, 'name': id, 'description': null, 'repo_url': null},
        ],
        'skills_install_batch': (_) => opResult(),
      });
      final c = install(k);
      await c.search('x');
      // a、b 保持默认（claude+codex）；c 只留 claude → 两个组
      c.toggleAgent('o/r@c', 'codex');
      c.toggleChecked('o/r@a');
      c.toggleChecked('o/r@b');
      c.toggleChecked('o/r@c');
      await c.installBatch();
      expect(k.countOf('skills_install_batch'), 2);
      // 三条全成功 → 勾选清空 + importOk
      expect(c.checked, isEmpty);
      expect(c.message, 'skills.importOk');
    });

    test('批量安装：一条都没勾就不发命令', () async {
      final k = FakeKernel({
        'skills_search': (_) => <Object?>[],
        'skills_install_batch': (_) => opResult(),
      });
      final c = install(k);
      await c.installBatch();
      expect(k.countOf('skills_install_batch'), 0);
    });

    test('批量安装：部分失败时报 importPartial，勾选仍清空', () async {
      var n = 0;
      final k = FakeKernel({
        'skills_search': (_) => [
          for (final id in ['o/r@a', 'o/r@c'])
            {'id': id, 'name': id, 'description': null, 'repo_url': null},
        ],
        'skills_install_batch': (_) => opResult(success: n++ == 0),
      });
      final c = install(k);
      await c.search('x');
      c.toggleAgent('o/r@c', 'codex'); // 分成两组
      c.toggleChecked('o/r@a');
      c.toggleChecked('o/r@c');
      await c.installBatch();
      expect(c.message, 'skills.importPartial');
      expect(c.checked, isEmpty);
    });
  });

  group('只读详情（SkillDetailView.tsx）', () {
    SkillDetailController detail(FakeKernel k, SkillInfo s) =>
        SkillDetailController(
          invoke: k.invoke,
          t: (key, [args]) => key,
          onChanged: () {},
          skill: s,
        );

    test('无安装路径：直接报错，一条命令都不发', () async {
      final k = FakeKernel(const {});
      final c = detail(k, SkillInfo.fromJson(skillJson('a', path: null)));
      await c.init();
      expect(c.error, 'skills.detail.loadFailed');
      expect(c.loadingList, isFalse);
      expect(k.calls, isEmpty);
    });

    test('默认选 SKILL.md，并立刻读它的内容', () async {
      final k = FakeKernel({
        'skill_detail': (_) => {
          'skill_name': 'a',
          'root': '/p/a',
          'files': [
            {'rel_path': 'refs/x.txt', 'size': 10, 'is_text': true},
            {'rel_path': 'SKILL.md', 'size': 2048, 'is_text': true},
          ],
        },
        'skill_read_file': (_) => {
          'content': '# hi',
          'truncated': false,
          'size': 4,
        },
      });
      final c = detail(k, SkillInfo.fromJson(skillJson('a', path: '/p/a')));
      await c.init();
      expect(c.selected, 'SKILL.md');
      expect(k.lastArgsOf('skill_read_file'), {
        'installedPath': '/p/a',
        'rel': 'SKILL.md',
      });
      expect(c.content!.content, '# hi');
      expect(c.selectedFile!.size, 2048);
    });

    test('没有 SKILL.md 就退第一条；空目录则不选也不读', () async {
      final k = FakeKernel({
        'skill_detail': (_) => {
          'files': [
            {'rel_path': 'a.txt', 'size': 1, 'is_text': true},
          ],
        },
        'skill_read_file': (_) => {'content': 'x'},
      });
      final c = detail(k, SkillInfo.fromJson(skillJson('a', path: '/p/a')));
      await c.init();
      expect(c.selected, 'a.txt');

      final k2 = FakeKernel({
        'skill_detail': (_) => {'files': <Object?>[]},
      });
      final c2 = detail(k2, SkillInfo.fromJson(skillJson('a', path: '/p/a')));
      await c2.init();
      expect(c2.selected, isNull);
      expect(k2.countOf('skill_read_file'), 0);
    });

    test('二进制文件：content 为 null，不预览', () async {
      final k = FakeKernel({
        'skill_detail': (_) => {
          'files': [
            {'rel_path': 'logo.png', 'size': 900, 'is_text': false},
          ],
        },
        'skill_read_file': (_) => {
          'content': null,
          'truncated': false,
          'size': 900,
        },
      });
      final c = detail(k, SkillInfo.fromJson(skillJson('a', path: '/p/a')));
      await c.init();
      expect(c.content!.content, isNull);
      expect(isMarkdownPath('logo.png'), isFalse);
      expect(isMarkdownPath('SKILL.MD'), isTrue, reason: '大小写不敏感');
    });

    test('读文件失败：记 error，loadingFile 收回', () async {
      final k = FakeKernel({
        'skill_detail': (_) => {
          'files': [
            {'rel_path': 'SKILL.md', 'size': 1, 'is_text': true},
          ],
        },
        'skill_read_file': (_) => throw StateError('denied'),
      });
      final c = detail(k, SkillInfo.fromJson(skillJson('a', path: '/p/a')));
      await c.init();
      expect(c.error, contains('denied'));
      expect(c.loadingFile, isFalse);
    });
  });
}

/// 极小的假时钟：只为「防抖 350ms」那一条，不引 `package:fake_async`
/// （pubspec 里没有它，为一条测试加依赖不划算）。
void fakeAsync(void Function(void Function(Duration)) body) {
  final timers = <(Duration, void Function())>[];
  runZoned(
    () {
      var now = Duration.zero;
      body((d) {
        now += d;
        final due = [
          for (final t in timers)
            if (t.$1 <= now) t,
        ];
        for (final t in due) {
          timers.remove(t);
          t.$2();
        }
      });
    },
    zoneSpecification: ZoneSpecification(
      createTimer: (self, parent, zone, duration, f) {
        late _FakeTimer timer;
        final entry = (duration, () => timer.fire(f));
        timers.add(entry);
        timer = _FakeTimer(() => timers.remove(entry));
        return timer;
      },
    ),
  );
}

class _FakeTimer implements Timer {
  _FakeTimer(this._onCancel);

  final void Function() _onCancel;
  bool _active = true;

  void fire(void Function() f) {
    if (_active) {
      _active = false;
      f();
    }
  }

  @override
  void cancel() {
    _active = false;
    _onCancel();
  }

  @override
  bool get isActive => _active;

  @override
  int get tick => 0;
}
