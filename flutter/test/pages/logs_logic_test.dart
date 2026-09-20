/// Logs / RequestLog 两页逻辑层的回归测试（票 I07）。
///
/// 前半部分是 `src/pages/Logs/useLogsFilters.test.ts` 的逐条翻译 ——
/// **数据与期望值一字不改**，这是最硬的对齐证据。每个 `test` 的标题就是 React 那边的标题。
///
/// 后半部分是本票新增的：SSE 增量提示的形状（每次挂载先整查、流上来只静默重查一次、
/// 在途不发第二轮），以及 RequestLog 的 count 过滤器补默认值。
library;

import 'package:aidog_flutter/src/pages/logs_logic.dart';
import 'package:aidog_flutter/src/pages/models.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';

/// `useLogsFilters.test.ts:25-27` 的 beforeEach 桩，值照抄。
FakeInvoke logsFake() => FakeInvoke({
  'platform_list': [
    {'id': 1, 'name': 'P1'},
  ],
  'group_detail_list': [
    {
      'group': {'group_key': 'g1', 'name': 'G1'},
    },
  ],
  'proxy_log_list_filtered': {'items': <Object?>[], 'has_more': false},
  'proxy_log_distinct_models': <Object?>[],
  'proxy_log_get': null,
  'proxy_log_clear': null,
  'proxy_log_cleanup_expired': null,
});

