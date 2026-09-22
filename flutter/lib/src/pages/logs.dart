/// 请求日志两页的界面（票 I07）：`LogsPage` ← `src/pages/Logs.tsx`、
/// `RequestLogPage` ← `src/pages/RequestLog.tsx`。
///
/// 两页长得几乎一样（筛选条 + 表格 + 分页 + 右侧详情抽屉），差别只在筛选维度与
/// 分页形态，所以表格、行、详情面板是同一套 widget，由页面各自喂数据。
///
/// 状态全在 [LogsController] / [RequestLogController]（`logs_logic.dart`），
/// 这里只负责画 + 把事件转成控制器上的一次调用。色值一律 `AidogTheme.of(context).c.*`。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../utils/formatters.dart';
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'filter_dropdown.dart';
import 'invoke.dart';
import 'logs_logic.dart';
import 'models.dart';
import 'ui_bits.dart';

// ── Logs 主页 ──────────────────────────────────────────────────────

class LogsPage extends StatefulWidget {
  const LogsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.logUpdates,
    this.copyText = native.writeText,
    this.initialPlatformId,
    this.initialGroupKey,
  });

  final InvokeFn invoke;

  /// 「有新请求日志」流；缺省是内核事件的 500ms 防抖流。
  /// **流只是增量提示**：页面 mount 时必定自己整查一遍，不等这个流。
  final Stream<void>? logUpdates;
  final Future<void> Function(String) copyText;

  /// 从别的页跳过来时带的初始筛选（平台卡「查看日志」、分组卡「查看日志」）。
  final int? initialPlatformId;
  final String? initialGroupKey;

  @override
  State<LogsPage> createState() => _LogsPageState();
}

class _LogsPageState extends State<LogsPage> {
  late final LogsController _c;
  StreamSubscription<void>? _sub;
  Timer? _copyTimer;

  @override
  void initState() {
    super.initState();
    _c = LogsController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
      initial: LogsFilterState(
        platform: widget.initialPlatformId?.toString() ?? '',
        group: widget.initialGroupKey ?? '',
      ),
    );
    _c.init();
    _sub = (widget.logUpdates ?? debounceStream(kernelProxyLogUpdated())).listen((
      _,
    ) {
      _c.refreshFromEvent();
    });
  }

  @override
  void dispose() {
    _sub?.cancel();
    _copyTimer?.cancel();
    super.dispose();
  }

  void _flashCopied() {
    _copyTimer?.cancel();
    _copyTimer = Timer(const Duration(seconds: 2), () {
      if (mounted) _c.clearCopied();
    });
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        PageHead(
          title: t.t('page.logs'),
          subtitle: _c.logs.isEmpty
              ? t.t('logs.empty')
              : '${_c.logs.length} ${t.t('logs.total')}',
          trailing: Wrap(
            spacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              if (_c.cleanupMessage.isNotEmpty)
                Text(
                  _c.cleanupMessage,
                  style: AidogType.micro.copyWith(color: theme.c.ok),
                ),
              SmallButton(
                label: t.t('logs.cleanupExpired'),
                onTap: () => _c.cleanupExpired(
                  doneText: t.t('logs.cleanupExpiredDone'),
                ),
              ),
              SmallButton(
                label: t.t('logs.clear'),
                danger: true,
                onTap: () => setState(() => _c.showClearConfirm = true),
              ),
            ],
          ),
        ),
        _LogsFilterBar(controller: _c),
        const SizedBox(height: AidogSpace.smd),
        if (_c.loading)
          CenteredNote(text: t.t('status.loading'))
        else if (_c.logs.isEmpty)
          CenteredNote(text: t.t('logs.empty'))
        else ...[
          _LogTable(
            rows: _c.logs,
            platformName: _c.platformName,
            groupName: _c.groupName,
            onOpen: _c.openDetail,
            onCopy: (id) {
              _c.copyRow(id, widget.copyText);
              _flashCopied();
            },
          ),
          const SizedBox(height: AidogSpace.smd),
          // Logs 主页没有精确总数（后端只回 has_more），所以分页只有上一页 / 下一页。
          _Pager(
            currentPage: _c.currentPage,
            hasMore: _c.hasMore,
            pageSize: _c.pageSize,
            onPage: _c.goToPage,
            onPageSize: _c.setPageSize,
          ),
        ],
        if (_c.showClearConfirm)
          ConfirmCard(
            title: t.t('logs.clearConfirmTitle'),
            body: t.t('logs.clearConfirm'),
            confirmLabel: t.t('logs.clear'),
            onCancel: () => setState(() => _c.showClearConfirm = false),
            onConfirm: _c.confirmClear,
          ),
        if (_c.detail != null)
          _DetailPanel(
            detail: _c.detail!,
            copied: _c.copied,
            onClose: _c.closeDetail,
            onCopyAll: () {
              _c.copyDetail(_c.detail!, widget.copyText);
              _flashCopied();
            },
          ),
      ],
    );
  }
}

