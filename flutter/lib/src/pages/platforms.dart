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
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'groups.dart';
import 'invoke.dart';
import 'model_test.dart';
import 'model_test_logic.dart';
import 'models.dart';
import 'platform_card_bits.dart';
import 'platform_card_view.dart';
import 'platform_form.dart';
import 'platform_form_logic.dart';
import 'platforms_logic.dart';
import 'share_panel.dart';
import 'ui_bits.dart';

class PlatformsPage extends StatefulWidget {
  const PlatformsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.logUpdates,
    this.onNavigate,
    this.showGroups = true,
    this.onEditPlatform,
    this.onDuplicatePlatform,
  });

  final InvokeFn invoke;

  /// 卡片「编辑」按钮。表单是另一张票，这里只留接口：没接上就不渲染这颗按钮。
  final void Function(PlatformRow p)? onEditPlatform;

  /// 卡片「复制平台」按钮（`usePlatformForm.ts:405::handleDuplicate`：
  /// 用这个平台的字段预填**新建**表单，不是直接建一份）。同样等表单票接。
  final void Function(PlatformRow p)? onDuplicatePlatform;

  /// 「有新请求日志」流；缺省 500ms 防抖。收到只做轻量统计刷新（不重拉整表）。
  final Stream<void>? logUpdates;

  /// 侧栏切页，可带跨页预筛参数（平台卡「查看日志」带 platformId，
  /// 分组卡「查看统计」带 groupKey）。宿主把它们包进 `NavContext` 交给 `navigate`。
  final void Function(String id, {int? platformId, String? groupKey})?
  onNavigate;

  /// 关掉可以单独渲染「未分组平台」那一半，widget 测试用它把两页拆开测。
  final bool showGroups;

  @override
  State<PlatformsPage> createState() => _PlatformsPageState();
}

class _PlatformsPageState extends State<PlatformsPage> {
  late final PlatformsController _c;

  /// 新增 / 编辑表单。`showForm` 为 true 时整页换成表单（与 React 的
  /// `Platforms.tsx:81` 同一条：表单是页面的另一种形态，不是弹窗）。
  late final PlatformFormController _form;

  /// 票 I09：模型测试面板的目标平台。null = 面板关着。
  PlatformRow? _testPanelTarget;

  /// 分享面板的数据：`platform_share_export` 的返回 + 平台名。null = 面板关着。
  ({Map<String, Object?> share, String name})? _shareData;
  StreamSubscription<void>? _sub;
  ({String text, bool ok})? _toast;
  Timer? _toastTimer;

