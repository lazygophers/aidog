/// 分组页逻辑层（票 I07），对应 `src/pages/Groups.tsx`（38.6 KB，全库第二大页）
/// 与它的 `src/pages/Groups/` 六个子文件 + `src/domains/groups/`。
///
/// 这一页承载的是**路由配置**：哪些平台归哪个组、组内优先级、调度策略、模型映射、
/// 环境变量。写错一个字段不是显示问题，是请求发去错的地方。所以下面三类东西按
/// React 逐条照搬，不做「差不多」的简化：
///
/// 1. **校验**：能不能点「创建」/「保存」，判据与 React 的 `disabled=` 完全一致；
///    分组密钥输入实时剔掉 `[^\w-]`（创建后锁定不可改，输错就得删组重建）。
/// 2. **破坏性操作一律先确认**：删组、删平台、批量删除、清理失效，四条都有独立的
///    确认态，确认之前一个命令都不发。
/// 3. **跨组成员关系实时拉后端**：「移除平台」弹窗里「这个平台还在几个组里」必须用
///    `group_detail_list` 现拉，不能用前端已分页的 `details` —— 用 stale 值算出
///    overcount 时，单组平台会被「移出本组」变成未分组而不是被删掉（React 07-08
///    回归的根因，`Groups.tsx:200-203` 把这段写成了注释留在那里）。
library;

import 'dart:convert';

import 'invoke.dart';
import 'models.dart';
/// 调度策略的全表与短名已由票 I08 落在设置页逻辑层（同一份 `routing.ts` 的投影），
/// 这里**转出去复用**，不抄第二份 —— 两页的下拉必须是同一个顺序同一套文案。
export 'settings/scheduling_logic.dart' show kRoutingModeLabels, kRoutingModes;

/// 策略说明（下拉旁的一行提示），对齐 `src/domains/groups/routing.ts:20::routingModeDesc`。
/// 取不到 key 时回落空串（与 React 的 `?? ""` 一致，注意与 label 的 `?? mode` 不同）。
const Map<String, (String key, String fallback)> kRoutingModeDescs = {
  'failover': ('group.routingModeDesc.failover', '按优先级升序选平台，失败逐个回退。'),
  'load_balance': ('group.routingModeDesc.load_balance', '在可用平台间加权随机分流。'),
  'health_aware': (
    'group.routingModeDesc.health_aware',
    '摘除熔断平台后，在健康平台间加权随机。',
  ),
  'least_latency': (
    'group.routingModeDesc.least_latency',
    '按各平台延迟均值升序优先选最快平台。',
  ),
  'sticky': ('group.routingModeDesc.sticky', '同会话绑定同一平台，失效/熔断后回退加权随机。'),
};

/// 新建分组预填的隐私环境变量，逐条照抄 `editReducer.ts:21-34`（顺序不动）。
/// 只有**新建**用这份初值；编辑已有组走 [GroupEditState.open]，用组自己的 env_vars。
const List<EnvVar> kPrivacyDefaultEnvVars = [
  EnvVar(key: 'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC', value: '1'),
  EnvVar(key: 'CLAUDE_CODE_ENABLE_TELEMETRY', value: '0'),
  EnvVar(key: 'CLAUDE_CODE_ENHANCED_TELEMETRY_BETA', value: '0'),
  EnvVar(key: 'CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY', value: '1'),
  EnvVar(key: 'CLAUDE_CODE_BYOC_ENABLE_DATADOG', value: '0'),
  EnvVar(key: 'CLAUDE_CODE_PROPAGATE_TRACEPARENT', value: '0'),
  EnvVar(key: 'DISABLE_GROWTHBOOK', value: '1'),
  EnvVar(key: 'CLAUDE_CODE_ATTRIBUTION_HEADER', value: '0'),
  EnvVar(key: 'DISABLE_INSTALLATION_CHECKS', value: '1'),
  EnvVar(key: 'CLAUDE_CODE_DISABLE_BG_EXIT_HANDOFF', value: '1'),
  EnvVar(key: 'CLAUDE_CODE_DISABLE_CRON', value: '1'),
  EnvVar(key: 'CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL', value: '1'),
];

/// `src/domains/groups/index.ts` 的批量测试并发上限。
const int kBatchTestConcurrency = 4;

/// 分组密钥输入过滤：`GroupCreateModal.tsx:78` 的 `replace(/[^\w-]/g, "")`。
/// JS 的 `\w` = `[A-Za-z0-9_]`，所以允许的字符集是「字母数字下划线连字符」。
///
/// 为什么值得单独一个函数：密钥创建后**锁定不可改**，且它同时是 Bearer token
/// 与路由匹配键。放进去一个空格，用户拿到的就是一个永远匹配不上的组。
String sanitizeGroupKey(String raw) =>
    raw.replaceAll(RegExp(r'[^A-Za-z0-9_-]'), '');

/// `editReducer.ts:4-46` 的编辑态。React 那边是 reducer + 三个 action；
/// Dart 这边同一个东西用不可变类 + 三个具名构造/方法表达，少一层 dispatch 间接。
class GroupEditState {
  const GroupEditState({
    this.target,
    this.name = '',
    this.mode = 'failover',
    this.platformIds = const [],
    this.mappings = const [],
    this.envVars = kPrivacyDefaultEnvVars,
    this.reqTimeout = 0,
    this.connTimeout = 0,
    this.maxRetries = 10,
  });

  /// `EMPTY_EDIT`（`editReducer.ts:36`）。`reset` 回到的就是这一个常量实例。
  static const GroupEditState empty = GroupEditState();

  /// `{type:"open"}`：按 GroupDetail 字段逐一映射。
  /// mappings / envVars 都**重建成新对象**，不复用 detail 里的实例 —— 编辑时改一个
  /// 字段不该把列表里那份也改了（`editReducer.test.ts:53` 专门盯这条）。
  factory GroupEditState.open(GroupDetail detail) => GroupEditState(
    target: detail,
    name: detail.group.name,
    mode: detail.group.routingMode,
    platformIds: [for (final gp in detail.platforms) gp.platform.id],
    mappings: [
      for (final m in detail.modelMappings)
        ModelMapping(
          sourceModel: m.sourceModel,
          targetPlatformId: m.targetPlatformId,
          targetModel: m.targetModel,
          requestTimeoutSecs: m.requestTimeoutSecs,
          connectTimeoutSecs: m.connectTimeoutSecs,
        ),
    ],
    envVars: [
      for (final e in detail.group.envVars) EnvVar(key: e.key, value: e.value),
    ],
    reqTimeout: detail.group.requestTimeoutSecs,
    connTimeout: detail.group.connectTimeoutSecs,
    maxRetries: detail.group.maxRetries,
  );

