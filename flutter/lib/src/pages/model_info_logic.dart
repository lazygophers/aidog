/// 模型信息页的逻辑层（票 I09），对应 `src/pages/ModelInfo/`
/// （`ModelInfoTab.tsx` + `priceData.ts` + `ModelName.tsx` + `Pagination.tsx`
/// + `CapabilityBadges.tsx` + `SyncStatusCard.tsx` + `ModelDetailDialog.tsx`）。
///
/// 数据源是 **`model_info_snapshot` 一次 RPC 拿全**（聚合行 + 平台预设），
/// 分页是纯客户端切片，不做二次 RPC 拼装。
///
/// **`model_entry_list` 不在这里**：那条命令在用户真实库里量到 3.44 MB / 4580 行，
/// React 的模型信息页也没用它（它是平台维度的按需查询）。本页不碰它，更不放启动路径。
library;

import 'dart:convert';

import 'package:flutter/foundation.dart' show VoidCallback;

import '../../utils/formatters.dart';
import 'invoke.dart';
import 'skills_logic.dart' show TrFn;

/// `CapabilityBadges.tsx:12` 的枚举，顺序一字不改（决定 `modelInfo.cap.*` 覆盖面）。
const List<String> kCapabilities = [
  'text', 'vision', 'tool_use', 'reasoning',
  'text_to_image', 'image_to_image', 'image_edit',
  'text_to_video', 'image_to_video', 'video_to_video', 'video_edit',
  'audio', 'video', 'embedding', 'rerank',
];

/// `Pagination.tsx:14`。
const List<int> kPageSizeOptions = [20, 50, 100, 200];

/// `SyncStatusCard.tsx:93` 的五个间隔档（秒）。
const List<int> kSyncIntervalOptions = [3600, 21600, 43200, 86400, 604800];

/// 未知能力（registry 先行加了新枚举、locale 还没跟上）**原样显示裸值，不隐藏**。
String capabilityLabel(TrFn t, String cap) =>
    kCapabilities.contains(cap) ? t('modelInfo.cap.$cap') : cap;

// ── 模型 ────────────────────────────────────────────────────────────

/// `types/generated/ModelEntry.ts`。价格维度整份留在 [priceData] 原文里。
class ModelEntry {
  const ModelEntry({
    required this.platformCode,
    required this.modelId,
    required this.displayName,
    required this.canonicalModel,
    required this.family,
    required this.version,
    required this.predecessor,
    required this.capabilities,
    required this.builtinToolsExcluded,
    this.maxInputTokens,
    this.maxOutputTokens,
    this.contextWindow,
    required this.official,
    required this.priceData,
    this.updatedAt = 0,
  });

  final String platformCode;
  final String modelId;
  final String displayName;
  final String canonicalModel;
  final String family;
  final String version;
  final String predecessor;
  final List<String> capabilities;
  final List<String> builtinToolsExcluded;
  final int? maxInputTokens;
  final int? maxOutputTokens;
  final int? contextWindow;
  final bool official;

  /// 整份 registry 模型 JSON 文本。
  final String priceData;
  final int updatedAt;

  static List<String> _strs(Object? raw) => [
    for (final e in ((raw as List<Object?>?) ?? const [])) '$e',
  ];

  static ModelEntry fromJson(Map<String, Object?> j) => ModelEntry(
    platformCode: (j['platform_code'] as String?) ?? '',
    modelId: (j['model_id'] as String?) ?? '',
    displayName: (j['display_name'] as String?) ?? '',
    canonicalModel: (j['canonical_model'] as String?) ?? '',
    family: (j['family'] as String?) ?? '',
    version: (j['version'] as String?) ?? '',
    predecessor: (j['predecessor'] as String?) ?? '',
    capabilities: _strs(j['capabilities']),
    builtinToolsExcluded: _strs(j['builtin_tools_excluded']),
    maxInputTokens: (j['max_input_tokens'] as num?)?.toInt(),
    maxOutputTokens: (j['max_output_tokens'] as num?)?.toInt(),
    contextWindow: (j['context_window'] as num?)?.toInt(),
    official: j['official'] == true,
    priceData: (j['price_data'] as String?) ?? '',
    updatedAt: (j['updated_at'] as num?)?.toInt() ?? 0,
  );
}

