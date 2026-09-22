/// 智能识别弹窗的 widget 测试（票 20），对应 `src/components/platforms/SmartPasteModal.tsx`。
///
/// 识别规则本身由 `platform_paste_logic_test.dart` 的 49 条守着，这里只测**弹窗这一层**：
/// 打开即读剪贴板、按协议多选、填入表单交出去的东西对不对、分享串命中时走另一条路。
library;

import 'package:aidog_flutter/pages.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/material.dart';

import 'harness.dart';

/// 识别用的 preset。真实运行时来自 registry（`ProtocolMetaTable.pastePresets`），
/// 测试里给一份最小集合，覆盖「单端点」「双端点」「带 key 前缀」三种形态。
const List<PastePresetRef> testPresets = [
  PastePresetRef(
    value: 'deepseek',
    label: 'DeepSeek',
    keywords: ['deepseek'],
    hosts: ['api.deepseek.com'],
  ),
  PastePresetRef(
    value: 'anthropic',
    label: 'Anthropic',
    keywords: ['claude'],
    hosts: ['api.anthropic.com'],
    keyPrefixes: ['sk-ant-'],
  ),
];

/// 一段会同时解析出「平台 + 两条不同协议的 base_url + 一个 key」的文案。
const String sampleText =
    '分享 claude 中转\n'
    'Anthropic: https://api.anthropic.com\n'
    'OpenAI: https://relay.example.com/v1\n'
    'key: sk-ant-abcdefghijklmnop1234';

