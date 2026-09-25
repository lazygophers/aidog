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

/// 卡与卡之间的间距：React 页容器 `gap: 20`（`CodingToolsSettings.tsx:366`）。
const double _cardGap = 20;

/// 代理 URL 的四个本地端口预设（Clash 7890 / 通用 8080 / SS 1080 /
/// Clash Verge 7899），对齐 React 的 `PROXY_URL_PRESETS`
/// （`CodingToolsSettings.tsx:74`）+ `<datalist>`（`:516-520`）。
const List<String> kProxyUrlPresets = [
  'http://127.0.0.1:7890',
  'http://127.0.0.1:8080',
  'http://127.0.0.1:1080',
  'http://127.0.0.1:7899',
];

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
        // React 加载态继承正文字号（`CodingToolsSettings.tsx:362`）。
        children: [CenteredNote(text: t.t('status.loading'), fontSize: 13)],
      );
    }
    return SettingsPageBody(
      title: t.t('codingTools.cliIntegrationTitle'),
      children: [
        // 说明卡（`CodingToolsSettings.tsx:366-373`）。
        Padding(
          padding: const EdgeInsets.only(bottom: _cardGap),
          // React 这一页的卡全挂 `hover-lift`（`CodingToolsSettings.tsx:49,93`）。
          child: HoverLift(
            child: Tile(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  t.t('codingTools.cliIntegrationTitle'),
                  style: AidogType.label.copyWith(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: AidogTheme.of(context).c.fg,
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Text(
                    t.t('codingTools.introDesc'),
                    style: AidogType.caption.copyWith(
                      fontSize: 12,
                      color: AidogTheme.of(context).c.fg2,
                    ),
                  ),
                ),
              ],
            ),
          ),
          ),
        ),
        // 四张一开关一卡（`CodingToolsSettings.tsx:374-415` 的 ToggleCard），
        // 等宽 hint 是各开关的「落点」。文字区 `paddingRight: 16`（`:99`）。
        ToggleCard(
          key: const ValueKey('apply-to-claude-plugin'),
          bottomGap: _cardGap,
          gap: 16,
          hoverLift: true,
          label: t.t('codingTools.applyPlugin.title'),
          descriptions: [t.t('codingTools.applyPlugin.desc')],
          hint: '~/.claude/config.json · primaryApiKey="any"',
          value: _c.applyToClaudePlugin,
          onChanged: _c.busy
              ? null
              : (v) => _c.toggleApplyToClaudePlugin(v, texts),
        ),
        ToggleCard(
          key: const ValueKey('skip-onboarding'),
          bottomGap: _cardGap,
          gap: 16,
          hoverLift: true,
          label: t.t('codingTools.skipOnboarding.title'),
          descriptions: [t.t('codingTools.skipOnboarding.desc')],
          hint: '~/.claude.json · hasCompletedOnboarding=true',
          value: _c.skipClaudeOnboarding,
          onChanged: _c.busy ? null : (v) => _c.toggleSkipOnboarding(v, texts),
        ),
        ToggleCard(
          key: const ValueKey('date-rewrite'),
          bottomGap: _cardGap,
          gap: 16,
          hoverLift: true,
          label: t.t('codingTools.dateRewrite.title'),
          descriptions: [t.t('codingTools.dateRewrite.desc')],
          hint: 'middleware · redaction · YYYY/MM/DD → YYYY-MM-DD',
          value: _c.dateRewriteEnabled == true,
          // 规则还没读到 / 不存在 → 开关不响应（`dateRewriteDisabled`）。
          onChanged: _c.dateRewriteDisabled
              ? null
              : (v) => _c.toggleDateRewrite(v, texts),
        ),
        // 内置工具兼容总开关：与「设置 → 系统」的同名开关同源（同一 setting）。
        ToggleCard(
          key: const ValueKey('btc-global'),
          bottomGap: _cardGap,
          gap: 16,
          hoverLift: true,
          label: t.t('proxy.btcGlobal'),
          descriptions: [t.t('proxy.btcGlobalDesc')],
          hint: 'settings · proxy · builtin_tool_compat',
          value: _c.btcGlobal,
          onChanged: _c.busy ? null : (v) => _c.toggleBtcGlobal(v, texts),
        ),
        // 语言卡：标题左、下拉右（`CodingToolsSettings.tsx:419-451`）。
        Padding(
          padding: const EdgeInsets.only(bottom: _cardGap),
          child: HoverLift(
            child: Tile(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        t.t('codingTools.language.title'),
                        style: AidogType.label.copyWith(
                          fontSize: 14,
                          fontWeight: FontWeight.w600,
                          color: AidogTheme.of(context).c.fg,
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(top: 2),
                        child: Text(
                          t.t('codingTools.language.desc'),
                          style: AidogType.caption.copyWith(
                            fontSize: 12,
                            color: AidogTheme.of(context).c.fg2,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                // 卡内左右 `gap: 16`（`CodingToolsSettings.tsx:424`）。
                const SizedBox(width: 16),
                _LanguageGroupSelect(
                  key: const ValueKey('cli-language'),
                  groups: _langGroups,
                  value: _c.language,
                  onChanged: _c.busy
                      ? null
                      : (v) => _c.setLanguage(v ?? '', texts),
                ),
              ],
            ),
          ),
          ),
        ),
        // 努力级别卡：标题 + 落点 hint 左、下拉右
        //（`CodingToolsSettings.tsx:454-485`），不是平铺 chips。
        Padding(
          padding: const EdgeInsets.only(bottom: _cardGap),
          child: HoverLift(
            child: Tile(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        t.t('codingTools.effort.title'),
                        style: AidogType.label.copyWith(
                          fontSize: 14,
                          fontWeight: FontWeight.w600,
                          color: AidogTheme.of(context).c.fg,
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(top: 2),
                        child: Text(
                          t.t('codingTools.effort.desc'),
                          style: AidogType.caption.copyWith(
                            fontSize: 12,
                            color: AidogTheme.of(context).c.fg2,
                          ),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(top: 6),
                        child: Text(
                          ltr(
                            'claude · effortLevel · codex · model_reasoning_effort',
                          ),
                          style: AidogType.numSm.copyWith(
                            fontSize: 11,
                            color: AidogTheme.of(context).c.fg3,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: AidogSpace.smd),
                // 首项「—」= 不设置（`__none__` → 写空串）。
                // 没有它设过一次就清不回不设置。
                InlineSelect<String>(
                  key: const ValueKey('cli-effort'),
                  value: _c.effort,
                  options: ['', ...kEffortOptions],
                  width: 120,
                  labelOf: (v) => v.isEmpty ? '—' : v,
                  onChanged: _c.busy
                      ? null
                      : (v) => _c.setEffort(v ?? '', texts),
                ),
              ],
            ),
          ),
          ),
        ),
        // 代理卡：两列 grid（label 11 w600 + Input 13），预设走不可见的
        // datalist（`CodingToolsSettings.tsx:497-540`），不再平铺成常驻 chips。
        Padding(
          padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
          child: Tile(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  t.t('codingTools.proxy.title'),
                  style: AidogType.label.copyWith(
                    fontSize: 14,
                    fontWeight: FontWeight.w600,
                    color: AidogTheme.of(context).c.fg,
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.only(top: 2),
                  child: Text(
                    t.t('codingTools.proxy.desc'),
                    style: AidogType.caption.copyWith(
                      fontSize: 12,
                      color: AidogTheme.of(context).c.fg2,
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.only(top: 6),
                  child: Text(
                    ltr(
                      'claude · env.HTTP_PROXY / HTTPS_PROXY / ALL_PROXY · NO_PROXY',
                    ),
                    style: AidogType.numSm.copyWith(
                      fontSize: 11,
                      color: AidogTheme.of(context).c.fg3,
                    ),
                  ),
                ),
                const SizedBox(height: AidogSpace.smd),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            t.t('codingTools.proxy.title'),
                            style: AidogType.caption.copyWith(
                              fontSize: 11,
                              fontWeight: FontWeight.w600,
                              color: AidogTheme.of(context).c.fg2,
                            ),
                          ),
                          const SizedBox(height: 4),
                          PlainTextField(
                            key: const ValueKey('cli-proxy-url'),
                            value: _c.proxyDraft.url,
                            hint: 'http://host:port',
                            onChanged: _c.setProxyUrl,
                            onSubmitted: (_) => _c.commitProxy(texts),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: AidogSpace.smd),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            'NO_PROXY',
                            style: AidogType.numSm.copyWith(
                              fontSize: 11,
                              fontWeight: FontWeight.w600,
                              color: AidogTheme.of(context).c.fg2,
                            ),
                          ),
                          const SizedBox(height: 4),
                          PlainTextField(
                            key: const ValueKey('cli-proxy-no'),
                            value: _c.proxyDraft.no,
                            hint: 'localhost,127.0.0.1,*.local',
                            onChanged: _c.setProxyNo,
                            onSubmitted: (_) => _c.commitProxy(texts),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
        // 常驻错误卡：danger 边 + 底三行（`CodingToolsSettings.tsx:545-569`）。
        if (_c.error.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
            child: Container(
              padding: const EdgeInsets.all(AidogSpace.smd),
              decoration: BoxDecoration(
                color: AidogTheme.of(context).c.bad.withValues(alpha: 0.08),
                border: Border.all(color: AidogTheme.of(context).c.bad),
                borderRadius: BorderRadius.circular(AidogRadius.md),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    t.t('codingTools.errorTitle'),
                    style: AidogType.label.copyWith(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: AidogTheme.of(context).c.bad,
                    ),
                  ),
                  Text(
                    ltr(_c.error),
                    style: AidogType.numSm.copyWith(
                      fontSize: 12,
                      color: AidogTheme.of(context).c.bad,
                    ),
                  ),
                  Text(
                    t.t('codingTools.errorHint'),
                    style: AidogType.caption.copyWith(
                      fontSize: 12,
                      color: AidogTheme.of(context).c.bad,
                    ),
                  ),
                ],
              ),
            ),
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

/// 语言下拉：三十多项按语族分组画标题，对齐 React 的 `SelectGroup`
/// （`CodingToolsSettings.tsx:433-450`）。拍平成一个长列表的话，找一门
/// 语言要一路滚到底。分组标题是点不动的灰字（disabled 的菜单项）。
class _LanguageGroupSelect extends StatelessWidget {
  const _LanguageGroupSelect({
    super.key,
    required this.groups,
    required this.value,
    this.onChanged,
  });

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
    return SizedBox(
      width: 200,
      child: DropdownButton<String>(
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
              DropdownMenuItem<String>(value: o.value, child: Text(o.label)),
          ],
        ],
      ),
    );
  }
}