/// `types/generated/ModelEntryGroup.ts`。一个 canonical 下的全部平台条目。
class ModelEntryGroup {
  const ModelEntryGroup({
    required this.canonicalModel,
    required this.displayName,
    required this.primaryPlatform,
    required this.entries,
  });

  final String canonicalModel;
  final String displayName;

  /// 代表条目所在平台：优先 `official = true`，否则 `platform_code` 字典序第一条。
  final String primaryPlatform;
  final List<ModelEntry> entries;

  static ModelEntryGroup fromJson(Map<String, Object?> j) => ModelEntryGroup(
    canonicalModel: (j['canonical_model'] as String?) ?? '',
    displayName: (j['display_name'] as String?) ?? '',
    primaryPlatform: (j['primary_platform'] as String?) ?? '',
    entries: [
      for (final e in (j['entries'] as List<Object?>? ?? const []))
        ModelEntry.fromJson((e as Map).cast<String, Object?>()),
    ],
  );

  /// `ModelInfoTab.tsx:338`：代表条目找不到就退第一条。
  ModelEntry? get primary {
    for (final e in entries) {
      if (e.platformCode == primaryPlatform) return e;
    }
    return entries.isNotEmpty ? entries.first : null;
  }
}

/// `types/generated/ModelInfoSnapshot.ts`。
class ModelInfoSnapshot {
  const ModelInfoSnapshot({
    required this.groups,
    required this.pricingOnly,
    required this.bundled,
  });

  final List<ModelEntryGroup> groups;

  /// 只出比价条目、没有 `platform.json` 的来源 code。**不得进平台筛选与平台维度列表**。
  final Set<String> pricingOnly;

  /// true = DB 尚无同步数据，当前是编译期内置 registry 兜底。
  final bool bundled;

  static ModelInfoSnapshot fromJson(Map<String, Object?> j) => ModelInfoSnapshot(
    groups: [
      for (final g in (j['groups'] as List<Object?>? ?? const []))
        ModelEntryGroup.fromJson((g as Map).cast<String, Object?>()),
    ],
    pricingOnly: {
      for (final c in (j['pricing_only'] as List<Object?>? ?? const [])) '$c',
    },
    bundled: j['bundled'] == true,
  );
}

/// `types/generated/PriceSyncSettings.ts`。
class PriceSyncSettings {
  const PriceSyncSettings({
    required this.autoSyncEnabled,
    required this.syncIntervalSecs,
    required this.lastSyncAt,
    required this.registryLastUpdated,
    required this.fallbackInputPrice,
    required this.fallbackOutputPrice,
  });

  final bool autoSyncEnabled;
  final int syncIntervalSecs;

  /// 上次同步时间（**ms** timestamp）。
  final int lastSyncAt;
  final int registryLastUpdated;
  final double fallbackInputPrice;
  final double fallbackOutputPrice;

  /// `ModelInfoTab.tsx:54::DEFAULT_SYNC_SETTINGS`，值一字不改。
  static const PriceSyncSettings defaults = PriceSyncSettings(
    autoSyncEnabled: false,
    syncIntervalSecs: 86400,
    lastSyncAt: 0,
    registryLastUpdated: 0,
    fallbackInputPrice: 3.0,
    fallbackOutputPrice: 3.0,
  );

