/// 模型信息页界面（票 I09），对应 `src/pages/ModelInfo/`。
///
/// 三块：同步状态卡 + 筛选栏 + 模型维度 / 平台维度双 tab；点一行开详情（按平台分 tab 比价）。
/// 详情是**页面 state 的一部分**（`selected` 单一状态驱动开合），不是 route。
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../utils/formatters.dart';
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'filter_dropdown.dart';
import 'mini_select.dart';
import 'invoke.dart';
import 'model_info_logic.dart';
import 'ui_bits.dart';

class ModelInfoPage extends StatefulWidget {
  const ModelInfoPage({super.key, this.invoke = kernelInvoke});

  final InvokeFn invoke;

  @override
  State<ModelInfoPage> createState() => _ModelInfoPageState();
}

class _ModelInfoPageState extends State<ModelInfoPage> {
  late final ModelInfoController _c;
  String? _labelLocale;

  bool _built = false;

  /// 控制器的 t 必须来自 context（全局 `i18n` 在 widget 测试里没 init 过）。
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_built) return;
    _built = true;
    final tr = AidogI18n.of(context);
    _c = ModelInfoController(
      invoke: widget.invoke,
      t: tr.t,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    _labelLocale = tr.locale;
    _c.init(tr.locale);
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    // 切语言要重取平台本地化名（React 的 `[i18n.language]` effect）。
    if (_labelLocale != t.locale) {
      _labelLocale = t.locale;
      _c.loadLabels(t.locale);
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        PageHead(
          title: t.t('modelInfo.syncTitle'),
          subtitle: t.t('modelInfo.syncDesc'),
        ),
        _syncCard(t),
        const SizedBox(height: AidogSpace.ssm),
        _filterBar(t),
        const SizedBox(height: AidogSpace.ssm),
        if (_c.loading)
          CenteredNote(text: t.t('status.loading'))
        else if (_c.groups.isEmpty)
          CenteredNote(text: t.t('modelInfo.empty'))
        else ...[
          Wrap(
            spacing: AidogSpace.ssm,
            children: [
              SmallButton(
                label: t.t('modelInfo.tabModels'),
                active: _c.tab == 'models',
                onTap: () => _c.setTab('models'),
              ),
              SmallButton(
                label: t.t('modelInfo.tabPlatforms'),
                active: _c.tab == 'platforms',
                onTap: () => _c.setTab('platforms'),
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.ssm),
          if (_c.tab == 'models') ...[
            _modelTable(t),
            const SizedBox(height: AidogSpace.ssm),
            _pagination(t),
          ] else
            _platformPane(t),
        ],
        if (_c.selectedGroup != null) _detailCard(t, _c.selectedGroup!),
        if (_c.message.isNotEmpty) ToastBar(text: _c.message, ok: false),
      ],
    );
  }

  // ── 同步状态卡（`SyncStatusCard.tsx`）──
  Widget _syncCard(I18nController t) {
    final theme = AidogTheme.of(context);
    final s = _c.settings;
    final result = _c.syncResult;
    return Tile(
      title: t.t('modelInfo.syncTitle'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              SmallButton(
                label: _c.syncing
                    ? t.t('modelInfo.syncing')
                    : t.t('modelInfo.syncNow'),
                onTap: _c.syncing ? null : _c.sync,
              ),
              const Spacer(),
              Text(
                '${t.t('modelInfo.lastSync')}: '
                '${s.lastSyncAt != 0 ? ltr(formatDateTime(s.lastSyncAt)) : '-'}',
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              ),
            ],
          ),
          // DB 还没同步数据时，展示的是编译期内置 registry 兜底 —— 要告诉用户。
          if (_c.bundled)
            Text(
              t.t('modelInfo.bundledNotice'),
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
          const SizedBox(height: AidogSpace.ssm),
          Wrap(
            spacing: AidogSpace.ssm,
            runSpacing: AidogSpace.sxs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Switch(
                value: s.autoSyncEnabled,
                onChanged: (v) =>
                    _c.updateSettings(s.copyWith(autoSyncEnabled: v)),
              ),
              Text(
                t.t('modelInfo.autoSync'),
                style: AidogType.micro.copyWith(color: theme.c.fg),
              ),
              // 间隔只在自动同步打开时才出现（React 的条件渲染）。
              // 一排裸时长读不出是什么，React 在它们前面写着「间隔」
              //（`SyncStatusCard.tsx:83`）。
              if (s.autoSyncEnabled) TileMeta(t.t('modelInfo.interval')),
              // React 是窄下拉（`SyncStatusCard.tsx:87-100`），不是一排按钮。
              if (s.autoSyncEnabled)
                MiniSelect(
                  key: const ValueKey('model-sync-interval'),
                  value: '${s.syncIntervalSecs}',
                  options: [for (final secs in kSyncIntervalOptions) '$secs'],
                  labelOf: (v) => ltr(_intervalLabel(int.parse(v))),
                  onChanged: (v) => _c.updateSettings(
                    s.copyWith(syncIntervalSecs: int.parse(v!)),
                  ),
                ),
            ],
          ),
          const SizedBox(height: AidogSpace.ssm),
          Wrap(
            spacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              TileMeta(t.t('modelInfo.fallback')),
              Text(
                t.t('modelInfo.input'),
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
              // 单价要反复试，`step: 0.1` 那对箭头是 React 就有的
              // （`SyncStatusCard.tsx:111-125` 的 `type="number" min=0 step=0.1`）。
              // 原先是纯文本框 + `?? 0`：敲错一个字符，兜底单价静默变 0，
              // 之后所有未登记模型都按 0 计价。
              NumberInput(
                key: const Key('fallback-input'),
                width: 96,
                decimal: true,
                min: 0,
                step: 0.1,
                value: '${s.fallbackInputPrice}',
                onChanged: (v) {
                  final n = double.tryParse(v);
                  if (n != null) {
                    _c.updateSettings(s.copyWith(fallbackInputPrice: n));
                  }
                },
              ),
              Text(
                t.t('modelInfo.output'),
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
              NumberInput(
                key: const Key('fallback-output'),
                width: 96,
                decimal: true,
                min: 0,
                step: 0.1,
                value: '${s.fallbackOutputPrice}',
                onChanged: (v) {
                  final n = double.tryParse(v);
                  if (n != null) {
                    _c.updateSettings(s.copyWith(fallbackOutputPrice: n));
                  }
                },
              ),
            ],
          ),
          if (result != null) ...[
            const SizedBox(height: AidogSpace.ssm),
            Text(
              t
                  .t('modelInfo.syncResult')
                  .replaceAll('{added}', '${result.added}')
                  .replaceAll('{updated}', '${result.updated}')
                  .replaceAll('{failed}', '${result.failed}')
                  .replaceAll('{total}', '${result.total}'),
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
            // partial 失败清单：文件 + 原因逐条列出，不合并成一句「部分失败」。
            if (result.failures.isNotEmpty) ...[
              Text(
                t
                    .t('modelInfo.failuresTitle')
                    .replaceAll('{count}', '${result.failures.length}'),
                style: AidogType.micro.copyWith(color: theme.c.bad),
              ),
              for (final f in result.failures)
                Text(
                  '${f.file} — ${f.error}',
                  style: AidogType.micro.copyWith(color: theme.c.fg2),
                ),
            ],
          ],
        ],
      ),
    );
  }

  static String _intervalLabel(int secs) => switch (secs) {
    3600 => '1h',
    21600 => '6h',
    43200 => '12h',
    86400 => '24h',
    604800 => '7d',
    _ => '${secs}s',
  };

  // ── 筛选栏 ──
  Widget _filterBar(I18nController t) {
    final theme = AidogTheme.of(context);
    return Tile(
      child: Wrap(
        spacing: AidogSpace.ssm,
        runSpacing: AidogSpace.sxs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          SizedBox(
            width: 200,
            child: TextField(
              key: const Key('model-info-search'),
              decoration: InputDecoration(
                isDense: true,
                hintText: t.t('modelInfo.searchPlaceholder'),
              ),
              style: AidogType.label.copyWith(color: theme.c.fg),
              onChanged: _c.setQuery,
            ),
          ),
          // 平台筛选走下拉，不是一排按钮。
          //
          // 原先是 `platformCodes.take(12)` 铺成按钮 —— registry 现在有 65 个协议，
          // **第 13 个往后在界面上完全没有入口**，筛不到就是筛不到。
          // React 是全量下拉（`ModelInfoTab.tsx:224-234`），这里照它做，
          // 顺带拿到搜索框（65 项靠肉眼找也不现实）。
          FilterDropdown(
            width: 160,
            value: _c.platformFilter,
            onChanged: _c.setPlatformFilter,
            allLabel: t.t('modelInfo.allPlatforms'),
            searchPlaceholder: t.t('modelInfo.allPlatforms'),
            emptyLabel: t.t('modelInfo.empty'),
            options: [
              for (final code in _c.platformCodes)
                FilterOption(value: code, label: _c.platformLabel(code)),
            ],
          ),
          // 一个下拉而不是十几颗平铺按钮（`ModelInfoTab.tsx:235-237`）：
          // 平铺会把筛选栏撑成好几行，真正在筛什么反而看不出来。
          FilterDropdown(
            width: 160,
            value: _c.capabilityFilter,
            onChanged: _c.setCapabilityFilter,
            allLabel: t.t('modelInfo.allCapabilities'),
            searchPlaceholder: t.t('modelInfo.allCapabilities'),
            emptyLabel: t.t('modelInfo.empty'),
            options: [
              for (final cap in kCapabilities)
                FilterOption(value: cap, label: capabilityLabel(t.t, cap)),
            ],
          ),
          // 开关与文字是同一个点击区（React 把两者包在一个 `<label>` 里，
          // `ModelInfoTab.tsx:246-249`）：原先点文字没反应，想关掉「仅官方」
          // 只能精准点那个小滑块。控件也换成全应用统一的 [AidogSwitch]，
          // 这里原先是 Material 默认样式的 `Switch`，与其余开关长得不一样。
          InkWell(
            key: const ValueKey('model-info-official-only'),
            onTap: () => _c.setOfficialOnly(!_c.officialOnly),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: AidogSpace.sxs,
                vertical: 2,
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  AidogSwitch(
                    value: _c.officialOnly,
                    onChanged: () => _c.setOfficialOnly(!_c.officialOnly),
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  Text(
                    t.t('modelInfo.officialOnly'),
                    style: AidogType.micro.copyWith(color: theme.c.fg2),
                  ),
                ],
              ),
            ),
          ),
          // 「清除筛选」只在真有筛选时才出现。
          if (_c.hasFilter)
            SmallButton(
              label: t.t('modelInfo.clearFilter'),
              onTap: _c.clearFilter,
            ),
        ],
      ),
    );
  }

  // ── 模型维度表 ──
  Widget _modelTable(I18nController t) {
    final theme = AidogTheme.of(context);
    return ListingTile.table(
      columns: [
        t.t('modelInfo.colModel'),
        t.t('modelInfo.colPlatform'),
        t.t('modelInfo.colCapabilities'),
        t.t('modelInfo.colContext'),
        t.t('modelInfo.colInput'),
        t.t('modelInfo.colOutput'),
      ],
      cells: [
        for (final g in _c.pageRows)
          () {
            final primary = g.primary;
            final price = parsePriceData(primary?.priceData ?? '');
            final extra = g.entries.length - 1;
            return <Widget>[
              InkWell(
                onTap: () => _c.select(g.canonicalModel),
                child: _ModelNameCell(canonicalModel: g.canonicalModel),
              ),
              Text(
                '${primary == null ? '-' : _c.platformLabel(primary.platformCode)}'
                '${primary?.official == true ? ' · ${t.t('modelInfo.official')}' : ''}'
                // 第二个及以后的平台折叠成「还有 N 个平台」。
                '${extra > 0 ? ' · ${t.t('modelInfo.morePlatforms').replaceAll('{count}', '$extra')}' : ''}',
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
              Text(
                _capText(t, primary?.capabilities ?? const []),
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
              Text(
                ltr(fmtTokens(primary?.contextWindow)),
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
              Text(
                ltr(fmtPricePerM(price.input)),
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
              Text(
                ltr(fmtPricePerM(price.output)),
                style: AidogType.micro.copyWith(color: theme.c.fg2),
              ),
            ];
          }(),
      ],
    );
  }

  /// 能力徽标：空清单渲染 `-`，未知枚举原样显示裸值。
  String _capText(I18nController t, List<String> caps) => caps.isEmpty
      ? '-'
      : [for (final c in caps) capabilityLabel(t.t, c)].join(' · ');

  // ── 分页 ──
  Widget _pagination(I18nController t) {
    final theme = AidogTheme.of(context);
    final total = _c.filtered.length;
    final rangeStart = total == 0 ? 0 : (_c.currentPage - 1) * _c.pageSize + 1;
    final rangeEndRaw = _c.currentPage * _c.pageSize;
    final rangeEnd = rangeEndRaw < total ? rangeEndRaw : total;
    return Tile(
      child: Wrap(
        spacing: AidogSpace.sxs,
        runSpacing: AidogSpace.sxs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Text(
            ltr('$rangeStart–$rangeEnd / $total'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          // React 是窄下拉（`Pagination.tsx:56-66`）。
          MiniSelect(
            key: const ValueKey('model-page-size'),
            value: '${_c.pageSize}',
            options: [for (final ps in kPageSizeOptions) '$ps'],
            labelOf: (v) => ltr('${int.parse(v)}/page'),
            onChanged: (v) => _c.setPageSize(int.parse(v!)),
          ),
          SmallButton(
            label: '⟪',
            onTap: _c.currentPage <= 1 ? null : () => _c.setPage(1),
          ),
          SmallButton(
            label: '←',
            onTap: _c.currentPage <= 1
                ? null
                : () => _c.setPage(_c.currentPage - 1),
          ),
          for (final p in paginationPages(_c.currentPage, _c.totalPages))
            p == null
                ? Text('…', style: AidogType.micro.copyWith(color: theme.c.fg3))
                : SmallButton(
                    label: ltr('$p'),
                    active: p == _c.currentPage,
                    onTap: () => _c.setPage(p),
                  ),
          SmallButton(
            label: '→',
            onTap: _c.currentPage >= _c.totalPages
                ? null
                : () => _c.setPage(_c.currentPage + 1),
          ),
          SmallButton(
            label: '⟫',
            onTap: _c.currentPage >= _c.totalPages
                ? null
                : () => _c.setPage(_c.totalPages),
          ),
          SizedBox(
            width: 60,
            // 跳页只收数字（React 是 `type="number" min=1 max=totalPages`）。
            child: TextField(
              key: const Key('model-info-jump'),
              decoration: const InputDecoration(isDense: true, hintText: '#'),
              keyboardType: TextInputType.number,
              inputFormatters: [FilteringTextInputFormatter.digitsOnly],
              style: AidogType.label.copyWith(color: theme.c.fg),
              onChanged: _c.setJumpPage,
              onSubmitted: (_) => _c.jump(),
            ),
          ),
          SmallButton(label: 'Go', onTap: _c.jump),
        ],
      ),
    );
  }

  // ── 平台维度 ──
  Widget _platformPane(I18nController t) {
    final theme = AidogTheme.of(context);
    final entries = _c.activePlatformEntries;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 240,
          child: Tile(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final code in _c.visiblePlatformCodes)
                  InkWell(
                    onTap: () => _c.selectPlatform(code),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 4),
                      child: Row(
                        children: [
                          Expanded(
                            child: Text(
                              _c.platformLabel(code),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: AidogType.micro.copyWith(
                                color: _c.activePlatform == code
                                    ? theme.c.accentText
                                    : theme.c.fg2,
                              ),
                            ),
                          ),
                          Text(
                            ltr('${_c.byPlatform[code]?.length ?? 0}'),
                            style: AidogType.micro.copyWith(color: theme.c.fg3),
                          ),
                        ],
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
        const SizedBox(width: AidogSpace.ssm),
        Expanded(
          child: _c.activePlatform == null || entries.isEmpty
              ? CenteredNote(text: t.t('modelInfo.selectPlatform'))
              : ListingTile.table(
                  columns: [
                    t.t('modelInfo.colModel'),
                    t.t('modelInfo.colCapabilities'),
                    t.t('modelInfo.colContext'),
                    t.t('modelInfo.colInput'),
                    t.t('modelInfo.colOutput'),
                  ],
                  cells: [
                    for (final e in entries)
                      () {
                        final price = parsePriceData(e.priceData);
                        return <Widget>[
                          _ModelNameCell(canonicalModel: e.canonicalModel),
                          Text(
                            _capText(t, e.capabilities),
                            style: AidogType.micro.copyWith(color: theme.c.fg2),
                          ),
                          Text(
                            ltr(fmtTokens(e.contextWindow)),
                            style: AidogType.micro.copyWith(color: theme.c.fg2),
                          ),
                          Text(
                            ltr(fmtPricePerM(price.input)),
                            style: AidogType.micro.copyWith(color: theme.c.fg2),
                          ),
                          Text(
                            ltr(fmtPricePerM(price.output)),
                            style: AidogType.micro.copyWith(color: theme.c.fg2),
                          ),
                        ];
                      }(),
                  ],
                ),
        ),
      ],
    );
  }

  // ── 详情（按平台分 tab 比价）──
  Widget _detailCard(I18nController t, ModelEntryGroup g) {
    final theme = AidogTheme.of(context);
    final entries = _c.detailEntries(g);
    final title = nameParts(g.displayName, g.canonicalModel);
    final active = _c.activeDetailEntry(g);
    if (active == null) return const SizedBox.shrink();
    // React 是普通 `Dialog`（`ModelDetailDialog.tsx:43`，maxWidth 720），点遮罩可关。
    return AidogModal(
      maxWidth: 720,
      onBarrierTap: () => _c.select(null),
      child: Tile(
        title: title.primary,
        meta: title.secondary == null
            ? null
            : '${t.t('modelInfo.canonical')}: ${title.secondary}',
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Wrap(
              spacing: AidogSpace.sxs,
              runSpacing: AidogSpace.sxs,
              children: [
                for (final e in entries)
                  SmallButton(
                    // 同平台多 SKU：model_id ≠ canonical 时补后缀区分，
                    // 否则三条 SKU 的 tab 会长得一模一样（已修过的 bug）。
                    label:
                        '${_c.platformLabel(e.platformCode)}'
                        '${e.modelId != g.canonicalModel ? '· ${e.modelId}' : ''}'
                        '${_c.pricingOnly.contains(e.platformCode) ? ' ${t.t('modelInfo.priceRefOnly')}' : ''}',
                    active:
                        ModelInfoController.detailTabKey(e) ==
                        ModelInfoController.detailTabKey(active),
                    onTap: () =>
                        _c.setDetailTab(ModelInfoController.detailTabKey(e)),
                  ),
                SmallButton(
                  label: t.t('action.close'),
                  onTap: () => _c.select(null),
                ),
              ],
            ),
            const SizedBox(height: AidogSpace.ssm),
            _entryDetail(t, theme, active),
          ],
        ),
      ),
    );
  }

  Widget _entryDetail(I18nController t, AidogTheme theme, ModelEntry e) {
    final price = parsePriceData(e.priceData);
    final flags = parseEntryFlags(e.priceData);
    // 未标注 = "-"（不与 false 混同）。
    String flag(bool? v) => v == null
        ? '-'
        : v
        ? t.t('common.yes')
        : t.t('common.no');
    final secondary = nameParts(e.displayName, e.modelId).secondary;

    Widget field(String label, String value) => Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 120,
            child: Text(
              label,
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
          ),
        ],
      ),
    );

    String priceRow(PriceTier? tier) =>
        '${t.t('modelInfo.colInput')}: ${fmtPricePerM(tier?.input)}  '
        '${t.t('modelInfo.colOutput')}: ${fmtPricePerM(tier?.output)}  '
        '${t.t('modelInfo.colCacheRead')}: ${fmtPricePerM(tier?.cacheRead)}';

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        TileMeta(t.t('modelInfo.basics')),
        Row(
          children: [
            Expanded(child: field(t.t('modelInfo.requestName'), e.modelId)),
            SmallButton(
              label: t.t('modelInfo.copyRequestName'),
              onTap: () => native.writeText(e.modelId),
            ),
          ],
        ),
        // 展示名与请求名同串时不重复出一行（回落已在后端读取层）。
        if (secondary != null)
          field(t.t('modelInfo.displayName'), e.displayName),
        field(
          t.t('modelInfo.official'),
          e.official
              ? t.t('modelInfo.officialYes')
              : t.t('modelInfo.officialNo'),
        ),
        TileMeta(t.t('modelInfo.versionChain')),
        field(t.t('modelInfo.family'), e.family.isEmpty ? '-' : e.family),
        field(t.t('modelInfo.version'), e.version.isEmpty ? '-' : e.version),
        field(
          t.t('modelInfo.predecessor'),
          e.predecessor.isEmpty ? '-' : e.predecessor,
        ),
        TileMeta(t.t('modelInfo.capabilities')),
        field('', _capText(t, e.capabilities)),
        field(
          t.t('modelInfo.thinkingSupported'),
          flag(flags.thinkingSupported),
        ),
        field(
          t.t('modelInfo.thinkingToggleable'),
          flag(flags.thinkingToggleable),
        ),
        field(
          t.t('modelInfo.builtinTools'),
          e.builtinToolsExcluded.isEmpty
              ? t.t('modelInfo.builtinToolsAll')
              : e.builtinToolsExcluded.join(', '),
        ),
        TileMeta(t.t('modelInfo.limits')),
        field(t.t('modelInfo.maxInput'), fmtTokens(e.maxInputTokens)),
        field(t.t('modelInfo.maxOutput'), fmtTokens(e.maxOutputTokens)),
        field(t.t('modelInfo.contextWindow'), fmtTokens(e.contextWindow)),
        TileMeta(t.t('modelInfo.prices')),
        // 非 token 计价（图像 / 视频 / 搜索）走 $/unit，token 价字段不适用。
        field(
          t.t('modelInfo.priceDefault'),
          price.unit != null && price.unit != 'token'
              ? fmtPricePerUnit(price.unitPrice, price.unit)
              : priceRow(price),
        ),
        if (price.peak != null)
          field(t.t('modelInfo.pricePeak'), priceRow(price.peak)),
        for (final tier in price.contextTiers)
          field(
            t
                .t('modelInfo.priceContextTier')
                .replaceAll('{tokens}', fmtTokens(tier.minTokens)),
            priceRow(tier),
          ),
      ],
    );
  }
}

/// 表格始终展示统一 canonical_model；请求名只在详情中展示。
class _ModelNameCell extends StatelessWidget {
  const _ModelNameCell({required this.canonicalModel});

  final String canonicalModel;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Text(
      canonicalModel,
      style: AidogType.micro.copyWith(color: theme.c.fg),
    );
  }
}
