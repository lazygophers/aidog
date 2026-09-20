/// GUI / JSON 双模式配置页的 widget 层（票 I16）—— 三个子页共用一棵树：
/// `settings/claude` / `settings/codex` / `settings/pi`。
///
/// 状态机、校验、脏判定、离页拦截全在 [SchemaConfigController]（票 I08 已测），
/// 本文件**只画界面**。在这里重写一遍校验就是第二份真值源，两份一定会漂移。
///
/// 字段定义来自 `assets/settings_schema.json`（由 `scripts/gen-flutter-settings-schema.mjs`
/// 从 TS 真值源生成，`yarn check:flutter-schema` 逐字节盯着）。**手改资产会被门禁拦下。**
///
/// 🔴 三页的差异是照搬 React 的，不是遗漏：codex / pi **不注册离页守卫、没有 Cmd+S**
/// （`schema_config_logic.dart:13` 写明理由）。要改属于产品改动，单开票。
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../i18n.dart';
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'import_diff.dart';
import 'schema_config_logic.dart';

/// 三页的接线 + 文案 key，集中在一处。
enum SchemaConfigKind {
  claude(
    wiring: SchemaConfigWiring.claude,
    schemaKey: 'claude',
    titleKey: 'settings.title',
  ),
  codex(
    wiring: SchemaConfigWiring.codex,
    schemaKey: 'codex',
    titleKey: 'appSettings.codexTab',
  ),
  pi(
    wiring: SchemaConfigWiring.pi,
    schemaKey: 'pi',
    titleKey: 'appSettings.piTab',
  );

  const SchemaConfigKind({
    required this.wiring,
    required this.schemaKey,
    required this.titleKey,
  });

  final SchemaConfigWiring wiring;
  final String schemaKey;
  final String titleKey;
}

/// 一个 schema 字段。字段名照 `src/services/claude-settings-schema.ts::SettingField`。
class SchemaField {
  SchemaField(this.raw);

  final Map<String, Object?> raw;

  String get key => '${raw['key']}';
  String get label => '${raw['label'] ?? raw['key']}';
  String get type => '${raw['type']}';
  String? get description => raw['description'] as String?;
  String? get placeholder => raw['placeholder'] as String?;
  bool get skipGui => raw['skipGui'] == true;
  List<String> get options =>
      (raw['options'] as List? ?? const []).map((e) => '$e').toList();
}

class SchemaSection {
  SchemaSection(this.raw);

  final Map<String, Object?> raw;

  String get id => '${raw['id']}';
  String get labelKey => '${raw['labelKey']}';
  List<SchemaField> get fields => (raw['fields'] as List? ?? const [])
      .map((e) => SchemaField(Map<String, Object?>.from(e as Map)))
      .toList();
}

/// 一份 schema + 它的推荐配置。
class SchemaBundle {
  const SchemaBundle({required this.sections, required this.recommended});

  final List<SchemaSection> sections;
  final Map<String, Object?> recommended;
}

/// 资产只解一次（121 KB JSON，每次进页面重解会在切页时掉帧）。
Future<Map<String, Object?>>? _schemaAssetCache;
Future<Map<String, Object?>>? _claudeDefaultsCache;

Future<Map<String, Object?>> _loadJsonAsset(String path) async =>
    Map<String, Object?>.from(
      jsonDecode(await rootBundle.loadString(path)) as Map,
    );

/// 读一份 schema bundle。[locale] 用于 claude 的推荐配置 `language` 覆盖
/// （与 React 的 `RECOMMENDED_CONFIG` 同规则：后端内置默认 + 运行时语言）。
Future<SchemaBundle> loadSchemaBundle(
  SchemaConfigKind kind, {
  String locale = 'zh-Hans',
}) async {
  final all = await (_schemaAssetCache ??= _loadJsonAsset(
    'assets/settings_schema.json',
  ));
  final part = Map<String, Object?>.from(all[kind.schemaKey] as Map);
  final sections = (part['sections'] as List)
      .map((e) => SchemaSection(Map<String, Object?>.from(e as Map)))
      .toList();
  final Map<String, Object?> recommended;
  if (kind == SchemaConfigKind.claude) {
    final defaults = await (_claudeDefaultsCache ??= _loadJsonAsset(
      'assets/claude_default_settings.json',
    ));
    recommended = {...defaults, 'language': locale};
  } else {
    recommended = Map<String, Object?>.from(
      (part['recommended'] as Map?) ?? const {},
    );
  }
  return SchemaBundle(sections: sections, recommended: recommended);
}

