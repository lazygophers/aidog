/// 「智能识别」的解析器（票 20），移植自 `src/utils/platformPaste.ts`。
///
/// 从论坛分享类杂乱文案里抽 4 类字段：apikey / base_url / 平台 / 过期时间。
/// 设计覆盖样例与 React 侧同一批：小米 MIMO（双 base_url）、防爬汉字 key、
/// kimicode（多 key + url:）、base64 编码 key。
///
/// **纯函数、零 widget 依赖** —— 识别规则要能单测，widget 只管渲染（票 20 的硬要求）。
/// 平台匹配词一律来自 registry `platform.json`（经 [PastePresetRef] 传入），
/// 代码里不写任何平台名。
///
/// Dart 正则与 JS 的差异只有两处，其余逐字照抄：
///   - JS 的 `atob` 产出 Latin-1 字节串且容忍缺省填充；Dart `base64.decode` 要求
///     长度是 4 的倍数，故有 [_padBase64]。
///   - JS 正则带 `g` flag 时 `lastIndex` 有状态（React 侧为此写了好几处复位）；
///     Dart 的 `RegExp` 无状态，那些复位代码没有对应物。
library;

import 'dart:convert';

// ── 类型 ──────────────────────────────────────────────────────────

/// base_url 的协议倾向（仅用于展示分组 / 排序，不是平台类型）。
/// `platformPaste.ts:6::ParsedProtocol`。
enum ParsedProtocol { anthropic, openai, gemini, unknown }

/// `platformPaste.ts:8::ParsedBaseUrl`。
class ParsedBaseUrl {
  const ParsedBaseUrl(this.url, this.protocol);

  final String url;
  final ParsedProtocol protocol;

  @override
  bool operator ==(Object other) =>
      other is ParsedBaseUrl && other.url == url && other.protocol == protocol;

  @override
  int get hashCode => Object.hash(url, protocol);

  @override
  String toString() => 'ParsedBaseUrl($url, ${protocol.name})';
}

/// 命中的内置平台。`platformPaste.ts:42` 那个匿名对象。
class PasteMatch {
  const PasteMatch(this.value, this.label, {this.codingPlan = false});

  final String value;
  final String label;

  /// coding 套餐变体标记：透传给填表逻辑选对普通 / coding 的 endpoints，
  /// 否则同名两 preset 命中后会拿错 base_url。
  final bool codingPlan;

  @override
  bool operator ==(Object other) =>
      other is PasteMatch &&
      other.value == value &&
      other.label == label &&
      other.codingPlan == codingPlan;

  @override
  int get hashCode => Object.hash(value, label, codingPlan);

  @override
  String toString() => 'PasteMatch($value, $label, coding=$codingPlan)';
}

/// 解析器需要的 preset 字段。`platformPaste.ts:19::PastePresetRef`。
class PastePresetRef {
  const PastePresetRef({
    required this.value,
    required this.label,
    this.keywords = const [],
    this.hosts = const [],
    this.keyPrefixes = const [],
    this.codingPlan = false,
  });

  final String value;
  final String label;

  /// registry `platform.json` 的 `keywords`。
  final List<String> keywords;

  /// base_url 的 host（或 host+path）子串，派生自 preset 的 default 端点。
  /// 多 preset 重叠时最长串胜出。
  final List<String> hosts;

  /// registry `platform.json` 的 `key_prefixes`（如 `sk-ant-` / `tp-` / `ark-`）。
  final List<String> keyPrefixes;

  final bool codingPlan;
}

/// `platformPaste.ts:35::ParsedPaste`。
class ParsedPaste {
  const ParsedPaste({
    this.apiKeys = const [],
    this.baseUrls = const [],
    this.platform,
    this.models = const [],
    this.expiresAt,
  });

  /// 去重后的候选 apikey（已剔防爬汉字、已尝试 base64 解码）。
  final List<String> apiKeys;

