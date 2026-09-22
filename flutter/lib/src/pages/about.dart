/// 关于页界面（票 I09），对应 `src/pages/About.tsx` 的四块：
/// 版本信息 / 软件更新 / GitHub 链接 / 本地环境。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../utils/formatters.dart';
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import '../updater.dart';
import 'about_logic.dart';
import 'invoke.dart';
import 'platform_card_bits.dart' show MiniBadge;
import 'ui_bits.dart';

class AboutPage extends StatefulWidget {
  const AboutPage({super.key, this.invoke = kernelInvoke, this.onCheckUpdate});

  final InvokeFn invoke;

  /// 票 I13：非 null 才渲染「检查更新」按钮（auto_updater 只覆盖 macOS / Windows）。
  /// null 走「这里不检查更新」的说明分支 —— widget 测试与不受支持的平台都用它。
  final Future<void> Function()? onCheckUpdate;

  @override
  State<AboutPage> createState() => _AboutPageState();
}

class _AboutPageState extends State<AboutPage> {
  late final AboutController _c;

  bool _built = false;

  /// 控制器的 t 必须来自 context（全局 `i18n` 在 widget 测试里没 init 过）。
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_built) return;
    _built = true;
    _c = AboutController(
      invoke: widget.invoke,
      t: AidogI18n.of(context).t,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    _c.init();
  }

  /// `About.tsx:95::buildTimeText`：非有限数 / ≤0 → 原样回显；否则按**秒**格式化。
  String _buildTimeText(I18nController t) {
    final raw = _c.info?.buildTime ?? '';
    if (raw.isEmpty) return '—';
    final secs = double.tryParse(raw);
    if (secs == null || !secs.isFinite || secs <= 0) return raw;
    final s = formatDateTime((secs * 1000).round());
    return s.isEmpty ? raw : s;
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final info = _c.info;
    final rows = info == null
        ? const <({String label, String value})>[]
        : [
            (label: t.t('about.appVersion'), value: 'v${info.appVersion}'),
            (label: t.t('about.tauriVersion'), value: 'v${info.tauriVersion}'),
            (label: t.t('about.os'), value: info.os),
            (label: t.t('about.arch'), value: info.arch),
            (label: t.t('about.profile'), value: info.profile),
            (label: t.t('about.gitCommit'), value: info.gitCommit),
            (label: t.t('about.buildTime'), value: _buildTimeText(t)),
          ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        PageHead(title: t.t('about.title'), subtitle: t.t('about.subtitle')),

        // ── 版本信息 ──
        // 四个区块入场错峰 0/80/160/240ms + 悬停抬升（`About.tsx:50-53,278`）。
        Reveal(
          delayMs: 0,
          child: HoverLift(
            child: Tile(
              child: rows.isEmpty
                  ? Text(
                      t.t('status.loading'),
                      style: AidogType.micro.copyWith(color: theme.c.fg2),
                    )
                  : Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        for (final r in rows)
                          Padding(
                            padding: const EdgeInsets.symmetric(vertical: 4),
                            child: Row(
                              children: [
                                Text(
                                  r.label,
                                  style: AidogType.micro.copyWith(
                                    color: theme.c.fg,
                                  ),
                                ),
                                const Spacer(),
                                // 版本号 / commit / 架构都是标识串，RTL 下不该被重排。
                                Flexible(
                                  child: Text(
                                    ltr(r.value),
                                    textAlign: TextAlign.end,
                                    style: AidogType.micro.copyWith(
                                      color: theme.c.fg2,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ),
                      ],
                    ),
            ),
          ),
        ),
        const SizedBox(height: AidogSpace.ssm),

        // ── 软件更新 ──
        // React 在 `About.tsx:291` 按 `isTauri()` 分两条分支；Flutter 壳对应的是
        // `onCheckUpdate` 有没有（票 I13 的 auto_updater，仅 macOS / Windows）：
        // 有 → 检查按钮，后续 UI（下载/安装/重启）Sparkle 自己接管；没有 → 一句说明。
        // 四个区块入场错峰 0/80/160/240ms + 悬停抬升（`About.tsx:50-53,278`）。
        Reveal(
          delayMs: 80,
          child: HoverLift(
            child: Tile(
              title: t.t('about.updateTitle'),
              child: widget.onCheckUpdate == null
                  ? Text(
                      t.t('about.updateDesktopOnly'),
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    )
                  : ValueListenableBuilder<(UpdateState, String)>(
                      valueListenable: updateStatus,
                      builder: (context, s, _) {
                        final (state, err) = s;
                        final busy = state == UpdateState.checking;
                        // 页内状态行（`About.tsx:283-287`）：Sparkle 的原生窗只在
                        // 「有新版本」时出现，「已是最新 / 检查失败」不弹窗，
                        // 没有这一行就等于点了没反应。
                        final status = switch (state) {
                          UpdateState.checking => t.t('about.checking'),
                          UpdateState.upToDate => t.t('about.upToDate'),
                          UpdateState.error =>
                            '${t.t('about.updateError')}: $err',
                          UpdateState.idle => '',
                        };
                        return Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            if (status.isNotEmpty) ...[
                              Text(
                                status,
                                style: AidogType.micro.copyWith(
                                  color: state == UpdateState.error
                                      ? theme.c.bad
                                      : theme.c.fg2,
                                ),
                              ),
                              const SizedBox(height: AidogSpace.sxs),
                            ],
                            SmallButton(
                              label: busy
                                  ? t.t('about.checking')
                                  : t.t('about.checkUpdate'),
                              onTap: busy ? null : widget.onCheckUpdate,
                            ),
                          ],
                        );
                      },
                    ),
            ),
          ),
        ),
        const SizedBox(height: AidogSpace.ssm),

        // ── GitHub 链接 ──
        // 四个区块入场错峰 0/80/160/240ms + 悬停抬升（`About.tsx:50-53,278`）。
        Reveal(
          delayMs: 160,
          child: HoverLift(
            child: Tile(
              title: t.t('about.githubTitle'),
              meta: ltr(kGithubRepo),
              child: Wrap(
                spacing: AidogSpace.ssm,
                runSpacing: AidogSpace.sxs,
                children: [
                  for (final b in const [
                    ('repo', 'about.repo'),
                    ('releases', 'about.releases'),
                    ('issues', 'about.issues'),
                    ('reportIssue', 'about.reportIssue'),
                  ])
                    SmallButton(
                      label: t.t(b.$2),
                      onTap: () => native.openUrl(kGithubLinks[b.$1]!),
                    ),
                ],
              ),
            ),
          ),
        ),
        const SizedBox(height: AidogSpace.ssm),

        // ── 本地环境 ──
        // 四个区块入场错峰 0/80/160/240ms + 悬停抬升（`About.tsx:50-53,278`）。
        Reveal(
          delayMs: 240,
          child: HoverLift(
            child: Tile(
              title: t.t('about.localEnv.title'),
              meta: t.t('about.localEnv.subtitle'),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Wrap(
                    spacing: AidogSpace.ssm,
                    children: [
                      SmallButton(
                        label: _c.cliBusy == 'check'
                            ? t.t('about.localEnv.checking')
                            : t.t('about.localEnv.check'),
                        onTap: _c.cliBusy.isEmpty ? _c.checkCli : null,
                      ),
                      SmallButton(
                        label: _c.cliBusy == 'diagnose'
                            ? t.t('about.localEnv.diagnosing')
                            : t.t('about.localEnv.diagnose'),
                        onTap: _c.cliBusy.isEmpty ? _c.diagnoseCli : null,
                      ),
                    ],
                  ),
                  for (final s in _c.cliTools)
                    _CliToolRow(
                      status: s,
                      conflict: _c.conflictFor(s.name),
                      busy: _c.cliBusy,
                      pending: _c.cliPendingTool == s.name,
                      onInstall: () => _c.installCli(s.name),
                      onUpgrade: () => _c.upgradeCli(s.name),
                    ),
                  if (_c.cliMsg.isNotEmpty) ToastBar(text: _c.cliMsg, ok: true),
                  if (_c.cliErr.isNotEmpty)
                    ToastBar(text: _c.cliErr, ok: false),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _CliToolRow extends StatelessWidget {
  const _CliToolRow({
    required this.status,
    required this.conflict,
    required this.busy,
    required this.pending,
    required this.onInstall,
    required this.onUpgrade,
  });

  final CliToolStatus status;
  final CliConflict? conflict;
  final String busy;
  final bool pending;
  final VoidCallback onInstall;
  final VoidCallback onUpgrade;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final kind = cliStatusKind(status);
    final statusColor = switch (kind) {
      CliStatusKind.notInstalled => theme.c.fg3,
      CliStatusKind.broken => theme.c.bad,
      CliStatusKind.conflict => theme.c.peak,
      CliStatusKind.installed => theme.c.ok,
    };
    final labelKey = kCliToolLabelKeys[status.name];
    final disabled = busy.isNotEmpty || pending;
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Text(
                labelKey == null ? status.name : t.t(labelKey),
                style: AidogType.body.copyWith(color: theme.c.fg),
              ),
              const SizedBox(width: AidogSpace.sxs),
              Text(
                t.t(cliStatusKey(kind)),
                style: AidogType.micro.copyWith(color: statusColor),
              ),
              const Spacer(),
              // 三个互斥的按钮：没装给「安装」；装了且有更新给「升级」；坏了给「修复」。
              if (!status.installed)
                SmallButton(
                  label: busy == 'install' && pending
                      ? t.t('about.localEnv.installing')
                      : t.t('about.localEnv.install'),
                  onTap: disabled ? null : onInstall,
                ),
              if (status.installed &&
                  !status.broken &&
                  status.hasUpdate == true)
                SmallButton(
                  label: busy == 'upgrade' && pending
                      ? t.t('about.localEnv.upgrading')
                      : t.t('about.localEnv.upgrade'),
                  onTap: disabled ? null : onUpgrade,
                ),
              if (status.broken)
                SmallButton(
                  label: busy == 'upgrade' && pending
                      ? t.t('about.localEnv.upgrading')
                      : t.t('about.localEnv.repair'),
                  // 「修复」到底修什么，解释写在悬浮提示里（`About.tsx:434`）。
                  tooltip: t.t('about.localEnv.brokenHint'),
                  onTap: disabled ? null : onUpgrade,
                ),
            ],
          ),
          Text(
            '${t.t('about.localEnv.version')}: '
            '${status.version ?? (status.installed ? t.t('about.localEnv.unknown') : '—')}'
            '${status.latestVersion != null ? '  ${t.t('about.localEnv.latest', {'version': status.latestVersion})}' : ''}',
            style: AidogType.micro.copyWith(color: theme.c.fg2),
          ),
          Text(
            '${t.t('about.localEnv.path')}: ${status.path ?? '—'}',
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          if (conflict != null && conflict!.installations.isNotEmpty)
            // 冲突诊断（`About.tsx:464-537`）：整块有警示边框与底色，
            // 每条安装里 source 是徽标、「已损坏」红、「PATH 默认」绿。
            // 原先一整行同色小字，哪条坏了、哪条是 PATH 默认要逐字读。
            Padding(
              padding: const EdgeInsets.only(top: AidogSpace.sxs),
              child: Container(
                padding: const EdgeInsets.all(AidogSpace.ssm),
                decoration: BoxDecoration(
                  color: conflict!.isConflicting
                      ? theme.c.peak.withValues(alpha: 0.08)
                      : theme.c.surface2,
                  border: Border.all(
                    color: conflict!.isConflicting
                        ? theme.c.peak
                        : theme.c.line,
                  ),
                  borderRadius: BorderRadius.circular(AidogRadius.sm),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      children: [
                        Text(
                          t.t('about.localEnv.installations', {
                            'count': conflict!.installations.length,
                          }),
                          style: AidogType.micro.copyWith(
                            color: theme.c.fg,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        if (conflict!.isConflicting) ...[
                          const SizedBox(width: AidogSpace.sxs),
                          Text(
                            t.t('about.localEnv.conflict'),
                            style: AidogType.micro.copyWith(
                              color: theme.c.peak,
                            ),
                          ),
                        ],
                      ],
                    ),
                    for (final inst in conflict!.installations)
                      Padding(
                        padding: const EdgeInsets.only(top: 3),
                        child: Wrap(
                          spacing: AidogSpace.ssm,
                          runSpacing: 2,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: [
                            Text(
                              inst.path,
                              style: AidogType.numSm.copyWith(
                                color: theme.c.fg2,
                              ),
                            ),
                            MiniBadge(text: inst.source, color: theme.c.fg3),
                            if (inst.version != null)
                              Text(
                                'v${inst.version}',
                                style: AidogType.numSm.copyWith(
                                  color: theme.c.fg2,
                                ),
                              ),
                            if (!inst.runnable)
                              Text(
                                t.t('about.localEnv.broken'),
                                style: AidogType.micro.copyWith(
                                  color: theme.c.bad,
                                ),
                              ),
                            if (inst.isPathDefault)
                              Text(
                                t.t('about.localEnv.pathDefault'),
                                style: AidogType.micro.copyWith(
                                  color: theme.c.ok,
                                ),
                              ),
                          ],
                        ),
                      ),
                    if (conflict!.suggestion.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.only(top: 2),
                        child: Text(
                          '${t.t('about.localEnv.suggestion')}: ${conflict!.suggestion}',
                          style: AidogType.micro.copyWith(color: theme.c.fg2),
                        ),
                      ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}
