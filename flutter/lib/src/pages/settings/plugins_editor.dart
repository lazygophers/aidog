/// 插件配置的可视化编辑器 —— 对齐
/// `src/components/settings/editors/PluginsSection.tsx`。
///
/// 四块，顺序与 React 一致：
///   1. Enabled Plugins  —— `enabledPlugins`：`plugin@marketplace` → 开/关
///   2. Extra Marketplaces —— `extraKnownMarketplaces`：命名市场源 + 来源编辑器
///   3. Plugin Configs   —— `pluginConfigs`：按插件 ID 的 JSON
///   4. Skipped          —— `skippedPlugins` / `skippedMarketplaces` 两个字符串列表
///
/// 这四个字段在 schema 里全是 `skipGui`，原先在 Flutter 侧一个都画不出来。
library;

import 'dart:convert';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'sandbox_editor.dart' show SandboxTagList;

/// 市场来源的九种形态。顺序与 React 的 `MARKETPLACE_SOURCE_TYPES` 一致。
const _sourceTypes = <String>[
  'github',
  'git',
  'url',
  'npm',
  'file',
  'directory',
  'settings',
  'hostPattern',
  'pathPattern',
];

/// 下拉里显示的人话名。React 的 `SOURCE_TYPE_LABELS`：**原样英文，不接 i18n**。
const _sourceTypeLabels = <String, String>{
  'github': 'GitHub',
  'git': 'Git URL',
  'url': 'URL (marketplace.json)',
  'npm': 'NPM Package',
  'file': 'File (marketplace.json)',
  'directory': 'Directory',
  'settings': 'Inline Settings',
  'hostPattern': 'Host Pattern (regex)',
  'pathPattern': 'Path Pattern (regex)',
};

/// 每种来源自己的字段。`(key, label, placeholder, required)`，同 `SOURCE_FIELDS`。
const _sourceFields = <String, List<(String, String, String, bool)>>{
  'github': [
    ('repo', 'Repository', 'owner/repo', true),
    ('ref', 'Ref (branch/tag/sha)', 'main', false),
    ('path', 'Subdirectory', 'marketplace', false),
  ],
  'git': [
    ('url', 'Git URL', 'https://git.example.com/plugins.git', true),
    ('ref', 'Ref (branch/tag/sha)', 'main', false),
    ('path', 'Subdirectory', 'marketplace', false),
  ],
  'url': [
    (
      'url',
      'Marketplace JSON URL',
      'https://plugins.example.com/marketplace.json',
      true,
    ),
  ],
  'npm': [('package', 'NPM Package', '@acme-corp/claude-plugins', true)],
  'file': [
    ('path', 'File Path', '/usr/local/share/claude/marketplace.json', true),
  ],
  'directory': [
    ('path', 'Directory Path', '/usr/local/share/claude/plugins', true),
  ],
  'settings': [('name', 'Marketplace Name', 'team-tools', true)],
  'hostPattern': [
    ('hostPattern', 'Host Pattern (regex)', r'^github\.example\.com$', true),
  ],
  'pathPattern': [
    ('pathPattern', 'Path Pattern (regex)', '^/opt/approved/', true),
  ],
};

/// 一个市场来源的编辑器（类型下拉 + 类型专属字段 + skipLfs / autoUpdate 开关）。
class MarketplaceSourceEditor extends StatelessWidget {
  const MarketplaceSourceEditor({
    super.key,
    required this.source,
    required this.onChanged,
    this.idPrefix = 'mkt',
    this.compact = false,
  });

  final Map<String, Object?> source;
  final ValueChanged<Map<String, Object?>> onChanged;

  /// widget key 前缀，嵌套的 inline plugins 用得上。
  final String idPrefix;

  /// 嵌在别人里面时用（React 的 `compact`）：省掉末尾那条分隔线，
  /// 否则每个内联插件下面都会多一条横杠。
  final bool compact;

  /// `settings` 源的内联插件清单。
  List<Map<String, Object?>> get _plugins => [
    for (final e in (source['plugins'] as List? ?? const []))
      if (e is Map) Map<String, Object?>.from(e),
  ];

  /// 写回插件清单：空清单删键（React 的 `plugs.length > 0 ? plugs : undefined`）。
  ///
  /// 不走 [_setField]：那条会把所有空串 / false 的键一并清掉，对整棵 plugins
  /// 子树不适用（内联插件的 name 允许暂时为空，用户刚点「+ Plugin」时就是空的）。
  void _setPlugins(List<Map<String, Object?>> next) {
    final map = {...source};
    if (next.isEmpty) {
      map.remove('plugins');
    } else {
      map['plugins'] = next;
    }
    onChanged(map);
  }

