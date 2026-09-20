/// 技能页界面（票 I09），对应 `src/pages/Skills.tsx` 的三个子视图。
///
/// 结构与 React 同：一个顶层页面，内部 `subView` 在 `list` / `install` 之间切；
/// 详情与七个弹窗是**页面 state 的一部分**（不是 route），widget 测试
/// `find.byType(ConfirmCard)` / `find.text(...)` 就能断言 —— 沿用票 I07 的做法。
///
/// 色值一律 `AidogTheme.of(context).c.*`，本文件零硬编码颜色。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'invoke.dart';
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

  @override
  void initState() {
    super.initState();
    _c = SkillsController(
      invoke: widget.invoke,
      t: (k, [a]) => i18n.t(k, a),
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    _c.init();
    WidgetsBinding.instance.addObserver(this);
  }

  /// React 那边靠 `window focus` + `visibilitychange` 两个事件触发重查；
  /// Flutter 的等价物是应用生命周期回到 resumed。节流仍是控制器里的 10 秒。
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) _c.maybeRevalidate();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _c.dispose();
    super.dispose();
  }

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
    final detail = _c.detailTarget;
    if (detail != null) {
      return SkillDetailView(
        invoke: widget.invoke,
        skill: detail,
        onClose: () => _c.setDetailTarget(null),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        PageHead(
          title: t.t('skills.title'),
          subtitle: ltr(
            '${_c.total} · claude ${_c.agentCounts['claude']} · codex ${_c.agentCounts['codex']}',
          ),
          trailing: Wrap(
            spacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              SmallButton(
                label: t.t('skills.install.addBtn'),
                onTap: () => _c.setSubView('install'),
              ),
              SmallButton(
                label: t.t('skills.updateAll'),
                onTap: _c.busyKey == null && _c.writeReady && !_c.scopeInvalid
                    ? _c.updateAll
                    : null,
              ),
              SmallButton(label: t.t('skills.alignTitle'), onTap: _c.openAlign),
              SmallButton(
                label: t.t('skills.importFromShare'),
                onTap: () => _c.setPasteOpen(true),
              ),
              SmallButton(
                label: t.t('skills.uninstallSelected', {
                  'count': _c.selectedNames.length,
                }),
                danger: true,
                onTap: _c.selectedNames.isEmpty ? null : _c.askUninstallBatch,
              ),
              SmallButton(
                label: t.t('skills.uninstallAll'),
                danger: true,
                onTap: _c.askUninstallAll,
              ),
            ],
          ),
        ),
        if (_c.env != null && !_c.writeReady)
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
            child: ToastBar(text: t.t('skills.envMissing'), ok: false),
          ),
        _scopeBar(t),
        const SizedBox(height: AidogSpace.ssm),
        _filterBar(t),
        const SizedBox(height: AidogSpace.ssm),
        if (_c.scopeInvalid)
          CenteredNote(text: t.t('skills.chooseProjectDir'))
        else if (_c.installedLoading)
          CenteredNote(text: t.t('status.loading'))
        else if (_c.filteredInstalled.isEmpty)
          CenteredNote(text: t.t('skills.installedEmpty'))
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
                    busy: _c.busyKey != null,
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
        if (_c.message != null) ToastBar(text: _c.message!, ok: true),
      ],
    );
  }

  Widget _scopeBar(I18nController t) => Tile(
    child: Wrap(
      spacing: AidogSpace.ssm,
      runSpacing: AidogSpace.sxs,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        TileMeta(t.t('skills.scope')),
        SmallButton(
          label: t.t('skills.scopeGlobal'),
          active: _c.scopeKind == 'global',
          onTap: () => _c.setScopeKind('global'),
        ),
        SmallButton(
          label: t.t('skills.scopeProject'),
          active: _c.scopeKind == 'project',
          onTap: () => _c.setScopeKind('project'),
        ),
        if (_c.scopeKind == 'project') ...[
          SizedBox(
            width: 280,
            child: TextField(
              key: const Key('skills-project-path'),
              controller: TextEditingController(text: _c.projectPath)
                ..selection = TextSelection.collapsed(
                  offset: _c.projectPath.length,
                ),
              decoration: InputDecoration(
                isDense: true,
                hintText: t.t('skills.chooseProjectDir'),
              ),
              style: AidogType.micro.copyWith(
                color: AidogTheme.of(context).c.fg,
              ),
              onSubmitted: _c.setProjectPath,
            ),
          ),
          SmallButton(
            label: t.t('skills.chooseProjectDir'),
            onTap: _pickProjectDir,
          ),
        ],
        for (final a in kSkillAgents)
          SmallButton(
            label: '${t.t('skills.enableAll')} ${t.t('skills.agent.$a')}',
            onTap: _c.busyKey == null && _c.writeReady && !_c.scopeInvalid
                ? () => _c.enableAll(a)
                : null,
          ),
      ],
    ),
  );

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
            style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg),
            onChanged: _c.setSearchQuery,
          ),
        ),
        for (final f in const ['all', 'enabled', 'disabled'])
          SmallButton(
            label: t.t('skills.filter\${f[0].toUpperCase()}\${f.substring(1)}'),
            active: _c.enabledFilter == f,
            onTap: () => _c.setEnabledFilter(f),
          ),
      ],
    ),
  );

  Widget _alignCard(I18nController t) => Padding(
    padding: const EdgeInsets.only(top: AidogSpace.smd),
    child: Tile(
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
              TileMeta(t.t('skills.alignFrom')),
              for (final a in kSkillAgents)
                SmallButton(
                  label: t.t('skills.agent.$a'),
                  active: _c.alignFrom == a,
                  onTap: () => _c.setAlignFrom(a),
                ),
              TileMeta(t.t('skills.alignTo')),
              for (final a in kSkillAgents)
                SmallButton(
                  label: t.t('skills.agent.$a'),
                  active: _c.alignTo == a,
                  onTap: () => _c.setAlignTo(a),
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
                onTap: _c.alignFrom == _c.alignTo ? null : _c.align,
              ),
            ],
          ),
        ],
      ),
    ),
  );

  Widget _pasteCard(I18nController t) => Padding(
    padding: const EdgeInsets.only(top: AidogSpace.smd),
    child: Tile(
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
            style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg),
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

  Widget _importCard(I18nController t) => Padding(
    padding: const EdgeInsets.only(top: AidogSpace.smd),
    child: Tile(
      title: t.t('skills.importConfirmTitle'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
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
              for (final a in kSkillAgents)
                SmallButton(
                  label: t.t('skills.agent.$a'),
                  active: _c.importAgents.contains(a),
                  onTap: () => _c.toggleImportAgent(a),
                ),
              SmallButton(
                label: t.t('skills.scopeGlobal'),
                active: _c.importScopeKind == 'global',
                onTap: () => _c.setImportScopeKind('global'),
              ),
              SmallButton(
                label: t.t('skills.scopeProject'),
                active: _c.importScopeKind == 'project',
                onTap: () => _c.setImportScopeKind('project'),
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
              style: AidogType.micro.copyWith(
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

  Widget _shareCard(I18nController t) {
    final data = _c.shareData!;
    final text = data.skills.join('\n');
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: '${t.t('skills.share.title')} · ${data.name}',
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            SelectableText(
              text,
              style: AidogType.micro.copyWith(
                color: AidogTheme.of(context).c.fg2,
              ),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('skills.share.copyUrl'),
                  onTap: () => native.writeText(text),
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(label: t.t('action.close'), onTap: _c.closeShare),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// 已装列表的一行：勾选 + 名字（点开详情）+ 两个 agent 开关 + 分享 / 卸载。
class _SkillRow extends StatelessWidget {
  const _SkillRow({
    required this.skill,
    required this.checked,
    required this.busy,
    required this.writeReady,
    required this.onCheck,
    required this.onOpen,
    required this.onToggleAgent,
    required this.onShare,
    required this.onUninstall,
  });

  final SkillInfo skill;
  final bool checked;
  final bool busy;
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
    return Tile(
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Checkbox(value: checked, onChanged: busy ? null : (_) => onCheck()),
          Expanded(
            flex: 3,
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
                    style: AidogType.body.copyWith(color: theme.c.fg),
                  ),
                  if ((skill.description ?? '').isNotEmpty)
                    Text(
                      skill.description!,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                ],
              ),
            ),
          ),
          Expanded(
            flex: 2,
            child: Text(
              skill.source ?? '',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
          Wrap(
            spacing: AidogSpace.sxs,
            children: [
              // 每个 agent 一个开关：亮 = 已启用。禁写（无 npx）时点不动。
              for (final a in kSkillAgents)
                SmallButton(
                  label: t.t('skills.agent.$a'),
                  active: skill.enabledAgents.contains(a),
                  onTap: busy || !writeReady ? null : () => onToggleAgent(a),
                ),
              SmallButton(label: t.t('skills.share.title'), onTap: onShare),
              SmallButton(
                label: t.t('action.delete'),
                danger: true,
                onTap: onUninstall,
              ),
            ],
          ),
        ],
      ),
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

  @override
  void initState() {
    super.initState();
    _c = SkillInstallController(
      invoke: widget.invoke,
      t: (k, [a]) => i18n.t(k, a),
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
          style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg),
          onChanged: _c.setKeyword,
        ),
        const SizedBox(height: AidogSpace.ssm),
        if (_c.message != null) ToastBar(text: _c.message!, ok: true),
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
                onTap: disabled ? null : onInstall,
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              TileMeta(t.t('skills.install.selectAgent')),
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

  @override
  void initState() {
    super.initState();
    _c = SkillDetailController(
      invoke: widget.invoke,
      t: (k, [a]) => i18n.t(k, a),
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
                        // Markdown 与纯文本都按等宽原文渲染：React 用 react-markdown，
                        // 这里不引 markdown 依赖（差异见 README）。
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