  /// 去重后的候选 base_url（含协议倾向）。
  final List<ParsedBaseUrl> baseUrls;

  /// 命中的内置平台；null = 无匹配（调用方据此决定要不要改平台选择）。
  final PasteMatch? platform;

  /// 候选模型名（来自 base64 解码的标签复合串「模型名X」）。多为空。
  final List<String> models;

  /// 识别到的过期时间（毫秒时间戳）；null = 未识别。
  final int? expiresAt;

  /// 有没有任何识别结果（`SmartPasteModal.tsx:159::hasResult`）。
  bool get hasResult =>
      apiKeys.isNotEmpty || baseUrls.isNotEmpty || platform != null;
}

// ── 常量与正则 ─────────────────────────────────────────────────────

/// 永不自动匹配的 preset（测试 / 开发占位平台）。
/// mock 的关键词（「测试」「调试」「假数据」）是通用子串，会命中论坛文案噪声而误匹配；
/// 从候选里硬排除，用户仍可在下拉里手动选它。`platformPaste.ts:16`。
const Set<String> kNeverAutoMatch = {'mock'};

/// 通用 apikey 前缀（格式约定，不是平台品牌）。平台专属前缀一律来自
/// registry `key_prefixes`，由 [collectKeyPrefixes] 合入 —— 禁在代码硬编码。
/// `platformPaste.ts:52`。
const List<String> kGenericKeyPrefixes = ['sk-', 'sk_'];

/// CJK 及全角标点区段，用来剔 key 里混入的防爬汉字。`platformPaste.ts:68`。
/// 含平假名 / 片假名 + CJK 标点 + 全角区段 + 圈数字（社区分享用 ②⑤⑨ 替数字）。
final RegExp _cjk = RegExp(
  r'[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯①-⓿]',
  unicode: true,
);

/// 赋值锚定：`API_KEY=` / `apikey:` / `秘药：` / `key=` 等后跟值。
/// `platformPaste.ts:91::ASSIGN_RE`。
final RegExp _assign = RegExp(
  r'''["']?(?:api[\s_-]*key|secret|token|秘药|密钥|key|auth[\s_-]*token|(?:[\w-]+)[\s_-]*(?:auth[\s_-]*token|api[\s_-]*key)|api)["']?\s*[:：=]\s*["'‘’《「]?\s*([A-Za-z0-9_\-+/=.\p{Script=Han}　-〿＀-￯①-⓿]{12,})''',
  caseSensitive: false,
  unicode: true,
);

/// 纯 base64 token 形态（无已知前缀时的解码启发式门槛）。`platformPaste.ts:95`。
final RegExp _base64Shape = RegExp(r'^[A-Za-z0-9+/]{20,}={0,2}$');

/// 裸 base64 token（无标签兜底扫描）。`platformPaste.ts:98`。
final RegExp _bareBase64 = RegExp(r'[A-Za-z0-9+/]{24,}={0,2}');

/// CJK 锚定的防爬指令噪声：以 CJK 开头、中间可夹 ASCII（指令里的「base64」字样）、
/// 以 CJK 收尾的整段，或单个 CJK。`platformPaste.ts:104`。
///
/// 与 [_cjk] 的区别：这条会**连带吞掉 CJK 包夹的 ASCII**，所以只用于 base64 拼接
/// 与 URL 清洗两个场景，不能拿去当通用的去汉字工具。
final RegExp _cjkNoise = RegExp(
  r'[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯][\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯A-Za-z0-9]*[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯]|[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯]',
  unicode: true,
);

/// 防爬汉字穿插的裸 base64：base64 段 + CJK 噪声 + base64 段（可多组）。
/// `platformPaste.ts:110`。
final RegExp _bareBase64Cjk = RegExp(
  r'[A-Za-z0-9+/]{8,}(?:[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯][\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯A-Za-z0-9]*[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}　-〿＀-￯]?[A-Za-z0-9+/]{8,})+={0,2}',
  unicode: true,
);

