/// 导入导出（`settings/importexport`）—— 对齐
/// `src/components/settings/ImportExport/ImportExportTab.tsx`
/// + `ScheduledBackupSection.tsx` + `CcSwitchImport.tsx` + `Sub2ApiImport.tsx`。
///
/// 这一页全是破坏性操作，确认与禁用条件必须照搬：
/// - 导出：先 `export_preview` 出可勾选清单 → 用户选路径 → `export_to_file`。
///   **没勾任何条目时不许导出**（会写出一个空备份，覆盖掉用户以为还在的文件）。
/// - 导入：`import_read_file` 出冲突预览 → 用户逐条决策 → `import_apply`。
///   **决策没定完不许应用**。
/// - 定时备份：间隔 ≥1 小时、保留 1..=90 天，后端会再 clamp 一次。
library;

import '../invoke.dart';

/// 可导入导出的范围。与后端 `ImportExportScope` 的 serde 值一致。
const List<String> kImportExportScopes = [
  'platforms',
  'groups',
  'settings',
  'skills',
  'mcp',
];

/// 冲突决策：保留本地 / 用导入的 / 两者都留（重命名）。
enum ConflictDecisionKind { keepLocal, useIncoming, keepBoth }

extension ConflictDecisionWire on ConflictDecisionKind {
  String get wire => switch (this) {
    ConflictDecisionKind.keepLocal => 'keep_local',
    ConflictDecisionKind.useIncoming => 'use_incoming',
    ConflictDecisionKind.keepBoth => 'keep_both',
  };
}

/// 定时备份设置（`types/manual.ts::BackupSettings`）。
class BackupSettings {
  const BackupSettings({
    required this.enabled,
    required this.intervalHours,
    required this.retentionDays,
    this.dir = '',
    this.lastBackupAt = 0,
    this.lastBackupError = '',
  });

  final bool enabled;

  /// ≥1。
  final int intervalHours;

  /// 1..=90。
  final int retentionDays;
  final String dir;

  /// 上次成功备份的 epoch 毫秒（0 = 从未），由后端写（`aidog_backup/src/lib.rs:73`）。
  final int lastBackupAt;

  /// 上次备份的错误信息（空 = 成功），由后端写（`aidog_backup/src/lib.rs:76`）。
  final String lastBackupError;

  factory BackupSettings.fromJson(Map<String, Object?> j) => BackupSettings(
    enabled: j['enabled'] as bool? ?? false,
    intervalHours: (j['interval_hours'] as num?)?.toInt() ?? 24,
    retentionDays: (j['retention_days'] as num?)?.toInt() ?? 7,
    dir: j['dir'] as String? ?? '',
    lastBackupAt: (j['last_backup_at'] as num?)?.toInt() ?? 0,
    lastBackupError: j['last_backup_error'] as String? ?? '',
  );

  Map<String, Object?> toJson() => {
    'enabled': enabled,
    'interval_hours': intervalHours,
    'retention_days': retentionDays,
    'dir': dir,
    'last_backup_at': lastBackupAt,
    'last_backup_error': lastBackupError,
  };

  /// 下次预计备份时刻（epoch 毫秒）。0 = 没开或从未备份过 → 界面上不显示这一行。
  /// 与 React `ScheduledBackupSection.tsx:84` 同算法。
  int get nextBackupAt => enabled && lastBackupAt > 0
      ? lastBackupAt + intervalHours * 3600 * 1000
      : 0;

  /// 前端先 clamp 一次，后端还会再 clamp。两边都做是为了输入框当场看到被纠正的值。
  BackupSettings clamped() => BackupSettings(
    enabled: enabled,
    intervalHours: intervalHours < 1 ? 1 : intervalHours,
    retentionDays: retentionDays.clamp(1, 90),
    dir: dir,
    lastBackupAt: lastBackupAt,
    lastBackupError: lastBackupError,
  );

