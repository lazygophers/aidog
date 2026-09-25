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
    // mono 标记照 React（About.tsx:199-206）：除构建时间外值全走等宽。
    final rows = info == null
        ? const <({String label, String value, bool mono})>[]
        : [
            (
              label: t.t('about.appVersion'),
              value: 'v${info.appVersion}',
              mono: true,
            ),
            (
              label: t.t('about.tauriVersion'),
              value: 'v${info.tauriVersion}',
              mono: true,
            ),
            (label: t.t('about.os'), value: info.os, mono: true),
            (label: t.t('about.arch'), value: info.arch, mono: true),
            (label: t.t('about.profile'), value: info.profile, mono: true),
            (label: t.t('about.gitCommit'), value: info.gitCommit, mono: true),
            (
              label: t.t('about.buildTime'),
              value: _buildTimeText(t),
              mono: false,
            ),
          ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // 页容器 `gap: 20`（`src/pages/About.tsx:221`）。
        PageHead(
          title: t.t('about.title'),
          subtitle: t.t('about.subtitle'),
          bottom: 20,
        ),

        // ── 版本信息 ──
        // 四个区块入场错峰 0/80/160/240ms + 悬停抬升（`About.tsx:50-53,278`）。
        Reveal(
          delayMs: 0,
          child: HoverLift(
            child: Tile(
              // React 首卡 padding 8/20（About.tsx:237），行距靠行自己的 12/0
              // padding + 行间 1px 分隔线撑出来。
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
              child: rows.isEmpty
                  ? Padding(
                      padding: const EdgeInsets.symmetric(vertical: 16),
                      child: Text(
                        t.t('status.loading'),
                        style: AidogType.caption.copyWith(
                          fontSize: 13,
                          color: theme.c.fg2,
                        ),
                      ),
                    )
                  : Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        for (final (i, r) in rows.indexed)
                          Container(
                            padding: const EdgeInsets.symmetric(vertical: 12),
                            decoration: BoxDecoration(
                              border: i == 0
                                  ? null
                                  : Border(
                                      top: BorderSide(color: theme.c.line),
                                    ),
                            ),
                            child: Row(
                              children: [
                                Text(
                                  r.label,
                                  style: AidogType.caption.copyWith(
                                    fontSize: 13,
                                    fontWeight: FontWeight.w600,
                                    color: theme.c.fg,
                                  ),
                                ),
                                const Spacer(),
                                // 版本号 / commit / 架构都是标识串，RTL 下不该被重排。
                                Flexible(
                                  child: Text(
                                    ltr(r.value),
                                    textAlign: TextAlign.end,
                                    style:
                                        (r.mono
                                                ? AidogType.numSm.copyWith(
                                                    fontFamily:
                                                        AidogType.familyMono,
                                                  )
                                                : AidogType.caption)
                                            .copyWith(
                                              fontSize: 13,
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
        const SizedBox(height: 20),

        // ── 软件更新 ──
        // React 在 `About.tsx:291` 按 `isTauri()` 分两条分支；Flutter 壳对应的是
        // `onCheckUpdate` 有没有（票 I13 的 auto_updater，仅 macOS / Windows）：
        // 有 → 检查按钮，后续 UI（下载/安装/重启）Sparkle 自己接管；没有 → 一句说明。
        // 四个区块入场错峰 0/80/160/240ms + 悬停抬升（`About.tsx:50-53,278`）。
        Reveal(
          delayMs: 80,
          child: HoverLift(
            child: Tile(
              // React 其余卡 padding 16/20（About.tsx:278,308,332）
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
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
                            // 关于页这一片 React 全是 `variant="default"` = 实心
                            //（`About.tsx:292,314,343,352,402,415,428`）。
                            SmallButton(
                              filled: true,
                              // 这颗**没有**内联 style（`About.tsx:292-299`），
                              // 吃 shadcn `<Button>` 默认档 36 高 / px-4 / text-sm；
                              // 旁边 GitHub 四颗与 CLI 两颗才是 13 / 6-12。
                              fontSize: 14,
                              padding: (16, 8),
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
        const SizedBox(height: 20),

        // ── GitHub 链接 ──
        // 四个区块入场错峰 0/80/160/240ms + 悬停抬升（`About.tsx:50-53,278`）。
        Reveal(
          delayMs: 160,
          child: HoverLift(
            child: Tile(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
              title: t.t('about.githubTitle'),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Wrap(
                    // `gap: 8` 横竖同值（`About.tsx:312`）。
                    spacing: 8,
                    runSpacing: 8,
                    children: [
                      for (final b in const [
                        ('repo', 'about.repo'),
                        ('releases', 'about.releases'),
                        ('issues', 'about.issues'),
                        ('reportIssue', 'about.reportIssue'),
                      ])
                        SmallButton(
                          filled: true,
                          fontSize: 13, // About.tsx:318
                          padding: (12, 6),
                          // 每颗前面一枚 14px 地球图标，与文字隔 6
                          //（`About.tsx:318,321` 的 `<IconGlobe size={14}>`）。
                          icon: Icons.public,
                          label: t.t(b.$2),
                          onTap: () => native.openUrl(kGithubLinks[b.$1]!),
                        ),
                    ],
                  ),
                  const SizedBox(height: AidogSpace.smd),
                  // repo 串：mono 12 tertiary，按钮下方独立一行（About.tsx:326-328）。
                  Text(
                    ltr(kGithubRepo),
                    style: AidogType.numSm.copyWith(
                      fontSize: 12,
                      color: theme.c.fg3,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
        const SizedBox(height: 20),

        // ── 本地环境 ──
        // 四个区块入场错峰 0/80/160/240ms + 悬停抬升（`About.tsx:50-53,278`）。
        Reveal(
          delayMs: 240,
          child: HoverLift(
            child: Tile(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
              title: t.t('about.localEnv.title'),
              meta: t.t('about.localEnv.subtitle'),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Wrap(
                    // 两颗按钮 `gap: 8`（`About.tsx:342`）。
                    spacing: 8,
                    children: [
                      SmallButton(
                        filled: true,
                        fontSize: 13, // About.tsx:346
                        padding: (12, 6),
                        label: _c.cliBusy == 'check'
                            ? t.t('about.localEnv.checking')
                            : t.t('about.localEnv.check'),
                        onTap: _c.cliBusy.isEmpty ? _c.checkCli : null,
                      ),
                      SmallButton(
                        filled: true,
                        // 与「检查版本」同档（`About.tsx:355`），原先漏传吃了缺省。
                        fontSize: 13,
                        padding: (12, 6),
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
                  // 提示 / 错误是卡尾**页面流**里的 `.toast` 方条
                  //（`About.tsx:550-559`：pad 12/16、12px、成功绿 / 失败红），
                  // 不是浮在窗口顶部的胶囊。
                  if (_c.cliMsg.isNotEmpty)
                    InlineNote(
                      text: _c.cliMsg,
                      padding: const EdgeInsets.symmetric(
                        horizontal: 16,
                        vertical: 12,
                      ),
                      fontSize: 12,
                      color: theme.c.ok,
                    ),
                  if (_c.cliErr.isNotEmpty)
                    InlineNote(
                      text: _c.cliErr,
                      padding: const EdgeInsets.symmetric(
                        horizontal: 16,
                        vertical: 12,
                      ),
                      fontSize: 12,
                      color: theme.c.bad,
                    ),
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
                style: AidogType.caption.copyWith(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: theme.c.fg,
                ),
              ),
              // 工具名↔状态 chip `gap: 8`（`About.tsx:381`）。
              const SizedBox(width: 8),
              // 状态 chip：11px、padding 2/8、radius 4、底 `--bg-glass`（= `--card`
              // ≈ surface，不是 surface2）（About.tsx:386-394）。
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                decoration: BoxDecoration(
                  color: theme.c.surface,
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  t.t(cliStatusKey(kind)),
                  style: AidogType.micro.copyWith(color: statusColor),
                ),
              ),
              const Spacer(),
              // 三个互斥的按钮：没装给「安装」；装了且有更新给「升级」；坏了给「修复」。
              if (!status.installed)
                SmallButton(
                  filled: true,
                  fontSize: 12,
                  padding: (10, 4),
                  label: busy == 'install' && pending
                      ? t.t('about.localEnv.installing')
                      : t.t('about.localEnv.install'),
                  onTap: disabled ? null : onInstall,
                ),
              if (status.installed &&
                  !status.broken &&
                  status.hasUpdate == true)
                SmallButton(
                  filled: true,
                  fontSize: 12,
                  padding: (10, 4),
                  label: busy == 'upgrade' && pending
                      ? t.t('about.localEnv.upgrading')
                      : t.t('about.localEnv.upgrade'),
                  onTap: disabled ? null : onUpgrade,
                ),
              if (status.broken)
                SmallButton(
                  filled: true,
                  fontSize: 12,
                  padding: (10, 4),
                  label: busy == 'upgrade' && pending
                      ? t.t('about.localEnv.upgrading')
                      : t.t('about.localEnv.repair'),
                  // 「修复」到底修什么，解释写在悬浮提示里（`About.tsx:434`）。
                  tooltip: t.t('about.localEnv.brokenHint'),
                  onTap: disabled ? null : onUpgrade,
                ),
            ],
          ),
          // 两行同在 `color: var(--text-secondary)` 的容器里，只有
          // 「版本：」「路径：」两个前缀是 tertiary（`About.tsx:443-460`）——
          // 原先整条路径行都是 fg3。
          Text.rich(
            TextSpan(
              children: [
                TextSpan(
                  text: '${t.t('about.localEnv.version')}: ',
                  style: TextStyle(color: theme.c.fg3),
                ),
                TextSpan(
                  text:
                      '${status.version ?? (status.installed ? t.t('about.localEnv.unknown') : '—')}'
                      '${status.latestVersion != null ? '  ${t.t('about.localEnv.latest', {'version': status.latestVersion})}' : ''}',
                ),
              ],
            ),
            style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg2),
          ),
          Text.rich(
            TextSpan(
              children: [
                TextSpan(
                  text: '${t.t('about.localEnv.path')}: ',
                  style: TextStyle(color: theme.c.fg3),
                ),
                TextSpan(text: status.path ?? '—'),
              ],
            ),
            style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg2),
          ),
          if (conflict != null && conflict!.installations.isNotEmpty)
            // 冲突诊断（`About.tsx:464-537`）：整块有警示边框与底色，
            // 每条安装里 source 是徽标、「已损坏」红、「PATH 默认」绿。
            // 原先一整行同色小字，哪条坏了、哪条是 PATH 默认要逐字读。
            Padding(
              padding: const EdgeInsets.only(top: AidogSpace.sxs),
              child: Container(
                // `padding: 10`（`About.tsx:467`）。
                padding: const EdgeInsets.all(10),
                decoration: BoxDecoration(
                  // 非冲突态 React 是 `--bg-glass`（= `--card` ≈ surface），
                  // 不是 surface2（`About.tsx:474`）。
                  color: conflict!.isConflicting
                      ? theme.c.peak.withValues(alpha: 0.08)
                      : theme.c.surface,
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
                        // 块内 `gap: 6`（`About.tsx:479`），行内 `gap: 8`
                        //（`About.tsx:504`）横竖同值。
                        padding: const EdgeInsets.only(top: 6),
                        child: Wrap(
                          spacing: 8,
                          runSpacing: 8,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: [
                            Text(
                              inst.path,
                              style: AidogType.numSm.copyWith(
                                color: theme.c.fg2,
                              ),
                            ),
                            // source 徽标：`padding: "1px 6px"`、radius 3、
                            // 底 `--bg-glass`、字 11 secondary（`About.tsx:511-520`）。
                            MiniBadge(
                              text: inst.source,
                              color: theme.c.fg2,
                              background: theme.c.surface,
                              radius: 3,
                              fontSize: 11,
                            ),
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
