/// `lib/src/utils/pinyin.dart` 的单测 —— 断言逐条翻译自 React 版
/// `src/utils/pinyin.test.ts`（pinyin-pro 语义，数据与期望值一字不改）。
library;

import 'package:aidog_flutter/src/utils/pinyin.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('pinyinMatch（pinyin.test.ts 逐条翻译）', () {
    test('空查询匹配一切', () {
      expect(pinyinMatch('', 'anything'), isTrue);
      expect(pinyinMatch('   ', 'anything'), isTrue);
    });

    test('直接子串（不分大小写）', () {
      expect(pinyinMatch('GLM', 'GLM-4'), isTrue);
      expect(pinyinMatch('glm', 'GLM-4'), isTrue);
      expect(pinyinMatch('lian', '百炼'), isTrue); // 命中全拼 bailian
    });

    test('中文目标转全拼', () {
      expect(pinyinMatch('bailian', '百炼'), isTrue);
      expect(pinyinMatch('bai', '百炼'), isTrue);
      expect(pinyinMatch('xiaomi', '小米'), isTrue);
    });

    test('中文 query 转拼音再匹配', () {
      expect(pinyinMatch('百', '百炼'), isTrue);
      expect(pinyinMatch('炼', '百炼'), isTrue);
    });

    test('中英混合', () {
      expect(pinyinMatch('百lian', '百炼'), isTrue);
      expect(pinyinMatch('xiao米', '小米'), isTrue);
    });

    test('都不命中 → false', () {
      expect(pinyinMatch('zzz', '百炼'), isFalse);
    });

    test('非中文字符原样保留', () {
      expect(pinyinMatch('xiaomiai', '小米AI'), isTrue);
    });
  });

  group('拼音首字母（pinyin.test.ts 逐条翻译）', () {
    test('bl → 百炼 / yzam → 月之暗面 / zp → 智谱 / xm → 小米', () {
      expect(pinyinMatch('bl', '百炼'), isTrue);
      expect(pinyinMatch('yzam', '月之暗面'), isTrue);
      expect(pinyinMatch('zp', '智谱'), isTrue);
      expect(pinyinMatch('xm', '小米'), isTrue);
    });

    test('首字母子串也命中（zh → 智谱）', () {
      expect(pinyinMatch('zh', '智谱'), isTrue);
    });

    test('拉丁 target 不走首字母分支', () {
      expect(pinyinMatch('zp', 'GLM'), isFalse);
      expect(pinyinMatch('gl', 'GLM'), isTrue);
    });

    test('含中文的 query 不触发首字母分支', () {
      expect(pinyinMatch('百', '智谱'), isFalse);
    });
  });

  group('词典边界', () {
    test('3500 常用字外：原样保留、不抛错', () {
      // 㵘（U+3D58）在《现代汉语常用字表》之外，词典查不到 → 按原字符处理。
      expect(pinyinMatch('㵘', '㵘测试'), isTrue);
      expect(toPinyin('㵘'), '㵘');
    });

    test('首字母串只含汉字的首字母，拉丁字符跳过', () {
      expect(toInitials('小米AI'), 'xm');
      expect(toPinyin('小米AI'), 'xiaomiai');
    });

    test('同一 target 重复转换走缓存，结果稳定', () {
      expect(pinyinMatch('fenzu', '分组'), isTrue);
      expect(pinyinMatch('fenzu', '分组'), isTrue);
      expect(pinyinMatch('fz', '分组'), isTrue);
    });
  });
}
