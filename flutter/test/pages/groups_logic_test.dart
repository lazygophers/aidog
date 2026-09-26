/// 分组页逻辑层的回归测试（票 I07）。
///
/// 前两组是 React 版现成测试的逐条翻译，**数据与期望值一字不改**：
///   - `src/domains/groups/editReducer.test.ts`（6 条）
///   - `src/domains/groups/query.test.ts`（部分，拼音那几条的处理见组内说明）
///
/// 其余是本票新增：校验、破坏性确认、批量四操作、`env_vars` 透传那个坑。
library;

import 'package:aidog_flutter/src/pages/groups_logic.dart';
import 'package:aidog_flutter/src/pages/models.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';

/// `editReducer.test.ts:5-28` 的 `makeDetail()`，字段值照抄。
GroupDetail makeDetail({Map<String, dynamic> groupOver = const {}}) =>
    GroupDetail.fromJson({
      'group': {
        'id': 7,
        'group_key': 'g1',
        'name': 'Group 1',
        'routing_mode': 'roundrobin',
        'env_vars': [
          {'key': 'K', 'value': 'V'},
        ],
        'request_timeout_secs': 30,
        'connect_timeout_secs': 5,
        'max_retries': 3,
        ...groupOver,
      },
      'platforms': [
        {
          'platform': {'id': 1},
        },
        {
          'platform': {'id': 2},
        },
      ],
      'model_mappings': [
        {
          'source_model': 'claude-3',
          'target_platform_id': 1,
          'target_model': 'gpt-4',
          'request_timeout_secs': 60,
          'connect_timeout_secs': 10,
        },
      ],
    });

Map<String, dynamic> platformJson(
  int id,
  String name, {
  String status = 'enabled',
  bool enabled = true,
  String type = 'openai',
  String baseUrl = '',
  double balance = 0,
}) => {
  'id': id,
  'name': name,
  'platform_type': type,
  'base_url': baseUrl,
  'status': status,
  'enabled': enabled,
  'est_balance_remaining': balance,
  'models': {'default': 'm'},
  'available_models': ['m'],
};

