/// 平台编辑表单控制器的回归测试。
///
/// 断言逐条对着 `src/pages/platforms/usePlatformForm.ts` 与
/// `src/pages/platforms/PlatformEditForm.tsx:117-125`（保存按钮禁用条件）写，
/// **期望值一字不改**。
library;

import 'dart:convert';

import 'package:aidog_flutter/src/pages/platform_extra.dart';
import 'package:aidog_flutter/src/pages/platform_form_logic.dart';
import 'package:aidog_flutter/src/pages/platforms_logic.dart';
import 'package:aidog_flutter/src/pages/time_window.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';

const String kDefaultsJson = '''
{"protocols":{
  "openai":{
    "name":{"en-US":"OpenAI","zh-Hans":"开放人工智能"},
    "endpoints":{"default":[{"protocol":"openai","base_url":"https://api.openai.com/v1"}]},
    "models":{"default":{"default":"gpt-5"}},
    "model_list":{"default":["gpt-5","gpt-5-mini"]},
    "quota_scripts":[{"id":"v1","name":{"en-US":"Official"},"requires":[{"key":"org_id"}]}],
    "plan_quotas":[{"id":"pro","name":"Pro","source_url":"u","budgets":[{"kind":"rolling","unit":"count","amount":300,"window_hours":5,"window_unit":"hour"}]}]
  },
  "anthropic":{
    "name":{"en-US":"Anthropic"},
    "endpoints":{"default":[{"protocol":"anthropic","base_url":"https://api.anthropic.com"}]},
    "models":{"default":{"default":"claude-opus-4"}},
    "model_list":{"default":["claude-opus-4"]},
    "peak":[{"start_hour":9,"end_hour":12,"multiplier":2}]
  },
  "mock":{"name":{"en-US":"Mock"}},
  "claude_code":{"name":{"en-US":"Claude Code"}},
  "opencode_zen":{"name":{"en-US":"OpenCode Zen"}},
  "glm_coding":{"is_coding_plan":true,"name":{"en-US":"GLM Coding"}},
  "newapi":{"name":{"en-US":"New API"}},
  "devin":{"name":{"en-US":"Devin"}}
}}''';

const String kClientTypesJson = '''
{"client_types":[
  {"value":"default","group":"","name":{"en-US":"Default"}},
  {"value":"claude_code","group":"Claude Code","name":{"en-US":"Claude Code"}},
  {"value":"codex_tui","group":"Codex","name":{"en-US":"Codex TUI"}}
]}''';

Map<String, dynamic> plat(
  int id,
  String name, {
  String type = 'openai',
  String extra = '',
  List<Object?> endpoints = const [],
  int expiresAt = 0,
}) => {
  'id': id,
  'name': name,
  'platform_type': type,
  'base_url': 'https://u$id',
  'api_key': 'sk-$id',
  'extra': extra,
  'models': {'default': 'm$id'},
  'available_models': ['m$id'],
  'endpoints': endpoints,
  'enabled': true,
  'status': 'enabled',
  'expires_at': expiresAt,
  'est_balance_remaining': 0,
};

FakeInvoke formFake({
  List<Object?>? platforms,
  List<Object?>? groups,
  Object? breaker,
}) => FakeInvoke({
  'platform_list': platforms ?? [plat(1, 'a')],
  'group_detail_list': groups ?? <Object?>[],
  'all_platform_usage_stats': <String, Object?>{},
  'get_last_test_result': null,
  'scheduling_settings_get': breaker,
  'get_defaults_json': kDefaultsJson,
  'get_client_types_json': kClientTypesJson,
  'platform_create': plat(9, 'new'),
  'platform_update': plat(1, 'a'),
  'platform_usage_stats': null,
  'platform_fetch_models': <Object?>[],
});

/// 建好一对控制器并跑完两侧 init。
Future<(PlatformsController, PlatformFormController)> boot(
  FakeInvoke k, {
  String locale = 'en-US',
}) async {
  final list = PlatformsController(invoke: k.fn);
  final form = PlatformFormController(list: list, invoke: k.fn);
  await list.init();
  await form.init(locale: locale);
  return (list, form);
}

