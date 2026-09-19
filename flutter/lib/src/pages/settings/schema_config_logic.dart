/// GUI / JSON 双模式配置页的状态机 —— 三个设置子页共用：
///
/// | 子页 | React 真值源 | 读 / 写命令 |
/// |---|---|---|
/// | `settings/claude` | `src/pages/Settings.tsx` | `settings_get` / `settings_set` |
/// | `settings/codex`  | `src/pages/CodexSettings.tsx` | `codex_config_read` / `codex_config_write` |
/// | `settings/pi`     | `src/pages/PiSettings.tsx` | `pi_settings_read` / `pi_settings_write` |
///
/// Codex 与 pi 两页在 React 侧是逐字节同构的（只换 schema 与命令），claude 多三件事：
/// 保存后 `sync_group_settings`、两种导入走差异弹窗、**注册离页 guard**。
/// 所以这里是一个带开关的控制器，不是三份拷贝。
///
/// 🔴 三页的差异是照搬的，不是「顺手统一」的：React 的 Codex / pi 页**没有**离页拦截，
/// 也**没有** Cmd+S；把它们一并加上会让行为与 React 不同 —— 票里写明了「行为不同就是 bug」。
/// 想加要先改 React 侧。
library;

import 'dart:convert';

import '../../shell/nav_guard.dart';
import '../invoke.dart';
import 'config_util.dart';
import 'import_diff.dart';

enum EditorMode { gui, json }

/// 待处理的导入差异（弹窗的数据源）。
class PendingImportDiff {
  PendingImportDiff({required this.source, required this.diff, required this.recommended});

  final Map<String, Object?> source;
  final List<DiffNode> diff;

  /// true = 「加载推荐配置」，false = 「从 Claude Code 导入」。
  /// 影响弹窗标题 / 按钮文案，以及应用时是否先深合并 `_aidog_*` 内部键。
  final bool recommended;
}

/// 三页各自的接线。字段少且全是数据，不值得为它造继承层级。
class SchemaConfigWiring {
  const SchemaConfigWiring({
    required this.readCmd,
    required this.writeCmd,
    required this.writeArgs,
    required this.readArgs,
    this.syncGroupSettingsAfterSave = false,
    this.usesEnvPairs = false,
    this.importDiffEnabled = false,
    this.guardsNavigation = false,
  });

  /// `settings/claude`：整份读写存在 DB 的 `global / claude_code` 键下。
  static const claude = SchemaConfigWiring(
    readCmd: 'settings_get',
    readArgs: {'scope': 'global', 'key': 'claude_code'},
    writeCmd: 'settings_set',
    writeArgs: _claudeWriteArgs,
    syncGroupSettingsAfterSave: true,
    usesEnvPairs: true,
    importDiffEnabled: true,
    guardsNavigation: true,
  );

  static const codex = SchemaConfigWiring(
    readCmd: 'codex_config_read',
    readArgs: null,
    writeCmd: 'codex_config_write',
    writeArgs: _valueArgs,
  );

  static const pi = SchemaConfigWiring(
    readCmd: 'pi_settings_read',
    readArgs: null,
    writeCmd: 'pi_settings_write',
    writeArgs: _configArgs,
  );

  final String readCmd;
  final Map<String, Object?>? readArgs;
  final String writeCmd;

  /// 把待保存的整份配置包成该命令的入参形状。
  final Map<String, Object?> Function(Map<String, Object?> value) writeArgs;

  /// 保存成功后 best-effort 调一次 `sync_group_settings`（失败不阻断保存）。
  final bool syncGroupSettingsAfterSave;

  /// 改字段时走 [updateConfigField]（settings 键 ↔ 环境变量 配对同步）。
  /// 只有 claude 页有这层配对；codex / pi 是「空值即删键」的朴素写法。
  final bool usesEnvPairs;

  /// 「加载推荐配置」走逐项勾选的差异弹窗（claude），还是直接深合并（codex / pi）。
  final bool importDiffEnabled;

  /// 脏状态下注册离页 guard。
  final bool guardsNavigation;
}

Map<String, Object?> _claudeWriteArgs(Map<String, Object?> v) => {
      'input': {'scope': 'global', 'key': 'claude_code', 'value': v},
    };
Map<String, Object?> _valueArgs(Map<String, Object?> v) => {'value': v};
Map<String, Object?> _configArgs(Map<String, Object?> v) => {'config': v};

/// 双模式配置页的全部状态与动作。零 widget 依赖 → 可直接单测。
class SchemaConfigController {
  SchemaConfigController({
    required this.wiring,
    required this.recommendedConfig,
    InvokeFn? invoke,
    this.onChanged,
  }) : _invoke = invoke ?? kernelInvoke;

