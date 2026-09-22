/// 平台页逻辑层（票 I07），对应 `src/pages/Platforms.tsx` 与 `src/pages/platforms/`
/// （`usePlatformsState` 769 行 + `usePlatformForm` 780 行 + `usePlatformQuota` 179 行）
/// 加上 `src/components/platforms/usePlatformCards.ts`。
///
/// 这一页的难点不在字段多，在**「乐观更新 + 慢响应晚到」的竞争**。React 侧用一个
/// `platformsEpochRef` 计数器解决：每次本地乐观写（保存 / 删除 / 清理）自增 epoch，
/// 在途的 `load()` / `refreshStats()` 回来时若 epoch 变了就**放弃整列表覆盖**。
/// 不这么做的症状是「删掉的平台过两秒自己回来了」。这里 [epoch] 逐条照搬。
library;

import 'dart:async';

import '../utils/pinyin.dart';
import 'groups_logic.dart' show parseProtocolSearchTerms;
import 'invoke.dart';
import 'models.dart';
import 'platform_card_bits.dart';

/// quota 查询的并发上限（`src/domains/platforms` 的 `QUOTA_CONCURRENCY`）。
/// 这些是真出网的 HTTP，不是本地查询，所以上限比列表渲染那类要低。
const int kQuotaConcurrency = 4;

/// `usePlatformQuota.ts:13::getPrimaryBaseUrl`：从 endpoints 推主 base_url ——
/// 先找协议与平台主协议相同的那条，没有就取第一条，再没有就空串。
String getPrimaryBaseUrl(String protocol, List<PlatformEndpoint> eps) {
  if (eps.isEmpty) return '';
  for (final ep in eps) {
    if (ep.protocol == protocol) return ep.baseUrl;
  }
  return eps.first.baseUrl;
}

/// `usePlatformForm.ts:239`：Claude Code 订阅纯透传（客户端自带 OAuth，原样转发）。
bool isPassthroughProtocol(String protocol) => protocol == 'claude_code';

/// `usePlatformForm.ts:241`：OpenCode Zen 免费匿名访问，key 留空由后端兜底。
bool isKeyOptionalProtocol(String protocol) => protocol == 'opencode_zen';

/// `usePlatformForm.ts:243`：要 key 但没填 —— 「获取模型」按钮的禁用判据。
bool apiKeyMissing(String protocol, String apiKey) =>
    !isKeyOptionalProtocol(protocol) && apiKey.isEmpty;

/// `usePlatformsState.ts:321::buildMembership`：platform id → 它所属的组名列表。
/// 未出现在任何组里的 id **不进这个 map** —— 「未分组」的判据就是 `!containsKey`。
Map<int, List<String>> buildMembership(List<GroupDetail> gds) {
  final m = <int, List<String>>{};
  for (final g in gds) {
    for (final gp in g.platforms) {
      (m[gp.platform.id] ??= []).add(g.group.name);
    }
  }
  return m;
}

/// `usePlatformForm.ts:554::buildModelsPayload`：五个槽位逐个 trim，
/// 全空返回 null（= 不带 models 字段），有一个非空就整份带上（空槽写 null）。
Map<String, Object?>? buildModelsPayload(Map<String, String> models) {
  const slots = ['default', 'sonnet', 'opus', 'haiku', 'gpt'];
  final result = <String, Object?>{};
  var hasAny = false;
  for (final slot in slots) {
    final v = (models[slot] ?? '').trim();
    if (v.isNotEmpty) {
      result[slot] = v;
      hasAny = true;
    } else {
      result[slot] = null;
    }
  }
  return hasAny ? result : null;
}

/// 平台页控制器。
class PlatformsController {
  PlatformsController({InvokeFn? invoke, this.onChanged, this.onToast})
    : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;
  final void Function(String text, {required bool ok})? onToast;

  List<PlatformRow> platforms = const [];
  List<GroupDetail> groupDetails = const [];
  Map<int, List<String>> membership = const {};
  Map<int, UsageStats> usageMap = const {};
  Map<int, LastTestResult> lastTestMap = const {};
  Map<int, PlatformQuota> quotaMap = const {};
  Map<int, bool> quotaRefreshing = const {};
  Map<int, bool> quotaPending = const {};

  /// 手动刷新校准过的平台 id（`usePlatformQuota.ts::quotaRealIds`）：
  /// 命中的走真查值而非预估值（`computeQuotaDisplay` 的 `preferRealCalibrated`）。
  Map<int, bool> quotaRealIds = const {};
  Map<int, String> testResults = const {};
  Map<String, List<String>> protocolTerms = const {};

  /// 全局熔断默认，编辑表单用它显示「继承默认 N」。拉不到就是 null（表单退到
  /// 「继承默认」这句不带数字的 placeholder）。
  BreakerDefaults? breakerDefaults;