class SchemaConfigPage extends StatefulWidget {
  const SchemaConfigPage({
    super.key,
    required this.kind,
    this.invoke = kernelInvoke,
    this.bundleLoader,
  });

  final SchemaConfigKind kind;
  final InvokeFn invoke;

  /// widget 测试塞一份小 schema，避免每个用例都解 121 KB 资产。
  final Future<SchemaBundle> Function(SchemaConfigKind kind)? bundleLoader;

  @override
  State<SchemaConfigPage> createState() => _SchemaConfigPageState();
}

class _SchemaConfigPageState extends State<SchemaConfigPage> {
  SchemaConfigController? _c;
  SchemaBundle? _bundle;

  /// json / object / kv 字段的解析错误（key → 错误串）。
  final Map<String, String> _fieldErrors = {};

  @override
  void initState() {
    super.initState();
    _boot();
  }

  Future<void> _boot() async {
    final bundle = await (widget.bundleLoader?.call(widget.kind) ??
        loadSchemaBundle(widget.kind, locale: i18n.locale));
    if (!mounted) return;
    final c = SchemaConfigController(
      wiring: widget.kind.wiring,
      recommendedConfig: bundle.recommended,
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    setState(() {
      _bundle = bundle;
      _c = c;
    });
    await c.load();
  }

  @override
  void dispose() {
    _c?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final c = _c;
    final bundle = _bundle;
    if (c == null || bundle == null) {
      return SettingsPageBody(
        title: t.t(widget.kind.titleKey),
        children: [CenteredNote(text: t.t('status.loading'))],
      );
    }

    final body = SettingsPageBody(
      title: t.t(widget.kind.titleKey),
      subtitle: c.dirty ? t.t('settings.unsavedChanges') : null,
      trailing: Wrap(
        spacing: AidogSpace.sxs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          SmallButton(
            label: t.t('settings.guiMode'),
            active: c.mode == EditorMode.gui,
            onTap: () => c.setMode(EditorMode.gui),
          ),
          SmallButton(
            label: t.t('settings.jsonMode'),
            active: c.mode == EditorMode.json,
            onTap: () => c.setMode(EditorMode.json),
          ),
          SmallButton(
            label: t.t('settings.loadRecommended'),
            onTap: () => c.loadRecommended(
              noDiffText: t.t('settings.noRecommendedDiff'),
              loadedText: t.t('settings.loadedRecommended'),
            ),
          ),
          if (widget.kind == SchemaConfigKind.claude)
            SmallButton(
              label: t.t('settings.importFromClaudeCode'),
              onTap: () => c.importFromClaudeCode(
                noDiffText: t.t('settings.noDiff'),
                failedText: (e) => e,
              ),
            ),
          SmallButton(
            label: c.saving ? t.t('status.loading') : t.t('action.save'),
            onTap: c.canSave ? () => c.save(savedText: t.t('settings.saved')) : null,
          ),
        ],
      ),
      children: [
        if (c.mode == EditorMode.json)
          SettingsCard(
            title: t.t('settings.jsonMode'),
            children: [
              TextRow(
                label: t.t('settings.editInJson'),
                value: c.editJson,
                maxLines: 24,
                onChanged: c.setEditJson,
              ),
            ],
          )
        else
          for (final s in bundle.sections) _section(t, c, s),
        if (c.saveError.isNotEmpty) ErrorNote(text: c.saveError),
        if (c.importDiff != null)
          ImportDiffCard(
            pending: c.importDiff!,
            onCancel: c.cancelImport,
            onApply: (paths) =>
                c.applyImport(paths, appliedText: t.t('settings.imported')),
          ),
        if (c.pendingNav != null)
          _UnsavedCard(
            onSave: () => c.saveAndLeave(savedText: t.t('settings.saved')),
            onDiscard: c.discardAndLeave,
            onCancel: c.cancelLeave,
          ),
        if (c.toast.isNotEmpty)
          _Toast(text: c.toast, onDone: c.clearToast),
      ],
    );

    // Cmd+S 只有 claude 页有（React：Codex / pi 页没有这个快捷键）。
    if (widget.kind != SchemaConfigKind.claude) return body;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.keyS, meta: true): () {
          if (c.canSave) c.save(savedText: t.t('settings.saved'));
        },
        const SingleActivator(LogicalKeyboardKey.keyS, control: true): () {
          if (c.canSave) c.save(savedText: t.t('settings.saved'));
        },
      },
      child: Focus(autofocus: true, child: body),
    );
  }

  Widget _section(
    I18nController t,
    SchemaConfigController c,
    SchemaSection s,
  ) {
    final rows = <Widget>[];
    for (final f in s.fields) {
      if (f.skipGui) continue;
      rows.add(_field(t, c, f));
    }
    if (rows.isEmpty) return const SizedBox.shrink();
    return SettingsCard(title: t.t(s.labelKey), children: rows);
  }

  Widget _field(
    I18nController t,
    SchemaConfigController c,
    SchemaField f,
  ) {
    final label = tOr(t, 'settings.f_${f.key}', f.label);
    final value = c.config[f.key];
    switch (f.type) {
      case 'boolean':
        return SwitchRow(
          key: ValueKey('field-${f.key}'),
          label: label,
          description: f.description,
          value: value == true,
          onChanged: (v) => c.updateField(f.key, v),
        );
      case 'select':
        return SelectRow(
          key: ValueKey('field-${f.key}'),
          label: label,
          description: f.description,
          options: f.options,
          value: value == null ? '' : '$value',
          onChanged: (v) => c.updateField(f.key, v),
        );
      case 'string':
        return TextRow(
          key: ValueKey('field-${f.key}'),
          label: label,
          description: f.description,
          hint: f.placeholder,
          value: value == null ? '' : '$value',
          onSubmitted: (v) => c.updateField(f.key, v.trim()),
        );
      case 'string[]':
        final list = value is List ? value.map((e) => '$e').join('\n') : '';
        return TextRow(
          key: ValueKey('field-${f.key}'),
          label: label,
          description: f.description,
          hint: f.placeholder,
          value: list,
          maxLines: 4,
          onSubmitted: (v) {
            final items = v
                .split('\n')
                .map((e) => e.trim())
                .where((e) => e.isNotEmpty)
                .toList();
            c.updateField(f.key, items.isEmpty ? null : items);
          },
        );
      default:
        // json / object / kv / kv-select：统一走 JSON 编辑框。
        // React 侧另有可视化编辑器（权限矩阵、hooks 构建器），本票未搬 ——
        // 见 flutter/README.md 的「I16 未对齐」清单。
        return _JsonField(
          key: ValueKey('field-${f.key}'),
          label: label,
          description: f.description,
          value: value,
          error: _fieldErrors[f.key],
          onSubmitted: (text) {
            final raw = text.trim();
            if (raw.isEmpty) {
              setState(() => _fieldErrors.remove(f.key));
              c.updateField(f.key, null);
              return;
            }
            try {
              final decoded = jsonDecode(raw);
              setState(() => _fieldErrors.remove(f.key));
              c.updateField(f.key, decoded);
            } catch (e) {
              setState(() => _fieldErrors[f.key] = '$e');
            }
          },
        );
    }
  }
}