class _LogsFilterBar extends StatelessWidget {
  const _LogsFilterBar({required this.controller});

  final LogsController controller;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final f = controller.filters;
    return Tile(
      child: Wrap(
        spacing: AidogSpace.ssm,
        runSpacing: AidogSpace.ssm,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          FilterDropdown(
            width: 160,
            value: f.platform,
            onChanged: (v) => controller.setFilters(f.copyWith(platform: v)),
            allLabel: t.t('logs.filterPlatform'),
            searchPlaceholder: t.t('logs.filterPlatform'),
            emptyLabel: t.t('logs.empty'),
            options: [
              for (final p in controller.platforms)
                FilterOption(value: '${p.id}', label: p.name),
            ],
          ),
          FilterDropdown(
            width: 160,
            value: f.group,
            onChanged: (v) => controller.setFilters(f.copyWith(group: v)),
            allLabel: t.t('logs.filterGroup'),
            searchPlaceholder: t.t('logs.filterGroup'),
            emptyLabel: t.t('logs.empty'),
            options: [
              for (final g in controller.groups)
                FilterOption(value: g.group.groupKey, label: g.group.name),
            ],
          ),
          FilterDropdown(
            width: 130,
            value: f.status,
            onChanged: (v) => controller.setFilters(f.copyWith(status: v)),
            allLabel: t.t('logs.filterStatus'),
            searchPlaceholder: t.t('logs.filterStatus'),
            emptyLabel: t.t('logs.empty'),
            options: [
              FilterOption(value: 'success', label: t.t('logs.statusSuccess')),
              FilterOption(value: 'error', label: t.t('logs.statusError')),
            ],
          ),
          FilterDropdown(
            width: 110,
            value: f.time == 'all' ? '' : f.time,
            onChanged: (v) =>
                controller.setFilters(f.copyWith(time: v.isEmpty ? 'all' : v)),
            allLabel: t.t('logs.filterTime'),
            searchPlaceholder: t.t('logs.filterTime'),
            emptyLabel: t.t('logs.empty'),
            options: [
              for (final p in kTimePresets)
                if (p != 'all') FilterOption(value: p, label: p),
            ],
          ),
          FilterDropdown(
            width: 180,
            value: f.modelText,
            onChanged: (v) => controller.setFilters(f.copyWith(modelText: v)),
            allLabel: t.t('logs.filterModel'),
            searchPlaceholder: t.t('logs.filterModel'),
            emptyLabel: t.t('logs.empty'),
            options: [
              for (final m in controller.modelOptions)
                FilterOption(value: m, label: m),
            ],
          ),
          if (f.hasFilter)
            SmallButton(
              label: t.t('logs.clearFilter'),
              onTap: controller.clearFilter,
            ),
        ],
      ),
    );
  }
}

// ── RequestLog 页 ─────────────────────────────────────────────────

class RequestLogPage extends StatefulWidget {
  const RequestLogPage({
    super.key,
    this.invoke = kernelInvoke,
    this.logUpdates,
    this.copyText = native.writeText,
  });

  final InvokeFn invoke;
  final Stream<void>? logUpdates;
  final Future<void> Function(String) copyText;

  @override
  State<RequestLogPage> createState() => _RequestLogPageState();
}

