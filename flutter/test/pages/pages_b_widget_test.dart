/// 页面批次 B 的 widget 测试（票 I07）：能挂载、能点、能填、失败有提示，
/// 以及**破坏性操作确认之前一个命令都不发**。
///
/// 两个坑沿用 I06 写在 README 里的（别再踩一遍）：
///   - 不用 `pumpAndSettle`：骨架的 `LiveDot` 是无限循环呼吸动画，等不到静止。
///     用 `harness.dart` 的 `settle(tester)`。
///   - 画布默认 800×600，这几页一屏放不下，`tap()` 会判成「点不到」。
///     交互测试先 `useBigSurface(tester)`。
library;

import 'dart:async';

import 'package:flutter_svg/flutter_svg.dart';

import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/shell.dart' show AidogColors;
import 'package:aidog_flutter/utils/formatters.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';
import 'harness.dart';

Map<String, dynamic> plat(
  int id,
  String name, {
  String status = 'enabled',
  bool enabled = true,
}) => {
  'id': id,
  'name': name,
  'platform_type': 'openai',
  'base_url': 'https://u$id',
  'api_key': 'k',
  'status': status,
  'enabled': enabled,
  'models': {'default': 'm'},
  'available_models': ['m'],
  'endpoints': <Object?>[],
  'est_balance_remaining': 0,
};

Map<String, dynamic> logRow(
  String id, {
  int status = 200,
  bool isStream = false,
  int retryCount = 0,
}) => {
  'id': id,
  'group_key': 'gk',
  'model': 'm',
  'actual_model': 'am',
  'platform_id': 1,
  'status_code': status,
  'duration_ms': 12,
  'input_tokens': 10,
  'output_tokens': 20,
  'cache_tokens': 0,
  'is_stream': isStream,
  'retry_count': retryCount,
  'created_at': 1700000000000,
};

FakeInvoke logsFake({List<Object?>? items}) => FakeInvoke({
  'platform_list': [plat(1, 'P1')],
  // 协议本地化名的数据源（详情面板印的是它，不是裸枚举值）。
  'get_defaults_json': '{"protocols":{}}',
  'get_client_types_json': '{}',
  'group_detail_list': [
    {
      'group': {'id': 1, 'group_key': 'gk', 'name': 'G1'},
      'platforms': <Object?>[],
    },
  ],
  'proxy_log_distinct_models': <Object?>[],
  'proxy_log_list_filtered': {
    'items': items ?? [logRow('a1')],
    'has_more': false,
  },
  'proxy_log_get': {
    'id': 'a1',
    'group_key': 'gk',
    'model': 'm',
    'actual_model': 'am',
    'status_code': 200,
    'duration_ms': 12,
    'request_headers': '{}',
    'request_body': '{}',
    'response_body': '{"ok":true}',
  },
  'proxy_log_clear': null,
  'proxy_log_cleanup_expired': null,
});

FakeInvoke reqLogFake({List<Object?>? items}) => FakeInvoke({
  'platform_list': [plat(1, 'P1')],
  'request_log_list': items ?? [logRow('r1')],
  'proxy_log_count_filtered': 1,
  'proxy_log_get': {'id': 'r1', 'status_code': 200},
});

