// 逐条翻自 React 版 src/components/charts/__tests__/ticks.test.ts（数据与期望值一字不改）。
import 'package:aidog/charts/ticks.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('niceTicks', () {
    test('0..100 → nice 1/2/5 步长刻度，含两端', () {
      expect(niceTicks(0, 100, 5), [0, 50, 100]);
    });

    test('3..97 with 6 ticks → step 20 covers to 100', () {
      expect(niceTicks(3, 97, 6), [0, 20, 40, 60, 80, 100]);
    });

    test('negative domain keeps sign and includes both ends', () {
      expect(niceTicks(-5, 5, 5), [-5, 0, 5]);
    });

    test('fractional domain snaps to clean decimal steps without fp noise', () {
      final ticks = niceTicks(0.1, 0.4, 5);
      // rawStep=0.075 → step 0.1 → [0.1, 0.2, 0.3, 0.4]（floor(0.1/0.1)=1 起）
      expect(ticks, [0.1, 0.2, 0.3, 0.4]);
      for (final t in ticks) {
        expect((t * 1e10) == (t * 1e10).roundToDouble(), true);
      }
    });

    test('raw step exactly one magnitude → step 1 (norm<=1 branch)', () {
      expect(niceTicks(0, 4, 5), [0, 1, 2, 3, 4]);
    });

    test('步长整跨数据域 → 末刻度补一档覆盖 max（防按刻度定轴域后顶值被裁）', () {
      // 0..7 with 4 ticks → step 5：循环止于 5，再补 10
      expect(niceTicks(0, 7, 4), [0, 5, 10]);
      expect(niceTicks(0, 7, 4).every((t) => t >= 0), true);
    });

    test('min === max → single tick', () {
      expect(niceTicks(7, 7, 5), [7]);
    });

    test('max < min → single tick (degenerate domain)', () {
      expect(niceTicks(10, 2, 5), [10]);
    });

    test('non-finite input → empty (axis falls back)', () {
      expect(niceTicks(double.nan, 5, 5), <double>[]);
      expect(niceTicks(0, double.infinity, 5), <double>[]);
    });
  });

  group('formatTimeTick', () {
    // 本地时区构造，避免 UTC 偏移影响断言
    final t = DateTime(2026, 6, 15, 9, 5, 0).millisecondsSinceEpoch;
    const hour = 3600000;

    test('span ≤ 48h → HH:MM', () {
      expect(formatTimeTick(t, 24 * hour), '09:05');
      expect(formatTimeTick(t, 48 * hour), '09:05');
    });

    test('span ≤ 60d → MM-DD', () {
      expect(formatTimeTick(t, 7 * 24 * hour), '06-15');
      expect(formatTimeTick(t, 60 * 24 * hour), '06-15');
    });

    test('longer span → YYYY-MM', () {
      expect(formatTimeTick(t, 200 * 24 * hour), '2026-06');
    });

    test('invalid timestamp → empty string', () {
      expect(formatTimeTick(double.nan, 24 * hour), '');
    });
  });

  group('bucketMs', () {
    test('minute/hourly bucket with time part → local parse via T', () {
      // 本地时区构造：目标串的本地 ms = 同一本地墙钟的 ms
      final local = DateTime(2026, 9, 13, 9, 30, 0).millisecondsSinceEpoch;
      expect(bucketMs('2026-09-13 09:30'), local);
      expect(
        bucketMs('2026-09-13 09:00:00'),
        DateTime(2026, 9, 13, 9, 0, 0).millisecondsSinceEpoch,
      );
    });

    test('daily bucket (date only) → local midnight', () {
      expect(
        bucketMs('2026-09-13'),
        DateTime(2026, 9, 13, 0, 0, 0).millisecondsSinceEpoch,
      );
    });
  });

  group('xNum', () {
    test('number passes through unchanged', () {
      expect(xNum(123.5), 123.5);
    });

    test('Date → its timestamp', () {
      final d = DateTime(2026, 9, 13, 10, 0, 0);
      expect(xNum(d), d.millisecondsSinceEpoch);
    });

    test('string with space → normalized local parse', () {
      expect(
        xNum('2026-09-13 09:30'),
        DateTime(2026, 9, 13, 9, 30, 0).millisecondsSinceEpoch,
      );
    });

    test('date-only string → UTC midnight (xNum 不做本地归一，与 bucketMs 不同)', () {
      expect(xNum('2026-09-13'), DateTime.utc(2026, 9, 13).millisecondsSinceEpoch);
    });

    test('invalid string → NaN (axis falls back)', () {
      expect(xNum('not-a-date'), isNaN);
    });
  });
}
