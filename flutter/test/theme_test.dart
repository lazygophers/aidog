// 主题：两套模式、默认深色、不跟随系统，以及「深色用光，浅色用色」这条规则的机器验证。

import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('ThemeController', () {
    test('默认深色（票 10 用户 2026-09-18 定：不跟随系统）', () {
      expect(ThemeController().mode, AidogMode.dark);
      expect(ThemeController().isDark, isTrue);
    });

    test('toggle 在深浅之间来回，并通知监听方', () {
      final c = ThemeController();
      var notified = 0;
      c.addListener(() => notified++);

      c.toggle();
      expect(c.mode, AidogMode.light);
      c.toggle();
      expect(c.mode, AidogMode.dark);
      expect(notified, 2);

      // 设成同一个模式不算变化，不空转通知。
      c.set(AidogMode.dark);
      expect(notified, 2);
    });

    test('ThemeData 的底色与亮度跟着模式走', () {
      final c = ThemeController();
      expect(c.data.brightness, Brightness.dark);
      expect(c.data.scaffoldBackgroundColor, AidogColors.dark.bg);
      c.toggle();
      expect(c.data.brightness, Brightness.light);
      expect(c.data.scaffoldBackgroundColor, AidogColors.light.bg);
    });
  });

  group('深色用光，浅色用色', () {
    test('live-halo 深色非空、浅色为 none', () {
      expect(AidogTheme.forMode(AidogMode.dark).liveHalo, isNotEmpty);
      expect(AidogTheme.forMode(AidogMode.light).liveHalo, isEmpty);
    });

    test('浅色下 live-edge 更实、live-fill 底色更深（halo 的接替者）', () {
      final dark = AidogColors.dark;
      final light = AidogColors.light;
      // 不透明度：edge 在浅色下明显更实（tokens.json: light .40 vs dark .25）。
      expect(light.liveEdge.a, greaterThan(dark.liveEdge.a));
      // 底色更深：light live-fill 是 rgb(78,89,196)，dark 是 rgb(94,106,210)。
      // 注意这里度量的是**亮度**不是 HSL 饱和度 —— tokens.json 实现「浅色用色」靠的是
      // 压深底色（78,89,196 比 94,106,210 深），而它的 HSL 饱和度反而更低。
      // 拿饱和度当度量会红，且红的是度量选错、不是 token 写错。
      expect(
        HSLColor.fromColor(light.liveFill).lightness,
        lessThan(HSLColor.fromColor(dark.liveFill).lightness),
      );
    });

    test('shadow-tile 反过来：浅色有、深色 none', () {
      expect(AidogTheme.forMode(AidogMode.dark).shadowTile, isEmpty);
      expect(AidogTheme.forMode(AidogMode.light).shadowTile, isNotEmpty);
    });
  });

  group('parseShadow（CSS 阴影串 → BoxShadow）', () {
    test('none / 空串 → 空列表', () {
      expect(parseShadow('none'), isEmpty);
      expect(parseShadow('  '), isEmpty);
    });

    test('四长度 + rgba：x / y / blur / spread / 颜色都对', () {
      final s = parseShadow('0 0 18px -6px rgba(94,106,210,.55)').single;
      expect(s.offset, const Offset(0, 0));
      expect(s.blurRadius, 18);
      expect(s.spreadRadius, -6);
      expect(s.color.r * 255, closeTo(94, 1));
      expect(s.color.g * 255, closeTo(106, 1));
      expect(s.color.b * 255, closeTo(210, 1));
      expect(s.color.a, closeTo(0.55, 0.01));
    });

    test('三长度（无 spread）时 spread 归零，负偏移不丢符号', () {
      final s = parseShadow('0 18px 44px rgba(9,10,12,.22)').single;
      expect(s.offset, const Offset(0, 18));
      expect(s.spreadRadius, 0);
      final n = parseShadow('-2px -3px 4px rgba(0,0,0,1)').single;
      expect(n.offset, const Offset(-2, -3));
    });

    test('逗号分隔的多段阴影各自解析（括号内的逗号不算分隔）', () {
      final list = parseShadow(
        '0 1px 2px rgba(9,10,12,.05), 0 0 0 3px rgba(46,147,103,.18)',
      );
      expect(list, hasLength(2));
      expect(list[1].spreadRadius, 3);
    });

    test('缺颜色 / 缺长度都报错，不静默回落', () {
      expect(() => parseShadow('0 0 4px'), throwsFormatException);
      expect(() => parseShadow('0 rgba(1,2,3,1)'), throwsFormatException);
    });

    test('token 表里现存的四个阴影串都能解析', () {
      for (final c in [AidogColors.dark, AidogColors.light]) {
        for (final raw in [c.liveHalo, c.liveRing, c.shadowTile, c.shadowFloat]) {
          expect(() => parseShadow(raw), returnsNormally, reason: raw);
        }
      }
    });
  });
}