  final GroupDetail? target;
  final String name;
  final String mode;
  final List<int> platformIds;
  final List<ModelMapping> mappings;
  final List<EnvVar> envVars;
  final int reqTimeout;
  final int connTimeout;
  final int maxRetries;

  /// `{type:"patch"}`：浅合并，没传的字段保留原值（引用也保留，`editReducer.test.ts:72`）。
  GroupEditState patch({
    String? name,
    String? mode,
    List<int>? platformIds,
    List<ModelMapping>? mappings,
    List<EnvVar>? envVars,
    int? reqTimeout,
    int? connTimeout,
    int? maxRetries,
  }) => GroupEditState(
    target: target,
    name: name ?? this.name,
    mode: mode ?? this.mode,
    platformIds: platformIds ?? this.platformIds,
    mappings: mappings ?? this.mappings,
    envVars: envVars ?? this.envVars,
    reqTimeout: reqTimeout ?? this.reqTimeout,
    connTimeout: connTimeout ?? this.connTimeout,
    maxRetries: maxRetries ?? this.maxRetries,
  );

  /// 「保存」按钮的启用判据：`GroupEditPanel.tsx:68` 的 `disabled={!editName}`。
  /// 注意是 `!name`（空串才禁），**不 trim** —— 一个空格在 React 里是可以保存的，
  /// 这里照搬，不擅自收紧。
  bool get canSave => name.isNotEmpty;
}

/// `editReducer.ts:81::upsertPlatformInto`：按 id 替换或追加。
/// 命中时只换那一项，其余项**保持原引用**（卡片的 memo 靠这个不重渲染）。
List<PlatformRow> upsertPlatformInto(List<PlatformRow> prev, PlatformRow plat) {
  final idx = prev.indexWhere((p) => p.id == plat.id);
  if (idx == -1) return [...prev, plat];
  final next = [...prev];
  next[idx] = plat;
  return next;
}

/// `src/domains/groups/query.ts:8::platformMatchesQuery`。
///
/// **一处与 React 不同，写下来**：React 用 `pinyinMatch`（`pinyin-pro` 的汉字字典），
/// 这里是不分大小写子串。沿用票 I06 已记录的同一取舍（Dart 侧的等价字典包 5 年
/// 未更新且许可证未核实，不往 AGPL 仓库里引）。**平台不受影响** —— registry 的
/// `keywords` 本来就把全拼与首字母当字面数据存着（项目 CLAUDE.md：「智谱 → zhipu + zp」），
/// 它们经 [protocolTerms] 进来。受影响的只有用户自起中文名的平台/分组：输拼音搜不到。
bool platformMatchesQuery(
  PlatformRow p,
  String q, [
  Map<String, List<String>>? protocolTerms,
]) {
  final needle = q.toLowerCase();
  if (needle.isEmpty) return true;
  if (p.name.toLowerCase().contains(needle)) return true;
  if (p.baseUrl.toLowerCase().contains(needle)) return true;
  if (p.platformType.toLowerCase().contains(needle)) return true;
  final terms = protocolTerms?[p.platformType];
  return terms != null && terms.any((t) => t.toLowerCase().contains(needle));
}

/// `query.ts:20::groupMatchesQuery`：命中组名或组密钥 → 整组展开。
bool groupMatchesQuery(GroupRow g, String q) {
  final needle = q.toLowerCase();
  if (needle.isEmpty) return true;
  return g.name.toLowerCase().contains(needle) ||
      g.groupKey.toLowerCase().contains(needle);
}

/// `useGroupData.ts:14::fetchGroupStats` 的纯函数部分（余额那一半）。
///
/// 余额**是平台级属性**：把组内各平台的 `est_balance_remaining` 求和，只累加 >0 的。
/// 没有 per-group 余额概念（项目 CLAUDE.md 的「Group 统计」段），别按组去后端要。
Map<int, double> groupBalanceOf(
  List<GroupDetail> details,
  List<PlatformRow> platforms,
) {
  final byId = {for (final p in platforms) p.id: p};
  final out = <int, double>{};
  for (final g in details) {
    var balance = 0.0;
    for (final gp in g.platforms) {
      final est = byId[gp.platform.id]?.estBalanceRemaining;
      if (est != null && est > 0) balance += est;
    }
    if (balance > 0) out[g.group.id] = balance;
  }
  return out;
}

/// `useGroupData.ts:30`：后端 fallback 直通落库用的虚拟 group_key，只读展示一张卡。
const String kUnmatchedGroupKey = '未匹配';

/// 「移除平台」弹窗的上下文（`Groups.tsx:174-176`）。
///
/// 为什么总是弹窗、不按数量直接执行：`Groups.tsx:172-173` 的注释说得很直白 ——
/// 「去掉 count 决定行为，避免 groupCountOf stale 走错分支」。数量只用来决定
/// **弹窗里给几个选项**，不用来跳过弹窗。
class RemovePlatformTarget {
  const RemovePlatformTarget({
    required this.platform,
    required this.groupId,
    required this.groupCount,
    required this.groupNames,
  });

  final PlatformRow platform;
  final int groupId;

  /// 该平台当前归属的组数（实时拉后端算的，不是前端 details 数的）。
  final int groupCount;
  final List<String> groupNames;

  /// 只属这一个组 → 弹窗只有「删除平台」一个动作；属多个组 → 多给一个
  /// 「仅移出本组」。判据与 `GroupListView` 里渲染分支一致。
  bool get onlyInThisGroup => groupCount <= 1;
}

/// 批量删除弹窗的上下文（`Groups.tsx:248-251`）。
class BatchDeleteTarget {
  const BatchDeleteTarget({
    required this.platforms,
    required this.groupNamesByPlatform,
  });

  final List<PlatformRow> platforms;

  /// platform id → 它当前所属的全部组名。跨组的要在弹窗里警告
  /// （删掉就是从所有组里消失，不只是本组）。
  final Map<int, List<String>> groupNamesByPlatform;

  /// 有任何一个选中平台属于 2 个以上的组 → 弹窗要显示跨组警告。
  bool get hasCrossGroup =>
      groupNamesByPlatform.values.any((names) => names.length > 1);
}

/// 一键测试本组平台的单行结果（`domains/groups::GroupTestRow`）。
class GroupTestRow {
  GroupTestRow({required this.platformId, required this.name});

  final int platformId;
  final String name;

  /// `pending` / `testing` / `ok` / `fail`。
  String status = 'pending';
  int? durationMs;
  String? error;
}

