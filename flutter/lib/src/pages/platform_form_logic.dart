/// 平台「新增 / 编辑」表单的状态与动作层，对齐 `src/pages/platforms/usePlatformForm.ts`
/// （780 行）加上 `formSections.tsx` 里几个分区各自持有的局部态
/// （套餐档位自动填入、配额变体回填、时段矩阵的列头编辑）。
///
/// 为什么是一个独立控制器而不是塞进 [PlatformsController]：
/// React 那边也是两个 hook（`usePlatformsState` 管列表、`usePlatformForm` 管表单），
/// 表单态有三十来个字段，混进列表控制器会让「乐观写 + epoch」那套竞争逻辑没法读。
/// 列表侧的依赖（平台名集合、分组清单、真正的落库动作）经构造参数注入，
/// 与 TS 的 `PlatformFormListDeps` 一一对应。
library;

import 'invoke.dart';
import 'models.dart';
import 'platform_defaults.dart';
import 'platform_extra.dart';
import 'platform_paste_logic.dart';
import 'platforms_logic.dart';
import 'time_window.dart';

/// 五个槽位的空映射（`usePlatformForm.ts:195` 的初值）。
Map<String, String> emptyModelSlots() => {
  'default': '',
  'sonnet': '',
  'opus': '',
  'haiku': '',
  'gpt': '',
};

class PlatformFormController {
  PlatformFormController({required this.list, InvokeFn? invoke, this.onChanged})
    : _invoke = invoke ?? kernelInvoke;

  /// 列表侧控制器（平台名集合 / 分组清单 / 真正的落库动作都在它身上）。
  final PlatformsController list;
  final InvokeFn _invoke;

  /// 给要自己发命令的子 widget 用（智能识别弹窗探测分享串走 `platform_share_parse`）。
  InvokeFn get invoke => _invoke;

  /// 重绘回调。非 final：页面可能在控制器造好之后才拿到 `setState`
  /// （widget 测试就是这么挂的）。
  void Function()? onChanged;

  // ── 表单开合 ────────────────────────────────────────────────────
  PlatformRow? editing;
  bool showForm = false;

  // ── 字段 ────────────────────────────────────────────────────────
  String name = '';
  String protocol = 'openai';
  bool codingPlan = false;
  String apiKey = '';
  bool showKey = false;

  /// 多 key 预览态：null = 非批量；非空 = 待创建的 key 列表（长度 >1 才渲染预览）。
  List<String>? batchPreviewKeys;

  Map<String, String> models = emptyModelSlots();
  List<String> availableModels = const [];
  List<PlatformEndpoint> endpoints = const [];

  /// 原始 extra（保存时以它为底，改完自己的键再写回，不吃掉别人的键）。
  String extra = '';
  MockConfig mockConfig = kDefaultMockConfig;

  String quotaVariantId = '';
  String quotaCustomScript = '';
  Map<String, String> quotaRequires = {};

  /// 配额方式（quota-ia 票 01/03）：manual = 手动预算（脚本侧不生效）。
  /// 显式值（`_quotaSourcePristine=false` 或编辑态）；创建态未切换时走
  /// [effectiveQuotaSource] 按协议能力派生（有变体→auto 保持开箱即查，无→manual）。
  String quotaSource = 'auto';
  bool _quotaSourcePristine = true;

  /// 显式切换（UI tab）：一经用户选择即停止自动落位。
  void setQuotaSource(String v) {
    quotaSource = v;
    _quotaSourcePristine = false;
    _notify();
  }

  /// 展示/保存用的生效值：pristine 创建态随协议能力派生（registry 变体异步到达也即时正确，
  /// 不依赖同步时机）；编辑态与显式切换后用 [quotaSource] 本值。
  String get effectiveQuotaSource {
    if (editing != null || !_quotaSourcePristine) return quotaSource;
    return quotaVariants.isNotEmpty ? 'auto' : 'manual';
  }

  DevinConfig devinConfig = kDefaultDevinConfig;

  List<ManualBudget> manualBudgets = const [];

  /// 套餐档位下拉的选中 id（`ManualBudgetsSection` 的局部态）。
  String planTierId = '';

  /// 空串 = 继承全局默认（`usePlatformForm.ts:215`）。
  String breakerFailureThreshold = '';
  String breakerOpenSecs = '';
  String breakerHalfOpenMax = '';

