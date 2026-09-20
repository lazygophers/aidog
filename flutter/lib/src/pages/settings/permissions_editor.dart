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
    'Bash(npm run build)', 'Bash(npm run *)', 'Bash(git commit *)',
    'Bash(git * main)', 'Bash(docker *)', 'Bash(* --version)', 'Bash',
  ]),
  ToolGroup('PowerShell', 'PowerShell', 'PowerShell(cmd) / PowerShell(prefix *) / PowerShell', [
    'PowerShell(Get-ChildItem *)', 'PowerShell(git commit *)', 'PowerShell',
  ]),
  ToolGroup('Read', 'Read', 'Read(path) — //绝对 / ~/主目录 / /项目根 / ./当前', [
    'Read(./.env)', 'Read(//**/*.key)', 'Read(~/.ssh/**)', 'Read(src/**)',
    'Read(**/.env)',
  ]),
  ToolGroup('Edit', 'Edit / Write', 'Edit(path) — 同 Read 路径规则', [
    'Edit(/src/**/*.ts)', 'Edit(./config.json)', 'Edit(/docs/**)',
  ]),
  ToolGroup('WebFetch', 'WebFetch', 'WebFetch(domain:host) / WebFetch', [
    'WebFetch(domain:example.com)', 'WebFetch',
  ]),
  ToolGroup('mcp__', 'MCP', 'mcp__server__tool / mcp__server__*', [
    'mcp__puppeteer__*', 'mcp__puppeteer__puppeteer_navigate',
  ]),
  ToolGroup('Agent', 'Agent (子代理)', 'Agent(name)', [
    'Agent(Explore)', 'Agent(Plan)', 'Agent(my-custom-agent)',
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
          for (final p in (widget.perms[m] as List? ?? const []))
            ('$p', m),
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
                  _jsonText =
                      const JsonEncoder.withIndent('  ').convert(widget.perms);
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
        PlainTextField(
          key: const ValueKey('perm-json'),
          value: _jsonText,
          maxLines: 10,
          hint: '{ "allow": [], "ask": [], "deny": [], "defaultMode": "default" }',
          onSubmitted: (text) {
            final raw = text.trim();
            try {
              final decoded = raw.isEmpty ? null : jsonDecode(raw);
              setState(() => _jsonError = null);
              if (decoded == null ||
                  (decoded is Map && decoded.isEmpty)) {
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
        if (_jsonError != null) ErrorNote(text: _jsonError!),
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
          options: [for (final m in kPermissionModes) m.$1],
          value: '${widget.perms['defaultMode'] ?? ''}',
          onChanged: (v) =>
              _updatePermKey('defaultMode', v == null || v.isEmpty ? null : v),
          labelOf: (v) {
            final e = kPermissionModes.where((m) => m.$1 == v).firstOrNull;
            return e == null ? v : tOr(t, 'settings.perm.mode_${e.$1}', e.$2);
          },
        ),
        Text(
          '${tOr(t, 'settings.perm.priorityLabel', '规则优先级')}: '
          '${tOr(t, 'settings.permissionsDeny', 'deny')} → '
          '${tOr(t, 'settings.permissionsAsk', 'ask')} → '
          '${tOr(t, 'settings.permissionsAllow', 'allow')}'
          '${tOr(t, 'settings.perm.priorityNote', '。第一个匹配的规则生效。')}',
          style: AidogType.micro.copyWith(color: theme.c.fg3),
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
        Wrap(
          spacing: AidogSpace.sxs,
          children: [
            for (final g in kToolGroups)
              SmallButton(
                key: ValueKey('perm-tab-${g.tool}'),
                label: tOr(t, 'settings.perm.toolLabel_${g.tool}', g.label) +
                    ((grouped[g.tool] ?? 0) > 0
                        ? ' ${grouped[g.tool]}'
                        : ''),
                active: _activeTool == g.tool,
                onTap: () => setState(() {
                  _activeTool = g.tool;
                  _showTemplates = false;
                }),
              ),
          ],
        ),
        Container(
          padding: const EdgeInsets.all(AidogSpace.ssm),
          decoration: BoxDecoration(
            color: theme.c.surface2,
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Text(
            '${tOr(t, 'settings.perm.toolLabel_${active.tool}', active.label)}: '
            '${tOr(t, 'settings.perm.syntax_${active.tool}', active.syntax)}',
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
        ),
        const SizedBox(height: AidogSpace.sxs),
        // ── 当前组的规则 ──
        if (groupRules.isEmpty)
          Text(
            '${tOr(t, 'settings.perm.noRulesPrefix', '暂无')} '
            '${tOr(t, 'settings.perm.toolLabel_${active.tool}', active.label)} '
            '${tOr(t, 'settings.perm.noRulesSuffix', '规则。使用下方输入框添加。')}',
            style: AidogType.micro.copyWith(color: theme.c.fg3),
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
                    labelOf: (m) => t.t('settings.permissions${m[0].toUpperCase()}${m.substring(1)}'),
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
                    onTap: () =>
                        _syncRules([...rules]..removeAt(idx)),
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
                onChanged: (v) => setState(() => _draftRule = v),
                onSubmitted: _addRule,
              ),
            ),
            const SizedBox(width: AidogSpace.ssm),
            _ModeSelect(
              value: _draftMode,
              color: modeColor(_draftMode),
              labelOf: (m) => t.t('settings.permissions${m[0].toUpperCase()}${m.substring(1)}'),
              onChanged: (m) => setState(() => _draftMode = m),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              key: const ValueKey('perm-add-rule'),
              label: '+',
              onTap: () => _addRule(_draftRule),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              key: const ValueKey('perm-templates'),
              label: tOr(t, 'settings.perm.ruleTemplates', '规则模板'),
              active: _showTemplates,
              onTap: () => setState(() => _showTemplates = !_showTemplates),
            ),
          ],
        ),
        if (_showTemplates)
          for (final g in kToolGroups)
            Padding(
              key: ValueKey('perm-tpl-${g.tool}'),
              padding: const EdgeInsets.only(top: AidogSpace.sxs),
              child: Wrap(
                spacing: AidogSpace.sxs,
                runSpacing: AidogSpace.sxs,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  Text(
                    tOr(t, 'settings.perm.toolLabel_${g.tool}', g.label),
                    style: AidogType.micro.copyWith(color: theme.c.accent),
                  ),
                  for (final ex in g.examples)
                    SmallButton(
                      label: ex,
                      onTap: () => setState(() {
                        _draftRule = ex;
                        _showTemplates = false;
                      }),
                    ),
                ],
              ),
            ),
        // ── 全部规则摘要 ──
        if (rules.isNotEmpty)
          Container(
            key: const ValueKey('perm-summary'),
            margin: const EdgeInsets.only(top: AidogSpace.ssm),
            padding: const EdgeInsets.all(AidogSpace.ssm),
            decoration: BoxDecoration(
              color: theme.c.surface2,
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  '${tOr(t, 'settings.perm.totalRulesPrefix', '共')} '
                  '${rules.length} '
                  '${tOr(t, 'settings.perm.totalRulesSuffix', '条规则')}   '
                  'deny: ${rules.where((r) => r.$2 == 'deny').length}   '
                  'ask: ${rules.where((r) => r.$2 == 'ask').length}   '
                  'allow: ${rules.where((r) => r.$2 == 'allow').length}',
                  style: AidogType.micro.copyWith(color: theme.c.fg3),
                ),
                for (final r in rules)
                  Padding(
                    padding: const EdgeInsets.symmetric(vertical: 1),
                    child: Text(
                      ltr('${r.$2}  ${r.$1}  ${ruleToolGroup(r.$1)}'),
                      style: AidogType.micro.copyWith(color: modeColor(r.$2)),
                    ),
                  ),
              ],
            ),
          ),
      ],
    );
  }

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
    return DropdownButton<String>(
      value: value,
      items: [
        for (final m in kRuleModes)
          DropdownMenuItem(
            value: m,
            child: Text(
              labelOf(m),
              style: AidogType.micro.copyWith(color: color),
            ),
          ),
      ],
      onChanged: (m) {
        if (m != null) onChanged(m);
      },
      style: AidogType.micro.copyWith(color: theme.c.fg),
      underline: const SizedBox.shrink(),
      iconSize: 16,
    );
  }
}
