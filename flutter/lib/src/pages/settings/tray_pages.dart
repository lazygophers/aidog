/// 浮窗两页的 widget 层：
/// - `settings/tray`    → `src/pages/TrayConfigTab.tsx`
/// - `settings/popover` → `src/pages/PopoverConfigTab/`
///
/// 两页都是**即时保存**：每次增删改整份写回，没有保存按钮 → 没有脏状态 →
/// 不需要离页守卫（与 React 一致）。order / row 每次写回前经
/// `popover_layout.dart::normalizePopoverConfig` 规整，拖拽后顺序在后端才不会乱。
///
/// 浮窗页是**二维布局编辑器**（I17，对齐 React 的 PopoverLayout + dnd-kit）：
/// 卡片长按拖拽（行内换位 / 跨行搬移，`movePopoverItemToRow` 同语义），
/// 每行可设 1-3 列，新添加的项落到新的一行。实时预览与托盘小窗本体共用
/// `PopoverGrid`，所见即所得。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../../popover.dart';
import '../../../utils/formatters.dart';
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'popover_layout.dart';
import 'popover_logic.dart';
import 'tray_logic.dart';

// ── 托盘 ──────────────────────────────────────────────────

class TraySettingsPage extends StatefulWidget {
  const TraySettingsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.logUpdates,
  });

  final InvokeFn invoke;

  /// 「有新请求日志」流。React 那边 `TrayConfigTab.tsx:208` 显式传 1000 ms 防抖。
  final Stream<void>? logUpdates;

  @override
  State<TraySettingsPage> createState() => _TraySettingsPageState();
}

class _TraySettingsPageState extends State<TraySettingsPage> {
  late final TrayController _c;
  StreamSubscription<void>? _sub;

  @override
  void initState() {
    super.initState();
    _c = TrayController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    unawaited(_c.load());
    _sub =
        (widget.logUpdates ??
                debounceStream(
                  kernelProxyLogUpdated(),
                  delay: const Duration(milliseconds: 1000),
                ))
            .listen((_) => _c.refreshStats());
  }

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    if (_c.loading) {
      return SettingsPageBody(
        title: t.t('appSettings.trayTab'),
        children: [CenteredNote(text: t.t('status.loading'))],
      );
    }
    final selected = _c.selected;
    final options = traySegmentOptions(
      _c.platforms,
      (key, fallback) => tOr(t, key, fallback),
    );
    final selectedKeys = selected.map(traySegmentKey).toSet();
    final full = selected.length >= kTrayMaxSegments;
    // 迁移留下的项：关着、且不在候选清单里（旧 separator / 已删平台）。
    final keptDisabled = _c.items
        .where(
          (i) => !i.enabled && !options.any((o) => o.key == traySegmentKey(i)),
        )
        .length;
    return SettingsPageBody(
      title: t.t('appSettings.trayTab'),
      subtitle: ltr('${selected.length}/$kTrayMaxSegments'),
      children: [
        _preview(t, selected),
        SettingsCard(
          title: tOr(t, 'tray.pickAtMost', '最多挑 3 项，菜单栏按勾选顺序显示'),
          children: [
            Wrap(
              spacing: AidogSpace.sxs,
              runSpacing: AidogSpace.sxs,
              children: [
                for (final opt in options)
                  SmallButton(
                    key: ValueKey('tray-seg-${opt.key}'),
                    label: opt.label,
                    active: selectedKeys.contains(opt.key),
                    onTap: !selectedKeys.contains(opt.key) && full
                        ? null
                        : () => _c.toggleSegment(opt),
                  ),
              ],
            ),
          ],
        ),
        if (keptDisabled > 0)
          CenteredNote(
            text: tOr(
              t,
              'tray.keptDisabled',
              '旧版托盘配置里多出的 $keptDisabled 项已保留但未启用，不会丢失',
            ),
          ),
        if (_c.message.isNotEmpty)
          AutoToast(
            text: _c.message,
            ok: false,
            onDone: () => setState(() => _c.message = ''),
          ),
      ],
    );
  }

  Widget _preview(I18nController t, List<TrayItem> selected) {
    // 空态不再早退：深色条本身要在，里面换一句斜体「暂无展示项」——
    // React 同样是「条常驻、内容换文案」（`TrayConfigTab.tsx:216-219`）。
    final parts = <String>[];
    for (final it in selected) {
      final p = _c.platforms.where((p) => p.id == it.platformId).firstOrNull;
      final text = computeItemText(
        it,
        p == null
            ? null
            : (name: p.name, balance: p.balance, codingPlan: p.codingPlan),
        _c.todayStats,
        (key, fallback) => tOr(t, key, fallback),
      );
      parts.add('${text.label} ${text.value}');
    }
    return SettingsCard(
      title: t.t('tray.preview'),
      children: [
        // 深色圆角条模拟 macOS 菜单栏外观（`TrayConfigTab.tsx:209-218`）。
        // 这几个色值是**菜单栏模拟色，不跟随 app 主题**，所以写死不取 token。
        Container(
          constraints: const BoxConstraints(minHeight: 26),
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 2),
          alignment: AlignmentDirectional.centerStart,
          decoration: BoxDecoration(
            color: const Color(0xF21E1E1E), // rgba(30,30,30,.95)
            borderRadius: BorderRadius.circular(8),
          ),
          child: parts.isEmpty
              // 空时原先拼出一串空字符串，看着像渲染坏了。
              ? Text(
                  t.t('tray.previewEmpty'),
                  style: AidogType.label.copyWith(
                    color: const Color(0x59FFFFFF), // rgba(255,255,255,.35)
                    fontStyle: FontStyle.italic,
                  ),
                )
              : Text(
                  ltr(parts.join(_c.separator)),
                  style: AidogType.numSm.copyWith(
                    color: const Color(0xD9FFFFFF), // rgba(255,255,255,.85)
                    fontWeight: FontWeight.w600,
                  ),
                ),
        ),
      ],
    );
  }
}