  BackupSettings copyWith({
    bool? enabled,
    int? intervalHours,
    int? retentionDays,
    String? dir,
  }) => BackupSettings(
    enabled: enabled ?? this.enabled,
    intervalHours: intervalHours ?? this.intervalHours,
    retentionDays: retentionDays ?? this.retentionDays,
    dir: dir ?? this.dir,
    lastBackupAt: lastBackupAt,
    lastBackupError: lastBackupError,
  );
}

class ImportExportController {
  ImportExportController({InvokeFn? invoke, this.onChanged})
    : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  /// 勾选的导出范围。
  Set<String> scopes = {...kImportExportScopes};

  /// `export_preview` / `import_read_file` 的结果。null = 还没预览过。
  Map<String, Object?>? preview;

  /// 预览里被勾中的条目（`[scope, key]` 对）。
  Set<String> selected = {};

  /// 冲突 key → 决策。
  Map<String, ConflictDecisionKind> decisions = {};

  /// `import_apply` 的报告。
  Map<String, Object?>? report;

  bool busy = false;
  String error = '';
  String message = '';

  void _notify() => onChanged?.call();

  /// 导出前预览：列出可勾选条目（conflicts 恒空）。
  Future<void> exportPreview() async {
    busy = true;
    error = '';
    _notify();
    try {
      preview = _map(
        await _invoke('export_preview', {'scopes': scopes.toList()}),
      );
      // 默认全选，与 React 的初始状态一致。
      selected = _allItemKeys(preview!);
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  /// 没勾任何条目就不许导出 —— 否则写出一个空备份，覆盖掉用户以为还在的文件。
  bool get canExport => !busy && preview != null && selected.isNotEmpty;

  /// [path] 由调用方用 I12 的 `pickPath` 取。
  Future<void> exportToFile(String path, String doneText) async {
    if (!canExport) return;
    busy = true;
    error = '';
    _notify();
    try {
      await _invoke('export_to_file', {
        'scopes': scopes.toList(),
        'path': path,
        'selection': selected.map(_splitKey).toList(),
      });
      message = doneText;
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  /// 读文件 → 解密 → 冲突预览。
  Future<void> readImportFile(String path) async {
    busy = true;
    error = '';
    report = null;
    _notify();
    try {
      preview = _map(await _invoke('import_read_file', {'path': path}));
      selected = _allItemKeys(preview!);
      decisions = {};
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  /// 预览里列出的冲突 key。
  List<String> get conflictKeys {
    final c = preview?['conflicts'];
    if (c is! List) return const [];
    return c.whereType<Map>().map((e) => '${e['scope']} ${e['key']}').toList();
  }

  /// 每个冲突都定了决策才能应用。
  bool get allConflictsDecided => conflictKeys.every(decisions.containsKey);

  bool get canApplyImport =>
      !busy && preview != null && allConflictsDecided && selected.isNotEmpty;

  void decide(String conflictKey, ConflictDecisionKind kind) {
    decisions = {...decisions, conflictKey: kind};
    _notify();
  }

  void toggleSelected(String itemKey, bool on) {
    final next = {...selected};
    if (on) {
      next.add(itemKey);
    } else {
      next.remove(itemKey);
    }
    selected = next;
    _notify();
  }

  void toggleScope(String scope, bool on) {
    final next = {...scopes};
    if (on) {
      next.add(scope);
    } else {
      next.remove(scope);
    }
    scopes = next;
    _notify();
  }

  /// 按决策应用导入。**破坏性**：调用方先确认。
  Future<void> applyImport(String path) async {
    if (!canApplyImport) return;
    busy = true;
    error = '';
    _notify();
    try {
      report = _map(
        await _invoke('import_apply', {
          'path': path,
          'decisions': [
            for (final e in decisions.entries)
              {
                'scope': _splitKey(e.key)[0],
                'key': _splitKey(e.key)[1],
                'decision': e.value.wire,
              },
          ],
          'selection': selected.map(_splitKey).toList(),
        }),
      );
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  static Set<String> _allItemKeys(Map<String, Object?> preview) {
    final items = preview['items'];
    if (items is! List) return <String>{};
    return items
        .whereType<Map>()
        .map((e) => '${e['scope']} ${e['key']}')
        .toSet();
  }

  static List<String> _splitKey(String k) => k.split(' ');

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}

/// 定时备份。
class ScheduledBackupController {
  ScheduledBackupController({InvokeFn? invoke, this.onChanged})
    : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  BackupSettings settings = const BackupSettings(
    enabled: false,
    intervalHours: 24,
    retentionDays: 7,
  );
  bool busy = false;
  String error = '';
  String message = '';

  /// 最近一次「立即备份」写出的文件路径。null = 还没成功备份过（本次会话内）。
  /// 「在文件夹显示 / 复制路径」按钮据此出现，与 React 的 `lastResultPath` 同语义。
  String? lastResultPath;

  void _notify() => onChanged?.call();

  Future<void> load() async {
    try {
      settings = BackupSettings.fromJson(
        _map(await _invoke('backup_settings_get')),
      );
    } catch (_) {
      /* 后端默认 */
    }
    _notify();
  }

  /// 后端会 clamp 非法值并返回规范化后的值 —— 用返回值覆盖本地，
  /// 这样输入框当场显示被纠正后的数字。
  Future<void> persist(BackupSettings next) async {
    settings = next.clamped();
    error = '';
    _notify();
    try {
      final r = _map(
        await _invoke('backup_settings_set', {'settings': settings.toJson()}),
      );
      if (r.isNotEmpty) settings = BackupSettings.fromJson(r);
    } catch (e) {
      error = '$e';
    }
    _notify();
  }

  /// 立即备份一次（忽略 throttle）。
  ///
  /// `backup_run_now` **不会抛错**：失败也返回 `{ok:false, error}`（`commands.rs:87`）。
  /// 所以 `ok == false` 必须单独判 —— 只 catch 异常会把失败显示成成功
  /// （React `ScheduledBackupSection.tsx:74` 同一判定）。成功后重读一次设置，
  /// 把 `last_backup_at` 刷新到界面上。
  Future<void> runNow(
    String Function(Map<String, Object?> result) doneText, {
    String failedText = '',
  }) async {
    busy = true;
    error = '';
    message = '';
    lastResultPath = null;
    _notify();
    try {
      final r = _map(await _invoke('backup_run_now'));
      if (r['ok'] == false) {
        final e = r['error'];
        error = (e is String && e.isNotEmpty) ? e : failedText;
      } else {
        message = doneText(r);
        final p = r['path'];
        lastResultPath = (p is String && p.isNotEmpty) ? p : null;
        try {
          settings = BackupSettings.fromJson(
            _map(await _invoke('backup_settings_get')),
          );
        } catch (_) {
          /* 刷新失败不影响这次备份本身 */
        }
      }
    } catch (e) {
      error = '$e';
    } finally {
      busy = false;
      _notify();
    }
  }

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}

/// 拖进来的一串路径里挑出要导入的那个。
///
/// 与 React 的 drop 分支逐条对齐（`ImportExportTab.tsx:288-296`）：
///   - 有 `.aidogx` → 取**第一个**（拖一堆文件进来也只导一个，不批量）
///   - 一个都没有但确实拖了东西 → 返回 null，调用方报 `error.notAidogx`
///   - 什么都没拖到 → 同样 null，调用方什么都不做（靠 [paths] 是否为空区分）
///
/// 大小写不敏感：macOS 的文件系统默认不区分大小写，`.AIDOGX` 也是合法文件名。
String? pickAidogxPath(List<String> paths) {
  for (final p in paths) {
    if (p.toLowerCase().endsWith('.aidogx')) return p;
  }
  return null;
}
