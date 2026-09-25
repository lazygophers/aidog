/// 平台编辑表单的 widget 测试。
///
/// 两组来源：
/// 1. `src/pages/platforms/WindowsEditModal.test.tsx` 的 6 条逐条翻译
///    （**数据与期望值一字不改**，只把「查 DOM 节点」换成「查 widget」）；
/// 2. 表单本身的分区显隐、禁用态、页头两个入口（缺口清单 #2 / #3）。
///
/// 两个坑沿用 I06/I07 的（别再踩）：不用 `pumpAndSettle`（骨架有无限呼吸动画），
/// 交互测试先 `useBigSurface`。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/src/pages/platform_extra.dart';
import 'package:aidog_flutter/src/pages/platform_form.dart';
import 'package:aidog_flutter/src/pages/platform_form_bits.dart';
import 'package:aidog_flutter/src/pages/platform_form_logic.dart';
import 'package:aidog_flutter/src/pages/time_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';
import 'harness.dart';
import 'platform_form_logic_test.dart'
    show kClientTypesJson, kDefaultsJson, formFake, plat;

// ── WindowsEditModal 的 6 条翻译 ──────────────────────────────────────

/// 维度 radio：`dim-<widx>-<none|week|month>` 这个 key 上的单选项，
/// 圆点画成实心（`radio_button_checked`）就是 React 的 `data-state="checked"`。
bool dimSelected(WidgetTester tester, int widx, String dim) =>
    tester
        .widget<Icon>(
          find.descendant(
            of: find.byKey(ValueKey('dim-$widx-$dim')),
            matching: find.byType(Icon),
          ),
        )
        .icon ==
    Icons.radio_button_checked;

/// 周几 toggle 组（React 里是 7 个带 `title=platform.weekday_short.N` 的按钮）。
Finder weekGroups() => find.byType(WeekdayToggles);

/// 数字网格（每月几日 1-31）按钮：纯数字文本、不在周几组里。
int monthButtonCount(WidgetTester tester) {
  var n = 0;
  for (final w in tester.widgetList<SmallButton>(find.byType(SmallButton))) {
    final v = int.tryParse(w.label);
    if (v != null && v >= 1 && v <= 31) n++;
  }
  return n;
}

Future<void> pumpWindowsEditor(
  WidgetTester tester,
  I18nController i18n,
  List<TimeWindow> windows, {
  required void Function(List<TimeWindow>) onSave,
  VoidCallback? onCancel,
}) async {
  await tester.pumpWidget(
    wrapPage(
      WindowsEditor(
        initial: windows,
        tzMode: TzMode.utc,
        onTzMode: (_) {},
        onSave: onSave,
        onCancel: onCancel ?? () {},
      ),
      i18n,
    ),
  );
  await settle(tester);
}

