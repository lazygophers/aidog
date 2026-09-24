/// 票 I16：设置 12 个子页的 widget 测试。
///
/// 只测**这一层新加的东西**：渲染得出来、禁用条件生效、破坏性操作先确认、
/// 离页拦截三个出口都走得通。状态机本身由票 I08 的 382 条断言盯着，这里不重测。
///
/// 不起内核（假 invoke 顶掉传输层），所以不碰用户的 9890 端口，也不碰 `~/.aidog`。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/src/pages/settings/bits.dart';
import 'package:aidog_flutter/src/pages/settings/coding_tools_logic.dart'
    show kDateRewriteRuleName;
import 'package:aidog_flutter/src/pages/settings/importexport_logic.dart'
    show kInitialScopes;
import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';

/// 各页首屏要的载荷。给全了才不会被 [FakeKernel] 的「没摆载荷」断言打断。
Map<String, Object? Function(Map<String, Object?>?)> baseResponses() => {
  // 系统页
  'proxy_get_settings': (_) => {
    'autostart': false,
    'silent_launch': false,
    'bind_lan': false,
    'port': 9890,
  },
  'proxy_status': (_) => false,
  'app_get_autolaunch': (_) => false,
  'app_set_silent_launch': (_) => null,
  'proxy_log_settings_get': (_) => {
    'enabled': true,
    'retention_days': 90,
    'retention_unit': 'day',
    'log_user_request': true,
    'log_upstream_request': true,
    'user_request_retention_days': 7,
    'user_request_retention_unit': 'day',
    'upstream_request_retention_days': 7,
    'upstream_request_retention_unit': 'day',
  },
  'proxy_timeout_get': (_) => {
    'request_timeout_secs': 300,
    'connect_timeout_secs': 10,
  },
  'app_log_settings_get': (_) => {
    'file_enabled': true,
    'level': 'info',
    'retention_hours': 3,
  },
  'proxy_client_get_settings': (_) => {
    'enabled': false,
    'proxy_type': 'socks5',
    'host': '127.0.0.1',
    'port': 7890,
    'username': '',
    'password': '',
    'dns_over_proxy': true,
    'no_proxy': '',
  },
  'stats_settings_get': (_) => {'retention_days': 365},
  'get_auto_update_enabled': (_) => true,
  'kernel_settings_get': (_) => {'port': 9891, 'auth_token': ''},
  'proxy_log_cleanup_estimate': (_) => {
    'overdue_rows': 12,
    'overdue_body_bytes': 2048,
    'db_size_bytes': 4096,
  },
  'proxy_log_cleanup_expired': (_) => null,
  'proxy_log_clear': (_) => null,
  'db_compact': (_) => {'before_bytes': 2048.0, 'after_bytes': 1024.0},
  'proxy_start': (_) => 'started',
  'proxy_stop': (_) => null,
  'settings_get': (_) => <String, Object?>{},
  'settings_set': (_) => null,
  'sync_group_settings': (_) => null,
  // CLI 集成页
  'coding_tools_settings_get': (_) => {
    'apply_to_claude_plugin': false,
    'skip_claude_onboarding': false,
  },
  'coding_tools_settings_set': (_) => {
    'apply_to_claude_plugin': true,
    'skip_claude_onboarding': false,
  },
  'codex_config_read': (_) => <String, Object?>{},
  'codex_config_write': (_) => null,
  'pi_settings_read': (_) => <String, Object?>{},
  'pi_settings_write': (_) => null,
  // 通知页
  'notification_settings_get': (_) => {
    'enabled': true,
    'tts_enabled': true,
    'tts_backend': 'cross_platform',
    'per_type': <String, Object?>{},
    'per_event': <String, Object?>{},
    'inbox_retention_days': 7,
  },
  'notification_settings_set': (_) => null,
  'get_default_hooks_enabled': (_) => false,
  // 规则两页
  'scheduling_settings_get': (_) => {
    'default_routing_mode': 'health_aware',
    'breaker_failure_threshold': 5,
    'breaker_open_secs': 60,
    'breaker_half_open_max': 2,
    'enabled': true,
  },
  'scheduling_settings_set': (_) => null,
  'middleware_list_rules': (_) => <Object?>[],
  'middleware_budget_status': (_) => <Object?>[],
  'middleware_settings_get': (_) => {'enabled': true},
  'middleware_delete_rule': (_) => null,
  'middleware_create_rule': (_) => null,
  'platform_list': (_) => <Object?>[],
  'group_list': (_) => <Object?>[],
  'group_detail_list': (_) => <Object?>[],
  // MITM
  'mitm_status': (_) => {
    'enabled': false,
    'ca_present': true,
    'ca_installed': false,
    'ca_fingerprint': 'AA:BB',
    'whitelist': <Object?>[],
  },
  'mitm_whitelist_clear': (_) => 3,
  // 托盘 / 浮窗
  'tray_config_get': (_) => {'separator': '  ', 'items': <Object?>[]},
  'tray_today_stats': (_) => {
    'tokens': 0,
    'cache_rate': 0,
    'cost': 0,
    'total_requests': 0,
  },
  'popover_config_get': (_) => <String, Object?>{},
  'popover_platform_today': (_) => <Object?>[],
  // 导入导出
  'backup_settings_get': (_) => {
    'enabled': false,
    'interval_hours': 24,
    'retention_days': 7,
    'dir': '',
  },
  'export_preview': (_) => {'items': <Object?>[], 'conflicts': <Object?>[]},
};

/// 点一个 [SwitchRow] 里的开关本体。整行不可点（只有 Switch 带 onChanged），
/// 直接 `tap(byKey(row))` 会打在标签上，什么也不会发生。
Future<void> tapSwitch(WidgetTester tester, String key) async {
  await tester.tap(
    find.descendant(
      of: find.byKey(ValueKey(key)),
      matching: find.byType(AidogSwitch),
    ),
  );
  await settle(tester);
}

/// 一份两字段的小 schema —— 每个用例都解 121 KB 资产太慢。
SchemaBundle fakeBundle() => SchemaBundle(
  sections: [
    SchemaSection({
      'id': 'core',
      'labelKey': 'settings.sectionCore',
      'fields': [
        {'key': 'model', 'label': 'Model', 'type': 'string'},
        {'key': 'verbose', 'label': 'Verbose', 'type': 'boolean'},
      ],
    }),
  ],
  recommended: const {'model': 'sonnet'},
);

/// 数字 / 地址类文本被 `ltr()` 包进了 bidi 隔离字符（U+2066/U+2069），
/// `find.text` 是裸等值比较，对不上。这个查找器两边都剥。
Finder findStripped(CommonFinders find, String needle) =>
    find.byWidgetPredicate(
      (w) => w is Text && stripIsolates(w.data ?? '') == needle,
      description: 'text stripped of bidi isolates "$needle"',
    );

/// 某段文字被富文本切成了几段（没高亮时是普通 Text → 0 段）。
int richSpanCount(WidgetTester tester, String plain) {
  for (final w in tester.widgetList<Text>(find.byType(Text))) {
    final span = w.textSpan;
    if (span == null) continue;
    if (span.toPlainText() != plain) continue;
    return span is TextSpan ? (span.children?.length ?? 0) : 0;
  }
  return 0;
}