/// 一键测试面板态（`useGroupTest.ts:6`）。
class GroupTestState {
  GroupTestState({
    required this.groupId,
    required this.groupName,
    required this.rows,
    this.running = true,
  });

  final int groupId;
  final String groupName;
  final List<GroupTestRow> rows;
  bool running;
}

/// 分组页控制器。
///
/// 数据加载形态照搬 React（`useGroupData.ts`）：平台列表一次全量，组列表**分页**
/// （`group_detail_list_paged`，每页 12），滚到底再要下一页。写操作后一律走
/// [silentReload] —— 不清空列表、不回第一页、不闪 loading，因为写操作只改一两行，
/// 没有理由让用户的滚动位置归零。
class GroupsController {
  GroupsController({InvokeFn? invoke, this.onChanged, this.onToast})
    : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  /// 提示条。`ok=false` 是错误。React 用的是 3 秒自动消失，调用方负责计时。
  final void Function(String text, {required bool ok})? onToast;

  /// `useGroupData.ts:74`。
  static const int pageSize = 12;

  List<GroupDetail> details = const [];
  List<PlatformRow> platforms = const [];
  Map<String, UsageStats> groupStats = const {};
  Map<int, double> groupBalance = const {};
  UsageStats? unmatchedStat;
  Map<String, List<String>> protocolTerms = const {};

  bool loading = true;
  bool loadingMore = false;
  bool hasMore = true;
  int proxyPort = 7890;

  /// 视图态：编辑 / 新建 / 列表三选一。
  GroupEditState edit = GroupEditState.empty;
  bool showCreate = false;

  // ── 新建表单（`Groups.tsx:79-82`）──
  String createName = '';
  String createGroupKey = '';
  String createMode = 'health_aware';
  List<int> createPlatformIds = const [];

  // ── 确认态：每一个破坏性动作一个，确认前不发命令 ──
  RemovePlatformTarget? removeTarget;
  BatchDeleteTarget? batchDeleteTarget;
  bool batchDeleteBusy = false;

  /// 待批量操作的平台（覆盖模型 / 改状态 / 移组共用三个独立的目标态）。
  List<PlatformRow>? batchOverrideTarget;
  bool batchOverrideBusy = false;
  ({List<PlatformRow> platforms, int groupId})? batchSetStatusTarget;
  List<int> batchSetStatusGroupEnabledIds = const [];
  bool batchSetStatusBusy = false;
  ({List<PlatformRow> platforms, int groupId})? batchMoveGroupTarget;
  bool batchMoveGroupBusy = false;

  /// 删组确认（React 走 AlertDialog，这里把「哪个组待删」记成态）。
  int? deleteGroupTarget;

  /// 清理失效平台：先拉预览清单给用户看，确认后才真删。
  ({int? groupId, List<Map<String, Object?>> candidates})? purgeTarget;

  GroupTestState? groupTest;

  /// 折叠的组 id（默认全展开，`Groups.tsx:125`）。
  Set<int> collapsedGroups = <int>{};

  int _loadSeq = 0;
  int _nextOffset = 0;
  List<GroupDetail> _loadedDetails = const [];
  bool _loadingMoreGuard = false;

  void _notify() => onChanged?.call();

  void _toast(String text, {bool ok = true}) => onToast?.call(text, ok: ok);

  /// `useGroupData.ts:67`。
  String get proxyBaseUrl => 'http://127.0.0.1:$proxyPort/proxy';

  /// `Groups.tsx:827`：批量「移组」下拉的候选。
  List<({int id, String name})> get allGroups => [
    for (final d in details) (id: d.group.id, name: d.group.name),
  ];

  // ── 加载 ────────────────────────────────────────────────────────

  Future<void> init() async {
    await Future.wait<void>([load(), _loadProxySettings(), _loadProtocolTerms()]);
  }

  Future<void> _loadProxySettings() async {
    try {
      final v = await _invoke('proxy_get_settings');
      final port = ((v as Map?)?['port'] as num?)?.toInt();
      if (port != null && port > 0) {
        proxyPort = port;
        _notify();
      }
    } catch (_) {
      /* React: 兜底 7890 */
    }
  }

  /// `Groups.tsx:681-685`：协议的跨语言搜索词，挂载时拉一次。
  /// 返回的是一整份 defaults JSON 字符串，这里只取 `protocols.<code>.keywords` +
  /// `name` 的各 locale 值（与 `defaults.ts::getProtocolSearchTermsMap` 同口径）。
  Future<void> _loadProtocolTerms() async {
    try {
      final raw = await _invoke('get_defaults_json');
      protocolTerms = parseProtocolSearchTerms(raw as String? ?? '');
      _notify();
    } catch (_) {
      /* React: .catch(console.error)，搜索退化为不含协议词条 */
    }
  }

  /// 全量重载（mount / 组结构变化）：重置分页游标，拉第一页。
  Future<void> load() async {
    final seq = ++_loadSeq;
    bool alive() => seq == _loadSeq;
    loading = true;
    _nextOffset = 0;
    _loadedDetails = const [];
    _loadingMoreGuard = false;
    hasMore = true;
    details = const [];
    platforms = const [];
    _notify();
    try {
      final v = await _invoke('platform_list');
      if (!alive()) return;
      platforms = [
        for (final e in (v as List? ?? const []))
          PlatformRow.fromJson(e as Map<String, dynamic>),
      ];
      loading = false;
      _notify();
      await loadMore();
    } catch (_) {
      if (alive()) {
        loading = false;
        _notify();
      }
    }
  }

  /// 触底加载下一页（`useGroupData.ts:116`）。
  Future<void> loadMore() async {
    if (_loadingMoreGuard) return;
    final seq = _loadSeq;
    bool alive() => seq == _loadSeq;
    _loadingMoreGuard = true;
    loadingMore = true;
    _notify();
    try {
      final offset = _nextOffset;
      final v = await _invoke('group_detail_list_paged', {
        'offset': offset,
        'limit': pageSize,
      });
      if (!alive()) return;
      final page = [
        for (final e in (v as List? ?? const []))
          GroupDetail.fromJson(e as Map<String, dynamic>),
      ];
      _nextOffset = offset + pageSize;
      if (page.length < pageSize) hasMore = false;
      if (page.isEmpty) return;
      _loadedDetails = [..._loadedDetails, ...page];
      details = [...details, ...page];
      var next = platforms;
      for (final d in page) {
        for (final gp in d.platforms) {
          next = upsertPlatformInto(next, gp.platform);
        }
      }
      platforms = next;
      await _recomputeStats(alive);
    } catch (_) {
      /* React: console.error(e) */
    } finally {
      _loadingMoreGuard = false;
      if (alive()) {
        loadingMore = false;
        _notify();
      }
    }
  }

