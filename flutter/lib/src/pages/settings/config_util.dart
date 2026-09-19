/// 设置页共用的纯函数层 —— 逐条对齐 React 侧的四个文件：
///
/// | Dart | React 真值源 |
/// |---|---|
/// | [stableStringify]    | `src/components/shared/stableStringify.ts` |
/// | [deepMerge]          | `src/utils/deepMerge.ts` |
/// | [updateConfigField]  | `src/services/settings-env-pairs.ts` |
/// | [applySelectedPaths] | `src/components/settings/applySelectedPaths.ts` |
///
/// 这一层没有 UI、没有 IO，所以 React 侧那四份 vitest 的断言可以**数据与期望值一字不改**
/// 地翻过来（见 `test/settings/config_util_test.dart`）—— 这是最硬的对齐证据。
library;

import 'dart:convert';

// ─── stableStringify ────────────────────────────────────────
// 键排序后的确定性 JSON，用来算「脏了没有」的签名。
// 直接比 Map 不行：两次读回来的 Map 键序可能不同，会把没改的表单判成脏的。

String stableStringify(Object? value) {
  if (value == null || value is num || value is String || value is bool) {
    return jsonEncode(value);
  }
  if (value is List) {
    return '[${value.map(stableStringify).join(',')}]';
  }
  if (value is Map) {
    final keys = value.keys.map((k) => k as String).toList()..sort();
    return '{${keys.map((k) => '${jsonEncode(k)}:${stableStringify(value[k])}').join(',')}}';
  }
  return jsonEncode(value);
}

// ─── deepMerge ──────────────────────────────────────────────

/// 与 TS 的严版 `isPlainObject` 同语义：只有普通对象才递归合并。
/// TS 那边额外排掉 Map / Set / Date / 类实例；Dart 侧 JSON 解码出来只有
/// Map / List / 标量，所以 `is Map` 就等价。
bool _isPlainObject(Object? v) => v is Map;

/// 深合并两份配置。override 胜；嵌套普通对象递归合并；
/// **数组与标量整体替换，不取并集**；只在 base 里的键原样保留。
Map<String, Object?> deepMerge(Map<String, Object?> base, Map<String, Object?> override) {
  final result = Map<String, Object?>.from(base);
  for (final key in override.keys) {
    final overrideVal = override[key];
    final baseVal = result[key];
    if (_isPlainObject(baseVal) && _isPlainObject(overrideVal)) {
      result[key] = deepMerge(
        Map<String, Object?>.from(baseVal as Map),
        Map<String, Object?>.from(overrideVal as Map),
      );
    } else {
      result[key] = overrideVal;
    }
  }
  return result;
}

// ─── settings 键 ↔ 环境变量 配对同步 ─────────────────────────
//
// Claude Code 里一批配置有两种写法：settings.json 的键（`fastMode`）和环境变量
// （`CLAUDE_CODE_DISABLE_FAST_MODE`）。只写一边，另一边的旧值会按 Claude Code 自身
// 的优先级反压回来 —— 界面上改了却不生效。
// 策略：**谁后改谁为准**，改一边就把另一边写成等价值，删一边就一起删。

enum PairKind { boolean, value }

enum Polarity { same, inverted }

class SettingEnvPair {
  const SettingEnvPair(this.settingKey, this.envKey, this.kind, [this.polarity]);

  final String settingKey;
  final String envKey;
  final PairKind kind;

  /// 仅 boolean 用：inverted 表示两边语义相反（一个叫「开」一个叫「禁用」）。
  final Polarity? polarity;
}

