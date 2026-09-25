/// registry 派生层，对齐 `src/domains/platforms/defaults.ts` + `constants.ts`。
///
/// 数据从两条命令来，**都只拉一次**（与 TS 的 `docPromise` / `clientTypesDocPromise`
/// 同一套路）：
/// - `get_defaults_json` → 协议清单 / 默认端点 / 默认模型 / 模型候选 / peak /
///   配额脚本变体 / 套餐档位 / 品牌名色 / 搜索词；
/// - `get_client_types_json` → 端点「客户端模拟」下拉候选（缺口清单 C4）。
///
/// 这里不是 UI，也不持有 UI 状态：一个可注入 [InvokeFn] 的加载器 + 一组纯派生。
library;

import 'dart:convert';

import 'invoke.dart';
import 'models.dart';
import 'time_window.dart';

/// Endpoint 协议：只有 AI 请求协议（非平台类型）。
/// `constants.ts:11::ENDPOINT_PROTOCOLS`，逐条同序同 label。
const List<({String value, String label})> kEndpointProtocols = [
  (value: 'openai', label: 'OpenAI Chat'),
  (value: 'openai_responses', label: 'OpenAI Responses'),
  (value: 'openai_completions', label: 'OpenAI Completions'),
  (value: 'anthropic', label: 'Anthropic'),
  (value: 'gemini', label: 'Gemini'),
];

/// 厂商直连平台端点锁死集合（`constants.ts:43::ENDPOINTS_LOCKED_PROTOCOLS`）。
/// 跨层对称：Rust `Protocol::endpoints_locked()` 是唯一真值，禁单侧改。
const Set<String> kEndpointsLockedProtocols = {
  'mock', 'claude_code',
  'glm', 'glm_coding', 'glm_en', 'glm_coding_en', 'kimi', 'kimi_en',
  'kimi_coding',
  'minimax', 'minimax_en', 'minimax_coding', 'codex', 'bailian', 'bailian_en',
  'bailian_coding', 'bailian_coding_en',
  'deepseek', 'stepfun', 'stepfun_en', 'doubao', 'byteplus',
  'qianfan', 'qianfan_coding', 'xiaomi_mimo', 'xiaomi_mimo_coding',
  'xiaomi_mimo_coding_en',
  'longcat', 'sensenova', 'sensenova_en', 'devin',
};

/// `constants.ts:60::MODEL_SLOTS` —— 5 个槽位 + 各自的文案 key，顺序即矩阵行序。
const List<({String key, String labelKey})> kModelSlots = [
  (key: 'default', labelKey: 'platform.modelDefault'),
  (key: 'sonnet', labelKey: 'platform.modelSonnet'),
  (key: 'opus', labelKey: 'platform.modelOpus'),
  (key: 'haiku', labelKey: 'platform.modelHaiku'),
  (key: 'gpt', labelKey: 'platform.modelGpt'),
];

/// `constants.ts:30::PROTOCOL_LABELS` —— 仅 5 条请求格式协议的兜底 label。
const Map<String, String> kProtocolLabels = {
  'openai': 'OpenAI',
  'openai_responses': 'OpenAI Responses',
  'openai_completions': 'OpenAI Completions',
  'anthropic': 'Anthropic',
  'gemini': 'Gemini',
};

/// endpoint 协议 → 默认客户端形态（`defaults.ts:259::clientTypeForProtocol`）。
/// 与 Rust `aidog_db::registry::derive_client_type` 对称。
String clientTypeForProtocol(String protocol) {
  switch (protocol) {
    case 'anthropic':
      return 'claude_code';
    case 'openai':
    case 'openai_responses':
    case 'openai_completions':
      return 'codex_tui';
    default:
      return 'default';
  }
}

/// `defaults.ts:270` 的旧名别名。
String defaultClientForProtocol(String protocol) =>
    clientTypeForProtocol(protocol);