  /// 全局熔断默认（列表控制器在 `init()` 时拉的，表单只读它显示 placeholder）。
  BreakerDefaults? get breakerDefaults => list.breakerDefaults;

  List<TimeWindow> peak = const [];
  TzMode windowsTz = TzMode.local;
  bool disableDuringPeak = false;
  List<TimeModelRule> timeModels = const [];

  bool autoGroup = true;
  List<int> joinGroupIds = const [];
  int? lockedGroupId;

  /// 毫秒 unix 时间戳，0 = 永不过期。
  int expiresAt = 0;
  bool expiryEnabled = false;

  bool fetching = false;
  String fetchError = '';
  String saveError = '';
  bool saving = false;

  PlatformDefaults defaults = PlatformDefaults.empty;
  Map<String, String> protocolLabelMap = const {};

  /// 「本协议的套餐档位已经自动填过一次」—— 用户清空后切回本协议不再回填
  /// （`formSections.tsx:386` 的 `autofilledFor` ref）。
  String? _autofilledFor;

  void _notify() => onChanged?.call();

  // ── 派生 ────────────────────────────────────────────────────────

  /// `usePlatformForm.ts:236`。
  bool get isMock => protocol == 'mock';

  /// Claude Code 订阅纯透传（`usePlatformForm.ts:239`）。
  bool get isPassthrough => protocol == 'claude_code';

  /// OpenCode Zen 免费匿名访问（`usePlatformForm.ts:241`）。
  bool get keyOptional => protocol == 'opencode_zen';

  /// 要 key 但没填 —— 「获取模型」按钮的禁用判据（`usePlatformForm.ts:243`）。
  bool get apiKeyMissing => !keyOptional && apiKey.isEmpty;

  /// 保存按钮的启用条件，就是 `PlatformEditForm.tsx:118-119` 那个 `disabled=` 取反。
  bool get canSave {
    if (name.isEmpty) return false;
    if (isPassthrough) return endpoints.isNotEmpty;
    if (isMock || keyOptional) return true;
    return endpoints.isNotEmpty && apiKey.isNotEmpty;
  }

  /// 批量创建态（预览卡渲染 + 保存按钮文案的判据）。
  bool get isBatch => batchPreviewKeys != null && batchPreviewKeys!.length > 1;

  /// `usePlatformForm.ts:598::previewNames` —— 撞名基准 = 当前平台名集合。
  List<String> get previewNames {
    final keys = batchPreviewKeys;
    if (keys == null || keys.isEmpty) return const [];
    return previewBatchNames(keys, name, {
      for (final p in list.platforms) p.name,
    });
  }

  /// 本协议的配额脚本变体（registry `quota_scripts`）。
  List<QuotaScriptVariant> get quotaVariants =>
      defaults.defaultQuotaScripts(protocol);

  /// 本协议的官方套餐档位。
  List<PlanQuotaTier> get planTiers => defaults.defaultPlanQuotas(protocol);

  /// 本协议的 preset 默认高峰窗口（「导入默认配置」的来源）。
  List<TimeWindow> get presetPeak => defaults.defaultPeak(protocol);

  /// 模型下拉候选：拉到过的 available 优先，否则回落 registry 的 model_list。
  List<String> get modelDropdownSource => availableModels.isNotEmpty
      ? availableModels
      : defaults.defaultModelList(protocol);

  /// 端点是否锁死（厂商直连平台，只读展示）。
  bool get endpointsLocked => kEndpointsLockedProtocols.contains(protocol);

  /// 配额脚本区的当前选中值（与后端 `resolve_quota_script` 对齐：
  /// custom 优先 → 显式 id → 回落首条）。`formSections.tsx:182`。
  String get quotaSelection {
    final hasCustom = quotaCustomScript.trim().isNotEmpty;
    final customOnly = quotaVariants.isEmpty;
    if (hasCustom || customOnly || quotaVariantId == kQuotaCustomVariant) {
      return kQuotaCustomVariant;
    }
    final valid = quotaVariants.any((v) => v.id == quotaVariantId);
    if (valid) return quotaVariantId;
    return quotaVariants.isNotEmpty ? quotaVariants.first.id : '';
  }

  /// 存量 id 失效（远程改名/删条）→ 已回落首条，提示用户重选。`formSections.tsx:186`。
  bool get quotaFellBack =>
      quotaCustomScript.trim().isEmpty &&
      quotaVariantId != kQuotaCustomVariant &&
      quotaVariantId.isNotEmpty &&
      !quotaVariants.any((v) => v.id == quotaVariantId);

