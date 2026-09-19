/// aidog Flutter 外壳的 i18n 层。其余所有票只从这里 import。
///
/// ```dart
/// import 'package:aidog_flutter/i18n.dart';
///
/// await i18n.init();                 // main() 里，在 runApp 之前
/// runApp(AidogI18n(child: const MyApp()));
///
/// Text(i18n.t('common.loading'))
/// Text(i18n.t('about.downloading', {'version': '0.1.17'}))
/// Text(ltr('+6.1%'))                 // 数字开头/结尾的短标签
/// AlwaysLtr(child: chart)            // 时间轴永远左→右
/// ```
///
/// 文案真值源是 `src-tauri/crates/aidog_i18n/locales/*.json`，Rust / React / Flutter
/// 共读同一份，**没有第二份拷贝**。补键时按域分组插入，**禁止 sort 重写整个文件**。
library;

export 'src/i18n/bidi.dart'
    show AlwaysLtr, kHourTicks, hourTickLabel, ltr, stripIsolates;
export 'src/i18n/controller.dart'
    show
        AidogI18n,
        I18nController,
        flutterLocaleOf,
        i18n,
        kLocaleSettingKey,
        kLocaleSettingScope,
        kSupportedFlutterLocales;
export 'src/i18n/locales.dart'
    show
        isRtlLocale,
        kAllLocales,
        kDefaultLocale,
        kFallbackLocale,
        kRtlLocales,
        localeFromPlatform,
        normalizeLocale,
        textDirectionOf;
export 'src/i18n/translations.dart'
    show Translations, interpolate, kLocaleAssetDir, translate;
