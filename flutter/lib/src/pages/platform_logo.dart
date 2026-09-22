/// 平台 logo 的四级回退，对齐 `src/components/platforms/PlatformCard.tsx:183-184,238-249`。
///
/// 顺序（前一级拿不到才走下一级）：
///   1. **本地缓存图**（`~/.aidog/logos/<protocol>.*`，`logo_sync` 同步下来的）
///   2. **内置 svg 资源**（`src/assets/platforms/*.svg`，与 React 同一份文件，
///      经 `aidog_platform_logos` 这个只有资产的 path 包挂进来，不拷贝第二份）
///   3. **站点 favicon**（从 base_url 取 origin 拼 `/favicon.ico`）
///   4. **协议名前两个字母**
///
/// 原先 Flutter 只有 1 和 4，于是没同步过 logo 的平台卡片永远是两个字母 ——
/// 而内置资源里其实躺着 16 个协议的图。
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

/// 别名表：逐条照抄 `src/assets/platforms/index.ts:11-22`。
/// 变体协议（`_en` / `_coding`）共用主协议的图。
const Map<String, String> kLogoAliases = {
  'openai_responses': 'openai',
  'openai_completions': 'openai',
  'glm_en': 'glm',
  'glm_coding_en': 'glm_coding',
  'bailian_en': 'bailian',
  'bailian_coding_en': 'bailian_coding',
  'kimi_en': 'kimi',
  'xiaomi_mimo_coding_en': 'xiaomi_mimo_coding',
  'sensenova_en': 'sensenova',
  'bailian_coding': 'bailian',
};

/// 内置 svg 的文件名（不含扩展名），与 `src/assets/platforms/` 里的文件一一对应。
/// 写成常量而不是运行时列目录：资产清单在编译期就定了，列不出来。
const Set<String> kBundledLogos = {
  'anthropic',
  'bailian',
  'claude_code',
  'cline',
  'doubao',
  'exa',
  'gemini',
  'github-copilot',
  'glm',
  'huggingface',
  'kimi',
  'meta',
  'ollama',
  'openai',
  'pi',
  'sensenova',
};

/// 协议 → 内置 svg 资产路径；没有内置图则 null。
String? bundledLogoAsset(String protocol) {
  final name = kBundledLogos.contains(protocol)
      ? protocol
      : kLogoAliases[protocol];
  if (name == null || !kBundledLogos.contains(name)) return null;
  return 'packages/aidog_platform_logos/$name.svg';
}

/// 从 base_url 取 origin 拼 favicon（`platforms/index.ts:83-96`）。
/// 解析不出 origin → null。
String? faviconUrl(String baseUrl) {
  if (baseUrl.isEmpty) return null;
  final uri = Uri.tryParse(baseUrl);
  if (uri == null || !uri.hasScheme || uri.host.isEmpty) return null;
  return '${uri.scheme}://${uri.authority}/favicon.ico';
}

/// `data:<mime>;base64,...` → (mime, 字节)；不是 data URL（或解不开）→ null。
///
/// **必须把 mime 一起带出来**：`~/.aidog/logos/` 里躺着的不只有 PNG，还有 `.svg`
/// 和 `.ico`（`defaults.rs:64` 按扩展名派生 mime）。`Image.memory` 这两种都解不开，
/// 直接喂进去就是满屏 `Invalid image data` —— 一个平台卡报一次。
({String mime, Uint8List bytes})? decodeLogoDataUrl(String? src) {
  if (src == null || src.isEmpty) return null;
  final i = src.indexOf('base64,');
  if (i < 0) return null;
  final head = src.substring(0, i);
  final colon = head.indexOf(':');
  final semi = head.indexOf(';');
  final mime = (colon >= 0 && semi > colon)
      ? head.substring(colon + 1, semi)
      : '';
  try {
    return (mime: mime, bytes: base64Decode(src.substring(i + 7)));
  } catch (_) {
    return null;
  }
}

