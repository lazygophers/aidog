// models.dart 的 fromJson 是 RPC 边界解码，React 版无对应断言（TS 由 ts-rs 类型保证）。
// 这里只留一条「wire snake_case → Dart 字段」的守门测试，不替代翻译自 React 的断言。
import 'package:aidog_flutter/stats/models.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('StatsSeries/StatsBucket 按 wire 的 snake_case 解码', () {
    final s = StatsSeries.fromJson({
      'name': 'glm',
      'buckets': [
        {
          'time_bucket': '2026-09-13 10:00:00',
          'total_requests': 7,
          'success_count': 6,
          'error_count': 1,
          'input_tokens': 100,
          'output_tokens': 200,
          'cache_tokens': 300,
          'avg_duration_ms': 12.5,
          'total_cost': 0.25,
        },
      ],
    });
    expect(s.name, 'glm');
    final b = s.buckets.single;
    expect(b.timeBucket, '2026-09-13 10:00:00');
    expect(b.totalRequests, 7);
    expect(b.successCount, 6);
    expect(b.errorCount, 1);
    expect(b.inputTokens, 100);
    expect(b.outputTokens, 200);
    expect(b.cacheTokens, 300);
    expect(b.avgDurationMs, 12.5);
    expect(b.totalCost, 0.25);
  });

  test('StatsBucket 缺省字段回落 0（后端老版本少字段不炸）', () {
    final b = StatsBucket.fromJson({'time_bucket': '2026-09-13', 'total_requests': 3});
    expect(b.totalRequests, 3);
    expect(b.successCount, 0);
    expect(b.totalCost, 0);
  });

  test('QuotaSnapshot 解码（created_at 是毫秒）', () {
    final q = QuotaSnapshot.fromJson({
      'platform_id': 2,
      'est_balance_remaining': 12.5,
      'created_at': 1757745600000,
    });
    expect(q.platformId, 2);
    expect(q.estBalanceRemaining, 12.5);
    expect(q.createdAt, 1757745600000);
  });

  test('ScatterHistogram 解码（边界数组转 double，counts 转 int）', () {
    final h = ScatterHistogram.fromJson({
      'duration_bins': [0, 1000],
      'cost_bins': [0, 0.02],
      'counts': [
        [4],
      ],
    });
    expect(h.durationBins, [0.0, 1000.0]);
    expect(h.costBins, [0.0, 0.02]);
    expect(h.counts, [
      [4],
    ]);
  });
}
