/// 请求日志两页的逻辑层（票 I07）：
/// - `LogsController` ← `src/pages/Logs.tsx` + `src/pages/Logs/use{Filters,List,Detail}.ts`
/// - `RequestLogController` ← `src/pages/RequestLog.tsx`
///
/// 两页都订阅后端的 `proxy-log-updated`。**流只是「有新数据」的提示，不是数据本身**：
/// SSE 对连接之前发生的事件什么都不补、断线期间的事件也全丢，所以
/// ① 每次 mount 必须先整查一遍（`load()`），② 流上收到的每一下都只触发一次静默重查。
/// React 侧票 11 量到过「转发期一秒四次整页重查」，根因是订阅没防抖 —— 这里沿用
/// I06 的 [debounceStream]（500ms 尾沿，对齐 `onProxyLogUpdated` 的 `debounceMs`），
/// 并且在途一轮未回来时不发第二轮（`_inFlight`）。
library;

import 'dart:convert';

import '../../utils/formatters.dart';
import 'invoke.dart';
import 'models.dart';

/// 时间范围预设，对齐 `src/pages/Logs/types.ts:6`。
const List<String> kTimePresets = ['all', '1h', '6h', '24h', '7d', '30d'];

/// `types.ts:11` 的毫秒表，一字不改。
const Map<String, int> kTimePresetMs = {
  '1h': 3600000,
  '6h': 21600000,
  '24h': 86400000,
  '7d': 604800000,
  '30d': 2592000000,
};

/// `types.ts:19`：分组筛选里「未分组」那一项的哨兵值。
const String kNoGroupSentinel = '__none__';

/// `types.ts:8::timePresetToRange`。`all` → 两端都不带；其余 → `[now-跨度, now]`。
({int? start, int? end}) timePresetToRange(String preset, int nowMs) {
  if (preset == 'all') return (start: null, end: null);
  return (start: nowMs - (kTimePresetMs[preset] ?? 0), end: nowMs);
}

/// `types.ts:15::safeParseJson` 的展示版：能解析就缩进 2 格重排，不能就原样返回。
/// 复制成 markdown 时用（React 里叫 `fj`）。
String prettyJsonOrRaw(String s) {
  try {
    return const JsonEncoder.withIndent('  ').convert(jsonDecode(s));
  } catch (_) {
    return s;
  }
}

// ── Logs 主页 ──────────────────────────────────────────────────────

/// `useLogsFilters.ts` 的筛选态。不可变，改字段走 [copyWith]，派生量是 getter。
class LogsFilterState {
  const LogsFilterState({
    this.platform = '',
    this.group = '',
    this.status = '',
    this.time = 'all',
    this.modelType = 'actual',
    this.modelText = '',
    this.path = '',
    this.observed = '',
  });

  /// 平台 id 的字符串形式，空串 = 不筛。
  final String platform;
  final String group;

  /// `''` / `'success'` / `'error'`。
  final String status;
  final String time;

  /// `'original'` / `'actual'`，缺省 `actual`（`useLogsFilters.ts:27`）。
  final String modelType;
  final String modelText;
  final String path;

  /// `''` = 全部；`'observed'` = 仅中间件观察模式命中的行（票 04）。
  final String observed;

  LogsFilterState copyWith({
    String? platform,
    String? group,
    String? status,
    String? time,
    String? modelType,
    String? modelText,
    String? path,
    String? observed,
  }) => LogsFilterState(
    platform: platform ?? this.platform,
    group: group ?? this.group,
    status: status ?? this.status,
    time: time ?? this.time,
    modelType: modelType ?? this.modelType,
    modelText: modelText ?? this.modelText,
    path: path ?? this.path,
    observed: observed ?? this.observed,
  );

  /// `useLogsFilters.ts:38-56`。**默认就带 `exclude_sources`** —— 测试 / 余额那两类
  /// 已经搬到 RequestLog 独立页，主列表不再混进来。
  Map<String, Object?> activeFilter(int nowMs) {
    final f = <String, Object?>{
      'exclude_sources': const ['test', 'quota'],
    };
    if (platform.isNotEmpty) f['platform_id'] = int.tryParse(platform);
    if (group.isNotEmpty) {
      f['group_key'] = group == kNoGroupSentinel ? '' : group;
    }
    if (status == 'success') {
      f['status'] = 200;
    } else if (status == 'error') {
      f['status'] = -1;
    }
    final tr = timePresetToRange(time, nowMs);
    if (tr.start != null) f['time_start'] = tr.start;
    if (tr.end != null) f['time_end'] = tr.end;
    if (modelText.trim().isNotEmpty) {
      f['model'] = modelText.trim();
      f['model_type'] = modelType;
    }
    if (path.trim().isNotEmpty) f['path'] = path.trim();
    if (observed == 'observed') f['observed'] = true;
    return f;
  }

