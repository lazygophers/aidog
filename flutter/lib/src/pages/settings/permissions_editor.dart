/// Claude 设置页的**权限矩阵编辑器**（I17）—— 对齐 React
/// `src/components/settings/editors/PermissionsSection.tsx::PermissionsEditor`。
///
/// 行为逐条对齐：可视化列表（默认）↔ 裸 JSON 回退双模式；allow / ask / deny
/// 三模式列表按工具组（Bash / Read / … / mcp__ / Agent）分页签管理；defaultMode
/// 下拉与 disableBypassPermissionsMode / disableAutoMode 两个安全开关；
/// 规则模板；全部规则摘要（按模式计数 + 着色清单）。
///
/// 读写同一个 `permissions` 对象（defaultMode / allow / ask / deny / … 键名不变），
/// 空 → null（删键），与 React `syncRules` / `updatePermKey` 同语义。
library;

import 'dart:convert';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../shell/theme.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'schema_config_page.dart' show JsonField;

const List<String> kRuleModes = ['allow', 'ask', 'deny'];

/// 权限默认模式（`PERMISSION_MODES`，i18n key + 中文回落）。
const List<(String, String)> kPermissionModes = [
  ('default', '标准模式'),
  ('acceptEdits', '接受编辑'),
  ('plan', '计划模式'),
  ('auto', '自动模式'),
  ('dontAsk', '不再询问'),
  ('bypassPermissions', '跳过权限'),
];

/// 工具组（`TOOL_GROUPS`）：tool → (i18n key 回落 label, 语法说明, 模板示例)。
class ToolGroup {
  const ToolGroup(this.tool, this.label, this.syntax, this.examples);
  final String tool;
  final String label;
  final String syntax;
  final List<String> examples;
}

const List<ToolGroup> kToolGroups = [
  ToolGroup('Bash', 'Bash / Shell', 'Bash(cmd) / Bash(prefix *) / Bash', [
    'Bash(npm run build)',
    'Bash(npm run *)',
    'Bash(git commit *)',
    'Bash(git * main)',
    'Bash(docker *)',
    'Bash(* --version)',
    'Bash',
  ]),
  ToolGroup(
    'PowerShell',
    'PowerShell',
    'PowerShell(cmd) / PowerShell(prefix *) / PowerShell',
    ['PowerShell(Get-ChildItem *)', 'PowerShell(git commit *)', 'PowerShell'],
  ),
  ToolGroup('Read', 'Read', 'Read(path) — //绝对 / ~/主目录 / /项目根 / ./当前', [
    'Read(./.env)',
    'Read(//**/*.key)',
    'Read(~/.ssh/**)',
    'Read(src/**)',
    'Read(**/.env)',
  ]),
  ToolGroup('Edit', 'Edit / Write', 'Edit(path) — 同 Read 路径规则', [
    'Edit(/src/**/*.ts)',
    'Edit(./config.json)',
    'Edit(/docs/**)',
  ]),
  ToolGroup('WebFetch', 'WebFetch', 'WebFetch(domain:host) / WebFetch', [
    'WebFetch(domain:example.com)',
    'WebFetch',
  ]),
  ToolGroup('mcp__', 'MCP', 'mcp__server__tool / mcp__server__*', [
    'mcp__puppeteer__*',
    'mcp__puppeteer__puppeteer_navigate',
  ]),
  ToolGroup('Agent', 'Agent (子代理)', 'Agent(name)', [
    'Agent(Explore)',
    'Agent(Plan)',
    'Agent(my-custom-agent)',
  ]),
];

/// 一条规则的分组归属（`ruleToolGroup`）：mcp__ 前缀单列，否则取首个字母段。
String ruleToolGroup(String pattern) {
  if (pattern.startsWith('mcp__')) return 'mcp__';
  final m = RegExp(r'^([A-Za-z_]+)').firstMatch(pattern);
  return m?.group(1) ?? '';
}

class PermissionsEditor extends StatefulWidget {
  const PermissionsEditor({
    super.key,
    required this.perms,
    required this.onChanged,
  });

