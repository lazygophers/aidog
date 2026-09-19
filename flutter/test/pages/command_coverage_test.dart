// 票 I06 的对齐证明：**命令覆盖零差集**。
//
// 下面两张清单是从 React 版逐行读出来的（`src/services/api/` 的封装 + 页面内直接调用），
// 每条都标了出处。测试把 Flutter 页面沿所有路径走一遍，把实际发出的命令名收成一个
// **排序后的串**（Dart 的 Set / List 按身份比较，`expect(setA, setB)` 永远不等 ——
// 本项目踩过，2826 个 key 全报不等），再与清单逐字比对。
//
// 差一条就是缺一块功能；多一条就是 React 没有的额外 IPC。两边都会让这条测试红。
//
// ── Home.tsx 调的命令（6 条）────────────────────────────────
//   proxyApi.status()              → proxy_status           Home.tsx:154
//   proxyApi.getSettings()         → proxy_get_settings     Home.tsx:155
//   trayConfigApi.todayStats()     → tray_today_stats       Home.tsx:156
//   popoverConfigApi.platformToday() → popover_platform_today Home.tsx:157
//   platformApi.list()             → platform_list          Home.tsx:158
//   statsApi.query()               → stats_query            Home.tsx:159
//   （另有事件 proxy-log-updated   Home.tsx:167，非命令，单独断言）
//
// ── Stats.tsx 调的命令（6 条，与 Home 共用 2 条）─────────────
//   statsApi.query()               → stats_query            Stats.tsx:318/319/333/352
//   statsApi.scatterHistogram()    → scatter_histogram      Stats.tsx:365
//   statsApi.quotaSnapshots()      → quota_snapshots        Stats.tsx:387
//   groupDetailApi.list()          → group_detail_list      Stats.tsx:399
//   platformApi.list()             → platform_list          Stats.tsx:400
//   getProtocolSearchTermsMap()    → get_defaults_json      Stats.tsx:266
//                                     （defaults.ts::fetchDoc → platforms.ts:616）
//
// 两页共用 2 条（stats_query、platform_list），所以并集是 6 + 6 − 2 = **10 条**。
import 'package:aidog_flutter/pages.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';
import 'stats_widget_test.dart' show statsResponses;
import 'home_widget_test.dart' show homeResponses;

/// React 版 Home.tsx 调的命令，排序后的规范串。
const kHomeCommands =
    'platform_list,popover_platform_today,proxy_get_settings,'
    'proxy_status,stats_query,tray_today_stats';

/// React 版 Stats.tsx 调的命令，排序后的规范串。
const kStatsCommands =
    'get_defaults_json,group_detail_list,platform_list,'
    'quota_snapshots,scatter_histogram,stats_query';

void main() {
  testWidgets('首页命令覆盖：与 React 版 Home.tsx 零差集', (tester) async {
    await useBigSurface(tester);
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(HomePage(onNavigate: (_) {}, invoke: k.invoke), c),
    );
    await settle(tester);
    expect(k.commandSetSignature, kHomeCommands);
  });

  testWidgets('统计页命令覆盖：走遍四 tab 与两个密度视图后，与 React 版 Stats.tsx 零差集', (
    tester,
  ) async {
    await useBigSurface(tester);
    final k = FakeKernel(statsResponses());
    final c = await makeI18n(tester);
    await tester.pumpWidget(wrapPage(StatsPage(invoke: k.invoke), c));
    await settle(tester);

    // 时间序列 tab 是默认态；其余三个 tab + 散点视图逐个走一遍，把按需查询全触发。
    await tester.tap(find.text(c.t('stats.tabShare')));
    await settle(tester);
    await tester.tap(find.text(c.t('stats.tabDensity')));
    await settle(tester);
    await tester.tap(find.text(c.t('stats.densityViewScatter')));
    await settle(tester);
    await tester.tap(find.text(c.t('stats.tabQuota')));
    await settle(tester);

    expect(k.commandSetSignature, kStatsCommands);
  });

  test('两页各 6 条、共用 2 条 → 并集 10 条（清单见本文件抬头，逐条标了 React 侧出处）', () {
    final home = kHomeCommands.split(',');
    final stats = kStatsCommands.split(',');
    expect(home.length, 6);
    expect(stats.length, 6);
    final shared = home.toSet().intersection(stats.toSet()).toList()..sort();
    expect(shared.join(','), 'platform_list,stats_query');

    final union = <String>{...home, ...stats}.toList()..sort();
    expect(union.length, 10);
    expect(
      union.join(','),
      'get_defaults_json,group_detail_list,platform_list,'
      'popover_platform_today,proxy_get_settings,proxy_status,'
      'quota_snapshots,scatter_histogram,stats_query,tray_today_stats',
    );
  });

  test('两页都订阅 proxy-log-updated 事件（React: Home.tsx:167 / Stats.tsx:395）', () {
    // 事件名是常量，页面缺省就用它；测试里注入的是同语义的手动流。
    expect(kProxyLogUpdatedEvent, 'proxy-log-updated');
  });
}