  /// registry 派生的协议元数据（label / 外链 / coding plan / 配额脚本 / 默认模型 / peak）。
  ProtocolMetaTable protocolMeta = const ProtocolMetaTable();

  /// 展开明细的平台 id（跨会话落盘，键 `_ui_expand_plat`）。
  Set<int> expandedIds = <int>{};

  /// 平台 logo 的可渲染来源（data URL / 本地路径）；未命中缓存的不入表。
  Map<String, String> protocolLogos = const {};
  final Set<String> _logoAsked = <String>{};

  bool loading = true;
  bool usageLoading = false;
  int? testingId;
  String searchQuery = '';

  /// 清理失效平台：先预览再确认（与分组页同一形态）。
  List<PurgeCandidate>? purgeCandidates;

  /// 预览拉取中。弹窗**先开再拉**，拉取期间显示 `status.loading`
  /// （`PlatformListView.tsx:285-286`）——原先是拉完才开弹窗，点了按钮没反应。
  bool purgePreviewLoading = false;

  /// 执行中。弹窗留在原地、两颗按钮禁掉、确认按钮文案换成「处理中…」
  /// （`PlatformListView.tsx:277,305-307`）。
  bool purging = false;

  /// 删除平台的确认态。React 的列表卡直接调 `handleDelete`（删除按钮自带
  /// AlertDialog），这里把「待删的是谁」显式记下来，让确认与执行分成两步。
  int? deleteTarget;

  /// 乐观写的代次。见文件抬头。
  int epoch = 0;

  void _notify() => onChanged?.call();
  void _toast(String text, {bool ok = true}) => onToast?.call(text, ok: ok);

  /// 让表单控制器也能发提示（批量创建的进度 / 汇总行）。
  void toast(String text, {bool ok = true}) => _toast(text, ok: ok);

  /// 未分组平台（主列表只展示这些；已分组的在分组卡里，避免重复），再套搜索过滤。
  /// `usePlatformsState.ts:620-629`。
  List<PlatformRow> get standalonePlatforms {
    final q = searchQuery.trim();
    return [
      for (final p in platforms)
        if (!membership.containsKey(p.id))
          if (q.isEmpty || _matches(p, q)) p,
    ];
  }

  /// `usePlatformsState.ts:620-629` 的口径：name / base_url / platform_type 走
  /// 拼音模糊（与 `query.ts::platformMatchesQuery` 同链），registry 词条纯子串。
  bool _matches(PlatformRow p, String q) {
    final needle = q.trim();
    if (needle.isEmpty) return true;
    if (pinyinMatch(needle, p.name)) return true;
    if (pinyinMatch(needle, p.baseUrl)) return true;
    if (pinyinMatch(needle, p.platformType)) return true;
    final lower = needle.toLowerCase();
    final terms = protocolTerms[p.platformType];
    return terms != null && terms.any((t) => t.toLowerCase().contains(lower));
  }

  /// `usePlatformsState.ts:631`。
  int get enabledCount => platforms.where((p) => p.enabled).length;

  void setSearchQuery(String q) {
    searchQuery = q;
    _notify();
  }

  // ── 加载 ────────────────────────────────────────────────────────

  Future<void> init() async {
    await Future.wait<void>([
      load(),
      _loadGroupDetails(),
      _loadBreakerDefaults(),
      _loadProtocolTerms(),
    ]);
  }

  /// `usePlatformsState.ts:385-429`。
  ///
  /// 注意顺序：平台列表到手**立刻**渲染（`loading = false`），余额与用量改后台
  /// 渐进填充。外部 quota 查询是走网络的，让它挡住整页首屏没有道理。
  Future<void> load() async {
    loading = true;
    _notify();
    final captured = epoch;
    List<PlatformRow> list = const [];
    try {
      final v = await _invoke('platform_list');
      list = [
        for (final e in (v as List? ?? const []))
          PlatformRow.fromJson(e as Map<String, dynamic>),
      ];
    } catch (_) {
      /* React: console.error(e) */
    }
    // 在途期间发生过本地乐观写 → 放弃整列表覆盖，避免晚到的响应把结果顶回去。
    if (captured != epoch) {
      loading = false;
      _notify();
      return;
    }
    platforms = list;
    quotaPending = {
      for (final p in list)
        if (platformWantsQuota(p)) p.id: true,
    };
    loading = false;
    _notify();

    usageLoading = true;
    _notify();
    try {
      final v = await _invoke('all_platform_usage_stats');
      final m = (v as Map?)?.cast<String, dynamic>() ?? const <String, dynamic>{};
      usageMap = {
        for (final e in m.entries)
          if (int.tryParse(e.key) != null && e.value is Map)
            int.parse(e.key): UsageStats.fromJson(
              (e.value as Map).cast<String, dynamic>(),
            ),
      };
    } catch (_) {
      /* ignore */
    } finally {
      usageLoading = false;
      _notify();
    }

    await _loadLastTests(list);
  }