  /// `useLogsFilters.ts:58`。
  bool get hasFilter =>
      platform.isNotEmpty ||
      group.isNotEmpty ||
      status.isNotEmpty ||
      time != 'all' ||
      modelText.trim().isNotEmpty ||
      path.trim().isNotEmpty ||
      observed.isNotEmpty;
}

/// Logs 主页：筛选 + 分页列表 + 详情，一个控制器管到底。
///
/// React 那边拆成三个 hook（filters / list / detail），是因为一个文件 400 行太长；
/// 这里三段的状态互相引用（筛选变了要回第一页、详情要用 platformMap 显示平台名），
/// 拆成三个类只会多出三份互相持有的引用。保持一个类。
class LogsController {
  LogsController({
    InvokeFn? invoke,
    this.onChanged,
    this.now = _nowMs,
    LogsFilterState? initial,
  }) : _invoke = invoke ?? kernelInvoke,
       filters = initial ?? const LogsFilterState();

  static int _nowMs() => DateTime.now().millisecondsSinceEpoch;

  final InvokeFn _invoke;
  final void Function()? onChanged;
  final int Function() now;

  /// `useLogsList.ts:5`。
  static const int defaultPageSize = 20;

  LogsFilterState filters;
  List<PlatformRow> platforms = const [];
  List<GroupDetail> groups = const [];
  List<String> modelOptions = const [];

  List<ProxyLogSummary> logs = const [];
  bool hasMore = false;
  int offset = 0;
  int pageSize = defaultPageSize;
  bool loading = true;

  /// 「清空日志」二次确认（`useLogsList.ts:18`）：破坏性操作，不确认不执行。
  bool showClearConfirm = false;
  String cleanupMessage = '';

  ProxyLogDetail? detail;
  bool copied = false;

  /// 并发去重：事件密集时在途一轮没回来就不发第二轮。
  bool _inFlight = false;

  void _notify() => onChanged?.call();

  int get currentPage => (offset ~/ pageSize) + 1;

  /// `useLogsFilters.ts:83-94`：平台 id → 名字；查不到显示 `-`（列表列的兜底）。
  String platformName(int id) {
    for (final p in platforms) {
      if (p.id == id) return p.name;
    }
    return '-';
  }

  /// `useLogsFilters.ts:94`：分组 key → 名字；查不到**回落 key 本身**（不是 `-`）。
  String groupName(String key) {
    if (key.isEmpty) return key;
    for (final g in groups) {
      if (g.group.groupKey == key) return g.group.name;
    }
    return key;
  }

  /// mount 时跑一次：下拉数据源 + 首屏列表。
  /// 三条各自 catch —— 平台列表拉不到不该让日志列表也空着（React `:34-36` 同语义）。
  Future<void> init() async {
    await Future.wait<void>([_loadPlatforms(), _loadGroups(), _loadModelOptions(), load()]);
  }

  Future<void> _loadPlatforms() async {
    try {
      final v = await _invoke('platform_list');
      platforms = [
        for (final e in (v as List? ?? const []))
          PlatformRow.fromJson(e as Map<String, dynamic>),
      ];
      _notify();
    } catch (_) {
      /* React: .catch(() => {}) */
    }
  }

  Future<void> _loadGroups() async {
    try {
      final v = await _invoke('group_detail_list');
      groups = [
        for (final e in (v as List? ?? const []))
          GroupDetail.fromJson(e as Map<String, dynamic>),
      ];
      _notify();
    } catch (_) {
      /* React: .catch(() => {}) */
    }
  }

  /// `useLogsFilters.ts:61-70`：模型下拉由后端 DISTINCT 直出，同样排掉 test/quota，
  /// 上限 200。`actual` 开关变了要重拉。
  Future<void> _loadModelOptions() async {
    try {
      final v = await _invoke('proxy_log_distinct_models', {
        'filter': {
          'exclude_sources': const ['test', 'quota'],
        },
        'actual': filters.modelType == 'actual',
        'limit': 200,
      });
      modelOptions = [for (final e in (v as List? ?? const [])) e as String];
      _notify();
    } catch (_) {
      /* ignore */
    }
  }