  /// `config.permissions ?? {}`（整份）。
  final Map<String, Object?> perms;

  /// 整份写回；空对象 → null（删键）。
  final ValueChanged<Object?> onChanged;

  @override
  State<PermissionsEditor> createState() => _PermissionsEditorState();
}

class _PermissionsEditorState extends State<PermissionsEditor> {
  String _activeTool = 'Bash';
  String _draftRule = '';
  String _draftMode = 'allow';
  bool _showTemplates = false;
  bool _jsonMode = false;
  String _jsonText = '';
  String? _jsonError;

  /// allow / ask / deny 拍平成统一规则清单（React `rules` 同构）。
  List<(String pattern, String mode)> get _rules => [
    for (final m in kRuleModes)
      for (final p in (widget.perms[m] as List? ?? const [])) ('$p', m),
  ];

  /// 写回整份 permissions（`syncRules`：保留 defaultMode / 安全开关，删空键）。
  void _syncRules(List<(String, String)> updated) {
    final next = <String, Object?>{};
    for (final k in [
      'defaultMode',
      'disableBypassPermissionsMode',
      'disableAutoMode',
    ]) {
      final v = widget.perms[k];
      if (v != null) next[k] = v;
    }
    for (final m in kRuleModes) {
      final list = [
        for (final r in updated)
          if (r.$2 == m) r.$1,
      ];
      if (list.isNotEmpty) next[m] = list;
    }
    widget.onChanged(next.isEmpty ? null : next);
  }

