/// 技能页的逻辑层（票 I09），对应 `src/pages/Skills.tsx` + `Skills/useSkillsData.ts`
/// + `SkillInstallView.tsx` + `SkillDetailView.tsx`。
///
/// 三个控制器，与 React 那边三个视图一一对应：
/// - [SkillsController]   已装列表（SWR 缓存 + 后台 revalidate、启停、更新、四种卸载、对齐、导入）
/// - [SkillInstallController] 搜索安装子视图（350ms 防抖搜索、逐条安装、按 agent 分组批量安装）
/// - [SkillDetailController]  只读详情（文件树 + 单文件内容）
///
/// 模型字段名一律抄 `src/services/api/types/manual.ts`（Skills 这一族是手写 DTO，
/// 不在 `types/generated/` 里）——**snake_case**，与 MCP 那族的 camelCase 不同。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart' show VoidCallback;

import '../utils/pinyin.dart';
import 'invoke.dart';

/// 目标 agent。与 MCP 的 `claude-code` **不是同一套 slug**，各留各的
/// （`src/pages/Skills/constants.ts` 顶部那条注释说的就是这件事）。
const List<String> kSkillAgents = ['claude', 'codex'];

/// `useSkillsData.ts:171` 的获焦重查节流窗口，一字不改。
const int kRevalidateThrottleMs = 10000;

/// 操作结果提示的存活时长（`useSkillsData.ts:189` 的 4000ms）。
const Duration kSkillsMessageTtl = Duration(seconds: 4);

// ── 模型 ────────────────────────────────────────────────────────────

/// `manual.ts:345::SkillsEnv`。
class SkillsEnv {
  const SkillsEnv({required this.npxAvailable, this.nodeVersion});

  final bool npxAvailable;
  final String? nodeVersion;

  static SkillsEnv fromJson(Map<String, Object?> j) => SkillsEnv(
    npxAvailable: j['npx_available'] == true,
    nodeVersion: j['node_version'] as String?,
  );
}

/// `manual.ts:353::SkillInfo`。锁文件独有字段旧缓存可能缺（null），照抄。
class SkillInfo {
  const SkillInfo({
    required this.name,
    required this.enabledAgents,
    this.installedPath,
    this.description,
    this.source,
    this.sourceType,
    this.sourceUrl,
    this.skillFolderHash,
    this.pluginName,
    this.installedAt,
    this.updatedAt,
  });

  final String name;
  final List<String> enabledAgents;
  final String? installedPath;
  final String? description;
  final String? source;
  final String? sourceType;
  final String? sourceUrl;
  final String? skillFolderHash;
  final String? pluginName;
  final String? installedAt;
  final String? updatedAt;

  static SkillInfo fromJson(Map<String, Object?> j) => SkillInfo(
    name: (j['name'] as String?) ?? '',
    enabledAgents: [
      for (final a in (j['enabled_agents'] as List<Object?>? ?? const []))
        a as String,
    ],
    installedPath: j['installed_path'] as String?,
    description: j['description'] as String?,
    source: j['source'] as String?,
    sourceType: j['source_type'] as String?,
    sourceUrl: j['source_url'] as String?,
    skillFolderHash: j['skill_folder_hash'] as String?,
    pluginName: j['plugin_name'] as String?,
    installedAt: j['installed_at'] as String?,
    updatedAt: j['updated_at'] as String?,
  );

  SkillInfo withAgents(List<String> agents) => SkillInfo(
    name: name,
    enabledAgents: agents,
    installedPath: installedPath,
    description: description,
    source: source,
    sourceType: sourceType,
    sourceUrl: sourceUrl,
    skillFolderHash: skillFolderHash,
    pluginName: pluginName,
    installedAt: installedAt,
    updatedAt: updatedAt,
  );
}

/// `manual.ts:385::SkillsOpResult`。
class SkillsOpResult {
  const SkillsOpResult({
    required this.success,
    required this.stdout,
    required this.stderr,
  });

  final bool success;
  final String stdout;
  final String stderr;

  static SkillsOpResult fromJson(Map<String, Object?> j) => SkillsOpResult(
    success: j['success'] == true,
    stdout: (j['stdout'] as String?) ?? '',
    stderr: (j['stderr'] as String?) ?? '',
  );