FakeInvoke groupsFake({List<Object?>? page}) => FakeInvoke({
  'platform_list': [plat(1, 'P1'), plat(2, 'P2')],
  'group_detail_list_paged':
      page ??
      [
        {
          'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
          'platforms': [
            {'platform': plat(1, 'P1')},
          ],
          'model_mappings': <Object?>[],
        },
      ],
  'group_detail_list': [
    {
      'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
      'platforms': [
        {'platform': plat(1, 'P1')},
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
  'platform_delete': null,
  'platform_purge_disabled_preview': <Object?>[],
  'platform_purge_disabled': {
    'deletedIds': <Object?>[],
    'unassignedIds': <Object?>[],
  },
  'model_test': {'success': true, 'duration_ms': 5, 'error': ''},
  'set_ui_extra': null,
});

FakeInvoke platformsFake() => FakeInvoke({
  ...groupsFake().responses,
  // 默认**没有任何分组成员**，两个平台都算「未分组」—— 否则 P1 被
  // groupsFake 的分组吃掉，未分组区里就只剩 P2 了。
  // 需要测「已分组的不出现在未分组区」的那条自己覆盖这一项。
  'group_detail_list': <Object?>[],
  'all_platform_usage_stats': <String, Object?>{},
  'get_last_test_result': null,
  'scheduling_settings_get': null,
  'platform_update': plat(1, 'P1', status: 'disabled', enabled: false),
  'platform_usage_stats': null,
  'platform_query_quota': {'success': true, 'queried_at': 1},
});

/// 组内平台卡的替身。本文件测的是**分组行为**（改名、映射、批量、移组…），
/// 不是卡片渲染：真卡片挂进来会把每个用例的断言都拖进 `PlatformCard` 的整棵子树。
/// 卡片本体由 `platform_card_test.dart` 守，组内接线由 `groups_platform_card_test.dart` 守。
/// per-group 优先级已并进卡内（行 1.5），替身直接复用同一个控件，
/// 让优先级用例不必挂整棵真卡。
Widget stubPlatformCard(
  PlatformRow p,
  int index, {
  int? levelPriority,
  void Function(int)? onLevelPriorityChange,
}) => Column(
  mainAxisSize: MainAxisSize.min,
  crossAxisAlignment: CrossAxisAlignment.start,
  children: [
    Text(p.name),
    if (onLevelPriorityChange != null)
      LevelPriorityControl(
        key: ValueKey('level-priority-${p.id}'),
        value: levelPriority ?? 5,
        onChanged: onLevelPriorityChange,
      ),
  ],
);

/// 组内拖放的插入线：2px 高、accent 底色的小条（私有 widget，按形状认）。
bool _isDropLine(Widget w) =>
    w is Container &&
    w.constraints?.maxHeight == 2 &&
    w.decoration is BoxDecoration;

void main() {
  group('LogsPage', () {
    testWidgets('挂载就整查一遍并把行画出来（不等事件）', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      expect(k.callsTo('proxy_log_list_filtered').length, 1);
      expect(find.text('am'), findsWidgets);
    });

    // 回归 2026-09-22：这三个筛选器的逻辑早就在 `LogsFilterState`
    //（observed / modelType / path），但筛选条上一个入口都没有，点不到。
    // 另外「无平台」`platform_id=0` 与「无分组」`group_key=''` 两个哨兵项也缺，
    // 隧道请求那批行筛不出来（`ListView.tsx:96,110`）。
    testWidgets('筛选条：中间件 / 模型类型 / 路径三个入口都在', (tester) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: logsFake().fn), c));
      await settle(tester);
      expect(find.text(c.t('logs.filterMiddleware')), findsOneWidget);
      // 「实际模型」「原始模型」在表头也各有一份，这里只确认筛选条那份也在。
      expect(find.text(c.t('logs.actualModel')), findsWidgets);
      expect(find.text(c.t('logs.model')), findsWidgets);
      expect(find.byKey(const Key('logs-path')), findsOneWidget);
    });

    testWidgets('路径搜索输进去 → 下一轮查询带上 path', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      await tester.enterText(
        find.byKey(const Key('logs-path')),
        '/v1/messages',
      );
      await settle(tester);
      final args = k.callsTo('proxy_log_list_filtered').last;
      final filter = args.args?['filter'] as Map<String, Object?>?;
      expect(filter?['path'], '/v1/messages');
    });

    testWidgets('平台下拉有「无平台」，分组下拉有「无分组」', (tester) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: logsFake().fn), c));
      await settle(tester);
      // 「平台」表头也有一份，筛选条在上面，取 .first。
      await tester.tap(find.text(c.t('logs.filterPlatform')).first);
      await settle(tester);
      expect(find.text(c.t('logs.noPlatform')), findsOneWidget);
      // 关掉再开分组那个，两个下拉不叠在一起。
      await tester.tap(find.text(c.t('logs.noPlatform')));
      await settle(tester);
      await tester.tap(find.text(c.t('logs.filterGroup')).first);
      await settle(tester);
      expect(find.text(c.t('logs.noGroup')), findsOneWidget);
    });

    // 第二梯队 2026-09-22：表里原先只有九列（缺「原始模型」），两个徽标的
    // 字段早就解析进来却不画（`ListView.tsx:202-211`、`primitives.tsx:296-306`）。
    testWidgets('表头有「原始模型」列；重试与流式各自挂徽标', (tester) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      final k = logsFake(items: [logRow('a1', isStream: true, retryCount: 3)]);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      expect(find.text(c.t('logs.model')), findsWidgets, reason: '表头缺这一列');
      expect(find.text('m'), findsOneWidget, reason: '原始模型的值');
      expect(find.text('am'), findsOneWidget, reason: '实际模型的值');
      expect(find.text('↻3'), findsOneWidget);
      expect(find.text('SSE'), findsOneWidget);
    });

    testWidgets('不重试、非流式时两个徽标都不出现', (tester) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: logsFake().fn), c));
      await settle(tester);
      expect(find.text('SSE'), findsNothing);
      expect(find.textContaining('↻'), findsNothing);
    });

    // 原先主日志页只能等事件流推送，想立刻看一眼最新的没有入口
    //（`ListView.tsx:70-72`）。
    testWidgets('页头有刷新按钮，点一下重查一遍', (tester) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      final k = logsFake();
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      final before = k.callsTo('proxy_log_list_filtered').length;
      await tester.tap(find.byKey(const ValueKey('logs-refresh')));
      await settle(tester);
      expect(k.callsTo('proxy_log_list_filtered').length, before + 1);
    });

    testWidgets('空列表显示空态，不画假的表格', (tester) async {
      await useBigSurface(tester);
      final k = logsFake(items: const []);
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      expect(find.byType(CenteredNote), findsOneWidget);
    });

    testWidgets('流上来一下 → 静默重查一次，界面不闪 loading', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final ticker = LogTicker();
      addTearDown(ticker.close);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(LogsPage(invoke: k.fn, logUpdates: ticker.stream), c),
      );
      await settle(tester);
      final before = k.callsTo('proxy_log_list_filtered').length;
      ticker.fire();
      await settle(tester);
      expect(k.callsTo('proxy_log_list_filtered').length, before + 1);
      expect(find.text(c.t('status.loading')), findsNothing);
    });

    testWidgets('「清空」必须先确认：点按钮只出确认卡，不发命令', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);

      expect(find.byType(ConfirmCard), findsNothing);
      await tester.tap(find.text(c.t('logs.clear')).first);
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);
      expect(k.commands.contains('proxy_log_clear'), isFalse);
    });

    testWidgets('确认之后才真的清空', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('logs.clear')).first);
      await settle(tester);
      // 确认卡里的那颗「清空」（确认卡是最后出现的，取 last）。
      await tester.tap(find.text(c.t('logs.clear')).last);
      await settle(tester);
      expect(k.callsTo('proxy_log_clear').length, 1);
      expect(find.byType(ConfirmCard), findsNothing);
    });

    testWidgets('取消确认 → 不发命令，确认卡收起', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('logs.clear')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('action.cancel')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsNothing);
      expect(k.commands.contains('proxy_log_clear'), isFalse);
    });

    testWidgets('点一行 → 取详情并展开详情面板', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text('am').first);
      await settle(tester);
      expect(k.lastCallTo('proxy_log_get')!.args!['id'], 'a1');
      expect(find.text(c.t('logs.detail')), findsOneWidget);
    });

    testWidgets('复制整条 → 走注入的剪贴板函数，内容是 markdown', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final written = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          LogsPage(invoke: k.fn, copyText: (s) async => written.add(s)),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byIcon(Icons.copy_outlined).first);
      await settle(tester);
      expect(written.single, startsWith('# Proxy Log a1'));
    });

    testWidgets('第一页时「上一页」是禁用的', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      final prev = tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, c.t('action.prev')),
      );
      expect(prev.enabled, isFalse);
    });

    testWidgets('has_more=false 时「下一页」也是禁用的', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      final next = tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, c.t('action.next')),
      );
      expect(next.enabled, isFalse);
    });

    testWidgets('列表查询失败也不白屏（保持空态，不抛到 framework）', (tester) async {
      await useBigSurface(tester);
      final k = logsFake();
      k.errors['proxy_log_list_filtered'] = StateError('boom');
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
      await settle(tester);
      expect(tester.takeException(), isNull);
      expect(find.byType(CenteredNote), findsOneWidget);
    });
  });

  group('RequestLogPage', () {
    testWidgets('挂载查列表 + 查总数，count 过滤器补了 sources 默认值', (tester) async {
      await useBigSurface(tester);
      final k = reqLogFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(RequestLogPage(invoke: k.fn), c));
      await settle(tester);
      expect(k.callsTo('request_log_list').length, 1);
      final f =
          k.lastCallTo('proxy_log_count_filtered')!.args!['filter']!
              as Map<String, Object?>;
      expect(f['sources'], ['test', 'quota']);
    });

    testWidgets('空态显示「暂无请求记录」', (tester) async {
      await useBigSurface(tester);
      final k = reqLogFake(items: const []);
      k.responses['proxy_log_count_filtered'] = 0;
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(RequestLogPage(invoke: k.fn), c));
      await settle(tester);
      expect(find.text(c.t('requestLog.empty')), findsWidgets);
    });

    testWidgets('刷新按钮再查一轮', (tester) async {
      await useBigSurface(tester);
      final k = reqLogFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(RequestLogPage(invoke: k.fn), c));
      await settle(tester);
      final before = k.callsTo('request_log_list').length;
      await tester.tap(find.text(c.t('action.refresh')));
      await settle(tester);
      expect(k.callsTo('request_log_list').length, before + 1);
    });
  });

  group('GroupsSection', () {
    testWidgets('挂载画出分组卡', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(find.text('G10'), findsOneWidget);
    });

    testWidgets('空分组显示空态', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake(page: const []);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(find.text(c.t('group.empty')), findsOneWidget);
    });

    testWidgets('新建分组：名字为空时「创建」是禁用的', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      // 建组入口在**平台页页头**那颗「+ 添加分组」上（分组区自己没有第二颗，
      // React 同样只有一颗）。单测只挂分组区，所以走 `onCreateGroupReady`
      // 把入口取出来调用 —— 与真页面同一条路。
      VoidCallback? openCreate;
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            onCreateGroupReady: (fn) => openCreate = fn,
          ),
          c,
        ),
      );
      await settle(tester);
      openCreate!();
      await settle(tester);

      final create = tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, c.t('action.create')),
      );
      expect(create.enabled, isFalse);
      expect(k.commands.contains('group_create'), isFalse);
    });

    testWidgets('填了名字 → 「创建」可点，点了才发命令', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      // 建组入口在**平台页页头**那颗「+ 添加分组」上（分组区自己没有第二颗，
      // React 同样只有一颗）。单测只挂分组区，所以走 `onCreateGroupReady`
      // 把入口取出来调用 —— 与真页面同一条路。
      VoidCallback? openCreate;
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            onCreateGroupReady: (fn) => openCreate = fn,
          ),
          c,
        ),
      );
      await settle(tester);
      openCreate!();
      await settle(tester);

      await tester.enterText(find.byType(TextField).first, '我的组');
      await settle(tester);
      final create = tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, c.t('action.create')),
      );
      expect(create.enabled, isTrue);

      await tester.tap(find.text(c.t('action.create')));
      await settle(tester);
      final input =
          k.lastCallTo('group_create')!.args!['input']! as Map<String, Object?>;
      expect(input['name'], '我的组');
    });

    testWidgets('分组密钥输入框实时滤掉非法字符', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      // 建组入口在**平台页页头**那颗「+ 添加分组」上（分组区自己没有第二颗，
      // React 同样只有一颗）。单测只挂分组区，所以走 `onCreateGroupReady`
      // 把入口取出来调用 —— 与真页面同一条路。
      VoidCallback? openCreate;
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            onCreateGroupReady: (fn) => openCreate = fn,
          ),
          c,
        ),
      );
      await settle(tester);
      openCreate!();
      await settle(tester);

      await tester.enterText(find.byType(TextField).at(0), 'n');
      await tester.enterText(find.byType(TextField).at(1), 'my key/1');
      await settle(tester);
      await tester.tap(find.text(c.t('action.create')));
      await settle(tester);
      final input =
          k.lastCallTo('group_create')!.args!['input']! as Map<String, Object?>;
      expect(input['group_key'], 'mykey1');
    });

    testWidgets('删组必须先确认', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('action.delete')).first);
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);
      expect(k.commands.contains('group_delete'), isFalse);

      await tester.tap(find.text(c.t('action.delete')).last);
      await settle(tester);
      expect(k.lastCallTo('group_delete')!.args!['id'], 10);
    });

    testWidgets('编辑分组：清空名字后「保存」变禁用', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      var save = tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, c.t('action.save')),
      );
      expect(save.enabled, isTrue);

      await tester.enterText(find.byType(TextField).first, '');
      await settle(tester);
      save = tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, c.t('action.save')),
      );
      expect(save.enabled, isFalse);
      expect(k.commands.contains('group_update'), isFalse);
    });

    testWidgets('保存编辑 → 先 group_update 再 group_set_platforms', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('action.save')));
      await settle(tester);

      final order = k.calls.map((x) => x.cmd).toList();
      expect(
        order.indexOf('group_set_platforms'),
        greaterThan(order.indexOf('group_update')),
      );
    });

    testWidgets('移除组内平台 → 弹窗先实时拉后端算跨组归属', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      final before = k.callsTo('group_detail_list').length;
      await tester.tap(find.byTooltip(c.t('group.deletePlatformTitle')).first);
      await settle(tester);
      expect(k.callsTo('group_detail_list').length, before + 1);
      expect(k.commands.contains('platform_delete'), isFalse);
    });

    testWidgets('只属本组 → 弹窗里没有「仅移出本组」这个选项', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('group.deletePlatformTitle')).first);
      await settle(tester);
      expect(find.text(c.t('group.removeFromGroupAction')), findsNothing);
    });

    testWidgets('属多个组 → 弹窗给出「仅移出本组」，点它走 group_set_platforms', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['group_detail_list'] = [
        {
          'group': {'id': 10, 'name': 'G10'},
          'platforms': [
            {'platform': plat(1, 'P1')},
          ],
        },
        {
          'group': {'id': 11, 'name': 'G11'},
          'platforms': [
            {'platform': plat(1, 'P1')},
          ],
        },
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('group.deletePlatformTitle')).first);
      await settle(tester);
      expect(find.text(c.t('group.removeFromGroupAction')), findsOneWidget);

      await tester.tap(find.text(c.t('group.removeFromGroupAction')));
      await settle(tester);
      expect(k.commands.contains('platform_delete'), isFalse);
      expect(k.callsTo('group_set_platforms').length, 1);
    });

    testWidgets('一键测试本组 → 出结果面板，每行有终态', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('group.testAll')).first);
      await settle(tester);
      expect(k.callsTo('model_test').length, 1);
      // 行状态是本地化文案 + 耗时（`GroupTestPanel.tsx:50` 的
      // `t("group.testAllOk") + " {ms}ms"`），不是裸的 `ok`；
      // 摘要行同样含「成功」，所以这里按「成功 + 空格 + 毫秒」认那一行。
      expect(
        find.textContaining(RegExp('${c.t('group.testAllOk')} \\d+ms')),
        findsOneWidget,
      );
      // 面板抬头：标题带组名。
      expect(find.textContaining(c.t('group.testAllTitle')), findsOneWidget);
    });

    testWidgets('折叠分组 → 组内平台行消失，并落盘折叠态', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(find.text('P1'), findsWidgets);

      await tester.tap(find.byIcon(Icons.expand_more).first);
      await settle(tester);
      expect(k.lastCallTo('set_ui_extra')!.args!['key'], '_ui_collapsed');
      expect(k.lastCallTo('set_ui_extra')!.args!['value'], true);
    });

    // ── 票 14 第二梯队：分组卡的控件族 ──

    testWidgets('点组名整块也能展开收起，不必非点那颗小箭头', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(find.text('P1'), findsWidgets);
      await tester.tap(find.text('G10'));
      await settle(tester);
      expect(find.text('P1'), findsNothing);
      expect(k.lastCallTo('set_ui_extra')!.args!['value'], true);
    });

    testWidgets('多选强制展开：折叠着进多选，组内平台照样看得到', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byIcon(Icons.expand_more).first);
      await settle(tester);
      expect(find.text('P1'), findsNothing);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      expect(find.text('P1'), findsWidgets, reason: '要选的平台得看得见');
      // 多选期间不许再折叠。
      await tester.tap(find.byIcon(Icons.expand_more).first);
      await settle(tester);
      expect(find.text('P1'), findsWidgets);
    });

    testWidgets('一键测试：空组点不动，有测试在跑时也点不动', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake(
        page: [
          {
            'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
            'platforms': <Object?>[],
            'model_mappings': <Object?>[],
          },
          {
            'group': {'id': 11, 'name': 'G11', 'group_key': 'gk11'},
            'platforms': [
              {'platform': plat(1, 'P1')},
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      final gate = Completer<Map<String, Object?>>();
      k.responses['model_test'] = () => gate.future;
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      // 一键测试现在是图标按钮，可点与否看它里面那层 InkWell 的 onTap。
      List<InkWell> testButtons() => tester
          .widgetList<InkWell>(
            find.descendant(
              of: find.byTooltip(c.t('group.testAll')),
              matching: find.byType(InkWell),
            ),
          )
          .toList();
      expect(testButtons().first.onTap, isNull, reason: '空组点不动');
      expect(testButtons()[1].onTap, isNotNull);

      // 有平台的那组起一轮测试，两颗按钮都该禁掉。
      await tester.tap(find.byTooltip(c.t('group.testAll')).last);
      await tester.pump();
      expect(
        testButtons().every((b) => b.onTap == null),
        isTrue,
        reason: '一轮在跑时不该再并发触发第二轮',
      );
      gate.complete({'success': true, 'duration_ms': 3, 'error': ''});
      await settle(tester);
    });

    testWidgets('清理失效：拉取中先开弹窗，执行中按钮换文案且关不掉', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final preview = Completer<List<Object?>>();
      k.responses['platform_purge_disabled_preview'] = () => preview.future;
      final run = Completer<Map<String, Object?>>();
      k.responses['platform_purge_disabled'] = () => run.future;
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('group.purgeDisabled')).first);
      await tester.pump();
      expect(find.text(c.t('status.loading')), findsOneWidget);

      preview.complete([
        {'id': 2, 'name': '坏平台', 'reason': 'expired', 'action': 'delete'},
      ]);
      await settle(tester);
      expect(find.text('坏平台'), findsOneWidget);

      await tester.tap(find.text(c.t('action.confirm')));
      await tester.pump();
      expect(find.text('坏平台'), findsOneWidget, reason: '执行中弹窗不关');
      expect(find.text(c.t('status.loading')), findsOneWidget);
      await tester.tap(find.text(c.t('action.cancel')));
      await tester.pump();
      expect(find.text('坏平台'), findsOneWidget, reason: '执行中取消也点不动');

      run.complete({
        'deletedIds': <Object?>[2],
        'unassignedIds': <Object?>[],
      });
      await settle(tester);
      expect(find.text('坏平台'), findsNothing);
    });

    testWidgets('默认分组徽标是实心的，不是一行裸字', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake(
        page: [
          {
            'group': {
              'id': 10,
              'name': 'G10',
              'group_key': 'gk10',
              'is_default': true,
            },
            'platforms': <Object?>[],
            'model_mappings': <Object?>[],
          },
        ],
      );
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      final badge = tester.widgetList<MiniBadge>(
        find.byWidgetPredicate(
          (w) => w is MiniBadge && w.text == c.t('group.isDefault'),
        ),
      );
      expect(badge, hasLength(1));
      expect(badge.single.solid, isTrue);
    });

    testWidgets('页头显示分组计数', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(find.text('1 ${c.t('nav.groups')}'), findsOneWidget);
    });

    testWidgets('组内拖拽：拖到另一行上方 → 画插入线 → 松手发新顺序', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake(
        page: [
          {
            'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
            'platforms': [
              {'platform': plat(1, 'P1')},
              {'platform': plat(2, 'P2')},
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      // 上下移按钮已随拖拽删除（React 没有）。
      expect(find.byIcon(Icons.arrow_upward), findsNothing);
      expect(find.byIcon(Icons.arrow_downward), findsNothing);
      expect(find.byIcon(Icons.drive_file_move_outline), findsNothing);

      // 按住第二行的把手，拖到第一行的上半部分。
      final handles = find.byTooltip(c.t('group.dragPlatform'));
      expect(handles, findsNWidgets(2));
      final firstRowTop = tester.getTopLeft(find.text('P1')).dy;
      final gesture = await tester.startGesture(tester.getCenter(handles.last));
      await tester.pump(const Duration(milliseconds: 150));
      await gesture.moveTo(
        Offset(tester.getCenter(find.text('P1')).dx, firstRowTop + 4),
      );
      await tester.pump();
      // 指针停在第一行上半 → 插入线画在它上面。
      expect(
        find.byWidgetPredicate(_isDropLine),
        findsOneWidget,
        reason: '指针停在第一行上半 → 线画在它上面',
      );

      await gesture.up();
      await settle(tester);
      expect(k.lastCallTo('group_platform_reorder')!.args!['orderedIds'], [
        2,
        1,
      ]);
      expect(
        find.byWidgetPredicate(_isDropLine),
        findsNothing,
        reason: '松手后线要收掉',
      );
    });

    testWidgets('滚到底自动拉下一页，没有「加载更多」按钮', (tester) async {
      // 一屏放不下才有得滚：给一块矮画布 + 满一页（12 条）的数据。
      await tester.binding.setSurfaceSize(const Size(1400, 700));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      // 每页一批**不同** id。同一个 id 在列表里出现两次会让子项 key 撞车，
      // 触发 `!childSemantics.renderObject._needsLayout`；真后端 id 是主键，不会重复。
      var pageNo = 0;
      final k = groupsFake(page: const []);
      k.responses['group_detail_list_paged'] = () {
        final base = 100 + pageNo++ * 12;
        return [
          for (var i = 0; i < 12; i++)
            {
              'group': {
                'id': base + i,
                'name': 'G${base + i}',
                'group_key': 'gk${base + i}',
              },
              'platforms': <Object?>[],
              'model_mappings': <Object?>[],
            },
        ];
      };
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(k.callsTo('group_detail_list_paged').length, 1, reason: '首帧不乱拉');
      // 对齐 React：那边只有哨兵，没有按钮（`GroupListView.tsx:277-285`）。
      expect(find.text(c.t('logs.hasMore')), findsNothing);

      await tester.drag(find.text('G100').first, const Offset(0, -2000));
      await settle(tester);
      expect(
        k.callsTo('group_detail_list_paged').length,
        greaterThan(1),
        reason: '滚到底就该自己接着拉',
      );
    });

    testWidgets('内容不满一屏：像 React 哨兵一样接着拉到没有下一页', (tester) async {
      // React 的 IntersectionObserver 首次 observe 就回调：哨兵可见就拉下一页，
      // 一页页拉到填满视口或没有下一页为止。按钮删掉之后这条路是唯一兜底。
      await useBigSurface(tester);
      var pageNo = 0;
      final k = groupsFake(page: const []);
      k.responses['group_detail_list_paged'] = () {
        pageNo++;
        // 第一页满 12 条（hasMore 仍真），第二页 2 条（不满一页 → hasMore 置假）。
        final n = pageNo == 1 ? 12 : 2;
        final base = 100 + (pageNo - 1) * 12;
        return [
          for (var i = 0; i < n; i++)
            {
              'group': {
                'id': base + i,
                'name': 'G${base + i}',
                'group_key': 'gk${base + i}',
              },
              'platforms': <Object?>[],
              'model_mappings': <Object?>[],
            },
        ];
      };
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(
        k.callsTo('group_detail_list_paged').length,
        2,
        reason: '没滚也要把第二页拉齐 —— 哨兵本来就可见',
      );
      expect(find.text('G113'), findsOneWidget, reason: '第二页的内容要真的上屏');
    });

    testWidgets('复制命令菜单四项各带图标', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('group.copyKeyLabel')));
      await settle(tester);
      // 密钥那项是钥匙图标，另外三项是平台 svg。
      expect(find.byIcon(Icons.key), findsOneWidget);
      // 只数菜单里的三颗（卡片上的组图标也是 svg，不算在内）。
      expect(
        find.descendant(
          of: find.byType(PopupMenuItem<String>),
          matching: find.byType(SvgPicture),
        ),
        findsNWidgets(3),
      );
    });

    testWidgets('编辑页：密钥旁有复制按钮，点完给「已复制」反馈', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final copied = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            copyText: (v) async => copied.add(v),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      // 页头四颗 + 密钥行一颗。
      expect(find.byTooltip(c.t('group.copyApiKeyTitle')), findsNWidgets(2));
      await tester.tap(find.text(c.t('action.copy')));
      await tester.pump();
      expect(copied.single, 'gk10');
      expect(find.text(c.t('logs.copied')), findsOneWidget, reason: '点完要有反馈');
    });

    testWidgets('编辑页：最大重试封顶 10，填 99 也只记 10', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);
      // 字段现在是「标签左、输入右」的两列，所以从标签往上找那一行。
      final field = find.ancestor(
        of: find.text(c.t('group.maxRetries')),
        matching: find.byType(Row),
      );
      await tester.enterText(
        find.descendant(of: field.first, matching: find.byType(TextField)),
        '99',
      );
      await settle(tester);
      await tester.tap(find.text(c.t('action.save')));
      await settle(tester);
      final input =
          k.lastCallTo('group_update')!.args!['input']! as Map<String, Object?>;
      expect(input['max_retries'], 10);
    });

    testWidgets('编辑页：保留字那一行自己高亮，不只靠底下一句红字', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);
      await tester.tap(find.text('+ ${c.t('group.addEnvVar')}'));
      await settle(tester);
      final keyField = find
          .byWidgetPredicate(
            (w) =>
                w is TextField &&
                w.decoration?.hintText == c.t('group.envVarKey'),
          )
          .last;
      await tester.enterText(keyField, 'ANTHROPIC_BASE_URL');
      await settle(tester);
      expect(find.text(c.t('group.envVarReservedHint')), findsOneWidget);
      // 那一行的描边换了颜色（与普通行不是同一个 Border）。
      final borders = tester
          .widgetList<Container>(find.byType(Container))
          .where((w) {
            final d = w.decoration;
            return d is BoxDecoration && d.border != null;
          })
          .map(
            (w) =>
                ((w.decoration! as BoxDecoration).border! as Border).top.color,
          )
          .toSet();
      expect(borders.length, greaterThan(1), reason: '保留字行与普通行描边不同色');
    });

    testWidgets('操作失败 → 通过 onToast 把错误报出去（不静默吞）', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.errors['group_delete'] = StateError('boom');
      final toasts = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            onToast: (t, {required ok}) => toasts.add('$ok|$t'),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.delete')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('action.delete')).last);
      await settle(tester);
      expect(toasts.single, startsWith('false|'));
    });

    // ── 界面对齐票 04 补的那批（批量操作 / 复制命令 / 优先级 / 映射 / 环境变量）──

    testWidgets('多选：进入后出工具栏，选中前四个批量按钮都点不动', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      expect(find.text(c.t('group.selectAll')), findsOneWidget);
      for (final key in const [
        'group.batchDelete',
        'group.batchOverrideModels',
        'group.batchSetStatus',
        'group.batchMoveGroup',
      ]) {
        final b = tester.widget<SmallButton>(
          find.widgetWithText(SmallButton, c.t(key)).last,
        );
        expect(b.enabled, isFalse, reason: '$key 没选中平台时必须禁用');
      }
    });

    testWidgets('多选：全选 → 批量删除要先确认，确认才发 batch_delete_platforms', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.selectAll')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchDelete')).last);
      await settle(tester);
      expect(find.text(c.t('group.batchDeleteTitle')), findsOneWidget);
      expect(k.commands.contains('batch_delete_platforms'), isFalse);
      // 不可逆操作，确认前必须列出**全部**待删平台（React `BatchDeleteModal.tsx:90-128`）。
      // 原先只列跨组的那几个，删几个、删哪几个在确认前根本看不到。
      expect(find.byKey(const ValueKey('batch-affected-list')), findsOneWidget);
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('batch-affected-list')),
          matching: find.text('P1'),
        ),
        findsOneWidget,
      );

      await tester.tap(
        find.textContaining(c.t('group.batchDeleteConfirm', {'count': '1'})),
      );
      await settle(tester);
      expect(k.lastCallTo('batch_delete_platforms')!.args!['ids'], [1]);
    });

    testWidgets('多选：批量改状态弹窗默认「禁用」，确认发 batch_set_status', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.selectAll')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchSetStatus')).last);
      await settle(tester);
      // 选中平台覆盖了本组全部 enabled 候选 → 要出无候选警告。
      expect(
        find.text(c.t('group.batchSetStatusNoCandidateWarning')),
        findsOneWidget,
      );
      // 待改平台清单 + 各自当前状态徽标（React `BatchSetStatusModal.tsx:110-140`）。
      final list = find.byKey(const ValueKey('batch-affected-list'));
      expect(list, findsOneWidget);
      expect(
        find.descendant(of: list, matching: find.text('P1')),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: list,
          matching: find.text(c.t('platform.statusEnabled')),
        ),
        findsOneWidget,
        reason: 'P1 当前是启用态，改状态前要看得出来',
      );

      await tester.tap(
        find.textContaining(c.t('group.batchSetStatusConfirm', {'count': '1'})),
      );
      await settle(tester);
      expect(k.lastCallTo('batch_set_status')!.args!['status'], 'disabled');
    });

    testWidgets('复制启动命令菜单：四项各复制各自的文本', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final copied = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            copyText: (s) async => copied.add(s),
          ),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.copyKeyLabel')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.menuCopyClaude')));
      await settle(tester);
      expect(copied.single, contains('~/.aidog/settings.gk10.json'));

      await tester.tap(find.byTooltip(c.t('group.copyKeyLabel')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.menuCopyPi')));
      await settle(tester);
      expect(copied.last, "pi --provider 'aidog-gk10'");
    });

    testWidgets('代理地址复制按钮复制的是 proxyBaseUrl', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final copied = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            copyText: (s) async => copied.add(s),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('group.copyBaseUrl')));
      await settle(tester);
      expect(copied.single, 'http://127.0.0.1:9999/proxy');
    });

    testWidgets('组内优先级步进器：加一档 → 发 group_platform_set_level_priority', (
      tester,
    ) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byIcon(Icons.add).first);
      await settle(tester);
      expect(k.lastCallTo('group_platform_set_level_priority')!.args, {
        'groupId': 10,
        'platformId': 1,
        'levelPriority': 6,
      });
    });

    // 回归 2026-09-23：这格原先是只读 `Text`，只能靠加减按钮一档一档点，
    // 1 调到 10 要点九下。React 这里是 `<Input type="number" min=1 max=10>`
    //（`PlatformCard.tsx:944-961`），可以直接敲。
    testWidgets('组内优先级可以直接敲数字，越界夹到 1~10', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      final field = find.byKey(const ValueKey('level-priority-1'));
      expect(field, findsOneWidget, reason: '优先级那一格必须是可输入的');
      await tester.enterText(
        find.descendant(of: field, matching: find.byType(TextField)),
        '9',
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      expect(k.lastCallTo('group_platform_set_level_priority')!.args, {
        'groupId': 10,
        'platformId': 1,
        'levelPriority': 9,
      });

      // 越界不报错，夹到上限。
      await tester.enterText(
        find.descendant(of: field, matching: find.byType(TextField)),
        '99',
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      expect(
        k
            .lastCallTo('group_platform_set_level_priority')!
            .args!['levelPriority'],
        10,
      );
    });

    testWidgets('编辑态：环境变量、pi 线路协议、锁定的分组密钥都在', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      expect(find.text(c.t('group.groupKeyLocked')), findsOneWidget);
      expect(find.text(c.t('group.envVarsHint')), findsOneWidget);
      expect(find.text(c.t('group.piApiHint')), findsOneWidget);
      expect(find.text(c.t('group.timeoutDefault')), findsOneWidget);

      // 切 pi 线路协议 → 立刻写 extra 并重生成 pi 配置。
      // 选项收在下拉弹层里（与 React 的 `<Select>` 一致），直接调它的 onChanged
      // ——测的是接线，与点开菜单再选一个的结果相同。
      final piDropdown = tester
          .widgetList<DropdownButton<String>>(
            find.byType(DropdownButton<String>),
          )
          .firstWhere(
            (d) => d.items!.any((it) => it.value == 'openai-responses'),
          );
      piDropdown.onChanged!('openai-responses');
      await settle(tester);
      expect(k.lastCallTo('set_ui_extra')!.args!['key'], 'pi_api');
      expect(k.commands.contains('sync_group_settings'), isTrue);
    });

    testWidgets('多选：批量覆盖模型 —— 槽位全空点不动，preset 灌进来后可确认', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['get_defaults_json'] =
          '{"protocols":{"openai":{"name":{"en-US":"OpenAI"},'
          '"models":{"default":{"default":"gpt-x","sonnet":"gpt-s"}}}}}';
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.selectAll')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchOverrideModels')).last);
      await settle(tester);

      // 全空 → 提示 + 确认禁用。
      expect(find.text(c.t('group.batchOverrideAllEmptyHint')), findsOneWidget);
      final confirmText = c.t('group.batchOverrideConfirm', {'count': '1'});
      expect(
        tester
            .widget<SmallButton>(find.widgetWithText(SmallButton, confirmText))
            .enabled,
        isFalse,
      );

      // 换到 preset 来源，选协议 → 槽位被灌满 → 可确认。
      await tester.tap(find.text(c.t('group.batchOverrideSourcePreset')));
      await settle(tester);
      // 平台行加了优先级输入框之后整页变高，弹窗里这颗按钮会落到滚动区之外，
      // 直接 tap 会打空 —— 先滚到它。
      await tester.ensureVisible(
        find.text(c.t('group.batchOverridePresetSelect')),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchOverridePresetSelect')));
      await settle(tester);
      await tester.tap(find.text('OpenAI').last);
      await settle(tester);
      await tester.tap(find.text(confirmText));
      await settle(tester);
      final models =
          k.lastCallTo('batch_override_models')!.args!['models']!
              as Map<String, Object?>;
      expect(models['default'], 'gpt-x');
      expect(models['sonnet'], 'gpt-s');
    });

    testWidgets('多选：批量覆盖模型 —— 从别的平台复制五槽', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.selectAll')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchOverrideModels')).last);
      await settle(tester);

      await tester.tap(find.text(c.t('group.batchOverrideSourceCopy')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchOverrideCopySelect')));
      await settle(tester);
      await tester.tap(find.text('P2').last);
      await settle(tester);
      await tester.tap(
        find.text(c.t('group.batchOverrideConfirm', {'count': '1'})),
      );
      await settle(tester);
      final models =
          k.lastCallTo('batch_override_models')!.args!['models']!
              as Map<String, Object?>;
      expect(models['default'], 'm');
    });

    testWidgets('多选：批量覆盖模型 —— 在已有值中间插字符，光标留在插入处', (tester) async {
      // 这条用例守的是「控制器不能造在 build 里」：那样写每敲一个字就换一个新
      // 控制器并把光标按到末尾，于是只能往后追加，改不了中间的字。
      // 所以断言的是**光标位置**，不是「字打进去了」—— 后者两种写法都过得了。
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.selectAll')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchOverrideModels')).last);
      await settle(tester);

      final slot = find.byKey(const ValueKey('batch-override-default'));
      expect(slot, findsOneWidget);
      await tester.enterText(slot, 'abc');
      await settle(tester);

      // 真实控制器实例：KeptTextField 把它收在 State 里，外面只能从
      // EditableText 拿到。拿到了才能在「中间」落光标。
      final editable = find.descendant(
        of: slot,
        matching: find.byType(EditableText),
      );
      final ctrl = tester.widget<EditableText>(editable).controller;
      ctrl.selection = const TextSelection.collapsed(offset: 1);
      await tester.pump();

      // 输入法真正发过来的东西：文本 + 新光标位置。
      tester
          .state<EditableTextState>(editable)
          .updateEditingValue(
            const TextEditingValue(
              text: 'aXbc',
              selection: TextSelection.collapsed(offset: 2),
            ),
          );
      await settle(tester);

      final after = tester.widget<EditableText>(editable).controller;
      expect(after.text, 'aXbc', reason: '插进去的字要留在中间');
      expect(
        after.selection.baseOffset,
        2,
        reason: '光标要停在刚插入的字后面；跳到末尾 = 控制器又被重建了',
      );

      // 值确实进了提交载荷，不只是停在输入框里。
      await tester.tap(
        find.text(c.t('group.batchOverrideConfirm', {'count': '1'})),
      );
      await settle(tester);
      final models =
          k.lastCallTo('batch_override_models')!.args!['models']!
              as Map<String, Object?>;
      expect(models['default'], 'aXbc');
    });

    testWidgets('多选：批量移组 —— 目标=当前组时不让确认，换一个组才发命令', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['group_detail_list_paged'] = [
        {
          'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
          'platforms': [
            {'platform': plat(1, 'P1')},
          ],
          'model_mappings': <Object?>[],
        },
        {
          'group': {'id': 11, 'name': 'G11', 'group_key': 'gk11'},
          'platforms': <Object?>[],
          'model_mappings': <Object?>[],
        },
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.selectAll')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchMoveGroup')).last);
      await settle(tester);

      final confirmText = c.t('group.batchMoveGroupConfirm', {
        'count': '1',
        'mode': c.t('group.batchMoveGroupModeMoveShort'),
      });
      // 还没选目标组 → 点不动。
      expect(
        tester
            .widget<SmallButton>(find.widgetWithText(SmallButton, confirmText))
            .enabled,
        isFalse,
      );

      // 选当前组 → 提示「与当前分组相同」，仍点不动。
      await tester.tap(find.text(c.t('group.batchMoveGroupSelect')));
      await settle(tester);
      await tester.tap(
        find.textContaining(c.t('group.batchMoveGroupCurrent')).last,
      );
      await settle(tester);
      expect(
        find.text(c.t('group.batchMoveGroupSameAsCurrent')),
        findsOneWidget,
      );

      // 换到另一个组 + 切「加入」模式 → 可确认。
      await tester.tap(
        find.textContaining(c.t('group.batchMoveGroupCurrent')).first,
      );
      await settle(tester);
      await tester.tap(find.text('G11').last);
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchMoveGroupModeAdd')));
      await settle(tester);
      await tester.tap(
        find.text(
          c.t('group.batchMoveGroupConfirm', {
            'count': '1',
            'mode': c.t('group.batchMoveGroupModeAddShort'),
          }),
        ),
      );
      await settle(tester);
      expect(k.lastCallTo('batch_move_group')!.args!['targetGroupId'], 11);
      expect(k.lastCallTo('batch_move_group')!.args!['mode'], 'add');
    });

    testWidgets('批量删除：跨组平台要在弹窗里被点名', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['group_detail_list'] = [
        {
          'group': {'id': 10, 'name': 'G10'},
          'platforms': [
            {'platform': plat(1, 'P1')},
          ],
        },
        {
          'group': {'id': 11, 'name': 'G11'},
          'platforms': [
            {'platform': plat(1, 'P1')},
          ],
        },
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.selectAll')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchDelete')).last);
      await settle(tester);
      expect(
        find.text(c.t('group.batchDeleteCrossGroupWarning', {'count': '1'})),
        findsOneWidget,
      );
      expect(find.textContaining('G10、G11'), findsOneWidget);
    });

    testWidgets('未匹配虚拟桶：有请求才画那张只读卡', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['all_group_usage_stats'] = {
        '未匹配': {'total_requests': 7, 'success_count': 7},
      };
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(find.text(c.t('group.unmatched')), findsOneWidget);
      expect(find.text(c.t('group.unmatchedHint')), findsOneWidget);
      expect(find.text('7'), findsOneWidget);
    });

    testWidgets('跨组拖：P1 落到 G11 的行上 → group_platform_move 带真实源组', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['group_detail_list_paged'] = [
        {
          'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
          'platforms': [
            {'platform': plat(1, 'P1')},
          ],
          'model_mappings': <Object?>[],
        },
        {
          'group': {'id': 11, 'name': 'G11', 'group_key': 'gk11'},
          'platforms': [
            {'platform': plat(2, 'P2')},
          ],
          'model_mappings': <Object?>[],
        },
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      // 下拉「移动到分组」已删（React 没有），跨组走拖拽：P1 拖到 G11 组内 P2
      // 那一行上松手。`fromGroupId` 必须是真实源组 10，不是 0（`usePlatformDrag.ts`
      // 的 payload.fromGid 同义）。
      final g11Row = find
          .ancestor(
            of: find.text('P2').first,
            matching: find.byType(DragTarget<int>),
          )
          .first;
      final target = tester.widget<DragTarget<int>>(g11Row);
      expect(
        target.onWillAcceptWithDetails!(
          DragTargetDetails<int>(data: 1, offset: Offset.zero),
        ),
        isTrue,
      );
      target.onAcceptWithDetails!(
        DragTargetDetails<int>(data: 1, offset: Offset.zero),
      );
      await settle(tester);
      expect(k.lastCallTo('group_platform_move')!.args, {
        'platformId': 1,
        'fromGroupId': 10,
        'toGroupId': 11,
      });
    });

    // 组内重排（原「上下移 → group_platform_reorder」用例）：上下移按钮已删，
    // 覆盖由上面的指针拖拽用例承担（拖到第一行上半 → 插入线 → group_platform_reorder
    // orderedIds [2,1] → 松手线收掉）。

    testWidgets('一键测试：失败的行显示「失败」+ 错误文案，摘要按成功/失败计数', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['model_test'] = {
        'success': false,
        'duration_ms': 3,
        'error': 'boom',
      };
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('group.testAll')).first);
      await settle(tester);
      expect(find.text(c.t('group.testAllFail')), findsOneWidget);
      expect(find.text('boom'), findsOneWidget);
      expect(
        find.text(
          c.t('group.testAllSummary', {'ok': '0', 'fail': '1', 'total': '1'}),
        ),
        findsOneWidget,
      );
    });

    testWidgets('新建态：选调度策略 + 用选择器挑平台 → 建完再关联一次', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      // 建组入口在**平台页页头**那颗「+ 添加分组」上（分组区自己没有第二颗，
      // React 同样只有一颗）。单测只挂分组区，所以走 `onCreateGroupReady`
      // 把入口取出来调用 —— 与真页面同一条路。
      VoidCallback? openCreate;
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            onCreateGroupReady: (fn) => openCreate = fn,
          ),
          c,
        ),
      );
      await settle(tester);
      openCreate!();
      await settle(tester);

      await tester.enterText(find.byType(TextField).first, '新组');
      await settle(tester);
      // 路由模式是下拉（与编辑面板一致）：先展开再点选项。
      await tester.tap(find.byKey(const ValueKey('create-routing-mode')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.loadBalance')).last);
      await settle(tester);
      await tester.tap(find.text(c.t('group.addPlatform')));
      await settle(tester);
      await tester.tap(find.text('P1').last);
      await settle(tester);
      await tester.tap(find.text(c.t('action.create')));
      await settle(tester);

      final input =
          k.lastCallTo('group_create')!.args!['input']! as Map<String, Object?>;
      expect(input['routing_mode'], 'load_balance');
      expect(k.lastCallTo('group_set_platforms')!.args!['platforms'], [
        {'platform_id': 1, 'priority': 1, 'weight': 1},
      ]);
    });

    testWidgets('未分组平台拖进分组卡 → 交给父级搬，然后静默重拉', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final dropped = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            onPlatformDropped: (pid, gid) async => dropped.add('$pid→$gid'),
          ),
          c,
        ),
      );
      await settle(tester);

      final before = k.callsTo('group_detail_list_paged').length;
      final target = tester.firstWidget<DragTarget<int>>(
        find.byType(DragTarget<int>),
      );
      // 本组已有 P1，拖它进来应当被拒；P2 不在组里，接受。
      expect(
        target.onWillAcceptWithDetails!(
          DragTargetDetails<int>(data: 1, offset: Offset.zero),
        ),
        isFalse,
      );
      expect(
        target.onWillAcceptWithDetails!(
          DragTargetDetails<int>(data: 2, offset: Offset.zero),
        ),
        isTrue,
      );
      target.onAcceptWithDetails!(
        DragTargetDetails<int>(data: 2, offset: Offset.zero),
      );
      await settle(tester);
      expect(dropped.single, '2→10');
      expect(
        k.callsTo('group_detail_list_paged').length,
        greaterThan(before),
        reason: '父级搬完之后分组区要自己静默重拉',
      );
    });

    testWidgets('编辑态：四个复制按钮各复制各自的文本（含 env 前置 export）', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['group_detail_list_paged'] = [
        {
          'group': {
            'id': 10,
            'name': 'G10',
            'group_key': 'gk10',
            'env_vars': [
              {'key': 'MY_VAR', 'value': 'v'},
            ],
          },
          'platforms': <Object?>[],
          'model_mappings': <Object?>[],
        },
      ];
      final copied = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            copyText: (s) async => copied.add(s),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      await tester.tap(find.text(c.t('group.apiKey')));
      await settle(tester);
      expect(copied.last, 'gk10');

      await tester.tap(find.text('Claude'));
      await settle(tester);
      expect(copied.last, contains('settings.gk10.json'));

      await tester.tap(find.text('Codex'));
      await settle(tester);
      expect(copied.last, startsWith("export MY_VAR='v';"));
      expect(copied.last, contains("AIDOG_KEY='gk10'"));

      await tester.tap(find.text('pi'));
      await settle(tester);
      expect(copied.last, endsWith("pi --provider 'aidog-gk10'"));
    });

    testWidgets('单组平台：确认删除 → platform_delete，并回传给父级局部移除', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final removed = <List<int>>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            onPlatformsDeleted: removed.add,
          ),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.deletePlatformTitle')).first);
      await settle(tester);
      // 「删除平台」这串文案同时是行内按钮、弹窗标题和确认按钮，取最后一个（确认）。
      await tester.tap(find.text(c.t('group.deletePlatformAction')).last);
      await settle(tester);
      expect(k.lastCallTo('platform_delete')!.args!['id'], 1);
      expect(removed.single, [1]);
    });

    testWidgets('默认分组：徽标 + 「默认配置已写入」按钮，再点一次取消默认', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['group_detail_list_paged'] = [
        {
          'group': {
            'id': 10,
            'name': 'G10',
            'group_key': 'gk10',
            'is_default': true,
          },
          'platforms': [
            {'platform': plat(1, 'P1')},
          ],
          'model_mappings': <Object?>[],
        },
      ];
      // 统计与余额两条分支也在这张卡上画。
      k.responses['all_group_usage_stats'] = {
        'gk10': {'total_requests': 12, 'success_count': 12},
      };
      k.responses['platform_list'] = [
        plat(1, 'P1')..['est_balance_remaining'] = 3.5,
        plat(2, 'P2'),
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      expect(find.text(c.t('group.isDefault')), findsOneWidget);
      // 聚合统计是三个带标签的 chip（React `GroupListItem.tsx:323-328`），
      // 不是一个没标签的请求总数裸数字 —— 裸数字读不出它是请求数还是 token 数。
      expect(find.text('tokens'), findsOneWidget);
      expect(find.text('cost'), findsOneWidget);
      expect(find.text('ok'), findsOneWidget);
      expect(find.text(formatPercent(100, 0)), findsOneWidget);
      // 🔴 group_key 就是这个分组的 API Key，列表上不许出现明文。
      expect(find.textContaining('gk10'), findsNothing);
      await tester.tap(find.byTooltip(c.t('group.unsetDefault')));
      await settle(tester);
      // 已是默认 → 再点一次传 null（取消默认）。
      expect(k.lastCallTo('group_set_default')!.args!['id'], isNull);
    });

    testWidgets('清理失效：没有候选时确认按钮点不动', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.purgeDisabled')).first);
      await settle(tester);
      expect(find.text(c.t('platform.purgeDisabledNone')), findsOneWidget);
      expect(
        tester
            .widget<SmallButton>(
              find.widgetWithText(SmallButton, c.t('action.confirm')),
            )
            .enabled,
        isFalse,
      );
      await tester.tap(find.text(c.t('action.cancel')).last);
      await settle(tester);
      expect(find.byType(ConfirmCard), findsNothing);
    });

    testWidgets('多选：取消按钮退出多选，工具栏消失', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.batchOps')).first);
      await settle(tester);
      expect(find.text(c.t('group.selectAll')), findsOneWidget);
      await tester.tap(find.text(c.t('action.cancel')).first);
      await settle(tester);
      expect(find.text(c.t('group.selectAll')), findsNothing);
    });

    testWidgets('拖分组卡的把手 → group_reorder 按新顺序发整串 id', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['group_detail_list_paged'] = [
        {
          'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
          'platforms': <Object?>[],
          'model_mappings': <Object?>[],
        },
        {
          'group': {'id': 11, 'name': 'G11', 'group_key': 'gk11'},
          'platforms': <Object?>[],
          'model_mappings': <Object?>[],
        },
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);

      // 拖到两张卡中心的正中间。不写死像素距离：卡高随按钮形态变化（图标化后
      // 更矮），固定距离会漂——落点太近没过换位线、太远越过末张卡又弹回原位
      //（2026-09-23 已两次因卡片变矮改数字：160→120→这次）。两张卡的中点
      // 永远在换位区内，怎么变版式都不漂。
      final from = tester.getCenter(find.byIcon(Icons.drag_handle).first);
      final mid =
          (tester.getRect(find.byKey(const ValueKey(10))).center.dy +
              tester.getRect(find.byKey(const ValueKey(11))).center.dy) /
          2;
      final gesture = await tester.startGesture(from);
      await tester.pump(const Duration(milliseconds: 200));
      await gesture.moveBy(Offset(0, mid - from.dy));
      await tester.pump(const Duration(milliseconds: 200));
      await gesture.up();
      await settle(tester);
      expect(k.lastCallTo('group_reorder')!.args!['orderedIds'], [11, 10]);
    });

    // 回归 2026-09-22：确认卡正文只有一个数量，而 `purgeTarget.candidates` 里
    // 名字和 action 都在。不可逆删除之前看不到删的是谁（`GroupListItem.tsx:566-598`）。
    testWidgets('清理失效确认卡：按「删除 / 移出」分两段列出平台名与失效原因', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['platform_purge_disabled_preview'] = [
        {'id': 1, 'name': 'P1', 'reason': 'auth_failed', 'action': 'delete'},
        {'id': 2, 'name': 'P2', 'reason': 'expired', 'action': 'unassign'},
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('group.purgeDisabled')).first);
      await settle(tester);
      expect(
        find.text(c.t('platform.purgeDisabledActionDelete')),
        findsOneWidget,
      );
      expect(
        find.text(c.t('platform.purgeDisabledActionUnassign')),
        findsOneWidget,
      );
      expect(find.text('P1'), findsWidgets);
      expect(find.text('P2'), findsWidgets);
      expect(
        find.text(c.t('platform.purgeDisabledReasonAuthFailed')),
        findsOneWidget,
      );
      expect(
        find.text(c.t('platform.purgeDisabledReasonExpired')),
        findsOneWidget,
      );
    });

    // 自动建组：组名旁要有 auto 徽标，且只要还有平台就不给删除按钮
    //（`GroupListItem.tsx:210-212,309`）—— 删了下次还会自动建出来。
    testWidgets('自动建组：auto 徽标在，删除按钮不渲染', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake(
        page: [
          {
            'group': {
              'id': 10,
              'name': 'G10',
              'group_key': 'gk10',
              'auto_from_platform': '1',
            },
            'platforms': [
              {'platform': plat(1, 'P1')},
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(find.text('auto'), findsOneWidget);
      expect(find.byTooltip(c.t('action.delete')), findsNothing);
    });

    testWidgets('手建组：没有 auto 徽标，删除按钮照常在', (tester) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: groupsFake().fn,
            buildPlatformCard: stubPlatformCard,
          ),
          c,
        ),
      );
      await settle(tester);
      expect(find.text('auto'), findsNothing);
      expect(find.byTooltip(c.t('action.delete')), findsWidgets);
    });

    testWidgets('分组图标：单平台组挂 GroupIcon', (tester) async {
      await useBigSurface(tester);
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: groupsFake().fn,
            buildPlatformCard: stubPlatformCard,
          ),
          c,
        ),
      );
      await settle(tester);
      expect(find.byType(GroupIcon), findsOneWidget);
    });

    testWidgets('分组图标：多平台组不跟 logo，画组名前三个字', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake(
        page: [
          {
            'group': {'id': 10, 'name': '生产分组', 'group_key': 'gk10'},
            'platforms': [
              {'platform': plat(1, 'P1')},
              {'platform': plat(2, 'P2')},
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      expect(find.text('生产分'), findsOneWidget);
    });

    // 用户 2026-09-23：自动建的组与手建的组要看得出区别。原先两档只差一层很淡的
    // 底色（深色强调色改近黑之后 1.05:1 / 1.15:1，肉眼分不出），现在靠一圈边区分。
    testWidgets('分组图标：手建组用 accentEdge 的亮边，自动建的组用普通 line', (tester) async {
      Future<Color> edgeOf(WidgetTester tester, {required bool auto}) async {
        await useBigSurface(tester);
        final k = groupsFake(
          page: [
            {
              'group': {
                'id': 10,
                'name': '生产分组',
                'group_key': 'gk10',
                if (auto) 'auto_from_platform': 'openai',
              },
              'platforms': [
                {'platform': plat(1, 'P1')},
                {'platform': plat(2, 'P2')},
              ],
              'model_mappings': <Object?>[],
            },
          ],
        );
        final c = await makeI18n(tester);
        await tester.pumpWidget(
          wrapPage(
            GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
            c,
          ),
        );
        await settle(tester);
        final box = tester.widget<Container>(
          find
              .ancestor(of: find.text('生产分'), matching: find.byType(Container))
              .first,
        );
        return ((box.decoration! as BoxDecoration).border! as Border).top.color;
      }

      expect(await edgeOf(tester, auto: false), AidogColors.dark.accentEdge);
    });

    testWidgets('分组图标：自动建的组那一圈是普通 line', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake(
        page: [
          {
            'group': {
              'id': 10,
              'name': '生产分组',
              'group_key': 'gk10',
              'auto_from_platform': 'openai',
            },
            'platforms': [
              {'platform': plat(1, 'P1')},
              {'platform': plat(2, 'P2')},
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      final box = tester.widget<Container>(
        find
            .ancestor(of: find.text('生产分'), matching: find.byType(Container))
            .first,
      );
      expect(
        ((box.decoration! as BoxDecoration).border! as Border).top.color,
        AidogColors.dark.line,
      );
    });

    testWidgets('分组卡的「清理失效」：先预览再确认，命令带上本组 id', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['platform_purge_disabled_preview'] = [
        {'id': 1, 'name': 'P1', 'reason': 'auth_failed', 'action': 'delete'},
      ];
      k.responses['platform_purge_disabled'] = {
        'deletedIds': [1],
        'unassignedIds': <Object?>[],
      };
      final toasts = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            onToast: (t, {required ok}) => toasts.add(t),
          ),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(c.t('group.purgeDisabled')).first);
      await settle(tester);
      expect(
        k.lastCallTo('platform_purge_disabled_preview')!.args!['groupId'],
        10,
      );
      expect(
        find.text(c.t('group.purgeDisabledConfirm', {'count': '1'})),
        findsOneWidget,
      );
      expect(k.commands.contains('platform_purge_disabled'), isFalse);

      await tester.tap(find.text(c.t('action.confirm')));
      await settle(tester);
      expect(k.lastCallTo('platform_purge_disabled')!.args!['groupId'], 10);
      expect(
        toasts.single,
        c.t('group.purgeDisabledDone', {'deleted': '1', 'unassigned': '0'}),
      );
    });

    testWidgets('「在此分组添加平台」「查看统计」把回调带上分组信息', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final created = <String>[];
      final navs = <String>[];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(
            invoke: k.fn,
            buildPlatformCard: stubPlatformCard,
            onCreatePlatform: ({List<int>? presetGroupIds, int? lockGid}) =>
                created.add('$presetGroupIds|$lockGid'),
            onNavigate: (id, {String? groupKey}) =>
                navs.add(groupKey == null ? id : '$id:$groupKey'),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('group.addPlatformToGroup')));
      await settle(tester);
      expect(created.single, '[10]|10');
      await tester.tap(find.byTooltip(c.t('group.viewStats')));
      await settle(tester);
      // 必须带上 group_key：统计页靠它预筛该分组（React `GroupListItem.tsx:236`
      // → `Stats.tsx:255`）。只传 'stats' 的话跳过去是空筛选，按钮价值减半。
      expect(navs.single, 'stats:gk10');
    });

    testWidgets('编辑态：关联平台选择器可加、可删，保存时按顺序发优先级', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      // 下拉里只剩没被选中的 P2；选它 → 排在 P1 后面。
      await tester.tap(find.text(c.t('group.addPlatform')));
      await settle(tester);
      await tester.tap(find.text('P2').last);
      await settle(tester);
      await tester.tap(find.text(c.t('action.save')));
      await settle(tester);
      expect(k.lastCallTo('group_set_platforms')!.args!['platforms'], [
        {'platform_id': 1, 'priority': 1, 'weight': 1},
        {'platform_id': 2, 'priority': 2, 'weight': 1},
      ]);
    });

    testWidgets('编辑态：关联平台上下移按钮改顺序，第一行不能上移', (tester) async {
      // 拖拽之外的第二条路：拖不稳 / 用键盘的人靠它改优先级。
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['group_detail_list_paged'] = [
        {
          'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
          'platforms': [
            {'platform': plat(1, 'P1')},
            {'platform': plat(2, 'P2')},
          ],
          'model_mappings': <Object?>[],
        },
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      // 第一行没得再往上，最后一行没得再往下。
      VoidCallback? pressedOf(String key) => tester
          .widget<IconButton>(
            find.descendant(
              of: find.byKey(ValueKey(key)),
              matching: find.byType(IconButton),
            ),
          )
          .onPressed;
      expect(pressedOf('picker-up-1'), isNull);
      expect(pressedOf('picker-down-2'), isNull);

      // 协议双字母徽标：同名不同协议的平台在这张列表里要分得出来。
      expect(find.text('OP'), findsNWidgets(2));

      // 第二行上移 → P2 排到 P1 前面。
      await tester.tap(find.byKey(const ValueKey('picker-up-2')));
      await settle(tester);
      await tester.tap(find.text(c.t('action.save')));
      await settle(tester);
      expect(k.lastCallTo('group_set_platforms')!.args!['platforms'], [
        {'platform_id': 2, 'priority': 1, 'weight': 1},
        {'platform_id': 1, 'priority': 2, 'weight': 1},
      ]);
    });

    testWidgets('编辑态：关联平台可拖拽重排，顺序即优先级', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      k.responses['group_detail_list_paged'] = [
        {
          'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
          'platforms': [
            {'platform': plat(1, 'P1')},
            {'platform': plat(2, 'P2')},
          ],
          'model_mappings': <Object?>[],
        },
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      // 选择器里第一行（P1）往下拖过第二行：分几步挪，让 reorderable 跟得上。
      final handles = find.byIcon(Icons.drag_handle);
      final start = tester.getCenter(handles.first);
      final target = tester.getCenter(handles.at(1));
      final gesture = await tester.startGesture(start);
      await tester.pump(const Duration(milliseconds: 200));
      final step = (target.dy - start.dy) / 4 + 4;
      for (var i = 0; i < 5; i++) {
        await gesture.moveBy(Offset(0, step));
        await tester.pump(const Duration(milliseconds: 60));
      }
      await gesture.up();
      await settle(tester);

      await tester.tap(find.text(c.t('action.save')));
      await settle(tester);
      expect(k.lastCallTo('group_set_platforms')!.args!['platforms'], [
        {'platform_id': 2, 'priority': 1, 'weight': 1},
        {'platform_id': 1, 'priority': 2, 'weight': 1},
      ]);
    });

    testWidgets('编辑态：从选择器里移除一个平台 → 保存时不再带它', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      await tester.tap(find.byIcon(Icons.close).first);
      await settle(tester);
      await tester.tap(find.text(c.t('action.save')));
      await settle(tester);
      expect(k.lastCallTo('group_set_platforms')!.args!['platforms'], isEmpty);
    });

    testWidgets('编辑态：加一条模型映射 → 保存时带上它', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      await tester.tap(find.text('+ ${c.t('mapping.add')}'));
      await settle(tester);
      await tester.enterText(
        find.widgetWithText(TextField, c.t('mapping.source')),
        'src',
      );
      await settle(tester);
      await tester.enterText(
        find.widgetWithText(TextField, c.t('mapping.target')),
        'dst',
      );
      await settle(tester);
      await tester.tap(find.text(c.t('action.save')));
      await settle(tester);
      final input =
          k.lastCallTo('group_update')!.args!['input']! as Map<String, Object?>;
      final maps = input['model_mappings']! as List;
      expect((maps.single as Map)['source_model'], 'src');
      expect((maps.single as Map)['target_model'], 'dst');
    });

    testWidgets('编辑态：加一条环境变量 → 保存时带上它', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          GroupsSection(invoke: k.fn, buildPlatformCard: stubPlatformCard),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.edit')).first);
      await settle(tester);

      await tester.tap(find.text('+ ${c.t('group.addEnvVar')}'));
      await settle(tester);
      await tester.enterText(
        find.widgetWithText(TextField, c.t('group.envVarKey')).last,
        'MY_VAR',
      );
      await settle(tester);
      await tester.tap(find.text(c.t('action.save')));
      await settle(tester);
      final input =
          k.lastCallTo('group_update')!.args!['input']! as Map<String, Object?>;
      final envVars = input['env_vars']! as List;
      expect(envVars.any((e) => (e as Map)['key'] == 'MY_VAR'), isTrue);
    });
  });

  group('PlatformsPage', () {
    testWidgets('挂载画出未分组平台（分组区关掉，单独测这一半）', (tester) async {
      await useBigSurface(tester);
      final k = platformsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(PlatformsPage(invoke: k.fn, showGroups: false), c),
      );
      await settle(tester);
      expect(find.text('P1'), findsOneWidget);
      expect(find.text('P2'), findsOneWidget);
    });

    testWidgets('已分组的平台不出现在未分组区', (tester) async {
      await useBigSurface(tester);
      final k = platformsFake();
      k.responses['group_detail_list'] = [
        {
          'group': {'id': 10, 'name': 'G10'},
          'platforms': [
            {'platform': plat(1, 'P1')},
          ],
        },
      ];
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(PlatformsPage(invoke: k.fn, showGroups: false), c),
      );
      await settle(tester);
      expect(find.text('P1'), findsNothing);
      expect(find.text('P2'), findsOneWidget);
    });

    testWidgets('启停 → platform_update，卡片立刻变（乐观更新）', (tester) async {
      await useBigSurface(tester);
      final k = platformsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(PlatformsPage(invoke: k.fn, showGroups: false), c),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('platform.disable')).first);
      await settle(tester);
      final input =
          k.lastCallTo('platform_update')!.args!['input']!
              as Map<String, Object?>;
      expect(input['status'], 'disabled');
      expect(find.byTooltip(c.t('platform.enable')), findsWidgets);
    });

    testWidgets('快速测试 → model_test，结果进 toast', (tester) async {
      await useBigSurface(tester);
      final k = platformsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(PlatformsPage(invoke: k.fn, showGroups: false), c),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('platform.quickTest')).first);
      await settle(tester);
      expect(k.callsTo('model_test').length, 1);
      expect(find.byType(ToastBar), findsOneWidget);
    });

    testWidgets('删平台必须先确认', (tester) async {
      await useBigSurface(tester);
      final k = platformsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(PlatformsPage(invoke: k.fn, showGroups: false), c),
      );
      await settle(tester);
      await tester.tap(find.byTooltip(c.t('action.delete')).first);
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);
      expect(k.commands.contains('platform_delete'), isFalse);

      await tester.tap(find.text(c.t('action.delete')).last);
      await settle(tester);
      expect(k.callsTo('platform_delete').length, 1);
    });

    testWidgets('清理失效平台：先预览再确认', (tester) async {
      await useBigSurface(tester);
      final k = platformsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(PlatformsPage(invoke: k.fn, showGroups: false), c),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('platform.purgeDisabled')).first);
      await settle(tester);
      expect(k.callsTo('platform_purge_disabled_preview').length, 1);
      expect(k.commands.contains('platform_purge_disabled'), isFalse);
      expect(find.byType(ConfirmCard), findsOneWidget);
    });

    testWidgets('搜索框过滤未分组平台', (tester) async {
      await useBigSurface(tester);
      final k = platformsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(PlatformsPage(invoke: k.fn, showGroups: false), c),
      );
      await settle(tester);
      // 用 base_url 的片段搜，不用平台名 —— 输进搜索框的字本身也是一个 Text，
      // 拿 `find.text('P2')` 断言会连搜索框里那个一起数到（踩过）。
      await tester.enterText(find.byType(TextField).first, 'u2');
      await settle(tester);
      expect(find.text('P1'), findsNothing);
      expect(find.text('P2'), findsOneWidget);
    });

    testWidgets('平台页整体挂载（含内嵌分组区）不抛异常', (tester) async {
      await useBigSurface(tester);
      final k = platformsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(PlatformsPage(invoke: k.fn), c));
      await settle(tester);
      expect(tester.takeException(), isNull);
      expect(find.text('G10'), findsOneWidget);
    });
  });
}
