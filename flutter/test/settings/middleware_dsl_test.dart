/// `middleware_dsl.dart` 单测 —— 断言逐条翻译自 React `src/utils/mwDsl.test.ts`
/// （票 05 的 round-trip 与错误定位，数据与期望值一字不改）。
library;

import 'package:aidog_flutter/src/pages/settings/middleware_dsl.dart';
import 'package:flutter_test/flutter_test.dart';

Map<String, Object?> leaf(
  String target,
  String pattern, [
  String matchType = 'contains',
  String field = '',
  String validator = '',
]) => {
      'kind': 'leaf',
      'target': target,
      'field': field,
      'match_type': matchType,
      'pattern': pattern,
      'validator': validator,
    };

void main() {
  group('mwDsl round-trip（mwDsl.test.ts 逐条翻译）', () {
    test('单叶子往返一致', () {
      final t = leaf('request_body', 'foo');
      expect(parseDsl(treeToDsl(t)), t);
    });

    test('嵌套 ALL/ANY + regex/exact + field 往返一致', () {
      final t = {
        'kind': 'all',
        'children': [
          leaf('request_body', r'sk-\w+', 'regex'),
          {
            'kind': 'any',
            'children': [
              leaf('status', '4', 'regex'),
              leaf('response_headers', 'x', 'exact', 'retry-after'),
            ],
          },
        ],
      };
      expect(parseDsl(treeToDsl(t)), t);
    });

    test(r'pattern 含引号 / 换行 / 反斜杠往返一致', () {
      final t = leaf('request_body', 'a"b\\c\nd');
      expect(parseDsl(treeToDsl(t)), t);
    });

    test('单子组在 DSL 中折叠为叶子（语义等价）', () {
      final one = {
        'kind': 'any',
        'children': [leaf('model', 'm', 'exact')],
      };
      expect(
        parseDsl(treeToDsl(one)),
        (one['children'] as List)[0],
      );
    });

    test('NOT 节点往返一致（含嵌套）', () {
      final t = {'kind': 'not', 'child': leaf('model', 'gpt')};
      expect(parseDsl(treeToDsl(t)), t);

      final nested = {
        'kind': 'all',
        'children': [
          leaf('request_body', 'hi'),
          {
            'kind': 'not',
            'child': {
              'kind': 'any',
              'children': [
                leaf('model', 'a', 'exact'),
                leaf('model', 'b', 'exact'),
              ],
            },
          },
        ],
      };
      expect(parseDsl(treeToDsl(nested)), nested);
    });

    test('checksum 尾缀往返一致', () {
      final t = leaf('request_body', r'\d{16}', 'regex', '', 'luhn');
      expect(treeToDsl(t), contains('checksum luhn'));
      expect(parseDsl(treeToDsl(t)), t);
    });

    test('未知校验器名保存前就报错', () {
      expect(
        () => parseDsl(r'request_body regex "\\d{16}" checksum luhnn'),
        throwsA(isA<DslException>().having(
          (e) => '$e',
          'message',
          contains('未知校验器'),
        )),
      );
    });
  });

  group('mwDsl 错误定位', () {
    test('未知 target 报位置', () {
      try {
        parseDsl('foo contains "x"');
        fail('unreachable');
      } on DslException catch (e) {
        expect('$e', contains('未知 target'));
        expect(e.pos, 0);
      }
    });

    test('未闭合字符串报位置', () {
      try {
        parseDsl('request_body contains "x');
        fail('unreachable');
      } on DslException catch (e) {
        expect('$e', contains('未闭合'));
        expect(e.pos, 22);
      }
    });

    test('缺 pattern / 缺右括号 / 空 ALL() 均拒绝', () {
      expect(() => parseDsl('request_body contains'), throwsException);
      expect(
        () => parseDsl('ALL(request_body contains "x"'),
        throwsException,
      );
      expect(() => parseDsl('ALL()'), throwsException);
    });

    test('NOT() 多于一个子条件拒绝', () {
      expect(
        () => parseDsl('NOT(request_body contains "x" model exact "y")'),
        throwsException,
      );
      expect(
        () => parseDsl('NOT request_body contains "x"'),
        throwsException,
      );
    });

    test('条件后多余内容拒绝', () {
      expect(
        () => parseDsl('request_body contains "x" )'),
        throwsA(isA<DslException>().having(
          (e) => '$e',
          'message',
          contains('多余内容'),
        )),
      );
    });
  });

  group('mixedPhase（与 Rust validate_rule_phases 对称）', () {
    test('同侧合法 / 跨侧拒绝', () {
      expect(
        mixedPhase({
          'kind': 'all',
          'children': [
            leaf('request_body', 'a'),
            leaf('request_headers', 'b'),
          ],
        }),
        isNull,
      );
      final err = mixedPhase({
        'kind': 'all',
        'children': [
          leaf('request_body', 'a'),
          leaf('response_body', 'b'),
        ],
      });
      expect(err, isNotNull);
      expect(err, contains('混阶段'));
      // NOT 子树里的叶子也参与检查。
      expect(
        mixedPhase({
          'kind': 'not',
          'child': leaf('response_body', 'x'),
        }),
        isNull,
      );
    });
  });
}
