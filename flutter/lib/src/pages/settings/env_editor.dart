/// 环境变量的结构化编辑器 —— 对齐
/// `src/components/settings/editors/EnvEditor.tsx`。
///
/// 339 条已知变量按 13 个组分区展示，每条按自己的 `type` 给控件（开关 / 下拉 /
/// 数字 / 密码 / 文本）；不在清单里的键归到「自定义变量」一组。顶上一条搜索框，
/// 按 key、label、description 三路匹配（与 React 的 `filterDefs` 同三路）。
///
/// 变量清单来自 `assets/settings_schema.json` 的 `envVarDefs` —— 与 React 同一份
/// TS 真值源生成，不在 Dart 侧抄第二份。
library;

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../ui_bits.dart';
import 'bits.dart';

/// 一条已知环境变量的定义。字段照 `claude-settings-schema.ts::EnvVarDef`。
class EnvVarDef {
  const EnvVarDef(this.raw);

  final Map<String, Object?> raw;

  String get key => '${raw['key']}';
  String get label => '${raw['label'] ?? raw['key']}';
  String get type => '${raw['type']}';
  String get group => '${raw['group']}';
  String? get description => raw['description'] as String?;
  String? get placeholder => raw['placeholder'] as String?;
  List<String> get options =>
      (raw['options'] as List? ?? const []).map((e) => '$e').toList();
}

/// 一份 envVarDefs + 组顺序 + 组标签 key。
class EnvVarCatalog {
  const EnvVarCatalog({
    required this.defs,
    required this.groupOrder,
    required this.groupLabelKeys,
  });

  static const empty = EnvVarCatalog(defs: [], groupOrder: [], groupLabelKeys: {});

  final List<EnvVarDef> defs;
  final List<String> groupOrder;
  final Map<String, String> groupLabelKeys;

  factory EnvVarCatalog.fromSchema(Map<String, Object?> claude) => EnvVarCatalog(
        defs: (claude['envVarDefs'] as List? ?? const [])
            .map((e) => EnvVarDef(Map<String, Object?>.from(e as Map)))
            .toList(),
        groupOrder:
            (claude['envVarGroupOrder'] as List? ?? const []).map((e) => '$e').toList(),
        groupLabelKeys: {
          for (final e in (claude['envVarGroupLabelKeys'] as Map? ?? const {}).entries)
            '${e.key}': '${e.value}',
        },
      );
}

/// `"1"/"true"/"yes"/"on"` → true。照抄 `EnvEditor.tsx::envBool`。
bool envBool(String? v) =>
    v != null && const ['1', 'true', 'yes', 'on'].contains(v.toLowerCase());

class EnvEditor extends StatefulWidget {
  const EnvEditor({
    super.key,
    required this.env,
    required this.onChanged,
    required this.catalog,
  });

  /// `settings.json` 的 `env` 子树。
  final Map<String, String> env;

  /// 写回整棵子树；空表回传 `null`（= 删掉 `env` 键）。
  final ValueChanged<Map<String, String>?> onChanged;

  final EnvVarCatalog catalog;

  @override
  State<EnvEditor> createState() => _EnvEditorState();
}

class _EnvEditorState extends State<EnvEditor> {
  final TextEditingController _search = TextEditingController();
  final TextEditingController _customKey = TextEditingController();
  final TextEditingController _customVal = TextEditingController();
  bool _showAddMenu = false;

  @override
  void dispose() {
    _search.dispose();
    _customKey.dispose();
    _customVal.dispose();
    super.dispose();
  }

  /// 空值 = 删键（React 的 `updateEnv`：`value !== undefined && value !== ""`）。
  void _update(String key, String? value) {
    final next = {...widget.env};
    if (value == null || value.isEmpty) {
      next.remove(key);
    } else {
      next[key] = value;
    }
    widget.onChanged(next.isEmpty ? null : next);
  }