  /// 改一个顶层键（defaultMode / 安全开关）：有值写、无值删；空 → null。
  void _updatePermKey(String key, Object? value) {
    final next = {...widget.perms};
    if (value == null || value == '') {
      next.remove(key);
    } else {
      next[key] = value;
    }
    widget.onChanged(next.isEmpty ? null : next);
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            SmallButton(
              key: const ValueKey('perm-view-visual'),
              label: t.t('settings.permissionsVisualView'),
              active: !_jsonMode,
              onTap: () => setState(() => _jsonMode = false),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              key: const ValueKey('perm-view-json'),
              label: t.t('settings.permissionsJsonView'),
              active: _jsonMode,
              onTap: () {
                setState(() {
                  _jsonMode = true;
                  _jsonText = const JsonEncoder.withIndent('  ')
                      .convert(widget.perms);
                  _jsonError = null;
                });
              },
            ),
          ],
        ),
        _jsonMode ? _jsonPane(t) : _visualPane(t),
      ],
    );
  }

  /// 裸 JSON 回退（React `viewMode === "json"`）。解析错误就地报，不写回。
  Widget _jsonPane(I18nController t) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // A6：React 的 JSON 回退走 `JsonEditor` → `JsonCodeEditor`（行号 / 高亮 /
        // 搜索替换 / 行内标红，`_shared.tsx:210-229`）。Flutter 侧同一件东西是
        // `JsonField`（re_editor），原先这里只是一个 10 行的纯文本框。
        JsonField(
          key: const ValueKey('perm-json'),
          text: _jsonText,
          height: 240,
          error: _jsonError,
          hint: '{ "allow": [], "ask": [], "deny": [], "defaultMode": "default" }',
          onSubmitted: (text) {
            final raw = text.trim();
            try {
              final decoded = raw.isEmpty ? null : jsonDecode(raw);
              setState(() => _jsonError = null);
              if (decoded == null || (decoded is Map && decoded.isEmpty)) {
                widget.onChanged(null);
              } else if (decoded is Map) {
                widget.onChanged(Map<String, Object?>.from(decoded));
              } else {
                setState(() => _jsonError = 'not an object');
              }
            } catch (e) {
              setState(() => _jsonError = '$e');
            }
          },
        ),
      ],
    );
  }

  Widget _visualPane(I18nController t) {
    final theme = AidogTheme.of(context);
    final rules = _rules;
    final active = kToolGroups.firstWhere(
      (g) => g.tool == _activeTool,
      orElse: () => kToolGroups.first,
    );
    final grouped = <String, int>{};
    for (final r in rules) {
      final g = ruleToolGroup(r.$1);
      grouped[g] = (grouped[g] ?? 0) + 1;
    }
    final groupRules = [
      for (var i = 0; i < rules.length; i++)
        if (ruleToolGroup(rules[i].$1) == active.tool) (i, rules[i]),
    ];
    Color modeColor(String m) => switch (m) {
      'deny' => theme.c.bad,
      'ask' => theme.c.peak,
      _ => theme.c.ok,
    };

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // ── defaultMode ──
        SelectRow(
          key: const ValueKey('perm-default-mode'),
          label: t.t('settings.permissionsDefaultMode'),
          labelIcon: Icons.rule,
          options: [for (final m in kPermissionModes) m.$1],
          value: '${widget.perms['defaultMode'] ?? ''}',
          onChanged: (v) =>
              _updatePermKey('defaultMode', v == null || v.isEmpty ? null : v),
          labelOf: (v) {
            final e = kPermissionModes.where((m) => m.$1 == v).firstOrNull;
            return e == null ? v : tOr(t, 'settings.perm.mode_${e.$1}', e.$2);
          },
        ),
        // B8：React 是 `F.hint` 13 · lineHeight 1.6 · paddingLeft 92，
        // deny/ask/allow 三词各按 MODE_COLORS w600 着色
        //（`PermissionsSection.tsx:209-213`）。
        Padding(
          padding: const EdgeInsets.only(left: 92),
          child: Text.rich(
            TextSpan(
              style: editorHintStyle(theme).copyWith(height: 1.6),
              children: [
                TextSpan(
                  text: '${tOr(t, 'settings.perm.priorityLabel', '规则优先级')}: ',
                ),
                for (final m in const ['deny', 'ask', 'allow']) ...[
                  TextSpan(
                    text: tOr(
                      t,
                      'settings.permissions${m[0].toUpperCase()}${m.substring(1)}',
                      m,
                    ),
                    style: TextStyle(
                      color: modeColor(m),
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  if (m != 'allow') const TextSpan(text: ' → '),
                ],
                TextSpan(
                  text: tOr(t, 'settings.perm.priorityNote', '。第一个匹配的规则生效。'),
                ),
              ],
            ),
          ),
        ),
        // ── 安全开关 ──
        SwitchRow(
          key: const ValueKey('perm-disable-bypass'),
          label: tOr(t, 'settings.perm.disableBypass', '禁用绕过模式'),
          value: widget.perms['disableBypassPermissionsMode'] != null,
          onChanged: (v) => _updatePermKey(
            'disableBypassPermissionsMode',
            v ? 'disable' : null,
          ),
        ),
        SwitchRow(
          key: const ValueKey('perm-disable-auto'),
          label: tOr(t, 'settings.perm.disableAuto', '禁用自动模式'),
          value: widget.perms['disableAutoMode'] != null,
          onChanged: (v) =>
              _updatePermKey('disableAutoMode', v ? 'disable' : null),
        ),
        // ── 工具组页签 ──
        // A1/A2/B6：React 是一条 1px 底线的 tab 栏，active 项带 2px accent 下边框，
        // 计数是独立徽标（`PermissionsSection.tsx:234-256`）。
        Container(
          decoration: BoxDecoration(
            border: Border(bottom: BorderSide(color: theme.c.line)),
          ),
          // 7 个页签在窄窗口下放不满一行，React 靠 flex 溢出、Flutter 会抛
          // overflow —— 横向滚动是等价的降级，不改视觉节奏。
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final g in kToolGroups)
                  _ToolTab(
                    key: ValueKey('perm-tab-${g.tool}'),
                    label: tOr(t, 'settings.perm.toolLabel_${g.tool}', g.label),
                    count: grouped[g.tool] ?? 0,
                    active: _activeTool == g.tool,
                    onTap: () => setState(() {
                      _activeTool = g.tool;
                      _showTemplates = false;
                    }),
                  ),
              ],
            ),
          ),
        ),
        // B7：React 是 `F.hint` 13 mono · pad 8/12 · bg-glass 底 · r-sm，
        // 工具名 w600 accent 色（`PermissionsSection.tsx:263-268`）。
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            color: theme.c.surface,
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Text.rich(
            TextSpan(
              style: AidogType.numSm.copyWith(
                fontSize: 13,
                letterSpacing: 0,
                height: 1.5,
                color: theme.c.fg3,
              ),
              children: [
                TextSpan(
                  text: tOr(
                    t,
                    'settings.perm.toolLabel_${active.tool}',
                    active.label,
                  ),
                  style: TextStyle(
                    color: theme.c.accentText,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                TextSpan(
                  text:
                      ': ${tOr(t, 'settings.perm.syntax_${active.tool}', active.syntax)}',
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: AidogSpace.sxs),
        // ── 当前组的规则 ──
        if (groupRules.isEmpty)
          // B9：React 是 `F.hint` 13 · padding 12px 0 · 居中
          //（`PermissionsSection.tsx:275-277`）。
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 12),
            child: Text(
              '${tOr(t, 'settings.perm.noRulesPrefix', '暂无')} '
              '${tOr(t, 'settings.perm.toolLabel_${active.tool}', active.label)} '
              '${tOr(t, 'settings.perm.noRulesSuffix', '规则。使用下方输入框添加。')}',
              textAlign: TextAlign.center,
              style: editorHintStyle(theme),
            ),
          )
        else
          for (final (idx, r) in groupRules)
            Padding(
              key: ValueKey('perm-rule-$idx'),
              padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
              child: Row(
                children: [
                  Expanded(
                    child: PlainTextField(
                      key: ValueKey('perm-rule-$idx-pattern'),
                      value: r.$1,
                      // B1：editors 域输入框 15 + pad 10/14（`_shared.tsx:285`）。
                      mono: true,
                      fontSize: kEditorInputFontSize,
                      contentPadding: kEditorInputPad,
                      onSubmitted: (v) {
                        final next = [...rules];
                        next[idx] = (v, r.$2);
                        _syncRules(next);
                      },
                    ),
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                  _ModeSelect(
                    value: r.$2,
                    color: modeColor(r.$2),
                    labelOf: (m) => t.t(
                      'settings.permissions${m[0].toUpperCase()}${m.substring(1)}',
                    ),
                    onChanged: (m) {
                      final next = [...rules];
                      next[idx] = (r.$1, m);
                      _syncRules(next);
                    },
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  SmallButton(
                    key: ValueKey('perm-rule-$idx-del'),
                    label: '×',
                    onTap: () => _syncRules([...rules]..removeAt(idx)),
                  ),
                ],
              ),
            ),
        // ── 加规则 ──
        Row(
          children: [
            Expanded(
              child: PlainTextField(
                key: const ValueKey('perm-add-pattern'),
                value: _draftRule,
                hint: active.examples.first,
                // B1：同上（`PermissionsSection.tsx:318`）。
                mono: true,
                fontSize: kEditorInputFontSize,
                contentPadding: kEditorInputPad,
                onChanged: (v) => setState(() => _draftRule = v),
                onSubmitted: _addRule,
              ),
            ),
            const SizedBox(width: AidogSpace.ssm),
            _ModeSelect(
              value: _draftMode,
              color: modeColor(_draftMode),
              labelOf: (m) => t.t(
                'settings.permissions${m[0].toUpperCase()}${m.substring(1)}',
              ),
              onChanged: (m) => setState(() => _draftMode = m),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              key: const ValueKey('perm-add-rule'),
              label: '+',
              onTap: () => _addRule(_draftRule),
            ),
            const SizedBox(width: AidogSpace.sxs),
            // A5：React 模板是带点击遮罩的浮层（`PermissionsSection.tsx:340-348`），
            // 不是把后面内容顶下去的内联块。
            AnchoredMenu(
              open: _showTemplates,
              onDismiss: () => setState(() => _showTemplates = false),
              above: false,
              minWidth: 320,
              maxHeight: 300,
              menuBuilder: (_) => _templatesMenu(t, theme),
              anchor: SmallButton(
                key: const ValueKey('perm-templates'),
                label: tOr(t, 'settings.perm.ruleTemplates', '规则模板'),
                active: _showTemplates,
                onTap: () => setState(() => _showTemplates = !_showTemplates),
              ),
            ),
          ],
        ),
        // ── 全部规则摘要 ──
        if (rules.isNotEmpty)
          Container(
            key: const ValueKey('perm-summary'),
            margin: const EdgeInsets.only(top: AidogSpace.ssm),
            // B10：React 是 pad 10/12 + bg-glass（= surface），不是 all 6 + surface2
            //（`PermissionsSection.tsx:394-397`）。
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            decoration: BoxDecoration(
              color: theme.c.surface,
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                // A4：三档计数各缀一枚 12px 图标并分色（`PermissionsSection.tsx:400-402`）。
                Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: Wrap(
                    spacing: 12,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      Text(
                        '${tOr(t, 'settings.perm.totalRulesPrefix', '共')} '
                        '${rules.length} '
                        '${tOr(t, 'settings.perm.totalRulesSuffix', '条规则')}',
                        style: editorHintStyle(theme),
                      ),
                      for (final (mode, icon) in const [
                        ('deny', Icons.close),
                        ('ask', null),
                        ('allow', Icons.check),
                      ])
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            if (icon != null) ...[
                              Icon(icon, size: 12, color: modeColor(mode)),
                              const SizedBox(width: 4),
                            ] else
                              Text(
                                '? ',
                                style: editorHintStyle(theme)
                                    .copyWith(color: modeColor(mode)),
                              ),
                            Text(
                              '$mode: ${rules.where((r) => r.$2 == mode).length}',
                              style: editorHintStyle(theme)
                                  .copyWith(color: modeColor(mode)),
                            ),
                          ],
                        ),
                    ],
                  ),
                ),
                // A3/B11：每行 3px 左色条 + 8% 底 + pad 3/8 + r-sm，
                // 行内三段字号各异（`PermissionsSection.tsx:406-423`）。
                for (final r in rules)
                  Container(
                    margin: const EdgeInsets.only(bottom: 2),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 3,
                    ),
                    decoration: BoxDecoration(
                      color: modeColor(r.$2).withValues(alpha: 0.03),
                      borderRadius: BorderRadius.circular(AidogRadius.sm),
                      border: Border(
                        left: BorderSide(color: modeColor(r.$2), width: 3),
                      ),
                    ),
                    child: Row(
                      children: [
                        SizedBox(
                          width: 32,
                          child: Text(
                            r.$2.toUpperCase(),
                            style: AidogType.label.copyWith(
                              fontSize: 10,
                              fontWeight: FontWeight.w600,
                              color: modeColor(r.$2),
                            ),
                          ),
                        ),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(
                            ltr(r.$1),
                            overflow: TextOverflow.ellipsis,
                            style: AidogType.numSm.copyWith(
                              fontSize: 12,
                              letterSpacing: 0,
                              color: theme.c.fg,
                            ),
                          ),
                        ),
                        const SizedBox(width: 6),
                        Text(
                          ruleToolGroup(r.$1),
                          style: AidogType.label.copyWith(
                            fontSize: 10,
                            color: theme.c.fg3,
                          ),
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
      ],
    );
  }

  /// 规则模板浮层的内容（React `TOOL_GROUPS.map`，`PermissionsSection.tsx:350-373`）。
  Widget _templatesMenu(I18nController t, AidogTheme theme) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    mainAxisSize: MainAxisSize.min,
    children: [
      for (final g in kToolGroups)
        Padding(
          key: ValueKey('perm-tpl-${g.tool}'),
          padding: const EdgeInsets.only(bottom: AidogSpace.s_8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              // C8：工具名 12 w600 accent + 语法串 10 mono fg3
              //（`PermissionsSection.tsx:352-356`），原先只有工具名一档。
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    tOr(t, 'settings.perm.toolLabel_${g.tool}', g.label),
                    style: AidogType.label.copyWith(
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      color: theme.c.accentText,
                    ),
                  ),
                  const SizedBox(width: 4),
                  Flexible(
                    child: Text(
                      tOr(t, 'settings.perm.syntax_${g.tool}', g.syntax),
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.numSm.copyWith(
                        fontSize: 10,
                        letterSpacing: 0,
                        color: theme.c.fg3,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: AidogSpace.sxs),
              Wrap(
                spacing: AidogSpace.sxs,
                runSpacing: AidogSpace.sxs,
                children: [
                  for (final ex in g.examples)
                    SmallButton(
                      label: ex,
                      fontSize: 13,
                      padding: (8, 3),
                      onTap: () => setState(() {
                        _draftRule = ex;
                        _showTemplates = false;
                      }),
                    ),
                ],
              ),
            ],
          ),
        ),
    ],
  );

  void _addRule(String raw) {
    final v = raw.trim();
    if (v.isEmpty) return;
    _syncRules([..._rules, (v, _draftMode)]);
    setState(() => _draftRule = '');
  }
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}