  String get _type => '${source['source'] ?? 'github'}';

  /// 空串当作「没填」删键，与 React 的 `val || undefined` 一致。
  void _setField(String key, Object? val) {
    final next = {...source, key: val};
    next.removeWhere((_, v) => v == null || v == false || v == '');
    next['source'] = _type;
    onChanged(next);
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final fields = _sourceFields[_type] ?? const [];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        SelectRow(
          key: ValueKey('$idPrefix-type'),
          label: 'Type',
          options: _sourceTypes,
          value: _type,
          labelOf: (o) => _sourceTypeLabels[o] ?? o,
          // 换类型只保留 source 本身，类型专属字段全清（React 同规则）。
          onChanged: (v) => onChanged({'source': v ?? 'github'}),
        ),
        for (final (key, label, placeholder, required) in fields)
          TextRow(
            key: ValueKey('$idPrefix-$key'),
            label: required ? '$label *' : label,
            hint: placeholder,
            value: '${source[key] ?? ''}',
            onSubmitted: (v) => _setField(key, v.trim()),
          ),
        if (_type == 'github' || _type == 'git')
          SwitchRow(
            key: ValueKey('$idPrefix-skip-lfs'),
            label: 'skipLfs',
            description: t.t('settings.plugins.skipLfs'),
            value: source['skipLfs'] == true,
            onChanged: (v) => _setField('skipLfs', v),
          ),
        if (_type == 'url')
          TextRow(
            key: ValueKey('$idPrefix-headers'),
            label: 'Headers',
            hint: '{"Authorization": "Bearer \${TOKEN}"}',
            value: source['headers'] == null
                ? ''
                : jsonEncode(source['headers']),
            // 非法 JSON 保持原值不写（React：`catch { /* keep as-is */ }`）。
            onSubmitted: (v) {
              if (v.trim().isEmpty) return;
              try {
                _setField('headers', jsonDecode(v));
              } catch (_) {
                /* 保持原值 */
              }
            },
          ),
        // `settings` 源可以在一条来源里内联定义多个插件，每个插件自己又是一个
        // 完整的来源编辑器（递归嵌套）。`PluginsSection.tsx:139-176`。
        if (_type == 'settings') ...[
          for (var pi = 0; pi < _plugins.length; pi++)
            _InlinePluginRow(
              key: ValueKey('$idPrefix-plugin-$pi'),
              idPrefix: '$idPrefix-plugin-$pi',
              plugin: _plugins[pi],
              onChanged: (next) {
                final plugs = _plugins;
                plugs[pi] = next;
                _setPlugins(plugs);
              },
              onRemove: () {
                final plugs = _plugins..removeAt(pi);
                _setPlugins(plugs);
              },
            ),
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: SmallButton(
              key: ValueKey('$idPrefix-plugin-add'),
              label: '+ Plugin',
              onTap: () => _setPlugins([
                ..._plugins,
                {
                  'name': '',
                  'source': {'source': 'github'},
                },
              ]),
            ),
          ),
        ],
        SwitchRow(
          key: ValueKey('$idPrefix-auto-update'),
          label: 'auto',
          description: t.t('settings.plugins.autoRefresh'),
          value: source['autoUpdate'] == true,
          onChanged: (v) => _setField('autoUpdate', v),
        ),
        if (!compact) Divider(height: AidogSpace.smd, color: theme.c.line),
      ],
    );
  }
}

/// 一条内联插件：名字 + 它自己的来源编辑器（递归）+ 移除。
/// `PluginsSection.tsx:141-168`。
class _InlinePluginRow extends StatelessWidget {
  const _InlinePluginRow({
    super.key,
    required this.idPrefix,
    required this.plugin,
    required this.onChanged,
    required this.onRemove,
  });

  final String idPrefix;
  final Map<String, Object?> plugin;
  final ValueChanged<Map<String, Object?>> onChanged;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final src = plugin['source'];
    return Container(
      margin: const EdgeInsets.only(left: AidogSpace.ssm, top: AidogSpace.sxs),
      padding: const EdgeInsets.all(AidogSpace.ssm),
      decoration: BoxDecoration(
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          TextRow(
            key: ValueKey('$idPrefix-name'),
            label: 'name',
            hint: 'plugin-name',
            value: '${plugin['name'] ?? ''}',
            onSubmitted: (v) => onChanged({...plugin, 'name': v.trim()}),
            trailing: IconButton(
              key: ValueKey('$idPrefix-remove'),
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
              iconSize: 14,
              visualDensity: VisualDensity.compact,
              tooltip: t.t('action.remove'),
              icon: Icon(Icons.close, color: theme.c.fg3),
              onPressed: onRemove,
            ),
          ),
          MarketplaceSourceEditor(
            idPrefix: '$idPrefix-src',
            compact: true,
            source: src is Map
                ? Map<String, Object?>.from(src)
                : const {'source': 'github'},
            onChanged: (s) => onChanged({...plugin, 'source': s}),
          ),
        ],
      ),
    );
  }
}