  /// 当前选中的变体对象（选了自定义伪变体则为 null）。
  QuotaScriptVariant? get selectedQuotaVariant {
    final sel = quotaSelection;
    if (sel == kQuotaCustomVariant) return null;
    for (final v in quotaVariants) {
      if (v.id == sel) return v;
    }
    return null;
  }

  // ── 初始化 ──────────────────────────────────────────────────────

  /// 拉 registry 派生层（协议清单 / 默认端点 / 客户端模拟候选…）。
  /// best-effort：拉不到就是空文档，表单照常能开。
  Future<void> init({String? locale}) async {
    final d = await loadPlatformDefaults(_invoke, locale);
    defaults = d;
    protocolLabelMap = d.protocolLabelMap(locale);
    _notify();
  }

  // ── 开 / 关表单 ────────────────────────────────────────────────

  /// `usePlatformForm.ts:314::resetForm`。
  void resetForm() {
    name = '';
    protocol = 'openai';
    codingPlan = false;
    apiKey = '';
    batchPreviewKeys = null;
    models = emptyModelSlots();
    availableModels = const [];
    endpoints = const [];
    editing = null;
    showForm = false;
    fetchError = '';
    saveError = '';
    extra = '';
    mockConfig = kDefaultMockConfig;
    quotaVariantId = '';
    quotaCustomScript = '';
    quotaSource = 'auto';
    _quotaSourcePristine = true;
    quotaRequires = {};
    devinConfig = kDefaultDevinConfig;
    manualBudgets = const [];
    planTierId = '';
    _autofilledFor = null;
    breakerFailureThreshold = '';
    breakerOpenSecs = '';
    breakerHalfOpenMax = '';
    peak = const [];
    windowsTz = TzMode.local;
    disableDuringPeak = false;
    timeModels = const [];
    autoGroup = true;
    joinGroupIds = const [];
    lockedGroupId = null;
    expiresAt = 0;
    expiryEnabled = false;
    saving = false;
    _notify();
  }

  /// `usePlatformForm.ts:335::openCreatePlatform`。
  /// [lockGid] 给定时预绑该分组并锁定、关掉 auto_group。
  void openCreatePlatform({List<int>? presetGroupIds, int? lockGid}) {
    resetForm();
    if (lockGid != null) {
      autoGroup = false;
      joinGroupIds = (presetGroupIds != null && presetGroupIds.isNotEmpty)
          ? presetGroupIds
          : [lockGid];
      lockedGroupId = lockGid;
    }
    showForm = true;
    _syncQuotaRequires();
    _syncPlanTiers();
    _notify();
  }

  /// `usePlatformForm.ts:351::handleEdit`。
  void handleEdit(PlatformRow p) {
    _fillFrom(p);
    editing = p;
    // 编辑态：加入的「已有分组」= 该平台所在的非 auto 分组。
    joinGroupIds = [
      for (final gd in list.groupDetails)
        if (gd.group.autoFromPlatform != '${p.id}' && gd.hasPlatform(p.id))
          gd.group.id,
    ];
    _syncQuotaRequires();
    _syncPlanTiers();
    _notify();
  }

  /// `usePlatformForm.ts:405::handleDuplicate` —— 与 [handleEdit] 唯一差别是
  /// `editing` 留 null（不绑源平台 id，保存时走 create）。
  void handleDuplicate(PlatformRow p) {
    _fillFrom(p);
    editing = null;
    joinGroupIds = [
      for (final gd in list.groupDetails)
        if (gd.group.autoFromPlatform != '${p.id}' && gd.hasPlatform(p.id))
          gd.group.id,
    ];
    _syncQuotaRequires();
    _syncPlanTiers();
    _notify();
  }

