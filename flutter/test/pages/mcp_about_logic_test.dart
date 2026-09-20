/// MCP 页 / 关于页 / 模型测试面板逻辑层的单测（票 I09）。
///
/// React 侧这三块没有现成测试，断言逐条对着
/// `Mcp/useMcpData.ts` + `Mcp/constants.ts` / `About.tsx` / `ModelTestPanel.tsx` 写。
library;

import 'dart:convert';

import 'package:aidog_flutter/pages.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';

Map<String, Object?> serverJson(
  String name, {
  String transport = 'stdio',
  String command = 'npx',
  List<String> args = const ['-y', 'pkg'],
  Map<String, String> env = const {},
  String url = '',
  Map<String, String> headers = const {},
  List<String> agents = const [],
}) => {
  'id': 1,
  'name': name,
  'transport': transport,
  'command': command,
  'args': args,
  'env': env,
  'url': url,
  'headers': headers,
  'enabledAgents': agents,
  'createdAt': 0,
  'updatedAt': 0,
};

Map<String, Object?> scanJson(
  String name, {
  String transport = 'stdio',
  List<String> foundIn = const ['claude-code'],
  bool already = false,
}) => {
  'name': name,
  'transport': transport,
  'command': 'npx',
  'args': const <String>['-y', 'x'],
  'env': const <String, String>{},
  'url': '',
  'headers': const <String, String>{},
  'foundInAgents': foundIn,
  'alreadyImported': already,
};

FakeKernel fakeMcp({
  List<Map<String, Object?>>? servers,
  Map<String, Object? Function(Map<String, Object?>?)> overrides = const {},
}) => FakeKernel({
  'mcp_list': (_) => servers ?? [serverJson('fs', agents: ['claude-code'])],
  ...overrides,
});

Future<McpController> bootedMcp(FakeKernel k) async {
  final c = McpController(
    invoke: k.invoke,
    t: (key, [args]) => key,
    onChanged: () {},
  );
  await c.refresh();
  return c;
}

