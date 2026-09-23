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

/// 冲突决策：保留本地（跳过） / 用导入的（覆盖） / 两者都留（重命名）。
///
/// 🔴 **wire 形状由后端的 `Decision` 定**（`gateway/import_export/mod.rs:182-188`）：
/// `#[serde(tag = "kind", rename_all = "snake_case")]` 的**对象**，
/// 取值只有 `overwrite` / `skip` / `rename`，且 `rename` 必须带 `new_key`。
/// 这里原先发的是裸字符串 `'keep_local'` —— 形状和取值都不对，
/// 于是**备份里只要有一条冲突，`import_apply` 必定反序列化失败**，
/// 而 `canApplyImport` 又要求冲突全决策完才放行，等于带冲突的导入整条是坏的。
enum ConflictDecisionKind { keepLocal, useIncoming, keepBoth }

/// 一条决策：种类 + 重命名时的新 key。
class ConflictDecision {
  const ConflictDecision(this.kind, {this.newKey = ''});

  final ConflictDecisionKind kind;

  /// 只有 [ConflictDecisionKind.keepBoth] 用得上。
  final String newKey;

  ConflictDecision withNewKey(String v) => ConflictDecision(kind, newKey: v);

  /// 后端 `Decision` 的 JSON 形状。
  Map<String, Object?> toWire() => switch (kind) {
    ConflictDecisionKind.keepLocal => const {'kind': 'skip'},
    ConflictDecisionKind.useIncoming => const {'kind': 'overwrite'},
    ConflictDecisionKind.keepBoth => {'kind': 'rename', 'new_key': newKey},
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

  /// 这份 [preview] 是导入那条路来的（true）还是导出那条路来的（false）。
  /// 两条路共用同一个字段，页面要靠它决定把勾选器画在哪张卡上 —— 否则读了
  /// 一份 .aidogx 之后，导出卡上也会冒出一份导入清单。
  bool previewIsImport = false;

  /// 预览里被勾中的条目（`[scope, key]` 对）。
  Set<String> selected = {};

  /// 冲突 key → 决策。
  Map<String, ConflictDecision> decisions = {};

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
      previewIsImport = false;
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
      previewIsImport = true;
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
  List<String> get conflictKeys => [for (final r in conflictRows) r.key];

  /// 冲突条目连同**本地现有那条的摘要**（后端 `ConflictItem.existing_summary`）。
  /// 只给 key 的话，用户要在「覆盖 / 跳过 / 保留两者」之间选，却看不到本地那条
  /// 长什么样 —— 等于让他闭着眼睛决定要不要覆盖自己的配置。
  List<({String key, String scope, String existing, String incoming})>
  get conflictRows {
    final c = preview?['conflicts'];
    if (c is! List) return const [];
    return [
      for (final e in c.whereType<Map>())
        (
          key: '${e['scope']} ${e['key']}',
          scope: '${e['scope']}',
          existing: '${e['existing_summary'] ?? ''}',
          incoming: '${e['incoming_summary'] ?? ''}',
        ),
    ];
  }

  /// 每个冲突都定了决策才能应用。
  /// 每个冲突都定了决策才能应用；选了「两者都留」还得真填了新 key。
  bool get allConflictsDecided => conflictKeys.every((k) {
    final d = decisions[k];
    if (d == null) return false;
    return d.kind != ConflictDecisionKind.keepBoth || d.newKey.trim().isNotEmpty;
  });

  bool get canApplyImport =>
      !busy && preview != null && allConflictsDecided && selected.isNotEmpty;

  /// 选「两者都留」时预填一个新 key（`ConflictRow.tsx:56` 的
  /// `item.key + "-imported"`），否则后端拿到空 key 会建一条没名字的行。
  void decide(String conflictKey, ConflictDecisionKind kind) {
    final key = _splitKey(conflictKey)[1];
    decisions = {
      ...decisions,
      conflictKey: ConflictDecision(
        kind,
        newKey: kind == ConflictDecisionKind.keepBoth ? '$key-imported' : '',
      ),
    };
    _notify();
  }

  /// 改重命名的新 key（`ConflictRow.tsx:62-67` 的输入框）。
  void setRenameKey(String conflictKey, String newKey) {
    final cur = decisions[conflictKey];
    if (cur == null) return;
    decisions = {...decisions, conflictKey: cur.withNewKey(newKey)};
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
                'decision': e.value.toWire(),
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

/// 导入条目 → 菜单组 id（`ImportExport/meta.ts:118::menuGroupOf`）。
///
/// `setting` scope 按 key 前缀再分一层（`scheduling:` 归调度、`tray:` / `popover:`
/// / `notification:` 归界面偏好），其余按 scope 直映射，未列出的一律归 `system`。
String menuGroupOf(String scope, String key) {
  if (scope == 'setting') {
    const settingScopeGroup = {
      'scheduling': 'scheduling',
      'tray': 'uiPref',
      'popover': 'uiPref',
      'notification': 'uiPref',
    };
    return settingScopeGroup[key.split(':').first] ?? 'system';
  }
  const scopeMenuGroup = {
    'platform': 'platform',
    'group': 'group',
    'group_platform': 'group_platform',
    'skills': 'extension',
    'mcp': 'extension',
    'middleware': 'rules',
    'codex': 'system',
    'claude_code': 'system',
    'model_price': 'system',
    'setting': 'system',
  };
  return scopeMenuGroup[scope] ?? 'system';
}

/// 菜单组 id → i18n key（`meta.ts:45-54` 的 `labelKey`）。
String menuGroupLabelKey(String id) => switch (id) {
  'platform' => 'importExport.menuGroup.platform',
  'group' => 'importExport.menuGroup.group',
  'group_platform' => 'importExport.menuGroup.groupPlatform',
  'extension' => 'importExport.menuGroup.extension',
  'rules' => 'importExport.menuGroup.rules',
  'scheduling' => 'importExport.menuGroup.scheduling',
  'uiPref' => 'importExport.menuGroup.uiPref',
  _ => 'importExport.menuGroup.system',
};