/// json / object / kv 字段：一个两空格缩进的 JSON 编辑框 + 解析错误提示。
class _JsonField extends StatelessWidget {
  const _JsonField({
    super.key,
    required this.label,
    required this.value,
    required this.onSubmitted,
    this.description,
    this.error,
  });

  final String label;
  final String? description;
  final Object? value;
  final String? error;
  final ValueChanged<String> onSubmitted;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      TextRow(
        label: label,
        description: description,
        value: value == null
            ? ''
            : const JsonEncoder.withIndent('  ').convert(value),
        maxLines: 6,
        onSubmitted: onSubmitted,
      ),
      if (error != null) ErrorNote(text: error!),
    ],
  );
}

/// 未保存确认卡，三个出口与 `Settings.tsx` 的弹窗一一对应。
class _UnsavedCard extends StatelessWidget {
  const _UnsavedCard({
    required this.onSave,
    required this.onDiscard,
    required this.onCancel,
  });

  final VoidCallback onSave;
  final VoidCallback onDiscard;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: t.t('settings.unsavedTitle'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              t.t('settings.unsavedBody'),
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(label: t.t('action.cancel'), onTap: onCancel),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: t.t('settings.discardChanges'),
                  danger: true,
                  onTap: onDiscard,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: t.t('settings.saveAndLeave'),
                  onTap: onSave,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// 3 秒自动消失的提示条。计时器挂在 widget 上，控制器不碰时间。
class _Toast extends StatefulWidget {
  const _Toast({required this.text, required this.onDone});

  final String text;
  final VoidCallback onDone;

  @override
  State<_Toast> createState() => _ToastState();
}

class _ToastState extends State<_Toast> {
  @override
  void initState() {
    super.initState();
    Future<void>.delayed(const Duration(seconds: 3)).then((_) {
      if (mounted) widget.onDone();
    });
  }

  @override
  Widget build(BuildContext context) => ToastBar(text: widget.text, ok: true);
}

// ── 导入差异弹窗 ──────────────────────────────────────────────

/// 逐项勾选的差异清单。与 React 的 `ImportDiff.tsx` 同结构：
/// 顶层节点带 children 时整组可折叠，勾选的最小单位是**叶子 path**。
class ImportDiffCard extends StatefulWidget {
  const ImportDiffCard({
    super.key,
    required this.pending,
    required this.onCancel,
    required this.onApply,
  });

  final PendingImportDiff pending;
  final VoidCallback onCancel;
  final void Function(Set<String> selectedPaths) onApply;

  @override
  State<ImportDiffCard> createState() => _ImportDiffCardState();
}

class _ImportDiffCardState extends State<ImportDiffCard> {
  late Set<String> _selected = _allLeaves();

  Set<String> _allLeaves() {
    final out = <String>[];
    for (final n in widget.pending.diff) {
      n.collectLeafPaths(out);
    }
    return out.toSet();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final all = _allLeaves();
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: widget.pending.recommended
            ? t.t('settings.editor.recommendTitle')
            : t.t('settings.editor.importTitle'),
        meta:
            '${t.t('settings.editor.selectedPrefix')} ${_selected.length} '
            '${t.t('settings.editor.selectedSuffix')}',
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                SmallButton(
                  label: t.t('settings.editor.selectAll'),
                  onTap: () => setState(() => _selected = all),
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: t.t('settings.editor.deselectAll'),
                  onTap: () => setState(() => _selected = {}),
                ),
              ],
            ),
            const SizedBox(height: AidogSpace.ssm),
            ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 320),
              child: SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    for (final n in widget.pending.diff) ..._nodeRows(t, n, 0),
                  ],
                ),
              ),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  onTap: widget.onCancel,
                ),
                const SizedBox(width: AidogSpace.ssm),
                // 一项都没勾就点不动 —— 应用空集合只会白跑一趟。
                SmallButton(
                  label: widget.pending.recommended
                      ? t.t('settings.editor.applySelected')
                      : t.t('settings.editor.importSelected'),
                  onTap: _selected.isEmpty
                      ? null
                      : () => widget.onApply(_selected),
                ),
              ],
            ),
            Text(
              '${t.t('settings.editor.diffCurrent')} → '
              '${widget.pending.recommended ? t.t('settings.editor.diffRecommended') : t.t('settings.editor.diffIncoming')}',
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ],
        ),
      ),
    );
  }

  List<Widget> _nodeRows(I18nController t, DiffNode n, int depth) {
    final theme = AidogTheme.of(context);
    final children = n.children;
    if (children != null && children.isNotEmpty) {
      return [
        Padding(
          padding: EdgeInsets.only(left: depth * 12.0, top: AidogSpace.sxs),
          child: Text(
            n.label,
            style: AidogType.micro.copyWith(color: theme.c.fg2),
          ),
        ),
        for (final ch in children) ..._nodeRows(t, ch, depth + 1),
      ];
    }
    final on = _selected.contains(n.path);
    return [
      InkWell(
        key: ValueKey('diff-${n.path}'),
        onTap: () => setState(() {
          final next = {..._selected};
          on ? next.remove(n.path) : next.add(n.path);
          _selected = next;
        }),
        child: Padding(
          padding: EdgeInsets.only(left: depth * 12.0, top: 2, bottom: 2),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(
                on ? Icons.check_box : Icons.check_box_outline_blank,
                size: 14,
                color: on ? theme.c.accent : theme.c.fg3,
              ),
              const SizedBox(width: AidogSpace.sxs),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      n.label,
                      style: AidogType.micro.copyWith(color: theme.c.fg),
                    ),
                    Text(
                      '${_short(n.current, t)} → ${_short(n.incoming, t)}',
                      style: AidogType.micro.copyWith(color: theme.c.fg3),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    ];
  }

  /// 值预览：对象只写「对象」二字，不把整棵树摊进一行（React 同规则）。
  static String _short(Object? v, I18nController t) {
    if (v == null) return t.t('settings.editor.none');
    if (v is Map) return t.t('settings.editor.diffObject');
    final s = jsonEncode(v);
    return s.length > 60 ? '${s.substring(0, 60)}…' : s;
  }
}
