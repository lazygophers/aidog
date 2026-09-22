/// 票 27 第 2 步的护栏：JSON 解析失败要指到**第几行第几列**。
///
/// 语法高亮 / 行号 / 折叠换成 `re_editor` 之后由包负责，只有报错定位仍是自己的。
/// 纯函数，直接喂文本断言，不必挂界面。
library;

import 'package:aidog_flutter/src/pages/settings/json_text.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('locateJsonError', () {
    test('合法 JSON → null', () {
      expect(locateJsonError('{"a": 1}'), isNull);
      expect(locateJsonError('[1, 2, 3]'), isNull);
    });

    test('空串当合法（= 这个字段没配）', () {
      expect(locateJsonError(''), isNull);
      expect(locateJsonError('   \n  '), isNull);
    });

    test('缺逗号 → 指到出错那一行', () {
      const src =
          '{\n'
          '  "a": 1\n'
          '  "b": 2\n'
          '}';
      final e = locateJsonError(src);
      expect(e, isNotNull);
      // 第 3 行是 `  "b": 2`，解析器在这里发现少了逗号。
      expect(e!.line, 3);
      expect(e.column, greaterThan(0));
    });

    test('首行就错 → 第 1 行', () {
      final e = locateJsonError('nope');
      expect(e, isNotNull);
      expect(e!.line, 1);
    });

    test('label 是 L行:列 + 原因，且不带 Dart 的「at character N」尾巴', () {
      final e = locateJsonError('{\n  "a": }')!;
      expect(e.label, startsWith('L${e.line}:${e.column} '));
      expect(e.label.contains('at character'), isFalse);
      expect(e.message, isNotEmpty);
    });
  });

  group('lineColumnAt', () {
    test('偏移 0 → 1 行 1 列', () {
      expect(lineColumnAt('abc', 0), (line: 1, column: 1));
    });

    test('跨行计数，列从该行行首重新起算', () {
      const src = 'ab\ncd\nef';
      expect(lineColumnAt(src, 3), (line: 2, column: 1));
      expect(lineColumnAt(src, 4), (line: 2, column: 2));
      expect(lineColumnAt(src, 6), (line: 3, column: 1));
    });

    test('越界偏移被夹到文本末尾，不抛', () {
      expect(lineColumnAt('ab', 999).line, 1);
    });
  });
}
