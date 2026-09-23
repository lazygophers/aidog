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

  /// 正在被拖的那张卡的下标。非 null = 拖拽进行中：那张换成虚线 ghost，
  /// 其余压暗（`PlatformListView.tsx:159-171,203-221`）。
  int? _dragIdx;

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
    _sub = (widget.logUpdates ?? debounceStream(kernelProxyLogUpdated()))
        .listen((_) {
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

  /// 一张平台卡的完整接线。抽出来是因为它有**两个**渲染位置：本页的未分组列表，
  /// 以及分组区展开后的组内列表（票 24）。十六个入参里有一半是本页自己的东西
  /// （模型测试面板、分享面板、表单控制器、跨页跳转），抄第二份必然漂移。
  ///
  /// 分组区拿到的是这个闭包而不是 [PlatformsController]：
  /// 卡片要的余额 / 用量 / logo / 测试结果全在本页这一份控制器里，已经取过数了，
  /// 传闭包等于让分组区**共用本页这一份实时数据**，不多发一轮命令、不出第二份缓存。
  Widget platformCard(PlatformRow p, int index, {bool draggable = true}) =>
      PlatformCard(
        c: _c,
        platform: p,
        index: index,
        draggable: draggable,
        usage: _c.usageMap[p.id],
        quota: _c.quotaMap[p.id],
        quotaPending: _c.quotaPending[p.id] == true,
        quotaRefreshing: _c.quotaRefreshing[p.id] == true,
        lastTest: _c.lastTestMap[p.id],
        testing: _c.testingId == p.id,
        onToggle: () => _c.togglePlatform(p),
        onTest: () => _c.quickTest(p),
        // 票 I09：完整的模型测试面板（六种模式），对应 React 的 `ModelTestPanel`。
        // 上面的 onTest 是平台卡自带的一键快测，两者并存。
        onModelTest: () => setState(() => _testPanelTarget = p),
        onRefreshQuota: () => _c.refreshQuota(p),
        onDelete: () => _c.askDelete(p.id),
        onViewLogs: () => widget.onNavigate?.call('logs', platformId: p.id),
        onShare: () => _openShare(p),
        // 缺口 #3 / #4：两颗按钮都由本页自带的表单控制器接管
        // （`platform_form_logic.dart:263 handleEdit` / `:279 handleDuplicate`）。
        // 外部若另传了回调就优先用外部的，便于宿主页覆盖跳转行为。
        onEdit: () => (widget.onEditPlatform ?? _form.handleEdit)(p),
        onDuplicate: () =>
            (widget.onDuplicatePlatform ?? _form.handleDuplicate)(p),
      );

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
          // `PlatformListView.tsx:106-108`：有平台时「启用数 / 总数 active」，
          // 一个平台都没有时整句换成空态文案。
          subtitle: _c.platforms.isEmpty
              ? t.t('platform.empty')
              : '${_c.enabledCount} / ${_c.platforms.length} active',
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
              // 缺口清单「平台页缺口」#37：页头「+ 添加分组」。分组区没渲染
              //（`showGroups: false` 的单页测试）时按钮不出现。
              if (widget.showGroups)
                SmallButton(
                  label: '+ ${t.t('group.add')}',
                  // `PlatformListView.tsx:118` 没写 variant = 默认实心。
                  filled: true,
                  onTap: _openCreateGroup,
                ),
              // 缺口清单「平台页缺口」#2：页头「+ 添加平台」。
              SmallButton(
                label: '+ ${t.t('platform.add')}',
                // `PlatformListView.tsx:121` 同上。
                filled: true,
                onTap: () => _form.openCreatePlatform(),
              ),
              // 破坏性动作排在两颗「添加」之后，并且弱化成 ghost
              //（`PlatformListView.tsx:125-131`）：它不该和主动作抢同一档视觉重量。
              SmallButton(
                label: t.t('platform.purgeDisabled'),
                ghost: true,
                onTap: _c.askPurgeDisabled,
              ),
            ],
          ),
        ),
        if (widget.showGroups) ...[
          GroupsSection(
            invoke: widget.invoke,
            onToast: _showToast,
            // 搜索框就在这一屏顶部，但原先只过滤未分组平台 ——
            // 已归组的平台一个都搜不到（`groups.dart` 旧注释自述「尚未接入」）。
            searchQuery: _c.searchQuery,
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
            // 票 24：组内渲染与本页同一张平台卡，只是不给拖拽手柄
            //（分组卡本身已在一个 ReorderableListView 里，卡内再套一个会抢手势）。
            buildPlatformCard: (p, i) => platformCard(p, i, draggable: false),
          ),
          const SizedBox(height: AidogSpace.s_2xl),
        ],
        // 未分组区没有标题，只有上面那条分隔线（`PlatformListView.tsx:140`）。
        if (_c.loading)
          CenteredNote(text: t.t('status.loading'))
        // 空态看的是**全部**平台（`PlatformListView.tsx:147`）：平台都归好组之后
        // 这里不该常驻一句「暂无平台」，那时列表只是空着。
        else if (_c.platforms.isEmpty)
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
            // 拖拽上下文（`PlatformListView.tsx:159-171,203-221`）：
            // 被拖的那张在原位换成虚线 ghost（协议色圆点 + 名称 + 协议徽标），
            // 其余卡压到 0.4。没有这层反馈，拖起来看不出会落在哪。
            onReorderStart: (i) => setState(() => _dragIdx = i),
            onReorderEnd: (_) => setState(() => _dragIdx = null),
            // 跟着指针飞的那张仍然是整张卡，只是抬起来。
            proxyDecorator: (child, i, anim) =>
                Material(color: Colors.transparent, child: child),
            itemBuilder: (context, i) {
              final p = _c.standalonePlatforms[i];
              final dragging = _dragIdx != null;
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
                  child: dragging && _dragIdx == i
                      ? _GhostCard(
                          name: p.name,
                          label: _c.protocolMeta.label(p.platformType),
                          color:
                              _c.protocolMeta.colors[p.platformType] ??
                              AidogTheme.of(context).c.accent,
                        )
                      : Opacity(
                          opacity: dragging ? 0.4 : 1,
                          child: platformCard(p, i),
                        ),
                ),
              );
            },
          ),
        if (_c.purgeCandidates != null || _c.purgePreviewLoading)
          ConfirmCard(
            title: t.t('platform.purgeDisabled'),
            body: _c.purgePreviewLoading
                ? t.t('status.loading')
                : _c.purgeCandidates!.isEmpty
                ? t.t('platform.purgeDisabledNone')
                : t.t('platform.purgeDisabledConfirm'),
            // 执行中确认按钮换文案（`PlatformListView.tsx:305-307`）。
            confirmLabel: _c.purging
                ? t.t('status.loading')
                : t.t('action.confirm'),
            busy: _c.purging,
            onCancel: _c.cancelPurgeDisabled,
            // 候选为空、或还在拉取，确认按钮都不可点
            //（React `PlatformListView.tsx:306` 的 disabled 同判据）。
            onConfirm:
                _c.purgePreviewLoading || _c.purgeCandidates!.isEmpty
                ? null
                : () => _c.confirmPurgeDisabled(
                    noneText: t.t('platform.purgeDisabledNone'),
                  ),
            // 清单 + 失效原因 badge（`PlatformListView.tsx:270-288`）。
            // 候选多时清单自己滚，不把弹窗撑长（React 同处 `maxHeight: 240`）。
            extra: _c.purgePreviewLoading || _c.purgeCandidates!.isEmpty
                ? null
                : ConstrainedBox(
                    constraints: const BoxConstraints(maxHeight: 240),
                    child: SingleChildScrollView(
                      child: Column(
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
                                        : t.t(
                                            'platform.purgeDisabledReasonExpired',
                                          ),
                                    color: AidogTheme.of(context).c.fg3,
                                  ),
                                ],
                              ),
                            ),
                        ],
                      ),
                    ),
                  ),
          ),
        if (_c.deleteTarget != null)
          // 原先借用的是分组场景那对 key，文案里写着「仅属此分组」——在平台列表里
          // 不成立。换成平台自己的一对，React 同用（`PlatformListView.tsx` 的
          // AlertDialog），两侧逐字一致。
          ConfirmCard(
            title: t.t('platform.deleteTitle'),
            body: t
                .t('platform.deleteConfirm')
                .replaceAll(
                  '{{name}}',
                  _c.platforms
                      .where((p) => p.id == _c.deleteTarget)
                      .map((p) => p.name)
                      .firstOrNull ??
                      '',
                ),
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

/// 拖拽时留在原位的虚线预览卡：协议色圆点 + 平台名 + 协议徽标。
/// 对齐 `PlatformListView.tsx:159-171`，让人看得出「松手会落在这儿」。
class _GhostCard extends StatelessWidget {
  const _GhostCard({
    required this.name,
    required this.label,
    required this.color,
  });

  final String name;
  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Opacity(
      opacity: 0.5,
      child: CustomPaint(
        painter: DashedBorder(
          color: theme.c.accent,
          radius: AidogRadius.md,
        ),
        child: Container(
          padding: const EdgeInsets.symmetric(
            horizontal: AidogSpace.smd,
            vertical: AidogSpace.ssm,
          ),
          child: Row(
            children: [
              Container(
                width: 10,
                height: 10,
                decoration: BoxDecoration(color: color, shape: BoxShape.circle),
              ),
              const SizedBox(width: AidogSpace.smd),
              Flexible(
                child: Text(
                  name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AidogType.label.copyWith(
                    color: theme.c.fg,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              const SizedBox(width: AidogSpace.ssm),
              MiniBadge(text: label, color: theme.c.fg3),
            ],
          ),
        ),
      ),
    );
  }
}