  /// React 每个失败分支都写同一串：`stderr.trim() || stdout.trim() || 兜底文案`。
  String errorText(String fallback) {
    final e = stderr.trim();
    if (e.isNotEmpty) return e;
    final o = stdout.trim();
    if (o.isNotEmpty) return o;
    return fallback;
  }
}

/// `manual.ts:377::CatalogEntry`。
class CatalogEntry {
  const CatalogEntry({
    required this.id,
    required this.name,
    this.description,
    this.repoUrl,
  });

  final String id;
  final String name;
  final String? description;
  final String? repoUrl;

  static CatalogEntry fromJson(Map<String, Object?> j) => CatalogEntry(
    id: (j['id'] as String?) ?? '',
    name: (j['name'] as String?) ?? '',
    description: j['description'] as String?,
    repoUrl: j['repo_url'] as String?,
  );
}

/// `manual.ts:392::SkillFile`。
class SkillFile {
  const SkillFile({
    required this.relPath,
    required this.size,
    required this.isText,
  });

  final String relPath;
  final int size;
  final bool isText;

  static SkillFile fromJson(Map<String, Object?> j) => SkillFile(
    relPath: (j['rel_path'] as String?) ?? '',
    size: (j['size'] as num?)?.toInt() ?? 0,
    isText: j['is_text'] == true,
  );
}

/// `manual.ts:405::SkillFileContent`。`content == null` = 二进制，不预览。
class SkillFileContent {
  const SkillFileContent({this.content, this.truncated = false, this.size = 0});

  final String? content;
  final bool truncated;
  final int size;

  static SkillFileContent fromJson(Map<String, Object?> j) => SkillFileContent(
    content: j['content'] as String?,
    truncated: j['truncated'] == true,
    size: (j['size'] as num?)?.toInt() ?? 0,
  );
}

// ── 分享编解码（`src/pages/Skills/share.ts`）────────────────────────

/// `share.ts:4::skillCatalogId`。无 source（手动 symlink）→ null，不可分享。
String? skillCatalogId(SkillInfo s) {
  final src = s.source;
  if (src == null || src.isEmpty || s.name.isEmpty) return null;
  return '$src@${s.name}';
}

final RegExp _base64Shape = RegExp(r'^[A-Za-z0-9+/=\s]+$');

/// `share.ts:15::decodeSkillShare`。接受 `{skills:[...]}` 包裹或裸数组；
/// 每项必须含 `@`（`owner/repo@skill` 形态）。非法一律 null。
List<String>? decodeSkillShare(String text) {
  final trimmed = text.trim();
  if (trimmed.isEmpty) return null;
  var json = trimmed;
  // 形如 base64（字符集 + 足够长）→ 试 atob；失败就拿原文走 JSON.parse。
  if (_base64Shape.hasMatch(trimmed) && trimmed.length > 16) {
    try {
      json = utf8.decode(base64.decode(trimmed.replaceAll(RegExp(r'\s'), '')));
    } catch (_) {
      // 非 base64，保持原文本。
    }
  }
  try {
    final parsed = jsonDecode(json);
    final List<Object?>? ids = parsed is List
        ? parsed
        : parsed is Map && parsed['skills'] is List
        ? parsed['skills'] as List<Object?>
        : null;
    if (ids == null) return null;
    if (!ids.every((id) => id is String && id.contains('@'))) return null;
    return [for (final id in ids) id as String];
  } catch (_) {
    return null;
  }
}

/// `useSkillsData.ts:363` / `:391` 的两条 stdout 计数正则。
/// 解析不出来按 0 算（React 的 `m ? Number(m[1]) : 0`）。
int parseAlignedCount(String stdout) =>
    int.tryParse(
      RegExp(r'aligned (\d+) changes').firstMatch(stdout)?.group(1) ?? '',
    ) ??
    0;

int parseEnabledCount(String stdout) =>
    int.tryParse(
      RegExp(r'enabled (\d+) skills').firstMatch(stdout)?.group(1) ?? '',
    ) ??
    0;

// ── 已装列表控制器 ──────────────────────────────────────────────────

/// 翻译函数。页面把 `AidogI18n.of(context).t` 传进来 —— 控制器不碰 BuildContext。
typedef TrFn = String Function(String key, [Map<String, Object?>? args]);

/// `useSkillsData.ts` 的 Dart 版。状态是普通字段，改完调 [onChanged]（页面 setState）。
class SkillsController {
  SkillsController({
    required this.invoke,
    required this.t,
    required this.onChanged,
    this.now = _wallClock,
  });