/// PNG 的 8 字节magic。ICO 里嵌的多半是 PNG 帧，靠它认出来。
const List<int> _pngMagic = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// 从 `.ico` 里抠出最大的一帧 PNG；抠不出（老式 BMP 帧 / 格式不对）→ null。
///
/// dart:ui 不认 ICO，但 `~/.aidog/logos/` 里的 ico 绝大多数是「PNG 装进 ICO 容器」
/// （`file` 报 `with PNG image data`）。ICO 目录结构很简单，照着抠一帧出来就能用，
/// 比为它引一个纯 Dart 图像解码包划算得多。
///
/// 结构（小端）：6 字节文件头（保留 / 类型 / 帧数），随后每帧 16 字节目录项，
/// 其中 offset 8 处是 4 字节 size、12 处是 4 字节 offset。
Uint8List? extractPngFromIco(Uint8List bytes) {
  if (bytes.length < 6) return null;
  final data = ByteData.sublistView(bytes);
  if (data.getUint16(0, Endian.little) != 0) return null; // 保留位必须为 0
  if (data.getUint16(2, Endian.little) != 1) return null; // 1 = ICO（2 = CUR）
  final count = data.getUint16(4, Endian.little);
  if (count == 0) return null;

  Uint8List? best;
  var bestSize = 0;
  for (var i = 0; i < count; i++) {
    final entry = 6 + i * 16;
    if (entry + 16 > bytes.length) break;
    final size = data.getUint32(entry + 8, Endian.little);
    final offset = data.getUint32(entry + 12, Endian.little);
    if (offset + size > bytes.length || size < _pngMagic.length) continue;
    final frame = Uint8List.sublistView(bytes, offset, offset + size);
    final isPng = () {
      for (var k = 0; k < _pngMagic.length; k++) {
        if (frame[k] != _pngMagic[k]) return false;
      }
      return true;
    }();
    if (isPng && size > bestSize) {
      best = frame;
      bestSize = size;
    }
  }
  return best;
}

/// 按 mime 选渲染方式；认不出的一律返回 null，由调用方走下一级回退。
Widget? logoWidget(({String mime, Uint8List bytes})? logo) {
  if (logo == null) return null;
  if (logo.mime == 'image/svg+xml') {
    return SvgPicture.memory(
      logo.bytes,
      fit: BoxFit.contain,
      placeholderBuilder: (_) => const SizedBox.shrink(),
    );
  }
  if (logo.mime == 'image/x-icon' || logo.mime == 'image/vnd.microsoft.icon') {
    final png = extractPngFromIco(logo.bytes);
    if (png == null) return null;
    return Image.memory(
      png,
      fit: BoxFit.contain,
      errorBuilder: (_, _, _) => const SizedBox.shrink(),
    );
  }
  const raster = {
    'image/png',
    'image/jpeg',
    'image/gif',
    'image/webp',
    'image/bmp',
  };
  if (!raster.contains(logo.mime)) return null;
  return Image.memory(
    logo.bytes,
    fit: BoxFit.contain,
    errorBuilder: (_, _, _) => const SizedBox.shrink(),
  );
}

/// 四级回退拼出来的 logo；四级都拿不到 → null（调用方画字母块）。
Widget? platformLogo({
  required String protocol,
  required String? cachedDataUrl,
  required String baseUrl,
}) {
  // 1. 本地缓存图
  final cached = logoWidget(decodeLogoDataUrl(cachedDataUrl));
  if (cached != null) return cached;

  // 2. 内置 svg
  final asset = bundledLogoAsset(protocol);
  if (asset != null) {
    return SvgPicture.asset(
      asset,
      fit: BoxFit.contain,
      placeholderBuilder: (_) => const SizedBox.shrink(),
    );
  }

  // 3. 站点 favicon。多半是 .ico，dart:ui 解不了 —— 交给 errorBuilder 落到字母块。
  //    仍然留着这一级：有些站点的 /favicon.ico 实际返回的是 PNG。
  final favicon = faviconUrl(baseUrl);
  if (favicon != null) {
    return Image.network(
      favicon,
      fit: BoxFit.contain,
      errorBuilder: (_, _, _) => const SizedBox.shrink(),
    );
  }
  return null;
}