  /// 静默重载（写操作之后）：一次性拉回「当前已加载的页深」条，整批替换，不闪空白。
  /// `useGroupData.ts:194` 的整段理由值得再说一遍：`load()` 会先清空再从 offset 0
  /// 重新分页，用户体感就是整页刷新 —— 滚动位置归零、第 2 页之后全丢。
  Future<void> silentReload() async {
    final seq = ++_loadSeq;
    bool alive() => seq == _loadSeq;
    final keep = _loadedDetails.length > pageSize ? _loadedDetails.length : pageSize;
    try {
      final r = await Future.wait<Object?>([
        _invoke('platform_list'),
        _invoke('group_detail_list_paged', {'offset': 0, 'limit': keep}),
      ]);
      if (!alive()) return;
      platforms = [
        for (final e in (r[0] as List? ?? const []))
          PlatformRow.fromJson(e as Map<String, dynamic>),
      ];
      final page = [
        for (final e in (r[1] as List? ?? const []))
          GroupDetail.fromJson(e as Map<String, dynamic>),
      ];
      _loadingMoreGuard = false;
      _nextOffset = keep;
      hasMore = page.length >= keep;
      _loadedDetails = page;
      details = page;
      await _recomputeStats(alive);
    } catch (_) {
      /* React: console.error(e) */
    }
    _notify();
  }

  /// 轻量刷新（`proxy-log-updated` 事件驱动）：只刷平台快照 + 重算统计，
  /// **不重拉组** —— 组结构没变，重拉只会打断用户的滚动与展开态。
  Future<void> refreshStats() async {
    try {
      final v = await _invoke('platform_list');
      platforms = [
        for (final e in (v as List? ?? const []))
          PlatformRow.fromJson(e as Map<String, dynamic>),
      ];
      await _recomputeStats(() => true);
      _notify();
    } catch (_) {
      /* ignore */
    }
  }

  /// 统计 = 一次批量 `all_group_usage_stats`（后端 GROUP BY group_key）+
  /// 前端按组求和余额。React 早期是逐组 N+1 往返，现在是单次，别退回去。
  Future<void> _recomputeStats(bool Function() alive) async {
    try {
      final v = await _invoke('all_group_usage_stats');
      final all = (v as Map?)?.cast<String, dynamic>() ?? const <String, dynamic>{};
      if (!alive()) return;
      final stats = <String, UsageStats>{};
      for (final g in _loadedDetails) {
        final raw = all[g.group.groupKey];
        if (raw is Map) {
          final s = UsageStats.fromJson(raw.cast<String, dynamic>());
          if (s.totalRequests > 0) stats[g.group.groupKey] = s;
        }
      }
      final u = all[kUnmatchedGroupKey];
      unmatchedStat = u is Map
          ? () {
              final s = UsageStats.fromJson(u.cast<String, dynamic>());
              return s.totalRequests > 0 ? s : null;
            }()
          : null;
      groupStats = stats;
    } catch (_) {
      /* React: try/catch 吞掉，统计缺失不挡列表 */
    }
    groupBalance = groupBalanceOf(_loadedDetails, platforms);
    _notify();
  }

  /// 单组就地刷新：只重拉该组（一次往返），原地替换。
  /// 组已不存在（被删）或不在当前页 → 回退静默全量。
  Future<void> refreshSingleGroup(int gid) async {
    try {
      final v = await _invoke('group_detail', {'id': gid});
      if (v == null) {
        await silentReload();
        return;
      }
      final filled = GroupDetail.fromJson((v as Map).cast<String, dynamic>());
      // 不在当前已加载的页里（用户可能刚翻过页）→ 退回静默全量，不硬插一条进列表。
      if (!details.any((x) => x.group.id == gid)) {
        await silentReload();
        return;
      }
      details = [
        for (final x in details) x.group.id == gid ? filled : x,
      ];
      _loadedDetails = [
        for (final x in _loadedDetails) x.group.id == gid ? filled : x,
      ];
      var next = platforms;
      for (final gp in filled.platforms) {
        next = upsertPlatformInto(next, gp.platform);
      }
      platforms = next;
      _notify();
      await refreshStats();
    } catch (_) {
      await silentReload();
    }
  }

  /// 单平台就地打补丁（启停这类只改一行的写操作用），不发任何请求。
  void patchPlatform(PlatformRow updated) {
    platforms = [
      for (final p in platforms) p.id == updated.id ? updated : p,
    ];
    List<GroupDetail> apply(List<GroupDetail> list) => [
      for (final d in list)
        if (!d.hasPlatform(updated.id))
          d
        else
          d.copyWith(
            platforms: [
              for (final gp in d.platforms)
                gp.platform.id == updated.id
                    ? gp.copyWith(platform: updated)
                    : gp,
            ],
          ),
    ];
    _loadedDetails = apply(_loadedDetails);
    details = apply(details);
    _notify();
  }

  // ── 新建分组 ────────────────────────────────────────────────────

  void openCreate() {
    showCreate = true;
    _notify();
  }

  /// `Groups.tsx:735`：关闭时把四个字段清回初值。
  /// 注意 mode 清回的是 `failover`，**不是**打开时的 `health_aware` —— React 就是
  /// 这么写的（`:576` 与 `:735` 两处都清成 failover），照搬，不「修正」。
  void closeCreate() {
    createName = '';
    createGroupKey = '';
    createMode = 'failover';
    createPlatformIds = const [];
    showCreate = false;
    _notify();
  }

  void setCreateName(String v) {
    createName = v;
    _notify();
  }

  /// 输入即过滤非法字符（见 [sanitizeGroupKey]）。
  void setCreateGroupKey(String v) {
    createGroupKey = sanitizeGroupKey(v);
    _notify();
  }

  void setCreateMode(String v) {
    createMode = v;
    _notify();
  }

  void setCreatePlatformIds(List<int> v) {
    createPlatformIds = v;
    _notify();
  }

  /// 「创建」按钮的启用判据：`GroupCreateModal.tsx:55` 的 `disabled={!cName}`。
  bool get canCreate => createName.isNotEmpty;

  /// 新建表单里可选的平台：`GroupCreateModal.tsx:33` 只列 **enabled** 的。
  List<PlatformRow> get createPlatformOptions =>
      [for (final p in platforms) if (p.enabled) p];