  /// `useLogsList.ts:21-32`。`silent` = 被动刷新（事件触发），不闪 loading。
  ///
  /// 注意这里**没有精确 COUNT**：React 侧 logs-query-ipc-slimming s2 把
  /// `proxy_log_count_filtered` 从主列表拿掉了（转发期 500ms 一次全表扫太贵），
  /// 改成后端回 `has_more` 探测下一页有没有。分页 UI 相应只有「上一页 / 下一页」。
  Future<void> load({bool silent = false}) async {
    if (_inFlight) return;
    _inFlight = true;
    if (!silent) {
      loading = true;
      _notify();
    }
    try {
      final v = await _invoke('proxy_log_list_filtered', {
        'filter': filters.activeFilter(now()),
        'limit': pageSize,
        'offset': offset,
      });
      final m = (v as Map?)?.cast<String, dynamic>() ?? const <String, dynamic>{};
      logs = [
        for (final e in (m['items'] as List? ?? const []))
          ProxyLogSummary.fromJson(e as Map<String, dynamic>),
      ];
      hasMore = (m['has_more'] as bool?) ?? false;
    } catch (_) {
      /* React: console.error(e)，列表保持原样 */
    } finally {
      _inFlight = false;
      if (!silent) loading = false;
      _notify();
    }
  }

  /// 流上来一下 = 重查一次，且是静默的（`useLogsList.ts:37-38`）。
  /// 调用方接的是 500ms 防抖流，所以一秒最多一次。
  Future<void> refreshFromEvent() async {
    await load(silent: true);
    await refreshDetail();
  }

  /// 改筛选：回第一页（`useLogsList.ts:35`）再重查。
  Future<void> setFilters(LogsFilterState next) async {
    final modelTypeChanged = next.modelType != filters.modelType;
    filters = next;
    offset = 0;
    _notify();
    if (modelTypeChanged) await _loadModelOptions();
    await load();
  }

  /// `useLogsFilters.ts:72-81`：全部字段复位到初值（含 modelType 回 `actual`）。
  Future<void> clearFilter() => setFilters(const LogsFilterState());

  Future<void> setPageSize(int size) async {
    pageSize = size;
    offset = 0;
    _notify();
    await load();
  }

  Future<void> goToPage(int page) async {
    offset = (page - 1) * pageSize;
    _notify();
    await load();
  }

  /// 清空全部日志。**破坏性**，所以入口是 [showClearConfirm] 打开的确认框，
  /// 本方法只在确认之后调（`useLogsList.ts:40-47`）。
  Future<void> confirmClear() async {
    try {
      await _invoke('proxy_log_clear');
      showClearConfirm = false;
      offset = 0;
      _notify();
      await load();
    } catch (_) {
      /* React: console.error(e) */
    }
  }

  /// 清理过期日志（按 retention 设置），不删当前有效行，所以**不需要二次确认**
  /// —— 与 React 一致（`useLogsList.ts:49-57`）。成功后挂 3 秒提示。
  Future<void> cleanupExpired({String doneText = '已清理过期日志'}) async {
    try {
      await _invoke('proxy_log_cleanup_expired');
      offset = 0;
      _notify();
      await load();
      cleanupMessage = doneText;
      _notify();
    } catch (_) {
      /* React: console.error(e) */
    }
  }

  void dismissCleanupMessage() {
    cleanupMessage = '';
    _notify();
  }

  Future<void> openDetail(String id) async {
    try {
      final v = await _invoke('proxy_log_get', {'id': id});
      if (v != null) {
        detail = ProxyLogDetail.fromJson((v as Map).cast<String, dynamic>());
        _notify();
      }
    } catch (_) {
      /* React: console.error(e) */
    }
  }

  void closeDetail() {
    detail = null;
    _notify();
  }

  /// 详情开着时跟着刷新（React 用的是 1000ms 防抖的第二个订阅；这里并进
  /// [refreshFromEvent]，同一次事件只重查一次详情）。
  Future<void> refreshDetail() async {
    final d = detail;
    if (d == null) return;
    try {
      final v = await _invoke('proxy_log_get', {'id': d.id});
      if (v != null) {
        detail = ProxyLogDetail.fromJson((v as Map).cast<String, dynamic>());
        _notify();
      }
    } catch (_) {
      /* React: .catch(() => {}) */
    }
  }

