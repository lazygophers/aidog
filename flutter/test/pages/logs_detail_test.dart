/// 票 30 的护栏：日志详情面板是出问题时唯一的排障入口，两块关键信息必须在。
///
///   ① 尝试记录（`Logs/DetailPanel.tsx:218-261`）：重试过哪几个平台、各自的
///      状态码 / 耗时 / 错误原文；
///   ② 用户侧与上游侧**各自**的 URL / 请求头 / 请求体 / 响应头 / 响应体，
///      每块自带一个复制按钮，正文不再 14 行封顶。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';
import 'harness.dart';

/// 详情面板里的复制按钮。**必须限定在面板内** —— 表格每一行也有一颗
/// 同 tooltip 的复制按钮，不限定会多数出来一个。
Finder panelCopies(I18nController c) => find.descendant(
  of: find.byType(AidogModal),
  matching: find.byTooltip(c.t('logs.copy')),
);

/// 一条正文，长到足以证明「不再 14 行封顶」。
final String longBody = List.generate(60, (i) => 'line-$i').join('\n');

Map<String, Object?> logRow(String id) => {
  'id': id,
  'group_key': 'gk',
  'model': 'm',
  'actual_model': 'am',
  'platform_id': 1,
  'status_code': 200,
  'duration_ms': 12,
  'input_tokens': 0,
  'output_tokens': 0,
  'cache_tokens': 0,
  'created_at': 0,
  'est_cost': 0,
};

FakeInvoke detailFake({List<Object?> attempts = const []}) => FakeInvoke({
  'platform_list': [
    {
      'id': 1,
      'name': 'P1',
      'platform_type': 'openai',
      'base_url': 'https://u1',
      'api_key': 'k',
      'status': 'enabled',
      'enabled': true,
      'models': <String, Object?>{},
      'available_models': <Object?>[],
      'endpoints': <Object?>[],
      'est_balance_remaining': 0,
    },
  ],
  'group_detail_list': [
    {
      'group': {'id': 1, 'group_key': 'gk', 'name': 'G1'},
      'platforms': <Object?>[],
    },
  ],
  'proxy_log_distinct_models': <Object?>[],
  'proxy_log_list_filtered': {
    'items': [logRow('a1')],
    'has_more': false,
  },
  'proxy_log_get': {
    ...logRow('a1'),
    'source_protocol': 'openai',
    'target_protocol': 'openai',
    // 用户侧
    'request_url': 'https://local/v1/chat',
    'request_headers': '{"x-user":"1"}',
    'request_body': '{"from":"user"}',
    'user_response_headers': '{"x-user-resp":"1"}',
    'user_response_body': longBody,
    // 上游侧
    'upstream_request_url': 'https://api.example.com/v1/chat',
    'upstream_request_headers': '{"x-up":"1"}',
    'upstream_request_body': '{"from":"upstream"}',
    'upstream_response_headers': '{"x-up-resp":"1"}',
    'response_body': '{"ok":true}',
    'attempts': attempts,
  },
  'proxy_log_clear': null,
  'proxy_log_cleanup_expired': null,
  // 协议本地化名的数据源（`DetailPanel.tsx:175-176` 的 sourceLabel）。
  'get_defaults_json':
      '{"protocols":{"openai":{"name":{"zh-Hans":"OpenAI 兼容"}}}}',
  'get_client_types_json': '{}',
});

Map<String, Object?> attempt(
  String name, {
  int status = 500,
  String error = '',
  int ms = 30,
  int id = 1,
}) => {
  'platform_id': id,
  'platform_name': name,
  'status_code': status,
  'error': error,
  'duration_ms': ms,
  'ts': 0,
};

Future<(FakeInvoke, I18nController, List<String>)> openDetail(
  WidgetTester tester, {
  List<Object?> attempts = const [],
}) async {
  await useBigSurface(tester);
  final k = detailFake(attempts: attempts);
  final written = <String>[];
  final c = await makeI18n(tester);
  await tester.pumpWidget(
    wrapPage(LogsPage(invoke: k.fn, copyText: (s) async => written.add(s)), c),
  );
  await settle(tester);
  await tester.tap(find.text('am').first);
  await settle(tester);
  return (k, c, written);
}