// ── 浮窗 ──────────────────────────────────────────────────

/// 预定义指标集（顺序即添加菜单顺序）—— 镜像
/// `src/pages/PopoverConfigTab/constants.ts::ALL_ITEM_TYPES`。
const List<String> kPopoverItemTypes = [
  'proxy_status',
  'platform_balance',
  'today_cost',
  'today_cache_rate',
  'today_tokens',
  'platform_today',
  'platform_metric',
  'group_cost',
  'group_tokens',
  'group_requests',
  'group_balance',
  'cost_trend',
  'platform_share',
  'hour_heatbar',
];

/// 指标类型 → i18n key + 默认中文标签（镜像 `TYPE_LABELS`）。
const Map<String, (String, String)> kPopoverTypeLabels = {
  'proxy_status': ('popover.itemProxyStatus', '代理状态'),
  'platform_balance': ('popover.itemPlatformBalance', '平台余额/配额'),
  'today_cost': ('popover.todayCost', '今日金额'),
  'today_cache_rate': ('popover.todayCacheRate', '今日缓存率'),
  'today_tokens': ('popover.todayTokens', '今日 Token'),
  'platform_today': ('popover.platformToday', '各平台今日'),
  'platform_metric': ('popover.itemPlatformMetric', '指定平台指标'),
  'group_cost': ('popover.itemGroupCost', '分组金额'),
  'group_tokens': ('popover.itemGroupTokens', '分组今日Token'),
  'group_requests': ('popover.itemGroupRequests', '分组今日请求'),
  'group_balance': ('popover.itemGroupBalance', '分组余额'),
  'cost_trend': ('popover.itemCostTrend', '消费趋势'),
  'platform_share': ('popover.itemPlatformShare', '平台占比'),
  'hour_heatbar': ('popover.itemHourHeat', '今日热力'),
};

/// 可重复添加的多实例类型（镜像 `MULTI_INSTANCE_TYPES`）。
const Set<String> kPopoverMultiInstanceTypes = {
  'cost_trend',
  'platform_share',
  'hour_heatbar',
  'platform_metric',
  'group_cost',
  'group_tokens',
  'group_requests',
  'group_balance',
};

/// group_* 系列：scope 锁 group（镜像 `GROUP_TYPES`）。
const Set<String> kPopoverGroupTypes = {
  'group_cost',
  'group_tokens',
  'group_requests',
  'group_balance',
};

const List<String> kPopoverTrendWindows = ['today', '7d', '30d'];
const List<String> kPopoverSizes = ['s', 'm', 'l'];

