/// `platform.extra` 读写层 + 时段内核 + 三个纯函数的回归测试。
///
/// 断言值一律照抄 React 侧的实现语义（`src/services/api/platforms.ts` /
/// `src/utils/timeWindow.ts` / `src/domains/platforms/autoCategorize.ts` /
/// `src/pages/platforms/platformPasteApply.ts`），**期望值一字不改**。
library;

import 'dart:convert';

import 'package:aidog_flutter/src/pages/platform_defaults.dart';
import 'package:aidog_flutter/src/pages/platform_extra.dart';
import 'package:aidog_flutter/src/pages/time_window.dart';
import 'package:flutter_test/flutter_test.dart';

Map<String, Object?> obj(String raw) =>
    Map<String, Object?>.from(jsonDecode(raw) as Map);

void main() {
  group('mock 配置', () {
    test('空串 / 非法 JSON / 无 mock 键 → 默认值', () {
      for (final raw in ['', '  ', 'not json', '{}', '[]', '{"mock":3}']) {
        final c = parseMockConfig(raw);
        expect(c.statusCode, 200);
        expect(c.responseText, 'Hello from mock');
        expect(c.finishReason, 'end_turn');
        expect(c.inputTokens, 100);
        expect(c.outputTokens, 50);
        expect(c.chunkCount, 5);
        expect(c.errorMode, 'none');
        expect(c.streamOverride, isNull);
      }
    });

    test('部分字段覆盖默认，其余保留默认', () {
      final c = parseMockConfig('{"mock":{"status_code":429,"error_rate":0.5}}');
      expect(c.statusCode, 429);
      expect(c.errorRate, 0.5);
      expect(c.delayMs, 0);
      expect(c.responseText, 'Hello from mock');
    });

    test('序列化保留 extra 其余键，三个可选字段为 null 时不写键', () {
      final out = serializeMockConfig('{"keep":1}', kDefaultMockConfig);
      final o = obj(out);
      expect(o['keep'], 1);
      final m = o['mock']! as Map;
      expect(m.containsKey('ttft_ms'), isFalse);
      expect(m.containsKey('inter_chunk_ms'), isFalse);
      expect(m.containsKey('error_rate'), isFalse);
      // stream_override 是三态，null 也要写（null ≠ 缺省）。
      expect(m.containsKey('stream_override'), isTrue);
      expect(m['stream_override'], isNull);
    });

    test('round-trip：写回再读出等于原值', () {
      const c = MockConfig(
        statusCode: 500,
        delayMs: 12,
        ttftMs: 3,
        interChunkMs: 4,
        streamOverride: true,
        responseText: 'x',
        finishReason: 'stop',
        inputTokens: 1,
        outputTokens: 2,
        cacheTokens: 3,
        errorMode: 'timeout',
        errorRate: 0.25,
        chunkCount: 9,
      );
      final back = parseMockConfig(serializeMockConfig('', c));
      expect(back.statusCode, 500);
      expect(back.ttftMs, 3);
      expect(back.interChunkMs, 4);
      expect(back.streamOverride, true);
      expect(back.errorMode, 'timeout');
      expect(back.errorRate, 0.25);
    });
  });

  group('devin 配置', () {
    test('数字 timeout 读成字符串；缺 devin 键 → 默认空', () {
      expect(parseDevinConfig('{"devin":{"devin_timeout":300}}').devinTimeout,
          '300');
      expect(parseDevinConfig('{"devin":{"devin_timeout":"7"}}').devinTimeout, '7');
      expect(parseDevinConfig('').devinTimeout, '');
      expect(parseDevinConfig('{"x":1}').devinMode, '');
    });

    test('三样全空 → 删整个 devin 键', () {
      final o = obj(serializeDevinConfig('{"devin":{},"keep":1}', kDefaultDevinConfig));
      expect(o.containsKey('devin'), isFalse);
      expect(o['keep'], 1);
    });

    test('存量 org_id 原样保留（本表单不编辑它）', () {
      final o = obj(
        serializeDevinConfig(
          '{"devin":{"org_id":"org-1"}}',
          const DevinConfig(devinTimeout: '0', devinMode: ''),
        ),
      );
      expect((o['devin']! as Map)['org_id'], 'org-1');
    });

    test('timeout 负数 / 非数字钳到 0 → 不写该键', () {
      final o = obj(
        serializeDevinConfig(
          '',
          const DevinConfig(devinTimeout: '-5', devinMode: 'fast'),
        ),
      );
      final d = o['devin']! as Map;
      expect(d.containsKey('devin_timeout'), isFalse);
      expect(d['devin_mode'], 'fast');
    });
  });

  group('配额脚本', () {
    test('parseQuotaScriptConfig 读 id 与自定义正文', () {
      final r = parseQuotaScriptConfig(
        '{"quota_script_id":"v1","quota_custom_script":"// x"}',
      );
      expect(r.variantId, 'v1');
      expect(r.customScript, '// x');
      expect(hasCustomQuotaScript('{"quota_custom_script":"// x"}'), isTrue);
      expect(hasCustomQuotaScript('{"quota_custom_script":"   "}'), isFalse);
      expect(hasCustomQuotaScript(''), isFalse);
    });

    test('readRequiresValue：嵌套优先（newapi → devin）→ 顶层兜底', () {
      const raw = '{"org_id":"top","devin":{"org_id":"nested"}}';
      expect(readRequiresValue(raw, 'org_id'), 'nested');
      expect(readRequiresValue('{"org_id":"top"}', 'org_id'), 'top');
      // 嵌套是空串 → 不算命中，回落顶层。
      expect(
        readRequiresValue('{"org_id":"top","devin":{"org_id":""}}', 'org_id'),
        'top',
      );
      expect(readRequiresValue('', 'org_id'), '');
      expect(readRequiresValue('bad json', 'org_id'), '');
    });

    test('custom 非空 → 写 custom 删 id（互斥）', () {
      final o = obj(
        serializeQuotaScriptConfig(
          '{"quota_script_id":"v1"}',
          variantId: 'v1',
          customScript: '// s',
          requires: const {},
          variants: const [(id: 'v1', requires: <String>[])],
          protocol: 'openai',
        ),
      );
      expect(o['quota_custom_script'], '// s');
      expect(o.containsKey('quota_script_id'), isFalse);
    });

    test('id 失效 → 不写 quota_script_id（后端回落首条）', () {
      final o = obj(
        serializeQuotaScriptConfig(
          '',
          variantId: 'gone',
          customScript: '',
          requires: const {},
          variants: const [(id: 'v1', requires: <String>[])],
          protocol: 'openai',
        ),
      );
      expect(o.containsKey('quota_script_id'), isFalse);
    });

    test('requires 值写顶层 + 镜像写旧嵌套家；置空则两处同删', () {
      var out = serializeQuotaScriptConfig(
        '',
        variantId: 'v1',
        customScript: '',
        requires: const {'org_id': 'org-9'},
        variants: const [(id: 'v1', requires: ['org_id'])],
        protocol: 'devin',
      );
      var o = obj(out);
      expect(o['org_id'], 'org-9');
      expect((o['devin']! as Map)['org_id'], 'org-9');

      out = serializeQuotaScriptConfig(
        out,
        variantId: 'v1',
        customScript: '',
        requires: const {'org_id': '  '},
        variants: const [(id: 'v1', requires: ['org_id'])],
        protocol: 'devin',
      );
      o = obj(out);
      expect(o.containsKey('org_id'), isFalse);
      expect((o['devin']! as Map).containsKey('org_id'), isFalse);
    });

    test('newapi 的 user_id 单独写 extra.newapi.user_id，空则删', () {
      var out = serializeQuotaScriptConfig(
        '',
        variantId: '',
        customScript: '',
        requires: const {'user_id': '42'},
        variants: const [],
        protocol: 'newapi',
      );
      expect((obj(out)['newapi']! as Map)['user_id'], '42');
      out = serializeQuotaScriptConfig(
        out,
        variantId: '',
        customScript: '',
        requires: const {'user_id': ''},
        variants: const [],
        protocol: 'newapi',
      );
      expect((obj(out)['newapi']! as Map).containsKey('user_id'), isFalse);
    });
  });

  group('熔断覆盖', () {
    test('缺键 / 非法 → 全 0', () {
      for (final raw in ['', '{}', 'bad', '{"breaker":5}']) {
        final b = parsePlatformBreaker(raw);
        expect(b.failureThreshold, 0);
        expect(b.openSecs, 0);
        expect(b.halfOpenMax, 0);
      }
    });

    test('三值全 0 → 删 breaker 键；任一非 0 → 写整个对象', () {
      var o = obj(
        serializePlatformBreaker(
          '{"breaker":{"open_secs":9},"keep":1}',
          const PlatformBreaker(
            failureThreshold: 0,
            openSecs: 0,
            halfOpenMax: 0,
          ),
        ),
      );
      expect(o.containsKey('breaker'), isFalse);
      expect(o['keep'], 1);

      o = obj(
        serializePlatformBreaker(
          '',
          const PlatformBreaker(
            failureThreshold: 3,
            openSecs: 0,
            halfOpenMax: 0,
          ),
        ),
      );
      expect((o['breaker']! as Map)['failure_threshold'], 3);
      expect((o['breaker']! as Map)['open_secs'], 0);
    });
  });

  group('peak / disable_during_peak / time_windows', () {
    test('parsePlatformPeak 缺键 / 非数组 → 空', () {
      expect(parsePlatformPeak(''), isEmpty);
      expect(parsePlatformPeak('{"peak":3}'), isEmpty);
      expect(parsePlatformPeak('bad'), isEmpty);
    });

    test('小数 start_hour 拆成 hour + minute（存量半时区数据）', () {
      final ws = parsePlatformPeak(
        '{"peak":[{"start_hour":8.5,"end_hour":17,"multiplier":2}]}',
      );
      expect(ws.single.startHour, 8);
      expect(ws.single.startMinute, 30);
      expect(ws.single.endHour, 17);
    });

    test('空数组 → 删 peak 键；非空 → 只写定义过的字段', () {
      expect(obj(serializePlatformPeak('{"peak":[]}', const [])).containsKey('peak'),
          isFalse);
      final o = obj(
        serializePlatformPeak('', const [
          TimeWindow(startHour: 9, endHour: 12, multiplier: 2.0),
        ]),
      );
      final w = (o['peak']! as List).single as Map;
      expect(w['start_hour'], 9);
      expect(w['multiplier'], 2.0);
      expect(w.containsKey('days_of_week'), isFalse);
      expect(w.containsKey('timezone'), isFalse);
      expect(w.containsKey('models'), isFalse);
    });

    test('disable_during_peak 严格布尔；false → 删键', () {
      expect(parseDisableDuringPeak('{"disable_during_peak":true}'), isTrue);
      expect(parseDisableDuringPeak('{"disable_during_peak":1}'), isFalse);
      expect(parseDisableDuringPeak('{"disable_during_peak":"true"}'), isFalse);
      expect(parseDisableDuringPeak(''), isFalse);
      expect(
        obj(serializeDisableDuringPeak('{"disable_during_peak":true}', false))
            .containsKey('disable_during_peak'),
        isFalse,
      );
      expect(obj(serializeDisableDuringPeak('', true))['disable_during_peak'],
          true);
    });

    test('time_windows round-trip；空数组删键', () {
      const rules = [
        TimeModelRule(
          windows: [TimeWindow(startHour: 1, endHour: 2, multiplier: 1)],
          models: {'default': 'm1'},
        ),
      ];
      final out = serializePlatformTimeWindows('{"keep":1}', rules);
      expect(obj(out)['keep'], 1);
      final back = parsePlatformTimeWindows(out);
      expect(back.single.models['default'], 'm1');
      expect(back.single.windows.single.startHour, 1);
      expect(
        obj(serializePlatformTimeWindows(out, const [])).containsKey('time_windows'),
        isFalse,
      );
    });

    test('未知键在 round-trip 后原样带回（不吃掉别人的数据）', () {
      final ws = parsePlatformPeak(
        '{"peak":[{"start_hour":0,"end_hour":24,"multiplier":1,"future_flag":7}]}',
      );
      final o = obj(serializePlatformPeak('', ws));
      expect(((o['peak']! as List).single as Map)['future_flag'], 7);
    });
  });

  group('时段命中（与 Rust peak::is_in_peak_window 对称）', () {
    // 2026-01-05 是周一。UTC 10:00。
    final mondayUtc10 = DateTime.utc(2026, 1, 5, 10).millisecondsSinceEpoch;

    test('空 / null → false', () {
      expect(isCurrentlyPeak(null, mondayUtc10), isFalse);
      expect(isCurrentlyPeak(const [], mondayUtc10), isFalse);
    });

    test('同天半开区间 [start, end)', () {
      const w = TimeWindow(startHour: 9, endHour: 12, multiplier: 2);
      expect(isCurrentlyPeak(const [w], mondayUtc10), isTrue);
      expect(
        isCurrentlyPeak(const [w],
            DateTime.utc(2026, 1, 5, 12).millisecondsSinceEpoch),
        isFalse,
      );
      expect(
        isCurrentlyPeak(const [w],
            DateTime.utc(2026, 1, 5, 9).millisecondsSinceEpoch),
        isTrue,
      );
    });

    test('跨天 end < start', () {
      const w = TimeWindow(startHour: 22, endHour: 6, multiplier: 2);
      expect(
        isCurrentlyPeak(const [w],
            DateTime.utc(2026, 1, 5, 23).millisecondsSinceEpoch),
        isTrue,
      );
      expect(
        isCurrentlyPeak(const [w],
            DateTime.utc(2026, 1, 5, 3).millisecondsSinceEpoch),
        isTrue,
      );
      expect(isCurrentlyPeak(const [w], mondayUtc10), isFalse);
    });

    test('start == end 退化为全天命中', () {
      const w = TimeWindow(startHour: 5, endHour: 5, multiplier: 2);
      expect(isCurrentlyPeak(const [w], mondayUtc10), isTrue);
    });

    test('days_of_week 过滤（0=Sunday…6=Saturday）', () {
      const mon = TimeWindow(
        startHour: 9,
        endHour: 12,
        multiplier: 2,
        daysOfWeek: [1],
      );
      const sun = TimeWindow(
        startHour: 9,
        endHour: 12,
        multiplier: 2,
        daysOfWeek: [0],
      );
      expect(isCurrentlyPeak(const [mon], mondayUtc10), isTrue);
      expect(isCurrentlyPeak(const [sun], mondayUtc10), isFalse);
    });

    test('days_of_month 过滤', () {
      const w = TimeWindow(
        startHour: 0,
        endHour: 24,
        multiplier: 2,
        daysOfMonth: [5],
      );
      expect(isCurrentlyPeak(const [w], mondayUtc10), isTrue);
      expect(
        isCurrentlyPeak(const [w],
            DateTime.utc(2026, 1, 6, 10).millisecondsSinceEpoch),
        isFalse,
      );
    });

    test('start_at / end_at 生效期，优先级最高', () {
      final sec = mondayUtc10 ~/ 1000;
      expect(
        isCurrentlyPeak(
          [TimeWindow(startHour: 0, endHour: 24, multiplier: 2, startAt: sec + 1)],
          mondayUtc10,
        ),
        isFalse,
      );
      expect(
        isCurrentlyPeak(
          [TimeWindow(startHour: 0, endHour: 24, multiplier: 2, endAt: sec)],
          mondayUtc10,
        ),
        isFalse,
      );
      expect(
        isCurrentlyPeak(
          [TimeWindow(startHour: 0, endHour: 24, multiplier: 2, startAt: sec)],
          mondayUtc10,
        ),
        isTrue,
      );
    });

    test('model scope：空 requestModel 跳过过滤；前缀通配 exact-first', () {
      const w = TimeWindow(
        startHour: 0,
        endHour: 24,
        multiplier: 2,
        models: ['glm-5.2*'],
      );
      expect(isCurrentlyPeak(const [w], mondayUtc10), isTrue);
      expect(isCurrentlyPeak(const [w], mondayUtc10, 'glm-5.2'), isTrue);
      expect(isCurrentlyPeak(const [w], mondayUtc10, 'glm-5.2-turbo'), isTrue);
      expect(isCurrentlyPeak(const [w], mondayUtc10, 'gpt-4'), isFalse);
      expect(modelMatch('glm-5.2', 'glm-5.2-turbo'), isFalse);
    });

    test('timezone 按该时区本地时刻解释（北京 = UTC+8）', () {
      const w = TimeWindow(
        startHour: 18,
        endHour: 19,
        multiplier: 3,
        timezone: 'Asia/Shanghai',
      );
      // UTC 10:00 = 北京 18:00 → 命中。
      expect(isCurrentlyPeak(const [w], mondayUtc10), isTrue);
      // UTC 11:00 = 北京 19:00 → 半开区间右端不含。
      expect(
        isCurrentlyPeak(const [w],
            DateTime.utc(2026, 1, 5, 11).millisecondsSinceEpoch),
        isFalse,
      );
    });

    test('非法时区名回落 UTC（数据脏不炸 UI）', () {
      const w = TimeWindow(
        startHour: 10,
        endHour: 11,
        multiplier: 2,
        timezone: 'Not/AZone',
      );
      expect(isCurrentlyPeak(const [w], mondayUtc10), isTrue);
    });

    test('多窗口 first-match：任一命中即 true', () {
      const a = TimeWindow(startHour: 0, endHour: 1, multiplier: 2);
      const b = TimeWindow(startHour: 9, endHour: 12, multiplier: 3);
      expect(isCurrentlyPeak(const [a, b], mondayUtc10), isTrue);
    });
  });

  group('时钟换算', () {
    test('shiftClock 按绝对分钟，半时区精确、跨日折回', () {
      expect(shiftClock(23, 45, 30), (hour: 0, minute: 15));
      expect(shiftClock(0, 15, -30), (hour: 23, minute: 45));
      expect(shiftClock(10, 0, 330), (hour: 15, minute: 30));
      expect(shiftClock(10, 0, 0), (hour: 10, minute: 0));
    });

    test('utc→display→utc 是恒等', () {
      for (final mode in TzMode.values) {
        final d = utcToDisplay(13, 27, mode);
        final back = displayToUtc(d.hour, d.minute, mode);
        expect(back, (hour: 13, minute: 27));
      }
    });

    test('utc 模式偏移恒为 0', () {
      expect(tzOffsetMinutes(TzMode.utc), 0);
      expect(utcToDisplay(7, 8, TzMode.utc), (hour: 7, minute: 8));
    });
  });

  group('autoCategorize（React: autoCategorize.ts）', () {
    test('按模式分配，已占用的 id 不重复分配', () {
      final r = autoCategorize([
        'claude-opus-4',
        'claude-sonnet-4',
        'claude-haiku-4',
        'gpt-5',
        'other-model',
      ]);
      expect(r['opus'], 'claude-opus-4');
      expect(r['sonnet'], 'claude-sonnet-4');
      expect(r['haiku'], 'claude-haiku-4');
      expect(r['gpt'], 'gpt-5');
      expect(r['default'], 'other-model');
    });

    test('gpt 排除 mini', () {
      final r = autoCategorize(['gpt-5-mini', 'gpt-5']);
      expect(r['gpt'], 'gpt-5');
      // 未被占用的第一个进 default。
      expect(r['default'], 'gpt-5-mini');
    });

    test('全被占用时 default 回落首个 id', () {
      final r = autoCategorize(['claude-opus-4']);
      expect(r['opus'], 'claude-opus-4');
      expect(r['default'], 'claude-opus-4');
    });

    test('空列表 → 五槽全空', () {
      final r = autoCategorize(const []);
      expect(r.values.every((v) => v.isEmpty), isTrue);
    });
  });

  group('splitApiKeys（React: platformPaste.ts:239）', () {
    test('空白 / 逗号 / 分号拆分，去重保序', () {
      expect(splitApiKeys('a b,c;d'), ['a', 'b', 'c', 'd']);
      expect(splitApiKeys('a\n\nb\ta'), ['a', 'b']);
      expect(splitApiKeys(''), isEmpty);
      expect(splitApiKeys('   '), isEmpty);
      expect(splitApiKeys('only'), ['only']);
    });
  });

  group('previewBatchNames（React: platformPasteApply.ts:254）', () {
    test('name = {base}-{key尾4位}', () {
      expect(previewBatchNames(['sk-abcd1234'], 'P', {}), ['P-1234']);
    });

    test('短 key 原样当尾巴', () {
      expect(previewBatchNames(['ab'], 'P', {}), ['P-ab']);
    });

    test('撞名追号 -2，且预览内部连发也追号', () {
      expect(
        previewBatchNames(['xx1234', 'yy1234', 'zz1234'], 'P', {'P-1234'}),
        ['P-1234-2', 'P-1234-3', 'P-1234-4'],
      );
    });

    test('baseName 为空回落 Platform；不污染传入的 used 集合', () {
      final used = <String>{'a'};
      expect(previewBatchNames(['k1234'], '', used), ['Platform-1234']);
      expect(used, {'a'});
    });
  });

  group('新建手动预算', () {
    test('默认 total/usd、consumed 0、启用，id 各不相同', () {
      final a = newManualBudget();
      final b = newManualBudget();
      expect(a.kind, 'total');
      expect(a.unit, 'usd');
      expect(a.amount, 0);
      expect(a.windowHours, isNull);
      expect(a.windowUnit, 'hour');
      expect(a.consumed, 0);
      expect(a.enabled, isTrue);
      expect(a.id.length, 32);
      expect(a.id == b.id, isFalse);
    });

    test('copyWith 保住 id / consumed / window_start_at', () {
      final a = ManualBudget.fromJson(const {
        'id': 'i1',
        'kind': 'total',
        'unit': 'usd',
        'amount': 5,
        'window_hours': null,
        'window_unit': 'hour',
        'consumed': 2.5,
        'window_start_at': 99,
        'enabled': true,
      });
      final b = a.copyWith(kind: 'daily');
      expect(b.id, 'i1');
      expect(b.consumed, 2.5);
      expect(b.windowStartAt, 99);
      expect(b.kind, 'daily');
    });
  });

  group('registry 派生层', () {
    const raw = '''
{"protocols":{
  "openai":{
    "name":{"en-US":"OpenAI","zh-Hans":"开放人工智能"},
    "keywords":["oai"],
    "endpoints":{"default":[{"protocol":"openai","base_url":"https://a/v1"}]},
    "models":{"default":{"default":"gpt-5"},"peak":{"default":"gpt-5-mini"}},
    "model_list":{"default":["gpt-5","gpt-5-mini"]},
    "peak":[{"start_hour":9,"end_hour":12,"multiplier":2}],
    "quota_scripts":[{"id":"v1","name":{"en-US":"Official"},"requires":[{"key":"org_id","label":{"en-US":"Org"}}]},{"bad":1}],
    "plan_quotas":[{"id":"pro","name":"Pro","source_url":"u","budgets":[{"kind":"rolling","unit":"count","amount":300,"window_hours":5,"window_unit":"hour"}]}]
  },
  "glm_coding":{"is_coding_plan":true,"name":{"en-US":"GLM"}}
}}''';

    test('默认端点补派生 client_type', () {
      final d = PlatformDefaults.parse(raw, '');
      final ep = d.defaultEndpoints('openai').single;
      expect(ep.protocol, 'openai');
      expect(ep.baseUrl, 'https://a/v1');
      expect(ep.clientType, 'codex_tui');
      expect(ep.codingPlan, isFalse);
      expect(d.defaultEndpoints('nope'), isEmpty);
    });

    test('clientTypeForProtocol 与 Rust derive_client_type 对称', () {
      expect(clientTypeForProtocol('anthropic'), 'claude_code');
      expect(clientTypeForProtocol('openai'), 'codex_tui');
      expect(clientTypeForProtocol('openai_responses'), 'codex_tui');
      expect(clientTypeForProtocol('openai_completions'), 'codex_tui');
      expect(clientTypeForProtocol('gemini'), 'default');
      expect(clientTypeForProtocol('whatever'), 'default');
    });

    test('models 的 peak 分支只有 isPeak 才用', () {
      final d = PlatformDefaults.parse(raw, '');
      expect(d.defaultModels('openai')['default'], 'gpt-5');
      expect(d.defaultModels('openai', isPeak: true)['default'], 'gpt-5-mini');
      // 没有 peak 分支的协议照样回落 default。
      expect(d.defaultModels('glm_coding', isPeak: true), isEmpty);
    });

    test('model_list / peak / 套餐档位 / 脚本变体', () {
      final d = PlatformDefaults.parse(raw, '');
      expect(d.defaultModelList('openai'), ['gpt-5', 'gpt-5-mini']);
      expect(d.defaultPeak('openai').single.startHour, 9);
      expect(d.defaultPeak('glm_coding'), isEmpty);
      // id 不是字符串的条目被丢掉。
      final vs = d.defaultQuotaScripts('openai');
      expect(vs.length, 1);
      expect(vs.single.requires.single.key, 'org_id');
      final tiers = d.defaultPlanQuotas('openai');
      expect(tiers.single.name, 'Pro');
      expect(tiers.single.budgets.single.amount, 300);
      expect(tiers.single.budgets.single.windowUnit, 'hour');
    });

    test('展示名三层回落 + locale 归一', () {
      final d = PlatformDefaults.parse(raw, '');
      expect(d.protocolLabel('openai', 'zh-Hans'), '开放人工智能');
      expect(d.protocolLabel('openai', 'ja-JP'), 'OpenAI');
      expect(d.protocolLabel('unknown', 'en-US'), 'unknown');
      expect(normalizeDefaultsLocale('zh_CN'), 'zh-Hans');
      expect(normalizeDefaultsLocale(null), 'en-US');
      expect(normalizeDefaultsLocale('pt-BR'), 'en-US');
    });

    test('协议选项带 codingPlan 标记与跨语言搜索词', () {
      final d = PlatformDefaults.parse(raw, '');
      final opts = {for (final o in d.protocolOptions('en-US')) o.value: o};
      expect(opts['glm_coding']!.codingPlan, isTrue);
      expect(opts['openai']!.codingPlan, isFalse);
      // 排序后比字符串 —— Dart 的 List 是身份相等，直接比会永远不等。
      final terms = [...opts['openai']!.searchTerms]..sort();
      expect(terms.join(','), 'OpenAI,oai,开放人工智能');
    });

    test('非法 JSON / 空串 → 空文档，各 getter 不崩', () {
      for (final bad in ['', 'nope', '{}', '{"protocols":3}']) {
        final d = PlatformDefaults.parse(bad, bad);
        expect(d.protocols, isEmpty);
        expect(d.clientTypes, isEmpty);
        expect(d.defaultEndpoints('openai'), isEmpty);
        expect(d.defaultModels('openai'), isEmpty);
        expect(d.defaultPeak('openai'), isEmpty);
        expect(d.protocolOptions(), isEmpty);
      }
    });

    test('客户端模拟候选按 locale 取名，默认组 group 为空串', () {
      const ct = '''
{"client_types":[
  {"value":"default","group":"","name":{"en-US":"Default","zh-Hans":"默认"}},
  {"value":"claude_code","group":"Claude Code","name":{"en-US":"Claude Code"}}
]}''';
      final d = PlatformDefaults.parse(raw, ct, 'zh-Hans');
      expect(d.clientTypes.first.value, 'default');
      expect(d.clientTypes.first.group, '');
      expect(d.clientTypes.first.label, '默认');
      expect(d.clientTypes.last.label, 'Claude Code');
    });

    test('端点锁死集合覆盖厂商直连协议', () {
      expect(kEndpointsLockedProtocols.contains('glm_coding'), isTrue);
      expect(kEndpointsLockedProtocols.contains('claude_code'), isTrue);
      expect(kEndpointsLockedProtocols.contains('openai'), isFalse);
      expect(kEndpointsLockedProtocols.length, 31);
    });
  });
}