  static PriceSyncSettings fromJson(Map<String, Object?> j) => PriceSyncSettings(
    autoSyncEnabled: j['auto_sync_enabled'] == true,
    syncIntervalSecs: (j['sync_interval_secs'] as num?)?.toInt() ?? 86400,
    lastSyncAt: (j['last_sync_at'] as num?)?.toInt() ?? 0,
    registryLastUpdated: (j['registry_last_updated'] as num?)?.toInt() ?? 0,
    fallbackInputPrice: (j['fallback_input_price'] as num?)?.toDouble() ?? 3.0,
    fallbackOutputPrice: (j['fallback_output_price'] as num?)?.toDouble() ?? 3.0,
  );

  Map<String, Object?> toJson() => {
    'auto_sync_enabled': autoSyncEnabled,
    'sync_interval_secs': syncIntervalSecs,
    'last_sync_at': lastSyncAt,
    'registry_last_updated': registryLastUpdated,
    'fallback_input_price': fallbackInputPrice,
    'fallback_output_price': fallbackOutputPrice,
  };

  PriceSyncSettings copyWith({
    bool? autoSyncEnabled,
    int? syncIntervalSecs,
    double? fallbackInputPrice,
    double? fallbackOutputPrice,
  }) => PriceSyncSettings(
    autoSyncEnabled: autoSyncEnabled ?? this.autoSyncEnabled,
    syncIntervalSecs: syncIntervalSecs ?? this.syncIntervalSecs,
    lastSyncAt: lastSyncAt,
    registryLastUpdated: registryLastUpdated,
    fallbackInputPrice: fallbackInputPrice ?? this.fallbackInputPrice,
    fallbackOutputPrice: fallbackOutputPrice ?? this.fallbackOutputPrice,
  );
}

/// `types/generated/SyncFailure.ts`。
class SyncFailure {
  const SyncFailure({required this.file, required this.error});

  final String file;
  final String error;

  static SyncFailure fromJson(Map<String, Object?> j) => SyncFailure(
    file: (j['file'] as String?) ?? '',
    error: (j['error'] as String?) ?? '',
  );
}

/// `types/generated/PriceSyncResult.ts`。`failures` 非空即 partial。
class PriceSyncResult {
  const PriceSyncResult({
    required this.added,
    required this.updated,
    required this.unchanged,
    required this.failed,
    required this.total,
    required this.failures,
  });

  final int added;
  final int updated;
  final int unchanged;
  final int failed;
  final int total;
  final List<SyncFailure> failures;

  static PriceSyncResult fromJson(Map<String, Object?> j) => PriceSyncResult(
    added: (j['added'] as num?)?.toInt() ?? 0,
    updated: (j['updated'] as num?)?.toInt() ?? 0,
    unchanged: (j['unchanged'] as num?)?.toInt() ?? 0,
    failed: (j['failed'] as num?)?.toInt() ?? 0,
    total: (j['total'] as num?)?.toInt() ?? 0,
    failures: [
      for (final f in (j['failures'] as List<Object?>? ?? const []))
        SyncFailure.fromJson((f as Map).cast<String, Object?>()),
    ],
  );
}

// ── `priceData.ts` ─────────────────────────────────────────────────

/// `priceData.ts:9::PriceTier`。单位 $/token，字段名 = registry `price` 子树简名。
class PriceTier {
  const PriceTier({
    this.input,
    this.output,
    this.cacheRead,
    this.cacheWrite,
    this.minTokens,
  });

  final double? input;
  final double? output;
  final double? cacheRead;
  final double? cacheWrite;

  /// 仅 `context_tiers` 档内有：起档阈值。
  final int? minTokens;

  static PriceTier fromJson(Map<String, Object?> j) => PriceTier(
    input: (j['input'] as num?)?.toDouble(),
    output: (j['output'] as num?)?.toDouble(),
    cacheRead: (j['cache_read'] as num?)?.toDouble(),
    cacheWrite: (j['cache_write'] as num?)?.toDouble(),
    minTokens: (j['min_tokens'] as num?)?.toInt(),
  );
}