/// base64 旁注标记（「KEY（base64编码）：」里的括号段）。夹在标签与分隔符之间会
/// 阻断 [_assign]，正则前先剔掉。`platformPaste.ts:116`。
final RegExp _base64Note = RegExp(
  r'[（(]\s*base64[^）)]*[）)]',
  caseSensitive: false,
  unicode: true,
);

/// 解码后的键形（短前缀 + 长串），裸 base64 兜底的误报守卫。`platformPaste.ts:120`。
final RegExp _decodedKeyShape = RegExp(r'^[a-z]{2,8}-[A-Za-z0-9_\-]{20,}$');

/// 中文 / 英文标签词典：解码后的复合串按标签切分。`platformPaste.ts:168`。
///
/// CJK 标签（令牌 / 密钥 / 地址 / 接口 / 模型名 / 模型）是反爬主标记，可紧贴值无分隔；
/// ASCII 标签须前置非字母边界 + 后随分隔符，否则会误切在值内部（如 `superToken`
/// 里的 token、URL 里的 base）。
final RegExp _compoundLabel = RegExp(
  r'(令牌|密钥|接口地址|地址|接口|模型名|模型)\s*[:：=]?\s*|(?<![A-Za-z])(api[_-]?key|key|token|base[_-]?url|url|base|model)\s*[:：=]\s*',
  caseSensitive: false,
  unicode: true,
);

/// 端点子路径后缀：base_url 要截到版本前缀为止（URL 构造规矩）。`platformPaste.ts:172`。
final RegExp _endpointSuffix = RegExp(
  r'/(?:chat/completions|messages|responses|completions)\b.*$',
  caseSensitive: false,
);

/// 正文里的 URL。字符类不排斥全角括号 / CJK，故防爬噪声会被一并吞入，
/// 匹配后再用 [_cjkNoise] 剔除还原。`platformPaste.ts:323`。
final RegExp _urlRe = RegExp(
  r'''https?://[^\s"'“”‘’《」】\])，。；、>]+''',
  unicode: true,
);

/// URL 尾部要剪掉的标点。`platformPaste.ts:326`。
final RegExp _urlTailPunct = RegExp(
  r'''[.,;:。，；、)）"'“”‘’》」】>]+$''',
  unicode: true,
);

/// 图片 / 静态资源后缀，直接跳过。`platformPaste.ts:329`。
final RegExp _assetExt = RegExp(
  r'\.(png|jpe?g|gif|webp|svg|ico)(\?|$)',
  caseSensitive: false,
);

// ── 小工具 ────────────────────────────────────────────────────────

String _stripCjk(String s) => s.replaceAll(_cjk, '');

bool _hasKnownPrefix(String s, List<String> prefixes) =>
    prefixes.any(s.startsWith);

void _pushUnique(List<String> arr, String v) {
  if (v.isNotEmpty && !arr.contains(v)) arr.add(v);
}

/// 补齐 base64 填充。JS 的 `atob` 容忍缺省填充，Dart 的 `base64.decode` 不容忍。
String _padBase64(String s) {
  final rem = s.length % 4;
  return rem == 0 ? s : s + '=' * (4 - rem);
}

/// base64 → 字节。非法输入返回 null。
List<int>? _decodeBytes(String s) {
  try {
    return base64.decode(_padBase64(s));
  } on FormatException {
    return null;
  }
}

/// base64 解码，只接受纯可打印 ASCII 的结果（排除二进制噪声）。
/// `platformPaste.ts:132::tryBase64Decode`。CJK 标签复合串走 [_tryBase64DecodeUtf8]。
String? tryBase64Decode(String s) {
  if (!_base64Shape.hasMatch(s)) return null;
  final bytes = _decodeBytes(s);
  if (bytes == null || bytes.isEmpty) return null;
  for (final b in bytes) {
    if (b < 0x20 || b > 0x7e) return null;
  }
  return String.fromCharCodes(bytes);
}