  /// `Groups.tsx:567-583`。密钥留空 → 不传（后端自动生成 `gk_<32hex>`）。
  /// 选了平台的话，建完再关联一次（两条命令，不是一条）。
  Future<void> createGroup({String failText = '创建分组失败'}) async {
    if (!canCreate) return;
    try {
      final v = await _invoke('group_create', {
        'input': {
          'name': createName,
          if (createGroupKey.trim().isNotEmpty) 'group_key': createGroupKey.trim(),
          'routing_mode': createMode,
        },
      });
      final gid = ((v as Map?)?['id'] as num?)?.toInt() ?? 0;
      if (createPlatformIds.isNotEmpty && gid > 0) {
        await _invoke('group_set_platforms', {
          'groupId': gid,
          'platforms': _priorityPayload(createPlatformIds),
        });
      }
      closeCreate();
      await silentReload();
    } catch (e) {
      _toast('$failText: $e', ok: false);
    }
  }

  /// `Groups.tsx:552`：优先级 = 列表顺序（从 1 起），权重固定 1。
  List<Map<String, Object?>> _priorityPayload(List<int> ids) => [
    for (var i = 0; i < ids.length; i++)
      {'platform_id': ids[i], 'priority': i + 1, 'weight': 1},
  ];

  // ── 编辑分组 ────────────────────────────────────────────────────

  void openEdit(GroupDetail detail) {
    edit = GroupEditState.open(detail);
    _notify();
  }

  void cancelEdit() {
    edit = GroupEditState.empty;
    _notify();
  }

  void patchEdit(GroupEditState next) {
    edit = next;
    _notify();
  }

  /// `Groups.tsx:535-564`：先更新组本体（含内联模型映射 + 环境变量），再重设平台集。
  /// 成功后只刷这一个组（`refreshSingleGroup`），不整列表重载。
  Future<void> saveEdit({String failText = '保存分组失败'}) async {
    final target = edit.target;
    if (target == null || !edit.canSave) return;
    final gid = target.group.id;
    try {
      await _invoke('group_update', {
        'input': {
          'id': gid,
          'name': edit.name,
          'routing_mode': edit.mode,
          'request_timeout_secs': edit.reqTimeout,
          'connect_timeout_secs': edit.connTimeout,
          'max_retries': edit.maxRetries,
          'model_mappings': [for (final m in edit.mappings) m.toJson()],
          'env_vars': [for (final e in edit.envVars) e.toJson()],
        },
      });
      await _invoke('group_set_platforms', {
        'groupId': gid,
        'platforms': _priorityPayload(edit.platformIds),
      });
      cancelEdit();
      await refreshSingleGroup(gid);
    } catch (e) {
      _toast('$failText: $e', ok: false);
    }
  }

  // ── 组本身的增删改 ──────────────────────────────────────────────

  /// 删组是破坏性的：先开确认态，确认之后才走 [confirmDeleteGroup]。
  void askDeleteGroup(int gid) {
    deleteGroupTarget = gid;
    _notify();
  }

  void cancelDeleteGroup() {
    deleteGroupTarget = null;
    _notify();
  }

  Future<void> confirmDeleteGroup({String failText = '删除分组失败'}) async {
    final gid = deleteGroupTarget;
    if (gid == null) return;
    deleteGroupTarget = null;
    _notify();
    try {
      await _invoke('group_delete', {'id': gid});
      await silentReload();
    } catch (e) {
      _toast('$failText: $e', ok: false);
    }
  }

  /// `Groups.tsx:598-610`：默认组是单选，再点一次就取消（传 null）。
  Future<void> toggleDefault(GroupRow group, {String failText = '设置默认分组失败'}) async {
    try {
      await _invoke('group_set_default', {
        'id': group.isDefault ? null : group.id,
      });
      await silentReload();
    } catch (e) {
      _toast('$failText: $e', ok: false);
    }
  }

  /// 列表拖拽排序。`Groups.tsx:519-524`：**搜索态下 no-op** —— 搜索是临时视图，
  /// 此时重排会把没命中的组挤掉。调用方把当前搜索词传进来。
  Future<void> reorderGroups(List<GroupDetail> next, {String searchQuery = ''}) async {
    if (searchQuery.trim().isNotEmpty) return;
    details = next;
    _notify();
    try {
      await _invoke('group_reorder', {
        'orderedIds': [for (final d in next) d.group.id],
      });
    } catch (_) {
      /* React: .catch(console.error) */
    }
  }

  /// 组内平台排序（`usePlatformDrag.ts:87`）。
  Future<void> reorderPlatformsInGroup(int gid, List<int> orderedIds) async {
    try {
      await _invoke('group_platform_reorder', {
        'groupId': gid,
        'orderedIds': orderedIds,
      });
    } catch (_) {
      /* React: .catch(console.error) */
    }
  }

  /// 把平台从一个组挪到另一个组（`usePlatformDrag.ts:102/136`；
  /// `fromGid = 0` 表示来自「未分组」区）。
  Future<void> movePlatform(int pid, int fromGid, int toGid) async {
    try {
      await _invoke('group_platform_move', {
        'platformId': pid,
        'fromGroupId': fromGid,
        'toGroupId': toGid,
      });
      await silentReload();
    } catch (e) {
      _toast('$e', ok: false);
    }
  }

  /// per-group 优先级就地编辑：乐观改本地 → 写后端 → 失败回滚到原值（缺省 5）。
  /// `Groups.tsx:482-507`。
  Future<void> setLevelPriority(
    int gid,
    int pid,
    int next, {
    String failText = '优先级保存失败',
  }) async {
    // 先记下原值（回滚要用），再乐观改。React 是在 setState updater 里顺手赋值的，
    // 那个写法依赖 React 同步执行 updater —— 这里分两步，不依赖调度时序。
    int? prevValue;
    for (final d in details) {
      if (d.group.id != gid) continue;
      for (final gp in d.platforms) {
        if (gp.platform.id == pid) prevValue = gp.levelPriority;
      }
    }
    List<GroupDetail> apply(int value) => [
      for (final d in details)
        if (d.group.id != gid)
          d
        else
          d.copyWith(
            platforms: [
              for (final gp in d.platforms)
                gp.platform.id == pid ? gp.copyWith(levelPriority: value) : gp,
            ],
          ),
    ];
    details = apply(next);
    _notify();
    try {
      await _invoke('group_platform_set_level_priority', {
        'groupId': gid,
        'platformId': pid,
        'levelPriority': next,
      });
    } catch (e) {
      _toast('$failText: $e', ok: false);
      // 回滚到原值；原值取不到就用 schema 默认 5（React `:503` 的 `?? 5`）。
      details = apply(prevValue ?? 5);
      _notify();
    }
  }