/// `priceData.ts:20::ModelPriceData`。
class ModelPriceData extends PriceTier {
  const ModelPriceData({
    super.input,
    super.output,
    super.cacheRead,
    super.cacheWrite,
    this.unit,
    this.unitPrice,
    this.peak,
    this.contextTiers = const [],
  });

  /// 计价单位，缺省 token。非 token（图像/视频/搜索）用 [unitPrice] 计价。
  final String? unit;
  final double? unitPrice;

  /// 高峰绝对价：命中平台 `peak` 窗口时整体替换默认价。
  final PriceTier? peak;
  final List<PriceTier> contextTiers;
}

/// `priceData.ts:33::parsePriceData`。取 `price` 子树；
/// 空串 / 非法 JSON / 缺 price 一律返回空对象（展示层按「无价格」渲染，不炸页）。
ModelPriceData parsePriceData(String raw) {
  if (raw.isEmpty) return const ModelPriceData();
  try {
    final parsed = jsonDecode(raw);
    if (parsed is! Map) return const ModelPriceData();
    final price = parsed['price'];
    if (price is! Map) return const ModelPriceData();
    final p = price.cast<String, Object?>();
    final peak = p['peak'];
    final tiers = p['context_tiers'];
    return ModelPriceData(
      input: (p['input'] as num?)?.toDouble(),
      output: (p['output'] as num?)?.toDouble(),
      cacheRead: (p['cache_read'] as num?)?.toDouble(),
      cacheWrite: (p['cache_write'] as num?)?.toDouble(),
      unit: p['unit'] as String?,
      unitPrice: (p['unit_price'] as num?)?.toDouble(),
      peak: peak is Map ? PriceTier.fromJson(peak.cast<String, Object?>()) : null,
      contextTiers: tiers is List
          ? [
              for (final t in tiers)
                if (t is Map) PriceTier.fromJson(t.cast<String, Object?>()),
            ]
          : const [],
    );
  } catch (_) {
    // registry 数据损坏不该炸整页；缺价格即显示 "-"。
    return const ModelPriceData();
  }
}

/// `priceData.ts:51::EntryFlags`。缺省 = 未标注，展示 `-`（不与 false 混同）。
class EntryFlags {
  const EntryFlags({this.thinkingSupported, this.thinkingToggleable});

  final bool? thinkingSupported;
  final bool? thinkingToggleable;
}

/// `priceData.ts:59::parseEntryFlags`。读的是 `price_data` **顶层**，不是 price 子树。
EntryFlags parseEntryFlags(String raw) {
  if (raw.isEmpty) return const EntryFlags();
  try {
    final parsed = jsonDecode(raw);
    if (parsed is! Map) return const EntryFlags();
    return EntryFlags(
      thinkingSupported: parsed['thinking_supported'] as bool?,
      thinkingToggleable: parsed['thinking_toggleable'] as bool?,
    );
  } catch (_) {
    return const EntryFlags();
  }
}

/// `priceData.ts:81::perMillion`。$/token → $/M tokens；非有限数 → null。
double? perMillion(double? v) =>
    v != null && v.isFinite ? v * 1000000 : null;

/// `priceData.ts:86::fmtPricePerM`。缺值 → `-`。
String fmtPricePerM(double? v) {
  final m = perMillion(v);
  return m == null ? '-' : formatCostUsd(m);
}

/// `priceData.ts:74::fmtPricePerUnit`。`char` 存 $/1K chars，其余 $/unit。
String fmtPricePerUnit(double? v, String? unit) {
  if (v == null || !v.isFinite) return '-';
  return unit == 'char'
      ? '${formatCostUsd(v)} /1K chars'
      : '${formatCostUsd(v)} /${unit ?? 'unit'}';
}

/// `priceData.ts:92::fmtTokens`。131072 → `131.1K`；缺值 → `-`。
String fmtTokens(num? v) =>
    v != null && v.toDouble().isFinite ? formatNumber(v) : '-';

// ── `ModelName.tsx` ────────────────────────────────────────────────