  final InvokeFn invoke;
  final TrFn t;
  final VoidCallback onChanged;

  /// 注入时钟，把获焦节流做成可测的（测试里推时间，不睡觉）。
  final int Function() now;

  static int _wallClock() => DateTime.now().millisecondsSinceEpoch;

  SkillsEnv? env;
  String scopeKind = 'global';
  String projectPath = '';

  /// `list` = 已装列表（默认）；`install` = 搜索安装页。
  String subView = 'list';
  SkillInfo? detailTarget;

  List<SkillInfo> installed = const [];
  bool installedLoading = false;
  bool refreshing = false;
  int _lastRefreshAt = 0;

  /// `"<name>::<agent>"` / `__update__` / `__uninstall__` …；非 null 时禁并发。
  String? busyKey;
  String? message;
  Timer? _messageTimer;

  bool confirmUninstall = false;
  SkillInfo? uninstallTarget;
  final Set<String> selectedNames = <String>{};
  bool confirmUninstallBatch = false;

  bool alignOpen = false;
  String alignFrom = 'claude';
  String alignTo = 'codex';

  /// 分享弹窗载荷：`{skills:[id]}` + skill 名。
  ({List<String> skills, String name})? shareData;

  bool pasteOpen = false;
  String pasteText = '';

  /// 批量导入确认：null = 关闭态。
  List<String>? importIds;
  Set<String> importAgents = {...kSkillAgents};
  String importScopeKind = 'global';
  String importProjectPath = '';
  bool importBusy = false;

  String searchQuery = '';

  /// `all` / `enabled` / `disabled`。
  String enabledFilter = 'all';

  Map<String, Object?> get scope => scopeKind == 'project'
      ? {'kind': 'project', 'path': projectPath}
      : const {'kind': 'global'};

  Map<String, Object?> get importScope => importScopeKind == 'project'
      ? {'kind': 'project', 'path': importProjectPath}
      : const {'kind': 'global'};

  bool get writeReady => env?.npxAvailable == true;
  bool get scopeInvalid => scopeKind == 'project' && projectPath.trim().isEmpty;

  int get total => installed.length;

  /// 每 agent 的启用数，从已装列表派生（`useSkillsData.ts:195`）。
  Map<String, int> get agentCounts => {
    for (final a in kSkillAgents)
      a: installed.where((s) => s.enabledAgents.contains(a)).length,
  };

  /// `useSkillsData.ts:81` 的 memo。**统计/总数仍用全量**，筛选只影响列表展示。
  ///
  /// 与 React 的唯一差异：搜索是不分大小写子串，不是 `pinyinMatch`
  /// —— 同 I06 / I07 已记的取舍（见 flutter/README.md）。
  List<SkillInfo> get filteredInstalled {
    final q = searchQuery.trim();
    return installed.where((s) {
      if (enabledFilter == 'enabled' && s.enabledAgents.isEmpty) return false;
      if (enabledFilter == 'disabled' && s.enabledAgents.isNotEmpty) {
        return false;
      }
      if (q.isEmpty) return true;
      // 走 pinyinMatch 而不是 contains（`useSkillsData.ts:88-90`）：
      // 中文描述打拼音也要搜得到，裸 contains 只能整字匹配。
      return pinyinMatch(q, s.name) ||
          pinyinMatch(q, s.description ?? '') ||
          pinyinMatch(q, s.source ?? '');
    }).toList();
  }

  /// 进页：探测环境 + 载入列表（React 的两个 mount effect）。
  Future<void> init() async {
    await Future.wait([_checkEnv(), loadInstalled()]);
  }

  Future<void> _checkEnv() async {
    try {
      final r = await invoke('skills_check_env');
      env = SkillsEnv.fromJson((r as Map).cast<String, Object?>());
      onChanged();
    } catch (_) {
      // `console.error("check env failed")`：不弹给用户，写不了就是 writeReady=false。
    }
  }

  void setMessage(String? text) {
    message = text;
    _messageTimer?.cancel();
    if (text != null) {
      _messageTimer = Timer(kSkillsMessageTtl, () {
        message = null;
        onChanged();
      });
    }
    onChanged();
  }

  void dispose() {
    _messageTimer?.cancel();
  }

  List<SkillInfo> _itemsOf(Object? raw) => [
    for (final e in ((raw as Map)['items'] as List<Object?>? ?? const []))
      SkillInfo.fromJson((e as Map).cast<String, Object?>()),
  ];

