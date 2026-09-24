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
          title: t.t('skills.title'),
          trailing: Wrap(
            spacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              // 这一排按钮的禁用条件照 `SkillsView.tsx:66-132`：
              // 没装 node（`!writeReady`）/ 项目路径没填（`scopeInvalid`）/
              // 正在跑别的活（`busyKey != null`），三者任一成立就点不动。
              // 原先只有「全部更新」判了，其余四颗无条件可点 —— 点下去必然失败。
              // 刷新按钮（`SkillsView.tsx:88-94`）。原先整个页头没有它，
              // 装完 / 改完只能切页再切回来才看得到新状态。
              SmallButton(
                label: _c.refreshing
                    ? t.t('skills.refreshing')
                    : t.t('skills.refresh'),
                onTap: _c.refreshing ? null : _c.refreshInstalled,
              ),
              SmallButton(
                // React 这颗没写 variant = 默认实心（`SkillsView.tsx:66-73`）。
                label: t.t('skills.install.addBtn'),
                filled: true,
                onTap: _ready ? () => _c.setSubView('install') : null,
              ),
              // 这几颗按钮跑起来要几秒（都在写外部配置文件），忙碌时换文案，
              // 否则点下去界面一动不动，用户只会再点一次
              //（`SkillsView.tsx:102/111/121/131` 逐颗照抄）。
              SmallButton(
                label: _c.busyKey == '__update__'
                    ? t.t('skills.updating')
                    : t.t('skills.updateAll'),
                onTap: _ready ? _c.updateAll : null,
              ),
              SmallButton(
                label: _c.busyKey == '__align__'
                    ? t.t('skills.aligning')
                    : t.t('skills.alignTitle'),
                onTap: _ready ? _c.openAlign : null,
              ),
              SmallButton(
                label: t.t('skills.importFromShare'),
                onTap: _ready ? () => _c.setPasteOpen(true) : null,
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
                onTap: (_ready && _c.selectedNames.isNotEmpty)
                    ? _c.askUninstallBatch
                    : null,
              ),
              SmallButton(
                label: _c.busyKey == '__uninstall__'
                    ? t.t('skills.uninstalling')
                    : t.t('skills.uninstallAll'),
                danger: true,
                filled: true,
                onTap: _ready ? _c.askUninstallAll : null,
              ),
            ],
          ),
        ),
        if (_c.env != null && !_c.writeReady)
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
            child: ToastBar(text: t.t('skills.envMissing'), ok: false),
          ),
        _statsBar(t),
        const SizedBox(height: AidogSpace.ssm),
        _filterBar(t),
        const SizedBox(height: AidogSpace.ssm),
        if (_c.scopeInvalid)
          CenteredNote(text: t.t('skills.chooseProjectDir'))
        else if (_c.installedLoading)
          CenteredNote(text: t.t('status.loading'))
        // 两种空态分开（`SkillsView.tsx:315-322`）：一个都没装 vs 搜不到。
        // 原先都显「暂无已安装」—— 搜了个不存在的词，用户会以为技能全没了。
        else if (_c.installed.isEmpty)
          CenteredNote(text: t.t('skills.installedEmpty'))
        else if (_c.filteredInstalled.isEmpty)
          CenteredNote(text: t.t('skills.searchEmpty'))
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
        if (_c.refreshing)
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.ssm),
            child: TileMeta(t.t('status.loading')),
          ),
        if (_c.confirmUninstall)
          ConfirmCard(
            title: t.t('skills.uninstallAll'),
            body: t.t('skills.uninstallAllConfirm', {'count': _c.total}),
            confirmLabel: t.t('action.confirm'),
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
            maxWidth: 720,
            onBarrierTap: () => _c.setDetailTarget(null),
            child: SkillDetailView(
              invoke: widget.invoke,
              skill: _c.detailTarget!,
              onClose: () => _c.setDetailTarget(null),
            ),
          ),
        // 批量卸载期间的全页遮罩（React `SkillsView.tsx:34-53` 只盖
        // `__uninstall_batch__`）：单行 / 全部卸载不盖全页遮罩，只禁按钮 ——
        // 照 React 真值，原先 `__uninstall__`（卸载全部）也盖了。
        if (_c.busyKey == '__uninstall_batch__')
          _BusyOverlay(text: t.t('skills.uninstalling')),
        if (_c.message != null)
          ToastBar(text: _c.message!, ok: true, onDismiss: _c.clearMessage),
      ],
    );
  }

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
        spacing: AidogSpace.slg,
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
                style: AidogType.caption.copyWith(color: theme.c.fg2),
              ),
            ],
          ),
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
                const SizedBox(width: AidogSpace.ssm),
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
                      style: AidogType.caption.copyWith(color: theme.c.fg2),
                    ),
                  ],
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: _c.busyKey == '__enableall_${a}__'
                      ? t.t('skills.enabling')
                      : t.t('skills.enableAll'),
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
              const SizedBox(width: AidogSpace.ssm),
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
                    style: AidogType.caption.copyWith(color: theme.c.fg2),
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
                spacing: AidogSpace.ssm,
                runSpacing: AidogSpace.sxs,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  FieldLabel(t.t('skills.scope')),
                  // React 是 `Select` 下拉（`SkillsView.tsx:262-270`）。
                  MiniSelect(
                    key: const ValueKey('skills-scope'),
                    value: _c.scopeKind,
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
                  padding: const EdgeInsets.only(top: AidogSpace.ssm),
                  child: Wrap(
                    spacing: AidogSpace.ssm,
                    runSpacing: AidogSpace.sxs,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      SizedBox(
                        width: 280,
                        child: KeptTextField(
                          key: const Key('skills-project-path'),
                          value: _c.projectPath,
                          hint: t.t('skills.chooseProjectDir'),
                          onSubmitted: _c.setProjectPath,
                        ),
                      ),
                      SmallButton(
                        label: t.t('skills.chooseProjectDir'),
                        onTap: _pickProjectDir,
                      ),
                    ],
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _filterBar(I18nController t) => Tile(
    child: Wrap(
      spacing: AidogSpace.ssm,
      runSpacing: AidogSpace.sxs,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        SizedBox(
          width: 220,
          child: TextField(
            key: const Key('skills-search'),
            decoration: InputDecoration(
              isDense: true,
              hintText: t.t('skills.searchPlaceholder'),
            ),
            style: AidogType.label.copyWith(color: AidogTheme.of(context).c.fg),
            onChanged: _c.setSearchQuery,
          ),
        ),
        // 三个筛选 key 直接写死，不拼 —— 拼出来的 key `check-ui-parity.mjs`
        // 这类扫字面量的工具照不出来（单引号串里 `\$` 还是字面美元符，
        // 原先平铺按钮版就在这里翻过车）。
        MiniSelect(
          key: const ValueKey('skills-enabled-filter'),
          value: _c.enabledFilter,
          options: const ['all', 'enabled', 'disabled'],
          labelOf: (f) => switch (f) {
            'enabled' => t.t('skills.filterEnabled'),
            'disabled' => t.t('skills.filterDisabled'),
            _ => t.t('skills.filterAll'),
          },
          onChanged: (v) => _c.setEnabledFilter(v!),
        ),
      ],
    ),
  );

  // React 是普通 `Dialog`（`SkillModals.tsx:135`，maxWidth 400），点遮罩可关。
  Widget _alignCard(I18nController t) => AidogModal(
    maxWidth: 400,
    onBarrierTap: _c.closeAlign,
    child: ModalCard(
      title: t.t('skills.alignTitle'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            t.t('skills.alignConfirm', {
              'from': t.t('skills.agent.${_c.alignFrom}'),
              'to': t.t('skills.agent.${_c.alignTo}'),
            }),
            style: AidogType.micro.copyWith(
              color: AidogTheme.of(context).c.fg2,
            ),
          ),
          const SizedBox(height: AidogSpace.ssm),
          Wrap(
            spacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              FieldLabel(t.t('skills.alignFrom')),
              MiniSelect(
                key: const ValueKey('skills-align-from'),
                value: _c.alignFrom,
                options: kSkillAgents,
                labelOf: (a) => t.t('skills.agent.$a'),
                onChanged: (v) => _c.setAlignFrom(v!),
              ),
              FieldLabel(t.t('skills.alignTo')),
              MiniSelect(
                key: const ValueKey('skills-align-to'),
                value: _c.alignTo,
                options: kSkillAgents,
                labelOf: (a) => t.t('skills.agent.$a'),
                onChanged: (v) => _c.setAlignTo(v!),
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.ssm),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SmallButton(label: t.t('action.cancel'), onTap: _c.closeAlign),
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(
                label: t.t('action.confirm'),
                // 同一个 agent 对齐自己是空操作，按钮直接禁掉。
                // 光禁不说原因等于把解释丢了，提示语照抄 React 的 `title=`。
                tooltip: _c.alignFrom == _c.alignTo
                    ? t.t('skills.alignSameAgent')
                    : null,
                onTap: _c.alignFrom == _c.alignTo ? null : _c.align,
              ),
            ],
          ),
        ],
      ),
    ),
  );

  // React 是普通 `Dialog`（`SkillModals.tsx:214`，maxWidth 560），点遮罩可关。
  Widget _pasteCard(I18nController t) => AidogModal(
    maxWidth: 560,
    onBarrierTap: () => _c.setPasteOpen(false),
    child: ModalCard(
      title: t.t('skills.importFromShare'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            key: const Key('skills-paste'),
            maxLines: 4,
            decoration: InputDecoration(
              isDense: true,
              hintText: t.t('skills.pasteHint'),
            ),
            style: AidogType.label.copyWith(color: AidogTheme.of(context).c.fg),
            onChanged: _c.setPasteText,
          ),
          const SizedBox(height: AidogSpace.ssm),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SmallButton(
                label: t.t('action.cancel'),
                onTap: () => _c.setPasteOpen(false),
              ),
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(label: t.t('action.confirm'), onTap: _c.pasteImport),
            ],
          ),
        ],
      ),
    ),
  );

  // React 是普通 `Dialog`（`SkillModals.tsx:248`，maxWidth 560）：装载中不许关。
  Widget _importCard(I18nController t) => AidogModal(
    maxWidth: 560,
    onBarrierTap: _c.importBusy ? null : _c.cancelImport,
    child: ModalCard(
      title: t.t('skills.importConfirmTitle'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 原先只列 id，不说这一步会跑 `npx skills add`、要联网
          //（`SkillModals.tsx:258`）。装不上的时候用户根本不知道该查网络。
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
            child: TileMeta(
              t.t('skills.importConfirmDesc', {'count': _c.importIds!.length}),
            ),
          ),
          for (final id in _c.importIds!)
            Text(
              id,
              style: AidogType.micro.copyWith(
                color: AidogTheme.of(context).c.fg2,
              ),
            ),
          const SizedBox(height: AidogSpace.ssm),
          Wrap(
            spacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              // 这一排按钮混着「目标 agent」和「范围」两件事，没有标签分不出来
              //（React 各自有一行标签，`SkillModals.tsx:268,295`）。
              FieldLabel(t.t('skills.importAgents')),
              for (final a in kSkillAgents)
                SmallButton(
                  label: t.t('skills.agent.$a'),
                  active: _c.importAgents.contains(a),
                  onTap: () => _c.toggleImportAgent(a),
                ),
              FieldLabel(t.t('skills.scope')),
              MiniSelect(
                key: const ValueKey('skills-import-scope'),
                value: _c.importScopeKind,
                options: const ['global', 'project'],
                labelOf: (v) => v == 'global'
                    ? t.t('skills.scopeGlobal')
                    : t.t('skills.scopeProject'),
                onChanged: (v) => _c.setImportScopeKind(v!),
              ),
            ],
          ),
          if (_c.importScopeKind == 'project') ...[
            const SizedBox(height: AidogSpace.ssm),
            TextField(
              key: const Key('skills-import-path'),
              decoration: InputDecoration(
                isDense: true,
                hintText: t.t('skills.chooseProjectDir'),
              ),
              style: AidogType.label.copyWith(
                color: AidogTheme.of(context).c.fg,
              ),
              onChanged: _c.setImportProjectPath,
            ),
          ],
          const SizedBox(height: AidogSpace.ssm),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SmallButton(
                label: t.t('action.cancel'),
                onTap: _c.importBusy ? null : _c.cancelImport,
              ),
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(
                label: _c.importBusy
                    ? t.t('status.loading')
                    : t.t('action.confirm'),
                // 一个 agent 都没选就装不了；项目 scope 要有路径。
                onTap:
                    _c.importBusy ||
                        _c.importAgents.isEmpty ||
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
          Checkbox(value: checked, onChanged: busy ? null : (_) => onCheck()),
          Expanded(
            flex: 3,
            child: Tooltip(
              // 点名字开详情这件事本身看不出来，React 把它写在 `title=` 上
              //（`SkillsView.tsx:400`）。
              message: t.t('skills.detail.view'),
              child: InkWell(
                onTap: onOpen,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      skill.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.body.copyWith(
                        fontSize: 13,
                        color: theme.c.fg,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    if ((skill.description ?? '').isNotEmpty)
                      Text(
                        skill.description!,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.caption.copyWith(color: theme.c.fg3),
                      ),
                    // 元信息行（`SkillsView.tsx:432-490`）：来源类型 / plugin 来源 /
                    // 更新时间 / 安装于 · 内容 hash 前 7 位。
                    // 原先这一整块在 Flutter 侧没有 —— 装了什么、从哪来、什么时候装的，
                    // 一条都看不到。
                    _SkillMeta(skill: skill),
                  ],
                ),
              ),
            ),
          ),
          // 来源：可点链接（React `SkillsView.tsx:406-431`，`source_url` 缺省
          // 拼 `https://github.com/<source>`）。原先纯文本点不动。
          Expanded(
            flex: 2,
            child: (skill.source ?? '').isEmpty
                ? const SizedBox.shrink()
                : Tooltip(
                    message: skill.sourceUrl ?? skill.source!,
                    child: InkWell(
                      onTap: () => native.openUrl(
                        skill.sourceUrl ?? 'https://github.com/${skill.source}',
                      ),
                      child: Text(
                        skill.source!,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.micro.copyWith(
                          color: theme.c.accentText,
                        ),
                      ),
                    ),
                  ),
          ),
          // 操作区要能换行：agent 开关的文案带上状态之后变长了，窄窗下一行放不下，
          // 裸 `Wrap` 在 `Row` 里拿到的是无限宽约束，永远不换行 —— 必须给它
          // 一个有界宽度（`Flexible`），`Wrap` 才会真的折行。
          Flexible(
            flex: 4,
            child: Wrap(
              alignment: WrapAlignment.end,
              spacing: AidogSpace.sxs,
              runSpacing: AidogSpace.sxs,
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
                    onTap: busy || !writeReady ? null : () => onToggleAgent(a),
                  ),
                // pi 是静态徽标不是开关：pi 原生扫公共 skill 目录，没有 per-skill
                // 启停概念，做成可点开关就是在骗用户（`SkillsView.tsx:536-537` 原注释）。
                MiniBadge(
                  text:
                      '${t.t('skills.agent.pi')} · ${t.t('skills.piAlwaysOn')}',
                  color: theme.c.fg3,
                  tooltip: t.t('skills.piAlwaysOnHint'),
                ),
                // 分享按钮只在 catalog 来源（有 source）时出现
                //（React `SkillsView.tsx:554-557` 的 `skillCatalogId(skill) &&`：
                // 手动 symlink 的 skill 无 source，点不出任何东西，直接隐藏）。
                if (skillCatalogId(skill) != null)
                  SmallButton(label: t.t('skills.share.title'), onTap: onShare),
                SmallButton(
                  // 单条卸载：React `variant="destructive"`（`SkillsView.tsx:567`），
                  // busy 期间换「卸载中…」并禁用（`SkillsView.tsx:575-577`）。
                  label: busyKey == '__uninstall_single_${skill.name}__'
                      ? t.t('skills.uninstalling')
                      : t.t('skills.uninstall'),
                  danger: true,
                  filled: true,
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

/// 技能行的元信息（`SkillsView.tsx:432-490`）：来源类型 / plugin 来源 /
/// 更新时间 / 安装于 · 内容 hash 前 7 位。字段为空的不渲染。
class _SkillMeta extends StatelessWidget {
  const _SkillMeta({required this.skill});

  final SkillInfo skill;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final chips = <Widget>[
      if ((skill.sourceType ?? '').isNotEmpty)
        MiniBadge(
          text: skill.sourceType!.toUpperCase(),
          color: theme.c.fg3,
          tooltip: t.t('skills.sourceType'),
        ),
      if ((skill.pluginName ?? '').isNotEmpty)
        MiniBadge(
          text: 'plugin: ${skill.pluginName}',
          color: theme.c.fg3,
          tooltip: t.t('skills.pluginName'),
        ),
    ];
    // 安装时间那行：「安装于 <时间> · <hash 前 7 位>」。
    final installedAt = skill.installedAt ?? '';
    final hash = skill.skillFolderHash ?? '';
    final second = <String>[
      if (installedAt.isNotEmpty) '${t.t('skills.installedAt')}: $installedAt',
      if (hash.isNotEmpty) hash.substring(0, hash.length < 7 ? hash.length : 7),
    ].join(' · ');
    final updatedAt = skill.updatedAt ?? '';
    if (chips.isEmpty && second.isEmpty && updatedAt.isEmpty) {
      return const SizedBox.shrink();
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (chips.isNotEmpty || updatedAt.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Wrap(
              spacing: AidogSpace.sxs,
              runSpacing: 2,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                ...chips,
                if (updatedAt.isNotEmpty)
                  Tooltip(
                    message: '${t.t('skills.updatedAt')}: $updatedAt',
                    child: Text(
                      updatedAt,
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                  ),
              ],
            ),
          ),
        if (second.isNotEmpty)
          Tooltip(
            message: hash.isEmpty ? '' : t.t('skills.hash'),
            child: Text(
              second,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
      ],
    );
  }
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
          title: t.t('skills.install.title'),
          subtitle: _c.loading ? t.t('skills.install.searching') : null,
          trailing: Wrap(
            spacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              SmallButton(
                label: t.t('skills.install.back'),
                onTap: _c.busyId != null ? null : widget.onBack,
              ),
              SmallButton(
                label: t.t('skills.install.installSelected', {
                  'count': _c.checked.length,
                }),
                onTap:
                    _c.busyId != null ||
                        !widget.writeReady ||
                        _c.checked.isEmpty
                    ? null
                    : _c.installBatch,
              ),
            ],
          ),
        ),
        TextField(
          key: const Key('skills-install-search'),
          autofocus: true,
          decoration: InputDecoration(
            isDense: true,
            hintText: t.t('skills.install.searchPlaceholder'),
          ),
          style: AidogType.label.copyWith(color: AidogTheme.of(context).c.fg),
          onChanged: _c.setKeyword,
        ),
        const SizedBox(height: AidogSpace.ssm),
        if (_c.message != null)
          ToastBar(text: _c.message!, ok: true, onDismiss: _c.clearMessage),
        if (_c.error != null)
          ToastBar(
            text: '${t.t('skills.install.loadFailed')}: ${_c.error}',
            ok: false,
          ),
        if (!_c.loading && _c.error == null && !_c.hasKeyword)
          CenteredNote(text: t.t('skills.install.emptyHint'))
        else if (!_c.loading && _c.error == null && _c.results.isEmpty)
          CenteredNote(text: t.t('skills.install.noResults'))
        else if (!_c.loading)
          Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final e in _c.results)
                Padding(
                  padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
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
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Checkbox(
                value: checked,
                onChanged: busyId != null ? null : (_) => onCheck(),
              ),
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
                          style: AidogType.body.copyWith(color: theme.c.fg),
                        ),
                        if (already) ...[
                          const SizedBox(width: AidogSpace.sxs),
                          Text(
                            t.t('skills.install.installed'),
                            style: AidogType.micro.copyWith(
                              color: theme.c.accentText,
                            ),
                          ),
                        ],
                      ],
                    ),
                    Text(
                      entry.id,
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                    if ((entry.description ?? '').isNotEmpty)
                      Text(
                        entry.description!,
                        style: AidogType.micro.copyWith(color: theme.c.fg2),
                      ),
                    if ((entry.repoUrl ?? '').isNotEmpty)
                      InkWell(
                        onTap: () => native.openUrl(entry.repoUrl!),
                        child: Text(
                          entry.repoUrl!,
                          style: AidogType.micro.copyWith(
                            color: theme.c.accentText,
                          ),
                        ),
                      ),
                  ],
                ),
              ),
              SmallButton(
                label: installing
                    ? t.t('skills.install.installing')
                    : already
                    ? t.t('skills.install.installed')
                    : t.t('skills.install.install'),
                // 只在「别的安装还没完」时给解释，与 React 的 `title=` 同条件
                //（`SkillInstallView.tsx:426-434`）。
                tooltip: otherBusy ? t.t('skills.install.busyOther') : null,
                onTap: disabled ? null : onInstall,
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              FieldLabel(t.t('skills.install.selectAgent')),
              for (final a in kSkillAgents)
                SmallButton(
                  label: t.t('skills.agent.$a'),
                  active: agents.contains(a),
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
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        PageHead(
          title: widget.skill.name,
          subtitle: widget.skill.installedPath == null
              ? null
              : ltr(widget.skill.installedPath!),
          trailing: SmallButton(
            label: t.t('action.close'),
            onTap: widget.onClose,
          ),
        ),
        // 头部 chips（React `SkillDetailView.tsx:131-159`）：来源 + 已启用
        // agent 徽标。原先只有标题/路径，装在哪个 agent 上一眼看不出来。
        if ((widget.skill.source ?? '').isNotEmpty ||
            widget.skill.enabledAgents.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
            child: Wrap(
              spacing: AidogSpace.sxs,
              runSpacing: AidogSpace.sxs,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                if ((widget.skill.source ?? '').isNotEmpty)
                  MiniBadge(
                    text: widget.skill.source!,
                    color: theme.c.accentText,
                  ),
                for (final a in widget.skill.enabledAgents)
                  MiniBadge(text: a, color: theme.c.fg2),
              ],
            ),
          ),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
              width: 240,
              child: Tile(
                title: '${t.t('skills.detail.files')} (${_c.files.length})',
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (_c.loadingList)
                      Text(
                        t.t('status.loading'),
                        style: AidogType.micro.copyWith(color: theme.c.fg3),
                      )
                    else if (_c.files.isEmpty)
                      Text(
                        t.t('skills.detail.empty'),
                        style: AidogType.micro.copyWith(color: theme.c.fg3),
                      ),
                    for (final f in _c.files)
                      InkWell(
                        onTap: () => _c.loadFile(f.relPath),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(vertical: 4),
                          child: Text(
                            '${f.isText ? '' : '📄 '}${f.relPath}',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: AidogType.micro.copyWith(
                              color: _c.selected == f.relPath
                                  ? theme.c.accentText
                                  : theme.c.fg2,
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
            const SizedBox(width: AidogSpace.ssm),
            Expanded(
              child: Tile(
                title: sel?.relPath,
                meta: sel == null ? null : ltr(formatSkillFileSize(sel.size)),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (_c.error != null)
                      ToastBar(
                        text: '${t.t('skills.detail.readFailed')}: ${_c.error}',
                        ok: false,
                      )
                    else if (_c.loadingFile)
                      Text(
                        t.t('status.loading'),
                        style: AidogType.micro.copyWith(color: theme.c.fg3),
                      )
                    else if (content == null && _c.selected == null)
                      Text(
                        t.t('skills.detail.noSkillMd'),
                        style: AidogType.micro.copyWith(color: theme.c.fg3),
                      )
                    else if (content != null) ...[
                      // content == null（字段本身为 null）= 二进制，不预览。
                      if (content.content == null)
                        Text(
                          t.t('skills.detail.binary'),
                          style: AidogType.micro.copyWith(color: theme.c.fg3),
                        )
                      else ...[
                        if (content.truncated)
                          Text(
                            t.t('skills.detail.truncated'),
                            style: AidogType.micro.copyWith(color: theme.c.fg3),
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
                            style: AidogType.micro.copyWith(color: theme.c.fg),
                          ),
                      ],
                    ],
                  ],
                ),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

/// SKILL.md 的 Markdown 皮肤。React 那边是 `.markdown-body`（13px / 1.6，
/// `SkillDetailView.tsx:282`），字号行高照抄，颜色接主题 token。
MarkdownStyleSheet markdownStyle(AidogTheme theme) {
  final body = AidogType.label.copyWith(color: theme.c.fg, height: 1.6);
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
            const SizedBox(width: AidogSpace.ssm),
            Flexible(
              child: Text(
                text,
                style: AidogType.label.copyWith(color: theme.c.fg2),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
