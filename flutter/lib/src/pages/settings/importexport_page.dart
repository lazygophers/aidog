/// 导入导出页（`settings/importexport`）的 widget 层 —— 对齐
/// `src/components/settings/ImportExport/ImportExportTab.tsx` +
/// `ScheduledBackupSection.tsx` + `CcSwitchImport.tsx` + `Sub2ApiImport.tsx`。
///
/// 这一页全是破坏性操作，禁用条件一条都不能少 —— 它们全在
/// [ImportExportController] 的派生态里（`canExport` / `canApplyImport`），
/// 本文件不重判。
///
/// 文件对话框走 I12 的 `pickPath`（`platform.dart`），不自己开 `file_selector`。
library;

import 'dart:async';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../../platform.dart' as native;
import '../../../utils/formatters.dart';
import '../../shell/theme.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import '../platform_card_bits.dart' show MiniBadge, ProtocolMetaTable;
import '../platform_defaults.dart';
import 'bits.dart';
import 'ccswitch_match.dart';
import 'foreign_import_logic.dart';
import 'importexport_logic.dart';

/// 选一个 .aidogx 路径。抽成函数是为了让 widget 测试能整体替换掉原生面板。
typedef PickPathFn = Future<String?> Function({bool save, String? suggested});

Future<String?> _defaultPickPath({bool save = false, String? suggested}) =>
    native.pickPath(
      native.PickPathOptions(
        save: save,
        defaultPath: suggested,
        filters: const [
          native.PickFilter('aidogx', ['aidogx']),
        ],
      ),
    );

class ImportExportPage extends StatefulWidget {
  const ImportExportPage({
    super.key,
    this.invoke = kernelInvoke,
    this.pickPath = _defaultPickPath,
  });

  final InvokeFn invoke;
  final PickPathFn pickPath;

  @override
  State<ImportExportPage> createState() => _ImportExportPageState();
}

class _ImportExportPageState extends State<ImportExportPage> {
  late final ImportExportController _c;
  late final ScheduledBackupController _b;
  late final ForeignImportController _cc;
  late final ForeignImportController _sub;
  late final ForeignImportFollowUp _followUp;

  /// 已选中的 .aidogx 路径（导入流程用）。
  String? _importPath;

  /// 有文件正悬在落区上方（拖拽高亮）。
  bool _dragActive = false;

  /// registry 派生的两份元数据，异源导入的协议匹配要用
  /// （关键词 / host / 默认端点全来自它们，代码里不写平台名）。
  ProtocolMetaTable _meta = const ProtocolMetaTable();
  PlatformDefaults _defaults = PlatformDefaults.empty;

  /// cc-switch 的三个导入维度（`CcSwitchImport.tsx:311-331`）。
  /// 平台类型那一维 React 锁定常开，这里同样不给关。
  CcImportDims _dims = const CcImportDims();

  /// 点按钮选文件 → 校验扩展名 → 读预览。点击与拖入两条入口走同一段。
  Future<void> _pickImportFile() async {
    final p = await widget.pickPath();
    if (p == null || !mounted) return;
    if (pickAidogxPath([p]) == null) {
      setState(
        () =>
            _c.error = AidogI18n.of(context).t('importExport.error.notAidogx'),
      );
      return;
    }
    await _loadImportFile(p);
  }

  Future<void> _loadImportFile(String path) async {
    setState(() => _importPath = path);
    await _c.readImportFile(path);
  }

  /// sub2api provider 的 key 前 4 后 4 遮罩（React `Sub2ApiImport.tsx` 同口径）。
  String _maskKey(Object? value) {
    final key = value is String ? value : '';
    if (key.isEmpty) return '';
    if (key.length <= 10) return '••••';
    return '${key.substring(0, 4)}••••${key.substring(key.length - 4)}';
  }

  /// sub2api 的粘贴文本。
  String _pasteText = '';

