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
import 'package:re_editor/re_editor.dart';
import 'package:re_highlight/languages/json.dart';

import '../../../i18n.dart';
import '../../shell/app_shell.dart' show PageStickyHeader;
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'env_editor.dart';
import 'field_editors.dart';
import 'json_text.dart';
import 'hooks_editor.dart';
import 'import_diff.dart';
import 'path_input.dart';
import 'permissions_editor.dart';
import 'plugins_editor.dart';
import 'sandbox_editor.dart';
import 'schema_config_logic.dart';
import 'statusline_panel.dart';

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
    // 页内标题用 `codex.title`（「Codex 配置」），不是侧栏的 tab 标签
    // （`appSettings.codexTab` = 「Codex」，仍由 nav.dart 用）——
    // 与 React `CodexSettings.tsx:167` 的页内标题栏一致。
    titleKey: 'codex.title',
  ),
  pi(
    wiring: SchemaConfigWiring.pi,
    schemaKey: 'pi',
    // 同上：页内标题 `pi.title`（「pi 配置」），对齐 `PiSettings.tsx:169`。
    titleKey: 'pi.title',
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

  /// `file` / `directory` —— 有值就走带补全的路径输入行（React `FieldRenderer.tsx:174`）。
  String? get pathType => raw['pathType'] as String?;
  List<String> get options =>
      (raw['options'] as List? ?? const []).map((e) => '$e').toList();

  /// kv / kv-select 新增行那一格的引导语（React `FieldRenderer.tsx:117`）。
  String? get keyPlaceholder => raw['keyPlaceholder'] as String?;

  /// kv-select 值那一格的候选。
  List<String> get valueOptions =>
      (raw['valueOptions'] as List? ?? const []).map((e) => '$e').toList();

  /// object 字段的子字段清单：`{key, label, type, options?, placeholder?}`。
  List<Map<String, Object?>> get objectFields => [
    for (final e in (raw['objectFields'] as List? ?? const []))
      if (e is Map) Map<String, Object?>.from(e),
  ];
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
  const SchemaBundle({
    required this.sections,
    required this.recommended,
    this.envCatalog = EnvVarCatalog.empty,
  });

  final List<SchemaSection> sections;
  final Map<String, Object?> recommended;

  /// 已知环境变量清单（只有 claude 有；codex / pi 留空）。
  final EnvVarCatalog envCatalog;
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
  return SchemaBundle(
    sections: sections,
    recommended: recommended,
    envCatalog: kind == SchemaConfigKind.claude
        ? EnvVarCatalog.fromSchema(part)
        : EnvVarCatalog.empty,
  );
}

