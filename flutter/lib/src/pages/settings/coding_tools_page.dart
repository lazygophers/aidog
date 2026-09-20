/// CLI 集成页（`settings/coding_tools`）的 widget 层 —— 对齐
/// `src/components/settings/CodingToolsSettings.tsx` 的 7 项控件。
///
/// 即时保存，没有保存按钮。乐观翻转 / 失败回滚 / 常驻错误全在
/// [CodingToolsController]（票 I08 已测），这里只画界面。
///
/// **代理两个输入框是唯一一处「有草稿」的地方**：React 靠 onBlur 提交，
/// 而在桌面壳里点侧栏切页不一定触发 blur —— 所以这里额外挂了离页守卫
/// （见 `_syncGuard`）。这是相对 React 的一处**有意增强**，写在
/// flutter/README.md 的差异清单里。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../shell/nav_guard.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'coding_tools_logic.dart';
import 'schema_config_page.dart' show loadClaudeLanguageOptions;

class CodingToolsPage extends StatefulWidget {
  const CodingToolsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.languageLoader,
  });

  final InvokeFn invoke;

  /// widget 测试塞一份短清单，避免每个用例都解 121 KB 资产。
  final Future<List<({String value, String label})>> Function()?
  languageLoader;

  @override
  State<CodingToolsPage> createState() => _CodingToolsPageState();
}

class _CodingToolsPageState extends State<CodingToolsPage> {
  late final CodingToolsController _c;
  List<({String value, String label})> _languages = const [];

  void Function()? _unregisterGuard;

  /// 被守卫拦下、等用户裁决的那次导航。
  void Function()? _pendingNav;

  @override
  void initState() {
    super.initState();
    _c = CodingToolsController(
      invoke: widget.invoke,
      onChanged: () {
        if (!mounted) return;
        setState(() {});
        _syncGuard();
      },
    );
    unawaited(_c.load());
    unawaited(_loadLanguages());
  }

  Future<void> _loadLanguages() async {
    final l = await (widget.languageLoader?.call() ??
        loadClaudeLanguageOptions());
    if (mounted) setState(() => _languages = l);
  }

  @override
  void dispose() {
    _unregisterGuard?.call();
    _c.dispose();
    super.dispose();
  }

  /// 代理草稿与已生效值不一致 = 有未保存编辑。
  bool get _dirty => _c.proxyDraft.differsFrom(_c.proxyApplied);

  void _syncGuard() {
    if (_dirty) {
      _unregisterGuard ??= registerNavGuard((proceed) {
        setState(() => _pendingNav = proceed);
      });
    } else {
      _unregisterGuard?.call();
      _unregisterGuard = null;
    }
  }

