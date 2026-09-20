/// 关于页的逻辑层（票 I09），对应 `src/pages/About.tsx`（23.4 KB，本批次最大的一个）。
///
/// 两块：版本信息（`about_info` 一次拿全）+ 本地 CLI 环境（检查 / 安装 / 升级 / 诊断冲突）。
///
/// **更新检查这一块不在本层**：React 那边走的是 `@tauri-apps/plugin-updater`，
/// 不是后端命令（所以它也不在本票的 39 条命令清单里）。Flutter 外壳的自动更新是票 I13
/// 的 `auto_updater`，还没落地 —— 页面渲染 React 的**另一条既有分支**
/// （`About.tsx:291` 的 `isTauri()` 三元里那半，提示「这里不检查更新」），
/// 不自己发明一个点了必然报错的按钮。
library;

import 'package:flutter/foundation.dart' show VoidCallback;

import 'invoke.dart';
import 'skills_logic.dart' show TrFn;

/// `About.tsx:20::CLI_TOOL_LABELS` 的键序。后端 `cli_env::TOOLS` 是名字的真值源。
const Map<String, String> kCliToolLabelKeys = {
  'claude': 'about.localEnv.claudeCode',
  'codex': 'about.localEnv.codex',
  'pi': 'about.localEnv.pi',
};

/// `About.tsx:26`。链接是常量，不走后端。
const String kGithubRepo = 'https://github.com/lazygophers/aidog';
const Map<String, String> kGithubLinks = {
  'repo': kGithubRepo,
  'releases': '$kGithubRepo/releases',
  'issues': '$kGithubRepo/issues',
  'reportIssue': '$kGithubRepo/issues/new',
};

/// `manual.ts:216::AboutInfo`。
class AboutInfo {
  const AboutInfo({
    required this.appVersion,
    required this.tauriVersion,
    required this.os,
    required this.arch,
    required this.family,
    required this.profile,
    required this.gitCommit,
    required this.buildTime,
  });

  final String appVersion;
  final String tauriVersion;
  final String os;
  final String arch;
  final String family;
  final String profile;
  final String gitCommit;

  /// 构建时间，**epoch 秒的字符串**（前端格式化）。
  final String buildTime;

  static AboutInfo fromJson(Map<String, Object?> j) => AboutInfo(
    appVersion: (j['app_version'] as String?) ?? '',
    tauriVersion: (j['tauri_version'] as String?) ?? '',
    os: (j['os'] as String?) ?? '',
    arch: (j['arch'] as String?) ?? '',
    family: (j['family'] as String?) ?? '',
    profile: (j['profile'] as String?) ?? '',
    gitCommit: (j['git_commit'] as String?) ?? '',
    buildTime: (j['build_time'] as String?) ?? '',
  );
}

/// `manual.ts:230::CliInstallation`。
class CliInstallation {
  const CliInstallation({
    required this.path,
    this.version,
    required this.runnable,
    required this.source,
    required this.isPathDefault,
  });

  final String path;
  final String? version;
  final bool runnable;
  final String source;
  final bool isPathDefault;

  static CliInstallation fromJson(Map<String, Object?> j) => CliInstallation(
    path: (j['path'] as String?) ?? '',
    version: j['version'] as String?,
    runnable: j['runnable'] == true,
    source: (j['source'] as String?) ?? '',
    isPathDefault: j['is_path_default'] == true,
  );
}

/// `manual.ts:244::CliToolStatus`。`hasUpdate` 为 null = 检测失败 / 离线。
class CliToolStatus {
  const CliToolStatus({
    required this.name,
    required this.installed,
    this.version,
    this.path,
    required this.broken,
    required this.conflict,
    this.latestVersion,
    this.hasUpdate,
  });

  final String name;
  final bool installed;
  final String? version;
  final String? path;
  final bool broken;
  final bool conflict;
  final String? latestVersion;
  final bool? hasUpdate;

  static CliToolStatus fromJson(Map<String, Object?> j) => CliToolStatus(
    name: (j['name'] as String?) ?? '',
    installed: j['installed'] == true,
    version: j['version'] as String?,
    path: j['path'] as String?,
    broken: j['broken'] == true,
    conflict: j['conflict'] == true,
    latestVersion: j['latest_version'] as String?,
    hasUpdate: j['has_update'] as bool?,
  );
}

/// `manual.ts:260::CliConflict`。仅报告 + 建议，**不自动卸载**。
class CliConflict {
  const CliConflict({
    required this.tool,
    required this.installations,
    required this.isConflicting,
    required this.suggestion,
  });

  final String tool;
  final List<CliInstallation> installations;
  final bool isConflicting;
  final String suggestion;

  static CliConflict fromJson(Map<String, Object?> j) => CliConflict(
    tool: (j['tool'] as String?) ?? '',
    installations: [
      for (final i in (j['installations'] as List<Object?>? ?? const []))
        CliInstallation.fromJson((i as Map).cast<String, Object?>()),
    ],
    isConflicting: j['is_conflicting'] == true,
    suggestion: (j['suggestion'] as String?) ?? '',
  );
}

/// 状态徽标的四分支（`About.tsx:182::cliStatusText`），**判定顺序不能换**：
/// 未安装 → 已损坏 → 冲突 → 已安装。
enum CliStatusKind { notInstalled, broken, conflict, installed }

CliStatusKind cliStatusKind(CliToolStatus s) {
  if (!s.installed) return CliStatusKind.notInstalled;
  if (s.broken) return CliStatusKind.broken;
  if (s.conflict) return CliStatusKind.conflict;
  return CliStatusKind.installed;
}