  // ── 模型映射（列表页的快捷添加 / 删除）────────────────────────────

  /// `Groups.tsx:613-642`。四个字段缺一不加（与 React 的早退判据逐条一致）。
  ///
  /// **`env_vars` 必须一并透传**：后端 `UpdateGroup.env_vars` 是 `#[serde(default)]`
  /// 的 `Vec`（不是 `Option`），不传就是空数组，会把用户已配的环境变量清光。
  /// React 在 `:629-631` 用三行注释标了这个坑，这里同样处理。
  Future<void> addMapping({
    required int groupId,
    required String sourceModel,
    required int targetPlatformId,
    required String targetModel,
    String failText = '添加映射失败',
  }) async {
    if (sourceModel.isEmpty || targetModel.isEmpty) return;
    GroupDetail? detail;
    for (final d in details) {
      if (d.group.id == groupId) detail = d;
    }
    if (detail == null) return;
    try {
      final next = [
        ...detail.modelMappings,
        ModelMapping(
          sourceModel: sourceModel,
          targetPlatformId: targetPlatformId,
          targetModel: targetModel,
          requestTimeoutSecs: 0,
          connectTimeoutSecs: 0,
        ),
      ];
      await _invoke('group_update', {
        'input': {
          'id': groupId,
          'model_mappings': [for (final m in next) m.toJson()],
          'env_vars': [for (final e in detail.group.envVars) e.toJson()],
        },
      });
      await refreshSingleGroup(groupId);
    } catch (e) {
      _toast('$failText: $e', ok: false);
    }
  }

  /// `Groups.tsx:660-674`。同样要透传 `env_vars`。
  Future<void> deleteMapping(
    int groupId,
    int index, {
    String failText = '删除映射失败',
  }) async {
    GroupDetail? detail;
    for (final d in details) {
      if (d.group.id == groupId) detail = d;
    }
    if (detail == null) return;
    try {
      final next = [
        for (var i = 0; i < detail.modelMappings.length; i++)
          if (i != index) detail.modelMappings[i],
      ];
      await _invoke('group_update', {
        'input': {
          'id': groupId,
          'model_mappings': [for (final m in next) m.toJson()],
          'env_vars': [for (final e in detail.group.envVars) e.toJson()],
        },
      });
      await refreshSingleGroup(groupId);
    } catch (e) {
      _toast('$failText: $e', ok: false);
    }
  }

  // ── 移除平台（删 vs 移出本组）────────────────────────────────────

  /// 点「移除」：**先实时拉后端**算该平台的跨组归属，再开弹窗。
  /// 拉失败才退回前端 `details`（可能 stale，但至少弹窗还能出来）。
  Future<void> askRemovePlatform(PlatformRow p, int gid) async {
    int groupCount;
    List<String> groupNames;
    try {
      final v = await _invoke('group_detail_list');
      final fresh = [
        for (final e in (v as List? ?? const []))
          GroupDetail.fromJson(e as Map<String, dynamic>),
      ];
      final hit = [for (final d in fresh) if (d.hasPlatform(p.id)) d];
      groupCount = hit.length;
      groupNames = [for (final d in hit) d.group.name];
    } catch (_) {
      final hit = [for (final d in details) if (d.hasPlatform(p.id)) d];
      groupCount = hit.length;
      groupNames = [for (final d in hit) d.group.name];
    }
    removeTarget = RemovePlatformTarget(
      platform: p,
      groupId: gid,
      groupCount: groupCount,
      groupNames: groupNames,
    );
    _notify();
  }

  void cancelRemovePlatform() {
    removeTarget = null;
    _notify();
  }

  /// 弹窗里选「仅移出本组」：用 `group_set_platforms` 重设本组平台集（去掉这一个），
  /// 平台行本身不动、其他组不动。`Groups.tsx:184-198`。
  Future<void> removePlatformFromGroup({String failText = '移出分组失败'}) async {
    final t = removeTarget;
    if (t == null) return;
    GroupDetail? detail;
    for (final d in details) {
      if (d.group.id == t.groupId) detail = d;
    }
    if (detail == null) return;
    final remaining = [
      for (final gp in detail.platforms)
        if (gp.platform.id != t.platform.id) gp,
    ];
    removeTarget = null;
    _notify();
    try {
      await _invoke('group_set_platforms', {
        'groupId': t.groupId,
        'platforms': [
          for (var i = 0; i < remaining.length; i++)
            {
              'platform_id': remaining[i].platform.id,
              'priority': i + 1,
              'weight': remaining[i].weight,
            },
        ],
      });
      await silentReload();
    } catch (e) {
      _toast('$failText: $e', ok: false);
    }
  }

  /// 弹窗里选「删除平台」：真删（后端会连带清 group_platform 关联）。
  /// 失败时**保持弹窗上下文不刷新**，只 toast —— 与 React `:236-241` 一致。
  Future<void> confirmDeletePlatform({String failText = '删除失败'}) async {
    final t = removeTarget;
    if (t == null) return;
    try {
      await _invoke('platform_delete', {'id': t.platform.id});
      removeTarget = null;
      _notify();
      await silentReload();
    } catch (e) {
      _toast('$failText: $e', ok: false);
      removeTarget = null;
      _notify();
    }
  }

  // ── 批量操作（四个，各自一个确认弹窗）────────────────────────────

  /// 工具栏「删除」：解析选中平台 + 实时拉跨组关系 → 开弹窗。
  Future<void> askBatchDelete(List<int> ids) async {
    final selected = [
      for (final id in ids)
        for (final p in platforms)
          if (p.id == id) p,
    ];
    if (selected.isEmpty) return;
    final byPlatform = <int, List<String>>{};
    try {
      final v = await _invoke('group_detail_list');
      final fresh = [
        for (final e in (v as List? ?? const []))
          GroupDetail.fromJson(e as Map<String, dynamic>),
      ];
      for (final p in selected) {
        byPlatform[p.id] = [
          for (final d in fresh)
            if (d.hasPlatform(p.id)) d.group.name,
        ];
      }
    } catch (_) {
      for (final p in selected) {
        byPlatform[p.id] = [
          for (final d in details)
            if (d.hasPlatform(p.id)) d.group.name,
        ];
      }
    }
    batchDeleteTarget = BatchDeleteTarget(
      platforms: selected,
      groupNamesByPlatform: byPlatform,
    );
    _notify();
  }

  void cancelBatchDelete() {
    batchDeleteTarget = null;
    _notify();
  }