  /// `useSkillsData.ts:102::refreshInstalled`。强制跑 npx 取最新，不阻塞列表。
  Future<void> refreshInstalled() async {
    if (scopeInvalid) {
      installed = const [];
      onChanged();
      return;
    }
    refreshing = true;
    onChanged();
    try {
      final r = await invoke('skills_list_refresh', {'scope': scope});
      installed = _itemsOf(r);
      _lastRefreshAt = now();
      if ((r as Map)['load_failed'] == true) {
        setMessage(t('skills.loadFailed'));
      }
    } catch (_) {
      // React 只 console.error：保留上一次的列表，不清空。
    } finally {
      refreshing = false;
      onChanged();
    }
  }

  /// `useSkillsData.ts:130::loadInstalled`。命中缓存 → 秒开 + 后台 revalidate；
  /// 未命中 / 缓存读失败 → 显加载态跑一次 refresh。
  Future<void> loadInstalled() async {
    if (scopeInvalid) {
      installed = const [];
      onChanged();
      return;
    }
    try {
      final cached = await invoke('skills_list_installed', {'scope': scope});
      if ((cached as Map)['stale'] != true) {
        installed = _itemsOf(cached);
        onChanged();
        await refreshInstalled();
        return;
      }
    } catch (_) {
      // 缓存读失败也兜底走 refresh。
    }
    installedLoading = true;
    onChanged();
    try {
      final r = await invoke('skills_list_refresh', {'scope': scope});
      installed = _itemsOf(r);
      if ((r as Map)['load_failed'] == true) {
        setMessage(t('skills.loadFailed'));
      }
    } catch (_) {
      // 同上：console.error，不炸页。
    } finally {
      installedLoading = false;
      onChanged();
    }
  }

  /// `useSkillsData.ts:172::maybeRevalidate`。窗口重新获焦时调；10 秒内不重复跑 npx。
  void maybeRevalidate() {
    if (subView != 'list' || scopeInvalid || refreshing) return;
    if (now() - _lastRefreshAt < kRevalidateThrottleMs) return;
    unawaited(refreshInstalled());
  }

  void setScopeKind(String kind) {
    scopeKind = kind;
    onChanged();
    unawaited(loadInstalled());
  }

  void setProjectPath(String path) {
    projectPath = path;
    onChanged();
    unawaited(loadInstalled());
  }

  void setSubView(String v) {
    subView = v;
    onChanged();
  }

  void setSearchQuery(String q) {
    searchQuery = q;
    onChanged();
  }

  void setEnabledFilter(String f) {
    enabledFilter = f;
    onChanged();
  }

  void setDetailTarget(SkillInfo? s) {
    detailTarget = s;
    onChanged();
  }

  /// `useSkillsData.ts:213::applyResult`：成功 → toast + 强制 refresh；失败 → 弹错。
  Future<void> _applyResult(SkillsOpResult res, String okKey) async {
    if (res.success) {
      setMessage(t(okKey));
      await refreshInstalled();
    } else {
      setMessage(res.errorText(t('skills.opFailed')));
    }
  }

  bool get _blocked => !writeReady || scopeInvalid || busyKey != null;

  /// `useSkillsData.ts:226::handleToggle`。**乐观翻转 + 失败回滚**。
  Future<void> toggle(SkillInfo skill, String agent) async {
    if (_blocked) return;
    final enabled = skill.enabledAgents.contains(agent);
    busyKey = '${skill.name}::$agent';
    setMessage(null);

    final prev = installed;
    installed = [
      for (final s in installed)
        if (s.name == skill.name)
          s.withAgents(
            enabled
                ? [
                    for (final a in s.enabledAgents)
                      if (a != agent) a,
                  ]
                : [...s.enabledAgents, agent],
          )
        else
          s,
    ];
    onChanged();

    try {
      final raw = enabled
          ? await invoke('skills_disable', {
              'name': skill.name,
              'agent': agent,
              'scope': scope,
            })
          : await invoke('skills_enable', {
              'name': skill.name,
              'path': skill.installedPath ?? '',
              'agent': agent,
              'scope': scope,
            });
      final res = SkillsOpResult.fromJson((raw as Map).cast<String, Object?>());
      if (res.success) {
        setMessage(t(enabled ? 'skills.disabled' : 'skills.enabled'));
      } else {
        installed = prev;
        setMessage(res.errorText(t('skills.opFailed')));
      }
    } catch (e) {
      installed = prev;
      setMessage('$e');
    } finally {
      busyKey = null;
      onChanged();
    }
  }