/// base64 解码为 UTF-8。`platformPaste.ts:148::tryBase64DecodeUtf8`。
///
/// 结果里必须含至少一个 CJK 标签字符，否则交给纯 ASCII 那条路，避免两条路重复采纳。
String? _tryBase64DecodeUtf8(String s) {
  if (!_base64Shape.hasMatch(s)) return null;
  final bytes = _decodeBytes(s);
  if (bytes == null) return null;
  final String decoded;
  try {
    decoded = utf8.decode(bytes);
  } on FormatException {
    return null;
  }
  if (!_cjk.hasMatch(decoded)) return null;
  return decoded;
}

/// 归一化用于平台关键字匹配：小写 + 非 alnum/CJK → 空格 + 折叠空白。
/// `platformPaste.ts:221::normalizeForMatch`。
String normalizeForMatch(String s) => s
    .toLowerCase()
    .replaceAll(RegExp(r'[^a-z0-9一-鿿]+', caseSensitive: false), ' ')
    .replaceAll(RegExp(r'\s+'), ' ')
    .trim();

/// 从 presets 收集全部 key 前缀（`key_prefixes` + 通用），**长在前** ——
/// 正则交替按序尝试，短的排前面会让 `sk-` 抢先吃掉 `sk-ant-`。
/// `platformPaste.ts:56::collectKeyPrefixes`。
List<String> collectKeyPrefixes(List<PastePresetRef> presets) {
  final set = <String>{...kGenericKeyPrefixes};
  for (final p in presets) {
    set.addAll(p.keyPrefixes);
  }
  final out = set.toList()..sort((a, b) => b.length.compareTo(a.length));
  return out;
}

final Map<String, RegExp> _prefixTokenCache = {};

/// 前缀锚定 token：前缀 + 后续 alnum/_/-，允许中间穿插防爬 CJK（后面整体 strip）。
/// `platformPaste.ts:77::prefixTokenRe`。
RegExp _prefixTokenRe(List<String> prefixes) => _prefixTokenCache.putIfAbsent(
  prefixes.join('|'),
  () => RegExp(
    '(${prefixes.map(RegExp.escape).join('|')})'
    r'[A-Za-z0-9_\-\.\p{Script=Han}　-〿＀-￯①-⓿]{12,}',
    unicode: true,
  ),
);

// ── 抽取：apikey ──────────────────────────────────────────────────

/// 抽取 apikey 候选。`platformPaste.ts:250::extractApiKeys`。
/// [prefixes] 是当前 presets 的前缀集合（[collectKeyPrefixes] 派生）。
List<String> extractApiKeys(String text, List<String> prefixes) {
  final keys = <String>[];

  // 旁注（「（base64编码）」）会夹在标签与分隔符之间阻断 _assign，先剔除。
  // 该短语永不出现在真实 key / url 里，全局剔除是安全的。
  final cleaned = text.replaceAll(_base64Note, '');

  // 1) 前缀锚定（覆盖 sk- / tp- / sk-kimi-，含防爬汉字穿插）。
  for (final m in _prefixTokenRe(prefixes).allMatches(cleaned)) {
    final clean = _stripCjk(m[0]!);
    if (clean.length >= 16) _pushUnique(keys, clean);
  }

  // 2) 赋值锚定（覆盖 API_KEY= / 秘药： 等；含无标准前缀 + base64 编码的 key）。
  for (final m in _assign.allMatches(cleaned)) {
    final raw = _stripCjk(m[1] ?? '');
    if (raw.isEmpty) continue;
    if (_hasKnownPrefix(raw, prefixes)) {
      if (raw.length >= 16) _pushUnique(keys, raw);
      continue;
    }
    final decoded = tryBase64Decode(raw);
    if (decoded != null && decoded.length >= 12) {
      _pushUnique(keys, decoded);
    } else if (raw.length >= 24) {
      // 解不出也保留原串（可能是非标准前缀的明文 key）。
      _pushUnique(keys, raw);
    }
  }

  // 3) 裸 base64 兜底（无标签、无旁注的整段 base64）。解码结果须是键形或带已知前缀，
  //    否则拒掉 —— 否则解码噪声和 URL 片段会大量误报。
  for (final m in _bareBase64.allMatches(cleaned)) {
    final decoded = tryBase64Decode(m[0]!);
    if (decoded == null || decoded.length < 12) continue;
    if (_hasKnownPrefix(decoded, prefixes) ||
        _decodedKeyShape.hasMatch(decoded)) {
      _pushUnique(keys, decoded);
    }
  }

  // 3.5) 防爬汉字穿插的裸 base64：整段被 CJK 切断成多片，上一步只能匹配单片、
  //      解出半截 key。这里先剔 CJK 拼回完整串再解码，门槛同上。
  for (final m in _bareBase64Cjk.allMatches(cleaned)) {
    final joined = m[0]!.replaceAll(_cjkNoise, '');
    if (joined.length < 24) continue;
    final decoded = tryBase64Decode(joined);
    if (decoded == null || decoded.length < 12) continue;
    if (_hasKnownPrefix(decoded, prefixes) ||
        _decodedKeyShape.hasMatch(decoded)) {
      _pushUnique(keys, decoded);
    }
  }

  return keys;
}

