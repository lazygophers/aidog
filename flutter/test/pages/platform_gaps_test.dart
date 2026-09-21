/// 平台页四条剩余缺口的测试（对齐清单 `.scratch/flutter-frontend/impl/I18-alignment-gaps.md`
/// 的「未完成」段）：
///   #8  未分组平台拖进分组
///   #20 「最近测试」徽章展开响应正文
///   #37 页头「+ 添加分组」
///   —   分享面板的二维码
///
/// `parseTestBody` 那一组是 `src/components/shared/TestResultBody.test.ts` 的
/// **逐条翻译，数据与期望值一字不改**。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pretty_qr_code/pretty_qr_code.dart';

import '../settings/fake_invoke.dart';
import 'harness.dart';

/// React 那边的 stub t 返回 fallback（第二参），这里把 key 直接映成同样的中文，
/// 好让下面的期望值与 TS 版逐字相同。
String _t(String key) => const {
  'testBody.errorMessage': '错误信息',
  'testBody.errorType': '错误类型',
  'testBody.errorCode': '错误码',
  'testBody.error': '错误',
  'testBody.inputTokens': '输入 tokens',
  'testBody.outputTokens': '输出 tokens',
  'testBody.model': '模型',
  'testBody.content': '响应内容',
}[key]!;

/// 把 rows 归一成字符串再断言 —— Dart 的 record / List 直接比内容虽可行，
/// 但失败信息难读，统一成 `label=value` 串。
List<String> _flat(ParsedTestBody r) =>
    [for (final row in r.rows) '${row.label}=${row.value}'];

Map<String, dynamic> _plat(int id, String name) => {
  'id': id,
  'name': name,
  'platform_type': 'openai',
  'status': 'enabled',
  'enabled': true,
};

FakeInvoke _pageFake({Object? lastTest, List<Object?>? groups}) => FakeInvoke({
  'platform_list': [_plat(1, 'Solo')],
  'group_detail_list': <Object?>[],
  'group_detail_list_paged':
      groups ??
      [
        {
          'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
          'platforms': <Object?>[],
          'model_mappings': <Object?>[],
        },
      ],
  'all_platform_usage_stats': <String, Object?>{},
  'all_group_usage_stats': <String, Object?>{},
  'get_last_test_result': lastTest,
  'scheduling_settings_get': null,
  'get_defaults_json': '{"protocols":{}}',
  'get_protocol_logo_path': '',
  'get_protocol_logo_data_url': '',
  'sync_protocol_logo': null,
  'proxy_get_settings': {'port': 9999},
  'platform_query_quota': {'success': true, 'queried_at': 1},
  'platform_usage_stats': null,
  'group_platform_move': null,
  'set_ui_extra': null,
});

Future<(FakeInvoke, I18nController)> _mountPage(
  WidgetTester tester,
  FakeInvoke k, {
  bool showGroups = true,
}) async {
  await useBigSurface(tester);
  final c = await makeI18n(tester);
  await tester.pumpWidget(
    wrapPage(
      PlatformsPage(
        invoke: k.fn,
        showGroups: showGroups,
        logUpdates: const Stream<void>.empty(),
      ),
      c,
    ),
  );
  await settle(tester);
  return (k, c);
}

