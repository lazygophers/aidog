/// 导入导出页（`settings/importexport`）的 widget 层 —— 对齐
/// `src/components/settings/ImportExport/ImportExportTab.tsx` +
/// `ScheduledBackupSection.tsx` + `CcSwitchImport.tsx` + `Sub2ApiImport.tsx`。
///
/// 这一页全是破坏性操作，禁用条件一条都不能少 —— 它们全在
/// [ImportExportController] 的派生态里（`canExport` / `canApplyImport` /
/// `allConflictsDecided`），本文件不重判。
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

  /// sub2api 的粘贴文本。
  String _pasteText = '';

  /// 应用导入前的确认卡。
  bool _confirmApply = false;

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
          title: t.t('importExport.sub2api.title'),
          description: t.t('importExport.sub2api.desc'),
          autoGroupLabel: t.t('importExport.sub2api.autoGroup'),
          header: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              TextRow(
                key: const ValueKey('sub2api-paste'),
                label: t.t('importExport.sub2api.title'),
                hint: t.t('importExport.sub2api.pastePlaceholder'),
                value: _pasteText,
                maxLines: 4,
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
        if (_confirmApply)
          ConfirmCard(
            title: t.t('importExport.applyBtn'),
            body: t.t('importExport.importDesc'),
            confirmLabel: t.t('importExport.applyN', {'n': _c.selected.length}),
            busy: _c.busy,
            onCancel: () => setState(() => _confirmApply = false),
            onConfirm: () {
              setState(() => _confirmApply = false);
              final p = _importPath;
              if (p != null) _c.applyImport(p);
            },
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
              label: tOr(t, 'importExport.scope.$s', s),
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
      Row(
        children: [
          SmallButton(
            key: const ValueKey('export-preview'),
            label: _c.busy
                ? t.t('importExport.loadingPreview')
                : t.t('importExport.previewItems'),
            // 一个范围都没勾就不许预览（React 的 `error.noScope`）。
            onTap: _c.busy || _c.scopes.isEmpty ? null : _c.exportPreview,
          ),
          const SizedBox(width: AidogSpace.ssm),
          SmallButton(
            key: const ValueKey('export-run'),
            // 三态照抄 `ImportExportTab.tsx:467-473`：跑着的时候说「导出中」，
            // 预览出来之后按钮上直接带上会导几项 —— 光写「导出」看不出导什么。
            label: _c.busy
                ? t.t('importExport.exporting')
                : _c.preview != null
                ? t.t('importExport.exportN', {'n': _c.selected.length})
                : t.t('importExport.exportBtn'),
            // 没勾任何条目不许导出 —— 会写出一个空备份覆盖掉用户以为还在的文件。
            onTap: _c.canExport
                ? () async {
                    final p = await widget.pickPath(
                      save: true,
                      suggested: 'aidog-backup.aidogx',
                    );
                    if (p == null || !mounted) return;
                    await _c.exportToFile(
                      p,
                      t.t('importExport.exportDone', {'path': p}),
                    );
                  }
                : null,
          ),
        ],
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
              label: _c.busy
                  ? t.t('importExport.applying')
                  : t.t('importExport.applyBtn'),
              // 冲突没定完 / 一项没选就点不动（`canApplyImport`）。
              onTap: _c.canApplyImport && _importPath != null
                  ? () => setState(() => _confirmApply = true)
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
          TileMetaLine(
            t.t('importExport.conflicts', {'n': _c.conflictKeys.length}),
          ),
          for (final k in _c.conflictKeys)
            Padding(
              key: ValueKey('conflict-$k'),
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      ltr(k),
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(
                        color: AidogTheme.of(context).c.fg,
                      ),
                    ),
                  ),
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
                  if (_c.decisions[k]?.kind == ConflictDecisionKind.keepBoth)
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
            ),
          Row(
            children: [
              SmallButton(
                label: t.t('importExport.bulkSkip'),
                onTap: () {
                  for (final k in _c.conflictKeys) {
                    _c.decide(k, ConflictDecisionKind.keepLocal);
                  }
                },
              ),
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(
                label: t.t('importExport.bulkOverwrite'),
                onTap: () {
                  for (final k in _c.conflictKeys) {
                    _c.decide(k, ConflictDecisionKind.useIncoming);
                  }
                },
              ),
            ],
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
              '${tOr(t, 'importExport.scope.${e.key}', e.key)}: ${e.value}',
          ]),
        if (skipped.isNotEmpty)
          section(t.t('importExport.skipped'), theme.c.fg3, [
            for (final e in skipped.entries)
              '${tOr(t, 'importExport.scope.${e.key}', e.key)}: ${e.value}',
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
                        '${tOr(t, 'importExport.scope.${e.key}', '${e.key}')} '
                        '${e.value}',
                    color: theme.c.fg3,
                  ),
              ],
            ),
          ),
      ],
    );
  }

  /// 预览里的可勾选条目（导出与导入共用同一份 `items`）。
  Widget _itemPicker(I18nController t) {
    final items = (_c.preview!['items'] as List? ?? const [])
        .whereType<Map>()
        .toList();
    if (items.isEmpty) {
      return CenteredNote(text: t.t('importExport.exportEmpty'));
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
              onTap: () {
                for (final e in items) {
                  _c.toggleSelected('${e['scope']} ${e['key']}', true);
                }
              },
            ),
            const SizedBox(width: AidogSpace.ssm),
            SmallButton(
              label: t.t('importExport.deselectAll'),
              onTap: () {
                for (final e in items) {
                  _c.toggleSelected('${e['scope']} ${e['key']}', false);
                }
              },
            ),
          ],
        ),
        ConstrainedBox(
          constraints: const BoxConstraints(maxHeight: 260),
          child: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final e in items)
                  Builder(
                    builder: (context) {
                      final k = '${e['scope']} ${e['key']}';
                      final on = _c.selected.contains(k);
                      return InkWell(
                        key: ValueKey('item-$k'),
                        onTap: () => _c.toggleSelected(k, !on),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(vertical: 2),
                          child: Row(
                            children: [
                              Icon(
                                on
                                    ? Icons.check_box
                                    : Icons.check_box_outline_blank,
                                size: 14,
                                color: on
                                    ? AidogTheme.of(context).c.accent
                                    : AidogTheme.of(context).c.fg3,
                              ),
                              const SizedBox(width: AidogSpace.sxs),
                              Expanded(
                                child: Text(
                                  // 后端给了人话标签（平台名 / 分组名 / 文件名），
                                  // 只画 `scope key` 的话用户认不出这条是什么。
                                  ltr('${e['label'] ?? ''}'.isEmpty
                                      ? k
                                      : '${e['label']}'),
                                  overflow: TextOverflow.ellipsis,
                                  style: AidogType.micro.copyWith(
                                    color: AidogTheme.of(context).c.fg2,
                                  ),
                                ),
                              ),
                              // 冲突徽标（`ItemSelector.tsx:160-162`）：本地已有同名
                              // 条目，导入时要在上面的冲突区逐条定夺。
                              if (e['conflict'] == true) ...[
                                const SizedBox(width: AidogSpace.sxs),
                                MiniBadge(
                                  text: t.t('importExport.conflictTag'),
                                  color: AidogTheme.of(context).c.peak,
                                ),
                              ],
                            ],
                          ),
                        ),
                      );
                    },
                  ),
              ],
            ),
          ),
        ),
      ],
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
  }) {
    final theme = AidogTheme.of(context);
    final report = c.report;
    return SettingsCard(
      title: title,
      description: description,
      children: [
        header,
        const SizedBox(height: AidogSpace.ssm),
        SwitchRow(
          label: autoGroupLabel,
          value: c.autoGroup,
          onChanged: c.setAutoGroup,
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
                return InkWell(
                  key: ValueKey('provider-$i'),
                  onTap: () => c.toggleSelected(i, !on),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(vertical: 2),
                    child: Row(
                      children: [
                        Icon(
                          on ? Icons.check_box : Icons.check_box_outline_blank,
                          size: 14,
                          color: on ? theme.c.accent : theme.c.fg3,
                        ),
                        const SizedBox(width: AidogSpace.sxs),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            mainAxisSize: MainAxisSize.min,
                            children: [
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
                        Text(
                          (p['api_key'] ??
                                      p['apiKey'] ??
                                      p['detectedApiKey']) ==
                                  null
                              ? t.t('importExport.ccswitch.noKey')
                              : t.t('importExport.ccswitch.dimApiKey'),
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
                      await c.runImport(_toPlatformPayload);
                      if (!mounted || c.error.isNotEmpty) return;
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
  const TileMetaLine(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: AidogSpace.ssm, bottom: 2),
    child: Text(
      text,
      style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg3),
    ),
  );
}

/// 导入落区（`ImportExportTab.tsx:487-503` 的 `DropZone` + 外层拖放容器）。
///
/// 拖放走 `desktop_drop`：选它是因为 Flutter 自带的 `Draggable` / `DragTarget`
/// 只认应用内部发起的拖拽，**收不到从 Finder / 资源管理器拖进来的系统文件**。
/// 该包 Apache-2.0，与本仓库兼容；macOS / Windows / Linux 三端都有实现。
///
/// 与 React 的一处**有意偏离**：React 在 dragenter 时会看拖的是不是 `.aidogx`，
/// 不是就不高亮（`ImportExportTab.tsx:288`）。`desktop_drop` 的
/// `DropEventDetails` 只带坐标不带文件列表（`drop_target.dart:49`），
/// 进入阶段拿不到路径，所以这里一律高亮，到 drop 才判扩展名。
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
        c.accent,
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
