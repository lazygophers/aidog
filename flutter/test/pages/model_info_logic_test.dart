/// 模型信息页逻辑层的单测（票 I09）。
///
/// 前半是**从 `src/pages/ModelInfo/ModelInfoTab.test.tsx` 逐条翻过来的**：
/// SNAPSHOT 的每一个字段、每一个断言的数据与期望值**一字不改**（含
/// `1.1e-6` / `4.2e-6` / `131072` / `aihubmix` 那三条同 canonical 的 SKU）。
/// 那份 React 测试断言的是渲染结果，这里断言的是驱动渲染的那份派生数据 ——
/// 同一批事实，换一层表达。
///
/// 后半是 React 侧没有测试覆盖、但本票新写的纯函数（价格解析 / 分页页码 / 名字拆分）。
library;

import 'dart:convert';

import 'package:aidog_flutter/pages.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';

/// `ModelInfoTab.test.tsx:31::entry()` 的默认值，一字不改。
Map<String, dynamic> entry(
  String platformCode,
  String modelId, {
  String? displayName,
  String? canonicalModel,
  String family = '',
  String version = '',
  String predecessor = '',
  List<String> capabilities = const [],
  List<String> builtinToolsExcluded = const [],
  int? maxInputTokens,
  int? maxOutputTokens,
  int? contextWindow,
  bool official = false,
  String priceData = '{}',
  int updatedAt = 0,
}) => {
  'platform_code': platformCode,
  'model_id': modelId,
  'display_name': displayName ?? modelId,
  'canonical_model': canonicalModel ?? modelId,
  'family': family,
  'version': version,
  'predecessor': predecessor,
  'capabilities': capabilities,
  'builtin_tools_excluded': builtinToolsExcluded,
  'max_input_tokens': maxInputTokens,
  'max_output_tokens': maxOutputTokens,
  'context_window': contextWindow,
  'official': official,
  'price_data': priceData,
  'updated_at': updatedAt,
};

/// `ModelInfoTab.test.tsx:50::SNAPSHOT`，逐字段照抄。
Map<String, dynamic> snapshotFixture({
  bool bundled = false,
  List<String> pricingOnly = const [],
  List<Map<String, dynamic>>? groups,
}) => {
  'bundled': bundled,
  'platforms': const <Object?>[],
  'pricing_only': pricingOnly,
  'groups':
      groups ??
      [
        {
          'canonical_model': 'glm-4.6',
          'display_name': 'GLM-4.6',
          'primary_platform': 'glm',
          'entries': [
            entry(
              'glm',
              'glm-4.6',
              displayName: 'GLM-4.6',
              capabilities: const ['text', 'tool_use'],
              contextWindow: 131072,
              official: true,
              family: 'glm',
              version: '4.6',
              predecessor: 'glm-4.5',
              priceData: jsonEncode({
                'price': {
                  'input': 1.1e-6,
                  'output': 4.2e-6,
                  'peak': {'input': 3.3e-6, 'output': 1.26e-5},
                },
              }),
            ),
            entry(
              'openrouter',
              'zhipu/glm-4.6',
              displayName: 'GLM-4.6 (OpenRouter)',
              capabilities: const ['text'],
              priceData: jsonEncode({
                'price': {'input': 2e-6},
              }),
            ),
          ],
        },
        // 未维护 display_name 的模型：读取层（T10）已把展示名回落成 model_id。
        {
          'canonical_model': 'mystery-model',
          'display_name': 'mystery-model',
          'primary_platform': 'openrouter',
          'entries': [entry('openrouter', 'mystery-model')],
        },
      ],
};

/// `ModelInfoTab.test.tsx:102::settingsGetMock` 的返回值，一字不改
/// （注意 React 那份**没有** `registry_last_updated`，读取层给缺省 0）。
const Map<String, dynamic> kSettingsFixture = {
  'auto_sync_enabled': false,
  'sync_interval_secs': 86400,
  'last_sync_at': 0,
  'fallback_input_price': 3,
  'fallback_output_price': 3,
};