// ── 抽取：base_url ────────────────────────────────────────────────

/// `platformPaste.ts:307::guessProtocol`。
ParsedProtocol guessProtocol(String url) {
  final u = url.toLowerCase();
  // 容错截断的 "anthropi"。
  if (RegExp(r'anthrop').hasMatch(u)) return ParsedProtocol.anthropic;
  if (RegExp(r'gemini|generativelanguage').hasMatch(u)) {
    return ParsedProtocol.gemini;
  }
  if (RegExp(r'openai|/v1(/|\b)').hasMatch(u)) return ParsedProtocol.openai;
  return ParsedProtocol.unknown;
}

/// 抽取 base_url 候选。`platformPaste.ts:316::extractBaseUrls`。
List<ParsedBaseUrl> extractBaseUrls(String text) {
  final out = <ParsedBaseUrl>[];
  final seen = <String>{};
  for (final m in _urlRe.allMatches(text)) {
    // URL 本不含 CJK，故整体剔噪声即可还原真实 URL；剔除范围严格限于这个
    // URL token 内，不误伤正文其他中文。
    var url = m[0]!.replaceAll(_cjkNoise, '').replaceAll(_urlTailPunct, '');
    if (url.isEmpty) continue;
    if (_assetExt.hasMatch(url)) continue;
    if (!seen.add(url)) continue;
    out.add(ParsedBaseUrl(url, guessProtocol(url)));
  }
  return out;
}

// ── 匹配平台 ──────────────────────────────────────────────────────

