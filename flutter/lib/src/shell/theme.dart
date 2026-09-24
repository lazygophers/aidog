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

/// 窗口底的主色光晕，对齐 React 的 `--app-bg-overlay`（`src/themes/mono.ts:41-46`）。
///
/// React 那边是几层 CSS `radial-gradient` 叠在 `bg` 之上；色值**从 accent 派生**
/// （token 表没有单独的 overlay 色，两侧都这么做，不写第二份字面色）。
/// 没有这层的话 Flutter 的窗口底是一块纯色，与 React 并排一眼能看出不一样。
///
/// CSS 的 `radial-gradient(<rx> <ry> at <x> <y>, …)` 画的是**椭圆**，
/// Flutter 的 [RadialGradient] 是正圆（半径按短边取比例）。这里用
/// `GradientTransform` 把圆按 rx/ry 的比例拉成椭圆，位置 / 透明度 / 收口百分比
/// 逐个照抄，不做「差不多就行」的近似。
List<Gradient> bgOverlays(AidogColors c) {
  final mode = c == AidogColors.dark;
  // (rx, ry, x, y, alpha, 收口停点)：数值与 mono.ts 一一对应。
  const dark = [
    (0.80, 0.50, 0.50, -0.12, 0.10, 0.60),
    (0.56, 0.42, 0.10, 0.20, 0.06, 0.58),
  ];
  const light = [
    (0.72, 0.52, 0.50, -0.10, 0.10, 0.62),
    (0.52, 0.44, 0.92, 0.08, 0.08, 0.60),
    (0.60, 0.50, 0.06, 1.00, 0.06, 0.64),
  ];
  return [
    for (final (rx, ry, x, y, a, stop) in mode ? dark : light)
      RadialGradient(
        // CSS 的百分比位置（0%~100%）映射到 Flutter 的 Alignment（-1~1）。
        center: Alignment(x * 2 - 1, y * 2 - 1),
        radius: rx,
        // 色源是 `glow` 而不是 `accent`：两者曾经是同一个值，直到深色强调色
        // 改成近黑（`#101012`，用户 2026-09-23 定）—— 跟着 accent 走的光晕压在
        // 窗口底上只有 1.006:1，等于没画。光晕的亮度和强调色的色相本来就是两件事。
        colors: [c.glow.withValues(alpha: a), c.glow.withValues(alpha: 0)],
        stops: [0, stop],
        transform: _EllipseY(ry / rx),
      ),
  ];
}

/// 把正圆渐变在 Y 轴上压扁成椭圆（CSS 的 `<rx> <ry>` 两个半径）。
@immutable
class _EllipseY extends GradientTransform {
  const _EllipseY(this.ratio);

  /// ry / rx。1 = 正圆。
  final double ratio;

  @override
  Matrix4? transform(Rect bounds, {TextDirection? textDirection}) {
    // 绕渐变中心缩放，否则压扁会把中心一起挪走。
    final cy = bounds.center.dy;
    return Matrix4.identity()
      ..translateByDouble(0.0, cy, 0.0, 1.0)
      ..scaleByDouble(1.0, ratio, 1.0, 1.0)
      ..translateByDouble(0.0, -cy, 0.0, 1.0);
  }
}

/// `.input` 的描边形状：r8（React --radius-sm）+ 1px。色由调用处给。
OutlineInputBorder _inputBorder(Color color) => OutlineInputBorder(
  borderRadius: BorderRadius.circular(AidogRadius.sm),
  borderSide: BorderSide(color: color),
  // 没有 prefix/suffix 时缺口不参与绘制，显式归零省掉 InputDecorator 的 gap 动画。
  gapPadding: 0,
);

/// Material 的 ThemeData 只是宿主：真正的绘制属性全在 AidogTheme 扩展里。
ThemeData aidogThemeData(AidogMode mode) {
  final ext = AidogTheme.forMode(mode);
  return ThemeData(
    useMaterial3: true,
    brightness: mode == AidogMode.dark ? Brightness.dark : Brightness.light,
    scaffoldBackgroundColor: ext.c.bg,
    canvasColor: ext.c.bg,
    fontFamily: AidogType.familySans,
    // 输入框 = React 的 `.input`（globals.css:433-445）：8/12 内衬 + 1px line 边 +
    // r8 + surface 底。原先全站 InputBorder.none 裸文字行，与 React 的描边盒形态
    // 是两版「不是同一个界面」感的最大单一来源（像素对齐批次二 P1）。统一在主题层
    // 给，新写的输入框不会再漏；个别不要描边的场合自己包 Container 覆盖。
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: ext.c.surface,
      contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      border: _inputBorder(ext.c.line),
      enabledBorder: _inputBorder(ext.c.line),
      // 聚焦描边换 accentEdge（React 是 --border-focus + 3px accent-subtle 光环；
      // 近黑 accent 时代替它发声的是这圈亮边，与选中态 pill 同一条路子）。
      focusedBorder: _inputBorder(ext.c.accentEdge),
      errorBorder: _inputBorder(ext.c.bad),
      focusedErrorBorder: _inputBorder(ext.c.bad),
      disabledBorder: _inputBorder(ext.c.line),
    ),
    // 下拉同理：Material 默认在 `DropdownButton` 底下画一条线，现在各处靠
    // 手写 `underline: SizedBox.shrink()` / `DropdownButtonHideUnderline` 去除，
    // 漏一个就多一条横线。这里给不出全局开关，故保留各处写法，但新增下拉
    // 一律用 `DropdownButtonHideUnderline` 包一层。
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
