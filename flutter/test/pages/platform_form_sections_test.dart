/// 表单各分区的交互测试：每个控件真的改到了控制器里的那个字段。
///
/// 下拉这一类不走「点开菜单 route」—— 那要 `pumpAndSettle`，而本项目骨架有无限
/// 呼吸动画（README 已记）。直接拿到 widget 调它的 `onChanged`，测的是**接线**，
/// 与点开菜单选中的结果一模一样。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/src/pages/platform_extra.dart';
import 'package:aidog_flutter/src/pages/platform_form.dart';
import 'package:aidog_flutter/src/pages/platform_form_bits.dart';
import 'package:aidog_flutter/src/pages/platform_form_logic.dart';
import 'package:aidog_flutter/src/pages/platforms_logic.dart';
import 'package:aidog_flutter/src/pages/time_window.dart';
import 'package:aidog_flutter/src/pages/ui_bits.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';
import 'harness.dart';
import 'platform_form_logic_test.dart' show formFake, plat;

Finder fieldWithHint(String hint) => find.byWidgetPredicate(
  (w) => w is PlatformField && w.hint == hint,
);

Finder fieldWithLabel(String label) => find.byWidgetPredicate(
  (w) => w is PlatformField && w.label == label,
);

Finder inputOf(Finder field) =>
    find.descendant(of: field, matching: find.byType(TextField));

/// 按「候选里含某个值」挑一个下拉（比按位置稳，加分区不会错位）。
FormDropdown dropdownWithOption(WidgetTester tester, String option) => tester
    .widgetList<FormDropdown>(find.byType(FormDropdown))
    .firstWhere((d) => d.options.contains(option));

SmallButton buttonWithLabel(WidgetTester tester, String label) => tester
    .widgetList<SmallButton>(find.byType(SmallButton))
    .firstWhere((b) => b.label == label);

