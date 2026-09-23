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
import 'package:flutter_svg/flutter_svg.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../utils/color_level.dart';
import '../../utils/formatters.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'groups_logic.dart';
import 'invoke.dart';
import 'models.dart';
import 'platform_card_bits.dart' show BalanceBar, MiniBadge, StatChip;
import 'platform_logo.dart';
import 'platform_defaults.dart' show kModelSlots;
import 'settings/bits.dart' show PlainTextField;
import 'ui_bits.dart';

/// 分组区。内嵌在平台页里（与 React 的 `GroupsEmbedded` 同位置），
/// 也可以单独渲染（widget 测试就是这么用的）。
/// 组内渲染一张平台卡。由宿主页（`platforms.dart::platformCard`）注入 ——
/// 理由见 [GroupsSection.buildPlatformCard]。
typedef PlatformCardBuilder = Widget Function(PlatformRow platform, int index);

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
    this.searchQuery = '',
    required this.buildPlatformCard,
  });

  /// 宿主页顶部那个搜索框的当前内容。
  ///
  /// 原先这里没有这个入参，源码自述「搜索态在本页尚未接入」—— 而搜索框就在同一屏
  /// 顶部。后果是**已归组的平台一个都搜不到**，用户看到的却是一个能正常打字的框。
  /// 语义照 `Groups.tsx:688-711`：命中组名整组展开、只命中组内几个平台就只渲染
  /// 命中的那几张并强制展开、整组零命中就整组不渲染。
  final String searchQuery;

  /// 组内那张平台卡怎么画（票 24）。
  ///
  /// **为什么是闭包，不是一个 `PlatformsController`**：卡片要的余额 / 用量 / logo /
  /// 最近测试结果全挂在平台页那一份控制器上，那份已经取过数了。分组区自己再起一份
  /// 就要多发一轮 `platform_list` + 统计 + 配额，还会出现两份缓存各自过期、同一个平台
  /// 在页面上下两处显示不同余额。传闭包 = 复用那一份实时数据，零额外取数、单一缓存，
  /// 而且分组区不必认识 `PlatformsController` 这个类型。
  final PlatformCardBuilder buildPlatformCard;

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
  final void Function(String pageId, {String? groupKey})? onNavigate;

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

  /// 触底自动加载（`GroupListView.tsx:277-285` 的 `IntersectionObserver`）。
  ///
  /// 监听的是**骨架那个 `CustomScrollView` 的 position**，不是列表里的哨兵 widget：
  /// position 的通知发生在滚动活动里，不在布局 / 语义那一趟，所以往列表里追加
  /// 下一页不会踩 `!childSemantics.renderObject._needsLayout`。
  /// 同一条路子见 `settings/schema_config_page.dart` 的 `_onScroll`。
  ///
  /// **只在真的滚动时触发**：首帧不主动查。内容没占满一屏时根本没得滚，
  /// 那种情况交给下面保留的「加载更多」按钮 —— 键盘用户和关了动效的用户也靠它。
  ScrollPosition? _scrollPos;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final pos = Scrollable.maybeOf(context)?.position;
    if (identical(pos, _scrollPos)) return;
    _scrollPos?.removeListener(_onScroll);
    _scrollPos = pos;
    _scrollPos?.addListener(_onScroll);
  }

  @override
  void dispose() {
    _scrollPos?.removeListener(_onScroll);
    super.dispose();
  }

  void _onScroll() {
    final pos = _scrollPos;
    if (pos == null || !pos.hasContentDimensions) return;
    if (!_c.hasMore || _c.loadingMore || _c.loading) return;
    // 提前 300 逻辑像素就开始拉，滚到底时下一页已经在路上。
    if (pos.extentAfter > 300) return;
    unawaited(_c.loadMore());
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
    // 搜索串每帧推给控制器 —— 它是纯派生的来源，不存第二份。
    _c.setSearchQuery(widget.searchQuery);
    return _GroupListView(
      controller: _c,
      searchQuery: widget.searchQuery,
      onPlatformsDeleted: widget.onPlatformsDeleted,
      onPlatformDropped: widget.onPlatformDropped == null ? null : _acceptDrop,
      onCreatePlatform: widget.onCreatePlatform,
      onNavigate: widget.onNavigate,
      copyText: widget.copyText,
      buildPlatformCard: widget.buildPlatformCard,
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
    this.searchQuery = '',
    required this.buildPlatformCard,
  });

  /// 见 [GroupsSection.searchQuery]。
  final String searchQuery;

  final PlatformCardBuilder buildPlatformCard;
  final GroupsController controller;
  final void Function(List<int> ids)? onPlatformsDeleted;
  final Future<void> Function(int platformId, int groupId)? onPlatformDropped;
  final void Function({List<int>? presetGroupIds, int? lockGid})?
  onCreatePlatform;
  final void Function(String pageId, {String? groupKey})? onNavigate;
  final Future<void> Function(String text) copyText;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final c = controller;
    final searching = searchQuery.trim().isNotEmpty;
    final gs = c.groupSearch;
    final rows = c.visibleDetails;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            // 这里**没有**「分组」标题，故意的：下面那行计数本身就带「分组」这个词
            //（`GroupListView.tsx:163-167` 的 `{details.length} {t("nav.groups")}`），
            // 再加一个标题读出来是「分组　3 分组」，同一个词一行印两遍。
            // 分组计数：有组才显，没有组时那行留白。
            if (c.details.isNotEmpty) ...[
              Text(
                '${c.details.length} ${t.t('nav.groups')}',
                style: AidogType.micro.copyWith(
                  color: AidogTheme.of(context).c.fg3,
                ),
              ),
            ],
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
            // 🔴 这里**没有**「添加分组」按钮，故意的。
            // `GroupsSection` 只有一个挂载点（`platforms.dart` 的平台页内嵌，
            // 与 React 的 `GroupsEmbedded` 同构），而平台页页头已经有一颗
            // 「+ 添加分组」（`PlatformListView.tsx:118`）。这里再放一颗就是
            // 同一页里的第二颗同功能按钮，React 没有。
            // 建组入口由 `onCreateGroupReady` 把 `c.openCreate` 交给页头那颗。
          ],
        ),
        const SizedBox(height: AidogSpace.ssm),
        if (c.loading)
          CenteredNote(text: t.t('status.loading'))
        else if (c.details.isEmpty)
          CenteredNote(text: t.t('group.empty'))
        // 搜了但一个组都没命中：不能显「还没有分组」，那会让人以为分组全没了。
        else if (rows.isEmpty)
          CenteredNote(text: t.t('logs.empty'))
        else
          // 分组列表拖拽排序（`Groups.tsx:517-524` 的 SortableList）：
          // 搜索态在本页尚未接入（无搜索入口），故不设 no-op 分支。
          ReorderableListView.builder(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            buildDefaultDragHandles: false,
            // 拖起来的那一份画在 Overlay 里，够不着 AidogI18n / 主题的 InheritedWidget
            // （整张卡在那儿重建会直接断言失败），所以只画一个名字标签。
            // 拖起整张分组卡（`GroupListView.tsx:203-205` 的 SortableList）。
            // 老注释说 Overlay 里够不着 `AidogI18n` / 主题 —— 那是测试骨架把
            // `AidogI18n` 套在 `MaterialApp.home` 里造成的假象；真机是
            // `runApp(AidogI18n(child: AidogApp()))`（`main.dart:32`），
            // 祖先在 Navigator 之上，Overlay 够得着。骨架已经改成同一层级。
            proxyDecorator: (child, i, animation) =>
                Material(color: Colors.transparent, child: child),
            itemCount: rows.length,
            onReorderItem: (o, n) {
              // 搜索态下顺序是过滤后的子集，拖了会把没显示的组一起重排 —— 不接受。
              if (searching) return;
              final next = [...c.details];
              next.insert(n, next.removeAt(o));
              unawaited(c.reorderGroups(next));
            },
            itemBuilder: (context, i) {
              final d = rows[i];
              return Padding(
                key: ValueKey(d.group.id),
                padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                // 逐卡错峰淡入 + 悬停抬升（React 列表行的既定约定：`index * 60`）。
                child: Reveal(
                  delayMs: i * 60,
                  child: HoverLift(
                child: _GroupCard(
                  controller: c,
                  detail: d,
                  index: i,
                  // 搜索命中的组强制展开（`GroupListView.tsx:245` 的 forceExpanded），
                  // 否则搜到了还得再点一下才看得见。
                  collapsed: searching
                      ? false
                      : c.collapsedGroups.contains(d.group.id),
                  // null = 不过滤；非 null = 只渲染这几张卡。
                  visiblePlatformIds: gs?[d.group.id],
                  onPlatformDropped: onPlatformDropped,
                  onCreatePlatform: onCreatePlatform,
                  onNavigate: onNavigate,
                  copyText: copyText,
                  buildPlatformCard: buildPlatformCard,
                ),
                  ),
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
          // 滚到底会自动拉下一页（见 `_GroupsSectionState._onScroll`）。
          // 这颗按钮**是有意保留的兜底**，React 也留着一颗：内容没占满一屏时
          // 没得滚，键盘操作与关了动效的用户也需要一个显式入口。
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.ssm),
            child: Align(
              child: SmallButton(label: t.t('logs.hasMore'), onTap: c.loadMore),
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
            onConfirm: () =>
                c.confirmDeleteGroup(failText: t.t('group.deleteFailed')),
          ),
        if (c.removeTarget != null)
          _RemovePlatformConfirm(
            controller: c,
            onDeleted: (id) => onPlatformsDeleted?.call([id]),
          ),
        if (c.batchDeleteTarget != null)
          ConfirmCard(
            title: t.t('group.batchDeleteTitle'),
            body: t.t('group.batchDeleteDesc', {
              'count': '${c.batchDeleteTarget!.platforms.length}',
            }),
            confirmLabel: c.batchDeleteBusy
                ? t.t('group.batchDeleting')
                : t.t('group.batchDeleteConfirm', {
                    'count': '${c.batchDeleteTarget!.platforms.length}',
                  }),
            busy: c.batchDeleteBusy,
            // 唯一一处 `variant="destructive"`（`BatchDeleteModal.tsx:132`）。
            dangerConfirm: true,
            // React 先列**全部**待删平台，再单独警告其中跨组的那几个
            // （`BatchDeleteModal.tsx:90-128`）。原先这里只列跨组的，
            // 于是「一共要删几个、删的是哪几个」在确认前看不到。
            extra: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: [
                if (c.batchDeleteTarget!.hasCrossGroup)
                  // 跨组警告：删掉就是从所有组里消失，不只是本组。
                  _CrossGroupWarning(target: c.batchDeleteTarget!),
                BatchAffectedList(platforms: c.batchDeleteTarget!.platforms),
              ],
            ),
            onCancel: c.cancelBatchDelete,
            onConfirm: () {
              final ids = [
                for (final p in c.batchDeleteTarget!.platforms) p.id,
              ];
              c
                  .confirmBatchDelete(
                    doneText: (n) =>
                        t.t('group.batchDeleteDone', {'count': '$n'}),
                    failText: t.t('group.batchDeleteFailed'),
                  )
                  .then((_) => onPlatformsDeleted?.call(ids));
            },
          ),
        if (c.batchOverrideTarget != null)
          _BatchOverrideModelsCard(controller: c),
        if (c.batchSetStatusTarget != null) _BatchSetStatusCard(controller: c),
        if (c.batchMoveGroupTarget != null) _BatchMoveGroupCard(controller: c),
        if (c.purgeTarget != null || c.purgePreviewLoading)
          ConfirmCard(
            title: t.t('group.purgeDisabled'),
            body: c.purgePreviewLoading
                ? t.t('status.loading')
                : c.purgeTarget!.candidates.isEmpty
                ? t.t('platform.purgeDisabledNone')
                : t.t('group.purgeDisabledConfirm', {
                    'count': '${c.purgeTarget!.candidates.length}',
                  }),
            // 执行中：按钮文案换「处理中…」，两颗按钮都禁掉，弹窗留在原地
            //（`GroupListItem.tsx:596-604`）。
            confirmLabel: c.purging
                ? t.t('status.loading')
                : t.t('action.confirm'),
            busy: c.purging,
            // 不可逆删除之前要看得见删的是谁（`GroupListItem.tsx:566-598`）：
            // 按 action 分「将永久删除」/「将移出本分组」两段，每行平台名 + 失效原因。
            // 名字和 action 本来就在 `purgeTarget.candidates` 里，之前只渲染了个数量。
            extra: c.purgePreviewLoading || c.purgeTarget!.candidates.isEmpty
                ? null
                : _PurgeCandidateList(candidates: c.purgeTarget!.candidates),
            onConfirm:
                c.purgePreviewLoading || c.purgeTarget!.candidates.isEmpty
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

/// 拖动分组时跟着指针走的那张小标签（对齐 `platforms.dart::_DragLabel` 的形状）。
/// 只用字面量色值以外的 token，不碰 i18n —— 它活在 Overlay 里，取不到页面的
/// InheritedWidget。
class _GroupDragLabel extends StatelessWidget {
  const _GroupDragLabel({required this.name});

  final String name;

  @override
  Widget build(BuildContext context) => Material(
    color: Colors.transparent,
    child: Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AidogSpace.smd,
        vertical: AidogSpace.sxs,
      ),
      decoration: BoxDecoration(
        color: AidogTheme.of(context).c.accentWash,
        border: Border.all(color: AidogTheme.of(context).c.accentEdge),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Text(
        name,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: AidogType.micro.copyWith(
          color: AidogTheme.of(context).c.accentText,
        ),
      ),
    ),
  );
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
    // 虚线边框 + 0.85 透明度（`GroupListView.tsx:446-449`）：
    // 它不是真分组（MITM fallback 直通的统计桶），长得和真分组卡一样会让人去点编辑。
    return Opacity(
      opacity: 0.85,
      child: CustomPaint(
        painter: DashedBorder(
          color: AidogTheme.of(context).c.line,
          radius: AidogRadius.md,
        ),
        child: Tile(
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
        ),
      ),
    );
  }
}

class _GroupCard extends StatelessWidget {
  const _GroupCard({
    required this.controller,
    required this.detail,
    required this.index,
    required this.collapsed,
    this.onPlatformDropped,
    this.onCreatePlatform,
    this.onNavigate,
    this.copyText = native.writeText,
    this.visiblePlatformIds,
    required this.buildPlatformCard,
  });

  /// 搜索命中的平台 id 集合。null = 不过滤（无搜索，或整组命中组名）。
  /// 对应 `GroupListItem.tsx:344-346` 的 `visiblePlatformIds`。
  final Set<int>? visiblePlatformIds;

  final PlatformCardBuilder buildPlatformCard;
  final GroupsController controller;
  final GroupDetail detail;

  /// 在分组列表里的下标 —— 拖拽把手要靠它告诉 `ReorderableListView` 拖的是哪一项。
  final int index;
  final bool collapsed;
  final Future<void> Function(int platformId, int groupId)? onPlatformDropped;
  final void Function({List<int>? presetGroupIds, int? lockGid})?
  onCreatePlatform;
  final void Function(String pageId, {String? groupKey})? onNavigate;
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
                  color: AidogTheme.of(context).c.accentEdge,
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
    // 多选态强制展开（`GroupListItem.tsx:357-359` 的
    // `expanded = forceExpanded || isExpanded || mode === "select"`）：
    // 折叠着进多选，要选的平台一个都看不到。
    final folded = collapsed && !selecting;
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
                icon: Icon(folded ? Icons.chevron_right : Icons.expand_more),
                // 多选时不许折叠（React 同处：`mode === "select"` 压过展开态）。
                onPressed: selecting
                    ? null
                    : () {
                        final next = c.toggleGroupCollapsed(g.id);
                        c.persistGroupCollapsed(g.id, next);
                      },
              ),
              const SizedBox(width: AidogSpace.sxs),
              // 分组排序拖拽把手（`Groups.tsx:186-195` 的 drag-handle）。
              // 卡片整体不可拖（`buildDefaultDragHandles: false`），只有这个把手能起拖，
              // 否则卡内的按钮会被拖拽手势吃掉。
              ReorderableDragStartListener(
                index: index,
                child: Tooltip(
                  message: t.t('group.dragToReorder'),
                  child: const Icon(Icons.drag_handle, size: 16),
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              // 组图标（`GroupListItem.tsx:197`）：单平台组跟随该平台 logo。
              GroupIcon(detail: detail),
              const SizedBox(width: AidogSpace.ssm),
              // 点组名区域整块切换展开（`GroupListItem.tsx:199-201`）：
              // 原先只有最左那颗 chevron 能切，点名字没反应。
              Expanded(
                child: GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onTap: selecting
                      ? null
                      : () {
                          final next = c.toggleGroupCollapsed(g.id);
                          c.persistGroupCollapsed(g.id, next);
                        },
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
                              padding: const EdgeInsets.only(
                                left: AidogSpace.sxs,
                              ),
                              // 实心徽标（`GroupListItem.tsx:207-209`）：
                              // 「哪个是默认组」要一眼认出来，裸字混在组名旁边看不见。
                              child: Tooltip(
                                message: t.t('group.isDefaultTitle'),
                                child: MiniBadge(
                                  text: t.t('group.isDefault'),
                                  color: theme.c.accentText,
                                  solid: true,
                                ),
                              ),
                            ),
                          // 自动建组的 `auto` 徽标（`GroupListItem.tsx:210-212`）。
                          // 没有它就看不出这个组是跟着某个平台自动生成的，
                          // 而下面的删除按钮守卫正是按这个条件走。
                          if (g.autoFromPlatform.isNotEmpty)
                            Padding(
                              padding: const EdgeInsets.only(
                                left: AidogSpace.sxs,
                              ),
                              child: MiniBadge(
                                text: 'auto',
                                color: theme.c.fg3,
                              ),
                            ),
                        ],
                      ),
                      // 🔴 副标题里**不印 group_key**：它就是这个分组的 API Key，
                      // 印在列表上意味着截图 / 录屏 / 投屏都会连 key 一起泄出去。
                      // React 只把它放在复制按钮和编辑页里（`GroupListItem.tsx:224-235`）。
                      // 这里照 React 的副标题来：路由模式 badge + 「N 平台」。
                      Row(
                        children: [
                          MiniBadge(
                            text: routingLabel(t, g.routingMode),
                            color: theme.c.fg3,
                          ),
                          if (detail.platforms.isNotEmpty) ...[
                            const SizedBox(width: AidogSpace.sxs),
                            Text(
                              '${detail.platforms.length} ${t.t('group.platforms')}',
                              style: AidogType.micro.copyWith(
                                color: theme.c.fg3,
                              ),
                            ),
                          ],
                        ],
                      ),
                    ],
                  ),
                ),
              ),
              // 快捷操作全在标题行右侧一排（`GroupListItem.tsx:236-315`）：
              // 哪几颗是图标、哪几颗留文字，逐颗对齐 React —— 原先拆成上下两行，
              // 一张卡的头部占两倍高。文案 key 全部进了 tooltip，一个都没删。
              Wrap(
                spacing: AidogSpace.sxs,
                runSpacing: AidogSpace.sxs,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  _CopyCommandMenu(
                    group: g,
                    proxyEnvVars: c.proxyEnvVars,
                    copyText: copyText,
                  ),
                  if (onNavigate != null)
                    _GroupIconAction(
                      icon: Icons.bar_chart,
                      tooltip: t.t('group.viewStats'),
                      // 带上 group_key 预筛统计页（React `GroupListItem.tsx:236`
                      // 传的是 `{ groupId, groupKey }`；统计页只读 groupKey，
                      // 见 `Stats.tsx:255`，所以这里只传它）。
                      onTap: () =>
                          onNavigate!.call('stats', groupKey: g.groupKey),
                    ),
                  // 禁用条件（`GroupListItem.tsx:241`）：组内一个启用平台都没有，
                  // 或者已经有一轮测试在跑。原先空组也能点、还能重复并发触发。
                  _GroupIconAction(
                    icon: Icons.bolt,
                    tooltip: t.t('group.testAll'),
                    onTap:
                        (c.groupTest != null ||
                            !detail.platforms.any(
                              (gp) => gp.platform.status == 'enabled',
                            ))
                        ? null
                        : () => c.testGroup(g, detail.platforms),
                  ),
                  if (onCreatePlatform != null)
                    _GroupIconAction(
                      icon: Icons.add,
                      tooltip: t.t('group.addPlatformToGroup'),
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
                  _GroupIconAction(
                    icon: Icons.edit_outlined,
                    tooltip: t.t('action.edit'),
                    onTap: () => c.openEdit(detail),
                  ),
                  // 自动建组只要还有平台就不给删除按钮
                  //（`GroupListItem.tsx:309` 的 `!auto_from_platform || gps.length === 0`）：
                  // 它是跟着平台自动生成的，删了下次还会再建出来。
                  if (g.autoFromPlatform.isEmpty || detail.platforms.isEmpty)
                    _GroupIconAction(
                      icon: Icons.delete_outline,
                      tooltip: t.t('action.delete'),
                      danger: true,
                      onTap: () => c.askDeleteGroup(g.id),
                    ),
                ],
              ),
            ],
          ),
          // 行 2：聚合统计 + 聚合余额（`GroupListItem.tsx:318-340`）。
          // 原先这里只有一个没标签的请求总数裸数字和一个裸金额 —— tokens / 花费 /
          // 成功率三个 chip 全丢了，余额也没有进度条和分级配色。
          if (stats != null || balance != null) ...[
            const SizedBox(height: AidogSpace.sxs),
            Padding(
              padding: const EdgeInsetsDirectional.only(start: 26),
              child: Wrap(
                spacing: AidogSpace.ssm,
                runSpacing: AidogSpace.sxs,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  if (stats != null) ...[
                    StatChip(
                      icon: Icons.bolt,
                      value: formatNumber(
                        stats.totalInputTokens + stats.totalOutputTokens,
                      ),
                      label: 'tokens',
                    ),
                    StatChip(
                      icon: Icons.attach_money,
                      value: formatCostUsd(stats.totalCost),
                      label: 'cost',
                      level: costLevel(stats.totalCost),
                    ),
                    // 一次请求都没有时不画成功率（0% 会被读成「全失败」）。
                    if (stats.totalRequests > 0)
                      StatChip(
                        icon: Icons.check_circle_outline,
                        value: formatPercent(
                          successRate(stats.successCount, stats.totalRequests),
                          0,
                        ),
                        label: 'ok',
                        level: successRateLevel(
                          successRate(stats.successCount, stats.totalRequests),
                          stats.totalRequests,
                        ),
                      ),
                  ],
                  if (balance != null)
                    SizedBox(
                      width: 90,
                      child: BalanceBar(remaining: balance, showTotal: false),
                    ),
                ],
              ),
            ),
          ],
          if (!folded) ...[
            const SizedBox(height: AidogSpace.ssm),
            if (selecting)
              _BatchToolbar(
                controller: c,
                gid: g.id,
                platforms: detail.platforms,
              ),
            if (detail.platforms.isEmpty)
              Text(
                t.t('group.noPlatforms'),
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              )
            else
              // 搜索命中过滤：只渲染命中的那几张（`GroupListItem.tsx:344-346`）。
              // **index / total 仍按全量算** —— 它们决定上下移按钮的可用性，
              // 按过滤后的算会让「上移」把平台挪到错的位置上。
              for (var i = 0; i < detail.platforms.length; i++)
                if (visiblePlatformIds == null ||
                    visiblePlatformIds!.contains(
                      detail.platforms[i].platform.id,
                    ))
                  _PlatformRow(
                    controller: c,
                    group: g,
                    gp: detail.platforms[i],
                    index: i,
                    total: detail.platforms.length,
                    allGroups: c.allGroups,
                    selecting: selecting,
                    buildPlatformCard: buildPlatformCard,
                  ),
              // 末位插入线（`GroupListItem.tsx:470-472`）：拖到最后一张下面时画在这儿。
              if (c.platDropIndicator?.gid == g.id &&
                  c.platDropIndicator?.idx == detail.platforms.length)
                const _DropLine(),
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
      // 四项各带图标（`GroupListItem.tsx:230-233`）：三颗用的就是 React 那三份
      // 同名 svg（`assets/platforms/` 是两侧共用的真值源），密钥那项用钥匙图标。
      itemBuilder: (context) => [
        PopupMenuItem(
          value: 'key',
          child: _CopyMenuRow(
            icon: const Icon(Icons.key, size: 14),
            label: t.t('group.menuCopyKey'),
          ),
        ),
        PopupMenuItem(
          value: 'claude',
          child: _CopyMenuRow(
            icon: _logoIcon('claude_code'),
            label: t.t('group.menuCopyClaude'),
          ),
        ),
        PopupMenuItem(
          value: 'codex',
          child: _CopyMenuRow(
            icon: _logoIcon('openai'),
            label: t.t('group.menuCopyCodex'),
          ),
        ),
        PopupMenuItem(
          value: 'pi',
          child: _CopyMenuRow(
            icon: _logoIcon('pi'),
            label: t.t('group.menuCopyPi'),
          ),
        ),
      ],
      child: IgnorePointer(child: SmallButton(label: t.t('group.copyCommand'))),
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
          SmallButton(
            label: t.t('action.cancel'),
            onTap: () => c.exitBatchSelect(gid),
          ),
          SmallButton(
            label: t.t('group.selectAll'),
            onTap: () =>
                c.selectAll(gid, [for (final gp in platforms) gp.platform.id]),
          ),
          Text(
            t.t('group.selectedCount', {'count': '${selected.length}'}),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          SmallButton(
            label: t.t('group.batchDelete'),
            danger: true,
            onTap: hasSelection
                ? () => c.askBatchDelete(selected.toList())
                : null,
          ),
          SmallButton(
            label: t.t('group.batchOverrideModels'),
            onTap: hasSelection
                ? () => c.askBatchOverrideModels(selected.toList())
                : null,
          ),
          SmallButton(
            label: t.t('group.batchSetStatus'),
            onTap: hasSelection
                ? () => c.askBatchSetStatus(selected.toList(), gid)
                : null,
          ),
          SmallButton(
            label: t.t('group.batchMoveGroup'),
            onTap: hasSelection
                ? () => c.askBatchMoveGroup(selected.toList(), gid)
                : null,
          ),
        ],
      ),
    );
  }
}