/// `defaults` 的平台本地化名：与 React 测试里 `getProtocolLabelMap` 的 mock
/// 返回一致（`{glm: 智谱 GLM, openrouter: OpenRouter, aihubmix: AIHubMix}`）。
String labelDefaultsJson() => jsonEncode({
  'protocols': {
    'glm': {
      'name': {'zh-Hans': '智谱 GLM', 'en-US': '智谱 GLM'},
    },
    'openrouter': {
      'name': {'zh-Hans': 'OpenRouter', 'en-US': 'OpenRouter'},
    },
    'aihubmix': {
      'name': {'zh-Hans': 'AIHubMix', 'en-US': 'AIHubMix'},
    },
  },
});

FakeKernel fake({
  Map<String, dynamic>? snapshot,
  Object? Function(Map<String, Object?>?)? sync,
}) => FakeKernel({
  'model_info_snapshot': (_) => snapshot ?? snapshotFixture(),
  'price_sync_settings_get': (_) => kSettingsFixture,
  'price_sync_settings_set': (_) => null,
  'get_defaults_json': (_) => labelDefaultsJson(),
  'model_price_sync': sync ?? (_) => throw StateError('本用例不该同步'),
});

Future<ModelInfoController> booted(FakeKernel k) async {
  final c = ModelInfoController(
    invoke: k.invoke,
    t: (key, [args]) => key,
    onChanged: () {},
  );
  await c.init('zh-Hans');
  return c;
}