  /// 列表行的「复制」：先取详情再整条复制（`useLogsDetail.ts:86-91`）。
  Future<void> copyRow(String id, Future<void> Function(String) write) async {
    try {
      final v = await _invoke('proxy_log_get', {'id': id});
      if (v != null) {
        await copyDetail(
          ProxyLogDetail.fromJson((v as Map).cast<String, dynamic>()),
          write,
        );
      }
    } catch (_) {
      /* React: console.error(err) */
    }
  }

  Future<void> copyDetail(
    ProxyLogDetail d,
    Future<void> Function(String) write,
  ) async {
    try {
      await write(buildProxyLogMarkdown(d));
      copied = true;
      _notify();
    } catch (_) {
      /* React: console.error(e) */
    }
  }

  void clearCopied() {
    copied = false;
    _notify();
  }
}

/// `useLogsDetail.ts:18-71` 的 markdown，逐行照搬（顺序、标题、兜底文案都不动）。
///
/// 两处兜底要注意：`[stream]` 是**已废的哨兵**（票 06），但 React 至今仍在判它，
/// 所以这里也判 —— 老库里还躺着写了哨兵的行，不判就会把字面量 `[stream]` 复制出去。
String buildProxyLogMarkdown(ProxyLogDetail d) {
  String fj(String s) => prettyJsonOrRaw(s);
  bool real(String s) => s.isNotEmpty && s != '[stream]';
  final userBody = real(d.userResponseBody)
      ? fj(d.userResponseBody)
      : real(d.responseBody)
      ? fj(d.responseBody)
      : '(streaming, not captured)';
  return [
    '# Proxy Log ${d.id}',
    '',
    '## Meta',
    '- ID: ${d.id}',
    '- Group: ${d.groupKey}',
    '- Model: ${d.model.isEmpty ? '-' : d.model}',
    '- Actual Model: ${d.actualModel.isEmpty ? '-' : d.actualModel}',
    '- Source Protocol: ${d.sourceProtocol.isEmpty ? '-' : d.sourceProtocol}',
    '- Target Protocol: ${d.targetProtocol.isEmpty ? '-' : d.targetProtocol}',
    '- Status: ${d.statusCode}',
    '- Duration: ${d.durationMs} ms',
    '- Input Tokens: ${d.inputTokens}',
    '- Output Tokens: ${d.outputTokens}',
    '- Cache Tokens: ${d.cacheTokens}',
    '- Time: ${d.createdAt}',
    '',
    '## User Request (Client → Proxy)',
    '- URL: ${d.requestUrl.isEmpty ? '-' : d.requestUrl}',
    '- Status Code: ${d.statusCode}',
    '### Request Headers',
    fj(d.requestHeaders),
    '',
    '### Request Body',
    fj(d.requestBody),
    '',
    '### Response Headers',
    fj(d.userResponseHeaders.isEmpty ? '{}' : d.userResponseHeaders),
    '',
    '### Response Body',
    userBody,
    '',
    '## Upstream Request (Proxy → Platform)',
    '- URL: ${d.upstreamRequestUrl.isEmpty ? '-' : d.upstreamRequestUrl}',
    '- Status Code: ${d.upstreamStatusCode == 0 ? '-' : d.upstreamStatusCode}',
    '### Request Headers',
    fj(d.upstreamRequestHeaders),
    '',
    '### Request Body',
    d.upstreamRequestBody.isNotEmpty
        ? fj(d.upstreamRequestBody)
        : '(not captured)',
    '',
    '### Response Headers',
    fj(d.upstreamResponseHeaders.isEmpty ? '{}' : d.upstreamResponseHeaders),
    '',
    '### Response Body',
    d.responseBody.isNotEmpty ? fj(d.responseBody) : '(streaming, not captured)',
  ].join('\n');
}

// ── RequestLog（测试 / 余额）页 ────────────────────────────────────

/// `RequestLog.tsx:32`：类型筛选三选一。
const List<String> kRequestLogTypes = ['all', 'test', 'quota'];

/// `RequestLog.tsx:34-37`：`all` → 不带（后端默认就是 `[test, quota]`）；否则单元素数组。
List<String>? typeToSources(String t) => t == 'all' ? null : [t];

/// RequestLog 页（测试 / 余额请求）。与 Logs 主页是**互补**的两半：
/// 那边 `exclude_sources: [test, quota]`，这边 `sources: [test, quota]`。
class RequestLogController {
  RequestLogController({InvokeFn? invoke, this.onChanged, this.now = _nowMs})
    : _invoke = invoke ?? kernelInvoke;

