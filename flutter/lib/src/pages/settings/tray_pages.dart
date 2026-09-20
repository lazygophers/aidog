/// 托盘与浮窗两页的 widget 层：
/// - `settings/tray`    → `src/pages/TrayConfigTab.tsx`
/// - `settings/popover` → `src/pages/PopoverConfigTab/`
///
/// 两页都是**即时保存**：每次增删改整份写回，没有保存按钮 → 没有脏状态 →
/// 不需要离页守卫（与 React 一致）。order 每次按下标重排，不这么做拖拽后
/// 顺序在后端就是乱的（`tray_logic.dart::withOrders`）。
///
/// **与 React 的差异**：拖拽排序换成「上移 / 下移」两个按钮。Flutter 的
/// `ReorderableListView` 要求自己管滚动，嵌在 Bento 页里会与外层滚动打架；
/// 上下移按钮在键盘 / 读屏下反而更好用。列在 flutter/README.md 的差异清单里。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../../popover.dart';
import '../../shell/theme.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
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
          (i) =>
              !i.enabled &&
              !options.any((o) => o.key == traySegmentKey(i)),
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
    final theme = AidogTheme.of(context);
    if (selected.isEmpty) {
      return SettingsCard(
        title: t.t('tray.preview'),
        children: [CenteredNote(text: t.t('tray.previewEmpty'))],
      );
    }
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
        Text(
          ltr(parts.join(_c.separator)),
          style: AidogType.numSm.copyWith(color: theme.c.fg),
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

  List<Map<String, Object?>> get _items =>
      (_c.config['items'] as List? ?? const [])
          .whereType<Map>()
          .map(Map<String, Object?>.from)
          .toList();

  /// 改完配置顺带重拉预览：新加的卡片要有自己的统计结果，否则永远停在加载态。
  Future<void> _persistItems(List<Map<String, Object?>> next) async {
    await _c.persist({
      ..._c.config,
      'items': PopoverController.withOrders(next),
    });
    await _c.loadPreview();
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
    final items = _items;
    return SettingsPageBody(
      title: t.t('popover.title'),
      subtitle: '${items.length}',
      children: [
        SettingsCard(
          description: t.t('popover.descGrid'),
          children: [
            Text(
              t.t('popover.layoutHint'),
              style: AidogType.micro.copyWith(
                color: AidogTheme.of(context).c.fg3,
              ),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('popover.addItem'),
          children: [
            ChoiceRow(
              key: const ValueKey('popover-add'),
              label: t.t('popover.addItem'),
              options: kPopoverItemTypes
                  // 单实例类型加过一次就不再出现在菜单里。
                  .where(
                    (ty) =>
                        kPopoverMultiInstanceTypes.contains(ty) ||
                        !items.any((e) => e['item_type'] == ty),
                  )
                  .toList(),
              value: '',
              labelOf: _typeLabel(t),
              onChanged: (ty) => _persistItems([
                ...items,
                {
                  'id': '$ty-${DateTime.now().microsecondsSinceEpoch}',
                  'item_type': ty,
                  'visible': true,
                  'order': items.length,
                  'row': items.length,
                  'size': 'm',
                  'scope': kPopoverGroupTypes.contains(ty)
                      ? 'group'
                      : 'overall',
                  'time_window': '7d',
                  'color': {'mode': 'follow', 'value': ''},
                },
              ]),
            ),
          ],
        ),
        if (items.isEmpty)
          CenteredNote(text: t.t('popover.empty'))
        else
          for (var i = 0; i < items.length; i++) _itemCard(t, items, i),
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

  Widget _itemCard(
    I18nController t,
    List<Map<String, Object?>> items,
    int i,
  ) {
    final it = items[i];
    final ty = '${it['item_type']}';
    final scope = '${it['scope'] ?? 'overall'}';
    List<Map<String, Object?>> patch(Map<String, Object?> p) {
      final next = [...items];
      next[i] = {...it, ...p};
      return next;
    }

    return SettingsCard(
      key: ValueKey('popover-item-$i'),
      title: _typeLabel(t)(ty),
      meta: ltr('#${i + 1}'),
      children: [
        Row(
          children: [
            SmallButton(
              label: t.t('action.moveUp'),
              onTap: i == 0
                  ? null
                  : () {
                      final next = [...items];
                      next.insert(i - 1, next.removeAt(i));
                      _persistItems(next);
                    },
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              label: t.t('action.moveDown'),
              onTap: i == items.length - 1
                  ? null
                  : () {
                      final next = [...items];
                      next.insert(i + 1, next.removeAt(i));
                      _persistItems(next);
                    },
            ),
            const Spacer(),
            SmallButton(
              label: t.t('popover.toggleVisible'),
              active: it['visible'] != false,
              onTap: () =>
                  _persistItems(patch({'visible': it['visible'] == false})),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              label: t.t('action.delete'),
              danger: true,
              onTap: () => _persistItems([...items]..removeAt(i)),
            ),
          ],
        ),
        ChoiceRow(
          label: t.t('popover.size'),
          options: kPopoverSizes,
          value: '${it['size'] ?? 'm'}',
          labelOf: (s) => t.t('popover.size_$s'),
          onChanged: (v) => _persistItems(patch({'size': v})),
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
            onChanged: (v) =>
                _persistItems(patch({'scope': v, 'scope_ref': null})),
          ),
          ChoiceRow(
            label: t.t('popover.previewTrend', {'window': ''}),
            options: kPopoverTrendWindows,
            value: '${it['time_window'] ?? '7d'}',
            labelOf: (w) => t.t('popover.trendWindow_$w'),
            onChanged: (v) => _persistItems(patch({'time_window': v})),
          ),
        ],
        if (scope == 'group' || kPopoverGroupTypes.contains(ty))
          ChoiceRow(
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
                _persistItems(patch({'scope': 'group', 'scope_ref': v})),
          ),
        if (scope == 'platform' || ty == 'platform_metric')
          ChoiceRow(
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
                _persistItems(patch({'scope': 'platform', 'scope_ref': v})),
          ),
      ],
    );
  }

  /// 实时预览：与托盘小窗本体**共用 `PopoverGrid`**（React 的 `renderGrid` 同样被
  /// 两处调用），所见即所得，预览与实际不会漂移。
  Widget _previewCard(I18nController t) => SettingsCard(
    title: t.t('popover.preview'),
    description: t.t('popover.previewHint'),
    children: [
      ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: kTrayPanelWidth),
        child: PopoverGrid(frame: _c.previewFrame),
      ),
    ],
  );
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}