  /// 智能识别弹窗点「填入表单」之后把结果灌进表单。
  /// `platformPasteApply.ts:78::applyPaste`。
  ///
  /// 三条路各走各的：
  ///   1. 命中 aidog 分享串（[SmartPasteApplyResult.fullShare]）→ 整体灌，以**新建态**打开；
  ///   2. 多 key → 灌表单 + 置 [batchPreviewKeys] 触发批量预览（**不立刻创建**，由预览区确认）；
  ///   3. 单 key / 无 key → 只填 base_url 与 key。
  void applyPaste(SmartPasteApplyResult r) {
    final share = r.fullShare;
    if (share != null) {
      String s(String k) => (share[k] as String?) ?? '';
      final eps = [
        for (final e in (share['endpoints'] as List? ?? const []))
          if (e is Map) PlatformEndpoint.fromJson(e.cast<String, dynamic>()),
      ];
      name = s('name');
      protocol = s('platform_type');
      // 分享串只含一个 api_key，这条路保持单平台行为。
      apiKey = s('api_key');
      codingPlan = eps.any((e) => e.codingPlan);
      final m = share['models'];
      models = {
        ...emptyModelSlots(),
        if (m is Map)
          for (final slot in emptyModelSlots().keys)
            if (m[slot] is String) slot: m[slot] as String,
      };
      availableModels = [
        for (final e in (share['available_models'] as List? ?? const []))
          if (e is String) e,
      ];
      endpoints = eps;
      manualBudgets = [
        for (final e in (share['manual_budgets'] as List? ?? const []))
          if (e is Map) ManualBudget.fromJson(e.cast<String, dynamic>()),
      ];
      extra = s('extra');
      mockConfig = parseMockConfig(extra);
      final qs = parseQuotaScriptConfig(extra);
      quotaVariantId = qs.variantId;
      quotaCustomScript = qs.customScript;
      quotaRequires = {};
      devinConfig = parseDevinConfig(extra);
      final brk = parsePlatformBreaker(extra);
      breakerFailureThreshold = brk.failureThreshold > 0
          ? '${brk.failureThreshold}'
          : '';
      breakerOpenSecs = brk.openSecs > 0 ? '${brk.openSecs}' : '';
      breakerHalfOpenMax = brk.halfOpenMax > 0 ? '${brk.halfOpenMax}' : '';
      editing = null;
      lockedGroupId = null;
      joinGroupIds = const [];
      fetchError = '';
      saveError = '';
      batchPreviewKeys = null;
      showForm = true;
      _syncQuotaRequires();
      _syncPlanTiers();
      _notify();
      return;
    }

    // 命中内置平台 → 走协议切换（顺带填上 name / 默认 endpoints / client_type）。
    // 没命中 → 不动平台选择，只填 base_url 与 key。
    final hit = r.platform;
    if (hit != null) {
      handleProtocolChange(hit.value, newCodingPlan: hit.codingPlan);
    }

    // 命中平台时用「该平台的默认 endpoints」当底（上面刚填进 endpoints），
    // 否则用当前表单里的。两条路都在这一刻取值，不存在读到旧态的问题。
    final merged = computePastedEndpoints(
      endpoints,
      r.baseUrls,
      platformMatched: hit != null,
    );

    if (r.apiKeys.length > 1) {
      // 多 key：apiKey 灌成多行文本（用户看得见，预览区再 splitApiKeys 拆回来）。
      apiKey = r.apiKeys.join('\n');
      endpoints = merged;
      batchPreviewKeys = r.apiKeys;
    } else {
      // 单 key / 无 key：清预览态，免得上一次多 key 的预览残留。
      batchPreviewKeys = null;
      if (r.baseUrls.isNotEmpty) endpoints = merged;
      if (r.apiKeys.length == 1) apiKey = r.apiKeys.first;
    }

    // 识别到过期时间就顺手把开关打开，否则 toggle 默认关着、字段藏起来，
    // 用户会以为「没识别到过期时间」。
    if (r.expiresAt > 0) {
      expiresAt = r.expiresAt;
      expiryEnabled = true;
    }
    // 弹窗可能从主列表直达（表单还没挂上），这里显式拉起表单展示已填字段。
    showForm = true;
    _notify();
  }