void main() {
  late FakeInvoke k;
  late PlatformFormController f;
  late I18nController t;

  Future<void> boot(
    WidgetTester tester, {
    String protocol = 'openai',
    bool edit = false,
    FakeInvoke? fake,
  }) async {
    await useBigSurface(tester);
    t = await makeI18n(tester);
    k = fake ?? formFake();
    final list = PlatformsController(invoke: k.fn);
    f = PlatformFormController(list: list, invoke: k.fn);
    await tester.runAsync(() async {
      await list.init();
      await f.init(locale: 'zh-Hans');
    });
    if (edit) {
      f.handleEdit(list.platforms.first);
    } else {
      f.openCreatePlatform();
      f.handleProtocolChange(protocol);
    }
    await tester.pumpWidget(
      wrapPage(
        StatefulBuilder(
          builder: (context, setState) {
            f.onChanged = () => setState(() {});
            return PlatformEditForm(controller: f);
          },
        ),
        t,
      ),
    );
    await settle(tester);
  }

  group('F1 基础信息', () {
    testWidgets('改名字写回控制器', (tester) async {
      await boot(tester);
      await tester.enterText(inputOf(fieldWithHint(t.t('platform.name'))), 'X');
      await settle(tester);
      expect(f.name, 'X');
    });

    testWidgets('协议选择器：展开 → 搜索 → 选中', (tester) async {
      await boot(tester);
      // 收起态只有触发器，没有搜索框。
      expect(fieldWithHint(t.t('platform.searchPlaceholder')), findsNothing);
      await tester.tap(find.byType(ProtocolPicker));
      await settle(tester);
      final search = fieldWithHint(t.t('platform.searchPlaceholder'));
      expect(search, findsOneWidget);

      await tester.enterText(inputOf(search), 'Anthropic');
      await settle(tester);
      // 过滤后只剩一条，点它。
      await tester.tap(find.text('Anthropic').last);
      await settle(tester);
      expect(f.protocol, 'anthropic');
      // 选完自动收起。
      expect(fieldWithHint(t.t('platform.searchPlaceholder')), findsNothing);
    });

    testWidgets('协议选择器：搜不到时给 No match（React 也是写死英文）', (tester) async {
      await boot(tester);
      await tester.tap(find.byType(ProtocolPicker));
      await settle(tester);
      await tester.enterText(
        inputOf(fieldWithHint(t.t('platform.searchPlaceholder'))),
        'zzzz-not-a-protocol',
      );
      await settle(tester);
      expect(find.text('No match'), findsOneWidget);
    });

    testWidgets('coding plan 协议在列表里带 Code 角标', (tester) async {
      await boot(tester);
      await tester.tap(find.byType(ProtocolPicker));
      await settle(tester);
      await tester.enterText(
        inputOf(fieldWithHint(t.t('platform.searchPlaceholder'))),
        'GLM',
      );
      await settle(tester);
      expect(find.text('Code'), findsOneWidget);
      // label 不含 Coding 字样时补后缀；这里 registry 的名字本就带 Coding，不补。
      expect(find.text('GLM Coding'), findsOneWidget);
    });
  });

  group('F2 Mock 配置', () {
    testWidgets('文本 / 必填数值 / 可选数值 / 两个下拉全都接到 mockConfig', (tester) async {
      await boot(tester, protocol: 'mock');

      await tester.enterText(
        inputOf(fieldWithLabel('${t.t('platform.mockResponseText')}（response_text）')),
        'hi',
      );
      await settle(tester);
      expect(f.mockConfig.responseText, 'hi');

      await tester.enterText(inputOf(fieldWithLabel('finish_reason')), 'stop');
      await settle(tester);
      expect(f.mockConfig.finishReason, 'stop');

      await tester.enterText(
        inputOf(fieldWithLabel('${t.t('platform.mockStatusCode')}（status_code）')),
        '500',
      );
      await settle(tester);
      expect(f.mockConfig.statusCode, 500);

      for (final (label, read) in <(String, int Function())>[
        (
          '${t.t('platform.mockDelayMs')}（delay_ms）',
          () => f.mockConfig.delayMs,
        ),
        (
          '${t.t('platform.mockInputTokens')}（input_tokens）',
          () => f.mockConfig.inputTokens,
        ),
        (
          '${t.t('platform.mockOutputTokens')}（output_tokens）',
          () => f.mockConfig.outputTokens,
        ),
        (
          '${t.t('platform.mockCacheTokens')}（cache_tokens）',
          () => f.mockConfig.cacheTokens,
        ),
        (
          '${t.t('platform.mockChunkCount')}（chunk_count）',
          () => f.mockConfig.chunkCount,
        ),
      ]) {
        await tester.enterText(inputOf(fieldWithLabel(label)), '7');
        await settle(tester);
        expect(read(), 7, reason: label);
      }

      // 可选数值：填了写进去，清空变回 null（禁塞 0 假装默认）。
      final ttft = fieldWithLabel('${t.t('platform.mockTtftMs')}（ttft_ms）');
      await tester.enterText(inputOf(ttft), '12');
      await settle(tester);
      expect(f.mockConfig.ttftMs, 12);
      await tester.enterText(inputOf(ttft), '');
      await settle(tester);
      expect(f.mockConfig.ttftMs, isNull);
      // 负数按 min 钳到 0。
      await tester.enterText(inputOf(ttft), '-3');
      await settle(tester);
      expect(f.mockConfig.ttftMs, 0);
      // 非数字不动值。
      await tester.enterText(inputOf(ttft), 'abc');
      await settle(tester);
      expect(f.mockConfig.ttftMs, 0);

      final inter = fieldWithLabel(
        '${t.t('platform.mockInterChunkMs')}（inter_chunk_ms）',
      );
      await tester.enterText(inputOf(inter), '9');
      await settle(tester);
      expect(f.mockConfig.interChunkMs, 9);
      await tester.enterText(inputOf(inter), '');
      await settle(tester);
      expect(f.mockConfig.interChunkMs, isNull);

      // error_rate 上下限都钳。
      final rate = fieldWithLabel('${t.t('platform.mockErrorRate')}（error_rate）');
      await tester.enterText(inputOf(rate), '5');
      await settle(tester);
      expect(f.mockConfig.errorRate, 1);
      await tester.enterText(inputOf(rate), '');
      await settle(tester);
      expect(f.mockConfig.errorRate, isNull);

      dropdownWithOption(tester, 'rate_limit_429').onChanged!('rate_limit_429');
      await settle(tester);
      expect(f.mockConfig.errorMode, 'rate_limit_429');

      dropdownWithOption(tester, 'force_on').onChanged!('force_on');
      await settle(tester);
      expect(f.mockConfig.streamOverride, isTrue);
      dropdownWithOption(tester, 'force_on').onChanged!('force_off');
      await settle(tester);
      expect(f.mockConfig.streamOverride, isFalse);
      dropdownWithOption(tester, 'force_on').onChanged!('follow');
      await settle(tester);
      expect(f.mockConfig.streamOverride, isNull);
    });
  });

  group('F3 配额脚本', () {
    testWidgets('变体下拉切到自定义 → 出编辑器；写脚本写回控制器', (tester) async {
      await boot(tester);
      dropdownWithOption(tester, kQuotaCustomVariant)
          .onChanged!(kQuotaCustomVariant);
      await settle(tester);
      await tester.enterText(
        inputOf(fieldWithLabel(t.t('platform.quotaScript.customLabel'))),
        '// s',
      );
      await settle(tester);
      expect(f.quotaCustomScript, '// s');
    });

    testWidgets('registry 变体的 requires 输入接到 quotaRequires', (tester) async {
      await boot(tester);
      // 夹具里 openai 的 v1 变体要一个 org_id。
      await tester.enterText(inputOf(fieldWithHint('org_id')), 'org-9');
      await settle(tester);
      expect(f.quotaRequires['org_id'], 'org-9');
    });

    testWidgets('newapi 的用户 ID 输入接到 user_id', (tester) async {
      await boot(tester, protocol: 'newapi');
      await tester.enterText(
        inputOf(fieldWithHint(t.t('platform.newapiUserIdPlaceholder'))),
        '42',
      );
      await settle(tester);
      expect(f.quotaRequires['user_id'], '42');
    });
  });

  group('F4 Devin', () {
    testWidgets('超时输入与模式下拉（含 __none__ 哨兵）', (tester) async {
      await boot(tester, protocol: 'devin');
      await tester.enterText(
        inputOf(fieldWithHint(t.t('platform.devinTimeoutPlaceholder'))),
        '300',
      );
      await settle(tester);
      expect(f.devinConfig.devinTimeout, '300');

      dropdownWithOption(tester, 'fusion').onChanged!('fusion');
      await settle(tester);
      expect(f.devinConfig.devinMode, 'fusion');
      dropdownWithOption(tester, 'fusion').onChanged!('__none__');
      await settle(tester);
      expect(f.devinConfig.devinMode, '');
    });
  });

  group('F5 透传', () {
    testWidgets('base_url 与可空 token 两个输入', (tester) async {
      await boot(tester, protocol: 'claude_code');
      await tester.enterText(
        inputOf(fieldWithHint('https://api.anthropic.com')),
        'https://x',
      );
      await settle(tester);
      expect(f.endpoints.single.baseUrl, 'https://x');

      await tester.enterText(
        inputOf(fieldWithHint(t.t('platform.apiKeyOptional'))),
        'tok',
      );
      await settle(tester);
      expect(f.apiKey, 'tok');
    });
  });

  group('F6 端点', () {
    testWidgets('加 / 改协议 / 改 URL / 切 C / 删', (tester) async {
      await boot(tester);
      expect(f.endpoints.length, 1);
      await tester.tap(find.text('+ ${t.t('platform.addEndpoint')}'));
      await settle(tester);
      expect(f.endpoints.length, 2);

      dropdownWithOption(tester, 'gemini').onChanged!('gemini');
      await settle(tester);
      expect(f.endpoints.first.protocol, 'gemini');
      expect(f.endpoints.first.clientType, 'default');

      await tester.enterText(
        inputOf(fieldWithHint('Endpoint Base URL')).first,
        'https://g',
      );
      await settle(tester);
      expect(f.endpoints.first.baseUrl, 'https://g');

      dropdownWithOption(tester, 'codex_tui').onChanged!('codex_tui');
      await settle(tester);
      expect(f.endpoints.first.clientType, 'codex_tui');

      await tester.tap(find.widgetWithText(SmallButton, 'C').first);
      await settle(tester);
      expect(f.endpoints.first.codingPlan, isTrue);

      await tester.tap(
        find.widgetWithText(SmallButton, t.t('action.delete')).first,
      );
      await settle(tester);
      expect(f.endpoints.length, 1);
    });

    testWidgets('锁死协议：下拉与输入禁用、无增删按钮', (tester) async {
      await boot(tester, protocol: 'glm_coding');
      expect(find.text('+ ${t.t('platform.addEndpoint')}'), findsNothing);
      expect(find.text(t.t('platform.endpointsLockedHint')), findsOneWidget);
      // 夹具里 glm_coding 没有默认端点 → 空态提示。
      expect(find.text(t.t('platform.noEndpoints')), findsOneWidget);
    });
  });

  group('F7 Token', () {
    testWidgets('编辑态是单行密文，可切明文', (tester) async {
      await boot(tester, edit: true);
      final fin = fieldWithHint(t.t('platform.tokenPlaceholderEdit'));
      expect(tester.widget<PlatformField>(fin).obscure, isTrue);
      await tester.tap(find.byIcon(Icons.visibility));
      await settle(tester);
      expect(tester.widget<PlatformField>(fin).obscure, isFalse);
      expect(find.byIcon(Icons.visibility_off), findsOneWidget);
    });

    testWidgets('创建态是多行，没有明文按钮', (tester) async {
      await boot(tester);
      final fin = fieldWithHint(t.t('platform.tokenPlaceholder'));
      expect(tester.widget<PlatformField>(fin).obscure, isFalse);
      expect(find.byIcon(Icons.visibility), findsNothing);
      await tester.enterText(inputOf(fin), 'a\nb\nc');
      await settle(tester);
      expect(f.batchPreviewKeys, ['a', 'b', 'c']);
    });
  });

  group('F9 模型矩阵', () {
    testWidgets('单元格输入写回槽位；下拉可展开并选中', (tester) async {
      await boot(tester);
      final cells = find.byType(ModelCell);
      expect(cells, findsNWidgets(5)); // 5 槽 × 只有默认列
      await tester.enterText(
        find.descendant(of: cells.first, matching: find.byType(TextField)),
        'gpt',
      );
      await settle(tester);
      expect(f.models['default'], 'gpt');

      // 候选来自 registry 的 model_list；按输入过滤后选一条。
      await tester.tap(
        find.descendant(of: cells.first, matching: find.byType(IconButton)),
      );
      await settle(tester);
      await tester.tap(find.text('gpt-5-mini').last);
      await settle(tester);
      expect(f.models['default'], 'gpt-5-mini');
    });

    testWidgets('没有候选时不画下拉箭头', (tester) async {
      await boot(tester, protocol: 'newapi'); // 夹具里没 model_list
      expect(
        find.descendant(
          of: find.byType(ModelCell).first,
          matching: find.byType(IconButton),
        ),
        findsNothing,
      );
    });

    testWidgets('一键填充：default 空时点不动，有值时灌满四槽', (tester) async {
      await boot(tester);
      f.setModel('default', '');
      await settle(tester);
      expect(buttonWithLabel(tester, t.t('platform.fillAll')).enabled, isFalse);
      f.setModel('default', 'm');
      await settle(tester);
      await tester.tap(find.text(t.t('platform.fillAll')));
      await settle(tester);
      expect(f.models['gpt'], 'm');
    });

    testWidgets('获取模型：缺 key 点不动，填了 key 才亮', (tester) async {
      await boot(tester);
      expect(
        buttonWithLabel(tester, t.t('platform.fetchModels')).enabled,
        isFalse,
      );
      f.setApiKey('sk');
      await settle(tester);
      await tester.tap(find.text(t.t('platform.fetchModels')));
      await settle(tester);
      // 夹具返空列表 → 报「未获取到模型」。
      expect(find.text(t.t('platform.fetchEmpty')), findsOneWidget);
    });

    testWidgets('时段档：加一列 → 列头可编辑 → 上下移 → 删', (tester) async {
      await boot(tester);
      expect(find.text(t.t('platform.time_windows_empty')), findsOneWidget);
      await tester.tap(find.text('+ ${t.t('platform.time_windows_add_rule')}'));
      await settle(tester);
      expect(f.timeModels.length, 1);
      // 列头描述：没有窗口 → 永不命中。
      expect(find.text(t.t('platform.window_never')), findsOneWidget);
      // 每行多了一格。
      expect(find.byType(ModelCell), findsNWidgets(10));

      // 时段档格子写回 rule.models；清空则删掉那个槽。
      await tester.enterText(
        find.descendant(
          of: find.byType(ModelCell).at(1),
          matching: find.byType(TextField),
        ),
        'mm',
      );
      await settle(tester);
      expect(f.timeModels.single.models['default'], 'mm');
      await tester.enterText(
        find.descendant(
          of: find.byType(ModelCell).at(1),
          matching: find.byType(TextField),
        ),
        '',
      );
      await settle(tester);
      expect(f.timeModels.single.models.containsKey('default'), isFalse);

      // 单列时上下移都点不动。
      expect(buttonWithLabel(tester, '↑').enabled, isFalse);
      expect(buttonWithLabel(tester, '↓').enabled, isFalse);

      await tester.tap(find.text('+ ${t.t('platform.time_windows_add_rule')}'));
      await settle(tester);
      f.setTimeModels([
        f.timeModels[0].copyWith(models: const {'default': 'a'}),
        f.timeModels[1].copyWith(models: const {'default': 'b'}),
      ]);
      await settle(tester);
      await tester.tap(find.text('↓').first);
      await settle(tester);
      expect(f.timeModels[0].models['default'], 'b');
      await tester.tap(find.text('↑').last);
      await settle(tester);
      expect(f.timeModels[0].models['default'], 'a');

      await tester.tap(find.text('×').first);
      await settle(tester);
      expect(f.timeModels.length, 1);
    });

    testWidgets('列头点开窗口编辑器，确认写回该档', (tester) async {
      await boot(tester);
      await tester.tap(find.text('+ ${t.t('platform.time_windows_add_rule')}'));
      await settle(tester);
      await tester.tap(find.text(t.t('platform.window_never')));
      await settle(tester);
      expect(find.byType(WindowsEditor), findsOneWidget);
      expect(find.text(t.t('platform.windows_edit_empty')), findsOneWidget);

      await tester.tap(find.text('+ ${t.t('platform.windows_edit_add')}'));
      await settle(tester);
      await tester.tap(find.text(t.t('action.confirm')));
      await settle(tester);
      expect(find.byType(WindowsEditor), findsNothing);
      expect(f.timeModels.single.windows.length, 1);
      // 全天 0-24 → 列头描述换成「全天」。
      expect(find.text(t.t('platform.window_all_day')), findsOneWidget);
    });

    testWidgets('窗口编辑器：取消不写回', (tester) async {
      await boot(tester);
      await tester.tap(find.text('+ ${t.t('platform.time_windows_add_rule')}'));
      await settle(tester);
      await tester.tap(find.text(t.t('platform.window_never')));
      await settle(tester);
      await tester.tap(find.text('+ ${t.t('platform.windows_edit_add')}'));
      await settle(tester);
      await tester.tap(
        find.descendant(
          of: find.byType(WindowsEditor),
          matching: find.text(t.t('action.cancel')),
        ),
      );
      await settle(tester);
      expect(find.byType(WindowsEditor), findsNothing);
      expect(f.timeModels.single.windows, isEmpty);
    });

    testWidgets('从高峰时段导入：无高峰点不动；有高峰则确认后加一档', (tester) async {
      await boot(tester);
      expect(
        buttonWithLabel(tester, t.t('platform.time_windows_import_peak')).enabled,
        isFalse,
      );
      f.setPeak(const [TimeWindow(startHour: 9, endHour: 12, multiplier: 2)]);
      await settle(tester);
      await tester.tap(find.text(t.t('platform.time_windows_import_peak')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);
      await tester.tap(
        find.text(t.t('platform.time_windows_import_confirm_button')),
      );
      await settle(tester);
      expect(f.timeModels.single.windows.single.startHour, 9);
      expect(find.byType(ConfirmCard), findsNothing);
    });

    testWidgets('导入确认卡可取消', (tester) async {
      await boot(tester);
      f.setPeak(const [TimeWindow(startHour: 9, endHour: 12, multiplier: 2)]);
      await settle(tester);
      await tester.tap(find.text(t.t('platform.time_windows_import_peak')));
      await settle(tester);
      await tester.tap(
        find.descendant(
          of: find.byType(ConfirmCard),
          matching: find.text(t.t('action.cancel')),
        ),
      );
      await settle(tester);
      expect(find.byType(ConfirmCard), findsNothing);
      expect(f.timeModels, isEmpty);
    });
  });

  group('F10 手动预算', () {
    testWidgets('加一条 → 改 kind/unit/额度 → 切窗口单位 → 开关 → 删', (tester) async {
      // resetForm 落在 openai 上时套餐档位已自动填过一条，这里先清空测空态。
      await boot(tester, protocol: 'anthropic');
      f.setManualBudgets(const []);
      await settle(tester);
      expect(find.text(t.t('platform.manualBudgetEmpty')), findsOneWidget);
      await tester.tap(find.text(t.t('platform.manualBudgetAdd')));
      await settle(tester);
      expect(f.manualBudgets.length, 1);

      // 切到 rolling 且没配过窗口 → 给 7 天默认。
      dropdownWithOption(tester, 'rolling').onChanged!('rolling');
      await settle(tester);
      expect(f.manualBudgets.single.kind, 'rolling');
      expect(f.manualBudgets.single.windowHours, 7);
      expect(f.manualBudgets.single.windowUnit, 'day');

      dropdownWithOption(tester, 'token').onChanged!('token');
      await settle(tester);
      expect(f.manualBudgets.single.unit, 'token');

      await tester.enterText(
        inputOf(fieldWithHint(t.t('platform.manualBudgetAmount'))),
        '12.5',
      );
      await settle(tester);
      expect(f.manualBudgets.single.amount, 12.5);

      final win = fieldWithHint(t.t('platform.manualBudgetWindow'));
      await tester.enterText(inputOf(win), '3');
      await settle(tester);
      expect(f.manualBudgets.single.windowHours, 3);
      await tester.enterText(inputOf(win), '');
      await settle(tester);
      expect(f.manualBudgets.single.windowHours, isNull);

      dropdownWithOption(tester, 'minute').onChanged!('minute');
      await settle(tester);
      expect(f.manualBudgets.single.windowUnit, 'minute');

      await tester.tap(find.text(t.t('platform.manualBudgetEnabled')));
      await settle(tester);
      expect(f.manualBudgets.single.enabled, isFalse);

      // 已经切回 total 才没有窗口输入；这里直接删。
      await tester.tap(
        find.widgetWithText(SmallButton, t.t('action.delete')).last,
      );
      await settle(tester);
      expect(f.manualBudgets, isEmpty);
    });

    testWidgets('有内置档位时「填入内置额度」可用', (tester) async {
      await boot(tester);
      f.setManualBudgets(const []);
      await settle(tester);
      await tester.tap(
        find.text(
          t.t('platform.manualBudgetUseTier', {'tier': 'Pro'}),
        ),
      );
      await settle(tester);
      expect(f.manualBudgets.single.amount, 300);
    });

    testWidgets('kind 停在 total 时没有窗口输入', (tester) async {
      await boot(tester, protocol: 'anthropic');
      f.setManualBudgets(const []);
      await settle(tester);
      await tester.tap(find.text(t.t('platform.manualBudgetAdd')));
      await settle(tester);
      expect(fieldWithHint(t.t('platform.manualBudgetWindow')), findsNothing);
    });
  });

  group('F11 熔断', () {
    testWidgets('三个输入各自写回；placeholder 带全局默认数字', (tester) async {
      final fake = formFake(
        breaker: {
          'default_routing_mode': 'failover',
          'breaker_failure_threshold': 5,
          'breaker_open_secs': 60,
          'breaker_half_open_max': 2,
          'enabled': true,
        },
      );
      await boot(tester, edit: true, fake: fake);
      expect(
        find.text(t.t('platform.breakerInherit', {'n': 5})),
        findsOneWidget,
      );
      await tester.enterText(
        inputOf(fieldWithLabel(t.t('platform.breakerFailureThreshold'))),
        '9',
      );
      await settle(tester);
      expect(f.breakerFailureThreshold, '9');
      await tester.enterText(
        inputOf(fieldWithLabel(t.t('platform.breakerOpenSecs'))),
        '30',
      );
      await settle(tester);
      expect(f.breakerOpenSecs, '30');
      await tester.enterText(
        inputOf(fieldWithLabel(t.t('platform.breakerHalfOpenMax'))),
        '1',
      );
      await settle(tester);
      expect(f.breakerHalfOpenMax, '1');
    });

    testWidgets('读不到全局默认 → placeholder 退到不带数字那句', (tester) async {
      await boot(tester, edit: true);
      expect(
        find.text(t.t('platform.breakerInheritGeneric')),
        findsNWidgets(3),
      );
    });
  });

  group('F12 高峰窗口', () {
    testWidgets('加窗口 → 改起止 / 倍率 / 时区 / 周几 → 删', (tester) async {
      await boot(tester, edit: true);
      expect(find.text(t.t('platform.peak_empty')), findsOneWidget);
      await tester.tap(find.text('+ ${t.t('platform.add_window')}'));
      await settle(tester);
      expect(f.peak.length, 1);

      // UTC 模式下输入值 = 存储值，断言不受本机时区影响。
      f.setWindowsTz(TzMode.utc);
      await settle(tester);

      final nums = find.descendant(
        of: find.byType(PlatformField),
        matching: find.byType(TextField),
      );
      // 窗口行的前四个数字框依次是 起时 / 起分 / 止时 / 止分。
      Finder boxAt(int i) => find.byWidgetPredicate(
        (w) => w is PlatformField && w.hint == null && w.label == null,
      ).at(i);
      expect(nums, findsWidgets);

      await tester.enterText(inputOf(boxAt(0)), '9');
      await settle(tester);
      expect(f.peak.single.startHour, 9);
      await tester.enterText(inputOf(boxAt(1)), '30');
      await settle(tester);
      expect(f.peak.single.startMinute, 30);
      await tester.enterText(inputOf(boxAt(2)), '18');
      await settle(tester);
      expect(f.peak.single.endHour, 18);
      await tester.enterText(inputOf(boxAt(3)), '45');
      await settle(tester);
      expect(f.peak.single.endMinute, 45);
      // 超界按 clampInt 钳。
      await tester.enterText(inputOf(boxAt(0)), '99');
      await settle(tester);
      expect(f.peak.single.startHour, 23);
      await tester.enterText(inputOf(boxAt(0)), 'abc');
      await settle(tester);
      expect(f.peak.single.startHour, 0);

      await tester.enterText(inputOf(boxAt(4)), '2.5');
      await settle(tester);
      expect(f.peak.single.multiplier, 2.5);

      dropdownWithOption(tester, 'Asia/Shanghai').onChanged!('Asia/Shanghai');
      await settle(tester);
      expect(f.peak.single.timezone, 'Asia/Shanghai');
      dropdownWithOption(tester, 'Asia/Shanghai').onChanged!('__utc__');
      await settle(tester);
      expect(f.peak.single.timezone, isNull);

      // 周几：点亮再点灭，灭光了要回到 null 而不是空数组。
      await tester.tap(find.widgetWithText(SmallButton, 'M').first);
      await settle(tester);
      expect(f.peak.single.daysOfWeek, [1]);
      await tester.tap(find.widgetWithText(SmallButton, 'M').first);
      await settle(tester);
      expect(f.peak.single.daysOfWeek, isNull);

      await tester.tap(find.widgetWithText(SmallButton, '✕').first);
      await settle(tester);
      expect(f.peak, isEmpty);
    });

    testWidgets('受影响模型：输入回车成 chip，再点 chip 删掉', (tester) async {
      await boot(tester, edit: true);
      await tester.tap(find.text('+ ${t.t('platform.add_window')}'));
      await settle(tester);
      expect(find.text(' · ${t.t('platform.peak_model_scope_all')}'),
          findsOneWidget);

      final scope = find.byWidgetPredicate(
        (w) => w is TextField && w.decoration?.hintText ==
            t.t('platform.peak_model_placeholder'),
      );
      await tester.enterText(scope, 'glm-5.2*');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      expect(f.peak.single.models, ['glm-5.2*']);

      await tester.tap(find.widgetWithText(SmallButton, 'glm-5.2* ✕'));
      await settle(tester);
      expect(f.peak.single.models, isNull);
    });

    testWidgets('生效期两个输入：写入 Unix 秒，清空回 null', (tester) async {
      await boot(tester, edit: true);
      await tester.tap(find.text('+ ${t.t('platform.add_window')}'));
      await settle(tester);
      final startAt = fieldWithLabel(t.t('platform.peak_start_at'));
      await tester.enterText(inputOf(startAt), '2026-01-05T10:00');
      await settle(tester);
      expect(
        f.peak.single.startAt,
        DateTime.parse('2026-01-05T10:00').millisecondsSinceEpoch ~/ 1000,
      );
      await tester.enterText(inputOf(startAt), '');
      await settle(tester);
      expect(f.peak.single.startAt, isNull);

      final endAt = fieldWithLabel(t.t('platform.peak_end_at'));
      await tester.enterText(inputOf(endAt), '2026-02-05T10:00');
      await settle(tester);
      expect(f.peak.single.endAt, isNotNull);
      await tester.enterText(inputOf(endAt), 'nope');
      await settle(tester);
      expect(f.peak.single.endAt, isNull);
    });

    testWidgets('时区模式两个按钮切换', (tester) async {
      await boot(tester, edit: true);
      await tester.tap(find.text(t.t('platform.timezone_utc')).first);
      await settle(tester);
      expect(f.windowsTz, TzMode.utc);
      await tester.tap(find.text(t.t('platform.timezone_local')).first);
      await settle(tester);
      expect(f.windowsTz, TzMode.local);
    });

    testWidgets('导入默认高峰配置：确认后覆盖，取消则不动', (tester) async {
      // anthropic 在夹具里带 preset peak。
      final fake = formFake(platforms: [plat(1, 'a', type: 'anthropic')]);
      await boot(tester, edit: true, fake: fake);
      await tester.tap(find.text(t.t('platform.peak_import_default')));
      await settle(tester);
      await tester.tap(
        find.descendant(
          of: find.byType(ConfirmCard),
          matching: find.text(t.t('action.cancel')),
        ),
      );
      await settle(tester);
      expect(f.peak, isEmpty);

      await tester.tap(find.text(t.t('platform.peak_import_default')));
      await settle(tester);
      await tester.tap(
        find.text(t.t('platform.peak_overwrite_confirm_button')),
      );
      await settle(tester);
      expect(f.peak.single.startHour, 9);
      expect(f.peak.single.multiplier, 2);
    });
  });

  group('F13 分组归属', () {
    Future<void> bootWithGroups(WidgetTester tester, {bool edit = false}) =>
        boot(
          tester,
          edit: edit,
          fake: formFake(
            groups: [
              {
                'group': {
                  'id': 1,
                  'name': 'G1',
                  'group_key': 'g1',
                  'auto_from_platform': '',
                },
                'platforms': <Object?>[],
              },
            ],
          ),
        );

    testWidgets('创建态有「创建默认分组」开关，可关', (tester) async {
      await bootWithGroups(tester);
      expect(find.text(t.t('platform.groupAssignAuto')), findsOneWidget);
      await tester.tap(find.byType(Switch).first);
      await settle(tester);
      expect(f.autoGroup, isFalse);
    });

    testWidgets('分组 chip 可加可减', (tester) async {
      await bootWithGroups(tester);
      await tester.tap(find.widgetWithText(SmallButton, 'G1'));
      await settle(tester);
      expect(f.joinGroupIds, [1]);
      await tester.tap(find.widgetWithText(SmallButton, 'G1'));
      await settle(tester);
      expect(f.joinGroupIds, isEmpty);
    });

    testWidgets('编辑态没有「创建默认分组」开关', (tester) async {
      await bootWithGroups(tester, edit: true);
      expect(find.text(t.t('platform.groupAssignAuto')), findsNothing);
      expect(find.widgetWithText(SmallButton, 'G1'), findsOneWidget);
    });

    testWidgets('锁定分组：只显示那一个组名 + 「已锁定」，没有 chip 可选', (tester) async {
      await boot(tester);
      f.openCreatePlatform(lockGid: 1);
      await settle(tester);
      expect(find.text(t.t('platform.groupLocked')), findsOneWidget);
      expect(find.text(t.t('platform.groupAssignJoin')), findsNothing);
    });
  });

  group('F14 过期时间', () {
    testWidgets('开关 → 填日期 → 清空 → 关开关', (tester) async {
      await boot(tester);
      expect(fieldWithHint('YYYY-MM-DDTHH:MM'), findsNothing);
      await tester.tap(find.byType(Switch).last);
      await settle(tester);
      expect(f.expiryEnabled, isTrue);

      final fin = fieldWithHint('YYYY-MM-DDTHH:MM');
      await tester.enterText(inputOf(fin), '2099-01-02T03:04');
      await settle(tester);
      expect(
        f.expiresAt,
        DateTime.parse('2099-01-02T03:04').millisecondsSinceEpoch,
      );
      // 未来日期且不在 24h 内 → 显示格式化时间，不是「已过期」。
      expect(find.text(t.t('platform.expired')), findsNothing);

      await tester.tap(find.text(t.t('platform.expiresAtClear')));
      await settle(tester);
      expect(f.expiresAt, 0);

      // 非法串不改值。
      await tester.enterText(inputOf(fin), 'nope');
      await settle(tester);
      expect(f.expiresAt, 0);

      await tester.tap(find.byType(Switch).last);
      await settle(tester);
      expect(f.expiryEnabled, isFalse);
    });

    testWidgets('已过期的时间显示「已过期」', (tester) async {
      await boot(tester);
      f.setExpiryEnabled(true);
      f.setExpiresAt(1);
      await settle(tester);
      expect(find.text(t.t('platform.expired')), findsOneWidget);
    });

    testWidgets('24 小时内到期显示「临近过期」', (tester) async {
      await boot(tester);
      f.setExpiryEnabled(true);
      final soon =
          DateTime.now().add(const Duration(hours: 1)).millisecondsSinceEpoch;
      f.setExpiresAt(soon);
      await settle(tester);
      expect(
        find.textContaining(
          t.t('platform.expiresAtSoon', {'time': ''}).split('{')[0],
        ),
        findsOneWidget,
      );
    });
  });

  group('F15 窗口编辑器内部控件', () {
    /// 直接挂编辑器：它在表单里要先加一档再点列头，那条路径已经由
    /// `platform_form_widget_test.dart` 覆盖，这里只盯控件接线。
    Future<Future<List<TimeWindow>> Function()> pumpEditor(
      WidgetTester tester,
      I18nController i18n,
      List<TimeWindow> initial,
    ) async {
      var saved = <TimeWindow>[];
      await tester.pumpWidget(
        wrapPage(
          WindowsEditor(
            initial: initial,
            tzMode: TzMode.utc,
            onTzMode: (_) {},
            onSave: (w) => saved = w,
            onCancel: () {},
          ),
          i18n,
        ),
      );
      await settle(tester);
      return () async {
        await tester.tap(find.text(i18n.t('action.confirm')));
        await settle(tester);
        return saved;
      };
    }

    testWidgets('起止时分 / 时区 / 周几 / 每月几日 全都写进窗口', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final save = await pumpEditor(tester, i18n, [
        const TimeWindow(startHour: 0, endHour: 24, multiplier: 1),
      ]);
      await settle(tester);

      Finder boxAt(int i) => find
          .byWidgetPredicate(
            (w) => w is PlatformField && w.hint == null && w.label == null,
          )
          .at(i);
      await tester.enterText(inputOf(boxAt(0)), '9');
      await tester.enterText(inputOf(boxAt(1)), '15');
      await tester.enterText(inputOf(boxAt(2)), '18');
      await tester.enterText(inputOf(boxAt(3)), '45');
      await settle(tester);

      // 时区下拉
      dropdownWithOption(tester, 'Europe/Zurich').onChanged!('Europe/Zurich');
      await settle(tester);
      dropdownWithOption(tester, 'Europe/Zurich').onChanged!('__utc__');
      await settle(tester);

      // 周几：点亮周一
      await tester.tap(find.byKey(const ValueKey('dim-0-week')));
      await settle(tester);
      await tester.tap(find.widgetWithText(SmallButton, 'M').first);
      await settle(tester);

      var out = await save();
      expect(out.single.startHour, 9);
      expect(out.single.startMinute, 15);
      expect(out.single.endHour, 18);
      expect(out.single.endMinute, 45);
      expect(out.single.timezone, isNull);
      expect(out.single.daysOfWeek, [1]);

      // 切到「每月几日」：周几被清掉，月份日可点可灭
      await tester.tap(find.byKey(const ValueKey('dim-0-month')));
      await settle(tester);
      await tester.tap(find.widgetWithText(SmallButton, '5'));
      await settle(tester);
      out = await save();
      expect(out.single.daysOfWeek, isNull);
      expect(out.single.daysOfMonth, [5]);

      await tester.tap(find.widgetWithText(SmallButton, '5'));
      await settle(tester);
      out = await save();
      expect(out.single.daysOfMonth, isNull);
    });

    testWidgets('跨天窗口显示「次日」；multiplier<=0 保存时补成 1.0', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final save = await pumpEditor(tester, i18n, [
        const TimeWindow(startHour: 22, endHour: 6, multiplier: 0),
      ]);
      await settle(tester);
      expect(
        find.text('（${i18n.t('platform.peak_next_day')}）'),
        findsOneWidget,
      );
      final out = await save();
      expect(out.single.multiplier, 1.0);
    });

    testWidgets('本地时区按钮可点', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      var mode = TzMode.utc;
      await tester.pumpWidget(
        wrapPage(
          WindowsEditor(
            initial: const [],
            tzMode: TzMode.utc,
            onTzMode: (m) => mode = m,
            onSave: (_) {},
            onCancel: () {},
          ),
          i18n,
        ),
      );
      await settle(tester);
      await tester.tap(find.text(i18n.t('platform.timezone_local')));
      await settle(tester);
      expect(mode, TzMode.local);
    });
  });

  group('窗口描述与预览', () {
    testWidgets('describeWindows：全天 / 时分精度 / 周几 / 每月几日 / 多窗口 +N',
        (tester) async {
      final i18n = await makeI18n(tester);
      expect(
        describeWindows(
          const [TimeWindow(startHour: 0, endHour: 24, multiplier: 1)],
          TzMode.utc,
          i18n,
        ),
        i18n.t('platform.window_all_day'),
      );
      expect(
        describeWindows(const [], TzMode.utc, i18n),
        i18n.t('platform.window_never'),
      );
      expect(
        describeWindows(
          const [TimeWindow(startHour: 9, endHour: 12, multiplier: 1)],
          TzMode.utc,
          i18n,
        ),
        '9-12（${i18n.t('platform.timezone_utc')}）',
      );
      expect(
        describeWindows(
          const [
            TimeWindow(
              startHour: 9,
              endHour: 12,
              multiplier: 1,
              startMinute: 30,
            ),
          ],
          TzMode.utc,
          i18n,
        ),
        '09:30-12:00（${i18n.t('platform.timezone_utc')}）',
      );
      expect(
        describeWindows(
          const [
            TimeWindow(
              startHour: 9,
              endHour: 12,
              multiplier: 1,
              daysOfWeek: [1, 3],
            ),
          ],
          TzMode.utc,
          i18n,
        ),
        '9-12(${i18n.t('platform.weekday_short.1')},'
        '${i18n.t('platform.weekday_short.3')})'
        '（${i18n.t('platform.timezone_utc')}）',
      );
      expect(
        describeWindows(
          const [
            TimeWindow(
              startHour: 9,
              endHour: 12,
              multiplier: 1,
              daysOfMonth: [1, 5],
            ),
          ],
          TzMode.utc,
          i18n,
        ),
        '9-12(每月1,5日)（${i18n.t('platform.timezone_utc')}）',
      );
      expect(
        describeWindows(
          const [
            TimeWindow(startHour: 9, endHour: 12, multiplier: 1),
            TimeWindow(startHour: 1, endHour: 2, multiplier: 1),
            TimeWindow(startHour: 3, endHour: 4, multiplier: 1),
          ],
          TzMode.utc,
          i18n,
        ),
        '9-12（${i18n.t('platform.timezone_utc')}）+2',
      );
      // 本地模式的标签换成「本地」。
      expect(
        describeWindows(
          const [TimeWindow(startHour: 0, endHour: 1, multiplier: 1)],
          TzMode.local,
          i18n,
        ).contains(i18n.t('platform.timezone_local')),
        isTrue,
      );
    });

    testWidgets('formatWindowPreview：半开区间右端显示 end-1:59；带时区直接显示 tz 名',
        (tester) async {
      final i18n = await makeI18n(tester);
      expect(
        formatWindowPreview(
          const TimeWindow(startHour: 9, endHour: 12, multiplier: 1),
          TzMode.utc,
          i18n,
        ),
        '09:00:00 - 11:59:59（${i18n.t('platform.timezone_utc')}）',
      );
      // 0:00 结束 → 跨天边界回到 23:59。
      expect(
        formatWindowPreview(
          const TimeWindow(startHour: 22, endHour: 0, multiplier: 1),
          TzMode.utc,
          i18n,
        ),
        '22:00:00 - 23:59:59（${i18n.t('platform.timezone_utc')}）'
        '（${i18n.t('platform.peak_next_day')}）',
      );
      // 带 timezone：存值即本地值，标签直接是 IANA 名；跨天补「次日」。
      expect(
        formatWindowPreview(
          const TimeWindow(
            startHour: 22,
            endHour: 6,
            multiplier: 1,
            timezone: 'Asia/Shanghai',
          ),
          TzMode.utc,
          i18n,
        ),
        '22:00:00 - 05:59:59（Asia/Shanghai）'
        '（${i18n.t('platform.peak_next_day')}）',
      );
      expect(
        formatWindowPreview(
          const TimeWindow(startHour: 0, endHour: 1, multiplier: 1),
          TzMode.local,
          i18n,
        ).contains(i18n.t('platform.timezone_local')),
        isTrue,
      );
    });
  });

  group('纯格式化函数', () {
    test('toDatetimeLocal / datetimeLocalToMs 互为逆', () {
      const s = '2026-01-05T10:00';
      final ms = datetimeLocalToMs(s)!;
      expect(toDatetimeLocal(ms), s);
      expect(datetimeLocalToMs(''), isNull);
      expect(datetimeLocalToMs('  '), isNull);
      expect(datetimeLocalToMs('nope'), isNull);
      expect(secToLocalInput(null), '');
      expect(secToLocalInput(0), '');
      expect(secToLocalInput(-5), '');
      expect(localInputToSec(''), isNull);
      expect(localInputToSec(s), ms ~/ 1000);
    });

    test('clampInt 裁边界，非法回落 min', () {
      expect(clampInt('5', 0, 23), 5);
      expect(clampInt('-1', 0, 23), 0);
      expect(clampInt('99', 0, 23), 23);
      expect(clampInt('x', 0, 23), 0);
      expect(clampInt('9.6', 0, 23), 10);
    });

    test('maskTail 只露尾 4 位，短 key 原样', () {
      expect(maskTail('abcdefgh'), '••••efgh');
      expect(maskTail('abcd'), 'abcd');
      expect(maskTail('ab'), 'ab');
    });

    test('dimensionOf 三态', () {
      expect(
        dimensionOf(const TimeWindow(startHour: 0, endHour: 1, multiplier: 1)),
        WindowDimension.none,
      );
      expect(
        dimensionOf(
          const TimeWindow(
            startHour: 0,
            endHour: 1,
            multiplier: 1,
            daysOfWeek: [1],
          ),
        ),
        WindowDimension.week,
      );
      expect(
        dimensionOf(
          const TimeWindow(
            startHour: 0,
            endHour: 1,
            multiplier: 1,
            daysOfMonth: [1],
          ),
        ),
        WindowDimension.month,
      );
      // 空数组不算选了维度。
      expect(
        dimensionOf(
          const TimeWindow(
            startHour: 0,
            endHour: 1,
            multiplier: 1,
            daysOfWeek: [],
          ),
        ),
        WindowDimension.none,
      );
    });
  });
}
