/// 主题：色值**只**来自 `package:aidog_tokens/tokens.dart`（由 design/tokens/tokens.json
/// 生成，`yarn check:tokens` 逐字节校验）。本文件不写任何色值字面量 —— 改色请改 tokens.json
/// 后跑 `yarn tokens`。
///
/// 两套模式，**默认深色、手动切、不跟随系统**（票 10 用户 2026-09-18 定）。
/// 浅色规则「深色用光，浅色用色」由 token 表自己表达：`live-halo` 在 light 下是 `none`，
/// 由更饱和的 `live-fill` + 更实的 `live-edge` 接替。这里只负责把它解析成 BoxShadow。
library;

import 'package:aidog_tokens/tokens.dart';
import 'package:flutter/material.dart';

export 'package:aidog_tokens/tokens.dart';

enum AidogMode { dark, light }

/// 把 token 表里的 CSS 阴影串解析成 Flutter 阴影。
///
/// token 表用的是 CSS 语法（一份真值源同时喂 Web 与 Flutter，见 build-tokens.mjs），
/// 所以这一步是必要的翻译，不是重复定义。支持的形状就是表里实际出现的两种：
/// `none` 和 `<x> <y> <blur> [<spread>] rgba(r,g,b,a)`。
List<BoxShadow> parseShadow(String css) {
  final s = css.trim();
  if (s.isEmpty || s == 'none') return const [];
  return s.split(RegExp(r',(?![^(]*\))')).map(_parseOne).toList(growable: false);
}

BoxShadow _parseOne(String raw) {
  final s = raw.trim();
  final colorMatch = RegExp(
    r'rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)',
  ).firstMatch(s);
  if (colorMatch == null) {
    throw FormatException('阴影缺少 rgb/rgba 颜色：$raw');
  }
  final a = double.parse(colorMatch.group(4) ?? '1');
  final color = Color.fromARGB(
    (a * 255).round(),
    int.parse(colorMatch.group(1)!),
    int.parse(colorMatch.group(2)!),
    int.parse(colorMatch.group(3)!),
  );
  // 单位可省：CSS 里零长度写成裸 `0`（tokens.json 的 live-halo 就是 `0 0 18px -6px`）。
  final lengths = RegExp(r'(-?[\d.]+)(?:px)?(?=\s|$)')
      .allMatches(s.substring(0, colorMatch.start))
      .map((m) => double.parse(m.group(1)!))
      .toList();
  if (lengths.length < 3) {
    throw FormatException('阴影至少要有 x/y/blur 三个长度：$raw');
  }
  return BoxShadow(
    color: color,
    offset: Offset(lengths[0], lengths[1]),
    blurRadius: lengths[2],
    spreadRadius: lengths.length > 3 ? lengths[3] : 0,
  );
}

/// 一套模式下的全部可画属性。页面一律走 `AidogTheme.of(context)`，不直接摸 AidogColors。
@immutable
class AidogTheme extends ThemeExtension<AidogTheme> {
  const AidogTheme._({
    required this.mode,
    required this.c,
    required this.liveHalo,
    required this.liveRing,
    required this.shadowTile,
    required this.shadowFloat,
  });

  factory AidogTheme.forMode(AidogMode mode) {
    final c = mode == AidogMode.dark ? AidogColors.dark : AidogColors.light;
    return AidogTheme._(
      mode: mode,
      c: c,
      liveHalo: parseShadow(c.liveHalo),
      liveRing: parseShadow(c.liveRing),
      shadowTile: parseShadow(c.shadowTile),
      shadowFloat: parseShadow(c.shadowFloat),
    );
  }

  final AidogMode mode;
  final AidogColors c;

  /// 「只有活着的东西才发光」：深色下非空，**浅色下必为空**（token `live-halo: none`）。
  final List<BoxShadow> liveHalo;
  final List<BoxShadow> liveRing;
  final List<BoxShadow> shadowTile;
  final List<BoxShadow> shadowFloat;

  static AidogTheme of(BuildContext context) {
    final t = Theme.of(context).extension<AidogTheme>();
    assert(t != null, 'AidogTheme 未注入：外层要有 AidogShellApp / aidogThemeData');
    return t!;
  }

  @override
  AidogTheme copyWith({AidogMode? mode}) => AidogTheme.forMode(mode ?? this.mode);

  /// 深浅之间不做色值插值：切换是一次离散的语言切换（发光 ↔ 上色），
  /// 中间帧的混合色不属于任何一套 token。
  @override
  AidogTheme lerp(ThemeExtension<AidogTheme>? other, double t) =>
      t < 0.5 ? this : (other as AidogTheme? ?? this);
}

/// Material 的 ThemeData 只是宿主：真正的绘制属性全在 AidogTheme 扩展里。
ThemeData aidogThemeData(AidogMode mode) {
  final ext = AidogTheme.forMode(mode);
  return ThemeData(
    useMaterial3: true,
    brightness: mode == AidogMode.dark ? Brightness.dark : Brightness.light,
    scaffoldBackgroundColor: ext.c.bg,
    canvasColor: ext.c.bg,
    fontFamily: AidogType.familySans,
    extensions: [ext],
  );
}

/// 手动切的主题开关。**不读系统**：`MaterialApp.themeMode` 恒为 light，
/// 由这里给出的 `data` 决定深浅，系统切深浅时界面不动。
class ThemeController extends ChangeNotifier {
  ThemeController({AidogMode initial = AidogMode.dark}) : _mode = initial;

  AidogMode _mode;
  AidogMode get mode => _mode;
  bool get isDark => _mode == AidogMode.dark;

  ThemeData get data => aidogThemeData(_mode);

  void toggle() => set(isDark ? AidogMode.light : AidogMode.dark);

  void set(AidogMode next) {
    if (next == _mode) return;
    _mode = next;
    notifyListeners();
  }
}