  String _labelOf(I18nController t, EnvVarDef d) => tOr(t, 'env.${d.key}', d.label);
  String _descOf(I18nController t, EnvVarDef d) =>
      tOr(t, 'env.${d.key}.desc', d.description ?? '');

  /// key / i18n label / i18n description / 原始 label 四路匹配（React `filterDefs`）。
  bool _matches(I18nController t, EnvVarDef d, String q) =>
      d.key.toLowerCase().contains(q) ||
      _labelOf(t, d).toLowerCase().contains(q) ||
      _descOf(t, d).toLowerCase().contains(q) ||
      d.label.toLowerCase().contains(q);

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final q = _search.text.trim().toLowerCase();
    final defs = widget.catalog.defs;
    final knownKeys = {for (final d in defs) d.key};

    final present = defs.where((d) => widget.env.containsKey(d.key)).toList();
    final addable = defs.where((d) => !widget.env.containsKey(d.key)).toList();
    final custom = widget.env.entries.where((e) => !knownKeys.contains(e.key)).where((e) =>
        q.isEmpty || e.key.toLowerCase().contains(q) || e.value.toLowerCase().contains(q));

    final groups = [
      for (final g in widget.catalog.groupOrder)
        (
          g,
          present.where((d) => d.group == g && (q.isEmpty || _matches(t, d, q))).toList(),
        ),
    ].where((e) => e.$2.isNotEmpty).toList();

    final hasResults = groups.isNotEmpty || custom.isNotEmpty;