void main() {
  /// 挂一个弹窗。[clip] 是剪贴板内容，[shareParse] 决定 `platform_share_parse`
  /// 是认这段文本（返回分享对象）还是抛（当普通文案）。
  Future<(List<SmartPasteApplyResult>, int, FakeKernel)> mount(
    WidgetTester tester, {
    String clip = '',
    String? initialText,
    Object? Function(Map<String, Object?>? args)? shareParse,
    bool withManualEntry = false,
  }) async {
    await useBigSurface(tester);
    final applied = <SmartPasteApplyResult>[];
    var closed = 0;
    final k = FakeKernel({
      'platform_share_parse':
          shareParse ?? (_) => throw StateError('not a share string'),
    });
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        SmartPasteModal(
          presets: testPresets,
          invoke: k.invoke,
          readClipboard: () async => clip,
          initialText: initialText,
          onApply: applied.add,
          onClose: () => closed++,
          onManualEntry: withManualEntry ? () => closed++ : null,
        ),
        i18n,
      ),
    );
    await settle(tester);
    return (applied, closed, k);
  }

  testWidgets('打开即读剪贴板，内容自动填进文本框并当场识别', (tester) async {
    final (_, _, _) = await mount(tester, clip: sampleText);
    final i18n = await makeI18n(tester);
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      sampleText,
    );
    // 识别结果三段都出来了：平台名 / 两条 base_url / 一条 key。
    expect(find.text('Anthropic'), findsWidgets);
    expect(find.text('https://api.anthropic.com'), findsOneWidget);
    expect(find.text('https://relay.example.com/v1'), findsOneWidget);
    expect(find.text('sk-ant-abcdefghijklmnop1234'), findsOneWidget);
    expect(find.text(i18n.t('platform.paste.detected')), findsOneWidget);
  });

  testWidgets('给了 initialText 就不读剪贴板', (tester) async {
    await mount(tester, clip: '剪贴板里的东西', initialText: 'deepseek');
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      'deepseek',
    );
  });

  testWidgets('剪贴板为空 → 停在空态提示，「填入表单」点不动', (tester) async {
    await mount(tester);
    final i18n = await makeI18n(tester);
    expect(find.text(i18n.t('platform.paste.empty')), findsOneWidget);
    final apply = tester.widget<SmallButton>(
      find.widgetWithText(SmallButton, i18n.t('platform.paste.apply')),
    );
    expect(apply.enabled, isFalse);
  });

  testWidgets('base_url 按协议多选：同协议只留一条，跨协议可并存', (tester) async {
    final (applied, _, _) = await mount(tester, clip: sampleText);
    final i18n = await makeI18n(tester);
    // 默认每协议各选第一条 → 两条都选中。
    expect(tester.widgetList<Checkbox>(find.byType(Checkbox)).length, 2);
    for (final cb in tester.widgetList<Checkbox>(find.byType(Checkbox))) {
      expect(cb.value, isTrue);
    }
    // 取消 anthropic 那条 → 只剩一条进 apply。
    await tester.tap(find.text('https://api.anthropic.com'));
    await settle(tester);
    await tester.tap(
      find.widgetWithText(SmallButton, i18n.t('platform.paste.apply')),
    );
    await settle(tester);
    expect(applied, hasLength(1));
    expect(applied.first.baseUrls.map((b) => b.url), [
      'https://relay.example.com/v1',
    ]);
  });

  testWidgets('「填入表单」交出平台 / 选中 base_url / key，并关掉弹窗', (tester) async {
    final applied = <SmartPasteApplyResult>[];
    var closed = 0;
    await useBigSurface(tester);
    final k = FakeKernel({
      'platform_share_parse': (_) => throw StateError('not a share string'),
    });
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        SmartPasteModal(
          presets: testPresets,
          invoke: k.invoke,
          readClipboard: () async => sampleText,
          onApply: applied.add,
          onClose: () => closed++,
        ),
        i18n,
      ),
    );
    await settle(tester);
    await tester.tap(
      find.widgetWithText(SmallButton, i18n.t('platform.paste.apply')),
    );
    await settle(tester);

    expect(applied, hasLength(1));
    final r = applied.first;
    expect(r.platform?.value, 'anthropic');
    expect(r.apiKeys, ['sk-ant-abcdefghijklmnop1234']);
    expect(r.baseUrls, hasLength(2));
    expect(r.fullShare, isNull);
    expect(closed, 1);
  });

  testWidgets('多 key → 多选框 + 「已选 n/total」提示，默认全选', (tester) async {
    await mount(
      tester,
      clip:
          'claude 中转 key1: sk-ant-aaaaaaaaaaaaaaaa1111 '
          'key2: sk-ant-bbbbbbbbbbbbbbbb2222',
    );
    final i18n = await makeI18n(tester);
    expect(find.text('sk-ant-aaaaaaaaaaaaaaaa1111'), findsOneWidget);
    expect(find.text('sk-ant-bbbbbbbbbbbbbbbb2222'), findsOneWidget);
    expect(
      find.text(i18n.t('platform.paste.apiKeyMultiHint', {'n': 2, 'total': 2})),
      findsOneWidget,
    );
  });

  testWidgets('命中 aidog 分享串 → 走 fullShare 那条路，不再出识别结果区', (tester) async {
    final (applied, _, k) = await mount(
      tester,
      clip: 'name: P1',
      shareParse: (_) => {
        'name': 'P1',
        'platform_type': 'deepseek',
        'api_key': 'sk-x',
        'endpoints': <Object?>[],
        'models': <String, Object?>{},
      },
    );
    final i18n = await makeI18n(tester);
    expect(k.countOf('platform_share_parse') > 0, isTrue);
    expect(find.text(i18n.t('platform.paste.shareDetected')), findsOneWidget);
    // 分享串命中时识别结果区整块不渲染。
    expect(find.text(i18n.t('platform.paste.detected')), findsNothing);

    await tester.tap(
      find.widgetWithText(SmallButton, i18n.t('platform.paste.apply')),
    );
    await settle(tester);
    expect(applied.first.fullShare?['name'], 'P1');
    expect(applied.first.platform, isNull);
  });

  testWidgets('「手动填写」只在宿主接了回调时渲染', (tester) async {
    final i18n = await makeI18n(tester);
    await mount(tester);
    expect(find.text(i18n.t('platform.paste.manualEntry')), findsNothing);
    await mount(tester, withManualEntry: true);
    expect(find.text(i18n.t('platform.paste.manualEntry')), findsOneWidget);
  });

  testWidgets('识别到过期时间 → 单列一段展示，并随 apply 交出去', (tester) async {
    final now = DateTime.now();
    final future = DateTime(now.year, now.month + 1, 28, 23, 59);
    String two(int n) => n.toString().padLeft(2, '0');
    final (applied, _, _) = await mount(
      tester,
      clip:
          'claude 中转 key: sk-ant-abcdefghijklmnop1234 '
          '即将过期 ${two(future.month)}-${two(future.day)} 23:59',
    );
    final i18n = await makeI18n(tester);
    expect(find.text(i18n.t('platform.expiresAt')), findsOneWidget);
    await tester.tap(
      find.widgetWithText(SmallButton, i18n.t('platform.paste.apply')),
    );
    await settle(tester);
    expect(applied.first.expiresAt > 0, isTrue);
    final got = DateTime.fromMillisecondsSinceEpoch(applied.first.expiresAt);
    expect(got.month, future.month);
    expect(got.day, future.day);
  });

  testWidgets('取消按钮只关窗，不交结果', (tester) async {
    final applied = <SmartPasteApplyResult>[];
    var closed = 0;
    await useBigSurface(tester);
    final k = FakeKernel({
      'platform_share_parse': (_) => throw StateError('not a share string'),
    });
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        SmartPasteModal(
          presets: testPresets,
          invoke: k.invoke,
          readClipboard: () async => sampleText,
          onApply: applied.add,
          onClose: () => closed++,
        ),
        i18n,
      ),
    );
    await settle(tester);
    await tester.tap(find.widgetWithText(SmallButton, i18n.t('action.cancel')));
    await settle(tester);
    expect(applied, isEmpty);
    expect(closed, 1);
  });
}