/// 插件区编辑器本体。
class PluginsEditor extends StatefulWidget {
  const PluginsEditor({
    super.key,
    required this.config,
    required this.updateField,
  });

  /// 整份 claude 配置（四个字段都是顶层键，不是一棵子树）。
  final Map<String, Object?> config;
  final void Function(String field, Object? value) updateField;

  @override
  State<PluginsEditor> createState() => _PluginsEditorState();
}

class _PluginsEditorState extends State<PluginsEditor> {
  final TextEditingController _newPlugin = TextEditingController();
  final TextEditingController _newMarket = TextEditingController();
  String? _pluginConfigsError;

  @override
  void dispose() {
    _newPlugin.dispose();
    _newMarket.dispose();
    super.dispose();
  }

  Map<String, Object?> get _enabled => widget.config['enabledPlugins'] is Map
      ? Map<String, Object?>.from(widget.config['enabledPlugins'] as Map)
      : <String, Object?>{};

  Map<String, Object?> get _markets =>
      widget.config['extraKnownMarketplaces'] is Map
      ? Map<String, Object?>.from(
          widget.config['extraKnownMarketplaces'] as Map,
        )
      : <String, Object?>{};

  List<String> _strList(String key) => widget.config[key] is List
      ? (widget.config[key] as List).map((e) => '$e').toList()
      : const [];

  void _setEnabled(String key, bool on) =>
      widget.updateField('enabledPlugins', {..._enabled, key: on});

  void _addPlugin() {
    final k = _newPlugin.text.trim();
    if (k.isEmpty) return;
    _setEnabled(k, true);
    _newPlugin.clear();
    setState(() {});
  }

  void _removePlugin(String key) {
    final next = {..._enabled}..remove(key);
    widget.updateField('enabledPlugins', next.isEmpty ? null : next);
  }

  void _addMarket() {
    final name = _newMarket.text.trim();
    if (name.isEmpty) return;
    widget.updateField('extraKnownMarketplaces', {
      ..._markets,
      name: {
        'source': {'source': 'github'},
      },
    });
    _newMarket.clear();
    setState(() {});
  }

  void _removeMarket(String name) {
    final next = {..._markets}..remove(name);
    widget.updateField('extraKnownMarketplaces', next.isEmpty ? null : next);
  }

