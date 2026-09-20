/// 平台页逻辑层的回归测试（票 I07）。
///
/// 第一组是 `src/pages/platforms/usePlatformsState.test.ts` 的逐条翻译
/// （那份测试盯的是 07-10「Groups 删平台只从组里移除」那个回归），
/// **数据与期望值一字不改**：三个平台 a/b/c，删 id=2，断言 epoch++ 与派生集合。
library;

import 'package:aidog_flutter/src/pages/models.dart';
import 'package:aidog_flutter/src/pages/platforms_logic.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';

/// `usePlatformsState.test.ts:80-91` 的 `mkPlatform(id, name)`，字段照抄。
Map<String, dynamic> mkPlatform(int id, String name) => {
  'id': id,
  'name': name,
  'platform_type': 'openai',
  'base_url': 'u$id',
  'api_key': 'k',
  'extra': '',
  'models': {'default': 'm', 'search': '', 'ask': ''},
  'available_models': ['m'],
  'endpoints': <Object?>[],
  'enabled': true,
  'status': 'enabled',
  'auto_disabled_until': 0,
  'auto_disable_strikes': 0,
  'created_at': 0,
  'updated_at': 0,
  'deleted_at': 0,
  'est_balance_remaining': 0,
  'est_coding_plan': '',
  'last_real_query_at': 0,
  'estimate_count': 0,
  'show_in_tray': false,
  'tray_display': '',
  'manual_budgets': <Object?>[],
  'expires_at': 0,
};

/// `usePlatformsState.test.ts:110-118` 的 beforeEach 桩。
FakeInvoke platformsFake() => FakeInvoke({
  'platform_list': [mkPlatform(1, 'a'), mkPlatform(2, 'b'), mkPlatform(3, 'c')],
  'platform_delete': null,
  'platform_update': mkPlatform(2, 'b'),
  'platform_create': mkPlatform(9, 'new'),
  'group_detail_list': <Object?>[], // 空 groupDetails：所有平台都未分组
  'all_platform_usage_stats': <String, Object?>{},
  'get_last_test_result': null,
  'scheduling_settings_get': null,
  'get_defaults_json': '{"protocols":{}}',
  'platform_usage_stats': null,
  'platform_reorder': null,
  'platform_purge_disabled_preview': <Object?>[],
  'platform_purge_disabled': {
    'deletedIds': <Object?>[],
    'unassignedIds': <Object?>[],
  },
  'platform_share_export': {'name': 'a'},
  'platform_share_parse': {'name': 'a'},
  'platform_fetch_models': <Object?>[],
  'platform_query_quota': {'success': true, 'queried_at': 1},
  'platform_query_quota_newapi': {'success': true, 'queried_at': 1},
  'platform_query_quota_devin': {'success': true, 'queried_at': 1},
  'group_platform_move': null,
  'model_test': {'success': true, 'duration_ms': 7, 'error': ''},
  'set_ui_extra': null,
});