  void _fillFrom(PlatformRow p) {
    name = p.name;
    protocol = p.platformType;
    apiKey = p.apiKey;
    codingPlan = p.endpoints.any((ep) => ep.codingPlan);
    models = {
      'default': p.models.defaultModel ?? '',
      'sonnet': p.models.sonnet ?? '',
      'opus': p.models.opus ?? '',
      'haiku': p.models.haiku ?? '',
      'gpt': p.models.gpt ?? '',
    };
    availableModels = [...p.availableModels];
    endpoints = [...p.endpoints];
    showForm = true;
    fetchError = '';
    saveError = '';
    batchPreviewKeys = null;
    extra = p.extra;
    mockConfig = parseMockConfig(p.extra);
    final qs = parseQuotaScriptConfig(p.extra);
    quotaVariantId = qs.variantId;
    quotaCustomScript = qs.customScript;
    quotaRequires = {};
    // 配额方式（quota-ia 票 02）：存量 '' 读侧当 auto；编辑即定位，不随协议能力漂移。
    quotaSource = p.quotaSource == 'manual' ? 'manual' : 'auto';
    _quotaSourcePristine = false;
    manualBudgets = const [];
    _autofilledFor = null;
    // 老平台 expires_at>0 → toggle 默认 ON；=0/未设 → OFF。
    expiresAt = p.expiresAt;
    expiryEnabled = p.expiresAt > 0;
    final brk = parsePlatformBreaker(p.extra);
    breakerFailureThreshold = brk.failureThreshold > 0
        ? '${brk.failureThreshold}'
        : '';
    breakerOpenSecs = brk.openSecs > 0 ? '${brk.openSecs}' : '';
    breakerHalfOpenMax = brk.halfOpenMax > 0 ? '${brk.halfOpenMax}' : '';
    peak = parsePlatformPeak(p.extra);
    disableDuringPeak = parseDisableDuringPeak(p.extra);
    timeModels = parsePlatformTimeWindows(p.extra);
    devinConfig = parseDevinConfig(p.extra);
    lockedGroupId = null;
    windowsTz = TzMode.local;
    autoGroup = true;
    saving = false;
  }

  // ── 字段动作 ────────────────────────────────────────────────────

  void setName(String v) {
    name = v;
    _notify();
  }

  /// `usePlatformForm.ts:586::handleApiKeyChange`：创建态 + 非 keyOptional +
  /// 多 key → 触发实时预览；编辑态 / keyOptional / 单 key → 清预览。
  void setApiKey(String v) {
    apiKey = v;
    if (editing == null && !keyOptional) {
      final keys = splitApiKeys(v);
      batchPreviewKeys = keys.length > 1 ? keys : null;
    } else {
      batchPreviewKeys = null;
    }
    _notify();
  }

  void toggleShowKey() {
    showKey = !showKey;
    _notify();
  }

  /// `usePlatformForm.ts:273::handleProtocolChange`。
  void handleProtocolChange(String newProtocol, {bool newCodingPlan = false}) {
    final label =
        protocolLabelMap[newProtocol] ??
        kProtocolLabels[newProtocol] ??
        newProtocol;
    // 「name 仍是协议默认名」→ 切协议时自动覆盖。默认名集合 = 5 个请求格式协议
    // 的静态 label + 运行时拉到的全部平台 name（`constants.ts:55::DEFAULT_NAMES`）。
    final isDefaultName =
        name.trim().isEmpty ||
        kProtocolLabels.values.contains(name) ||
        protocolLabelMap.values.contains(name);
    if (isDefaultName) name = label;

    endpoints = defaults.defaultEndpoints(newProtocol);
    models = {...emptyModelSlots(), ...defaults.defaultModels(newProtocol)};
    if (newProtocol == 'mock') mockConfig = parseMockConfig(extra);
    if (newProtocol == 'devin') devinConfig = parseDevinConfig(extra);
    quotaVariantId = '';
    quotaCustomScript = '';
    quotaRequires = {};
    protocol = newProtocol;
    codingPlan = newCodingPlan;
    _syncQuotaRequires();
    _syncPlanTiers();
    _notify();
  }

  /// `usePlatformForm.ts:268::handleQuotaVariantChange`：切回 registry 变体时
  /// 清自定义正文，保持互斥。
  void handleQuotaVariantChange(String v) {
    quotaVariantId = v;
    if (v != kQuotaCustomVariant) quotaCustomScript = '';
    _syncQuotaRequires();
    _notify();
  }

  void setQuotaCustomScript(String v) {
    quotaCustomScript = v;
    _notify();
  }

  void setQuotaRequire(String key, String value) {
    quotaRequires = {...quotaRequires, key: value};
    _notify();
  }

  /// `usePlatformForm.ts:253` 的 requires 初值回填 effect：为选中变体缺失的
  /// requires key 从 extra 读初值（嵌套优先）。**用户已输入的键不覆盖**。
  void _syncQuotaRequires() {
    final sel =
        selectedQuotaVariant ??
        (quotaVariants.isNotEmpty ? quotaVariants.first : null);
    final keys = [...?sel?.requires.map((r) => r.key)];
    if (protocol == 'newapi' && !keys.contains('user_id')) keys.add('user_id');
    final next = {...quotaRequires};
    var changed = false;
    for (final k in keys) {
      if (!next.containsKey(k)) {
        next[k] = readRequiresValue(extra, k);
        changed = true;
      }
    }
    if (changed) quotaRequires = next;
  }