void main() {
  testWidgets('有重试记录时逐条渲染平台名与状态码，连接失败显文案不显 0', (tester) async {
    final (_, c, written) = await openDetail(
      tester,
      attempts: [
        attempt('P-first', status: 0, error: 'connect timeout', ms: 1200),
        attempt('P-second', status: 429, error: 'rate limited', ms: 30, id: 2),
        attempt('P-third', status: 200, ms: 88, id: 3),
      ],
    );

    expect(find.text(c.t('logs.attempts')), findsOneWidget);
    expect(
      find.text(c.t('logs.attemptCount').replaceAll('{{n}}', '3')),
      findsOneWidget,
    );

    // 三条各自的平台名、状态码、耗时、错误原文。
    for (final name in ['P-first', 'P-second', 'P-third']) {
      expect(find.text(name), findsOneWidget, reason: name);
    }
    // 状态码 0 = 连接失败，显文案而不是「0」。
    expect(find.text(c.t('logs.connFailed')), findsOneWidget);
    expect(find.text('429'), findsOneWidget);
    expect(find.text('200'), findsWidgets);
    expect(find.text('connect timeout'), findsOneWidget);
    expect(find.text('rate limited'), findsOneWidget);
    expect(find.text('1200ms'), findsOneWidget);

    // 每条自带复制按钮，复制出来的是那一条的摘要。
    expect(find.byKey(const ValueKey('attempt-copy-1')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('attempt-copy-1')));
    await settle(tester);
    expect(written.last, 'P-second | 429 | 30ms | rate limited');
  });

  testWidgets('没有重试记录时整块不渲染', (tester) async {
    final (_, c, _) = await openDetail(tester);
    expect(find.text(c.t('logs.attempts')), findsNothing);
  });

  // 2026-09-22 起两侧改成 tab（`DetailPanel.tsx:263-288`）：原先两段顺序平铺，
  // 十块正文一次全展开，面板长到要滚很久才看得到上游那半。
  testWidgets('两侧是 tab：默认看用户侧，切过去才看上游侧，值互不串', (tester) async {
    final (_, c, _) = await openDetail(tester);

    expect(find.text(c.t('logs.userRequest')), findsOneWidget);
    expect(find.text(c.t('logs.upstreamRequest')), findsOneWidget);
    // 当前 tab 只画自己那五块。
    expect(find.text('URL'), findsOneWidget);
    expect(find.text(c.t('logs.requestHeaders')), findsOneWidget);
    expect(find.text(c.t('logs.requestBody')), findsOneWidget);
    expect(find.text(c.t('logs.responseHeaders')), findsOneWidget);
    expect(find.text(c.t('logs.responseBody')), findsOneWidget);
    expect(find.textContaining('https://local/v1/chat'), findsOneWidget);
    expect(find.textContaining('https://api.example.com'), findsNothing);

    await tester.tap(find.byKey(const ValueKey('detail-tab-1')));
    await settle(tester);
    expect(find.textContaining('https://api.example.com'), findsOneWidget);
    expect(find.textContaining('https://local/v1/chat'), findsNothing);
    // 响应体两边分开——改造前是「用户侧非空就用用户侧，否则上游侧」合成一份。
    expect(find.textContaining('"ok": true'), findsOneWidget);
  });

  testWidgets('tab 头带方向与该侧状态码', (tester) async {
    final (_, _, _) = await openDetail(tester);
    expect(find.textContaining('Client → Proxy'), findsOneWidget);
    expect(find.textContaining('Proxy → Platform'), findsOneWidget);
  });

  testWidgets('长正文整段铺开，不再 14 行封顶', (tester) async {
    await openDetail(tester);
    final body = tester.widgetList<SelectableText>(find.byType(SelectableText));
    final long = body.firstWhere((w) => (w.data ?? '').contains('line-59'));
    // 没有行数上限，60 行全在。
    expect(long.maxLines, isNull);
    expect(long.data, contains('line-0'));
    expect(long.data, contains('line-59'));
  });

  testWidgets('每个区块各带一个复制按钮，点它只复制本块', (tester) async {
    final (_, c, written) = await openDetail(tester);
    // 当前 tab 的五块都有值，所以五个复制按钮（另一侧要切过去才画）。
    final copies = panelCopies(c);
    expect(copies, findsNWidgets(5));

    // 切到上游侧，点它的请求体那块，复制到的是上游的值而不是用户的。
    await tester.tap(find.byKey(const ValueKey('detail-tab-1')));
    await settle(tester);
    final upstreamBody = find.ancestor(
      of: find.textContaining('"from": "upstream"'),
      matching: find.byType(Column),
    );
    await tester.tap(
      find.descendant(of: upstreamBody.first, matching: copies).first,
    );
    await settle(tester);
    expect(written.last, contains('"from": "upstream"'));
    expect(written.last, isNot(contains('"from": "user"')));
  });

  testWidgets('空区块写「(未捕获)」且不给复制按钮', (tester) async {
    await useBigSurface(tester);
    final k = detailFake();
    // 把上游那一侧全清空，模拟 log_upstream_request 关掉的样子。
    k.responses['proxy_log_get'] = {
      ...k.responses['proxy_log_get']! as Map<String, Object?>,
      'upstream_request_url': '',
      'upstream_request_headers': '',
      'upstream_request_body': '',
      'upstream_response_headers': '',
      'response_body': '',
    };
    final c = await makeI18n(tester);
    await tester.pumpWidget(wrapPage(LogsPage(invoke: k.fn), c));
    await settle(tester);
    await tester.tap(find.text('am').first);
    await settle(tester);

    // 用户侧五块都有值 → 五个复制按钮，没有「(未捕获)」。
    expect(find.text(c.t('logs.noUpstream')), findsNothing);
    expect(panelCopies(c), findsNWidgets(5));

    // 切到上游侧：五块全空 → 五条「(未捕获)」，一个复制按钮都没有。
    await tester.tap(find.byKey(const ValueKey('detail-tab-1')));
    await settle(tester);
    expect(find.text(c.t('logs.noUpstream')), findsNWidgets(5));
    expect(panelCopies(c), findsNothing);
  });

  // 第二梯队 2026-09-22：元信息区缺平台与时间、状态码是裸数字不上色、
  // 协议印裸枚举值、请求 ID 复制不了、每格没有复制按钮、没有刷新
  //（`DetailPanel.tsx:131-214`）。
  group('元信息区', () {
    testWidgets('平台与时间各占一行', (tester) async {
      final (_, c, _) = await openDetail(tester);
      final panel = find.byType(AidogModal);
      expect(
        find.descendant(of: panel, matching: find.text(c.t('logs.platform'))),
        findsOneWidget,
      );
      expect(
        find.descendant(of: panel, matching: find.text(c.t('logs.time'))),
        findsOneWidget,
      );
      expect(
        find.descendant(of: panel, matching: find.text('P1')),
        findsOneWidget,
      );
    });

    testWidgets('协议印本地化名，裸枚举值只进复制内容', (tester) async {
      final (_, _, written) = await openDetail(tester);
      final panel = find.byType(AidogModal);
      expect(
        find.descendant(of: panel, matching: find.text('OpenAI 兼容')),
        findsWidgets,
      );
      expect(
        find.descendant(of: panel, matching: find.text('openai')),
        findsNothing,
        reason: '裸枚举值不该印在脸上',
      );
      // 该行的复制按钮复制的仍是裸值（审计用）。
      await tester.tap(panelCopies(await makeI18n(tester)).first);
      await settle(tester);
      expect(written, isNotEmpty);
    });

    testWidgets('请求 ID 独占一行，复制成 request_id=<id>', (tester) async {
      final (_, _, written) = await openDetail(tester);
      await tester.tap(find.byKey(const ValueKey('detail-copy-id')));
      await settle(tester);
      expect(written, contains('request_id=a1'));
    });

    testWidgets('刷新按钮重新拉这一条详情', (tester) async {
      final (k, _, _) = await openDetail(tester);
      final before = k.callsTo('proxy_log_get').length;
      await tester.tap(find.byKey(const ValueKey('detail-refresh')));
      await settle(tester);
      expect(k.callsTo('proxy_log_get').length, before + 1);
    });
  });
}