void main() {
  group('WindowsEditModal（React: WindowsEditModal.test.tsx 逐条翻译）', () {
    testWidgets('切「周几」: 七个星期 toggle 出现，选中态不弹回', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await pumpWindowsEditor(tester, i18n, const [
        TimeWindow(startHour: 0, endHour: 24, multiplier: 1),
      ], onSave: (_) {});
      await tester.tap(find.byKey(const ValueKey('dim-0-week')));
      await settle(tester);
      expect(dimSelected(tester, 0, 'week'), isTrue);
      expect(weekGroups(), findsOneWidget);
      expect(monthButtonCount(tester), 0);
    });

    testWidgets('切回「每天」: 两个选择器收起, 保存后 days_of_week/days_of_month 均 undefined', (
      tester,
    ) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      List<TimeWindow>? saved;
      await pumpWindowsEditor(tester, i18n, const [
        TimeWindow(
          startHour: 0,
          endHour: 24,
          multiplier: 1,
          daysOfWeek: [1, 2],
        ),
      ], onSave: (w) => saved = w);
      await tester.tap(find.byKey(const ValueKey('dim-0-none')));
      await settle(tester);
      expect(weekGroups(), findsNothing);
      expect(monthButtonCount(tester), 0);

      await tester.tap(find.text(i18n.t('action.confirm')));
      await settle(tester);
      expect(saved, isNotNull);
      expect(saved!.single.daysOfWeek, isNull);
      expect(saved!.single.daysOfMonth, isNull);
    });

    testWidgets('选「周几」但一天不勾: 保存后不产生空数组脏数据, 仍是 undefined', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      List<TimeWindow>? saved;
      await pumpWindowsEditor(tester, i18n, const [
        TimeWindow(startHour: 0, endHour: 24, multiplier: 1),
      ], onSave: (w) => saved = w);
      await tester.tap(find.byKey(const ValueKey('dim-0-week')));
      await settle(tester);
      expect(weekGroups(), findsOneWidget); // 选择器已露出、未勾选任何一天

      await tester.tap(find.text(i18n.t('action.confirm')));
      await settle(tester);
      expect(saved!.single.daysOfWeek, isNull);
      expect(saved!.single.daysOfMonth, isNull);
    });

    testWidgets('多窗口: 切其中一个的维度不影响其他窗口', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await pumpWindowsEditor(tester, i18n, const [
        TimeWindow(startHour: 0, endHour: 24, multiplier: 1),
        TimeWindow(startHour: 0, endHour: 24, multiplier: 1),
      ], onSave: (_) {});
      await tester.tap(find.byKey(const ValueKey('dim-0-week')));
      await settle(tester);

      expect(dimSelected(tester, 0, 'week'), isTrue);
      // window 1 仍是初始「无」选中态，未被 window 0 的切换影响
      expect(dimSelected(tester, 1, 'none'), isTrue);
      expect(dimSelected(tester, 1, 'week'), isFalse);
      // 只有 window 0 露出了周几选择器（一组），不是两组
      expect(weekGroups(), findsOneWidget);
    });

    testWidgets('关闭再重开: uiDim 按 windows 数据重算, 周/月/都无三种形态选中态都对', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      const windows = [
        TimeWindow(
          startHour: 0,
          endHour: 24,
          multiplier: 1,
          daysOfWeek: [1, 2],
        ),
        TimeWindow(startHour: 0, endHour: 24, multiplier: 1, daysOfMonth: [5]),
        TimeWindow(startHour: 0, endHour: 24, multiplier: 1),
      ];
      // 「关闭再重开」在页内格子的形态下 = 整个编辑器被销毁后重建
      // （`platform_form.dart` 给它挂了 `ValueKey('windows-<idx>')`）。
      await pumpWindowsEditor(tester, i18n, windows, onSave: (_) {});
      await tester.pumpWidget(wrapPage(const SizedBox.shrink(), i18n));
      await settle(tester);
      await pumpWindowsEditor(tester, i18n, windows, onSave: (_) {});

      expect(dimSelected(tester, 0, 'week'), isTrue);
      expect(dimSelected(tester, 1, 'month'), isTrue);
      expect(dimSelected(tester, 2, 'none'), isTrue);
    });

    testWidgets('删中间窗口: 剩余窗口的选中态各自不变（不因数组前移而错位）', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await pumpWindowsEditor(tester, i18n, const [
        TimeWindow(startHour: 0, endHour: 24, multiplier: 1), // widx0: none
        TimeWindow(
          startHour: 0,
          endHour: 24,
          multiplier: 1,
          daysOfWeek: [3],
        ), // widx1: week
        TimeWindow(
          startHour: 0,
          endHour: 24,
          multiplier: 1,
          daysOfMonth: [9],
        ), // widx2: month
      ], onSave: (_) {});
      // 删除按钮以符号 "×" 标识（非文案，跨语言不变），结构上按序对应各窗口。
      final removes = find.widgetWithText(SmallButton, '×');
      expect(removes, findsNWidgets(3));
      await tester.tap(removes.at(1));
      await settle(tester);

      // 原 widx0(none) 仍是 dim-0-none，原 widx2(month) 现变成 dim-1-month
      expect(dimSelected(tester, 0, 'none'), isTrue);
      expect(dimSelected(tester, 1, 'month'), isTrue);
      // 不应错位成 dim-1-none 被选中
      expect(dimSelected(tester, 1, 'none'), isFalse);
      expect(monthButtonCount(tester), 31);
      expect(weekGroups(), findsNothing);
    });
  });

  // ── 表单本体 ────────────────────────────────────────────────────

  Future<PlatformFormController> bootForm(
    WidgetTester tester,
    FakeInvoke k,
  ) async {
    final list = PlatformsController(invoke: k.fn);
    final form = PlatformFormController(list: list, invoke: k.fn);
    await tester.runAsync(() async {
      await list.init();
      await form.init(locale: 'zh-Hans');
    });
    return form;
  }

  Future<void> pumpForm(
    WidgetTester tester,
    I18nController i18n,
    PlatformFormController form,
  ) async {
    await tester.pumpWidget(
      wrapPage(
        StatefulBuilder(
          builder: (context, setState) {
            form.onChanged = () => setState(() {});
            return PlatformEditForm(controller: form);
          },
        ),
        i18n,
      ),
    );
    await settle(tester);
  }

  group('分区显隐（PlatformEditForm.tsx 的条件渲染）', () {
    testWidgets('创建态 openai：端点 / 认证 / 模型在，熔断与高峰不在', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      await pumpForm(tester, i18n, f);

      expect(find.text(i18n.t('platform.sectionBasic')), findsOneWidget);
      expect(find.text(i18n.t('platform.endpoints')), findsOneWidget);
      expect(find.text(i18n.t('platform.sectionAuth')), findsOneWidget);
      expect(find.text(i18n.t('platform.models')), findsOneWidget);
      expect(find.text(i18n.t('platform.manualBudgetTitle')), findsOneWidget);
      expect(find.text(i18n.t('platform.groupAssignTitle')), findsOneWidget);
      expect(find.text(i18n.t('platform.expiresAt')), findsOneWidget);
      // 熔断与高峰只在编辑态出现。
      expect(find.text(i18n.t('platform.breakerTitle')), findsNothing);
      expect(find.text(i18n.t('platform.peak')), findsNothing);
      // 协议未锁定 → 有选择器。
      expect(find.byType(ProtocolPicker), findsOneWidget);
    });

    testWidgets('编辑态：协议锁定 + 熔断 + 高峰都在', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final k = formFake();
      final f = await bootForm(tester, k);
      f.handleEdit(f.list.platforms.first);
      await pumpForm(tester, i18n, f);

      expect(find.byType(ProtocolPicker), findsNothing);
      expect(find.text(i18n.t('platform.protocolLocked')), findsOneWidget);
      expect(find.text(i18n.t('platform.breakerTitle')), findsOneWidget);
      expect(find.text(i18n.t('platform.peak')), findsOneWidget);
    });

    testWidgets('mock 平台：只出特例配置，端点/认证/模型整块不渲染', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('mock');
      await pumpForm(tester, i18n, f);

      expect(find.text(i18n.t('platform.sectionSpecial')), findsOneWidget);
      expect(find.text(i18n.t('platform.endpoints')), findsNothing);
      expect(find.text(i18n.t('platform.sectionAuth')), findsNothing);
      expect(find.text(i18n.t('platform.models')), findsNothing);
      expect(find.text(i18n.t('platform.quotaScript.title')), findsNothing);
    });

    testWidgets('透传平台：只出透传配置，手动预算与分组归属不渲染', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('claude_code');
      await pumpForm(tester, i18n, f);

      expect(find.text(i18n.t('platform.sectionPassthrough')), findsOneWidget);
      expect(find.text(i18n.t('platform.manualBudgetTitle')), findsNothing);
      expect(find.text(i18n.t('platform.groupAssignTitle')), findsNothing);
      expect(find.text(i18n.t('platform.endpoints')), findsNothing);
    });

    testWidgets('devin 平台才出 Devin 配置区', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      await pumpForm(tester, i18n, f);
      expect(find.text(i18n.t('platform.devinConfig')), findsNothing);

      f.handleProtocolChange('devin');
      await settle(tester);
      expect(find.text(i18n.t('platform.devinConfig')), findsOneWidget);
    });

    testWidgets('newapi 才出「用户 ID」输入', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('newapi');
      await pumpForm(tester, i18n, f);
      expect(find.text(i18n.t('platform.newapiUserId')), findsOneWidget);
    });
  });

  group('F3 配额脚本分区（QuotaScriptSection.test.tsx 逐条翻译）', () {
    testWidgets('registry 无内置变体：仍渲染区块并直接给出自定义脚本编辑器', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('devin'); // 夹具里 devin 没有 quota_scripts
      await pumpForm(tester, i18n, f);
      expect(find.text(i18n.t('platform.quotaScript.title')), findsOneWidget);
      expect(
        find.text(i18n.t('platform.quotaScript.noBuiltin')),
        findsOneWidget,
      );
      expect(
        find.text(i18n.t('platform.quotaScript.customLabel')),
        findsOneWidget,
      );
    });

    testWidgets('有内置变体且未选自定义：显示变体下拉，不显示脚本编辑器', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      await pumpForm(tester, i18n, f);
      expect(find.text(i18n.t('platform.quotaScript.variant')), findsOneWidget);
      expect(
        find.text(i18n.t('platform.quotaScript.customLabel')),
        findsNothing,
      );
    });

    testWidgets('有内置变体但选中自定义：显示脚本编辑器', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      f.handleQuotaVariantChange(kQuotaCustomVariant);
      await pumpForm(tester, i18n, f);
      expect(
        find.text(i18n.t('platform.quotaScript.customLabel')),
        findsOneWidget,
      );
    });
  });

  group('F16 保存按钮', () {
    testWidgets('条件不满足时点不动，满足后可点', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      f.setName('');
      await pumpForm(tester, i18n, f);

      SmallButton saveBtn() => tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, i18n.t('action.create')),
      );
      expect(saveBtn().enabled, isFalse);
      f.setName('n');
      await settle(tester);
      expect(saveBtn().enabled, isFalse); // 还缺 key
      f.setApiKey('sk');
      await settle(tester);
      expect(saveBtn().enabled, isTrue);
    });

    testWidgets('多 key 时按钮文案换成「批量创建（N）」', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      f.setName('P');
      f.setApiKey('aaaa1111 bbbb2222');
      await pumpForm(tester, i18n, f);
      expect(
        find.text(i18n.t('platform.batch.createN', {'n': 2})),
        findsOneWidget,
      );
      expect(find.text(i18n.t('action.create')), findsNothing);
      // F8：预览卡列出两行，name 带尾 4 位。
      expect(find.text('P-1111'), findsOneWidget);
      expect(find.text('P-2222'), findsOneWidget);
      expect(find.text('••••1111'), findsOneWidget);
    });

    testWidgets('编辑态按钮文案是「保存」', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.handleEdit(f.list.platforms.first);
      await pumpForm(tester, i18n, f);
      expect(
        find.widgetWithText(SmallButton, i18n.t('action.save')),
        findsOneWidget,
      );
    });
  });

  group('F17 保存错误提示', () {
    testWidgets('保存失败 → 底部错误条出现且表单不关', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final k = formFake();
      k.errors['platform_create'] = StateError('boom');
      final f = await bootForm(tester, k);
      f.openCreatePlatform();
      f.handleProtocolChange('openai');
      f.setName('n');
      f.setApiKey('sk');
      await pumpForm(tester, i18n, f);
      expect(find.byType(ToastBar), findsNothing);

      await tester.tap(
        find.widgetWithText(SmallButton, i18n.t('action.create')),
      );
      await settle(tester);
      expect(f.showForm, isTrue);
      expect(find.byType(ToastBar), findsOneWidget);
      expect(find.textContaining('boom'), findsOneWidget);
    });
  });

  group('F12 高峰分区的实时态', () {
    testWidgets('开关关着不显实时态；打开后显示「当前：高峰/非高峰」', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.handleEdit(f.list.platforms.first);
      await pumpForm(tester, i18n, f);
      expect(find.text(i18n.t('platform.currently_peak')), findsNothing);
      expect(find.text(i18n.t('platform.currently_off_peak')), findsNothing);

      f.setDisableDuringPeak(true);
      await settle(tester);
      // 没配窗口 → 一定是非高峰。
      expect(find.text(i18n.t('platform.currently_off_peak')), findsOneWidget);

      f.setPeak(const [TimeWindow(startHour: 0, endHour: 24, multiplier: 2)]);
      await settle(tester);
      expect(find.text(i18n.t('platform.currently_peak')), findsOneWidget);
    });

    testWidgets('无 preset 默认高峰时「导入默认配置」点不动', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final f = await bootForm(tester, formFake());
      f.handleEdit(f.list.platforms.first); // openai：夹具里没有 peak
      await pumpForm(tester, i18n, f);
      expect(
        tester
            .widget<SmallButton>(
              find.widgetWithText(
                SmallButton,
                i18n.t('platform.peak_import_default'),
              ),
            )
            .enabled,
        isFalse,
      );
    });
  });

  group('缺口 #2 / #3：页头「+ 添加平台」与卡片「编辑」', () {
    FakeInvoke pageFake() => FakeInvoke({
      'platform_list': [plat(1, 'P1')],
      'group_detail_list': <Object?>[],
      'all_platform_usage_stats': <String, Object?>{},
      'get_last_test_result': null,
      'scheduling_settings_get': null,
      'get_defaults_json': kDefaultsJson,
      'get_client_types_json': kClientTypesJson,
      'platform_query_quota': {'success': false, 'queried_at': 0},
      'platform_usage_stats': null,
      'platform_create': plat(9, 'new'),
      'platform_update': plat(1, 'P1'),
    });

    testWidgets('页头按钮打开创建表单', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final k = pageFake();
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream.empty(),
          ),
          i18n,
        ),
      );
      await settle(tester);

      await tester.tap(find.text('+ ${i18n.t('platform.add')}'));
      await settle(tester);
      // 表单整页接管：基础信息区出现，页头搜索框不见了。
      expect(find.text(i18n.t('platform.sectionBasic')), findsOneWidget);
      expect(find.text(i18n.t('platform.add')), findsOneWidget);

      // 「返回」把表单收掉。
      await tester.tap(find.text('← ${i18n.t('action.back')}'));
      await settle(tester);
      expect(find.text(i18n.t('platform.sectionBasic')), findsNothing);
    });

    testWidgets('新建表单页头有「智能识别」，点开出弹窗；编辑态没有这颗按钮（票 20）', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final k = pageFake();
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream.empty(),
          ),
          i18n,
        ),
      );
      await settle(tester);

      // 新建态：按钮在，点开出弹窗。
      await tester.tap(find.text('+ ${i18n.t('platform.add')}'));
      await settle(tester);
      expect(find.text(i18n.t('platform.paste.title')), findsOneWidget);
      await tester.tap(find.text(i18n.t('platform.paste.title')));
      await settle(tester);
      expect(find.byType(SmartPasteModal), findsOneWidget);
      expect(find.text(i18n.t('platform.paste.placeholder')), findsOneWidget);

      // 取消关窗，再退回列表。
      await tester.tap(
        find.widgetWithText(SmallButton, i18n.t('action.cancel')).first,
      );
      await settle(tester);
      expect(find.byType(SmartPasteModal), findsNothing);
      await tester.tap(find.text('← ${i18n.t('action.back')}'));
      await settle(tester);

      // 编辑态：整段灌入会冲掉用户改过的字段，所以这颗按钮不该出现。
      await tester.tap(find.byTooltip(i18n.t('action.edit')));
      await settle(tester);
      expect(find.text(i18n.t('platform.paste.title')), findsNothing);
    });

    testWidgets('卡片「编辑」打开编辑表单并带上该平台的名字', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final k = pageFake();
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream.empty(),
          ),
          i18n,
        ),
      );
      await settle(tester);

      await tester.tap(find.byTooltip(i18n.t('action.edit')));
      await settle(tester);
      expect(find.text(i18n.t('platform.protocolLocked')), findsOneWidget);
      // 页头标题用的是平台名（React `PlatformEditForm.tsx:104`）。
      expect(find.text('P1'), findsWidgets);
    });

    testWidgets('表单没打开时不渲染任何分区', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      final k = pageFake();
      await tester.pumpWidget(
        wrapPage(
          PlatformsPage(
            invoke: k.fn,
            showGroups: false,
            logUpdates: const Stream.empty(),
          ),
          i18n,
        ),
      );
      await settle(tester);
      expect(find.text(i18n.t('platform.sectionBasic')), findsNothing);
      expect(find.byType(PlatformEditForm), findsNothing);
    });
  });
}