  Future<void> confirmBatchDelete({
    String Function(int count)? doneText,
    String failText = '批量删除失败',
  }) async {
    final t = batchDeleteTarget;
    if (t == null) return;
    batchDeleteBusy = true;
    _notify();
    try {
      final v = await _invoke('batch_delete_platforms', {
        'ids': [for (final p in t.platforms) p.id],
      });
      final report = BatchReport.fromJson((v as Map?)?.cast<String, dynamic>() ?? const {});
      batchDeleteTarget = null;
      await silentReload();
      _toast((doneText ?? (n) => '已删除 $n 个平台')(report.applied));
    } catch (e) {
      _toast('$failText: $e', ok: false);
      batchDeleteTarget = null;
    } finally {
      batchDeleteBusy = false;
      _notify();
    }
  }

  void askBatchOverrideModels(List<int> ids) {
    final selected = [
      for (final id in ids)
        for (final p in platforms)
          if (p.id == id) p,
    ];
    if (selected.isEmpty) return;
    batchOverrideTarget = selected;
    _notify();
  }

  void cancelBatchOverrideModels() {
    batchOverrideTarget = null;
    _notify();
  }

  /// 整体覆盖五个模型槽（不是合并）。`Groups.tsx:328-351`。
  Future<void> confirmBatchOverrideModels(
    PlatformModels models, {
    String Function(int count)? doneText,
    String failText = '批量覆盖模型失败',
  }) async {
    final t = batchOverrideTarget;
    if (t == null) return;
    batchOverrideBusy = true;
    _notify();
    try {
      final v = await _invoke('batch_override_models', {
        'ids': [for (final p in t) p.id],
        'models': models.toJson(),
      });
      final report = BatchReport.fromJson((v as Map?)?.cast<String, dynamic>() ?? const {});
      batchOverrideTarget = null;
      await silentReload();
      _toast((doneText ?? (n) => '已覆盖 $n 个平台的模型')(report.applied));
    } catch (e) {
      _toast('$failText: $e', ok: false);
      batchOverrideTarget = null;
    } finally {
      batchOverrideBusy = false;
      _notify();
    }
  }

  /// 工具栏「改状态」：同时算出本组当前 enabled 的平台 id —— 弹窗要用它警告
  /// 「全禁用后这个组就没有可用平台了」。`Groups.tsx:363-375`。
  void askBatchSetStatus(List<int> ids, int gid) {
    final selected = [
      for (final id in ids)
        for (final p in platforms)
          if (p.id == id) p,
    ];
    if (selected.isEmpty) return;
    GroupDetail? detail;
    for (final d in details) {
      if (d.group.id == gid) detail = d;
    }
    batchSetStatusGroupEnabledIds = [
      for (final gp in detail?.platforms ?? const <GroupPlatform>[])
        if (gp.platform.status == 'enabled') gp.platform.id,
    ];
    batchSetStatusTarget = (platforms: selected, groupId: gid);
    _notify();
  }

  void cancelBatchSetStatus() {
    batchSetStatusTarget = null;
    _notify();
  }

  Future<void> confirmBatchSetStatus(
    String status, {
    String Function(int count)? doneText,
    String failText = '批量改状态失败',
  }) async {
    final t = batchSetStatusTarget;
    if (t == null) return;
    batchSetStatusBusy = true;
    _notify();
    try {
      final v = await _invoke('batch_set_status', {
        'ids': [for (final p in t.platforms) p.id],
        'status': status,
      });
      final report = BatchReport.fromJson((v as Map?)?.cast<String, dynamic>() ?? const {});
      batchSetStatusTarget = null;
      await silentReload();
      _toast((doneText ?? (n) => '已改 $n 个平台状态')(report.applied));
    } catch (e) {
      _toast('$failText: $e', ok: false);
      batchSetStatusTarget = null;
    } finally {
      batchSetStatusBusy = false;
      _notify();
    }
  }

  void askBatchMoveGroup(List<int> ids, int gid) {
    final selected = [
      for (final id in ids)
        for (final p in platforms)
          if (p.id == id) p,
    ];
    if (selected.isEmpty) return;
    batchMoveGroupTarget = (platforms: selected, groupId: gid);
    _notify();
  }

  void cancelBatchMoveGroup() {
    batchMoveGroupTarget = null;
    _notify();
  }

  /// [mode] 只取 `move`（挪走）或 `add`（加入但保留原组）。`Groups.tsx:423-449`。
  Future<void> confirmBatchMoveGroup(
    int targetGroupId,
    String mode, {
    String Function(int count, String mode)? doneText,
    String failText = '批量移组失败',
  }) async {
    final t = batchMoveGroupTarget;
    if (t == null) return;
    batchMoveGroupBusy = true;
    _notify();
    try {
      final v = await _invoke('batch_move_group', {
        'ids': [for (final p in t.platforms) p.id],
        'targetGroupId': targetGroupId,
        'mode': mode,
      });
      final report = BatchReport.fromJson((v as Map?)?.cast<String, dynamic>() ?? const {});
      batchMoveGroupTarget = null;
      await silentReload();
      _toast(
        (doneText ?? (n, m) => '已${m == 'move' ? '移动' : '加入'} $n 个平台')(
          report.applied,
          mode,
        ),
      );
    } catch (e) {
      _toast('$failText: $e', ok: false);
      batchMoveGroupTarget = null;
    } finally {
      batchMoveGroupBusy = false;
      _notify();
    }
  }

  // ── 清理失效平台 ────────────────────────────────────────────────

  /// 先只读预览会被处理的候选（`GroupListItem.tsx:149`），给用户看清单再确认。
  /// [groupId] 为空 = 全局清理；非空 = 只清这个组。
  Future<void> askPurgeDisabled(int? groupId) async {
    try {
      final v = await _invoke('platform_purge_disabled_preview', {
        'groupId': groupId,
      });
      purgeTarget = (
        groupId: groupId,
        candidates: [
          for (final e in (v as List? ?? const []))
            (e as Map).cast<String, Object?>(),
        ],
      );
      _notify();
    } catch (_) {
      purgeTarget = (groupId: groupId, candidates: const []);
      _notify();
    }
  }

  void cancelPurgeDisabled() {
    purgeTarget = null;
    _notify();
  }