  /// `useSkillsData.ts:273::handleUpdate`。
  Future<void> updateAll() async {
    if (_blocked) return;
    busyKey = '__update__';
    setMessage(null);
    try {
      final raw = await invoke('skills_update', {'scope': scope});
      await _applyResult(
        SkillsOpResult.fromJson((raw as Map).cast<String, Object?>()),
        'skills.updated',
      );
    } catch (e) {
      setMessage('$e');
    } finally {
      busyKey = null;
      onChanged();
    }
  }

  void askUninstallAll() {
    confirmUninstall = true;
    onChanged();
  }

  void cancelUninstallAll() {
    confirmUninstall = false;
    onChanged();
  }

  /// `useSkillsData.ts:289::handleUninstallAll`。**确认框先关，再判 blocked**（照搬顺序）。
  Future<void> uninstallAll() async {
    confirmUninstall = false;
    onChanged();
    if (_blocked) return;
    busyKey = '__uninstall__';
    setMessage(null);
    try {
      final raw = await invoke('skills_uninstall_all', {'scope': scope});
      await _applyResult(
        SkillsOpResult.fromJson((raw as Map).cast<String, Object?>()),
        'skills.uninstallAllDone',
      );
    } catch (e) {
      setMessage('$e');
    } finally {
      busyKey = null;
      onChanged();
    }
  }

  void askUninstall(SkillInfo s) {
    uninstallTarget = s;
    onChanged();
  }

  void cancelUninstall() {
    uninstallTarget = null;
    onChanged();
  }

  /// `useSkillsData.ts:306::handleUninstallSingle`。
  Future<void> uninstallSingle() async {
    final target = uninstallTarget;
    uninstallTarget = null;
    onChanged();
    if (target == null || _blocked) return;
    busyKey = '__uninstall_single_${target.name}__';
    setMessage(null);
    try {
      final raw = await invoke('skills_uninstall', {
        'name': target.name,
        'scope': scope,
      });
      await _applyResult(
        SkillsOpResult.fromJson((raw as Map).cast<String, Object?>()),
        'skills.uninstallDone',
      );
    } catch (e) {
      setMessage('$e');
    } finally {
      busyKey = null;
      onChanged();
    }
  }

  void toggleSelected(String name) {
    if (!selectedNames.remove(name)) selectedNames.add(name);
    onChanged();
  }

  void askUninstallBatch() {
    confirmUninstallBatch = true;
    onChanged();
  }

  void cancelUninstallBatch() {
    confirmUninstallBatch = false;
    onChanged();
  }

  /// `useSkillsData.ts:334::handleUninstallBatch`。
  /// **勾选集合在判 blocked 之前就被清空**（React 同序），照搬。
  Future<void> uninstallBatch() async {
    confirmUninstallBatch = false;
    final names = selectedNames.toList();
    selectedNames.clear();
    onChanged();
    if (names.isEmpty || _blocked) return;
    busyKey = '__uninstall_batch__';
    setMessage(null);
    try {
      final raw = await invoke('skills_uninstall_batch', {
        'names': names,
        'scope': scope,
      });
      await _applyResult(
        SkillsOpResult.fromJson((raw as Map).cast<String, Object?>()),
        'skills.uninstallDone',
      );
    } catch (e) {
      setMessage('$e');
    } finally {
      busyKey = null;
      onChanged();
    }
  }

  void openAlign() {
    alignOpen = true;
    onChanged();
  }

  void closeAlign() {
    alignOpen = false;
    onChanged();
  }

  void setAlignFrom(String a) {
    alignFrom = a;
    onChanged();
  }

  void setAlignTo(String a) {
    alignTo = a;
    onChanged();
  }

  /// `useSkillsData.ts:353::handleAlign`。同 agent 直接 return（连弹窗都不关）。
  Future<void> align() async {
    if (alignFrom == alignTo) return;
    alignOpen = false;
    onChanged();
    if (_blocked) return;
    busyKey = '__align__';
    setMessage(null);
    try {
      final raw = await invoke('skills_align_agents', {
        'from': alignFrom,
        'to': alignTo,
        'scope': scope,
      });
      final res = SkillsOpResult.fromJson((raw as Map).cast<String, Object?>());
      if (res.success) {
        final n = parseAlignedCount(res.stdout);
        setMessage(
          n == 0 ? t('skills.alignNoop') : t('skills.alignDone', {'count': n}),
        );
        await refreshInstalled();
      } else {
        setMessage(res.errorText(t('skills.opFailed')));
      }
    } catch (e) {
      setMessage('$e');
    } finally {
      busyKey = null;
      onChanged();
    }
  }

