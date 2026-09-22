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
import 'platform_card_bits.dart' show MiniBadge;
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
    _sub = (widget.logUpdates ?? debounceStream(kernelProxyLogUpdated()))
        .listen((_) {
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
                onTap: () =>
                    _c.cleanupExpired(doneText: t.t('logs.cleanupExpiredDone')),
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
            groupName: _c.groupName,
            copied: _c.copied,
            onClose: _c.closeDetail,
            onCopyAll: () {
              _c.copyDetail(_c.detail!, widget.copyText);
              _flashCopied();
            },
            onCopy: (text) {
              widget.copyText(text);
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
            searchPlaceholder: t.t('stats.searchPlatform'),
            emptyLabel: t.t('stats.noMatch'),
            options: [
              for (final p in controller.platforms)
                FilterOption(
                  value: '${p.id}',
                  label: p.name,
                  searchTerms:
                      controller.protocolTerms[p.platformType] ?? const [],
                ),
              // 隧道请求的 host 没命中任何平台 → `platform_id = 0`
              //（`ListView.tsx:96`）。没有这一项就筛不出这批行。
              FilterOption(value: '0', label: t.t('logs.noPlatform')),
            ],
          ),
          FilterDropdown(
            width: 160,
            value: f.group,
            onChanged: (v) => controller.setFilters(f.copyWith(group: v)),
            allLabel: t.t('logs.filterGroup'),
            searchPlaceholder: t.t('stats.searchGroup'),
            emptyLabel: t.t('stats.noMatch'),
            options: [
              for (final g in controller.groups)
                FilterOption(value: g.group.groupKey, label: g.group.name),
              // 隧道请求没有 apikey → `group_key = ''`，用哨兵值表示
              //（`ListView.tsx:110`，空串会被下拉当成「全部」）。
              FilterOption(value: kNoGroupSentinel, label: t.t('logs.noGroup')),
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
          // 中间件观察模式命中（票 04）：只看「规则命中但放行」的请求。
          // 筛选逻辑早就在 `LogsFilterState.observed`，之前没有入口，点不到。
          FilterDropdown(
            width: 150,
            value: f.observed,
            onChanged: (v) => controller.setFilters(f.copyWith(observed: v)),
            allLabel: t.t('logs.filterMiddleware'),
            searchPlaceholder: t.t('logs.filterMiddleware'),
            emptyLabel: t.t('stats.noMatch'),
            options: [
              FilterOption(
                value: 'observed',
                label: t.t('logs.observedOnly'),
              ),
            ],
          ),
          // 模型名按「实际发给上游的」还是「客户端原始请求的」匹配
          //（`ListView.tsx:148-161`）。两者在有模型改写时不是一回事。
          SmallButton(
            label: t.t('logs.actualModel'),
            active: f.modelType == 'actual',
            ghost: f.modelType != 'actual',
            onTap: () =>
                controller.setFilters(f.copyWith(modelType: 'actual')),
          ),
          SmallButton(
            label: t.t('logs.model'),
            active: f.modelType == 'original',
            ghost: f.modelType != 'original',
            onTap: () =>
                controller.setFilters(f.copyWith(modelType: 'original')),
          ),
          FilterDropdown(
            width: 180,
            value: f.modelText,
            onChanged: (v) => controller.setFilters(f.copyWith(modelText: v)),
            allLabel: t.t('logs.filterModel'),
            searchPlaceholder: t.t('stats.searchModel'),
            emptyLabel: t.t('stats.noMatch'),
            options: [
              for (final m in controller.modelOptions)
                FilterOption(value: m, label: m),
            ],
          ),
          // 路径搜索：对 request_url 做 LIKE 匹配（`ListView.tsx:174-180`）。
          SizedBox(
            width: 180,
            child: KeptTextField(
              key: const Key('logs-path'),
              value: f.path,
              hint: t.t('logs.filterPath'),
              onChanged: (v) => controller.setFilters(f.copyWith(path: v)),
            ),
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
    _sub = (widget.logUpdates ?? debounceStream(kernelProxyLogUpdated()))
        .listen((_) {
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
                  FilterOption(
                    value: 'test',
                    label: t.t('requestLog.typeTest'),
                  ),
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
            groupName: _c.groupName,
            copied: _c.copied,
            onClose: _c.closeDetail,
            onCopyAll: () {
              _c.copyDetail(_c.detail!, widget.copyText);
              _flashCopied();
            },
            onCopy: (text) {
              widget.copyText(text);
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
                        // 两个状态码有专门的说法，不显示裸数字
                        //（`Logs/primitives.tsx:310-314`）：
                        //   0   = 还没有终态（流式在跑）→「未完成」
                        //   499 = 客户端提前断开 → 「已中断」
                        switch (log.statusCode) {
                          0 => t.t('logs.statusIncomplete'),
                          499 => t.t('logs.statusInterrupted'),
                          _ => '${log.statusCode}',
                        },
                        style: AidogType.micro.copyWith(
                          // 2xx 绿、其余一律红 —— 包括 0。
                          // 原先把 0 画成灰色，与 React 相反：流式跑到一半没落终态
                          // 通常就是出事了，灰色会让人以为「正常，只是还没结束」。
                          color: log.statusCode >= 200 && log.statusCode < 300
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
    required this.onCopy,
    required this.groupName,
  });

  /// group_key → 分组名。详情里显名字，密钥只作复制内容 —— 那串是 API Key。
  final String Function(String) groupName;

  final ProxyLogDetail detail;
  final bool copied;
  final VoidCallback onClose;
  final VoidCallback onCopyAll;

  /// 单个区块的复制（React 每块自带一个 `CopyButton`，不是整页一个）。
  final void Function(String text) onCopy;

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
            // 显**分组名**，不显 group_key —— 那串正好是该分组的 API Key，
            // 分组卡已经因为这个理由不印它了（`groups.dart:530-533`），
            // 详情面板这处原先还留着。React 同样只显名字（`DetailPanel.tsx:171`）。
            _kv(theme, t.t('logs.group'), groupName(detail.groupKey)),
            _kv(theme, t.t('logs.model'), detail.model),
            _kv(theme, t.t('logs.actualModel'), detail.actualModel),
            _kv(theme, t.t('logs.sourceProtocol'), detail.sourceProtocol),
            _kv(theme, t.t('logs.targetProtocol'), detail.targetProtocol),
            _kv(theme, t.t('logs.status'), '${detail.statusCode}'),
            // 上游状态码：0 / 缺失 = 没捕获到（`DetailPanel.tsx:190-208`）。
            // 这个字段早就解析进来了，详情区就是没这一项。
            _kv(
              theme,
              t.t('logs.upstreamStatus'),
              detail.upstreamStatusCode == 0
                  ? t.t('logs.notCaptured')
                  : '${detail.upstreamStatusCode}',
            ),
            // 传输方式（`DetailPanel.tsx:209`）。模型层原先没接这个字段。
            _kv(
              theme,
              t.t('logs.stream'),
              detail.isStream
                  ? t.t('logs.streaming')
                  : t.t('logs.nonStreaming'),
            ),
            _kv(
              theme,
              t.t('logs.duration'),
              formatDurationMs(detail.durationMs.toDouble()),
            ),
            _kv(
              theme,
              t.t('logs.inputTokens'),
              formatNumber(detail.inputTokens),
            ),
            _kv(
              theme,
              t.t('logs.outputTokens'),
              formatNumber(detail.outputTokens),
            ),
            _kv(
              theme,
              t.t('logs.cacheTokens'),
              formatNumber(detail.cacheTokens),
            ),
            if (detail.attempts.isNotEmpty) _attempts(t, theme),
            // 用户侧与上游侧**分开列**：两边受不同开关控制
            // （`log_user_request` / `log_upstream_request`，见项目 CLAUDE.md
            // 的「Proxy 日志」段）。合成一份会让人分不清关掉的是哪个开关。
            const SizedBox(height: AidogSpace.smd),
            TileMeta(t.t('logs.userRequest')),
            _section(t, theme, 'URL', detail.requestUrl),
            _section(
              t,
              theme,
              t.t('logs.requestHeaders'),
              detail.requestHeaders,
            ),
            _section(t, theme, t.t('logs.requestBody'), detail.requestBody),
            _section(
              t,
              theme,
              t.t('logs.responseHeaders'),
              detail.userResponseHeaders,
            ),
            _section(
              t,
              theme,
              t.t('logs.responseBody'),
              detail.userResponseBody,
            ),
            const SizedBox(height: AidogSpace.smd),
            TileMeta(t.t('logs.upstreamRequest')),
            _section(t, theme, 'URL', detail.upstreamRequestUrl),
            _section(
              t,
              theme,
              t.t('logs.requestHeaders'),
              detail.upstreamRequestHeaders,
            ),
            _section(
              t,
              theme,
              t.t('logs.requestBody'),
              detail.upstreamRequestBody,
            ),
            _section(
              t,
              theme,
              t.t('logs.responseHeaders'),
              detail.upstreamResponseHeaders,
            ),
            _section(t, theme, t.t('logs.responseBody'), detail.responseBody),
          ],
        ),
      ),
    );
  }

  /// 尝试记录（`DetailPanel.tsx:218-261`）：多平台重试时逐次列出平台 / 状态码 /
  /// 耗时 / 错误原文。**失败排障时最关键的一块** —— 没有它只知道「失败了」，
  /// 不知道试了哪几个平台、各自怎么失败的。
  Widget _attempts(I18nController t, AidogTheme theme) => Padding(
    padding: const EdgeInsets.only(top: AidogSpace.smd),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            TileMeta(t.t('logs.attempts')),
            const SizedBox(width: AidogSpace.sxs),
            MiniBadge(
              text: t
                  .t('logs.attemptCount')
                  .replaceAll('{{n}}', '${detail.attempts.length}'),
              color: theme.c.peak,
            ),
          ],
        ),
        const SizedBox(height: AidogSpace.sxs),
        for (var i = 0; i < detail.attempts.length; i++)
          _attemptRow(t, theme, i, detail.attempts[i]),
      ],
    ),
  );

  Widget _attemptRow(
    I18nController t,
    AidogTheme theme,
    int i,
    ProxyAttempt a,
  ) {
    final tone = a.ok ? theme.c.ok : theme.c.bad;
    final name = a.platformName.isNotEmpty
        ? a.platformName
        : '#${a.platformId}';
    final status = a.statusCode == 0
        ? t.t('logs.connFailed')
        : '${a.statusCode}';
    // 摘要串照 React：平台名 | 状态码 | 耗时ms | 错误（有才拼）。
    final summary = [
      name,
      status,
      '${a.durationMs}ms',
      if (a.error.isNotEmpty) a.error,
    ].join(' | ');
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
      child: Container(
        key: ValueKey('attempt-$i'),
        padding: const EdgeInsets.symmetric(
          horizontal: AidogSpace.ssm,
          vertical: AidogSpace.sxs,
        ),
        decoration: BoxDecoration(
          border: Border.all(color: tone),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            SizedBox(
              width: 24,
              child: Text(
                '#${i + 1}',
                style: AidogType.numSm.copyWith(color: theme.c.fg3),
              ),
            ),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: AidogType.micro.copyWith(color: theme.c.fg),
                  ),
                  if (a.error.isNotEmpty)
                    Text(
                      a.error,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(color: theme.c.bad),
                    ),
                ],
              ),
            ),
            const SizedBox(width: AidogSpace.ssm),
            Text(
              status,
              style: AidogType.micro.copyWith(
                color: tone,
                fontWeight: FontWeight.w600,
              ),
            ),
            const SizedBox(width: AidogSpace.ssm),
            Text(
              '${a.durationMs}ms',
              style: AidogType.numSm.copyWith(color: theme.c.fg3),
            ),
            IconButton(
              key: ValueKey('attempt-copy-$i'),
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
              iconSize: 13,
              visualDensity: VisualDensity.compact,
              tooltip: t.t('logs.copy'),
              icon: Icon(Icons.copy_outlined, color: theme.c.fg3),
              onPressed: () => onCopy(summary),
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
          child: Text(k, style: AidogType.micro.copyWith(color: theme.c.fg3)),
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

  /// 一个正文区块：小标题 + 正文 + **本块自己的**复制按钮。
  ///
  /// 不再 `maxLines: 14` 封顶：一个正常大小的请求体就看不全，也滚不动。
  /// 正文整段铺开，由面板自己那层滚动承接 —— 区块内不再套第二层滚动条
  /// （套了之后手势会被内层吃掉，滚到底也接不上外层）。
  Widget _section(
    I18nController t,
    AidogTheme theme,
    String title,
    String body,
  ) {
    final empty = body.trim().isEmpty;
    final text = empty ? t.t('logs.noUpstream') : prettyJsonOrRaw(body);
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              TileMeta(title),
              const Spacer(),
              // 空块没什么可复制的，不画按钮（React：占位串不给复制）。
              if (!empty)
                IconButton(
                  key: ValueKey('sec-copy-$title-${body.hashCode}'),
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(
                    minWidth: 22,
                    minHeight: 22,
                  ),
                  iconSize: 13,
                  visualDensity: VisualDensity.compact,
                  tooltip: t.t('logs.copy'),
                  icon: Icon(Icons.copy_outlined, color: theme.c.fg3),
                  onPressed: () => onCopy(text),
                ),
            ],
          ),
          const SizedBox(height: 4),
          Container(
            width: double.infinity,
            padding: const EdgeInsets.all(AidogSpace.ssm),
            decoration: BoxDecoration(
              color: theme.c.surface2,
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: SelectableText(
              text,
              style: AidogType.micro.copyWith(
                color: empty ? theme.c.fg3 : theme.c.fg2,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
