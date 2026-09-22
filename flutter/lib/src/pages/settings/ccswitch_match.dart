/// cc-switch provider → aidog Platform 的匹配回退链与 JSON 转换。
/// `src/utils/ccswitchMatch.ts` 的 Dart 版，纯函数，不碰 IO。
///
/// **为什么前端必须转**：后端 `ccswitch_import` / `sub2api_import` 收的是
/// **已经转好的 Platform JSON**，缺什么字段就写空串（`apply/db_rows.rs:113-160`）。
/// 原样把 provider map 发过去，导进去的是一串没有协议、没有 base_url、
/// 没有密钥的空壳平台。
///
/// **平台匹配词一律来自 registry**（`ProtocolMetaTable` / `PlatformDefaults`），
/// 代码里不写任何平台名 —— 项目既定规矩，见根 `CLAUDE.md`。
///
/// 回退链 4 步，与 TS 侧逐步对齐：
///   1. preset 关键词匹配（`matchPlatform`，name + base_url 一起当 hay）
///   2. base_url host 匹配（各 preset 默认端点的 host 双向子串）
///   3. codex 的 `wireApi` 回退
///   4. claude 回退 anthropic
library;

import '../models.dart';
import '../platform_card_bits.dart' show ProtocolMetaTable;
import '../platform_defaults.dart';
import '../platform_paste_logic.dart';

/// 命中方式。UI 要把它显示出来（React 用三色徽标），所以不是内部细节。
enum CcMatchedBy { presetKeyword, baseUrlHost, protocolFallback }

/// `ccswitchMatch.ts:26::CcMatchResult`。
class CcMatchResult {
  const CcMatchResult({
    required this.protocol,
    required this.matchedBy,
    required this.endpoints,
    this.matchedLabel,
  });

  final String protocol;
  final CcMatchedBy matchedBy;

  /// preset 骨架端点，其中同协议那条的 base_url 换成 provider 实际探测到的。
  final List<PlatformEndpoint> endpoints;

  /// 命中的 preset 显示名；回退时为 null。
  final String? matchedLabel;

  /// 展示用：实际会写进平台的 base_url（取同协议那条端点）。
  String get baseUrl {
    for (final ep in endpoints) {
      if (ep.protocol == protocol) return ep.baseUrl;
    }
    return endpoints.isEmpty ? '' : endpoints.first.baseUrl;
  }
}

/// 取 url 的 host（小写）。解析不出来就手工抠 `https://host/...`，再不行给空串。
String ccHostOf(String url) {
  final uri = Uri.tryParse(url);
  if (uri != null && uri.host.isNotEmpty) return uri.host.toLowerCase();
  final m = RegExp(r'^https?://([^/]+)', caseSensitive: false).firstMatch(url);
  return m == null ? '' : m.group(1)!.toLowerCase();
}

String _str(Object? v) => v == null ? '' : '$v';

Map<String, Object?> _mapOf(Object? v) =>
    v is Map ? Map<String, Object?>.from(v) : const {};

/// 步骤 2：拿每个 preset 默认端点的 host 与 provider 的 host 双向子串比。
/// `ccswitchMatch.ts:64::matchByBaseUrlHost` —— 含那条 `h.length >= 4` 的下限，
/// 少了它 `api` / `com` 这种短 host 会互相乱命中。
({String protocol, String label})? _matchByHost(
  String baseUrl,
  ProtocolMetaTable meta,
  PlatformDefaults defaults,
) {
  final target = normalizeForMatch(ccHostOf(baseUrl));
  if (target.isEmpty) return null;
  for (final code in meta.labels.keys) {
    for (final ep in defaults.defaultEndpoints(code)) {
      final h = normalizeForMatch(ccHostOf(ep.baseUrl));
      if (h.isEmpty || h.length < 4) continue;
      if (target.contains(h) || h.contains(target)) {
        return (protocol: code, label: meta.label(code));
      }
    }
  }
  return null;
}

/// 命中之后的成品：preset 骨架端点 + 用实际 base_url 覆盖同协议那条。
CcMatchResult _buildMatch(
  String protocol,
  CcMatchedBy matchedBy,
  String baseUrl,
  PlatformDefaults defaults, {
  String? label,
}) {
  final skeleton = defaults.defaultEndpoints(protocol);
  return CcMatchResult(
    protocol: protocol,
    matchedBy: matchedBy,
    matchedLabel: label,
    endpoints: baseUrl.isEmpty
        ? skeleton
        : [
            for (final ep in skeleton)
              if (ep.protocol == protocol)
                PlatformEndpoint(
                  protocol: ep.protocol,
                  baseUrl: baseUrl,
                  clientType: ep.clientType,
                  codingPlan: ep.codingPlan,
                )
              else
                ep,
          ],
  );
}