void main() {
  // 时间冻结：`timePresetToRange` 拿 now 算区间，不冻结就没法断言具体值。
  const nowMs = 1700000000000;

  group('useLogsFilters（React: src/pages/Logs/useLogsFilters.test.ts 逐条翻译）', () {
    test('初始 activeFilter 只带 exclude_sources 默认值', () {
      const f = LogsFilterState();
      expect(f.activeFilter(nowMs), {
        'exclude_sources': ['test', 'quota'],
      });
      expect(f.hasFilter, false);
    });

    test('filterGroup=NO_GROUP_SENTINEL 时 activeFilter.group_key 映射为空串', () {
      const f = LogsFilterState(group: kNoGroupSentinel);
      expect(f.activeFilter(nowMs)['group_key'], '');
      expect(f.hasFilter, true);
    });

    test('filterModelText 非空才在 activeFilter 上带 model/model_type', () {
      const empty = LogsFilterState();
      expect(empty.activeFilter(nowMs)['model'], isNull);
      const filled = LogsFilterState(modelText: 'claude-3');
      expect(filled.activeFilter(nowMs)['model'], 'claude-3');
      expect(filled.activeFilter(nowMs)['model_type'], 'actual');
    });

    test('filterStatus success/error 映射为 status=200/-1', () {
      expect(
        const LogsFilterState(status: 'success').activeFilter(nowMs)['status'],
        200,
      );
      expect(
        const LogsFilterState(status: 'error').activeFilter(nowMs)['status'],
        -1,
      );
    });

    test('filterObserved=observed 时 activeFilter.observed=true（票 04 观察模式命中筛选）', () {
      expect(const LogsFilterState().activeFilter(nowMs)['observed'], isNull);
      const on = LogsFilterState(observed: 'observed');
      expect(on.activeFilter(nowMs)['observed'], true);
      expect(on.hasFilter, true);
      expect(const LogsFilterState(observed: '').activeFilter(nowMs)['observed'], isNull);
    });

    test('clearFilter 把全部筛选字段复位为初始值', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.setFilters(
        const LogsFilterState(
          platform: '1',
          group: 'g1',
          status: 'error',
          modelText: 'x',
          path: '/v1',
          observed: 'observed',
        ),
      );
      expect(c.filters.hasFilter, true);
      await c.clearFilter();
      expect(c.filters.hasFilter, false);
      expect(c.filters.platform, '');
      expect(c.filters.group, '');
      expect(c.filters.status, '');
      expect(c.filters.modelText, '');
      expect(c.filters.modelType, 'actual');
      expect(c.filters.path, '');
      expect(c.filters.observed, '');
    });

    test('platformMap/groupName 由异步加载的 platforms/groups 派生', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      expect(c.platformName(1), 'P1');
      expect(c.groupName('g1'), 'G1');
      expect(c.groupName('unknown'), 'unknown');
    });
  });

  group('timePresetToRange（React: src/pages/Logs/types.ts:8）', () {
    test('all → 两端都不带', () {
      final r = timePresetToRange('all', nowMs);
      expect(r.start, isNull);
      expect(r.end, isNull);
    });

    test('各档的毫秒跨度与 types.ts:11 的表逐条相等', () {
      // 表里的五个值一字不改。
      expect(timePresetToRange('1h', nowMs).start, nowMs - 3600000);
      expect(timePresetToRange('6h', nowMs).start, nowMs - 21600000);
      expect(timePresetToRange('24h', nowMs).start, nowMs - 86400000);
      expect(timePresetToRange('7d', nowMs).start, nowMs - 604800000);
      expect(timePresetToRange('30d', nowMs).start, nowMs - 2592000000);
      expect(timePresetToRange('24h', nowMs).end, nowMs);
    });

    test('未知档位 → 跨度按 0 算（React 的 `?? 0`）', () {
      expect(timePresetToRange('999y', nowMs).start, nowMs);
    });
  });

  group('SSE 是增量提示，不是数据源（本票新增）', () {
    test('每次挂载先整查一遍 —— 连接之前发生的事件 SSE 一条都不补', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      expect(
        k.callsTo('proxy_log_list_filtered').length,
        1,
        reason: 'init 必须自己查一次列表，不能等事件来了才有数据',
      );
    });

    test('流上来一下 = 一次静默重查（不是整页 loading）', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      final before = k.callsTo('proxy_log_list_filtered').length;
      await c.refreshFromEvent();
      expect(k.callsTo('proxy_log_list_filtered').length, before + 1);
      expect(c.loading, false, reason: '静默刷新不许把 loading 打开（会闪整页骨架）');
    });

    test('在途一轮没回来时不发第二轮 —— 票 11 那个「一秒四次整页重查」的形状不许复现', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      final before = k.callsTo('proxy_log_list_filtered').length;
      // 不 await，模拟事件密集期同一帧内连发四下。
      final futures = [
        c.refreshFromEvent(),
        c.refreshFromEvent(),
        c.refreshFromEvent(),
        c.refreshFromEvent(),
      ];
      await Future.wait(futures);
      expect(
        k.callsTo('proxy_log_list_filtered').length,
        before + 1,
        reason: '四次事件叠在一起时只该落地一次查询',
      );
    });

    test('详情开着时事件也刷详情；没开就不发 proxy_log_get', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      await c.refreshFromEvent();
      expect(k.callsTo('proxy_log_get'), isEmpty);

      k.responses['proxy_log_get'] = {'id': 'abc', 'status_code': 200};
      await c.openDetail('abc');
      expect(c.detail?.id, 'abc');
      final before = k.callsTo('proxy_log_get').length;
      await c.refreshFromEvent();
      expect(k.callsTo('proxy_log_get').length, before + 1);
    });
  });

  group('清空日志是破坏性的，必须先确认（React: useLogsList.ts:40）', () {
    test('没确认之前一个命令都不发', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      c.showClearConfirm = true;
      expect(k.commands.contains('proxy_log_clear'), false);
    });

    test('确认之后才 proxy_log_clear，并回到第一页', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      await c.goToPage(3);
      expect(c.offset, 40);
      await c.confirmClear();
      expect(k.callsTo('proxy_log_clear').length, 1);
      expect(c.offset, 0);
      expect(c.showClearConfirm, false);
    });

    test('清理过期不需要二次确认（只删过了保留期的行）', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      await c.cleanupExpired();
      expect(k.callsTo('proxy_log_cleanup_expired').length, 1);
      expect(c.cleanupMessage, '已清理过期日志');
    });
  });

  group('分页', () {
    test('改筛选回第一页（React: useLogsList.ts:35）', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      await c.goToPage(4);
      expect(c.offset, 60);
      await c.setFilters(const LogsFilterState(status: 'error'));
      expect(c.offset, 0);
      expect(c.currentPage, 1);
    });

    test('改每页条数也回第一页', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      await c.goToPage(3);
      await c.setPageSize(50);
      expect(c.offset, 0);
      expect(c.pageSize, 50);
    });

    test('切 modelType 会重拉模型下拉（React: useLogsFilters.ts:70 的依赖数组）', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      final before = k.callsTo('proxy_log_distinct_models').length;
      await c.setFilters(const LogsFilterState(modelType: 'original'));
      expect(k.callsTo('proxy_log_distinct_models').length, before + 1);
      expect(k.lastCallTo('proxy_log_distinct_models')!.args!['actual'], false);
    });

    test('只改别的筛选不重拉模型下拉', () async {
      final k = logsFake();
      final c = LogsController(invoke: k.fn, now: () => nowMs);
      await c.init();
      final before = k.callsTo('proxy_log_distinct_models').length;
      await c.setFilters(const LogsFilterState(status: 'error'));
      expect(k.callsTo('proxy_log_distinct_models').length, before);
    });
  });

  group('RequestLog（React: src/pages/RequestLog.tsx）', () {
    FakeInvoke reqFake() => FakeInvoke({
      'platform_list': [
        {'id': 1, 'name': 'P1'},
      ],
      'request_log_list': <Object?>[],
      'proxy_log_count_filtered': 0,
      'proxy_log_get': null,
    });

    test('typeToSources：all → null（后端默认 [test,quota]），其余 → 单元素数组', () {
      expect(typeToSources('all'), isNull);
      expect(typeToSources('test'), ['test']);
      expect(typeToSources('quota'), ['quota']);
    });

    test('初始 activeFilter 是空对象（与 Logs 主页不同，这边不带 exclude_sources）', () {
      final c = RequestLogController(invoke: reqFake().fn, now: () => nowMs);
      expect(c.activeFilter, <String, Object?>{});
      expect(c.hasFilter, false);
    });

    test('count 用的过滤器要补上 sources 默认值（RequestLog.tsx:119-120）', () async {
      final k = reqFake();
      final c = RequestLogController(invoke: k.fn, now: () => nowMs);
      await c.init();
      final countArgs =
          k.lastCallTo('proxy_log_count_filtered')!.args!['filter']!
              as Map<String, Object?>;
      expect(
        countArgs['sources'],
        ['test', 'quota'],
        reason: '不补的话 count 会把全部来源算进去，页数偏大',
      );
      // 列表那一侧不补，保持 undefined（后端自己有默认）。
      final listArgs =
          k.lastCallTo('request_log_list')!.args!['filter']! as Map<String, Object?>;
      expect(listArgs.containsKey('sources'), false);
    });

    test('选了具体类型时两侧 sources 一致', () async {
      final k = reqFake();
      final c = RequestLogController(invoke: k.fn, now: () => nowMs);
      await c.init();
      await c.setFilterType('quota');
      final countArgs =
          k.lastCallTo('proxy_log_count_filtered')!.args!['filter']!
              as Map<String, Object?>;
      final listArgs =
          k.lastCallTo('request_log_list')!.args!['filter']! as Map<String, Object?>;
      expect(countArgs['sources'], ['quota']);
      expect(listArgs['sources'], ['quota']);
    });

    test('status success/error → 200/-1（与 Logs 主页同一映射）', () async {
      final c = RequestLogController(invoke: reqFake().fn, now: () => nowMs);
      await c.setFilterStatus('success');
      expect(c.activeFilter['status'], 200);
      await c.setFilterStatus('error');
      expect(c.activeFilter['status'], -1);
    });

    test('totalPages 至少是 1（RequestLog.tsx:181 的 Math.max(1, ...)）', () async {
      final c = RequestLogController(invoke: reqFake().fn, now: () => nowMs);
      expect(c.total, 0);
      expect(c.totalPages, 1);
      c.total = 21;
      expect(c.totalPages, 2);
      c.total = 40;
      expect(c.totalPages, 2);
      c.total = 41;
      expect(c.totalPages, 3);
    });

    test('groupName 直返 group_key，空则 `-`（本页无分组维度）', () {
      final c = RequestLogController(invoke: reqFake().fn, now: () => nowMs);
      expect(c.groupName('gk_abc'), 'gk_abc');
      expect(c.groupName(''), '-');
    });

    test('clearFilter 复位四个字段并回第一页', () async {
      final k = reqFake();
      final c = RequestLogController(invoke: k.fn, now: () => nowMs);
      await c.init();
      await c.setFilterType('test');
      await c.setFilterPlatform('1');
      await c.setFilterStatus('error');
      await c.setFilterTime('24h');
      expect(c.hasFilter, true);
      await c.goToPage(2);
      await c.clearFilter();
      expect(c.hasFilter, false);
      expect(c.filterType, 'all');
      expect(c.filterPlatform, '');
      expect(c.filterStatus, '');
      expect(c.filterTime, 'all');
      expect(c.offset, 0);
    });

    test('挂载先整查，且在途不重入', () async {
      final k = reqFake();
      final c = RequestLogController(invoke: k.fn, now: () => nowMs);
      await c.init();
      expect(k.callsTo('request_log_list').length, 1);
      await Future.wait([
        c.refreshFromEvent(),
        c.refreshFromEvent(),
        c.refreshFromEvent(),
      ]);
      expect(k.callsTo('request_log_list').length, 2);
    });
  });

  group('复制成 markdown', () {
    ProxyLogDetail detail(Map<String, dynamic> over) => ProxyLogDetail.fromJson({
      'id': 'abc',
      'group_key': 'gk',
      'model': 'm',
      'actual_model': 'am',
      'source_protocol': 'anthropic',
      'target_protocol': 'openai',
      'status_code': 200,
      'duration_ms': 12,
      'input_tokens': 1,
      'output_tokens': 2,
      'cache_tokens': 3,
      'created_at': 0,
      'request_headers': '{"a":1}',
      'request_body': 'not json',
      'response_body': '',
      ...over,
    });

    test('合法 JSON 缩进重排，非 JSON 原样', () {
      expect(prettyJsonOrRaw('{"a":1}'), '{\n  "a": 1\n}');
      expect(prettyJsonOrRaw('not json'), 'not json');
    });

    test('用户侧把已废的 [stream] 哨兵当成「没抓到」（老库里还有这种行）', () {
      final md = buildProxyLogMarkdown(
        detail({'user_response_body': '[stream]', 'response_body': '[stream]'}),
      );
      // 用户侧那一段两个候选都是哨兵 → 落到兜底文案（React useLogsDetail.ts:51-55）。
      expect(md, contains('(streaming, not captured)'));
    });

    test('上游侧只判空、不判哨兵 —— React 就是这样，照搬不「修正」', () {
      final md = buildProxyLogMarkdown(detail({'response_body': '[stream]'}));
      // useLogsDetail.ts:70 是 `d.response_body ? fj(...) : "(streaming...)"`，
      // 非空的 `[stream]` 会被原样打进去。行为差异不是 bug 修复的入口，
      // 要改先改 React 那边，两侧一起动。
      expect(md, contains('## Upstream Request (Proxy → Platform)'));
      expect(md.split('## Upstream Request (Proxy → Platform)').last,
          contains('[stream]'));
    });

    test('user_response_body 优先于 response_body', () {
      final md = buildProxyLogMarkdown(
        detail({'user_response_body': 'USER', 'response_body': 'UP'}),
      );
      expect(md, contains('USER'));
    });

    test('上游 body 为空 → (not captured)，不是 (streaming, not captured)', () {
      final md = buildProxyLogMarkdown(detail({'upstream_request_body': ''}));
      expect(md, contains('(not captured)'));
    });

    test('RequestLog 那份段落更少：没有 Response Headers、没有 token 三行', () {
      final md = buildRequestLogMarkdown(detail({}));
      expect(md, startsWith('# Request Log abc'));
      expect(md, isNot(contains('### Response Headers')));
      expect(md, isNot(contains('- Input Tokens:')));
      expect(md, contains('- Time: '));
    });

    test('Logs 那份带 token 三行与 Response Headers', () {
      final md = buildProxyLogMarkdown(detail({}));
      expect(md, startsWith('# Proxy Log abc'));
      expect(md, contains('- Input Tokens: 1'));
      expect(md, contains('- Output Tokens: 2'));
      expect(md, contains('- Cache Tokens: 3'));
      expect(md, contains('### Response Headers'));
    });

    test('空字段一律显示 `-`（与 React 的 `|| "-"` 一致）', () {
      final md = buildProxyLogMarkdown(
        detail({'model': '', 'actual_model': '', 'request_url': ''}),
      );
      expect(md, contains('- Model: -'));
      expect(md, contains('- Actual Model: -'));
      expect(md, contains('- URL: -'));
    });
  });
}