/// 匹配内置平台 preset。`platformPaste.ts:344::matchPlatform`。
///
/// 优先级 1：base_url 命中 preset 的 `hosts` 子串（最强信号），多 preset 重叠时
/// **最长串胜出** —— 例如粘贴 coding 专属 host 时，coding preset 比普通版更特异而赢。
/// 优先级 2：keyword 文本扫描打分（命中数 desc > 最长命中关键字长度 desc > 列表顺序 asc）。
/// 打分是为了根治「排在前面的 preset 用通用词抢走同族更具体 preset」。
PasteMatch? matchPlatform(
  String text,
  List<PastePresetRef> presets, [
  List<ParsedBaseUrl>? baseUrls,
]) {
  if (baseUrls != null && baseUrls.isNotEmpty) {
    final urls = [for (final b in baseUrls) b.url.toLowerCase()];
    PasteMatch? best;
    var bestLen = 0;
    for (final p in presets) {
      if (kNeverAutoMatch.contains(p.value)) continue;
      for (final h in p.hosts) {
        final hl = h.toLowerCase();
        if (hl.length > bestLen && urls.any((u) => u.contains(hl))) {
          best = PasteMatch(p.value, p.label, codingPlan: p.codingPlan);
          bestLen = hl.length;
        }
      }
    }
    if (best != null) return best;
  }

  final hay = normalizeForMatch(text);
  PasteMatch? best;
  var bestHits = 0;
  var bestLongest = 0;
  for (final p in presets) {
    if (kNeverAutoMatch.contains(p.value)) continue;
    var hits = 0;
    var longest = 0;
    for (final kw in p.keywords) {
      final needle = normalizeForMatch(kw);
      if (needle.isNotEmpty && hay.contains(needle)) {
        hits++;
        if (needle.length > longest) longest = needle.length;
      }
    }
    if (hits == 0) continue;
    if (hits > bestHits || (hits == bestHits && longest > bestLongest)) {
      best = PasteMatch(p.value, p.label, codingPlan: p.codingPlan);
      bestHits = hits;
      bestLongest = longest;
    }
  }
  return best;
}

// ── base64 复合串（第三变体）──────────────────────────────────────

/// `platformPaste.ts:174::CompoundParts`。
class _CompoundParts {
  String? apiKey;
  String? baseUrl;
  String? model;

  bool get isEmpty => apiKey == null && baseUrl == null && model == null;
}

/// 解析「标签紧贴值」的复合串（base64 解码后的形态）。
/// `platformPaste.ts:183::parseCompoundLabeled`。
_CompoundParts? _parseCompoundLabeled(String s) {
  final segs = <({String label, String value})>[];
  String? lastLabel;
  var lastEnd = 0;
  for (final m in _compoundLabel.allMatches(s)) {
    if (lastLabel != null) {
      segs.add((label: lastLabel, value: s.substring(lastEnd, m.start)));
    }
    lastLabel = (m[1] ?? m[2]!).toLowerCase();
    lastEnd = m.end;
  }
  if (lastLabel != null) {
    segs.add((label: lastLabel, value: s.substring(lastEnd)));
  }
  if (segs.isEmpty) return null;

  final parts = _CompoundParts();
  for (final seg in segs) {
    final v = _stripCjk(seg.value).trim();
    if (v.isEmpty) continue;
    if (RegExp(r'令牌|密钥|key|token', caseSensitive: false).hasMatch(seg.label)) {
      if (parts.apiKey == null && v.length >= 12) parts.apiKey = v;
    } else if (RegExp(
      r'地址|接口|url|base',
      caseSensitive: false,
    ).hasMatch(seg.label)) {
      if (parts.baseUrl == null) {
        final um = RegExp(r'https?://\S+').firstMatch(v);
        if (um != null) {
          parts.baseUrl = um[0]!.replaceAll(_endpointSuffix, '');
        }
      }
    } else if (RegExp(r'模型|model', caseSensitive: false).hasMatch(seg.label)) {
      parts.model ??= v;
    }
  }
  return parts.isEmpty ? null : parts;
}

/// 扫 base64 token → UTF-8 解码 → 标签复合串解析。
/// `platformPaste.ts:413::extractCompoundFromBase64`。
///
/// 标签锚定有个盲区：**裸 key**（没有令牌 / 密钥 / key 标签，如 MiMo 文案里跟在
/// 「接口协议：URL」后面的那个 key）会被归进「接口」段、被 URL 正则忽略而漏提。
/// 所以切分之后若 apiKey 还空着，对解码明文补跑一遍前缀锚定扫描兜底。
List<_CompoundParts> _extractCompoundFromBase64(
  String text,
  List<String> prefixes,
) {
  final out = <_CompoundParts>[];
  for (final m in _bareBase64.allMatches(text)) {
    if (m[0]!.length < 24) continue;
    final decoded = _tryBase64DecodeUtf8(m[0]!);
    if (decoded == null) continue;
    final parts = _parseCompoundLabeled(decoded) ?? _CompoundParts();
    if (parts.apiKey == null) {
      for (final km in _prefixTokenRe(prefixes).allMatches(decoded)) {
        final clean = _stripCjk(km[0]!);
        if (clean.length >= 16 &&
            (_hasKnownPrefix(clean, prefixes) ||
                _decodedKeyShape.hasMatch(clean))) {
          parts.apiKey = clean;
          break;
        }
      }
    }
    if (!parts.isEmpty) out.add(parts);
  }
  return out;
}