  @override
  void initState() {
    super.initState();
    void rebuild() {
      if (mounted) setState(() {});
    }

    _c = ImportExportController(invoke: widget.invoke, onChanged: rebuild);
    _b = ScheduledBackupController(invoke: widget.invoke, onChanged: rebuild);
    _cc = ForeignImportController(
      source: ForeignSource.ccswitch,
      invoke: widget.invoke,
      onChanged: rebuild,
    );
    _sub = ForeignImportController(
      source: ForeignSource.sub2api,
      invoke: widget.invoke,
      onChanged: rebuild,
    );
    _followUp = ForeignImportFollowUp(invoke: widget.invoke);
    unawaited(_b.load());
    // React 的 debounce effect 挂载即跑一次（初始 scopes 非空）。
    _c.schedulePreview();
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  bool _metaRequested = false;

  /// 语言要从 context 取，而 `initState` 里还够不着 InheritedWidget ——
  /// 所以这一拉挂在 `didChangeDependencies` 上，并用标志位保证只拉一次。
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_metaRequested) return;
    _metaRequested = true;
    unawaited(_loadProtocolMeta());
    unawaited(_loadGroupOptions());
  }

  /// 协议元数据是 best-effort：拉不到就回落空表，匹配退化成协议回退那一支，
  /// 页面照常能用（与 TS 侧 `.catch` 同语义）。
  Future<void> _loadProtocolMeta() async {
    final locale = AidogI18n.of(context).locale;
    String raw = '';
    try {
      raw = '${await widget.invoke('get_defaults_json') ?? ''}';
    } catch (_) {
      raw = '';
    }
    final defaults = await loadPlatformDefaults(widget.invoke, locale);
    if (!mounted) return;
    setState(() {
      _meta = ProtocolMetaTable.parse(raw, locale);
      _defaults = defaults;
    });
  }

  /// sub2api 预览行里用户手改的协议（下标 → 协议）。没改过的走
  /// [mapSub2apiPlatform] 的映射。
  final Map<int, String> _subProtocolOverrides = {};

  /// sub2api 账号 → Platform JSON（`sub2apiMatch.ts:60`）。
  ///
  /// 这里和 cc-switch 用的**不是**同一个转换器：sub2api 的字段是
  /// `platform` / `baseUrl` / `apiKey`，拿 cc-switch 那套 `appType` /
  /// `detectedBaseUrl` 去读只会读到空，导进来又是一批空壳平台。
  List<Map<String, Object?>> _toSubPayload(List<Map<String, Object?>> chosen) {
    final out = <Map<String, Object?>>[];
    for (final a in chosen) {
      final i = _sub.providers.indexOf(a);
      out.add(
        sub2apiAccountToPlatformJson(
          a,
          defaults: _defaults,
          protocolOverride: _subProtocolOverrides[i],
        ),
      );
    }
    return out;
  }

  /// 下拉里可选的协议（coding 套餐是独立协议，不进这个列表 ——
  /// 与 React `Sub2ApiImport.tsx:269` 的 `filter(p => !p.codingPlan)` 同）。
  List<String> get _protocolOptions => [
    for (final code in _meta.labels.keys)
      if (!_meta.codingPlanProtocols.contains(code)) code,
  ];

  /// sub2api 预览行右侧：协议下拉 + 「未识别·已兜底」徽标
  /// （`Sub2ApiImport.tsx:260-275`）。
  ///
  /// 协议必须能当场手改：sub2api 只给 `platform` 一个词，认不出来就兜底成
  /// openai，导完才发现协议不对的话，那条平台是废的。
  Widget _subRowExtra(int i, Map<String, Object?> row) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final mapped = mapSub2apiPlatform('${row['platform'] ?? ''}');
    final current = _subProtocolOverrides[i] ?? mapped.protocol;
    final options = _protocolOptions;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        DropdownButton<String>(
          key: ValueKey('sub2api-protocol-$i'),
          value: options.contains(current) ? current : null,
          hint: Text(
            current,
            style: AidogType.micro.copyWith(color: theme.c.fg2),
          ),
          underline: const SizedBox.shrink(),
          isDense: true,
          dropdownColor: theme.c.surface2,
          style: AidogType.micro.copyWith(color: theme.c.fg),
          items: [
            for (final o in options)
              DropdownMenuItem<String>(
                value: o,
                child: Text(_meta.labels[o] ?? o),
              ),
          ],
          onChanged: (v) {
            if (v == null) return;
            setState(() => _subProtocolOverrides[i] = v);
          },
        ),
        // 手改过就不再提「未识别」——那条提示说的是自动映射的结果。
        if (!mapped.recognized && !_subProtocolOverrides.containsKey(i)) ...[
          const SizedBox(width: AidogSpace.sxs),
          MiniBadge(
            text: t.t('importExport.sub2api.unrecognized'),
            color: theme.c.peak,
          ),
        ],
        const SizedBox(width: AidogSpace.sxs),
      ],
    );
  }

  /// 已有分组（批量归属的候选）。`(id, name)` 两样够画 chip 了。
  List<({int id, String name})> _groupOptions = const [];

  /// 勾中的分组 id：导入完把这批平台一起加进去（`CcSwitchImport.tsx:203-212`）。
  final Set<int> _batchJoinGroupIds = {};

  Future<void> _loadGroupOptions() async {
    try {
      final rows = await _followUp.listGroupDetails();
      if (rows is! List || !mounted) return;
      setState(() {
        _groupOptions = [
          for (final r in rows.whereType<Map>())
            if (r['group'] is Map)
              (
                id: ((r['group'] as Map)['id'] as num?)?.toInt() ?? 0,
                name: '${(r['group'] as Map)['name'] ?? ''}',
              ),
        ];
      });
    } catch (_) {
      // 拉不到就不画 chip：自动建组那条路照旧可用，不拦导入。
    }
  }

  /// 导入完把这批平台加进勾中的分组（`CcSwitchImport.tsx:203-212`）：
  /// 按最终名字在平台列表里找回刚建出来的行，再逐个 `platform_update`。
  /// 失败不回滚、不拦报告 —— 平台已经建好了，分组没挂上是可以再手工挂的。
  Future<void> _joinChosenGroups(List<Map<String, Object?>> payload) async {
    if (_batchJoinGroupIds.isEmpty) return;
    try {
      final names = {for (final p in payload) '${p['name'] ?? ''}'};
      final rows = await _followUp.listPlatforms();
      if (rows is! List) return;
      for (final r in rows.whereType<Map>()) {
        if (!names.contains('${r['name'] ?? ''}')) continue;
        await _followUp.updatePlatform({
          'id': r['id'],
          'join_group_ids': _batchJoinGroupIds.toList(),
        });
      }
    } catch (_) {
      // 同上：不阻断导入报告。
    }
  }

  /// 单个 provider 的匹配结果（行内读数与 payload 共用同一条链，不算两遍）。
  CcMatchResult _matchOf(Map<String, Object?> p) =>
      matchCcProvider(p, meta: _meta, defaults: _defaults);

  /// 勾中的 provider → Platform JSON（`CcSwitchImport.tsx:176-190`）。
  /// 原先这里是 `(chosen) => chosen` —— 原样把 cc-switch 的 provider map 发出去，
  /// 后端缺字段一律写空串，导进去的是一串没有协议 / 没有 URL / 没有密钥的空壳。
  List<Map<String, Object?>> _toPlatformPayload(
    List<Map<String, Object?>> chosen,
  ) => [
    for (final p in chosen)
      ccProviderToPlatformJson(
        p,
        matchCcProvider(p, meta: _meta, defaults: _defaults),
        _dims,
      ),
  ];

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return SettingsPageBody(
      title: t.t('appSettings.importExportTab'),
      children: [
        _exportCard(t),
        _importCard(t),
        _backupCard(t),
        _foreignCard(
          t,
          c: _cc,
          showDims: true,
          toPayload: _toPlatformPayload,
          groupAssignHint: t.t('importExport.ccswitch.groupAssignHint'),
          title: t.t('importExport.ccswitch.title'),
          description: t.t('importExport.ccswitch.desc'),
          autoGroupLabel: t.t('importExport.ccswitch.autoGroup'),
          header: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              const Padding(
                padding: EdgeInsets.only(bottom: AidogSpace.ssm),
                child: PiUnsupportedNote(
                  reasonKey: 'pi.unsupportedCcSwitch',
                  reasonFallback: '上游 cc-switch 只管 claude 与 codex provider，本身不认识 pi，没有可导入的 pi 数据。',
                ),
              ),
              Row(
                children: [
                  SmallButton(
                    key: const ValueKey('ccswitch-detect'),
                    label: _cc.busy
                        ? t.t('importExport.ccswitch.detecting')
                        : t.t('importExport.ccswitch.detectBtn'),
                    onTap: _cc.busy
                        ? null
                        : () async {
                            await _cc.detect();
                            if (mounted) await _cc.read();
                          },
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                  SmallButton(
                    key: const ValueKey('ccswitch-pick-dir'),
                    label: t.t('importExport.ccswitch.selectDir'),
                    onTap: _cc.busy
                        ? null
                        : () async {
                            final dir = await native.pickPath(
                              const native.PickPathOptions(directory: true),
                            );
                            if (dir == null || !mounted) return;
                            await _cc.detect(overridePath: dir);
                            if (mounted) await _cc.read(path: dir);
                          },
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                  if (_cc.detection != null)
                    Text(
                      _cc.detection!.isEmpty
                          ? t.t('importExport.ccswitch.notDetected')
                          : t.t('importExport.ccswitch.detected'),
                      style: AidogType.micro.copyWith(
                        color: AidogTheme.of(context).c.fg3,
                      ),
                    ),
                ],
              ),
            ],
          ),
        ),
        _foreignCard(
          t,
          c: _sub,
          toPayload: _toSubPayload,
          rowExtra: _subRowExtra,
          title: t.t('importExport.sub2api.title'),
          description: t.t('importExport.sub2api.desc'),
          autoGroupLabel: t.t('importExport.sub2api.autoGroup'),
          header: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              // 粘的是 sub2api 导出的整份账号 JSON，`maxLines: 4` 只看得见
              // 四行 —— 粘全没粘全、括号对不对都得靠猜。React 那边是
              // `JsonCodeEditor`，高度 120–320px（`Sub2ApiImport.tsx:195-201`）。
              // 等宽行高 12.5×1.35 ≈ 17px：7 行 ≈ 119px 起、19 行 ≈ 323px 封顶。
              TextRow(
                key: const ValueKey('sub2api-paste'),
                label: t.t('importExport.sub2api.title'),
                hint: t.t('importExport.sub2api.pastePlaceholder'),
                value: _pasteText,
                minLines: 7,
                maxLines: 19,
                mono: true,
                onChanged: (v) => setState(() => _pasteText = v),
              ),
              Row(
                children: [
                  SmallButton(
                    key: const ValueKey('sub2api-parse'),
                    label: _sub.busy
                        ? t.t('importExport.sub2api.parsing')
                        : t.t('importExport.sub2api.parsePaste'),
                    // 没内容就点不动（React 的 `emptyInput` 分支）。
                    // 只禁不说等于把那句解释丢了，所以禁用时把它挂成悬浮提示。
                    tooltip: _pasteText.trim().isEmpty
                        ? t.t('importExport.sub2api.emptyInput')
                        : null,
                    onTap: _sub.busy || _pasteText.trim().isEmpty
                        ? null
                        : () => _sub.parse(_pasteText),
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                  SmallButton(
                    key: const ValueKey('sub2api-pick-file'),
                    label: t.t('importExport.sub2api.pickFile'),
                    onTap: _sub.busy
                        ? null
                        : () async {
                            final p = await native.pickPath(
                              const native.PickPathOptions(
                                filters: [
                                  native.PickFilter('json', ['json']),
                                ],
                              ),
                            );
                            if (p == null || !mounted) return;
                            final text = await _sub.readFile(p);
                            if (text == null || !mounted) return;
                            setState(() => _pasteText = text);
                            await _sub.parse(text);
                          },
                  ),
                ],
              ),
            ],
          ),
        ),
        if (_c.error.isNotEmpty) ErrorNote(text: _c.error),
        if (_c.message.isNotEmpty)
          AutoToast(
            text: _c.message,
            onDone: () => setState(() => _c.message = ''),
          ),
      ],
    );
  }

  // ── 导出 ────────────────────────────────────────────────

  Widget _exportCard(I18nController t) => SettingsCard(
    title: t.t('importExport.exportTitle'),
    description: t.t('importExport.exportDesc'),
    children: [
      TileMetaLine(t.t('importExport.scopeHeader')),
      Wrap(
        spacing: AidogSpace.sxs,
        runSpacing: AidogSpace.sxs,
        children: [
          for (final s in kImportExportScopes)
            SmallButton(
              key: ValueKey('scope-$s'),
              // label key 是 camelCase（`scopeLabelKey`），wire id 是 snake_case。
              label: tOr(t, scopeLabelKey(s), s),
              active: _c.scopes.contains(s),
              onTap: () => _c.toggleScope(s, !_c.scopes.contains(s)),
            ),
        ],
      ),
      Text(
        t.t('importExport.skillsScopeHint'),
        style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg3),
      ),
      const SizedBox(height: AidogSpace.ssm),
      // 没有手动预览按钮：scope 勾选变化经控制器 300ms 防抖自动拉预览
      // （`ImportExportTab.tsx:112-126`），预览期间 busy 顶掉导出。
      Align(
        alignment: AlignmentDirectional.centerStart,
        child: SmallButton(
          key: const ValueKey('export-run'),
          // 三态照抄 `ImportExportTab.tsx:467-473`：跑着的时候说「导出中」，
          // 预览出来之后按钮上直接带上会导几项 —— 光写「导出」看不出导什么。
          label: _c.busy
              ? (_c.preview == null
                    ? t.t('importExport.loadingPreview')
                    : t.t('importExport.exporting'))
              : _c.preview != null
              ? t.t('importExport.exportN', {'n': _c.selected.length})
              : t.t('importExport.exportBtn'),
          // 没勾任何条目不许导出 —— 会写出一个空备份覆盖掉用户以为还在的文件。
          onTap: _c.canExport
              ? () async {
                  final now = DateTime.now();
                  final day = '${now.year}-${pad(now.month)}-${pad(now.day)}';
                  final p = await widget.pickPath(
                    save: true,
                    // 默认文件名照 React（`ImportExportTab.tsx:137`）。
                    suggested: 'aidog-export-$day.aidogx',
                  );
                  if (p == null || !mounted) return;
                  await _c.exportToFile(
                    p,
                    t.t('importExport.exportDone', {'path': p}),
                  );
                }
              : null,
        ),
      ),
      if (_c.scopes.isEmpty) ErrorNote(text: t.t('importExport.error.noScope')),
      if (_c.preview != null && !_c.previewIsImport) _itemPicker(t),
    ],
  );

  // ── 导入 ────────────────────────────────────────────────

  Widget _importCard(I18nController t) {
    final report = _c.report;
    return SettingsCard(
      title: t.t('importExport.importTitle'),
      description: t.t('importExport.importDesc'),
      children: [
        _DropZone(
          active: _dragActive,
          hint: t.t('importExport.dropHint'),
          title: t.t('importExport.pickFile'),
          onTap: _c.busy ? null : _pickImportFile,
          onDragEntered: () => setState(() => _dragActive = true),
          onDragExited: () => setState(() => _dragActive = false),
          onDropped: (paths) {
            setState(() => _dragActive = false);
            if (paths.isEmpty) return;
            final target = pickAidogxPath(paths);
            if (target == null) {
              setState(() => _c.error = t.t('importExport.error.notAidogx'));
              return;
            }
            unawaited(_loadImportFile(target));
          },
        ),
        const SizedBox(height: AidogSpace.ssm),
        Row(
          children: [
            SmallButton(
              key: const ValueKey('import-pick'),
              label: t.t('importExport.pickFile'),
              onTap: _c.busy ? null : _pickImportFile,
            ),
            const SizedBox(width: AidogSpace.ssm),
            SmallButton(
              key: const ValueKey('import-apply'),
              // 三态照抄 React（`ImportExportTab.tsx:566-578`）：跑着说「导入中」，
              // 平时按钮上直接带会导几项。点了直接执行，没有二次确认卡。
              label: _c.busy
                  ? t.t('importExport.applying')
                  : t.t('importExport.applyN', {'n': _c.selected.length}),
              // 冲突没定完 / 一项没选就点不动（`canApplyImport`）。
              onTap: _c.canApplyImport && _importPath != null
                  ? () {
                      final p = _importPath;
                      if (p != null) _c.applyImport(p);
                    }
                  : null,
            ),
          ],
        ),
        if (_importPath != null)
          InfoRow(
            label: t.t('importExport.pickFile'),
            value: ltr(_importPath!),
          ),
        // 概要卡 + 逐项勾选（`ImportExportTab.tsx:506-537`）：光有一条文件路径
        // 看不出这份备份是谁、什么时候、导进来会动多少东西。
        if (_c.preview != null && _c.previewIsImport) ...[
          _previewSummary(t),
          _itemPicker(t),
        ],
        if (_c.conflictKeys.isNotEmpty) ...[
          // 计数 + 两颗批量决策同一行，压在冲突行**上方**
          //（`ImportExportTab.tsx:541-549`）。原先批量按钮落在整张清单末尾：
          // 冲突多的时候要一路滚到底才找得到「全部覆盖 / 全部跳过」。
          Row(
            children: [
              Expanded(
                child: TileMetaLine(
                  t.t('importExport.conflicts', {'n': _c.conflictKeys.length}),
                  // 告警色（`ImportExportTab.tsx:542` 的 --color-warning）：
                  // 冲突是要逐条拍板的事，标题不能混成普通小节标题的灰。
                  color: AidogTheme.of(context).c.peak,
                ),
              ),
              SmallButton(
                key: const ValueKey('bulk-overwrite'),
                label: t.t('importExport.bulkOverwrite'),
                onTap: () {
                  for (final k in _c.conflictKeys) {
                    _c.decide(k, ConflictDecisionKind.useIncoming);
                  }
                },
              ),
              const SizedBox(width: AidogSpace.sxs),
              SmallButton(
                key: const ValueKey('bulk-skip'),
                label: t.t('importExport.bulkSkip'),
                onTap: () {
                  for (final k in _c.conflictKeys) {
                    _c.decide(k, ConflictDecisionKind.keepLocal);
                  }
                },
              ),
            ],
          ),
          for (final row in _c.conflictRows)
            Builder(
              key: ValueKey('conflict-${row.key}'),
              builder: (context) {
                final k = row.key;
                final theme = AidogTheme.of(context);
                return Container(
                  margin: const EdgeInsets.only(bottom: AidogSpace.sxs),
                  padding: const EdgeInsets.all(AidogSpace.sxs),
                  decoration: BoxDecoration(
                    border: Border.all(color: theme.c.line),
                    borderRadius: BorderRadius.circular(AidogRadius.sm),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Row(
                        children: [
                          Icon(
                            scopeIcon(row.scope),
                            size: 12,
                            color: theme.c.fg3,
                          ),
                          const SizedBox(width: AidogSpace.sxs),
                          Expanded(
                            child: Text(
                              ltr(k),
                              overflow: TextOverflow.ellipsis,
                              style: AidogType.micro.copyWith(
                                color: theme.c.fg,
                              ),
                            ),
                          ),
                        ],
                      ),
                      // 本地现有那条的摘要（`ConflictRow.tsx:46`）：不给它，
                      // 用户就得闭着眼睛决定要不要覆盖自己的配置。
                      if (row.existing.isNotEmpty)
                        Text(
                          ltr(row.existing),
                          key: ValueKey('conflict-existing-$k'),
                          style: AidogType.caption.copyWith(color: theme.c.fg3),
                        ),
                      Row(
                        children: [
                          for (final d in ConflictDecisionKind.values) ...[
                            SmallButton(
                              key: ValueKey('decide-$k-${d.name}'),
                              label: switch (d) {
                                ConflictDecisionKind.keepLocal => t.t(
                                  'importExport.skip',
                                ),
                                ConflictDecisionKind.useIncoming => t.t(
                                  'importExport.overwrite',
                                ),
                                ConflictDecisionKind.keepBoth => t.t(
                                  'importExport.rename',
                                ),
                              },
                              active: _c.decisions[k]?.kind == d,
                              onTap: () => _c.decide(k, d),
                            ),
                            const SizedBox(width: AidogSpace.sxs),
                          ],
                          // 选了「重命名」才出新名输入框（`ConflictRow.tsx:62-67`）。
                          // 没有它的话这个选项等于发一个空 key 过去。
                          if (_c.decisions[k]?.kind ==
                              ConflictDecisionKind.keepBoth)
                            SizedBox(
                              width: 220,
                              child: KeptTextField(
                                key: ValueKey('rename-$k'),
                                value: _c.decisions[k]!.newKey,
                                onChanged: (v) => _c.setRenameKey(k, v),
                              ),
                            ),
                        ],
                      ),
                    ],
                  ),
                );
              },
            ),
        ],
        // 导入结果：三个计数 + 三个分区 + 错误原文逐条（`ReportView.tsx:24-61`）。
        // 原先是把整个 map 直接 `InfoRow(key, '$value')` 铺开，屏幕上出现的是
        // `applied → {platform: 3}` 这种 Dart toString —— 错误原文读不出来。
        if (report != null) _report(t, report),
      ],
    );
  }

  /// 导入结果报告（`ImportReport`：applied / skipped 两张计数表 + errors 数组，
  /// `gateway/import_export/mod.rs:198-203`）。
  Widget _report(I18nController t, Map<String, Object?> report) {
    final theme = AidogTheme.of(context);
    Map<String, int> counts(String key) {
      final v = report[key];
      if (v is! Map) return const {};
      return {
        for (final e in v.entries) '${e.key}': (e.value as num?)?.toInt() ?? 0,
      };
    }

    final applied = counts('applied');
    final skipped = counts('skipped');
    final errors = [
      for (final e in (report['errors'] as List? ?? const [])) '$e',
    ];
    int total(Map<String, int> m) => m.values.fold(0, (a, b) => a + b);

    Widget section(String title, Color color, List<String> rows) => Padding(
      padding: const EdgeInsets.only(top: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(title, style: AidogType.micro.copyWith(color: color)),
          for (final r in rows)
            Text(r, style: AidogType.caption.copyWith(color: theme.c.fg2)),
        ],
      ),
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        TileMetaLine(t.t('importExport.reportTitle')),
        Wrap(
          spacing: AidogSpace.ssm,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            MiniBadge(
              text: '${t.t('importExport.applied')} ${total(applied)}',
              color: theme.c.ok,
            ),
            MiniBadge(
              text: '${t.t('importExport.skipped')} ${total(skipped)}',
              color: theme.c.fg3,
            ),
            if (errors.isNotEmpty)
              MiniBadge(
                text: '${t.t('importExport.errorsLabel')} ${errors.length}',
                color: theme.c.bad,
              ),
          ],
        ),
        if (applied.isNotEmpty)
          section(t.t('importExport.applied'), theme.c.ok, [
            for (final e in applied.entries)
              '${tOr(t, scopeLabelKey(e.key), e.key)}: ${e.value}',
          ]),
        if (skipped.isNotEmpty)
          section(t.t('importExport.skipped'), theme.c.fg3, [
            for (final e in skipped.entries)
              '${tOr(t, scopeLabelKey(e.key), e.key)}: ${e.value}',
          ]),
        // 错误原文逐条列出来 —— 这是导入失败时唯一能查的东西。
        if (errors.isNotEmpty)
          section(
            t.t('importExport.errors', {'n': '${errors.length}'}),
            theme.c.bad,
            errors,
          ),
      ],
    );
  }

  /// 导入预览的概要卡（`ImportExportTab.tsx:506-523`）：来源机器 + 导出时间
  /// + 每个范围各几条。回答的是「这份备份是谁的、导进来会动哪些东西」。
  Widget _previewSummary(I18nController t) {
    final theme = AidogTheme.of(context);
    final manifest = _c.preview!['manifest'];
    final counts = _c.preview!['counts'];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (manifest is Map) ...[
          InfoRow(
            label: t.t('importExport.sourceMachine'),
            value: ltr('${manifest['source_machine'] ?? ''}'),
          ),
          InfoRow(
            label: t.t('importExport.createdAt'),
            value: ltr('${manifest['created_at'] ?? ''}'),
          ),
        ],
        if (counts is Map && counts.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.sxs),
            child: Wrap(
              spacing: AidogSpace.sxs,
              runSpacing: AidogSpace.sxs,
              children: [
                for (final e in counts.entries)
                  MiniBadge(
                    text:
                        '${tOr(t, scopeLabelKey(e.key), '${e.key}')} '
                        '${e.value}',
                    color: theme.c.fg3,
                  ),
              ],
            ),
          ),
      ],
    );
  }

  /// 折叠起来的菜单组 id。默认全展开（条目多时用户自己收）。
  final Set<String> _collapsedGroups = {};

  /// 预览里的可勾选条目（导出与导入共用同一份 `items`）。
  ///
  /// 按菜单组分组 + 可折叠 + 组级三态全选（`ItemSelector.tsx:39-171`）。
  /// 原先是几百条平铺：要只导某一类，得一条条点过去。
  Widget _itemPicker(I18nController t) {
    final theme = AidogTheme.of(context);
    final items = (_c.preview!['items'] as List? ?? const [])
        .whereType<Map>()
        .toList();
    if (items.isEmpty) {
      return CenteredNote(text: t.t('importExport.exportEmpty'));
    }
    String keyOf(Map e) => '${e['scope']} ${e['key']}';

    // 按出现顺序分组，不重排 —— 后端给的顺序本身有意义。
    final groups = <String, List<Map>>{};
    for (final e in items) {
      final gid = menuGroupOf('${e['scope']}', '${e['key']}');
      (groups[gid] ??= []).add(e);
    }

    void setMany(List<Map> rows, bool on) {
      for (final e in rows) {
        _c.toggleSelected(keyOf(e), on);
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // 标题 + 「已选 n / 共 m」（`ItemSelector.tsx:63-72`）：只报已选数看不出
        // 还剩多少没勾。
        TileMetaLine(t.t('importExport.selectItems')),
        TileMetaLine(
          '${t.t('importExport.selectedLabel')} '
          '${_c.selected.length} / ${items.length}',
        ),
        Row(
          children: [
            SmallButton(
              label: t.t('importExport.selectAll'),
              onTap: () => setMany(items, true),
            ),
            const SizedBox(width: AidogSpace.ssm),
            SmallButton(
              label: t.t('importExport.deselectAll'),
              onTap: () => setMany(items, false),
            ),
          ],
        ),
        ConstrainedBox(
          constraints: const BoxConstraints(maxHeight: 360),
          child: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final entry in groups.entries)
                  _itemGroup(t, theme, entry.key, entry.value, keyOf, setMany),
              ],
            ),
          ),
        ),
      ],
    );
  }

  /// 一个菜单组：组头（折叠箭头 + 三态复选框 + 组名 + 计数）+ 展开后的条目行。
  Widget _itemGroup(
    I18nController t,
    AidogTheme theme,
    String gid,
    List<Map> rows,
    String Function(Map) keyOf,
    void Function(List<Map>, bool) setMany,
  ) {
    final open = !_collapsedGroups.contains(gid);
    final selected = rows.where((e) => _c.selected.contains(keyOf(e))).length;
    final allOn = selected == rows.length;
    // 半选：这组里挑了几条。没有这一档的话，收起来之后就看不出这组动过没有。
    final someOn = selected > 0 && !allOn;
    // skills 条目要单独提醒（`ItemSelector.tsx:118-131`）：它会跑 npx 装东西，
    // 用户手动全清掉时得说一声。
    final skills = rows.where((e) => e['scope'] == 'skills').toList();
    final skillsSelected = skills
        .where((e) => _c.selected.contains(keyOf(e)))
        .length;
    return Container(
      margin: const EdgeInsets.only(bottom: AidogSpace.sxs),
      decoration: BoxDecoration(
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          InkWell(
            key: ValueKey('item-group-$gid'),
            onTap: () => setState(
              () => open
                  ? _collapsedGroups.add(gid)
                  : _collapsedGroups.remove(gid),
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: AidogSpace.sxs,
                vertical: 4,
              ),
              child: Row(
                children: [
                  Icon(
                    open ? Icons.expand_more : Icons.chevron_right,
                    size: 14,
                    color: theme.c.fg3,
                  ),
                  // 组级三态：全选 / 半选 / 全不选。点它整组翻转。
                  GestureDetector(
                    key: ValueKey('item-group-check-$gid'),
                    onTap: () => setMany(rows, !allOn),
                    child: Icon(
                      allOn
                          ? Icons.check_box
                          : someOn
                          ? Icons.indeterminate_check_box
                          : Icons.check_box_outline_blank,
                      size: 14,
                      color: allOn || someOn ? theme.c.accentText : theme.c.fg3,
                    ),
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  Expanded(
                    child: Text(
                      t.t(menuGroupLabelKey(gid)),
                      style: AidogType.micro.copyWith(color: theme.c.fg),
                    ),
                  ),
                  if (skills.isNotEmpty && skillsSelected == 0) ...[
                    MiniBadge(
                      text: t.t('importExport.skillsScopeHint'),
                      color: theme.c.peak,
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                  ],
                  Text(
                    ltr('$selected / ${rows.length}'),
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                ],
              ),
            ),
          ),
          if (open)
            for (final e in rows) _itemRow(t, theme, e, keyOf(e)),
        ],
      ),
    );
  }

  /// 一条可勾选条目。
  Widget _itemRow(I18nController t, AidogTheme theme, Map e, String k) {
    final on = _c.selected.contains(k);
    final label = '${e['label'] ?? ''}';
    final localized = settingLabelKey(
      '${e['scope'] ?? ''}',
      '${e['key'] ?? ''}',
    );
    return InkWell(
      key: ValueKey('item-$k'),
      onTap: () => _c.toggleSelected(k, !on),
      child: Padding(
        padding: const EdgeInsets.only(
          left: AidogSpace.smd,
          right: AidogSpace.sxs,
          top: 2,
          bottom: 2,
        ),
        child: Row(
          children: [
            Icon(
              on ? Icons.check_box : Icons.check_box_outline_blank,
              size: 14,
              color: on ? theme.c.accentText : theme.c.fg3,
            ),
            const SizedBox(width: AidogSpace.sxs),
            // scope 图标（`ItemSelector.tsx:155`）：混在一张清单里时，
            // 图标比 scope 名更快认出这条是平台还是设置。
            Icon(scopeIcon('${e['scope']}'), size: 12, color: theme.c.fg3),
            const SizedBox(width: AidogSpace.sxs),
            Expanded(
              child: Text(
                // 后端给了人话标签（平台名 / 分组名 / 文件名），
                // 只画 `scope key` 的话用户认不出这条是什么。
                ltr(
                  localized.isEmpty ? label : t.t(localized),
                ),
                overflow: TextOverflow.ellipsis,
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
            ),
            // 冲突徽标（`ItemSelector.tsx:160-162`）：本地已有同名条目，
            // 导入时要在上面的冲突区逐条定夺。
            if (e['conflict'] == true) ...[
              const SizedBox(width: AidogSpace.sxs),
              MiniBadge(
                text: t.t('importExport.conflictTag'),
                color: theme.c.peak,
              ),
            ],
          ],
        ),
      ),
    );
  }

  // ── 定时备份 ────────────────────────────────────────────

  /// 备份时刻：0 = 从未。格式照 React `ScheduledBackupSection.tsx::formatBackupTime`
  /// 的 `YYYY-MM-DD HH:MM:SS`（不是 `formatters.dart::formatDateTime` 的斜杠形态）。
  static String _backupTime(I18nController t, int ms) {
    if (ms <= 0) return t.t('settings.backup.never');
    final d = DateTime.fromMillisecondsSinceEpoch(ms);
    return '${d.year}-${pad(d.month)}-${pad(d.day)} '
        '${pad(d.hour)}:${pad(d.minute)}:${pad(d.second)}';
  }

  Widget _backupCard(I18nController t) {
    final s = _b.settings;
    return SettingsCard(
      title: t.t('settings.backup.title'),
      description: t.t('settings.backup.desc'),
      children: [
        SwitchRow(
          key: const ValueKey('backup-enabled'),
          label: t.t('settings.backup.enable'),
          value: s.enabled,
          onChanged: (v) => _b.persist(s.copyWith(enabled: v)),
        ),
        NumberRow(
          key: const ValueKey('backup-interval'),
          label:
              '${t.t('settings.backup.interval')} '
              '(${t.t('settings.backup.hours')})',
          value: s.intervalHours,
          onChanged: (v) => _b.persist(s.copyWith(intervalHours: v)),
        ),
        Wrap(
          key: const ValueKey('backup-interval-presets'),
          spacing: AidogSpace.sxs,
          runSpacing: AidogSpace.sxs,
          children: [
            for (final preset in const [
              (hours: 1, key: 'settings.backup.preset1h'),
              (hours: 6, key: 'settings.backup.preset6h'),
              (hours: 12, key: 'settings.backup.preset12h'),
              (hours: 24, key: 'settings.backup.presetDaily'),
              (hours: 168, key: 'settings.backup.presetWeekly'),
            ])
              SmallButton(
                key: ValueKey('backup-preset-${preset.hours}h'),
                label: t.t(preset.key),
                active: s.intervalHours == preset.hours,
                onTap: () =>
                    _b.persist(s.copyWith(intervalHours: preset.hours)),
              ),
          ],
        ),
        NumberRow(
          key: const ValueKey('backup-retention'),
          label:
              '${t.t('settings.backup.retention')} '
              '(${t.t('settings.backup.days')})',
          value: s.retentionDays,
          onChanged: (v) => _b.persist(s.copyWith(retentionDays: v)),
        ),
        if (s.enabled) ...[
          InfoRow(
            key: const ValueKey('backup-last'),
            label: t.t('settings.backup.lastBackup'),
            value: _backupTime(t, s.lastBackupAt),
          ),
          if (s.nextBackupAt > 0)
            InfoRow(
              key: const ValueKey('backup-next'),
              label: t.t('settings.backup.nextBackup'),
              value: _backupTime(t, s.nextBackupAt),
            ),
          InfoRow(
            label: t.t('settings.backup.location'),
            value: ltr(s.dir.isEmpty ? '~/.aidog/backups/' : s.dir),
          ),
          if (s.lastBackupError.isNotEmpty)
            ErrorNote(
              key: const ValueKey('backup-last-error'),
              text: '${t.t('settings.backup.lastError')}: ${s.lastBackupError}',
            ),
        ],
        Row(
          children: [
            SmallButton(
              key: const ValueKey('backup-run-now'),
              label: _b.busy
                  ? t.t('settings.backup.running')
                  : t.t('settings.backup.runNow'),
              onTap: _b.busy
                  ? null
                  : () => _b.runNow(
                      (r) =>
                          '${t.t('settings.backup.success')}'
                          '${r['path'] == null ? '' : '：${r['path']}'}',
                      failedText: t.t('settings.backup.failed'),
                    ),
            ),
            if (_b.lastResultPath != null) ...[
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(
                key: const ValueKey('backup-reveal'),
                // 没有「在文件管理器里定位」命令的系统上 revealItemInDir 退化成
                // 复制路径（`platform.dart:89`），按钮文案跟着换 —— 说「在文件夹显示」
                // 却只复制了路径就是骗人（React `ScheduledBackupSection.tsx:212` 同理）。
                label: native.canRevealItemInDir()
                    ? t.t('settings.backup.reveal')
                    : t.t('settings.backup.copyPath'),
                onTap: () => native.revealItemInDir(_b.lastResultPath!),
              ),
            ],
          ],
        ),
        if (_b.error.isNotEmpty) ErrorNote(text: _b.error),
        if (_b.message.isNotEmpty)
          AutoToast(
            text: _b.message,
            onDone: () => setState(() => _b.message = ''),
          ),
      ],
    );
  }

  // ── 异源导入（cc-switch / sub2api 同形）────────────────

  Widget _foreignCard(
    I18nController t, {
    required ForeignImportController c,
    required String title,
    required String description,
    required String autoGroupLabel,
    required Widget header,
    bool showDims = false,
    String groupAssignHint = '',
    Widget Function(int index, Map<String, Object?> row)? rowExtra,
    required List<Map<String, Object?>> Function(List<Map<String, Object?>>)
    toPayload,
  }) {
    final theme = AidogTheme.of(context);
    final report = c.report;
    return SettingsCard(
      title: title,
      description: description,
      children: [
        header,
        const SizedBox(height: AidogSpace.ssm),
        // 这个开关是**批量**的：作用于本次导入的全部平台，不是当前这一条。
        // 不写这句，用户会以为它跟着某一行走（`CcSwitchImport.tsx:357-361`）。
        if (groupAssignHint.isNotEmpty) TileMetaLine(groupAssignHint),
        SwitchRow(
          label: autoGroupLabel,
          value: c.autoGroup,
          onChanged: c.setAutoGroup,
        ),
        // 已有分组的 chip 多选（`CcSwitchImport.tsx:365-391`）：
        // 只有那个开关的话，导进来的平台只能进固定的自动分组，
        // 想直接放进现成的分组还得回平台页一个个改。
        if (groupAssignHint.isNotEmpty && _groupOptions.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
            child: Wrap(
              spacing: AidogSpace.sxs,
              runSpacing: AidogSpace.sxs,
              children: [
                for (final g in _groupOptions)
                  SmallButton(
                    key: ValueKey('batch-join-group-${g.id}'),
                    label: g.name,
                    pill: true,
                    active: _batchJoinGroupIds.contains(g.id),
                    onTap: () => setState(
                      () => _batchJoinGroupIds.contains(g.id)
                          ? _batchJoinGroupIds.remove(g.id)
                          : _batchJoinGroupIds.add(g.id),
                    ),
                  ),
              ],
            ),
          ),
        // 三个导入维度（`CcSwitchImport.tsx:311-331`）：它们直接决定 payload
        // 里带什么字段，不是纯展示。平台类型那一维 React 锁定常开 ——
        // 不带协议和 endpoints 的话导进来就是空壳，关掉没有意义。
        if (showDims) ...[
          SwitchRow(
            label:
                '${t.t('importExport.ccswitch.dimPlatformType')}'
                '（${t.t('importExport.ccswitch.dimPlatformTypeHint')}）',
            value: true,
            onChanged: null,
          ),
          SwitchRow(
            key: const ValueKey('cc-dim-models'),
            label:
                '${t.t('importExport.ccswitch.dimModels')}'
                '（${t.t('importExport.ccswitch.dimModelsHint')}）',
            value: _dims.d2,
            onChanged: (v) => setState(() => _dims = _dims.copyWith(d2: v)),
          ),
          SwitchRow(
            key: const ValueKey('cc-dim-apikey'),
            label:
                '${t.t('importExport.ccswitch.dimApiKey')}'
                '（${t.t('importExport.ccswitch.dimApiKeyHint')}）',
            value: _dims.d4,
            onChanged: (v) => setState(() => _dims = _dims.copyWith(d4: v)),
          ),
        ],
        if (c.providers.isEmpty)
          CenteredNote(text: t.t('importExport.ccswitch.nothingSelected'))
        else ...[
          TileMetaLine(
            '${t.t('importExport.ccswitch.providerList')} '
            '${c.providers.length} ${t.t('importExport.ccswitch.providerCount')}',
          ),
          for (var i = 0; i < c.providers.length; i++)
            Builder(
              builder: (context) {
                final p = c.providers[i];
                final on = c.selected.contains(i);
                final maskedKey = _maskKey(
                  p['api_key'] ?? p['apiKey'] ?? p['detectedApiKey'],
                );
                return InkWell(
                  key: ValueKey('provider-$i'),
                  onTap: () => c.toggleSelected(i, !on),
                  // 选中的整行换底色 + 描边（`CcSwitchImport.tsx:524-528`）：
                  // 只有一个小勾的话，十几条里选了哪几条要逐行去找。
                  child: Container(
                    decoration: BoxDecoration(
                      color: on ? theme.c.accentWash : Colors.transparent,
                      border: Border.all(
                        color: on ? theme.c.accentEdge : Colors.transparent,
                      ),
                      borderRadius: BorderRadius.circular(AidogRadius.sm),
                    ),
                    margin: const EdgeInsets.only(bottom: 2),
                    padding: const EdgeInsets.symmetric(
                      vertical: 4,
                      horizontal: AidogSpace.sxs,
                    ),
                    child: Row(
                      children: [
                        Icon(
                          on ? Icons.check_box : Icons.check_box_outline_blank,
                          size: 14,
                          color: on ? theme.c.accentText : theme.c.fg3,
                        ),
                        const SizedBox(width: AidogSpace.sxs),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              // appType 大写小字（`CcSwitchImport.tsx:542-544`）：
                              // claude / codex 混在一张清单里，先认类型再看名字。
                              Text(
                                '${p['appType'] ?? ''}'.toUpperCase(),
                                style: AidogType.micro.copyWith(
                                  color: theme.c.fg3,
                                ),
                              ),
                              Text(
                                ltr('${p['name'] ?? p['id'] ?? i}'),
                                overflow: TextOverflow.ellipsis,
                                style: AidogType.micro.copyWith(
                                  color: theme.c.fg,
                                ),
                              ),
                              // 匹配结果读数（`CcSwitchImport.tsx:543-551`）：
                              // 匹配到哪个协议、凭什么匹配上的、最终用哪个
                              // base_url —— 导入前得先看得到，否则是盲导。
                              _MatchReadout(match: _matchOf(p)),
                            ],
                          ),
                        ),
                        if (rowExtra != null) rowExtra(i, p),
                        Text(
                          maskedKey.isEmpty
                              ? t.t('importExport.ccswitch.noKey')
                              : maskedKey,
                          style: AidogType.micro.copyWith(color: theme.c.fg3),
                        ),
                      ],
                    ),
                  ),
                );
              },
            ),
          const SizedBox(height: AidogSpace.ssm),
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: SmallButton(
              key: ValueKey('foreign-import-${c.source.autoGroupName}'),
              label: t.t('importExport.ccswitch.importBtn', {
                'n': c.selected.length,
              }),
              // 一项没选就点不动（`canImport`）。
              onTap: c.canImport
                  ? () async {
                      final chosen = [
                        for (var i = 0; i < c.providers.length; i++)
                          if (c.selected.contains(i)) c.providers[i],
                      ];
                      await c.runImport(toPayload);
                      if (!mounted || c.error.isNotEmpty) return;
                      await _joinChosenGroups(toPayload(chosen));
                      if (!mounted) return;
                      if (c.autoGroup) {
                        // 导入后建 / 取自动分组，再刷一次平台列表。
                        await _followUp.ensureAutoGroup(c.source.autoGroupName);
                        await _followUp.listPlatforms();
                        await _followUp.listGroupDetails();
                      }
                    }
                  : null,
            ),
          ),
        ],
        if (report != null)
          for (final e in report.entries)
            InfoRow(label: e.key, value: '${e.value}'),
        if (c.error.isNotEmpty) ErrorNote(text: c.error),
      ],
    );
  }
}