  /// 「最近一次测试」徽章：每平台一条，**有值才填**（null 不填 = 不渲染徽章）。
  /// `usePlatformsState.ts:420-428`。
  Future<void> _loadLastTests(List<PlatformRow> list) async {
    final next = <int, LastTestResult>{};
    await Future.wait([
      for (final p in list)
        () async {
          try {
            final v = await _invoke('get_last_test_result', {'platformId': p.id});
            if (v != null) {
              next[p.id] = LastTestResult.fromJson(
                (v as Map).cast<String, dynamic>(),
              );
            }
          } catch (_) {
            /* React: .catch(() => null) */
          }
        }(),
    ]);
    lastTestMap = next;
    _notify();
  }

  Future<void> refreshLastTest(int platformId) async {
    try {
      final v = await _invoke('get_last_test_result', {'platformId': platformId});
      final next = {...lastTestMap};
      if (v != null) {
        next[platformId] = LastTestResult.fromJson(
          (v as Map).cast<String, dynamic>(),
        );
      } else {
        next.remove(platformId);
      }
      lastTestMap = next;
      _notify();
    } catch (_) {
      /* ignore */
    }
  }

  Future<void> _loadGroupDetails() async {
    try {
      final v = await _invoke('group_detail_list');
      groupDetails = [
        for (final e in (v as List? ?? const []))
          GroupDetail.fromJson(e as Map<String, dynamic>),
      ];
      membership = buildMembership(groupDetails);
      _notify();
    } catch (_) {
      /* React: .catch(() => {})，拉不到不挡编辑 */
    }
  }

  /// 全局熔断默认值，用来在表单里显示「继承默认 N」。读失败不挡编辑。
  /// `usePlatformsState.ts:708-716`。
  Future<void> _loadBreakerDefaults() async {
    try {
      final v = await _invoke('scheduling_settings_get');
      if (v is Map) {
        breakerDefaults = BreakerDefaults.fromJson(v.cast<String, dynamic>());
      }
      _notify();
    } catch (_) {
      /* React: console.error(...)，不挡编辑 */
    }
  }

  /// `get_defaults_json` 一次拉全：搜索词 + 协议元数据（label / 外链 / coding plan /
  /// 配额脚本索引 / 默认模型 / preset peak）。React 那边是 `useProtocolMeta` 每卡
  /// 一次 `Promise.all`（共享 docPromise 缓存），这里整份文档只解析一次。
  Future<void> _loadProtocolTerms() async {
    try {
      final raw = await _invoke('get_defaults_json');
      final json = raw as String? ?? '';
      protocolTerms = parseProtocolSearchTerms(json);
      protocolMeta = ProtocolMetaTable.parse(json, locale);
      _notify();
    } catch (_) {
      /* React: .catch(console.error) */
    }
  }

  /// UI 语言，决定协议 label 取哪个 locale 的 name。切语言后调 [setLocale] 重解析。
  String locale = 'zh-Hans';

  void setLocale(String next) {
    if (next == locale) return;
    locale = next;
    unawaited(_loadProtocolTerms());
  }

  // ── 平台 logo 三级回退（`useProtocolLogo.ts`）─────────────────────
  //
  // ① `get_protocol_logo_path` 查本地缓存路径 → ② 命中就换成
  // `get_protocol_logo_data_url`（Flutter 没有 Tauri 的 `asset://`，只能走 data URL，
  // 与 React 的浏览器分支同一条路）→ ③ miss 就触发 `sync_protocol_logo` 后台补拉，
  // **本会话不再轮询**，下次进页面命中即用。全程失败 → 表里没有这一项 → 卡片显首字母。
  Future<void> ensureProtocolLogo(String protocol) async {
    if (protocol.isEmpty || !_logoAsked.add(protocol)) return;
    try {
      final path = await _invoke('get_protocol_logo_path', {
        'protocol': protocol,
      });
      if (path is String && path.isNotEmpty) {
        final dataUrl = await _invoke('get_protocol_logo_data_url', {
          'protocol': protocol,
        });
        if (dataUrl is String && dataUrl.isNotEmpty) {
          protocolLogos = {...protocolLogos, protocol: dataUrl};
          _notify();
        }
      } else {
        // 缓存 miss：后台补拉，不等待（React: `.catch(console.warn)`）。
        unawaited(
          _invoke('sync_protocol_logo', {'protocol': protocol}).catchError(
            (Object _) => null,
          ),
        );
      }
    } catch (_) {
      /* React: console.warn("[logo] getProtocolLogoPath failed") */
    }
  }

  // ── 展开明细（`usePlatformsState.ts:187`）────────────────────────