/// `ModelName.tsx:18::NameParts`。[secondary] 为 null = 两者同串，只渲染一行。
class NameParts {
  const NameParts(this.primary, this.secondary);

  final String primary;
  final String? secondary;
}

/// `ModelName.tsx:27::nameParts`。回落已经发生在后端读取层，这里**不写第二份回落**，
/// 只判「展示名和请求名是不是同一串」。
NameParts nameParts(String displayName, String modelId) {
  final name = displayName.trim();
  if (name.isEmpty || name == modelId) return NameParts(modelId, null);
  return NameParts(name, modelId);
}

// ── `Pagination.tsx` 的页码算法 ────────────────────────────────────

/// 页码按钮序列；`null` = 省略号。`Pagination.tsx:33` 的算法一字不改：
/// 总页数 ≤ 7 全列；否则首页 + 当前页 ±1 + 末页，中间按条件插省略号。
List<int?> paginationPages(int currentPage, int totalPages) {
  final pages = <int?>[];
  if (totalPages <= 7) {
    for (var i = 1; i <= totalPages; i++) {
      pages.add(i);
    }
    return pages;
  }
  pages.add(1);
  if (currentPage > 3) pages.add(null);
  final start = currentPage - 1 < 2 ? 2 : currentPage - 1;
  final end = currentPage + 1 > totalPages - 1 ? totalPages - 1 : currentPage + 1;
  for (var i = start; i <= end; i++) {
    pages.add(i);
  }
  if (currentPage < totalPages - 2) pages.add(null);
  pages.add(totalPages);
  return pages;
}

// ── 控制器 ──────────────────────────────────────────────────────────

class ModelInfoController {
  ModelInfoController({
    required this.invoke,
    required this.t,
    required this.onChanged,
  });

  final InvokeFn invoke;
  final TrFn t;
  final VoidCallback onChanged;

  ModelInfoSnapshot? snapshot;

  /// platform_code → 本地化平台名。空 = 回落裸 code。
  Map<String, String> labelMap = const {};

  bool loading = true;
  bool syncing = false;
  PriceSyncResult? syncResult;
  String message = '';
  PriceSyncSettings settings = PriceSyncSettings.defaults;

  String query = '';
  String platformFilter = '';
  String capabilityFilter = '';
  bool officialOnly = false;
  int pageSize = 50;
  int page = 1;
  String jumpPage = '';

  /// 选中的 canonical（详情弹窗的单一驱动状态）。
  String? selected;
  String? activePlatform;

  /// `models` / `platforms` 两个 tab。
  String tab = 'models';

  List<ModelEntryGroup> get groups => snapshot?.groups ?? const [];
  Set<String> get pricingOnly => snapshot?.pricingOnly ?? const {};
  bool get bundled => snapshot?.bundled ?? false;

  /// `ModelInfoTab.tsx:113`：聚合行反向展开成 platform_code → 条目，
  /// 每个平台内按 `model_id` 升序。
  Map<String, List<ModelEntry>> get byPlatform {
    final m = <String, List<ModelEntry>>{};
    for (final g in groups) {
      for (final e in g.entries) {
        m.putIfAbsent(e.platformCode, () => []).add(e);
      }
    }
    for (final list in m.values) {
      list.sort((a, b) => a.modelId.compareTo(b.modelId));
    }
    return m;
  }

  /// `ModelInfoTab.tsx:133`：**剔掉 pricing_only**，再按本地化名排序。
  List<String> get platformCodes {
    final codes = byPlatform.keys.where((c) => !pricingOnly.contains(c)).toList();
    codes.sort((a, b) => (labelMap[a] ?? a).compareTo(labelMap[b] ?? b));
    return codes;
  }

  bool get hasFilter =>
      query.trim().isNotEmpty ||
      platformFilter.isNotEmpty ||
      capabilityFilter.isNotEmpty ||
      officialOnly;