/// 卡片内的小节标题。`TileMeta` 自带间距，这里包一层只为少写一次 Padding。
class TileMetaLine extends StatelessWidget {
  const TileMetaLine(this.text, {super.key, this.color});

  final String text;

  /// 缺省 fg3；要喊人的标题（如冲突计数）传告警色。
  final Color? color;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: AidogSpace.ssm, bottom: 2),
    child: Text(
      text,
      style: AidogType.micro.copyWith(
        color: color ?? AidogTheme.of(context).c.fg3,
      ),
    ),
  );
}

/// 导入落区（`ImportExportTab.tsx:487-503` 的 `DropZone` + 外层拖放容器）。
///
/// 拖放走 `desktop_drop`：选它是因为 Flutter 自带的 `Draggable` / `DragTarget`
/// 只认应用内部发起的拖拽，**收不到从 Finder / 资源管理器拖进来的系统文件**。
/// 该包 Apache-2.0，与本仓库兼容；macOS / Windows / Linux 三端都有实现。
///
/// 与 React 的一处**框架不可达偏离**：React 在 dragenter 时会看拖的是不是
/// `.aidogx`，不是就不高亮（`ImportExportTab.tsx:288`）。desktop_drop 0.8.4 的
/// `DropEventDetails` 只带坐标不带文件列表（`drop_target.dart:50-58`，包源码
/// 已核），进入阶段拿不到路径，所以这里一律高亮，到 drop 才判扩展名。
class _DropZone extends StatelessWidget {
  const _DropZone({
    required this.active,
    required this.title,
    required this.hint,
    required this.onTap,
    required this.onDragEntered,
    required this.onDragExited,
    required this.onDropped,
  });