  final Map<int, Timer> _expandTimers = {};

  /// 展开 / 收起一张卡。落盘走 300ms 防抖 —— 连点不会打后端一串写。
  void toggleExpanded(int platformId, bool next) {
    expandedIds = {...expandedIds};
    if (next) {
      expandedIds.add(platformId);
    } else {
      expandedIds.remove(platformId);
    }
    _notify();
    _expandTimers[platformId]?.cancel();
    _expandTimers[platformId] = Timer(
      const Duration(milliseconds: 300),
      () => unawaited(persistExpanded(platformId, next)),
    );
  }

  /// 页面 dispose 时叫一次：把没到点的防抖计时器掐掉。
  void dispose() {
    for (final t in _expandTimers.values) {
      t.cancel();
    }
    _expandTimers.clear();
  }

  /// 轻量刷新（`proxy-log-updated` 驱动）：**只 merge 后台派生的统计字段**，
  /// 不整列表替换。整列表替换会打断 memo 与拖拽态，还会和乐观写竞争。
  /// `usePlatformsState.ts:434-473`。
  Future<void> refreshStats() async {
    final captured = epoch;
    try {
      final v = await _invoke('platform_list');
      final fresh = <int, PlatformRow>{};
      for (final e in (v as List? ?? const [])) {
        final row = PlatformRow.fromJson(e as Map<String, dynamic>);
        fresh[row.id] = row;
      }
      if (captured == epoch) {
        var changed = false;
        final next = <PlatformRow>[];
        for (final p in platforms) {
          final f = fresh[p.id];
          // 后端没有这一行（刚被别处删了）→ 原样留着，删除交给显式写操作处理。
          // 七个统计字段全等 → 保留原对象引用，卡片的 memo 就不会重渲染。
          if (f == null || p.statsEqual(f)) {
            next.add(p);
          } else {
            changed = true;
            next.add(p.mergeStats(f));
          }
        }
        if (changed) {
          platforms = next;
          _notify();
        }
      }
      final u = await _invoke('all_platform_usage_stats');
      final m = (u as Map?)?.cast<String, dynamic>() ?? const <String, dynamic>{};
      usageMap = {
        for (final e in m.entries)
          if (int.tryParse(e.key) != null && e.value is Map)
            int.parse(e.key): UsageStats.fromJson(
              (e.value as Map).cast<String, dynamic>(),
            ),
      };
      _notify();
    } catch (_) {
      /* React: try/catch 吞掉 */
    }
  }

  /// 删平台后按 id 局部移除（不调 API —— 调用方已调过），epoch++ 让派生层重算。
  /// `usePlatformsState.ts:356-361`。分组页删平台之后由父级调这个，
  /// 不走全量 refetch，消除整页刷新的体感。
  void removePlatformsByIds(List<int> ids) {
    if (ids.isEmpty) return;
    epoch++;
    final idSet = ids.toSet();
    platforms = [
      for (final p in platforms)
        if (!idSet.contains(p.id)) p,
    ];
    _notify();
  }

  /// 全量 refetch（分组页删平台后的另一条入口）。`usePlatformsState.ts:344-352`。
  Future<void> refreshPlatforms() async {
    epoch++;
    try {
      final v = await _invoke('platform_list');
      platforms = [
        for (final e in (v as List? ?? const []))
          PlatformRow.fromJson(e as Map<String, dynamic>),
      ];
      _notify();
    } catch (_) {
      /* React: console.error("refreshPlatforms failed", e) */
    }
  }

  // ── 单平台操作 ──────────────────────────────────────────────────

  void askDelete(int id) {
    deleteTarget = id;
    _notify();
  }

  void cancelDelete() {
    deleteTarget = null;
    _notify();
  }

  /// `usePlatformsState.ts:475-511`：乐观从列表移除 → 调删除 → 失败**插回原位**。
  /// 插回原位（不是追加到末尾）很重要，否则用户看到的是「删除失败，顺便还被挪到了最后」。
  Future<void> deletePlatform(int id, {String failText = '删除失败'}) async {
    deleteTarget = null;
    final removedIndex = platforms.indexWhere((x) => x.id == id);
    final removed = removedIndex >= 0 ? platforms[removedIndex] : null;
    epoch++;
    platforms = [
      for (final p in platforms)
        if (p.id != id) p,
    ];
    _notify();
    try {
      await _invoke('platform_delete', {'id': id});
      await _loadGroupDetails();
    } catch (_) {
      if (removed != null && !platforms.any((x) => x.id == removed.id)) {
        final next = [...platforms];
        final at = (removedIndex >= 0 && removedIndex <= next.length)
            ? removedIndex
            : next.length;
        next.insert(at, removed);
        platforms = next;
      }
      _toast(failText, ok: false);
      _notify();
    }
  }