/// i18next locale → registry `name` 的 locale key（`defaults.ts:358`）。
String normalizeDefaultsLocale(String? locale) {
  switch ((locale ?? '').trim().toLowerCase().replaceAll('_', '-')) {
    case 'zh':
    case 'zh-cn':
    case 'zh-hans':
      return 'zh-Hans';
    case 'ja':
    case 'ja-jp':
      return 'ja-JP';
    case 'fr':
    case 'fr-fr':
      return 'fr-FR';
    case 'de':
    case 'de-de':
      return 'de-DE';
    case 'ru':
    case 'ru-ru':
      return 'ru-RU';
    case 'ar':
    case 'ar-sa':
      return 'ar-SA';
    case 'es':
    case 'es-es':
      return 'es-ES';
    default:
      return 'en-US';
  }
}

/// 展示名三层回落（`defaults.ts:374::resolveName`）：
/// `name[locale]` → `name["en-US"]` → code。空白值视同缺失。
String resolveDefaultsName(Map<String, Object?>? name, String code,
    [String? locale]) {
  String? pick(String l) {
    final v = name?[l];
    return v is String && v.trim().isNotEmpty ? v : null;
  }

  return pick(normalizeDefaultsLocale(locale)) ?? pick('en-US') ?? code;
}

/// `defaults.ts:348::quotaVariantLabel`。
String quotaVariantLabel(Map<String, Object?>? name, String id,
        [String? locale]) =>
    resolveDefaultsName(name, id, locale);

/// `defaults.ts:99::QuotaScriptRequire`。
class QuotaScriptRequire {
  const QuotaScriptRequire({required this.key, this.label});

  final String key;
  final Map<String, Object?>? label;
}

/// `defaults.ts:113::QuotaScriptVariant`（表单不需要脚本正文，故不带 `script`）。
class QuotaScriptVariant {
  const QuotaScriptVariant({
    required this.id,
    this.name,
    this.requires = const [],
  });

  final String id;
  final Map<String, Object?>? name;
  final List<QuotaScriptRequire> requires;
}

/// `defaults.ts:127::PlanQuotaBudget`。
class PlanQuotaBudget {
  const PlanQuotaBudget({
    required this.kind,
    required this.unit,
    required this.amount,
    this.windowHours,
    this.windowUnit,
  });

  final String kind;
  final String unit;
  final double amount;
  final double? windowHours;
  final String? windowUnit;
}

/// `defaults.ts:136::PlanQuotaTier`。
class PlanQuotaTier {
  const PlanQuotaTier({
    required this.id,
    required this.name,
    required this.budgets,
    required this.sourceUrl,
  });

  final String id;

  /// 官方原文档位名（不译）。
  final String name;
  final List<PlanQuotaBudget> budgets;
  final String sourceUrl;
}

/// `constants.ts:5::ProtocolOption`。
class ProtocolOption {
  const ProtocolOption({
    required this.value,
    required this.label,
    required this.codingPlan,
    required this.searchTerms,
  });

  final String value;
  final String label;
  final bool codingPlan;

  /// name 全 8 locale + label + keywords 去重（跨语言搜索用，UI 语言无关）。
  final List<String> searchTerms;
}

/// 端点「客户端模拟」下拉的一条（`defaults.ts:579::buildClientTypesFromPresets`）。
class ClientTypeOption {
  const ClientTypeOption({
    required this.value,
    required this.group,
    required this.label,
  });

  final String value;

  /// 分组名（`Claude Code` / `Codex` / `IDE` / `""` 默认）。
  final String group;
  final String label;
}

/// 整份 registry 文档的只读投影。构造靠 [PlatformDefaults.parse]，纯函数好测。
class PlatformDefaults {
  const PlatformDefaults(this._protocols, this._clientTypes);

  /// 空文档（拉取失败时的回落，**不崩**，各 getter 返空）。
  static const PlatformDefaults empty = PlatformDefaults({}, []);

  final Map<String, Map<String, Object?>> _protocols;
  final List<ClientTypeOption> _clientTypes;

  /// 解析 `get_defaults_json` 的返回串。非法 / 无 `protocols` → 空文档。
  static Map<String, Map<String, Object?>> parseProtocols(String raw) {
    if (raw.trim().isEmpty) return const {};
    try {
      final doc = jsonDecode(raw);
      final protocols = (doc as Map?)?['protocols'];
      if (protocols is! Map) return const {};
      return {
        for (final e in protocols.entries)
          if (e.value is Map)
            '${e.key}': Map<String, Object?>.from(e.value as Map),
      };
    } catch (_) {
      return const {};
    }
  }

