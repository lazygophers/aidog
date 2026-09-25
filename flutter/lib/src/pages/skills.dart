/// 技能页界面（票 I09），对应 `src/pages/Skills.tsx` 的三个子视图。
///
/// 结构与 React 同：一个顶层页面，内部 `subView` 在 `list` / `install` 之间切；
/// 详情与七个弹窗的开合是**页面 state 的一部分**（不是 route），浮层本身由
/// [AidogModal] 画进根 Overlay；widget 测试 `find.byType(ConfirmCard)` /
/// `find.text(...)` 照常断言。
///
/// 色值一律 `AidogTheme.of(context).c.*`，本文件零硬编码颜色。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../utils/formatters.dart';
import '../deep_link.dart';
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'invoke.dart';
import 'mini_select.dart';
import 'platform_card_bits.dart' show MiniBadge;
import 'platform_logo.dart' show AgentIconButton;
import 'share_panel.dart';
import 'skills_logic.dart';
import 'ui_bits.dart';

class SkillsPage extends StatefulWidget {
  const SkillsPage({super.key, this.invoke = kernelInvoke});

  final InvokeFn invoke;

  @override
  State<SkillsPage> createState() => _SkillsPageState();
}

class _SkillsPageState extends State<SkillsPage> with WidgetsBindingObserver {
  late final SkillsController _c;
  StreamSubscription<DeepLinkPayload>? _deepLinkSub;

  bool _built = false;