/// 单个分组内平台行（票 24）：左侧多选勾选框 + 中间**完整平台卡** +
/// 右侧组内控件（上下移 / 优先级 / 移组 / 移除）。
///
/// 中间那张卡与平台页用的是同一张（`platform_card_view.dart::PlatformCard`），
/// 经 [buildPlatformCard] 注入，只是不给拖拽手柄。组内控件留在卡**外面**：
/// 它们是「这个平台在这个分组里」的属性，不属于平台本身，卡片也不认识分组。
class _PlatformRow extends StatelessWidget {
  const _PlatformRow({
    required this.controller,
    required this.group,
    required this.gp,
    required this.index,
    required this.total,
    required this.allGroups,
    required this.selecting,
    required this.buildPlatformCard,
  });

  final PlatformCardBuilder buildPlatformCard;

  final GroupsController controller;
  final GroupRow group;
  final GroupPlatform gp;

  /// 组内位次（0 起）与本组平台总数：决定上下移按钮的可用性。
  final int index;
  final int total;
  final List<({int id, String name})> allGroups;
  final bool selecting;

  @override
  Widget build(BuildContext context) {
    final c = controller;
    final pid = gp.platform.id;
    final t = AidogI18n.of(context);
    final row = Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // 多选态的勾选框（`GroupListItem.tsx:425-434`）。非多选态不再画状态小圆点 ——
          // 卡片 logo 右上角那颗健康点信息更全，画两颗只会互相打架。
          if (selecting)
            Padding(
              padding: const EdgeInsets.only(right: AidogSpace.sxs),
              child: Checkbox(
                value: c.selectedIdsOf(group.id).contains(pid),
                onChanged: (_) => c.toggleSelected(group.id, pid),
              ),
            )
          else
            // 组内拖拽把手（`GroupListItem.tsx:437-444`）。
            // 与分组卡自己的排序把手是**两个不同的节点**，所以两套拖拽不打架 ——
            // React 的 `usePlatformDrag.ts` 抬头写的就是这件事。
            Padding(
              padding: const EdgeInsets.only(right: AidogSpace.sxs, top: 6),
              child: Draggable<int>(
                data: pid,
                dragAnchorStrategy: pointerDragAnchorStrategy,
                feedback: _GroupDragLabel(name: gp.platform.name),
                onDragEnd: (_) => c.setPlatDropIndicator(null),
                child: Tooltip(
                  message: t.t('group.dragPlatform'),
                  child: MouseRegion(
                    cursor: SystemMouseCursors.grab,
                    child: Icon(
                      Icons.drag_indicator,
                      size: 14,
                      color: AidogTheme.of(context).c.fg3,
                    ),
                  ),
                ),
              ),
            ),
          Expanded(child: buildPlatformCard(gp.platform, index)),
          const SizedBox(width: AidogSpace.sxs),
          _GroupPlatformControls(
            controller: c,
            group: group,
            gp: gp,
            index: index,
            total: total,
            allGroups: allGroups,
            selecting: selecting,
          ),
        ],
      ),
    );
    // 每一行自己就是一个落点：指针落在上半 → 插在我前面，下半 → 插在我后面
    //（`usePlatformDrag.ts:43-50::computeDropIdx` 的 `clientY < top + height/2`）。
    return DragTarget<int>(
      // 拖自己也接：落在别的位次上就是组内重排。
      onWillAcceptWithDetails: (_) => true,
      onMove: (d) {
        final box = context.findRenderObject() as RenderBox?;
        if (box == null || !box.hasSize) return;
        final localY = box.globalToLocal(d.offset).dy;
        c.setPlatDropIndicator((
          gid: group.id,
          idx: localY < box.size.height / 2 ? index : index + 1,
        ));
      },
      onLeave: (_) {
        final ind = c.platDropIndicator;
        if (ind != null &&
            ind.gid == group.id &&
            (ind.idx == index || ind.idx == index + 1)) {
          c.setPlatDropIndicator(null);
        }
      },
      onAcceptWithDetails: (d) {
        final ind = c.platDropIndicator;
        final idx = (ind != null && ind.gid == group.id) ? ind.idx : index;
        unawaited(c.dropPlatformAt(d.data, group.id, idx));
      },
      builder: (context, candidate, _) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 插入线画在被指向的那一行上方（`GroupListItem.tsx:419-421`）。
          if (c.platDropIndicator?.gid == group.id &&
              c.platDropIndicator?.idx == index)
            const _DropLine(),
          row,
        ],
      ),
    );
  }
}