/// 步骤 3 + 4：协议回退（`ccswitchMatch.ts:189::buildFallback`）。
CcMatchResult _buildFallback(Map<String, Object?> provider, String baseUrl) {
  if (_str(provider['appType']) == 'codex') {
    final wireApi = _str(_mapOf(provider['codexConfigParsed'])['wireApi']);
    return CcMatchResult(
      protocol: 'openai',
      matchedBy: CcMatchedBy.protocolFallback,
      endpoints: [
        PlatformEndpoint(
          protocol: wireApi == 'responses' ? 'openai_responses' : 'openai',
          baseUrl: baseUrl,
          // 与 `defaults/client-types.json` 的 `codex_tui` 对齐；
          // 这是协议检测启发式的默认值，改它要同步 TS 侧同一处字面量。
          clientType: 'codex_tui',
          codingPlan: false,
        ),
      ],
    );
  }
  // claude 回退：用户原话「claude 类回退 anthropic 协议」——
  // 即使 URL 看着像 openai（NewAPI 之类）也仍然走 anthropic 端点。
  return CcMatchResult(
    protocol: 'anthropic',
    matchedBy: CcMatchedBy.protocolFallback,
    endpoints: [
      PlatformEndpoint(
        protocol: 'anthropic',
        baseUrl: baseUrl,
        clientType: 'claude_code',
        codingPlan: false,
      ),
    ],
  );
}

/// 匹配主入口。`ccswitchMatch.ts:163::matchCcProvider`。
CcMatchResult matchCcProvider(
  Map<String, Object?> provider, {
  required ProtocolMetaTable meta,
  required PlatformDefaults defaults,
}) {
  final baseUrl = _str(provider['detectedBaseUrl']);
  final name = _str(provider['name']);

  // 步骤 1：关键词匹配，name 与 base_url 拼在一起当 hay（与 TS 同）。
  final hit = matchPlatform('$name $baseUrl', meta.pastePresets);
  if (hit != null) {
    return _buildMatch(
      hit.value,
      CcMatchedBy.presetKeyword,
      baseUrl,
      defaults,
      label: hit.label,
    );
  }

  // 步骤 2：base_url host 匹配。
  final hostHit = _matchByHost(baseUrl, meta, defaults);
  if (hostHit != null) {
    return _buildMatch(
      hostHit.protocol,
      CcMatchedBy.baseUrlHost,
      baseUrl,
      defaults,
      label: hostHit.label,
    );
  }

  // 步骤 3 + 4。
  return _buildFallback(provider, baseUrl);
}

/// 从 provider 里抽模型映射（D2 维度）。`ccswitchMatch.ts:200::extractModels`。
PlatformModels extractCcModels(Map<String, Object?> provider) {
  final appType = _str(provider['appType']);
  if (appType == 'claude') {
    final env = _mapOf(_mapOf(provider['settingsConfig'])['env']);
    String? pick(String key) {
      final v = _str(env[key]);
      return v.isEmpty ? null : v;
    }

    return PlatformModels(
      defaultModel: pick('ANTHROPIC_MODEL'),
      haiku: pick('ANTHROPIC_DEFAULT_HAIKU_MODEL'),
      sonnet: pick('ANTHROPIC_DEFAULT_SONNET_MODEL'),
      opus: pick('ANTHROPIC_DEFAULT_OPUS_MODEL'),
    );
  }
  if (appType == 'codex') {
    final model = _str(_mapOf(provider['codexConfigParsed'])['model']);
    return PlatformModels(defaultModel: model.isEmpty ? null : model);
  }
  return const PlatformModels();
}

/// 三个导入维度（`ccswitchMatch.ts:219::CcImportDims`）。
/// D1（平台类型 + endpoints）在 React 里是锁定常开的，这里保留字段但默认 true。
class CcImportDims {
  const CcImportDims({this.d1 = true, this.d2 = true, this.d4 = true});

  /// 平台类型 + endpoints。
  final bool d1;

  /// 模型映射。
  final bool d2;

  /// 密钥。
  final bool d4;

  CcImportDims copyWith({bool? d1, bool? d2, bool? d4}) =>
      CcImportDims(d1: d1 ?? this.d1, d2: d2 ?? this.d2, d4: d4 ?? this.d4);
}

/// provider + 匹配结果 → Platform JSON（`collect.rs` 里 `Platform` 的序列化形态）。
/// `ccswitchMatch.ts:236::ccProviderToPlatformJson`，字段逐个对齐，一个不少 ——
/// 少写的字段后端会填空串，那正是「空壳平台」的来源。
Map<String, Object?> ccProviderToPlatformJson(
  Map<String, Object?> provider,
  CcMatchResult match,
  CcImportDims dims,
) => {
  'name': _str(provider['name']),
  'platform_type': match.protocol,
  'base_url': _str(provider['detectedBaseUrl']),
  'api_key': dims.d4 ? _str(provider['detectedApiKey']) : '',
  'extra': '',
  'models': dims.d2 ? extractCcModels(provider).toJson() : <String, Object?>{},
  'available_models': <Object?>[],
  'endpoints': [for (final ep in match.endpoints) ep.toJson()],
  'enabled': true,
  'status': 'enabled',
  'auto_disabled_until': 0,
  'auto_disable_strikes': 0,
  'est_balance_remaining': 0,
  'est_coding_plan': '',
  'last_real_query_at': 0,
  'estimate_count': 0,
  'show_in_tray': false,
  'tray_display': '',
  'sort_order': 0,
  'manual_budgets': <Object?>[],
};
