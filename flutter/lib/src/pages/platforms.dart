/// 平台页界面（票 I07），对应 `src/pages/Platforms.tsx`。
///
/// React 那边这一页同时装着「未分组平台列表」和内嵌的分组区（`GroupsEmbedded`），
/// 这里保持同一个结构：[PlatformsPage] 上半是分组区（[GroupsSection]，在 `groups.dart`），
/// 下半是未分组平台。**分组区是子 widget 不是兄弟页**，所以 React 那三个
/// `window` 自定义事件（`aidog-groups-changed` 等）在这里退化成普通回调 ——
/// 父子之间不需要事件总线。
///
/// 状态全在 [PlatformsController]（`platforms_logic.dart`）。色值一律走主题。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../utils/formatters.dart';
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'groups.dart';
import 'invoke.dart';
import 'models.dart';
import 'platforms_logic.dart';
import 'ui_bits.dart';

class PlatformsPage extends StatefulWidget {
  const PlatformsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.logUpdates,
    this.onNavigate,
    this.showGroups = true,
  });

  final InvokeFn invoke;

  /// 「有新请求日志」流；缺省 500ms 防抖。收到只做轻量统计刷新（不重拉整表）。
  final Stream<void>? logUpdates;

  /// 侧栏切页（平台卡「查看日志」用）。
  final void Function(String id, {int? platformId})? onNavigate;

  /// 关掉可以单独渲染「未分组平台」那一半，widget 测试用它把两页拆开测。
  final bool showGroups;

  @override
  State<PlatformsPage> createState() => _PlatformsPageState();
}

class _PlatformsPageState extends State<PlatformsPage> {
  late final PlatformsController _c;
  StreamSubscription<void>? _sub;
  ({String text, bool ok})? _toast;
  Timer? _toastTimer;

  @override
  void initState() {
    super.initState();
    _c = PlatformsController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
      onToast: _showToast,
    );
    _c.init().then((_) {
      // 平台列表到手之后再后台查余额 —— 查余额是真出网的 HTTP，不能挡首屏。
      if (mounted) _c.pumpQuota();
    });
    _sub = (widget.logUpdates ?? debounceStream(kernelProxyLogUpdated())).listen((
      _,
    ) {
      _c.refreshStats();
    });
  }

  @override
  void dispose() {
    _sub?.cancel();
    _toastTimer?.cancel();
    super.dispose();
  }

  void _showToast(String text, {required bool ok}) {
    if (!mounted) return;
    setState(() => _toast = (text: text, ok: ok));
    _toastTimer?.cancel();
    _toastTimer = Timer(const Duration(seconds: 3), () {
      if (mounted) setState(() => _toast = null);
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
          title: t.t('page.platforms'),
          subtitle: '${_c.enabledCount} / ${_c.platforms.length}',
          trailing: Wrap(
            spacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              SizedBox(
                width: 180,
                child: TextField(
                  decoration: InputDecoration(
                    isDense: true,
                    hintText: t.t('platform.searchPlaceholder'),
                    hintStyle: AidogType.micro.copyWith(
                      color: AidogTheme.of(context).c.fg3,
                    ),
                  ),
                  style: AidogType.micro.copyWith(
                    color: AidogTheme.of(context).c.fg,
                  ),
                  onChanged: _c.setSearchQuery,
                ),
              ),
              SmallButton(
                label: t.t('platform.purgeDisabled'),
                onTap: _c.askPurgeDisabled,
              ),
            ],
          ),
        ),
        if (widget.showGroups) ...[
          GroupsSection(
            invoke: widget.invoke,
            onToast: _showToast,
            // 分组区删平台之后，主列表要把那几行局部移掉（不整页重拉）。
            onPlatformsDeleted: _c.removePlatformsByIds,
          ),
          const SizedBox(height: AidogSpace.s_2xl),
        ],
        TileMeta(t.t('platform.ungrouped')),
        const SizedBox(height: AidogSpace.ssm),
        if (_c.loading)
          CenteredNote(text: t.t('status.loading'))
        else if (_c.standalonePlatforms.isEmpty)
          CenteredNote(text: t.t('platform.empty'))
        else
          Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final p in _c.standalonePlatforms)
                Padding(
                  padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                  child: _PlatformCard(
                    platform: p,
                    usage: _c.usageMap[p.id],
                    quota: _c.quotaMap[p.id],
                    quotaPending: _c.quotaPending[p.id] == true,
                    quotaRefreshing: _c.quotaRefreshing[p.id] == true,
                    lastTest: _c.lastTestMap[p.id],
                    testing: _c.testingId == p.id,
                    onToggle: () => _c.togglePlatform(p),
                    onTest: () => _c.quickTest(p),
                    onRefreshQuota: () => _c.refreshQuota(p),
                    onDelete: () => _c.askDelete(p.id),
                    onViewLogs: () =>
                        widget.onNavigate?.call('logs', platformId: p.id),
                  ),
                ),
            ],
          ),
        if (_c.purgeCandidates != null)
          ConfirmCard(
            title: t.t('platform.purgeDisabled'),
            body: _c.purgeCandidates!.isEmpty
                ? t.t('platform.purgeDisabledNone')
                : '${_c.purgeCandidates!.length}',
            confirmLabel: t.t('action.confirm'),
            onCancel: _c.cancelPurgeDisabled,
            onConfirm: () => _c.confirmPurgeDisabled(
              noneText: t.t('platform.purgeDisabledNone'),
            ),
          ),
        if (_c.deleteTarget != null)
          ConfirmCard(
            title: t.t('group.deletePlatformAction'),
            body: t.t('group.deletePlatformConfirm'),
            confirmLabel: t.t('action.delete'),
            onCancel: _c.cancelDelete,
            onConfirm: () => _c.deletePlatform(_c.deleteTarget!),
          ),
        if (_toast != null) ToastBar(text: _toast!.text, ok: _toast!.ok),
      ],
    );
  }
}