  CodingToolsTexts _texts(I18nController t) => CodingToolsTexts(
    applied: t.t('codingTools.applied'),
    cleared: t.t('codingTools.cleared'),
    writeFailed: t.t('codingTools.writeFailed'),
  );

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final texts = _texts(t);
    if (_c.loading) {
      return SettingsPageBody(
        title: t.t('codingTools.cliIntegrationTitle'),
        children: [CenteredNote(text: t.t('status.loading'))],
      );
    }
    return SettingsPageBody(
      title: t.t('codingTools.cliIntegrationTitle'),
      children: [
        SettingsCard(
          description: t.t('codingTools.introDesc'),
          children: [
            SwitchRow(
              key: const ValueKey('apply-to-claude-plugin'),
              label: t.t('codingTools.applyPlugin.title'),
              description: t.t('codingTools.applyPlugin.desc'),
              value: _c.applyToClaudePlugin,
              onChanged: _c.busy
                  ? null
                  : (v) => _c.toggleApplyToClaudePlugin(v, texts),
            ),
            SwitchRow(
              key: const ValueKey('skip-onboarding'),
              label: t.t('codingTools.skipOnboarding.title'),
              description: t.t('codingTools.skipOnboarding.desc'),
              value: _c.skipClaudeOnboarding,
              onChanged: _c.busy
                  ? null
                  : (v) => _c.toggleSkipOnboarding(v, texts),
            ),
            SwitchRow(
              key: const ValueKey('date-rewrite'),
              label: t.t('codingTools.dateRewrite.title'),
              description: t.t('codingTools.dateRewrite.desc'),
              value: _c.dateRewriteEnabled == true,
              // 规则还没读到 / 不存在 → 开关不响应（`dateRewriteDisabled`）。
              onChanged: _c.dateRewriteDisabled
                  ? null
                  : (v) => _c.toggleDateRewrite(v, texts),
            ),
            SwitchRow(
              key: const ValueKey('btc-global'),
              label: t.t('proxy.btcGlobal'),
              description: t.t('proxy.btcGlobalDesc'),
              value: _c.btcGlobal,
              onChanged: _c.busy ? null : (v) => _c.toggleBtcGlobal(v, texts),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('codingTools.language.title'),
          description: t.t('codingTools.language.desc'),
          children: [
            SelectRow(
              key: const ValueKey('cli-language'),
              label: t.t('codingTools.language.title'),
              options: _languages.map((e) => e.value).toList(),
              labelOf: (v) => _languages
                  .firstWhere(
                    (e) => e.value == v,
                    orElse: () => (value: v, label: v),
                  )
                  .label,
              value: _c.language,
              onChanged: _c.busy
                  ? null
                  : (v) => _c.setLanguage(v ?? '', texts),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('codingTools.effort.title'),
          description: t.t('codingTools.effort.desc'),
          children: [
            ChoiceRow(
              key: const ValueKey('cli-effort'),
              label: t.t('settings.f_effortLevel'),
              options: kEffortOptions,
              value: _c.effort,
              onChanged: _c.busy ? null : (v) => _c.setEffort(v, texts),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('codingTools.proxy.title'),
          description: t.t('codingTools.proxy.desc'),
          children: [
            ChoiceRow(
              label: t.t('proxy.upstreamProxy'),
              options: kProxyUrlPresets,
              value: _c.proxyDraft.url,
              onChanged: _c.busy
                  ? null
                  : (v) {
                      _c.setProxyUrl(v);
                      _c.commitProxy(texts);
                    },
            ),
            TextRow(
              key: const ValueKey('cli-proxy-url'),
              label: t.t('proxy.upstreamProxy'),
              value: _c.proxyDraft.url,
              onChanged: _c.setProxyUrl,
              onSubmitted: (_) => _c.commitProxy(texts),
            ),
            TextRow(
              key: const ValueKey('cli-proxy-no'),
              label: t.t('proxy.noProxy'),
              description: t.t('proxy.noProxyDesc'),
              hint: t.t('proxy.noProxyPlaceholder'),
              value: _c.proxyDraft.no,
              onChanged: _c.setProxyNo,
              onSubmitted: (_) => _c.commitProxy(texts),
            ),
          ],
        ),
        if (_c.error.isNotEmpty)
          SettingsCard(
            title: t.t('codingTools.errorTitle'),
            description: t.t('codingTools.errorHint'),
            children: [ErrorNote(text: _c.error)],
          ),
        if (_pendingNav != null)
          UnsavedChangesCard(
            busy: _c.busy,
            onSave: () async {
              await _c.commitProxy(texts);
              if (!mounted) return;
              // 写失败时 error 非空 —— 留在原地，让用户看见为什么没走成。
              if (_c.error.isNotEmpty) return;
              final proceed = _pendingNav;
              setState(() => _pendingNav = null);
              proceed?.call();
            },
            onDiscard: () {
              final proceed = _pendingNav;
              setState(() {
                _c.proxyDraft = _c.proxyApplied;
                _pendingNav = null;
              });
              _syncGuard();
              proceed?.call();
            },
            onCancel: () => setState(() => _pendingNav = null),
          ),
        if (_c.message.isNotEmpty)
          AutoToast(
            text: _c.message,
            onDone: () => setState(() => _c.message = ''),
          ),
      ],
    );
  }
}
