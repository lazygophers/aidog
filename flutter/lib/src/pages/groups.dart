/// 分组界面（票 I07），对应 `src/pages/Groups.tsx` 的 `GroupsEmbedded`
/// 与 `src/pages/Groups/{GroupListView,GroupListItem,GroupCreateModal,GroupEditPanel}.tsx`。
///
/// 三个视图态互斥，与 React 的三条早返回一一对应：
///   编辑某个组（`edit.target != null`）→ 新建组（`showCreate`）→ 列表。
///
/// 本文件只负责画。校验（能不能点保存/创建）、破坏性确认、批量操作全在
/// [GroupsController]（`groups_logic.dart`），这里把它们接到按钮上。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../utils/formatters.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'groups_logic.dart';
import 'invoke.dart';
import 'models.dart';
import 'platform_defaults.dart' show kModelSlots;
import 'ui_bits.dart';

/// 分组区。内嵌在平台页里（与 React 的 `GroupsEmbedded` 同位置），
/// 也可以单独渲染（widget 测试就是这么用的）。
class GroupsSection extends StatefulWidget {
  const GroupsSection({
    super.key,
    this.invoke = kernelInvoke,
    this.onToast,
    this.onPlatformsDeleted,
    this.onCreateGroupReady,
    this.onPlatformDropped,
    this.onCreatePlatform,
    this.onNavigate,
    this.copyText = native.writeText,
  });

  final InvokeFn invoke;
  final void Function(String text, {required bool ok})? onToast;

  /// 分组区删掉平台之后通知父级，让主列表把那几行局部移掉（不整页重拉）。
  final void Function(List<int> ids)? onPlatformsDeleted;

  /// 把「打开新建分组」交给父级，好让页头也能放一颗「+ 添加分组」
  /// （React 那边是 `openCreateGroupRef`，`PlatformListView.tsx:104-106`）。
  /// 建好之后回调一次，父级存下来当按钮的 onTap。
  final void Function(VoidCallback openCreate)? onCreateGroupReady;

  /// 未分组平台被拖进某个分组卡时调用（`usePlatformsState.ts:296-312`）。
  /// 真正的搬迁由父级做（它持有平台列表控制器，要顺带刷 membership），
  /// 这里只在它 await 完之后把分组区自己的数据静默重拉一遍。
  final Future<void> Function(int platformId, int groupId)? onPlatformDropped;

  /// 「在此分组添加平台」（`Groups.tsx:245-251`）：父级(Platforms)打开同页创建表单，
  /// 预绑并锁定归属分组。父级 `openCreatePlatform(presetGroupIds:, lockGid:)`。
  final void Function({List<int>? presetGroupIds, int? lockGid})?
  onCreatePlatform;

  /// 「查看统计」（`Groups.tsx:237`）跳转 stats 页。**已知简化**：React 侧带
  /// `{groupId, groupKey}` context 预筛该组，Dart 侧顶层导航目前只按页面 id 切换
  /// （`main.dart::_nav.navigate` 无 payload 通道），跳过去后落在总览态，
  /// 不预筛 —— 文案 key 与跳转动作都在，只是没带上下文。
  final void Function(String pageId)? onNavigate;

  /// 复制到剪贴板，测试可注入假实现。
  final Future<void> Function(String text) copyText;

  @override
  State<GroupsSection> createState() => _GroupsSectionState();
}

class _GroupsSectionState extends State<GroupsSection> {
  late final GroupsController _c;
  bool _hydrated = false;

  @override
  void initState() {
    super.initState();
    _c = GroupsController(
      invoke: widget.invoke,
      onChanged: () {
        if (!mounted) return;
        // 折叠态只从 group.extra 回灌一次：之后每次刷新都灌，会把用户刚点开的又收回去。
        if (!_hydrated && _c.details.isNotEmpty) {
          _hydrated = true;
          _c.hydrateCollapsedFrom(_c.details);
        }
        setState(() {});
      },
      onToast: widget.onToast,
    );
    _c.init();
    widget.onCreateGroupReady?.call(_c.openCreate);
  }

  /// 父级搬完（后端已写）→ 分组区自己的 details 静默重拉，目标组里立刻出现那一行。
  Future<void> _acceptDrop(int pid, int gid) async {
    await widget.onPlatformDropped!(pid, gid);
    if (mounted) await _c.silentReload();
  }

  @override
  Widget build(BuildContext context) {
    if (_c.edit.target != null) {
      return _GroupEditPanel(controller: _c, copyText: widget.copyText);
    }
    if (_c.showCreate) return _GroupCreatePanel(controller: _c);
    return _GroupListView(
      controller: _c,
      onPlatformsDeleted: widget.onPlatformsDeleted,
      onPlatformDropped: widget.onPlatformDropped == null ? null : _acceptDrop,
      onCreatePlatform: widget.onCreatePlatform,
      onNavigate: widget.onNavigate,
      copyText: widget.copyText,
    );
  }
}

// ── 列表态 ────────────────────────────────────────────────────────

class _GroupListView extends StatelessWidget {
  const _GroupListView({
    required this.controller,
    this.onPlatformsDeleted,
    this.onPlatformDropped,
    this.onCreatePlatform,
    this.onNavigate,
    this.copyText = native.writeText,
  });