void main() {
  group(
    'usePlatformsState — refreshPlatforms（React: usePlatformsState.test.ts 逐条翻译）',
    () {
      test('refreshPlatforms 全量 refetch setPlatforms + ++epoch', () async {
        final k = platformsFake();
        final c = PlatformsController(invoke: k.fn);
        await c.init();
        expect(c.platforms.length, 3);
        final epochBefore = c.epoch;

        // 模拟删后后端返回：被删 id=2 不在列表
        k.responses['platform_list'] = [mkPlatform(1, 'a'), mkPlatform(3, 'c')];
        await c.refreshPlatforms();

        expect(k.commands.contains('platform_list'), isTrue);
        expect(c.platforms.map((p) => p.id), [1, 3]);
        expect(c.epoch, epochBefore + 1);
      });

      test('refreshPlatforms 后 standalonePlatforms 派生不含被删 id（R4）', () async {
        final k = platformsFake();
        final c = PlatformsController(invoke: k.fn);
        await c.init();
        expect(c.standalonePlatforms.map((p) => p.id), [1, 2, 3]);

        k.responses['platform_list'] = [mkPlatform(1, 'a'), mkPlatform(3, 'c')];
        await c.refreshPlatforms();

        expect(c.standalonePlatforms.map((p) => p.id), [1, 3]);
        expect(
          c.standalonePlatforms.where((p) => p.id == 2),
          isEmpty,
        );
      });

      test(
        'confirmDeletePlatform 信号链：onPlatformDeleted 绑 refreshPlatforms 后被删平台从 standalone 消失',
        () async {
          final k = platformsFake();
          final c = PlatformsController(invoke: k.fn);
          await c.init();
          expect(c.standalonePlatforms.map((p) => p.id), [1, 2, 3]);

          // 用户在 Groups 删 id=2：后端真删后 refreshPlatforms 拉回删后集
          k.responses['platform_list'] = [mkPlatform(1, 'a'), mkPlatform(3, 'c')];
          await c.refreshPlatforms();

          expect(c.standalonePlatforms.map((p) => p.id), [1, 3]);
          // 关键：被删平台不再以「未分组」残留
          expect(c.standalonePlatforms.any((p) => p.id == 2), isFalse);
        },
      );
    },
  );

  group('usePlatformsState — handleDelete（React: R3 Platforms 页路径复验）', () {
    test('乐观 setPlatforms(filter) + ++epoch + platformApi.delete + groupDetails refetch', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      expect(c.platforms.length, 3);
      final epochBefore = c.epoch;
      final groupsBefore = k.callsTo('group_detail_list').length;

      await c.deletePlatform(2);

      // 1. platform_delete 被调（R3 入口）
      expect(k.lastCallTo('platform_delete')!.args!['id'], 2);
      // 2. 乐观更新：platforms state 立即不含被删 id
      expect(c.platforms.map((p) => p.id), [1, 3]);
      // 3. epoch 自增（派生层重算触发）
      expect(c.epoch, epochBefore + 1);
      // 4. groupDetails refetch
      expect(k.callsTo('group_detail_list').length, groupsBefore + 1);
      // 5. standalonePlatforms 派生正确（被删 id 不含）
      expect(c.standalonePlatforms.map((p) => p.id), [1, 3]);
    });

    test('handleDelete 失败时 platform_delete 仍被调（错误处理路径不阻塞入口契约）', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      final epochBefore = c.epoch;

      k.errors['platform_delete'] = StateError('boom');
      await c.deletePlatform(2);

      expect(k.lastCallTo('platform_delete')!.args!['id'], 2);
      expect(c.epoch, epochBefore + 1);
    });

    test('删除失败 → 被删平台插回**原位**，不是追加到末尾', () async {
      // React 那份测试明说回滚在 jsdom 下时序不稳、不在 scope 内；
      // Dart 这边不依赖调度时序，所以把这条补上。
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      k.errors['platform_delete'] = StateError('boom');
      await c.deletePlatform(2);
      expect(c.platforms.map((p) => p.id), [1, 2, 3]);
    });
  });

  group('epoch 守卫：慢响应晚到不许覆盖乐观结果', () {
    test('load 在途期间发生删除 → 放弃整列表覆盖', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      // 模拟：load 取到快照后、写回前，发生了一次本地乐观写。
      final loading = c.load();
      c.removePlatformsByIds([2]);
      await loading;
      expect(
        c.platforms.any((p) => p.id == 2),
        isFalse,
        reason: '晚到的 load 不许把删掉的行带回来',
      );
    });

    test('removePlatformsByIds 空列表 → 不动 epoch', () async {
      final c = PlatformsController(invoke: platformsFake().fn);
      await c.init();
      final before = c.epoch;
      c.removePlatformsByIds([]);
      expect(c.epoch, before);
    });

    test('refreshStats 只 merge 七个统计字段，不换整行', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      k.responses['platform_list'] = [
        {...mkPlatform(1, '改过的名字'), 'est_balance_remaining': 42.0},
        mkPlatform(2, 'b'),
        mkPlatform(3, 'c'),
      ];
      await c.refreshStats();
      final p1 = c.platforms.firstWhere((p) => p.id == 1);
      expect(p1.estBalanceRemaining, 42.0, reason: '统计字段要跟上');
      expect(p1.name, 'a', reason: '名字这种非统计字段不该被后台刷新改掉');
    });

    test('七个统计字段全等 → 保留原对象引用（memo 不重渲染）', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      final before = c.platforms;
      await c.refreshStats();
      expect(identical(c.platforms, before), isTrue);
    });

    test('refreshStats 在途遇到乐观写 → 不覆盖', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      final f = c.refreshStats();
      c.removePlatformsByIds([1]);
      await f;
      expect(c.platforms.any((p) => p.id == 1), isFalse);
    });
  });

  group('三态启停（usePlatformsState.ts:513）', () {
    test('enabled → disabled', () async {
      final k = platformsFake();
      k.responses['platform_update'] = {
        ...mkPlatform(2, 'b'),
        'status': 'disabled',
        'enabled': false,
      };
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      await c.togglePlatform(c.platforms.firstWhere((p) => p.id == 2));
      final input =
          k.lastCallTo('platform_update')!.args!['input']! as Map<String, Object?>;
      expect(input['status'], 'disabled');
      expect(c.platforms.firstWhere((p) => p.id == 2).enabled, isFalse);
    });

    test('auto_disabled → enabled（不是 → disabled）', () async {
      final k = platformsFake();
      k.responses['platform_list'] = [
        {...mkPlatform(1, 'a'), 'status': 'auto_disabled', 'enabled': false},
      ];
      k.responses['platform_update'] = {...mkPlatform(1, 'a'), 'status': 'enabled'};
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      await c.togglePlatform(c.platforms.single);
      final input =
          k.lastCallTo('platform_update')!.args!['input']! as Map<String, Object?>;
      expect(input['status'], 'enabled');
    });

    test('失败 → 回滚那一行并报错，其他行不动', () async {
      final k = platformsFake();
      k.errors['platform_update'] = StateError('boom');
      final toasts = <String>[];
      final c = PlatformsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add('$ok|$t'),
      );
      await c.init();
      await c.togglePlatform(c.platforms.firstWhere((p) => p.id == 2));
      expect(c.platforms.firstWhere((p) => p.id == 2).status, 'enabled');
      expect(c.platforms.length, 3);
      expect(toasts.single, startsWith('false|b: 切换失败'));
    });

    test('启停不刷 groupDetails（状态不改分组归属）', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      final before = k.callsTo('group_detail_list').length;
      await c.togglePlatform(c.platforms.first);
      expect(k.callsTo('group_detail_list').length, before);
    });
  });

  group('membership 与未分组派生', () {
    test('buildMembership：未出现在任何组里的 id 不进 map', () {
      final m = buildMembership([
        GroupDetail.fromJson({
          'group': {'id': 1, 'name': 'G1'},
          'platforms': [
            {'platform': mkPlatform(1, 'a')},
          ],
        }),
        GroupDetail.fromJson({
          'group': {'id': 2, 'name': 'G2'},
          'platforms': [
            {'platform': mkPlatform(1, 'a')},
            {'platform': mkPlatform(3, 'c')},
          ],
        }),
      ]);
      expect(m[1], ['G1', 'G2']);
      expect(m[3], ['G2']);
      expect(m.containsKey(2), isFalse);
    });

    test('已分组的平台不出现在主列表（避免与分组卡重复）', () async {
      final k = platformsFake();
      k.responses['group_detail_list'] = [
        {
          'group': {'id': 1, 'name': 'G1'},
          'platforms': [
            {'platform': mkPlatform(2, 'b')},
          ],
        },
      ];
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      expect(c.standalonePlatforms.map((p) => p.id), [1, 3]);
    });

    test('搜索按 name / base_url / 协议 / 协议词条匹配', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      c.setSearchQuery('u2');
      expect(c.standalonePlatforms.map((p) => p.id), [2]);
      c.setSearchQuery('openai');
      expect(c.standalonePlatforms.length, 3);
      c.setSearchQuery('没有这个');
      expect(c.standalonePlatforms, isEmpty);
      c.setSearchQuery('   ');
      expect(c.standalonePlatforms.length, 3, reason: '全空格 = 不过滤');
    });

    test('enabledCount 只数 enabled', () async {
      final k = platformsFake();
      k.responses['platform_list'] = [
        mkPlatform(1, 'a'),
        {...mkPlatform(2, 'b'), 'enabled': false, 'status': 'disabled'},
      ];
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      expect(c.enabledCount, 1);
    });
  });

  group('主 base_url 推导（usePlatformQuota.ts:13）', () {
    test('优先取与主协议同名的端点', () {
      expect(
        getPrimaryBaseUrl('anthropic', [
          const PlatformEndpoint(
            protocol: 'openai',
            baseUrl: 'A',
            clientType: '',
            codingPlan: false,
          ),
          const PlatformEndpoint(
            protocol: 'anthropic',
            baseUrl: 'B',
            clientType: '',
            codingPlan: false,
          ),
        ]),
        'B',
      );
    });

    test('没有同名的就取第一条；一条都没有就空串', () {
      expect(
        getPrimaryBaseUrl('gemini', [
          const PlatformEndpoint(
            protocol: 'openai',
            baseUrl: 'A',
            clientType: '',
            codingPlan: false,
          ),
        ]),
        'A',
      );
      expect(getPrimaryBaseUrl('openai', const []), '');
    });
  });

  group('模型载荷（usePlatformForm.ts:554）', () {
    test('全空 → null（= 不带 models 字段）', () {
      expect(buildModelsPayload({'default': '', 'sonnet': '  '}), isNull);
    });

    test('有一个非空 → 整份带上，空槽写 null，非空槽 trim', () {
      expect(buildModelsPayload({'default': '  m  ', 'gpt': ''}), {
        'default': 'm',
        'sonnet': null,
        'opus': null,
        'haiku': null,
        'gpt': null,
      });
    });
  });

  group('协议特例（usePlatformForm.ts:239-243）', () {
    test('claude_code 是纯透传', () {
      expect(isPassthroughProtocol('claude_code'), isTrue);
      expect(isPassthroughProtocol('openai'), isFalse);
    });

    test('opencode_zen 的 key 可空', () {
      expect(isKeyOptionalProtocol('opencode_zen'), isTrue);
      expect(apiKeyMissing('opencode_zen', ''), isFalse);
      expect(apiKeyMissing('openai', ''), isTrue);
      expect(apiKeyMissing('openai', 'sk-x'), isFalse);
    });
  });

  group('获取模型的多协议回退链（usePlatformForm.ts:471）', () {
    const eps = [
      PlatformEndpoint(
        protocol: 'anthropic',
        baseUrl: 'ANT',
        clientType: '',
        codingPlan: false,
      ),
      PlatformEndpoint(
        protocol: 'openai',
        baseUrl: 'OAI',
        clientType: '',
        codingPlan: false,
      ),
    ];

    test('openai 端点排在最前面试', () async {
      final k = platformsFake();
      k.responses['platform_fetch_models'] = ['m1'];
      final c = PlatformsController(invoke: k.fn);
      final (models, err) = await c.fetchModels(
        protocol: 'anthropic',
        apiKey: 'sk',
        endpoints: eps,
      );
      expect(models, ['m1']);
      expect(err, isNull);
      expect(k.callsTo('platform_fetch_models').first.args!['baseUrl'], 'OAI');
    });

    test('没 key 且协议要 key → 一条命令都不发', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.fetchModels(protocol: 'openai', apiKey: '', endpoints: eps);
      expect(k.commands.contains('platform_fetch_models'), isFalse);
    });

    test('空列表不算成功，继续试下一个端点', () async {
      final k = platformsFake();
      var n = 0;
      k.responses['platform_fetch_models'] = () => n++ == 0 ? <Object?>[] : ['m2'];
      final c = PlatformsController(invoke: k.fn);
      final (models, _) = await c.fetchModels(
        protocol: 'anthropic',
        apiKey: 'sk',
        endpoints: eps,
      );
      expect(models, ['m2']);
      expect(k.callsTo('platform_fetch_models').length, 2);
    });

    test('全部端点都返空 → 报「未获取到模型」', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      final (models, err) = await c.fetchModels(
        protocol: 'anthropic',
        apiKey: 'sk',
        endpoints: eps,
      );
      expect(models, isEmpty);
      expect(err, '未获取到模型');
    });

    test('端点按 (协议, URL) 去重，同一个不试两遍', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.fetchModels(
        protocol: 'openai',
        apiKey: 'sk',
        endpoints: const [
          PlatformEndpoint(
            protocol: 'openai',
            baseUrl: 'SAME',
            clientType: '',
            codingPlan: false,
          ),
        ],
      );
      expect(k.callsTo('platform_fetch_models').length, 1);
    });

    test('没有可试的端点 → 不发命令、不报错', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      final (models, err) = await c.fetchModels(
        protocol: 'openai',
        apiKey: 'sk',
        endpoints: const [],
      );
      expect(models, isEmpty);
      expect(err, isNull);
      expect(k.commands.contains('platform_fetch_models'), isFalse);
    });
  });

  group('保存平台', () {
    test('创建带 auto_group，更新不带', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      await c.savePlatform(
        name: 'n',
        protocol: 'openai',
        apiKey: 'sk',
        models: const {'default': 'm'},
        availableModels: const ['m'],
        endpoints: const [],
        extra: '',
        joinGroupIds: const [],
        expiresAt: 0,
      );
      var input =
          k.lastCallTo('platform_create')!.args!['input']! as Map<String, Object?>;
      expect(input['auto_group'], true);

      await c.savePlatform(
        name: 'n',
        protocol: 'openai',
        apiKey: 'sk',
        models: const {'default': 'm'},
        availableModels: const ['m'],
        endpoints: const [],
        extra: '',
        joinGroupIds: const [],
        expiresAt: 0,
        editingId: 1,
      );
      input =
          k.lastCallTo('platform_update')!.args!['input']! as Map<String, Object?>;
      expect(input.containsKey('auto_group'), isFalse);
      expect(input['id'], 1);
    });

    test('更新时 manual_budgets 为空也要带（表示清空）；创建时空则不带', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      await c.savePlatform(
        name: 'n',
        protocol: 'openai',
        apiKey: 'sk',
        models: const {},
        availableModels: const [],
        endpoints: const [],
        extra: '',
        joinGroupIds: const [],
        expiresAt: 0,
        editingId: 1,
      );
      var input =
          k.lastCallTo('platform_update')!.args!['input']! as Map<String, Object?>;
      expect(input['manual_budgets'], isEmpty);

      await c.savePlatform(
        name: 'n',
        protocol: 'openai',
        apiKey: 'sk',
        models: const {},
        availableModels: const [],
        endpoints: const [],
        extra: '',
        joinGroupIds: const [],
        expiresAt: 0,
      );
      input =
          k.lastCallTo('platform_create')!.args!['input']! as Map<String, Object?>;
      expect(input.containsKey('manual_budgets'), isFalse);
    });

    test('纯透传平台的手动预算一律清空', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      await c.savePlatform(
        name: 'n',
        protocol: 'claude_code',
        apiKey: '',
        models: const {},
        availableModels: const [],
        endpoints: const [],
        extra: '',
        joinGroupIds: const [],
        expiresAt: 0,
        manualBudgets: const [
          {'limit': 10},
        ],
        editingId: 1,
      );
      final input =
          k.lastCallTo('platform_update')!.args!['input']! as Map<String, Object?>;
      expect(input['manual_budgets'], isEmpty);
    });

    test('保存成功 → epoch++ 且补刷用量 / 最近测试 / 分组归属', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      final epochBefore = c.epoch;
      final saved = await c.savePlatform(
        name: 'n',
        protocol: 'openai',
        apiKey: 'sk',
        models: const {},
        availableModels: const [],
        endpoints: const [],
        extra: '',
        joinGroupIds: const [],
        expiresAt: 0,
      );
      expect(saved!.id, 9);
      expect(c.epoch, epochBefore + 1);
      expect(c.platforms.any((p) => p.id == 9), isTrue);
      expect(k.lastCallTo('platform_usage_stats')!.args!['platformId'], 9);
      expect(k.callsTo('get_last_test_result').any((x) => x.args!['platformId'] == 9), isTrue);
      expect(c.quotaPending[9], isTrue, reason: '局部保存没走 load，余额要自己补排一次');
    });

    test('保存失败 → 返回 null 并报错，列表不动', () async {
      final k = platformsFake();
      k.errors['platform_create'] = StateError('boom');
      final toasts = <String>[];
      final c = PlatformsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add('$ok|$t'),
      );
      await c.init();
      final saved = await c.savePlatform(
        name: 'n',
        protocol: 'openai',
        apiKey: 'sk',
        models: const {},
        availableModels: const [],
        endpoints: const [],
        extra: '',
        joinGroupIds: const [],
        expiresAt: 0,
      );
      expect(saved, isNull);
      expect(c.platforms.length, 3);
      expect(toasts.single, startsWith('false|保存失败'));
    });

    test('extra 为空串不带该字段（避免把已有 extra 写成空）', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      await c.savePlatform(
        name: 'n',
        protocol: 'openai',
        apiKey: 'sk',
        models: const {},
        availableModels: const [],
        endpoints: const [],
        extra: '',
        joinGroupIds: const [],
        expiresAt: 0,
      );
      final input =
          k.lastCallTo('platform_create')!.args!['input']! as Map<String, Object?>;
      expect(input.containsKey('extra'), isFalse);
    });
  });

  group('余额查询', () {
    test('没 key 不查；没 base_url 不查', () {
      final c = PlatformsController(invoke: platformsFake().fn);
      final noKey = PlatformRow.fromJson({...mkPlatform(1, 'a'), 'api_key': ''});
      expect(c.platformWantsQuota(noKey), isFalse);
      final noUrl = PlatformRow.fromJson({
        ...mkPlatform(1, 'a'),
        'base_url': '',
        'endpoints': <Object?>[],
      });
      expect(c.platformWantsQuota(noUrl), isFalse);
      expect(
        c.platformWantsQuota(PlatformRow.fromJson(mkPlatform(1, 'a'))),
        isTrue,
      );
    });

    test('按协议选对命令：newapi / devin 多传 extra', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.refreshQuota(
        PlatformRow.fromJson({...mkPlatform(1, 'a'), 'platform_type': 'newapi'}),
      );
      expect(k.lastCallTo('platform_query_quota_newapi')!.args!.containsKey('extra'), isTrue);

      await c.refreshQuota(
        PlatformRow.fromJson({...mkPlatform(2, 'b'), 'platform_type': 'devin'}),
      );
      expect(k.lastCallTo('platform_query_quota_devin')!.args!.containsKey('extra'), isTrue);

      await c.refreshQuota(PlatformRow.fromJson(mkPlatform(3, 'c')));
      expect(k.lastCallTo('platform_query_quota')!.args!.containsKey('extra'), isFalse);
    });

    test('手动刷新没 key → 直接提示，不发命令', () async {
      final k = platformsFake();
      final toasts = <String>[];
      final c = PlatformsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add('$ok|$t'),
      );
      await c.refreshQuota(
        PlatformRow.fromJson({...mkPlatform(1, 'a'), 'api_key': ''}),
      );
      expect(k.commands.contains('platform_query_quota'), isFalse);
      expect(toasts.single, 'false|a: 缺少 Token');
    });

    test('上游返回 success:false → 报它给的错误文案', () async {
      final k = platformsFake();
      k.responses['platform_query_quota'] = {
        'success': false,
        'error': '402 余额不足',
      };
      final toasts = <String>[];
      final c = PlatformsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add(t),
      );
      await c.refreshQuota(PlatformRow.fromJson(mkPlatform(1, 'a')));
      expect(toasts.single, 'a: 402 余额不足');
      expect(c.quotaRefreshing[1], isFalse, reason: '转圈必须停');
    });

    test('后台批量查：并发不超过上限，且每个 pending 都结算', () async {
      final k = platformsFake();
      k.responses['platform_list'] = [
        for (var i = 1; i <= 10; i++) mkPlatform(i, 'p$i'),
      ];
      var active = 0;
      var peak = 0;
      k.responses['platform_query_quota'] = () {
        active++;
        if (active > peak) peak = active;
        active--;
        return {'success': true, 'queried_at': 1};
      };
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      expect(c.quotaPending.length, 10);
      await c.pumpQuota();
      expect(k.callsTo('platform_query_quota').length, 10);
      expect(c.quotaPending, isEmpty, reason: 'pending 必须全部结算，否则卡在骨架态');
      expect(peak, lessThanOrEqualTo(kQuotaConcurrency));
    });

    test('单个平台查失败不影响别的，pending 照样结算', () async {
      final k = platformsFake();
      k.errors['platform_query_quota'] = StateError('boom');
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      await c.pumpQuota();
      expect(c.quotaPending, isEmpty);
      expect(c.quotaMap, isEmpty);
    });
  });

  group('清理失效平台（全局）', () {
    test('先预览，确认才删', () async {
      final k = platformsFake();
      k.responses['platform_purge_disabled_preview'] = [
        {'id': 2, 'reason': 'expired', 'action': 'delete'},
      ];
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      await c.askPurgeDisabled();
      expect(k.commands.contains('platform_purge_disabled'), isFalse);
      expect(c.purgeCandidates!.length, 1);
      await c.confirmPurgeDisabled();
      expect(k.lastCallTo('platform_purge_disabled')!.args!['groupId'], isNull);
    });

    test('取消 → 候选清空且不发命令', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      await c.askPurgeDisabled();
      c.cancelPurgeDisabled();
      expect(c.purgeCandidates, isNull);
      expect(k.commands.contains('platform_purge_disabled'), isFalse);
    });

    test('删掉的按 id 局部移除 + epoch++', () async {
      final k = platformsFake();
      k.responses['platform_purge_disabled'] = {
        'deletedIds': [2],
        'unassignedIds': <Object?>[],
      };
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      final epochBefore = c.epoch;
      await c.askPurgeDisabled();
      await c.confirmPurgeDisabled();
      expect(c.platforms.map((p) => p.id), [1, 3]);
      expect(c.epoch, epochBefore + 1);
    });

    test('什么都没删 → 提示「暂无失效平台」，列表不动', () async {
      final k = platformsFake();
      final toasts = <String>[];
      final c = PlatformsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add(t),
      );
      await c.init();
      await c.askPurgeDisabled();
      await c.confirmPurgeDisabled();
      expect(toasts, contains('暂无失效平台'));
      expect(c.platforms.length, 3);
    });
  });

  group('快速测试与其它单平台动作', () {
    test('测试成功 → 结果记 ok，带耗时', () async {
      final k = platformsFake();
      final toasts = <String>[];
      final c = PlatformsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add(t),
      );
      await c.init();
      await c.quickTest(c.platforms.first);
      expect(c.testResults[1], 'ok');
      expect(toasts.single, 'a: 测试成功 (7ms)');
      expect(c.testingId, isNull);
    });

    test('耗时为 0 时不带括号', () async {
      final k = platformsFake();
      k.responses['model_test'] = {
        'success': true,
        'duration_ms': 0,
        'error': '',
      };
      final toasts = <String>[];
      final c = PlatformsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add(t),
      );
      await c.init();
      await c.quickTest(c.platforms.first);
      expect(toasts.single, 'a: 测试成功');
    });

    test('测试完刷新该平台的「最近测试」徽章', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      final before = k.callsTo('get_last_test_result').length;
      await c.quickTest(c.platforms.first);
      expect(k.callsTo('get_last_test_result').length, before + 1);
    });

    test('分享导出失败 → 返回 null 并提示', () async {
      final k = platformsFake();
      k.errors['platform_share_export'] = StateError('boom');
      final toasts = <String>[];
      final c = PlatformsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add(t),
      );
      await c.init();
      final r = await c.shareExport(c.platforms.first);
      expect(r, isNull);
      expect(toasts.single, 'a: 生成分享内容失败');
    });

    test('拖进分组 → group_platform_move 的 fromGroupId 是 0，并刷 membership', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      final before = k.callsTo('group_detail_list').length;
      await c.moveIntoGroup(1, 5);
      expect(k.lastCallTo('group_platform_move')!.args, {
        'platformId': 1,
        'fromGroupId': 0,
        'toGroupId': 5,
      });
      expect(k.callsTo('group_detail_list').length, before + 1);
    });

    test('展开态落盘用 _ui_expand_plat 这个键（与组内卡的 _ui_expand_grp 区分）', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.persistExpanded(7, true);
      expect(k.lastCallTo('set_ui_extra')!.args, {
        'target': 'platform',
        'id': 7,
        'key': '_ui_expand_plat',
        'value': true,
      });
    });

    test('删除确认态：ask 不发命令，取消后也不发', () async {
      final k = platformsFake();
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      c.askDelete(2);
      expect(c.deleteTarget, 2);
      expect(k.commands.contains('platform_delete'), isFalse);
      c.cancelDelete();
      expect(c.deleteTarget, isNull);
    });
  });

  group('加载兜底', () {
    test('platform_list 失败 → 空列表且 loading 收尾，不卡在骨架', () async {
      final k = platformsFake();
      k.errors['platform_list'] = StateError('boom');
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      expect(c.platforms, isEmpty);
      expect(c.loading, isFalse);
    });

    test('用量 / 最近测试 / 分组 / 熔断默认 任一失败都不挡主列表', () async {
      final k = platformsFake();
      k.errors['all_platform_usage_stats'] = StateError('x');
      k.errors['get_last_test_result'] = StateError('x');
      k.errors['group_detail_list'] = StateError('x');
      k.errors['scheduling_settings_get'] = StateError('x');
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      expect(c.platforms.length, 3);
      expect(c.usageLoading, isFalse);
    });

    test('get_last_test_result 返回 null 的平台不进 lastTestMap', () async {
      final k = platformsFake();
      var n = 0;
      k.responses['get_last_test_result'] = () =>
          n++ == 0 ? {'success': true, 'status_code': 200} : null;
      final c = PlatformsController(invoke: k.fn);
      await c.init();
      expect(c.lastTestMap.length, 1);
    });
  });
}