  /// `formSections.tsx:388` 的套餐档位 effect：换协议时重置下拉选中，
  /// 创建态且本协议还没自动填过 → 预算为空时填入首档。
  void _syncPlanTiers() {
    final tiers = planTiers;
    planTierId = tiers.isNotEmpty ? tiers.first.id : '';
    if (editing == null && tiers.isNotEmpty && _autofilledFor != protocol) {
      _autofilledFor = protocol;
      if (manualBudgets.isEmpty) manualBudgets = tierToBudgets(tiers.first);
    }
  }

  /// `formSections.tsx:363::tierToBudgets`：内置档位 → 一组新预算条目
  /// （各条独立 id，consumed 从 0 起算）。
  static List<ManualBudget> tierToBudgets(PlanQuotaTier tier) => [
    for (final b in tier.budgets)
      newManualBudget().copyWith(
        kind: b.kind,
        unit: b.unit,
        amount: b.amount,
        windowHours: b.windowHours,
        windowUnit: b.windowUnit ?? 'hour',
        clearWindowHours: b.windowHours == null,
      ),
  ];

  void setPlanTierId(String id) {
    planTierId = id;
    _notify();
  }

  PlanQuotaTier? get selectedPlanTier {
    final tiers = planTiers;
    for (final x in tiers) {
      if (x.id == planTierId) return x;
    }
    return tiers.isNotEmpty ? tiers.first : null;
  }

  void setMockConfig(MockConfig cfg) {
    mockConfig = cfg;
    _notify();
  }

  void setDevinConfig(DevinConfig cfg) {
    devinConfig = cfg;
    _notify();
  }

  void setEndpoints(List<PlatformEndpoint> next) {
    endpoints = next;
    _notify();
  }

  /// `formSectionsEndpoints.tsx:54`：新端点默认 openai + 派生 client_type。
  void addEndpoint() {
    setEndpoints([
      ...endpoints,
      PlatformEndpoint(
        protocol: 'openai',
        baseUrl: '',
        clientType: defaultClientForProtocol('openai'),
        codingPlan: false,
      ),
    ]);
  }

  /// `formSectionsEndpoints.tsx:71`：改端点协议会**连带重置 client_type**
  /// （派生值），否则下拉留着上一个协议的模拟形态。
  void setEndpointProtocol(int idx, String proto) {
    final next = [...endpoints];
    next[idx] = PlatformEndpoint(
      protocol: proto,
      baseUrl: next[idx].baseUrl,
      clientType: defaultClientForProtocol(proto),
      codingPlan: next[idx].codingPlan,
    );
    setEndpoints(next);
  }

  void setEndpointBaseUrl(int idx, String url) {
    final next = [...endpoints];
    next[idx] = PlatformEndpoint(
      protocol: next[idx].protocol,
      baseUrl: url,
      clientType: next[idx].clientType,
      codingPlan: next[idx].codingPlan,
    );
    setEndpoints(next);
  }

  void setEndpointClientType(int idx, String clientType) {
    final next = [...endpoints];
    next[idx] = PlatformEndpoint(
      protocol: next[idx].protocol,
      baseUrl: next[idx].baseUrl,
      clientType: clientType,
      codingPlan: next[idx].codingPlan,
    );
    setEndpoints(next);
  }

  void toggleEndpointCodingPlan(int idx) {
    final next = [...endpoints];
    next[idx] = PlatformEndpoint(
      protocol: next[idx].protocol,
      baseUrl: next[idx].baseUrl,
      clientType: next[idx].clientType,
      codingPlan: !next[idx].codingPlan,
    );
    setEndpoints(next);
  }

  void removeEndpoint(int idx) {
    setEndpoints([
      for (var i = 0; i < endpoints.length; i++)
        if (i != idx) endpoints[i],
    ]);
  }

  /// `formSections.tsx:336`：透传平台只有一个 base_url 输入；没端点就现造一条。
  void setPassthroughBaseUrl(String url) {
    final next = [...endpoints];
    if (next.isEmpty) {
      next.add(
        PlatformEndpoint(
          protocol: 'anthropic',
          baseUrl: url,
          clientType: 'default',
          codingPlan: false,
        ),
      );
    } else {
      next[0] = PlatformEndpoint(
        protocol: next[0].protocol,
        baseUrl: url,
        clientType: next[0].clientType,
        codingPlan: next[0].codingPlan,
      );
    }
    setEndpoints(next);
  }