  /// `Groups.tsx:644-658`。返回里两个数组含义不同：`deletedIds` 是真删掉的，
  /// `unassignedIds` 是只解除了本组关联、平台行还在的（组级清理才可能出现）。
  Future<void> confirmPurgeDisabled({
    String noneText = '暂无失效平台',
    String Function(int deleted, int unassigned)? doneText,
    String failText = '清理失效',
  }) async {
    final t = purgeTarget;
    if (t == null) return;
    purgeTarget = null;
    _notify();
    try {
      final v = await _invoke('platform_purge_disabled', {'groupId': t.groupId});
      final r = PurgeReport.fromJson((v as Map?)?.cast<String, dynamic>() ?? const {});
      if (r.deletedIds.isEmpty && r.unassignedIds.isEmpty) {
        _toast(noneText);
      } else {
        _toast(
          (doneText ?? (d, u) => '已清理：删除 $d，移除 $u')(
            r.deletedIds.length,
            r.unassignedIds.length,
          ),
        );
      }
      await silentReload();
    } catch (e) {
      _toast('$failText: $e', ok: false);
    }
  }

  // ── 一键测试本组 ────────────────────────────────────────────────

  /// `useGroupTest.ts:21-69`。
  ///
  /// 两条容易搬丢的规则：
  /// ① **只测 enabled 的平台** —— disabled / auto_disabled 既不出现在结果行里，
  ///    也不发测试请求；一个 enabled 都没有就整个不弹面板（早退）。
  /// ② 有界并发（[kBatchTestConcurrency]），共享游标各自领任务，不是一次全发。
  Future<void> testGroup(
    GroupRow group,
    List<GroupPlatform> gps, {
    String failText = '测试失败',
  }) async {
    final enabled = [
      for (final gp in gps)
        if (gp.platform.status == 'enabled') gp,
    ];
    if (enabled.isEmpty) return;
    final rows = [
      for (final gp in enabled)
        GroupTestRow(platformId: gp.platform.id, name: gp.platform.name),
    ];
    groupTest = GroupTestState(
      groupId: group.id,
      groupName: group.name,
      rows: rows,
    );
    _notify();

    /// 面板被中途关掉、或被新一轮测试取代时，在途的写回是 no-op（不复活面板）。
    bool stillMine() => groupTest?.groupId == group.id;

    Future<void> testOne(int idx) async {
      final gp = enabled[idx];
      if (!stillMine()) return;
      rows[idx].status = 'testing';
      _notify();
      final model =
          gp.platform.models.defaultModel?.isNotEmpty == true
          ? gp.platform.models.defaultModel!
          : (gp.platform.availableModels.isNotEmpty
                ? gp.platform.availableModels.first
                : '');
      final start = DateTime.now().millisecondsSinceEpoch;
      try {
        final v = await _invoke('model_test', {
          'req': {'platform_id': gp.platform.id, 'model': model},
        });
        final r = ModelTestResult.fromJson(
          (v as Map?)?.cast<String, dynamic>() ?? const {},
        );
        if (!stillMine()) return;
        rows[idx]
          ..status = r.success ? 'ok' : 'fail'
          ..durationMs = DateTime.now().millisecondsSinceEpoch - start
          ..error = r.success ? null : (r.error.isEmpty ? failText : r.error);
      } catch (e) {
        if (!stillMine()) return;
        rows[idx]
          ..status = 'fail'
          ..durationMs = DateTime.now().millisecondsSinceEpoch - start
          ..error = '$e';
      }
      _notify();
    }

    var next = 0;
    Future<void> worker() async {
      while (next < enabled.length) {
        final idx = next++;
        await testOne(idx);
      }
    }

    final poolSize = enabled.length < kBatchTestConcurrency
        ? enabled.length
        : kBatchTestConcurrency;
    await Future.wait([for (var i = 0; i < poolSize; i++) worker()]);
    if (stillMine()) {
      groupTest!.running = false;
      _notify();
    }
  }

  void closeGroupTest() {
    groupTest = null;
    _notify();
  }

  // ── 折叠态（跨会话持久化到 group.extra._ui_collapsed）────────────

  /// `Groups.tsx:140-153`：乐观翻转本地态，300ms 之后才写库（连点只写末次）。
  /// 这里把「写库」交给调用方的计时器，保持本类无 Timer（测试不必等真时钟）。
  bool toggleGroupCollapsed(int gid) {
    final next = !collapsedGroups.contains(gid);
    final s = {...collapsedGroups};
    if (next) {
      s.add(gid);
    } else {
      s.remove(gid);
    }
    collapsedGroups = s;
    _notify();
    return next;
  }

  /// mount 首次拿到 details 后从 `group.extra._ui_collapsed` 回灌折叠态。
  /// **只灌一次**（`Groups.tsx:129-139` 的 `collapseInitRef`）—— 之后每次刷新都灌
  /// 会把用户刚点的展开又收回去。`extra` 不是合法 JSON 就当未折叠。
  void hydrateCollapsedFrom(List<GroupDetail> ds) {
    final ids = <int>{};
    for (final d in ds) {
      try {
        final e = d.group.extra.isEmpty ? {} : jsonDecode(d.group.extra);
        if (e is Map && e['_ui_collapsed'] == true) ids.add(d.group.id);
      } catch (_) {
        /* extra 非法 JSON → 视作未折叠 */
      }
    }
    if (ids.isNotEmpty) {
      collapsedGroups = ids;
      _notify();
    }
  }

  /// 落盘折叠态（debounce 之后调）。`set_ui_extra` 的四参形状见 `ui_extra.ts:27`。
  Future<void> persistGroupCollapsed(int gid, bool collapsed) async {
    try {
      await _invoke('set_ui_extra', {
        'target': 'group',
        'id': gid,
        'key': '_ui_collapsed',
        'value': collapsed,
      });
    } catch (_) {
      /* React: .catch(console.error) */
    }
  }
}

/// 从 `get_defaults_json` 的整份文档里抠出「协议 → 跨语言搜索词」。
/// 与 `src/domains/platforms/defaults.ts::getProtocolSearchTermsMap` 同口径：
/// 各 locale 的 `name` 值 + `keywords` 全收，去重。
Map<String, List<String>> parseProtocolSearchTerms(String rawJson) {
  if (rawJson.isEmpty) return const {};
  final Object? doc;
  try {
    doc = jsonDecode(rawJson);
  } catch (_) {
    return const {};
  }
  final protocols = (doc as Map?)?['protocols'];
  if (protocols is! Map) return const {};
  final out = <String, List<String>>{};
  protocols.forEach((code, entry) {
    if (entry is! Map) return;
    final terms = <String>{};
    final name = entry['name'];
    if (name is Map) {
      for (final v in name.values) {
        if (v is String && v.isNotEmpty) terms.add(v);
      }
    }
    final kw = entry['keywords'];
    if (kw is List) {
      for (final v in kw) {
        if (v is String && v.isNotEmpty) terms.add(v);
      }
    }
    if (terms.isNotEmpty) out['$code'] = terms.toList();
  });
  return out;
}