  final GroupsController controller;
  final void Function(List<int> ids)? onPlatformsDeleted;
  final Future<void> Function(int platformId, int groupId)? onPlatformDropped;
  final void Function({List<int>? presetGroupIds, int? lockGid})?
  onCreatePlatform;
  final void Function(String pageId)? onNavigate;
  final Future<void> Function(String text) copyText;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final c = controller;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            TileMeta(t.t('page.groups')),
            const Spacer(),
            Flexible(
              child: Text(
                c.proxyBaseUrl,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.micro.copyWith(
                  color: AidogTheme.of(context).c.fg3,
                ),
              ),
            ),
            const SizedBox(width: AidogSpace.sxs),
            Tooltip(
              message: t.t('group.copyBaseUrlTitle'),
              child: SmallButton(
                label: t.t('group.copyBaseUrl'),
                onTap: () => copyText(c.proxyBaseUrl),
              ),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(label: t.t('group.add'), onTap: c.openCreate),
          ],
        ),
        const SizedBox(height: AidogSpace.ssm),
        if (c.loading)
          CenteredNote(text: t.t('status.loading'))
        else if (c.details.isEmpty)
          CenteredNote(text: t.t('group.empty'))
        else
          // 分组列表拖拽排序（`Groups.tsx:517-524` 的 SortableList）：
          // 搜索态在本页尚未接入（无搜索入口），故不设 no-op 分支。
          ReorderableListView.builder(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            buildDefaultDragHandles: false,
            itemCount: c.details.length,
            onReorderItem: (o, n) {
              final next = [...c.details];
              next.insert(n, next.removeAt(o));
              unawaited(c.reorderGroups(next));
            },
            itemBuilder: (context, i) {
              final d = c.details[i];
              return Padding(
                key: ValueKey(d.group.id),
                padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                child: _GroupCard(
                  controller: c,
                  detail: d,
                  collapsed: c.collapsedGroups.contains(d.group.id),
                  onPlatformDropped: onPlatformDropped,
                  onCreatePlatform: onCreatePlatform,
                  onNavigate: onNavigate,
                  copyText: copyText,
                ),
              );
            },
          ),
        if (c.loadingMore)
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.ssm),
            child: CenteredNote(text: t.t('status.loading')),
          )
        else if (c.hasMore)
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.ssm),
            child: Align(
              child: SmallButton(
                label: t.t('logs.hasMore'),
                onTap: c.loadMore,
              ),
            ),
          ),
        // 虚拟桶「未匹配」（MITM fallback 直通）：`GroupListView.tsx:276-279`。
        if (c.unmatchedStat != null && c.unmatchedStat!.totalRequests > 0)
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.ssm),
            child: _UnmatchedBucketCard(stat: c.unmatchedStat!),
          ),
        // ── 破坏性确认：互斥，同一时刻最多一个 ──
        if (c.deleteGroupTarget != null)
          ConfirmCard(
            title: t.t('group.delete'),
            body: t.t('group.deleteConfirm'),
            confirmLabel: t.t('action.delete'),
            onCancel: c.cancelDeleteGroup,
            onConfirm: () => c.confirmDeleteGroup(failText: t.t('group.deleteFailed')),
          ),
        if (c.removeTarget != null)
          _RemovePlatformConfirm(
            controller: c,
            onDeleted: (id) => onPlatformsDeleted?.call([id]),
          ),
        if (c.batchDeleteTarget != null)
          ConfirmCard(
            title: t.t('group.batchDeleteTitle'),
            body: t.t(
              'group.batchDeleteDesc',
              {'count': '${c.batchDeleteTarget!.platforms.length}'},
            ),
            confirmLabel: c.batchDeleteBusy
                ? t.t('group.batchDeleting')
                : t.t(
                    'group.batchDeleteConfirm',
                    {'count': '${c.batchDeleteTarget!.platforms.length}'},
                  ),
            busy: c.batchDeleteBusy,
            extra: c.batchDeleteTarget!.hasCrossGroup
                // 跨组警告：删掉就是从所有组里消失，不只是本组。
                ? _CrossGroupWarning(target: c.batchDeleteTarget!)
                : null,
            onCancel: c.cancelBatchDelete,
            onConfirm: () {
              final ids = [for (final p in c.batchDeleteTarget!.platforms) p.id];
              c
                  .confirmBatchDelete(
                    doneText: (n) => t.t('group.batchDeleteDone', {'count': '$n'}),
                    failText: t.t('group.batchDeleteFailed'),
                  )
                  .then((_) => onPlatformsDeleted?.call(ids));
            },
          ),
        if (c.batchOverrideTarget != null)
          _BatchOverrideModelsCard(controller: c),
        if (c.batchSetStatusTarget != null) _BatchSetStatusCard(controller: c),
        if (c.batchMoveGroupTarget != null) _BatchMoveGroupCard(controller: c),
        if (c.purgeTarget != null)
          ConfirmCard(
            title: t.t('group.purgeDisabled'),
            body: c.purgeTarget!.candidates.isEmpty
                ? t.t('platform.purgeDisabledNone')
                : t.t(
                    'group.purgeDisabledConfirm',
                    {'count': '${c.purgeTarget!.candidates.length}'},
                  ),
            confirmLabel: t.t('action.confirm'),
            onConfirm: c.purgeTarget!.candidates.isEmpty
                ? null
                : () => c.confirmPurgeDisabled(
                    noneText: t.t('platform.purgeDisabledNone'),
                    doneText: (deleted, unassigned) => t.t(
                      'group.purgeDisabledDone',
                      {'deleted': '$deleted', 'unassigned': '$unassigned'},
                    ),
                    failText: t.t('group.purgeDisabled'),
                  ),
            onCancel: c.cancelPurgeDisabled,
          ),
        if (c.groupTest != null) _GroupTestPanel(controller: c),
      ],
    );
  }
}

/// `GroupListView.tsx:401-434` 的只读虚拟桶卡片：MITM 解密非 API 流量 fallback
/// 直通的统计，无平台/余额/编辑。
class _UnmatchedBucketCard extends StatelessWidget {
  const _UnmatchedBucketCard({required this.stat});

  final UsageStats stat;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Tile(
      title: t.t('group.unmatched'),
      meta: t.t('group.unmatchedBadge'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            t.t('group.unmatchedHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.sxs),
          Text(
            formatNumber(stat.totalRequests),
            style: AidogType.micro.copyWith(color: theme.c.fg2),
          ),
        ],
      ),
    );
  }
}

class _GroupCard extends StatelessWidget {
  const _GroupCard({
    required this.controller,
    required this.detail,
    required this.collapsed,
    this.onPlatformDropped,
    this.onCreatePlatform,
    this.onNavigate,
    this.copyText = native.writeText,
  });

  final GroupsController controller;
  final GroupDetail detail;
  final bool collapsed;
  final Future<void> Function(int platformId, int groupId)? onPlatformDropped;
  final void Function({List<int>? presetGroupIds, int? lockGid})?
  onCreatePlatform;
  final void Function(String pageId)? onNavigate;
  final Future<void> Function(String text) copyText;

  @override
  Widget build(BuildContext context) {
    final card = _card(context);
    if (onPlatformDropped == null) return card;
    // 未分组平台的落点。已经在本组里的平台不接（React 的 `findGroupAt` 落在
    // 未分组区的卡片上时也拿不到 group id，等价于不接）。
    final gid = detail.group.id;
    final memberIds = {for (final gp in detail.platforms) gp.platform.id};
    return DragTarget<int>(
      onWillAcceptWithDetails: (d) => !memberIds.contains(d.data),
      onAcceptWithDetails: (d) => onPlatformDropped!(d.data, gid),
      builder: (context, candidate, _) => Container(
        decoration: candidate.isEmpty
            ? null
            : BoxDecoration(
                // React 那边是 `outline: 2px solid var(--accent)` + 2px offset。
                border: Border.all(
                  color: AidogTheme.of(context).c.accent,
                  width: 2,
                ),
                borderRadius: BorderRadius.circular(AidogRadius.md),
              ),
        child: card,
      ),
    );
  }