  /// 解析 `get_client_types_json` 的返回串。非法 / 无 `client_types` → 空列表。
  static List<ClientTypeOption> parseClientTypes(String raw, [String? locale]) {
    if (raw.trim().isEmpty) return const [];
    try {
      final doc = jsonDecode(raw);
      final list = (doc as Map?)?['client_types'];
      if (list is! List) return const [];
      return [
        for (final e in list)
          if (e is Map)
            ClientTypeOption(
              value: '${e['value'] ?? ''}',
              group: '${e['group'] ?? ''}',
              label: resolveDefaultsName(
                e['name'] is Map
                    ? Map<String, Object?>.from(e['name'] as Map)
                    : null,
                '${e['value'] ?? ''}',
                locale,
              ),
            ),
      ];
    } catch (_) {
      return const [];
    }
  }

  static PlatformDefaults parse(
    String defaultsRaw,
    String clientTypesRaw, [
    String? locale,
  ]) => PlatformDefaults(
    parseProtocols(defaultsRaw),
    parseClientTypes(clientTypesRaw, locale),
  );

  Map<String, Object?>? _entry(String protocol) => _protocols[protocol];

  /// 协议 code 全集（下拉候选的来源）。
  Iterable<String> get protocols => _protocols.keys;

  List<ClientTypeOption> get clientTypes => _clientTypes;

  /// `defaults.ts:276::getDefaultEndpoints`：浅拷贝 + 缺省 client_type 补派生值。
  List<PlatformEndpoint> defaultEndpoints(String protocol) {
    final eps = (_entry(protocol)?['endpoints'] as Map?)?['default'];
    if (eps is! List) return const [];
    return [
      for (final e in eps)
        if (e is Map)
          PlatformEndpoint(
            protocol: '${e['protocol'] ?? ''}',
            baseUrl: '${e['base_url'] ?? ''}',
            clientType: e['client_type'] is String &&
                    (e['client_type'] as String).isNotEmpty
                ? e['client_type'] as String
                : clientTypeForProtocol('${e['protocol'] ?? ''}'),
            codingPlan: e['coding_plan'] == true,
          ),
    ];
  }

  /// `defaults.ts:291::getDefaultModels`。`isPeak=true` 且有 `models.peak` → 用 peak 分支。
  Map<String, String> defaultModels(String protocol, {bool isPeak = false}) {
    final section = _entry(protocol)?['models'];
    if (section is! Map) return const {};
    final branch = (isPeak && section['peak'] is Map)
        ? section['peak']
        : section['default'];
    if (branch is! Map) return const {};
    return {
      for (final e in branch.entries)
        if (e.value is String) '${e.key}': e.value as String,
    };
  }

  /// `defaults.ts:302::getDefaultModelList`。
  List<String> defaultModelList(String protocol) {
    final list = (_entry(protocol)?['model_list'] as Map?)?['default'];
    if (list is! List) return const [];
    return [for (final m in list) '$m'];
  }

  /// `defaults.ts:312::getDefaultPeak`（deep copy 防 mutate 污染缓存）。
  List<TimeWindow> defaultPeak(String protocol) {
    final list = _entry(protocol)?['peak'];
    if (list is! List) return const [];
    return [
      for (final w in list)
        if (w is Map)
          timeWindowFromJsonNormalized(Map<String, dynamic>.from(w)).clone(),
    ];
  }

  /// `defaults.ts:325::getDefaultQuotaScripts`（只留 id 是字符串的条目）。
  List<QuotaScriptVariant> defaultQuotaScripts(String protocol) {
    final list = _entry(protocol)?['quota_scripts'];
    if (list is! List) return const [];
    return [
      for (final v in list)
        if (v is Map && v['id'] is String)
          QuotaScriptVariant(
            id: v['id'] as String,
            name: v['name'] is Map
                ? Map<String, Object?>.from(v['name'] as Map)
                : null,
            requires: [
              for (final r in (v['requires'] as List?) ?? const [])
                if (r is Map && r['key'] is String)
                  QuotaScriptRequire(
                    key: r['key'] as String,
                    label: r['label'] is Map
                        ? Map<String, Object?>.from(r['label'] as Map)
                        : null,
                  ),
            ],
          ),
    ];
  }

