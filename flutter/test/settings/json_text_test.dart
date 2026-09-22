/// 票 27 第 2 步的护栏：JSON 解析失败要指到**第几行第几列**，分词器要能把
/// 键 / 字符串 / 数字 / 字面量 / 标点分开（高亮就建在它上面）。
///
/// 这两件都是纯函数，直接喂文本断言，不必挂界面。
library;

import 'package:aidog_flutter/src/pages/settings/json_text.dart';
import 'package:flutter_test/flutter_test.dart';

/// 取某一类 token 对应的原文，方便逐条断言。
List<String> textsOf(String src, JsonTokenKind kind) => [
  for (final t in tokenizeJson(src))
    if (t.kind == kind) src.substring(t.start, t.end),
];

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

  group('tokenizeJson', () {
    test('键与普通字符串分开：跟冒号的才是键', () {
      const src = '{"a": "b"}';
      expect(textsOf(src, JsonTokenKind.key), ['"a"']);
      expect(textsOf(src, JsonTokenKind.string), ['"b"']);
    });

    test('数组里的字符串都不是键', () {
      const src = '["a", "b"]';
      expect(textsOf(src, JsonTokenKind.key), isEmpty);
      expect(textsOf(src, JsonTokenKind.string), ['"a"', '"b"']);
    });

    test('数字含负号 / 小数 / 指数', () {
      const src = '[-1, 2.5, 3e10]';
      expect(textsOf(src, JsonTokenKind.number), ['-1', '2.5', '3e10']);
    });

    test('true / false / null 归字面量，别的词归 plain', () {
      const src = '[true, false, null, nope]';
      expect(textsOf(src, JsonTokenKind.literal), ['true', 'false', 'null']);
      expect(textsOf(src, JsonTokenKind.plain), contains('nope'));
    });

    test('字符串里的转义引号不截断 token', () {
      const src = r'{"a": "x\"y"}';
      expect(textsOf(src, JsonTokenKind.string), [r'"x\"y"']);
    });

    test('半成品文本照样切得动（用户正在打字）', () {
      // 未闭合的字符串、悬空的冒号：不抛，剩下的部分照常分类。
      expect(() => tokenizeJson('{"a": "unclosed'), returnsNormally);
      expect(() => tokenizeJson('{'), returnsNormally);
      expect(textsOf('{"a": "unclosed', JsonTokenKind.key), ['"a"']);
    });

    test('token 连续覆盖全文，不漏字不重叠', () {
      const src = '{\n  "a": [1, true],\n  "b": "c"\n}';
      final tokens = tokenizeJson(src);
      var cursor = 0;
      for (final t in tokens) {
        expect(t.start, greaterThanOrEqualTo(cursor));
        expect(t.end, greaterThan(t.start));
        cursor = t.end;
      }
      expect(cursor, src.length);
    });
  });
}