  Widget _card(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = controller;
    final g = detail.group;
    final stats = c.groupStats[g.groupKey];
    final balance = c.groupBalance[g.id];
    final selecting = c.isBatchSelecting(g.id);
    return Tile(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              IconButton(
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(),
                iconSize: 16,
                color: theme.c.fg3,
                tooltip: t.t('group.toggleDetails'),
                icon: Icon(
                  collapsed ? Icons.chevron_right : Icons.expand_more,
                ),
                onPressed: () {
                  final next = c.toggleGroupCollapsed(g.id);
                  c.persistGroupCollapsed(g.id, next);
                },
              ),
              const SizedBox(width: AidogSpace.sxs),
              // 分组排序拖拽把手（`Groups.tsx:186-195` 的 drag-handle）。
              Tooltip(
                message: t.t('group.dragToReorder'),
                child: const Icon(Icons.drag_handle, size: 16),
              ),
              const SizedBox(width: AidogSpace.sxs),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      children: [
                        Flexible(
                          child: Text(
                            g.name,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: AidogType.body.copyWith(color: theme.c.fg),
                          ),
                        ),
                        if (g.isDefault)
                          Padding(
                            padding: const EdgeInsets.only(left: AidogSpace.sxs),
                            child: Tooltip(
                              message: t.t('group.isDefaultTitle'),
                              child: Text(
                                t.t('group.isDefault'),
                                style: AidogType.micro.copyWith(
                                  color: theme.c.accentText,
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                    Text(
                      '${g.groupKey} · ${routingLabel(t, g.routingMode)}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                  ],
                ),
              ),
              if (stats != null)
                Padding(
                  padding: const EdgeInsets.only(right: AidogSpace.ssm),
                  child: Text(
                    formatNumber(stats.totalRequests),
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                ),
              if (balance != null)
                Padding(
                  padding: const EdgeInsets.only(right: AidogSpace.ssm),
                  child: Text(
                    formatCostUsd(balance),
                    style: AidogType.micro.copyWith(color: theme.c.fg2),
                  ),
                ),
              Wrap(
                spacing: AidogSpace.sxs,
                children: [
                  SmallButton(
                    label: t.t('group.testAll'),
                    onTap: () => c.testGroup(g, detail.platforms),
                  ),
                  Tooltip(
                    message: g.isDefault
                        ? t.t('group.unsetDefault')
                        : t.t('group.setAsDefault'),
                    child: SmallButton(
                      label: g.isDefault
                          ? t.t('group.defaultConfigWritten')
                          : t.t('group.setAsDefault'),
                      active: g.isDefault,
                      onTap: () => c.toggleDefault(
                        g,
                        failText: t.t('group.setDefaultFailed'),
                      ),
                    ),
                  ),
                  SmallButton(
                    label: t.t('action.edit'),
                    onTap: () => c.openEdit(detail),
                  ),
                  SmallButton(
                    label: t.t('action.delete'),
                    danger: true,
                    onTap: () => c.askDeleteGroup(g.id),
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.sxs),
          // 行 1.5：复制启动命令 / 查看统计 / 分组内添加平台 / 清理失效 / 多选。
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            children: [
              _CopyCommandMenu(group: g, proxyEnvVars: c.proxyEnvVars, copyText: copyText),
              if (onNavigate != null)
                SmallButton(
                  label: t.t('group.viewStats'),
                  onTap: () => onNavigate!.call('stats'),
                ),
              if (onCreatePlatform != null)
                SmallButton(
                  label: t.t('group.addPlatformToGroup'),
                  onTap: () => onCreatePlatform!.call(
                    presetGroupIds: [g.id],
                    lockGid: g.id,
                  ),
                ),
              SmallButton(
                label: t.t('group.purgeDisabled'),
                onTap: () => c.askPurgeDisabled(g.id),
              ),
              if (detail.platforms.isNotEmpty)
                SmallButton(
                  label: t.t('group.batchOps'),
                  active: selecting,
                  onTap: () => selecting
                      ? c.exitBatchSelect(g.id)
                      : c.enterBatchSelect(g.id),
                ),
            ],
          ),
          if (!collapsed) ...[
            const SizedBox(height: AidogSpace.ssm),
            if (selecting) _BatchToolbar(controller: c, gid: g.id, platforms: detail.platforms),
            if (detail.platforms.isEmpty)
              Text(
                t.t('group.noPlatforms'),
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              )
            else
              for (final gp in detail.platforms)
                _PlatformRow(
                  controller: c,
                  group: g,
                  gp: gp,
                  allGroups: c.allGroups,
                  selecting: selecting,
                ),
            _MappingsSection(controller: c, detail: detail),
          ],
        ],
      ),
    );
  }
}

/// 复制启动命令菜单：`group.copyCommand` 主按钮 + 悬浮/点击展开
/// key / Claude / Codex / pi 四项（`GroupListItem.tsx:224-236`）。
class _CopyCommandMenu extends StatelessWidget {
  const _CopyCommandMenu({
    required this.group,
    required this.proxyEnvVars,
    required this.copyText,
  });

  final GroupRow group;
  final List<EnvVar> proxyEnvVars;
  final Future<void> Function(String text) copyText;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final envVars = [...group.envVars, ...proxyEnvVars];
    return PopupMenuButton<String>(
      // 平时显示「复制启动命令」，悬浮提示「复制密钥」——与 React 的 CopyButton
      // defaultLabel / hoverLabel 一致（`GroupListItem.tsx:227-229`）。
      tooltip: t.t('group.copyKeyLabel'),
      onSelected: (key) {
        final text = switch (key) {
          'key' => group.groupKey,
          'claude' => buildClaudeCommand(group.groupKey),
          'codex' => buildCodexCommand(group.groupKey, envVars),
          'pi' => buildPiCommand(group.groupKey, envVars),
          _ => '',
        };
        copyText(text);
      },
      itemBuilder: (context) => [
        PopupMenuItem(value: 'key', child: Text(t.t('group.menuCopyKey'))),
        PopupMenuItem(value: 'claude', child: Text(t.t('group.menuCopyClaude'))),
        PopupMenuItem(value: 'codex', child: Text(t.t('group.menuCopyCodex'))),
        PopupMenuItem(value: 'pi', child: Text(t.t('group.menuCopyPi'))),
      ],
      child: IgnorePointer(
        child: SmallButton(label: t.t('group.copyCommand')),
      ),
    );
  }
}

/// per-group 多选工具栏（`GroupListItem.tsx:374-413`）：全选 / 计数 / 四个批量操作。
class _BatchToolbar extends StatelessWidget {
  const _BatchToolbar({
    required this.controller,
    required this.gid,
    required this.platforms,
  });

  final GroupsController controller;
  final int gid;
  final List<GroupPlatform> platforms;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = controller;
    final selected = c.selectedIdsOf(gid);
    final hasSelection = selected.isNotEmpty;
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Wrap(
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: AidogSpace.sxs,
        runSpacing: AidogSpace.sxs,
        children: [
          SmallButton(label: t.t('action.cancel'), onTap: () => c.exitBatchSelect(gid)),
          SmallButton(
            label: t.t('group.selectAll'),
            onTap: () => c.selectAll(gid, [for (final gp in platforms) gp.platform.id]),
          ),
          Text(
            t.t('group.selectedCount', {'count': '${selected.length}'}),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          SmallButton(
            label: t.t('group.batchDelete'),
            danger: true,
            onTap: hasSelection ? () => c.askBatchDelete(selected.toList()) : null,
          ),
          SmallButton(
            label: t.t('group.batchOverrideModels'),
            onTap: hasSelection ? () => c.askBatchOverrideModels(selected.toList()) : null,
          ),
          SmallButton(
            label: t.t('group.batchSetStatus'),
            onTap: hasSelection ? () => c.askBatchSetStatus(selected.toList(), gid) : null,
          ),
          SmallButton(
            label: t.t('group.batchMoveGroup'),
            onTap: hasSelection ? () => c.askBatchMoveGroup(selected.toList(), gid) : null,
          ),
        ],
      ),
    );
  }
}

/// 单个分组内平台行：多选态给 checkbox，非多选态给优先级步进器 + 移组下拉 + 移除。
class _PlatformRow extends StatelessWidget {
  const _PlatformRow({
    required this.controller,
    required this.group,
    required this.gp,
    required this.allGroups,
    required this.selecting,
  });

  final GroupsController controller;
  final GroupRow group;
  final GroupPlatform gp;
  final List<({int id, String name})> allGroups;
  final bool selecting;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = controller;
    final pid = gp.platform.id;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        children: [
          if (selecting)
            Padding(
              padding: const EdgeInsets.only(right: AidogSpace.sxs),
              child: Checkbox(
                value: c.selectedIdsOf(group.id).contains(pid),
                onChanged: (_) => c.toggleSelected(group.id, pid),
              ),
            )
          else
            Container(
              width: 6,
              height: 6,
              margin: const EdgeInsets.only(right: AidogSpace.ssm),
              decoration: BoxDecoration(
                color: gp.platform.status == 'enabled' ? theme.c.ok : theme.c.fg3,
                shape: BoxShape.circle,
              ),
            ),
          Expanded(
            child: Text(
              gp.platform.name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
          ),
          if (!selecting) ...[
            // per-group 优先级（1~10，10 最高）。就地改，乐观更新 + 失败回滚。
            // `PlatformCard.tsx:895-961::LevelPriorityControl` 逐条翻译。
            Tooltip(
              message: t.t('group.levelPriorityHint'),
              child: Text(
                t.t('group.levelPriority'),
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              ),
            ),
            IconButton(
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
              iconSize: 14,
              tooltip: t.t('group.levelPriorityDown'),
              onPressed: gp.levelPriority <= 1
                  ? null
                  : () => c.setLevelPriority(group.id, pid, gp.levelPriority - 1, failText: t.t('group.levelPriorityFailed')),
              icon: const Icon(Icons.remove),
            ),
            SizedBox(
              width: 18,
              child: Text(
                '${gp.levelPriority}',
                textAlign: TextAlign.center,
                style: AidogType.micro.copyWith(color: theme.c.fg),
              ),
            ),
            IconButton(
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
              iconSize: 14,
              tooltip: t.t('group.levelPriorityUp'),
              onPressed: gp.levelPriority >= 10
                  ? null
                  : () => c.setLevelPriority(group.id, pid, gp.levelPriority + 1, failText: t.t('group.levelPriorityFailed')),
              icon: const Icon(Icons.add),
            ),
            Text(
              t.t('group.levelPriorityMax'),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
            const SizedBox(width: AidogSpace.ssm),
            // 移动到另一分组：`usePlatformDrag.ts` 的跨组拖拽等价功能（不同交互形态，
            // 同一后端命令 `group_platform_move`）——嵌套 `ReorderableListView`
            // 手势冲突，改下拉选目标组。
            if (allGroups.length > 1)
              PopupMenuButton<int>(
                tooltip: t.t('group.dragPlatform'),
                icon: Icon(Icons.drive_file_move_outline, size: 14, color: theme.c.fg3),
                onSelected: (targetGid) =>
                    c.movePlatform(pid, group.id, targetGid),
                itemBuilder: (context) => [
                  for (final og in allGroups)
                    if (og.id != group.id)
                      PopupMenuItem(value: og.id, child: Text(og.name)),
                ],
              ),
          ],
          SmallButton(
            label: t.t('group.deletePlatformTitle'),
            onTap: () => c.askRemovePlatform(gp.platform, group.id),
          ),
        ],
      ),
    );
  }
}

/// 模型映射列表 + 列表页快捷添加表单（`GroupListItem.tsx:476-553` 逐条翻译）。
class _MappingsSection extends StatelessWidget {
  const _MappingsSection({required this.controller, required this.detail});

