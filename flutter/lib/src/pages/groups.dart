/// 分组界面（票 I07），对应 `src/pages/Groups.tsx` 的 `GroupsEmbedded`
/// 与 `src/pages/Groups/{GroupListView,GroupListItem,GroupCreateModal,GroupEditPanel}.tsx`。
///
/// 三个视图态互斥，与 React 的三条早返回一一对应：
///   编辑某个组（`edit.target != null`）→ 新建组（`showCreate`）→ 列表。
///
/// 本文件只负责画。校验（能不能点保存/创建）、破坏性确认、批量操作全在
/// [GroupsController]（`groups_logic.dart`），这里把它们接到按钮上。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../utils/formatters.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'groups_logic.dart';
import 'invoke.dart';
import 'models.dart';
import 'ui_bits.dart';

/// 分组区。内嵌在平台页里（与 React 的 `GroupsEmbedded` 同位置），
/// 也可以单独渲染（widget 测试就是这么用的）。
class GroupsSection extends StatefulWidget {
  const GroupsSection({
    super.key,
    this.invoke = kernelInvoke,
    this.onToast,
    this.onPlatformsDeleted,
  });

  final InvokeFn invoke;
  final void Function(String text, {required bool ok})? onToast;

  /// 分组区删掉平台之后通知父级，让主列表把那几行局部移掉（不整页重拉）。
  final void Function(List<int> ids)? onPlatformsDeleted;

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
  }

  @override
  Widget build(BuildContext context) {
    if (_c.edit.target != null) return _GroupEditPanel(controller: _c);
    if (_c.showCreate) return _GroupCreatePanel(controller: _c);
    return _GroupListView(
      controller: _c,
      onPlatformsDeleted: widget.onPlatformsDeleted,
    );
  }
}

// ── 列表态 ────────────────────────────────────────────────────────

class _GroupListView extends StatelessWidget {
  const _GroupListView({required this.controller, this.onPlatformsDeleted});

  final GroupsController controller;
  final void Function(List<int> ids)? onPlatformsDeleted;

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
            SmallButton(label: t.t('group.add'), onTap: c.openCreate),
          ],
        ),
        const SizedBox(height: AidogSpace.ssm),
        if (c.loading)
          CenteredNote(text: t.t('status.loading'))
        else if (c.details.isEmpty)
          CenteredNote(text: t.t('group.empty'))
        else
          for (final d in c.details)
            Padding(
              padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
              child: _GroupCard(
                controller: c,
                detail: d,
                collapsed: c.collapsedGroups.contains(d.group.id),
              ),
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
        // ── 破坏性确认：四个互斥，同一时刻最多一个 ──
        if (c.deleteGroupTarget != null)
          ConfirmCard(
            title: t.t('group.delete'),
            body: t.t('group.deleteConfirm'),
            confirmLabel: t.t('action.delete'),
            onCancel: c.cancelDeleteGroup,
            onConfirm: c.confirmDeleteGroup,
          ),
        if (c.removeTarget != null)
          _RemovePlatformConfirm(
            controller: c,
            onDeleted: (id) => onPlatformsDeleted?.call([id]),
          ),
        if (c.batchDeleteTarget != null)
          ConfirmCard(
            title: t.t('group.batchDelete'),
            body: '${c.batchDeleteTarget!.platforms.length}',
            confirmLabel: t.t('action.delete'),
            busy: c.batchDeleteBusy,
            extra: c.batchDeleteTarget!.hasCrossGroup
                // 跨组警告：删掉就是从所有组里消失，不只是本组。
                ? _CrossGroupWarning(target: c.batchDeleteTarget!)
                : null,
            onCancel: c.cancelBatchDelete,
            onConfirm: () {
              final ids = [for (final p in c.batchDeleteTarget!.platforms) p.id];
              c.confirmBatchDelete().then((_) => onPlatformsDeleted?.call(ids));
            },
          ),
        if (c.purgeTarget != null)
          ConfirmCard(
            title: t.t('group.purgeDisabled'),
            body: c.purgeTarget!.candidates.isEmpty
                ? t.t('platform.purgeDisabledNone')
                : '${c.purgeTarget!.candidates.length}',
            confirmLabel: t.t('action.confirm'),
            onCancel: c.cancelPurgeDisabled,
            onConfirm: () => c.confirmPurgeDisabled(
              noneText: t.t('platform.purgeDisabledNone'),
            ),
          ),
        if (c.groupTest != null) _GroupTestPanel(controller: c),
      ],
    );
  }
}

class _GroupCard extends StatelessWidget {
  const _GroupCard({
    required this.controller,
    required this.detail,
    required this.collapsed,
  });

