/// 页面批次 B 的 widget 测试（票 I07）：能挂载、能点、能填、失败有提示，
/// 以及**破坏性操作确认之前一个命令都不发**。
///
/// 两个坑沿用 I06 写在 README 里的（别再踩一遍）：
///   - 不用 `pumpAndSettle`：骨架的 `LiveDot` 是无限循环呼吸动画，等不到静止。
///     用 `harness.dart` 的 `settle(tester)`。
///   - 画布默认 800×600，这几页一屏放不下，`tap()` 会判成「点不到」。
///     交互测试先 `useBigSurface(tester)`。
library;

import 'package:aidog_flutter/pages.dart';
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

Map<String, dynamic> logRow(String id, {int status = 200}) => {
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
  'is_stream': false,
  'retry_count': 0,
  'created_at': 1700000000000,
};

FakeInvoke logsFake({List<Object?>? items}) => FakeInvoke({
  'platform_list': [plat(1, 'P1')],
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
          LogsPage(
            invoke: k.fn,
            copyText: (s) async => written.add(s),
          ),
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      expect(find.text('G10'), findsOneWidget);
    });

    testWidgets('空分组显示空态', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake(page: const []);
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      expect(find.text(c.t('group.empty')), findsOneWidget);
    });

    testWidgets('新建分组：名字为空时「创建」是禁用的', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('group.add')));
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('group.add')));
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('group.add')));
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);

      await tester.tap(find.text(c.t('action.delete')).first);
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('action.edit')).first);
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('action.edit')).first);
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);

      final before = k.callsTo('group_detail_list').length;
      await tester.tap(find.text(c.t('group.deletePlatformTitle')).first);
      await settle(tester);
      expect(k.callsTo('group_detail_list').length, before + 1);
      expect(k.commands.contains('platform_delete'), isFalse);
    });

    testWidgets('只属本组 → 弹窗里没有「仅移出本组」这个选项', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('group.deletePlatformTitle')).first);
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('group.deletePlatformTitle')).first);
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('group.testAll')).first);
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
      expect(
        find.textContaining(c.t('group.testAllTitle')),
        findsOneWidget,
      );
    });

    testWidgets('折叠分组 → 组内平台行消失，并落盘折叠态', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      expect(find.text('P1'), findsWidgets);

      await tester.tap(find.byIcon(Icons.expand_more).first);
      await settle(tester);
      expect(k.lastCallTo('set_ui_extra')!.args!['key'], '_ui_collapsed');
      expect(k.lastCallTo('set_ui_extra')!.args!['value'], true);
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
            onToast: (t, {required ok}) => toasts.add('$ok|$t'),
          ),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('action.delete')).first);
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);

      await tester.tap(find.text(c.t('group.batchOps')).first);
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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);

      await tester.tap(find.text(c.t('group.batchOps')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.selectAll')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchDelete')).last);
      await settle(tester);
      expect(find.text(c.t('group.batchDeleteTitle')), findsOneWidget);
      expect(k.commands.contains('batch_delete_platforms'), isFalse);

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
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);

      await tester.tap(find.text(c.t('group.batchOps')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.selectAll')));
      await settle(tester);
      await tester.tap(find.text(c.t('group.batchSetStatus')).last);
      await settle(tester);
      // 选中平台覆盖了本组全部 enabled 候选 → 要出无候选警告。
      expect(find.text(c.t('group.batchSetStatusNoCandidateWarning')), findsOneWidget);

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
            copyText: (s) async => copied.add(s),
          ),
          c,
        ),
      );
      await settle(tester);

      await tester.tap(find.text(c.t('group.copyCommand')).first);
      await settle(tester);
      await tester.tap(find.text(c.t('group.menuCopyClaude')));
      await settle(tester);
      expect(copied.single, contains('~/.aidog/settings.gk10.json'));

      await tester.tap(find.text(c.t('group.copyCommand')).first);
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
          GroupsSection(invoke: k.fn, copyText: (s) async => copied.add(s)),
          c,
        ),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('group.copyBaseUrl')));
      await settle(tester);
      expect(copied.single, 'http://127.0.0.1:9999/proxy');
    });

    testWidgets('组内优先级步进器：加一档 → 发 group_platform_set_level_priority', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);

      await tester.tap(find.byIcon(Icons.add).first);
      await settle(tester);
      expect(k.lastCallTo('group_platform_set_level_priority')!.args, {
        'groupId': 10,
        'platformId': 1,
        'levelPriority': 6,
      });
    });

    testWidgets('列表卡快捷添加映射：填齐才可点，点了发 group_update', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);

      await tester.tap(find.text('+ ${c.t('mapping.add')}').first);
      await settle(tester);
      var create = tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, c.t('action.create')),
      );
      expect(create.enabled, isFalse);

      await tester.enterText(find.byType(TextField).first, 'src');
      await settle(tester);
      await tester.tap(find.text(c.t('mapping.targetPlatform')).first);
      await settle(tester);
      await tester.tap(find.text('P1').last);
      await settle(tester);
      await tester.tap(find.text(c.t('mapping.target')).first);
      await settle(tester);
      await tester.tap(find.text('m').last);
      await settle(tester);

      create = tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, c.t('action.create')),
      );
      expect(create.enabled, isTrue);
      await tester.tap(find.text(c.t('action.create')));
      await settle(tester);
      final input =
          k.lastCallTo('group_update')!.args!['input']! as Map<String, Object?>;
      expect((input['model_mappings']! as List).length, 1);
    });

    testWidgets('编辑态：环境变量、pi 线路协议、锁定的分组密钥都在', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('action.edit')).first);
      await settle(tester);

      expect(find.text(c.t('group.groupKeyLocked')), findsOneWidget);
      expect(find.text(c.t('group.envVarsHint')), findsOneWidget);
      expect(find.text(c.t('group.piApiHint')), findsOneWidget);
      expect(find.text(c.t('group.timeoutDefault')), findsOneWidget);

      // 切 pi 线路协议 → 立刻写 extra 并重生成 pi 配置。
      await tester.tap(find.text('OpenAI Responses'));
      await settle(tester);
      expect(k.lastCallTo('set_ui_extra')!.args!['key'], 'pi_api');
      expect(k.commands.contains('sync_group_settings'), isTrue);
    });

    testWidgets('编辑态：加一条环境变量 → 保存时带上它', (tester) async {
      await useBigSurface(tester);
      final k = groupsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(GroupsSection(invoke: k.fn), c));
      await settle(tester);
      await tester.tap(find.text(c.t('action.edit')).first);
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
      expect(
        envVars.any((e) => (e as Map)['key'] == 'MY_VAR'),
        isTrue,
      );
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
      await tester.tap(find.text(c.t('platform.disable')).first);
      await settle(tester);
      final input =
          k.lastCallTo('platform_update')!.args!['input']! as Map<String, Object?>;
      expect(input['status'], 'disabled');
      expect(find.text(c.t('platform.enable')), findsWidgets);
    });

    testWidgets('快速测试 → model_test，结果进 toast', (tester) async {
      await useBigSurface(tester);
      final k = platformsFake();
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(PlatformsPage(invoke: k.fn, showGroups: false), c),
      );
      await settle(tester);
      await tester.tap(find.text(c.t('platform.quickTest')).first);
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
      await tester.tap(find.text(c.t('action.delete')).first);
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