// ── 过期时间识别 ──────────────────────────────────────────────────

/// 时间候选：匹配到的串 + 字符位置（用于「离语义词多近」排序）。
class _TimeCandidate {
  const _TimeCandidate(this.raw, this.index);
  final String raw;
  final int index;
}

/// 形如 `MM-DD` / `MM.DD` / `MM月DD` / `MM-DD HH:MM` / `YYYY-MM-DD[ HH:MM]` 的子串。
/// `platformPaste.ts:451::DATETIME_RE`。
///
/// `MM-DD` 分支的分隔符含 `.`（社区帖的「6.27到期」）；版本号（如 4.5）由调用方的
/// 语义词硬门 + 60 字符距离门槛挡掉。不匹配纯 `HH:MM`（易与版本号 / 比例混淆）。
final RegExp _datetime = RegExp(
  r'(?:(\d{4})[-/](\d{1,2})[-/](\d{1,2})(?:[ T](\d{1,2}):(\d{1,2}))?)|(?:(\d{1,2})[-/月.](\d{1,2})(?:[日号 T](\d{1,2}):(\d{1,2}))?)',
  unicode: true,
);

/// 过期语义词。文案里出现这些词时，邻近的日期候选才作数。`platformPaste.ts:455`。
final RegExp _expiryKeywords = RegExp(
  r'过期|到期|有效期|即将过期|临近过期|expir(?:e|y|ation)|\bexp\b',
  caseSensitive: false,
  unicode: true,
);

/// 解析单个候选。`platformPaste.ts:464::parseCandidate`。
///
/// 返回的 `dateOnly` 表示候选没有时间分量（只到日）；调用方据此把时间推到当日
/// 23:59:59.999，避免 00:00 让「今天到期」在中午就被判成已过期。
({int ts, bool dateOnly}) _parseCandidate(_TimeCandidate c, int now) {
  final m = _datetime.firstMatch(c.raw);
  if (m == null) return (ts: 0, dateOnly: false);
  int y, mo, d, h, mi;
  bool dateOnly;
  if (m[1] != null) {
    y = int.parse(m[1]!);
    mo = int.parse(m[2]!);
    d = int.parse(m[3]!);
    dateOnly = m[4] == null;
    h = m[4] == null ? 0 : int.parse(m[4]!);
    mi = m[5] == null ? 0 : int.parse(m[5]!);
  } else {
    y = DateTime.fromMillisecondsSinceEpoch(now).year;
    mo = int.parse(m[6]!);
    d = int.parse(m[7]!);
    dateOnly = m[8] == null;
    h = m[8] == null ? 0 : int.parse(m[8]!);
    mi = m[9] == null ? 0 : int.parse(m[9]!);
    // 当年这一天已经过去（且不是今天）→ 推到次年。
    final thisYear = DateTime(y, mo, d, h, mi).millisecondsSinceEpoch;
    if (thisYear < now && now - thisYear > 86400000) y += 1;
  }
  if (mo < 1 || mo > 12) return (ts: 0, dateOnly: false);
  if (d < 1 || d > 31) return (ts: 0, dateOnly: false);
  if (dateOnly) {
    return (
      ts: DateTime(y, mo, d, 23, 59, 59, 999).millisecondsSinceEpoch,
      dateOnly: true,
    );
  }
  return (
    ts: DateTime(y, mo, d, h, mi).millisecondsSinceEpoch,
    dateOnly: false,
  );
}