  /// `useSkillsData.ts:384::handleEnableAll`。只增不减，非破坏性，不需确认。
  Future<void> enableAll(String agent) async {
    if (_blocked) return;
    busyKey = '__enableall_${agent}__';
    setMessage(null);
    try {
      final raw = await invoke('skills_enable_all', {
        'agent': agent,
        'scope': scope,
      });
      final res = SkillsOpResult.fromJson((raw as Map).cast<String, Object?>());
      if (res.success) {
        final n = parseEnabledCount(res.stdout);
        final agentLabel = t('skills.agent.$agent');
        setMessage(
          n == 0
              ? t('skills.enableAllNoop', {'agent': agentLabel})
              : t('skills.enableAllDone', {'agent': agentLabel, 'count': n}),
        );
        await refreshInstalled();
      } else {
        setMessage(res.errorText(t('skills.opFailed')));
      }
    } catch (e) {
      setMessage('$e');
    } finally {
      busyKey = null;
      onChanged();
    }
  }

  /// `useSkillsData.ts:417::handleShare`。非 catalog 来源 → 提示，不开弹窗。
  void share(SkillInfo skill) {
    final id = skillCatalogId(skill);
    if (id == null) {
      setMessage(t('skills.share.noSource'));
      return;
    }
    shareData = (skills: [id], name: skill.name);
    onChanged();
  }

  void closeShare() {
    shareData = null;
    onChanged();
  }

  /// `useSkillsData.ts:427::openImportConfirm`。空清单不开框。
  void openImportConfirm(List<String> ids) {
    if (ids.isEmpty) return;
    importIds = ids;
    importAgents = {...kSkillAgents};
    importScopeKind = 'global';
    importProjectPath = '';
    onChanged();
  }

  void cancelImport() {
    importIds = null;
    onChanged();
  }

  void toggleImportAgent(String agent) {
    if (!importAgents.remove(agent)) importAgents.add(agent);
    onChanged();
  }

  void setImportScopeKind(String kind) {
    importScopeKind = kind;
    onChanged();
  }

  void setImportProjectPath(String p) {
    importProjectPath = p;
    onChanged();
  }

  void setPasteOpen(bool open) {
    pasteOpen = open;
    onChanged();
  }

  void setPasteText(String s) {
    pasteText = s;
    onChanged();
  }

  /// `useSkillsData.ts:440::handleImport`。**逐条 `skills_install`**（不是批量命令），
  /// 分成功 / 全失败 / 部分成功三种文案；失败清单跟在部分成功那条后面。
  Future<void> runImport() async {
    final ids = importIds;
    if (ids == null || importAgents.isEmpty) return;
    if (importScopeKind == 'project' && importProjectPath.trim().isEmpty) {
      return;
    }
    if (env?.npxAvailable != true) {
      setMessage(t('skills.envMissing'));
      return;
    }
    importBusy = true;
    setMessage(null);
    final target = importScope;
    final agents = importAgents.toList();
    var ok = 0;
    var fail = 0;
    final failed = <String>[];
    for (final id in ids) {
      try {
        final raw = await invoke('skills_install', {
          'id': id,
          'agents': agents,
          'scope': target,
        });
        final res = SkillsOpResult.fromJson(
          (raw as Map).cast<String, Object?>(),
        );
        if (res.success) {
          ok += 1;
        } else {
          fail += 1;
          failed.add(id);
        }
      } catch (_) {
        fail += 1;
        failed.add(id);
      }
    }
    importIds = null;
    importBusy = false;
    if (fail == 0) {
      setMessage(t('skills.importOk', {'count': ok}));
    } else if (ok == 0) {
      setMessage(t('skills.importFail', {'count': fail}));
    } else {
      setMessage(
        t('skills.importPartial', {'ok': ok, 'fail': fail}) +
            (failed.isNotEmpty ? '\n${failed.join(', ')}' : ''),
      );
    }
    unawaited(refreshInstalled());
  }

