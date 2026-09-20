/// 票 I15：托盘配置从二维网格降成「最多挑 3 项」。
/// 与 React `src/pages/TrayConfigTab.test.tsx` 逐条对应 —— 两边同一套数据模型。
library;

import 'package:aidog_flutter/src/pages/settings/tray_logic.dart';
import 'package:flutter_test/flutter_test.dart';

String _t(String key, String fallback) => fallback;

TraySegmentOption _opt(String key) =>
    traySegmentOptions(const [], _t).firstWhere((o) => o.key == key);

void main() {
  group('traySegmentOptions', () {
    test('候选清单含今日 4 指标 + 命中平台 + 高峰 + 每个平台', () {
      final opts = traySegmentOptions(
        const [(id: 7, name: 'Acme', balance: 1, codingPlan: null)],
        _t,
      );
      expect(
        opts.map((o) => o.key),
        containsAllInOrder([
          'today_usage:tokens',
          'today_usage:cache_rate',
          'today_usage:cost',
          'today_usage:requests',
          'routed_platform',
          'peak',
          'platform:7',
        ]),
      );
    });
  });

  group('toggleTraySegment', () {
    test('取消勾选只关掉项，不删除，也不动二维字段', () {
      final items = [
        TrayItem.todayUsage('cost', 0).copyWith(lineMode: 'two', alignRow2: 'right'),
      ];
      final next = toggleTraySegment(items, _opt('today_usage:cost'));
      expect(next.length, 1);
      expect(next.first.enabled, isFalse);
      expect(next.first.lineMode, 'two');
      expect(next.first.alignRow2, 'right');
    });

    test('再勾回来复用原项，保留自定义标签', () {
      final items = [
        TrayItem.todayUsage('cost', 0).copyWith(label: '💰', enabled: false),
      ];
      final next = toggleTraySegment(items, _opt('today_usage:cost'));
      expect(next.first.enabled, isTrue);
      expect(next.first.label, '💰');
    });

    test('已满 3 项时再勾无效（原样返回）', () {
      final items = [
        TrayItem.todayUsage('cost', 0),
        TrayItem.todayUsage('tokens', 1),
        TrayItem.simple('peak', 2),
      ];
      expect(items.where((i) => i.enabled).length, kTrayMaxSegments);
      expect(
        identical(toggleTraySegment(items, _opt('routed_platform')), items),
        isTrue,
      );
    });

    test('关掉的项被排到启用项之后', () {
      final items = [
        TrayItem.todayUsage('cost', 0),
        TrayItem.todayUsage('tokens', 1),
      ];
      final next = toggleTraySegment(items, _opt('today_usage:cost'));
      expect(next.map(traySegmentKey).toList(), [
        'today_usage:tokens',
        'today_usage:cost',
      ]);
      expect(next.first.enabled, isTrue);
      expect(next.last.enabled, isFalse);
    });
  });

  group('computeItemText', () {
    test('命中平台 / 高峰由后端实时算，前端占位', () {
      expect(
        computeItemText(TrayItem.simple('routed_platform', 0), null, null, _t).value,
        '—',
      );
      expect(
        computeItemText(TrayItem.simple('peak', 0), null, null, _t).value,
        '—',
      );
    });

    test('今日费用取 todayStats.cost（与统计页同一份数据）', () {
      const stats = TodayStats(
        tokens: 5,
        cacheRate: 0,
        cost: 1.25,
        totalRequests: 2,
      );
      final text = computeItemText(TrayItem.todayUsage('cost', 0), null, stats, _t);
      expect(text.value, r'$1.25');
    });
  });
}