/// 逐条对齐 `settings-env-pairs.ts::SETTING_ENV_PAIRS`（14 对，顺序一致）。
const List<SettingEnvPair> kSettingEnvPairs = [
  SettingEnvPair('fastMode', 'CLAUDE_CODE_DISABLE_FAST_MODE', PairKind.boolean, Polarity.inverted),
  SettingEnvPair('autoCompactEnabled', 'DISABLE_AUTO_COMPACT', PairKind.boolean, Polarity.inverted),
  SettingEnvPair('autoMemoryEnabled', 'CLAUDE_CODE_DISABLE_AUTO_MEMORY', PairKind.boolean, Polarity.inverted),
  SettingEnvPair('fileCheckpointingEnabled', 'CLAUDE_CODE_DISABLE_FILE_CHECKPOINTING', PairKind.boolean, Polarity.inverted),
  SettingEnvPair('includeGitInstructions', 'CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS', PairKind.boolean, Polarity.inverted),
  SettingEnvPair('syntaxHighlightingDisabled', 'CLAUDE_CODE_SYNTAX_HIGHLIGHT', PairKind.boolean, Polarity.inverted),
  SettingEnvPair('autoConnectIde', 'CLAUDE_CODE_AUTO_CONNECT_IDE', PairKind.boolean, Polarity.same),
  SettingEnvPair('disableArtifact', 'CLAUDE_CODE_DISABLE_ARTIFACT', PairKind.boolean, Polarity.same),
  SettingEnvPair('disableWorkflows', 'CLAUDE_CODE_DISABLE_WORKFLOWS', PairKind.boolean, Polarity.same),
  SettingEnvPair('disableBundledSkills', 'CLAUDE_CODE_DISABLE_BUNDLED_SKILLS', PairKind.boolean, Polarity.same),
  SettingEnvPair('disableAgentView', 'CLAUDE_CODE_DISABLE_AGENT_VIEW', PairKind.boolean, Polarity.same),
  SettingEnvPair('effortLevel', 'CLAUDE_CODE_EFFORT_LEVEL', PairKind.value),
  SettingEnvPair('autoCompactWindow', 'CLAUDE_CODE_AUTO_COMPACT_WINDOW', PairKind.value),
  SettingEnvPair('model', 'ANTHROPIC_MODEL', PairKind.value),
];

final Map<String, SettingEnvPair> _pairBySetting = {
  for (final p in kSettingEnvPairs) p.settingKey: p,
};
final Map<String, SettingEnvPair> _pairByEnv = {
  for (final p in kSettingEnvPairs) p.envKey: p,
};

/// 环境变量的真值解析，与 `EnvEditor.envBool` 同规则。
bool envBool(String? v) {
  if (v == null || v.isEmpty) return false;
  return const ['1', 'true', 'yes', 'on'].contains(v.toLowerCase());
}

String _envFromSetting(SettingEnvPair pair, Object? value) {
  if (pair.kind == PairKind.boolean) {
    final on = value == true;
    return (pair.polarity == Polarity.inverted ? !on : on) ? '1' : '0';
  }
  // TS 的 `String(value)`：数字不带 .0，null → "null"。
  return _jsString(value);
}

Object? _settingFromEnv(SettingEnvPair pair, String value) {
  if (pair.kind == PairKind.boolean) {
    final on = envBool(value);
    return pair.polarity == Polarity.inverted ? !on : on;
  }
  return value;
}

/// JS `String(v)` 的等价物：整数 double 不带小数点（`1` 而不是 `1.0`）。
String _jsString(Object? v) {
  if (v is double && v == v.truncateToDouble() && v.isFinite) {
    return v.toInt().toString();
  }
  return '$v';
}

/// 与 React 同语义：undefined / null / 空串视为「未设置」，`false` 与 `0` 保留。
/// Dart 没有 undefined，所以「删除」由调用方传 `null` 表达。
bool _isUnset(Object? value) => value == null || value == '';