  /// 三态启停：`enabled → disabled`，其余（含 auto_disabled）→ `enabled`。
  /// 乐观翻转 → 用后端写回值校正那一行 → 失败回滚那一行。
  /// 状态切换**不改分组归属**，所以不刷 groupDetails（`usePlatformsState.ts:517-518`）。
  Future<void> togglePlatform(PlatformRow p, {String failText = '切换失败'}) async {
    final nextStatus = p.status == 'enabled' ? 'disabled' : 'enabled';
    platforms = [
      for (final x in platforms) x.id == p.id ? x.withStatus(nextStatus) : x,
    ];
    _notify();
    try {
      final v = await _invoke('platform_update', {
        'input': {'id': p.id, 'status': nextStatus},
      });
      final updated = PlatformRow.fromJson((v as Map).cast<String, dynamic>());
      platforms = [
        for (final x in platforms) x.id == p.id ? updated : x,
      ];
    } catch (_) {
      platforms = [
        for (final x in platforms) x.id == p.id ? p : x,
      ];
      _toast('${p.name}: $failText', ok: false);
    }
    _notify();
  }

  /// 快速测试一个平台：用默认模型，没有就用可用列表第一个，再没有就空串。
  /// `usePlatformsState.ts:534-554`。
  Future<void> quickTest(
    PlatformRow p, {
    String okText = '测试成功',
    String failText = '测试失败',
  }) async {
    testingId = p.id;
    _notify();
    final model = p.models.defaultModel?.isNotEmpty == true
        ? p.models.defaultModel!
        : (p.availableModels.isNotEmpty ? p.availableModels.first : '');
    try {
      final v = await _invoke('model_test', {
        'req': {'platform_id': p.id, 'model': model},
      });
      final r = ModelTestResult.fromJson(
        (v as Map?)?.cast<String, dynamic>() ?? const {},
      );
      testResults = {...testResults, p.id: r.success ? 'ok' : 'fail'};
      _toast(
        r.success
            ? '${p.name}: $okText${r.durationMs > 0 ? ' (${r.durationMs}ms)' : ''}'
            : '${p.name}: ${r.error.isEmpty ? failText : r.error}',
        ok: r.success,
      );
    } catch (e) {
      testResults = {...testResults, p.id: 'fail'};
      _toast('${p.name}: $e', ok: false);
    }
    testingId = null;
    _notify();
    await refreshLastTest(p.id);
  }

  /// 导出可分享配置（含明文 api_key），调用方拿去开分享弹窗。
  /// `usePlatformsState.ts:569-578`。
  Future<Map<String, Object?>?> shareExport(
    PlatformRow p, {
    String failText = '生成分享内容失败',
  }) async {
    try {
      final v = await _invoke('platform_share_export', {'platformId': p.id});
      return (v as Map?)?.cast<String, Object?>();
    } catch (_) {
      _toast('${p.name}: $failText', ok: false);
      return null;
    }
  }

  /// 解析别人分享过来的配置文本（智能粘贴弹窗用）。`platforms.ts:543`。
  Future<Map<String, Object?>?> shareParse(String text) async {
    try {
      final v = await _invoke('platform_share_parse', {'text': text});
      return (v as Map?)?.cast<String, Object?>();
    } catch (_) {
      return null;
    }
  }

  /// 拖完一张卡：未分组列表里把 [oldIndex] 挪到 [newIndex]，本地先乐观重排
  /// （松手即到位），再把整串 id 发后端。
  ///
  /// 下标口径按 `ReorderableListView.onReorderItem`：newIndex 已经把「旧项先被
  /// 移走」算进去了，直接当落点用（老的 `onReorder` 要自己减 1，那个已废弃）。
  Future<void> reorderStandalone(int oldIndex, int newIndex) async {
    if (oldIndex == newIndex) return;
    final list = [...standalonePlatforms];
    if (oldIndex < 0 || oldIndex >= list.length) return;
    final moved = list.removeAt(oldIndex);
    list.insert(newIndex.clamp(0, list.length), moved);
    applyLocalOrder(list);
    await reorder([for (final p in list) p.id]);
  }

  /// 拖拽松手的乐观重排：把 [ordered]（只含未分组那批）按新顺序放回 [platforms]，
  /// **原本属于分组的行留在原位**不动。没有这一步，卡片会等后端回来才跳到新位置。
  void applyLocalOrder(List<PlatformRow> ordered) {
    final movable = {for (final p in ordered) p.id};
    var next = 0;
    platforms = [
      for (final p in platforms)
        if (movable.contains(p.id)) ordered[next++] else p,
    ];
    _notify();
  }

  /// 拖拽排序：传按新顺序排的 id 列表。`usePlatformsState.ts:251`。
  Future<void> reorder(List<int> orderedIds) async {
    try {
      await _invoke('platform_reorder', {'orderedIds': orderedIds});
    } catch (_) {
      /* React: .catch(console.error) */
    }
  }

