/// 导入导出（`settings/importexport`）—— 对齐
/// `src/components/settings/ImportExport/ImportExportTab.tsx`
/// + `ScheduledBackupSection.tsx` + `CcSwitchImport.tsx` + `Sub2ApiImport.tsx`。
///
/// 这一页全是破坏性操作，确认与禁用条件必须照搬：
/// - 导出：先 `export_preview` 出可勾选清单 → 用户选路径 → `export_to_file`。
///   **没勾任何条目时不许导出**（会写出一个空备份，覆盖掉用户以为还在的文件）。
/// - 导入：`import_read_file` 出冲突预览 → 用户逐条决策 → `import_apply`。
///   禁用线照 React：没勾条目不许应用（冲突缺省即覆盖，不强制逐条拍板）。
/// - 定时备份：间隔 ≥1 小时、保留 1..=90 天，后端会再 clamp 一次。
library;

import 'dart:async';

import '../invoke.dart';

/// 可导入导出的范围（10 个，`meta.ts:16-26` 的 ALL_SCOPES 逐条对应）。
/// 与后端 `collect.rs` 的 scope_set 精确字符串匹配一致（
/// `aidog_core/src/gateway/import_export/mod.rs:32-40` 的 SCOPE_* 常量）。
const List<String> kImportExportScopes = [
  'platform',
  'group',
  'group_platform',
  'setting',
  'codex',
  'claude_code',
  'model_price',
  'mcp',
  'middleware',
  'skills',
];

/// 初始勾选的导出范围，照抄 React（`ImportExportTab.tsx:60-61` 的
/// `["platform", "group", "group_platform", "setting"]`，不含 skills / mcp）。
const Set<String> kInitialScopes = {
  'platform',
  'group',
  'group_platform',
  'setting',
};

/// scope wire id（snake_case）→ i18n label key（camelCase，`meta.ts` 的
/// labelKey 逐条对应）。三处不一致的显式列出，其余直接拼。
String scopeLabelKey(String scope) => switch (scope) {
  'group_platform' => 'importExport.scope.groupPlatform',
  'claude_code' => 'importExport.scope.claudeCode',
  'model_price' => 'importExport.scope.modelPrice',
  _ => 'importExport.scope.$scope',
};

