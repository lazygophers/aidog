import 'package:flutter/widgets.dart';

/// 8 种语言的 locale 码。顺序 = React 侧 `src/locales/index.ts` 的 `ALL_LOCALES`，
/// 也就是语言下拉里的顺序，别重排。
const List<String> kAllLocales = <String>[
  'zh-Hans',
  'en-US',
  'ar-SA',
  'fr-FR',
  'de-DE',
  'ru-RU',
  'ja-JP',
  'es-ES',
];

/// 找不到文案时回落到这一种（与 React 的 `fallbackLng` 一致）。
const String kFallbackLocale = 'en-US';

/// DB / localStorage 里没有偏好时的默认语言（与 React 的 `normalizeLocale` 一致）。
const String kDefaultLocale = 'zh-Hans';

/// 从右往左排版的语言。
const List<String> kRtlLocales = <String>['ar-SA'];

bool isRtlLocale(String locale) => kRtlLocales.contains(locale);

TextDirection textDirectionOf(String locale) =>
    isRtlLocale(locale) ? TextDirection.rtl : TextDirection.ltr;

/// 脏值 / 历史值（如 `zh-CN`）归一化到 8 种之一，避免 `t('lang.$locale')` 落空显裸 key。
/// 归一化表照抄 Rust 的 `Lang::from_locale`（`aidog_i18n/src/lib.rs`），两侧认同一批别名。
String normalizeLocale(Object? raw) {
  if (raw is! String) return kDefaultLocale;
  if (kAllLocales.contains(raw)) return raw;
  switch (raw.trim().toLowerCase()) {
    case 'zh-cn':
    case 'zh_cn':
    case 'zh-hans':
    case 'zh_hans':
    case 'zh':
      return 'zh-Hans';
    case 'ja-jp':
    case 'ja_jp':
    case 'ja':
      return 'ja-JP';
    case 'fr-fr':
    case 'fr_fr':
    case 'fr':
      return 'fr-FR';
    case 'de-de':
    case 'de_de':
    case 'de':
      return 'de-DE';
    case 'ru-ru':
    case 'ru_ru':
    case 'ru':
      return 'ru-RU';
    case 'ar-sa':
    case 'ar_sa':
    case 'ar':
      return 'ar-SA';
    case 'es-es':
    case 'es_es':
    case 'es':
      return 'es-ES';
    case 'en-us':
    case 'en_us':
    case 'en':
      return 'en-US';
    default:
      return kDefaultLocale;
  }
}

/// 把 Flutter 的系统语言映射到 8 种之一；认不出的用 [kDefaultLocale]。
String localeFromPlatform(Locale platform) {
  final tag = platform.toLanguageTag();
  if (kAllLocales.contains(tag)) return tag;
  if (platform.languageCode == 'zh') return 'zh-Hans';
  return normalizeLocale(platform.languageCode);
}