class PopoverSettingsPage extends StatefulWidget {
  const PopoverSettingsPage({super.key, this.invoke = kernelInvoke});

  final InvokeFn invoke;

  @override
  State<PopoverSettingsPage> createState() => _PopoverSettingsPageState();
}

class _PopoverSettingsPageState extends State<PopoverSettingsPage> {
  late final PopoverController _c;

  @override
  void initState() {
    super.initState();
    _c = PopoverController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    // 预览的统计查询按 config 算，所以要等 load 先把 config 读回来。
    unawaited(_c.load().then((_) => _c.loadPreview()));
    unawaited(_c.loadPickers());
  }

  /// 规整后的编辑视图（React 在 load 后 `setConfig(normalizeConfig(...))`，
  /// 这里每次 build 派生 —— 幂等、且不为展示而写后端）。行号连续、行内有序。
  Map<String, Object?> get _norm {
    final items = (_c.config['items'] as List? ?? const [])
        .whereType<Map>()
        .map(Map<String, Object?>.from)
        .toList();
    final rows = (_c.config['rows'] as List? ?? const [])
        .whereType<Map>()
        .map(Map<String, Object?>.from)
        .toList();
    return normalizePopoverConfig(items, rows);
  }

  List<Map<String, Object?>> _itemsOf(Map<String, Object?> cfg) =>
      ((cfg['items'] as List? ?? const []))
          .whereType<Map>()
          .map(Map<String, Object?>.from)
          .toList();

  /// 改完配置顺带重拉预览：新加的卡片要有自己的统计结果，否则永远停在加载态。
  /// 写回前先规整（row/order/rows 对齐），再合进整份 config —— 只带 items/rows
  /// 会把 config 里其它顶层键清掉。
  Future<void> _persistRaw(
    List<Map<String, Object?>> items,
    List<Map<String, Object?>> rows,
  ) async {
    await _c.persist({..._c.config, ...normalizePopoverConfig(items, rows)});
    await _c.loadPreview();
  }

  void _updateItem(String id, Map<String, Object?> patch) {
    final items = _itemsOf(_norm)
        .map((it) => '${it['id']}' == id ? {...it, ...patch} : it)
        .toList();
    _persistRaw(items, _rowsOf(_norm));
  }

  void _removeItem(String id) {
    _persistRaw(
      _itemsOf(_norm).where((it) => '${it['id']}' != id).toList(),
      _rowsOf(_norm),
    );
  }

  void _setRowCols(int row, int cols) {
    _persistRaw(_itemsOf(_norm), [
      for (var r = 0; r < _rowGroups.length; r++)
        {'cols': r == row ? cols : _colsOf(r)},
    ]);
  }

  List<Map<String, Object?>> _rowsOf(Map<String, Object?> cfg) =>
      ((cfg['rows'] as List? ?? const []))
          .whereType<Map>()
          .map(Map<String, Object?>.from)
          .toList();

  int _colsOf(int row) {
    final rows = _rowsOf(_norm);
    return row < rows.length ? ((rows[row]['cols'] as num?)?.toInt() ?? 1) : 1;
  }

  /// 行分组视图（编辑器用）：含隐藏卡（显隐是卡内开关，不是从布局里消失）。
  List<List<Map<String, Object?>>> get _rowGroups {
    final byRow = <int, List<Map<String, Object?>>>{};
    for (final it in _itemsOf(_norm)) {
      (byRow[effRow(it)] ??= []).add(it);
    }
    final nums = byRow.keys.toList()..sort();
    return [
      for (final n in nums)
        byRow[n]!..sort(
          (a, b) => ((a['order'] as num?)?.toInt() ?? 0).compareTo(
            (b['order'] as num?)?.toInt() ?? 0,
          ),
        ),
    ];
  }

  /// 跨行 / 行内搬移（`usePopoverConfig.ts::moveItemToRow` 同语义）。
  void _moveItem(String activeId, int targetRow, String? beforeId) {
    _persistRaw(
      movePopoverItemToRow(_itemsOf(_norm), activeId, targetRow, beforeId),
      _rowsOf(_norm),
    );
  }