  final SchemaConfigWiring wiring;

  /// 「推荐配置」。claude 页 = 后端内置默认 + 运行时语言；codex / pi 来自各自 schema。
  final Map<String, Object?> recommendedConfig;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  EditorMode mode = EditorMode.gui;
  Map<String, Object?> config = <String, Object?>{};
  String editJson = '';
  bool saving = false;
  String saveError = '';
  String toast = '';
  PendingImportDiff? importDiff;

  /// 上次保存后的基线签名。空串 = 还没读到过数据 → 一律不脏
  /// （首屏加载中点走人不该弹「未保存」）。
  String baseline = '';

  void Function()? _unregisterGuard;

  /// 被 guard 拦下、等用户裁决的那次导航。null = 没有待处理的导航。
  void Function()? pendingNav;

  void _notify() => onChanged?.call();

  /// 当前草稿签名。JSON 模式下解析失败视为脏（正在打字的半截 JSON 也是改动）。
  String get currentSig {
    if (mode == EditorMode.json) {
      try {
        return stableStringify(jsonDecode(editJson));
      } catch (_) {
        return '__invalid__$editJson';
      }
    }
    return stableStringify(config);
  }

  bool get dirty => baseline != '' && currentSig != baseline;

  /// 保存按钮可用 = 脏 且 不在保存中（React：`disabled={saving || !dirty}`）。
  bool get canSave => dirty && !saving;

  Future<void> load() async {
    try {
      final stored = await _invoke(wiring.readCmd, wiring.readArgs);
      final map = stored is Map ? Map<String, Object?>.from(stored) : <String, Object?>{};
      // 从未配置过 → 默认填入推荐配置，便于一键起步。
      final data = map.isNotEmpty ? deepCopyConfig(map) : deepCopyConfig(recommendedConfig);
      config = data;
      editJson = _pretty(data);
      baseline = stableStringify(data);
    } catch (_) {
      // React 版这里只 console.error，页面留空表单。照搬：不把错误摆到界面上，
      // 否则首次打开（配置文件还不存在）就会看到一条吓人的红字。
    }
    _syncGuard();
    _notify();
  }

  /// 切模式。进 JSON 模式时用当前结构化配置重新生成文本
  /// （与 React 的 `onModeChange` 一致：GUI → JSON 会覆盖掉 JSON 里的手改）。
  void setMode(EditorMode m) {
    if (m == EditorMode.json) editJson = _pretty(config);
    mode = m;
    _syncGuard();
    _notify();
  }

  void setEditJson(String s) {
    editJson = s;
    _syncGuard();
    _notify();
  }

  /// 改一个 GUI 字段。claude 页顺带同步配对的环境变量；codex / pi 走
  /// 「undefined / null / 空串 = 删掉这个键」的朴素写法。
  void updateField(String field, Object? value) {
    if (wiring.usesEnvPairs) {
      config = updateConfigField(config, field, value);
    } else {
      final next = <String, Object?>{};
      for (final e in config.entries) {
        if (e.key != field) next[e.key] = e.value;
      }
      if (value != null && value != '') next[field] = value;
      config = next;
    }
    editJson = _pretty(config);
    _syncGuard();
    _notify();
  }

  /// 保存。返回是否成功 —— 「保存并离开」要靠这个返回值决定走不走。
  Future<bool> save({String savedText = ''}) async {
    saving = true;
    saveError = '';
    _notify();
    try {
      final Map<String, Object?> value = mode == EditorMode.json
          ? Map<String, Object?>.from(jsonDecode(editJson) as Map)
          : Map<String, Object?>.from(config);
      await _invoke(wiring.writeCmd, wiring.writeArgs(value));
      config = value;
      editJson = _pretty(value);
      baseline = stableStringify(value); // 草稿变回「干净」
      if (wiring.syncGroupSettingsAfterSave) {
        // best-effort：同步失败绝不阻断保存本身（与 React 一致）。
        try {
          await _invoke('sync_group_settings');
        } catch (_) {/* console.error 等价物 */}
      }
      toast = savedText;
      saving = false;
      _syncGuard();
      _notify();
      return true;
    } catch (e) {
      saveError = '$e';
      saving = false;
      _notify();
      return false;
    }
  }

  void clearToast() {
    if (toast.isEmpty) return;
    toast = '';
    _notify();
  }