  final GroupsController controller;
  final GroupDetail detail;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = controller;
    final gid = detail.group.id;
    final showForm = c.mappingGroupId == gid;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (detail.modelMappings.isNotEmpty)
          for (var i = 0; i < detail.modelMappings.length; i++)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      '${detail.modelMappings[i].sourceModel} → '
                      '${detail.modelMappings[i].targetModel}',
                      style: AidogType.micro.copyWith(color: theme.c.fg2),
                    ),
                  ),
                  IconButton(
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
                    iconSize: 14,
                    icon: const Icon(Icons.close),
                    onPressed: () => c.deleteMapping(
                      gid,
                      i,
                      failText: t.t('group.deleteMappingFailed'),
                    ),
                  ),
                ],
              ),
            ),
        Align(
          alignment: Alignment.centerLeft,
          child: SmallButton(
            label: '+ ${t.t('mapping.add')}',
            onTap: () => c.setMappingGroupId(showForm ? null : gid),
          ),
        ),
        if (showForm)
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.sxs),
            child: Wrap(
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: AidogSpace.sxs,
              runSpacing: AidogSpace.sxs,
              children: [
                SizedBox(
                  width: 140,
                  child: TextField(
                    decoration: InputDecoration(hintText: t.t('mapping.source')),
                    style: AidogType.micro.copyWith(color: theme.c.fg),
                    onChanged: c.setMSource,
                  ),
                ),
                DropdownButtonHideUnderline(
                  child: DropdownButton<int>(
                    hint: Text(t.t('mapping.targetPlatform')),
                    value: c.mTargetPlatform,
                    items: [
                      for (final p in c.platforms)
                        DropdownMenuItem(value: p.id, child: Text(p.name)),
                    ],
                    onChanged: c.setMTargetPlatform,
                  ),
                ),
                if (c.mAvailableModels.isNotEmpty)
                  DropdownButtonHideUnderline(
                    child: DropdownButton<String>(
                      hint: Text(t.t('mapping.target')),
                      value: c.mTargetModel.isEmpty ? null : c.mTargetModel,
                      items: [
                        for (final m in c.mAvailableModels)
                          DropdownMenuItem(value: m, child: Text(m)),
                      ],
                      onChanged: (v) => c.setMTargetModel(v ?? ''),
                    ),
                  )
                else
                  SizedBox(
                    width: 120,
                    child: TextField(
                      decoration: InputDecoration(hintText: t.t('mapping.target')),
                      style: AidogType.micro.copyWith(color: theme.c.fg),
                      onChanged: c.setMTargetModel,
                    ),
                  ),
                SmallButton(
                  label: t.t('action.create'),
                  onTap: (c.mSource.isEmpty || c.mTargetPlatform == null || c.mTargetModel.isEmpty)
                      ? null
                      : () => c.submitAddMapping(failText: t.t('group.addMappingFailed')),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

/// 「移除平台」确认。属几个组决定给几个选项 —— 这是本票点名的风险位：
/// 只属本组 → 只有「删除平台」；属多个组 → 多一个「仅移出本组」。
class _RemovePlatformConfirm extends StatelessWidget {
  const _RemovePlatformConfirm({
    required this.controller,
    required this.onDeleted,
  });

  final GroupsController controller;
  final void Function(int id) onDeleted;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final target = controller.removeTarget!;
    // 属多个组 → 标题/正文/主按钮都换一套措辞（`GroupListView.tsx:313-347`）。
    final multi = !target.onlyInThisGroup;
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: multi
            ? t.t('group.deletePlatformMultiTitle')
            : t.t('group.deletePlatformTitle'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              multi
                  ? t.t('group.deletePlatformMultiDesc', {
                      'name': target.platform.name,
                      'count': '${target.groupCount}',
                      'groups': target.groupNames.join('、'),
                    })
                  : t.t('group.deletePlatformConfirm', {
                      'name': target.platform.name,
                    }),
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  onTap: controller.cancelRemovePlatform,
                ),
                const SizedBox(width: AidogSpace.ssm),
                // 属多个组才给「仅移出本组」—— 只属本组时这个选项没有意义
                // （移出去它就变成一个谁也不管的未分组平台）。
                if (multi) ...[
                  SmallButton(
                    label: t.t('group.removeFromGroupAction'),
                    onTap: () => controller.removePlatformFromGroup(
                      failText: t.t('group.removeFromGroupFailed'),
                    ),
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                ],
                SmallButton(
                  label: multi
                      ? t.t('group.deleteFromAllGroupsAction')
                      : t.t('group.deletePlatformAction'),
                  danger: true,
                  onTap: () {
                    final id = target.platform.id;
                    controller.confirmDeletePlatform().then(
                      (_) => onDeleted(id),
                    );
                  },
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _CrossGroupWarning extends StatelessWidget {
  const _CrossGroupWarning({required this.target});

  final BatchDeleteTarget target;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final cross = [
      for (final p in target.platforms)
        if ((target.groupNamesByPlatform[p.id] ?? const []).length > 1) p,
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          t.t('group.batchDeleteCrossGroupWarning', {'count': '${cross.length}'}),
          style: AidogType.micro.copyWith(color: theme.c.bad),
        ),
        for (final p in cross)
          Text(
            '${p.name} · '
            '${t.t('group.batchDeleteCrossGroupItem', {
              'count': '${target.groupNamesByPlatform[p.id]!.length}',
              'groups': target.groupNamesByPlatform[p.id]!.join('、'),
            })}',
            style: AidogType.micro.copyWith(color: theme.c.bad),
          ),
      ],
    );
  }
}