  /// `useSkillsData.ts:486::handlePasteImport`。解码失败只提示，不关弹窗。
  void pasteImport() {
    final ids = decodeSkillShare(pasteText);
    if (ids == null || ids.isEmpty) {
      setMessage(t('skills.importInvalid'));
      return;
    }
    pasteOpen = false;
    pasteText = '';
    openImportConfirm(ids);
  }

  /// `useSkillsData.ts:501::openDeepLinkImport`。`aidog://skill/import?data=…`。
  void openDeepLinkImport(String data) {
    if (data.isEmpty) return;
    final ids = decodeSkillShare(data);
    if (ids == null || ids.isEmpty) {
      setMessage(t('skills.importInvalid'));
      return;
    }
    openImportConfirm(ids);
  }
}

// ── 搜索安装子视图（`SkillInstallView.tsx`）────────────────────────

/// `SkillInstallView.tsx:93` 的防抖窗口，一字不改。
const Duration kSkillSearchDebounce = Duration(milliseconds: 350);

class SkillInstallController {
  SkillInstallController({
    required this.invoke,
    required this.t,
    required this.onChanged,
    required this.scope,
    required this.onInstalled,
    this.debounce = kSkillSearchDebounce,
  });

  final InvokeFn invoke;
  final TrFn t;
  final VoidCallback onChanged;
  final Map<String, Object?> scope;

  /// 安装成功后通知父级刷新已装列表。
  final VoidCallback onInstalled;
  final Duration debounce;

  String keyword = '';
  List<CatalogEntry> results = const [];
  bool loading = false;
  String? error;

  /// catalog id → 选中的 agent 集合，默认全选。
  final Map<String, Set<String>> selected = {};

  /// 批量安装的勾选集合。
  final Set<String> checked = <String>{};

  /// 正在装的条目 id；`__batch__` = 批量安装中。非 null 时禁并发。
  String? busyId;
  String? message;

  Timer? _debounceTimer;

  bool get hasKeyword => keyword.trim().isNotEmpty;

  void dispose() {
    _debounceTimer?.cancel();
  }

  /// 输入即重置 350ms 防抖计时（`SkillInstallView.tsx:91` 的 effect）。
  void setKeyword(String kw) {
    keyword = kw;
    onChanged();
    _debounceTimer?.cancel();
    _debounceTimer = Timer(debounce, () => unawaited(search(kw)));
  }

  /// `SkillInstallView.tsx:59::doSearch`。空关键字 = 清空，不发命令。
  Future<void> search(String kw) async {
    final k = kw.trim();
    if (k.isEmpty) {
      results = const [];
      error = null;
      loading = false;
      onChanged();
      return;
    }
    loading = true;
    error = null;
    onChanged();
    try {
      final raw = await invoke('skills_search', {'keyword': k});
      results = [
        for (final e in (raw as List<Object?>))
          CatalogEntry.fromJson((e as Map).cast<String, Object?>()),
      ];
      for (final e in results) {
        selected.putIfAbsent(e.id, () => {...kSkillAgents});
      }
    } catch (e) {
      error = '$e';
      results = const [];
    } finally {
      loading = false;
      onChanged();
    }
  }

  void toggleAgent(String id, String agent) {
    final set = selected.putIfAbsent(id, () => <String>{});
    if (!set.remove(agent)) set.add(agent);
    onChanged();
  }

  void toggleChecked(String id) {
    if (!checked.remove(id)) checked.add(id);
    onChanged();
  }

  /// `SkillInstallView.tsx:110::handleInstall`。一条 catalog 一次 `skills_install`。
  Future<void> install(CatalogEntry entry) async {
    final agents = selected[entry.id];
    if (agents == null || agents.isEmpty) return;
    busyId = entry.id;
    message = null;
    onChanged();
    try {
      final raw = await invoke('skills_install', {
        'id': entry.id,
        'agents': agents.toList(),
        'scope': scope,
      });
      final res = SkillsOpResult.fromJson((raw as Map).cast<String, Object?>());
      if (res.success) {
        message = t('skills.install.installSuccess', {'name': entry.name});
        onInstalled();
      } else {
        message = res.errorText(t('skills.install.installFailed'));
      }
    } catch (e) {
      message = '$e';
    } finally {
      busyId = null;
      onChanged();
    }
  }