String cliStatusKey(CliStatusKind k) => switch (k) {
  CliStatusKind.notInstalled => 'about.localEnv.notInstalled',
  CliStatusKind.broken => 'about.localEnv.broken',
  CliStatusKind.conflict => 'about.localEnv.conflict',
  CliStatusKind.installed => 'about.localEnv.installed',
};

class AboutController {
  AboutController({
    required this.invoke,
    required this.t,
    required this.onChanged,
  });

  final InvokeFn invoke;
  final TrFn t;
  final VoidCallback onChanged;

  AboutInfo? info;

  List<CliToolStatus> cliTools = const [];
  List<CliConflict> cliConflicts = const [];

  /// `''` / `check` / `install` / `upgrade` / `diagnose`。
  String cliBusy = '';
  String cliMsg = '';
  String cliErr = '';
  String? cliPendingTool;

  /// `About.tsx:55` 的 mount effect：拉版本信息 + 立刻检查一次 CLI 版本。
  Future<void> init() async {
    await Future.wait([_loadInfo(), checkCli()]);
  }

  Future<void> _loadInfo() async {
    try {
      final raw = await invoke('about_info');
      info = AboutInfo.fromJson((raw as Map).cast<String, Object?>());
    } catch (_) {
      info = null; // `.catch(() => setInfo(null))`
    }
    onChanged();
  }

  /// `About.tsx:105::handleCliCheck`。
  /// `cli_check_versions` 是主流程；`cli_check_updates` **并行跟一发**，
  /// 失败静默（React 的 `console.warn` 分支），不影响已经拿到的版本列表。
  Future<void> checkCli() async {
    cliBusy = 'check';
    cliMsg = '';
    cliErr = '';
    onChanged();
    try {
      final raw = await invoke('cli_check_versions');
      cliTools = _tools(raw);
      unawaitedCheckUpdates();
    } catch (e) {
      cliErr = '${t('about.localEnv.checkFailed')}: $e';
    } finally {
      cliBusy = '';
      onChanged();
    }
  }

  List<CliToolStatus> _tools(Object? raw) => [
    for (final s in (raw as List<Object?>))
      CliToolStatus.fromJson((s as Map).cast<String, Object?>()),
  ];

  /// 更新可用性探测：拿到就覆盖列表，拿不到什么都不做。
  Future<void> unawaitedCheckUpdates() async {
    try {
      final raw = await invoke('cli_check_updates');
      cliTools = _tools(raw);
      onChanged();
    } catch (_) {
      // 检测失败静默忽略（React: console.warn），不改 cliErr。
    }
  }

  /// `About.tsx:126::handleCliInstall`。成功后重跑一次检查。
  Future<void> installCli(String tool) async {
    cliBusy = 'install';
    cliPendingTool = tool;
    cliMsg = '';
    cliErr = '';
    onChanged();
    try {
      await invoke('cli_install', {'tool': tool});
      cliBusy = '';
      cliPendingTool = null;
      await checkCli();
      // **有意偏离 React 的一个既有 bug**：那边 `setCliMsg(成功)` 之后立刻
      // `await handleCliCheck()`，而后者开头就 `setCliMsg("")` —— 成功提示当场被自己清掉，
      // 用户永远看不到。这里把它放在重查**之后**，提示真的会出现。见 README 差异表。
      cliMsg = t('about.localEnv.installSuccess', {'tool': tool});
    } catch (e) {
      cliErr = '${t('about.localEnv.installFailed')}: $e';
    } finally {
      cliBusy = '';
      cliPendingTool = null;
      onChanged();
    }
  }

  /// `About.tsx:143::handleCliUpgrade`。「修复」按钮走的也是这条（broken 时）。
  Future<void> upgradeCli(String tool) async {
    cliBusy = 'upgrade';
    cliPendingTool = tool;
    cliMsg = '';
    cliErr = '';
    onChanged();
    try {
      await invoke('cli_upgrade', {'tool': tool});
      cliBusy = '';
      cliPendingTool = null;
      await checkCli();
      // 同 [installCli]：提示放在重查之后，否则会被 checkCli 的清空吞掉。
      cliMsg = t('about.localEnv.upgradeSuccess', {'tool': tool});
    } catch (e) {
      cliErr = '${t('about.localEnv.upgradeFailed')}: $e';
    } finally {
      cliBusy = '';
      cliPendingTool = null;
      onChanged();
    }
  }

  /// `About.tsx:160::handleCliDiagnose`。0 个冲突 → 「无冲突」，否则报数量。
  Future<void> diagnoseCli() async {
    cliBusy = 'diagnose';
    cliMsg = '';
    cliErr = '';
    onChanged();
    try {
      final raw = await invoke('cli_diagnose_conflicts');
      cliConflicts = [
        for (final c in (raw as List<Object?>))
          CliConflict.fromJson((c as Map).cast<String, Object?>()),
      ];
      final n = cliConflicts.where((c) => c.isConflicting).length;
      cliMsg = n == 0
          ? t('about.localEnv.noConflicts')
          : t('about.localEnv.conflictFound', {'count': n});
    } catch (e) {
      cliErr = '${t('about.localEnv.diagnoseFailed')}: $e';
    } finally {
      cliBusy = '';
      onChanged();
    }
  }

  CliConflict? conflictFor(String tool) {
    for (final c in cliConflicts) {
      if (c.tool == tool) return c;
    }
    return null;
  }
}