/// 批量覆盖模型弹窗（`BatchOverrideModelsModal.tsx`）：三来源 radio（手输 / preset /
/// 从别平台复制）+ 全 diff 预览，确认时整体覆盖五槽（不是合并）。
class _BatchOverrideModelsCard extends StatefulWidget {
  const _BatchOverrideModelsCard({required this.controller});

  final GroupsController controller;

  @override
  State<_BatchOverrideModelsCard> createState() => _BatchOverrideModelsCardState();
}

class _BatchOverrideModelsCardState extends State<_BatchOverrideModelsCard> {
  String _source = 'manual';
  String _presetProtocol = '';
  int? _copyPlatformId;
  Map<String, String> _slots = const {};

  Map<String, String> _modelsOf(PlatformModels m) => {
    'default': m.defaultModel ?? '',
    'sonnet': m.sonnet ?? '',
    'opus': m.opus ?? '',
    'haiku': m.haiku ?? '',
    'gpt': m.gpt ?? '',
  };

  void _setSource(String v) {
    setState(() {
      _source = v;
      _presetProtocol = '';
      _copyPlatformId = null;
      _slots = const {};
    });
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = widget.controller;
    final target = c.batchOverrideTarget;
    if (target == null) return const SizedBox.shrink();
    final allEmpty = kModelSlots.every((s) => (_slots[s.key] ?? '').trim().isEmpty);
    return ConfirmCard(
      title: t.t('group.batchOverrideModelsTitle'),
      body: t.t('group.batchOverrideModelsDesc', {'count': '${target.length}'}),
      confirmLabel: c.batchOverrideBusy
          ? t.t('group.batchOverrideApplying')
          : t.t('group.batchOverrideConfirm', {'count': '${target.length}'}),
      busy: c.batchOverrideBusy,
      onCancel: c.cancelBatchOverrideModels,
      onConfirm: (c.batchOverrideBusy || allEmpty)
          ? null
          : () => c.confirmBatchOverrideModels(
              PlatformModels(
                defaultModel: _slots['default'],
                sonnet: _slots['sonnet'],
                opus: _slots['opus'],
                haiku: _slots['haiku'],
                gpt: _slots['gpt'],
              ),
              doneText: (n) => t.t('group.batchOverrideModelsDone', {'count': '$n'}),
              failText: t.t('group.batchOverrideModelsFailed'),
            ),
      extra: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Wrap(
            spacing: AidogSpace.sxs,
            children: [
              for (final s in const ['manual', 'preset', 'copy'])
                SmallButton(
                  label: switch (s) {
                    'manual' => t.t('group.batchOverrideSourceManual'),
                    'preset' => t.t('group.batchOverrideSourcePreset'),
                    _ => t.t('group.batchOverrideSourceCopy'),
                  },
                  active: _source == s,
                  onTap: () => _setSource(s),
                ),
            ],
          ),
          if (_source == 'preset')
            DropdownButtonHideUnderline(
              child: DropdownButton<String>(
                isExpanded: true,
                hint: Text(t.t('group.batchOverridePresetSelect')),
                value: _presetProtocol.isEmpty ? null : _presetProtocol,
                items: [
                  for (final o in c.defaults.protocolOptions())
                    DropdownMenuItem(value: o.value, child: Text(o.label)),
                ],
                onChanged: (v) {
                  if (v == null) return;
                  setState(() {
                    _presetProtocol = v;
                    _slots = c.defaults.defaultModels(v);
                  });
                },
              ),
            ),
          if (_source == 'copy')
            DropdownButtonHideUnderline(
              child: DropdownButton<int>(
                isExpanded: true,
                hint: Text(t.t('group.batchOverrideCopySelect')),
                value: _copyPlatformId,
                items: [
                  for (final p in c.platforms)
                    DropdownMenuItem(value: p.id, child: Text(p.name)),
                ],
                onChanged: (v) {
                  if (v == null) return;
                  PlatformRow? src;
                  for (final p in c.platforms) {
                    if (p.id == v) src = p;
                  }
                  setState(() {
                    _copyPlatformId = v;
                    if (src != null) _slots = _modelsOf(src.models);
                  });
                },
              ),
            ),
          const SizedBox(height: AidogSpace.ssm),
          for (final s in kModelSlots)
            Padding(
              padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
              child: Row(
                children: [
                  SizedBox(
                    width: 70,
                    child: Text(t.t(s.labelKey), style: AidogType.micro.copyWith(color: theme.c.fg3)),
                  ),
                  Expanded(
                    child: TextField(
                      controller: TextEditingController(text: _slots[s.key] ?? '')
                        ..selection = TextSelection.collapsed(offset: (_slots[s.key] ?? '').length),
                      style: AidogType.micro.copyWith(color: theme.c.fg),
                      onChanged: (v) => setState(() => _slots = {..._slots, s.key: v}),
                    ),
                  ),
                ],
              ),
            ),
          // 全空 → 确认按钮禁用，把原因写出来（React 是按钮 title，这里是一行提示）。
          if (allEmpty)
            Text(
              t.t('group.batchOverrideAllEmptyHint'),
              style: AidogType.micro.copyWith(color: theme.c.bad),
            ),
          Text(
            t.t('group.batchOverrideDiffTitle'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          for (final p in target)
            for (final s in kModelSlots)
              if ((p.models.toJson()[s.key] as String? ?? '').isNotEmpty ||
                  (_slots[s.key] ?? '').isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(left: AidogSpace.ssm, bottom: 1),
                  child: Text(
                    '${p.name} · ${t.t(s.labelKey)}: '
                    '${(p.models.toJson()[s.key] as String?) ?? t.t('group.batchOverrideEmpty')} → '
                    '${(_slots[s.key]?.isNotEmpty ?? false) ? _slots[s.key] : t.t('group.batchOverrideEmpty')}',
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                ),
        ],
      ),
    );
  }
}

/// 批量改状态弹窗（`BatchSetStatusModal.tsx`）：启用/禁用 radio +
/// 「整组将无候选」警告。
class _BatchSetStatusCard extends StatefulWidget {
  const _BatchSetStatusCard({required this.controller});

  final GroupsController controller;

  @override
  State<_BatchSetStatusCard> createState() => _BatchSetStatusCardState();
}

class _BatchSetStatusCardState extends State<_BatchSetStatusCard> {
  String _status = 'disabled';

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = widget.controller;
    final target = c.batchSetStatusTarget;
    if (target == null) return const SizedBox.shrink();
    final selectedIds = {for (final p in target.platforms) p.id};
    final willEmpty = _status == 'disabled' &&
        c.batchSetStatusGroupEnabledIds.isNotEmpty &&
        c.batchSetStatusGroupEnabledIds.every(selectedIds.contains);
    return ConfirmCard(
      title: t.t('group.batchSetStatusTitle'),
      body: t.t('group.batchSetStatusDesc', {'count': '${target.platforms.length}'}),
      confirmLabel: c.batchSetStatusBusy
          ? t.t('group.batchSetStatusApplying')
          : t.t('group.batchSetStatusConfirm', {'count': '${target.platforms.length}'}),
      busy: c.batchSetStatusBusy,
      onCancel: c.cancelBatchSetStatus,
      onConfirm: c.batchSetStatusBusy
          ? null
          : () => c.confirmBatchSetStatus(
              _status,
              doneText: (n) => t.t('group.batchSetStatusDone', {'count': '$n'}),
              failText: t.t('group.batchSetStatusFailed'),
            ),
      extra: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Wrap(
            spacing: AidogSpace.sxs,
            children: [
              for (final s in const ['disabled', 'enabled'])
                SmallButton(
                  label: s == 'enabled'
                      ? t.t('group.batchSetStatusEnabled')
                      : t.t('group.batchSetStatusDisabled'),
                  active: _status == s,
                  onTap: () => setState(() => _status = s),
                ),
            ],
          ),
          if (willEmpty)
            Padding(
              padding: const EdgeInsets.only(top: AidogSpace.sxs),
              child: Text(
                t.t('group.batchSetStatusNoCandidateWarning'),
                style: AidogType.micro.copyWith(color: theme.c.bad),
              ),
            ),
        ],
      ),
    );
  }
}