/// 拖放插入位的 2px accent 线（`GroupListItem.tsx:419-421`）。
class _DropLine extends StatelessWidget {
  const _DropLine();

  @override
  Widget build(BuildContext context) => Opacity(
    opacity: 0.7,
    child: Container(
      height: 2,
      margin: const EdgeInsets.symmetric(vertical: 1),
      decoration: BoxDecoration(
        color: AidogTheme.of(context).c.accentEdge,
        borderRadius: BorderRadius.circular(1),
      ),
    ),
  );
}

/// 「这个平台在这个分组里」的那几个控件：上下移 / 优先级 / 移组 / 移除。
///
/// 从 [_PlatformRow] 拆出来，是因为它现在要和一张整卡并排 —— 横向塞不下一长条，
/// 用 [Wrap] 限宽换行。
class _GroupPlatformControls extends StatelessWidget {
  const _GroupPlatformControls({
    required this.controller,
    required this.group,
    required this.gp,
    required this.index,
    required this.total,
    required this.allGroups,
    required this.selecting,
  });

  final GroupsController controller;
  final GroupRow group;
  final GroupPlatform gp;
  final int index;
  final int total;
  final List<({int id, String name})> allGroups;
  final bool selecting;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final c = controller;
    final pid = gp.platform.id;
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 190),
      child: Wrap(
        alignment: WrapAlignment.end,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          if (!selecting) ...[
            // 组内位次（= 路由优先级顺序）。拖拽把手在行首（见 `_PlatformRow`），
            // 这两颗**是键盘可达的那条路**，不因为有了拖拽就删。
            IconButton(
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
              iconSize: 14,
              tooltip: t.t('group.dragToReorder'),
              onPressed: index == 0
                  ? null
                  : () => c.movePlatformWithinGroup(group.id, index, index - 1),
              icon: const Icon(Icons.arrow_upward),
            ),
            IconButton(
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
              iconSize: 14,
              tooltip: t.t('group.dragToReorder'),
              onPressed: index >= total - 1
                  ? null
                  : () => c.movePlatformWithinGroup(group.id, index, index + 1),
              icon: const Icon(Icons.arrow_downward),
            ),
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
                  : () => c.setLevelPriority(
                      group.id,
                      pid,
                      gp.levelPriority - 1,
                      failText: t.t('group.levelPriorityFailed'),
                    ),
              icon: const Icon(Icons.remove),
            ),
            // 可以直接敲数字，不是只读（React 这里是 `<Input type="number">`，
            // `PlatformCard.tsx:944-961`）。只有加减按钮的话，1 调到 10 要点九下。
            // 失焦 / 回车提交，越界夹到 1~10，敲成非数字就还原当前值。
            SizedBox(
              width: 38,
              child: PlainTextField(
                key: ValueKey('level-priority-$pid'),
                value: '${gp.levelPriority}',
                onSubmitted: (raw) {
                  final v = int.tryParse(raw.trim());
                  if (v == null || v == gp.levelPriority) return;
                  c.setLevelPriority(
                    group.id,
                    pid,
                    v.clamp(1, 10),
                    failText: t.t('group.levelPriorityFailed'),
                  );
                },
              ),
            ),
            IconButton(
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
              iconSize: 14,
              tooltip: t.t('group.levelPriorityUp'),
              onPressed: gp.levelPriority >= 10
                  ? null
                  : () => c.setLevelPriority(
                      group.id,
                      pid,
                      gp.levelPriority + 1,
                      failText: t.t('group.levelPriorityFailed'),
                    ),
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
                icon: Icon(
                  Icons.drive_file_move_outline,
                  size: 14,
                  color: theme.c.fg3,
                ),
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
            // 每条映射是一张小卡：玻璃底 + 描边，源模型 accent 高亮，中间一个箭头
            //（`GroupListItem.tsx:478-497`）。原先是一行裸字 `源 → 目标`，
            // 连行与行的边界都看不出来。
            Container(
              margin: const EdgeInsets.only(bottom: 3),
              padding: const EdgeInsets.symmetric(
                horizontal: AidogSpace.ssm,
                vertical: 3,
              ),
              decoration: BoxDecoration(
                color: theme.c.surface2,
                border: Border.all(color: theme.c.line),
                borderRadius: BorderRadius.circular(AidogRadius.sm),
              ),
              child: Row(
                children: [
                  Flexible(
                    child: Text(
                      detail.modelMappings[i].sourceModel,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(
                        color: theme.c.accentText,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: AidogSpace.sxs,
                    ),
                    child: Icon(
                      Icons.arrow_forward,
                      size: 12,
                      color: theme.c.fg3,
                    ),
                  ),
                  Expanded(
                    child: Text(
                      detail.modelMappings[i].targetModel,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(color: theme.c.fg2),
                    ),
                  ),
                  IconButton(
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(
                      minWidth: 22,
                      minHeight: 22,
                    ),
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
                    key: const ValueKey('mapping-quick-source'),
                    decoration: InputDecoration(
                      hintText: t.t('mapping.source'),
                    ),
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
                      decoration: InputDecoration(
                        hintText: t.t('mapping.target'),
                      ),
                      style: AidogType.micro.copyWith(color: theme.c.fg),
                      onChanged: c.setMTargetModel,
                    ),
                  ),
                SmallButton(
                  label: t.t('action.create'),
                  // `GroupListItem.tsx:547` 没写 variant = 默认实心。
                  filled: true,
                  onTap:
                      (c.mSource.isEmpty ||
                          c.mTargetPlatform == null ||
                          c.mTargetModel.isEmpty)
                      ? null
                      : () => c.submitAddMapping(
                          failText: t.t('group.addMappingFailed'),
                        ),
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
    // React 是 `AlertDialog`（`GroupListView.tsx:314`，maxWidth 420）：点遮罩不关，
    // 但按 Esc 关（Radix `AlertDialog` 的默认行为）。
    return AidogModal(
      onEscape: controller.cancelRemovePlatform,
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
          t.t('group.batchDeleteCrossGroupWarning', {
            'count': '${cross.length}',
          }),
          style: AidogType.micro.copyWith(color: theme.c.bad),
        ),
        for (final p in cross)
          Text(
            '${p.name} · '
            '${t.t('group.batchDeleteCrossGroupItem', {'count': '${target.groupNamesByPlatform[p.id]!.length}', 'groups': target.groupNamesByPlatform[p.id]!.join('、')})}',
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
  State<_BatchOverrideModelsCard> createState() =>
      _BatchOverrideModelsCardState();
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
    final allEmpty = kModelSlots.every(
      (s) => (_slots[s.key] ?? '').trim().isEmpty,
    );
    return ConfirmCard(
      title: t.t('group.batchOverrideModelsTitle'),
      body: t.t('group.batchOverrideModelsDesc', {'count': '${target.length}'}),
      confirmLabel: c.batchOverrideBusy
          ? t.t('group.batchOverrideApplying')
          : t.t('group.batchOverrideConfirm', {'count': '${target.length}'}),
      busy: c.batchOverrideBusy,
      // React 是普通 `Dialog`（`BatchOverrideModelsModal.tsx:123`），执行中不许关。
      dismissOnBarrier: true,
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
              doneText: (n) =>
                  t.t('group.batchOverrideModelsDone', {'count': '$n'}),
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
                // radio 组（`BatchOverrideModelsModal.tsx`）：三选一是互斥语义，
                // 一排高亮按钮看不出「只能选一个」。
                _RadioChoice(
                  label: switch (s) {
                    'manual' => t.t('group.batchOverrideSourceManual'),
                    'preset' => t.t('group.batchOverrideSourcePreset'),
                    _ => t.t('group.batchOverrideSourceCopy'),
                  },
                  selected: _source == s,
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
                    child: Text(
                      t.t(s.labelKey),
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                  ),
                  Expanded(
                    // 🔴 控制器必须活在 State 里（`KeptTextField`）。
                    // 这里原先是 `TextField(controller: TextEditingController(...))`，
                    // 而本 widget 的 onChanged 自己就 setState —— 每敲一个字重建一个
                    // 新控制器并把光标按到末尾，于是**改不了中间的字**，中文输入法
                    // 的候选串也会被打断。这正是 `KeptTextField` 存在的理由。
                    child: KeptTextField(
                      key: ValueKey('batch-override-${s.key}'),
                      value: _slots[s.key] ?? '',
                      onChanged: (v) =>
                          setState(() => _slots = {..._slots, s.key: v}),
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
                  padding: const EdgeInsets.only(
                    left: AidogSpace.ssm,
                    bottom: 1,
                  ),
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
    final willEmpty =
        _status == 'disabled' &&
        c.batchSetStatusGroupEnabledIds.isNotEmpty &&
        c.batchSetStatusGroupEnabledIds.every(selectedIds.contains);
    return ConfirmCard(
      title: t.t('group.batchSetStatusTitle'),
      body: t.t('group.batchSetStatusDesc', {
        'count': '${target.platforms.length}',
      }),
      confirmLabel: c.batchSetStatusBusy
          ? t.t('group.batchSetStatusApplying')
          : t.t('group.batchSetStatusConfirm', {
              'count': '${target.platforms.length}',
            }),
      busy: c.batchSetStatusBusy,
      // React 是普通 `Dialog`（`BatchSetStatusModal.tsx:68`），执行中不许关。
      dismissOnBarrier: true,
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
                // 同上（`BatchSetStatusModal.tsx`）。
                _RadioChoice(
                  label: s == 'enabled'
                      ? t.t('group.batchSetStatusEnabled')
                      : t.t('group.batchSetStatusDisabled'),
                  selected: _status == s,
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
          // 要改的是哪些平台、它们现在各是什么状态（`BatchSetStatusModal.tsx:110-140`）。
          BatchAffectedList(platforms: target.platforms, statusBadge: true),
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
      body: t.t('group.batchMoveGroupDesc', {
        'count': '${target.platforms.length}',
      }),
      confirmLabel: c.batchMoveGroupBusy
          ? t.t('group.batchMoveGroupApplying')
          : t.t('group.batchMoveGroupConfirm', {
              'count': '${target.platforms.length}',
              'mode': _mode == 'move'
                  ? t.t('group.batchMoveGroupModeMoveShort')
                  : t.t('group.batchMoveGroupModeAddShort'),
            }),
      busy: c.batchMoveGroupBusy,
      // React 是普通 `Dialog`（`BatchMoveGroupModal.tsx:80`），执行中不许关。
      dismissOnBarrier: true,
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
          Text(
            t.t('group.batchMoveGroupTarget'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
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
          // 要移的是哪些平台（`BatchMoveGroupModal.tsx:138-157`）。
          BatchAffectedList(platforms: target.platforms),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            children: [
              for (final m in const ['move', 'add'])
                // 同上（`BatchMoveGroupModal.tsx`）。
                _RadioChoice(
                  label: m == 'move'
                      ? t.t('group.batchMoveGroupModeMove')
                      : t.t('group.batchMoveGroupModeAdd'),
                  selected: _mode == m,
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
        t.t('group.testAllOk') +
            (r.durationMs == null ? '' : ' ${r.durationMs}ms'),
      _ => t.t('group.testAllFail'),
    };
    // React 是 createPortal 的手写遮罩（`GroupTestPanel.tsx:53`，width 560）：
    // 点遮罩即关（`onClick={onClose}` 挂在遮罩上）。
    return AidogModal(
      maxWidth: 560,
      onBarrierTap: controller.closeGroupTest,
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
          // 下拉，与编辑面板一致 —— React 两处都是 `<Select>`
          //（`GroupCreateModal.tsx:88-95`、`GroupEditPanel.tsx:97-106`）。
          // 原先这里是一排互斥按钮，同一个字段在同一页有两种长相。
          DropdownButtonHideUnderline(
            child: DropdownButton<String>(
              key: const ValueKey('create-routing-mode'),
              isExpanded: true,
              value: c.createMode,
              items: [
                for (final m in kRoutingModes)
                  DropdownMenuItem(value: m, child: Text(routingLabel(t, m))),
              ],
              onChanged: (v) {
                if (v != null) c.setCreateMode(v);
              },
            ),
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
          // 只列 enabled 的平台（与 React 的 `GroupCreateModal.tsx:33` 同口径）。
          _PlatformPicker(
            platformIds: c.createPlatformIds,
            options: c.enabledPlatformOptions,
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
                // `GroupCreateModal.tsx:55` 没写 variant = 默认实心；
                // 旁边的「取消」是 `variant="outline"`，保持描边。
                filled: true,
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

  String _nameOf(int pid) {
    for (final p in options) {
      if (p.id == pid) return p.name;
    }
    return '#$pid';
  }

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
            // 只有把手能起拖（同分组列表），整行可拖会把移除按钮的点击吃掉。
            buildDefaultDragHandles: false,
            proxyDecorator: (child, i, animation) => _GroupDragLabel(
              name: _nameOf(platformIds[i.clamp(0, platformIds.length - 1)]),
            ),
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
                    ReorderableDragStartListener(
                      index: i,
                      child: Tooltip(
                        message: t.t('group.dragToReorder'),
                        child: Icon(
                          Icons.drag_handle,
                          size: 14,
                          color: theme.c.fg3,
                        ),
                      ),
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                    Text(
                      '${i + 1}',
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                    // 协议双字母徽标（`PlatformPicker.tsx:63-71`）：同名不同协议的
                    // 两个平台在这张列表里本来长得一模一样，选错了只能退出去看平台页。
                    if (p != null)
                      Container(
                        width: 26,
                        alignment: Alignment.center,
                        padding: const EdgeInsets.symmetric(vertical: 2),
                        margin: const EdgeInsets.only(right: AidogSpace.sxs),
                        decoration: BoxDecoration(
                          color: theme.c.accentWash,
                          borderRadius: BorderRadius.circular(AidogRadius.sm),
                        ),
                        child: Text(
                          p.platformType.characters.take(2).toString().toUpperCase(),
                          style: AidogType.micro.copyWith(
                            color: theme.c.accentText,
                          ),
                        ),
                      ),
                    Expanded(
                      child: Text(
                        p?.name ?? '#$pid',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.micro.copyWith(color: theme.c.fg2),
                      ),
                    ),
                    // 上下移：与拖拽并存的第二条路（`PlatformPicker.tsx:72-92`）。
                    // 这个顺序就是路由优先级，只有拖拽的话，拖不稳的人和用键盘
                    // 的人根本改不了优先级。
                    _PickerIconButton(
                      key: ValueKey('picker-up-$pid'),
                      icon: Icons.arrow_upward,
                      tooltip: t.t('action.moveUp'),
                      onPressed: i == 0
                          ? null
                          : () => onChange(_swapped(platformIds, i, i - 1)),
                    ),
                    _PickerIconButton(
                      key: ValueKey('picker-down-$pid'),
                      icon: Icons.arrow_downward,
                      tooltip: t.t('action.moveDown'),
                      onPressed: i == platformIds.length - 1
                          ? null
                          : () => onChange(_swapped(platformIds, i, i + 1)),
                    ),
                    _PickerIconButton(
                      key: ValueKey('picker-remove-$pid'),
                      icon: Icons.close,
                      tooltip: t.t('action.delete'),
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
    final hasReserved = e.envVars.any((ev) => kReservedEnvKeys.contains(ev.key));
    return Tile(
      title: t.t('group.edit'),
      // 副标题是 `#<id>`（`GroupEditPanel.tsx:61`）。
      // 🔴 这里**不印 group_key**：它就是这个分组的 API Key，
      // 印在标题栏上意味着截图 / 录屏 / 投屏都会连 key 一起泄出去。
      meta: '#${g.id}',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 头部复制按钮组（`GroupEditPanel.tsx:63-66`）。
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            children: [
              // 三颗命令按钮用的就是 React 那三份同名 svg，且点完给「已复制」反馈
              //（`GroupEditPanel.tsx:63-66` 的 CopyButton）。
              _CopyChip(
                tooltip: t.t('group.copyApiKeyTitle'),
                label: t.t('group.apiKey'),
                copyText: widget.copyText,
                textOf: () => g.groupKey,
              ),
              _CopyChip(
                tooltip: t.t('group.copyCommand'),
                label: 'Claude',
                icon: _logoIcon('claude_code'),
                copyText: widget.copyText,
                textOf: () => buildClaudeCommand(g.groupKey),
              ),
              _CopyChip(
                tooltip: t.t('group.copyCodexCommand'),
                label: 'Codex',
                icon: _logoIcon('openai'),
                copyText: widget.copyText,
                textOf: () => buildCodexCommand(g.groupKey, envVars),
              ),
              _CopyChip(
                tooltip: t.t('group.copyPiCommand'),
                label: 'pi',
                icon: _logoIcon('pi'),
                copyText: widget.copyText,
                textOf: () => buildPiCommand(g.groupKey, envVars),
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
          // 密钥旁边就要有复制（`GroupEditPanel.tsx:85-90`）：
          // 只读一行字没法选，之前得滚回页头那颗 API Key 按钮。
          Row(
            children: [
              Flexible(
                child: Text(
                  g.groupKey,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AidogType.micro.copyWith(color: theme.c.fg2),
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              _CopyChip(
                tooltip: t.t('group.copyApiKeyTitle'),
                label: t.t('action.copy'),
                copyText: widget.copyText,
                textOf: () => g.groupKey,
              ),
            ],
          ),
          Text(
            t.t('group.groupKeyLocked'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('group.routingMode')),
          const SizedBox(height: AidogSpace.sxs),
          // 下拉而不是一排互斥按钮（`GroupEditPanel.tsx:97-106`）：
          // 平铺会把选项全堆在表单里，当前选的是哪个反而要扫一遍才看出来。
          DropdownButtonHideUnderline(
            child: DropdownButton<String>(
              isExpanded: true,
              value: e.mode,
              items: [
                for (final m in kRoutingModes)
                  DropdownMenuItem(value: m, child: Text(routingLabel(t, m))),
              ],
              onChanged: (v) {
                if (v != null) c.patchEdit(e.patch(mode: v));
              },
            ),
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
          // 同上（`GroupEditPanel.tsx:115-122`）。
          DropdownButtonHideUnderline(
            child: DropdownButton<String>(
              isExpanded: true,
              value: _piApi,
              items: [
                for (final api in kPiApis)
                  DropdownMenuItem(
                    value: api,
                    child: Text(piApiLabel(t.t, api)),
                  ),
              ],
              onChanged: (v) {
                if (v == null) return;
                setState(() => _piApi = v);
                unawaited(c.setGroupPiApi(g.id, v));
              },
            ),
          ),
          Text(
            t.t('group.piApiHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.ssm),
          _NumField(
            label: t.t('group.maxRetries'),
            value: e.maxRetries,
            // 上限 10（`GroupEditPanel.tsx:145-148` 的 `max={10}`）：
            // 没有上限时填个 999 会让一次失败重试到天荒地老。
            max: 10,
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
            blankWhenZero: true,
            hint: t.t('group.reqTimeout'),
            onChanged: (v) => c.patchEdit(e.patch(reqTimeout: v)),
          ),
          _NumField(
            label: t.t('group.connTimeout'),
            value: e.connTimeout,
            blankWhenZero: true,
            hint: t.t('group.connTimeout'),
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
              // 说明前挂一枚 auto 徽标（`GroupEditPanel.tsx:154-159`），
              // 与列表卡上的那枚同一个标识。
              child: Row(
                children: [
                  MiniBadge(text: 'auto', color: theme.c.fg3),
                  const SizedBox(width: AidogSpace.sxs),
                  Flexible(
                    child: Text(
                      t.t('group.autoFromPlatform'),
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                  ),
                ],
              ),
            ),
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('group.platforms')),
          Text(
            t.t('group.platformsHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.sxs),
          // 只列 enabled 的（`GroupEditPanel.tsx:34`）——原先是全量 `c.platforms`。
          _PlatformPicker(
            platformIds: e.platformIds,
            options: c.enabledPlatformOptions,
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
            // 一行一张小卡（`GroupEditPanel.tsx:189-193`）：裸 Row 只隔 2px，
            // 三四行挤在一起分不清哪个输入属于哪条映射。
            _RowCard(
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
                      // 0 号占位项 = 清回未选（`GroupEditPanel.tsx:217` 的
                      // `__none__`）。没有它，选错了只能删掉整行重建。
                      items: [
                        DropdownMenuItem(
                          value: 0,
                          child: Text(t.t('mapping.targetPlatform')),
                        ),
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
                  // 目标模型：目标平台配过模型就给下拉，没配才退回文本框
                  // （`GroupEditPanel.tsx:221-239` 的两条分支）。原先恒为文本框，
                  // 模型名打错了当场没有任何提示，要等真发请求才失败。
                  Expanded(
                    child: Builder(
                      builder: (context) {
                        final models = c.modelsOfPlatform(
                          e.mappings[i].targetPlatformId,
                        );
                        void setTarget(String v) {
                          final next = [...e.mappings];
                          next[i] = ModelMapping(
                            sourceModel: next[i].sourceModel,
                            targetPlatformId: next[i].targetPlatformId,
                            targetModel: v,
                            requestTimeoutSecs: next[i].requestTimeoutSecs,
                            connectTimeoutSecs: next[i].connectTimeoutSecs,
                          );
                          c.patchEdit(e.patch(mappings: next));
                        }

                        if (models.isEmpty) {
                          return _Field(
                            hint: t.t('mapping.target'),
                            value: e.mappings[i].targetModel,
                            onChanged: setTarget,
                          );
                        }
                        return DropdownButtonHideUnderline(
                          child: DropdownButton<String>(
                            isExpanded: true,
                            hint: Text(t.t('mapping.target')),
                            value: models.contains(e.mappings[i].targetModel)
                                ? e.mappings[i].targetModel
                                : null,
                            // 空串占位项 = 清回未选（`GroupEditPanel.tsx:233`）。
                            items: [
                              DropdownMenuItem(
                                value: '',
                                child: Text(t.t('mapping.target')),
                              ),
                              for (final m in models)
                                DropdownMenuItem(value: m, child: Text(m)),
                            ],
                            onChanged: (v) => setTarget(v ?? ''),
                          ),
                        );
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
            // 同上 + 保留字那几行整行描边换 warning 色
            //（`GroupEditPanel.tsx:270-276`）：底下那句全局红字说不清是哪一行。
            _RowCard(
              warn: kReservedEnvKeys.contains(e.envVars[i].key),
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
                  envVars: [
                    ...e.envVars,
                    const EnvVar(key: '', value: ''),
                  ],
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
                // `GroupEditPanel.tsx:68` 没写 variant = 默认实心。
                filled: true,
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
/// 交换列表里两项的位置，返回新列表（原列表不动）。
List<int> _swapped(List<int> ids, int a, int b) {
  final next = [...ids];
  final tmp = next[a];
  next[a] = next[b];
  next[b] = tmp;
  return next;
}

/// 关联平台行右侧的小图标按钮。三颗（上移 / 下移 / 移除）尺寸与禁用语义一致，
/// 所以收成一个。
class _PickerIconButton extends StatelessWidget {
  const _PickerIconButton({
    super.key,
    required this.icon,
    required this.tooltip,
    required this.onPressed,
  });

  final IconData icon;
  final String tooltip;

  /// null = 禁用（第一行不能上移、最后一行不能下移）。
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) => IconButton(
    padding: EdgeInsets.zero,
    constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
    iconSize: 14,
    tooltip: tooltip,
    icon: Icon(icon),
    onPressed: onPressed,
  );
}

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

  Widget _input(AidogTheme theme) => TextField(
    controller: _ctrl,
    style: AidogType.micro.copyWith(color: theme.c.fg),
    decoration: InputDecoration(
      isDense: true,
      hintText: widget.hint,
      hintStyle: AidogType.micro.copyWith(color: theme.c.fg3),
    ),
    onChanged: widget.onChanged,
  );

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      // 有标签时走两列：标签左、输入右（`GroupEditPanel.tsx:76` 起各字段的
      // `grid-template-columns: auto 1fr`）。原先标签在上输入在下，同样的表单高一倍。
      child: widget.label == null
          ? _input(theme)
          : Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                SizedBox(
                  width: 96,
                  child: Text(
                    widget.label!,
                    style: AidogType.micro.copyWith(color: theme.c.fg2),
                  ),
                ),
                const SizedBox(width: AidogSpace.ssm),
                Expanded(child: _input(theme)),
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
    this.max,
    this.blankWhenZero = false,
    this.hint,
  });

  final String label;
  final int value;
  final ValueChanged<int> onChanged;

  /// 上限（对齐 React 那边 `<Input type="number" max=...>`）。null = 不封顶。
  final int? max;

  /// 0 显示成空 + placeholder（`GroupEditPanel.tsx:131-136` 的
  /// `value={editReqTimeout || ""}`）。超时那两个格子「0」的意思是「系统默认」，
  /// 印个 `0` 在那儿会被当成「我设成了 0 秒」。
  final bool blankWhenZero;
  final String? hint;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      // 布局照 [_Field]：标签左、输入右（`GroupEditPanel.tsx:76` 的两列 grid）。
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          SizedBox(
            width: 96,
            child: Text(
              label,
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
          ),
          const SizedBox(width: AidogSpace.ssm),
          // 数字过滤 + 夹取 + ± / ↑↓ 步进都在 [NumberInput] 里（React 那边是
          // `<input type="number" min max>` 自带的四样）。原先是普通文本框 +
          // `tryParse ?? 0`：敲错一个字符，超时 / 重试次数静默变成 0。
          Expanded(
            child: NumberInput(
              value: blankWhenZero && value == 0 ? '' : '$value',
              hint: hint,
              min: 0,
              max: max,
              onChanged: (v) {
                final n = int.tryParse(v.trim());
                if (n != null) onChanged(n);
              },
            ),
          ),
        ],
      ),
    );
  }
}

/// 分组环境变量里的保留字：这几个键由 aidog 自己注入，用户再写一遍会互相覆盖。
/// 判定与提示同源，别在两处各抄一份（`GroupEditPanel.tsx:270`）。
const kReservedEnvKeys = {
  'ANTHROPIC_BASE_URL',
  'ANTHROPIC_AUTH_TOKEN',
  'AIDOG_KEY',
};

/// 编辑页里「一行就是一条记录」的容器：玻璃底 + 描边小卡
/// （`GroupEditPanel.tsx:189-193,272-276`）。[warn] 时描边换 warning 色。
class _RowCard extends StatelessWidget {
  const _RowCard({required this.child, this.warn = false});

  final Widget child;
  final bool warn;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Container(
      margin: const EdgeInsets.only(bottom: 4),
      padding: const EdgeInsets.symmetric(
        horizontal: AidogSpace.ssm,
        vertical: 4,
      ),
      decoration: BoxDecoration(
        color: theme.c.surface2,
        border: Border.all(color: warn ? theme.c.peak : theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: child,
    );
  }
}

/// 复制按钮：图标（可选）+ 文案，点完 1.2 秒内显示「已复制」
/// （React 的 `CopyButton`，全页共用一颗）。
class _CopyChip extends StatefulWidget {
  const _CopyChip({
    required this.tooltip,
    required this.label,
    required this.copyText,
    required this.textOf,
    this.icon,
  });

  final String tooltip;
  final String label;
  final Widget? icon;
  final Future<void> Function(String text) copyText;

  /// 延迟取值：命令要按当前的环境变量现拼。
  final String Function() textOf;

  @override
  State<_CopyChip> createState() => _CopyChipState();
}

class _CopyChipState extends State<_CopyChip> {
  bool _done = false;
  Timer? _reset;

  @override
  void dispose() {
    _reset?.cancel();
    super.dispose();
  }

  Future<void> _copy() async {
    await widget.copyText(widget.textOf());
    if (!mounted) return;
    setState(() => _done = true);
    _reset?.cancel();
    _reset = Timer(const Duration(milliseconds: 1200), () {
      if (mounted) setState(() => _done = false);
    });
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final button = SmallButton(
      label: _done ? t.t('logs.copied') : widget.label,
      active: _done,
      onTap: () => unawaited(_copy()),
    );
    if (widget.icon == null) {
      return Tooltip(message: widget.tooltip, child: button);
    }
    return Tooltip(
      message: widget.tooltip,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(width: 14, height: 14, child: widget.icon),
          const SizedBox(width: 3),
          button,
        ],
      ),
    );
  }
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

/// 批量操作弹窗里的「要动哪些平台」清单。
///
/// 三个批量弹窗（改状态 / 移组 / 删除）确认前都得先让人看见影响范围 ——
/// 这是不可逆操作，不是观感。React 三处各写了一遍同样的滚动清单
/// （`BatchSetStatusModal.tsx:110-140` / `BatchMoveGroupModal.tsx:138-157` /
/// `BatchDeleteModal.tsx:90-128`），这里合成一个。
///
/// [statusBadge] = true 时每行前面带该平台的当前状态徽标（只有改状态那个弹窗要，
/// 因为「把已经禁用的再禁用一遍」需要看得出来）。
class BatchAffectedList extends StatelessWidget {
  const BatchAffectedList({
    super.key,
    required this.platforms,
    this.statusBadge = false,
    this.maxHeight = 220,
  });

  final List<PlatformRow> platforms;
  final bool statusBadge;

  /// React 用的是 `32vh` / `28vh`，这里取一个固定值：弹窗本身已经能整体滚动
  /// （`AidogModal` 里套了 `SingleChildScrollView`），清单再按视口比例算会嵌套两层滚动。
  final double maxHeight;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    if (platforms.isEmpty) return const SizedBox.shrink();
    return Container(
      key: const ValueKey('batch-affected-list'),
      constraints: BoxConstraints(maxHeight: maxHeight),
      margin: const EdgeInsets.only(top: AidogSpace.ssm),
      padding: const EdgeInsets.symmetric(
        horizontal: AidogSpace.ssm,
        vertical: AidogSpace.sxs,
      ),
      decoration: BoxDecoration(
        color: theme.c.surface2,
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: ListView.builder(
        shrinkWrap: true,
        itemCount: platforms.length,
        itemBuilder: (context, i) {
          final p = platforms[i];
          return Padding(
            padding: const EdgeInsets.symmetric(vertical: 3),
            child: Row(
              children: [
                if (statusBadge) ...[
                  MiniBadge(
                    text: switch (p.status) {
                      'enabled' => t.t('platform.statusEnabled'),
                      'auto_disabled' => t.t('platform.statusAutoDisabled'),
                      _ => t.t('platform.statusDisabled'),
                    },
                    color: switch (p.status) {
                      'enabled' => theme.c.ok,
                      'auto_disabled' => theme.c.peak,
                      _ => theme.c.fg3,
                    },
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                ] else ...[
                  Icon(Icons.arrow_forward, size: 12, color: theme.c.fg3),
                  const SizedBox(width: AidogSpace.sxs),
                ],
                Expanded(
                  child: Text(
                    p.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: AidogType.micro.copyWith(color: theme.c.fg),
                  ),
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}

/// 清理失效的影响清单（`GroupListItem.tsx:566-598`）：按 action 分两段，
/// 「将永久删除」在前、「将移出本分组」在后，每行是平台名 + 失效原因徽标。
/// 候选多时自己滚，不把确认卡撑长（React 同处 `maxHeight: 240`）。
class _PurgeCandidateList extends StatelessWidget {
  const _PurgeCandidateList({required this.candidates});

  final List<Map<String, Object?>> candidates;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    Widget section(String action, String titleKey) {
      final items = [
        for (final c in candidates)
          if (c['action'] == action) c,
      ];
      if (items.isEmpty) return const SizedBox.shrink();
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            t.t(titleKey),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.sxs),
          for (final c in items)
            Padding(
              padding: const EdgeInsets.only(bottom: 3),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      (c['name'] as String?) ?? '',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.caption.copyWith(color: theme.c.fg2),
                    ),
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                  MiniBadge(
                    text: c['reason'] == 'auth_failed'
                        ? t.t('platform.purgeDisabledReasonAuthFailed')
                        : t.t('platform.purgeDisabledReasonExpired'),
                    color: theme.c.fg3,
                  ),
                ],
              ),
            ),
          const SizedBox(height: AidogSpace.sxs),
        ],
      );
    }

    return ConstrainedBox(
      constraints: const BoxConstraints(maxHeight: 240),
      child: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            section('delete', 'platform.purgeDisabledActionDelete'),
            section('unassign', 'platform.purgeDisabledActionUnassign'),
          ],
        ),
      ),
    );
  }
}

/// 分组图标（`src/domains/groups/GroupIcon.tsx`）：组里只有一个平台（或只有一个
/// 启用中的平台）就跟随那个平台的 logo，否则画组名前三个字的方块。
/// 自动建组的方块用弱化配色，与手建组一眼可分。
class GroupIcon extends StatelessWidget {
  const GroupIcon({super.key, required this.detail});

  final GroupDetail detail;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final gps = detail.platforms;
    final enabled = [
      for (final gp in gps)
        if (gp.platform.status == 'enabled') gp,
    ];
    final single = gps.length == 1
        ? gps.first.platform
        : enabled.length == 1
        ? enabled.first.platform
        : null;
    // React 这里只走内置图 + favicon（不读缓存图），照抄，不多接一层。
    final logo = single == null
        ? null
        : platformLogo(
            protocol: single.platformType,
            cachedDataUrl: null,
            baseUrl: single.baseUrl,
          );
    if (logo != null) {
      return SizedBox(
        width: 32,
        height: 32,
        child: Padding(padding: const EdgeInsets.all(4), child: logo),
      );
    }
    final auto = detail.group.autoFromPlatform.isNotEmpty;
    return Container(
      width: 32,
      height: 32,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: auto ? theme.c.surface2 : theme.c.accentWash,
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Text(
        detail.group.name.characters.take(3).toString(),
        maxLines: 1,
        overflow: TextOverflow.clip,
        style: AidogType.caption.copyWith(
          color: auto ? theme.c.fg2 : theme.c.accentText,
          fontWeight: FontWeight.w700,
        ),
      ),
    );
  }
}

/// 复制菜单的一项：图标 + 文案。
class _CopyMenuRow extends StatelessWidget {
  const _CopyMenuRow({required this.icon, required this.label});

  final Widget icon;
  final String label;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      SizedBox(width: 14, height: 14, child: icon),
      const SizedBox(width: AidogSpace.ssm),
      Text(label),
    ],
  );
}

/// 内置平台 svg 当小图标。协议没有内置图就留空白（不画占位方块）。
Widget _logoIcon(String protocol) {
  final asset = bundledLogoAsset(protocol);
  if (asset == null) return const SizedBox.shrink();
  return SvgPicture.asset(
    asset,
    fit: BoxFit.contain,
    placeholderBuilder: (_) => const SizedBox.shrink(),
  );
}

/// 分组卡标题行的图标操作。文案不删，挪进 tooltip。
class _GroupIconAction extends StatelessWidget {
  const _GroupIconAction({
    required this.icon,
    required this.tooltip,
    required this.onTap,
    this.danger = false,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback? onTap;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final c = AidogTheme.of(context).c;
    return Tooltip(
      message: tooltip,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 3),
          child: Icon(
            icon,
            size: 14,
            color: onTap == null
                ? c.fg3
                : danger
                ? c.bad
                : c.fg2,
          ),
        ),
      ),
    );
  }
}

/// 单选项：radio 圆点 + 文案，整块可点。
/// 批量弹窗里的「来源 / 模式」是互斥选择，一排高亮按钮讲不出这层语义
/// （React 三个批量弹窗用的都是 radio 组）。
class _RadioChoice extends StatelessWidget {
  const _RadioChoice({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(AidogRadius.sm),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          // 自己画圆点而不是用 Material 的 `Radio`：后者的 `groupValue` /
          // `onChanged` 在 3.32 之后要求外层套 `RadioGroup`，为一个单选项
          // 拉一层祖先不值当。
          Icon(
            selected ? Icons.radio_button_checked : Icons.radio_button_off,
            size: 14,
            color: selected ? theme.c.accentText : theme.c.fg3,
          ),
          const SizedBox(width: 4),
          Text(
            label,
            style: AidogType.micro.copyWith(
              color: selected ? theme.c.fg : theme.c.fg2,
            ),
          ),
          const SizedBox(width: AidogSpace.sxs),
        ],
      ),
    );
  }
}