    return Column(
      key: const ValueKey('env-editor'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        TextField(
          key: const ValueKey('env-search'),
          controller: _search,
          style: AidogType.micro.copyWith(color: theme.c.fg),
          decoration: InputDecoration(
            isDense: true,
            hintText: t.t('env.searchPlaceholder'),
            prefixIcon: Icon(Icons.search, size: 15, color: theme.c.fg3),
            suffixIcon: _search.text.isEmpty
                ? null
                : IconButton(
                    key: const ValueKey('env-search-clear'),
                    icon: Icon(Icons.close, size: 15, color: theme.c.fg3),
                    onPressed: () => setState(_search.clear),
                  ),
          ),
          onChanged: (_) => setState(() {}),
        ),
        if (!hasResults && q.isNotEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: AidogSpace.smd),
            child: Text(
              t.t('env.noResults'),
              key: const ValueKey('env-no-results'),
              textAlign: TextAlign.center,
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
        for (final (group, list) in groups) ...[
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t(widget.catalog.groupLabelKeys[group] ?? 'env.group.$group')),
          for (final d in list) _row(t, theme, d),
        ],
        if (custom.isNotEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('env.group.custom')),
          for (final e in custom)
            TextRow(
              key: ValueKey('env-custom-${e.key}'),
              label: e.key,
              value: e.value,
              onSubmitted: (v) => _update(e.key, v),
            ),
        ],
        // 搜索态下不显示「添加」（React：`{!search && ...}`）——过滤中加变量会立刻被滤掉。
        if (q.isEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          Row(
            children: [
              Expanded(
                child: TextField(
                  key: const ValueKey('env-custom-key'),
                  controller: _customKey,
                  style: AidogType.micro.copyWith(color: theme.c.fg),
                  decoration: const InputDecoration(isDense: true, hintText: 'KEY'),
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              Expanded(
                child: TextField(
                  key: const ValueKey('env-custom-value'),
                  controller: _customVal,
                  style: AidogType.micro.copyWith(color: theme.c.fg),
                  decoration: const InputDecoration(isDense: true, hintText: 'VALUE'),
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              SmallButton(
                key: const ValueKey('env-add-custom'),
                label: t.t('env.addCustom'),
                onTap: () {
                  final k = _customKey.text.trim();
                  if (k.isEmpty) return;
                  _update(k, _customVal.text);
                  _customKey.clear();
                  _customVal.clear();
                  setState(() {});
                },
              ),
              if (addable.isNotEmpty) ...[
                const SizedBox(width: AidogSpace.sxs),
                SmallButton(
                  key: const ValueKey('env-add-known'),
                  label: t.t('env.addKnown'),
                  active: _showAddMenu,
                  onTap: () => setState(() => _showAddMenu = !_showAddMenu),
                ),
              ],
            ],
          ),
          if (_showAddMenu) _addMenu(t, theme, addable),
        ],
      ],
    );
  }

  /// 「添加已知变量」下拉：按组分节，点一条就按类型填一个默认值
  /// （boolean → "1"，select → 第一个选项，其余 → "1"，与 React 同一行表达式）。
  Widget _addMenu(I18nController t, AidogTheme theme, List<EnvVarDef> addable) => Container(
        key: const ValueKey('env-add-menu'),
        margin: const EdgeInsets.only(top: AidogSpace.sxs),
        constraints: const BoxConstraints(maxHeight: 360),
        decoration: BoxDecoration(
          color: theme.c.surface2,
          border: Border.all(color: theme.c.line),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final g in widget.catalog.groupOrder)
                if (addable.any((d) => d.group == g)) ...[
                  Padding(
                    padding: const EdgeInsets.fromLTRB(
                        AidogSpace.ssm, AidogSpace.sxs, AidogSpace.ssm, 0),
                    child: Text(
                      t.t(widget.catalog.groupLabelKeys[g] ?? 'env.group.$g'),
                      style: AidogType.micro.copyWith(
                        color: theme.c.fg3,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ),
                  for (final d in addable.where((d) => d.group == g))
                    InkWell(
                      key: ValueKey('env-add-${d.key}'),
                      onTap: () {
                        final def = switch (d.type) {
                          'boolean' => '1',
                          'select' => d.options.isEmpty ? '1' : d.options.first,
                          _ => '1',
                        };
                        _update(d.key, def);
                        setState(() => _showAddMenu = false);
                      },
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                            horizontal: AidogSpace.ssm, vertical: AidogSpace.sxs),
                        child: Row(
                          children: [
                            Flexible(
                              child: Text(
                                _labelOf(t, d),
                                style: AidogType.micro.copyWith(color: theme.c.fg),
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            const SizedBox(width: AidogSpace.sxs),
                            Flexible(
                              child: Text(
                                d.key,
                                style: AidogType.numSm.copyWith(color: theme.c.fg3),
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                ],
            ],
          ),
        ),
      );

  Widget _row(I18nController t, AidogTheme theme, EnvVarDef d) {
    final value = widget.env[d.key];
    final desc = _descOf(t, d);
    // key 在 React 里是 label 下面的小字（等宽），这里并进 description 一起显示。
    final description = desc.isEmpty ? d.key : '${d.key} · $desc';
    switch (d.type) {
      case 'boolean':
        return SwitchRow(
          key: ValueKey('env-${d.key}'),
          label: _labelOf(t, d),
          description: description,
          value: envBool(value),
          onChanged: (v) => _update(d.key, v ? '1' : '0'),
        );
      case 'select':
        return SelectRow(
          key: ValueKey('env-${d.key}'),
          label: _labelOf(t, d),
          description: description,
          options: d.options,
          value: value ?? '',
          onChanged: (v) => _update(d.key, v),
        );
      case 'password':
        return TextRow(
          key: ValueKey('env-${d.key}'),
          label: _labelOf(t, d),
          description: description,
          hint: d.placeholder,
          value: value ?? '',
          obscure: true,
          onSubmitted: (v) => _update(d.key, v),
        );
      default:
        // number / string 共用文本框：number 的 min/max 由 Claude Code 自己校验，
        // React 那边也只是 <input type=number> 的浏览器级提示，不做写入拦截。
        return TextRow(
          key: ValueKey('env-${d.key}'),
          label: _labelOf(t, d),
          description: description,
          hint: d.placeholder,
          value: value ?? '',
          onSubmitted: (v) => _update(d.key, v),
        );
    }
  }
}
