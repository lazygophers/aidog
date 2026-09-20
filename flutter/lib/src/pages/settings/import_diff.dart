/// 导入 / 推荐配置的差异树 —— 对齐 `src/components/settings/editors/ImportDiff.tsx`
/// 的三个纯函数（`buildImportDiffTree` / `buildRecommendedDiffTree` / `readManagedPaths`）。
///
/// 弹窗本身在 `import_diff_dialog.dart`；这里只有逻辑，所以
/// `ImportDiff.test.ts` 的断言能一字不改地翻过来。
///
/// **一处已知语义差**：JS 区分「键不存在」与「键存在但值为 undefined」，
/// Dart 侧两者都读成 `null`。配置树来自 JSON（JSON 没有 undefined），
/// 所以实际取不到这个差；写在这里是因为它是真的差，不是没想到。
library;

import 'dart:convert';

import '../invoke.dart';

/// 差异树的一个节点。`path` 是 dot-path（`env.FOO`、`permissions.allow`）。
/// 值为普通对象的顶层键**展开一层**成 [children]（深度 1，与 React 版同一 TODO）。
class DiffNode {
  DiffNode({
    required this.path,
    required this.label,
    required this.current,
    required this.incoming,
    this.children,
  });

  final String path;

  /// 展示用标签（path 的最后一段）。
  final String label;
  final Object? current;
  final Object? incoming;
  final List<DiffNode>? children;

  /// 本节点下所有叶子 path（没有 children 时就是自己）—— 勾选的最小单位。
  void collectLeafPaths(List<String> out) {
    final c = children;
    if (c != null && c.isNotEmpty) {
      for (final n in c) {
        n.collectLeafPaths(out);
      }
    } else {
      out.add(path);
    }
  }
}

/// 松版 isPlainObject（与 React `ImportDiff.tsx` 那份对应，不是 `deepMerge.ts` 的严版）。
bool _isPlainObject(Object? v) => v is Map;

/// 读 aidog 自己注入的 leaf dot-path 集合（命令 `get_managed_paths`）。
/// 缺失 / 报错 → 空集合 → diff 退回未过滤的行为（零回归）。
Future<Set<String>> readManagedPaths([InvokeFn? invoke]) async {
  try {
    final paths = await (invoke ?? kernelInvoke)('get_managed_paths');
    if (paths is! List) return <String>{};
    return paths.map((e) => '$e').toSet();
  } catch (_) {
    return <String>{};
  }
}

/// Rust `collect_leaf_paths` 的镜像：走一棵值，push 每个叶子（标量 / 数组 / null）
/// 的 dot-path，跳过 `_aidog_` 键。
void _collectValueLeafPaths(Object? value, String prefix, List<String> out) {
  if (_isPlainObject(value)) {
    for (final k in (value as Map).keys) {
      final key = '$k';
      if (key.startsWith('_aidog_')) continue;
      final path = prefix.isEmpty ? key : '$prefix.$key';
      _collectValueLeafPaths(value[k], path, out);
    }
  } else if (prefix.isNotEmpty) {
    out.add(prefix);
  }
}

/// 一个一级 diff 子节点「整棵子树都是 aidog 托管的」才算 fully managed。
/// 子树里只要有一个用户自己加的叶子，就留在 diff 里。
bool _isFullyManaged(Object? incomingValue, String path, Set<String> managed) {
  if (managed.isEmpty) return false;
  final leaves = <String>[];
  _collectValueLeafPaths(incomingValue, path, leaves);
  if (leaves.isEmpty) return false;
  return leaves.every(managed.contains);
}

/// Claude Code 自己写的运行时偏好。aidog 从不托管它们（写入侧只注入
/// `env.ANTHROPIC_BASE_URL` / `env.ANTHROPIC_AUTH_TOKEN`），所以搭不上 managed 标记，
/// 只能在这里过滤 —— 列出来对用户是纯噪音。
const _ccRuntimeIgnoreExact = <String>{
  'model',
  'effortLevel',
  'ultracode',
  'maxSkillDescriptionChars',
  'skipWebFetchPreflight',
  'workflowKeywordTriggerEnabled',
};
const _ccRuntimeIgnorePrefixes = <String>['fileSuggestion.', 'env.ANTHROPIC_DEFAULT_'];