void main() {
  // ══ parseTestBody（React: TestResultBody.test.ts 逐条翻译）══════════

  group('parseTestBody（React: TestResultBody.test.ts）', () {
    test('空字符串 → raw 空', () {
      expect(parseTestBody('', _t).rows, isEmpty);
      expect(parseTestBody('', _t).text, '');
      expect(parseTestBody('   ', _t).rows, isEmpty);
      expect(parseTestBody('   ', _t).text, '');
    });

    test('非 JSON → raw 原文', () {
      final r = parseTestBody('HTTP 500 Internal Error', _t);
      expect(r.rows, isEmpty);
      expect(r.text, 'HTTP 500 Internal Error');
    });

    test('非对象 JSON（数字/数组）→ raw 原文', () {
      expect(parseTestBody('123', _t).rows, isEmpty);
      expect(parseTestBody('123', _t).text, '123');
      expect(parseTestBody('[1,2]', _t).rows, isEmpty);
      expect(parseTestBody('[1,2]', _t).text, '[1,2]');
    });

    test('error 对象 → 拆 message/type/code', () {
      const body =
          '{"error":{"message":"bad key","type":"auth_error","code":"401"}}';
      expect(_flat(parseTestBody(body, _t)), [
        '错误信息=bad key',
        '错误类型=auth_error',
        '错误码=401',
      ]);
    });

    test('error 字符串 → 单行错误', () {
      final r = parseTestBody('{"error":"rate limited"}', _t);
      expect(_flat(r), contains('错误=rate limited'));
    });

    test('usage（anthropic 风格）→ input/output tokens', () {
      final r = parseTestBody('{"usage":{"input_tokens":10,"output_tokens":5}}', _t);
      expect(_flat(r), contains('输入 tokens=10'));
      expect(_flat(r), contains('输出 tokens=5'));
    });

    test('usage（openai 风格）→ prompt/completion tokens 归一', () {
      final r = parseTestBody(
        '{"usage":{"prompt_tokens":7,"completion_tokens":3}}',
        _t,
      );
      expect(_flat(r), contains('输入 tokens=7'));
      expect(_flat(r), contains('输出 tokens=3'));
    });

    test('anthropic content → 响应内容', () {
      final r = parseTestBody(
        '{"content":[{"type":"text","text":"hello"}]}',
        _t,
      );
      expect(_flat(r), contains('响应内容=hello'));
    });

    test('openai choices → 响应内容', () {
      final r = parseTestBody(
        '{"choices":[{"message":{"content":"world"}}]}',
        _t,
      );
      expect(_flat(r), contains('响应内容=world'));
    });

    test('已知但无可识别字段（空对象）→ raw 回退', () {
      final r = parseTestBody('{"foo":1}', _t);
      expect(r.rows, isEmpty);
      expect(r.text, '{"foo":1}');
    });

    test('error 对象存在但无 message/type/code → 整体序列化兜底（known）', () {
      final r = parseTestBody('{"error":{"detail":"x"}}', _t);
      expect(_flat(r), ['错误={"detail":"x"}']);
    });

    // 本票新补的边界（React 未覆盖，但两侧行为必须一致）。
    test('model 单独成行；usage 子字段缺失就不出那一行', () {
      final r = parseTestBody('{"model":"gpt-5","usage":{"input_tokens":4}}', _t);
      expect(_flat(r), ['输入 tokens=4', '模型=gpt-5']);
    });

    test('content 取第一段非空文本，前面的空块跳过', () {
      final r = parseTestBody(
        '{"content":[{"type":"thinking"},{"type":"text","text":"ok"}]}',
        _t,
      );
      expect(_flat(r), ['响应内容=ok']);
    });

    test('choices 里 message 不是对象 → 不出内容行，回退 raw', () {
      final r = parseTestBody('{"choices":[{"message":1}]}', _t);
      expect(r.rows, isEmpty);
      expect(r.text, '{"choices":[{"message":1}]}');
    });

    test('对象 / 数组值走 JSON 序列化（toDisplay）', () {
      expect(testBodyDisplay(null), '');
      expect(testBodyDisplay('x'), 'x');
      expect(testBodyDisplay(3), '3');
      expect(testBodyDisplay(true), 'true');
      expect(testBodyDisplay({'a': 1}), '{"a":1}');
      expect(testBodyDisplay([1, 2]), '[1,2]');
    });
  });

  // ══ #20 「最近测试」徽章展开响应正文 ════════════════════════════

  group('#20 最近测试徽章可展开', () {
    testWidgets('有响应正文 → 带 ▸，点开出结构化正文，再点收起', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      final (_, _) = await _mountPage(
        tester,
        _pageFake(
          lastTest: {
            'success': true,
            'status_code': 200,
            'duration_ms': 12,
            'created_at': now - 5 * 60000,
            'error': '',
            'response_body': '{"model":"gpt-5"}',
          },
        ),
        showGroups: false,
      );
      expect(find.textContaining('▸'), findsOneWidget);
      expect(find.byType(TestResultBody), findsNothing);

      await tester.tap(find.textContaining('▸'));
      await settle(tester);
      expect(find.byType(TestResultBody), findsOneWidget);
      expect(find.text('gpt-5'), findsOneWidget);
      expect(find.textContaining('▾'), findsOneWidget);

      await tester.tap(find.textContaining('▾'));
      await settle(tester);
      expect(find.byType(TestResultBody), findsNothing);
    });

    testWidgets('响应正文为空 → 不带箭头，点了也不展开', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      await _mountPage(
        tester,
        _pageFake(
          lastTest: {
            'success': false,
            'status_code': 500,
            'duration_ms': 0,
            'created_at': now - 2 * 3600000,
            'error': 'boom',
            'response_body': '   ',
          },
        ),
        showGroups: false,
      );
      expect(find.textContaining('▸'), findsNothing);
      await tester.tap(find.textContaining('✗'));
      await settle(tester);
      expect(find.byType(TestResultBody), findsNothing);
    });

    testWidgets('TestResultBody 拿到空正文 → 什么都不画', (tester) async {
      final c = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(const TestResultBody(body: '  '), c));
      await settle(tester);
      // 宽度由外层 SizedBox(width: 1280) 撑开，能证明「什么都不画」的是高度。
      expect(tester.getSize(find.byType(TestResultBody)).height, 0);
    });

    testWidgets('正文解析不出已知结构 → 原文照出', (tester) async {
      final now = DateTime.now().millisecondsSinceEpoch;
      await _mountPage(
        tester,
        _pageFake(
          lastTest: {
            'success': false,
            'status_code': 502,
            'duration_ms': 3,
            'created_at': now,
            'error': '',
            'response_body': 'Bad Gateway',
          },
        ),
        showGroups: false,
      );
      await tester.tap(find.textContaining('▸'));
      await settle(tester);
      expect(find.text('Bad Gateway'), findsOneWidget);
    });
  });

  // ══ #37 页头「+ 添加分组」════════════════════════════════════════

  group('#37 页头「+ 添加分组」', () {
    testWidgets('点一下切到新建分组面板', (tester) async {
      final (_, c) = await _mountPage(tester, _pageFake());
      // 页头那颗带 `+ ` 前缀，分组区自己那颗不带 —— 两颗并存（React 同样两处都有）。
      expect(find.text('+ ${c.t('group.add')}'), findsOneWidget);
      await tester.tap(find.text('+ ${c.t('group.add')}'));
      await settle(tester);
      expect(find.text(c.t('group.groupKeyHint')), findsOneWidget);
    });

    testWidgets('showGroups=false（不渲染分组区）→ 页头不画这颗按钮', (tester) async {
      final (_, c) = await _mountPage(tester, _pageFake(), showGroups: false);
      expect(find.text('+ ${c.t('group.add')}'), findsNothing);
    });
  });

  // ══ #8 未分组平台拖进分组 ════════════════════════════════════════

  group('#8 未分组平台拖进分组', () {
    testWidgets('拖到分组卡上松手 → group_platform_move(fromGroupId=0)', (tester) async {
      final (k, _) = await _mountPage(tester, _pageFake());
      final card = find.byType(PlatformCard);
      expect(card, findsOneWidget);

      final gesture = await tester.startGesture(tester.getCenter(card));
      // 先挪开一点让 Draggable 赢下手势竞技场，再落到分组卡中心。
      await tester.pump(const Duration(milliseconds: 20));
      await gesture.moveBy(const Offset(0, -20));
      await tester.pump();
      await gesture.moveTo(tester.getCenter(find.text('G10')));
      await tester.pump();
      await gesture.up();
      await settle(tester);

      final call = k.lastCallTo('group_platform_move');
      expect(call, isNotNull);
      expect(call!.args!['platformId'], 1);
      expect(call.args!['fromGroupId'], 0);
      expect(call.args!['toGroupId'], 10);
      // 搬完两边都要重拉：平台侧刷 membership，分组侧刷 details。
      expect(k.callsTo('group_detail_list').length, greaterThanOrEqualTo(2));
    });

    testWidgets('松手时不在任何分组卡上 → 不发命令', (tester) async {
      final (k, _) = await _mountPage(tester, _pageFake());
      final card = find.byType(PlatformCard);
      final gesture = await tester.startGesture(tester.getCenter(card));
      await tester.pump(const Duration(milliseconds: 20));
      await gesture.moveBy(const Offset(0, 40));
      await tester.pump();
      await gesture.up();
      await settle(tester);
      expect(k.callsTo('group_platform_move'), isEmpty);
    });

    testWidgets('已经在该分组里的平台 → 该分组不接这次拖拽', (tester) async {
      final k = _pageFake(
        groups: [
          {
            'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
            'platforms': [
              {'platform': _plat(1, 'Solo')},
            ],
            'model_mappings': <Object?>[],
          },
        ],
      );
      await _mountPage(tester, k);
      final card = find.byType(PlatformCard);
      final gesture = await tester.startGesture(tester.getCenter(card));
      await tester.pump(const Duration(milliseconds: 20));
      await gesture.moveBy(const Offset(0, -20));
      await tester.pump();
      await gesture.moveTo(tester.getCenter(find.text('G10')));
      await tester.pump();
      await gesture.up();
      await settle(tester);
      expect(k.callsTo('group_platform_move'), isEmpty);
    });
  });

  // ══ 分享面板的二维码 ═════════════════════════════════════════════

  group('分享面板二维码', () {
    Future<I18nController> mount(
      WidgetTester tester,
      Map<String, Object?> share, {
      String? urlScheme = 'aidog://platform/import',
    }) async {
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SharePanel(
            share: share,
            title: 'P',
            urlScheme: urlScheme,
            onToast: (_, {required bool ok}) {},
            onClose: () {},
            copy: (_) async {},
          ),
          c,
        ),
      );
      await settle(tester);
      return c;
    }

    testWidgets('给了 urlScheme → 画二维码 + 「扫码导入」小标', (tester) async {
      final c = await mount(tester, {'name': 'P', 'api_key': 'sk-1'});
      expect(find.text(c.t('platform.share.scanToImport')), findsOneWidget);
      expect(find.byType(PrettyQrView), findsOneWidget);
      expect(find.text(c.t('platform.share.qrTooLong')), findsNothing);
    });

    testWidgets('没给 urlScheme → 整个二维码区块都不画', (tester) async {
      final c = await mount(tester, {'name': 'P'}, urlScheme: null);
      expect(find.text(c.t('platform.share.scanToImport')), findsNothing);
      expect(find.byType(PrettyQrView), findsNothing);
    });

    testWidgets('深链超 2900 字 → 降级成「内容过长」提示，不画二维码', (tester) async {
      final c = await mount(tester, {'name': 'P', 'api_key': 'k' * 4000});
      expect(find.text(c.t('platform.share.qrTooLong')), findsOneWidget);
      expect(find.byType(PrettyQrView), findsNothing);
    });

    testWidgets('切到 YAML 格式，二维码仍是深链（跟着格式变的只有正文）', (tester) async {
      final c = await mount(tester, {'name': 'P', 'api_key': 'sk-1'});
      await tester.tap(find.text(c.t('platform.share.format.yaml')));
      await settle(tester);
      expect(find.byType(PrettyQrView), findsOneWidget);
    });
  });
}