  void setModel(String slot, String value) {
    models = {...models, slot: value};
    _notify();
  }

  /// `usePlatformForm.ts:540::handleFillAll`：把 default 填到其余四槽（覆盖已有值）。
  void handleFillAll() {
    final d = models['default']!.trim();
    if (d.isEmpty) return;
    final next = {...models};
    for (final slot in kModelSlots) {
      if (slot.key != 'default') next[slot.key] = d;
    }
    models = next;
    _notify();
  }

  /// `usePlatformForm.ts:471::handleFetchModels`。
  /// 成功时**连带自动归类**（`autoCategorize`）覆盖 5 个槽位 —— 与 React 一致。
  Future<void> handleFetchModels({
    String Function(int code)? authText,
    String? emptyText,
  }) async {
    if (apiKeyMissing) return;
    fetching = true;
    fetchError = '';
    _notify();
    final (ids, err) = await list.fetchModels(
      protocol: protocol,
      apiKey: apiKey,
      endpoints: endpoints,
      emptyText: emptyText,
      authText: authText,
    );
    if (ids.isNotEmpty) {
      availableModels = ids;
      models = {...emptyModelSlots(), ...autoCategorize(ids)};
      fetchError = '';
    } else if (err != null) {
      fetchError = err;
    }
    fetching = false;
    _notify();
  }

  void setManualBudgets(List<ManualBudget> next) {
    manualBudgets = next;
    _notify();
  }

  void setBreakerFailureThreshold(String v) {
    breakerFailureThreshold = v;
    _notify();
  }

  void setBreakerOpenSecs(String v) {
    breakerOpenSecs = v;
    _notify();
  }

  void setBreakerHalfOpenMax(String v) {
    breakerHalfOpenMax = v;
    _notify();
  }

  void setPeak(List<TimeWindow> next) {
    peak = next;
    _notify();
  }

  void setWindowsTz(TzMode mode) {
    windowsTz = mode;
    _notify();
  }

  void setDisableDuringPeak(bool v) {
    disableDuringPeak = v;
    _notify();
  }

  void setTimeModels(List<TimeModelRule> next) {
    timeModels = next;
    _notify();
  }

  void setAutoGroup(bool v) {
    autoGroup = v;
    _notify();
  }

  void toggleJoinGroup(int gid) {
    joinGroupIds = joinGroupIds.contains(gid)
        ? [
            for (final id in joinGroupIds)
              if (id != gid) id,
          ]
        : [...joinGroupIds, gid];
    _notify();
  }

  void setExpiresAt(int ms) {
    expiresAt = ms;
    _notify();
  }

  /// `formSections.tsx:1126`：ON→OFF 清零 expiresAt；OFF→ON 保留已有值。
  void setExpiryEnabled(bool v) {
    expiryEnabled = v;
    if (!v) expiresAt = 0;
    _notify();
  }

  // ── 保存 ────────────────────────────────────────────────────────

  /// `usePlatformForm.ts:607::buildSharedCreateFields` 的 extra 链，顺序不可换：
  /// mock → devin → quota 脚本（须在 devin 之后，org_id 会镜像写 extra.devin）
  /// → breaker → peak → disable_during_peak → time_windows。
  String buildExtraPayload() {
    var out = extra;
    if (isMock) out = serializeMockConfig(out, mockConfig);
    if (protocol == 'devin') out = serializeDevinConfig(out, devinConfig);
    out = serializeQuotaScriptConfig(
      out,
      variantId: quotaVariantId,
      customScript: quotaCustomScript,
      requires: quotaRequires,
      variants: [
        for (final v in quotaVariants)
          (id: v.id, requires: [for (final r in v.requires) r.key]),
      ],
      protocol: protocol,
    );
    int toBreakerNum(String v) {
      final n = num.tryParse(v.trim()) ?? 0;
      final f = n.floor();
      return f < 0 ? 0 : f;
    }

    out = serializePlatformBreaker(
      out,
      PlatformBreaker(
        failureThreshold: toBreakerNum(breakerFailureThreshold),
        openSecs: toBreakerNum(breakerOpenSecs),
        halfOpenMax: toBreakerNum(breakerHalfOpenMax),
      ),
    );
    out = serializePlatformPeak(out, peak);
    out = serializeDisableDuringPeak(out, disableDuringPeak);
    out = serializePlatformTimeWindows(out, timeModels);
    return out;
  }

