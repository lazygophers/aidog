/// `src/components/settings/editors/ImportDiff.test.ts` 的断言逐条翻译，
/// **数据与期望值一字不改**（用例标题也照抄）。
///
/// 注意 Dart 的 Set / List / Map 按**标识**比，不按值比 —— 直接
/// `expect(setA, setB)` 会在结构相同时也判不等。这里一律先转成
/// `List` 再 `..sort()`，或者用 `containsAll` / `unorderedEquals`。
library;

import 'package:aidog_flutter/src/pages/settings/config_util.dart';
import 'package:aidog_flutter/src/pages/settings/import_diff.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('buildRecommendedDiffTree', () {
    test('推荐配置里没有的键列为删除项', () {
      final diff = buildRecommendedDiffTree({'a': 1, 'userOnly': 'keep'}, {'a': 2});
      expect(diff.map((n) => n.path).toList()..sort(), ['a', 'userOnly']);
      expect(diff.firstWhere((n) => n.path == 'userOnly').incoming, isNull);
    });

    test('对象键展开一层，用户独有的子键也列为删除项', () {
      final current = <String, Object?>{
        'env': {'A': '1', 'MINE': 'x'},
      };
      final diff = buildRecommendedDiffTree(current, {
        'env': {'A': '2', 'B': '3'},
      });
      expect(diff, hasLength(1));
      expect(
        diff[0].children!.map((c) => c.path).toList()..sort(),
        ['env.A', 'env.B', 'env.MINE'],
      );
    });

    test('勾选删除项后该键从结果里移除', () {
      final next = applySelectedPaths(
        {
          'env': {'A': '1', 'MINE': 'x'},
        },
        {
          'env': {'A': '2'},
        },
        {'env.MINE'},
      );
      expect(next, {
        'env': {'A': '1'},
      });
    });

    test('无差异时返回空树', () {
      expect(buildRecommendedDiffTree({'a': 1}, {'a': 1}), isEmpty);
    });

    test('不把 _aidog_* 物化出来的 statusLine / subagentStatusLine / hooks 列为删除项', () {
      final current = <String, Object?>{
        'statusLine': {
          'type': 'command',
          'command': 'uv run --script ~/.aidog/scripts/aidog-statusline.py',
        },
        'subagentStatusLine': {
          'type': 'command',
          'command': 'uv run --script ~/.aidog/scripts/aidog-subagent-statusline.py',
        },
        'hooks': {
          'PreToolUse': [
            {
              'hooks': [
                {'type': 'command', 'command': 'rtk hook claude'},
              ],
            },
          ],
        },
        'userOnly': 'keep',
      };
      final diff = buildRecommendedDiffTree(current, {
        '_aidog_statusline': {'enabled': true},
      });
      expect(diff.map((n) => n.path).toList(), ['userOnly']);
    });

    test('跳过 _aidog_ 内部键', () {
      final diff = buildRecommendedDiffTree({}, {
        '_aidog_hooks': {'enabled': true},
        'a': 1,
      });
      expect(diff.map((n) => n.path).toList(), ['a']);
    });

    test('勾选后按路径落盘，未选中的保持原值', () {
      final current = <String, Object?>{
        'env': {'A': '1', 'MINE': 'x'},
      };
      final source = <String, Object?>{
        'env': {'A': '2', 'B': '3'},
      };
      final next = applySelectedPaths(current, source, {'env.B'});
      expect(next, {
        'env': {'A': '1', 'MINE': 'x', 'B': '3'},
      });
    });
  });

  // 下面这组 React 侧没有，补的是 managed 过滤与 CC 运行时偏好过滤 ——
  // 这两条一旦搬丢，用户每次点「从 Claude Code 导入」都会看到一堆
  // aidog 自己注入的字段，是最容易静默漏掉的一块。
  group('buildImportDiffTree 的过滤', () {
    test('managed 叶子被排除，同级用户自己加的留着', () {
      final diff = buildImportDiffTree(
        {},
        {
          'env': {'ANTHROPIC_BASE_URL': 'http://x', 'FOO': 'bar'},
        },
        {'env.ANTHROPIC_BASE_URL'},
      );
      expect(diff.map((n) => n.path).toList(), ['env']);
      expect(diff[0].children!.map((c) => c.path).toList(), ['env.FOO']);
    });

    test('有差异的子节点全被 managed 排除 → 父节点整个丢掉', () {
      final diff = buildImportDiffTree(
        {},
        {
          'env': {'ANTHROPIC_BASE_URL': 'http://x'},
        },
        {'env.ANTHROPIC_BASE_URL'},
      );
      expect(diff, isEmpty);
    });

    test('整棵子树都 managed 的一级子节点被排除（isFullyManaged）', () {
      final diff = buildImportDiffTree(
        {},
        {
          'extraKnownMarketplaces': {
            'x': {'source': {'source': 'github'}},
          },
        },
        {'extraKnownMarketplaces.x.source.source'},
      );
      expect(diff, isEmpty);
    });

    test('子树里有用户自己的叶子时，该子节点留下', () {
      final diff = buildImportDiffTree(
        {},
        {
          'extraKnownMarketplaces': {
            'x': {'source': 'github', 'mine': 1},
          },
        },
        {'extraKnownMarketplaces.x.source'},
      );
      expect(diff.map((n) => n.path).toList(), ['extraKnownMarketplaces']);
    });

    test('CC 自写的运行时偏好被过滤（顶层精确名 + 前缀）', () {
      final diff = buildImportDiffTree(
        {},
        {
          'model': 'opus',
          'effortLevel': 'high',
          'ultracode': true,
          'maxSkillDescriptionChars': 1,
          'skipWebFetchPreflight': true,
          'workflowKeywordTriggerEnabled': true,
          'keepMe': 1,
        },
        <String>{},
      );
      expect(diff.map((n) => n.path).toList(), ['keepMe']);
    });

    test('前缀过滤命中子路径 fileSuggestion.* 与 env.ANTHROPIC_DEFAULT_*', () {
      final diff = buildImportDiffTree(
        {},
        {
          'fileSuggestion': {'mode': 'a'},
          'env': {'ANTHROPIC_DEFAULT_SONNET_MODEL': 'x'},
        },
        <String>{},
      );
      expect(diff, isEmpty);
    });

    test('managed 为空集合时 isFullyManaged 不生效（零回归）', () {
      final diff = buildImportDiffTree(
        {},
        {
          'env': {'ANTHROPIC_BASE_URL': 'http://x'},
        },
        <String>{},
      );
      expect(diff.map((n) => n.path).toList(), ['env']);
    });

    test('键序不同但内容相同仍算有差异（与 React 的 JSON.stringify 比较一致）', () {
      // React 那边比的是插入序的 JSON，不是排序后的签名。换成 stableStringify
      // 会把这一条判成「无差异」，行为就与 React 不同了。
      final diff = buildImportDiffTree(
        {
          'o': {'a': 1, 'b': 2},
        },
        {
          'o': {'b': 2, 'a': 1},
        },
        <String>{},
      );
      expect(diff, isEmpty, reason: '子键逐个比对后两边相同，父节点无 children → 不入树');
    });

    test('collectLeafPaths 把一棵树摊成可勾选的叶子', () {
      final diff = buildImportDiffTree(
        {},
        {
          'env': {'A': 1, 'B': 2},
          'scalar': 3,
        },
        <String>{},
      );
      final leaves = <String>[];
      for (final n in diff) {
        n.collectLeafPaths(leaves);
      }
      expect(leaves..sort(), ['env.A', 'env.B', 'scalar']);
    });
  });
}