  /// 「加载推荐配置」。
  /// - claude：先算差异树，**无差异时只弹一条提示不开弹窗**；有差异才让用户逐项勾选。
  /// - codex / pi：直接深合并，没有弹窗。
  ///
  /// [noDiffText] 是 claude 无差异时的提示文案，[loadedText] 是 codex / pi 合并后的提示。
  void loadRecommended({String noDiffText = '', String loadedText = ''}) {
    if (!wiring.importDiffEnabled) {
      config = deepMerge(config, recommendedConfig);
      editJson = _pretty(config);
      toast = loadedText;
      _syncGuard();
      _notify();
      return;
    }
    final diff = buildRecommendedDiffTree(config, recommendedConfig);
    if (diff.isEmpty) {
      toast = noDiffText;
      _notify();
      return;
    }
    importDiff = PendingImportDiff(source: recommendedConfig, diff: diff, recommended: true);
    _notify();
  }

  /// 「从 Claude Code 导入」：读 `~/.claude/settings.json`，扣掉 aidog 自己注入的字段，
  /// 算差异树。失败时把错误摆成 toast（React 的文案是 `导入失败：${message}`）。
  ///
  /// [noDiffText] 无差异提示；[failedText] 接错误串返回成品文案。
  Future<void> importFromClaudeCode({
    String noDiffText = '',
    String Function(String err)? failedText,
  }) async {
    try {
      final raw = await _invoke('read_claude_code_settings');
      final source = raw is Map ? Map<String, Object?>.from(raw) : <String, Object?>{};
      final managed = await readManagedPaths(_invoke);
      final diff = buildImportDiffTree(config, source, managed);
      if (diff.isEmpty) {
        toast = noDiffText;
        _notify();
        return;
      }
      importDiff = PendingImportDiff(source: source, diff: diff, recommended: false);
    } catch (e) {
      toast = failedText?.call('$e') ?? '$e';
    }
    _notify();
  }

  /// 应用弹窗里勾选的路径。
  ///
  /// `_aidog_*` 是内部键，差异树按约定跳过它们；**推荐配置**的那部分（状态栏 / hooks 开关）
  /// 随勾选结果一起深合并进去，保持「加载推荐配置」的旧行为。
  void applyImport(Set<String> selectedPaths, {String appliedText = ''}) {
    final pending = importDiff;
    if (pending == null) return;
    final base = pending.recommended
        ? deepMerge(config, _pickAidogKeys(pending.source))
        : config;
    config = applySelectedPaths(base, pending.source, selectedPaths);
    editJson = _pretty(config);
    importDiff = null;
    toast = appliedText;
    _syncGuard();
    _notify();
  }

  void cancelImport() {
    importDiff = null;
    _notify();
  }

  // ── 离页拦截 ────────────────────────────────────────────
  // 语义照抄 `src/utils/navGuard.ts` + `Settings.tsx` 的那个 useEffect：
  //   脏 → 注册 guard；guard 不自己放行，只把 proceed 记下来并弹确认框；
  //   不脏 → 注销 guard，导航畅通。

  void _syncGuard() {
    if (!wiring.guardsNavigation) return;
    if (dirty) {
      // 已经注册过就不重复注册（重复注册会把上一个 guard 顶掉，
      // 虽然 registerNavGuard 是后注册者胜、结果相同，但会白白丢掉注销闭包）。
      _unregisterGuard ??= registerNavGuard((proceed) {
        pendingNav = proceed;
        _notify();
      });
    } else {
      _unregisterGuard?.call();
      _unregisterGuard = null;
    }
  }

  /// 「保存并离开」：保存失败就留在原地（错误已就地显示），成功才放行。
  Future<void> saveAndLeave({String savedText = ''}) async {
    final ok = await save(savedText: savedText);
    if (!ok) return;
    final proceed = pendingNav;
    pendingNav = null;
    proceed?.call();
  }

  /// 「不保存，直接离开」。
  void discardAndLeave() {
    final proceed = pendingNav;
    pendingNav = null;
    proceed?.call();
    _notify();
  }

  /// 「取消」：留在本页，草稿原样保留，guard 仍然挂着。
  void cancelLeave() {
    pendingNav = null;
    _notify();
  }

  /// 页面销毁时必须调，否则离开设置页后 guard 还挂着，后面每次导航都被拦。
  void dispose() {
    _unregisterGuard?.call();
    _unregisterGuard = null;
  }

  static Map<String, Object?> _pickAidogKeys(Map<String, Object?> source) => {
        for (final e in source.entries)
          if (e.key.startsWith('_aidog_')) e.key: e.value,
      };

  /// 与 `JSON.stringify(x, null, 2)` 一致的两空格缩进。
  static String _pretty(Object? v) => const JsonEncoder.withIndent('  ').convert(v);
}