  /// 纯透传平台没有手动预算的概念（`usePlatformForm.ts:636`）。
  List<Map<String, Object?>> get _budgetPayload =>
      isPassthrough ? const [] : [for (final b in manualBudgets) b.toJson()];

  /// `usePlatformForm.ts:680::handleSave`。多 key 预览态直接走批量创建。
  ///
  /// 返回 true = 保存成功（表单已关）；false = 失败（[saveError] 已填）。
  Future<bool> handleSave({
    String saveFailText = '保存失败',
    String Function(int done, int total)? progressText,
    String Function(int n)? allOkText,
    String Function(int ok, int fail)? summaryText,
    String noBaseUrlText = '批量创建失败：未设置 Base URL',
  }) async {
    saveError = '';
    if (isBatch) {
      await _runBatchCreate(
        batchPreviewKeys!,
        progressText: progressText,
        allOkText: allOkText,
        summaryText: summaryText,
        noBaseUrlText: noBaseUrlText,
      );
      return true;
    }
    saving = true;
    _notify();
    final saved = await list.savePlatform(
      name: name,
      protocol: protocol,
      apiKey: apiKey,
      models: models,
      availableModels: availableModels,
      endpoints: endpoints,
      extra: buildExtraPayload(),
      joinGroupIds: joinGroupIds,
      expiresAt: expiresAt,
      manualBudgets: _budgetPayload,
      quotaSource: effectiveQuotaSource,
      autoGroup: autoGroup,
      editingId: editing?.id,
      failText: saveFailText,
      onError: (msg) => saveError = msg,
    );
    saving = false;
    if (saved == null) {
      _notify();
      return false;
    }
    resetForm();
    return true;
  }

  /// `platformPasteApply.ts:283::runBatchCreateFromPaste`（手动表单那条入口）。
  /// 串行创建，失败项不中断整批，末尾汇总；成功即关表单。
  Future<void> _runBatchCreate(
    List<String> keys, {
    String Function(int done, int total)? progressText,
    String Function(int n)? allOkText,
    String Function(int ok, int fail)? summaryText,
    required String noBaseUrlText,
  }) async {
    final prefix = (name.isEmpty ? 'Platform' : name).trim();
    final baseUrl = getPrimaryBaseUrl(protocol, endpoints);
    if (baseUrl.isEmpty && endpoints.isEmpty) {
      list.toast(noBaseUrlText, ok: false);
      return;
    }
    final joinIds = lockedGroupId != null ? [lockedGroupId!] : joinGroupIds;
    final auto = lockedGroupId != null ? false : autoGroup;
    final usedNames = {for (final p in list.platforms) p.name};
    final extraPayload = buildExtraPayload();
    final budgets = _budgetPayload;
    var okCount = 0;
    final failures = <({String key, String err})>[];
    saving = true;
    list.toast(progressText?.call(0, keys.length) ?? '', ok: true);
    for (var i = 0; i < keys.length; i++) {
      final k = keys[i];
      final pname = previewBatchNames([k], prefix, usedNames)[0];
      final saved = await list.savePlatform(
        name: pname,
        protocol: protocol,
        apiKey: k,
        models: models,
        availableModels: availableModels,
        endpoints: endpoints,
        extra: extraPayload,
        joinGroupIds: joinIds,
        expiresAt: expiresAt,
        manualBudgets: budgets,
        quotaSource: effectiveQuotaSource,
        autoGroup: auto,
        silent: true,
        onError: (msg) => failures.add((
          key: k.length >= 4 ? k.substring(k.length - 4) : k,
          err: msg,
        )),
      );
      if (saved != null) {
        usedNames.add(pname);
        okCount++;
        list.toast(progressText?.call(i + 1, keys.length) ?? '', ok: true);
      }
    }
    saving = false;
    resetForm();
    if (failures.isEmpty) {
      list.toast(allOkText?.call(okCount) ?? '', ok: true);
    } else {
      final failList = failures.map((f) => '${f.key}: ${f.err}').join('; ');
      list.toast(
        '${summaryText?.call(okCount, failures.length) ?? ''} — $failList',
        ok: okCount > 0,
      );
    }
  }
}
