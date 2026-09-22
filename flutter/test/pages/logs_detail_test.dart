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

  testWidgets('用户侧与上游侧各自成段，两边的头 / 体 / URL 都看得到', (tester) async {
    final (_, c, _) = await openDetail(tester);

    expect(find.text(c.t('logs.userRequest')), findsOneWidget);
    expect(find.text(c.t('logs.upstreamRequest')), findsOneWidget);
    // 两侧各一个 URL / 请求头 / 请求体 / 响应头 / 响应体。
    expect(find.text('URL'), findsNWidgets(2));
    expect(find.text(c.t('logs.requestHeaders')), findsNWidgets(2));
    expect(find.text(c.t('logs.requestBody')), findsNWidgets(2));
    expect(find.text(c.t('logs.responseHeaders')), findsNWidgets(2));
    expect(find.text(c.t('logs.responseBody')), findsNWidgets(2));

    // 两侧的值互不串：上游 URL 与用户 URL 各显各的。
    expect(find.textContaining('https://local/v1/chat'), findsOneWidget);
    expect(find.textContaining('https://api.example.com'), findsOneWidget);
    // 响应体两边分开——改造前是「用户侧非空就用用户侧，否则上游侧」合成一份。
    expect(find.textContaining('"ok": true'), findsOneWidget);
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
    // 十个区块（两侧各五块）都有值，所以十个复制按钮。
    final copies = panelCopies(c);
    expect(copies, findsNWidgets(10));

    // 点上游请求体那块，复制到的是上游的值而不是用户的。
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

    // 上游五块全空 → 五条「(未捕获)」，复制按钮只剩用户侧那五个。
    expect(find.text(c.t('logs.noUpstream')), findsNWidgets(5));
    expect(panelCopies(c), findsNWidgets(5));
  });
}