/// scope wire id → ScopeCard 的 desc key（`meta.ts` 的 descKey 同一映射）。
String scopeDescKey(String scope) => switch (scope) {
  'group_platform' => 'importExport.scopeDesc.groupPlatform',
  'claude_code' => 'importExport.scopeDesc.claudeCode',
  'model_price' => 'importExport.scopeDesc.modelPrice',
  _ => 'importExport.scopeDesc.$scope',
};

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

  /// 勾选的导出范围（初值对齐 React，见 [kInitialScopes]）。
  Set<String> scopes = {...kInitialScopes};

  /// scope 变化 → 300ms 防抖自动拉预览（`ImportExportTab.tsx:112-126`）。
  /// 取代手动「预览导出项」按钮 —— 勾选即展开条目，连续勾多个只拉一次。
  Timer? _previewDebounce;
  int _previewRequest = 0;

  /// 初始化时也自动拉一次（React 的 effect 挂载即触发）。
  void schedulePreview() {
    _previewDebounce?.cancel();
    final request = ++_previewRequest;
    _previewDebounce = Timer(const Duration(milliseconds: 300), () {
      if (!previewIsImport) unawaited(exportPreview(request));
    });
  }

  /// 取消挂着的防抖（页面 dispose 时调，防 pending timer）。
  void dispose() {
    _previewDebounce?.cancel();
  }

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
  Future<void> exportPreview(int request) async {
    if (request != _previewRequest || previewIsImport) return;
    busy = true;
    error = '';
    _notify();
    try {
      final nextPreview = _map(
        await _invoke('export_preview', {'scopes': scopes.toList()}),
      );
      if (request != _previewRequest || previewIsImport) return;
      preview = nextPreview;
      previewIsImport = false;
      // 默认全选，与 React 的初始状态一致。
      selected = _allItemKeys(preview!);
    } catch (e) {
      if (request == _previewRequest && !previewIsImport) error = '$e';
    } finally {
      if (request == _previewRequest && !previewIsImport) {
        busy = false;
        _notify();
      }
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
    _previewDebounce?.cancel();
    ++_previewRequest;
    previewIsImport = true;
    preview = null;
    selected = {};
    decisions = {};
    busy = true;
    error = '';
    report = null;
    _notify();
    try {
      preview = _map(await _invoke('import_read_file', {'path': path}));
      selected = _allItemKeys(preview!);
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

  /// 应用按钮的禁用线照 React（`ImportExportTab.tsx:568`）：
  /// 只看「跑着没 / 有没有预览 / 有没有勾条目」，不要求冲突全决策完 ——
  /// React 侧冲突缺省即 overwrite，用户不改就直接覆盖。
  bool get canApplyImport => !busy && preview != null && selected.isNotEmpty;

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

  /// 区头的「全选 / 反选」（`ImportExportTab.tsx:83-84` 的 selectAll/deselectAll）。
  void setAllScopes(bool on) {
    scopes = on ? {...kImportExportScopes} : <String>{};
    schedulePreview();
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
    schedulePreview();
    _notify();
  }

  /// 按决策应用导入。**破坏性**：调用方先确认。
  ///
  /// [renameRequiredText]：选了「保留两者」但新名字空着时直接报错不发请求 ——
  /// 后端对空 new_key 是静默回落原名（`apply/mod.rs:583-586`），等于用户以为
  /// 保留了两条、实际覆盖了本地那条，所以这道闸必须在客户端拦。
  Future<void> applyImport(
    String path, {
    String renameRequiredText = '',
  }) async {
    if (!canApplyImport) return;
    if (decisions.values.any(
      (d) => d.kind == ConflictDecisionKind.keepBoth && d.newKey.isEmpty,
    )) {
      if (renameRequiredText.isNotEmpty) error = renameRequiredText;
      _notify();
      return;
    }
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

/// `setting` 条目按稳定的 scope:key 映射本地化标签；未知 key 由调用方保留后端标签。
String settingLabelKey(String scope, String key) =>
    const {
      'app:theme': 'importExport.settingLabel.app_theme',
      'app:locale': 'importExport.settingLabel.app_locale',
      'app:logging': 'importExport.settingLabel.app_logging',
      'app:script_executor': 'importExport.settingLabel.app_script_executor',
      'proxy:settings': 'importExport.settingLabel.proxy_settings',
      'proxy:proxy_client': 'importExport.settingLabel.proxy_client',
      'proxy:timeout': 'importExport.settingLabel.proxy_timeout',
      'proxy:logging': 'importExport.settingLabel.proxy_logging',
      'notification:settings':
          'importExport.settingLabel.notification_settings',
      'middleware:settings': 'importExport.settingLabel.middleware_settings',
      'scheduling:settings': 'importExport.settingLabel.scheduling_settings',
      'stats:settings': 'importExport.settingLabel.stats_settings',
      'stats:agg_rebuild_v1': 'importExport.settingLabel.stats_agg_rebuild_v1',
      'stats:agg_count_tokens_excluded_v1':
          'importExport.settingLabel.stats_agg_count_tokens_excluded_v1',
      'pricing:sync': 'importExport.settingLabel.pricing_sync',
      'tray:config': 'importExport.settingLabel.tray_config',
      'popover:config': 'importExport.settingLabel.popover_config',
      'global:claude_code': 'importExport.settingLabel.global_claude_code',
      'global:coding_tools_settings':
          'importExport.settingLabel.global_coding_tools_settings',
      'global:cc_codex_settings':
          'importExport.settingLabel.global_cc_codex_settings',
      'backup:settings': 'importExport.settingLabel.backup_settings',
      'db:compact_migrated_v1':
          'importExport.settingLabel.db_compact_migrated_v1',
    }['$scope:$key'] ??
    '';

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