Map<String, Object?> lastInput(FakeInvoke k, String cmd) =>
    k.lastCallTo(cmd)!.args!['input']! as Map<String, Object?>;

void main() {
  group('派生标记（usePlatformForm.ts:236-243）', () {
    test('isMock / isPassthrough / keyOptional / apiKeyMissing', () async {
      final (_, f) = await boot(formFake());
      f.protocol = 'mock';
      expect(f.isMock, isTrue);
      f.protocol = 'claude_code';
      expect(f.isPassthrough, isTrue);
      f.protocol = 'opencode_zen';
      expect(f.keyOptional, isTrue);
      // keyOptional 平台没填 key 也不算「缺 key」。
      expect(f.apiKeyMissing, isFalse);
      f.protocol = 'openai';
      expect(f.apiKeyMissing, isTrue);
      f.apiKey = 'sk';
      expect(f.apiKeyMissing, isFalse);
    });
  });

  group('F16 保存按钮禁用条件（PlatformEditForm.tsx:118-119）', () {
    test('名字为空 → 一律不可保存', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('openai');
      f.setApiKey('sk');
      f.setName('');
      expect(f.canSave, isFalse);
    });

    test('普通协议：要有端点且有 key', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('openai'); // 自动填了一条默认端点
      f.setName('n');
      expect(f.endpoints.length, 1);
      expect(f.canSave, isFalse); // 还没填 key
      f.setApiKey('sk');
      expect(f.canSave, isTrue);
      f.setEndpoints(const []);
      expect(f.canSave, isFalse);
    });

    test('mock / keyOptional：有名字就能存（不要求 key 与端点）', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('mock');
      f.setName('n');
      expect(f.canSave, isTrue);
      f.handleProtocolChange('opencode_zen');
      expect(f.canSave, isTrue);
    });

    test('透传：只看端点，不看 key', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('claude_code');
      f.setName('n');
      expect(f.canSave, isFalse);
      f.setPassthroughBaseUrl('https://api.anthropic.com');
      expect(f.canSave, isTrue);
      expect(f.endpoints.single.protocol, 'anthropic');
      expect(f.endpoints.single.clientType, 'default');
    });
  });

  group('F1 切协议（usePlatformForm.ts:273）', () {
    test('name 仍是默认名 → 跟着协议换；用户改过就不动', () async {
      final (_, f) = await boot(formFake());
      f.openCreatePlatform();
      expect(f.name, ''); // resetForm 后为空
      f.handleProtocolChange('openai');
      expect(f.name, 'OpenAI');
      f.handleProtocolChange('anthropic');
      expect(f.name, 'Anthropic');
      f.setName('我的平台');
      f.handleProtocolChange('openai');
      expect(f.name, '我的平台');
    });

    test('切协议会重填默认端点与默认模型', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('anthropic');
      expect(f.endpoints.single.baseUrl, 'https://api.anthropic.com');
      expect(f.endpoints.single.clientType, 'claude_code');
      expect(f.models['default'], 'claude-opus-4');
      // 没有 preset 的协议 → 端点清空、五槽全空。
      f.handleProtocolChange('mock');
      expect(f.endpoints, isEmpty);
      expect(f.models.values.every((v) => v.isEmpty), isTrue);
    });

    test('切协议会重置配额脚本选择', () async {
      final (_, f) = await boot(formFake());
      f.handleQuotaVariantChange(kQuotaCustomVariant);
      f.setQuotaCustomScript('// x');
      f.handleProtocolChange('anthropic');
      expect(f.quotaVariantId, '');
      expect(f.quotaCustomScript, '');
    });

    test('coding plan 标记跟着选项走', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('glm_coding', newCodingPlan: true);
      expect(f.codingPlan, isTrue);
      f.handleProtocolChange('openai');
      expect(f.codingPlan, isFalse);
    });
  });

  group('F6 端点编辑（formSectionsEndpoints.tsx）', () {
    test('加端点默认 openai + 派生 client_type', () async {
      final (_, f) = await boot(formFake());
      f.setEndpoints(const []);
      f.addEndpoint();
      expect(f.endpoints.single.protocol, 'openai');
      expect(f.endpoints.single.baseUrl, '');
      expect(f.endpoints.single.clientType, 'codex_tui');
      expect(f.endpoints.single.codingPlan, isFalse);
    });

    test('改端点协议会连带重置 client_type', () async {
      final (_, f) = await boot(formFake());
      f.setEndpoints(const []);
      f.addEndpoint();
      f.setEndpointClientType(0, 'claude_code');
      f.setEndpointProtocol(0, 'gemini');
      expect(f.endpoints.single.clientType, 'default');
    });

    test('coding_plan 开关 / 改 URL / 删端点', () async {
      final (_, f) = await boot(formFake());
      f.setEndpoints(const []);
      f.addEndpoint();
      f.addEndpoint();
      f.setEndpointBaseUrl(1, 'https://b');
      f.toggleEndpointCodingPlan(1);
      expect(f.endpoints[1].baseUrl, 'https://b');
      expect(f.endpoints[1].codingPlan, isTrue);
      f.removeEndpoint(0);
      expect(f.endpoints.length, 1);
      expect(f.endpoints.single.baseUrl, 'https://b');
    });

    test('厂商直连协议端点锁死', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('glm_coding');
      expect(f.endpointsLocked, isTrue);
      f.handleProtocolChange('openai');
      expect(f.endpointsLocked, isFalse);
    });

    test('C4：客户端模拟候选来自 get_client_types_json', () async {
      final k = formFake();
      final (_, f) = await boot(k);
      expect(k.commands.contains('get_client_types_json'), isTrue);
      expect(
        f.defaults.clientTypes.map((c) => c.value).join(','),
        'default,claude_code,codex_tui',
      );
    });
  });

  group('F7 + F8 Token 与批量预览', () {
    test('创建态多 key → 预览；单 key / 编辑态 / keyOptional → 不预览', () async {
      final (_, f) = await boot(formFake());
      f.openCreatePlatform();
      f.setApiKey('k1\nk2');
      expect(f.batchPreviewKeys, ['k1', 'k2']);
      expect(f.isBatch, isTrue);
      f.setApiKey('k1');
      expect(f.batchPreviewKeys, isNull);
      expect(f.isBatch, isFalse);

      f.protocol = 'opencode_zen';
      f.setApiKey('k1\nk2');
      expect(f.batchPreviewKeys, isNull);
    });

    test('编辑态不触发批量预览', () async {
      final k = formFake();
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      f.setApiKey('k1 k2 k3');
      expect(f.batchPreviewKeys, isNull);
    });

    test('预览名按 {base}-{尾4位} 生成，撞名追号', () async {
      final k = formFake(platforms: [plat(1, 'P-1234')]);
      final (_, f) = await boot(k);
      f.openCreatePlatform();
      f.setName('P');
      f.setApiKey('aaaa1234 bbbb1234');
      expect(f.previewNames, ['P-1234-2', 'P-1234-3']);
    });
  });

  group('F9 模型矩阵', () {
    test('一键填充把 default 灌到其余四槽；default 为空时不动', () async {
      final (_, f) = await boot(formFake());
      f.handleFillAll();
      expect(f.models['sonnet'], '');
      f.setModel('default', 'gpt-5');
      f.handleFillAll();
      expect(f.models['sonnet'], 'gpt-5');
      expect(f.models['opus'], 'gpt-5');
      expect(f.models['haiku'], 'gpt-5');
      expect(f.models['gpt'], 'gpt-5');
    });

    test('获取模型成功 → 自动归类并覆盖五槽', () async {
      final k = formFake();
      k.responses['platform_fetch_models'] = ['claude-opus-4', 'other'];
      final (_, f) = await boot(k);
      f.handleProtocolChange('openai');
      f.setApiKey('sk');
      await f.handleFetchModels();
      expect(f.availableModels, ['claude-opus-4', 'other']);
      expect(f.models['opus'], 'claude-opus-4');
      expect(f.models['default'], 'other');
      expect(f.fetchError, '');
      expect(f.fetching, isFalse);
    });

    test('缺 key 直接返回，不发命令', () async {
      final k = formFake();
      final (_, f) = await boot(k);
      f.handleProtocolChange('openai');
      await f.handleFetchModels();
      expect(k.commands.contains('platform_fetch_models'), isFalse);
    });

    test('全部端点返空 → 报 emptyText', () async {
      final k = formFake();
      final (_, f) = await boot(k);
      f.handleProtocolChange('openai');
      f.setApiKey('sk');
      await f.handleFetchModels(emptyText: '未获取到模型');
      expect(f.fetchError, '未获取到模型');
      expect(f.availableModels, isEmpty);
    });

    test('下拉候选：available 优先，否则回落 registry model_list', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('openai');
      expect(f.modelDropdownSource, ['gpt-5', 'gpt-5-mini']);
      f.availableModels = const ['x'];
      expect(f.modelDropdownSource, ['x']);
    });
  });

  group('F3 配额脚本分区（QuotaScriptSection.test.tsx 逐条翻译）', () {
    test('registry 无内置变体 → 直接进自定义编辑态', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('mock'); // 没有 quota_scripts
      expect(f.quotaVariants, isEmpty);
      expect(f.quotaSelection, kQuotaCustomVariant);
    });

    test('有内置变体且未选自定义 → 选中首条，非自定义', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('openai');
      expect(f.quotaSelection, 'v1');
      expect(f.selectedQuotaVariant!.id, 'v1');
    });

    test('有内置变体但选中自定义 → 进自定义编辑态', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('openai');
      f.handleQuotaVariantChange(kQuotaCustomVariant);
      expect(f.quotaSelection, kQuotaCustomVariant);
      expect(f.selectedQuotaVariant, isNull);
    });

    test('存量 id 失效 → 回落首条并打 fellBack 旗标', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('openai');
      f.quotaVariantId = 'gone';
      expect(f.quotaFellBack, isTrue);
      expect(f.quotaSelection, 'v1');
    });

    test('切回 registry 变体时清掉自定义正文（互斥）', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('openai');
      f.handleQuotaVariantChange(kQuotaCustomVariant);
      f.setQuotaCustomScript('// s');
      expect(f.quotaSelection, kQuotaCustomVariant);
      f.handleQuotaVariantChange('v1');
      expect(f.quotaCustomScript, '');
      expect(f.quotaSelection, 'v1');
    });

    test('requires 初值从 extra 回填（嵌套优先），用户填过的不覆盖', () async {
      final k = formFake(
        platforms: [
          plat(1, 'a', extra: '{"devin":{"org_id":"nested"},"org_id":"top"}'),
        ],
      );
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      f.handleProtocolChange('openai');
      expect(f.quotaRequires['org_id'], 'nested');
      f.setQuotaRequire('org_id', 'mine');
      f.handleQuotaVariantChange('v1');
      expect(f.quotaRequires['org_id'], 'mine');
    });

    test('newapi 协议额外带一个 user_id 键', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('newapi');
      expect(f.quotaRequires.containsKey('user_id'), isTrue);
    });
  });

  group('F10 手动预算与内置档位', () {
    test('创建态首次进某协议 → 自动填首档；编辑态不填', () async {
      final (_, f) = await boot(formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      expect(f.manualBudgets.length, 1);
      expect(f.manualBudgets.single.kind, 'rolling');
      expect(f.manualBudgets.single.unit, 'count');
      expect(f.manualBudgets.single.amount, 300);
      expect(f.manualBudgets.single.windowHours, 5);
      expect(f.manualBudgets.single.windowUnit, 'hour');
    });

    test('用户清空后切回本协议不再回填', () async {
      final (_, f) = await boot(formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      f.setManualBudgets(const []);
      f.handleProtocolChange('anthropic');
      f.handleProtocolChange('openai');
      expect(f.manualBudgets, isEmpty);
    });

    test('编辑态进来不自动填', () async {
      final k = formFake();
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      expect(f.manualBudgets, isEmpty);
    });

    test('tierToBudgets 每条独立 id，consumed 从 0 起算', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('openai');
      final tier = f.selectedPlanTier!;
      final a = PlatformFormController.tierToBudgets(tier);
      final b = PlatformFormController.tierToBudgets(tier);
      expect(a.single.consumed, 0);
      expect(a.single.id == b.single.id, isFalse);
    });
  });

  group('F11 熔断覆盖', () {
    test('scheduling_settings_get 的结果被存下来（旧实现丢了）', () async {
      final k = formFake(
        breaker: {
          'default_routing_mode': 'failover',
          'breaker_failure_threshold': 5,
          'breaker_open_secs': 60,
          'breaker_half_open_max': 2,
          'enabled': true,
        },
      );
      final (_, f) = await boot(k);
      expect(f.breakerDefaults!.failureThreshold, 5);
      expect(f.breakerDefaults!.openSecs, 60);
      expect(f.breakerDefaults!.halfOpenMax, 2);
    });

    test('读不到就是 null（表单退到不带数字的 placeholder）', () async {
      final (_, f) = await boot(formFake());
      expect(f.breakerDefaults, isNull);
    });

    test('编辑态从 extra.breaker 回填，0 显示成空串', () async {
      final k = formFake(
        platforms: [
          plat(
            1,
            'a',
            extra: '{"breaker":{"failure_threshold":3,"open_secs":0,'
                '"half_open_max":7}}',
          ),
        ],
      );
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      expect(f.breakerFailureThreshold, '3');
      expect(f.breakerOpenSecs, '');
      expect(f.breakerHalfOpenMax, '7');
    });

    test('保存时空 → 删 breaker 键；负值钳到 0', () async {
      final (_, f) = await boot(formFake());
      expect(decodeExtraObject(f.buildExtraPayload()).containsKey('breaker'),
          isFalse);
      f.setBreakerFailureThreshold('-9');
      expect(decodeExtraObject(f.buildExtraPayload()).containsKey('breaker'),
          isFalse);
      f.setBreakerFailureThreshold('4');
      expect(
        (decodeExtraObject(f.buildExtraPayload())['breaker']!
            as Map)['failure_threshold'],
        4,
      );
    });
  });

  group('F12 高峰时段', () {
    test('preset 默认窗口可导入', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('anthropic');
      expect(f.presetPeak.single.startHour, 9);
      f.handleProtocolChange('openai');
      expect(f.presetPeak, isEmpty);
    });

    test('编辑态从 extra 回填 peak / disable_during_peak / time_windows', () async {
      final k = formFake(
        platforms: [
          plat(
            1,
            'a',
            extra: jsonEncode({
              'peak': [
                {'start_hour': 1, 'end_hour': 2, 'multiplier': 3},
              ],
              'disable_during_peak': true,
              'time_windows': [
                {
                  'windows': [
                    {'start_hour': 4, 'end_hour': 5, 'multiplier': 1},
                  ],
                  'models': {'default': 'mm'},
                },
              ],
            }),
          ),
        ],
      );
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      expect(f.peak.single.multiplier, 3);
      expect(f.disableDuringPeak, isTrue);
      expect(f.timeModels.single.models['default'], 'mm');
      expect(f.timeModels.single.windows.single.startHour, 4);
    });

    test('时区模式默认本地，可切 UTC', () async {
      final (_, f) = await boot(formFake());
      expect(f.windowsTz, TzMode.local);
      f.setWindowsTz(TzMode.utc);
      expect(f.windowsTz, TzMode.utc);
    });
  });

  group('F13 分组归属', () {
    test('编辑态预选该平台所在的非 auto 分组', () async {
      final k = formFake(
        groups: [
          {
            'group': {
              'id': 1,
              'name': 'G1',
              'group_key': 'g1',
              'auto_from_platform': '',
            },
            'platforms': [
              {'platform': plat(1, 'a')},
            ],
          },
          {
            'group': {
              'id': 2,
              'name': 'auto',
              'group_key': 'g2',
              'auto_from_platform': '1',
            },
            'platforms': [
              {'platform': plat(1, 'a')},
            ],
          },
        ],
      );
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      expect(f.joinGroupIds, [1]);
    });

    test('从分组 ➕ 创建：锁定分组并关掉 auto_group', () async {
      final (_, f) = await boot(formFake());
      f.openCreatePlatform(lockGid: 7);
      expect(f.lockedGroupId, 7);
      expect(f.autoGroup, isFalse);
      expect(f.joinGroupIds, [7]);
    });

    test('toggleJoinGroup 是加/减开关', () async {
      final (_, f) = await boot(formFake());
      f.toggleJoinGroup(3);
      expect(f.joinGroupIds, [3]);
      f.toggleJoinGroup(3);
      expect(f.joinGroupIds, isEmpty);
    });
  });

  group('F14 过期时间', () {
    test('老平台 expires_at>0 → 开关默认 ON', () async {
      final k = formFake(platforms: [plat(1, 'a', expiresAt: 1700000000000)]);
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      expect(f.expiryEnabled, isTrue);
      expect(f.expiresAt, 1700000000000);
    });

    test('ON→OFF 清零；OFF→ON 保留已有值', () async {
      final (_, f) = await boot(formFake());
      f.setExpiresAt(123);
      f.setExpiryEnabled(true);
      expect(f.expiresAt, 123);
      f.setExpiryEnabled(false);
      expect(f.expiresAt, 0);
    });
  });

  group('extra 序列化链（usePlatformForm.ts:607）', () {
    test('mock 配置只有 mock 协议才写', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('openai');
      expect(decodeExtraObject(f.buildExtraPayload()).containsKey('mock'),
          isFalse);
      f.handleProtocolChange('mock');
      expect(decodeExtraObject(f.buildExtraPayload()).containsKey('mock'),
          isTrue);
    });

    test('quota 脚本序列化在 devin 之后（org_id 镜像写 extra.devin）', () async {
      final (_, f) = await boot(formFake());
      f.handleProtocolChange('devin');
      f.setDevinConfig(const DevinConfig(devinTimeout: '300', devinMode: 'fast'));
      f.setQuotaRequire('org_id', 'org-1');
      // devin 协议在夹具里没有 quota_scripts → 走 customOnly 分支，
      // 此时 requires 不落盘（React 同语义：custom 无元数据，不动 requires 键）。
      final o = decodeExtraObject(f.buildExtraPayload());
      expect((o['devin']! as Map)['devin_timeout'], 300);
      expect((o['devin']! as Map)['devin_mode'], 'fast');
    });

    test('原有 extra 的其他键一路保留到最后', () async {
      final k = formFake(platforms: [plat(1, 'a', extra: '{"mine":42}')]);
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      expect(decodeExtraObject(f.buildExtraPayload())['mine'], 42);
    });
  });

  group('F17 保存', () {
    test('保存成功 → 关表单、清空状态', () async {
      final k = formFake();
      final (_, f) = await boot(k);
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      f.setName('n');
      f.setApiKey('sk');
      final ok = await f.handleSave();
      expect(ok, isTrue);
      expect(f.showForm, isFalse);
      expect(f.saveError, '');
      expect(lastInput(k, 'platform_create')['name'], 'n');
      expect(lastInput(k, 'platform_create')['auto_group'], true);
    });

    test('保存失败 → 表单留着，saveError 带原文', () async {
      final k = formFake();
      k.errors['platform_create'] = StateError('boom');
      final (_, f) = await boot(k);
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      f.setName('n');
      f.setApiKey('sk');
      final ok = await f.handleSave();
      expect(ok, isFalse);
      expect(f.showForm, isTrue);
      expect(f.saveError.contains('boom'), isTrue);
    });

    test('编辑保存走 platform_update 且带 id、不带 auto_group', () async {
      final k = formFake();
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      await f.handleSave();
      final input = lastInput(k, 'platform_update');
      expect(input['id'], 1);
      expect(input.containsKey('auto_group'), isFalse);
      // 更新语义：手动预算空也要带（表示清空）。
      expect(input['manual_budgets'], isEmpty);
    });

    test('透传平台不带手动预算', () async {
      final k = formFake();
      final (_, f) = await boot(k);
      f.openCreatePlatform();
      f.handleProtocolChange('claude_code');
      f.setName('n');
      f.setPassthroughBaseUrl('https://api.anthropic.com');
      f.setManualBudgets([newManualBudget()]);
      await f.handleSave();
      expect(
        lastInput(k, 'platform_create').containsKey('manual_budgets'),
        isFalse,
      );
    });

    test('批量创建：每 key 一个平台，name 各自带尾 4 位', () async {
      final k = formFake();
      var nextId = 10;
      k.responses['platform_create'] = () => plat(nextId++, 'x');
      final (_, f) = await boot(k);
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      f.setName('P');
      f.setApiKey('aaaa1111 bbbb2222');
      final ok = await f.handleSave();
      expect(ok, isTrue);
      final names = k
          .callsTo('platform_create')
          .map((c) => (c.args!['input']! as Map)['name'])
          .join(',');
      expect(names, 'P-1111,P-2222');
      expect(f.showForm, isFalse);
    });

    test('批量创建：没有 base_url 也没有端点 → 一条都不发', () async {
      final k = formFake();
      final (_, f) = await boot(k);
      f.openCreatePlatform();
      f.setName('P');
      f.setEndpoints(const []);
      f.setApiKey('aaaa1111 bbbb2222');
      await f.handleSave();
      expect(k.commands.contains('platform_create'), isFalse);
    });

    test('批量创建：单条失败不中断整批', () async {
      final k = formFake();
      var n = 0;
      k.responses['platform_create'] = () {
        n++;
        if (n == 1) throw StateError('nope');
        return plat(20 + n, 'x');
      };
      final (_, f) = await boot(k);
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      f.setName('P');
      f.setApiKey('aaaa1111 bbbb2222');
      await f.handleSave();
      expect(k.callsTo('platform_create').length, 2);
    });
  });

  group('resetForm', () {
    test('把三十来个字段全复位', () async {
      final k = formFake();
      final (l, f) = await boot(k);
      f.handleEdit(l.platforms.first);
      f.setName('x');
      f.setApiKey('y');
      f.setDisableDuringPeak(true);
      f.setWindowsTz(TzMode.utc);
      f.setPeak(const [TimeWindow(startHour: 1, endHour: 2, multiplier: 1)]);
      f.setExpiryEnabled(true);
      f.resetForm();
      expect(f.name, '');
      expect(f.protocol, 'openai');
      expect(f.apiKey, '');
      expect(f.editing, isNull);
      expect(f.showForm, isFalse);
      expect(f.endpoints, isEmpty);
      expect(f.availableModels, isEmpty);
      expect(f.models.values.every((v) => v.isEmpty), isTrue);
      expect(f.peak, isEmpty);
      expect(f.timeModels, isEmpty);
      expect(f.disableDuringPeak, isFalse);
      expect(f.windowsTz, TzMode.local);
      expect(f.autoGroup, isTrue);
      expect(f.joinGroupIds, isEmpty);
      expect(f.lockedGroupId, isNull);
      expect(f.expiresAt, 0);
      expect(f.expiryEnabled, isFalse);
      expect(f.batchPreviewKeys, isNull);
      expect(f.saveError, '');
      expect(f.fetchError, '');
    });
  });

  group('handleDuplicate（usePlatformForm.ts:405）', () {
    test('灌满字段但 editing 留 null（保存走 create）', () async {
      final k = formFake(platforms: [plat(1, 'a', extra: '{"mine":1}')]);
      final (l, f) = await boot(k);
      f.handleDuplicate(l.platforms.first);
      expect(f.editing, isNull);
      expect(f.showForm, isTrue);
      expect(f.name, 'a');
      expect(f.apiKey, 'sk-1');
      expect(f.models['default'], 'm1');
      expect(decodeExtraObject(f.buildExtraPayload())['mine'], 1);
    });
  });

  group('init 的容错', () {
    test('两条命令都炸 → 空文档，表单照常能开', () async {
      final k = formFake();
      k.errors['get_defaults_json'] = StateError('x');
      k.errors['get_client_types_json'] = StateError('y');
      final list = PlatformsController(invoke: k.fn);
      final f = PlatformFormController(list: list, invoke: k.fn);
      await f.init();
      expect(f.defaults.protocols, isEmpty);
      expect(f.defaults.clientTypes, isEmpty);
      f.openCreatePlatform();
      expect(f.showForm, isTrue);
      f.handleProtocolChange('openai');
      expect(f.endpoints, isEmpty);
    });
  });
}