  /// 分组区交上来的「打开新建分组」，给页头那颗「+ 添加分组」用
  /// （React 的 `openCreateGroupRef`，`PlatformListView.tsx:104-106`）。
  VoidCallback? _openCreateGroup;

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
    _form = PlatformFormController(
      list: _c,
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    _form.init(locale: i18n.locale);
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
  void didChangeDependencies() {
    super.didChangeDependencies();
    // 切语言要重取协议 label（registry 的 name 是 8 locale 的 map）。
    _c.setLocale(AidogI18n.of(context).locale);
  }

  @override
  void dispose() {
    _sub?.cancel();
    _toastTimer?.cancel();
    _c.dispose();
    super.dispose();
  }

  Future<void> _openShare(PlatformRow p) async {
    final share = await _c.shareExport(p);
    if (!mounted || share == null) return;
    setState(() => _shareData = (share: share, name: p.name));
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
    // 表单打开时整页换成表单（React `Platforms.tsx:81-83` 的 `showForm` 分支）。
    if (_form.showForm) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          PlatformEditForm(controller: _form),
          if (_toast != null) ToastBar(text: _toast!.text, ok: _toast!.ok),
        ],
      );
    }
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
              // 缺口清单「平台页缺口」#37：页头「+ 添加分组」。分组区没渲染
              //（`showGroups: false` 的单页测试）时按钮不出现。
              if (widget.showGroups)
                SmallButton(
                  label: '+ ${t.t('group.add')}',
                  onTap: _openCreateGroup,
                ),
              // 缺口清单「平台页缺口」#2：页头「+ 添加平台」。
              SmallButton(
                label: '+ ${t.t('platform.add')}',
                onTap: () => _form.openCreatePlatform(),
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
            // 分组区是子 widget，`openCreate` 在它 initState 里才拿得到；
            // 拿到时页头这一帧已经画完了，所以补一帧重建让按钮可点。
            onCreateGroupReady: (open) {
              if (_openCreateGroup != null) return;
              _openCreateGroup = open;
              WidgetsBinding.instance.addPostFrameCallback((_) {
                if (mounted) setState(() {});
              });
            },
            // 缺口 #8：未分组平台拖进分组。
            onPlatformDropped: _c.moveIntoGroup,
            // 分组卡里的「在此分组添加平台」：打开同页创建表单并锁定归属分组。
            onCreatePlatform: _form.openCreatePlatform,
            // 分组卡里的「查看统计」/「查看日志」：切页并带上分组名预筛
            // （React `Groups.tsx` 走同一条 NavContext.groupKey）。
            onNavigate: (id, {String? groupKey}) =>
                widget.onNavigate?.call(id, groupKey: groupKey),
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
          // 拖拽排序（`usePlatformsState.ts:251::reorder`）：手柄在卡片最左侧，
          // 松手按新顺序把整串 id 发给后端。列表自身不滚动（外层已有滚动容器）。
          ReorderableListView.builder(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            buildDefaultDragHandles: false,
            itemCount: _c.standalonePlatforms.length,
            onReorderItem: (o, n) => unawaited(_c.reorderStandalone(o, n)),
            itemBuilder: (context, i) {
              final p = _c.standalonePlatforms[i];
              return Padding(
                key: ValueKey(p.id),
                padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                // 缺口 #8：按住卡片空白区拖到上方任一分组卡即加入该分组。
                // 排序手柄自带 ReorderableDragStartListener（更靠内层，手势竞技场里
                // 先注册先胜出），所以从手柄起手的拖拽仍然是排序不是入组。
                child: Draggable<int>(
                  data: p.id,
                  dragAnchorStrategy: pointerDragAnchorStrategy,
                  feedback: _DragLabel(name: p.name),
                  child: PlatformCard(
                  c: _c,
                  platform: p,
                  index: i,
                  usage: _c.usageMap[p.id],
                  quota: _c.quotaMap[p.id],
                  quotaPending: _c.quotaPending[p.id] == true,
                  quotaRefreshing: _c.quotaRefreshing[p.id] == true,
                  lastTest: _c.lastTestMap[p.id],
                  testing: _c.testingId == p.id,
                  onToggle: () => _c.togglePlatform(p),
                  onTest: () => _c.quickTest(p),
                  // 票 I09：完整的模型测试面板（六种模式），对应 React 的
                  // `ModelTestPanel`。上面的 onTest 是平台卡自带的一键快测，两者并存。
                  onModelTest: () => setState(() => _testPanelTarget = p),
                  onRefreshQuota: () => _c.refreshQuota(p),
                  onDelete: () => _c.askDelete(p.id),
                  onViewLogs: () =>
                      widget.onNavigate?.call('logs', platformId: p.id),
                  onShare: () => _openShare(p),
                  // 缺口 #3 / #4：两颗按钮都由本页自带的表单控制器接管
                  // （`platform_form_logic.dart:263 handleEdit` / `:279 handleDuplicate`）。
                  // 外部若另传了回调就优先用外部的，便于宿主页覆盖跳转行为。
                  onEdit: () =>
                      (widget.onEditPlatform ?? _form.handleEdit)(p),
                  onDuplicate: () =>
                      (widget.onDuplicatePlatform ?? _form.handleDuplicate)(p),
                  ),
                ),
              );
            },
          ),
        if (_c.purgeCandidates != null)
          ConfirmCard(
            title: t.t('platform.purgeDisabled'),
            body: _c.purgeCandidates!.isEmpty
                ? t.t('platform.purgeDisabledNone')
                : t.t('platform.purgeDisabledConfirm'),
            confirmLabel: t.t('action.confirm'),
            onCancel: _c.cancelPurgeDisabled,
            // 候选为空时确认按钮不可点（React `PlatformListView.tsx:291` 的 disabled 同判据）。
            onConfirm: _c.purgeCandidates!.isEmpty
                ? null
                : () => _c.confirmPurgeDisabled(
                    noneText: t.t('platform.purgeDisabledNone'),
                  ),
            // 清单 + 失效原因 badge（`PlatformListView.tsx:270-288`）。
            extra: _c.purgeCandidates!.isEmpty
                ? null
                : Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        t.t('platform.purgeDisabledListTitle'),
                        style: AidogType.micro.copyWith(
                          color: AidogTheme.of(context).c.fg3,
                        ),
                      ),
                      const SizedBox(height: AidogSpace.sxs),
                      for (final cand in _c.purgeCandidates!)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 3),
                          child: Row(
                            children: [
                              Expanded(
                                child: Text(
                                  cand.name,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: AidogType.caption.copyWith(
                                    color: AidogTheme.of(context).c.fg2,
                                  ),
                                ),
                              ),
                              const SizedBox(width: AidogSpace.ssm),
                              MiniBadge(
                                text: cand.reason == 'auth_failed'
                                    ? t.t(
                                        'platform.purgeDisabledReasonAuthFailed',
                                      )
                                    : t.t('platform.purgeDisabledReasonExpired'),
                                color: AidogTheme.of(context).c.fg3,
                              ),
                            ],
                          ),
                        ),
                    ],
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
        if (_testPanelTarget != null)
          ModelTestPanel(
            invoke: widget.invoke,
            platform: TestTargetPlatform.fromPlatformRow(_testPanelTarget!),
            onClose: () => setState(() => _testPanelTarget = null),
            // 整轮跑完刷一次统计：每条测试都落 proxy_log(source_protocol='test')。
            onResult: (_) => _c.refreshStats(),
          ),
        if (_shareData != null)
          SharePanel(
            share: _shareData!.share,
            title: _shareData!.name,
            urlScheme: 'aidog://platform/import',
            onToast: _showToast,
            onClose: () => setState(() => _shareData = null),
          ),
        if (_toast != null) ToastBar(text: _toast!.text, ok: _toast!.ok),
      ],
    );
  }
}

/// 拖拽时跟着指针走的小标签（`PlatformListView.tsx:223-235` 的 groupDrag portal）。
class _DragLabel extends StatelessWidget {
  const _DragLabel({required this.name});

  final String name;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Transform.translate(
      // React 那边标签偏在指针右下 14px，这里照抄。
      offset: const Offset(14, 14),
      child: Material(
        color: Colors.transparent,
        child: Container(
          padding: const EdgeInsets.symmetric(
            horizontal: AidogSpace.smd,
            vertical: AidogSpace.sxs,
          ),
          decoration: BoxDecoration(
            // React 用的是实心 accent + primary-foreground；token 表里没有
            // 「accent 上的前景色」，所以换成同一套里的 wash + 描边 + accentText。
            color: theme.c.accentWash,
            border: Border.all(color: theme.c.accent),
            borderRadius: BorderRadius.circular(AidogRadius.md),
          ),
          child: Text(
            name,
            style: AidogType.micro.copyWith(
              color: theme.c.accentText,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      ),
    );
  }
}