class _RequestLogPageState extends State<RequestLogPage> {
  late final RequestLogController _c;
  StreamSubscription<void>? _sub;
  Timer? _copyTimer;

  @override
  void initState() {
    super.initState();
    _c = RequestLogController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    _c.init();
    _sub = (widget.logUpdates ?? debounceStream(kernelProxyLogUpdated())).listen((
      _,
    ) {
      _c.refreshFromEvent();
    });
  }

  @override
  void dispose() {
    _sub?.cancel();
    _copyTimer?.cancel();
    super.dispose();
  }

  void _flashCopied() {
    _copyTimer?.cancel();
    _copyTimer = Timer(const Duration(seconds: 2), () {
      if (mounted) _c.clearCopied();
    });
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        PageHead(
          title: t.t('page.requestLog'),
          subtitle: _c.total > 0
              ? '${_c.total} ${t.t('logs.total')}'
              : t.t('requestLog.empty'),
          trailing: SmallButton(
            label: t.t('action.refresh'),
            onTap: _c.loading ? null : () => _c.load(),
          ),
        ),
        Tile(
          child: Wrap(
            spacing: AidogSpace.ssm,
            runSpacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              FilterDropdown(
                width: 130,
                value: _c.filterType == 'all' ? '' : _c.filterType,
                onChanged: (v) => _c.setFilterType(v.isEmpty ? 'all' : v),
                allLabel: t.t('requestLog.filterType'),
                searchPlaceholder: t.t('requestLog.filterType'),
                emptyLabel: t.t('requestLog.empty'),
                options: [
                  FilterOption(value: 'test', label: t.t('requestLog.typeTest')),
                  FilterOption(
                    value: 'quota',
                    label: t.t('requestLog.typeQuota'),
                  ),
                ],
              ),
              FilterDropdown(
                width: 160,
                value: _c.filterPlatform,
                onChanged: _c.setFilterPlatform,
                allLabel: t.t('logs.filterPlatform'),
                searchPlaceholder: t.t('logs.filterPlatform'),
                emptyLabel: t.t('requestLog.empty'),
                options: [
                  for (final p in _c.platforms)
                    FilterOption(value: '${p.id}', label: p.name),
                ],
              ),
              FilterDropdown(
                width: 130,
                value: _c.filterStatus,
                onChanged: _c.setFilterStatus,
                allLabel: t.t('logs.filterStatus'),
                searchPlaceholder: t.t('logs.filterStatus'),
                emptyLabel: t.t('requestLog.empty'),
                options: [
                  FilterOption(
                    value: 'success',
                    label: t.t('logs.statusSuccess'),
                  ),
                  FilterOption(value: 'error', label: t.t('logs.statusError')),
                ],
              ),
              FilterDropdown(
                width: 110,
                value: _c.filterTime == 'all' ? '' : _c.filterTime,
                onChanged: (v) => _c.setFilterTime(v.isEmpty ? 'all' : v),
                allLabel: t.t('logs.filterTime'),
                searchPlaceholder: t.t('logs.filterTime'),
                emptyLabel: t.t('requestLog.empty'),
                options: [
                  for (final p in kTimePresets)
                    if (p != 'all') FilterOption(value: p, label: p),
                ],
              ),
              if (_c.hasFilter)
                SmallButton(
                  label: t.t('logs.clearFilter'),
                  onTap: _c.clearFilter,
                ),
            ],
          ),
        ),
        const SizedBox(height: AidogSpace.smd),
        if (_c.loading)
          CenteredNote(text: t.t('status.loading'))
        else if (_c.logs.isEmpty)
          CenteredNote(text: t.t('requestLog.empty'))
        else ...[
          _LogTable(
            rows: _c.logs,
            platformName: _c.platformName,
            groupName: _c.groupName,
            onOpen: _c.openDetail,
            onCopy: (id) {
              _c.copyRow(id, widget.copyText);
              _flashCopied();
            },
          ),
          const SizedBox(height: AidogSpace.smd),
          // 本页有精确 total，所以分页是「第 N / 共 M 页」。
          _Pager(
            currentPage: _c.currentPage,
            hasMore: _c.currentPage < _c.totalPages,
            totalPages: _c.totalPages,
            pageSize: _c.pageSize,
            onPage: _c.goToPage,
            onPageSize: _c.setPageSize,
          ),
        ],
        if (_c.detail != null)
          _DetailPanel(
            detail: _c.detail!,
            copied: _c.copied,
            onClose: _c.closeDetail,
            onCopyAll: () {
              _c.copyDetail(_c.detail!, widget.copyText);
              _flashCopied();
            },
          ),
      ],
    );
  }
}