  static int _nowMs() => DateTime.now().millisecondsSinceEpoch;

  final InvokeFn _invoke;
  final void Function()? onChanged;
  final int Function() now;

  /// `RequestLog.tsx:29`。
  static const int defaultPageSize = 20;

  String filterType = 'all';
  String filterPlatform = '';
  String filterStatus = '';
  String filterTime = 'all';

  List<PlatformRow> platforms = const [];
  List<ProxyLogSummary> logs = const [];
  int total = 0;
  int offset = 0;
  int pageSize = defaultPageSize;
  bool loading = true;

  ProxyLogDetail? detail;
  bool copied = false;

  bool _inFlight = false;

  void _notify() => onChanged?.call();

  /// `RequestLog.tsx:99-110`。
  Map<String, Object?> get activeFilter {
    final f = <String, Object?>{};
    final srcs = typeToSources(filterType);
    if (srcs != null) f['sources'] = srcs;
    if (filterPlatform.isNotEmpty) {
      f['platform_id'] = int.tryParse(filterPlatform);
    }
    if (filterStatus == 'success') {
      f['status'] = 200;
    } else if (filterStatus == 'error') {
      f['status'] = -1;
    }
    final tr = timePresetToRange(filterTime, now());
    if (tr.start != null) f['time_start'] = tr.start;
    if (tr.end != null) f['time_end'] = tr.end;
    return f;
  }

  /// `RequestLog.tsx:112`。
  bool get hasFilter =>
      filterType != 'all' ||
      filterPlatform.isNotEmpty ||
      filterStatus.isNotEmpty ||
      filterTime != 'all';

  /// `RequestLog.tsx:181-182`：本页仍有精确 total（`proxy_log_count_filtered`），
  /// 所以分页是「第 N / 共 M 页」，与 Logs 主页的 has_more 形态不同。
  int get totalPages {
    final p = (total / pageSize).ceil();
    return p < 1 ? 1 : p;
  }

  int get currentPage => (offset ~/ pageSize) + 1;

  String platformName(int id) {
    for (final p in platforms) {
      if (p.id == id) return p.name;
    }
    return '-';
  }

  /// `RequestLog.tsx:179`：本页没有分组维度（测试 / 余额不经组路由），
  /// 所以 group 列直接显示 group_key，空则 `-`。
  String groupName(String key) => key.isEmpty ? '-' : key;

  Future<void> init() async {
    await Future.wait<void>([_loadPlatforms(), load()]);
  }

  Future<void> _loadPlatforms() async {
    try {
      final v = await _invoke('platform_list');
      platforms = [
        for (final e in (v as List? ?? const []))
          PlatformRow.fromJson(e as Map<String, dynamic>),
      ];
      _notify();
    } catch (_) {
      /* React: .catch(() => {}) */
    }
  }

  /// `RequestLog.tsx:114-129`：列表与 count 两条并发。
  /// count 用的是 `proxy_log_count_filtered`（`request_log_list` 不回总数），
  /// 且**补上 sources 默认值** —— 不补的话 count 会把全部来源都算进去，页数偏大。
  Future<void> load({bool silent = false}) async {
    if (_inFlight) return;
    _inFlight = true;
    if (!silent) {
      loading = true;
      _notify();
    }
    final f = activeFilter;
    final countFilter = Map<String, Object?>.from(f);
    countFilter['sources'] ??= const ['test', 'quota'];
    try {
      final r = await Future.wait<Object?>([
        _invoke('request_log_list', {
          'filter': f,
          'limit': pageSize,
          'offset': offset,
        }),
        _invoke('proxy_log_count_filtered', {'filter': countFilter}),
      ]);
      logs = [
        for (final e in (r[0] as List? ?? const []))
          ProxyLogSummary.fromJson(e as Map<String, dynamic>),
      ];
      total = (r[1] as num?)?.toInt() ?? 0;
    } catch (_) {
      /* React: console.error(e) */
    } finally {
      _inFlight = false;
      if (!silent) loading = false;
      _notify();
    }
  }

  Future<void> refreshFromEvent() async {
    await load(silent: true);
    await refreshDetail();
  }

  /// 改筛选 → 回第一页（`RequestLog.tsx:132`）。
  Future<void> _applyFilterChange() async {
    offset = 0;
    _notify();
    await load();
  }

  Future<void> setFilterType(String v) {
    filterType = v;
    return _applyFilterChange();
  }