  /// `defaults.ts:339::getDefaultPlanQuotas`（只留 id 是字符串且 budgets 是数组的条目）。
  List<PlanQuotaTier> defaultPlanQuotas(String protocol) {
    final list = _entry(protocol)?['plan_quotas'];
    if (list is! List) return const [];
    return [
      for (final tier in list)
        if (tier is Map && tier['id'] is String && tier['budgets'] is List)
          PlanQuotaTier(
            id: tier['id'] as String,
            name: '${tier['name'] ?? tier['id']}',
            sourceUrl: '${tier['source_url'] ?? ''}',
            budgets: [
              for (final b in tier['budgets'] as List)
                if (b is Map)
                  PlanQuotaBudget(
                    kind: '${b['kind'] ?? 'total'}',
                    unit: '${b['unit'] ?? 'usd'}',
                    amount: (b['amount'] as num?)?.toDouble() ?? 0,
                    windowHours: (b['window_hours'] as num?)?.toDouble(),
                    windowUnit: b['window_unit'] as String?,
                  ),
            ],
          ),
    ];
  }

  /// `defaults.ts:389::getProtocolLabel`。
  String protocolLabel(String protocol, [String? locale]) => resolveDefaultsName(
    _entry(protocol)?['name'] is Map
        ? Map<String, Object?>.from(_entry(protocol)!['name'] as Map)
        : null,
    protocol,
    locale,
  );

  /// `defaults.ts:419::getProtocolLabelMap`。
  Map<String, String> protocolLabelMap([String? locale]) => {
    for (final p in _protocols.keys) p: protocolLabel(p, locale),
  };

  /// 协议 → 品牌色（registry `platform.json` 的 `color`）。
  /// `ProtocolLogo.tsx:26` 的 `getProtocolColorMap` 同一份数据。
  Map<String, String> protocolColorMap() => {
    for (final e in _protocols.entries)
      if (e.value['color'] is String && '${e.value['color']}'.isNotEmpty)
        e.key: '${e.value['color']}',
  };

  /// `defaults.ts:412::isCodingPlanProtocol`。
  bool isCodingPlanProtocol(String protocol) =>
      _entry(protocol)?['is_coding_plan'] == true;

  /// `defaults.ts:479::buildProtocolsFromPresets`（本页只用到
  /// value / label / codingPlan / searchTerms 四项，hosts 与 keyPrefixes 是
  /// 智能粘贴那条链的输入，不在本票范围）。
  List<ProtocolOption> protocolOptions([String? locale]) {
    final out = <ProtocolOption>[];
    for (final proto in _protocols.keys) {
      final entry = _protocols[proto]!;
      final name = entry['name'] is Map
          ? Map<String, Object?>.from(entry['name'] as Map)
          : null;
      final label = resolveDefaultsName(name, proto, locale);
      final keywords = [
        for (final k in (entry['keywords'] as List?) ?? const []) '$k',
      ];
      final terms = <String>{
        for (final v in name?.values ?? const <Object?>[])
          if (v is String && v.trim().isNotEmpty) v,
        label,
        ...keywords,
      };
      out.add(
        ProtocolOption(
          value: proto,
          label: label,
          codingPlan: entry['is_coding_plan'] == true,
          searchTerms: terms.toList(),
        ),
      );
    }
    return out;
  }
}

/// 拉两条命令并解析。两条都是 best-effort：任一失败都回落空串，
/// 表单照常能开（端点下拉回落空列表，不崩 —— 与 TS 侧 `.catch` 同语义）。
Future<PlatformDefaults> loadPlatformDefaults(
  InvokeFn invoke, [
  String? locale,
]) async {
  final results = await Future.wait<String>([
    invoke('get_defaults_json').then((v) => v as String? ?? '').catchError(
      (_) => '',
    ),
    invoke('get_client_types_json').then((v) => v as String? ?? '').catchError(
      (_) => '',
    ),
  ]);
  return PlatformDefaults.parse(results[0], results[1], locale);
}
