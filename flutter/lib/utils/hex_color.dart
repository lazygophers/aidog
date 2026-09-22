/// 十六进制色值解析。两处用同一份：statusline 的自定义段颜色（用户手填）、
/// 平台卡的协议品牌色（registry `platform.json` 的 `color`）。
library;

import 'dart:ui' show Color;

/// `#RRGGBB` / `#RGB` → `[r,g,b]`（0–255），非法返回 null。
List<int>? hexToRgb(String? hex) {
  if (hex == null || hex.isEmpty) return null;
  var h = hex.trim().replaceFirst(RegExp(r'^#'), '');
  if (h.length == 3) h = h.split('').map((c) => '$c$c').join();
  if (!RegExp(r'^[0-9a-fA-F]{6}$').hasMatch(h)) return null;
  return [
    int.parse(h.substring(0, 2), radix: 16),
    int.parse(h.substring(2, 4), radix: 16),
    int.parse(h.substring(4, 6), radix: 16),
  ];
}

/// 同上，直接给 [Color]（不透明）。非法返回 null 让调用方回落主题色。
Color? parseHexColor(String? hex) {
  final rgb = hexToRgb(hex);
  return rgb == null ? null : Color.fromARGB(255, rgb[0], rgb[1], rgb[2]);
}
