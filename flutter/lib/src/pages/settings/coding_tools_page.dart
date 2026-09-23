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
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'coding_tools_logic.dart';
import 'schema_config_page.dart' show loadClaudeLanguageGroups;

class CodingToolsPage extends StatefulWidget {
  const CodingToolsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.languageLoader,
  });

  final InvokeFn invoke;

  /// widget 测试塞一份短清单，避免每个用例都解 121 KB 资产。
  /// 带语族分组（下拉里画分组标题，`CodingToolsSettings.tsx:433-450`）。
  final Future<
    List<({String family, List<({String value, String label})> options})>
  >
  Function()?
  languageLoader;

  @override
  State<CodingToolsPage> createState() => _CodingToolsPageState();
}

class _CodingToolsPageState extends State<CodingToolsPage> {
  late final CodingToolsController _c;
  List<({String family, List<({String value, String label})> options})>
  _langGroups = const [];

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
    final g =
        await (widget.languageLoader?.call() ?? loadClaudeLanguageGroups());
    if (mounted) setState(() => _langGroups = g);
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
              hint: '~/.claude/config.json · primaryApiKey="any"',
              value: _c.applyToClaudePlugin,
              onChanged: _c.busy
                  ? null
                  : (v) => _c.toggleApplyToClaudePlugin(v, texts),
            ),
            SwitchRow(
              key: const ValueKey('skip-onboarding'),
              label: t.t('codingTools.skipOnboarding.title'),
              description: t.t('codingTools.skipOnboarding.desc'),
              hint: '~/.claude.json · hasCompletedOnboarding=true',
              value: _c.skipClaudeOnboarding,
              onChanged: _c.busy
                  ? null
                  : (v) => _c.toggleSkipOnboarding(v, texts),
            ),
            SwitchRow(
              key: const ValueKey('date-rewrite'),
              label: t.t('codingTools.dateRewrite.title'),
              description: t.t('codingTools.dateRewrite.desc'),
              hint: 'middleware · redaction · YYYY/MM/DD → YYYY-MM-DD',
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
              hint: 'settings · proxy · builtin_tool_compat',
              value: _c.btcGlobal,
              onChanged: _c.busy ? null : (v) => _c.toggleBtcGlobal(v, texts),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('codingTools.language.title'),
          description: t.t('codingTools.language.desc'),
          children: [
            _LanguageGroupSelect(
              key: const ValueKey('cli-language'),
              label: t.t('codingTools.language.title'),
              groups: _langGroups,
              value: _c.language,
              onChanged: _c.busy ? null : (v) => _c.setLanguage(v ?? '', texts),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('codingTools.effort.title'),
          description: t.t('codingTools.effort.desc'),
          children: [
            // 「落点」等宽小字：这张卡改的是哪几个键
            // （`CodingToolsSettings.tsx:465`）。开关会动哪个文件、哪个键，
            // 原先界面上完全看不到。
            _LandingHint(
              'claude · effortLevel · codex · model_reasoning_effort',
            ),
            ChoiceRow(
              key: const ValueKey('cli-effort'),
              label: t.t('settings.f_effortLevel'),
              // 首项「—」= 不设置（`CodingToolsSettings.tsx:478-479` 的
              // `__none__` → 写空串）。没有它设过一次就清不回不设置。
              options: ['', ...kEffortOptions],
              labelOf: (v) => v.isEmpty ? '—' : v,
              value: _c.effort,
              onChanged: _c.busy ? null : (v) => _c.setEffort(v, texts),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('codingTools.proxy.title'),
          description: t.t('codingTools.proxy.desc'),
          children: [
            // `CodingToolsSettings.tsx:494`。
            _LandingHint(
              'claude · env.HTTP_PROXY / HTTPS_PROXY / ALL_PROXY · NO_PROXY',
            ),
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

/// 卡片里的「落点」一行：等宽、弱化，写明这张卡改的是哪个文件的哪个键。
/// 开关行上的同名信息走 `SwitchRow.hint`。
class _LandingHint extends StatelessWidget {
  const _LandingHint(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
    child: Text(
      // 路径与键名是标识串，RTL 下不该被重排。
      ltr(text),
      style: AidogType.numSm.copyWith(color: AidogTheme.of(context).c.fg3),
    ),
  );
}

/// 语言下拉：三十多项按语族分组画标题，对齐 React 的 `SelectGroup`
/// （`CodingToolsSettings.tsx:433-450`）。拍平成一个长列表的话，找一门
/// 语言要一路滚到底。分组标题是点不动的灰字（disabled 的菜单项）。
class _LanguageGroupSelect extends StatelessWidget {
  const _LanguageGroupSelect({
    super.key,
    required this.label,
    required this.groups,
    required this.value,
    this.onChanged,
  });

  final String label;
  final List<({String family, List<({String value, String label})> options})>
  groups;
  final String value;
  final ValueChanged<String?>? onChanged;

  /// 候选里找显示名；不在候选里（自定义值）原样返回。
  String _labelOf(String v) {
    for (final g in groups) {
      for (final o in g.options) {
        if (o.value == v) return o.label;
      }
    }
    return v;
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final flat = [for (final g in groups) ...g.options.map((o) => o.value)];
    final hintStyle = AidogType.micro.copyWith(color: theme.c.fg3);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          TileMeta(label),
          DropdownButton<String>(
            value: value.isEmpty ? null : value,
            underline: const SizedBox.shrink(),
            isDense: true,
            isExpanded: true,
            dropdownColor: theme.c.surface2,
            style: AidogType.micro.copyWith(color: theme.c.fg),
            hint: Text('—', style: hintStyle),
            onChanged: onChanged,
            items: [
              // 当前值不在候选里（用户手填的自定义值）也要能显示，
              // 否则 Dropdown 会断言失败。
              if (value.isNotEmpty && !flat.contains(value))
                DropdownMenuItem<String>(
                  value: value,
                  child: Text(_labelOf(value)),
                ),
              for (final g in groups) ...[
                DropdownMenuItem<String>(
                  enabled: false,
                  child: Text(
                    g.family,
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                ),
                for (final o in g.options)
                  DropdownMenuItem<String>(
                    value: o.value,
                    child: Text(o.label),
                  ),
              ],
            ],
          ),
        ],
      ),
    );
  }
}