/// 模式下拉（React `ModeSelect`：按模式着色）。
class _ModeSelect extends StatelessWidget {
  const _ModeSelect({
    required this.value,
    required this.color,
    required this.labelOf,
    required this.onChanged,
  });

  final String value;
  final Color color;
  final String Function(String mode) labelOf;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    // B5：React 是 `F.small` 12 w600 · minWidth 72 · pad 4/8 · r-sm ·
    // 底 12% · 字 mode 色 · 边 35%（`PermissionsSection.tsx:131-138`）。
    final style = AidogType.label.copyWith(
      fontSize: 12,
      fontWeight: FontWeight.w600,
      color: color,
    );
    return Container(
      constraints: const BoxConstraints(minWidth: 72),
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.07),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        border: Border.all(color: color.withValues(alpha: 0.21)),
      ),
      child: DropdownButton<String>(
        value: value,
        isDense: true,
        items: [
          for (final m in kRuleModes)
            DropdownMenuItem(
              value: m,
              child: Text(labelOf(m), style: style),
            ),
        ],
        selectedItemBuilder: (_) => [
          for (final m in kRuleModes) Text(labelOf(m), style: style),
        ],
        onChanged: (m) {
          if (m != null) onChanged(m);
        },
        style: style,
        dropdownColor: theme.c.surface2,
        underline: const SizedBox.shrink(),
        icon: Icon(Icons.arrow_drop_down, size: 16, color: color),
        iconSize: 16,
      ),
    );
  }
}

/// 权限工具组页签（React `PermissionsSection.tsx:234-258`）：
/// pad 6/12 · `F.small` 12 · active w600 accent 字 + 2px accent 下边框；
/// 计数是独立徽标（10 w600 · pad 1/5 · r8）。
class _ToolTab extends StatelessWidget {
  const _ToolTab({
    super.key,
    required this.label,
    required this.count,
    required this.active,
    required this.onTap,
  });

  final String label;
  final int count;
  final bool active;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return InkWell(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        decoration: BoxDecoration(
          border: Border(
            bottom: BorderSide(
              width: 2,
              color: active ? theme.c.accentText : Colors.transparent,
            ),
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              label,
              style: AidogType.label.copyWith(
                fontSize: 12,
                fontWeight: active ? FontWeight.w600 : FontWeight.w400,
                color: active ? theme.c.accentText : theme.c.fg2,
              ),
            ),
            if (count > 0) ...[
              const SizedBox(width: 4),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                decoration: BoxDecoration(
                  color: active ? theme.c.accent : theme.c.surface,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Text(
                  '$count',
                  style: AidogType.label.copyWith(
                    fontSize: 10,
                    fontWeight: FontWeight.w600,
                    color: active ? AidogColors.light.surface : theme.c.fg3,
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