  /// 新项追加到新的一行（行号 = 当前最大行 + 1，React `addItem` 同规则）。
  void _addItem(String ty) {
    final nextRow = _rowGroups.length;
    final item = makePopoverItem(
      ty,
      0,
      nextRow,
      platforms: _c.platforms,
      groups: _c.groups,
    );
    _persistRaw(
      [..._itemsOf(_norm), item],
      [
        ..._rowsOf(_norm),
        {'cols': 1},
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    if (_c.loading) {
      return SettingsPageBody(
        title: t.t('popover.title'),
        children: [CenteredNote(text: t.t('status.loading'))],
      );
    }
    final groups = _rowGroups;
    final items = _itemsOf(_norm);
    return SettingsPageBody(
      title: t.t('popover.title'),
      subtitle: '${items.length}',
      children: [
        SettingsCard(
          description: t.t('popover.descGrid'),
          children: [
            // 说明改成按钮，点了才把那段提示发成 toast
            //（`PopoverLayout.tsx:169-175` + `usePopoverConfig.ts:202-204`）。
            // 原先常驻一行小字，功能没丢但一直占屏。
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: SmallButton(
                key: const ValueKey('popover-layout-hint'),
                label: t.t('popover.rowHintBtn'),
                ghost: true,
                onTap: () =>
                    setState(() => _c.message = t.t('popover.layoutHint')),
              ),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('popover.items'),
          children: [
            // 一颗「添加项」按钮 + 点开的菜单（`PopoverLayout.tsx:85-107`）。
            // 原先是把 14 个类型全平铺成一排按钮，占掉整张卡。
            // 单实例类型加过一次就不再出现在菜单里。
            _AddItemMenu(
              key: const ValueKey('popover-add'),
              label: t.t('popover.addItem'),
              types: kPopoverItemTypes
                  .where(
                    (ty) =>
                        kPopoverMultiInstanceTypes.contains(ty) ||
                        !items.any((e) => e['item_type'] == ty),
                  )
                  .toList(),
              labelOf: _typeLabel(t),
              onPick: _addItem,
            ),
          ],
        ),
        if (groups.isEmpty)
          CenteredNote(text: t.t('popover.empty'))
        else
          for (var r = 0; r < groups.length; r++) _rowEditor(t, r, groups[r]),
        _previewCard(t),
        if (_c.message.isNotEmpty)
          AutoToast(
            text: _c.message,
            ok: false,
            onDone: () => setState(() => _c.message = ''),
          ),
      ],
    );
  }

  String Function(String) _typeLabel(I18nController t) => (ty) {
    final e = kPopoverTypeLabels[ty];
    return e == null ? ty : tOr(t, e.$1, e.$2);
  };

  /// 一行：行号 + 列数选择 + 卡片格。行容器本身是落点（拖到行内空白处 = 追加到行尾）。
  Widget _rowEditor(
    I18nController t,
    int row,
    List<Map<String, Object?>> items,
  ) {
    final theme = AidogTheme.of(context);
    final cols = _colsOf(row);
    return Padding(
      key: ValueKey('popover-row-$row'),
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: DragTarget<String>(
        onWillAcceptWithDetails: (details) => details.data.isNotEmpty,
        onAcceptWithDetails: (details) => _moveItem(details.data, row, null),
        builder: (context, candidate, rejected) => Container(
          padding: const EdgeInsets.all(AidogSpace.ssm),
          decoration: BoxDecoration(
            border: Border.all(
              color: candidate.isNotEmpty ? theme.c.accent : theme.c.line,
              width: candidate.isNotEmpty ? 1.5 : 1,
            ),
            borderRadius: BorderRadius.circular(AidogRadius.md),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              Row(
                children: [
                  Text(
                    ltr(t.t('popover.rowLabel', {'n': row + 1})),
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                  Text(
                    t.t('popover.cols'),
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  for (var c = 1; c <= kPopoverMaxCols; c++)
                    Padding(
                      padding: const EdgeInsets.only(left: AidogSpace.sxs),
                      child: SmallButton(
                        key: ValueKey('popover-cols-$row-$c'),
                        label: '$c',
                        active: cols == c,
                        onTap: () => _setRowCols(row, c),
                      ),
                    ),
                ],
              ),
              const SizedBox(height: AidogSpace.ssm),
              LayoutBuilder(
                builder: (context, constraints) {
                  const gap = AidogSpace.ssm;
                  final w = (constraints.maxWidth - (cols - 1) * gap) / cols;
                  return Wrap(
                    spacing: gap,
                    runSpacing: gap,
                    children: [
                      for (final it in items)
                        SizedBox(width: w, child: _card(t, it)),
                    ],
                  );
                },
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// 单张卡的编辑体：拖拽源 + 落点（落在另一张卡上 = 插到它前面）。
  /// 内容对齐 React `CardEditor.tsx`：标题 + 预览摘要 / 显隐 / 删除 / scope /
  /// 尺寸 / 颜色预设 + 自定义 hex。
  Widget _card(I18nController t, Map<String, Object?> it) {
    final theme = AidogTheme.of(context);
    final id = '${it['id']}';
    final ty = '${it['item_type']}';
    final color = it['color'] is Map
        ? Map<String, Object?>.from(it['color'] as Map)
        : {'mode': 'follow', 'value': ''};
    return LongPressDraggable<String>(
      data: id,
      delay: const Duration(milliseconds: 250),
      feedback: Container(
        padding: const EdgeInsets.symmetric(
          horizontal: AidogSpace.smd,
          vertical: AidogSpace.ssm,
        ),
        decoration: BoxDecoration(
          color: theme.c.accentWash,
          border: Border.all(color: theme.c.accent),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
          boxShadow: theme.shadowFloat,
        ),
        child: Text(
          _typeLabel(t)(ty),
          style: AidogType.label.copyWith(color: theme.c.accent),
        ),
      ),
      childWhenDragging: Opacity(opacity: 0.4, child: _cardBody(t, it, color)),
      child: DragTarget<String>(
        onWillAcceptWithDetails: (details) => details.data != id,
        onAcceptWithDetails: (details) =>
            _moveItem(details.data, _rowOf(id), id),
        builder: (context, candidate, rejected) => Container(
          padding: const EdgeInsets.all(AidogSpace.ssm),
          decoration: BoxDecoration(
            color: theme.c.surface2,
            border: Border.all(
              color: candidate.isNotEmpty ? theme.c.accent : theme.c.line,
            ),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: _cardBody(t, it, color),
        ),
      ),
    );
  }

  /// id 所在的行号（编辑视图）。注意不能用 `indexOf` —— List 按身份比较，
  /// `_rowGroups` 每次 getter 都是新实例，永远查不到（本项目踩过的同类坑：
  /// Dart 的 Set / List / Map 都不按值比较）。
  int _rowOf(String id) {
    final groups = _rowGroups;
    for (var r = 0; r < groups.length; r++) {
      if (groups[r].any((it) => '${it['id']}' == id)) return r;
    }
    return 0;
  }

  Widget _cardBody(
    I18nController t,
    Map<String, Object?> it,
    Map<String, Object?> color,
  ) {
    final theme = AidogTheme.of(context);
    final id = '${it['id']}';
    final ty = '${it['item_type']}';
    final scope = '${it['scope'] ?? 'overall'}';
    void patch(Map<String, Object?> p) => _updateItem(id, p);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    _typeLabel(t)(ty),
                    style: AidogType.label.copyWith(color: theme.c.fg),
                  ),
                  Text(
                    _summary(t, it),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                ],
              ),
            ),
            SmallButton(
              key: ValueKey('popover-visible-$id'),
              label: t.t('popover.toggleVisible'),
              active: it['visible'] != false,
              onTap: () => patch({'visible': it['visible'] == false}),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              key: ValueKey('popover-del-$id'),
              label: t.t('action.delete'),
              danger: true,
              onTap: () => _removeItem(id),
            ),
          ],
        ),
        const SizedBox(height: AidogSpace.sxs),
        // 尺寸（S / M / L）
        Row(
          children: [
            Text(
              t.t('popover.size'),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
            const SizedBox(width: AidogSpace.sxs),
            for (final s in kPopoverSizes)
              Padding(
                padding: const EdgeInsets.only(left: AidogSpace.sxs),
                child: SmallButton(
                  key: ValueKey('popover-size-$id-$s'),
                  label: s.toUpperCase(),
                  active: '${it['size'] ?? 'm'}' == s,
                  onTap: () => patch({'size': s}),
                ),
              ),
          ],
        ),
        // 颜色：跟随 + 三预设 + 自定义 hex（色值走主题 token，与渲染层同映射）。
        Row(
          children: [
            Text(
              t.t('popover.color'),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
            const SizedBox(width: AidogSpace.sxs),
            _colorDot(
              id,
              selected: color['mode'] == 'follow',
              fill: null,
              tooltip: t.t('popover.colorFollow'),
              onTap: () => patch({
                'color': {'mode': 'follow', 'value': ''},
              }),
            ),
            _colorDot(
              id,
              selected: color['mode'] == 'preset' && color['value'] == 'red',
              fill: theme.c.bad,
              tooltip: 'red',
              onTap: () => patch({
                'color': {'mode': 'preset', 'value': 'red'},
              }),
            ),
            _colorDot(
              id,
              selected: color['mode'] == 'preset' && color['value'] == 'green',
              fill: theme.c.ok,
              tooltip: 'green',
              onTap: () => patch({
                'color': {'mode': 'preset', 'value': 'green'},
              }),
            ),
            _colorDot(
              id,
              selected: color['mode'] == 'preset' && color['value'] == 'orange',
              fill: theme.c.peak,
              tooltip: 'orange',
              onTap: () => patch({
                'color': {'mode': 'preset', 'value': 'orange'},
              }),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SizedBox(
              width: 90,
              child: _HexField(
                key: ValueKey('popover-hex-$id'),
                value: color['mode'] == 'custom' ? '${color['value']}' : '',
                active: color['mode'] == 'custom',
                onChanged: (hex) => patch({
                  'color': {'mode': 'custom', 'value': hex},
                }),
              ),
            ),
          ],
        ),
        if (ty == 'cost_trend') ...[
          ChoiceRow(
            label: t.t('popover.trendScopeOverall'),
            options: const ['overall', 'group', 'platform'],
            value: scope,
            labelOf: (s) => switch (s) {
              'group' => t.t('popover.trendScopeGroup'),
              'platform' => t.t('popover.trendScopePlatform'),
              _ => t.t('popover.trendScopeOverall'),
            },
            // 切到 platform / group 就预填第一个（`ScopeConfig.tsx:29-37`）：
            // 原先一律清成 null，切完这张卡是空的，还得再点一次选具体对象。
            onChanged: (v) => patch({
              'scope': v,
              'scope_ref': switch (v) {
                'platform' =>
                  _c.platforms.isEmpty ? null : '${_c.platforms.first.id}',
                'group' => _c.groups.isEmpty ? null : _c.groups.first.groupKey,
                _ => null,
              },
            }),
          ),
          ChoiceRow(
            label: t.t('popover.previewTrend', {'window': ''}),
            options: kPopoverTrendWindows,
            value: '${it['time_window'] ?? '7d'}',
            labelOf: (w) => t.t('popover.trendWindow_$w'),
            onChanged: (v) => patch({'time_window': v}),
          ),
        ],
        if (ty == 'platform_share' || ty == 'hour_heatbar')
          ChoiceRow(
            label: t.t('popover.previewTrend', {'window': ''}),
            options: kPopoverTrendWindows,
            value:
                '${it['time_window'] ?? (ty == 'hour_heatbar' ? 'today' : '7d')}',
            labelOf: (w) => t.t('popover.trendWindow_$w'),
            onChanged: (v) => patch({'time_window': v}),
          ),
        // 分组 / 平台候选是**不定长**的：平铺成一排按钮，平台一多就铺满屏
        // （React 这两处是 Select，`ScopeConfig.tsx:83-114`）。
        // 候选为空时 ChoiceRow 整行什么都不画，这里给一句空态。
        if (scope == 'group' || kPopoverGroupTypes.contains(ty))
          _c.groups.isEmpty
              ? _ScopeEmptyNote(label: t.t('popover.trendScopeGroup'))
              : SelectRow(
                  key: const ValueKey('popover-scope-group'),
                  label: t.t('popover.trendScopeGroup'),
                  options: _c.groups.map((g) => g.groupKey).toList(),
                  value: '${it['scope_ref'] ?? ''}',
                  labelOf: (k) => _c.groups
                      .firstWhere(
                        (g) => g.groupKey == k,
                        orElse: () => (id: -1, name: k, groupKey: k),
                      )
                      .name,
                  onChanged: (v) =>
                      patch({'scope': 'group', 'scope_ref': v ?? ''}),
                ),
        if (scope == 'platform' || ty == 'platform_metric')
          _c.platforms.isEmpty
              ? _ScopeEmptyNote(label: t.t('popover.trendScopePlatform'))
              : SelectRow(
                  key: const ValueKey('popover-scope-platform'),
                  label: t.t('popover.trendScopePlatform'),
                  options: _c.platforms.map((p) => '${p.id}').toList(),
                  value: '${it['scope_ref'] ?? ''}',
                  labelOf: (id) => _c.platforms
                      .firstWhere(
                        (p) => '${p.id}' == id,
                        orElse: () => (id: -1, name: id),
                      )
                      .name,
                  onChanged: (v) =>
                      patch({'scope': 'platform', 'scope_ref': v ?? ''}),
                ),
      ],
    );
  }

  Widget _colorDot(
    String id, {
    required bool selected,
    required Color? fill,
    required String tooltip,
    required VoidCallback onTap,
  }) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(left: AidogSpace.sxs),
      child: InkWell(
        key: ValueKey('popover-color-$id-$tooltip'),
        onTap: onTap,
        customBorder: const CircleBorder(),
        child: Container(
          width: 16,
          height: 16,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: fill,
            border: Border.all(
              color: selected ? theme.c.accent : theme.c.line,
              width: selected ? 2 : 1,
            ),
          ),
        ),
      ),
    );
  }

  /// 卡片标题下的预览摘要（React `previewValue` / `trendSummary` 同口径）。
  String _summary(I18nController t, Map<String, Object?> it) {
    final ty = '${it['item_type']}';
    if (ty == 'cost_trend' ||
        ty == 'platform_metric' ||
        kPopoverGroupTypes.contains(ty)) {
      return _trendSummary(t, it);
    }
    switch (ty) {
      case 'proxy_status':
        return t.t('popover.previewStatusLine');
      case 'platform_balance':
        return t.t('popover.previewTrayCols');
      case 'today_cost':
        return formatCostUsd(
          ((_c.trayToday['cost'] as num?) ??
                      _numAt(_c.popoverData, 'today_stats', 'cost'))
                  ?.toDouble() ??
              0,
        );
      case 'today_cache_rate':
        final v =
            (_c.trayToday['cache_rate'] as num?) ??
            _numAt(_c.popoverData, 'today_stats', 'cache_rate') ??
            0;
        return formatPercent(v.toDouble(), 0);
      case 'today_tokens':
        final v =
            (_c.trayToday['tokens'] as num?) ??
            _numAt(_c.popoverData, 'today_stats', 'tokens') ??
            0;
        return '${formatNumber(v)} tok';
      case 'platform_today':
        return _c.platformToday.isEmpty
            ? t.t('popover.noUsageToday')
            : t.t('popover.previewPlatformCount', {
                'count': _c.platformToday.length,
              });
      default:
        final e = kPopoverTypeLabels[ty];
        return e == null ? ty : tOr(t, e.$1, e.$2);
    }
  }

  static num? _numAt(Map<String, Object?> map, String k1, String k2) =>
      (map[k1] is Map) ? (map[k1] as Map)[k2] as num? : null;

  /// `usePopoverConfig.ts::trendSummary`：scope 名 + 时间窗（group_balance 只显示 scope）。
  String _trendSummary(I18nController t, Map<String, Object?> it) {
    final ty = '${it['item_type']}';
    final scope = '${it['scope'] ?? 'overall'}';
    String scopeLabel;
    if (scope == 'platform') {
      final p = _c.platforms
          .where((x) => '${x.id}' == '${it['scope_ref'] ?? ''}')
          .firstOrNull;
      scopeLabel = p?.name ?? t.t('popover.trendScopePlatform');
    } else if (scope == 'group') {
      final g = _c.groups
          .where((x) => x.groupKey == '${it['scope_ref'] ?? ''}')
          .firstOrNull;
      scopeLabel = g?.name ?? t.t('popover.trendScopeGroup');
    } else {
      scopeLabel = t.t('popover.trendScopeOverall');
    }
    if (ty == 'group_balance') return scopeLabel;
    final win = '${it['time_window'] ?? '7d'}';
    final winLabel = t.t('popover.trendWindow_$win');
    return '$scopeLabel · $winLabel';
  }

  /// 实时预览：与托盘小窗本体**共用 `PopoverGrid`**（React 的 `renderGrid` 同样被
  /// 两处调用），所见即所得，预览与实际不会漂移。
  Widget _previewCard(I18nController t) => SettingsCard(
    title: t.t('popover.preview'),
    description: t.t('popover.previewHint'),
    children: [
      ConstrainedBox(
        // 预览按小窗能长到的最宽来框（真窗按内容在 min..max 之间自适应，
        // 预览里没有那个测量循环，取上限即可）。
        constraints: const BoxConstraints(maxWidth: kTrayPanelMaxWidth),
        child: PopoverGrid(frame: _c.previewFrame),
      ),
    ],
  );
}

/// 自定义颜色 hex 输入（React `CustomHexInput`）：合法 6 位 hex（可带 #）即写回；
/// 非法时描边变红、不写。
class _HexField extends StatefulWidget {
  const _HexField({
    super.key,
    required this.value,
    required this.active,
    required this.onChanged,
  });

  final String value;
  final bool active;
  final ValueChanged<String> onChanged;

  @override
  State<_HexField> createState() => _HexFieldState();
}

class _HexFieldState extends State<_HexField> {
  late final TextEditingController _ctrl = TextEditingController(
    text: widget.value,
  );

  @override
  void didUpdateWidget(covariant _HexField old) {
    super.didUpdateWidget(old);
    if (widget.value != _ctrl.text) {
      _ctrl.value = TextEditingValue(
        text: widget.value,
        selection: TextSelection.collapsed(offset: widget.value.length),
      );
    }
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  static bool _validHex(String s) =>
      RegExp(r'^#?[0-9a-fA-F]{6}$').hasMatch(s.trim());

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final draft = _ctrl.text;
    final valid = draft.isEmpty || _validHex(draft);
    return TextField(
      controller: _ctrl,
      style: AidogType.micro.copyWith(color: theme.c.fg),
      decoration: InputDecoration(
        isDense: true,
        hintText: 'RRGGBB',
        hintStyle: AidogType.micro.copyWith(color: theme.c.fg3),
        contentPadding: const EdgeInsets.symmetric(
          horizontal: AidogSpace.sxs,
          vertical: AidogSpace.sxs,
        ),
        enabledBorder: OutlineInputBorder(
          borderSide: BorderSide(
            color: widget.active
                ? theme.c.accent
                : valid
                ? theme.c.line
                : theme.c.bad,
          ),
        ),
        focusedBorder: OutlineInputBorder(
          borderSide: BorderSide(color: valid ? theme.c.accent : theme.c.bad),
        ),
      ),
      onChanged: (v) {
        if (_validHex(v)) widget.onChanged(v.trim().replaceFirst('#', ''));
      },
    );
  }
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}

/// 分组 / 平台候选为空时的一行说明。原先候选为空整行什么都不画，
/// 用户只看到一张配好了「按分组」却挑不了分组的卡。
class _ScopeEmptyNote extends StatelessWidget {
  const _ScopeEmptyNote({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          TileMeta(label),
          Text(
            t.t('stats.noMatch'),
            style: AidogType.micro.copyWith(
              color: AidogTheme.of(context).c.fg3,
            ),
          ),
        ],
      ),
    );
  }
}

/// 「添加项」按钮：点开一列候选类型，选一个就加进布局。
/// 对齐 `PopoverLayout.tsx:85-107` 的按钮 + 浮层菜单；候选为空时按钮点不动。
class _AddItemMenu extends StatelessWidget {
  const _AddItemMenu({
    super.key,
    required this.label,
    required this.types,
    required this.labelOf,
    required this.onPick,
  });

  final String label;
  final List<String> types;
  final String Function(String) labelOf;
  final ValueChanged<String> onPick;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
    child: Align(
      alignment: AlignmentDirectional.centerStart,
      child: MenuAnchor(
        menuChildren: [
          for (final ty in types)
            MenuItemButton(
              key: ValueKey('popover-add-$ty'),
              onPressed: () => onPick(ty),
              child: Text(labelOf(ty)),
            ),
        ],
        builder: (context, controller, _) => SmallButton(
          label: '+ $label',
          onTap: types.isEmpty
              ? null
              : () =>
                    controller.isOpen ? controller.close() : controller.open(),
        ),
      ),
    ),
  );
}
