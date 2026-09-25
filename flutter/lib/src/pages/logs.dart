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

/// 日志两页页头按钮的字阶 / 内衬：React 是 `fontSize: F.hint(13)` +
/// `padding: "4px 10px"`（`ListView.tsx:70,75,78`），不是 [SmallButton] 缺省的
/// micro 11 + 10/5。
const double kLogsBtnFontSize = 13;
const (double, double) kLogsBtnPadding = (10, 4);

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
          // 后端为了省开销不再跑精确 COUNT(*)（logs-query-ipc-slimming s2），
          // 手上只有**当前页**的条数。原先把它当总数写成「N 条」——
          // 翻到第二页数字还是 20，读起来像「一共就这么多」。React 同样只说
          // 「日志列表」（`ListView.tsx:63`），不编一个总数出来。
          subtitle: _c.logs.isEmpty
              ? t.t('logs.empty')
              : t.t('logs.totalUnknown'),
          bottom: 16, // React 全页 gap 16（ListView.tsx:56）
          trailing: Wrap(
            spacing: 8, // ListView.tsx:66 gap 8
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              if (_c.cleanupMessage.isNotEmpty)
                Text(
                  _c.cleanupMessage,
                  // F.hint 13 secondary（ListView.tsx:68），不是绿色 micro。
                  style: AidogType.caption.copyWith(
                    fontSize: 13,
                    color: theme.c.fg2,
                  ),
                ),
              // 三颗都是实心（React `variant="default"` ×2 + `destructive`，
              // `ListView.tsx:70,75,78`），字阶 13 / padding 10-4。
              SmallButton(
                key: const ValueKey('logs-refresh'),
                label: '',
                icon: Icons.refresh,
                filled: true,
                fontSize: kLogsBtnFontSize,
                padding: kLogsBtnPadding,
                tooltip: t.t('logs.refresh'),
                onTap: _c.loading ? null : () => _c.load(),
              ),
              // 一条日志都没有时这两颗不出现（`ListView.tsx:73`）：
              // 没东西可清，摆两颗按钮在那儿只会让人以为清失败了。
              if (_c.logs.isNotEmpty) ...[
                SmallButton(
                  label: t.t('logs.cleanupExpired'),
                  filled: true,
                  fontSize: kLogsBtnFontSize,
                  padding: kLogsBtnPadding,
                  onTap: () => _c.cleanupExpired(
                    doneText: t.t('logs.cleanupExpiredDone'),
                  ),
                ),
                SmallButton(
                  label: t.t('logs.clear'),
                  danger: true,
                  filled: true,
                  fontSize: kLogsBtnFontSize,
                  padding: kLogsBtnPadding,
                  onTap: () => setState(() => _c.showClearConfirm = true),
                ),
              ],
            ],
          ),
        ),
        _LogsFilterBar(controller: _c),
        const SizedBox(height: 16), // React 全页 gap 16（ListView.tsx:56）
        if (_c.loading)
          // React 是裸 div：padding 20、继承 15 正体 secondary（ListView.tsx:191）。
          Padding(
            padding: const EdgeInsets.all(20),
            child: Text(
              t.t('status.loading'),
              style: AidogType.body.copyWith(color: theme.c.fg2),
            ),
          )
        else if (_c.logs.isEmpty)
          // glass-surface + padding 40 + 居中 + 13 tertiary（ListView.tsx:193-195）
          CenteredNote(text: t.t('logs.empty'), padding: 40, fontSize: 13)
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
          const SizedBox(height: 16), // React 全页 gap 16（ListView.tsx:56）
          // Logs 主页没有精确总数（后端只回 has_more），所以分页只有上一页 / 下一页。
          _Pager(
            currentPage: _c.currentPage,
            hasMore: _c.hasMore,
            resultCount: _c.logs.length,
            pageSize: _c.pageSize,
            onPage: _c.goToPage,
            onPageSize: _c.setPageSize,
          ),
        ],
        if (_c.showClearConfirm)
          // shadcn AlertDialog：maxWidth 380 / padding 20、标题 13 w600、
          // 正文 12 secondary lh1.6、按钮 12 / 6-14、确认键实心红
          //（`ListView.tsx:247-270`）。
          ConfirmCard(
            title: t.t('logs.clearConfirmTitle'),
            body: t.t('logs.clearConfirm'),
            confirmLabel: t.t('logs.clear'),
            maxWidth: 380,
            padding: const EdgeInsets.all(20),
            titleStyle: AidogType.label.copyWith(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: theme.c.fg,
            ),
            bodyStyle: AidogType.caption.copyWith(
              fontSize: 12,
              height: 1.6,
              color: theme.c.fg2,
            ),
            buttonFontSize: 12,
            buttonPadding: (14, 6),
            dangerConfirm: true,
            onCancel: () => setState(() => _c.showClearConfirm = false),
            onConfirm: _c.confirmClear,
          ),
        if (_c.detail != null)
          _DetailPanel(
            detail: _c.detail!,
            groupName: _c.groupName,
            platformName: _c.platformName,
            protocolLabel: (code) => _c.protocolLabel(code, t.locale),
            onRefresh: _c.refreshDetail,
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
      // React glass-surface：padding 12/16、gap 10（ListView.tsx:87）
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: Wrap(
        spacing: 10,
        runSpacing: 10,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          FilterDropdown(
            width: 140, // React 平台下拉 140（ListView.tsx:91）
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
            width: 140, // React 分组下拉 140（ListView.tsx:107）
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
          // 状态 / 时间 / 中间件三项在 React 是**无搜索框**的 `FilterSelect`
          //（`ListView.tsx:117-147` → `primitives.tsx:414-428`）：选项只有两三条，
          // 搭一个搜索框反而多一步。
          _Select(
            width: 130,
            value: f.status,
            placeholder: t.t('logs.filterStatus'),
            onChanged: (v) => controller.setFilters(f.copyWith(status: v)),
            items: [
              (value: 'success', label: t.t('logs.statusSuccess')),
              (value: 'error', label: t.t('logs.statusError')),
            ],
          ),
          _Select(
            width: 110,
            value: f.time == 'all' ? '' : f.time,
            placeholder: t.t('logs.filterTime'),
            onChanged: (v) =>
                controller.setFilters(f.copyWith(time: v.isEmpty ? 'all' : v)),
            items: [
              for (final p in kTimePresets)
                if (p != 'all') (value: p, label: p),
            ],
          ),
          // 中间件观察模式命中（票 04）：只看「规则命中但放行」的请求。
          // 筛选逻辑早就在 `LogsFilterState.observed`，之前没有入口，点不到。
          _Select(
            width: 150,
            value: f.observed,
            placeholder: t.t('logs.filterMiddleware'),
            onChanged: (v) => controller.setFilters(f.copyWith(observed: v)),
            items: [(value: 'observed', label: t.t('logs.observedOnly'))],
          ),
          // 模型名按「实际发给上游的」还是「客户端原始请求的」匹配
          //（`ListView.tsx:148-161`）。两者在有模型改写时不是一回事。
          SmallButton(
            label: t.t('logs.actualModel'),
            active: f.modelType == 'actual',
            ghost: f.modelType != 'actual',
            onTap: () => controller.setFilters(f.copyWith(modelType: 'actual')),
          ),
          SmallButton(
            label: t.t('logs.model'),
            active: f.modelType == 'original',
            ghost: f.modelType != 'original',
            onTap: () =>
                controller.setFilters(f.copyWith(modelType: 'original')),
          ),
          FilterDropdown(
            width: 170, // React 模型下拉 170（ListView.tsx:163）
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
          ConstrainedBox(
            // React 路径框 max-width 180 / min-width 120（ListView.tsx:178）
            constraints: const BoxConstraints(minWidth: 120, maxWidth: 180),
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
          bottom: 16, // React 全页 gap 16（RequestLog.tsx:193）
          // 带 14px 刷新图标的实心按钮、无文字（`RequestLog.tsx:203-205`）。
          trailing: SmallButton(
            key: const ValueKey('request-log-refresh'),
            label: '',
            icon: Icons.refresh,
            filled: true,
            fontSize: kLogsBtnFontSize,
            padding: kLogsBtnPadding,
            tooltip: t.t('action.refresh'),
            onTap: _c.loading ? null : () => _c.load(),
          ),
        ),
        Tile(
          // React glass-surface：padding 12/16、gap 10（RequestLog.tsx:210）
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Wrap(
            spacing: 10,
            runSpacing: 10,
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
                width: 140, // React FilterSelect max-width 140（primitives.tsx:419）
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
        const SizedBox(height: 16), // React 全页 gap 16（RequestLog.tsx:193）
        if (_c.loading)
          // 裸 div：padding 20、15 正体 secondary（RequestLog.tsx 同 ListView.tsx:191）
          Padding(
            padding: const EdgeInsets.all(20),
            child: Text(
              t.t('status.loading'),
              style: AidogType.body.copyWith(
                color: AidogTheme.of(context).c.fg2,
              ),
            ),
          )
        else if (_c.logs.isEmpty)
          // glass-surface + padding 40 + 13 tertiary（RequestLog.tsx:262-264）
          CenteredNote(text: t.t('requestLog.empty'), padding: 40, fontSize: 13)
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
          const SizedBox(height: 16), // React 全页 gap 16（RequestLog.tsx:193）
          // 本页有精确 total，所以分页是「第 N / 共 M 页」。
          _Pager(
            currentPage: _c.currentPage,
            hasMore: _c.currentPage < _c.totalPages,
            resultCount: _c.logs.length,
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
            platformName: _c.platformName,
            protocolLabel: (code) => _c.protocolLabel(code, t.locale),
            onRefresh: _c.refreshDetail,
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

/// 日志表。窄窗横向滚动 + 操作列 sticky（对齐 React `ListView.tsx:199` 的
/// `overflow:auto` 容器 + `primitives.tsx:232-262` 的 `position: sticky; right: 0`）：
/// 数据列放进共用一个 controller 的横向滚动区，28px 复制按钮钉在右缘不随滚动 ——
/// Flutter 没有 CSS sticky，这是 sticky 列的等价实现。
class _LogTable extends StatefulWidget {
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
  State<_LogTable> createState() => _LogTableState();
}

class _LogTableState extends State<_LogTable> {
  // 表头与所有数据行共用同一个横向 controller，滚动才能同步（React 是同一容器）。
  final ScrollController _hScroll = ScrollController();

  @override
  void dispose() {
    _hScroll.dispose();
    super.dispose();
  }

  // 九个数据列的最小总宽（逻辑 px）。窗口够宽时列区拉伸填满剩余空间
  // （等价 React 的 flex），不够宽时锁在最小宽、容器出横向滚动。
  static const double _kMinTableWidth = 1050;

  /// 行高（数据行与操作列共用）：两侧各画一半（滚动列 / 钉住的操作列），
  /// 高度必须逐行严格相等才能对齐，所以锁死而不是让内容自己撑。
  /// React 行是 13px 文字 + 上下 10 padding ≈ 40（primitives.tsx:249-262）。
  static const double _kRowHeight = 40;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Tile(
      child: LayoutBuilder(
        builder: (context, cons) {
          // 窗口够宽时列区拉伸填满剩余空间（等价 React 的 flex），
          // 不够宽时锁在最小宽、容器出横向滚动。
          final tableWidth = cons.maxWidth < _kMinTableWidth
              ? _kMinTableWidth
              : cons.maxWidth;
          return Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // 数据列：整表（表头 + 所有行）共用一个横向滚动视图，
              // 滚动天然同步（React 是同一容器 `overflow:auto`）。
              Expanded(
                child: SingleChildScrollView(
                  controller: _hScroll,
                  scrollDirection: Axis.horizontal,
                  child: SizedBox(
                    width: tableWidth,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        // 表头下边框 1px --border（`primitives.tsx:237`）。
                        Container(
                          height: _kRowHeight,
                          decoration: BoxDecoration(
                            border: Border(
                              bottom: BorderSide(color: theme.c.line),
                            ),
                          ),
                          child: Row(
                            children: [
                              _Cell(t.t('logs.time'), width: 150, header: true),
                              _Cell(
                                t.t('logs.group'),
                                width: 110,
                                header: true,
                              ),
                              _Cell(
                                t.t('logs.platform'),
                                width: 130,
                                header: true,
                              ),
                              // 「原始模型」列：发生模型改写时，不写出来就看不出
                              // 客户端原本请求的是哪个模型
                              //（`ListView.tsx:202-211` 是十列）。
                              _Cell(
                                t.t('logs.model'),
                                width: 170,
                                header: true,
                              ),
                              _Cell(
                                t.t('logs.actualModel'),
                                width: 170,
                                header: true,
                              ),
                              _Cell(
                                t.t('logs.status'),
                                width: 70,
                                header: true,
                              ),
                              _Cell(
                                t.t('logs.duration'),
                                width: 90,
                                header: true,
                              ),
                              _Cell(
                                t.t('logs.inputTokens'),
                                width: 80,
                                header: true,
                              ),
                              _Cell(
                                t.t('logs.outputTokens'),
                                width: 80,
                                header: true,
                              ),
                            ],
                          ),
                        ),
                        // 入场错峰 + 悬停抬升（`primitives.tsx:284-290` 的
                        // `useReveal(idx*60)` + `hover-lift`）。
                        for (final (i, log) in widget.rows.indexed)
                          Container(
                            height: _kRowHeight,
                            // 行分隔线：shadcn `TableRow` 的 `border-b`，被
                            // `.glass-table tbody tr` 压到 --border 40%
                            //（`globals.css:282-284`）。
                            decoration: BoxDecoration(
                              border: Border(
                                bottom: BorderSide(
                                  color: theme.c.line.withValues(
                                    alpha: theme.c.line.a * 0.4,
                                  ),
                                ),
                              ),
                            ),
                            child: Reveal(
                              delayMs: i * 60,
                              child: HoverLift(
                                child: InkWell(
                                  onTap: () => widget.onOpen(log.id),
                                  child: Row(
                                    children: [
                                      _Cell(
                                        formatDateTime(log.createdAt),
                                        width: 150,
                                      ),
                                      SizedBox(
                                        width: 110,
                                        child: Padding(
                                          padding: _kCellPad,
                                          child: Align(
                                            alignment: AlignmentDirectional
                                                .centerStart,
                                            // `.badge badge-accent`：11 w600
                                            // ls0.02em / pad 2-8 / radius 6 /
                                            // 底 accent-subtle（globals.css:542-556）。
                                            child: MiniBadge(
                                              text: widget.groupName(
                                                log.groupKey,
                                              ),
                                              color: theme.c.accentText,
                                              fontSize: 11,
                                              padX: 8,
                                              padY: 2,
                                              radius: 6,
                                              background: theme.c.accentWash,
                                            ),
                                          ),
                                        ),
                                      ),
                                      // 重试徽标 ↻N
                                      //（`primitives.tsx:296-300`）。
                                      _CellWithBadge(
                                        text: widget.platformName(
                                          log.platformId,
                                        ),
                                        width: 130,
                                        // 平台名 12 secondary（primitives.tsx:27）
                                        textColor: theme.c.fg2,
                                        badge: log.retryCount > 0
                                            ? '↻${log.retryCount}'
                                            : null,
                                        // 重试徽标是 warning 语义色（primitives.tsx:28）
                                        badgeColor: theme.c.peak,
                                        badgeTooltip: t.t('logs.retriedHint', {
                                          'n': '${log.retryCount}',
                                        }),
                                      ),
                                      // 流式徽标 SSE
                                      //（`primitives.tsx:304-306`）。
                                      _CellWithBadge(
                                        text: log.model.isEmpty
                                            ? '-'
                                            : log.model,
                                        width: 170,
                                        // 模型名 12 w500（primitives.tsx:29）
                                        fontWeight: FontWeight.w500,
                                        badge: log.isStream ? 'SSE' : null,
                                        // SSE 徽标是 accent 语义色（primitives.tsx:30）
                                        badgeColor: theme.c.accentText,
                                        badgeBackground: theme.c.accentWash,
                                        badgeTooltip: t.t('logs.streaming'),
                                      ),
                                      _Cell(
                                        log.actualModel.isEmpty
                                            ? '-'
                                            : log.actualModel,
                                        width: 170,
                                        // 实际模型也是 12 w500（primitives.tsx:312）
                                        fontSize: 12,
                                        fontWeight: FontWeight.w500,
                                      ),
                                      // 状态码（primitives.tsx:314-320）：
                                      // 0 →「未完成」、499 →「已中断」、
                                      // 其余裸数字；2xx 绿 / 非 2xx 红。
                                      _Cell(
                                        log.statusCode == 0
                                            ? t.t('logs.statusIncomplete')
                                            : log.statusCode == 499
                                            ? t.t('logs.statusInterrupted')
                                            : '${log.statusCode}',
                                        width: 70,
                                        color:
                                            log.statusCode >= 200 &&
                                                log.statusCode < 300
                                            ? theme.c.ok
                                            : theme.c.bad,
                                      ),
                                      _Cell(
                                        formatDurationMs(
                                          log.durationMs.toDouble(),
                                        ),
                                        width: 90,
                                      ),
                                      _Cell(
                                        formatNumber(log.inputTokens),
                                        width: 80,
                                      ),
                                      _Cell(
                                        formatNumber(log.outputTokens),
                                        width: 80,
                                      ),
                                    ],
                                  ),
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              ),
              // 操作列：钉在右缘不随横向滚动（React `primitives.tsx:232-262`
              // 的 `position: sticky; right: 0`；Flutter 没有 CSS sticky，
              // 把它画在滚动区外就是等价实现）。首格对齐表头行高。
              SizedBox(
                // 按钮 padding 2 + 14px 图标，外层 TdCell 10/14 → 约 42
                //（`primitives.tsx:31,252,333`）。
                width: 42,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const SizedBox(height: _kRowHeight),
                    for (final log in widget.rows)
                      SizedBox(
                        height: _kRowHeight,
                        child: IconButton(
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints(),
                          iconSize: 14,
                          tooltip: t.t('logs.copy'),
                          color: theme.c.fg3,
                          icon: const Icon(Icons.copy_outlined),
                          onPressed: () => widget.onCopy(log.id),
                        ),
                      ),
                  ],
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}

/// 一格文字 + 可选的行内徽标（重试 ↻N / 流式 SSE）。
/// 徽标为 null 时与 [_Cell] 完全一样。
/// 单元格内边距：React `ThCell` / `TdCell` 都是 `padding: 10px 14px`
/// （`primitives.tsx:235,252`）。
const EdgeInsets _kCellPad = EdgeInsets.symmetric(horizontal: 14, vertical: 10);

class _CellWithBadge extends StatelessWidget {
  const _CellWithBadge({
    required this.text,
    required this.width,
    required this.badge,
    required this.badgeTooltip,
    this.textColor,
    this.fontWeight,
    this.badgeColor,
    this.badgeBackground,
  });

  final String text;
  final double width;
  final String? badge;
  final String badgeTooltip;

  /// 文字色 / 字重覆盖：平台名是 12 secondary、模型名是 12 w500
  /// （`primitives.tsx:27,29`）。
  final Color? textColor;
  final FontWeight? fontWeight;

  /// 徽标语义色：重试是 warning、SSE 是 accent（`primitives.tsx:28,30`）。
  /// 原先两枚共用一个灰徽标，语义全丢。
  final Color? badgeColor;
  final Color? badgeBackground;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return SizedBox(
      width: width,
      child: Padding(
        padding: _kCellPad,
        child: Row(
          children: [
            Flexible(
              child: Text(
                text,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.caption.copyWith(
                  fontSize: 12, // F.small（primitives.tsx:27,29）
                  fontWeight: fontWeight,
                  color: textColor ?? theme.c.fg,
                ),
              ),
            ),
            if (badge case final b?) ...[
              const SizedBox(width: 6), // INLINE_FLEX_STYLE gap 6（primitives.tsx:26）
              Tooltip(
                message: badgeTooltip,
                child: MiniBadge(
                  text: b,
                  color: badgeColor ?? theme.c.fg3,
                  background: badgeBackground,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _Cell extends StatelessWidget {
  const _Cell(
    this.text, {
    required this.width,
    this.header = false,
    this.color,
    this.fontSize,
    this.fontWeight,
  });

  final String text;
  final double width;
  final bool header;
  final Color? color;
  final double? fontSize;
  final FontWeight? fontWeight;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return SizedBox(
      width: width,
      child: Padding(
        padding: _kCellPad,
        child: Text(
          text, // 混合大小写：React 表头与正文都不大写
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: header
              ? AidogType.caption.copyWith(
                  // React ThCell：12 w600 secondary（primitives.tsx:232-241）
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: theme.c.fg2,
                )
              : AidogType.caption.copyWith(
                  // React 行 13px（ListView.tsx:200 表 fontSize F.hint）
                  fontSize: fontSize ?? 13,
                  fontWeight: fontWeight,
                  color: color ?? theme.c.fg,
                ),
        ),
      ),
    );
  }
}

class _Pager extends StatelessWidget {
  const _Pager({
    required this.currentPage,
    required this.hasMore,
    required this.resultCount,
    required this.pageSize,
    required this.onPage,
    required this.onPageSize,
    this.totalPages,
  });

  final int currentPage;
  final bool hasMore;

  /// 本页行数，用来算区间 `rangeStart–rangeEnd`（`primitives.tsx:358-359`）。
  final int resultCount;
  final int? totalPages;
  final int pageSize;
  final void Function(int) onPage;
  final void Function(int) onPageSize;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final rangeStart = (currentPage - 1) * pageSize + 1;
    final rangeEnd = rangeStart + resultCount - 1;
    // 分页文字统一 12 / ls0 / tertiary，正常大小写（`primitives.tsx:366,371`）。
    final noteStyle = AidogType.micro.copyWith(
      fontSize: 12,
      letterSpacing: 0,
      color: theme.c.fg3,
    );
    // React 是 space-between：**左**边区间 + 每页选择器，**右**边 ⟪ ← →
    //（`primitives.tsx:364-397`）。原先两组位置是反的。
    return Row(
      children: [
        Text(
          resultCount > 0 ? '$rangeStart–$rangeEnd' : '$rangeStart',
          style: noteStyle,
        ),
        if (hasMore || currentPage > 1)
          Text(
            ' · ${t.t(hasMore ? 'logs.hasMore' : 'logs.noMore')}',
            style: noteStyle,
          ),
        if (totalPages != null) ...[
          const SizedBox(width: 8),
          Text('$currentPage / $totalPages', style: noteStyle),
        ],
        const SizedBox(width: 8), // primitives.tsx:365 gap 8
        // 三个裸数字看不出是什么，React 在它们前面写着「每页」
        //（`primitives.tsx:371`）。
        Text(t.t('logs.pageSize'), style: noteStyle),
        const SizedBox(width: 4), // primitives.tsx:370 gap 4
        // 每页条数是一个 80×28 的下拉（`primitives.tsx:372-387`），
        // 不是三颗并排按钮。
        _Select(
          width: 80,
          height: 28,
          value: '$pageSize',
          placeholder: '$pageSize',
          showAll: false,
          onChanged: (v) => onPageSize(int.parse(v)),
          items: [
            for (final size in const [20, 50, 100])
              (value: '$size', label: '$size'),
          ],
        ),
        const Spacer(),
        // ghost 档 12 / 4-8 / minWidth 28（`primitives.tsx:359-361`）。
        SmallButton(
          label: '⟪',
          fontSize: 12,
          padding: (8, 4),
          minWidth: 28,
          ghost: true,
          // 首页直达（`primitives.tsx:391-392`）。
          onTap: currentPage > 1 ? () => onPage(1) : null,
        ),
        const SizedBox(width: 4), // primitives.tsx:390 gap 4
        SmallButton(
          label: t.t('action.prev'),
          fontSize: 12,
          padding: (8, 4),
          minWidth: 28,
          ghost: true,
          onTap: currentPage > 1 ? () => onPage(currentPage - 1) : null,
        ),
        const SizedBox(width: 4),
        SmallButton(
          label: t.t('action.next'),
          fontSize: 12,
          padding: (8, 4),
          minWidth: 28,
          ghost: true,
          onTap: hasMore ? () => onPage(currentPage + 1) : null,
        ),
      ],
    );
  }
}

/// React 的 `btn-icon` ghost 图标按钮（详情工具栏的刷新 / 复制、请求 ID 的复制）。
/// [onTap] 为 null = 禁用。
class _IconBtn extends StatelessWidget {
  const _IconBtn({
    super.key,
    required this.icon,
    required this.tooltip,
    required this.onTap,
    this.size = 16,
    this.color,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback? onTap;
  final double size;
  final Color? color;

  @override
  Widget build(BuildContext context) => IconButton(
    padding: EdgeInsets.zero,
    constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
    iconSize: size,
    visualDensity: VisualDensity.compact,
    tooltip: tooltip,
    color: color,
    icon: Icon(icon),
    onPressed: onTap,
  );
}

/// 浮在代码块 / 尝试行**右上角**的 24×24 半透明复制按钮
/// （React `CopyButton` 的 `COPY_ICON_STYLE`，`primitives.tsx:35-42`：
/// `top:4; right:4; 24×24; 底 bg-surface 70%; 1px border; radius 6; opacity .55`）。
class _CopyOverlayBtn extends StatelessWidget {
  const _CopyOverlayBtn({
    super.key,
    required this.tooltip,
    required this.onTap,
  });

  final String tooltip;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = AidogTheme.of(context).c;
    return Tooltip(
      message: tooltip,
      child: Opacity(
        opacity: 0.55,
        child: Material(
          color: c.surface.withValues(alpha: 0.7),
          shape: RoundedRectangleBorder(
            side: BorderSide(color: c.line),
            borderRadius: BorderRadius.circular(6),
          ),
          child: InkWell(
            onTap: onTap,
            borderRadius: BorderRadius.circular(6),
            child: SizedBox(
              width: 24,
              height: 24,
              child: Icon(Icons.copy_outlined, size: 14, color: c.fg2),
            ),
          ),
        ),
      ),
    );
  }
}

/// JSON 极简高亮：key / 字符串 / 数字 / 字面量各一色。
///
/// React 那四块 headers / body 走的是 `JsonCodeEditor`（`primitives.tsx:192` 等），
/// 带完整语法高亮；Flutter 侧没有等价渲染器，这里按同一套语义分色 ——
/// 分不出 key 与值的一整块灰是最难读的那种。解析失败 / 非 JSON 原样返回。
TextSpan jsonSpan(String src, TextStyle base, AidogColors c) {
  final spans = <TextSpan>[];
  var i = 0;
  // 未上色的一段（括号、逗号、空白）攒着一次性提交，少建 span。
  var plainFrom = 0;
  void flushPlain(int end) {
    if (end > plainFrom) {
      spans.add(TextSpan(text: src.substring(plainFrom, end), style: base));
    }
  }

  while (i < src.length) {
    final ch = src[i];
    if (ch == '"') {
      // 扫到配对的引号（跳过转义）。
      var j = i + 1;
      while (j < src.length && src[j] != '"') {
        j += src[j] == r'\' ? 2 : 1;
      }
      final end = j < src.length ? j + 1 : src.length;
      // 引号后（跳空白）跟冒号的是 key。
      var k = end;
      while (k < src.length && (src[k] == ' ' || src[k] == '\t')) {
        k++;
      }
      final isKey = k < src.length && src[k] == ':';
      flushPlain(i);
      spans.add(
        TextSpan(
          text: src.substring(i, end),
          style: base.copyWith(color: isKey ? c.accentText : c.ok),
        ),
      );
      i = plainFrom = end;
      continue;
    }
    if (_isNumStart(src, i)) {
      var j = i + 1;
      while (j < src.length && _isNumBody(src[j])) {
        j++;
      }
      flushPlain(i);
      spans.add(
        TextSpan(text: src.substring(i, j), style: base.copyWith(color: c.peak)),
      );
      i = plainFrom = j;
      continue;
    }
    final lit = _literalAt(src, i);
    if (lit != null) {
      flushPlain(i);
      spans.add(TextSpan(text: lit, style: base.copyWith(color: c.accentText)));
      i = plainFrom = i + lit.length;
      continue;
    }
    i++;
  }
  flushPlain(src.length);
  return TextSpan(children: spans, style: base);
}

bool _isDigit(String ch) => ch.codeUnitAt(0) >= 0x30 && ch.codeUnitAt(0) <= 0x39;

bool _isNumStart(String s, int i) =>
    _isDigit(s[i]) ||
    (s[i] == '-' && i + 1 < s.length && _isDigit(s[i + 1]));

bool _isNumBody(String ch) =>
    _isDigit(ch) || ch == '.' || ch == 'e' || ch == 'E' || ch == '+' ||
    ch == '-';

String? _literalAt(String s, int i) {
  for (final lit in const ['true', 'false', 'null']) {
    if (s.startsWith(lit, i)) return lit;
  }
  return null;
}

/// 无搜索框的下拉（状态 / 时间 / 中间件筛选、每页条数）。
///
/// React 这几处是 shadcn `<Select>`（`primitives.tsx:372-387,414-428`），
/// 不是带搜索框的 `FilterDropdown` —— 选项只有两三条，搜索框反而多一步。
class _Select extends StatelessWidget {
  const _Select({
    required this.width,
    required this.value,
    required this.items,
    required this.onChanged,
    required this.placeholder,
    this.height = 30,
    this.showAll = true,
  });

  final double width;
  final double height;
  final String value;
  final List<({String value, String label})> items;
  final ValueChanged<String> onChanged;

  /// 未选中时显示的占位；[showAll] 时它同时是「全部」那一项的文案
  /// （React 的 `SelectItem value={NONE}`，`primitives.tsx:423`）。
  final String placeholder;
  final bool showAll;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Container(
      width: width,
      height: height,
      // shadcn SelectTrigger：bg-card + 1px --input + rounded-md(12)
      //（`ui/select.tsx:24`）；这几处另写了 padding 4/8（`primitives.tsx:378,419`）。
      padding: const EdgeInsets.symmetric(horizontal: 8),
      decoration: BoxDecoration(
        color: t.c.surface,
        border: Border.all(color: t.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.md),
      ),
      child: DropdownButtonHideUnderline(
        child: DropdownButton<String>(
          value: value,
          isExpanded: true,
          isDense: true,
          dropdownColor: t.c.surface,
          iconSize: 16,
          iconEnabledColor: t.c.fg3,
          style: AidogType.caption.copyWith(fontSize: 12, color: t.c.fg),
          items: [
            if (showAll)
              DropdownMenuItem<String>(
                value: '',
                child: Text(placeholder, overflow: TextOverflow.ellipsis),
              ),
            for (final i in items)
              DropdownMenuItem<String>(
                value: i.value,
                child: Text(i.label, overflow: TextOverflow.ellipsis),
              ),
          ],
          onChanged: (v) {
            if (v != null) onChanged(v);
          },
        ),
      ),
    );
  }
}

/// 详情面板。React 侧 2026-09-22 已从右侧 Sheet 改成居中 Radix Dialog
/// （`Logs/DetailPanel.tsx:47` `width: min(900px, 90vw)`）—— 为的正是与 Flutter
/// 对齐；这里维持居中 `AidogModal(maxWidth: 900)`，形态一致，无差异。
class _DetailPanel extends StatefulWidget {
  const _DetailPanel({
    required this.detail,
    required this.copied,
    required this.onClose,
    required this.onCopyAll,
    required this.onCopy,
    required this.groupName,
    required this.platformName,
    required this.protocolLabel,
    this.onRefresh,
  });

  /// group_key → 分组名。详情里显名字，密钥只作复制内容 —— 那串是 API Key。
  final String Function(String) groupName;

  /// platform_id → 平台名；协议 code → 本地化协议名。
  final String Function(int) platformName;
  final String Function(String) protocolLabel;

  /// 重新拉这一条详情。null = 该页不提供刷新。
  final VoidCallback? onRefresh;

  final ProxyLogDetail detail;
  final bool copied;
  final VoidCallback onClose;
  final VoidCallback onCopyAll;

  /// 单个区块的复制（React 每块自带一个 `CopyButton`，不是整页一个）。
  final void Function(String text) onCopy;

  @override
  State<_DetailPanel> createState() => _DetailPanelState();
}

class _DetailPanelState extends State<_DetailPanel> {
  /// 0 = 用户请求（Client → Proxy），1 = 上游请求（Proxy → Platform）。
  /// React 这两段是 tab（`DetailPanel.tsx:263-288`）；原先两段顺序平铺，
  /// 十块正文一次全展开，面板长到要滚很久才能看到上游那半。
  int _tab = 0;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final detail = widget.detail;
    final groupName = widget.groupName;
    final platformName = widget.platformName;
    final protocolLabel = widget.protocolLabel;
    final onClose = widget.onClose;
    final onCopyAll = widget.onCopyAll;
    final onCopy = widget.onCopy;
    final onRefresh = widget.onRefresh;
    final copied = widget.copied;
    // React 现在也是居中 Dialog（`Logs/DetailPanel.tsx:40-50`，2026-09-22 改），
    // 不再是右侧 Sheet —— 旧注释已随 React 侧改动过期。
    return AidogModal(
      // `width: min(900, 90vw)`（`DetailPanel.tsx:46-47`）。React 另写了
      // `maxHeight: 85vh` + 面板内滚；Flutter 这边整张面板本来就装在
      // `AidogModal` 的滚动视口里（视口高 = 窗口高），再套一层限高只会多一层
      // 裁剪，观感一致，所以不加。
      maxWidth: 900,
      maxWidthFactor: 0.9,
      onBarrierTap: onClose,
      child: ModalCard(
        // React DialogContent padding 20（DetailPanel.tsx:53）
        padding: const EdgeInsets.all(20),
        // `DialogContent` 自带 ✕。
        onClose: onClose,
        // `DialogTitle` 那个 sr-only 是另一个元素（`DetailPanel.tsx:58`）；
        // 可见标题「请求详情」在工具栏行里（`:141-143`），见下。
        semanticLabel: '${t.t('logs.detail')} ${detail.id}',
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 工具栏：两颗 16px 图标按钮 + 可见标题（`DetailPanel.tsx:130-143`）。
            // 关闭由 `ModalCard` 右上角的 ✕ 承担，这里不再摆第三颗文字按钮。
            Row(
              children: [
                _IconBtn(
                  key: const ValueKey('detail-refresh'),
                  icon: Icons.refresh,
                  size: 16,
                  tooltip: t.t('logs.refresh'),
                  onTap: onRefresh,
                  color: theme.c.fg2,
                ),
                const SizedBox(width: 12), // DetailPanel.tsx:130 gap 12
                _IconBtn(
                  key: const ValueKey('detail-copy-all'),
                  icon: copied ? Icons.check : Icons.copy_outlined,
                  size: 16,
                  tooltip: t.t('logs.copyAll'),
                  onTap: onCopyAll,
                  color: copied ? theme.c.ok : theme.c.fg2,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    t.t('logs.detail'),
                    style: AidogType.display.copyWith(
                      fontSize: 20, // F.title w700（DetailPanel.tsx:142）
                      fontWeight: FontWeight.w700,
                      color: theme.c.fg,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),
            // 请求 ID 独占一行：等宽 + 复制成 `request_id=<id>`
            //（`DetailPanel.tsx:146-167`）。原先只作标题旁的小字，复制不了。
            // 请求 ID 行是一张卡片：padding 12/20、label 12 w600、id 13 mono
            //（`DetailPanel.tsx:147-149`）。
            Container(
              // 右侧多留 24：`DialogContent` 自带的 ✕ 浮在右上角
              //（`ui/dialog.tsx:47`，right-4 top-4），不留出来会压住复制按钮。
              padding: const EdgeInsets.fromLTRB(20, 12, 24, 12),
              decoration: BoxDecoration(
                // `.glass-surface` 的底是 --bg-surface（globals.css:265-271），
                // 不是更深的 surface2。
                color: theme.c.surface,
                border: Border.all(color: theme.c.line),
                borderRadius: BorderRadius.circular(AidogRadius.md),
              ),
              child: Row(
                children: [
                  Text(
                    t.t('logs.requestId'),
                    style: AidogType.caption.copyWith(
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      color: theme.c.fg3,
                    ),
                  ),
                  const SizedBox(width: AidogSpace.smd),
                  Expanded(
                    child: SelectableText(
                      ltr(detail.id),
                      style: AidogType.numSm.copyWith(
                        fontSize: 13,
                        color: theme.c.fg,
                      ),
                    ),
                  ),
                  // React 是 `btn-icon` 图标按钮（`DetailPanel.tsx:150-166`）。
                  _IconBtn(
                    key: const ValueKey('detail-copy-id'),
                    icon: Icons.copy_outlined,
                    size: 16,
                    tooltip: t.t('logs.copyRequestId'),
                    color: theme.c.fg2,
                    onTap: () => onCopy('request_id=${detail.id}'),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 16),
            // 元信息 grid 是一张 `.glass-surface hover-lift` 卡、padding 20
            //（`DetailPanel.tsx:170`）；格内 label 12 tertiary / 值 15 w600，
            // `minmax(160px, 1fr)` gap 14。
            _metaGrid(theme, [
              _kv(
                theme,
                t.t('logs.group'),
                groupName(detail.groupKey),
                // 复制的是原始 group_key（审计用），显示的是名字。
                copyText: detail.groupKey,
              ),
              // 平台与时间原先整个没有：面板里看不出这条请求什么时候发生、打到哪
              //（`DetailPanel.tsx:172,214`）。
              _kv(
                theme,
                t.t('logs.platform'),
                platformName(detail.platformId),
                copyText: platformName(detail.platformId),
              ),
              _kv(theme, t.t('logs.time'), formatDateTime(detail.createdAt)),
              _kv(
                theme,
                t.t('logs.model'),
                detail.model,
                copyText: detail.model,
              ),
              _kv(
                theme,
                t.t('logs.actualModel'),
                detail.actualModel,
                copyText: detail.actualModel,
              ),
              // 协议印本地化名，裸枚举值只留在复制内容里供审计
              //（`DetailPanel.tsx:175-176`）。
              _kv(
                theme,
                t.t('logs.sourceProtocol'),
                protocolLabel(detail.sourceProtocol),
                copyText: detail.sourceProtocol,
              ),
              _kv(
                theme,
                t.t('logs.targetProtocol'),
                protocolLabel(detail.targetProtocol),
                copyText: detail.targetProtocol,
              ),
              // 状态码与列表同口径：0 →「未完成」、499 →「已中断」，2xx 绿其余红
              //（`DetailPanel.tsx:179-186`）。详情里原先是裸数字且不上色。
              _kv(
                theme,
                t.t('logs.status'),
                switch (detail.statusCode) {
                  0 => t.t('logs.statusIncomplete'),
                  499 => t.t('logs.statusInterrupted'),
                  _ => '${detail.statusCode}',
                },
                color: detail.statusCode >= 200 && detail.statusCode < 300
                    ? theme.c.ok
                    : theme.c.bad,
              ),
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
            ]),
            if (detail.attempts.isNotEmpty) _attempts(t, theme),
            // 用户侧与上游侧**分开列**：两边受不同开关控制
            // （`log_user_request` / `log_upstream_request`，见项目 CLAUDE.md
            // 的「Proxy 日志」段）。合成一份会让人分不清关掉的是哪个开关。
            const SizedBox(height: 16),
            // 两个 tab 的头：标题 + 副标题 + 协议 + 该侧状态码
            //（`primitives.tsx:101-133`）。整条底边 1px，激活那个按钮底边 2px accent。
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Row(
                  // React 两颗按内容宽、靠左排（inline-flex，`primitives.tsx:106-118`），
                  // 不是各占半条宽。
                  mainAxisSize: MainAxisSize.min,
                  // Flexible(loose) = CSS flex item 的默认 `flex-shrink: 1`：
                  // 按内容宽，装不下时才收（不是 Expanded 的各占一半）。
                  children: [
                    Flexible(
                      child: _tabButton(
                        t,
                        theme,
                        index: 0,
                        title: t.t('logs.userRequest'),
                        subtitle: 'Client → Proxy',
                        protocol: protocolLabel(detail.sourceProtocol),
                        statusCode: detail.statusCode,
                      ),
                    ),
                    Flexible(
                      child: _tabButton(
                        t,
                        theme,
                        index: 1,
                        title: t.t('logs.upstreamRequest'),
                        subtitle: 'Proxy → Platform',
                        protocol: protocolLabel(detail.targetProtocol),
                        statusCode: detail.upstreamStatusCode,
                      ),
                    ),
                  ],
                ),
                // 底边 1px --border（`primitives.tsx:101`）；不指定 color
                // 会吃 Material 默认灰。
                Divider(height: 1, thickness: 1, color: theme.c.line),
              ],
            ),
            // tab 正文是 `.glass-surface hover-lift` + padding 20 + gap 12
            //（`primitives.tsx:136`）。
            HoverLift(
              child: Tile(
                padding: const EdgeInsets.all(20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: _tab == 0
                      ? _userSections(t, theme, detail)
                      : _upstreamSections(t, theme, detail),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  List<Widget> _userSections(
    I18nController t,
    AidogTheme theme,
    ProxyLogDetail detail,
  ) => [
              _section(t, theme, 'URL', detail.requestUrl, hideWhenEmpty: true),
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
                detail.userResponseBody.trim().isEmpty
                    ? detail.responseBody
                    : detail.userResponseBody,
                emptyText: t.t('logs.streamResponse'),
              ),
  ];

  List<Widget> _upstreamSections(
    I18nController t,
    AidogTheme theme,
    ProxyLogDetail detail,
  ) => [
              _section(t, theme, 'URL', detail.upstreamRequestUrl, hideWhenEmpty: true),
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
              _section(
                t,
                theme,
                t.t('logs.responseBody'),
                detail.responseBody,
                emptyText: t.t('logs.streamResponse'),
              ),
  ];

  /// 尝试记录（`DetailPanel.tsx:218-261`）：多平台重试时逐次列出平台 / 状态码 /
  /// 耗时 / 错误原文。**失败排障时最关键的一块** —— 没有它只知道「失败了」，
  /// 不知道试了哪几个平台、各自怎么失败的。
  Widget _attempts(I18nController t, AidogTheme theme) => Padding(
    padding: const EdgeInsets.only(top: AidogSpace.smd),
    // `.glass-surface hover-lift` + padding 20（`DetailPanel.tsx:219`）。
    child: HoverLift(
      child: Tile(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                // 区标题是 F.body 15 w600 正常大小写（`DetailPanel.tsx:221`），
                // 不是全大写 micro。
                Text(
                  t.t('logs.attempts'),
                  style: AidogType.body.copyWith(
                    fontWeight: FontWeight.w600,
                    color: theme.c.fg,
                  ),
                ),
                const SizedBox(width: 8), // DetailPanel.tsx:220 gap 8
                MiniBadge(
                  text: t
                      .t('logs.attemptCount')
                      .replaceAll('{{n}}', '${widget.detail.attempts.length}'),
                  color: theme.c.peak,
                ),
              ],
            ),
            const SizedBox(height: 10), // DetailPanel.tsx:219 容器 gap 10
            for (var i = 0; i < widget.detail.attempts.length; i++)
              _attemptRow(t, theme, i, widget.detail.attempts[i]),
          ],
        ),
      ),
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
      padding: const EdgeInsets.only(bottom: 6), // 行间 gap 6（DetailPanel.tsx:226）
      // 复制按钮浮在行右上角（React 的 `CopyButton` 是 `position:absolute;
      // top:4; right:4`，`primitives.tsx:36-37`）。
      child: Stack(
        children: [
          Container(
            key: ValueKey('attempt-$i'),
            // padding 6px 28px 6px 10px —— 右侧 28 是给浮动复制按钮留的
            //（`DetailPanel.tsx:236`）。
            padding: const EdgeInsets.fromLTRB(10, 6, 28, 6),
            decoration: BoxDecoration(
              // React：底 = 语义色 8%、边 = 语义色 25%（原先边是全强度，视觉重一截）
              color: tone.withValues(alpha: 0.08),
              border: Border.all(color: tone.withValues(alpha: 0.25)),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                SizedBox(
                  width: 24,
                  child: Text(
                    '#${i + 1}',
                    style: AidogType.numSm.copyWith(
                      fontSize: 11,
                      color: theme.c.fg3,
                    ),
                  ),
                ),
                const SizedBox(width: 10), // grid gap 10（DetailPanel.tsx:235）
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.caption.copyWith(
                          fontSize: 12,
                          fontWeight: FontWeight.w500,
                          color: theme.c.fg,
                        ),
                      ),
                      if (a.error.isNotEmpty)
                        Text(
                          a.error,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: AidogType.micro.copyWith(
                            fontSize: 10,
                            color: theme.c.bad,
                          ),
                        ),
                    ],
                  ),
                ),
                const SizedBox(width: 10),
                Text(
                  status,
                  style: AidogType.caption.copyWith(
                    fontSize: 12,
                    color: tone,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(width: 10),
                Text(
                  '${a.durationMs}ms',
                  // React 这一格是 sans 11 tertiary（`DetailPanel.tsx:254`），
                  // 不是等宽。
                  style: AidogType.caption.copyWith(
                    fontSize: 11,
                    color: theme.c.fg3,
                  ),
                ),
              ],
            ),
          ),
          Positioned(
            top: 4,
            right: 4,
            child: _CopyOverlayBtn(
              key: ValueKey('attempt-copy-$i'),
              tooltip: t.t('logs.copy'),
              onTap: () => widget.onCopy(summary),
            ),
          ),
        ],
      ),
    );
  }

  /// 一个 tab 头：标题 + 协议 + 方向副标题 + 该侧状态码。
  Widget _tabButton(
    I18nController t,
    AidogTheme theme, {
    required int index,
    required String title,
    required String subtitle,
    required String protocol,
    required int statusCode,
  }) {
    final active = _tab == index;
    // tab 徽标口径对齐 React（`primitives.tsx:125-134`）：statusCode > 0 才画，
    // 且是**裸数字**（499 不转「已中断」——那是列表/详情 kv 的口径），
    // 0 = 没捕获到，直接不渲染。
    final showStatus = statusCode > 0;
    final statusText = '$statusCode';
    // React（primitives.tsx:104-121）：padding 10/20、13px、激活 w700 +
    // 底边 2px accent、未激活底边 2px 透明；没有盒子边框，没有淡底。
    return InkWell(
      key: ValueKey('detail-tab-$index'),
      onTap: () => setState(() => _tab = index),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
        decoration: BoxDecoration(
          border: Border(
            bottom: BorderSide(
              width: 2,
              color: active ? theme.c.accentText : Colors.transparent,
            ),
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Flexible(
                child: Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AidogType.label.copyWith(
                    fontSize: 13,
                    color: active ? theme.c.accentText : theme.c.fg2,
                    fontWeight: active ? FontWeight.w700 : FontWeight.w400,
                  ),
                ),
            ),
            const SizedBox(width: 8), // primitives.tsx:117 gap 8
            // 副标题也要能收：窄窗下（弹窗宽是 `min(900, 90vw)`）标题 + 副标题
            // + 徽标三件挤不下，React 那边靠 flex 收缩，这里对应 Flexible。
            Flexible(
              child: Text(
                ltr(subtitle),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg3,
                ),
              ),
            ),
            if (protocol.isNotEmpty) ...[
              const SizedBox(width: 8),
              // `.badge` + 10 / 1-5，底 --bg-glass 字 secondary
              //（`primitives.tsx:122`）。
              MiniBadge(
                text: protocol,
                color: theme.c.fg2,
                padX: 5,
                radius: 6,
                background: theme.c.surface,
              ),
            ],
            if (showStatus) ...[
              const SizedBox(width: 8),
              Text(
                statusText,
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  // 2xx 绿、其余一律红（`primitives.tsx:131-133`）。
                  color: statusCode >= 200 && statusCode < 300
                      ? theme.c.ok
                      : theme.c.bad,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }

  /// 元信息一列（React `MetaItem`）：label 12 tertiary 上、值 15 w600 下，
  /// 复制按钮随值同行。[color] 用于状态码那格的绿 / 红。
  Widget _kv(
    AidogTheme theme,
    String k,
    String v, {
    String? copyText,
    Color? color,
  }) {
    final value = v.isEmpty ? '-' : v;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: 2),
          child: Text(
            k,
            style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg3),
          ),
        ),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Text(
                value,
                style: AidogType.body.copyWith(
                  fontWeight: FontWeight.w600,
                  color: color ?? theme.c.fg,
                ),
              ),
            ),
            if (copyText != null && copyText.isNotEmpty)
              SizedBox(
                // React 给复制按钮留 24（`primitives.tsx:79`）
                width: 24,
                child: IconButton(
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(),
                  iconSize: 12,
                  color: theme.c.fg3,
                  icon: const Icon(Icons.copy_outlined),
                  onPressed: () => widget.onCopy(copyText),
                ),
              ),
          ],
        ),
      ],
    );
  }

  /// 元信息区网格：`minmax(160px, 1fr)` gap 14 的 auto-fill 等价实现，
  /// 外面是 `.glass-surface hover-lift` + padding 20（`DetailPanel.tsx:170`）。
  Widget _metaGrid(AidogTheme theme, List<Widget> items) => HoverLift(
    child: Tile(
      padding: const EdgeInsets.all(20),
      child: LayoutBuilder(
        builder: (context, cons) {
          final cols = (((cons.maxWidth + 14) / (160 + 14)).floor()).clamp(
            1,
            items.length,
          );
          final w = (cons.maxWidth - (cols - 1) * 14) / cols;
          return Wrap(
            spacing: 14,
            runSpacing: 14,
            children: [for (final it in items) SizedBox(width: w, child: it)],
          );
        },
      ),
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
    String body, {

    /// 空块的占位说明。响应正文为空的原因和请求正文不一样 —— 流式响应本来就
    /// 不落正文，写「未捕获」会让人以为日志坏了（`DetailPanel.tsx:100-107`）。
    String? emptyText,

    /// URL 块在 React 是 `{url && (...)}`：为空时整块不出现
    /// （`primitives.tsx:178`），不画「未捕获」占位框。
    bool hideWhenEmpty = false,
  }) {
    final empty = body.trim().isEmpty;
    if (empty && hideWhenEmpty) return const SizedBox.shrink();
    final text = empty
        ? (emptyText ?? t.t('logs.noUpstream'))
        : prettyJsonOrRaw(body);
    final baseStyle = TextStyle(
      fontFamily: AidogType.familyMono,
      fontFamilyFallback: AidogType.familyMonoFallback,
      fontSize: 12,
      height: 1.7,
      color: empty ? theme.c.fg3 : theme.c.fg,
    );
    return Padding(
      // 容器 gap 12（`primitives.tsx:136`）
      padding: const EdgeInsets.only(top: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          // React 段落小标题：12 w600 secondary（primitives.tsx:180-183）
          Text(
            title,
            style: AidogType.caption.copyWith(
              fontSize: 12,
              fontWeight: FontWeight.w600,
              color: theme.c.fg2,
            ),
          ),
          const SizedBox(height: 4),
          // React `.code-block`（globals.css:565-576）：mono 12 lh1.7、
          // padding 14/16、底 bg-base、1px 边、radius 8；编辑器 minHeight 60。
          // 复制按钮浮在块**右上角**（`primitives.tsx:36-37`），不排在标题行里。
          Stack(
            children: [
              Container(
                width: double.infinity,
                constraints: const BoxConstraints(minHeight: 60),
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 14,
                ),
                decoration: BoxDecoration(
                  color: theme.c.bg,
                  border: Border.all(color: theme.c.line),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: SelectableText.rich(
                  // headers / body 在 React 走 `JsonCodeEditor`（带语法高亮，
                  // `primitives.tsx:192,203,215,224`）；Flutter 没有那个渲染器，
                  // 这里至少把 key / 字符串 / 数字 / 字面量分色。URL 块不高亮
                  // （React 那块用的是纯 `.code-block`，`:182`）。
                  empty || title == 'URL'
                      ? TextSpan(text: text, style: baseStyle)
                      : jsonSpan(text, baseStyle, theme.c),
                  style: baseStyle,
                ),
              ),
              // 空块没什么可复制的，不画按钮（React：占位串不给复制）。
              if (!empty)
                Positioned(
                  top: 4,
                  right: 4,
                  child: _CopyOverlayBtn(
                    key: ValueKey('sec-copy-$title-${body.hashCode}'),
                    tooltip: t.t('logs.copy'),
                    onTap: () => widget.onCopy(text),
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}
