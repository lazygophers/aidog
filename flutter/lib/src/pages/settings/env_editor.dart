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

  static const empty = EnvVarCatalog(
    defs: [],
    groupOrder: [],
    groupLabelKeys: {},
  );

  final List<EnvVarDef> defs;
  final List<String> groupOrder;
  final Map<String, String> groupLabelKeys;

  factory EnvVarCatalog.fromSchema(Map<String, Object?> claude) =>
      EnvVarCatalog(
        defs: (claude['envVarDefs'] as List? ?? const [])
            .map((e) => EnvVarDef(Map<String, Object?>.from(e as Map)))
            .toList(),
        groupOrder: (claude['envVarGroupOrder'] as List? ?? const [])
            .map((e) => '$e')
            .toList(),
        groupLabelKeys: {
          for (final e
              in (claude['envVarGroupLabelKeys'] as Map? ?? const {}).entries)
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
  /// 当前以明文显示的密码变量 key。**只活在这个 State 里** —— 切换态不写设置、
  /// 不落盘、不进日志，关掉页面即恢复密文。
  final Set<String> _revealed = {};
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

  /// 显式移除一条变量（React 的 `removeBtn` → `onChange(undefined)`）。
  ///
  /// 与「把值清成空串」是两件事：清空串只是**碰巧**也走到删键，而 boolean 变量
  /// 关到 `"0"` 仍是已设置态，没有空串可清 —— 那条路根本删不掉它。
  void _remove(String key) => _update(key, null);

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

  String _labelOf(I18nController t, EnvVarDef d) =>
      tOr(t, 'env.${d.key}', d.label);
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
    final custom = widget.env.entries
        .where((e) => !knownKeys.contains(e.key))
        .where(
          (e) =>
              q.isEmpty ||
              e.key.toLowerCase().contains(q) ||
              e.value.toLowerCase().contains(q),
        );

    final groups = [
      for (final g in widget.catalog.groupOrder)
        (
          g,
          present
              .where((d) => d.group == g && (q.isEmpty || _matches(t, d, q)))
              .toList(),
        ),
    ].where((e) => e.$2.isNotEmpty).toList();

    final hasResults = groups.isNotEmpty || custom.isNotEmpty;

    return Column(
      key: const ValueKey('env-editor'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // B21：React 搜索图标 14、输入 `F.body` 15（`EnvEditor.tsx:193-197`）。
        TextField(
          key: const ValueKey('env-search'),
          controller: _search,
          style: AidogType.label.copyWith(
            fontSize: kEditorInputFontSize,
            color: theme.c.fg,
          ),
          decoration: InputDecoration(
            isDense: true,
            contentPadding: kEditorInputPad,
            hintText: t.t('env.searchPlaceholder'),
            prefixIcon: Icon(Icons.search, size: 14, color: theme.c.fg3),
            suffixIcon: _search.text.isEmpty
                ? null
                : IconButton(
                    key: const ValueKey('env-search-clear'),
                    icon: Icon(Icons.close, size: 14, color: theme.c.fg3),
                    onPressed: () => setState(_search.clear),
                  ),
          ),
          onChanged: (_) => setState(() {}),
        ),
        if (!hasResults && q.isNotEmpty)
          // B22：React padding 20 · `F.body` 15 · 居中（`EnvEditor.tsx:207-210`）。
          Padding(
            padding: const EdgeInsets.all(20),
            child: Text(
              t.t('env.noResults'),
              key: const ValueKey('env-no-results'),
              textAlign: TextAlign.center,
              style: AidogType.label.copyWith(
                fontSize: kEditorInputFontSize,
                color: theme.c.fg3,
              ),
            ),
          ),
        for (final (group, list) in groups) ...[
          _groupHeading(
            theme,
            t.t(widget.catalog.groupLabelKeys[group] ?? 'env.group.$group'),
          ),
          for (final d in list) _row(t, theme, d),
        ],
        if (custom.isNotEmpty) ...[
          _groupHeading(theme, t.t('env.group.custom')),
          // B25：React 自定义变量行也是 `200px 1fr` 两列 grid，左列是只读 Input
          //（`EnvEditor.tsx:232-241`）。
          for (final e in custom)
            _EnvGridRow(
              key: ValueKey('env-custom-${e.key}'),
              label: e.key,
              control: Row(
                children: [
                  Expanded(
                    child: PlainTextField(
                      value: e.value,
                      fontSize: kEditorInputFontSize,
                      contentPadding: kEditorInputPad,
                      onSubmitted: (v) => _update(e.key, v),
                    ),
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                  _removeBtn(t, theme, e.key),
                ],
              ),
            ),
        ],
        // 搜索态下不显示「添加」（React：`{!search && ...}`）——过滤中加变量会立刻被滤掉。
        if (q.isEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              // React 这两格是定宽 120 + `F.body` 15（`EnvEditor.tsx:252-255`）。
              SizedBox(
                width: 120,
                child: TextField(
                  key: const ValueKey('env-custom-key'),
                  controller: _customKey,
                  style: AidogType.label.copyWith(
                    fontSize: kEditorInputFontSize,
                    color: theme.c.fg,
                  ),
                  decoration: const InputDecoration(
                    isDense: true,
                    contentPadding: kEditorInputPad,
                    hintText: 'KEY',
                  ),
                ),
              ),
              const SizedBox(width: AidogSpace.ssm),
              SizedBox(
                width: 120,
                child: TextField(
                  key: const ValueKey('env-custom-value'),
                  controller: _customVal,
                  style: AidogType.label.copyWith(
                    fontSize: kEditorInputFontSize,
                    color: theme.c.fg,
                  ),
                  decoration: const InputDecoration(
                    isDense: true,
                    contentPadding: kEditorInputPad,
                    hintText: 'VALUE',
                  ),
                ),
              ),
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(
                key: const ValueKey('env-add-custom'),
                label: t.t('env.addCustom'),
                // React 两颗按钮是 `F.body` 15 + `S.btnPad` 8/18
                //（`EnvEditor.tsx:256,267`）。
                fontSize: kEditorInputFontSize,
                padding: (18, 8),
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
                const SizedBox(width: AidogSpace.s_8),
                // A15：React 是 `bottom:100%; right:0; zIndex:100` 的浮层
                // （minWidth 340 / maxHeight 360 / `0 8px 32px` 阴影，
                // `EnvEditor.tsx:271-278`），不是行下方的内联块。
                AnchoredMenu(
                  open: _showAddMenu,
                  onDismiss: () => setState(() => _showAddMenu = false),
                  minWidth: 340,
                  maxHeight: 360,
                  menuBuilder: (_) => _addMenu(t, theme, addable),
                  anchor: SmallButton(
                    key: const ValueKey('env-add-known'),
                    label: t.t('env.addKnown'),
                    active: _showAddMenu,
                    fontSize: kEditorInputFontSize,
                    padding: (18, 8),
                    onTap: () => setState(() => _showAddMenu = !_showAddMenu),
                  ),
                ),
              ],
            ],
          ),
        ],
      ],
    );
  }

  /// 组标题 —— React `EnvGroupHeading`（`EnvEditor.tsx:126-132`）：
  /// `F.label` 15 w600 fg2 + 右侧撑满的 1px 分隔线 + paddingTop 16 / bottom 4。
  Widget _groupHeading(AidogTheme theme, String label) => Padding(
    padding: const EdgeInsets.only(top: 16, bottom: 4),
    child: Row(
      children: [
        Text(
          label,
          style: AidogType.label.copyWith(
            fontSize: 15,
            fontWeight: FontWeight.w600,
            color: theme.c.fg2,
          ),
        ),
        const SizedBox(width: 12),
        Expanded(child: Container(height: 1, color: theme.c.line)),
      ],
    ),
  );

  /// 「添加已知变量」下拉：按组分节，点一条就按类型填一个默认值
  /// （boolean → "1"，select → 第一个选项，其余 → "1"，与 React 同一行表达式）。
  Widget _addMenu(
    I18nController t,
    AidogTheme theme,
    List<EnvVarDef> addable,
  ) => Column(
    key: const ValueKey('env-add-menu'),
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      for (final g in widget.catalog.groupOrder)
        if (addable.any((d) => d.group == g)) ...[
          // B24：React 组标题 `F.hint` 13 w600 fg3 · pad `6px 10px 2px`
          //（`EnvEditor.tsx:284`）。
          Padding(
            padding: const EdgeInsets.fromLTRB(10, 6, 10, 2),
            child: Text(
              t.t(widget.catalog.groupLabelKeys[g] ?? 'env.group.$g'),
              style: AidogType.label.copyWith(
                fontSize: 13,
                color: theme.c.fg3,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
          for (final d in addable.where((d) => d.group == g))
            // B23：React 菜单项 pad 6/10 · 名字 `F.body` 15 w500 ·
            // key `F.hint` 13 mono marginLeft 8 · hover bg-glass 底
            //（`EnvEditor.tsx:288-306`）。
            InkWell(
              key: ValueKey('env-add-${d.key}'),
              hoverColor: theme.c.surface,
              borderRadius: BorderRadius.circular(AidogRadius.sm),
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
                  horizontal: 10,
                  vertical: 6,
                ),
                child: Row(
                  children: [
                    Flexible(
                      child: Text(
                        _labelOf(t, d),
                        style: AidogType.label.copyWith(
                          fontSize: 15,
                          fontWeight: FontWeight.w500,
                          color: theme.c.fg,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Flexible(
                      child: Text(
                        d.key,
                        style: AidogType.numSm.copyWith(
                          fontSize: 13,
                          color: theme.c.fg3,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
              ),
            ),
        ],
    ],
  );

  /// 「移除」×。只在该变量**已设置**时出现（React 的 `isSet` 同判据）。
  ///
  /// C5：React 是 `S.btnIcon` 34×34 的 ghost 按钮（`_shared.tsx:266`），
  /// 原先 env 侧是 22×22、其余 24×24 两档不一。统一到 34。
  Widget _removeBtn(I18nController t, AidogTheme theme, String key) =>
      IconButton(
        key: ValueKey('env-remove-$key'),
        padding: EdgeInsets.zero,
        constraints: const BoxConstraints(minWidth: 34, minHeight: 34),
        iconSize: 14,
        visualDensity: VisualDensity.compact,
        tooltip: t.t('action.remove'),
        icon: Icon(Icons.close, color: theme.c.fg3),
        onPressed: () => _remove(key),
      );

  Widget _row(I18nController t, AidogTheme theme, EnvVarDef d) {
    final value = widget.env[d.key];
    // React `EnvEditor.tsx:34`：`value !== undefined && value !== ""`。
    final isSet = value != null && value.isNotEmpty;
    final shown = _revealed.contains(d.key);
    // A14：React 左列是**三行独立文本** —— label 15 w500 / key mono 13 /
    // desc 13（`EnvEditor.tsx:113-117`），原先 key 与 desc 被拼成一条字符串。
    final control = switch (d.type) {
      'boolean' => Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          if (isSet) _removeBtn(t, theme, d.key),
          const SizedBox(width: 8),
          AidogSwitch(
            value: envBool(value),
            onChanged: () => _update(d.key, envBool(value) ? '0' : '1'),
          ),
        ],
      ),
      'select' => Row(
        children: [
          Expanded(
            child: InlineSelect<String?>(
              value: value == null || value.isEmpty ? null : value,
              options: [null, ...d.options],
              width: null,
              bordered: true,
              fontSize: kEditorInputFontSize,
              boxPadding: kEditorInputPad,
              labelOf: (o) => o ?? '—',
              onChanged: (v) => _update(d.key, v),
            ),
          ),
          if (isSet) _removeBtn(t, theme, d.key),
        ],
      ),
      'password' => Row(
        children: [
          Expanded(
            child: PlainTextField(
              value: value ?? '',
              hint: d.placeholder,
              obscure: !shown,
              fontSize: kEditorInputFontSize,
              contentPadding: kEditorInputPad,
              onSubmitted: (v) => _update(d.key, v),
            ),
          ),
          // 眼睛在输入框右边、「移除」左边（`EnvEditor.tsx:87-94` 同位置）。
          // 密文态下粘错一个字符是看不出来的，得能看一眼再切回去。
          IconButton(
            key: ValueKey('env-reveal-${d.key}'),
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 34, minHeight: 34),
            iconSize: 14,
            visualDensity: VisualDensity.compact,
            tooltip: shown ? 'Hide' : 'Show',
            icon: Icon(
              shown ? Icons.visibility_off : Icons.visibility,
              color: theme.c.fg3,
            ),
            // 只活在这个 State 里：不写设置、不落盘、不进日志。
            onPressed: () => setState(
              () => shown ? _revealed.remove(d.key) : _revealed.add(d.key),
            ),
          ),
          if (isSet) _removeBtn(t, theme, d.key),
        ],
      ),
      // number / string 共用文本框：number 的 min/max 由 Claude Code 自己校验，
      // React 那边也只是 <input type=number> 的浏览器级提示，不做写入拦截。
      _ => Row(
        children: [
          Expanded(
            child: PlainTextField(
              value: value ?? '',
              hint: d.placeholder,
              fontSize: kEditorInputFontSize,
              contentPadding: kEditorInputPad,
              onSubmitted: (v) => _update(d.key, v),
            ),
          ),
          if (isSet) _removeBtn(t, theme, d.key),
        ],
      ),
    };
    return _EnvGridRow(
      key: ValueKey('env-${d.key}'),
      label: _labelOf(t, d),
      varKey: d.key,
      description: _descOf(t, d),
      control: control,
    );
  }
}

/// env 的一行 —— React `EnvVarRow` 的 `200px 1fr` 两列 grid
/// （`EnvEditor.tsx:112-121`）：左列三行（label 15 w500 / key mono 13 / desc 13），
/// 右列控件，两列都 `paddingTop: 10`、列距 12。
class _EnvGridRow extends StatelessWidget {
  const _EnvGridRow({
    super.key,
    required this.label,
    required this.control,
    this.varKey,
    this.description = '',
  });

  final String label;

  /// 环境变量名（label 下面那行等宽小字）。自定义变量行没有，传 null。
  final String? varKey;
  final String description;
  final Widget control;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.smd),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 200,
            child: Padding(
              padding: const EdgeInsets.only(top: 10),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  HighlightedText(
                    label,
                    style: AidogType.label.copyWith(
                      fontSize: 15,
                      fontWeight: FontWeight.w500,
                      height: 1.4,
                      color: theme.c.fg,
                    ),
                  ),
                  if (varKey != null)
                    Padding(
                      padding: const EdgeInsets.only(top: 2),
                      child: HighlightedText(
                        varKey!,
                        style: AidogType.numSm.copyWith(
                          fontSize: 13,
                          letterSpacing: 0,
                          color: theme.c.fg3,
                        ),
                      ),
                    ),
                  if (description.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.only(top: 3),
                      child: Text(
                        description,
                        style: AidogType.label.copyWith(
                          fontSize: 13,
                          height: 1.5,
                          color: theme.c.fg3,
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.only(top: 10),
              child: control,
            ),
          ),
        ],
      ),
    );
  }
}