  /// `SkillInstallView.tsx:156::handleInstallBatch`。
  /// **按「agent 集合」分组**：同组 agents 相同 → 一次 `skills_install_batch`
  /// （后端再把同仓库合并成单次 npx）。分组键是排序后的 agents 串。
  Future<void> installBatch() async {
    final entries = [
      for (final e in results)
        if (checked.contains(e.id)) e,
    ];
    if (entries.isEmpty) return;
    busyId = '__batch__';
    message = null;
    onChanged();

    final groups = <String, ({List<String> ids, List<String> agents})>{};
    for (final e in entries) {
      final agents = (selected[e.id] ?? const <String>{}).toList();
      if (agents.isEmpty) continue;
      final key = ([...agents]..sort()).join(',');
      final g = groups.putIfAbsent(
        key,
        () => (ids: <String>[], agents: agents),
      );
      g.ids.add(e.id);
    }

    var ok = 0;
    var fail = 0;
    for (final g in groups.values) {
      try {
        final raw = await invoke('skills_install_batch', {
          'ids': g.ids,
          'agents': g.agents,
          'scope': scope,
        });
        final res = SkillsOpResult.fromJson(
          (raw as Map).cast<String, Object?>(),
        );
        if (res.success) {
          ok += g.ids.length;
        } else {
          fail += g.ids.length;
          message = res.errorText(t('skills.install.installFailed'));
        }
      } catch (e) {
        fail += g.ids.length;
        message = '$e';
      }
    }
    busyId = null;
    if (ok > 0 && fail == 0) {
      message = t('skills.importOk', {'count': ok});
    } else if (fail > 0 && ok > 0) {
      message = t('skills.importPartial', {'ok': ok, 'fail': fail});
    }
    if (ok > 0) {
      checked.clear();
      onInstalled();
    }
    onChanged();
  }
}

// ── 只读详情（`SkillDetailView.tsx`）──────────────────────────────

/// `SkillDetailView.tsx:31::formatSize`，阈值与小数位一字不改。
String formatSkillFileSize(int n) {
  if (n < 1024) return '$n B';
  if (n < 1024 * 1024) return '${(n / 1024).toStringAsFixed(1)} KB';
  return '${(n / (1024 * 1024)).toStringAsFixed(1)} MB';
}

bool isMarkdownPath(String rel) =>
    RegExp(r'\.md$', caseSensitive: false).hasMatch(rel);

class SkillDetailController {
  SkillDetailController({
    required this.invoke,
    required this.t,
    required this.onChanged,
    required this.skill,
  });

  final InvokeFn invoke;
  final TrFn t;
  final VoidCallback onChanged;
  final SkillInfo skill;

  List<SkillFile> files = const [];
  String? selected;
  SkillFileContent? content;
  bool loadingList = true;
  bool loadingFile = false;
  String? error;

  SkillFile? get selectedFile {
    for (final f in files) {
      if (f.relPath == selected) return f;
    }
    return null;
  }

  /// `SkillDetailView.tsx:49` 的 mount effect：无路径直接报错，不发命令。
  /// 默认选 `SKILL.md`，没有就选第一条。
  Future<void> init() async {
    final path = skill.installedPath;
    if (path == null || path.isEmpty) {
      error = t('skills.detail.loadFailed');
      loadingList = false;
      onChanged();
      return;
    }
    loadingList = true;
    error = null;
    onChanged();
    try {
      final raw = await invoke('skill_detail', {'installedPath': path});
      files = [
        for (final f in ((raw as Map)['files'] as List<Object?>? ?? const []))
          SkillFile.fromJson((f as Map).cast<String, Object?>()),
      ];
      final hasSkillMd = files.any((f) => f.relPath == 'SKILL.md');
      selected = hasSkillMd
          ? 'SKILL.md'
          : files.isNotEmpty
          ? files.first.relPath
          : null;
    } catch (e) {
      error = '$e';
    } finally {
      loadingList = false;
      onChanged();
    }
    final sel = selected;
    if (sel != null) await loadFile(sel);
  }

  /// `SkillDetailView.tsx:77::loadFile`。
  Future<void> loadFile(String rel) async {
    final path = skill.installedPath;
    if (path == null || path.isEmpty) return;
    selected = rel;
    loadingFile = true;
    content = null;
    onChanged();
    try {
      final raw = await invoke('skill_read_file', {
        'installedPath': path,
        'rel': rel,
      });
      content = SkillFileContent.fromJson((raw as Map).cast<String, Object?>());
    } catch (e) {
      error = '$e';
    } finally {
      loadingFile = false;
      onChanged();
    }
  }
}