  final GroupsController controller;
  final GroupDetail detail;
  final bool collapsed;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = controller;
    final g = detail.group;
    final stats = c.groupStats[g.groupKey];
    final balance = c.groupBalance[g.id];
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
                icon: Icon(
                  collapsed ? Icons.chevron_right : Icons.expand_more,
                ),
                onPressed: () {
                  final next = c.toggleGroupCollapsed(g.id);
                  c.persistGroupCollapsed(g.id, next);
                },
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
                            child: Text(
                              t.t('group.isDefault'),
                              style: AidogType.micro.copyWith(
                                color: theme.c.accentText,
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
                  SmallButton(
                    label: g.isDefault
                        ? t.t('group.unsetDefault')
                        : t.t('group.setAsDefault'),
                    onTap: () => c.toggleDefault(g),
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
          if (!collapsed) ...[
            const SizedBox(height: AidogSpace.ssm),
            if (detail.platforms.isEmpty)
              Text(
                t.t('group.noPlatforms'),
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              )
            else
              for (final gp in detail.platforms)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 3),
                  child: Row(
                    children: [
                      Container(
                        width: 6,
                        height: 6,
                        margin: const EdgeInsets.only(right: AidogSpace.ssm),
                        decoration: BoxDecoration(
                          color: gp.platform.status == 'enabled'
                              ? theme.c.ok
                              : theme.c.fg3,
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
                      // per-group 优先级（1~10，10 最高）。就地改，乐观更新 + 失败回滚。
                      Text(
                        'P${gp.levelPriority}',
                        style: AidogType.micro.copyWith(color: theme.c.fg3),
                      ),
                      const SizedBox(width: AidogSpace.ssm),
                      SmallButton(
                        label: t.t('group.deletePlatformTitle'),
                        onTap: () => c.askRemovePlatform(gp.platform, g.id),
                      ),
                    ],
                  ),
                ),
          ],
        ],
      ),
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
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: t.t('group.deletePlatformTitle'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              target.platform.name,
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
            if (!target.onlyInThisGroup)
              Padding(
                padding: const EdgeInsets.only(top: AidogSpace.sxs),
                child: Text(
                  target.groupNames.join(' / '),
                  style: AidogType.micro.copyWith(color: theme.c.fg3),
                ),
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
                if (!target.onlyInThisGroup) ...[
                  SmallButton(
                    label: t.t('group.removeFromGroupAction'),
                    onTap: controller.removePlatformFromGroup,
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                ],
                SmallButton(
                  label: t.t('group.deletePlatformAction'),
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
    final theme = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final p in target.platforms)
          if ((target.groupNamesByPlatform[p.id] ?? const []).length > 1)
            Text(
              '${p.name}: ${target.groupNamesByPlatform[p.id]!.join(' / ')}',
              style: AidogType.micro.copyWith(color: theme.c.bad),
            ),
      ],
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
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: gt.groupName,
        meta: gt.running ? t.t('status.loading') : t.t('status.done'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final r in gt.rows)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 2),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        r.name,
                        style: AidogType.micro.copyWith(color: theme.c.fg2),
                      ),
                    ),
                    Text(
                      r.durationMs == null
                          ? ''
                          : formatDurationMs(r.durationMs!.toDouble()),
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                    const SizedBox(width: AidogSpace.ssm),
                    Text(
                      r.status,
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
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            children: [
              // 只列 enabled 的平台（与 React 的 createPlatformOptions 同口径）。
              for (final p in c.createPlatformOptions)
                SmallButton(
                  label: p.name,
                  active: c.createPlatformIds.contains(p.id),
                  onTap: () {
                    final next = [...c.createPlatformIds];
                    next.contains(p.id) ? next.remove(p.id) : next.add(p.id);
                    c.setCreatePlatformIds(next);
                  },
                ),
            ],
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
                onTap: c.canCreate ? c.createGroup : null,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

// ── 编辑态 ────────────────────────────────────────────────────────

class _GroupEditPanel extends StatelessWidget {
  const _GroupEditPanel({required this.controller});

  final GroupsController controller;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = controller;
    final e = c.edit;
    return Tile(
      title: t.t('group.edit'),
      meta: e.target!.group.groupKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          _Field(
            label: t.t('group.name'),
            value: e.name,
            onChanged: (v) => c.patchEdit(e.patch(name: v)),
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
          _NumField(
            label: t.t('group.maxRetries'),
            value: e.maxRetries,
            onChanged: (v) => c.patchEdit(e.patch(maxRetries: v)),
          ),
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
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('group.platforms')),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            children: [
              for (final p in c.platforms)
                SmallButton(
                  label: p.name,
                  active: e.platformIds.contains(p.id),
                  onTap: () {
                    final next = [...e.platformIds];
                    next.contains(p.id) ? next.remove(p.id) : next.add(p.id);
                    c.patchEdit(e.patch(platformIds: next));
                  },
                ),
            ],
          ),
          const SizedBox(height: AidogSpace.ssm),
          // 模型映射：列表页的快捷删除（添加走列表卡里的表单）。
          TileMeta(t.t('group.modelMappings')),
          for (var i = 0; i < e.mappings.length; i++)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      '${e.mappings[i].sourceModel} → ${e.mappings[i].targetModel}',
                      style: AidogType.micro.copyWith(color: theme.c.fg2),
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
          const SizedBox(height: AidogSpace.smd),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SmallButton(label: t.t('action.cancel'), onTap: c.cancelEdit),
              const SizedBox(width: AidogSpace.ssm),
              // 名字为空就点不动 —— React 的 `disabled={!editName}`。
              SmallButton(
                label: t.t('action.save'),
                onTap: e.canSave ? c.saveEdit : null,
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
    required this.label,
    required this.value,
    required this.onChanged,
    this.hint,
  });

  final String label;
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
          TileMeta(widget.label),
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