  Future<void> setFilterPlatform(String v) {
    filterPlatform = v;
    return _applyFilterChange();
  }

  Future<void> setFilterStatus(String v) {
    filterStatus = v;
    return _applyFilterChange();
  }

  Future<void> setFilterTime(String v) {
    filterTime = v;
    return _applyFilterChange();
  }

  /// `RequestLog.tsx:165-170`。
  Future<void> clearFilter() {
    filterType = 'all';
    filterPlatform = '';
    filterStatus = '';
    filterTime = 'all';
    return _applyFilterChange();
  }

  Future<void> setPageSize(int size) {
    pageSize = size;
    return _applyFilterChange();
  }

  Future<void> goToPage(int page) async {
    offset = (page - 1) * pageSize;
    _notify();
    await load();
  }

  Future<void> openDetail(String id) async {
    try {
      final v = await _invoke('proxy_log_get', {'id': id});
      if (v != null) {
        detail = ProxyLogDetail.fromJson((v as Map).cast<String, dynamic>());
        _notify();
      }
    } catch (_) {
      /* React: console.error(e) */
    }
  }

  void closeDetail() {
    detail = null;
    _notify();
  }

  Future<void> refreshDetail() async {
    final d = detail;
    if (d == null) return;
    try {
      final v = await _invoke('proxy_log_get', {'id': d.id});
      if (v != null) {
        detail = ProxyLogDetail.fromJson((v as Map).cast<String, dynamic>());
        _notify();
      }
    } catch (_) {
      /* React: .catch(() => {}) */
    }
  }

  Future<void> copyRow(String id, Future<void> Function(String) write) async {
    try {
      final v = await _invoke('proxy_log_get', {'id': id});
      if (v != null) {
        await copyDetail(
          ProxyLogDetail.fromJson((v as Map).cast<String, dynamic>()),
          write,
        );
      }
    } catch (_) {
      /* React: console.error(err) */
    }
  }

  Future<void> copyDetail(
    ProxyLogDetail d,
    Future<void> Function(String) write,
  ) async {
    try {
      await write(buildRequestLogMarkdown(d));
      copied = true;
      _notify();
    } catch (_) {
      /* React: console.error(e) */
    }
  }

  void clearCopied() {
    copied = false;
    _notify();
  }
}

/// `RequestLog.tsx:39-73` 的 markdown。**与 Logs 主页那份不同**：段落更少
/// （没有 Response Headers、没有 token 三行），时间是格式化过的本地时刻而不是毫秒戳。
/// 两份各自照搬，不合并 —— 合并就得加参数分支，读的人反而要先想「这次走哪支」。
String buildRequestLogMarkdown(ProxyLogDetail d) {
  String fj(String s) => prettyJsonOrRaw(s);
  final userBody = d.userResponseBody.isNotEmpty
      ? fj(d.userResponseBody)
      : d.responseBody.isNotEmpty
      ? fj(d.responseBody)
      : '(streaming, not captured)';
  return [
    '# Request Log ${d.id}',
    '',
    '## Meta',
    '- Group: ${d.groupKey}',
    '- Model: ${d.model.isEmpty ? '-' : d.model}',
    '- Actual Model: ${d.actualModel.isEmpty ? '-' : d.actualModel}',
    '- Source Protocol: ${d.sourceProtocol.isEmpty ? '-' : d.sourceProtocol}',
    '- Target Protocol: ${d.targetProtocol.isEmpty ? '-' : d.targetProtocol}',
    '- Status: ${d.statusCode}',
    '- Duration: ${d.durationMs} ms',
    '- Time: ${formatDateTime(d.createdAt)}',
    '',
    '## User Request',
    '- URL: ${d.requestUrl.isEmpty ? '-' : d.requestUrl}',
    '### Request Headers',
    fj(d.requestHeaders),
    '',
    '### Request Body',
    fj(d.requestBody),
    '',
    '### Response Body',
    userBody,
    '',
    '## Upstream Request',
    '- URL: ${d.upstreamRequestUrl.isEmpty ? '-' : d.upstreamRequestUrl}',
    '### Request Headers',
    fj(d.upstreamRequestHeaders),
    '',
    '### Request Body',
    d.upstreamRequestBody.isNotEmpty
        ? fj(d.upstreamRequestBody)
        : '(not captured)',
    '',
    '### Response Body',
    d.responseBody.isNotEmpty ? fj(d.responseBody) : '(streaming, not captured)',
  ].join('\n');
}