/// Claude Code 的语言清单（`claude-settings-schema.ts::LANGUAGE_GROUPS` 拍平）。
/// CLI 集成页的语言下拉用它 —— 与 claude 页同一份数据，不抄第二份。
Future<List<({String value, String label})>> loadClaudeLanguageOptions() async {
  final all = await (_schemaAssetCache ??= _loadJsonAsset(
    'assets/settings_schema.json',
  ));
  final groups =
      ((all['claude'] as Map)['languageGroups'] as List? ?? const []);
  return [
    for (final g in groups.whereType<Map>())
      for (final o in (g['options'] as List? ?? const []).whereType<Map>())
        (value: '${o['value']}', label: '${g['family']} · ${o['label']}'),
  ];
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

  /// 全局搜索（R8，只有 claude 页有——`Settings.tsx` 独有，Codex / pi 页没有）。
  /// 空串 = 未过滤。
  String _searchQuery = '';

  /// 锚点导航（`SectionAnchorNav.tsx` + `Settings.tsx:283-321`）：
  /// 每个 section 一个 key 用来滚过去，`_activeSection` 是滚动联动高亮的当前节。
  final Map<String, GlobalKey> _sectionKeys = {};
  String _activeSection = '';

  /// 壳层那个 `SingleChildScrollView` 的位置对象。滚动联动挂在它上面 ——
  /// ScrollNotification 是往上冒泡的，在滚动内容里面挂 listener 收不到。
  ScrollPosition? _scrollPos;

  /// 骨架的粘顶横条插槽（`PageStickyHeader`）。null = 没有骨架（widget 测试）。
  ValueNotifier<Widget?>? _stickySlot;

  @override
  void initState() {
    super.initState();
    _boot();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _stickySlot = PageStickyHeader.maybeOf(context);
    final pos = Scrollable.maybeOf(context)?.position;
    if (identical(pos, _scrollPos)) return;
    _scrollPos?.removeListener(_onScroll);
    _scrollPos = pos;
    _scrollPos?.addListener(_onScroll);
  }

  GlobalKey _keyFor(String id) => _sectionKeys.putIfAbsent(id, GlobalKey.new);

  /// 滚动联动：取「还没滚出视口顶部」的最后一节当作当前节。
  /// React 用 IntersectionObserver 取可见比例最高的那节，判据不同但落点一样。
  void _onScroll() {
    String found = '';
    for (final e in _sectionKeys.entries) {
      final box = e.value.currentContext?.findRenderObject() as RenderBox?;
      if (box == null || !box.attached) continue;
      if (box.localToGlobal(Offset.zero).dy <= 120) found = e.key;
    }
    if (found.isNotEmpty && found != _activeSection) {
      setState(() => _activeSection = found);
    }
  }

  /// 锚点横条本体。两种挂法（骨架粘顶 / 页内一行）共用它。
  Widget _anchorBar(I18nController t, List<SchemaSection> sections) =>
      SingleChildScrollView(
        key: const ValueKey('settings-anchor-nav'),
        scrollDirection: Axis.horizontal,
        child: Row(
          children: [
            for (final s in sections)
              Padding(
                padding: const EdgeInsets.only(right: AidogSpace.sxs),
                child: SmallButton(
                  label: t.t(s.labelKey),
                  pill: true,
                  active: _activeSection == s.id,
                  onTap: () => _jumpToSection(s.id),
                ),
              ),
          ],
        ),
      );

  /// 点 chip / 搜索命中后滚过去（`Settings.tsx:316-321`）。
  void _jumpToSection(String id) {
    final ctx = _sectionKeys[id]?.currentContext;
    if (ctx == null) return;
    setState(() => _activeSection = id);
    Scrollable.ensureVisible(
      ctx,
      duration: AidogMotion.slow,
      curve: AidogMotion.easeStandard,
      alignment: 0,
    );
  }

  Future<void> _boot() async {
    final bundle =
        await (widget.bundleLoader?.call(widget.kind) ??
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
    // 离页时把横条摘掉，否则下一页顶上还挂着上一页的 chip 条。
    _stickySlot?.value = null;
    _scrollPos?.removeListener(_onScroll);
    _c?.dispose();
    super.dispose();
  }

  /// R8 全局搜索：section 标签命中 → 整节显示；否则按字段 label/key/description 命中
  /// → 只留命中的字段。返回值：section.id → 命中字段集合（`null` = 整节命中，show all）。
  /// 与 React `Settings.tsx` 的 `search` useMemo 同算法。
  Map<String, Set<String>?>? _computeSearch(
    I18nController t,
    SchemaBundle bundle,
  ) {
    final q = _searchQuery.trim().toLowerCase();
    if (q.isEmpty) return null;
    final matched = <String, Set<String>?>{};
    for (final s in bundle.sections) {
      final sectionLabel = t.t(s.labelKey).toLowerCase();
      if (sectionLabel.contains(q)) {
        matched[s.id] = null;
        continue;
      }
      final hits = <String>{};
      for (final f in s.fields) {
        final label = tOr(t, 'settings.f_${f.key}', f.label).toLowerCase();
        final desc = (f.description ?? '').toLowerCase();
        if (label.contains(q) ||
            f.key.toLowerCase().contains(q) ||
            desc.contains(q)) {
          hits.add(f.key);
        }
      }
      if (hits.isNotEmpty) matched[s.id] = hits;
    }
    return matched;
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

    final isClaude = widget.kind == SchemaConfigKind.claude;
    final search = isClaude ? _computeSearch(t, bundle) : null;
    final visibleSections = search == null
        ? bundle.sections
        : bundle.sections.where((s) => search.containsKey(s.id)).toList();

    // 有骨架就把横条挂到粘顶插槽上。不能在 build 里直接写 notifier
    // （会在构建期触发骨架重建），推到本帧之后。
    final slot = _stickySlot;
    if (slot != null) {
      final bar =
          isClaude && c.mode == EditorMode.gui && visibleSections.length > 1
          ? _anchorBar(t, visibleSections)
          : null;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) slot.value = bar;
      });
    }

    final body = SettingsPageBody(
      title: t.t(widget.kind.titleKey),
      // React 三页都有这条持久提示（不只是保存后的一次性 toast）：脏 → 未保存更改，
      // 干净 → 已保存（`SettingsHeader.tsx:157` / `CodexSettings.tsx:216`）。
      subtitle: c.dirty
          ? t.t('settings.unsavedChanges')
          : t.t('settings.allSaved'),
      trailing: Wrap(
        spacing: AidogSpace.sxs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          if (isClaude)
            SizedBox(
              width: 200,
              child: TextField(
                key: const ValueKey('settings-search'),
                decoration: InputDecoration(
                  isDense: true,
                  hintText: t.t('settings.search'),
                ),
                onChanged: (v) {
                  setState(() => _searchQuery = v);
                  // 搜完滚到第一个命中的 section（`Settings.tsx:307-315`）；
                  // 清空搜索时保持当前位置，不跳回顶部。
                  if (v.trim().isEmpty) return;
                  WidgetsBinding.instance.addPostFrameCallback((_) {
                    final hit = _computeSearch(t, bundle);
                    final first = hit == null
                        ? null
                        : bundle.sections
                              .where((s) => hit.containsKey(s.id))
                              .firstOrNull;
                    if (first != null) _jumpToSection(first.id);
                  });
                },
              ),
            ),
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
            onTap: c.canSave
                ? () => c.save(savedText: t.t('settings.saved'))
                : null,
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
        else if (search != null && visibleSections.isEmpty)
          CenteredNote(
            key: const ValueKey('settings-search-no-match'),
            text: t.t('settings.searchNoMatch'),
          )
        else ...[
          // section 锚点 chip 条（`SectionAnchorNav.tsx:19-68`）：十几节的长页面
          // 原先只能一路滚。React 那条是 sticky 的，这里的滚动视口在壳层、
          // 拿不到 sliver，先做成页内一行（跳转与联动高亮都在）。
          // 骨架能接粘顶横条时交给它（滚动时钉在视口顶部，对齐 React 的
          // `position: sticky`）；接不住（widget 测试单独挂页面，没有骨架）
          // 就退回页内一行 —— 跳转与高亮两种形态下都一样。
          if (isClaude && visibleSections.length > 1 && _stickySlot == null)
            Padding(
              padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
              child: _anchorBar(t, visibleSections),
            ),
          for (final s in visibleSections)
            KeyedSubtree(
              key: _keyFor(s.id),
              child: _section(t, c, s, fieldFilter: search?[s.id]),
            ),
        ],
        if (c.saveError.isNotEmpty) ErrorNote(text: c.saveError),
        if (c.importDiff != null)
          ImportDiffCard(
            pending: c.importDiff!,
            onCancel: c.cancelImport,
            onApply: (paths) =>
                c.applyImport(paths, appliedText: t.t('settings.imported')),
          ),
        if (c.pendingNav != null)
          UnsavedChangesCard(
            busy: c.saving,
            onSave: () => c.saveAndLeave(savedText: t.t('settings.saved')),
            onDiscard: c.discardAndLeave,
            onCancel: c.cancelLeave,
          ),
        if (c.toast.isNotEmpty) AutoToast(text: c.toast, onDone: c.clearToast),
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
    SchemaSection s, {

    /// R8 搜索命中的字段集合。`null` = 未过滤或整节命中（显示全部字段）。
    Set<String>? fieldFilter,
  }) {
    // hooks 区在 schema 里标了 skipGui（通用行渲染器画不了树），但 React 侧
    // 给它配了专用构建器（HooksSectionInline）—— 这里同样走专用编辑器。
    if (widget.kind == SchemaConfigKind.claude && s.id == 'hooks') {
      return SettingsCard(
        title: t.t(s.labelKey),
        children: [
          HooksEditor(
            hooks: c.config['hooks'] is Map
                ? Map<String, Object?>.from(c.config['hooks'] as Map)
                : {},
            onChanged: (v) => c.updateField('hooks', v),
            updateField: c.updateField,
            invoke: widget.invoke,
          ),
        ],
      );
    }
    // 环境变量区：React 侧整节换成 EnvEditor（339 条已知变量分组 + 搜索 + 自定义）。
    if (widget.kind == SchemaConfigKind.claude && s.id == 'env') {
      final env = c.config['env'] is Map
          ? {
              for (final e in (c.config['env'] as Map).entries)
                '${e.key}': '${e.value}',
            }
          : <String, String>{};
      return SettingsCard(
        title: t.t(s.labelKey),
        children: [
          EnvEditor(
            env: env,
            catalog: _bundle?.envCatalog ?? EnvVarCatalog.empty,
            onChanged: (v) => c.updateField('env', v),
          ),
        ],
      );
    }
    // 插件区：五个字段在 schema 里全是 skipGui，React 侧整节换成 PluginsSectionInline。
    if (widget.kind == SchemaConfigKind.claude && s.id == 'plugins') {
      return SettingsCard(
        title: t.t(s.labelKey),
        children: [PluginsEditor(config: c.config, updateField: c.updateField)],
      );
    }
    // 沙箱区同理：schema 里是一个 skipGui 的 json 字段，React 侧整节换成
    // SandboxSectionInline（文件系统 / 网络 / 安全策略 / 排除命令四块）。
    if (widget.kind == SchemaConfigKind.claude && s.id == 'sandbox') {
      return SettingsCard(
        title: t.t(s.labelKey),
        children: [
          SandboxEditor(
            sandbox: c.config['sandbox'] is Map
                ? Map<String, Object?>.from(c.config['sandbox'] as Map)
                : const {},
            invoke: widget.invoke,
            onChanged: (v) => c.updateField('sandbox', v),
          ),
        ],
      );
    }
    final rows = <Widget>[];
    // 状态栏区：React 侧是专用面板（StatusLineSection = 两个段编辑面板 +
    // fileSuggestion 字段 + 数据字段参考表），不是逐字段铺开。
    if (widget.kind == SchemaConfigKind.claude && s.id == 'status') {
      rows.add(
        StatusLineSectionBody(
          key: const ValueKey('section-status'),
          config: c.config,
          updateField: c.updateField,
          invoke: widget.invoke,
        ),
      );
    }
    for (final f in s.fields) {
      if (f.skipGui) continue;
      if (fieldFilter != null && !fieldFilter.contains(f.key)) continue;
      rows.add(_field(t, c, f));
    }
    if (widget.kind == SchemaConfigKind.claude && s.id == 'status') {
      rows.add(
        const StatusLineDataRef(key: ValueKey('section-status-dataref')),
      );
    }
    // Attribution 固定编辑器（commit + pr 两个子字段）：`attribution` 是 skipGui 的
    // json 字段，React 侧在「advanced」节末尾单独铺开两个文本框，搜索命中具体字段时隐藏
    // （`fieldFilter instanceof Set` 那条件的镜像：这里是 fieldFilter != null）。
    if (widget.kind == SchemaConfigKind.claude &&
        s.id == 'advanced' &&
        fieldFilter == null) {
      final attr = c.config['attribution'] is Map
          ? Map<String, Object?>.from(c.config['attribution'] as Map)
          : <String, Object?>{};
      void setAttr(String field, String v) {
        final next = {...attr, field: v};
        c.updateField(
          'attribution',
          next.values.any((x) => (x as String?)?.isNotEmpty == true)
              ? next
              : null,
        );
      }

      rows.add(
        Padding(
          key: const ValueKey('field-attribution'),
          padding: const EdgeInsets.only(top: AidogSpace.smd),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              TileMeta(tOr(t, 'settings.f_attribution', 'Attribution')),
              TextRow(
                key: const ValueKey('field-attribution-commit'),
                label: t.t('settings.attribution.commit'),
                value: '${attr['commit'] ?? ''}',
                onSubmitted: (v) => setAttr('commit', v),
              ),
              TextRow(
                key: const ValueKey('field-attribution-pr'),
                label: t.t('settings.attribution.pr'),
                value: '${attr['pr'] ?? ''}',
                onSubmitted: (v) => setAttr('pr', v),
              ),
            ],
          ),
        ),
      );
    }
    if (rows.isEmpty) return const SizedBox.shrink();
    return SettingsCard(title: t.t(s.labelKey), children: rows);
  }

  /// R10：字段当前值与推荐默认值不同 → 显示重置徽标。深比较不认键序
  /// （`FieldRenderer.tsx::stableEq` 的镜像，用 jsonEncode 排序键做等价替代）。
  static bool _stableEq(Object? a, Object? b) {
    if (a == null || b == null) return a == b;
    if (a is List && b is List) {
      if (a.length != b.length) return false;
      for (var i = 0; i < a.length; i++) {
        if (!_stableEq(a[i], b[i])) return false;
      }
      return true;
    }
    if (a is Map && b is Map) {
      if (a.length != b.length) return false;
      for (final k in a.keys) {
        if (!b.containsKey(k) || !_stableEq(a[k], b[k])) return false;
      }
      return true;
    }
    return a == b;
  }

  Widget _field(I18nController t, SchemaConfigController c, SchemaField f) {
    final value = c.config[f.key];
    // 权限矩阵：React 侧是专用可视化编辑器（PermissionsSectionInline），
    // 这里对齐 —— 编辑器自带「可视化 ↔ JSON」双模式，裸 JSON 没有丢；权限矩阵在
    // React 里整节 bypass FieldRenderer，同样没有重置徽标。
    if (widget.kind == SchemaConfigKind.claude && f.key == 'permissions') {
      return PermissionsEditor(
        key: const ValueKey('field-permissions'),
        perms: value is Map ? Map<String, Object?>.from(value) : {},
        onChanged: (v) => c.updateField(f.key, v),
      );
    }
    final recommended = _bundle?.recommended ?? const {};
    final hasDefault = recommended.containsKey(f.key);
    final defaultValue = hasDefault ? recommended[f.key] : null;
    final nonDefault = hasDefault && !_stableEq(value, defaultValue);
    final content = _fieldContent(t, c, f, value);
    if (!nonDefault) return content;
    return Column(
      key: ValueKey('field-wrap-${f.key}'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        content,
        Align(
          alignment: AlignmentDirectional.centerEnd,
          child: Tooltip(
            message: t.t('settings.resetToDefault'),
            child: SmallButton(
              key: ValueKey('field-reset-${f.key}'),
              label: t.t('settings.reset'),
              onTap: () => c.updateField(f.key, defaultValue),
            ),
          ),
        ),
      ],
    );
  }

  /// kv / kv-select 的值：只认「对象」，数组和标量当没配过。
  /// 对应 React 的 `typeof value === "object" && !Array.isArray(value)`
  /// —— JSON 数组在 Dart 侧解成 `List`，`is Map` 天然把它挡在外面。
  static Map<String, String> _asStringMap(Object? v) => v is Map
      ? {for (final e in v.entries) '${e.key}': '${e.value}'}
      : const {};

  Widget _fieldContent(
    I18nController t,
    SchemaConfigController c,
    SchemaField f,
    Object? value,
  ) {
    final label = tOr(t, 'settings.f_${f.key}', f.label);
    // 带 pathType 的字段走带补全的路径输入（React `FieldRenderer.tsx:174`）。
    if (f.pathType != null) {
      // fileSuggestion 在 React 侧是 StatusLineSection.tsx 就地构造的字段对象，
      // description 直接来自 t("statusline.fileSuggestionDesc", ...) 而不是 schema.ts
      // 里的静态字符串（schema.json 里那份是同一句中文的字面量副本，非 8 语言联动）。
      final description = f.key == 'fileSuggestion'
          ? tOr(t, 'statusline.fileSuggestionDesc', f.description ?? '')
          : f.description;
      return PathInputRow(
        key: ValueKey('field-${f.key}'),
        label: label,
        description: description,
        hint: f.placeholder,
        value: value == null ? null : '$value',
        pathType: f.pathType!,
        invoke: widget.invoke,
        onChanged: (v) => c.updateField(f.key, v),
      );
    }
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
        return FieldShell(
          key: ValueKey('field-${f.key}'),
          label: label,
          description: f.description,
          child: StringListEditor(
            idPrefix: 'field-${f.key}',
            items: value is List ? value.map((e) => '$e').toList() : const [],
            addLabel: f.placeholder ?? t.t('settings.addRule'),
            onChanged: (list) => c.updateField(f.key, list),
          ),
        );
      case 'kv':
        return FieldShell(
          key: ValueKey('field-${f.key}'),
          label: label,
          description: f.description,
          child: KvEditor(
            idPrefix: 'field-${f.key}',
            items: _asStringMap(value),
            keyPlaceholder: f.keyPlaceholder ?? 'KEY',
            onChanged: (kv) => c.updateField(f.key, kv),
          ),
        );
      case 'kv-select':
        return FieldShell(
          key: ValueKey('field-${f.key}'),
          label: label,
          description: f.description,
          child: KvSelectEditor(
            idPrefix: 'field-${f.key}',
            items: _asStringMap(value),
            valueOptions: f.valueOptions,
            keyPlaceholder: f.keyPlaceholder ?? 'KEY',
            onChanged: (kv) => c.updateField(f.key, kv),
          ),
        );
      case 'object':
        return FieldShell(
          key: ValueKey('field-${f.key}'),
          label: label,
          description: f.description,
          child: ObjectEditor(
            idPrefix: 'field-${f.key}',
            value: value is Map ? Map<String, Object?>.from(value) : const {},
            fields: f.objectFields,
            addLabel: t.t('settings.addRule'),
            onChanged: (v) => c.updateField(f.key, v),
          ),
        );
      default:
        // 剩下的 json 类型走 JSON 编辑框。权限矩阵、hooks 构建器另有专用编辑器。
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
            // 解析失败时**指出第几行第几列**，不再只说「格式错误」——
            // 一份几十行的配置里，「哪一行」才是能直接去改的那条信息。
            final err = locateJsonError(raw);
            if (err != null) {
              setState(() => _fieldErrors[f.key] = err.label);
              return;
            }
            setState(() => _fieldErrors.remove(f.key));
            c.updateField(f.key, jsonDecode(raw));
          },
        );
    }
  }
}

