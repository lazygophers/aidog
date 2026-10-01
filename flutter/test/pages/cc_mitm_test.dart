// cc-sub-mitm Flutter 侧的纯逻辑回归：
// ① mitm_stats 开关 extra 读写（对齐 platforms.ts::parseMitmStats/serializeMitmStats）
// ② 采样点 → 趋势线数据的最新值口径（CcMitmTrendSection 显示 last）
import 'package:aidog_flutter/src/pages/cc_mitm.dart';
import 'package:aidog_flutter/src/pages/platform_extra.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('mitm_stats extra 读写', () {
    test('缺失 / 空串 / 非法 JSON / 非布尔 → false', () {
      expect(parseMitmStats(''), isFalse);
      expect(parseMitmStats('{}'), isFalse);
      expect(parseMitmStats('not-json'), isFalse);
      expect(parseMitmStats('{"mitm_stats": "yes"}'), isFalse);
    });

    test('true 读回 true；false 写入后删键', () {
      expect(parseMitmStats('{"mitm_stats": true}'), isTrue);
      // false → 删键（默认行为），别的键保留
      final out = serializeMitmStats('{"mitm_stats": true, "peak": []}', false);
      expect(out.contains('mitm_stats'), isFalse);
      expect(out.contains('peak'), isTrue);
      // true 写入保留别人的键
      final on = serializeMitmStats('{"peak": []}', true);
      expect(parseMitmStats(on), isTrue);
      expect(on.contains('peak'), isTrue);
    });
  });

  group('CcMitmInfoData / 采样模型', () {
    test('fromJson：snake_case 字段、缺省兜零', () {
      final s = OauthUsageSample.fromJson({
        'five_hour_pct': 12.5,
        'seven_day_pct': 3,
        'sampled_at': 1700,
      });
      expect(s.fiveHourPct, 12.5);
      expect(s.sevenDayPct, 3.0);
      expect(s.sampledAt, 1700);
      final d = OauthUsageSample.fromJson(const {});
      expect(d.fiveHourPct, 0);
    });

    test('MitmRefreshStats / CcProfile 缺省兜零空串', () {
      final r = MitmRefreshStats.fromJson(const {});
      expect(r.attempts, 0);
      expect(r.failures, 0);
      final p = CcProfile.fromJson({'tier': 'max'});
      expect(p.tier, 'max');
    });

    test('MitmBypassRow：status 400+ 由 UI 上红（数据层只透传）', () {
      final r = MitmBypassRow.fromJson({
        'id': 7,
        'group_name': 'g',
        'host': 'api.anthropic.com',
        'path': '/v1/messages',
        'status_code': 403,
        'req_bytes': 10,
        'resp_bytes': 20,
        'decrypted': false,
        'created_at': 1,
      });
      expect(r.statusCode, 403);
      expect(r.decrypted, isFalse);
    });
  });
}
