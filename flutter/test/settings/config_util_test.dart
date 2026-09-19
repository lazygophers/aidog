/// React 侧三份 vitest 的断言逐条翻译，**数据与期望值一字不改**：
///
/// - `src/utils/deepMerge.test.ts`
/// - `src/services/settings-env-pairs.test.ts`
/// - `src/components/settings/applySelectedPaths.test.ts`
///
/// 用例标题也照抄，便于两边对照。加出来的只有 [stableStringify]
/// 那一组（React 侧没有独立测试，但它是「脏了没有」的判据，错了整页的保存按钮就废了）。
library;

import 'package:aidog_flutter/src/pages/settings/config_util.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('deepMerge', () {
    test('override wins for scalar keys', () {
      expect(deepMerge({'a': 1, 'b': 2}, {'b': 3}), {'a': 1, 'b': 3});
    });
    test('preserves base-only keys', () {
      expect(deepMerge({'a': 1}, {'b': 2}), {'a': 1, 'b': 2});
    });
    test('merges nested plain objects recursively', () {
      final out = deepMerge(
        {
          'env': {'A': '1', 'B': '2'},
          'x': 1,
        },
        {
          'env': {'B': '3', 'C': '4'},
        },
      );
      expect(out, {
        'env': {'A': '1', 'B': '3', 'C': '4'},
        'x': 1,
      });
    });
    test('replaces arrays instead of unioning them', () {
      expect(
        deepMerge({
          'list': [1, 2, 3],
        }, {
          'list': [9],
        }),
        {
          'list': [9],
        },
      );
    });
    test('overrides object with scalar (mismatched types do not merge)', () {
      expect(
        deepMerge({
          'a': {'x': 1},
        }, {
          'a': 5,
        }),
        {'a': 5},
      );
    });
    test('overrides scalar with object', () {
      expect(
        deepMerge({'a': 5}, {
          'a': {'x': 1},
        }),
        {
          'a': {'x': 1},
        },
      );
    });
    test('treats null / array as non-plain objects (no recursion)', () {
      // React 那条还带 `new Date()`；Dart 侧配置树是 JSON 解码结果，
      // 不可能出现 Date，所以只保留 null 与 array 两支。
      expect(
        deepMerge({
          'a': {'x': 1},
        }, {
          'a': null,
        }),
        {'a': null},
      );
      expect(
        deepMerge({
          'a': {'x': 1},
        }, {
          'a': [1, 2],
        }),
        {
          'a': [1, 2],
        },
      );
      expect(
        deepMerge({'a': null}, {
          'a': {'y': 2},
        }),
        {
          'a': {'y': 2},
        },
      );
    });
  });

  group('updateConfigField', () {
    test('改 settings 键，语义相反的环境变量写成相反值', () {
      final next = updateConfigField({}, 'fastMode', false);
      expect(next['fastMode'], false);
      expect(next['env'], {'CLAUDE_CODE_DISABLE_FAST_MODE': '1'});

      final on = updateConfigField(next, 'fastMode', true);
      expect(on['env'], {'CLAUDE_CODE_DISABLE_FAST_MODE': '0'});
    });

    test('改 settings 键，语义相同的环境变量写成同值', () {
      final next = updateConfigField({}, 'disableArtifact', true);
      expect(next['env'], {'CLAUDE_CODE_DISABLE_ARTIFACT': '1'});
    });

    test('标量配对按原样写进环境变量', () {
      final next = updateConfigField({}, 'effortLevel', 'high');
      expect(next['env'], {'CLAUDE_CODE_EFFORT_LEVEL': 'high'});
    });

    test('改环境变量，对应 settings 键跟着变（相反语义会翻转）', () {
      final prev = {
        'fastMode': true,
        'env': {'CLAUDE_CODE_DISABLE_FAST_MODE': '0'},
      };
      final next = updateConfigField(prev, 'env', {'CLAUDE_CODE_DISABLE_FAST_MODE': '1'});
      expect(next['fastMode'], false);
    });

    test('删掉 settings 键时，配对的环境变量一起删', () {
      final prev = {
        'effortLevel': 'high',
        'env': {'CLAUDE_CODE_EFFORT_LEVEL': 'high', 'DEBUG': '1'},
      };
      final next = updateConfigField(prev, 'effortLevel', null);
      expect(next['effortLevel'], isNull);
      expect(next['env'], {'DEBUG': '1'});
    });

    test('删掉环境变量时，配对的 settings 键一起删', () {
      final prev = {
        'effortLevel': 'high',
        'env': {'CLAUDE_CODE_EFFORT_LEVEL': 'high'},
      };
      final next = updateConfigField(prev, 'env', null);
      expect(next['effortLevel'], isNull);
      expect(next['env'], isNull);
    });

    test('没动过的环境变量不会反压 settings 键', () {
      // 用户先把 fastMode 关了，再去 env 编辑器加了个无关变量：
      // fastMode 必须保持关闭，不能被 env 里的旧值倒推回来。
      final prev = updateConfigField({}, 'fastMode', false);
      final next = updateConfigField(prev, 'env', {
        ...(prev['env']! as Map<String, Object?>),
        'DEBUG': '1',
      });
      expect(next['fastMode'], false);
      expect((next['env']! as Map)['DEBUG'], '1');
    });

    test('未配对的字段行为不变：false 保留，空串视为删除', () {
      final kept = updateConfigField({}, 'includeCoAuthoredBy', false);
      expect(kept['includeCoAuthoredBy'], false);
      expect(kept['env'], isNull);

      final dropped = updateConfigField({'outputStyle': 'Concise'}, 'outputStyle', '');
      expect(dropped['outputStyle'], isNull);
    });

    test('不改入参', () {
      final prev = <String, Object?>{
        'fastMode': true,
        'env': {'DEBUG': '1'},
      };
      final snapshot = stableStringify(prev);
      updateConfigField(prev, 'fastMode', false);
      expect(stableStringify(prev), snapshot);
    });
  });

  group('applySelectedPaths', () {
    test('写入选中的顶层 path，未选中的兄弟 key 保留原值', () {
      final config = <String, Object?>{'a': 1, 'b': 2};
      final source = <String, Object?>{'a': 100, 'b': 200};
      final next = applySelectedPaths(config, source, {'a'});
      expect(next, {'a': 100, 'b': 2});
    });

    test('嵌套 path 只覆盖选中叶子，同级未选中键不受影响', () {
      final config = <String, Object?>{
        'permissions': {
          'allow': ['x'],
          'deny': ['y'],
        },
      };
      final source = <String, Object?>{
        'permissions': {
          'allow': ['new'],
          'deny': ['y'],
        },
      };
      final next = applySelectedPaths(config, source, {'permissions.allow'});
      expect(next, {
        'permissions': {
          'allow': ['new'],
          'deny': ['y'],
        },
      });
    });

    test('path 在 source 中不存在（removed diff）→ 从结果删除该 key', () {
      final config = <String, Object?>{
        'hooks': {'PreToolUse': 'x'},
        'keep': 1,
      };
      final source = <String, Object?>{'keep': 1};
      final next = applySelectedPaths(config, source, {'hooks.PreToolUse'});
      expect(next, {'hooks': <String, Object?>{}, 'keep': 1});
    });

    test('path 中间层 config 缺失时按需创建中间对象', () {
      final config = <String, Object?>{};
      final source = <String, Object?>{
        'a': {
          'b': {'c': 1},
        },
      };
      final next = applySelectedPaths(config, source, {'a.b.c'});
      expect(next, {
        'a': {
          'b': {'c': 1},
        },
      });
    });

    test('不修改传入的 config（返回新对象）', () {
      final config = <String, Object?>{'a': 1};
      final source = <String, Object?>{'a': 2};
      final next = applySelectedPaths(config, source, {'a'});
      expect(config, {'a': 1});
      expect(identical(next, config), isFalse);
    });
  });

  group('stableStringify', () {
    // React 侧无独立测试，但整个设置页的「脏了没有」都压在它身上：
    // 键序不稳 → 刚读回来的配置被判成脏的 → 每次切页都弹未保存提示。
    test('键序不同但内容相同 → 签名相同', () {
      expect(
        stableStringify({'b': 1, 'a': 2}),
        stableStringify({'a': 2, 'b': 1}),
      );
    });
    test('数组顺序参与签名', () {
      expect(
        stableStringify([1, 2]) == stableStringify([2, 1]),
        isFalse,
      );
    });
    test('嵌套对象递归排序', () {
      expect(
        stableStringify({
          'x': {'z': 1, 'y': 2},
        }),
        '{"x":{"y":2,"z":1}}',
      );
    });
    test('null / bool / 数字 / 字符串按 JSON 字面量', () {
      expect(stableStringify(null), 'null');
      expect(stableStringify(true), 'true');
      expect(stableStringify(1), '1');
      expect(stableStringify('a'), '"a"');
    });
  });
}