bool _isCcRuntimeIgnored(String path) {
  if (_ccRuntimeIgnoreExact.contains(path)) return true;
  return _ccRuntimeIgnorePrefixes.any(path.startsWith);
}

/// JS `JSON.stringify(a) === JSON.stringify(b)` 的等价物。
/// **不用 stableStringify**：React 那边比的是插入序的 JSON，换成排序版会把
/// 「只是键序不同」也判成无差异，与 React 行为不同。
bool _sameJson(Object? a, Object? b) => jsonEncode(a) == jsonEncode(b);

/// 当前配置与来源配置的差异树。跳过内部 `_aidog_` 键；对象顶层键展开一层。
///
/// [managed] = aidog 注入的 leaf dot-path；命中的叶子被排除，只留用户自己加的字段。
/// 排除精确到子键：`env.ANTHROPIC_BASE_URL` 被丢掉，用户的 `env.FOO` 留着。
/// 父节点只有在**所有有差异的子节点都被排除**时才整个丢掉。
List<DiffNode> buildImportDiffTree(
  Map<String, Object?> current,
  Map<String, Object?> incoming,
  Set<String> managed,
) {
  final nodes = <DiffNode>[];
  final keys = <String>{...current.keys, ...incoming.keys};
  for (final key in keys) {
    if (key.startsWith('_aidog_')) continue;
    final cur = current[key];
    final inc = incoming[key];
    if (_sameJson(cur, inc)) continue;

    if (_isPlainObject(cur) || _isPlainObject(inc)) {
      final curObj = _isPlainObject(cur) ? cur! as Map : const {};
      final incObj = _isPlainObject(inc) ? inc! as Map : const {};
      final childKeys = <String>{
        ...curObj.keys.map((e) => '$e'),
        ...incObj.keys.map((e) => '$e'),
      };
      final children = <DiffNode>[];
      for (final ck in childKeys) {
        final childPath = '$key.$ck';
        if (managed.contains(childPath) ||
            _isCcRuntimeIgnored(childPath) ||
            _isFullyManaged(incObj[ck], childPath, managed)) {
          continue;
        }
        if (_sameJson(curObj[ck], incObj[ck])) continue;
        children.add(DiffNode(
          path: childPath,
          label: ck,
          current: curObj[ck],
          incoming: incObj[ck],
        ));
      }
      // 有差异的子节点全被排除 → 父节点整个丢掉。
      if (children.isNotEmpty) {
        nodes.add(DiffNode(path: key, label: key, current: cur, incoming: inc, children: children));
      }
      continue;
    }
    if (managed.contains(key) || _isCcRuntimeIgnored(key)) continue;
    nodes.add(DiffNode(path: key, label: key, current: cur, incoming: inc));
  }
  return nodes;
}

/// 由 `_aidog_*` 物化出来的原生字段，不参与推荐配置 diff：
/// `statusLine` / `subagentStatusLine` 由 Rust 的 `inject_statusline` 每次 sync 重生成，
/// `hooks` 由 `inject_claude_code_hooks` 注入。不排除的话，用户已启用的状态栏 / 通知 hook
/// 会被列成「删除」项 —— 而它们下次保存又被写回去，提示本身就是错的。
const _derivedKeys = <String>{'statusLine', 'subagentStatusLine', 'hooks'};

/// 推荐配置差异树：与「从 Claude Code 导入」共用同一棵树与同一个弹窗，
/// 唯一区别是不做 managed 过滤（推荐值本来就是 aidog 自己的默认）。
List<DiffNode> buildRecommendedDiffTree(
  Map<String, Object?> current,
  Map<String, Object?> recommended,
) =>
    buildImportDiffTree(current, recommended, <String>{})
        .where((n) => !_derivedKeys.contains(n.path))
        .toList();