  /// 控制器要用 `AidogI18n.of(context).t` —— 全局 `i18n` 单例在 widget 测试里
  /// 没 init 过，用它会抛「i18n.init() 还没跑完」。`of(context)` 最早只能在
  /// didChangeDependencies 里调，所以构造挪到这儿（只跑一次）。
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_built) return;
    _built = true;
    final tr = AidogI18n.of(context);
    _c = SkillsController(
      invoke: widget.invoke,
      t: tr.t,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    _c.init();
    final pending = deepLinks.takePending('skill');
    if (pending != null) _consumeDeepLink(pending);
    _deepLinkSub = deepLinks.subscribe('skill', _consumeDeepLink);
    WidgetsBinding.instance.addObserver(this);
  }

  void _consumeDeepLink(DeepLinkPayload payload) {
    if (payload.action != 'import' || payload.data.isEmpty) return;
    _c.openDeepLinkImport(payload.data);
  }

  /// React 那边靠 `window focus` + `visibilitychange` 两个事件触发重查；
  /// Flutter 的等价物是应用生命周期回到 resumed。节流仍是控制器里的 10 秒。
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) _c.maybeRevalidate();
  }

  @override
  void dispose() {
    _deepLinkSub?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    _c.dispose();
    super.dispose();
  }

  /// 能不能动技能：没装 node / 项目路径没填 / 正在跑别的活，都不能。
  /// 与 `SkillsView.tsx:66` 的 `!writeReady || scopeInvalid || busyKey` 同口径。
  bool get _ready => _c.writeReady && !_c.scopeInvalid && _c.busyKey == null;

  Future<void> _pickProjectDir() async {
    final selected = await native.pickPath(
      const native.PickPathOptions(directory: true),
    );
    if (selected != null) _c.setProjectPath(selected);
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    if (_c.subView == 'install') {
      return SkillInstallView(
        invoke: widget.invoke,
        scope: _c.scope,
        installedNames: {for (final s in _c.installed) s.name},
        writeReady: _c.writeReady,
        onBack: () => _c.setSubView('list'),
        onInstalled: _c.refreshInstalled,
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        PageHead(
          // 页标题 18 w700 **ls 0**（`SkillsView.tsx:58` 是内联 style，
          // 不是 `.section-title` 的 -0.02em）。
          titleStyle: AidogType.display.copyWith(
            fontSize: 18,
            fontWeight: FontWeight.w700,
            letterSpacing: 0,
          ),
          title: t.t('skills.title'),
          // 「刷新中…」在标题**同一行右侧** 12 secondary（`SkillsView.tsx:59-63`），
          // 不是列表底部那条 meta。
          subtitle: (_c.refreshing && !_c.installedLoading)
              ? t.t('skills.refreshing')
              : null,
          subtitleStyle: AidogType.caption.copyWith(
            fontSize: 12,
            color: AidogTheme.of(context).c.fg2,
          ),
          inlineSubtitle: true,
          trailing: Wrap(
            // 页头按钮间距 8（`SkillsView.tsx:65`）。
            spacing: 8,
            runSpacing: 8,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              // 顺序照 `SkillsView.tsx:66-132`：添加 → 从分享导入 → 刷新 →
              // 更新全部 → 卸载全部 → 卸载选中 → 对齐配置。
              // 七颗都是 shadcn `<Button>` 默认档（36 高 / px-4 / py-2）
              // 外加内联 `fontSize: 12`。
              //
              // 禁用条件照同一段：没装 node（`!writeReady`）/ 项目路径没填
              //（`scopeInvalid`）/ 正在跑别的活（`busyKey != null`）。
              SmallButton(
                // React 这颗没写 variant = 默认实心（`SkillsView.tsx:66-73`）。
                label: t.t('skills.install.addBtn'),
                filled: true,
                fontSize: 12,
                padding: (16, 8),
                onTap: _ready ? () => _c.setSubView('install') : null,
              ),
              SmallButton(
                label: t.t('skills.importFromShare'),
                fontSize: 12,
                padding: (16, 8),
                onTap: _ready ? () => _c.setPasteOpen(true) : null,
              ),
              // 刷新按钮（`SkillsView.tsx:85-94`）。
              SmallButton(
                label: _c.refreshing
                    ? t.t('skills.refreshing')
                    : t.t('skills.refresh'),
                fontSize: 12,
                padding: (16, 8),
                onTap: _c.refreshing ? null : _c.refreshInstalled,
              ),
              // 这几颗按钮跑起来要几秒（都在写外部配置文件），忙碌时换文案，
              // 否则点下去界面一动不动，用户只会再点一次
              //（`SkillsView.tsx:102/111/121/131` 逐颗照抄）。
              SmallButton(
                label: _c.busyKey == '__update__'
                    ? t.t('skills.updating')
                    : t.t('skills.updateAll'),
                fontSize: 12,
                padding: (16, 8),
                onTap: _ready ? _c.updateAll : null,
              ),
              SmallButton(
                label: _c.busyKey == '__uninstall__'
                    ? t.t('skills.uninstalling')
                    : t.t('skills.uninstallAll'),
                danger: true,
                filled: true,
                fontSize: 12,
                padding: (16, 8),
                onTap: _ready ? _c.askUninstallAll : null,
              ),
              SmallButton(
                label: _c.busyKey == '__uninstall_batch__'
                    ? t.t('skills.uninstalling')
                    : t.t('skills.uninstallSelected', {
                        'count': _c.selectedNames.length,
                      }),
                // React `variant="destructive"`（`SkillsView.tsx:113`）= 实心红。
                danger: true,
                filled: true,
                fontSize: 12,
                padding: (16, 8),
                onTap: (_ready && _c.selectedNames.isNotEmpty)
                    ? _c.askUninstallBatch
                    : null,
              ),
              SmallButton(
                label: _c.busyKey == '__align__'
                    ? t.t('skills.aligning')
                    : t.t('skills.alignTitle'),
                fontSize: 12,
                padding: (16, 8),
                onTap: _ready ? _c.openAlign : null,
              ),
            ],
          ),
        ),
        // 环境缺失是**页内常驻提示条**，不是浮动 toast：surface 底 +
        // 左缘 3px accent 竖条 + 12/16 内衬 + 13 secondary
        //（`SkillsView.tsx:140-152`）。
        if (_c.env != null && !_c.writeReady)
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
            child: InlineNote(
              text: t.t('skills.envMissing'),
              padding: const EdgeInsets.symmetric(
                horizontal: 16,
                vertical: 12,
              ),
              leadingBar: AidogTheme.of(context).c.accent,
            ),
          ),
        _statsBar(t),
        const SizedBox(height: AidogSpace.ssm),
        // 搜索 / 筛选行只在「有已装 skills」时出现（`SkillsView.tsx:290`）。
        if (!_c.installedLoading && _c.installed.isNotEmpty) ...[
          _filterBar(t),
          const SizedBox(height: AidogSpace.ssm),
        ],
        // 空态 24/16 内衬 + 13 secondary（`SkillsView.tsx:316,320`）。
        if (_c.scopeInvalid)
          CenteredNote(text: t.t('skills.chooseProjectDir'), fontSize: 13)
        else if (_c.installedLoading)
          CenteredNote(text: t.t('status.loading'), fontSize: 13)
        // 两种空态分开（`SkillsView.tsx:315-322`）：一个都没装 vs 搜不到。
        // 原先都显「暂无已安装」—— 搜了个不存在的词，用户会以为技能全没了。
        else if (_c.installed.isEmpty)
          CenteredNote(
            text: t.t('skills.installedEmpty'),
            fontSize: 13,
            vertical: 24,
          )
        else if (_c.filteredInstalled.isEmpty)
          CenteredNote(
            text: t.t('skills.searchEmpty'),
            fontSize: 13,
            vertical: 24,
          )
        else
          Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final s in _c.filteredInstalled)
                Padding(
                  padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                  child: _SkillRow(
                    skill: s,
                    checked: _c.selectedNames.contains(s.name),
                    busyKey: _c.busyKey,
                    writeReady: _c.writeReady,
                    onCheck: () => _c.toggleSelected(s.name),
                    onOpen: () => _c.setDetailTarget(s),
                    onToggleAgent: (a) => _c.toggle(s, a),
                    onShare: () => _c.share(s),
                    onUninstall: () => _c.askUninstall(s),
                  ),
                ),
            ],
          ),
        // 三个卸载确认都是 `AlertDialog`（无 ✕）：380 宽 / 24 内衬 /
        // 标题 15 w700 / 正文 13 lh1.6 / 页脚按钮 13 + shadcn 默认内衬
        //（`SkillModals.tsx:54-132`）。
        if (_c.confirmUninstall)
          ConfirmCard(
            title: t.t('skills.uninstallAll'),
            body: t.t('skills.uninstallAllConfirm', {'count': _c.total}),
            confirmLabel: t.t('action.confirm'),
            maxWidth: 380,
            titleStyle: _confirmTitleStyle,
            bodyStyle: _confirmBodyStyle(context),
            buttonFontSize: 13,
            buttonPadding: (16, 8),
            onCancel: _c.cancelUninstallAll,
            onConfirm: _c.uninstallAll,
          ),
        if (_c.uninstallTarget != null)
          ConfirmCard(
            title: t.t('skills.uninstall'),
            body: t.t('skills.uninstallConfirm', {
              'name': _c.uninstallTarget!.name,
            }),
            confirmLabel: t.t('action.delete'),
            maxWidth: 380,
            titleStyle: _confirmTitleStyle,
            bodyStyle: _confirmBodyStyle(context),
            buttonFontSize: 13,
            buttonPadding: (16, 8),
            onCancel: _c.cancelUninstall,
            onConfirm: _c.uninstallSingle,
          ),
        if (_c.confirmUninstallBatch)
          ConfirmCard(
            title: t.t('skills.uninstallSelected', {
              'count': _c.selectedNames.length,
            }),
            body: t.t('skills.uninstallSelectedConfirm', {
              'count': _c.selectedNames.length,
            }),
            confirmLabel: t.t('action.delete'),
            maxWidth: 380,
            titleStyle: _confirmTitleStyle,
            bodyStyle: _confirmBodyStyle(context),
            buttonFontSize: 13,
            buttonPadding: (16, 8),
            onCancel: _c.cancelUninstallBatch,
            onConfirm: _c.uninstallBatch,
          ),
        if (_c.alignOpen) _alignCard(t),
        if (_c.pasteOpen) _pasteCard(t),
        if (_c.importIds != null) _importCard(t),
        if (_c.shareData != null) _shareCard(t),
        // 详情是浮层，列表连同筛选状态留在背后（React 是 Radix `Dialog`，
        // `SkillModals.tsx:194-196`）。原先整页替换，关掉详情回来筛选全没了。
        if (_c.detailTarget != null)
          AidogModal(
            // React 是 `min(95vw, 1100px)` 宽、`padding 0`
            //（`SkillDetailView.tsx:108-117`）：内衬由头部与两栏自己管。
            maxWidth: 1100,
            onBarrierTap: () => _c.setDetailTarget(null),
            child: ModalCard(
              padding: EdgeInsets.zero,
              onClose: () => _c.setDetailTarget(null),
              child: SkillDetailView(
                invoke: widget.invoke,
                skill: _c.detailTarget!,
                onClose: () => _c.setDetailTarget(null),
              ),
            ),
          ),
        // 批量卸载期间的全页遮罩（React `SkillsView.tsx:34-53` 只盖
        // `__uninstall_batch__`）：单行 / 全部卸载不盖全页遮罩，只禁按钮 ——
        // 照 React 真值，原先 `__uninstall__`（卸载全部）也盖了。
        if (_c.busyKey == '__uninstall_batch__')
          _BusyOverlay(text: t.t('skills.uninstalling')),
        if (_c.message != null)
          // React 这条 portal 浮层写的是 `top: 16`（`SkillsView.tsx:160`），
          // 不是平台页 toast 的 24。
          ToastBar(
            text: _c.message!,
            ok: true,
            top: 16,
            onDismiss: _c.clearMessage,
          ),
      ],
    );
  }

  /// 三个卸载确认弹窗的标题：15 w700（`SkillModals.tsx:57,86,112`）。
  static final TextStyle _confirmTitleStyle = AidogType.title.copyWith(
    fontSize: 15,
    fontWeight: FontWeight.w700,
  );

  /// 同上的正文：13 secondary lh1.6（`SkillModals.tsx:60`）。
  static TextStyle _confirmBodyStyle(BuildContext context) =>
      AidogType.label.copyWith(
        fontSize: 13,
        height: 1.6,
        color: AidogTheme.of(context).c.fg2,
      );

  /// 统计 + scope 合并卡（`SkillsView.tsx:203-287`）：左侧大数字（总计 40px
  /// + 每 agent 计数 + enableAll），右侧范围筛选。原先只是一行小字副标题 ——
  /// 数字不带视觉重量，扫一眼抓不到「装了多少、各 agent 开了多少」。
  Widget _statsBar(I18nController t) {
    final theme = AidogTheme.of(context);
    final counts = _c.agentCounts;
    return Tile(
      // React 统计卡容器 20/24（`SkillsView.tsx:205`），不是 Tile 缺省 16/14。
      padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 20),
      child: Wrap(
        // 卡内主间距 28（`SkillsView.tsx:210,215`）。
        spacing: 28,
        runSpacing: AidogSpace.ssm,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          // 总计大数字（React：fontSize 40 / 800 / accent，`SkillsView.tsx:216-223`）。
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                '${_c.total}',
                style: AidogType.body.copyWith(
                  fontSize: 40,
                  fontWeight: FontWeight.w800,
                  height: 1,
                  color: theme.c.accentText,
                ),
              ),
              Text(
                t.t('skills.total'),
                // 「已安装总计」12 secondary（`SkillsView.tsx:220`）。
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg2,
                ),
              ),
            ],
          ),
          // agent 组之间 20（`SkillsView.tsx:224`），组内 8（`:226`）。
          Wrap(
            spacing: 20,
            runSpacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              // 每 agent：图标 + 计数 + 「全部启用」（`SkillsView.tsx:224-245`）。
              for (final a in kSkillAgents)
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    SvgPicture.asset(
                      AgentIconButton.assetFor(a) ?? '',
                      width: 22,
                      height: 22,
                    ),
                    const SizedBox(width: 8),
                    Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          '${counts[a] ?? 0}',
                          style: AidogType.body.copyWith(
                            fontSize: 18,
                            fontWeight: FontWeight.w700,
                            height: 1.1,
                          ),
                        ),
                        Text(
                          t.t('skills.agent.$a'),
                          // agent 名标签 11 secondary（`SkillsView.tsx:230`）。
                          style: AidogType.caption.copyWith(
                            fontSize: 11,
                            color: theme.c.fg2,
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(width: 8),
                    SmallButton(
                      label: _c.busyKey == '__enableall_${a}__'
                          ? t.t('skills.enabling')
                          : t.t('skills.enableAll'),
                      // 「全部启用」11 / 3px 8px（`SkillsView.tsx:237`）。
                      fontSize: 11,
                      padding: (8, 3),
                      // 禁用条件照 `SkillsView.tsx：238`：未就绪 / 忙 / 空列表 /
                      // 该 agent 已全部启用（再点是空操作）。
                      onTap:
                          (_c.writeReady &&
                              !_c.scopeInvalid &&
                              _c.busyKey == null &&
                              _c.installed.isNotEmpty &&
                              counts[a] != _c.installed.length)
                          ? () => _c.enableAll(a)
                          : null,
                    ),
                  ],
                ),
              // pi：无 per-skill 启停，全部常开，只报总数不给按钮
              //（`SkillsView.tsx:246-255`）。
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  SvgPicture.asset(
                    'packages/aidog_platform_logos/pi.svg',
                    width: 22,
                    height: 22,
                  ),
                  const SizedBox(width: 8),
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        '${_c.total}',
                        style: AidogType.body.copyWith(
                          fontSize: 18,
                          fontWeight: FontWeight.w700,
                          height: 1.1,
                        ),
                      ),
                      Text(
                        t.t('skills.piAlwaysOnDesc'),
                        style: AidogType.caption.copyWith(
                          fontSize: 11,
                          color: theme.c.fg2,
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ],
          ),
          // 右侧：范围筛选（`SkillsView.tsx:259-286`）。
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            mainAxisSize: MainAxisSize.min,
            children: [
              Wrap(
                // 标签↔下拉 8、列内 8、字号 13（`SkillsView.tsx:260-271`）。
                spacing: 8,
                runSpacing: AidogSpace.sxs,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  FieldLabel(t.t('skills.scope'), fontSize: 13),
                  // React 是 `Select` 下拉（`SkillsView.tsx:262-270`）。
                  MiniSelect(
                    key: const ValueKey('skills-scope'),
                    value: _c.scopeKind,
                    fontSize: 13,
                    options: const ['global', 'project'],
                    labelOf: (v) => v == 'global'
                        ? t.t('skills.scopeGlobal')
                        : t.t('skills.scopeProject'),
                    onChanged: (v) => _c.setScopeKind(v!),
                  ),
                ],
              ),
              if (_c.scopeKind == 'project')
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  // 输入框 `flex: 1`、按钮跟在后面、列间 8
                  //（`SkillsView.tsx:274-284`）。Wrap 里拿不到有界宽度，
                  // 给一个固定宽的盒子，`Expanded` 才有得分。
                  child: SizedBox(
                    width: 380,
                    child: Row(
                      children: [
                        Expanded(
                          child: KeptTextField(
                            key: const Key('skills-project-path'),
                            value: _c.projectPath,
                            // `<Input>` 是 14（shadcn `text-sm`）。
                            fontSize: 14,
                            hint: t.t('skills.projectPathPlaceholder'),
                            onSubmitted: _c.setProjectPath,
                          ),
                        ),
                        const SizedBox(width: 8),
                        SmallButton(
                          // React 这颗写的是 `skills.browse`（「浏览…」），
                          // 不是路径占位文案（`SkillsView.tsx:281-283`）。
                          label: t.t('skills.browse'),
                          fontSize: 12,
                          padding: (16, 8),
                          onTap: _pickProjectDir,
                        ),
                      ],
                    ),
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }

  /// 搜索框 + 启用态筛选。React 这一行**没有卡面**，就是页面流里的一个
  /// `display:flex; gap:8`（`SkillsView.tsx:291-308`）—— 原先外面套了一层 `Tile`。
  Widget _filterBar(I18nController t) => Row(
    children: [
      Expanded(
        child: TextField(
          key: const Key('skills-search'),
          decoration: InputDecoration(
            isDense: true,
            hintText: t.t('skills.searchPlaceholder'),
          ),
          // Input `flex:1` + 13（`SkillsView.tsx:292-297`）。
          style: AidogType.label.copyWith(
            fontSize: 13,
            color: AidogTheme.of(context).c.fg,
          ),
          onChanged: _c.setSearchQuery,
        ),
      ),
      const SizedBox(width: 8),
      // 三个筛选 key 直接写死，不拼 —— 拼出来的 key `check-ui-parity.mjs`
      // 这类扫字面量的工具照不出来（单引号串里 `\$` 还是字面美元符，
      // 原先平铺按钮版就在这里翻过车）。
      MiniSelect(
        key: const ValueKey('skills-enabled-filter'),
        value: _c.enabledFilter,
        fontSize: 13,
        options: const ['all', 'enabled', 'disabled'],
        labelOf: (f) => switch (f) {
          'enabled' => t.t('skills.filterEnabled'),
          'disabled' => t.t('skills.filterDisabled'),
          _ => t.t('skills.filterAll'),
        },
        onChanged: (v) => _c.setEnabledFilter(v!),
      ),
    ],
  );

  // React 是普通 `Dialog`（`SkillModals.tsx:135`，maxWidth 400 / padding 24 /
  // gap 16），点遮罩可关，右上角自带 ✕。
  Widget _alignCard(I18nController t) {
    final theme = AidogTheme.of(context);
    final same = _c.alignFrom == _c.alignTo;
    return AidogModal(
      maxWidth: 400,
      onBarrierTap: _c.closeAlign,
      child: ModalCard(
        title: t.t('skills.alignTitle'),
        // 标题 15 w700（`SkillModals.tsx:138`）。
        titleStyle: _confirmTitleStyle,
        onClose: _c.closeAlign,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 两行各一行：标签定宽 72 / 13、标签↔下拉 8、两行之间 10
            //（`SkillModals.tsx:142-168`）。原先四个控件挤在一个 Wrap 里。
            _alignRow(
              t,
              label: t.t('skills.alignFrom'),
              child: MiniSelect(
                key: const ValueKey('skills-align-from'),
                value: _c.alignFrom,
                fontSize: 13,
                options: kSkillAgents,
                labelOf: (a) => t.t('skills.agent.$a'),
                onChanged: (v) => _c.setAlignFrom(v!),
              ),
            ),
            const SizedBox(height: 10),
            _alignRow(
              t,
              label: t.t('skills.alignTo'),
              child: MiniSelect(
                key: const ValueKey('skills-align-to'),
                value: _c.alignTo,
                fontSize: 13,
                options: kSkillAgents,
                labelOf: (a) => t.t('skills.agent.$a'),
                onChanged: (v) => _c.setAlignTo(v!),
              ),
            ),
            const SizedBox(height: 16),
            // 说明排在两个下拉**之下**，且与「源与目标不能相同」二选一显示在
            // 正文里（`SkillModals.tsx:170-177`）。原先说明在上、同名提示只在
            // 按钮 tooltip 里 —— 禁用的原因用户看不见。
            Text(
              same
                  ? t.t('skills.alignSameAgent')
                  : t.t('skills.alignConfirm', {
                      'from': t.t('skills.agent.${_c.alignFrom}'),
                      'to': t.t('skills.agent.${_c.alignTo}'),
                    }),
              style: AidogType.caption.copyWith(
                fontSize: 12,
                height: 1.6,
                color: theme.c.fg2,
              ),
            ),
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  fontSize: 13,
                  padding: (16, 8),
                  onTap: _c.closeAlign,
                ),
                const SizedBox(width: 8),
                SmallButton(
                  // React 主按钮文案就是「对齐配置」（`SkillModals.tsx:188`）。
                  label: t.t('skills.alignTitle'),
                  filled: true,
                  fontSize: 13,
                  padding: (16, 8),
                  onTap: same ? null : _c.align,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  /// 对齐弹窗的一行：标签定宽 72 + 13，下拉占满余下（`SkillModals.tsx:143-155`）。
  Widget _alignRow(
    I18nController t, {
    required String label,
    required Widget child,
  }) => Row(
    children: [
      SizedBox(
        width: 72,
        child: FieldLabel(label, fontSize: 13),
      ),
      const SizedBox(width: 8),
      Expanded(child: child),
    ],
  );

  // React 是普通 `Dialog`（`SkillModals.tsx:214`，maxWidth 560 / padding 20 /
  // gap 10），点遮罩可关，右上角自带 ✕。
  Widget _pasteCard(I18nController t) {
    final theme = AidogTheme.of(context);
    return AidogModal(
      maxWidth: 560,
      onBarrierTap: () => _c.setPasteOpen(false),
      child: ModalCard(
        title: t.t('skills.pasteTitle'),
        titleStyle: _confirmTitleStyle,
        padding: const EdgeInsets.all(20),
        onClose: () => _c.setPasteOpen(false),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 说明是**独立一行** 12 lh1.5（`SkillModals.tsx:221-223`）；
            // 原先塞进输入框的 hint 里，一打字就没了。
            Text(
              t.t('skills.pasteHint'),
              style: AidogType.caption.copyWith(
                fontSize: 12,
                height: 1.5,
                color: theme.c.fg2,
              ),
            ),
            const SizedBox(height: 10),
            // `<Textarea minHeight 160>` + 等宽字体（`SkillModals.tsx:224-229`）。
            SizedBox(
              height: 160,
              child: TextField(
                key: const Key('skills-paste'),
                maxLines: null,
                expands: true,
                textAlignVertical: TextAlignVertical.top,
                decoration: const InputDecoration(
                  isDense: true,
                  // placeholder 照抄 React 的示例串（`SkillModals.tsx:227`）。
                  hintText:
                      'aidog://skill/import?data=... 或 base64 或 JSON 数组',
                ),
                style: AidogType.numSm.copyWith(color: theme.c.fg),
                onChanged: _c.setPasteText,
              ),
            ),
            const SizedBox(height: 10),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  fontSize: 13,
                  padding: (16, 8),
                  onTap: () => _c.setPasteOpen(false),
                ),
                const SizedBox(width: 8),
                SmallButton(
                  // 主按钮文案是「导入」，不是「确认」（`SkillModals.tsx:240`）。
                  label: t.t('skills.importBtn'),
                  filled: true,
                  fontSize: 13,
                  padding: (16, 8),
                  onTap: _c.pasteImport,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  // React 是普通 `Dialog`（`SkillModals.tsx:247-251`，maxWidth 560 /
  // padding 22 / gap 12 / maxHeight 82vh）：装载中不许关。
  Widget _importCard(I18nController t) {
    final theme = AidogTheme.of(context);
    final ids = _c.importIds!;
    return AidogModal(
      maxWidth: 560,
      onBarrierTap: _c.importBusy ? null : _c.cancelImport,
      child: ModalCard(
        title: t.t('skills.importConfirmTitle'),
        titleStyle: _confirmTitleStyle,
        padding: const EdgeInsets.all(22),
        onClose: _c.importBusy ? null : _c.cancelImport,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 说明行是**正体** 12 lh1.5（`SkillModals.tsx:257`），
            // 不是 `TileMeta` 的全大写 micro。
            Text(
              t.t('skills.importConfirmDesc', {'count': ids.length}),
              style: AidogType.caption.copyWith(
                fontSize: 12,
                height: 1.5,
                color: theme.c.fg2,
              ),
            ),
            const SizedBox(height: 12),
            // id 清单：描边容器 + 8 内衬 + r8 + 30vh 上限可滚 + 每行等宽 12
            //（`SkillModals.tsx:261-264`）。原先是裸 Text 列表、装多了撑爆弹窗。
            ConstrainedBox(
              constraints: BoxConstraints(
                maxHeight: MediaQuery.sizeOf(context).height * 0.3,
              ),
              child: Container(
                padding: const EdgeInsets.all(8),
                decoration: BoxDecoration(
                  color: theme.c.surface2,
                  border: Border.all(color: theme.c.line),
                  borderRadius: BorderRadius.circular(AidogRadius.sm),
                ),
                child: SingleChildScrollView(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      for (var i = 0; i < ids.length; i++)
                        Padding(
                          padding: EdgeInsets.only(top: i == 0 ? 0 : 4),
                          child: Text(
                            ids[i],
                            style: AidogType.numSm.copyWith(
                              fontSize: 12,
                              color: theme.c.fg2,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
              ),
            ),
            const SizedBox(height: 12),
            // scope 与 agent 各占**一行**，标签定宽 56 / 12
            //（`SkillModals.tsx:268-295`）。原先两件事挤在一个 Wrap 里。
            Row(
              children: [
                SizedBox(
                  width: 56,
                  child: FieldLabel(t.t('skills.scope'), fontSize: 12),
                ),
                const SizedBox(width: 8),
                MiniSelect(
                  key: const ValueKey('skills-import-scope'),
                  value: _c.importScopeKind,
                  fontSize: 12,
                  options: const ['global', 'project'],
                  labelOf: (v) => v == 'global'
                      ? t.t('skills.scopeGlobal')
                      : t.t('skills.scopeProject'),
                  onChanged: (v) => _c.setImportScopeKind(v!),
                ),
                if (_c.importScopeKind == 'project') ...[
                  const SizedBox(width: 8),
                  Expanded(
                    child: TextField(
                      key: const Key('skills-import-path'),
                      decoration: InputDecoration(
                        isDense: true,
                        hintText: t.t('skills.projectPathPlaceholder'),
                      ),
                      style: AidogType.label.copyWith(
                        fontSize: 14,
                        color: theme.c.fg,
                      ),
                      onChanged: _c.setImportProjectPath,
                    ),
                  ),
                ],
              ],
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                SizedBox(
                  width: 56,
                  child: FieldLabel(t.t('skills.importAgents'), fontSize: 12),
                ),
                const SizedBox(width: 10),
                // agent 多选按钮带 16px 图标（`SkillModals.tsx:313-324`）：
                // 4/10 内衬、gap 6、12 字、未选 0.5 —— 原先是纯文字按钮。
                for (final a in kSkillAgents)
                  Padding(
                    padding: const EdgeInsets.only(right: 10),
                    child: AgentIconButton(
                      agent: a,
                      enabled: _c.importAgents.contains(a),
                      supported: true,
                      tooltip: t.t('skills.agent.$a'),
                      label: t.t('skills.agent.$a'),
                      iconSize: 16,
                      fontSize: 12,
                      padding: (10, 4),
                      lockHeight: false,
                      offOpacity: 0.5,
                      onTap: () => _c.toggleImportAgent(a),
                    ),
                  ),
              ],
            ),
            // 页脚 gap 12 + marginTop 4（`SkillModals.tsx:251,329`）。
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  fontSize: 13,
                  padding: (16, 8),
                  onTap: _c.importBusy ? null : _c.cancelImport,
                ),
                const SizedBox(width: 8),
                SmallButton(
                  label: _c.importBusy
                      ? t.t('skills.installing')
                      // 主按钮文案是「导入」（`SkillModals.tsx:339`）。
                      : t.t('skills.importBtn'),
                  filled: true,
                  fontSize: 13,
                  padding: (16, 8),
                  // 一个 agent 都没选就装不了；项目 scope 要有路径；
                  // 没装 npx 也装不了（`SkillModals.tsx:337` 的 `!env?.npx_available`，
                  // 原先这条漏了 —— 点下去必然失败）。
                  onTap:
                      _c.importBusy ||
                          _c.importAgents.isEmpty ||
                          !_c.writeReady ||
                          (_c.importScopeKind == 'project' &&
                              _c.importProjectPath.trim().isEmpty)
                      ? null
                      : _c.runImport,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  /// React 复用的是同一个泛化 `ShareModal`（`SkillModals.tsx:200-210`）：
  /// 4 种格式 + 自动复制 + `aidog://skill/import` 深链二维码。
  /// 分享体是 `{skills: [catalogId]}`（`useSkillsData.ts:423`）。
  Widget _shareCard(I18nController t) {
    final data = _c.shareData!;
    return SharePanel(
      share: {'skills': data.skills},
      title: data.name,
      urlScheme: 'aidog://skill/import',
      titleKey: 'skills.share.title',
      warningKey: 'skills.share.warning',
      onToast: (text, {required ok}) => _c.setMessage(text),
      onClose: _c.closeShare,
    );
  }
}

/// 已装列表的一行：勾选 + 名字（点开详情）+ 两个 agent 开关 + 分享 / 卸载。
class _SkillRow extends StatelessWidget {
  const _SkillRow({
    required this.skill,
    required this.checked,
    required this.busyKey,
    required this.writeReady,
    required this.onCheck,
    required this.onOpen,
    required this.onToggleAgent,
    required this.onShare,
    required this.onUninstall,
  });

  final SkillInfo skill;
  final bool checked;

  /// 全页 busyKey（`<name>::<agent>` / `__update__` / `__uninstall_single_<name>__`
  /// …）；非 null 时行内按钮全部禁用（React `SkillsView.tsx:571` 的
  /// `busyKey !== null`，原先卸载按钮漏了这条）。
  final String? busyKey;
  final bool writeReady;
  final VoidCallback onCheck;
  final VoidCallback onOpen;
  final void Function(String agent) onToggleAgent;
  final VoidCallback onShare;
  final VoidCallback onUninstall;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final busy = busyKey != null;
    return Tile(
      // React 行卡 12/16（SkillsView.tsx:384）。
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          // 15×15 勾选框（`SkillsView.tsx:393`）；Material 缺省是 18 + 40×40 靶。
          TinyCheckbox(value: checked, onChanged: busy ? null : onCheck),
          // 卡内各列间距 12（`SkillsView.tsx:384`）。
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                // 名字与所有 chip 在**同一行**（`SkillsView.tsx:396-473`），
                // 行内 gap 8。原先来源被拆成独立一列。
                Wrap(
                  spacing: 8,
                  runSpacing: 2,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Tooltip(
                      // 点名字开详情这件事本身看不出来，React 把它写在 `title=` 上
                      //（`SkillsView.tsx:400`）。
                      message: t.t('skills.detail.view'),
                      child: InkWell(
                        onTap: onOpen,
                        child: Text(
                          skill.name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: AidogType.body.copyWith(
                            fontSize: 13,
                            // React 这处是内联 13 w600，字距 normal
                            //（`SkillsView.tsx:401`），不带 body 档的 -0.16。
                            letterSpacing: 0,
                            color: theme.c.fg,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                      ),
                    ),
                    // 来源是**带底色描边的胶囊链接**（`SkillsView.tsx:408-430`）：
                    // 11 / 2px 8px / r6 / accent-subtle 底 / accent 字 / 1px border。
                    if ((skill.source ?? '').isNotEmpty)
                      Tooltip(
                        message: skill.sourceUrl ?? skill.source!,
                        child: InkWell(
                          onTap: () => native.openUrl(
                            skill.sourceUrl ??
                                'https://github.com/${skill.source}',
                          ),
                          child: MiniBadge(
                            text: skill.source!,
                            color: theme.c.accentText,
                            background: theme.c.accentWash,
                            borderColor: theme.c.line,
                            fontSize: 11,
                            padX: 8,
                            padY: 2,
                            radius: 6,
                          ),
                        ),
                      ),
                    // sourceType / plugin 徽标是 shadcn `Badge variant=outline`：
                    // bg-floating 底 + secondary 字 + 中性边，不是语义色兑水
                    //（`SkillsView.tsx:432-461`）。
                    if ((skill.sourceType ?? '').isNotEmpty)
                      MiniBadge(
                        text: skill.sourceType!.toUpperCase(),
                        color: theme.c.fg2,
                        background: theme.c.surface2,
                        borderColor: theme.c.line,
                        padY: 2,
                        letterSpacing: 0.3,
                        tooltip: t.t('skills.sourceType'),
                      ),
                    if ((skill.pluginName ?? '').isNotEmpty)
                      MiniBadge(
                        text: 'plugin: ${skill.pluginName}',
                        color: theme.c.fg2,
                        background: theme.c.surface2,
                        borderColor: theme.c.line,
                        padY: 2,
                        tooltip: t.t('skills.pluginName'),
                      ),
                    // 更新时间显示的是**相对时间**（「3 天前」），绝对时刻进
                    // tooltip（`SkillsView.tsx:462-472`）。原先直接打印 ISO 原串。
                    if ((skill.updatedAt ?? '').isNotEmpty)
                      Tooltip(
                        message:
                            '${t.t('skills.updatedAt')}: '
                            '${formatDateTimeIso(skill.updatedAt)}',
                        child: Text(
                          formatRelativeTimeIso(skill.updatedAt),
                          style: AidogType.micro.copyWith(
                            letterSpacing: 0,
                            color: theme.c.fg2,
                          ),
                        ),
                      ),
                  ],
                ),
                // 描述 12 secondary + 上距 4（`SkillsView.tsx:475`）。
                if ((skill.description ?? '').isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Text(
                      skill.description!,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.caption.copyWith(
                        fontSize: 12,
                        color: theme.c.fg2,
                      ),
                    ),
                  ),
                _SkillInstalledLine(skill: skill),
              ],
            ),
          ),
          const SizedBox(width: 12),
          // 操作区要能换行：agent 开关的文案带上状态之后变长了，窄窗下一行放不下，
          // 裸 `Wrap` 在 `Row` 里拿到的是无限宽约束，永远不换行 —— 必须给它
          // 一个有界宽度（`Flexible`），`Wrap` 才会真的折行。
          Flexible(
            child: Wrap(
              alignment: WrapAlignment.end,
              // agent 组与行尾按钮的间距都是 8（`SkillsView.tsx:494,559,570`）。
              spacing: 8,
              runSpacing: 8,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                // 每个 agent 一个开关：亮 = 已启用。禁写（无 npx）时点不动。
                // agent 开关：按钮上写的是**当前状态**（启用 / 未启用 / …），
                // 不是 agent 名 —— 只写 agent 名的话，开没开全靠底色猜
                //（`SkillsView.tsx:530`）。agent 名挪进 tooltip。
                for (final a in kSkillAgents)
                  AgentIconButton(
                    agent: a,
                    enabled: skill.enabledAgents.contains(a),
                    supported: true,
                    // agent 名进 tooltip（React 的 `alt`），按钮上写的是**状态**。
                    tooltip:
                        '${t.t('skills.agent.$a')} · '
                        '${skill.enabledAgents.contains(a) ? t.t('skills.disableAgent') : t.t('skills.enableAgent')}',
                    label: busy
                        ? t.t('skills.toggling')
                        : skill.enabledAgents.contains(a)
                        ? t.t('skills.on')
                        : t.t('skills.off'),
                    // 5px 10px 内衬、高度由内容撑、未启用 0.45
                    //（`SkillsView.tsx:513-531`）。
                    padding: (10, 5),
                    lockHeight: false,
                    fontSize: 11,
                    offOpacity: 0.45,
                    onTap: busy || !writeReady ? null : () => onToggleAgent(a),
                  ),
                // pi 是静态徽标不是开关：pi 原生扫公共 skill 目录，没有 per-skill
                // 启停概念，做成可点开关就是在骗用户（`SkillsView.tsx:536-537` 原注释）。
                _PiAlwaysOnBadge(),
                // 分享按钮只在 catalog 来源（有 source）时出现
                //（React `SkillsView.tsx:554-557` 的 `skillCatalogId(skill) &&`：
                // 手动 symlink 的 skill 无 source，点不出任何东西，直接隐藏）。
                if (skillCatalogId(skill) != null)
                  SmallButton(
                    label: t.t('skills.share.title'),
                    // 11 / 4px 10px（`SkillsView.tsx:559`）。
                    fontSize: 11,
                    padding: (10, 4),
                    onTap: onShare,
                  ),
                SmallButton(
                  // 单条卸载：React `variant="destructive"`（`SkillsView.tsx:567`），
                  // busy 期间换「卸载中…」并禁用（`SkillsView.tsx:575-577`）。
                  label: busyKey == '__uninstall_single_${skill.name}__'
                      ? t.t('skills.uninstalling')
                      : t.t('skills.uninstall'),
                  danger: true,
                  filled: true,
                  fontSize: 11,
                  padding: (10, 4),
                  onTap: busy || !writeReady ? null : onUninstall,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// pi 的静态徽标：18px pi 图标 + 「常开」11 w600，外包 radius-md 描边胶囊
///（`SkillsView.tsx:537-552`）。原先是纯文字 [MiniBadge]，图标丢了。
class _PiAlwaysOnBadge extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Tooltip(
      message: t.t('skills.piAlwaysOnHint'),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        decoration: BoxDecoration(
          color: theme.c.surface2,
          border: Border.all(color: theme.c.line),
          borderRadius: BorderRadius.circular(AidogRadius.md),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            SvgPicture.asset(
              'packages/aidog_platform_logos/pi.svg',
              width: 18,
              height: 18,
            ),
            const SizedBox(width: 6),
            Text(
              t.t('skills.piAlwaysOn'),
              style: AidogType.micro.copyWith(
                letterSpacing: 0,
                fontWeight: FontWeight.w600,
                color: theme.c.fg2,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// 「安装于 `<时刻>` · `<hash 前 7 位>`」那行次要信息（`SkillsView.tsx:478-491`）：
/// 10 secondary / 上距 2 / 整体 opacity .8；分隔点左右各 6 且更淡；
/// hash 走等宽。时刻是**格式化过的本地时刻**，ISO 原串进 tooltip。
class _SkillInstalledLine extends StatelessWidget {
  const _SkillInstalledLine({required this.skill});

  final SkillInfo skill;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final installedAt = skill.installedAt ?? '';
    final hash = skill.skillFolderHash ?? '';
    if (installedAt.isEmpty) return const SizedBox.shrink();
    final base = AidogType.micro.copyWith(
      fontSize: 10,
      letterSpacing: 0,
      color: theme.c.fg2,
    );
    return Padding(
      padding: const EdgeInsets.only(top: 2),
      child: Opacity(
        opacity: 0.8,
        child: Tooltip(
          message: installedAt,
          child: Text.rich(
            TextSpan(
              children: [
                TextSpan(text: '${t.t('skills.installedAt')}: '),
                TextSpan(text: formatDateTimeIso(installedAt)),
                if (hash.isNotEmpty) ...[
                  TextSpan(
                    text: '      ·      ',
                    style: base.copyWith(color: theme.c.fg2.withValues(alpha: 0.5)),
                  ),
                  TextSpan(
                    text: hash.substring(
                      0,
                      hash.length < 7 ? hash.length : 7,
                    ),
                    style: AidogType.numSm.copyWith(
                      fontSize: 10,
                      color: theme.c.fg2,
                    ),
                  ),
                ],
              ],
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: base,
          ),
        ),
      ),
    );
  }
}

/// 15×15 勾选框 —— React 两处批量勾选都是原生 `<input type=checkbox>` 写死
/// `width/height: 15`（`SkillsView.tsx:393`、`SkillInstallView.tsx:377`）。
/// Material 的 `Checkbox` 缺省 18 + 40×40 点击靶，行高会被它整个撑开。
class TinyCheckbox extends StatelessWidget {
  const TinyCheckbox({super.key, required this.value, this.onChanged});

  final bool value;

  /// null = 禁用。
  final VoidCallback? onChanged;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: 15,
    height: 15,
    child: FittedBox(
      fit: BoxFit.contain,
      child: Checkbox(
        value: value,
        visualDensity: VisualDensity.compact,
        materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
        activeColor: AidogTheme.of(context).c.accent,
        onChanged: onChanged == null ? null : (_) => onChanged!(),
      ),
    ),
  );
}

// ── 搜索安装子视图 ────────────────────────────────────────────────

class SkillInstallView extends StatefulWidget {
  const SkillInstallView({
    super.key,
    this.invoke = kernelInvoke,
    required this.scope,
    required this.installedNames,
    required this.writeReady,
    required this.onBack,
    required this.onInstalled,
  });

  final InvokeFn invoke;
  final Map<String, Object?> scope;
  final Set<String> installedNames;
  final bool writeReady;
  final VoidCallback onBack;
  final VoidCallback onInstalled;

  @override
  State<SkillInstallView> createState() => _SkillInstallViewState();
}

class _SkillInstallViewState extends State<SkillInstallView> {
  late final SkillInstallController _c;

  bool _built = false;

  /// 同 [SkillsPage]：控制器的 t 必须来自 context，不能是全局单例。
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_built) return;
    _built = true;
    _c = SkillInstallController(
      invoke: widget.invoke,
      t: AidogI18n.of(context).t,
      onChanged: () {
        if (mounted) setState(() {});
      },
      scope: widget.scope,
      onInstalled: widget.onInstalled,
    );
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // 批量安装期间的全页遮罩（React `SkillInstallView.tsx:201-222`）：
        // 安装要跑 npx、可能连装好几个，原先只禁按钮 / 换文案，页面看不出在忙。
        if (_c.busyId == '__batch__')
          _BusyOverlay(text: t.t('skills.install.installing')),
        PageHead(
          // 全页 gap 16（`SkillInstallView.tsx:201`）。
          bottom: 16,
          title: t.t('skills.install.title'),
          // 标题 18 w700 ls0（`SkillInstallView.tsx:236`）。
          titleStyle: AidogType.display.copyWith(
            fontSize: 18,
            fontWeight: FontWeight.w700,
            letterSpacing: 0,
          ),
          // 「搜索中…」在标题同行右侧 12 secondary（`SkillInstallView.tsx:239-243`）。
          subtitle: _c.loading ? t.t('skills.install.searching') : null,
          subtitleStyle: AidogType.caption.copyWith(
            fontSize: 12,
            color: AidogTheme.of(context).c.fg2,
          ),
          inlineSubtitle: true,
          // 「← 返回」排在标题**左边**，ghost 变体 + 12
          //（`SkillInstallView.tsx:226-235`）。
          leading: SmallButton(
            label: t.t('skills.install.back'),
            ghost: true,
            fontSize: 12,
            padding: (16, 8),
            onTap: _c.busyId != null ? null : widget.onBack,
          ),
          trailing: SmallButton(
            label: t.t('skills.install.installSelected', {
              'count': _c.checked.length,
            }),
            filled: true,
            fontSize: 12,
            padding: (16, 8),
            onTap:
                _c.busyId != null || !widget.writeReady || _c.checked.isEmpty
                ? null
                : _c.installBatch,
          ),
        ),
        TextField(
          key: const Key('skills-install-search'),
          autofocus: true,
          decoration: InputDecoration(
            isDense: true,
            hintText: t.t('skills.install.searchPlaceholder'),
          ),
          // `<Input>` 14（shadcn `text-sm`，`SkillInstallView.tsx:259-267`）。
          style: AidogType.label.copyWith(
            fontSize: 14,
            color: AidogTheme.of(context).c.fg,
          ),
          onChanged: _c.setKeyword,
        ),
        const SizedBox(height: 16),
        // 消息 / 错误都是**页内常驻条**，不是浮动 toast
        //（`SkillInstallView.tsx:270-291`）：消息 8/12 13 secondary，
        // 错误 12/16 13 danger。
        if (_c.message != null) ...[
          InlineNote(
            text: _c.message!,
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          ),
          const SizedBox(height: 16),
        ],
        if (_c.error != null) ...[
          InlineNote(
            text: '${t.t('skills.install.loadFailed')}: ${_c.error}',
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            color: AidogTheme.of(context).c.bad,
          ),
          const SizedBox(height: 16),
        ],
        // 空态 / 无结果：32/16 内衬 + 13（`SkillInstallView.tsx:294-313`）。
        if (!_c.loading && _c.error == null && !_c.hasKeyword)
          CenteredNote(
            text: t.t('skills.install.emptyHint'),
            fontSize: 13,
            vertical: 32,
          )
        else if (!_c.loading && _c.error == null && _c.results.isEmpty)
          CenteredNote(
            text: t.t('skills.install.noResults'),
            fontSize: 13,
            vertical: 32,
          )
        else if (!_c.loading)
          Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final e in _c.results)
                Padding(
                  // 结果卡间距 8（`SkillInstallView.tsx:317`）。
                  padding: const EdgeInsets.only(bottom: 8),
                  child: _CatalogRow(
                    entry: e,
                    agents: _c.selected[e.id] ?? const <String>{},
                    already: widget.installedNames.contains(e.name),
                    checked: _c.checked.contains(e.id),
                    busyId: _c.busyId,
                    writeReady: widget.writeReady,
                    onToggleAgent: (a) => _c.toggleAgent(e.id, a),
                    onCheck: () => _c.toggleChecked(e.id),
                    onInstall: () => _c.install(e),
                  ),
                ),
            ],
          ),
      ],
    );
  }
}

class _CatalogRow extends StatelessWidget {
  const _CatalogRow({
    required this.entry,
    required this.agents,
    required this.already,
    required this.checked,
    required this.busyId,
    required this.writeReady,
    required this.onToggleAgent,
    required this.onCheck,
    required this.onInstall,
  });

  final CatalogEntry entry;
  final Set<String> agents;
  final bool already;
  final bool checked;
  final String? busyId;
  final bool writeReady;
  final void Function(String agent) onToggleAgent;
  final VoidCallback onCheck;
  final VoidCallback onInstall;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final installing = busyId == entry.id;
    final otherBusy = busyId != null && !installing;
    // `SkillInstallView.tsx:362` 的 disabled 表达式，五个条件照抄。
    final disabled =
        installing || otherBusy || !writeReady || already || agents.isEmpty;
    return Tile(
      // React 结果卡 12/16（`SkillInstallView.tsx:367`）。
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // 15×15 勾选框 + 上距 3（`SkillInstallView.tsx:377`）。
              Padding(
                padding: const EdgeInsets.only(top: 3),
                child: TinyCheckbox(
                  value: checked,
                  onChanged: busyId != null ? null : onCheck,
                ),
              ),
              // 行头 gap 12（`SkillInstallView.tsx:369`）。
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          entry.name,
                          // 条目名 14 w600（`SkillInstallView.tsx:381`），
                          // 不是 body 档的 15 w400。
                          style: AidogType.body.copyWith(
                            fontSize: 14,
                            fontWeight: FontWeight.w600,
                            letterSpacing: 0,
                            color: theme.c.fg,
                          ),
                        ),
                        if (already) ...[
                          // 名字↔已装 gap 8（`SkillInstallView.tsx:380`）。
                          const SizedBox(width: 8),
                          // 「已装」是 accent-subtle 底的小胶囊、字 secondary
                          //（`SkillInstallView.tsx:383-393`），不是裸文字。
                          MiniBadge(
                            text: t.t('skills.install.installed'),
                            color: theme.c.fg2,
                            background: theme.c.accentWash,
                            borderColor: Colors.transparent,
                            fontSize: 11,
                            padX: 6,
                            radius: 4,
                          ),
                        ],
                      ],
                    ),
                    // 名字块内间距 2（`SkillInstallView.tsx:379`）。
                    const SizedBox(height: 2),
                    Text(
                      entry.id,
                      // 条目 id 走**等宽** 11 secondary（`SkillInstallView.tsx:396-404`）。
                      style: AidogType.numSm.copyWith(
                        fontSize: 11,
                        color: theme.c.fg2,
                      ),
                    ),
                    if ((entry.description ?? '').isNotEmpty) ...[
                      const SizedBox(height: 2),
                      Text(
                        entry.description!,
                        // 描述 12 secondary（`SkillInstallView.tsx:407`）。
                        style: AidogType.caption.copyWith(
                          fontSize: 12,
                          color: theme.c.fg2,
                        ),
                      ),
                    ],
                    if ((entry.repoUrl ?? '').isNotEmpty) ...[
                      const SizedBox(height: 2),
                      InkWell(
                        onTap: () => native.openUrl(entry.repoUrl!),
                        child: Text(
                          entry.repoUrl!,
                          style: AidogType.micro.copyWith(
                            letterSpacing: 0,
                            color: theme.c.accentText,
                          ),
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 12),
              SmallButton(
                label: installing
                    ? t.t('skills.install.installing')
                    : already
                    ? t.t('skills.install.installed')
                    : t.t('skills.install.install'),
                filled: true,
                // 安装按钮 12（`SkillInstallView.tsx:424`）。
                fontSize: 12,
                padding: (16, 8),
                // 只在「别的安装还没完」时给解释，与 React 的 `title=` 同条件
                //（`SkillInstallView.tsx:426-434`）。
                tooltip: otherBusy ? t.t('skills.install.busyOther') : null,
                onTap: disabled ? null : onInstall,
              ),
            ],
          ),
          // 卡内竖向间距 8（`SkillInstallView.tsx:367`）。
          const SizedBox(height: 8),
          Wrap(
            // 「安装到」这一行 gap 6（`SkillInstallView.tsx:443`）。
            spacing: 6,
            runSpacing: 6,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              FieldLabel(t.t('skills.install.selectAgent'), fontSize: 11),
              // agent 候选带 16px 图标（`SkillInstallView.tsx:450-473`）：
              // 4/8 内衬、gap 4、11 字、未选 0.4 —— 原先是纯文字按钮。
              for (final a in kSkillAgents)
                AgentIconButton(
                  agent: a,
                  enabled: agents.contains(a),
                  supported: true,
                  tooltip: t.t('skills.agent.$a'),
                  label: t.t('skills.agent.$a'),
                  iconSize: 16,
                  gap: 4,
                  fontSize: 11,
                  padding: (8, 4),
                  lockHeight: false,
                  offOpacity: 0.4,
                  onTap: () => onToggleAgent(a),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

// ── 只读详情 ──────────────────────────────────────────────────────

class SkillDetailView extends StatefulWidget {
  const SkillDetailView({
    super.key,
    this.invoke = kernelInvoke,
    required this.skill,
    required this.onClose,
  });

  final InvokeFn invoke;
  final SkillInfo skill;
  final VoidCallback onClose;

  @override
  State<SkillDetailView> createState() => _SkillDetailViewState();
}

class _SkillDetailViewState extends State<SkillDetailView> {
  late final SkillDetailController _c;

  bool _built = false;

  /// 同 [SkillsPage]：控制器的 t 必须来自 context，不能是全局单例。
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_built) return;
    _built = true;
    _c = SkillDetailController(
      invoke: widget.invoke,
      t: AidogI18n.of(context).t,
      onChanged: () {
        if (mounted) setState(() {});
      },
      skill: widget.skill,
    );
    _c.init();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final sel = _c.selectedFile;
    final content = _c.content;
    final path = widget.skill.installedPath;
    // 弹窗高度 `min(88vh, 760)`（`SkillDetailView.tsx:110`）；两栏各自滚动，
    // 头部与栏边框固定。原先整块没高度约束、也没有滚动。
    final h = MediaQuery.sizeOf(context).height * 0.88;
    return SizedBox(
      height: h < 760 ? h : 760,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // 头部：12/18 内衬 + 一条下边框（`SkillDetailView.tsx:120-128`）。
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 12),
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: theme.c.line)),
            ),
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      // 名字与 chips 同一行，gap 8（`SkillDetailView.tsx:131-159`）。
                      Wrap(
                        spacing: 8,
                        runSpacing: 4,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          Text(
                            widget.skill.name,
                            style: AidogType.title.copyWith(
                              fontSize: 15,
                              fontWeight: FontWeight.w700,
                              color: theme.c.fg,
                            ),
                          ),
                          // 来源 chip：11 / 1px 6px / r4 / accent-subtle 底 /
                          // 字 **secondary**（`SkillDetailView.tsx:134-144`）。
                          if ((widget.skill.source ?? '').isNotEmpty)
                            MiniBadge(
                              text: widget.skill.source!,
                              color: theme.c.fg2,
                              background: theme.c.accentWash,
                              borderColor: Colors.transparent,
                              fontSize: 11,
                              padX: 6,
                              radius: 4,
                            ),
                          // 已启用 agent chip：10 / 1px 5px / r3 / **无底色** +
                          // 1px 中性边（`SkillDetailView.tsx:146-159`）。
                          for (final a in widget.skill.enabledAgents)
                            MiniBadge(
                              text: a,
                              color: theme.c.fg2,
                              background: Colors.transparent,
                              borderColor: theme.c.line,
                              padX: 5,
                              radius: 3,
                            ),
                        ],
                      ),
                      // 路径 10 等宽 secondary（`SkillDetailView.tsx:161-172`）。
                      if (path != null)
                        Padding(
                          padding: const EdgeInsets.only(top: 2),
                          child: Ltr(
                            child: Text(
                              path,
                              style: AidogType.numSm.copyWith(
                                fontSize: 10,
                                color: theme.c.fg2,
                              ),
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
                // 关闭是 ghost 的 `✕` 图标按钮（`SkillDetailView.tsx:174-182`），
                // 不是「关闭」文字按钮。
                SmallButton(
                  label: '✕',
                  ghost: true,
                  fontSize: 12,
                  padding: (16, 8),
                  tooltip: t.t('action.close'),
                  onTap: widget.onClose,
                ),
              ],
            ),
          ),
          Expanded(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                // 左树：220 宽 + 只有右边框 + 竖向 8（`SkillDetailView.tsx:188-195`），
                // 不是一整张带四边框和阴影的卡。
                Container(
                  width: 220,
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  decoration: BoxDecoration(
                    border: Border(
                      right: BorderSide(color: theme.c.line),
                    ),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      // 树标题 10 w600 全大写 + `0 14px 6px` 内衬
                      //（`SkillDetailView.tsx:197-207`）。
                      Padding(
                        padding: const EdgeInsets.fromLTRB(14, 0, 14, 6),
                        child: Text(
                          '${t.t('skills.detail.files')} (${_c.files.length})'
                              .toUpperCase(),
                          style: AidogType.micro.copyWith(
                            fontSize: 10,
                            letterSpacing: 0,
                            fontWeight: FontWeight.w600,
                            color: theme.c.fg2,
                          ),
                        ),
                      ),
                      Expanded(
                        child: SingleChildScrollView(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              if (_c.loadingList)
                                _treeNote(theme, t.t('status.loading'))
                              else if (_c.files.isEmpty)
                                _treeNote(theme, t.t('skills.detail.empty')),
                              for (final f in _c.files)
                                _FileRow(
                                  label: '${f.isText ? '' : '📄 '}${f.relPath}',
                                  selected: _c.selected == f.relPath,
                                  onTap: () => _c.loadFile(f.relPath),
                                ),
                            ],
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                // 右内容：16/20 内衬 + 自己滚，两栏之间**无间距**（靠边框分隔，
                // `SkillDetailView.tsx:186,230`）。
                Expanded(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 20,
                      vertical: 16,
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        if (_c.error != null)
                          // 读取失败是页内条 10/14 + 13 danger
                          //（`SkillDetailView.tsx:231-238`），不是浮动 toast。
                          InlineNote(
                            text:
                                '${t.t('skills.detail.readFailed')}: ${_c.error}',
                            padding: const EdgeInsets.symmetric(
                              horizontal: 14,
                              vertical: 10,
                            ),
                            color: theme.c.bad,
                          )
                        else if (_c.loadingFile)
                          Text(
                            t.t('status.loading'),
                            style: AidogType.label.copyWith(
                              fontSize: 13,
                              color: theme.c.fg2,
                            ),
                          )
                        else if (content == null && _c.selected == null)
                          Text(
                            t.t('skills.detail.noSkillMd'),
                            style: AidogType.label.copyWith(
                              fontSize: 13,
                              color: theme.c.fg2,
                            ),
                          )
                        else if (content != null) ...[
                          // 文件元信息是**一行等宽 11**、两段隔 12、下距 10
                          //（`SkillDetailView.tsx:244-256`），不是卡片标题行。
                          if (sel != null)
                            Padding(
                              padding: const EdgeInsets.only(bottom: 10),
                              child: Row(
                                children: [
                                  Text(
                                    sel.relPath,
                                    style: AidogType.numSm.copyWith(
                                      fontSize: 11,
                                      color: theme.c.fg2,
                                    ),
                                  ),
                                  const SizedBox(width: 12),
                                  Ltr(
                                    child: Text(
                                      formatSkillFileSize(sel.size),
                                      style: AidogType.numSm.copyWith(
                                        fontSize: 11,
                                        color: theme.c.fg2,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          // content == null（字段本身为 null）= 二进制，不预览。
                          if (content.content == null)
                            Text(
                              t.t('skills.detail.binary'),
                              // 13 secondary **斜体**（`SkillDetailView.tsx:258-268`）。
                              style: AidogType.label.copyWith(
                                fontSize: 13,
                                fontStyle: FontStyle.italic,
                                color: theme.c.fg2,
                              ),
                            )
                          else ...[
                            if (content.truncated)
                              Padding(
                                padding: const EdgeInsets.only(bottom: 8),
                                child: Text(
                                  t.t('skills.detail.truncated'),
                                  style: AidogType.micro.copyWith(
                                    letterSpacing: 0,
                                    color: theme.c.fg2,
                                  ),
                                ),
                              ),
                            // `*.md` 渲染成 Markdown，其余按原文等宽
                            //（`SkillDetailView.tsx:281-291` 同一条判据 `/\.md$/i`）。
                            // SKILL.md 几乎全是标题 + 列表 + 代码块，不渲染就是一堵
                            // 带 `#` 和 `-` 的字墙。
                            if (isMarkdownPath(_c.selected ?? ''))
                              MarkdownBody(
                                data: content.content!,
                                selectable: true,
                                onTapLink: (_, href, _) {
                                  if (href != null) native.openUrl(href);
                                },
                                styleSheet: markdownStyle(theme),
                              )
                            else
                              SelectableText(
                                content.content!,
                                // `<pre>` 是 12 **等宽**（`SkillDetailView.tsx:288-299`）。
                                style: AidogType.numSm.copyWith(
                                  fontSize: 12,
                                  color: theme.c.fg,
                                ),
                              ),
                          ],
                        ],
                      ],
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  /// 左树里的「加载中 / 空」一句话（`SkillDetailView.tsx:208-217`：8/14 + 12）。
  Widget _treeNote(AidogTheme theme, String text) => Padding(
    padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
    child: Text(
      text,
      style: AidogType.label.copyWith(fontSize: 12, color: theme.c.fg2),
    ),
  );
}

/// 左树的一行文件（`SkillDetailView.tsx:333-349`）：`5px 14px` 内衬、等宽 12、
/// 单行省略；选中时 accent-subtle 底 + 左缘 2px accent 竖条 + accent 字 ——
/// 原先只换了字色，选中行在一列文件里几乎看不出来。
class _FileRow extends StatelessWidget {
  const _FileRow({
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
      child: Container(
        decoration: BoxDecoration(
          color: selected ? theme.c.accentWash : Colors.transparent,
          border: BorderDirectional(
            start: BorderSide(
              color: selected ? theme.c.accent : Colors.transparent,
              width: 2,
            ),
          ),
        ),
        padding: const EdgeInsetsDirectional.fromSTEB(12, 5, 14, 5),
        child: Text(
          label,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: AidogType.numSm.copyWith(
            fontSize: 12,
            color: selected ? theme.c.accentText : theme.c.fg,
          ),
        ),
      ),
    );
  }
}

/// SKILL.md 的 Markdown 皮肤。React 那边是 `.markdown-body`（13px / 1.6，
/// `SkillDetailView.tsx:282`），字号行高照抄，颜色接主题 token。
MarkdownStyleSheet markdownStyle(AidogTheme theme) {
  // 正文 13 / 1.6（`SkillDetailView.tsx:282`），不是 label 档的 13.5。
  final body = AidogType.label.copyWith(
    fontSize: 13,
    color: theme.c.fg,
    height: 1.6,
  );
  TextStyle heading(double size) => AidogType.label.copyWith(
    color: theme.c.fg,
    fontSize: size,
    fontWeight: FontWeight.w600,
    height: 1.4,
  );
  return MarkdownStyleSheet(
    p: body,
    listBullet: body,
    tableBody: body,
    tableHead: body.copyWith(fontWeight: FontWeight.w600),
    a: body.copyWith(color: theme.c.accentText),
    h1: heading(19),
    h2: heading(17),
    h3: heading(15),
    h4: heading(14),
    h5: heading(13.5),
    h6: heading(13.5),
    code: AidogType.numSm.copyWith(color: theme.c.fg),
    codeblockPadding: const EdgeInsets.all(AidogSpace.ssm),
    codeblockDecoration: BoxDecoration(
      color: theme.c.surface2,
      border: Border.all(color: theme.c.line),
      borderRadius: BorderRadius.circular(AidogRadius.sm),
    ),
    blockquote: body.copyWith(color: theme.c.fg2),
    blockquotePadding: const EdgeInsets.only(left: AidogSpace.ssm),
    blockquoteDecoration: BoxDecoration(
      border: Border(left: BorderSide(color: theme.c.line, width: 3)),
    ),
    tableBorder: TableBorder.all(color: theme.c.line),
    horizontalRuleDecoration: BoxDecoration(
      border: Border(top: BorderSide(color: theme.c.line)),
    ),
  );
}

/// 长耗时操作期间的全页遮罩（`SkillsView.tsx:34-53`）：转圈 + 一句在做什么。
/// 盖住整页是故意的 —— 批量卸载跑到一半再点别的按钮，结果不可预期。
class _BusyOverlay extends StatelessWidget {
  const _BusyOverlay({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return AidogModal(
      child: ModalCard(
        // React 这张卡是 `24px 32px` 内衬 / 14 字 / gap 12
        //（`SkillsView.tsx:47-50`）。
        padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 24),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox(
              width: 16,
              height: 16,
              child: CircularProgressIndicator(
                strokeWidth: 2,
                color: theme.c.accentText,
              ),
            ),
            const SizedBox(width: 12),
            Flexible(
              child: Text(
                text,
                style: AidogType.label.copyWith(
                  fontSize: 14,
                  color: theme.c.fg2,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
