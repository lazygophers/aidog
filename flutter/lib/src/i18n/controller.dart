import 'dart:ui' show PlatformDispatcher;

import 'package:flutter/widgets.dart';

import '../../transport.dart';
import 'locales.dart';
import 'translations.dart';

/// DB 里存语言偏好的位置，与 React 的 `AppContext` 一字不差。
const String kLocaleSettingScope = 'app';
const String kLocaleSettingKey = 'locale';

/// 当前语言 + 查文案。全局单例 [i18n]，与 I01 的 [kernel] 同一 idiom。
///
/// **文案不走 RPC**：8 份 JSON 是构建期资产，[init] 不需要内核活着 —— 「后端连接中」
/// 那一屏自己就得是翻译好的，走 RPC 会让它只能显英文或裸 key。内核起来之后再用
/// [loadFromBackend] 把 DB 里的偏好盖上来。
class I18nController extends ChangeNotifier {
  /// [persist] 是写回 DB 的动作，缺省写内核。留这个口子只为了让单测能在没有内核的
  /// 情况下切语言 —— [kernel.invoke] 在内核没起来时会一直等。
  I18nController({Future<void> Function(String locale)? persist})
    : _persist = persist ?? _persistToKernel;

  final Future<void> Function(String locale) _persist;

  String _locale = kDefaultLocale;
  Translations? _current;
  Translations? _fallback;

  String get locale => _locale;
  bool get ready => _current != null && _fallback != null;
  TextDirection get textDirection => textDirectionOf(_locale);
  Locale get flutterLocale => flutterLocaleOf(_locale);

  /// 首屏之前调一次（`main()` 里 await）。[initial] 缺省取系统语言。
  Future<void> init({String? initial, AssetBundle? bundle}) async {
    _fallback = await Translations.load(kFallbackLocale, bundle: bundle);
    await _apply(
      initial ?? localeFromPlatform(PlatformDispatcher.instance.locale),
      bundle: bundle,
    );
  }

  /// 切语言：换资源 + 通知重建 + 写回 DB（后端也要用它出代理错误消息）。
  /// DB 写失败不回滚 UI —— 与 React 的 best-effort 一致。
  Future<void> setLocale(String raw, {AssetBundle? bundle}) async {
    await _apply(raw, bundle: bundle);
    try {
      await _persist(_locale);
    } catch (_) {
      // DB 写失败只影响后端错误消息的语言，不该把界面按回去。
    }
  }

  /// 内核连上之后调：DB 是权威源，读不到就保持当前语言。
  Future<void> loadFromBackend({AssetBundle? bundle}) async {
    try {
      final row = await kernel.invoke<Map<String, dynamic>?>('settings_get', {
        'scope': kLocaleSettingScope,
        'key': kLocaleSettingKey,
      });
      final stored = row?['locale'];
      if (stored == null) return;
      await _apply(normalizeLocale(stored), bundle: bundle);
    } catch (_) {
      // 读不到就用系统语言，不阻断启动。
    }
  }

  Future<void> _apply(String raw, {AssetBundle? bundle}) async {
    final next = normalizeLocale(raw);
    if (next == _locale && _current != null) return;
    _current = await Translations.load(next, bundle: bundle);
    _locale = next;
    notifyListeners();
  }

  /// 查文案。[init] 之前调会抛 —— 宁可当场炸，也别静默出裸 key。
  String t(String key, [Map<String, Object?>? args]) {
    final current = _current;
    final fallback = _fallback;
    if (current == null || fallback == null) {
      throw StateError('i18n.init() 还没跑完就调了 t("$key")');
    }
    return translate(key, current, fallback, args: args);
  }
}

Future<void> _persistToKernel(String locale) => kernel.invoke<void>(
  'settings_set',
  {
    'input': {
      'scope': kLocaleSettingScope,
      'key': kLocaleSettingKey,
      'value': {'locale': locale},
    },
  },
);

/// 全局单例。
final I18nController i18n = I18nController();

Locale flutterLocaleOf(String locale) => switch (locale) {
  'zh-Hans' => const Locale.fromSubtags(
    languageCode: 'zh',
    scriptCode: 'Hans',
  ),
  _ => Locale(locale.split('-')[0], locale.split('-')[1]),
};

/// 8 种语言对应的 Flutter [Locale]，给 `MaterialApp.supportedLocales`。
List<Locale> get kSupportedFlutterLocales =>
    kAllLocales.map(flutterLocaleOf).toList(growable: false);

/// 挂在应用根上：定下 [Directionality]，并让**通过 context 取词的后代**在切语言时重建。
///
/// 用 [InheritedNotifier] 而不是 `ListenableBuilder(child: ...)`：后者把 `child` 从
/// builder 外面捕获，语言变了传下去的还是同一个 widget 实例，Flutter 的 `Element.update`
/// 看到同一实例直接跳过重建 —— 方向翻了、文案却还是旧的。
///
/// 后代要在切语言时自动更新，必须**经由 context 取词**：
/// ```dart
/// Text(AidogI18n.of(context).t('common.cancel'))
/// ```
/// 直接读全局 `i18n.t(...)` 的地方不会订阅，只有祖先恰好重建时才跟着变。
class AidogI18n extends StatelessWidget {
  const AidogI18n({super.key, required this.child, this.controller});

  final Widget child;
  final I18nController? controller;

  /// 取当前语言控制器，并让调用方订阅它的变化。
  static I18nController of(BuildContext context) {
    final scope =
        context.dependOnInheritedWidgetOfExactType<_I18nScope>();
    assert(scope != null, 'AidogI18n.of() 找不到祖先 AidogI18n');
    return scope!.controller;
  }

  @override
  Widget build(BuildContext context) {
    final c = controller ?? i18n;
    return _I18nScope(
      controller: c,
      child: ListenableBuilder(
        listenable: c,
        builder: (context, _) =>
            Directionality(textDirection: c.textDirection, child: child),
      ),
    );
  }
}

class _I18nScope extends InheritedNotifier<I18nController> {
  const _I18nScope({required this.controller, required super.child})
      : super(notifier: controller);

  final I18nController controller;
}
