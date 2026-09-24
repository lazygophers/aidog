/// 平台 logo 的四级回退（`platform_logo.dart`）。
///
/// 回归 2026-09-22：Flutter 只有「缓存图 → 两字母」两级，而 React 是四级
/// （`PlatformCard.tsx:183-184,238-249`）。后果是**没同步过 logo 的平台卡片
/// 永远是两个字母**，哪怕内置资源里就躺着这个协议的图。
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_test/flutter_test.dart';

/// 拼一个最小可用的 ICO：一帧，内容是 PNG 的 magic + 若干填充。
/// 只用来验「能不能从容器里把 PNG 那段抠出来」，不要求它是一张能渲染的图。
Uint8List _icoWithPng(List<int> pngBody) {
  const pngMagic = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
  final frame = <int>[...pngMagic, ...pngBody];
  final out = BytesBuilder();
  final head = ByteData(6)
    ..setUint16(0, 0, Endian.little) // 保留位
    ..setUint16(2, 1, Endian.little) // 1 = ICO
    ..setUint16(4, 1, Endian.little); // 帧数
  out.add(head.buffer.asUint8List());
  final entry = ByteData(16)
    ..setUint32(8, frame.length, Endian.little)
    ..setUint32(12, 6 + 16, Endian.little); // 数据紧跟在目录项之后
  out.add(entry.buffer.asUint8List());
  out.add(frame);
  return out.toBytes();
}

void main() {
  group('内置 svg（第 2 级）', () {
    test('直接命中：16 个内置协议各自有资产路径', () {
      expect(
        bundledLogoAsset('anthropic'),
        'packages/aidog_platform_logos/anthropic.svg',
      );
      expect(
        bundledLogoAsset('openai'),
        'packages/aidog_platform_logos/openai.svg',
      );
    });

    test('别名命中：变体协议共用主协议的图（index.ts:11-22 逐条对齐）', () {
      expect(
        bundledLogoAsset('openai_responses'),
        'packages/aidog_platform_logos/openai.svg',
      );
      expect(
        bundledLogoAsset('kimi_en'),
        'packages/aidog_platform_logos/kimi.svg',
      );
      expect(
        bundledLogoAsset('bailian_coding'),
        'packages/aidog_platform_logos/bailian.svg',
      );
    });

    test('coding 协议除 bailian_coding 外无内置图（与 React 共同空白）', () {
      // React `src/assets/platforms/index.ts` 的 SVG_URLS/ALIASES 同样没有
      // kimi_coding / glm_coding / minimax_coding —— 两侧一致靠 favicon 兜底，
      // 不算 Flutter 缺口；要补得连 React 一起补。
      expect(bundledLogoAsset('bailian_coding'),
          'packages/aidog_platform_logos/bailian.svg');
      expect(bundledLogoAsset('kimi_coding'), isNull);
      expect(bundledLogoAsset('glm_coding'), isNull);
    });

    test('别名指向一个没有内置图的协议 → null，不返回不存在的资产路径', () {
      expect(bundledLogoAsset('glm_coding_en'), isNull);
      expect(bundledLogoAsset('xiaomi_mimo_coding_en'), isNull);
      expect(bundledLogoAsset('没这个协议'), isNull);
    });
  });

  group('favicon（第 3 级）', () {
    test('从 base_url 取 origin 拼 /favicon.ico', () {
      expect(
        faviconUrl('https://api.anthropic.com/v1'),
        'https://api.anthropic.com/favicon.ico',
      );
      expect(
        faviconUrl('http://127.0.0.1:9890/proxy'),
        'http://127.0.0.1:9890/favicon.ico',
      );
    });

    test('空串 / 不是 URL → null', () {
      expect(faviconUrl(''), isNull);
      expect(faviconUrl('不是个网址'), isNull);
    });
  });

  group('缓存图（第 1 级）按 mime 分流', () {
    test('认得出 mime', () {
      expect(decodeLogoDataUrl('data:image/png;base64,AAAA')?.mime, 'image/png');
      expect(
        decodeLogoDataUrl('data:image/svg+xml;base64,AAAA')?.mime,
        'image/svg+xml',
      );
      expect(decodeLogoDataUrl('/tmp/x.png'), isNull);
      expect(decodeLogoDataUrl('data:image/png;base64,!!!'), isNull);
    });

    test('SVG 与 PNG 各走各的渲染器', () {
      expect(
        logoWidget(decodeLogoDataUrl('data:image/png;base64,AAAA')),
        isA<Image>(),
      );
      expect(
        logoWidget(decodeLogoDataUrl('data:image/svg+xml;base64,AAAA')),
        isA<SvgPicture>(),
      );
    });

    test('ICO：抠得出内嵌 PNG 就渲染，抠不出返回 null 让它落到下一级', () {
      // `~/.aidog/logos/` 里的 ico 绝大多数是「PNG 装进 ICO 容器」，
      // dart:ui 不认 ICO 但认得那帧 PNG。
      final good = base64Encode(_icoWithPng(List.filled(32, 0)));
      expect(logoWidget((mime: 'image/x-icon', bytes: base64Decode(good))),
          isA<Image>());

      // 老式 BMP 帧的 ico：抠不出 PNG。
      expect(
        logoWidget((mime: 'image/x-icon', bytes: Uint8List(64))),
        isNull,
      );
    });
  });

  group('四级串起来', () {
    test('缓存图能用就用缓存图', () {
      expect(
        platformLogo(
          protocol: 'anthropic',
          cachedDataUrl: 'data:image/png;base64,AAAA',
          baseUrl: 'https://api.anthropic.com',
        ),
        isA<Image>(),
      );
    });

    test('没缓存图 → 落到内置 svg（这条就是用户看到「永远两个字母」的修复点）', () {
      expect(
        platformLogo(
          protocol: 'anthropic',
          cachedDataUrl: null,
          baseUrl: 'https://api.anthropic.com',
        ),
        isA<SvgPicture>(),
      );
    });

    test('没缓存图也没内置图 → 落到 favicon', () {
      expect(
        platformLogo(
          protocol: '某个没有内置图的协议',
          cachedDataUrl: null,
          baseUrl: 'https://example.com/v1',
        ),
        isA<Image>(),
      );
    });

    test('四级全落空 → null，调用方画字母块', () {
      expect(
        platformLogo(
          protocol: '某个没有内置图的协议',
          cachedDataUrl: null,
          baseUrl: '',
        ),
        isNull,
      );
    });
  });
}