void main() {
  group('constants.ts 的三个纯函数', () {
    test('agentSupported: codex 仅 stdio；claude-code 全支持', () {
      expect(mcpAgentSupported('stdio', 'codex'), isTrue);
      expect(mcpAgentSupported('http', 'codex'), isFalse);
      expect(mcpAgentSupported('sse', 'codex'), isFalse);
      for (final t in const ['stdio', 'http', 'sse']) {
        expect(mcpAgentSupported(t, 'claude-code'), isTrue);
      }
    });

    test('summaryOf: stdio → command + 首参；http/sse → url（空给破折号）', () {
      expect(
        mcpSummaryOf(
          transport: 'stdio',
          command: 'npx',
          args: const ['-y', 'pkg'],
          url: '',
        ),
        'npx -y',
      );
      // 没有参数就只剩 command（不留尾随空格）
      expect(
        mcpSummaryOf(
          transport: 'stdio',
          command: 'node',
          args: const [],
          url: '',
        ),
        'node',
      );
      expect(
        mcpSummaryOf(
          transport: 'http',
          command: '',
          args: const [],
          url: 'https://x',
        ),
        'https://x',
      );
      expect(
        mcpSummaryOf(transport: 'sse', command: '', args: const [], url: ''),
        '—',
      );
    });

    test('normalizeMcpPaste: base64 且解码后是合法 JSON 才用解码结果', () {
      final json = jsonEncode({
        'mcpServers': {'a': 1},
      });
      final b64 = base64.encode(utf8.encode(json));
      expect(normalizeMcpPaste(b64), json);
      // 明文 JSON 原样（只 trim）
      expect(normalizeMcpPaste('  {"a":1}  '), '{"a":1}');
      // 形如 base64 但解码后不是 JSON → 保持原文本
      final notJson = base64.encode(utf8.encode('hello world hello world'));
      expect(normalizeMcpPaste(notJson), notJson);
      // 太短（≤16）不尝试解码
      expect(normalizeMcpPaste('abcd'), 'abcd');
    });
  });

  group('MCP 列表与启停', () {
    test('加载失败也把 loading 落下来并报错', () async {
      final k = FakeKernel({'mcp_list': (_) => throw StateError('db')});
      final c = await bootedMcp(k);
      expect(c.loading, isFalse);
      expect(c.message!.ok, isFalse);
      expect(c.message!.text, contains('db'));
    });

    test('启用 codex + http：直接报错，不发命令', () async {
      final k = fakeMcp(
        servers: [serverJson('remote', transport: 'http', url: 'https://x')],
        overrides: {'mcp_set_agent': (_) => null},
      );
      final c = await bootedMcp(k);
      await c.toggle(c.servers.first, 'codex');
      expect(k.countOf('mcp_set_agent'), 0);
      expect(c.message!.ok, isFalse);
      expect(c.message!.text, 'mcp.unsupportedTransport');
    });

    test('禁用总是允许（即使传输不被支持）', () async {
      final k = fakeMcp(
        servers: [
          serverJson(
            'remote',
            transport: 'http',
            url: 'https://x',
            agents: ['codex'],
          ),
        ],
        overrides: {'mcp_set_agent': (_) => null},
      );
      final c = await bootedMcp(k);
      await c.toggle(c.servers.first, 'codex');
      expect(k.lastArgsOf('mcp_set_agent'), {
        'name': 'remote',
        'agent': 'codex',
        'enabled': false,
      });
      expect(c.servers.first.enabledAgents, isEmpty);
    });

    test('乐观更新 + 失败回滚', () async {
      final k = fakeMcp(
        overrides: {'mcp_set_agent': (_) => throw StateError('write denied')},
      );
      final c = await bootedMcp(k);
      await c.toggle(c.servers.first, 'codex');
      expect(c.servers.first.enabledAgents, ['claude-code'], reason: '必须回滚');
      expect(c.message!.ok, isFalse);
    });

    test('有任务在跑时不并发', () async {
      final k = fakeMcp(overrides: {'mcp_set_agent': (_) => null});
      final c = await bootedMcp(k);
      c.busyKey = 'x';
      await c.toggle(c.servers.first, 'codex');
      expect(k.countOf('mcp_set_agent'), 0);
    });
  });

  group('扫描导入', () {
    test('默认预选所有未导入项', () async {
      final k = fakeMcp(
        overrides: {
          'mcp_scan': (_) => [
            scanJson('a'),
            scanJson('b', already: true),
            scanJson('c'),
          ],
        },
      );
      final c = await bootedMcp(k);
      await c.openScan();
      expect(c.selected, {'a', 'c'});
      expect(c.scanning, isFalse);
      expect(c.scanOpen, isTrue);
    });

    test('扫描失败：关掉弹窗并报错', () async {
      final k = fakeMcp(
        overrides: {'mcp_scan': (_) => throw StateError('no config')},
      );
      final c = await bootedMcp(k);
      await c.openScan();
      expect(c.scanOpen, isFalse);
      expect(c.message!.ok, isFalse);
    });

    test('导入载荷：sourceAgent 取首个发现 agent，空数组回落 claude-code', () async {
      final k = fakeMcp(
        overrides: {
          'mcp_scan': (_) => [
            scanJson('a', foundIn: const ['codex']),
            scanJson('b', foundIn: const []),
          ],
          'mcp_import': (_) => {
            'imported': ['a', 'b'],
            'skipped': <String>[],
          },
        },
      );
      final c = await bootedMcp(k);
      await c.openScan();
      await c.importSelected();
      final items = (k.lastArgsOf('mcp_import')!['items'] as List)
          .cast<Map<String, Object?>>();
      expect(items[0]['sourceAgent'], 'codex');
      expect(items[1]['sourceAgent'], 'claude-code');
      expect(c.scanOpen, isFalse);
      expect(c.message!.ok, isTrue);
    });

    test('有跳过项时消息按错误色出', () async {
      final k = fakeMcp(
        overrides: {
          'mcp_scan': (_) => [scanJson('a')],
          'mcp_import': (_) => {
            'imported': ['a'],
            'skipped': ['b'],
          },
        },
      );
      final c = await bootedMcp(k);
      await c.openScan();
      await c.importSelected();
      expect(c.message!.ok, isFalse);
      expect(c.message!.text, 'mcp.importPartial');
    });

    test('一条都没勾就不发命令', () async {
      final k = fakeMcp(
        overrides: {
          'mcp_scan': (_) => [scanJson('a', already: true)],
          'mcp_import': (_) => {'imported': <String>[], 'skipped': <String>[]},
        },
      );
      final c = await bootedMcp(k);
      await c.openScan();
      expect(c.selected, isEmpty);
      await c.importSelected();
      expect(k.countOf('mcp_import'), 0);
    });
  });

  group('粘贴 / deep-link 导入', () {
    test('空白文本不发命令', () async {
      final k = fakeMcp(
        overrides: {
          'mcp_import_json': (_) => {
            'imported': <String>[],
            'skipped': <String>[],
          },
        },
      );
      final c = await bootedMcp(k);
      c.setPasteText('   ');
      await c.pasteImport();
      expect(k.countOf('mcp_import_json'), 0);
    });

    test('粘贴 base64：解码后再发，成功后清空并关窗', () async {
      final k = fakeMcp(
        overrides: {
          'mcp_import_json': (_) => {
            'imported': ['a'],
            'skipped': <String>[],
          },
        },
      );
      final c = await bootedMcp(k);
      final json = jsonEncode({
        'mcpServers': {'a': <String, Object?>{}},
      });
      c.setPasteOpen(true);
      c.setPasteText(base64.encode(utf8.encode(json)));
      await c.pasteImport();
      expect(k.lastArgsOf('mcp_import_json'), {'json': json});
      expect(c.pasteOpen, isFalse);
      expect(c.pasteText, '');
      // 导入后必须整表重拉
      expect(k.countOf('mcp_list'), 2);
    });

    test('deep-link：不做 JSON 预检，坏 base64 只报错', () async {
      final k = fakeMcp(
        overrides: {
          'mcp_import_json': (_) => {
            'imported': ['x'],
            'skipped': <String>[],
          },
        },
      );
      final c = await bootedMcp(k);
      await c.openDeepLinkImport('!!!not base64!!!');
      expect(k.countOf('mcp_import_json'), 0);
      expect(c.message!.ok, isFalse);
      // 空 data 什么都不做
      c.clearMessage();
      await c.openDeepLinkImport('');
      expect(c.message, isNull);
    });
  });

  group('删除 / 重同步 / 分享', () {
    test('删除：确认之前不发命令；成功后就地移行不重拉', () async {
      final k = fakeMcp(overrides: {'mcp_delete': (_) => null});
      final c = await bootedMcp(k);
      c.askDelete(c.servers.first);
      expect(k.countOf('mcp_delete'), 0);
      c.cancelDelete();
      expect(c.deleteTarget, isNull);

      c.askDelete(c.servers.first);
      await c.delete();
      expect(k.lastArgsOf('mcp_delete'), {'name': 'fs'});
      expect(c.servers, isEmpty);
      expect(k.countOf('mcp_list'), 1, reason: '不整表重拉');
      expect(c.deleteTarget, isNull);
    });

    test('重同步：有任务在跑时不并发；成功带条数', () async {
      final k = fakeMcp(overrides: {'mcp_resync': (_) => 7});
      final c = await bootedMcp(k);
      c.busyKey = 'x';
      await c.resync();
      expect(k.countOf('mcp_resync'), 0);
      c.busyKey = null;
      await c.resync();
      expect(c.message!.ok, isTrue);
      expect(c.message!.text, 'mcp.resyncDone');
    });

    test('分享：拿到导出配置才开弹窗，失败只报错', () async {
      final k = fakeMcp(
        overrides: {
          'mcp_share_export': (_) => {'name': 'fs', 'command': 'npx'},
        },
      );
      final c = await bootedMcp(k);
      await c.share(c.servers.first);
      expect(c.shareData!.name, 'fs');
      expect(c.shareData!.share['command'], 'npx');
      c.closeShare();
      expect(c.shareData, isNull);

      final k2 = fakeMcp(
        overrides: {'mcp_share_export': (_) => throw StateError('nope')},
      );
      final c2 = await bootedMcp(k2);
      await c2.share(c2.servers.first);
      expect(c2.shareData, isNull);
      expect(c2.message!.ok, isFalse);
    });
  });

  group('编辑 / 新增表单', () {
    test('openEdit: args 按换行铺开，env / headers 铺成行', () async {
      final k = fakeMcp(
        servers: [
          serverJson(
            'fs',
            args: const ['a', 'b'],
            env: const {'K': '***'},
            headers: const {'H': 'v'},
          ),
        ],
      );
      final c = await bootedMcp(k);
      c.openEdit(c.servers.first);
      expect(c.editForm.argsText, 'a\nb');
      expect(c.editForm.envRows.single.k, 'K');
      expect(c.editForm.headersRows.single.v, 'v');
      expect(c.editTarget, isNotNull);
      expect(c.editOpen, isTrue);
    });

    test('openAdd: 空表单，editTarget = null 即「新增」', () async {
      final c = await bootedMcp(fakeMcp());
      c.openAdd();
      expect(c.editTarget, isNull);
      expect(c.editForm.name, '');
      expect(c.editForm.transport, 'stdio');
      expect(c.editForm.envRows, isEmpty);
    });

    test('name 空（trim 后）→ 只报错不发命令', () async {
      final k = fakeMcp(overrides: {'mcp_add': (_) => serverJson('x')});
      final c = await bootedMcp(k);
      c.openAdd();
      c.editForm.name = '   ';
      await c.saveEdit();
      expect(k.countOf('mcp_add'), 0);
      expect(c.message!.text, 'mcp.nameRequired');
    });

    test('新增走 mcp_add：args 按行 trim 剔空，env 只留 key 非空的行', () async {
      final k = fakeMcp(overrides: {'mcp_add': (_) => serverJson('new')});
      final c = await bootedMcp(k);
      c.openAdd();
      c.editForm
        ..name = '  new  '
        ..command = 'node'
        ..argsText = ' a \n\n  \n b '
        ..envRows = [KvRow(' K ', ' v '), KvRow('  ', 'dropped')];
      await c.saveEdit();
      final payload = (k.lastArgsOf('mcp_add')!['payload'] as Map)
          .cast<String, Object?>();
      expect(payload['name'], 'new');
      expect(payload['args'], ['a', 'b']);
      // key trim、**value 不 trim**（照搬 React）
      expect(payload['env'], {'K': ' v '});
      expect(c.editOpen, isFalse);
      expect(c.message!.text, 'mcp.saved');
      expect(k.countOf('mcp_list'), 2, reason: '保存后整表重拉');
    });

    test('编辑走 mcp_update 并带 oldName', () async {
      final k = fakeMcp(overrides: {'mcp_update': (_) => serverJson('fs2')});
      final c = await bootedMcp(k);
      c.openEdit(c.servers.first);
      c.editForm.name = 'fs2';
      await c.saveEdit();
      expect(k.lastArgsOf('mcp_update')!['oldName'], 'fs');
      expect(
        ((k.lastArgsOf('mcp_update')!['payload'] as Map)
            .cast<String, Object?>())['name'],
        'fs2',
      );
    });

    test('保存失败：报错但不关弹窗', () async {
      final k = fakeMcp(overrides: {'mcp_add': (_) => throw StateError('dup')});
      final c = await bootedMcp(k);
      c.openAdd();
      c.editForm.name = 'x';
      await c.saveEdit();
      expect(c.editOpen, isTrue);
      expect(c.message!.ok, isFalse);
      expect(c.busyKey, isNull);
    });

    test('弹窗没开时 saveEdit 直接返回', () async {
      final k = fakeMcp(overrides: {'mcp_add': (_) => serverJson('x')});
      final c = await bootedMcp(k);
      await c.saveEdit();
      expect(k.countOf('mcp_add'), 0);
    });
  });

  // ── 关于页 ─────────────────────────────────────────────────────

  group('关于页（About.tsx）', () {
    Map<String, Object?> toolJson(
      String name, {
      bool installed = true,
      bool broken = false,
      bool conflict = false,
      String? version = '1.0.0',
      String? path = '/usr/bin/x',
      String? latest,
      bool? hasUpdate,
    }) => {
      'name': name,
      'installed': installed,
      'version': version,
      'path': path,
      'broken': broken,
      'conflict': conflict,
      'latest_version': latest,
      'has_update': hasUpdate,
    };

    FakeKernel fakeAbout({
      Map<String, Object? Function(Map<String, Object?>?)> overrides = const {},
    }) => FakeKernel({
      'about_info': (_) => const {
        'app_version': '0.1.17',
        'tauri_version': '2.0.0',
        'os': 'macos',
        'arch': 'aarch64',
        'family': 'unix',
        'profile': 'release',
        'git_commit': 'abc1234',
        'build_time': '1700000000',
      },
      'cli_check_versions': (_) => [toolJson('claude')],
      'cli_check_updates': (_) => [
        toolJson('claude', latest: '1.2.0', hasUpdate: true),
      ],
      ...overrides,
    });

    Future<AboutController> bootedAbout(FakeKernel k) async {
      final c = AboutController(
        invoke: k.invoke,
        t: (key, [args]) => key,
        onChanged: () {},
      );
      await c.init();
      return c;
    }

    test('进页拉版本信息 + 立刻检查一次 CLI；更新探测并行跟一发', () async {
      final k = fakeAbout();
      final c = await bootedAbout(k);
      expect(c.info!.appVersion, '0.1.17');
      expect(c.info!.gitCommit, 'abc1234');
      expect(k.countOf('cli_check_versions'), 1);
      expect(k.countOf('cli_check_updates'), 1);
      expect(c.cliBusy, '');
    });

    test('about_info 失败 → info 为 null，不抛', () async {
      final c = await bootedAbout(
        fakeAbout(overrides: {'about_info': (_) => throw StateError('x')}),
      );
      expect(c.info, isNull);
    });

    test('更新探测失败是静默的：不写 cliErr，版本列表仍在', () async {
      final c = await bootedAbout(
        fakeAbout(
          overrides: {'cli_check_updates': (_) => throw StateError('offline')},
        ),
      );
      expect(c.cliErr, '');
      expect(c.cliTools, hasLength(1));
    });

    test('检查版本失败 → cliErr 带前缀文案', () async {
      final c = await bootedAbout(
        fakeAbout(
          overrides: {'cli_check_versions': (_) => throw StateError('boom')},
        ),
      );
      expect(c.cliErr, startsWith('about.localEnv.checkFailed'));
      expect(c.cliErr, contains('boom'));
      expect(c.cliBusy, '');
    });

    test('状态四分支的判定顺序：未安装 → 损坏 → 冲突 → 已安装', () {
      CliToolStatus s({
        bool installed = true,
        bool broken = false,
        bool conflict = false,
      }) => CliToolStatus.fromJson({
        'name': 'x',
        'installed': installed,
        'broken': broken,
        'conflict': conflict,
      });
      // 未安装压过其余一切
      expect(
        cliStatusKind(s(installed: false, broken: true, conflict: true)),
        CliStatusKind.notInstalled,
      );
      expect(
        cliStatusKind(s(broken: true, conflict: true)),
        CliStatusKind.broken,
      );
      expect(cliStatusKind(s(conflict: true)), CliStatusKind.conflict);
      expect(cliStatusKind(s()), CliStatusKind.installed);
      expect(cliStatusKey(CliStatusKind.broken), 'about.localEnv.broken');
    });

    test('安装：发 tool 参数、成功后重查、提示可见', () async {
      final k = fakeAbout(overrides: {'cli_install': (_) => null});
      final c = await bootedAbout(k);
      await c.installCli('codex');
      expect(k.lastArgsOf('cli_install'), {'tool': 'codex'});
      expect(k.countOf('cli_check_versions'), 2);
      // **有意偏离 React 的 bug**：那边成功提示被随后的 handleCliCheck 清掉了
      expect(c.cliMsg, 'about.localEnv.installSuccess');
      expect(c.cliBusy, '');
      expect(c.cliPendingTool, isNull);
    });

    test('安装失败 → cliErr，且 pending 清空', () async {
      final k = fakeAbout(
        overrides: {'cli_install': (_) => throw StateError('EACCES')},
      );
      final c = await bootedAbout(k);
      await c.installCli('pi');
      expect(c.cliErr, startsWith('about.localEnv.installFailed'));
      expect(c.cliPendingTool, isNull);
      expect(c.cliBusy, '');
    });

    test('升级（「修复」走的也是这条）', () async {
      final k = fakeAbout(overrides: {'cli_upgrade': (_) => null});
      final c = await bootedAbout(k);
      await c.upgradeCli('claude');
      expect(k.lastArgsOf('cli_upgrade'), {'tool': 'claude'});
      expect(c.cliMsg, 'about.localEnv.upgradeSuccess');
    });

    test('诊断：0 个冲突报 noConflicts，否则报数量', () async {
      final k = fakeAbout(
        overrides: {
          'cli_diagnose_conflicts': (_) => [
            {
              'tool': 'claude',
              'installations': [
                {
                  'path': '/a',
                  'version': '1',
                  'runnable': true,
                  'source': 'npm-global',
                  'is_path_default': true,
                },
              ],
              'is_conflicting': false,
              'suggestion': '',
            },
          ],
        },
      );
      final c = await bootedAbout(k);
      await c.diagnoseCli();
      expect(c.cliMsg, 'about.localEnv.noConflicts');
      expect(c.conflictFor('claude')!.installations.single.source, 'npm-global');
      expect(c.conflictFor('nope'), isNull);
    });

    test('诊断：只数 is_conflicting 为 true 的那些', () async {
      final k = fakeAbout(
        overrides: {
          'cli_diagnose_conflicts': (_) => [
            {
              'tool': 'a',
              'installations': <Object?>[],
              'is_conflicting': true,
              'suggestion': 's',
            },
            {
              'tool': 'b',
              'installations': <Object?>[],
              'is_conflicting': false,
              'suggestion': '',
            },
          ],
        },
      );
      final c = await bootedAbout(k);
      await c.diagnoseCli();
      expect(c.cliMsg, 'about.localEnv.conflictFound');
      expect(c.cliConflicts, hasLength(2));
    });

    test('诊断失败 → cliErr', () async {
      final k = fakeAbout(
        overrides: {'cli_diagnose_conflicts': (_) => throw StateError('nope')},
      );
      final c = await bootedAbout(k);
      await c.diagnoseCli();
      expect(c.cliErr, startsWith('about.localEnv.diagnoseFailed'));
    });

    test('GitHub 链接是常量，四条都在仓库域下', () {
      expect(kGithubLinks['repo'], kGithubRepo);
      expect(kGithubLinks['releases'], '$kGithubRepo/releases');
      expect(kGithubLinks['issues'], '$kGithubRepo/issues');
      expect(kGithubLinks['reportIssue'], '$kGithubRepo/issues/new');
      expect(kCliToolLabelKeys.keys.toList(), ['claude', 'codex', 'pi']);
    });

    test('本页不走 updater：一条更新检查类命令都不发', () async {
      final k = fakeAbout();
      await bootedAbout(k);
      expect(
        k.calls.toSet().toList()..sort(),
        ['about_info', 'cli_check_updates', 'cli_check_versions'],
      );
    });
  });

  // ── 模型测试面板 ────────────────────────────────────────────────

  group('模型测试面板（ModelTestPanel.tsx）', () {
    const platform = TestTargetPlatform(
      id: 7,
      name: 'GLM',
      platformType: 'glm',
      availableModels: ['m1', 'm2', 'm3', 'm4', 'm5', 'm6'],
      models: {'default': 'm1'},
    );

    ModelTestController ctl(
      FakeKernel k, {
      TestTargetPlatform p = platform,
      void Function(bool)? onResult,
    }) => ModelTestController(
      invoke: k.invoke,
      onChanged: () {},
      platform: p,
      onResult: onResult,
    );

    Map<String, Object?> result(String model, {bool ok = true}) => {
      'success': ok,
      'model': model,
      'prompt_preview': 'p',
      'response_preview': 'r',
      'duration_ms': 120,
      'input_tokens': 3,
      'output_tokens': 4,
      'error': ok ? '' : 'boom',
    };

    test('allModels：available_models 优先；否则按五个键的固定顺序取非空', () {
      expect(platform.allModels, ['m1', 'm2', 'm3', 'm4', 'm5', 'm6']);
      const p2 = TestTargetPlatform(
        id: 1,
        name: 'x',
        platformType: 'openai',
        models: {'gpt': 'g', 'sonnet': 's', 'default': ''},
      );
      // 顺序是 default / sonnet / opus / haiku / gpt，空值跳过
      expect(p2.allModels, ['s', 'g']);
      expect(p2.defaultModel, 's', reason: 'default 空 → 取列表第一条');
      const p3 = TestTargetPlatform(id: 1, name: 'x', platformType: 'y');
      expect(p3.defaultModel, '');
    });

    test('六种模式各自选出哪些模型', () {
      final c = ctl(FakeKernel(const {}));
      expect(c.models, ['m1']); // quick
      c.setMode('random');
      expect(c.models, platform.allModels);
      c.setMode('tool');
      expect(c.models, platform.allModels);
      c.setMode('batch');
      expect(c.models, ['m1', 'm2', 'm3', 'm4', 'm5'], reason: '未选则取前 5 条');
      c.setMode('single');
      expect(c.models, ['m1'], reason: '未选则取 default');
      c.toggleModel('m3');
      c.toggleModel('m4');
      expect(c.models, ['m3'], reason: 'single 只取第一条');
      c.setMode('batch');
      expect(c.models, ['m1', 'm2', 'm3', 'm4', 'm5'], reason: '切模式清空选中');
      c.toggleModel('m2');
      expect(c.models, ['m2']);
    });

    test('needsModelSelect 与 runDisabled', () {
      final c = ctl(FakeKernel(const {}));
      expect(c.needsModelSelect, isFalse);
      c.setMode('batch');
      expect(c.needsModelSelect, isTrue);
      // allModels 非空 → 即使没选也能跑
      expect(c.runDisabled, isFalse);
      // 平台一个模型都没有 + 需要选模型 + 未选 + 非 batch → 禁用
      final c2 = ctl(
        FakeKernel(const {}),
        p: const TestTargetPlatform(id: 1, name: 'x', platformType: 'y'),
      );
      c2.setMode('single');
      expect(c2.runDisabled, isTrue);
    });

    test('串行跑：每条一次 model_test，结果逐条推进列表', () async {
      final k = FakeKernel({
        'model_test': (args) {
          final req = ((args!['req']) as Map).cast<String, Object?>();
          return result(req['model'] as String);
        },
      });
      var allOk = false;
      final c = ctl(k, onResult: (v) => allOk = v);
      c.setMode('batch');
      c.toggleModel('m1');
      c.toggleModel('m2');
      await c.run();
      expect(k.countOf('model_test'), 2);
      expect([for (final r in c.results) r.model], ['m1', 'm2']);
      expect(c.results.first.inputTokens, 3);
      expect(c.results.first.outputTokens, 4);
      expect(c.running, isFalse);
      expect(c.currentIdx, -1);
      expect(allOk, isTrue);
    });

    test('custom 模式才带 prompt，tool 模式才带 tool_test', () async {
      final k = FakeKernel({'model_test': (_) => result('m1')});
      final c = ctl(k);
      await c.run(); // quick
      var req = (k.lastArgsOf('model_test')!['req'] as Map);
      expect(req.containsKey('prompt'), isFalse);
      expect(req.containsKey('tool_test'), isFalse);
      expect(req['platform_id'], 7);

      c.setMode('custom');
      c.setCustomPrompt('hello');
      await c.run();
      req = (k.lastArgsOf('model_test')!['req'] as Map);
      expect(req['prompt'], 'hello');

      // custom 但 prompt 为空 → 不带（React 的 `mode==="custom" && customPrompt`）
      c.setMode('custom');
      c.setCustomPrompt('');
      await c.run();
      req = (k.lastArgsOf('model_test')!['req'] as Map);
      expect(req.containsKey('prompt'), isFalse);

      c.setMode('tool');
      await c.run();
      req = (k.lastArgsOf('model_test')!['req'] as Map);
      expect(req['tool_test'], isTrue);
    });

    test('单条抛异常：兜底成一行失败结果，整轮继续', () async {
      var n = 0;
      final k = FakeKernel({
        'model_test': (args) {
          final req = ((args!['req']) as Map).cast<String, Object?>();
          if (n++ == 0) throw StateError('timeout');
          return result(req['model'] as String);
        },
      });
      var allOk = true;
      final c = ctl(k, onResult: (v) => allOk = v);
      c.setMode('batch');
      c.toggleModel('m1');
      c.toggleModel('m2');
      await c.run();
      expect(c.results, hasLength(2));
      expect(c.results.first.success, isFalse);
      expect(c.results.first.model, 'm1');
      expect(c.results.first.error, contains('timeout'));
      expect(c.results.first.durationMs, 0);
      expect(c.results[1].success, isTrue);
      expect(allOk, isFalse, reason: '有一条失败就不算全绿');
    });

    test('空模型清单：直接返回，连 running 都不置', () async {
      final k = FakeKernel({'model_test': (_) => result('x')});
      final c = ctl(
        k,
        p: const TestTargetPlatform(id: 1, name: 'x', platformType: 'y'),
      );
      c.setMode('random'); // allModels 为空
      await c.run();
      expect(k.countOf('model_test'), 0);
      expect(c.running, isFalse);
      expect(c.results, isEmpty);
    });

    test('模式列表与 React 的按钮顺序一致', () {
      expect(kTestModes, [
        'quick',
        'single',
        'batch',
        'random',
        'custom',
        'tool',
      ]);
    });
  });
}