  /// `ModelInfoTab.tsx:142`：四个条件依次短路，关键字匹配展示名 / canonical / 任一 model_id。
  List<ModelEntryGroup> get filtered {
    final q = query.trim().toLowerCase();
    return groups.where((g) {
      if (platformFilter.isNotEmpty &&
          !g.entries.any((e) => e.platformCode == platformFilter)) {
        return false;
      }
      if (capabilityFilter.isNotEmpty &&
          !g.entries.any((e) => e.capabilities.contains(capabilityFilter))) {
        return false;
      }
      if (officialOnly && !g.entries.any((e) => e.official)) return false;
      if (q.isEmpty) return true;
      if (g.displayName.toLowerCase().contains(q)) return true;
      if (g.canonicalModel.toLowerCase().contains(q)) return true;
      return g.entries.any((e) => e.modelId.toLowerCase().contains(q));
    }).toList();
  }

  int get totalPages {
    final n = (filtered.length / pageSize).ceil();
    return n < 1 ? 1 : n;
  }

  int get currentPage => page < totalPages ? page : totalPages;

  List<ModelEntryGroup> get pageRows {
    final list = filtered;
    final from = (currentPage - 1) * pageSize;
    if (from >= list.length) return const [];
    final to = currentPage * pageSize;
    return list.sublist(from, to > list.length ? list.length : to);
  }

  ModelEntryGroup? get selectedGroup {
    for (final g in groups) {
      if (g.canonicalModel == selected) return g;
    }
    return null;
  }

  /// `ModelInfoTab.tsx:86::load` + 两个 mount effect。
  /// 平台本地化名来自 `get_defaults_json`（registry 读取层），失败就留空 map 回落裸 code。
  Future<void> init(String language) async {
    await Future.wait([load(), _loadSettings(), loadLabels(language)]);
  }

  Future<void> load() async {
    loading = true;
    onChanged();
    try {
      final raw = await invoke('model_info_snapshot');
      snapshot = ModelInfoSnapshot.fromJson((raw as Map).cast<String, Object?>());
    } catch (e) {
      message = '$e';
    }
    loading = false;
    onChanged();
  }

  Future<void> _loadSettings() async {
    try {
      final raw = await invoke('price_sync_settings_get');
      settings = PriceSyncSettings.fromJson((raw as Map).cast<String, Object?>());
    } catch (_) {
      // 用默认值（React: `.catch(() => {})`）。
    }
    onChanged();
  }

  /// `getProtocolLabelMap(language)`：`get_defaults_json` 返的是**字符串**，
  /// 里面 `protocols.<code>.name` 是 8 locale 的 map。
  Future<void> loadLabels(String language) async {
    try {
      final raw = await invoke('get_defaults_json');
      final doc = jsonDecode(raw as String);
      final protocols = (doc as Map)['protocols'];
      if (protocols is! Map) return;
      final out = <String, String>{};
      for (final e in protocols.entries) {
        final name = (e.value as Map?)?['name'];
        if (name is Map) {
          final v = name[language] ?? name['en-US'];
          if (v is String && v.isNotEmpty) out['${e.key}'] = v;
        }
      }
      labelMap = out;
      onChanged();
    } catch (_) {
      // 拿不到就留空 map，展示层回落裸 code —— 与 React 的 `labelMap[c] ?? c` 同义。
    }
  }

  String platformLabel(String code) => labelMap[code] ?? code;

  /// `ModelInfoTab.tsx:169::handleSync`。同步完**重读设置再整页重载**。
  Future<void> sync() async {
    syncing = true;
    message = '';
    onChanged();
    try {
      final raw = await invoke('model_price_sync');
      syncResult = PriceSyncResult.fromJson((raw as Map).cast<String, Object?>());
      await _loadSettings();
      await load();
    } catch (e) {
      message = '$e';
    } finally {
      syncing = false;
      onChanged();
    }
  }

