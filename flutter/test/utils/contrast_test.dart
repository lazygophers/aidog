// 对比度工具：生产代码与护栏测试共用同一份实现，别各写各的。
library;

import 'dart:ui';

import 'package:aidog_flutter/src/utils/contrast.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('极值：纯黑对纯白 21:1，同色 1:1', () {
    const black = Color(0xFF000000);
    const white = Color(0xFFFFFFFF);
    expect(contrastRatio(black, white), closeTo(21, 0.01));
    expect(contrastRatio(white, white), closeTo(1, 0.001));
  });

  test('与顺序无关：谁在前谁在后算出来一样', () {
    const a = Color(0xFF101012);
    const b = Color(0xFF08090A);
    expect(contrastRatio(a, b), closeTo(contrastRatio(b, a), 1e-12));
  });

  test('近黑强调色压在窗口底上约 1.05:1 —— 这就是需要描边层的原因', () {
    expect(
      contrastRatio(const Color(0xFF101012), const Color(0xFF08090A)),
      closeTo(1.05, 0.01),
    );
  });

  group('compositeOver', () {
    test('全不透明 → 原样返回前景', () {
      const fg = Color(0xFF5E6AD2);
      const bg = Color(0xFF08090A);
      final out = compositeOver(fg, bg);
      expect(out.r, closeTo(fg.r, 1e-6));
      expect(out.g, closeTo(fg.g, 1e-6));
      expect(out.b, closeTo(fg.b, 1e-6));
    });

    test('全透明 → 原样返回底色', () {
      const bg = Color(0xFF111214);
      final out = compositeOver(const Color(0x00FFFFFF), bg);
      expect(out.r, closeTo(bg.r, 1e-6));
    });

    test('白 34% 压在近黑 accent 上 → 对窗口底过 3:1（描边层成立的依据）', () {
      // accentEdge = rgba(255,255,255,.34)，accent = #101012，窗口底 = #08090A
      final edge = compositeOver(
        const Color(0x57FFFFFF),
        const Color(0xFF101012),
      );
      expect(contrastRatio(edge, const Color(0xFF08090A)), greaterThan(3.0));
    });
  });
}