  /// 把未分组平台拖进某个组（`fromGroupId = 0` = 来自未分组区）。
  /// `usePlatformsState.ts:303-311`。
  Future<void> moveIntoGroup(
    int pid,
    int gid, {
    String okText = '已加入分组',
    String failText = '加入分组失败',
  }) async {
    try {
      await _invoke('group_platform_move', {
        'platformId': pid,
        'fromGroupId': 0,
        'toGroupId': gid,
      });
      _toast(okText);
      await _loadGroupDetails();
    } catch (e) {
      _toast('$failText: $e', ok: false);
    }
  }

  // ── 清理失效平台（全局）────────────────────────────────────────

  Future<void> askPurgeDisabled() async {
    purgeCandidates = null;
    purgePreviewLoading = true;
    _notify();
    try {
      final v = await _invoke('platform_purge_disabled_preview', {
        'groupId': null,
      });
      purgeCandidates = [
        for (final e in (v as List? ?? const []))
          PurgeCandidate.fromJson((e as Map).cast<String, dynamic>()),
      ];
    } catch (_) {
      purgeCandidates = const [];
    }
    purgePreviewLoading = false;
    _notify();
  }

  void cancelPurgeDisabled() {
    if (purging) return;
    purgeCandidates = null;
    purgePreviewLoading = false;
    _notify();
  }

  /// `usePlatformsState.ts:582-608`：删完按 `deletedIds` 局部移除（不整页 load），
  /// `unassignedIds`（只解除了分组关联、平台还在）交给 groupDetails 重建 membership。
  Future<void> confirmPurgeDisabled({
    String noneText = '暂无失效平台',
    String Function(int count)? doneText,
    String failText = '清理失效平台',
  }) async {
    purging = true;
    _notify();
    try {
      final v = await _invoke('platform_purge_disabled', {'groupId': null});
      final r = PurgeReport.fromJson(
        (v as Map?)?.cast<String, dynamic>() ?? const {},
      );
      if (r.deletedIds.isEmpty) {
        _toast(noneText);
      } else {
        _toast((doneText ?? (n) => '已删除 $n 个失效平台')(r.deletedIds.length));
        final del = r.deletedIds.toSet();
        epoch++;
        platforms = [
          for (final p in platforms)
            if (!del.contains(p.id)) p,
        ];
      }
      await _loadGroupDetails();
    } catch (e) {
      _toast('$failText: $e', ok: false);
    } finally {
      purging = false;
      purgeCandidates = null;
      _notify();
    }
  }

  // ── 余额 / 配额 ────────────────────────────────────────────────

  /// `usePlatformQuota.ts:63-71`：没 key 不查；没配额脚本不查；没 base_url 不查。
  ///
  /// 配额脚本门控走 [ProtocolMetaTable.hasQuotaScript]（registry 变体或用户自定义脚本；
  /// registry 未到手时回落旧启发式，与 `defaults.ts::platformHasQuotaScript` 同口径）。
  /// **不**做 requires 满足度门控 —— newapi 的 balance_* 仅 unlimited 路径必需，
  /// 门控会回归 limited token 用户。
  bool platformWantsQuota(PlatformRow p) {
    if (p.apiKey.isEmpty) return false;
    if (!protocolMeta.hasQuotaScript(p.platformType, p.extra)) return false;
    return getPrimaryBaseUrl(p.platformType, p.endpoints).isNotEmpty ||
        p.baseUrl.isNotEmpty;
  }

  /// 按平台协议选对应的查询命令。三个命令参数不同：newapi / devin 要多传 `extra`。
  /// `usePlatformQuota.ts:73-86`。
  Future<PlatformQuota?> _queryQuota(PlatformRow p) async {
    final baseUrl =
        getPrimaryBaseUrl(p.platformType, p.endpoints).isNotEmpty
        ? getPrimaryBaseUrl(p.platformType, p.endpoints)
        : p.baseUrl;
    final Object? v;
    if (p.platformType == 'newapi') {
      v = await _invoke('platform_query_quota_newapi', {
        'baseUrl': baseUrl,
        'apiKey': p.apiKey,
        'extra': p.extra,
        'platformId': p.id,
      });
    } else if (p.platformType == 'devin') {
      v = await _invoke('platform_query_quota_devin', {
        'baseUrl': baseUrl,
        'apiKey': p.apiKey,
        'extra': p.extra,
        'platformId': p.id,
      });
    } else {
      v = await _invoke('platform_query_quota', {
        'baseUrl': baseUrl,
        'apiKey': p.apiKey,
        'platformId': p.id,
      });
    }
    return v == null
        ? null
        : PlatformQuota.fromJson((v as Map).cast<String, dynamic>());
  }