/// json / object / kv 字段：一个两空格缩进的 JSON 编辑框 + 格式化按钮 + 搜索/替换
/// （React `JsonCodeEditor.tsx` 的精简对应：多行文本域没有 CodeMirror 的语法高亮，
/// 但格式化、查找下一个/上一个、全部替换三个动作是真实可用的，不是摆设文案）。
class _JsonField extends StatefulWidget {
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
  State<_JsonField> createState() => _JsonFieldState();
}

/// 本项目 token → re_highlight 的着色表。
///
/// 不直接用 re_highlight 自带的 `atomOneDarkTheme` 之类：那些主题自带背景色与
/// 一整套与本项目无关的色板，深浅两套切换时会和 AidogTheme 打架。这里只给
/// JSON 用得到的几类 scope 上色，全部取自 token。
Map<String, TextStyle> _highlightTheme(AidogTheme theme) => {
  'attr': TextStyle(color: theme.c.accentText),
  'string': TextStyle(color: theme.c.ok),
  'number': TextStyle(color: theme.c.peak),
  'literal': TextStyle(color: theme.c.bad),
  'keyword': TextStyle(color: theme.c.bad),
  'punctuation': TextStyle(color: theme.c.fg3),
};

class _JsonFieldState extends State<_JsonField> {
  // 票 27 第 2 步（折叠）：换成 re_editor 的 CodeEditor。语法高亮、行号、
  // 折叠标记、折叠检测全部由它提供 —— 自己那套分词高亮随之删掉，不留两份。
  late final CodeLineEditingController _ctrl =
      CodeLineEditingController.fromText(_initialText());