/// 未分组区的平台卡。React 的 `PlatformCard` 还带 favicon、拖拽手柄、展开的端点明细；
/// 这里先把**动作**做全（启停 / 测试 / 刷余额 / 查日志 / 删除），展开明细未做。
class _PlatformCard extends StatelessWidget {
  const _PlatformCard({
    required this.platform,
    required this.usage,
    required this.quota,
    required this.quotaPending,
    required this.quotaRefreshing,
    required this.lastTest,
    required this.testing,
    required this.onToggle,
    required this.onTest,
    required this.onRefreshQuota,
    required this.onDelete,
    required this.onViewLogs,
  });

  final PlatformRow platform;
  final UsageStats? usage;
  final PlatformQuota? quota;
  final bool quotaPending;
  final bool quotaRefreshing;
  final LastTestResult? lastTest;
  final bool testing;
  final VoidCallback onToggle;
  final VoidCallback onTest;
  final VoidCallback onRefreshQuota;
  final VoidCallback onDelete;
  final VoidCallback onViewLogs;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    // 三态各自一个颜色：启用绿、自动禁用红（是上游把我们踢了）、手动禁用灰。
    final statusColor = switch (platform.status) {
      'enabled' => theme.c.ok,
      'auto_disabled' => theme.c.bad,
      _ => theme.c.fg3,
    };
    return Tile(
      live: platform.status == 'enabled',
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Container(
            width: 8,
            height: 8,
            margin: const EdgeInsets.only(right: AidogSpace.ssm),
            decoration: BoxDecoration(
              color: statusColor,
              shape: BoxShape.circle,
            ),
          ),
          Expanded(
            flex: 3,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  platform.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AidogType.body.copyWith(color: theme.c.fg),
                ),
                Text(
                  platform.platformType,
                  style: AidogType.micro.copyWith(color: theme.c.fg3),
                ),
              ],
            ),
          ),
          // 余额：还在查时显示占位，查到了显示数，查不到就什么都不显示（不画假的 0）。
          Expanded(
            flex: 2,
            child: Text(
              quotaPending || quotaRefreshing
                  ? '…'
                  : quota?.balanceRemaining != null
                  ? formatCostUsd(quota!.balanceRemaining!)
                  : platform.estBalanceRemaining > 0
                  ? formatCostUsd(platform.estBalanceRemaining)
                  : '',
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
          ),
          Expanded(
            flex: 2,
            child: Text(
              usage == null ? '' : formatNumber(usage!.totalRequests),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
          if (lastTest != null)
            Padding(
              padding: const EdgeInsets.only(right: AidogSpace.ssm),
              child: Icon(
                lastTest!.success ? Icons.check_circle : Icons.error,
                size: 13,
                color: lastTest!.success ? theme.c.ok : theme.c.bad,
              ),
            ),
          Wrap(
            spacing: AidogSpace.sxs,
            children: [
              SmallButton(
                label: testing ? t.t('status.loading') : t.t('platform.quickTest'),
                onTap: testing ? null : onTest,
              ),
              SmallButton(
                label: t.t('platform.quotaRefresh'),
                onTap: quotaRefreshing ? null : onRefreshQuota,
              ),
              SmallButton(
                label: platform.status == 'enabled'
                    ? t.t('platform.disable')
                    : t.t('platform.enable'),
                onTap: onToggle,
              ),
              SmallButton(label: t.t('page.logs'), onTap: onViewLogs),
              SmallButton(
                label: t.t('action.delete'),
                danger: true,
                onTap: onDelete,
              ),
            ],
          ),
        ],
      ),
    );
  }
}