  /// 后台批量查余额，**有界并发** [kQuotaConcurrency]（不是一次全发 —— 这些是真出
  /// 网的 HTTP，几十个平台同时打会把上游打到限流）。
  ///
  /// **与 React 的一处差异**：那边由 `IntersectionObserver` 按卡片进入视口的顺序
  /// 入队（可视优先），Flutter 这边没有等价的廉价原语，改成列表顺序入队。
  /// 并发上限、去重、pending 三态都一样，差别只在**先查哪个**。
  Future<void> pumpQuota() async {
    final queue = [
      for (final p in platforms)
        if (platformWantsQuota(p) && quotaPending[p.id] == true) p,
    ];
    var next = 0;
    Future<void> worker() async {
      while (next < queue.length) {
        final p = queue[next++];
        try {
          final q = await _queryQuota(p);
          if (q != null && q.success) {
            quotaMap = {...quotaMap, p.id: q};
          }
        } catch (_) {
          /* React: catch 吞掉，单个平台查不到不影响别的 */
        } finally {
          final pend = {...quotaPending}..remove(p.id);
          quotaPending = pend;
          _notify();
        }
      }
    }

    final poolSize = queue.length < kQuotaConcurrency
        ? queue.length
        : kQuotaConcurrency;
    await Future.wait([for (var i = 0; i < poolSize; i++) worker()]);
  }

  /// 手动点刷新：带转圈、带失败提示。没 key 直接提示，不发命令。
  /// `usePlatformQuota.ts:125-156`。
  Future<void> refreshQuota(
    PlatformRow p, {
    String noKeyText = '缺少 Token',
    String failText = '刷新额度失败',
  }) async {
    if (p.apiKey.isEmpty) {
      _toast('${p.name}: $noKeyText', ok: false);
      return;
    }
    final pend = {...quotaPending}..remove(p.id);
    quotaPending = pend;
    quotaRefreshing = {...quotaRefreshing, p.id: true};
    _notify();
    try {
      final q = await _queryQuota(p);
      if (q != null && q.success) {
        quotaMap = {...quotaMap, p.id: q};
        // 手动刷新 = 真值校准：这一条之后优先用真查值，不再回落预估。
        quotaRealIds = {...quotaRealIds, p.id: true};
      } else {
        final err = q?.error;
        _toast('${p.name}: ${err == null || err.isEmpty ? failText : err}', ok: false);
      }
    } catch (_) {
      _toast('${p.name}: $failText', ok: false);
    }
    quotaRefreshing = {...quotaRefreshing, p.id: false};
    _notify();
  }

  // ── 表单：新建 / 编辑 / 保存 ────────────────────────────────────

  /// 「获取模型列表」的多协议回退链（`usePlatformForm.ts:471-537`）。
  ///
  /// 顺序：openai 端点优先 → 主协议端点 → 其余端点，按 `(协议, URL)` 去重。
  /// 逐个试，**401/403 立即停**（鉴权错了再换协议没有意义），404 / 其他错继续试下一个。
  /// 首个返回非空列表的即采用。
  ///
  /// 返回 `(models, error)`：成功时 error 为 null；全试完没拿到就带最后一条错。
  Future<(List<String>, String?)> fetchModels({
    required String protocol,
    required String apiKey,
    required List<PlatformEndpoint> endpoints,
    String emptyText = '未获取到模型',
    String Function(int code)? authText,
  }) async {
    if (apiKeyMissing(protocol, apiKey)) return (const <String>[], null);
    final primaryBase = getPrimaryBaseUrl(protocol, endpoints);
    final seen = <String>{};
    final tryList = <({String proto, String url})>[];
    void push(String proto, String url) {
      if (url.isEmpty) return;
      final key = '$proto|$url';
      if (!seen.add(key)) return;
      tryList.add((proto: proto, url: url));
    }

    for (final ep in endpoints) {
      if (ep.protocol == 'openai') {
        push('openai', ep.baseUrl);
        break;
      }
    }
    if (primaryBase.isNotEmpty) push(protocol, primaryBase);
    for (final ep in endpoints) {
      if (ep.protocol == 'openai' || ep.protocol == protocol) continue;
      push(ep.protocol, ep.baseUrl);
    }
    if (tryList.isEmpty) return (const <String>[], null);

    String? lastError;
    for (final t in tryList) {
      try {
        final v = await _invoke('platform_fetch_models', {
          'protocol': t.proto,
          'baseUrl': t.url,
          'apiKey': apiKey,
        });
        final ids = [for (final e in (v as List? ?? const [])) e as String];
        // 空列表（200 但没 data）不算成功，继续试下一个端点。
        if (ids.isEmpty) continue;
        return (ids, null);
      } catch (e) {
        final m = _errorMap(e);
        final kind = m?['kind'];
        if (kind == 'Auth') {
          final code = (m?['code'] as num?)?.toInt() ?? 0;
          return (
            const <String>[],
            authText != null ? authText(code) : '鉴权失败（$code）',
          );
        }
        lastError = (m?['message'] as String?) ?? '$e';
      }
    }
    return (const <String>[], lastError ?? emptyText);
  }