// ── 两页共用的零件 ────────────────────────────────────────────────

class _LogTable extends StatelessWidget {
  const _LogTable({
    required this.rows,
    required this.platformName,
    required this.groupName,
    required this.onOpen,
    required this.onCopy,
  });

  final List<ProxyLogSummary> rows;
  final String Function(int) platformName;
  final String Function(String) groupName;
  final void Function(String id) onOpen;
  final void Function(String id) onCopy;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Tile(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
            child: Row(
              children: [
                _Cell(t.t('logs.time'), flex: 3, header: true),
                _Cell(t.t('logs.group'), flex: 2, header: true),
                _Cell(t.t('logs.platform'), flex: 2, header: true),
                _Cell(t.t('logs.actualModel'), flex: 3, header: true),
                _Cell(t.t('logs.status'), flex: 1, header: true),
                _Cell(t.t('logs.duration'), flex: 2, header: true),
                _Cell(t.t('logs.inputTokens'), flex: 2, header: true),
                _Cell(t.t('logs.outputTokens'), flex: 2, header: true),
                const SizedBox(width: 28),
              ],
            ),
          ),
          for (final log in rows)
            InkWell(
              onTap: () => onOpen(log.id),
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 5),
                child: Row(
                  children: [
                    _Cell(formatDateTime(log.createdAt), flex: 3),
                    _Cell(groupName(log.groupKey), flex: 2),
                    _Cell(platformName(log.platformId), flex: 2),
                    _Cell(
                      log.actualModel.isEmpty ? '-' : log.actualModel,
                      flex: 3,
                    ),
                    Expanded(
                      flex: 1,
                      child: Text(
                        '${log.statusCode}',
                        style: AidogType.micro.copyWith(
                          // 2xx 绿、其余红；0 = 还没有终态（流式在跑）。
                          color: log.statusCode == 0
                              ? theme.c.fg3
                              : (log.statusCode >= 200 && log.statusCode < 300)
                              ? theme.c.ok
                              : theme.c.bad,
                        ),
                      ),
                    ),
                    _Cell(formatDurationMs(log.durationMs.toDouble()), flex: 2),
                    _Cell(formatNumber(log.inputTokens), flex: 2),
                    _Cell(formatNumber(log.outputTokens), flex: 2),
                    SizedBox(
                      width: 28,
                      child: IconButton(
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints(),
                        iconSize: 14,
                        tooltip: t.t('logs.copy'),
                        color: theme.c.fg3,
                        icon: const Icon(Icons.copy_outlined),
                        onPressed: () => onCopy(log.id),
                      ),
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _Cell extends StatelessWidget {
  const _Cell(this.text, {required this.flex, this.header = false});

  final String text;
  final int flex;
  final bool header;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Expanded(
      flex: flex,
      child: Text(
        header ? text.toUpperCase() : text,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: AidogType.micro.copyWith(
          color: header ? theme.c.fg3 : theme.c.fg2,
        ),
      ),
    );
  }
}

class _Pager extends StatelessWidget {
  const _Pager({
    required this.currentPage,
    required this.hasMore,
    required this.pageSize,
    required this.onPage,
    required this.onPageSize,
    this.totalPages,
  });

  final int currentPage;
  final bool hasMore;
  final int? totalPages;
  final int pageSize;
  final void Function(int) onPage;
  final void Function(int) onPageSize;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Row(
      children: [
        SmallButton(
          label: t.t('action.prev'),
          onTap: currentPage > 1 ? () => onPage(currentPage - 1) : null,
        ),
        const SizedBox(width: AidogSpace.ssm),
        Text(
          totalPages == null ? '$currentPage' : '$currentPage / $totalPages',
          style: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
        const SizedBox(width: AidogSpace.ssm),
        SmallButton(
          label: t.t('action.next'),
          onTap: hasMore ? () => onPage(currentPage + 1) : null,
        ),
        const Spacer(),
        for (final size in const [20, 50, 100])
          Padding(
            padding: const EdgeInsets.only(left: 4),
            child: SmallButton(
              label: '$size',
              active: size == pageSize,
              onTap: () => onPageSize(size),
            ),
          ),
      ],
    );
  }
}

/// 详情抽屉。React 那边是 Radix Sheet（Portal 到 body），这里是页面内的一张大格子 ——
/// Flutter 没有「祖先 transform 让 fixed 退化」那个问题（那是 CSS 的坑，项目
/// CLAUDE.md 里记的 modal 居中铁律只对 Web 侧成立），所以不必绕 Portal。
class _DetailPanel extends StatelessWidget {
  const _DetailPanel({
    required this.detail,
    required this.copied,
    required this.onClose,
    required this.onCopyAll,
  });

  final ProxyLogDetail detail;
  final bool copied;
  final VoidCallback onClose;
  final VoidCallback onCopyAll;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    // React 用的是 Radix `Sheet`（`Logs/DetailPanel.tsx:37`，右侧抽屉，width 900）。
    // 这里同为 Portal 浮层但居中，不做侧滑抽屉：Flutter 没有等价原语，
    // 自造一套抽屉动画换来的只是入场方向不同。
    return AidogModal(
      maxWidth: 900,
      onBarrierTap: onClose,
      child: Tile(
        title: t.t('logs.detail'),
        meta: detail.id,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                SmallButton(
                  label: copied ? t.t('logs.copied') : t.t('logs.copyAll'),
                  onTap: onCopyAll,
                ),
                const Spacer(),
                SmallButton(label: t.t('action.close'), onTap: onClose),
              ],
            ),
            const SizedBox(height: AidogSpace.ssm),
            _kv(theme, t.t('logs.group'), detail.groupKey),
            _kv(theme, t.t('logs.model'), detail.model),
            _kv(theme, t.t('logs.actualModel'), detail.actualModel),
            _kv(theme, t.t('logs.sourceProtocol'), detail.sourceProtocol),
            _kv(theme, t.t('logs.targetProtocol'), detail.targetProtocol),
            _kv(theme, t.t('logs.status'), '${detail.statusCode}'),
            _kv(
              theme,
              t.t('logs.duration'),
              formatDurationMs(detail.durationMs.toDouble()),
            ),
            _kv(theme, t.t('logs.inputTokens'), formatNumber(detail.inputTokens)),
            _kv(
              theme,
              t.t('logs.outputTokens'),
              formatNumber(detail.outputTokens),
            ),
            _kv(theme, t.t('logs.cacheTokens'), formatNumber(detail.cacheTokens)),
            const SizedBox(height: AidogSpace.ssm),
            _section(theme, t.t('logs.requestBody'), detail.requestBody),
            _section(
              theme,
              t.t('logs.responseBody'),
              detail.userResponseBody.isNotEmpty
                  ? detail.userResponseBody
                  : detail.responseBody,
            ),
          ],
        ),
      ),
    );
  }

  Widget _kv(AidogTheme theme, String k, String v) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 2),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 110,
          child: Text(
            k,
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
        ),
        Expanded(
          child: Text(
            v.isEmpty ? '-' : v,
            style: AidogType.micro.copyWith(color: theme.c.fg2),
          ),
        ),
      ],
    ),
  );

  Widget _section(AidogTheme theme, String title, String body) => Padding(
    padding: const EdgeInsets.only(top: AidogSpace.ssm),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        TileMeta(title),
        const SizedBox(height: 4),
        Container(
          width: double.infinity,
          padding: const EdgeInsets.all(AidogSpace.ssm),
          decoration: BoxDecoration(
            color: theme.c.surface2,
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: SelectableText(
            body.isEmpty ? '(streaming, not captured)' : prettyJsonOrRaw(body),
            maxLines: 14,
            style: AidogType.micro.copyWith(color: theme.c.fg2),
          ),
        ),
      ],
    ),
  );
}