/// 从文案里识别过期时间。`platformPaste.ts:512::extractExpiryAt`。
///
/// 收紧模式（React 侧 2026-06-25 的回归修复）：文案必须出现过期语义词，且日期候选
/// 距最近的语义词 ≤ 60 字符才算数 —— 否则「更新于 2026-07-15」这种非过期语境的
/// 日期会被误识别灌进表单。另保留 7 天回退保护（早于 `now - 7d` 视为历史无效）。
int? extractExpiryAt(String text, [int? nowMs]) {
  if (text.isEmpty) return null;
  final now = nowMs ?? DateTime.now().millisecondsSinceEpoch;

  final kwPositions = [
    for (final m in _expiryKeywords.allMatches(text)) m.start,
  ];
  if (kwPositions.isEmpty) return null;

  final candidates = [
    for (final m in _datetime.allMatches(text)) _TimeCandidate(m[0]!, m.start),
  ];
  if (candidates.isEmpty) return null;

  final cutoff = now - 7 * 86400000;
  final parsed = <({_TimeCandidate c, int ts})>[];
  for (final c in candidates) {
    final r = _parseCandidate(c, now);
    if (r.ts > cutoff) parsed.add((c: c, ts: r.ts));
  }
  if (parsed.isEmpty) return null;

  int dist(_TimeCandidate c) => kwPositions
      .map((p) => (p - c.index).abs())
      .reduce((a, b) => a < b ? a : b);

  parsed.sort((a, b) => dist(a.c).compareTo(dist(b.c)));
  final best = parsed.first;
  // 最近的候选距语义词还超过 60 字符 → 视为无关日期（保守阈值）。
  if (dist(best.c) > 60) return null;
  return best.ts;
}

// ── 入口 ──────────────────────────────────────────────────────────

/// 解析粘贴文本。`platformPaste.ts:563::parsePlatformPaste`。
ParsedPaste parsePlatformPaste(
  String text,
  List<PastePresetRef> presets, {
  int? nowMs,
}) {
  if (text.trim().isEmpty) return const ParsedPaste();

  final keyPrefixes = collectKeyPrefixes(presets);
  final baseUrls = extractBaseUrls(text);
  final apiKeys = extractApiKeys(text, keyPrefixes);
  final models = <String>[];

  // 第三变体：base64 解码后是中文标签复合串，补提 key / base_url / model。
  for (final parts in _extractCompoundFromBase64(text, keyPrefixes)) {
    final k = parts.apiKey;
    if (k != null) _pushUnique(apiKeys, k);
    final b = parts.baseUrl;
    if (b != null && !baseUrls.any((x) => x.url == b)) {
      baseUrls.add(ParsedBaseUrl(b, guessProtocol(b)));
    }
    final m = parts.model;
    if (m != null) _pushUnique(models, m);
  }

  // 优先级 2：apiKeys 命中某 preset 的 key_prefixes → 直接认它，跳过 keyword 打分。
  // 纯 token 粘贴（无文案无 URL）也能识别；host 匹配（优先级 1）未命中时由它兜底。
  PasteMatch? platform;
  if (apiKeys.isNotEmpty) {
    for (final p in presets) {
      if (kNeverAutoMatch.contains(p.value)) continue;
      if (p.keyPrefixes.isNotEmpty &&
          apiKeys.any((k) => p.keyPrefixes.any(k.startsWith))) {
        platform = PasteMatch(p.value, p.label, codingPlan: p.codingPlan);
        break;
      }
    }
  }
  platform ??= matchPlatform(text, presets, baseUrls);

  return ParsedPaste(
    apiKeys: apiKeys,
    baseUrls: baseUrls,
    platform: platform,
    models: models,
    expiresAt: extractExpiryAt(text, nowMs),
  );
}