  /// `platform_create` / `platform_update`。
  ///
  /// 两条语义差别（`usePlatformForm.ts:691-702`）：
  /// - `auto_group`（要不要顺手建一个默认分组）**只有创建才有**，更新不带；
  /// - `manual_budgets` 更新时即使为空也要带（表示「清空」），创建时空则不带。
  Future<PlatformRow?> savePlatform({
    required String name,
    required String protocol,
    required String apiKey,
    required Map<String, String> models,
    required List<String> availableModels,
    required List<PlatformEndpoint> endpoints,
    required String extra,
    required List<int> joinGroupIds,
    required int expiresAt,
    List<Map<String, Object?>> manualBudgets = const [],
    bool autoGroup = true,
    int? editingId,
    String failText = '保存失败',
    /// 批量创建时由调用方统一汇总失败，逐条 toast 会刷屏 —— 置 true 就不单独提示。
    bool silent = false,
    /// 失败原因原文，供表单底部的错误条展示（React `setSaveError(msg)`）。
    void Function(String message)? onError,
  }) async {
    final shared = <String, Object?>{
      'platform_type': protocol,
      'base_url': getPrimaryBaseUrl(protocol, endpoints),
      if (extra.isNotEmpty) 'extra': extra,
      'models': ?buildModelsPayload(models),
      if (availableModels.isNotEmpty) 'available_models': availableModels,
      if (endpoints.isNotEmpty)
        'endpoints': [for (final e in endpoints) e.toJson()],
      'join_group_ids': joinGroupIds,
      'expires_at': expiresAt,
    };
    // 纯透传平台没有手动预算的概念（`usePlatformForm.ts:636`）。
    final budgets = isPassthroughProtocol(protocol)
        ? const <Map<String, Object?>>[]
        : manualBudgets;
    try {
      final Object? v;
      if (editingId != null) {
        v = await _invoke('platform_update', {
          'input': {
            'id': editingId,
            'name': name,
            'api_key': apiKey,
            ...shared,
            'manual_budgets': budgets,
          },
        });
      } else {
        v = await _invoke('platform_create', {
          'input': {
            'name': name,
            'api_key': apiKey,
            ...shared,
            'auto_group': autoGroup,
            if (budgets.isNotEmpty) 'manual_budgets': budgets,
          },
        });
      }
      final saved = PlatformRow.fromJson((v as Map).cast<String, dynamic>());
      epoch++;
      platforms = editingId != null
          ? [for (final x in platforms) x.id == saved.id ? saved : x]
          : (platforms.any((x) => x.id == saved.id)
                ? platforms
                : [...platforms, saved]);
      _notify();
      // 局部保存没走 load()，所以该平台的用量 / 最近测试 / 余额要自己补一遍。
      await Future.wait<void>([
        _refreshUsageFor(saved.id),
        refreshLastTest(saved.id),
        _loadGroupDetails(),
      ]);
      quotaPending = {...quotaPending, saved.id: true};
      _notify();
      return saved;
    } catch (e) {
      onError?.call('$e');
      if (!silent) _toast('$failText: $e', ok: false);
      return null;
    }
  }

  Future<void> _refreshUsageFor(int id) async {
    try {
      final v = await _invoke('platform_usage_stats', {'platformId': id});
      if (v != null) {
        usageMap = {
          ...usageMap,
          id: UsageStats.fromJson((v as Map).cast<String, dynamic>()),
        };
        _notify();
      }
    } catch (_) {
      /* React: .catch(() => {}) */
    }
  }

  /// 列表卡的展开态落盘（300ms 防抖之后调，键与分组内卡的 `_ui_expand_grp` 区分开）。
  /// `usePlatformsState.ts:187`。
  Future<void> persistExpanded(int platformId, bool expanded) async {
    try {
      await _invoke('set_ui_extra', {
        'target': 'platform',
        'id': platformId,
        'key': '_ui_expand_plat',
        'value': expanded,
      });
    } catch (_) {
      /* React: .catch(console.error) */
    }
  }

  static Map<String, Object?>? _errorMap(Object e) {
    // I01 的 RpcException 把后端返回的错误值原样挂在 body 上；
    // `FetchModelsError` 就是 `{kind, code, message}` 这个形状。
    try {
      final body = (e as dynamic).body;
      if (body is Map) return body.cast<String, Object?>();
    } catch (_) {
      /* 不是 RpcException */
    }
    return null;
  }

}