  final bool active;
  final String title;
  final String hint;
  final VoidCallback? onTap;
  final VoidCallback onDragEntered;
  final VoidCallback onDragExited;
  final void Function(List<String> paths) onDropped;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return DropTarget(
      onDragEntered: (_) => onDragEntered(),
      onDragExited: (_) => onDragExited(),
      onDragDone: (d) => onDropped([for (final f in d.files) f.path]),
      child: InkWell(
        key: const ValueKey('import-dropzone'),
        onTap: onTap,
        borderRadius: BorderRadius.circular(AidogRadius.md),
        child: Container(
          width: double.infinity,
          padding: const EdgeInsets.symmetric(
            horizontal: AidogSpace.smd,
            vertical: AidogSpace.slg,
          ),
          decoration: BoxDecoration(
            color: active ? theme.c.surface2 : null,
            border: Border.all(
              color: active ? theme.c.accentText : theme.c.line,
            ),
            borderRadius: BorderRadius.circular(AidogRadius.md),
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.file_upload_outlined,
                size: 20,
                color: active ? theme.c.accentText : theme.c.fg3,
              ),
              const SizedBox(height: AidogSpace.sxs),
              Text(
                title,
                textAlign: TextAlign.center,
                style: AidogType.body.copyWith(
                  color: theme.c.fg,
                  fontWeight: FontWeight.w600,
                ),
              ),
              const SizedBox(height: 2),
              Text(
                hint,
                textAlign: TextAlign.center,
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// provider 行里的匹配读数：协议名 + 命中方式徽标 + 实际 base_url。
/// 对齐 `CcSwitchImport.tsx:50-58,543-551`：三种命中方式三种颜色 ——
/// 关键词命中（accent）/ host 命中（ok）/ 回退（peak，提醒这条是猜的）。
class _MatchReadout extends StatelessWidget {
  const _MatchReadout({required this.match});

  final CcMatchResult match;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final c = AidogTheme.of(context).c;
    final (label, color) = switch (match.matchedBy) {
      CcMatchedBy.presetKeyword => (
        t.t('importExport.ccswitch.matched'),
        c.accentText,
      ),
      CcMatchedBy.baseUrlHost => (t.t('importExport.ccswitch.hostMatch'), c.ok),
      CcMatchedBy.protocolFallback => (
        t.t('importExport.ccswitch.fallback'),
        c.peak,
      ),
    };
    return Padding(
      padding: const EdgeInsets.only(top: 2),
      child: Wrap(
        spacing: AidogSpace.sxs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Text(
            match.matchedLabel ?? match.protocol,
            style: AidogType.micro.copyWith(color: c.fg2),
          ),
          MiniBadge(text: label, color: color),
          // `Wrap` 里不能用 `Flexible`（它只认 Flex 的 ParentData）——
          // 长 URL 靠 ConstrainedBox 限宽 + 省略号收住。
          if (match.baseUrl.isNotEmpty)
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 260),
              child: Text(
                ltr(match.baseUrl),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.micro.copyWith(color: c.fg3),
              ),
            ),
        ],
      ),
    );
  }
}

/// scope → 图标（`ImportExport/meta.ts:16-26` 的 `icon` 字段逐条对应）。
///
/// React 那边是 `SectionIcon` 的名字，这边取 Material 里语义最近的一枚；
/// 未登记的 scope 落到文件夹图标（与 React 的 `SCOPE_ICON[...] ?? "folder"` 同）。
IconData scopeIcon(String scope) => switch (scope) {
  'platform' => Icons.lan_outlined,
  'group' => Icons.workspaces_outlined,
  'group_platform' => Icons.account_tree_outlined,
  'setting' => Icons.bolt_outlined,
  'codex' => Icons.description_outlined,
  'claude_code' => Icons.memory_outlined,
  'model_price' => Icons.sell_outlined,
  'mcp' => Icons.extension_outlined,
  'middleware' => Icons.rule_outlined,
  'skills' => Icons.auto_awesome_outlined,
  _ => Icons.folder_outlined,
};