void main() {
  group('editReducer（React: src/domains/groups/editReducer.test.ts 逐条翻译）', () {
    test('open：按 GroupDetail 字段逐一映射到编辑态', () {
      final detail = makeDetail();
      final next = GroupEditState.open(detail);
      expect(next.target, same(detail));
      expect(next.name, 'Group 1');
      expect(next.mode, 'roundrobin');
      expect(next.platformIds, [1, 2]);
      expect(next.mappings.length, 1);
      expect(next.mappings[0].sourceModel, 'claude-3');
      expect(next.mappings[0].targetPlatformId, 1);
      expect(next.mappings[0].targetModel, 'gpt-4');
      expect(next.mappings[0].requestTimeoutSecs, 60);
      expect(next.mappings[0].connectTimeoutSecs, 10);
      expect(next.envVars.length, 1);
      expect(next.envVars[0].key, 'K');
      expect(next.envVars[0].value, 'V');
      expect(next.reqTimeout, 30);
      expect(next.connTimeout, 5);
      expect(next.maxRetries, 3);
    });

    test('open：mappings/envVars 是新对象（非原 detail 对象引用），不会互相污染', () {
      final detail = makeDetail();
      final next = GroupEditState.open(detail);
      expect(identical(next.mappings[0], detail.modelMappings[0]), isFalse);
      expect(identical(next.envVars[0], detail.group.envVars[0]), isFalse);
    });

    test('reset：无论当前态如何，回落到 EMPTY_EDIT（含隐私默认 env）', () {
      final dirty = GroupEditState.open(makeDetail());
      expect(dirty.name, isNotEmpty);
      const next = GroupEditState.empty;
      expect(next.envVars.length, greaterThan(0));
      expect(next.target, isNull);
      expect(next.name, '');
      expect(next.mode, 'failover');
      expect(next.maxRetries, 10);
    });

    test('patch：浅合并，未 patch 的字段保留原值', () {
      final base = GroupEditState.open(makeDetail());
      final next = base.patch(name: 'renamed');
      expect(next.name, 'renamed');
      expect(next.mode, base.mode);
      expect(identical(next.platformIds, base.platformIds), isTrue);
    });

    test('EMPTY_EDIT 的隐私 env 是 12 条，顺序与 editReducer.ts:21-34 一致', () {
      expect(kPrivacyDefaultEnvVars.length, 12);
      expect(
        kPrivacyDefaultEnvVars.first.key,
        'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC',
      );
      expect(
        kPrivacyDefaultEnvVars.last.key,
        'CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL',
      );
      expect(kPrivacyDefaultEnvVars.first.value, '1');
    });
  });

  group('upsertPlatformInto（React: editReducer.test.ts:76-96 逐条翻译）', () {
    final p1 = PlatformRow.fromJson(platformJson(1, 'P1'));
    final p2 = PlatformRow.fromJson(platformJson(2, 'P2'));

    test('命中 id → 原位替换该项，其余项引用不变', () {
      final prev = [p1, p2];
      final updated = PlatformRow.fromJson(platformJson(1, 'P1-renamed'));
      final next = upsertPlatformInto(prev, updated);
      expect(next.map((p) => p.name), ['P1-renamed', 'P2']);
      expect(identical(next[1], p2), isTrue);
      expect(identical(next, prev), isFalse);
    });

    test('未命中 id → 追加到末尾，不改动已有项', () {
      final prev = [p1];
      final p3 = PlatformRow.fromJson(platformJson(3, 'P3'));
      final next = upsertPlatformInto(prev, p3);
      expect(next.map((p) => p.id), [1, 3]);
      expect(identical(next[0], p1), isTrue);
    });
  });

  group('搜索匹配（React: src/domains/groups/query.test.ts）', () {
    // query.test.ts 的 fixture，字段值照抄。
    final p = PlatformRow.fromJson(
      platformJson(
        1,
        'GLM 测试',
        type: 'glm',
        baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
      ),
    );
    const terms = {
      'glm': ['智谱', 'zhipu', 'GLM-4.7', 'bigmodel', 'codegeex'],
    };

    test('用户自填 name 直接命中（无 termsMap 也成立）', () {
      expect(platformMatchesQuery(p, 'glm 测试'), isTrue);
    });

    test('protocolTerms 命中 registry 词条（UI 语言无关）', () {
      expect(platformMatchesQuery(p, '智谱', terms), isTrue);
      expect(platformMatchesQuery(p, 'zhipu', terms), isTrue);
      expect(platformMatchesQuery(p, 'bigmodel', terms), isTrue);
      // 中文词条的拼音形式已作为字面词条入库（platform.json keywords），纯子串命中。
      expect(platformMatchesQuery(p, 'zhip', terms), isTrue);
    });

    test('无 protocolTerms 或词条不存在 → 不误报', () {
      expect(platformMatchesQuery(p, '智谱'), isFalse);
      expect(
        platformMatchesQuery(p, '智谱', {
          'kimi': ['moonshot'],
        }),
        isFalse,
      );
      expect(platformMatchesQuery(p, 'kimi', terms), isFalse);
    });

    test('拼音匹配：`ceshi` 搜「GLM 测试」——自建词典已对齐 React（I17）', () {
      // React 的 `query.test.ts:11` 断言 `platformMatchesQuery(p, "ceshi") === true`，
      // 靠 `pinyin-pro` 的汉字字典。I17 起 Dart 侧有自建 3500 常用字词典
      // （lib/src/utils/pinyin.dart），这条照 React 原断言翻译回来。
      expect(platformMatchesQuery(p, 'ceshi'), isTrue);
      expect(platformMatchesQuery(p, 'shi', terms), isTrue); // 全拼子串
    });

    test('groupMatchesQuery：分组名 / 组密钥子串命中', () {
      final g = GroupRow.fromJson({
        'id': 1,
        'name': '测试组',
        'group_key': 'test',
      });
      expect(groupMatchesQuery(g, '测试组'), isTrue);
      expect(groupMatchesQuery(g, 'test'), isTrue);
      expect(groupMatchesQuery(g, 'other'), isFalse);
      // 拼音（React `query.test.ts:33` 原断言，I17 起对齐）。
      expect(groupMatchesQuery(g, 'ceshizu'), isTrue);
    });
  });

  group('分组密钥输入过滤（GroupCreateModal.tsx:78）', () {
    test('只留字母数字下划线连字符', () {
      expect(sanitizeGroupKey('my-key_1'), 'my-key_1');
      expect(sanitizeGroupKey('my key'), 'mykey');
      expect(sanitizeGroupKey('a/b.c@d'), 'abcd');
      expect(sanitizeGroupKey('中文key'), 'key');
      expect(sanitizeGroupKey(''), '');
    });

    test('密钥创建后锁定，所以非法字符必须在输入时就挡掉', () {
      // 放进去一个空格，用户拿到的就是一个永远匹配不上的 Bearer。
      expect(sanitizeGroupKey('gk abc').contains(' '), isFalse);
    });
  });

  group('余额聚合（useGroupData.ts:34-41）', () {
    test('组内平台 est_balance_remaining 求和，只累加 >0 的', () {
      final details = [
        GroupDetail.fromJson({
          'group': {'id': 1, 'name': 'G1'},
          'platforms': [
            {
              'platform': platformJson(1, 'A'),
            },
            {
              'platform': platformJson(2, 'B'),
            },
          ],
        }),
      ];
      final platforms = [
        PlatformRow.fromJson(platformJson(1, 'A', balance: 3.5)),
        PlatformRow.fromJson(platformJson(2, 'B', balance: -1)),
      ];
      expect(groupBalanceOf(details, platforms), {1: 3.5});
    });

    test('全组余额为 0 → 该组不进 map（不是进 map 值为 0）', () {
      final details = [
        GroupDetail.fromJson({
          'group': {'id': 1, 'name': 'G1'},
          'platforms': [
            {
              'platform': platformJson(1, 'A'),
            },
          ],
        }),
      ];
      final platforms = [PlatformRow.fromJson(platformJson(1, 'A'))];
      expect(groupBalanceOf(details, platforms), isEmpty);
    });
  });

  // ── 以下是本票新增的行为测试 ──────────────────────────────────

  FakeInvoke groupsFake({List<Object?>? page}) => FakeInvoke({
    'platform_list': [platformJson(1, 'A'), platformJson(2, 'B')],
    'group_detail_list_paged':
        page ??
        [
          {
            'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
            'platforms': [
              {
                'platform': platformJson(1, 'A'),
              },
            ],
            'model_mappings': <Object?>[],
          },
        ],
    'group_detail_list': [
      {
        'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
        'platforms': [
          {
            'platform': platformJson(1, 'A'),
          },
        ],
      },
    ],
    'group_detail': {
      'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
      'platforms': <Object?>[],
      'model_mappings': <Object?>[],
    },
    'all_group_usage_stats': <String, Object?>{},
    'proxy_get_settings': {'port': 9999},
    'get_defaults_json': '{"protocols":{}}',
    'group_create': {'id': 42},
    'group_set_platforms': null,
    'group_update': null,
    'group_delete': null,
    'group_set_default': null,
    'group_reorder': null,
    'group_platform_move': null,
    'group_platform_reorder': null,
    'group_platform_set_level_priority': null,
    'platform_delete': null,
    'platform_purge_disabled_preview': <Object?>[],
    'platform_purge_disabled': {
      'deletedIds': <Object?>[],
      'unassignedIds': <Object?>[],
    },
    'batch_delete_platforms': {'applied': 2},
    'batch_override_models': {'applied': 2},
    'batch_set_status': {'applied': 2},
    'batch_move_group': {'applied': 2},
    'model_test': {'success': true, 'duration_ms': 5, 'error': ''},
    'set_ui_extra': null,
  });

  group('创建分组的校验', () {
    test('名字为空 → 不能创建（GroupCreateModal.tsx:55 的 disabled={!cName}）', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      expect(c.canCreate, isFalse);
      await c.createGroup();
      expect(k.commands.contains('group_create'), isFalse,
          reason: '禁用态下不许发命令');
    });

    test('填了名字 → 可以创建，密钥留空时不传（后端自动生成）', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      c.setCreateName('我的组');
      expect(c.canCreate, isTrue);
      await c.createGroup();
      final input =
          k.lastCallTo('group_create')!.args!['input']! as Map<String, Object?>;
      expect(input['name'], '我的组');
      expect(input.containsKey('group_key'), isFalse);
      expect(input['routing_mode'], 'health_aware');
    });

    test('没选平台就不发第二条命令', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      c.setCreateName('x');
      await c.createGroup();
      expect(k.commands.contains('group_set_platforms'), isFalse);
    });

    test('选了平台 → 建完再关联一次，优先级按列表顺序从 1 起，权重固定 1', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      c.setCreateName('x');
      c.setCreatePlatformIds([5, 3]);
      await c.createGroup();
      final args = k.lastCallTo('group_set_platforms')!.args!;
      expect(args['groupId'], 42);
      expect(args['platforms'], [
        {'platform_id': 5, 'priority': 1, 'weight': 1},
        {'platform_id': 3, 'priority': 2, 'weight': 1},
      ]);
    });

    test('密钥输入实时过滤非法字符', () {
      final c = GroupsController(invoke: groupsFake().fn);
      c.setCreateGroupKey('my key/1');
      expect(c.createGroupKey, 'mykey1');
    });

    test('新建表单的平台候选只列 enabled 的（GroupCreateModal.tsx:33）', () async {
      // 组页里不放平台：否则 loadMore 的 upsert 会用组内那份覆盖同 id 的平台行，
      // 断言看到的就不是 platform_list 给的名字了。
      final k = FakeInvoke({
        ...groupsFake(page: const []).responses,
        'platform_list': [
          platformJson(1, 'on'),
          platformJson(2, 'off', status: 'disabled', enabled: false),
        ],
      });
      final c = GroupsController(invoke: k.fn);
      await c.load();
      expect(c.enabledPlatformOptions.map((p) => p.name), ['on']);
    });

    test('编辑表单的关联平台候选也只列 enabled（原先用的是全量 platforms）', () async {
      // 已禁用的平台加进分组也不会被路由选中，等于埋一个「配了但不生效」的坑。
      // React 两处都过滤（`GroupCreateModal.tsx:33` / `GroupEditPanel.tsx:34`）。
      // 同上一条：组页留空，否则 loadMore 的 upsert 会盖掉 platform_list 那份。
      final k = FakeInvoke({
        ...groupsFake(page: const []).responses,
        'platform_list': [
          platformJson(1, 'on'),
          platformJson(2, 'off', status: 'disabled', enabled: false),
        ],
      });
      final c = GroupsController(invoke: k.fn);
      await c.load();
      expect(c.platforms.length, 2, reason: '全量里两个都在');
      expect(c.enabledPlatformOptions.map((p) => p.id), [1]);
    });

    test('modelsOfPlatform：给出该平台五槽去重值；平台 id 为 0 / null / 不存在时为空', () async {
      final k = FakeInvoke({
        ...groupsFake(page: const []).responses,
        'platform_list': [
          {
            ...platformJson(1, 'on'),
            'models': {'default': 'm1', 'opus': 'm2'},
          },
        ],
      });
      final c = GroupsController(invoke: k.fn);
      await c.load();
      expect(c.modelsOfPlatform(1), ['m1', 'm2']);
      expect(c.modelsOfPlatform(0), isEmpty, reason: '0 = 还没选目标平台');
      expect(c.modelsOfPlatform(null), isEmpty);
      expect(c.modelsOfPlatform(999), isEmpty, reason: '找不到的平台不该抛');
    });

    test('关闭新建表单把四个字段清回初值（mode 回 failover，照搬 React）', () {
      final c = GroupsController(invoke: groupsFake().fn);
      c.openCreate();
      c.setCreateName('x');
      c.setCreateGroupKey('k');
      c.setCreateMode('sticky');
      c.setCreatePlatformIds([1]);
      c.closeCreate();
      expect(c.createName, '');
      expect(c.createGroupKey, '');
      expect(c.createMode, 'failover');
      expect(c.createPlatformIds, isEmpty);
      expect(c.showCreate, isFalse);
    });
  });

  group('编辑分组的校验与保存', () {
    test('名字为空 → 不能保存（GroupEditPanel.tsx:68 的 disabled={!editName}）', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      c.openEdit(makeDetail());
      c.patchEdit(c.edit.patch(name: ''));
      expect(c.edit.canSave, isFalse);
      await c.saveEdit();
      expect(k.commands.contains('group_update'), isFalse);
    });

    test('一个空格算有效名（React 判的是 !name，不 trim）', () {
      final c = GroupsController(invoke: groupsFake().fn);
      c.openEdit(makeDetail());
      c.patchEdit(c.edit.patch(name: ' '));
      expect(c.edit.canSave, isTrue);
    });

    test('保存发两条命令：先更新组本体，再重设平台集', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      c.openEdit(makeDetail());
      await c.saveEdit();
      final order = k.calls.map((x) => x.cmd).toList();
      final iUpdate = order.indexOf('group_update');
      final iSet = order.indexOf('group_set_platforms');
      expect(iUpdate, greaterThanOrEqualTo(0));
      expect(iSet, greaterThan(iUpdate));
    });

    test('保存带上全部七个字段（含 env_vars 与 model_mappings）', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      c.openEdit(makeDetail());
      await c.saveEdit();
      final input =
          k.lastCallTo('group_update')!.args!['input']! as Map<String, Object?>;
      expect(input['id'], 7);
      expect(input['name'], 'Group 1');
      expect(input['routing_mode'], 'roundrobin');
      expect(input['request_timeout_secs'], 30);
      expect(input['connect_timeout_secs'], 5);
      expect(input['max_retries'], 3);
      expect((input['env_vars']! as List).length, 1);
      expect((input['model_mappings']! as List).length, 1);
    });

    test('保存成功后退出编辑态，并只刷这一个组（不整列表重载）', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      final pagedBefore = k.callsTo('group_detail_list_paged').length;
      c.openEdit(makeDetail(groupOver: {'id': 10}));
      await c.saveEdit();
      expect(c.edit.target, isNull);
      expect(k.callsTo('group_detail').length, 1);
      expect(k.callsTo('group_detail_list_paged').length, pagedBefore,
          reason: '单组刷新不该触发分页重载');
    });
  });

  group('破坏性操作一律先确认', () {
    test('删组：askDeleteGroup 不发命令，confirm 才发', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      c.askDeleteGroup(10);
      expect(k.commands.contains('group_delete'), isFalse);
      expect(c.deleteGroupTarget, 10);
      await c.confirmDeleteGroup();
      expect(k.lastCallTo('group_delete')!.args!['id'], 10);
      expect(c.deleteGroupTarget, isNull);
    });

    test('取消删组 → 目标清空且永不发命令', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      c.askDeleteGroup(10);
      c.cancelDeleteGroup();
      await c.confirmDeleteGroup();
      expect(k.commands.contains('group_delete'), isFalse);
    });

    test('移除平台：弹窗上下文用**实时拉的**后端数据算跨组归属', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      final before = k.callsTo('group_detail_list').length;
      await c.askRemovePlatform(
        PlatformRow.fromJson(platformJson(1, 'A')),
        10,
      );
      expect(k.callsTo('group_detail_list').length, before + 1,
          reason: '必须现拉，不能用已分页的前端 details');
      expect(c.removeTarget!.groupCount, 1);
      expect(c.removeTarget!.groupNames, ['G10']);
      expect(c.removeTarget!.onlyInThisGroup, isTrue);
    });

    test('拉后端失败 → 回退前端 details，弹窗仍然能弹出来', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      k.errors['group_detail_list'] = StateError('boom');
      await c.askRemovePlatform(
        PlatformRow.fromJson(platformJson(1, 'A')),
        10,
      );
      expect(c.removeTarget, isNotNull);
      expect(c.removeTarget!.groupCount, 1);
    });

    test('属多个组 → 弹窗要给「仅移出本组」这个选项', () async {
      final k = groupsFake();
      k.responses['group_detail_list'] = [
        {
          'group': {'id': 10, 'name': 'G10'},
          'platforms': [
            {
              'platform': platformJson(1, 'A'),
            },
          ],
        },
        {
          'group': {'id': 11, 'name': 'G11'},
          'platforms': [
            {
              'platform': platformJson(1, 'A'),
            },
          ],
        },
      ];
      final c = GroupsController(invoke: k.fn);
      await c.load();
      await c.askRemovePlatform(PlatformRow.fromJson(platformJson(1, 'A')), 10);
      expect(c.removeTarget!.groupCount, 2);
      expect(c.removeTarget!.onlyInThisGroup, isFalse);
      expect(c.removeTarget!.groupNames, ['G10', 'G11']);
    });

    test('选「仅移出本组」→ 用 group_set_platforms 重设本组，不调 platform_delete', () async {
      final k = groupsFake(
        page: [
          {
            'group': {'id': 10, 'name': 'G', 'group_key': 'gk'},
            'platforms': [
              {'platform': platformJson(1, 'A'), 'weight': 3},
              {'platform': platformJson(2, 'B'), 'weight': 1},
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      final c = GroupsController(invoke: k.fn);
      await c.load();
      await c.askRemovePlatform(PlatformRow.fromJson(platformJson(1, 'A')), 10);
      await c.removePlatformFromGroup();
      expect(k.commands.contains('platform_delete'), isFalse);
      expect(k.lastCallTo('group_set_platforms')!.args!['platforms'], [
        {'platform_id': 2, 'priority': 1, 'weight': 1},
      ]);
      expect(c.removeTarget, isNull);
    });

    test('选「删除平台」→ platform_delete；失败时只 toast、不留残弹窗', () async {
      final k = groupsFake();
      final toasts = <String>[];
      final c = GroupsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add('$ok|$t'),
      );
      await c.load();
      await c.askRemovePlatform(PlatformRow.fromJson(platformJson(1, 'A')), 10);
      k.errors['platform_delete'] = StateError('boom');
      await c.confirmDeletePlatform();
      expect(k.callsTo('platform_delete').length, 1);
      expect(c.removeTarget, isNull);
      expect(toasts.single, startsWith('false|删除失败'));
    });

    test('清理失效：先预览，确认才真删', () async {
      final k = groupsFake();
      k.responses['platform_purge_disabled_preview'] = [
        {'id': 3, 'reason': 'auth_failed', 'action': 'delete'},
      ];
      final c = GroupsController(invoke: k.fn);
      await c.load();
      await c.askPurgeDisabled(10);
      expect(k.commands.contains('platform_purge_disabled'), isFalse);
      expect(c.purgeTarget!.candidates.length, 1);
      expect(c.purgeTarget!.groupId, 10);
      await c.confirmPurgeDisabled();
      expect(k.lastCallTo('platform_purge_disabled')!.args!['groupId'], 10);
    });

    test('清理结果为空 → 提示「暂无失效平台」', () async {
      final k = groupsFake();
      final toasts = <String>[];
      final c = GroupsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add(t),
      );
      await c.load();
      await c.askPurgeDisabled(null);
      await c.confirmPurgeDisabled();
      expect(toasts, contains('暂无失效平台'));
    });

    test('清理有结果 → 分别报删除数与移除数', () async {
      final k = groupsFake();
      k.responses['platform_purge_disabled'] = {
        'deletedIds': [1, 2],
        'unassignedIds': [3],
      };
      final toasts = <String>[];
      final c = GroupsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add(t),
      );
      await c.load();
      await c.askPurgeDisabled(null);
      await c.confirmPurgeDisabled();
      expect(toasts, contains('已清理：删除 2，移除 1'));
    });
  });

  group('批量四操作', () {
    test('删除：选中为空 → 不开弹窗', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      await c.askBatchDelete([999]);
      expect(c.batchDeleteTarget, isNull);
    });

    test('删除：跨组的要能被弹窗标出来', () async {
      final k = groupsFake();
      k.responses['group_detail_list'] = [
        {
          'group': {'id': 10, 'name': 'G10'},
          'platforms': [
            {
              'platform': platformJson(1, 'A'),
            },
          ],
        },
        {
          'group': {'id': 11, 'name': 'G11'},
          'platforms': [
            {
              'platform': platformJson(1, 'A'),
            },
          ],
        },
      ];
      final c = GroupsController(invoke: k.fn);
      await c.load();
      await c.askBatchDelete([1, 2]);
      expect(c.batchDeleteTarget!.hasCrossGroup, isTrue);
      expect(c.batchDeleteTarget!.groupNamesByPlatform[1], ['G10', 'G11']);
      expect(c.batchDeleteTarget!.groupNamesByPlatform[2], isEmpty);
    });

    test('删除：确认才调 batch_delete_platforms，并报 applied 条数', () async {
      final k = groupsFake();
      final toasts = <String>[];
      final c = GroupsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add(t),
      );
      await c.load();
      await c.askBatchDelete([1, 2]);
      expect(k.commands.contains('batch_delete_platforms'), isFalse);
      await c.confirmBatchDelete();
      expect(k.lastCallTo('batch_delete_platforms')!.args!['ids'], [1, 2]);
      expect(toasts, contains('已删除 2 个平台'));
      expect(c.batchDeleteBusy, isFalse);
    });

    test('覆盖模型：整份五槽写过去（不是合并）', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      c.askBatchOverrideModels([1, 2]);
      await c.confirmBatchOverrideModels(
        const PlatformModels(defaultModel: 'x'),
      );
      expect(k.lastCallTo('batch_override_models')!.args!['models'], {
        'default': 'x',
        'sonnet': null,
        'opus': null,
        'haiku': null,
        'gpt': null,
      });
    });

    test('改状态：同时算出本组当前 enabled 的平台（无候选警告的数据源）', () async {
      final k = groupsFake(
        page: [
          {
            'group': {'id': 10, 'name': 'G', 'group_key': 'gk'},
            'platforms': [
              {
                'platform': platformJson(1, 'A'),
              },
              {
                'platform': platformJson(
                  2,
                  'B',
                  status: 'disabled',
                  enabled: false,
                ),
              },
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      final c = GroupsController(invoke: k.fn);
      await c.load();
      c.askBatchSetStatus([1, 2], 10);
      expect(c.batchSetStatusGroupEnabledIds, [1]);
      await c.confirmBatchSetStatus('disabled');
      expect(k.lastCallTo('batch_set_status')!.args!['status'], 'disabled');
    });

    test('移组：move / add 两种模式各自报对应文案', () async {
      final k = groupsFake();
      final toasts = <String>[];
      final c = GroupsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add(t),
      );
      await c.load();
      c.askBatchMoveGroup([1], 10);
      await c.confirmBatchMoveGroup(11, 'move');
      expect(k.lastCallTo('batch_move_group')!.args!['targetGroupId'], 11);
      expect(k.lastCallTo('batch_move_group')!.args!['mode'], 'move');
      expect(toasts.last, '已移动 2 个平台');

      c.askBatchMoveGroup([1], 10);
      await c.confirmBatchMoveGroup(11, 'add');
      expect(toasts.last, '已加入 2 个平台');
    });

    test('批量失败 → busy 复位、目标清空、报错', () async {
      final k = groupsFake();
      k.errors['batch_delete_platforms'] = StateError('boom');
      final toasts = <String>[];
      final c = GroupsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add('$ok|$t'),
      );
      await c.load();
      await c.askBatchDelete([1]);
      await c.confirmBatchDelete();
      expect(c.batchDeleteBusy, isFalse);
      expect(c.batchDeleteTarget, isNull);
      expect(toasts.single, startsWith('false|批量删除失败'));
    });
  });

  group('组内优先级：乐观改 + 失败回滚', () {
    test('成功 → 本地值留在新值', () async {
      final k = groupsFake(
        page: [
          {
            'group': {'id': 10, 'name': 'G', 'group_key': 'gk'},
            'platforms': [
              {'platform': platformJson(1, 'A'), 'level_priority': 5},
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      final c = GroupsController(invoke: k.fn);
      await c.load();
      await c.setLevelPriority(10, 1, 9);
      expect(c.details.first.platforms.first.levelPriority, 9);
      final args = k.lastCallTo('group_platform_set_level_priority')!.args!;
      expect(args['groupId'], 10);
      expect(args['platformId'], 1);
      expect(args['levelPriority'], 9);
    });

    test('失败 → 回滚到原值并报错', () async {
      final k = groupsFake(
        page: [
          {
            'group': {'id': 10, 'name': 'G', 'group_key': 'gk'},
            'platforms': [
              {'platform': platformJson(1, 'A'), 'level_priority': 4},
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      k.errors['group_platform_set_level_priority'] = StateError('boom');
      final toasts = <String>[];
      final c = GroupsController(
        invoke: k.fn,
        onToast: (t, {required ok}) => toasts.add('$ok|$t'),
      );
      await c.load();
      await c.setLevelPriority(10, 1, 9);
      expect(c.details.first.platforms.first.levelPriority, 4);
      expect(toasts.single, startsWith('false|优先级保存失败'));
    });
  });

  group('一键测试本组', () {
    test('只测 enabled 的平台，disabled 既不出现在结果行也不发请求', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      final gps = [
        GroupPlatform.fromJson({'platform': platformJson(1, 'A')}),
        GroupPlatform.fromJson({
          'platform': platformJson(2, 'B', status: 'disabled', enabled: false),
        }),
        GroupPlatform.fromJson({
          'platform': platformJson(3, 'C', status: 'auto_disabled', enabled: false),
        }),
      ];
      await c.testGroup(GroupRow.fromJson({'id': 1, 'name': 'G'}), gps);
      expect(c.groupTest!.rows.map((r) => r.name), ['A']);
      expect(k.callsTo('model_test').length, 1);
    });

    test('一个 enabled 都没有 → 整个不弹面板，也不发请求', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      final gps = [
        GroupPlatform.fromJson({
          'platform': platformJson(2, 'B', status: 'disabled', enabled: false),
        }),
      ];
      await c.testGroup(GroupRow.fromJson({'id': 1, 'name': 'G'}), gps);
      expect(c.groupTest, isNull);
      expect(k.commands.contains('model_test'), isFalse);
    });

    test('跑完 running 变 false，每行都有终态', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      final gps = [
        GroupPlatform.fromJson({'platform': platformJson(1, 'A')}),
        GroupPlatform.fromJson({'platform': platformJson(2, 'B')}),
      ];
      await c.testGroup(GroupRow.fromJson({'id': 1, 'name': 'G'}), gps);
      expect(c.groupTest!.running, isFalse);
      expect(c.groupTest!.rows.map((r) => r.status), ['ok', 'ok']);
      expect(c.groupTest!.rows.every((r) => r.durationMs != null), isTrue);
    });

    test('上游返回 success:false → 该行 fail 并带错误文案', () async {
      final k = groupsFake();
      k.responses['model_test'] = {
        'success': false,
        'duration_ms': 3,
        'error': '401 Unauthorized',
      };
      final c = GroupsController(invoke: k.fn);
      await c.testGroup(GroupRow.fromJson({'id': 1, 'name': 'G'}), [
        GroupPlatform.fromJson({'platform': platformJson(1, 'A')}),
      ]);
      expect(c.groupTest!.rows.single.status, 'fail');
      expect(c.groupTest!.rows.single.error, '401 Unauthorized');
    });

    test('测试用的模型：默认槽优先，空则用 available_models 第一个', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.testGroup(GroupRow.fromJson({'id': 1, 'name': 'G'}), [
        GroupPlatform.fromJson({
          'platform': {
            ...platformJson(1, 'A'),
            'models': <String, Object?>{},
            'available_models': ['fallback-model'],
          },
        }),
      ]);
      final req = k.lastCallTo('model_test')!.args!['req']! as Map<String, Object?>;
      expect(req['model'], 'fallback-model');
    });
  });

  group('排序与折叠', () {
    test('搜索态下重排是 no-op（Groups.tsx:520）', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      await c.reorderGroups(c.details.reversed.toList(), searchQuery: 'abc');
      expect(k.commands.contains('group_reorder'), isFalse);
    });

    test('非搜索态才发 group_reorder', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      await c.reorderGroups(c.details, searchQuery: '   ');
      expect(k.lastCallTo('group_reorder')!.args!['orderedIds'], [10]);
    });

    test('折叠态从 group.extra 回灌，非法 JSON 当未折叠', () {
      final c = GroupsController(invoke: groupsFake().fn);
      c.hydrateCollapsedFrom([
        GroupDetail.fromJson({
          'group': {'id': 1, 'extra': '{"_ui_collapsed":true}'},
        }),
        GroupDetail.fromJson({
          'group': {'id': 2, 'extra': 'not json'},
        }),
        GroupDetail.fromJson({
          'group': {'id': 3, 'extra': ''},
        }),
      ]);
      expect(c.collapsedGroups, {1});
    });

    test('toggle 返回新态并写进集合；落盘走 set_ui_extra 四参', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      expect(c.toggleGroupCollapsed(5), isTrue);
      expect(c.collapsedGroups.contains(5), isTrue);
      expect(c.toggleGroupCollapsed(5), isFalse);
      expect(c.collapsedGroups.contains(5), isFalse);
      await c.persistGroupCollapsed(5, true);
      expect(k.lastCallTo('set_ui_extra')!.args, {
        'target': 'group',
        'id': 5,
        'key': '_ui_collapsed',
        'value': true,
      });
    });
  });

  group('加载形态', () {
    test('mount 拉平台 + 第一页组 + 统计 + 代理端口 + 协议词条', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.init();
      expect(k.commands, containsAll(<String>{
        'platform_list',
        'group_detail_list_paged',
        'all_group_usage_stats',
        'proxy_get_settings',
        'get_defaults_json',
      }));
      expect(c.proxyBaseUrl, 'http://127.0.0.1:9999/proxy');
    });

    test('取不到端口 → 兜底 7890', () async {
      final k = groupsFake();
      k.errors['proxy_get_settings'] = StateError('boom');
      final c = GroupsController(invoke: k.fn);
      await c.init();
      expect(c.proxyBaseUrl, 'http://127.0.0.1:7890/proxy');
    });

    test('页不满 → hasMore 置 false', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      expect(c.hasMore, isFalse);
    });

    test('silentReload 不清空列表、不动 loading', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      expect(c.details.length, 1);
      await c.silentReload();
      expect(c.details.length, 1);
      expect(c.loading, isFalse);
    });

    test('patchPlatform 同时更新平台列表与各组内那一行，且不发请求', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.load();
      final before = k.calls.length;
      c.patchPlatform(
        PlatformRow.fromJson(
          platformJson(1, 'A', status: 'disabled', enabled: false),
        ),
      );
      expect(k.calls.length, before);
      expect(c.platforms.firstWhere((p) => p.id == 1).status, 'disabled');
      expect(c.details.first.platforms.first.platform.status, 'disabled');
    });
  });

  group('策略文案表', () {
    test('五个策略的顺序与 routing.ts:5 一致（下拉顺序，别按字母重排）', () {
      expect(kRoutingModes, [
        'failover',
        'load_balance',
        'health_aware',
        'least_latency',
        'sticky',
      ]);
    });

    test('每个策略都有 label 与 desc', () {
      for (final m in kRoutingModes) {
        expect(kRoutingModeLabels[m], isNotNull, reason: '缺 label: $m');
        expect(kRoutingModeDescs[m], isNotNull, reason: '缺 desc: $m');
      }
    });
  });

  group('协议搜索词解析', () {
    test('name 各 locale 值 + keywords 全收，去重', () {
      final t = parseProtocolSearchTerms(
        '{"protocols":{"glm":{"name":{"zh-Hans":"智谱","en-US":"Zhipu"},'
        '"keywords":["zhipu","zp"]}}}',
      );
      expect(t['glm']!.toSet(), {'智谱', 'Zhipu', 'zhipu', 'zp'});
    });

    test('空串 / 非法 JSON / 没有 protocols → 空 map，不抛', () {
      expect(parseProtocolSearchTerms(''), isEmpty);
      expect(parseProtocolSearchTerms('not json'), isEmpty);
      expect(parseProtocolSearchTerms('{}'), isEmpty);
    });

    test('词条全空的协议不进 map', () {
      final t = parseProtocolSearchTerms(
        '{"protocols":{"x":{"name":{},"keywords":[]}}}',
      );
      expect(t.containsKey('x'), isFalse);
    });
  });

  group('启动命令（React: src/domains/groups/commands.test.ts 逐条翻译）', () {
    test('selects the aidog provider for the group', () {
      expect(buildPiCommand('teamA'), "pi --provider 'aidog-teamA'");
    });

    test('carries no routing env — token lives in models.json apiKey', () {
      expect(buildPiCommand('teamA').contains('AIDOG_KEY'), isFalse);
      expect(buildPiCommand('teamA').contains('ANTHROPIC_'), isFalse);
    });

    test('exports user env vars ahead of the command', () {
      final cmd = buildPiCommand('g', const [
        EnvVar(key: 'HTTP_PROXY', value: 'http://127.0.0.1:7890'),
        EnvVar(key: 'EMPTY', value: ''),
      ]);
      expect(
        cmd,
        "export HTTP_PROXY='http://127.0.0.1:7890'; pi --provider 'aidog-g'",
      );
    });

    test('quotes a group key containing shell metacharacters', () {
      // 分组名是用户自由输入，未加引号的 `;` 会把后半段当成第二条命令执行。
      expect(
        buildPiCommand("a'b; rm -rf /"),
        "pi --provider 'aidog-a'\\''b; rm -rf /'",
      );
    });

    test('claude 命令指向该组的 settings 文件（commands.ts:4）', () {
      expect(
        buildClaudeCommand('teamA'),
        'claude --brief --dangerously-skip-permissions '
        '--settings ~/.aidog/settings.teamA.json',
      );
    });

    test('codex 命令带 AIDOG_KEY 路由 token，并丢掉用户同名变量', () {
      final cmd = buildCodexCommand('g', const [
        EnvVar(key: 'AIDOG_KEY', value: 'hacked'),
        EnvVar(key: 'HTTP_PROXY', value: 'http://p'),
      ]);
      expect(cmd.contains("export AIDOG_KEY="), isFalse);
      expect(cmd.contains("export HTTP_PROXY='http://p';"), isTrue);
      expect(cmd.contains("AIDOG_KEY='g' codex -p 'g'"), isTrue);
    });
  });

  group('pi 线路协议（React: src/domains/groups/piApi.test.ts 逐条翻译）', () {
    test('reads the stored protocol', () {
      expect(parseGroupPiApi('{"pi_api":"openai-responses"}'), 'openai-responses');
    });

    test('falls back for old groups with no value, junk, or an unknown protocol', () {
      expect(parseGroupPiApi(''), kPiApiDefault);
      expect(parseGroupPiApi('{"_ui_collapsed":true}'), kPiApiDefault);
      expect(parseGroupPiApi('not json'), kPiApiDefault);
      expect(parseGroupPiApi('{"pi_api":"nonsense"}'), kPiApiDefault);
    });

    test('label 取不到 i18n key 时回落英文字面量', () {
      expect(piApiLabel((k) => k, 'openai-responses'), 'OpenAI Responses');
      expect(piApiLabel((k) => '译文', 'openai-responses'), '译文');
    });

    test('选中即写 group.extra 并立刻重生成 pi 配置', () async {
      final k = FakeInvoke({'set_ui_extra': null, 'sync_group_settings': null});
      await GroupsController(invoke: k.fn).setGroupPiApi(7, 'openai-responses');
      expect(k.lastCallTo('set_ui_extra')!.args, {
        'target': 'group',
        'id': 7,
        'key': 'pi_api',
        'value': 'openai-responses',
      });
      expect(k.commands.contains('sync_group_settings'), isTrue);
    });
  });

  group('出站代理 env（React: src/domains/groups/proxy-env.ts）', () {
    test('只取四个代理键里非空的那些', () async {
      final k = FakeInvoke({
        'settings_get': {
          'env': {
            'HTTP_PROXY': 'http://p',
            'HTTPS_PROXY': '',
            'UNRELATED': 'x',
          },
        },
      });
      // EnvVar 没有 `==`，按字段比（Dart 的默认相等是身份相等）。
      final got = await loadProxyEnvVars(k.fn);
      expect([for (final e in got) '${e.key}=${e.value}'], [
        'HTTP_PROXY=http://p',
      ]);
      expect(k.lastCallTo('settings_get')!.args, {
        'scope': 'global',
        'key': 'claude_code',
      });
    });

    test('读不到 / 没有 env 段 → 空数组，不抛', () async {
      final bad = FakeInvoke();
      bad.errors['settings_get'] = StateError('boom');
      expect(await loadProxyEnvVars(bad.fn), isEmpty);
      expect(await loadProxyEnvVars(FakeInvoke({'settings_get': {}}).fn), isEmpty);
    });
  });

  group('per-group 多选模式（GroupListItem.tsx:136-179）', () {
    test('进入 / 退出多选：选中集随之建立与清空', () {
      final c = GroupsController(invoke: FakeInvoke().fn);
      expect(c.isBatchSelecting(10), isFalse);
      c.enterBatchSelect(10);
      expect(c.isBatchSelecting(10), isTrue);
      expect(c.selectedIdsOf(10), isEmpty);
      c.toggleSelected(10, 1);
      expect(c.selectedIdsOf(10), {1});
      c.toggleSelected(10, 1);
      expect(c.selectedIdsOf(10), isEmpty);
      c.exitBatchSelect(10);
      expect(c.isBatchSelecting(10), isFalse);
    });

    test('全选收下整串 id；各组的选中集互不干扰', () {
      final c = GroupsController(invoke: FakeInvoke().fn);
      c.enterBatchSelect(10);
      c.enterBatchSelect(11);
      c.selectAll(10, [1, 2, 3]);
      expect(c.selectedIdsOf(10), {1, 2, 3});
      expect(c.selectedIdsOf(11), isEmpty);
    });

    test('非删除类批量成功后统一退出多选（React 的 batchDoneSignal）', () async {
      final k = groupsFake();
      final c = GroupsController(invoke: k.fn);
      await c.init();
      c.enterBatchSelect(10);
      c.selectAll(10, [1]);
      c.askBatchSetStatus([1], 10);
      await c.confirmBatchSetStatus('disabled');
      expect(c.isBatchSelecting(10), isFalse);
      expect(c.selectedIdsOf(10), isEmpty);
    });
  });

  group('组内平台换位（usePlatformDrag.ts:70-87 的组内重排分支）', () {
    FakeInvoke twoPlatformGroup() => groupsFake(
      page: [
        {
          'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
          'platforms': [
            {'platform': platformJson(1, 'A')},
            {'platform': platformJson(2, 'B')},
          ],
          'model_mappings': <Object?>[],
        },
      ],
    );

    test('往上挪一位 → 本地顺序立刻变，并把整串新 id 发给后端', () async {
      final k = twoPlatformGroup();
      final c = GroupsController(invoke: k.fn);
      await c.init();
      await c.movePlatformWithinGroup(10, 1, 0);
      expect(
        [for (final gp in c.details.single.platforms) gp.platform.id],
        [2, 1],
      );
      expect(k.lastCallTo('group_platform_reorder')!.args, {
        'groupId': 10,
        'orderedIds': [2, 1],
      });
    });

    test('越界 / 原地不动 → 不发命令', () async {
      final k = twoPlatformGroup();
      final c = GroupsController(invoke: k.fn);
      await c.init();
      await c.movePlatformWithinGroup(10, 0, 0);
      await c.movePlatformWithinGroup(10, 0, 5);
      await c.movePlatformWithinGroup(10, -1, 0);
      await c.movePlatformWithinGroup(999, 0, 1); // 组不存在
      expect(k.commands.contains('group_platform_reorder'), isFalse);
    });
  });

  // ── 触底自动加载（`groups.dart` 的 `_maybeLoadMore` → `loadMore`）──
  group('触底翻页', () {
    /// id 连号的组条目，`group_detail_list_paged` 的分页返回形状。
    Map<String, Object?> pagedEntry(int id) => {
      'group': {'id': id, 'name': 'G$id', 'group_key': 'gk$id'},
      'platforms': <Object?>[],
      'model_mappings': <Object?>[],
    };

    test('满页翻下一页：offset 推进、条目追加', () async {
      var calls = 0;
      final k = FakeInvoke({
        'platform_list': <Object?>[],
        'group_detail_list': <Object?>[],
        'group_detail': <Object?>[],
        'all_group_usage_stats': <String, Object?>{},
        'group_detail_list_paged': () {
          calls = calls + 1;
          return [for (var i = 0; i < GroupsController.pageSize; i++) pagedEntry((calls - 1) * GroupsController.pageSize + i)];
        },
      });
      final c = GroupsController(invoke: k.fn);
      await c.load();
      expect(c.details.length, GroupsController.pageSize);
      expect(c.hasMore, isTrue);
      await c.loadMore();
      expect(k.callsTo('group_detail_list_paged').last.args, {
        'offset': GroupsController.pageSize,
        'limit': GroupsController.pageSize,
      });
      expect(c.details.length, GroupsController.pageSize * 2);
      expect(c.hasMore, isTrue);
    });

    test('短页封底：hasMore 置 false', () async {
      // loadMore 自身不查 hasMore —— React 也是（`useGroupData.ts:116` 只查
      // loadingMore），翻到底不再发起由哨兵调用方挡（React 的 IO effect /
      // Flutter 的 `_maybeLoadMore`），这里只验封底信号本身。
      var calls = 0;
      final k = FakeInvoke({
        'platform_list': <Object?>[],
        'group_detail_list': <Object?>[],
        'group_detail': <Object?>[],
        'all_group_usage_stats': <String, Object?>{},
        'group_detail_list_paged': () {
          calls++;
          return calls == 1
              ? [for (var i = 0; i < GroupsController.pageSize; i++) pagedEntry(i)]
              : [pagedEntry(100), pagedEntry(101)];
        },
      });
      final c = GroupsController(invoke: k.fn);
      await c.load();
      await c.loadMore();
      expect(c.hasMore, isFalse);
      expect(c.details.length, GroupsController.pageSize + 2);
    });
  });

}