/// 批量移组弹窗（`BatchMoveGroupModal.tsx`）：目标组下拉 + move/add radio。
class _BatchMoveGroupCard extends StatefulWidget {
  const _BatchMoveGroupCard({required this.controller});

  final GroupsController controller;

  @override
  State<_BatchMoveGroupCard> createState() => _BatchMoveGroupCardState();
}

class _BatchMoveGroupCardState extends State<_BatchMoveGroupCard> {
  int? _targetGroupId;
  String _mode = 'move';

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = widget.controller;
    final target = c.batchMoveGroupTarget;
    if (target == null) return const SizedBox.shrink();
    final isCurrentGroup = _targetGroupId == target.groupId;
    final canConfirm = _targetGroupId != null && !isCurrentGroup;
    return ConfirmCard(
      title: t.t('group.batchMoveGroupTitle'),
      body: t.t('group.batchMoveGroupDesc', {'count': '${target.platforms.length}'}),
      confirmLabel: c.batchMoveGroupBusy
          ? t.t('group.batchMoveGroupApplying')
          : t.t('group.batchMoveGroupConfirm', {
              'count': '${target.platforms.length}',
              'mode': _mode == 'move'
                  ? t.t('group.batchMoveGroupModeMoveShort')
                  : t.t('group.batchMoveGroupModeAddShort'),
            }),
      busy: c.batchMoveGroupBusy,
      onCancel: c.cancelBatchMoveGroup,
      onConfirm: (c.batchMoveGroupBusy || !canConfirm)
          ? null
          : () => c.confirmBatchMoveGroup(
              _targetGroupId!,
              _mode,
              doneText: (n, m) => t.t('group.batchMoveGroupDone', {
                'count': '$n',
                'mode': m == 'move'
                    ? t.t('group.batchMoveGroupModeMoveShort')
                    : t.t('group.batchMoveGroupModeAddShort'),
              }),
              failText: t.t('group.batchMoveGroupFailed'),
            ),
      extra: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(t.t('group.batchMoveGroupTarget'), style: AidogType.micro.copyWith(color: theme.c.fg3)),
          DropdownButtonHideUnderline(
            child: DropdownButton<int>(
              isExpanded: true,
              hint: Text(t.t('group.batchMoveGroupSelect')),
              value: _targetGroupId,
              items: [
                for (final og in c.allGroups)
                  DropdownMenuItem(
                    value: og.id,
                    child: Text(
                      og.id == target.groupId
                          ? '${og.name} (${t.t('group.batchMoveGroupCurrent')})'
                          : og.name,
                    ),
                  ),
              ],
              onChanged: (v) => setState(() => _targetGroupId = v),
            ),
          ),
          if (isCurrentGroup)
            Padding(
              padding: const EdgeInsets.only(top: AidogSpace.sxs),
              child: Text(
                t.t('group.batchMoveGroupSameAsCurrent'),
                style: AidogType.micro.copyWith(color: theme.c.bad),
              ),
            ),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            children: [
              for (final m in const ['move', 'add'])
                SmallButton(
                  label: m == 'move'
                      ? t.t('group.batchMoveGroupModeMove')
                      : t.t('group.batchMoveGroupModeAdd'),
                  active: _mode == m,
                  onTap: () => setState(() => _mode = m),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class _GroupTestPanel extends StatelessWidget {
  const _GroupTestPanel({required this.controller});

  final GroupsController controller;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final gt = controller.groupTest!;
    // 行状态文案逐条对齐 `GroupTestPanel.tsx:47-52`：ok 带耗时，testing 是省略号。
    final ok = gt.rows.where((r) => r.status == 'ok').length;
    final fail = gt.rows.where((r) => r.status == 'fail').length;
    String statusText(GroupTestRow r) => switch (r.status) {
      'testing' => '…',
      'pending' => t.t('group.testAllPending'),
      'ok' =>
        t.t('group.testAllOk') + (r.durationMs == null ? '' : ' ${r.durationMs}ms'),
      _ => t.t('group.testAllFail'),
    };
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: '${t.t('group.testAllTitle')}：${gt.groupName}',
        meta: gt.running
            ? t.t('group.testAllProgress', {
                'done': '${ok + fail}',
                'total': '${gt.rows.length}',
              })
            : t.t('group.testAllSummary', {
                'ok': '$ok',
                'fail': '$fail',
                'total': '${gt.rows.length}',
              }),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final r in gt.rows)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 2),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      children: [
                        Expanded(
                          child: Text(
                            r.name,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: AidogType.micro.copyWith(color: theme.c.fg2),
                          ),
                        ),
                        Text(
                          statusText(r),
                          style: AidogType.micro.copyWith(
                            color: switch (r.status) {
                              'ok' => theme.c.ok,
                              'fail' => theme.c.bad,
                              _ => theme.c.fg3,
                            },
                          ),
                        ),
                      ],
                    ),
                    if (r.status == 'fail' && (r.error ?? '').isNotEmpty)
                      Text(
                        r.error!,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.micro.copyWith(color: theme.c.bad),
                      ),
                  ],
                ),
              ),
            const SizedBox(height: AidogSpace.ssm),
            Align(
              alignment: Alignment.centerRight,
              child: SmallButton(
                label: t.t('action.close'),
                onTap: controller.closeGroupTest,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ── 新建态 ────────────────────────────────────────────────────────

class _GroupCreatePanel extends StatelessWidget {
  const _GroupCreatePanel({required this.controller});

  final GroupsController controller;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = controller;
    return Tile(
      title: t.t('group.add'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          TileMeta(t.t('group.basicInfo')),
          const SizedBox(height: AidogSpace.sxs),
          _Field(
            label: t.t('group.name'),
            hint: t.t('group.nameHint'),
            value: c.createName,
            onChanged: c.setCreateName,
          ),
          _Field(
            label: t.t('group.groupKey'),
            hint: t.t('group.groupKeyHint'),
            // 输入即过滤非法字符：密钥创建后锁定不可改，放进去一个空格就废了。
            value: c.createGroupKey,
            onChanged: c.setCreateGroupKey,
          ),
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('group.routingMode')),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            children: [
              for (final m in kRoutingModes)
                SmallButton(
                  label: routingLabel(t, m),
                  active: c.createMode == m,
                  onTap: () => c.setCreateMode(m),
                ),
            ],
          ),
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.sxs),
            child: Text(
              routingDesc(t, c.createMode),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('group.platforms')),
          Text(
            t.t('group.platformsHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.sxs),
          // 只列 enabled 的平台（与 React 的 createPlatformOptions 同口径）。
          _PlatformPicker(
            platformIds: c.createPlatformIds,
            options: c.createPlatformOptions,
            onChange: c.setCreatePlatformIds,
          ),
          const SizedBox(height: AidogSpace.smd),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SmallButton(label: t.t('action.cancel'), onTap: c.closeCreate),
              const SizedBox(width: AidogSpace.ssm),
              // 名字为空就点不动 —— React 的 `disabled={!cName}`。
              SmallButton(
                label: t.t('action.create'),
                onTap: c.canCreate
                    ? () => c.createGroup(failText: t.t('group.createFailed'))
                    : null,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// 关联平台选择器（`src/domains/groups/PlatformPicker.tsx`）：
/// 已选平台按顺序排（顺序 = 优先级），可拖拽重排、可移除；下拉添加未选平台。
class _PlatformPicker extends StatelessWidget {
  const _PlatformPicker({
    required this.platformIds,
    required this.options,
    required this.onChange,
  });

  final List<int> platformIds;
  final List<PlatformRow> options;
  final ValueChanged<List<int>> onChange;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final remaining = [
      for (final p in options)
        if (!platformIds.contains(p.id)) p,
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (platformIds.isNotEmpty)
          ReorderableListView.builder(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            itemCount: platformIds.length,
            onReorderItem: (o, n) {
              final next = [...platformIds];
              next.insert(n, next.removeAt(o));
              onChange(next);
            },
            itemBuilder: (context, i) {
              final pid = platformIds[i];
              PlatformRow? p;
              for (final o in options) {
                if (o.id == pid) p = o;
              }
              return Padding(
                key: ValueKey(pid),
                padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
                child: Row(
                  children: [
                    Tooltip(
                      message: t.t('group.dragToReorder'),
                      child: Icon(Icons.drag_handle, size: 14, color: theme.c.fg3),
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                    Text(
                      '${i + 1}',
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                    Expanded(
                      child: Text(
                        p?.name ?? '#$pid',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.micro.copyWith(color: theme.c.fg2),
                      ),
                    ),
                    IconButton(
                      padding: EdgeInsets.zero,
                      constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
                      iconSize: 14,
                      icon: const Icon(Icons.close),
                      onPressed: () => onChange([
                        for (final id in platformIds)
                          if (id != pid) id,
                      ]),
                    ),
                  ],
                ),
              );
            },
          ),
        if (remaining.isNotEmpty)
          DropdownButtonHideUnderline(
            child: DropdownButton<int>(
              isExpanded: true,
              hint: Text(t.t('group.addPlatform')),
              value: null,
              items: [
                for (final p in remaining)
                  DropdownMenuItem(value: p.id, child: Text(p.name)),
              ],
              onChanged: (v) {
                if (v != null) onChange([...platformIds, v]);
              },
            ),
          ),
      ],
    );
  }
}

// ── 编辑态 ────────────────────────────────────────────────────────

class _GroupEditPanel extends StatefulWidget {
  const _GroupEditPanel({
    required this.controller,
    this.copyText = native.writeText,
  });

  final GroupsController controller;
  final Future<void> Function(String text) copyText;

  @override
  State<_GroupEditPanel> createState() => _GroupEditPanelState();
}

class _GroupEditPanelState extends State<_GroupEditPanel> {
  /// pi 线路协议存 `group.extra`，不在 UpdateGroup 字段集里 → 本地态 + 选中即写
  /// （`GroupEditPanel.tsx:41`）。
  late String _piApi = parseGroupPiApi(
    widget.controller.edit.target?.group.extra ?? '',
  );

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = widget.controller;
    final e = c.edit;
    final g = e.target!.group;
    final envVars = [...e.envVars, ...c.proxyEnvVars];
    final hasReserved = e.envVars.any(
      (ev) =>
          ev.key == 'ANTHROPIC_BASE_URL' ||
          ev.key == 'ANTHROPIC_AUTH_TOKEN' ||
          ev.key == 'AIDOG_KEY',
    );
    return Tile(
      title: t.t('group.edit'),
      meta: g.groupKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 头部复制按钮组（`GroupEditPanel.tsx:63-66`）。
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            children: [
              Tooltip(
                message: t.t('group.copyApiKeyTitle'),
                child: SmallButton(
                  label: t.t('group.apiKey'),
                  onTap: () => widget.copyText(g.groupKey),
                ),
              ),
              Tooltip(
                message: t.t('group.copyCommand'),
                child: SmallButton(
                  label: 'Claude',
                  onTap: () => widget.copyText(buildClaudeCommand(g.groupKey)),
                ),
              ),
              Tooltip(
                message: t.t('group.copyCodexCommand'),
                child: SmallButton(
                  label: 'Codex',
                  onTap: () =>
                      widget.copyText(buildCodexCommand(g.groupKey, envVars)),
                ),
              ),
              Tooltip(
                message: t.t('group.copyPiCommand'),
                child: SmallButton(
                  label: 'pi',
                  onTap: () =>
                      widget.copyText(buildPiCommand(g.groupKey, envVars)),
                ),
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('group.basicInfo')),
          const SizedBox(height: AidogSpace.sxs),
          _Field(
            label: t.t('group.name'),
            value: e.name,
            onChanged: (v) => c.patchEdit(e.patch(name: v)),
          ),
          // 分组密钥：创建后锁定不可改（只读展示 + 说明）。
          TileMeta(t.t('group.groupKey')),
          Text(
            g.groupKey,
            style: AidogType.micro.copyWith(color: theme.c.fg2),
          ),
          Text(
            t.t('group.groupKeyLocked'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('group.routingMode')),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            children: [
              for (final m in kRoutingModes)
                SmallButton(
                  label: routingLabel(t, m),
                  active: e.mode == m,
                  onTap: () => c.patchEdit(e.patch(mode: m)),
                ),
            ],
          ),
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.sxs),
            child: Text(
              routingDesc(t, e.mode),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
          const SizedBox(height: AidogSpace.ssm),
          // pi 线路协议：写 group.extra 即时生效（不参与 onSave 的字段集）。
          TileMeta(t.t('group.piApiLabel')),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            children: [
              for (final api in kPiApis)
                SmallButton(
                  label: piApiLabel(t.t, api),
                  active: _piApi == api,
                  onTap: () {
                    setState(() => _piApi = api);
                    unawaited(c.setGroupPiApi(g.id, api));
                  },
                ),
            ],
          ),
          Text(
            t.t('group.piApiHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.ssm),
          _NumField(
            label: t.t('group.maxRetries'),
            value: e.maxRetries,
            onChanged: (v) => c.patchEdit(e.patch(maxRetries: v)),
          ),
          Text(
            t.t('group.maxRetriesHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.sxs),
          TileMeta(t.t('group.timeout')),
          _NumField(
            label: t.t('group.reqTimeout'),
            value: e.reqTimeout,
            onChanged: (v) => c.patchEdit(e.patch(reqTimeout: v)),
          ),
          _NumField(
            label: t.t('group.connTimeout'),
            value: e.connTimeout,
            onChanged: (v) => c.patchEdit(e.patch(connTimeout: v)),
          ),
          Text(
            t.t('group.timeoutDefault'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          // 自动建组的说明（`GroupEditPanel.tsx:154-159`）。
          if (g.autoFromPlatform.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(top: AidogSpace.sxs),
              child: Text(
                t.t('group.autoFromPlatform'),
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              ),
            ),
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('group.platforms')),
          Text(
            t.t('group.platformsHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.sxs),
          _PlatformPicker(
            platformIds: e.platformIds,
            options: c.platforms,
            onChange: (ids) => c.patchEdit(e.patch(platformIds: ids)),
          ),
          const SizedBox(height: AidogSpace.ssm),
          // 模型映射：逐行可编辑 + 删除 + 新增一行空映射。
          TileMeta(t.t('group.modelMappings')),
          Text(
            t.t('group.mappingsHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          for (var i = 0; i < e.mappings.length; i++)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                children: [
                  SizedBox(
                    width: 130,
                    child: _Field(
                      hint: t.t('mapping.source'),
                      value: e.mappings[i].sourceModel,
                      onChanged: (v) {
                        final next = [...e.mappings];
                        next[i] = ModelMapping(
                          sourceModel: v,
                          targetPlatformId: next[i].targetPlatformId,
                          targetModel: next[i].targetModel,
                          requestTimeoutSecs: next[i].requestTimeoutSecs,
                          connectTimeoutSecs: next[i].connectTimeoutSecs,
                        );
                        c.patchEdit(e.patch(mappings: next));
                      },
                    ),
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  DropdownButtonHideUnderline(
                    child: DropdownButton<int>(
                      hint: Text(t.t('mapping.targetPlatform')),
                      value: e.mappings[i].targetPlatformId == 0
                          ? null
                          : e.mappings[i].targetPlatformId,
                      items: [
                        for (final p in c.platforms)
                          if (p.enabled)
                            DropdownMenuItem(value: p.id, child: Text(p.name)),
                      ],
                      onChanged: (v) {
                        final next = [...e.mappings];
                        next[i] = ModelMapping(
                          sourceModel: next[i].sourceModel,
                          targetPlatformId: v ?? 0,
                          targetModel: '',
                          requestTimeoutSecs: next[i].requestTimeoutSecs,
                          connectTimeoutSecs: next[i].connectTimeoutSecs,
                        );
                        c.patchEdit(e.patch(mappings: next));
                      },
                    ),
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  Expanded(
                    child: _Field(
                      hint: t.t('mapping.target'),
                      value: e.mappings[i].targetModel,
                      onChanged: (v) {
                        final next = [...e.mappings];
                        next[i] = ModelMapping(
                          sourceModel: next[i].sourceModel,
                          targetPlatformId: next[i].targetPlatformId,
                          targetModel: v,
                          requestTimeoutSecs: next[i].requestTimeoutSecs,
                          connectTimeoutSecs: next[i].connectTimeoutSecs,
                        );
                        c.patchEdit(e.patch(mappings: next));
                      },
                    ),
                  ),
                  SmallButton(
                    label: t.t('action.delete'),
                    danger: true,
                    onTap: () {
                      final next = [...e.mappings]..removeAt(i);
                      c.patchEdit(e.patch(mappings: next));
                    },
                  ),
                ],
              ),
            ),
          Align(
            alignment: Alignment.centerLeft,
            child: SmallButton(
              label: '+ ${t.t('mapping.add')}',
              onTap: () => c.patchEdit(
                e.patch(
                  mappings: [
                    ...e.mappings,
                    const ModelMapping(
                      sourceModel: '',
                      targetPlatformId: 0,
                      targetModel: '',
                      requestTimeoutSecs: 0,
                      connectTimeoutSecs: 0,
                    ),
                  ],
                ),
              ),
            ),
          ),
          const SizedBox(height: AidogSpace.ssm),
          // 分组维度环境变量：sync 注入 Claude settings.env，复制 Codex 命令时前置 export。
          TileMeta(t.t('group.envVars')),
          Text(
            t.t('group.envVarsHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          for (var i = 0; i < e.envVars.length; i++)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                children: [
                  SizedBox(
                    width: 190,
                    child: _Field(
                      hint: t.t('group.envVarKey'),
                      value: e.envVars[i].key,
                      onChanged: (v) {
                        final next = [...e.envVars];
                        next[i] = EnvVar(key: v, value: next[i].value);
                        c.patchEdit(e.patch(envVars: next));
                      },
                    ),
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  Expanded(
                    child: _Field(
                      hint: t.t('group.envVarValue'),
                      value: e.envVars[i].value,
                      onChanged: (v) {
                        final next = [...e.envVars];
                        next[i] = EnvVar(key: next[i].key, value: v);
                        c.patchEdit(e.patch(envVars: next));
                      },
                    ),
                  ),
                  SmallButton(
                    label: t.t('action.delete'),
                    danger: true,
                    onTap: () {
                      final next = [...e.envVars]..removeAt(i);
                      c.patchEdit(e.patch(envVars: next));
                    },
                  ),
                ],
              ),
            ),
          if (hasReserved)
            Text(
              t.t('group.envVarReservedHint'),
              style: AidogType.micro.copyWith(color: theme.c.bad),
            ),
          Align(
            alignment: Alignment.centerLeft,
            child: SmallButton(
              label: '+ ${t.t('group.addEnvVar')}',
              onTap: () => c.patchEdit(
                e.patch(
                  envVars: [...e.envVars, const EnvVar(key: '', value: '')],
                ),
              ),
            ),
          ),
          const SizedBox(height: AidogSpace.smd),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SmallButton(label: t.t('action.cancel'), onTap: c.cancelEdit),
              const SizedBox(width: AidogSpace.ssm),
              // 名字为空就点不动 —— React 的 `disabled={!editName}`。
              SmallButton(
                label: t.t('action.save'),
                onTap: e.canSave
                    ? () => c.saveEdit(failText: t.t('group.saveFailed'))
                    : null,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// 受控文本框。
///
/// 值由上层控制器持有（分组密钥要在输入时就滤掉非法字符，所以不能是非受控的），
/// 但 `TextEditingController` **必须活在 State 里**：放在 build 里 new 一个，
/// 每帧都是新实例 —— 光标每次输入都跳回开头，而且旧实例从不 dispose。
class _Field extends StatefulWidget {
  const _Field({
    required this.value,
    required this.onChanged,
    this.label,
    this.hint,
  });

  /// null = 不画标题行（映射 / 环境变量这种行内小框，React 那边也只有 placeholder）。
  final String? label;
  final String value;
  final String? hint;
  final ValueChanged<String> onChanged;

  @override
  State<_Field> createState() => _FieldState();
}

class _FieldState extends State<_Field> {
  late final TextEditingController _ctrl = TextEditingController(
    text: widget.value,
  );

  @override
  void didUpdateWidget(_Field old) {
    super.didUpdateWidget(old);
    // 外部把值改了（比如密钥被过滤掉了几个字符）才同步回输入框，
    // 并把光标放到末尾；值没变就不碰，免得打断用户正在选的那一段。
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

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          if (widget.label != null) TileMeta(widget.label!),
          TextField(
            controller: _ctrl,
            style: AidogType.micro.copyWith(color: theme.c.fg),
            decoration: InputDecoration(
              isDense: true,
              hintText: widget.hint,
              hintStyle: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
            onChanged: widget.onChanged,
          ),
        ],
      ),
    );
  }
}

class _NumField extends StatelessWidget {
  const _NumField({
    required this.label,
    required this.value,
    required this.onChanged,
  });

  final String label;
  final int value;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) => _Field(
    label: label,
    value: '$value',
    // 非数字 / 空 → 0，与 React 那边 `Number(v) || 0` 同语义。
    onChanged: (v) => onChanged(int.tryParse(v.trim()) ?? 0),
  );
}

/// 策略短名：优先 i18n，取不到回落 mode 字面量（与 `routing.ts` 的 `?? mode` 一致）。
String routingLabel(I18nController t, String mode) {
  final e = kRoutingModeLabels[mode];
  if (e == null) return mode;
  final s = t.t(e.$1);
  return s == e.$1 ? e.$2 : s;
}

/// 策略说明：取不到回落**空串**（`routing.ts` 是 `?? ""`，与 label 不同）。
String routingDesc(I18nController t, String mode) {
  final e = kRoutingModeDescs[mode];
  if (e == null) return '';
  final s = t.t(e.$1);
  return s == e.$1 ? e.$2 : s;
}
