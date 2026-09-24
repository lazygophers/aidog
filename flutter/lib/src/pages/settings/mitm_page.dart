/// MITM 解密页（`settings/mitm`）的 widget 层 —— 对齐
/// `src/components/settings/MitmConfig.tsx`。
///
/// 六个控件的禁用条件全部来自 [MitmController] 的派生态（`canAdd` / `canTest` /
/// `canClear` / `canInstallCa`），本文件不自己再判一遍 —— 判两遍就会漂移。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../../platform.dart' as native;
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'mitm_logic.dart';

class MitmSettingsPage extends StatefulWidget {
  const MitmSettingsPage({super.key, this.invoke = kernelInvoke, this.copyFn});

  final InvokeFn invoke;

  /// 复制手动安装命令到剪贴板（I12 的 `writeText`）。测试塞假的。
  final Future<void> Function(String text)? copyFn;

  @override
  State<MitmSettingsPage> createState() => _MitmSettingsPageState();
}

class _MitmSettingsPageState extends State<MitmSettingsPage> {
  late final MitmController _c;

  @override
  void initState() {
    super.initState();
    _c = MitmController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    unawaited(_c.refresh());
  }

  MitmInstallTexts _texts(I18nController t) => MitmInstallTexts(
    cancel: t.t('mitm.installCancel'),
    authFail: t.t('mitm.installAuthFail'),
    noAgent: t.t('mitm.installNoAgent'),
    failed: (code) => t.t('mitm.installFailed', {'code': code}),
  );

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    if (_c.loading) {
      return SettingsPageBody(
        title: t.t('appSettings.mitmTab'),
        children: [CenteredNote(text: t.t('status.loading'))],
      );
    }
    return SettingsPageBody(
      title: t.t('appSettings.mitmTab'),
      subtitle: '${_c.whitelist.length}',
      children: [
        ToggleCard(
          key: const ValueKey('mitm-master'),
          label: t.t('mitm.masterToggle'),
          descriptions: [t.t('mitm.masterToggleDesc')],
          value: _c.enabled,
          onChanged: _c.busy ? null : (_) => _c.toggleEnabled(),
        ),
        // 关掉总开关就整块收起，与 React 一致（`MitmConfig.tsx:272` / `:291` / `:386`
        // 三处都是 `{enabled && ...}`，URL 命中测试在白名单那块里面）。
        // 原先这四张卡无条件渲染：开关关着照样能装 CA、改白名单、跑命中测试，
        // 改完却一条都不生效，也没人告诉你为什么。
        if (_c.enabled) ...[
          _riskCard(t),
          _caCard(t),
          _whitelistCard(t),
        ],
        if (_c.showClearConfirm)
          ConfirmCard(
            title: t.t('mitm.clear'),
            body: t.t('mitm.clearConfirm', {'n': _c.whitelist.length}),
            confirmLabel: t.t('mitm.clear'),
            busy: _c.busy,
            // React 这处用的是普通 `Dialog`（`MitmConfig.tsx:610`），点遮罩可关。
            dismissOnBarrier: true,
            onCancel: _c.closeClearConfirm,
            onConfirm: () =>
                _c.confirmClear((n) => t.t('mitm.clearDone', {'n': n})),
          ),
        if (_c.error.isNotEmpty) ErrorNote(text: _c.error),
        if (_c.message.isNotEmpty)
          AutoToast(
            text: _c.message,
            onDone: () => setState(() => _c.message = ''),
          ),
      ],
    );
  }

  /// 风险提示卡：左缘 3px warning 色条、padding 14/16
  /// （`MitmConfig.tsx:274-287`）。
  Widget _riskCard(I18nController t) {
    final theme = AidogTheme.of(context);
    return Padding(
      key: const ValueKey('mitm-risk-card'),
      padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
        decoration: BoxDecoration(
          color: theme.c.surface,
          border: Border(
            left: BorderSide(color: theme.c.peak, width: 3),
          ),
          borderRadius: BorderRadius.circular(AidogRadius.md),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              t.t('mitm.riskTitle'),
              style: AidogType.caption.copyWith(
                fontSize: 12,
                fontWeight: FontWeight.w600,
                color: theme.c.fg,
              ),
            ),
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                t.t('mitm.riskDesc'),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg2,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  // ── 假根证书 CA ────────────────────────────────────────

  Widget _caCard(I18nController t) {
    final theme = AidogTheme.of(context);
    final manual = _c.manualInstall;
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
      child: Tile(
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              t.t('mitm.caCard'),
              style: AidogType.label.copyWith(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: theme.c.fg,
              ),
            ),
            Padding(
              padding: const EdgeInsets.only(top: 2),
              child: Text(
                t.t('mitm.caCardDesc'),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg2,
                ),
              ),
            ),
            const SizedBox(height: AidogSpace.smd),
            // 两项横排：12px + 是/否 strong 着色 success/tertiary
            //（`MitmConfig.tsx:303-312`）。
            Wrap(
              spacing: 16,
              runSpacing: AidogSpace.sxs,
              children: [
                _caFlag(t.t('mitm.caPresent'), _c.caPresent, t),
                _caFlag(t.t('mitm.caInstalled'), _c.caInstalled, t),
              ],
            ),
            if (_c.fingerprint.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: AidogSpace.sxs),
                child: Text(
                  '${t.t('mitm.fingerprint')} ${ltr(_c.fingerprint)}',
                  style: AidogType.numSm.copyWith(
                    fontSize: 11,
                    color: theme.c.fg3,
                  ),
                ),
              ),
            const SizedBox(height: AidogSpace.smd),
            Row(
              children: [
                SmallButton(
                  key: const ValueKey('mitm-install-ca'),
                  filled: true,
                  fontSize: 13,
                  padding: (16, 7),
                  label: _c.busy
                      ? t.t('common.loading')
                      : t.t('mitm.installCa'),
                  // CA 还没生成就装不了（`canInstallCa`）。
                  onTap: _c.canInstallCa ? () => _c.installCa(_texts(t)) : null,
                ),
                if (_c.caInstalled) ...[
                  const SizedBox(width: AidogSpace.ssm),
                  Text(
                    t.t('mitm.installedHint'),
                    style: AidogType.caption.copyWith(
                      fontSize: 12,
                      color: theme.c.ok,
                    ),
                  ),
                ],
              ],
            ),
            if (manual != null) ...[
              const SizedBox(height: AidogSpace.ssm),
              // 手动装兜底（D8）：warning wash 盒（`MitmConfig.tsx:340-358`）。
              Container(
                width: double.infinity,
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: theme.c.peak.withValues(alpha: 0.10),
                  border: Border.all(
                    color: theme.c.peak.withValues(alpha: 0.30),
                  ),
                  borderRadius: BorderRadius.circular(AidogRadius.md),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      t.t('mitm.manualInstallTitle'),
                      style: AidogType.caption.copyWith(
                        fontSize: 12,
                        fontWeight: FontWeight.w600,
                        color: theme.c.fg,
                      ),
                    ),
                    const SizedBox(height: 6),
                    Wrap(
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
                        Text(
                          'CA PEM: ',
                          style: AidogType.caption.copyWith(
                            fontSize: 12,
                            color: theme.c.fg2,
                          ),
                        ),
                        SelectableText(
                          manual.caPemPath,
                          style: AidogType.numSm.copyWith(color: theme.c.fg),
                        ),
                      ],
                    ),
                    const SizedBox(height: 6),
                    Wrap(
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
                        Text(
                          t.t('mitm.command'),
                          style: AidogType.caption.copyWith(
                            fontSize: 12,
                            color: theme.c.fg2,
                          ),
                        ),
                        SelectableText(
                          manual.manualDisplay,
                          style: AidogType.numSm.copyWith(color: theme.c.fg),
                        ),
                      ],
                    ),
                    // 装 CA 失败的真实诊断（`MitmConfig.tsx:362-375`）：exit /
                    // program / stderr / stdout 直接摆出来，用户复现不必开控制台。
                    //
                    // 这块数据 `mitm_logic.dart:249` 早就存进控制器了，**一直没有
                    // 渲染点** —— 装不上时用户只看得到「照着这条命令手动装」，
                    // 看不到为什么自动装失败。
                    if (_c.installResult != null)
                      _CaInstallDiagnostics(out: _c.installResult!),
                    Text(
                      t.t('mitm.manualInstallHint'),
                      style: AidogType.caption.copyWith(
                        fontSize: 11,
                        color: theme.c.fg3,
                      ),
                    ),
                    Align(
                      alignment: AlignmentDirectional.centerStart,
                      child: SmallButton(
                        key: const ValueKey('mitm-copy-command'),
                        label: t.t('logs.copy'),
                        onTap: () =>
                            (widget.copyFn ?? native.writeText)(
                              manual.manualDisplay,
                            ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }

  /// CA 状态项：「已生成：是」——12px label + 着色 strong
  ///（success=已生成 / tertiary=没有，`MitmConfig.tsx:303-312`）。
  Widget _caFlag(String label, bool on, I18nController t) {
    final theme = AidogTheme.of(context);
    return Text.rich(
      TextSpan(
        children: [
          TextSpan(
            text: label,
            style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg2),
          ),
          TextSpan(
            text: on ? t.t('common.yes') : t.t('common.no'),
            style: AidogType.caption.copyWith(
              fontSize: 12,
              fontWeight: FontWeight.w600,
              color: on ? theme.c.ok : theme.c.fg3,
            ),
          ),
        ],
      ),
    );
  }

  // ── 白名单（含 URL 命中测试，`MitmConfig.tsx:382-640`）──────────

  Widget _whitelistCard(I18nController t) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
      child: Tile(
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 页头：标题说明左、导入/清空按钮右（`MitmConfig.tsx:387-413`）。
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        t.t('mitm.whitelistTitle'),
                        style: AidogType.label.copyWith(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: theme.c.fg,
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(top: 2),
                        child: Text(
                          t.t('mitm.whitelistDesc'),
                          style: AidogType.caption.copyWith(
                            fontSize: 12,
                            color: theme.c.fg2,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: AidogSpace.smd),
                SmallButton(
                  key: const ValueKey('mitm-import-defaults'),
                  fontSize: 12,
                  padding: (12, 6),
                  label: t.t('mitm.importDefaults'),
                  onTap: _c.busy
                      ? null
                      : () => _c.importDefaults(
                          (imported, skipped) => t.t('mitm.importDefaultsDone', {
                            'imported': imported,
                            'skipped': skipped,
                          }),
                        ),
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  key: const ValueKey('mitm-clear'),
                  fontSize: 12,
                  padding: (12, 6),
                  label: t.t('mitm.clear'),
                  danger: true,
                  // 列表空时点不动（`canClear`）。
                  onTap: _c.canClear ? _c.openClearConfirm : null,
                ),
              ],
            ),
            const SizedBox(height: AidogSpace.smd),
            // 添加行：匹配方式 Select(≤120) + 输入(≤320) + 添加按钮，同一行
            //（`MitmConfig.tsx:423-456`）。
            Wrap(
              spacing: AidogSpace.ssm,
              runSpacing: AidogSpace.ssm,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                InlineSelect<String>(
                  key: const ValueKey('mitm-rule-type'),
                  value: _c.newRuleType.wire,
                  options: WhitelistRuleType.values
                      .map((e) => e.wire)
                      .toList(),
                  width: 120,
                  onChanged: (v) =>
                      _c.setNewRuleType(WhitelistRuleType.parse(v!)),
                ),
                SizedBox(
                  width: 320,
                  child: KeptTextField(
                    key: const ValueKey('mitm-new-pattern'),
                    value: _c.newPattern,
                    hint: t.t('mitm.addPlaceholder'),
                    fontSize: 12,
                    onChanged: _c.setNewPattern,
                    // React `MitmConfig.tsx:444`：回车 = 点「添加」。
                    onSubmitted: (_) {
                      if (_c.canAdd) _c.addRule();
                    },
                  ),
                ),
                SmallButton(
                  key: const ValueKey('mitm-add'),
                  fontSize: 13,
                  padding: (14, 7),
                  label: t.t('common.add'),
                  onTap: _c.canAdd ? _c.addRule : null,
                ),
              ],
            ),
            const SizedBox(height: AidogSpace.smd),
            // D2 搜索过滤（前端纯 filter，实时无按钮），maxWidth 320、12px。
            SizedBox(
              width: 320,
              child: KeptTextField(
                key: const ValueKey('mitm-search'),
                value: _c.search,
                hint: t.t('mitm.searchPlaceholder'),
                fontSize: 12,
                onChanged: _c.setSearch,
              ),
            ),
            const SizedBox(height: AidogSpace.smd),
            _testBox(t),
            const SizedBox(height: AidogSpace.smd),
            // 列表（D2 搜索过滤后）。两种空态文案不同：整份为空 vs 搜索没命中。
            if (_c.showEmptyList)
              Text(
                t.t('mitm.whitelistEmpty'),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg3,
                ),
              )
            else if (_c.showSearchEmpty)
              Text(
                t.t('mitm.searchEmpty'),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg3,
                ),
              )
            else
              for (final e in _c.filteredWhitelist)
                Padding(
                  key: ValueKey('wl-${e.hostPattern}'),
                  padding: const EdgeInsets.only(bottom: 6),
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 8,
                    ),
                    decoration: BoxDecoration(
                      color: theme.c.surface2,
                      border: Border.all(color: theme.c.line),
                      borderRadius: BorderRadius.circular(AidogRadius.md),
                    ),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            ltr(e.hostPattern),
                            overflow: TextOverflow.ellipsis,
                            style: AidogType.numMd.copyWith(
                              fontSize: 12,
                              color: theme.c.fg,
                            ),
                          ),
                        ),
                        const SizedBox(width: AidogSpace.ssm),
                        // source 徽标：默认=primary wash、自定义=tertiary wash
                        //（`MitmConfig.tsx:548-559`）。
                        _wlBadge(
                          theme,
                          text: e.source == 'default'
                              ? t.t('mitm.sourceDefault')
                              : t.t('mitm.sourceUser'),
                          color: e.source == 'default'
                              ? theme.c.accent
                              : theme.c.fg3,
                        ),
                        const SizedBox(width: AidogSpace.sxs),
                        // 规则类型徽标（`MitmConfig.tsx:560-579`）：同一条 host
                        // 写成域名 / 后缀 / 关键字 / IP 段，命中范围差很远，
                        // 列表里必须分得出。
                        _wlBadge(
                          theme,
                          text: _ruleTypeLabel(t, e.ruleType),
                          color: theme.c.ok,
                        ),
                        const SizedBox(width: AidogSpace.ssm),
                        // 开关而不是按钮（`MitmConfig.tsx:580-584`）：按钮上写的
                        // 是「当前是什么状态」还是「点了会变成什么」，本身就有歧义。
                        AidogSwitch(
                          compact: true,
                          value: e.enabled,
                          onChanged: () =>
                              _c.toggleEntry(e.hostPattern, !e.enabled),
                        ),
                        const SizedBox(width: AidogSpace.sxs),
                        IconGhostButton(
                          icon: Icons.close,
                          tooltip: t.t('action.remove'),
                          onTap: () => _c.removeRule(e.hostPattern),
                        ),
                      ],
                    ),
                  ),
                ),
          ],
        ),
      ),
    );
  }

  /// 10px 徽标（2/6、r4、wash 底 + secondary 字，`MitmConfig.tsx:548-579`）。
  Widget _wlBadge(AidogTheme theme, {required String text, required Color color}) {
    final theme = AidogTheme.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        text,
        style: AidogType.caption.copyWith(
          fontSize: 10,
          color: theme.c.fg2,
        ),
      ),
    );
  }

  /// URL 命中测试：白名单卡内嵌套盒（bg-glass 边框 r-md padding 10/12，
  /// `MitmConfig.tsx:467-523`），不是独立卡。
  Widget _testBox(I18nController t) {
    final theme = AidogTheme.of(context);
    final result = _c.testResult;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: theme.c.surface2,
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.md),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            t.t('mitm.testUrlLabel'),
            style: AidogType.caption.copyWith(
              fontSize: 12,
              fontWeight: FontWeight.w600,
              color: theme.c.fg,
            ),
          ),
          const SizedBox(height: AidogSpace.ssm),
          Row(
            children: [
              Expanded(
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 360),
                  child: KeptTextField(
                    key: const ValueKey('mitm-test-url'),
                    value: _c.testUrl,
                    hint: t.t('mitm.testUrlPlaceholder'),
                    fontSize: 12,
                    onChanged: _c.setTestUrl,
                    // React `MitmConfig.tsx:477`：回车 = 点「测试」。
                    onSubmitted: (_) {
                      if (_c.canTest) _c.runUrlTest();
                    },
                  ),
                ),
              ),
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(
                key: const ValueKey('mitm-test'),
                fontSize: 12,
                padding: (14, 6),
                label: t.t('mitm.testUrlBtn'),
                onTap: _c.canTest ? _c.runUrlTest : null,
              ),
            ],
          ),
          // null = 还没测过（不渲染结果区）；空列表 = 测过但未命中。两者不是一回事。
          if (result != null) ...[
            const SizedBox(height: AidogSpace.ssm),
            Text(
              result.isEmpty
                  ? t.t('mitm.testNoHit')
                  : t.t('mitm.testUrlHit', {'n': result.length}),
              style: AidogType.caption.copyWith(
                fontSize: 12,
                color: theme.c.fg2,
              ),
            ),
            for (final r in result)
              Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        ltr(r.hostPattern),
                        style: AidogType.numSm.copyWith(
                          fontSize: 11,
                          color: theme.c.fg,
                        ),
                      ),
                    ),
                    const SizedBox(width: AidogSpace.ssm),
                    _wlBadge(
                      theme,
                      text: _ruleTypeLabel(t, r.ruleType),
                      color: theme.c.ok,
                    ),
                  ],
                ),
              ),
          ],
        ],
      ),
    );
  }

  /// 规则类型 → 文案。解析层已把认不出的值归到 `suffix`
  /// （`mitm_logic.dart:29-30`，与 `MitmConfig.tsx:561` 同缺省）。
  String _ruleTypeLabel(I18nController t, WhitelistRuleType ruleType) =>
      switch (ruleType) {
        WhitelistRuleType.domain => t.t('mitm.ruleDomain'),
        WhitelistRuleType.suffix => t.t('mitm.ruleSuffix'),
        WhitelistRuleType.keyword => t.t('mitm.ruleKeyword'),
        WhitelistRuleType.ipcidr => t.t('mitm.ruleIpcidr'),
      };

  // ── URL 命中测试 ────────────────────────────────────────

}

/// 装 CA 失败时的诊断块（`MitmConfig.tsx:362-375`）：等宽、可选中、能整段复制走。
class _CaInstallDiagnostics extends StatelessWidget {
  const _CaInstallDiagnostics({required this.out});

  final CaInstallOutcome out;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final text =
        'exit=${out.code} program=${out.program}\n'
        'stderr: ${out.stderr.isEmpty ? '(empty)' : out.stderr}\n'
        'stdout: ${out.stdout.isEmpty ? '(empty)' : out.stdout}';
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.sxs),
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.all(AidogSpace.ssm),
        decoration: BoxDecoration(
          color: theme.c.surface2,
          border: Border.all(color: theme.c.bad),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: SelectableText(
          text,
          key: const ValueKey('mitm-install-diagnostics'),
          style: AidogType.numSm.copyWith(color: theme.c.fg2),
        ),
      ),
    );
  }
}