void main() {
  group('翻自 ModelInfoTab.test.tsx（数据与期望值一字不改）', () {
    test('模型维度: 一行一 canonical，代表平台 = primary_platform，官方徽标', () async {
      final c = await booted(fake());
      expect(c.pageRows, hasLength(2));
      final g = c.pageRows.first;
      expect(g.displayName, 'GLM-4.6');
      // 代表条目 = primary_platform，平台名走 registry labelMap
      expect(g.primary!.platformCode, 'glm');
      expect(c.platformLabel('glm'), '智谱 GLM');
      expect(g.primary!.official, isTrue);
      // 第二个平台折叠成「还有 N 个平台」提示 → N = entries - 1
      expect(g.entries.length - 1, 1);
    });

    test('展示名存在时: 展示名与真实请求名同屏并列', () async {
      final c = await booted(fake());
      final g = c.pageRows.first;
      final parts = nameParts(g.displayName, g.primary!.modelId);
      expect(parts.primary, 'GLM-4.6');
      expect(parts.secondary, 'glm-4.6');
    });

    test('展示名缺省时: 只渲染一次 model_id，不出现空节点', () async {
      final c = await booted(fake());
      final g = c.pageRows[1];
      final parts = nameParts(g.displayName, g.entries.first.modelId);
      expect(parts.primary, 'mystery-model');
      expect(parts.secondary, isNull);
    });

    test('详情: 展示名缺省时不重复出一行展示名，请求名可复制', () async {
      final c = await booted(fake());
      c.select('mystery-model');
      final g = c.selectedGroup!;
      final e = c.activeDetailEntry(g)!;
      // 展示名 == 请求名 → 只保留「请求名」Field
      expect(nameParts(e.displayName, e.modelId).secondary, isNull);
      // 可复制的那一串就是 model_id（React: writeText('mystery-model')）
      expect(e.modelId, 'mystery-model');
    });

    test('详情: 展示名与请求名不同则两行都在', () async {
      final c = await booted(fake());
      c.select('glm-4.6');
      final e = c.activeDetailEntry(c.selectedGroup!)!;
      expect(nameParts(e.displayName, e.modelId).primary, 'GLM-4.6');
      expect(nameParts(e.displayName, e.modelId).secondary, 'glm-4.6');
    });

    test('平台维度: 切过去后按平台列出模型条目', () async {
      final c = await booted(fake());
      c.setTab('platforms');
      // 未选平台时给引导文案 → 右栏为空
      expect(c.activePlatform, isNull);
      expect(c.activePlatformEntries, isEmpty);
      c.selectPlatform('openrouter');
      final ids = [for (final e in c.activePlatformEntries) e.modelId];
      expect(ids, contains('zhipu/glm-4.6'));
      final names = [for (final e in c.activePlatformEntries) e.displayName];
      expect(names, contains('GLM-4.6 (OpenRouter)'));
    });

    test('详情: 按平台分 tab，聚合版本链 / 默认价 / 高峰价', () async {
      final c = await booted(fake());
      c.select('glm-4.6');
      final g = c.selectedGroup!;
      final entries = c.detailEntries(g);
      // 两个平台条目各一个 tab
      expect([for (final e in entries) c.platformLabel(e.platformCode)], [
        '智谱 GLM',
        'OpenRouter',
      ]);
      // 默认打开 primary_platform：版本链 + 高峰价
      final e = c.activeDetailEntry(g)!;
      expect(e.predecessor, 'glm-4.5');
      final price = parsePriceData(e.priceData);
      expect(price.input, 1.1e-6);
      expect(price.output, 4.2e-6);
      expect(price.peak, isNotNull);
      expect(price.peak!.input, 3.3e-6);
      expect(price.peak!.output, 1.26e-5);
    });

    test('详情: 同平台多条 model_id 各一个 tab，非 canonical 条目带 model_id 后缀', () async {
      // 复现 glm-5.3 重复：aihubmix 对同一 canonical 挂 coding-/按量/free 三条 SKU。
      // 修复前 tab 的 key/value 只有 platform_code → 三 tab 同值互相覆盖。
      final c = await booted(
        fake(
          snapshot: snapshotFixture(
            groups: [
              {
                'canonical_model': 'glm-5.3',
                'display_name': 'GLM-5.3',
                'primary_platform': 'aihubmix',
                'entries': [
                  entry(
                    'aihubmix',
                    'coding-glm-5.3',
                    canonicalModel: 'glm-5.3',
                    displayName: 'GLM-5.3',
                    priceData: jsonEncode({
                      'price': {'input': 6e-8},
                    }),
                  ),
                  entry(
                    'aihubmix',
                    'glm-5.3',
                    canonicalModel: 'glm-5.3',
                    displayName: 'GLM-5.3',
                    priceData: jsonEncode({
                      'price': {'input': 1.13e-6},
                    }),
                  ),
                  entry(
                    'aihubmix',
                    'coding-glm-5.3-free',
                    canonicalModel: 'glm-5.3',
                    displayName: 'GLM-5.3',
                    priceData: jsonEncode({
                      'price': {'input': 0},
                    }),
                  ),
                ],
              },
            ],
          ),
        ),
      );
      c.select('glm-5.3');
      final g = c.selectedGroup!;
      final entries = c.detailEntries(g);
      // 三条 SKU = 三个 tab，不再被同 value 覆盖成一个
      expect(entries, hasLength(3));
      final keys = [for (final e in entries) ModelInfoController.detailTabKey(e)];
      expect(keys.toSet(), hasLength(3));
      // model_id = canonical 的 tab 无后缀；另两条带 model_id 后缀区分
      final labels = [
        for (final e in entries)
          '${c.platformLabel(e.platformCode)}'
              '${e.modelId != g.canonicalModel ? '· ${e.modelId}' : ''}',
      ];
      expect(labels, contains('AIHubMix'));
      expect(labels, contains('AIHubMix· coding-glm-5.3'));
      expect(labels, contains('AIHubMix· coding-glm-5.3-free'));
      // 默认选中 primary（entries 顺序里的第一条 aihubmix）
      expect(c.activeDetailEntry(g)!.modelId, 'coding-glm-5.3');
    });

    test('同步失败清单: partial failures 列出文件与原因', () async {
      final k = fake(
        sync: (_) => const {
          'added': 1,
          'updated': 2,
          'unchanged': 0,
          'failed': 1,
          'total': 4,
          'failures': [
            {'file': 'platforms/glm/models/glm-4.6.json', 'error': '404'},
          ],
        },
      );
      final c = await booted(k);
      await c.sync();
      expect(k.countOf('model_price_sync'), 1);
      expect(c.syncResult!.added, 1);
      expect(c.syncResult!.updated, 2);
      expect(c.syncResult!.failed, 1);
      expect(c.syncResult!.total, 4);
      expect(c.syncResult!.failures, hasLength(1));
      expect(c.syncResult!.failures.first.file,
          'platforms/glm/models/glm-4.6.json');
      expect(c.syncResult!.failures.first.error, '404');
    });

    test('pricing_only 来源: 不进平台筛选与平台维度，详情里仍保留比价条目', () async {
      final c = await booted(
        fake(snapshot: snapshotFixture(pricingOnly: const ['openrouter'])),
      );
      // 平台维度左侧列表不再列它
      expect(c.platformCodes, isNot(contains('openrouter')));
      expect(c.visiblePlatformCodes, isNot(contains('openrouter')));
      // 详情仍保留该条目比价，且被排到最后（带「仅比价」标注）
      c.select('glm-4.6');
      final entries = c.detailEntries(c.selectedGroup!);
      expect(entries.last.platformCode, 'openrouter');
      expect(c.pricingOnly.contains('openrouter'), isTrue);
    });

    test('bundled 兜底: snapshot.bundled=true 时提示尚未同步', () async {
      final c = await booted(fake(snapshot: snapshotFixture(bundled: true)));
      expect(c.bundled, isTrue);
    });
  });

  group('筛选与分页', () {
    test('关键字匹配展示名 / canonical / 任一 model_id', () async {
      final c = await booted(fake());
      c.setQuery('zhipu/');
      expect([for (final g in c.filtered) g.canonicalModel], ['glm-4.6']);
      c.setQuery('MYSTERY');
      expect([for (final g in c.filtered) g.canonicalModel], ['mystery-model']);
      c.setQuery('GLM-4');
      expect([for (final g in c.filtered) g.canonicalModel], ['glm-4.6']);
    });

    test('平台 / 能力 / 官方三个筛选各自短路', () async {
      final c = await booted(fake());
      c.setPlatformFilter('glm');
      expect(c.filtered, hasLength(1));
      c.clearFilter();
      c.setCapabilityFilter('tool_use');
      expect(c.filtered, hasLength(1));
      c.clearFilter();
      c.setOfficialOnly(true);
      expect(c.filtered, hasLength(1));
      expect(c.hasFilter, isTrue);
      c.clearFilter();
      expect(c.hasFilter, isFalse);
      expect(c.filtered, hasLength(2));
    });

    test('改任一筛选都把页码打回第 1 页', () async {
      final c = await booted(fake());
      c.setPageSize(1);
      c.setPage(2);
      expect(c.currentPage, 2);
      c.setQuery('');
      expect(c.page, 1);
    });

    test('页码越界时 currentPage 被收进合法范围，pageRows 不越界', () async {
      final c = await booted(fake());
      c.setPageSize(1);
      c.setPage(99);
      expect(c.totalPages, 2);
      expect(c.currentPage, 2);
      expect(c.pageRows, hasLength(1));
    });

    test('跳页: 越界什么都不做，合法才跳并清空输入', () async {
      final c = await booted(fake());
      c.setPageSize(1);
      c.setJumpPage('9');
      c.jump();
      expect(c.page, 1);
      expect(c.jumpPage, '9');
      c.setJumpPage('2');
      c.jump();
      expect(c.page, 2);
      expect(c.jumpPage, '');
    });

    test('空结果时 totalPages 仍是 1，pageRows 为空', () async {
      final c = await booted(fake());
      c.setQuery('绝无此模型');
      expect(c.filtered, isEmpty);
      expect(c.totalPages, 1);
      expect(c.pageRows, isEmpty);
    });

    test('paginationPages: ≤7 全列；>7 首末 + 当前 ±1 + 省略号', () {
      expect(paginationPages(1, 3), [1, 2, 3]);
      expect(paginationPages(1, 7), [1, 2, 3, 4, 5, 6, 7]);
      // 当前页在最前：右边一个省略号
      expect(paginationPages(1, 20), [1, 2, null, 20]);
      // 当前页在中间：两边各一个
      expect(paginationPages(10, 20), [1, null, 9, 10, 11, null, 20]);
      // 当前页在最后：左边一个
      expect(paginationPages(20, 20), [1, null, 19, 20]);
    });
  });

  group('价格解析（priceData.ts）', () {
    test('空串 / 非法 JSON / 缺 price 一律给空对象，不抛', () {
      for (final raw in ['', 'not json', '[]', '{}', '{"price": 3}']) {
        final p = parsePriceData(raw);
        expect(p.input, isNull, reason: raw);
        expect(p.contextTiers, isEmpty, reason: raw);
      }
    });

    test('每 token 单价 → 每百万 token 单价', () {
      expect(perMillion(1.1e-6), closeTo(1.1, 1e-9));
      expect(perMillion(null), isNull);
      expect(perMillion(double.infinity), isNull);
      expect(fmtPricePerM(null), '-');
      expect(fmtPricePerM(3e-6), '\$3.00');
    });

    test('非 token 计价：char 是 /1K chars，其余是 /unit', () {
      expect(fmtPricePerUnit(0.04, 'image'), '\$0.040 /image');
      expect(fmtPricePerUnit(0.04, 'char'), '\$0.040 /1K chars');
      expect(fmtPricePerUnit(0.04, null), '\$0.040 /unit');
      expect(fmtPricePerUnit(null, 'image'), '-');
    });

    test('context_tiers 解析出 min_tokens', () {
      final p = parsePriceData(
        jsonEncode({
          'price': {
            'input': 1e-6,
            'context_tiers': [
              {'min_tokens': 200000, 'input': 2e-6, 'output': 8e-6},
            ],
          },
        }),
      );
      expect(p.contextTiers, hasLength(1));
      expect(p.contextTiers.first.minTokens, 200000);
      expect(p.contextTiers.first.input, 2e-6);
    });

    test('顶层 thinking 标记：未标注是 null，不与 false 混同', () {
      expect(parseEntryFlags('{}').thinkingSupported, isNull);
      expect(parseEntryFlags('').thinkingToggleable, isNull);
      final f = parseEntryFlags(
        jsonEncode({'thinking_supported': true, 'thinking_toggleable': false}),
      );
      expect(f.thinkingSupported, isTrue);
      expect(f.thinkingToggleable, isFalse);
    });

    test('fmtTokens: 131072 → 131.1K；缺值 → -', () {
      expect(fmtTokens(131072), '131.1K');
      expect(fmtTokens(null), '-');
    });

    test('capabilityLabel: 已知枚举走 i18n key，未知值原样显示', () {
      expect(capabilityLabel((k, [a]) => 'T:$k', 'text'), 'T:modelInfo.cap.text');
      expect(capabilityLabel((k, [a]) => 'T:$k', 'brand_new_cap'), 'brand_new_cap');
    });
  });

  group('同步设置', () {
    test('取不到设置时用 DEFAULT_SYNC_SETTINGS 的值', () async {
      final k = FakeKernel({
        'model_info_snapshot': (_) => snapshotFixture(),
        'price_sync_settings_get': (_) => throw StateError('boom'),
        'get_defaults_json': (_) => labelDefaultsJson(),
      });
      final c = await booted(k);
      expect(c.settings.syncIntervalSecs, 86400);
      expect(c.settings.fallbackInputPrice, 3.0);
      expect(c.settings.autoSyncEnabled, isFalse);
    });

    test('改设置先本地生效再落盘，落盘的 JSON 是 snake_case', () async {
      final k = fake();
      final c = await booted(k);
      await c.updateSettings(c.settings.copyWith(autoSyncEnabled: true));
      expect(c.settings.autoSyncEnabled, isTrue);
      final args = k.lastArgsOf('price_sync_settings_set')!;
      final sent = (args['settings'] as Map).cast<String, Object?>();
      expect(sent['auto_sync_enabled'], isTrue);
      expect(sent['sync_interval_secs'], 86400);
      expect(sent['fallback_input_price'], 3.0);
    });

    test('快照拉取失败时 message 落地、loading 收回、不抛', () async {
      final k = FakeKernel({
        'model_info_snapshot': (_) => throw StateError('db closed'),
        'price_sync_settings_get': (_) => kSettingsFixture,
        'get_defaults_json': (_) => labelDefaultsJson(),
      });
      final c = await booted(k);
      expect(c.loading, isFalse);
      expect(c.message, contains('db closed'));
      expect(c.groups, isEmpty);
    });

    test('平台本地化名取不到时回落裸 code', () async {
      final k = FakeKernel({
        'model_info_snapshot': (_) => snapshotFixture(),
        'price_sync_settings_get': (_) => kSettingsFixture,
        'get_defaults_json': (_) => throw StateError('no defaults'),
      });
      final c = await booted(k);
      expect(c.platformLabel('glm'), 'glm');
    });

    test('本页不碰 model_entry_list（3.44 MB / 4580 行，不该进任何页面路径）', () async {
      final k = fake();
      await booted(k);
      expect(k.calls, isNot(contains('model_entry_list')));
    });
  });
}