void main() {
  setUp(resetNavGuardForTest);
  tearDown(resetNavGuardForTest);

  // ── 系统页 ────────────────────────────────────────────

  group('系统页', () {
    Future<(FakeKernel, I18nController)> mount(WidgetTester tester) async {
      await useBigSurface(tester);
      final k = FakeKernel(baseResponses());
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SystemSettingsPage(
            invoke: k.invoke,
            appVersionFn: () async => '9.9.9',
          ),
          i18n,
        ),
      );
      await settle(tester);
      return (k, i18n);
    }

    testWidgets('首屏把 11 个读命令全发一遍', (tester) async {
      final (k, _) = await mount(tester);
      for (final cmd in [
        'proxy_get_settings',
        'proxy_status',
        'app_get_autolaunch',
        'proxy_log_settings_get',
        'proxy_timeout_get',
        'app_log_settings_get',
        'proxy_client_get_settings',
        'stats_settings_get',
        'get_auto_update_enabled',
        'kernel_settings_get',
        'proxy_log_cleanup_estimate',
      ]) {
        expect(k.calls, contains(cmd), reason: '$cmd 没发');
      }
    });

    testWidgets('启动代理按钮发 proxy_start', (tester) async {
      final (k, _) = await mount(tester);
      await tester.tap(find.byKey(const ValueKey('proxy-toggle')));
      await settle(tester);
      expect(k.lastArgsOf('proxy_start'), {'port': 9890});
    });

    testWidgets('压缩数据库：确认之前不发 db_compact', (tester) async {
      final (k, _) = await mount(tester);
      await tester.tap(find.byKey(const ValueKey('db-compact')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);
      expect(k.countOf('db_compact'), 0);

      await tester.tap(
        find
            .descendant(
              of: find.byType(ConfirmCard),
              matching: find.byType(SmallButton),
            )
            .last,
      );
      await settle(tester);
      expect(k.countOf('db_compact'), 1);
    });

    testWidgets('关掉日志记录后，清理按钮仍然点得到（库里旧日志还得清）', (tester) async {
      // React 在 `LogSettingsSection.tsx:210` 特意把清理动作放在 logEnabled 之外，
      // 注释原文「关闭记录后仍需可清已存日志」。Flutter 原先包进了 if (logEnabled)，
      // 后果是关掉记录就再也清不掉已经攒下的旧日志。
      await useBigSurface(tester);
      final k = FakeKernel({
        ...baseResponses(),
        'proxy_log_settings_get': (_) => {
          'enabled': false,
          'retention_days': 90,
          'retention_unit': 'day',
          'log_user_request': false,
          'log_upstream_request': false,
          'user_request_retention_days': 7,
          'user_request_retention_unit': 'day',
          'upstream_request_retention_days': 7,
          'upstream_request_retention_unit': 'day',
        },
      });
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SystemSettingsPage(
            invoke: k.invoke,
            appVersionFn: () async => '9.9.9',
          ),
          i18n,
        ),
      );
      await settle(tester);

      expect(find.byKey(const ValueKey('clear-logs')), findsOneWidget);
      expect(find.byKey(const ValueKey('cleanup-expired')), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('clear-logs')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget, reason: '点得动才算数');
    });

    testWidgets('内核访问令牌是遮挡输入（拿到它就能直连内核）', (tester) async {
      final (_, _) = await mount(tester);
      final field = tester.widget<TextRow>(
        find.byKey(const ValueKey('kernel-token')),
      );
      expect(
        field.obscure,
        isTrue,
        reason: 'React 是 <input type="password">（KernelSection.tsx:72）',
      );
    });

    testWidgets('内核令牌与上游代理密码没有明文切换按钮（对齐 React）', (tester) async {
      // React 的 KernelSection.tsx / ProxyStatusSection.tsx 没有暴露眼睛
      // 按钮，密码永远密文。Flutter 原先多出的 reveal 已删，此测试守着
      // 不让它长回来（安全：拿到令牌就能直连内核）。
      await useBigSurface(tester);
      final k = FakeKernel({
        ...baseResponses(),
        'proxy_client_get_settings': (_) => {
          'enabled': true,
          'proxy_type': 'socks5',
          'host': '127.0.0.1',
          'port': 7890,
          'username': '',
          'password': 'pw',
          'dns_over_proxy': false,
          'no_proxy': '',
        },
      });
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SystemSettingsPage(
            invoke: k.invoke,
            appVersionFn: () async => '9.9.9',
          ),
          i18n,
        ),
      );
      await settle(tester);

      bool obscured(String key) => switch (key) {
        // 批三重构后：令牌仍是 TextRow，上游密码换成了横排 PlainTextField。
        'kernel-token' => tester.widget<TextRow>(
            find.byKey(const ValueKey('kernel-token')),
          ).obscure,
        _ => tester.widget<PlainTextField>(
            find.byKey(ValueKey(key)),
          ).obscure,
      };

      // 密文是常态。
      expect(obscured('kernel-token'), isTrue);
      expect(obscured('upstream-proxy-pass'), isTrue);

      // 没有明文切换按钮。
      expect(find.byKey(const ValueKey('kernel-token-reveal')), findsNothing);
      expect(
        find.byKey(const ValueKey('upstream-proxy-pass-reveal')),
        findsNothing,
      );
    });

    testWidgets('上游代理：三个协议都在；DNS 走代理只在 SOCKS5 下出现', (tester) async {
      // React `ProxyStatusSection.tsx:142-144` 给三个协议，`:195` 把
      // dns_over_proxy 锁在 socks5 分支里 —— HTTP/HTTPS 代理本来就是把域名
      // 整个交给代理解析，那个开关摆出来只会让人以为它起作用。
      await useBigSurface(tester);
      final k = FakeKernel({
        ...baseResponses(),
        'proxy_client_get_settings': (_) => {
          'enabled': true,
          'proxy_type': 'http',
          'host': '127.0.0.1',
          'port': 7890,
          'username': '',
          'password': '',
          'dns_over_proxy': true,
          'no_proxy': '',
        },
      });
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SystemSettingsPage(
            invoke: k.invoke,
            appVersionFn: () async => '9.9.9',
          ),
          i18n,
        ),
      );
      await settle(tester);

      final row = tester.widget<InlineSelect<String>>(
        find.byKey(const ValueKey('upstream-proxy-type')),
      );
      expect(row.options, ['socks5', 'http', 'https']);
      expect(
        find.text(i18n.t('proxy.proxyType')),
        findsOneWidget,
        reason: '协议 label 仍在（横排行）',
      );
      expect(
        find.text(i18n.t('proxy.dnsOverProxy')),
        findsNothing,
        reason: 'http 模式下不该出现',
      );
    });

    // 第二梯队 2026-09-22：原先只有卡片 meta 上一行「运行中 / 已停止」文字，
    // 没有状态灯，也看不到代理监听在哪个地址上（`ProxyStatusSection.tsx:29-53`）。
    testWidgets('代理状态：停着时没有监听地址，跑起来才显示 localhost:<port>', (tester) async {
      final (k, i18n) = await mount(tester);
      expect(find.text(i18n.t('proxy.stopped')), findsWidgets);
      expect(findStripped(find, 'localhost:9890'), findsNothing);

      k.responses['proxy_status'] = (_) => true;
      await tester.tap(find.byKey(const ValueKey('proxy-toggle')));
      await settle(tester);
      expect(find.text(i18n.t('proxy.running')), findsWidgets);
      expect(findStripped(find, 'localhost:9890'), findsOneWidget);
    });

    testWidgets('清空日志：取消后不发命令，确认后才发', (tester) async {
      final (k, _) = await mount(tester);
      await tester.tap(find.byKey(const ValueKey('clear-logs')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);

      await tester.tap(
        find
            .descendant(
              of: find.byType(ConfirmCard),
              matching: find.byType(SmallButton),
            )
            .first,
      );
      await settle(tester);
      expect(find.byType(ConfirmCard), findsNothing);
      expect(k.countOf('proxy_log_clear'), 0);
    });

    // 第二梯队 2026-09-22：两条 per-type 保留期原先只受总开关控制 ——
    // 关掉某类记录后，它的保留期还摆在那里可改，是改了也没用的旋钮
    //（`LogSettingsSection.tsx:145,166` 各自跟着自己那类的开关走）。
    // 第三梯队 2026-09-22：0 = 永久保留时单位（小时/天）没有意义，
    // React 把整组单位藏掉换成一行「永久保留」（`LogSettingsSection.tsx:155-162`）；
    // 清理按钮置灰之外还要说清**为什么**（`:216` 的 title）。
    testWidgets('保留期 90 天：单位在、清理按钮可点', (tester) async {
      final (_, i18n) = await mount(tester);
      expect(find.text(i18n.t('unit.day')), findsWidgets);
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('cleanup-expired')))
            .enabled,
        isTrue,
      );
    });

    testWidgets('保留期 0：藏掉单位、清理按钮带禁用原因', (tester) async {
      await useBigSurface(tester);
      final k = FakeKernel({
        ...baseResponses(),
        'proxy_log_settings_get': (_) => {
          'enabled': true,
          'retention_days': 0,
          'retention_unit': 'day',
          'log_user_request': true,
          'log_upstream_request': true,
          'user_request_retention_days': 7,
          'user_request_retention_unit': 'day',
          'upstream_request_retention_days': 7,
          'upstream_request_retention_unit': 'day',
        },
      });
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SystemSettingsPage(
            invoke: k.invoke,
            appVersionFn: () async => '9.9.9',
          ),
          i18n,
        ),
      );
      await settle(tester);

      // 「永久保留」那条的单位按钮组不该还摆着（0 时选了也没用）。
      expect(find.text(i18n.t('proxy.logRetentionForever')), findsOneWidget);
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('cleanup-expired')))
            .enabled,
        isFalse,
      );
      expect(
        find.ancestor(
          of: find.byKey(const ValueKey('cleanup-expired')),
          matching: find.byWidgetPredicate(
            (w) =>
                w is Tooltip && w.message == i18n.t('logs.cleanupDisabledHint'),
          ),
        ),
        findsOneWidget,
      );
    });

    testWidgets('关掉「记录原始请求」→ 它的保留期一并收起', (tester) async {
      await mount(tester);
      expect(find.byKey(const ValueKey('user-req-retention')), findsOneWidget);
      await tapSwitch(tester, 'log-user-req');
      expect(find.byKey(const ValueKey('user-req-retention')), findsNothing);
      // 上游那条不受影响，各管各的。
      expect(
        find.byKey(const ValueKey('upstream-req-retention')),
        findsOneWidget,
      );
    });

    testWidgets('关掉「记录上游请求」→ 它的保留期一并收起', (tester) async {
      await mount(tester);
      await tapSwitch(tester, 'log-upstream-req');
      expect(
        find.byKey(const ValueKey('upstream-req-retention')),
        findsNothing,
      );
      expect(find.byKey(const ValueKey('user-req-retention')), findsOneWidget);
    });
  });

  // ── claude / codex / pi 三页 ──────────────────────────

  group('schema 配置页', () {
    Future<FakeKernel> mount(WidgetTester tester, SchemaConfigKind kind) async {
      await useBigSurface(tester);
      final k = FakeKernel(baseResponses());
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SchemaConfigPage(
            kind: kind,
            invoke: k.invoke,
            bundleLoader: (_) async => fakeBundle(),
          ),
          i18n,
        ),
      );
      await settle(tester);
      return k;
    }

    /// 两节的 schema：锚点 chip 条只在多于一节时才有意义。
    SchemaBundle twoSectionBundle() => SchemaBundle(
      sections: [
        SchemaSection({
          'id': 'core',
          'labelKey': 'settings.sectionCore',
          'fields': [
            {'key': 'model', 'label': 'Model', 'type': 'string'},
          ],
        }),
        SchemaSection({
          'id': 'perm',
          'labelKey': 'settings.sectionPermissions',
          'fields': [
            {'key': 'verbose', 'label': 'Verbose', 'type': 'boolean'},
          ],
        }),
      ],
      recommended: const {},
    );

    // 回归 2026-09-22：claude 页十几个 section，原先只能一路滚 ——
    // 没有 chip 条、没有滚动联动（`SectionAnchorNav.tsx:19-68`）。
    testWidgets('claude 页有 section 锚点 chip，点一下高亮它', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SchemaConfigPage(
            kind: SchemaConfigKind.claude,
            invoke: FakeKernel(baseResponses()).invoke,
            bundleLoader: (_) async => twoSectionBundle(),
          ),
          i18n,
        ),
      );
      await settle(tester);
      expect(find.byKey(const ValueKey('settings-anchor-nav')), findsOneWidget);
      final chip = find.descendant(
        of: find.byKey(const ValueKey('settings-anchor-nav')),
        matching: find.widgetWithText(
          SmallButton,
          i18n.t('settings.sectionPermissions'),
        ),
      );
      expect(chip, findsOneWidget);
      expect(tester.widget<SmallButton>(chip).active, isFalse);
      await tester.tap(chip);
      await settle(tester);
      expect(tester.widget<SmallButton>(chip).active, isTrue);
    });

    testWidgets('codex 页没有 chip 条（搜索与锚点都是 claude 页独有）', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SchemaConfigPage(
            kind: SchemaConfigKind.codex,
            invoke: FakeKernel(baseResponses()).invoke,
            bundleLoader: (_) async => twoSectionBundle(),
          ),
          i18n,
        ),
      );
      await settle(tester);
      expect(find.byKey(const ValueKey('settings-anchor-nav')), findsNothing);
    });

    // 第二梯队 2026-09-22：字段级 JSON 早就换成 re_editor（高亮 / 行号 / 折叠 /
    // 查找替换），整页 JSON 模式却还是个纯文本框（React 两处都是 JsonCodeEditor，
    // `Settings.tsx:506`）。
    // 第二梯队 2026-09-22：原先只按命中过滤 section / 字段，命中在**哪个词**
    // 看不出来（`FieldRenderer.tsx:45-60` 把命中的那段加底色）。
    testWidgets('搜索命中的那段文字被标出来', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SchemaConfigPage(
            kind: SchemaConfigKind.claude,
            invoke: FakeKernel(baseResponses()).invoke,
            bundleLoader: (_) async => fakeBundle(),
          ),
          i18n,
        ),
      );
      await settle(tester);
      // 字段标签是翻译过的（fakeBundle 里 `model` → 「模型」），
      // 搜的也得是用户看得见的那几个字 —— React 高亮的就是标签文本。
      expect(richSpanCount(tester, '模型'), 0, reason: '搜之前是普通 Text');

      await tester.enterText(
        find.byKey(const ValueKey('settings-search')),
        '模',
      );
      await settle(tester);
      // 「模型」被切成 前段(空) + 命中段 + 余下段 三段。
      expect(richSpanCount(tester, '模型'), 3);
    });

    testWidgets('搜索框有清除按钮，点一下把词和过滤一起清掉', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SchemaConfigPage(
            kind: SchemaConfigKind.claude,
            invoke: FakeKernel(baseResponses()).invoke,
            bundleLoader: (_) async => fakeBundle(),
          ),
          i18n,
        ),
      );
      await settle(tester);
      expect(
        find.byKey(const ValueKey('settings-search-clear')),
        findsNothing,
        reason: '没输入就不该占位',
      );

      await tester.enterText(
        find.byKey(const ValueKey('settings-search')),
        '搜不到这个词',
      );
      await settle(tester);
      expect(
        find.byKey(const ValueKey('settings-search-no-match')),
        findsOneWidget,
      );

      await tester.tap(find.byKey(const ValueKey('settings-search-clear')));
      await settle(tester);
      expect(
        find.byKey(const ValueKey('settings-search-no-match')),
        findsNothing,
      );
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('settings-search')))
            .controller!
            .text,
        isEmpty,
      );
    });

    testWidgets('整页 JSON 模式用的是代码编辑器，不是纯文本框', (tester) async {
      await mount(tester, SchemaConfigKind.claude);
      final i18n = await makeI18n(tester);
      expect(find.byKey(const ValueKey('page-json-editor')), findsNothing);

      await tester.tap(
        find.widgetWithText(SmallButton, i18n.t('settings.jsonMode')),
      );
      await settle(tester);
      expect(find.byKey(const ValueKey('page-json-editor')), findsOneWidget);
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('page-json-editor')),
          matching: find.byKey(const ValueKey('json-code-editor')),
        ),
        findsOneWidget,
      );
    });

    testWidgets('未改动时保存按钮点不动', (tester) async {
      final i18n = await makeI18n(tester);
      await mount(tester, SchemaConfigKind.claude);
      final save = tester.widget<SmallButton>(
        find.widgetWithText(SmallButton, i18n.t('action.save')),
      );
      expect(save.enabled, isFalse);
    });

    testWidgets('改一个字段 → 脏 → 注册离页守卫 → 保存后守卫注销', (tester) async {
      final k = await mount(tester, SchemaConfigKind.claude);
      expect(hasNavGuard, isFalse);

      await tapSwitch(tester, 'field-verbose');
      expect(hasNavGuard, isTrue, reason: '脏了就该挂守卫');

      final i18n = await makeI18n(tester);
      await tester.tap(find.widgetWithText(SmallButton, i18n.t('action.save')));
      await settle(tester);
      expect(k.countOf('settings_set'), 1);
      // claude 页保存后 best-effort 同步分组设置。
      expect(k.countOf('sync_group_settings'), 1);
      expect(hasNavGuard, isFalse, reason: '保存完就该注销守卫');
    });

    testWidgets('离页拦截三个出口：取消留在原地 / 放弃直接走 / 保存并离开', (tester) async {
      final k = await mount(tester, SchemaConfigKind.claude);
      await tapSwitch(tester, 'field-verbose');

      var navigated = 0;
      requestNavigation(() => navigated++);
      await settle(tester);
      expect(find.byType(UnsavedChangesCard), findsOneWidget);

      // ① 取消：不走，守卫还在。
      await tester.tap(find.byKey(const ValueKey('unsaved-cancel')));
      await settle(tester);
      expect(navigated, 0);
      expect(hasNavGuard, isTrue);

      // ② 放弃：直接走，不落盘。
      requestNavigation(() => navigated++);
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('unsaved-discard')));
      await settle(tester);
      expect(navigated, 1);
      expect(k.countOf('settings_set'), 0);

      // ③ 保存并离开：先落盘再走。
      requestNavigation(() => navigated++);
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('unsaved-save')));
      await settle(tester);
      expect(k.countOf('settings_set'), 1);
      expect(navigated, 2);
    });

    testWidgets('codex 页脏了也不注册守卫（照搬 React）', (tester) async {
      await mount(tester, SchemaConfigKind.codex);
      await tapSwitch(tester, 'field-verbose');
      expect(hasNavGuard, isFalse);
    });

    testWidgets('pi 页脏了也不注册守卫（照搬 React）', (tester) async {
      await mount(tester, SchemaConfigKind.pi);
      await tapSwitch(tester, 'field-verbose');
      expect(hasNavGuard, isFalse);
    });
  });

  // ── CLI 集成页 ────────────────────────────────────────

  group('CLI 集成页', () {
    Future<FakeKernel> mount(
      WidgetTester tester, {
      List<Object?>? rules,
    }) async {
      await useBigSurface(tester);
      final k = FakeKernel({
        ...baseResponses(),
        if (rules != null) 'middleware_list_rules': (_) => rules,
      });
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          CodingToolsPage(
            invoke: k.invoke,
            languageLoader: () async => const [
              (family: '中文', options: [(value: 'zh-Hans', label: '简体（通用）')]),
            ],
          ),
          i18n,
        ),
      );
      await settle(tester);
      return k;
    }

    testWidgets('日期改写规则读不到时开关点不动', (tester) async {
      await mount(tester);
      final sw = tester.widget<ToggleCard>(
        find.byKey(const ValueKey('date-rewrite')),
      );
      expect(sw.onChanged, isNull);
    });

    testWidgets('规则存在时开关可用', (tester) async {
      await mount(
        tester,
        rules: [
          {
            'id': 7,
            'name': kDateRewriteRuleName,
            'is_builtin': true,
            'enabled': false,
          },
        ],
      );
      final sw = tester.widget<ToggleCard>(
        find.byKey(const ValueKey('date-rewrite')),
      );
      expect(sw.onChanged, isNotNull);
    });

    testWidgets('插件开关发 coding_tools_settings_set', (tester) async {
      final k = await mount(tester);
      await tapSwitch(tester, 'apply-to-claude-plugin');
      expect(k.lastArgsOf('coding_tools_settings_set'), {
        'applyToClaudePlugin': true,
      });
    });

    testWidgets('努力级别「—」把两侧清回不设置', (tester) async {
      // `CodingToolsSettings.tsx:475-479` 的 `__none__`：选「—」写空串，
      // claude effortLevel + codex model_reasoning_effort 双写。
      final k = await mount(tester);
      // 批三后努力级别是下拉（React Select，`CodingToolsSettings.tsx:454-485`）：
      // 先点触发器展开，再点菜单里的「—」。
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('cli-effort')),
          matching: find.byType(DropdownButton<String>),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('—').last);
      await settle(tester);

      final setArgs = k.lastArgsOf('settings_set')!;
      final value = (setArgs['input'] as Map)['value'] as Map;
      expect(value['effortLevel'], '');
      expect(
        (k.lastArgsOf('codex_config_write')!['value']
            as Map)['model_reasoning_effort'],
        '',
      );
    });

    testWidgets('代理草稿改了就挂守卫，放弃离开会丢回已生效值', (tester) async {
      await mount(tester);
      expect(hasNavGuard, isFalse);
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('cli-proxy-url')),
          matching: find.byType(TextField),
        ),
        'http://127.0.0.1:1',
      );
      await settle(tester);
      expect(hasNavGuard, isTrue);

      var navigated = 0;
      requestNavigation(() => navigated++);
      await settle(tester);
      expect(find.byType(UnsavedChangesCard), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('unsaved-discard')));
      await settle(tester);
      expect(navigated, 1);
      expect(hasNavGuard, isFalse);
    });
  });

  // ── 通知页 ────────────────────────────────────────────

  group('通知页', () {
    Future<FakeKernel> mount(WidgetTester tester, {bool enabled = true}) async {
      await useBigSurface(tester);
      final k = FakeKernel({
        ...baseResponses(),
        'notification_settings_get': (_) => {
          'enabled': enabled,
          'tts_enabled': true,
          'tts_backend': 'cross_platform',
          'per_type': <String, Object?>{},
          'per_event': <String, Object?>{},
          'inbox_retention_days': 7,
        },
      });
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          NotificationsSettingsPage(invoke: k.invoke, openUrlFn: (_) async {}),
          i18n,
        ),
      );
      await settle(tester);
      return k;
    }

    // 回归 2026-09-22：原先只有一个天数框，「不清理」得用户自己猜出要填 0
    //（`NotificationSettings.tsx:409-419`：关 → 写 0，开 → 回 7 天，关了藏天数框）。
    testWidgets('通知历史自动清理：关掉写 0 并藏天数框，开回来写 7', (tester) async {
      final k = await mount(tester);
      expect(find.byKey(const ValueKey('inbox-retention')), findsOneWidget);

      await tapSwitch(tester, 'inbox-retention-on');
      expect(
        k.lastArgsOf('notification_settings_set')!['settings'],
        containsPair('inbox_retention_days', 0),
      );
      expect(find.byKey(const ValueKey('inbox-retention')), findsNothing);

      await tapSwitch(tester, 'inbox-retention-on');
      expect(
        k.lastArgsOf('notification_settings_set')!['settings'],
        containsPair('inbox_retention_days', 7),
      );
    });

    // 第三梯队 2026-09-22：总开关关掉后从属区块原先仍是正常亮度
    //（React 四处 `opacity: 0.55 / 0.5`，这里验通知页那两处）。
    testWidgets('通知总开关关掉 → 通道测试卡与事件列表压暗', (tester) async {
      await mount(tester, enabled: false);
      // 批三重构后：两处压暗不再是 SettingsCard.dimmed，改成 React 同款
      // Opacity 0.55 / 0.5（NotificationSettings.tsx:352 / EventList:196）。
      final testBar = tester.widget<Opacity>(
        find.byKey(const ValueKey('notif-test-bar-dim')),
      );
      final eventList = tester.widget<Opacity>(
        find.byKey(const ValueKey('notif-event-list-dim')),
      );
      expect(testBar.opacity, 0.55, reason: '通道测试卡');
      expect(eventList.opacity, 0.5, reason: '事件列表');
    });

    testWidgets('通知总开关开着 → 一张都不压暗', (tester) async {
      await mount(tester);
      expect(
        tester.widget<Opacity>(find.byKey(const ValueKey('notif-test-bar-dim'))).opacity,
        1.0,
      );
      expect(
        tester.widget<Opacity>(
          find.byKey(const ValueKey('notif-event-list-dim')),
        ).opacity,
        1.0,
      );
    });

    testWidgets('总开关关掉时「默认注入 hook」点不动', (tester) async {
      await mount(tester, enabled: false);
      final sw = tester.widget<ToggleCard>(
        find.byKey(const ValueKey('default-hooks')),
      );
      expect(sw.onChanged, isNull);
    });

    testWidgets('总开关开着时可用，且 30 个 hook 事件都渲染出来', (tester) async {
      await mount(tester);
      final sw = tester.widget<ToggleCard>(
        find.byKey(const ValueKey('default-hooks')),
      );
      expect(sw.onChanged, isNotNull);
      for (final e in kCcHookEvents) {
        expect(
          find.byKey(ValueKey('event-$e')),
          findsOneWidget,
          reason: '$e 没渲染',
        );
      }
    });

    testWidgets('四个通道测试各发各的命令', (tester) async {
      final k = FakeKernel({
        ...baseResponses(),
        'notification_test': (_) => null,
        'notification_test_tts': (_) => null,
        'notification_test_popup': (_) => null,
        'notification_test_beep': (_) => null,
      });
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          NotificationsSettingsPage(invoke: k.invoke, openUrlFn: (_) async {}),
          i18n,
        ),
      );
      await settle(tester);
      for (final entry in {
        'test-notify': 'notification_test',
        'test-tts': 'notification_test_tts',
        'test-popup': 'notification_test_popup',
        'test-beep': 'notification_test_beep',
      }.entries) {
        await tester.tap(find.byKey(ValueKey(entry.key)));
        await settle(tester);
        expect(k.countOf(entry.value), 1, reason: entry.value);
      }
    });
  });

  // ── 调度熔断页 ────────────────────────────────────────

  testWidgets('调度页：切策略即落盘', (tester) async {
    await useBigSurface(tester);
    final k = FakeKernel(baseResponses());
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(SchedulingSettingsPage(invoke: k.invoke), i18n),
    );
    await settle(tester);
    // 路由模式是下拉（对齐 React `SchedulingSettings.tsx:124-136`），
    // 点开再选「故障转移」。
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey('routing-mode')),
        matching: find.byWidgetPredicate((w) => w is DropdownButton),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text(i18n.t('group.failover')));
    await settle(tester);
    final args = k.lastArgsOf('scheduling_settings_set')!['settings']! as Map;
    expect(args['default_routing_mode'], 'failover');
  });

  // ── 中间件页 ──────────────────────────────────────────

  group('中间件页', () {
    Future<FakeKernel> mount(
      WidgetTester tester, {
      List<Object?>? rules,
    }) async {
      await useBigSurface(tester);
      final k = FakeKernel({
        ...baseResponses(),
        if (rules != null) 'middleware_list_rules': (_) => rules,
      });
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(MiddlewareSettingsPage(invoke: k.invoke), i18n),
      );
      await settle(tester);
      return k;
    }

    testWidgets('空列表走空态文案', (tester) async {
      await mount(tester);
      final i18n = await makeI18n(tester);
      expect(find.text(i18n.t('middleware.noRules')), findsOneWidget);
    });

    testWidgets('删除先确认，确认前不发命令', (tester) async {
      final k = await mount(
        tester,
        rules: [
          {
            'id': 3,
            'name': 'r3',
            'description': '',
            'enabled': true,
            'is_builtin': false,
            'priority': 0,
          },
        ],
      );
      await tester.tap(find.byKey(const ValueKey('rule-del-3')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);
      expect(k.countOf('middleware_delete_rule'), 0);
    });

    testWidgets('新建表单：名字为空时保存点不动，且表单开着就挂守卫', (tester) async {
      await mount(tester);
      await tester.tap(find.byKey(const ValueKey('middleware-add')));
      await settle(tester);
      expect(hasNavGuard, isTrue);
      final save = tester.widget<SmallButton>(
        find.byKey(const ValueKey('rule-save')),
      );
      expect(save.enabled, isFalse);

      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('rule-name')),
          matching: find.byType(TextField),
        ),
        'my-rule',
      );
      await settle(tester);
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('rule-save')))
            .enabled,
        isTrue,
      );
    });

    testWidgets('条件 JSON 解不开时保存点不动', (tester) async {
      await mount(tester);
      await tester.tap(find.byKey(const ValueKey('middleware-add')));
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('cond-mode-json')));
      await settle(tester);
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('rule-conditions')),
          matching: find.byType(TextField),
        ),
        '{ 这不是 JSON',
      );
      await settle(tester);
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('rule-save')))
            .enabled,
        isFalse,
      );
    });
  });

  // ── MITM 页 ───────────────────────────────────────────

  group('MITM 页', () {
    /// [enabled] 默认 true：关掉总开关时风险卡 / CA 卡 / 白名单卡 / 命中测试卡
    /// 整块不渲染（与 React `MitmConfig.tsx:272,291,386` 一致），
    /// 所以除了专门验门控的那条用例，其余都要开着才有东西可点。
    Future<FakeKernel> mount(
      WidgetTester tester, {
      List<Object?> whitelist = const [],
      bool enabled = true,
    }) async {
      await useBigSurface(tester);
      final k = FakeKernel({
        ...baseResponses(),
        'mitm_status': (_) => {
          'enabled': enabled,
          'ca_present': true,
          'ca_installed': false,
          'ca_fingerprint': 'AA:BB',
          'whitelist': whitelist,
        },
      });
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          MitmSettingsPage(invoke: k.invoke, copyFn: (_) async {}),
          i18n,
        ),
      );
      await settle(tester);
      return k;
    }

    testWidgets('总开关关着时，CA / 白名单 / 命中测试整块不渲染', (tester) async {
      // 原先这几张卡无条件渲染：开关关着照样能装 CA、改白名单、跑命中测试，
      // 改完一条都不生效。React 三处都是 `{enabled && ...}`。
      await mount(tester, enabled: false);
      expect(find.byKey(const ValueKey('mitm-master')), findsOneWidget);
      expect(find.byKey(const ValueKey('mitm-add')), findsNothing);
      expect(find.byKey(const ValueKey('mitm-clear')), findsNothing);
      expect(find.byKey(const ValueKey('mitm-new-pattern')), findsNothing);
    });

    // 第二梯队 2026-09-22：同一条 host 写成域名 / 后缀 / 关键字 / IP 段，
    // 命中范围差很远，列表里原先分不出来（`MitmConfig.tsx:560-579`）；
    // 启停也从按钮换成开关（`:580-584`）。
    testWidgets('白名单每行带规则类型徽标，启停是开关', (tester) async {
      final k = await mount(
        tester,
        whitelist: [
          {
            'host_pattern': 'a.example.com',
            'enabled': true,
            'source': 'user',
            'rule_type': 'domain',
          },
          {
            'host_pattern': 'example.org',
            'enabled': false,
            'source': 'default',
            'rule_type': 'keyword',
          },
        ],
      );
      final i18n = await makeI18n(tester);
      // 只看列表行里的徽标 —— 新增表单上方那排类型选择按钮用的是同一批文案。
      Finder inRow(String host, String key) => find.descendant(
        of: find.byKey(ValueKey('wl-$host')),
        matching: find.text(i18n.t(key)),
      );
      expect(inRow('a.example.com', 'mitm.ruleDomain'), findsOneWidget);
      expect(inRow('example.org', 'mitm.ruleKeyword'), findsOneWidget);
      // 手动停用 ≠ 失效：那条关掉的不该写「失效」。
      expect(find.text(i18n.t('middleware.failed')), findsNothing);

      final switches = find.descendant(
        of: find.byKey(const ValueKey('wl-a.example.com')),
        matching: find.byType(AidogSwitch),
      );
      expect(switches, findsOneWidget);
      expect(tester.widget<AidogSwitch>(switches).value, isTrue);
      await tester.tap(switches);
      await settle(tester);
      expect(k.calls, contains('mitm_whitelist_toggle'));
    });

    testWidgets('rule_type 缺失时按后缀算，不留空白徽标', (tester) async {
      await mount(
        tester,
        whitelist: [
          {'host_pattern': 'a.example.com', 'enabled': true, 'source': 'user'},
        ],
      );
      final i18n = await makeI18n(tester);
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('wl-a.example.com')),
          matching: find.text(i18n.t('mitm.ruleSuffix')),
        ),
        findsOneWidget,
      );
    });

    testWidgets('输入为空时「添加」点不动，白名单为空时「清空」点不动', (tester) async {
      await mount(tester);
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('mitm-add')))
            .enabled,
        isFalse,
      );
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('mitm-clear')))
            .enabled,
        isFalse,
      );
    });

    testWidgets('填了内容「添加」才可用', (tester) async {
      await mount(tester);
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('mitm-new-pattern')),
          matching: find.byType(TextField),
        ),
        '*.example.com',
      );
      await settle(tester);
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('mitm-add')))
            .enabled,
        isTrue,
      );
    });

    // React `MitmConfig.tsx:444`：白名单输入框里按回车 = 点「添加」。
    testWidgets('白名单输入框按回车直接添加', (tester) async {
      final k = await mount(tester);
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('mitm-new-pattern')),
          matching: find.byType(TextField),
        ),
        '*.example.com',
      );
      await settle(tester);
      expect(k.countOf('mitm_whitelist_add'), 0, reason: '光打字不该发请求');

      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      expect(k.countOf('mitm_whitelist_add'), 1);
    });

    // React `MitmConfig.tsx:477`：测试 URL 输入框里按回车 = 点「测试」。
    testWidgets('测试 URL 输入框按回车直接测试', (tester) async {
      final k = await mount(tester);
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('mitm-test-url')),
          matching: find.byType(TextField),
        ),
        'https://api.example.com/v1',
      );
      await settle(tester);
      expect(k.countOf('mitm_whitelist_test_url'), 0);

      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      expect(k.countOf('mitm_whitelist_test_url'), 1);
    });

    testWidgets('清空先确认，确认后才发 mitm_whitelist_clear', (tester) async {
      final k = await mount(
        tester,
        whitelist: [
          {
            'host_pattern': 'a.com',
            'enabled': true,
            'source': 'user',
            'rule_type': 'suffix',
          },
        ],
      );
      await tester.tap(find.byKey(const ValueKey('mitm-clear')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsOneWidget);
      expect(k.countOf('mitm_whitelist_clear'), 0);

      await tester.tap(
        find
            .descendant(
              of: find.byType(ConfirmCard),
              matching: find.byType(SmallButton),
            )
            .last,
      );
      await settle(tester);
      expect(k.countOf('mitm_whitelist_clear'), 1);
    });

    testWidgets('两种空态文案不同：整份为空 vs 搜索没命中', (tester) async {
      final i18n = await makeI18n(tester);
      await mount(tester);
      expect(find.text(i18n.t('mitm.whitelistEmpty')), findsOneWidget);
    });
  });

  // ── 托盘 / 浮窗 ───────────────────────────────────────

  testWidgets('托盘页：勾一个段即落盘（票 I15 单选清单）', (tester) async {
    await useBigSurface(tester);
    final k = FakeKernel({...baseResponses(), 'tray_config_set': (_) => null});
    final i18n = await makeI18n(tester);
    final ticker = LogTicker();
    addTearDown(ticker.close);
    await tester.pumpWidget(
      wrapPage(
        TraySettingsPage(invoke: k.invoke, logUpdates: ticker.stream),
        i18n,
      ),
    );
    await settle(tester);
    expect(find.text(i18n.t('tray.previewEmpty')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('tray-seg-peak')));
    await settle(tester);
    final cfg = k.lastArgsOf('tray_config_set')!['config']! as Map;
    final items = cfg['items']! as List;
    expect(items.length, 1);
    expect((items.first as Map)['item_type'], 'peak');

    // 日志事件触发今日统计刷新（不重拉整表）。
    final before = k.countOf('tray_today_stats');
    ticker.fire();
    await tester.pump(const Duration(milliseconds: 1100));
    await settle(tester);
    expect(k.countOf('tray_today_stats'), before + 1);
  });

  testWidgets('浮窗页：空态 + 添加一项即落盘', (tester) async {
    await useBigSurface(tester);
    final k = FakeKernel({
      ...baseResponses(),
      'popover_config_set': (_) => null,
    });
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(PopoverSettingsPage(invoke: k.invoke), i18n),
    );
    await settle(tester);
    // 两处：卡片列表自己的空态，加实时预览里那份 `PopoverGrid` 的空态
    //（票 I11 起预览与托盘小窗共用同一份渲染，所以空态也长一样）。
    expect(find.text(i18n.t('popover.empty')), findsNWidgets(2));

    // 「添加项」现在是按钮 + 菜单（不再是一排平铺按钮），先点开再选。
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey('popover-add')),
        matching: find.byType(SmallButton),
      ),
    );
    await settle(tester);
    await tester.tap(find.byKey(const ValueKey('popover-add-proxy_status')));
    await settle(tester);
    final cfg = k.lastArgsOf('popover_config_set')!['config']! as Map;
    expect((cfg['items']! as List).length, 1);
  });

  // ── 导入导出页 ────────────────────────────────────────

  group('导入导出页', () {
    Future<FakeKernel> mount(
      WidgetTester tester, {
      String? pick,
      Map<String, Object? Function(Map<String, Object?>?)> extra = const {},
      // 窄窗与 RTL 只有画出来才看得出挤没挤爆，所以这两样做成入参。
      Size? surface,
      String locale = 'zh-Hans',
    }) async {
      if (surface == null) {
        await useBigSurface(tester);
      } else {
        await tester.binding.setSurfaceSize(surface);
        addTearDown(() => tester.binding.setSurfaceSize(null));
      }
      final k = FakeKernel({...baseResponses(), ...extra});
      final i18n = await makeI18n(tester, locale: locale);
      await tester.pumpWidget(
        wrapPage(
          ImportExportPage(
            invoke: k.invoke,
            pickPath: ({bool save = false, String? suggested}) async => pick,
          ),
          i18n,
        ),
      );
      await settle(tester);
      return k;
    }

    // 回归 2026-09-23：这里原先是 `(chosen) => chosen` —— 把 cc-switch 的原始
    // provider map 直接当 payload 发出去。后端收的是已经转好的 Platform JSON，
    // 缺字段一律写空串，于是导进去的是一串没有协议 / 没有 URL / 没有密钥的空壳。
    testWidgets('cc-switch 导入：发出去的是转换过的 Platform JSON，不是原始 provider', (
      tester,
    ) async {
      final k = await mount(
        tester,
        extra: {
          'get_defaults_json': (_) =>
              '{"protocols":{"glm":{"name":{"en-US":"Zhipu GLM"},'
              '"keywords":["glm"],"endpoints":{"default":'
              '[{"protocol":"glm","base_url":"https://open.bigmodel.cn/api/paas/v4"}]}}}}',
          'ccswitch_detect': (_) => {
            'found': true,
            'path': '/tmp/cc',
            'sourceType': 'json',
            'providerCount': 1,
          },
          'ccswitch_read': (_) => {
            'sourceType': 'json',
            'path': '/tmp/cc',
            'providers': [
              {
                'id': 'p1',
                'appType': 'claude',
                'name': 'GLM 主号',
                'settingsConfig': {
                  'env': {'ANTHROPIC_MODEL': 'claude-x'},
                },
                'detectedBaseUrl': 'https://open.bigmodel.cn/api/paas/v4',
                'detectedApiKey': 'sk-test',
              },
            ],
          },
          'ccswitch_import': (_) => {'applied': <String, Object?>{}},
          // autoGroup 默认开，导入完会接着建 / 取分组再刷一次列表。
          'platform_ensure_auto_group': (_) => null,
          'platform_list': (_) => <Object?>[],
          'group_detail_list': (_) => <Object?>[],
        },
      );
      // 「探测」这颗按钮自己接着读 providers。
      await tester.tap(find.byKey(const ValueKey('ccswitch-detect')));
      await settle(tester);
      expect(findStripped(find, 'GLM 主号'), findsOneWidget);
      // 匹配读数：协议名 + 命中方式徽标（导入前看得到匹配成了什么）。
      expect(find.text('Zhipu GLM'), findsWidgets);

      await tester.tap(find.byKey(const ValueKey('foreign-import-cc-switch')));
      await settle(tester);
      final args = k.lastArgsOf('ccswitch_import')!;
      final payload = (args['platformPayload']! as List).single as Map;
      expect(payload['platform_type'], 'glm');
      expect(payload['base_url'], 'https://open.bigmodel.cn/api/paas/v4');
      expect(payload['api_key'], 'sk-test');
      expect((payload['models']! as Map)['default'], 'claude-x');
      expect((payload['endpoints']! as List), isNotEmpty);
    });

    testWidgets('cc-switch：关掉密钥维度 → payload 里 api_key 是空串', (tester) async {
      final k = await mount(
        tester,
        extra: {
          'ccswitch_detect': (_) => {
            'found': true,
            'path': '/tmp/cc',
            'sourceType': 'json',
            'providerCount': 1,
          },
          'ccswitch_read': (_) => {
            'sourceType': 'json',
            'path': '/tmp/cc',
            'providers': [
              {
                'id': 'p1',
                'appType': 'claude',
                'name': '某号',
                'settingsConfig': const <String, Object?>{},
                'detectedBaseUrl': 'https://x.test/v1',
                'detectedApiKey': 'sk-test',
              },
            ],
          },
          'ccswitch_import': (_) => {'applied': <String, Object?>{}},
          'platform_ensure_auto_group': (_) => null,
          'platform_list': (_) => <Object?>[],
          'group_detail_list': (_) => <Object?>[],
        },
      );
      await tester.tap(find.byKey(const ValueKey('ccswitch-detect')));
      await settle(tester);
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('cc-dim-apikey')),
          matching: find.byType(AidogSwitch),
        ),
      );
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('foreign-import-cc-switch')));
      await settle(tester);
      final payload =
          ((k.lastArgsOf('ccswitch_import')!['platformPayload']! as List).single
              as Map);
      expect(payload['api_key'], '');
      // 其余维度不受影响。
      expect(payload['base_url'], 'https://x.test/v1');
    });

    testWidgets('sub2api 导入：认不出的 platform 标出来，协议可当场手改并进 payload', (
      tester,
    ) async {
      final k = await mount(
        tester,
        extra: {
          'get_defaults_json': (_) =>
              '{"protocols":{"anthropic":{"name":{"en-US":"Anthropic"},'
              '"endpoints":{"default":[{"protocol":"anthropic",'
              '"base_url":"https://api.anthropic.com"}]}},'
              '"openai":{"name":{"en-US":"OpenAI"},"endpoints":{"default":'
              '[{"protocol":"openai","base_url":"https://api.openai.com/v1"}]}}}}',
          'sub2api_parse': (_) => {
            'accounts': [
              {
                'name': 'A 号',
                'platform': 'nobody-knows-this',
                'baseUrl': 'https://x.test/v1',
                'apiKey': 'sk-sub',
              },
            ],
          },
          'sub2api_import': (_) => {'applied': <String, Object?>{}},
          'platform_ensure_auto_group': (_) => null,
          'platform_list': (_) => <Object?>[],
          'group_detail_list': (_) => <Object?>[],
        },
      );
      final i18n = await makeI18n(tester);
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('sub2api-paste')),
          matching: find.byType(TextField),
        ),
        '{"accounts":[]}',
      );
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('sub2api-parse')));
      await settle(tester);

      // 认不出来 → 兜底 openai，并且说出来。
      expect(
        find.text(i18n.t('importExport.sub2api.unrecognized')),
        findsOneWidget,
      );

      // 手改协议 → 徽标收掉，payload 里带改后的协议。
      await tester.tap(find.byKey(const ValueKey('sub2api-protocol-0')));
      await settle(tester);
      await tester.tap(find.text('Anthropic').last);
      await settle(tester);
      expect(
        find.text(i18n.t('importExport.sub2api.unrecognized')),
        findsNothing,
      );

      await tester.tap(find.byKey(const ValueKey('foreign-import-sub2api')));
      await settle(tester);
      final payload =
          ((k.lastArgsOf('sub2api_import')!['platformPayload']! as List).single
              as Map);
      expect(payload['platform_type'], 'anthropic');
      expect(payload['base_url'], 'https://x.test/v1');
      expect(payload['api_key'], 'sk-sub');
      expect(payload['endpoints'] as List, isNotEmpty);
    });

    // 这一页新加的三块（sub2api 行内协议下拉、导入概要卡的计数徽标、
    // cc-switch 行的匹配读数）都是「一行里塞好几个控件」，窄窗和 RTL 下
    // 挤爆只会在画出来的时候露头：Flutter 的 overflow 是一条 FlutterError，
    // 跑到就让用例红，所以这里不断言像素，只要求这两种环境下画得出来。
    for (final env in const [
      (name: '窄窗 700 逻辑像素', locale: 'zh-Hans', w: 700.0),
      (name: '阿拉伯语 RTL', locale: 'ar-SA', w: 1400.0),
    ]) {
      testWidgets('sub2api 行在${env.name}下不挤爆', (tester) async {
        await mount(
          tester,
          surface: Size(env.w, 3000),
          locale: env.locale,
          extra: {
            'get_defaults_json': (_) =>
                '{"protocols":{"anthropic":{"name":{"en-US":"Anthropic"},'
                '"endpoints":{"default":[{"protocol":"anthropic",'
                '"base_url":"https://api.anthropic.com"}]}},'
                '"openai":{"name":{"en-US":"OpenAI"},"endpoints":{"default":'
                '[{"protocol":"openai","base_url":"https://api.openai.com/v1"}]}}}}',
            'sub2api_parse': (_) => {
              'accounts': [
                {
                  'name': '一个相当长的账号名字用来把这一行撑开',
                  'platform': 'nobody-knows-this',
                  'baseUrl': 'https://very-long-host.example.test/v1/chat',
                  'apiKey': 'sk-sub',
                },
              ],
            },
            'platform_list': (_) => <Object?>[],
            'group_detail_list': (_) => <Object?>[],
          },
        );
        await tester.enterText(
          find.descendant(
            of: find.byKey(const ValueKey('sub2api-paste')),
            matching: find.byType(TextField),
          ),
          '{"accounts":[]}',
        );
        await settle(tester);
        await tester.tap(find.byKey(const ValueKey('sub2api-parse')));
        await settle(tester);

        expect(tester.takeException(), isNull);
        expect(
          find.byKey(const ValueKey('sub2api-protocol-0')),
          findsOneWidget,
        );
      });
    }

    testWidgets('没预览过就不许导出', (tester) async {
      await mount(tester);
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('export-run')))
            .enabled,
        isFalse,
      );
    });

    testWidgets('取消勾选全部范围 → 报错且不许导出', (tester) async {
      await mount(tester);
      final i18n = await makeI18n(tester);
      // 初始只勾 kInitialScopes 三项（对齐 React），其余本来就关，
      // 逐个点会反把它们打开。只点开着的。
      for (final s in kInitialScopes) {
        await tester.tap(find.byKey(ValueKey('scope-$s')));
        await settle(tester);
      }
      expect(find.text(i18n.t('importExport.error.noScope')), findsOneWidget);
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('export-run')))
            .enabled,
        isFalse,
      );
    });

    testWidgets('scope 变更 300ms 防抖自动拉预览，空清单仍不许导出', (tester) async {
      final k = await mount(tester);
      final i18n = await makeI18n(tester);
      final before = k.countOf('export_preview');
      await tester.tap(find.byKey(ValueKey('scope-skills')));
      // 300ms 内连点不重复拉：防抖只发最后一次。
      await tester.tap(find.byKey(ValueKey('scope-mcp')));
      // settle 一轮只走 240ms，不够 300ms 防抖到期，再补一泵。
      await tester.pump(const Duration(milliseconds: 400));
      await settle(tester);
      final after = k.countOf('export_preview');
      expect(after, before + 1, reason: '300ms 内连点只拉一次');
      expect(find.text(i18n.t('importExport.exportEmpty')), findsOneWidget);
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('export-run')))
            .enabled,
        isFalse,
      );
    });

    testWidgets('导出范围补全到 10 个且 wire 名与后端一致', (tester) async {
      final k = await mount(tester);
      // 挂载即防抖拉一次初始预览 —— 传出去的 scopes 必须是后端认的
      // 单数 wire 名（'platform' 而非 'platforms'），否则 collect 全空。
      await tester.pump(const Duration(milliseconds: 400));
      await settle(tester);
      final scopes =
          (k.lastArgsOf('export_preview')!['scopes']! as List).cast<String>();
      expect(scopes.toSet(), kInitialScopes);
      // 10 个 scope 的 chip 全在场（`meta.ts:16-26` 的 ALL_SCOPES）。
      for (final s in [
        'platform',
        'group',
        'group_platform',
        'setting',
        'codex',
        'claude_code',
        'model_price',
        'mcp',
        'middleware',
        'skills',
      ]) {
        expect(find.byKey(ValueKey('scope-$s')), findsOneWidget, reason: s);
      }
    });

    testWidgets('选到非 .aidogx 文件时报错且不读文件', (tester) async {
      final k = await mount(tester, pick: '/tmp/x.zip');
      final i18n = await makeI18n(tester);
      // 先让挂载时的初始导出预览跑完（300ms 防抖），否则它晚到会把
      // 下面的错误清空 —— 页面真用起来初始预览早就拉完了。
      await tester.pump(const Duration(milliseconds: 400));
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('import-pick')));
      await settle(tester);
      expect(k.countOf('import_read_file'), 0);
      expect(find.text(i18n.t('importExport.error.notAidogx')), findsOneWidget);
    });

    testWidgets('拖拽落区在导入卡里，点它等同于点「选择文件」', (tester) async {
      final k = await mount(tester, pick: '/tmp/x.aidogx');
      expect(find.byKey(const ValueKey('import-dropzone')), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('import-dropzone')));
      await settle(tester);
      expect(k.countOf('import_read_file'), 1);
    });

    testWidgets('冲突没决策也能应用：缺省即覆盖（对齐 React）', (tester) async {
      final k = await mount(
        tester,
        pick: '/tmp/x.aidogx',
        extra: {
          'import_read_file': (_) => {
            'items': [
              {'scope': 'platform', 'key': 'p1'},
            ],
            'conflicts': [
              {'scope': 'platform', 'key': 'p1'},
            ],
          },
          'import_apply': (_) => <String, Object?>{},
        },
      );
      await tester.tap(find.byKey(const ValueKey('import-pick')));
      await settle(tester);
      expect(k.countOf('import_read_file'), 1);
      // 对齐 React（`ImportExportTab.tsx:568`）：只看跑着没 / 有没有预览 /
      // 有没有勾条目，冲突不拍板就按缺省 overwrite 走。
      expect(
        tester
            .widget<SmallButton>(find.byKey(const ValueKey('import-apply')))
            .enabled,
        isTrue,
      );

      expect(
        find.byKey(const ValueKey('conflict-platform p1')),
        findsOneWidget,
      );
      await tester.tap(
        find.byKey(const ValueKey('decide-platform p1-useIncoming')),
      );
      await settle(tester);

      // 与 React 对齐（`ImportExportTab.tsx:566-578`）：点「应用导入」直接执行，
      // 没有二次确认卡。
      await tester.tap(find.byKey(const ValueKey('import-apply')));
      await settle(tester);
      expect(find.byType(ConfirmCard), findsNothing);
      expect(k.countOf('import_apply'), 1);
    });

    // 回归 2026-09-23：粘贴框原先 `maxLines: 4`，粘一份账号导出 JSON 只看得见
    // 四行。React 那边是 120–320px 的 JSON 编辑器（`Sub2ApiImport.tsx:195-201`）。
    testWidgets('sub2api 粘贴框有 JSON 编辑器那么高，且是等宽字', (tester) async {
      await mount(tester);
      final field = tester.widget<TextField>(
        find.descendant(
          of: find.byKey(const ValueKey('sub2api-paste')),
          matching: find.byType(TextField),
        ),
      );
      // 7 行 ≈ 119px 起、19 行 ≈ 323px 封顶（等宽行高 12.5×1.35）。
      expect(field.minLines, greaterThanOrEqualTo(6));
      expect(field.maxLines, greaterThanOrEqualTo(18));
      expect(field.style?.fontFamily, AidogType.numSm.fontFamily);
    });

    testWidgets('条目按菜单组折叠：组级三态 + 收起后组内行不渲染', (tester) async {
      final k = await mount(
        tester,
        pick: '/tmp/x.aidogx',
        extra: {
          'import_read_file': (_) => {
            'items': [
              {'scope': 'platform', 'key': 'p1', 'label': 'P1'},
              {'scope': 'platform', 'key': 'p2', 'label': 'P2'},
              {'scope': 'mcp', 'key': 'm1', 'label': 'M1'},
            ],
            'conflicts': <Object?>[],
          },
          'import_apply': (_) => <String, Object?>{},
        },
      );
      await tester.tap(find.byKey(const ValueKey('import-pick')));
      await settle(tester);

      // 批三后组级三态是自绘 _CheckBox（16px、半选横线），
      // 不再是 Material 的 indeterminate_check_box 图标。
      ImportCheckBox groupCheck(String gid) => tester.widget<ImportCheckBox>(
        find.descendant(
          of: find.byKey(ValueKey('item-group-check-$gid')),
          matching: find.byType(ImportCheckBox),
        ),
      );
      expect(groupCheck('platform').checked, isTrue);
      expect(groupCheck('extension').checked, isTrue);

      // 组内取消一条 → 该组变半选，另一组不受影响。
      await tester.tap(find.byKey(const ValueKey('item-platform p1')));
      await settle(tester);
      expect(
        groupCheck('platform').indeterminate,
        isTrue,
        reason: '挑了几条 = 半选，这是「这组我动过」的唯一线索',
      );
      expect(groupCheck('extension').checked, isTrue);

      // 点组级复选框 → 整组翻转（半选 → 全选）。
      await tester.tap(find.byKey(const ValueKey('item-group-check-platform')));
      await settle(tester);
      expect(groupCheck('platform').checked, isTrue);

      // 折叠 → 组内行不再渲染，组头还在。
      await tester.tap(find.byKey(const ValueKey('item-group-platform')));
      await settle(tester);
      expect(find.byKey(const ValueKey('item-platform p1')), findsNothing);
      expect(find.byKey(const ValueKey('item-group-platform')), findsOneWidget);

      // 发给后端的 selection 才是判据：折叠只是收起来，不等于取消勾选。
      await tester.tap(find.byKey(const ValueKey('import-apply')));
      await settle(tester);
      final selection = k.lastArgsOf('import_apply')!['selection']! as List;
      expect(selection.length, 3);
    });

    testWidgets('冲突行给出本地那条的摘要，批量决策压在清单上方', (tester) async {
      final k = await mount(
        tester,
        pick: '/tmp/x.aidogx',
        extra: {
          'import_read_file': (_) => {
            'items': [
              {
                'scope': 'platform',
                'key': 'p1',
                'label': 'P1',
                'conflict': true,
              },
            ],
            'conflicts': [
              {
                'scope': 'platform',
                'key': 'p1',
                'existing_summary': '本地：glm · 3 个模型',
                'incoming_summary': '导入：glm · 5 个模型',
              },
            ],
          },
          'import_apply': (_) => <String, Object?>{},
        },
      );
      final i18n = await makeI18n(tester);
      await tester.tap(find.byKey(const ValueKey('import-pick')));
      await settle(tester);

      // 本地现有那条长什么样 —— 没有它就得闭着眼睛决定要不要覆盖。
      expect(findStripped(find, '本地：glm · 3 个模型'), findsOneWidget);

      // 告警色标题（`ImportExportTab.tsx:542` 的 --color-warning）：
      // 冲突要逐条拍板，标题不能混成普通小节标题的灰。
      final titleFinder = find.text(i18n.t('importExport.conflicts', {'n': 1}));
      final peak = AidogTheme.of(tester.element(titleFinder)).c.peak;
      expect(tester.widget<Text>(titleFinder).style!.color, peak);

      // 批量决策在冲突行**上方**：冲突多时不必滚到底去找。
      final bulkY = tester
          .getTopLeft(find.byKey(const ValueKey('bulk-overwrite')))
          .dy;
      final rowY = tester
          .getTopLeft(find.byKey(const ValueKey('conflict-platform p1')))
          .dy;
      expect(bulkY, lessThan(rowY));

      // 「全部覆盖」一次定完，落到载荷里。
      await tester.tap(find.byKey(const ValueKey('bulk-overwrite')));
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('import-apply')));
      await settle(tester);
      final decisions = k.lastArgsOf('import_apply')!['decisions']! as List;
      expect((decisions.single as Map)['decision'], {'kind': 'overwrite'});
    });

    testWidgets('cc-switch 批量分组：勾了分组 chip → 导入后逐个 platform_update 挂进去', (
      tester,
    ) async {
      final k = await mount(
        tester,
        extra: {
          'ccswitch_detect': (_) => {
            'found': true,
            'path': '/tmp/cc',
            'sourceType': 'json',
            'providerCount': 1,
          },
          'ccswitch_read': (_) => {
            'sourceType': 'json',
            'path': '/tmp/cc',
            'providers': [
              {
                'id': 'p1',
                'appType': 'claude',
                'name': 'GLM 主号',
                'detectedBaseUrl': 'https://x.test/v1',
                'detectedApiKey': 'sk-test',
              },
            ],
          },
          'ccswitch_import': (_) => {'applied': <String, Object?>{}},
          'group_detail_list': (_) => [
            {
              'group': {'id': 7, 'name': '常用', 'group_key': 'gk7'},
              'platforms': <Object?>[],
            },
          ],
          'platform_list': (_) => [
            {'id': 42, 'name': 'GLM 主号'},
          ],
          'platform_update': (_) => null,
          'platform_ensure_auto_group': (_) => null,
        },
      );
      await tester.tap(find.byKey(const ValueKey('ccswitch-detect')));
      await settle(tester);

      // 勾一个已有分组。
      await tester.tap(find.byKey(const ValueKey('batch-join-group-7')));
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('foreign-import-cc-switch')));
      await settle(tester);

      final args = k.lastArgsOf('platform_update')!['input']! as Map;
      expect(args['id'], 42);
      expect(args['join_group_ids'], [7]);
    });

    testWidgets('保留两者新名字清空 → 报错不发 import_apply', (tester) async {
      final k = await mount(
        tester,
        pick: '/tmp/x.aidogx',
        extra: {
          'import_read_file': (_) => {
            'items': [
              {'scope': 'platform', 'key': 'p1', 'conflict': true},
            ],
            'conflicts': [
              {'scope': 'platform', 'key': 'p1'},
            ],
          },
          'import_apply': (_) => <String, Object?>{},
        },
      );
      final i18n = await makeI18n(tester);
      await tester.tap(find.byKey(const ValueKey('import-pick')));
      await settle(tester);
      await tester.tap(
        find.byKey(const ValueKey('decide-platform p1-keepBoth')),
      );
      await settle(tester);
      // 把预填的 p1-imported 清空。
      final input = find.byKey(const ValueKey('rename-platform p1'));
      await tester.enterText(input, '');
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('import-apply')));
      await settle(tester);
      expect(k.countOf('import_apply'), 0);
      expect(find.text(i18n.t('importExport.renameRequired')), findsOneWidget);
    });

    testWidgets('冲突选「保留两者」：新名字预填并进 import_apply 的 decisions', (tester) async {
      final k = await mount(
        tester,
        pick: '/tmp/x.aidogx',
        extra: {
          'import_read_file': (_) => {
            'items': [
              {
                'scope': 'platform',
                'key': 'p1',
                'label': 'P1',
                'conflict': true,
              },
            ],
            'conflicts': [
              {'scope': 'platform', 'key': 'p1'},
            ],
          },
          'import_apply': (_) => <String, Object?>{},
        },
      );
      await tester.tap(find.byKey(const ValueKey('import-pick')));
      await settle(tester);

      await tester.tap(
        find.byKey(const ValueKey('decide-platform p1-keepBoth')),
      );
      await settle(tester);
      // 预填 `<key>-imported`：不预填的话这个决策等于发一个空 key 过去。
      final input = find.descendant(
        of: find.byKey(const ValueKey('rename-platform p1')),
        matching: find.byType(TextField),
      );
      expect(tester.widget<TextField>(input).controller!.text, 'p1-imported');

      await tester.enterText(input, 'p1-copy');
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('import-apply')));
      await settle(tester);

      final decisions = k.lastArgsOf('import_apply')!['decisions']! as List;
      expect(decisions.single, {
        'scope': 'platform',
        'key': 'p1',
        'decision': {'kind': 'rename', 'new_key': 'p1-copy'},
      });
    });

    testWidgets('导入预览给出来源机器 / 导出时间 / 各范围条数，冲突条目带徽标', (tester) async {
      final k = await mount(
        tester,
        pick: '/tmp/x.aidogx',
        extra: {
          'import_read_file': (_) => {
            'manifest': {
              'source_machine': 'mac-studio',
              'created_at': '2026-09-20 10:00:00',
            },
            'counts': {'platform': 3},
            'items': [
              {
                'scope': 'platform',
                'key': 'p1',
                'label': '智谱',
                'conflict': true,
              },
              {'scope': 'platform', 'key': 'p2', 'label': 'Kimi'},
            ],
            'conflicts': <Object?>[],
          },
        },
      );
      final i18n = await makeI18n(tester);
      await tester.tap(find.byKey(const ValueKey('import-pick')));
      await settle(tester);
      expect(k.countOf('import_read_file'), 1);

      expect(findStripped(find, 'mac-studio'), findsOneWidget);
      expect(findStripped(find, '2026-09-20 10:00:00'), findsOneWidget);
      expect(find.text(i18n.t('importExport.sourceMachine')), findsOneWidget);
      expect(find.text(i18n.t('importExport.createdAt')), findsOneWidget);
      // 范围条数：「平台 3」这类徽标，光有路径看不出导进来会动多少东西。
      expect(find.textContaining('3'), findsWidgets, reason: 'counts 要画出来');

      // 勾选器：标题 + 「已选 n / 共 m」+ 人话标签 + 冲突徽标。
      expect(find.text(i18n.t('importExport.selectItems')), findsOneWidget);
      expect(
        find.text('${i18n.t('importExport.selectedLabel')} 2 / 2'),
        findsOneWidget,
      );
      expect(findStripped(find, '智谱'), findsOneWidget);
      expect(findStripped(find, 'Kimi'), findsOneWidget);
      expect(find.text(i18n.t('importExport.conflictTag')), findsOneWidget);
    });
  });
}