  /// `ModelInfoTab.tsx:185::updateSettings`。**先本地生效再落盘**，落盘失败只报错不回滚
  /// （React 同）。
  Future<void> updateSettings(PriceSyncSettings next) async {
    settings = next;
    onChanged();
    try {
      await invoke('price_sync_settings_set', {'settings': next.toJson()});
    } catch (e) {
      message = '$e';
      onChanged();
    }
  }

  /// 筛选任一维度变化都把页码打回 1（`ModelInfoTab.tsx:155` 的 effect）。
  void _resetPage() {
    page = 1;
  }

  void setQuery(String v) {
    query = v;
    _resetPage();
    onChanged();
  }

  void setPlatformFilter(String v) {
    platformFilter = v;
    _resetPage();
    onChanged();
  }

  void setCapabilityFilter(String v) {
    capabilityFilter = v;
    _resetPage();
    onChanged();
  }

  void setOfficialOnly(bool v) {
    officialOnly = v;
    _resetPage();
    onChanged();
  }

  void setPageSize(int v) {
    pageSize = v;
    _resetPage();
    onChanged();
  }

  void setPage(int v) {
    page = v;
    onChanged();
  }

  void setJumpPage(String v) {
    jumpPage = v;
    onChanged();
  }

  /// `ModelInfoTab.tsx:200::handleJumpPage`。越界什么都不做（输入框也不清）。
  void jump() {
    final p = int.tryParse(jumpPage);
    if (p != null && p >= 1 && p <= totalPages) {
      page = p;
      jumpPage = '';
      onChanged();
    }
  }

  /// `ModelInfoTab.tsx:193::clearFilter`。
  void clearFilter() {
    query = '';
    platformFilter = '';
    capabilityFilter = '';
    officialOnly = false;
    _resetPage();
    onChanged();
  }

  void setTab(String v) {
    tab = v;
    onChanged();
  }

  void select(String? canonical) {
    selected = canonical;
    onChanged();
  }

  void selectPlatform(String code) {
    activePlatform = code;
    onChanged();
  }

  /// `PlatformPane`（`ModelInfoTab.tsx:388`）：平台维度左栏用**同一个关键字**过滤，
  /// 匹配的是 code 子串或本地化名子串（后者不分大小写）。
  List<String> get visiblePlatformCodes {
    final q = query.trim().toLowerCase();
    if (q.isEmpty) return platformCodes;
    return platformCodes
        .where(
          (c) =>
              c.contains(q) || (labelMap[c] ?? '').toLowerCase().contains(q),
        )
        .toList();
  }

  /// 右栏条目：没选平台（或选的平台已不在数据里）→ 空。
  List<ModelEntry> get activePlatformEntries {
    final map = byPlatform;
    final cur = activePlatform;
    if (cur == null || !map.containsKey(cur)) return const [];
    return map[cur] ?? const [];
  }

  /// `ModelDetailDialog.tsx:32`：**可选平台在前，pricing_only 排最后**。
  /// 稳定排序（Dart 的 `sort` 不保证稳定，故用带下标的比较）。
  List<ModelEntry> detailEntries(ModelEntryGroup g) {
    final indexed = [
      for (var i = 0; i < g.entries.length; i++) (i, g.entries[i]),
    ];
    indexed.sort((a, b) {
      final d =
          (pricingOnly.contains(a.$2.platformCode) ? 1 : 0) -
          (pricingOnly.contains(b.$2.platformCode) ? 1 : 0);
      return d != 0 ? d : a.$1 - b.$1;
    });
    return [for (final e in indexed) e.$2];
  }

  /// `ModelDetailDialog.tsx:39::tabKey`。`platform_code/model_id` 是 model_entry 主键，
  /// 天然唯一 —— 只用 platform_code 会让同平台多 SKU 的 tab 互相覆盖（已修过的 bug）。
  static String detailTabKey(ModelEntry e) => '${e.platformCode}/${e.modelId}';
}
