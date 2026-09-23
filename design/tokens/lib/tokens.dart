// 由 build-tokens.mjs 从 tokens.json 生成，禁止手改。
// 用法：AidogColors.dark / AidogColors.light 喂进 ThemeExtension，
// 尺寸与字阶走 AidogSpace / AidogRadius / AidogLayout / AidogType / AidogMotion（与模式无关）。
import 'package:flutter/widgets.dart';

@immutable
class AidogColors {
  final Color bg;
  final Color bgChrome;
  final Color surface;
  final Color surface2;
  final Color line;
  final Color lineStrong;
  final Color fg;
  final Color fg2;
  final Color fg3;
  final Color accent;
  final Color accentText;
  final Color accentWash;
  final Color ok;
  final Color peak;
  final Color peakIdle;
  final Color bad;
  final Color liveFill;
  final Color liveEdge;
  final String liveHalo; // 阴影/none，按平台自行解析
  final String liveRing; // 阴影/none，按平台自行解析
  final String shadowTile; // 阴影/none，按平台自行解析
  final String shadowFloat; // 阴影/none，按平台自行解析

  const AidogColors({
    required this.bg,
    required this.bgChrome,
    required this.surface,
    required this.surface2,
    required this.line,
    required this.lineStrong,
    required this.fg,
    required this.fg2,
    required this.fg3,
    required this.accent,
    required this.accentText,
    required this.accentWash,
    required this.ok,
    required this.peak,
    required this.peakIdle,
    required this.bad,
    required this.liveFill,
    required this.liveEdge,
    required this.liveHalo,
    required this.liveRing,
    required this.shadowTile,
    required this.shadowFloat,
  });

  static const dark = AidogColors(
    bg: Color(0xFF08090A),
    bgChrome: Color(0xFF0C0D0F),
    surface: Color(0xFF111214),
    surface2: Color(0xFF16171A),
    line: Color(0x12FFFFFF),
    lineStrong: Color(0x1FFFFFFF),
    fg: Color(0xFFF7F8F8),
    fg2: Color(0xFF9BA1A6),
    fg3: Color(0xFF62676C),
    accent: Color(0xFF101012),
    accentText: Color(0xFF8A91E8),
    accentWash: Color(0x295E6AD2),
    ok: Color(0xFF4CB782),
    peak: Color(0xFFF2C94C),
    peakIdle: Color(0x0DFFFFFF),
    bad: Color(0xFFEB5757),
    liveFill: Color(0x215E6AD2),
    liveEdge: Color(0x405E6AD2),
    liveHalo: "0 0 18px -6px rgba(94,106,210,.55)",
    liveRing: "0 0 0 3px rgba(76,183,130,.16)",
    shadowTile: "none",
    shadowFloat: "0 24px 64px -16px rgba(0,0,0,.9)",
  );

  static const light = AidogColors(
    bg: Color(0xFFF4F5F7),
    bgChrome: Color(0xFFFAFAFB),
    surface: Color(0xFFFFFFFF),
    surface2: Color(0xFFF1F2F4),
    line: Color(0x1A090A0C),
    lineStrong: Color(0x2B090A0C),
    fg: Color(0xFF14161A),
    fg2: Color(0xFF5B6169),
    fg3: Color(0xFF8A9099),
    accent: Color(0xFF4E59C4),
    accentText: Color(0xFF3F47A6),
    accentWash: Color(0x1A4E59C4),
    ok: Color(0xFF2E9367),
    peak: Color(0xFFE9A400),
    peakIdle: Color(0xFFE7E9EC),
    bad: Color(0xFFC93B3B),
    liveFill: Color(0x174E59C4),
    liveEdge: Color(0x664E59C4),
    liveHalo: "none",
    liveRing: "0 0 0 3px rgba(46,147,103,.18)",
    shadowTile: "0 1px 2px rgba(9,10,12,.05)",
    shadowFloat: "0 18px 44px -14px rgba(9,10,12,.22)",
  );
}

class AidogSpace {
  static const double sxs = 4.0;
  static const double ssm = 6.0;
  static const double smd = 10.0;
  static const double slg = 14.0;
  static const double sxl = 18.0;
  static const double s_2xl = 24.0;
}

class AidogRadius {
  static const double sm = 6.0;
  static const double md = 8.0;
  static const double lg = 12.0;
  static const double xl = 14.0;
  static const double pill = 999.0;
}

class AidogLayout {
  static const double railW = 200.0;
  static const double railWCollapsed = 56.0;
  static const double titlebarH = 40.0;
  static const double gridCols = 12.0;
  static const double gridGap = 10.0;
  static const double tilePadX = 16.0;
  static const double tilePadY = 14.0;
  static const double pagePad = 18.0;
  static const double rowH = 9.0;
  static const double contentMax = 1180.0;
  static const double trayW = 328.0;
  static const double breakpointNarrow = 940.0;
}

class AidogType {
  static const familySans = 'Inter';
  static const familyMono = 'JetBrains Mono';
  static const display = TextStyle(fontFamily: familySans, fontSize: 24.0, fontWeight: FontWeight.w600, letterSpacing: -0.60, height: 1.55);
  static const title = TextStyle(fontFamily: familySans, fontSize: 16.5, fontWeight: FontWeight.w600, letterSpacing: -0.25, height: 1.55);
  static const tile = TextStyle(fontFamily: familySans, fontSize: 13.5, fontWeight: FontWeight.w600, letterSpacing: 0.00, height: 1.55);
  static const body = TextStyle(fontFamily: familySans, fontSize: 15.0, fontWeight: FontWeight.w400, letterSpacing: -0.16, height: 1.55);
  static const label = TextStyle(fontFamily: familySans, fontSize: 13.5, fontWeight: FontWeight.w400, letterSpacing: 0.00, height: 1.55);
  static const caption = TextStyle(fontFamily: familySans, fontSize: 12.5, fontWeight: FontWeight.w400, letterSpacing: 0.00, height: 1.55);
  static const micro = TextStyle(fontFamily: familySans, fontSize: 11.0, fontWeight: FontWeight.w500, letterSpacing: 0.66, height: 1.55);
  static const numXl = TextStyle(fontFamily: familyMono, fontSize: 28.0, fontWeight: FontWeight.w600, letterSpacing: -0.84, height: 1.35);
  static const numLg = TextStyle(fontFamily: familyMono, fontSize: 18.5, fontWeight: FontWeight.w600, letterSpacing: -0.55, height: 1.35);
  static const numMd = TextStyle(fontFamily: familyMono, fontSize: 13.5, fontWeight: FontWeight.w400, letterSpacing: -0.27, height: 1.35);
  static const numSm = TextStyle(fontFamily: familyMono, fontSize: 12.5, fontWeight: FontWeight.w400, letterSpacing: -0.25, height: 1.35);
}

class AidogMotion {
  static const instant = Duration(milliseconds: 0);
  static const fast = Duration(milliseconds: 140);
  static const base = Duration(milliseconds: 180);
  static const slow = Duration(milliseconds: 260);
  static const breathe = Duration(milliseconds: 3000);
  static const easeStandard = Cubic(0.20, 0.00, 0.00, 1.00);
  static const easeExit = Cubic(0.40, 0.00, 1.00, 1.00);
}
