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
        SettingsCard(
          children: [
            SwitchRow(
              key: const ValueKey('mitm-master'),
              label: t.t('mitm.masterToggle'),
              description: t.t('mitm.masterToggleDesc'),
              value: _c.enabled,
              onChanged: _c.busy ? null : (_) => _c.toggleEnabled(),
            ),
          ],
        ),
        // 关掉总开关就整块收起，与 React 一致（`MitmConfig.tsx:272` / `:291` / `:386`
        // 三处都是 `{enabled && ...}`，URL 命中测试在白名单那块里面）。
        // 原先这四张卡无条件渲染：开关关着照样能装 CA、改白名单、跑命中测试，
        // 改完却一条都不生效，也没人告诉你为什么。
        if (_c.enabled) ...[
          SettingsCard(
            title: t.t('mitm.riskTitle'),
            description: t.t('mitm.riskDesc'),
            children: const [],
          ),
          _caCard(t),
          _whitelistCard(t),
          _testCard(t),
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

  // ── 假根证书 CA ────────────────────────────────────────

  Widget _caCard(I18nController t) {
    final theme = AidogTheme.of(context);
    final manual = _c.manualInstall;
    return SettingsCard(
      title: t.t('mitm.caCard'),
      description: t.t('mitm.caCardDesc'),
      children: [
        InfoRow(
          label: t.t('mitm.caPresent'),
          value: _c.caPresent ? t.t('common.yes') : t.t('common.no'),
        ),
        InfoRow(
          label: t.t('mitm.caInstalled'),
          value: _c.caInstalled ? t.t('mitm.installedHint') : t.t('common.no'),
        ),
        if (_c.fingerprint.isNotEmpty)
          InfoRow(label: t.t('mitm.fingerprint'), value: ltr(_c.fingerprint)),
        const SizedBox(height: AidogSpace.ssm),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: SmallButton(
            key: const ValueKey('mitm-install-ca'),
            label: t.t('mitm.installCa'),
            // CA 还没生成就装不了（`canInstallCa`）。
            onTap: _c.canInstallCa ? () => _c.installCa(_texts(t)) : null,
          ),
        ),
        if (manual != null) ...[
          const SizedBox(height: AidogSpace.ssm),
          Text(
            t.t('mitm.manualInstallTitle'),
            style: AidogType.micro.copyWith(color: theme.c.fg2),
          ),
          Wrap(
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Text(
                'CA PEM: ',
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
              SelectableText(
                manual.caPemPath,
                style: AidogType.numSm.copyWith(color: theme.c.fg),
              ),
            ],
          ),
          Wrap(
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Text(
                t.t('mitm.command'),
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
              SelectableText(
                manual.manualDisplay,
                style: AidogType.numSm.copyWith(color: theme.c.fg),
              ),
            ],
          ),
          // 装 CA 失败的真实诊断（`MitmConfig.tsx:362-375`）：exit / program /
          // stderr / stdout 直接摆出来，用户复现不必开控制台。
          //
          // 这块数据 `mitm_logic.dart:249` 早就存进控制器了，**一直没有渲染点** ——
          // 装不上时用户只看得到「照着这条命令手动装」，看不到为什么自动装失败。
          if (_c.installResult != null)
            _CaInstallDiagnostics(out: _c.installResult!),
          Text(
            t.t('mitm.manualInstallHint'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: SmallButton(
              key: const ValueKey('mitm-copy-command'),
              label: t.t('logs.copy'),
              onTap: () =>
                  (widget.copyFn ?? native.writeText)(manual.manualDisplay),
            ),
          ),
        ],
      ],
    );
  }

  // ── 白名单 ──────────────────────────────────────────────

  Widget _whitelistCard(I18nController t) {
    final theme = AidogTheme.of(context);
    return SettingsCard(
      title: t.t('mitm.whitelistTitle'),
      description: t.t('mitm.whitelistDesc'),
      children: [
        ChoiceRow(
          key: const ValueKey('mitm-rule-type'),
          label: t.t('mitm.ruleTypeLabel'),
          options: WhitelistRuleType.values.map((e) => e.wire).toList(),
          value: _c.newRuleType.wire,
          labelOf: (v) => switch (v) {
            'domain' => t.t('mitm.ruleDomain'),
            'suffix' => t.t('mitm.ruleSuffix'),
            'keyword' => t.t('mitm.ruleKeyword'),
            _ => t.t('mitm.ruleIpcidr'),
          },
          onChanged: (v) => _c.setNewRuleType(WhitelistRuleType.parse(v)),
        ),
        Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Expanded(
              child: TextRow(
                key: const ValueKey('mitm-new-pattern'),
                label: t.t('common.add'),
                hint: t.t('mitm.addPlaceholder'),
                value: _c.newPattern,
                onChanged: _c.setNewPattern,
                // React `MitmConfig.tsx:444`：回车 = 点「添加」。
                onEnter: (_) {
                  if (_c.canAdd) _c.addRule();
                },
              ),
            ),
            const SizedBox(width: AidogSpace.ssm),
            Padding(
              padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
              child: SmallButton(
                key: const ValueKey('mitm-add'),
                label: t.t('common.add'),
                onTap: _c.canAdd ? _c.addRule : null,
              ),
            ),
          ],
        ),
        Row(
          children: [
            SmallButton(
              key: const ValueKey('mitm-import-defaults'),
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
              label: t.t('mitm.clear'),
              danger: true,
              // 列表空时点不动（`canClear`）。
              onTap: _c.canClear ? _c.openClearConfirm : null,
            ),
          ],
        ),
        const SizedBox(height: AidogSpace.ssm),
        TextRow(
          key: const ValueKey('mitm-search'),
          label: t.t('mitm.searchPlaceholder'),
          hint: t.t('mitm.searchPlaceholder'),
          value: _c.search,
          onChanged: _c.setSearch,
        ),
        // 两种空态文案不同：整份为空 vs 搜索没命中。
        if (_c.showEmptyList)
          CenteredNote(text: t.t('mitm.whitelistEmpty'))
        else if (_c.showSearchEmpty)
          CenteredNote(text: t.t('mitm.searchEmpty'))
        else
          for (final e in _c.filteredWhitelist)
            Padding(
              key: ValueKey('wl-${e.hostPattern}'),
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      ltr(e.hostPattern),
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(color: theme.c.fg),
                    ),
                  ),
                  Text(
                    e.source == 'default'
                        ? t.t('mitm.sourceDefault')
                        : t.t('mitm.sourceUser'),
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                  SmallButton(
                    // 手动停用 ≠ 失效。原先关闭态复用了 `middleware.failed`
                    // （「失效」），用户自己关掉的白名单会显示成「失效」，
                    // 像是出了故障。
                    label: e.enabled
                        ? t.t('middleware.enabled')
                        : t.t('settings.perm.disableAuto'),
                    active: e.enabled,
                    onTap: () => _c.toggleEntry(e.hostPattern, !e.enabled),
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  SmallButton(
                    label: t.t('action.remove'),
                    danger: true,
                    onTap: () => _c.removeRule(e.hostPattern),
                  ),
                ],
              ),
            ),
      ],
    );
  }

  // ── URL 命中测试 ────────────────────────────────────────

  Widget _testCard(I18nController t) {
    final theme = AidogTheme.of(context);
    final result = _c.testResult;
    return SettingsCard(
      title: t.t('mitm.testUrlLabel'),
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Expanded(
              child: TextRow(
                key: const ValueKey('mitm-test-url'),
                label: t.t('mitm.testUrlLabel'),
                hint: t.t('mitm.testUrlPlaceholder'),
                value: _c.testUrl,
                onChanged: _c.setTestUrl,
                // React `MitmConfig.tsx:477`：回车 = 点「测试」。
                onEnter: (_) {
                  if (_c.canTest) _c.runUrlTest();
                },
              ),
            ),
            const SizedBox(width: AidogSpace.ssm),
            Padding(
              padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
              child: SmallButton(
                key: const ValueKey('mitm-test'),
                label: t.t('mitm.testUrlBtn'),
                onTap: _c.canTest ? _c.runUrlTest : null,
              ),
            ),
          ],
        ),
        // null = 还没测过（不渲染结果区）；空列表 = 测过但未命中。两者不是一回事。
        if (result != null) ...[
          Text(
            result.isEmpty
                ? t.t('mitm.testNoHit')
                : t.t('mitm.testUrlHit', {'n': result.length}),
            style: AidogType.micro.copyWith(color: theme.c.fg2),
          ),
          for (final r in result)
            Text(
              ltr('${r.hostPattern} · ${r.ruleType.wire}'),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
        ],
      ],
    );
  }
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