/// 在配置对象上写一个字段，并把配对的另一侧同步成等价值。
///
/// - `field` 是 settings 键 → 顺带写 / 删 `env` 里对应的环境变量
/// - `field == "env"` → 逐个比对**变动过的**环境变量，顺带写 / 删对应的 settings 键
/// - 未配对的字段：行为与同步前一致
///
/// 纯函数，返回新对象，不改入参。
Map<String, Object?> updateConfigField(
  Map<String, Object?> prev,
  String field,
  Object? value,
) {
  final next = Map<String, Object?>.from(prev);
  if (_isUnset(value)) {
    next.remove(field);
  } else {
    next[field] = value;
  }

  if (field == 'env') {
    final prevEnv = _asStringMap(prev['env']);
    final nextEnv = _asStringMap(value);
    // 只看真正变动的环境变量：没动过的不去覆盖 settings 侧，
    // 否则用户单独调整某个 settings 键的动作会在下一次 env 编辑里被抹掉。
    final touched = {...prevEnv.keys, ...nextEnv.keys};
    for (final envKey in touched) {
      final pair = _pairByEnv[envKey];
      if (pair == null || prevEnv[envKey] == nextEnv[envKey]) continue;
      final after = nextEnv[envKey];
      if (_isUnset(after)) {
        next.remove(pair.settingKey);
      } else {
        next[pair.settingKey] = _settingFromEnv(pair, after!);
      }
    }
    return next;
  }

  final pair = _pairBySetting[field];
  if (pair == null) return next;

  final env = Map<String, Object?>.from(_asStringMap(prev['env']));
  if (_isUnset(value)) {
    env.remove(pair.envKey);
  } else {
    env[pair.envKey] = _envFromSetting(pair, value);
  }
  if (env.isNotEmpty) {
    next['env'] = env;
  } else {
    next.remove('env');
  }
  return next;
}

Map<String, String> _asStringMap(Object? v) {
  if (v is! Map) return const {};
  return {
    for (final e in v.entries)
      if (e.value != null) '${e.key}': '${e.value}',
  };
}

// ─── applySelectedPaths ─────────────────────────────────────

/// 按选中的 dot-path 集合，把 source 中对应值写进 config 的深拷贝。
/// 未选中的兄弟键保留 config 原值；path 在 source 中不存在（"removed" 类差异）
/// → 从结果里删掉该键。
Map<String, Object?> applySelectedPaths(
  Map<String, Object?> config,
  Map<String, Object?> source,
  Set<String> selectedPaths,
) {
  final next = _deepCopy(config);
  for (final p in selectedPaths) {
    final segs = p.split('.');
    // 沿 path 在 source 里取值。
    Object? incoming = source;
    var found = true;
    for (final s in segs) {
      if (incoming is Map && incoming.containsKey(s)) {
        incoming = incoming[s];
      } else {
        incoming = null;
        found = false;
        break;
      }
    }
    // 写进 next，中间层缺失时按需创建。
    Map<String, Object?> cursor = next;
    for (var i = 0; i < segs.length - 1; i++) {
      final s = segs[i];
      final cur = cursor[s];
      if (cur is! Map) {
        final created = <String, Object?>{};
        cursor[s] = created;
        cursor = created;
      } else {
        cursor = cur as Map<String, Object?>;
      }
    }
    final leaf = segs[segs.length - 1];
    if (found) {
      cursor[leaf] = incoming;
    } else {
      cursor.remove(leaf);
    }
  }
  return next;
}

/// `JSON.parse(JSON.stringify(x))` 的等价物 —— 返回的树必须是可写的
/// `Map<String, Object?>` / `List<Object?>`，否则 asset 解码出来的只读 map 会在写入时抛。
Map<String, Object?> _deepCopy(Map<String, Object?> src) =>
    _copyValue(src) as Map<String, Object?>;

Object? _copyValue(Object? v) {
  if (v is Map) {
    return <String, Object?>{for (final e in v.entries) '${e.key}': _copyValue(e.value)};
  }
  if (v is List) return <Object?>[for (final e in v) _copyValue(e)];
  return v;
}

/// 对外暴露的深拷贝（页面改草稿前用，避免改到 asset / 上次响应的只读结构）。
Map<String, Object?> deepCopyConfig(Map<String, Object?> src) => _deepCopy(src);
