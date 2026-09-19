import 'dart:convert';

import 'package:flutter/services.dart';

import 'locales.dart';

/// 文案 JSON 的资产目录。真值源是 `src-tauri/crates/aidog_i18n/locales/`，
/// 通过 path 依赖 `aidog_i18n_locales` 当资产挂进来 —— 仓库里没有第二份拷贝。
const String kLocaleAssetDir = 'packages/aidog_i18n_locales/locales/';

/// 一种语言的全部文案（已拍平成点分键）。
class Translations {
  const Translations(this.locale, this.entries);

  final String locale;
  final Map<String, String> entries;

  static final Map<String, Translations> _cache = <String, Translations>{};

  /// 读并解析一种语言，同一 locale 只读一次。
  static Future<Translations> load(String locale, {AssetBundle? bundle}) async {
    final cached = _cache[locale];
    if (cached != null) return cached;
    final raw = await (bundle ?? rootBundle).loadString(
      '$kLocaleAssetDir$locale.json',
    );
    final parsed = Translations(
      locale,
      flatten(jsonDecode(raw) as Map<String, dynamic>),
    );
    _cache[locale] = parsed;
    return parsed;
  }

  /// 仅测试用：清掉解析缓存。
  static void clearCache() => _cache.clear();

  /// JSON 大部分是平铺的点分键，但有两处写成了嵌套对象（`group` / `logs`），
  /// 且同一个键两种形态都存在（如 `"group.addEnvVar"` 与 `group: {addEnvVar}`，
  /// 文案还不完全一样）。i18next 先走嵌套再退平铺，这里照同一优先级拍平。
  static Map<String, String> flatten(Map<String, dynamic> json) {
    final out = <String, String>{};
    void walk(Map<String, dynamic> node, String prefix) {
      node.forEach((key, value) {
        final path = prefix.isEmpty ? key : '$prefix.$key';
        if (value is Map<String, dynamic>) {
          walk(value, path);
        } else if (value is String) {
          out[path] = value;
        }
      });
    }

    // 先嵌套（优先），再平铺（不覆盖已有）。
    json.forEach((key, value) {
      if (value is Map<String, dynamic>) walk(value, key);
    });
    json.forEach((key, value) {
      if (value is String) out.putIfAbsent(key, () => value);
    });
    return out;
  }
}

/// 查文案：当前语言 → [kFallbackLocale] → 键本身（与 React 的 i18next 同一条链）。
///
/// [args] 按 i18next 的 `{{name}}` 占位插值。JSON 里还有少量单花括号 `{name}`
/// 的历史写法（如 `modelInfo.syncResult`），i18next 不认，这里同样不认 —— 两侧一致。
String translate(
  String key,
  Translations current,
  Translations fallback, {
  Map<String, Object?>? args,
}) {
  final raw = current.entries[key] ?? fallback.entries[key] ?? key;
  if (args == null || args.isEmpty) return raw;
  return interpolate(raw, args);
}

final RegExp _placeholder = RegExp(r'\{\{\s*([^{}]+?)\s*\}\}');

String interpolate(String template, Map<String, Object?> args) =>
    template.replaceAllMapped(_placeholder, (m) {
      final name = m.group(1)!;
      return args.containsKey(name) ? '${args[name]}' : m.group(0)!;
    });
