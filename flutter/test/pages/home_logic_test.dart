// 首页纯函数单测。`normPoints` 那一组**逐条翻自 React 版 src/pages/Home.test.ts**，
// 输入数据与期望值一字不改 —— 这是「与现在 ship 的前端对齐」最硬的证据。
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/stats/models.dart';
import 'package:flutter_test/flutter_test.dart';

/// React 版返回 `Array<[number, number]>`，Dart 这边是 `(double, double)`。
/// 断言写成一对 `double` 的列表，值本身一字不改。
List<List<double>> pts(List<(double, double)> v) =>
    [for (final p in v) [p.$1, p.$2]];

StatsBucket bucket(
  String tb, {
  int requests = 0,
  double cost = 0,
  int input = 0,
  int output = 0,
  int cache = 0,
}) => StatsBucket(
  timeBucket: tb,
  totalRequests: requests,
  totalCost: cost,
  inputTokens: input,
  outputTokens: output,
  cacheTokens: cache,
);

TodayPlatformStat stat(int id, String name, double cost, int tokens, int req) =>
    TodayPlatformStat(
      platformId: id,
      platformName: name,
      cost: cost,
      tokens: tokens,
      requests: req,
    );

void main() {
  // ── 逐条翻自 src/pages/Home.test.ts ──────────────────────
  group('normPoints', () {
    test('空序列 → 空点集', () {
      expect(normPoints([], 100, 22), isEmpty);
    });

    test('单点 → 居中', () {
      expect(pts(normPoints([5], 100, 22)), [
        [50, 11],
      ]);
    });

    test('min-max 归一化：最大值顶 pad、最小值落 h-pad，x 均分覆盖全宽', () {
      final p = pts(normPoints([0, 5, 10], 100, 22, 2));
      expect(p[0], [0, 20]); // 最小值 → 底部
      expect(p[1], [50, 11]); // 中值 → 中线
      expect(p[2], [100, 2]); // 最大值 → 顶部
    });

    test('平坦序列 → 全部落中线（不除零）', () {
      expect(pts(normPoints([7, 7, 7], 90, 20, 2)), [
        [0, 10],
        [45, 10],
        [90, 10],
      ]);
    });

    test('负值 / 小数序列同规则', () {
      final p = normPoints([-2, 2], 40, 10, 1);
      expect(p[0].$2, closeTo(9, 1e-9));
      expect(p[1].$2, closeTo(1, 1e-9));
    });
  });

  // ── React 版没有单测、但页面里有分支的那些 ────────────────
  group('hasTodayData', () {
    TodayStats t({int req = 0, double cost = 0, int tokens = 0}) => TodayStats(
      tokens: tokens,
      inputTokens: 0,
      outputTokens: 0,
      cacheTokens: 0,
      cacheRate: 0,
      cost: cost,
      totalRequests: req,
    );

    test('null → false', () => expect(hasTodayData(null), isFalse));
    test('三项全 0 → false', () => expect(hasTodayData(t()), isFalse));
    test('requests > 0 → true', () => expect(hasTodayData(t(req: 1)), isTrue));
    test('cost > 0 → true', () => expect(hasTodayData(t(cost: 0.001)), isTrue));
    test(
      'tokens > 0 → true',
      () => expect(hasTodayData(t(tokens: 1)), isTrue),
    );
  });

  group('topPlatformsOf', () {
    test('滤掉全零行、按 cost 降序、截断到 4 条', () {
      final top = topPlatformsOf([
        stat(1, 'a', 1, 0, 0),
        stat(2, 'zero', 0, 0, 0), // 三项全零 → 滤掉
        stat(3, 'c', 5, 0, 0),
        stat(4, 'd', 3, 0, 0),
        stat(5, 'e', 2, 0, 0),
        stat(6, 'f', 0.5, 0, 0), // 第 5 名 → 截断
      ]);
      expect([for (final p in top) p.platformName], ['c', 'd', 'e', 'a']);
    });

    test('cost 为 0 但有 token 或请求的仍保留', () {
      final top = topPlatformsOf([
        stat(1, 'tokens-only', 0, 10, 0),
        stat(2, 'req-only', 0, 0, 3),
        stat(3, 'all-zero', 0, 0, 0),
      ]);
      expect([for (final p in top) p.platformName], [
        'tokens-only',
        'req-only',
      ]);
    });

    test('cost 并列时保持首现序（JS sort 稳定，Dart 的不保证）', () {
      final top = topPlatformsOf([
        stat(1, 'first', 2, 0, 0),
        stat(2, 'second', 2, 0, 0),
        stat(3, 'third', 2, 0, 0),
      ]);
      expect([for (final p in top) p.platformName], [
        'first',
        'second',
        'third',
      ]);
    });

    test('空列表 → 空结果', () => expect(topPlatformsOf(const []), isEmpty));
  });

  test('totalBalanceOf 求和；空列表 → 0', () {
    PlatformSummary p(double b) => PlatformSummary(
      id: 1,
      name: 'x',
      platformType: 'openai',
      estBalanceRemaining: b,
    );
    expect(totalBalanceOf(const []), 0);
    expect(totalBalanceOf([p(1.5), p(2.25), p(0)]), closeTo(3.75, 1e-9));
  });

  test('last24h 是 now-24h → now 的滚动窗口', () {
    final now = DateTime.fromMillisecondsSinceEpoch(1_700_000_000_000);
    final w = last24h(now);
    expect(w.end, 1_700_000_000_000);
    expect(w.end - w.start, 24 * 3600 * 1000);
  });

  test('trendSeriesOf：tokens = input + output + cache', () {
    final s = trendSeriesOf([
      bucket('2026-09-13 10:00:00', requests: 2, cost: 0.5, input: 1, output: 2, cache: 3),
      bucket('2026-09-13 11:00:00', requests: 0),
    ]);
    expect(s.requests, [2, 0]);
    expect(s.cost, [0.5, 0]);
    expect(s.tokens, [6, 0]);
    expect(s.cache, [3, 0]);
  });

  test('trendPeakOf / hasTrend：全零桶算空态', () {
    final zeros = [bucket('2026-09-13 10:00:00'), bucket('2026-09-13 11:00:00')];
    expect(trendPeakOf(zeros), 0);
    expect(hasTrend(zeros), isFalse);
    expect(trendPeakOf(const []), 0);
    expect(hasTrend(const []), isFalse);
    final some = [...zeros, bucket('2026-09-13 12:00:00', requests: 7)];
    expect(trendPeakOf(some), 7);
    expect(hasTrend(some), isTrue);
  });

  test('hourTickOf：hourly 桶取 HH，daily 桶无小时信息返空串', () {
    expect(hourTickOf(bucket('2026-09-13 14:00:00')), '14');
    expect(hourTickOf(bucket('2026-09-13')), '');
  });
}