  final FocusNode _focus = FocusNode();
  final TextEditingController _searchCtrl = TextEditingController();
  final TextEditingController _replaceCtrl = TextEditingController();
  bool _showSearch = false;
  bool _showReplace = false;

  String _initialText() => widget.value == null
      ? ''
      : const JsonEncoder.withIndent('  ').convert(widget.value);

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      if (!_focus.hasFocus) widget.onSubmitted(_ctrl.text);
    });
  }

  @override
  void didUpdateWidget(_JsonField old) {
    super.didUpdateWidget(old);
    final v = _initialText();
    if (v != _ctrl.text && !_focus.hasFocus) _ctrl.text = v;
  }

  @override
  void dispose() {
    _focus.dispose();
    _ctrl.dispose();
    _searchCtrl.dispose();
    _replaceCtrl.dispose();
    super.dispose();
  }

  void _format() {
    try {
      final pretty = const JsonEncoder.withIndent('  ')
          .convert(jsonDecode(_ctrl.text));
      _ctrl.text = pretty;
      widget.onSubmitted(pretty);
    } catch (_) {
      // 非法 JSON：不动文本，外层的错误提示已经在说明原因。
    }
  }

  /// 光标所在的「行 + 列」换算回整段文本的字符偏移。
  int _offsetOfSelection(String text) {
    final sel = _ctrl.selection;
    final lines = text.split('\n');
    if (sel.extentIndex < 0 || sel.extentIndex >= lines.length) return 0;
    var off = 0;
    for (var i = 0; i < sel.extentIndex; i++) {
      off += lines[i].length + 1; // +1 = 换行符
    }
    return off + sel.extentOffset;
  }

  void _findNext({bool backward = false}) {
    final q = _searchCtrl.text;
    if (q.isEmpty) return;
    final text = _ctrl.text;
    // CodeLineSelection 是「第几行 + 行内第几列」，而查找按整段文本的字符偏移
    // 算更简单，所以两边各转一次（转换函数就是行内报错那套 lineColumnAt 的反向）。
    final from = _offsetOfSelection(text);
    int idx;
    if (backward) {
      final upTo = (from - q.length - 1).clamp(0, text.length);
      idx = text.lastIndexOf(q, upTo);
      if (idx < 0) idx = text.lastIndexOf(q);
    } else {
      idx = text.indexOf(q, from);
      if (idx < 0) idx = text.indexOf(q);
    }
    if (idx < 0) return;
    final start = lineColumnAt(text, idx);
    final end = lineColumnAt(text, idx + q.length);
    setState(() {
      _ctrl.selection = CodeLineSelection(
        baseIndex: start.line - 1,
        baseOffset: start.column - 1,
        extentIndex: end.line - 1,
        extentOffset: end.column - 1,
      );
    });
    _focus.requestFocus();
  }

  void _replaceAll() {
    final q = _searchCtrl.text;
    if (q.isEmpty) return;
    final next = _ctrl.text.replaceAll(q, _replaceCtrl.text);
    _ctrl.text = next;
    widget.onSubmitted(next);
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.keyF, meta: true): () =>
            setState(() {
              _showSearch = true;
              _showReplace = false;
            }),
        const SingleActivator(LogicalKeyboardKey.keyF, control: true): () =>
            setState(() {
              _showSearch = true;
              _showReplace = false;
            }),
        const SingleActivator(
          LogicalKeyboardKey.keyF,
          meta: true,
          alt: true,
        ): () => setState(() {
          _showSearch = true;
          _showReplace = true;
        }),
        const SingleActivator(
          LogicalKeyboardKey.keyF,
          control: true,
          alt: true,
        ): () => setState(() {
          _showSearch = true;
          _showReplace = true;
        }),
      },
      child: Focus(
        canRequestFocus: false,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            TileMeta(widget.label),
            if (widget.description != null && widget.description!.isNotEmpty)
              Text(
                widget.description!,
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              ),
            Row(
              children: [
                SmallButton(label: t.t('jsonEditor.format'), onTap: _format),
                const SizedBox(width: AidogSpace.ssm),
                Expanded(
                  child: Text(
                    t.t('jsonEditor.searchHint'),
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ),
            if (_showSearch) ...[
              const SizedBox(height: AidogSpace.sxs),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      key: const ValueKey('json-search'),
                      controller: _searchCtrl,
                      style: AidogType.micro.copyWith(color: theme.c.fg),
                      decoration: const InputDecoration(isDense: true),
                      onSubmitted: (_) => _findNext(),
                    ),
                  ),
                  SmallButton(
                    label: '↑',
                    onTap: () => _findNext(backward: true),
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  SmallButton(label: '↓', onTap: () => _findNext()),
                  const SizedBox(width: AidogSpace.sxs),
                  SmallButton(
                    label: '×',
                    onTap: () => setState(() {
                      _showSearch = false;
                      _showReplace = false;
                    }),
                  ),
                ],
              ),
              if (_showReplace) ...[
                const SizedBox(height: AidogSpace.sxs),
                Row(
                  children: [
                    Expanded(
                      child: TextField(
                        key: const ValueKey('json-replace'),
                        controller: _replaceCtrl,
                        style: AidogType.micro.copyWith(color: theme.c.fg),
                        decoration: const InputDecoration(isDense: true),
                      ),
                    ),
                    // React 侧的替换按钮文案来自 CodeMirror 内建搜索面板，本身不接 i18n
                    // （@codemirror/search 的默认 keymap 硬编码英文），这里照抄同一处理。
                    SmallButton(
                      key: const ValueKey('json-replace-all'),
                      label: 'Replace All',
                      onTap: _replaceAll,
                    ),
                  ],
                ),
              ],
              const SizedBox(height: AidogSpace.sxs),
            ],
            // 固定高度：CodeEditor 自带视口（折叠、行号、横向滚动都靠它），
            // 不能像 TextField 那样随内容长高 —— 那样折叠就没有意义了。
            // React 的 JsonCodeEditor 同样是 maxHeight + 内部滚动。
            SizedBox(
              height: 260,
              child: CodeEditor(
                key: const ValueKey('json-code-editor'),
                controller: _ctrl,
                focusNode: _focus,
                padding: const EdgeInsets.all(AidogSpace.ssm),
                border: Border.all(color: theme.c.line),
                borderRadius: BorderRadius.circular(AidogRadius.sm),
                // `{}` / `[]` 自动识别折叠区间。
                chunkAnalyzer: const DefaultCodeChunkAnalyzer(),
                style: CodeEditorStyle(
                  fontSize: AidogType.numSm.fontSize,
                  fontFamily: AidogType.familyMono,
                  textColor: theme.c.fg,
                  backgroundColor: theme.c.surface2,
                  cursorColor: theme.c.accent,
                  selectionColor: theme.c.accentWash,
                  codeTheme: CodeHighlightTheme(
                    languages: {'json': CodeHighlightThemeMode(mode: langJson)},
                    theme: _highlightTheme(theme),
                  ),
                ),
                indicatorBuilder:
                    (context, editingController, chunkController, notifier) =>
                        Row(
                          children: [
                            DefaultCodeLineNumber(
                              controller: editingController,
                              notifier: notifier,
                              textStyle: AidogType.numSm.copyWith(
                                color: theme.c.fg3,
                              ),
                            ),
                            DefaultCodeChunkIndicator(
                              width: 20,
                              controller: chunkController,
                              notifier: notifier,
                            ),
                          ],
                        ),
                // **不接 onChanged**：CodeEditor 每敲一个键都会通知，而回写要走
                // 父级 setState —— 在 build 期间触发就是
                // 「setState() called during build」。何况边打字边校验会在写到
                // 一半时刷一串报错。提交仍由上面那个失焦监听负责，与换包前一致。
              ),
            ),
            if (widget.error != null) ErrorNote(text: widget.error!),
          ],
        ),
      ),
    );
  }
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
    // React 是普通 `Dialog` + createPortal（`ImportDiff.tsx:361`，width 680），
    // 点遮罩可关。
    return AidogModal(
      maxWidth: 680,
      onBarrierTap: widget.onCancel,
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

  /// React `nodeState`：组内叶子全选 = on，全不选 = off，介于两者 = partial。
  String _nodeState(DiffNode n) {
    final leaves = <String>[];
    n.collectLeafPaths(leaves);
    final on = leaves.where(_selected.contains).length;
    if (on == 0) return 'off';
    if (on == leaves.length) return 'on';
    return 'partial';
  }

  /// React `toggleNode`：组内叶子全选中就全部取消，否则全部选上。
  void _toggleNode(DiffNode n) {
    final leaves = <String>[];
    n.collectLeafPaths(leaves);
    final allOn = leaves.every(_selected.contains);
    setState(() {
      final next = {..._selected};
      for (final p in leaves) {
        if (allOn) {
          next.remove(p);
        } else {
          next.add(p);
        }
      }
      _selected = next;
    });
  }

  List<Widget> _nodeRows(I18nController t, DiffNode n, int depth) {
    final theme = AidogTheme.of(context);
    final children = n.children;
    if (children != null && children.isNotEmpty) {
      final state = _nodeState(n);
      final badgeColor = state == 'partial' ? theme.c.peak : theme.c.accent;
      final badgeText = state == 'partial'
          ? t.t('settings.editor.diffPartial')
          : t.t('settings.editor.diffObject');
      return [
        InkWell(
          key: ValueKey('diff-group-${n.path}'),
          onTap: () => _toggleNode(n),
          child: Padding(
            padding: EdgeInsets.only(left: depth * 12.0, top: AidogSpace.sxs),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Icon(
                  state == 'off'
                      ? Icons.check_box_outline_blank
                      : Icons.check_box,
                  size: 14,
                  color: state == 'off' ? theme.c.fg3 : theme.c.accent,
                ),
                const SizedBox(width: AidogSpace.sxs),
                Text(
                  n.label,
                  style: AidogType.micro.copyWith(
                    color: theme.c.fg2,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(width: AidogSpace.sxs),
                Text(
                  badgeText,
                  style: AidogType.micro.copyWith(
                    color: badgeColor,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ],
            ),
          ),
        ),
        for (final ch in children) ..._nodeRows(t, ch, depth + 1),
      ];
    }
    final on = _selected.contains(n.path);
    final changeType = _changeType(n);
    final labelColor = switch (changeType) {
      'added' => theme.c.ok,
      'removed' => theme.c.bad,
      _ => theme.c.accent,
    };
    final changeLabel = switch (changeType) {
      'added' => t.t('settings.editor.diffAdded'),
      'removed' => t.t('settings.editor.diffRemoved'),
      _ => t.t('settings.editor.diffChanged'),
    };
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
                    Row(
                      children: [
                        Text(
                          n.label,
                          style: AidogType.micro.copyWith(color: theme.c.fg),
                        ),
                        const SizedBox(width: AidogSpace.sxs),
                        Text(
                          changeLabel,
                          style: AidogType.micro.copyWith(
                            color: labelColor,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                      ],
                    ),
                    // 收起时给一行截断摘要当索引；展开后换成下面的完整两栏，
                    // 免得同一个值以截断和完整两种形态并排出现。
                    if (!on)
                      Text(
                        '${_short(n.current, t)} → ${_short(n.incoming, t)}',
                        style: AidogType.micro.copyWith(color: theme.c.fg3),
                      ),
                    if (on) _valuePanes(t, theme, n),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    ];
  }

  /// 选中即展开的「当前 / 导入」两栏（`ImportDiff.tsx:329-352`）。
  ///
  /// **导入是覆盖操作**，一行 60 字符的摘要（对象干脆只写「对象」二字）看不出
  /// 要把什么覆盖成什么，按不下去。这里给完整值，超高就在栏内滚。
  Widget _valuePanes(I18nController t, AidogTheme theme, DiffNode n) => Padding(
    padding: const EdgeInsets.only(top: AidogSpace.sxs),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: _valuePane(
            t,
            theme,
            keyName: 'diff-current-${n.path}',
            label: t.t('settings.editor.diffCurrent'),
            value: n.current,
          ),
        ),
        const SizedBox(width: AidogSpace.ssm),
        Expanded(
          child: _valuePane(
            t,
            theme,
            keyName: 'diff-incoming-${n.path}',
            label: t.t('settings.editor.diffIncoming'),
            value: n.incoming,
          ),
        ),
      ],
    ),
  );

  Widget _valuePane(
    I18nController t,
    AidogTheme theme, {
    required String keyName,
    required String label,
    required Object? value,
  }) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    mainAxisSize: MainAxisSize.min,
    children: [
      Text(label, style: AidogType.micro.copyWith(color: theme.c.fg3)),
      const SizedBox(height: 2),
      Container(
        width: double.infinity,
        constraints: const BoxConstraints(maxHeight: 120),
        padding: const EdgeInsets.all(AidogSpace.ssm),
        decoration: BoxDecoration(
          color: theme.c.surface2,
          border: Border.all(color: theme.c.line),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: SingleChildScrollView(
          child: SelectableText(
            _formatValue(value, t),
            key: ValueKey(keyName),
            style: AidogType.numSm.copyWith(
              // 没有值的那一侧压暗，与 React 的 `text-tertiary` 同义。
              color: value == null ? theme.c.fg3 : theme.c.fg,
            ),
          ),
        ),
      ),
    ],
  );

  /// `ImportDiff.tsx:280::formatValue`：无值写「(无)」，对象 / 数组缩进两格
  /// 序列化，其余直接转字符串（**不截断** —— 这一栏的全部意义就是看全）。
  static String _formatValue(Object? v, I18nController t) {
    if (v == null) return t.t('settings.editor.none');
    if (v is Map || v is List) {
      return const JsonEncoder.withIndent('  ').convert(v);
    }
    return '$v';
  }

  /// React `getChangeType`：`current == null` → 新增，`incoming == null` → 删除，
  /// 否则变更。JSON 没有 JS 的 `undefined`，null 就是它在 Dart 侧唯一的投影
  /// （`import_diff.dart` 顶部注释已写明这处已知语义差，实际取不到）。
  static String _changeType(DiffNode n) {
    if (n.current == null) return 'added';
    if (n.incoming == null) return 'removed';
    return 'changed';
  }

  /// 值预览：对象只写「对象」二字，不把整棵树摊进一行（React 同规则）。
  static String _short(Object? v, I18nController t) {
    if (v == null) return t.t('settings.editor.none');
    if (v is Map) return t.t('settings.editor.diffObject');
    final s = jsonEncode(v);
    return s.length > 60 ? '${s.substring(0, 60)}…' : s;
  }
}
