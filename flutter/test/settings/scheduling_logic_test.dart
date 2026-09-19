/// `SchedulingController` —— 对照 `src/components/settings/SchedulingSettings.tsx`。
library;

import 'package:aidog_flutter/src/pages/settings/scheduling_logic.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fake_invoke.dart';

void main() {
  group('数字框校验 clampNonNegativeInt', () {
    // 逐条对应 JS 的 `Math.max(0, Math.floor(Number(v) || 0))`。
    test('空串 → 0', () => expect(clampNonNegativeInt(''), 0));
    test('全空白 → 0', () => expect(clampNonNegativeInt('   '), 0));
    test('非数字 → 0', () => expect(clampNonNegativeInt('abc'), 0));
    test('负数钳到 0', () => expect(clampNonNegativeInt('-5'), 0));
    test('小数向下取整', () => expect(clampNonNegativeInt('7.9'), 7));
    test('正常整数原样', () => expect(clampNonNegativeInt('42'), 42));
    test('前后空白允许', () => expect(clampNonNegativeInt(' 12 '), 12));
    test('0 就是 0', () => expect(clampNonNegativeInt('0'), 0));
  });

  group('加载', () {
    test('读到就用，loading 落下', () async {
      final fake = FakeInvoke({
        'scheduling_settings_get': {
          'default_routing_mode': 'failover',
          'breaker_failure_threshold': 9,
          'breaker_open_secs': 120,
          'breaker_half_open_max': 4,
          'enabled': false,
        },
      });
      final c = SchedulingController(invoke: fake.fn);
      await c.load();

      expect(c.loading, isFalse);
      expect(c.settings.defaultRoutingMode, 'failover');
      expect(c.settings.breakerFailureThreshold, 9);
      expect(c.settings.breakerOpenSecs, 120);
      expect(c.settings.breakerHalfOpenMax, 4);
      expect(c.settings.enabled, isFalse);
    });

    test('读失败 → 退回默认值，界面上不报错', () async {
      final fake = FakeInvoke();
      fake.errors['scheduling_settings_get'] = StateError('nope');
      final c = SchedulingController(invoke: fake.fn);
      await c.load();

      expect(c.loading, isFalse);
      expect(c.error, '', reason: 'React 只 console.error，不把它摆到界面上');
      expect(c.settings.defaultRoutingMode, 'health_aware');
      expect(c.settings.breakerFailureThreshold, 5);
      expect(c.settings.breakerOpenSecs, 60);
      expect(c.settings.breakerHalfOpenMax, 2);
      expect(c.settings.enabled, isTrue);
    });
  });

  group('写回', () {
    test('改一次就整份写回（没有保存按钮）', () async {
      final fake = FakeInvoke({'scheduling_settings_get': SchedulingSettings.defaults.toJson()});
      final c = SchedulingController(invoke: fake.fn);
      await c.load();
      await c.setFailureThreshold('8');

      expect(c.settings.breakerFailureThreshold, 8);
      final sent = fake.lastCallTo('scheduling_settings_set')!.args!['settings']! as Map;
      expect(sent.keys.toSet(), {
        'default_routing_mode',
        'breaker_failure_threshold',
        'breaker_open_secs',
        'breaker_half_open_max',
        'enabled',
      });
      expect(sent['breaker_failure_threshold'], 8);
      expect(sent['enabled'], isTrue, reason: '未改的字段一并回传，否则被清掉');
    });

    test('写失败不回滚本地值，只挂一条错误（与 React 一致）', () async {
      final fake = FakeInvoke({'scheduling_settings_get': SchedulingSettings.defaults.toJson()});
      fake.errors['scheduling_settings_set'] = StateError('rejected');
      final c = SchedulingController(invoke: fake.fn);
      await c.load();
      await c.setOpenSecs('300');

      expect(c.settings.breakerOpenSecs, 300);
      expect(c.error, contains('rejected'));
    });

    test('下一次成功写入清掉上一条错误', () async {
      final fake = FakeInvoke({'scheduling_settings_get': SchedulingSettings.defaults.toJson()});
      fake.errors['scheduling_settings_set'] = StateError('rejected');
      final c = SchedulingController(invoke: fake.fn);
      await c.load();
      await c.setOpenSecs('300');
      expect(c.error, isNotEmpty);

      fake.errors.remove('scheduling_settings_set');
      await c.setOpenSecs('120');
      expect(c.error, '');
    });

    test('总开关取反', () async {
      final fake = FakeInvoke({'scheduling_settings_get': SchedulingSettings.defaults.toJson()});
      final c = SchedulingController(invoke: fake.fn);
      await c.load();
      await c.toggleEnabled();
      expect(c.settings.enabled, isFalse);
      await c.toggleEnabled();
      expect(c.settings.enabled, isTrue);
    });

    test('非法数字输入写进去的是 0，不是把字段弄丢', () async {
      final fake = FakeInvoke({'scheduling_settings_get': SchedulingSettings.defaults.toJson()});
      final c = SchedulingController(invoke: fake.fn);
      await c.load();
      await c.setHalfOpenMax('abc');
      expect(c.settings.breakerHalfOpenMax, 0);
      final sent = fake.lastCallTo('scheduling_settings_set')!.args!['settings']! as Map;
      expect(sent['breaker_half_open_max'], 0);
    });

    test('调度策略下拉', () async {
      final fake = FakeInvoke({'scheduling_settings_get': SchedulingSettings.defaults.toJson()});
      final c = SchedulingController(invoke: fake.fn);
      await c.load();
      await c.setRoutingMode('sticky');
      expect(c.settings.defaultRoutingMode, 'sticky');
    });
  });

  test('五个调度策略与 React 的 ROUTING_MODES 同序', () {
    expect(kRoutingModes, [
      'failover',
      'load_balance',
      'health_aware',
      'least_latency',
      'sticky',
    ]);
    // 每一个都要有标签映射，否则下拉里会露出裸 key。
    for (final m in kRoutingModes) {
      expect(kRoutingModeLabels.containsKey(m), isTrue, reason: m);
    }
  });
}