  void _updateMarket(String name, Map<String, Object?> value) =>
      widget.updateField('extraKnownMarketplaces', {..._markets, name: value});

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final enabled = _enabled;
    final markets = _markets;
    return Column(
      key: const ValueKey('plugins-editor'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // ── Enabled Plugins ──
        const TileMeta('Enabled Plugins'),
        Text(
          t.t('settings.plugins.enabledHint'),
          style: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
        const SizedBox(height: AidogSpace.sxs),
        for (final e in enabled.entries)
          Padding(
            padding: const EdgeInsets.only(bottom: 2),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    e.key,
                    style: AidogType.numSm.copyWith(color: theme.c.fg),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                Switch(
                  key: ValueKey('plugin-on-${e.key}'),
                  value: e.value == true,
                  onChanged: (v) => _setEnabled(e.key, v),
                  activeThumbColor: theme.c.accent,
                  inactiveTrackColor: theme.c.surface2,
                  inactiveThumbColor: theme.c.fg3,
                ),
                Tooltip(
                  message: t.t('settings.plugins.removePlugin'),
                  child: SmallButton(
                    key: ValueKey('plugin-del-${e.key}'),
                    label: '×',
                    danger: true,
                    onTap: () => _removePlugin(e.key),
                  ),
                ),
              ],
            ),
          ),
        Row(
          children: [
            Expanded(
              child: TextField(
                key: const ValueKey('plugin-new'),
                controller: _newPlugin,
                style: AidogType.micro.copyWith(color: theme.c.fg),
                decoration: const InputDecoration(
                  isDense: true,
                  hintText: 'plugin-name@marketplace',
                ),
                onSubmitted: (_) => _addPlugin(),
              ),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              key: const ValueKey('plugin-add'),
              label: '+',
              onTap: _addPlugin,
            ),
          ],
        ),

        // ── Extra Marketplaces ──
        const SizedBox(height: AidogSpace.smd),
        const TileMeta('Extra Marketplaces'),
        Text(
          t.t('settings.plugins.marketplacesHint'),
          style: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
        const SizedBox(height: AidogSpace.sxs),
        for (final e in markets.entries)
          _marketCard(
            t,
            theme,
            e.key,
            e.value is Map ? Map<String, Object?>.from(e.value as Map) : {},
          ),
        Row(
          children: [
            Expanded(
              child: TextField(
                key: const ValueKey('market-new'),
                controller: _newMarket,
                style: AidogType.micro.copyWith(color: theme.c.fg),
                decoration: const InputDecoration(
                  isDense: true,
                  hintText: 'marketplace-name',
                ),
                onSubmitted: (_) => _addMarket(),
              ),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              key: const ValueKey('market-add'),
              label: '+',
              onTap: _addMarket,
            ),
          ],
        ),

        // ── Plugin Configs ──
        const SizedBox(height: AidogSpace.smd),
        const TileMeta('Plugin Configs'),
        Text(
          t.t('settings.plugins.configsHint'),
          style: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
        TextRow(
          key: const ValueKey('plugin-configs'),
          label: 'pluginConfigs',
          value: widget.config['pluginConfigs'] == null
              ? ''
              : const JsonEncoder.withIndent('  ')
                    .convert(widget.config['pluginConfigs']),
          maxLines: 6,
          onSubmitted: (text) {
            final raw = text.trim();
            if (raw.isEmpty) {
              setState(() => _pluginConfigsError = null);
              widget.updateField('pluginConfigs', null);
              return;
            }
            try {
              final decoded = jsonDecode(raw);
              setState(() => _pluginConfigsError = null);
              widget.updateField('pluginConfigs', decoded);
            } catch (e) {
              setState(() => _pluginConfigsError = '$e');
            }
          },
        ),
        if (_pluginConfigsError != null) ErrorNote(text: _pluginConfigsError!),

        // ── Skipped ──
        const SizedBox(height: AidogSpace.smd),
        const TileMeta('Skipped'),
        Text(
          t.t('settings.plugins.skippedHint'),
          style: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
        const SizedBox(height: AidogSpace.sxs),
        const TileMeta('skippedPlugins'),
        SandboxTagList(
          key: const ValueKey('skipped-plugins'),
          items: _strList('skippedPlugins'),
          hint: 'plugin-name@marketplace',
          onChanged: (v) =>
              widget.updateField('skippedPlugins', v.isEmpty ? null : v),
        ),
        const SizedBox(height: AidogSpace.sxs),
        const TileMeta('skippedMarketplaces'),
        SandboxTagList(
          key: const ValueKey('skipped-marketplaces'),
          items: _strList('skippedMarketplaces'),
          hint: 'marketplace-name',
          onChanged: (v) =>
              widget.updateField('skippedMarketplaces', v.isEmpty ? null : v),
        ),
      ],
    );
  }

  Widget _marketCard(
    I18nController t,
    AidogTheme theme,
    String name,
    Map<String, Object?> cfg,
  ) => Container(
    key: ValueKey('market-$name'),
    margin: const EdgeInsets.only(bottom: AidogSpace.ssm),
    padding: const EdgeInsets.all(AidogSpace.ssm),
    decoration: BoxDecoration(
      color: theme.c.surface2,
      border: Border.all(color: theme.c.line),
      borderRadius: BorderRadius.circular(AidogRadius.sm),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                name,
                style: AidogType.numSm.copyWith(color: theme.c.accent),
                overflow: TextOverflow.ellipsis,
              ),
            ),
            Tooltip(
              message: t.t('settings.plugins.removeMarketplace'),
              child: SmallButton(
                key: ValueKey('market-del-$name'),
                label: '×',
                danger: true,
                onTap: () => _removeMarket(name),
              ),
            ),
          ],
        ),
        MarketplaceSourceEditor(
          idPrefix: 'market-$name',
          source: cfg['source'] is Map
              ? Map<String, Object?>.from(cfg['source'] as Map)
              : {'source': 'github'},
          onChanged: (s) => _updateMarket(name, {...cfg, 'source': s}),
        ),
        TextRow(
          key: ValueKey('market-path-$name'),
          label: 'Path',
          hint: t.t('settings.plugins.localPathPh'),
          value: '${cfg['path'] ?? ''}',
          onSubmitted: (v) {
            final next = {...cfg};
            if (v.trim().isEmpty) {
              next.remove('path');
            } else {
              next['path'] = v.trim();
            }
            _updateMarket(name, next);
          },
        ),
      ],
    ),
  );
}
