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
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'filter_dropdown.dart';
import 'mini_select.dart';
import 'invoke.dart';
import 'model_info_logic.dart';
import 'platform_card_bits.dart';
import 'platform_logo.dart';
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
    // React 的 ModelInfoTab 顶部直接是同步状态卡（`ModelInfoTab.tsx:206`），
    // 没有 PageHead；节间距 16。
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        _syncCard(t),
        const SizedBox(height: 16),
        _filterBar(t),
        const SizedBox(height: 16),
        if (_c.loading)
          CenteredNote(text: t.t('status.loading'))
        else if (_c.groups.isEmpty)
          // 全页空态：`glass-surface` + padding 40 + 13（`ModelInfoTab.tsx:260-262`）。
          CenteredNote(text: t.t('modelInfo.empty'), padding: 40, fontSize: 13)
        else ...[
          // shadcn `TabsList` 是一条分段容器：bg-muted 底 + p-1 + rounded-lg，
          // active 项 bg-background + shadow（`ModelInfoTab.tsx:265-268`
          // → `src/components/ui/tabs.tsx:16,33`）。原先是两颗独立按钮，没有容器底。
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: Container(
              padding: const EdgeInsets.all(4),
              decoration: BoxDecoration(
                color: AidogTheme.of(context).c.surface2,
                borderRadius: BorderRadius.circular(AidogRadius.lg),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  _tabTrigger(t.t('modelInfo.tabModels'), 'models'),
                  _tabTrigger(t.t('modelInfo.tabPlatforms'), 'platforms'),
                ],
              ),
            ),
          ),
          // `TabsContent` 的 `mt-2` = 8（`src/components/ui/tabs.tsx:50`）。
          const SizedBox(height: 8),
          if (_c.tab == 'models') ...[
            _modelTable(t),
            const SizedBox(height: 16),
            _pagination(t),
          ] else
            _platformPane(t),
        ],
        if (_c.selectedGroup != null) _detailCard(t, _c.selectedGroup!),
        if (_c.message.isNotEmpty) ToastBar(text: _c.message, ok: false),
      ],
    );
  }

  /// 一颗 `TabsTrigger`：`px-3 py-1` = 12/4、14 w500，选中时 `bg-background` + 阴影
  /// （`src/components/ui/tabs.tsx:33`）。选中态不是 accent 底 —— 它是「纸抬起来」。
  Widget _tabTrigger(String label, String key) {
    final theme = AidogTheme.of(context);
    final on = _c.tab == key;
    return InkWell(
      key: ValueKey('model-info-tab-$key'),
      onTap: () => _c.setTab(key),
      borderRadius: BorderRadius.circular(AidogRadius.md),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        decoration: BoxDecoration(
          color: on ? theme.c.surface : null,
          borderRadius: BorderRadius.circular(AidogRadius.md),
          boxShadow: on ? theme.shadowTile : null,
        ),
        child: Text(
          label,
          style: AidogType.label.copyWith(
            fontSize: 14,
            fontWeight: FontWeight.w500,
            color: on ? theme.c.fg : theme.c.fg2,
          ),
        ),
      ),
    );
  }

  // ── 同步状态卡（`SyncStatusCard.tsx`）──
  Widget _syncCard(I18nController t) {
    final theme = AidogTheme.of(context);
    final s = _c.settings;
    final result = _c.syncResult;
    return Tile(
      title: t.t('modelInfo.syncTitle'),
      // React 在同步卡里带一句描述（`SyncStatusCard.tsx:46`）。
      meta: t.t('modelInfo.syncDesc'),
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
              AidogSwitch(
                value: s.autoSyncEnabled,
                compact: true,
                onChanged: () => _c.updateSettings(
                  s.copyWith(autoSyncEnabled: !s.autoSyncEnabled),
                ),
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
    // 筛选卡：`padding "10px 16px"` + 两向 `gap 10`（`ModelInfoTab.tsx:217`）。
    // 搜索框是 `flex "1 1 180px"`（同上 :222）—— 弹性撑满，不是固定宽，
    // 所以这里是 Row + Expanded，其余控件仍可自行折行。
    return Tile(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Expanded(
            child: ConstrainedBox(
              constraints: const BoxConstraints(minWidth: 180),
              child: SizedBox(
                height: 32,
                child: TextField(
                  key: const Key('model-info-search'),
                  decoration: InputDecoration(
                    isDense: true,
                    // React `padding: "6px 10px"`（同上 :222）。
                    contentPadding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 6,
                    ),
                    hintText: t.t('modelInfo.searchPlaceholder'),
                  ),
                  style: AidogType.label.copyWith(
                    fontSize: 12,
                    color: theme.c.fg,
                  ),
                  onChanged: _c.setQuery,
                ),
              ),
            ),
          ),
          const SizedBox(width: 10),
          Wrap(
            spacing: 10,
            runSpacing: 10,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
          // 平台筛选走下拉，不是一排按钮。
          //
          // 原先是 `platformCodes.take(12)` 铺成按钮 —— registry 现在有 65 个协议，
          // **第 13 个往后在界面上完全没有入口**，筛不到就是筛不到。
          // React 是全量下拉（`ModelInfoTab.tsx:224-234`），这里照它做，
          // 顺带拿到搜索框（65 项靠肉眼找也不现实）。
          // 触发器 `height 32, padding "6px 8px", fontSize 12`（同上 :225）。
          FilterDropdown(
            width: 160,
            height: 32,
            padX: 8,
            fontSize: 12,
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
          // 能力下拉 React 是 `width: 140`（同上 :236），比平台那颗窄。
          FilterDropdown(
            width: 140,
            height: 32,
            padX: 8,
            fontSize: 12,
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
                  // shadcn `Switch` 是 36×20（同上 :247），比缺省的 40×22 小一档。
                  AidogSwitch(
                    value: _c.officialOnly,
                    compact: true,
                    onChanged: () => _c.setOfficialOnly(!_c.officialOnly),
                  ),
                  // 开关↔文字 `gap: 6`、文字 12 正文色（同上 :246）。
                  const SizedBox(width: 6),
                  Text(
                    t.t('modelInfo.officialOnly'),
                    style: AidogType.micro.copyWith(
                      fontSize: 12,
                      letterSpacing: 0,
                      color: theme.c.fg,
                    ),
                  ),
                ],
              ),
            ),
          ),
          // 「清除筛选」只在真有筛选时才出现。
          // `fontSize 12, padding "4px 8px", color --text-tertiary` + 一枚
          // 11px 的 ✕（同上 :251-252）。
          if (_c.hasFilter)
            SmallButton(
              label: t.t('modelInfo.clearFilter'),
              icon: Icons.close,
              iconSize: 11,
              fontSize: 12,
              padding: (8, 4),
              color: theme.c.fg3,
              onTap: _c.clearFilter,
            ),
            ],
          ),
        ],
      ),
    );
  }

  // ── 模型维度表 ──
  Widget _modelTable(I18nController t) {
    return ListingTile.table(
      // 表头原文直出不大写、12 w600 fg2，全列左对齐，内衬 8/12，
      // 底色 `--primary 5%` 兑 surface（`ModelInfoTab.tsx:462-475`
      // + `src/styles/globals.css:290-292`）。
      uppercaseHeaders: false,
      startAlignAll: true,
      headerStyle: AidogType.micro.copyWith(
        fontSize: 12,
        letterSpacing: 0,
        fontWeight: FontWeight.w600,
        color: AidogTheme.of(context).c.fg2,
      ),
      // `--primary 5%` 兑 surface（`globals.css:290-292`）。原先回落到既有的
      // accentWash；用户 2026-09-25 裁决与 React 逐字对齐，这里直接按 `c.accent`
      // 5% 合成，不再换色。
      headerBackground: Color.alphaBlend(
        AidogTheme.of(context).c.accent.withValues(alpha: 0.05),
        AidogTheme.of(context).c.surface,
      ),
      cellPadding: const EdgeInsets.symmetric(vertical: 8, horizontal: 12),
      // 整行热区（`ModelInfoTab.tsx:343-347`），原先只有第一列的模型名可点。
      onRowTap: [
        for (final g in _c.pageRows) () => _c.select(g.canonicalModel),
      ],
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
              _ModelNameCell(canonicalModel: g.canonicalModel),
              _platformCell(t, primary, extra),
              _capBadges(t, primary?.capabilities ?? const []),
              _cellText(ltr(fmtTokens(primary?.contextWindow)), size: 12),
              _cellText(ltr(fmtPricePerM(price.input))),
              _cellText(ltr(fmtPricePerM(price.output))),
            ];
          }(),
      ],
    );
  }

  /// 表格数据格：`<Table style={{fontSize: F.hint}}>` = 13；「上下文」列显式 12
  /// 且用 secondary 色（`ModelInfoTab.tsx:273,371`）。
  Widget _cellText(String text, {double size = 13}) => Text(
    text,
    style: AidogType.micro.copyWith(
      fontSize: size,
      letterSpacing: 0,
      color: size == 13
          ? AidogTheme.of(context).c.fg
          : AidogTheme.of(context).c.fg2,
    ),
  );

  /// 平台列：logo 16 + 名称 12 + official 徽标 +「还有 N 个」11 tertiary，
  /// 四段各自成形（`ModelInfoTab.tsx:353-367`），不是拼成一行纯文本。
  Widget _platformCell(I18nController t, ModelEntry? primary, int extra) {
    final theme = AidogTheme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (primary != null) ...[
          ProtocolLogo(protocol: primary.platformCode, size: 16),
          const SizedBox(width: 6),
        ],
        Text(
          primary == null ? '-' : _c.platformLabel(primary.platformCode),
          style: AidogType.micro.copyWith(
            fontSize: 12,
            letterSpacing: 0,
            color: theme.c.fg,
          ),
        ),
        if (primary?.official == true) ...[
          const SizedBox(width: 6),
          // `Badge variant="secondary"`：10px / `1px 5px` / 无描边（同上 :359）。
          MiniBadge(
            text: t.t('modelInfo.official'),
            color: theme.c.fg2,
            background: theme.c.surface2,
            borderColor: Colors.transparent,
            padX: 5,
          ),
        ],
        if (extra > 0) ...[
          const SizedBox(width: 6),
          Text(
            t.t('modelInfo.morePlatforms').replaceAll('{count}', '$extra'),
            style: AidogType.micro.copyWith(
              fontSize: 11,
              letterSpacing: 0,
              color: theme.c.fg3,
            ),
          ),
        ],
      ],
    );
  }

  /// 能力徽标：一个能力一枚 `Badge`（10px / `1px 6px` / gap 4 / 无描边，
  /// `CapabilityBadges.tsx:28-34`）；空清单渲染 `-`，未知枚举原样显示裸值。
  Widget _capBadges(I18nController t, List<String> caps) {
    final theme = AidogTheme.of(context);
    if (caps.isEmpty) {
      return Text(
        '-',
        style: AidogType.micro.copyWith(
          fontSize: 13,
          letterSpacing: 0,
          color: theme.c.fg3,
        ),
      );
    }
    return Wrap(
      spacing: 4,
      runSpacing: 4,
      children: [
        for (final c in caps)
          MiniBadge(
            text: capabilityLabel(t.t, c),
            color: theme.c.fg2,
            background: theme.c.surface2,
            borderColor: Colors.transparent,
            padX: 6,
          ),
      ],
    );
  }

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
        // React 平台清单：220 宽、maxHeight 520、内 padding 6；选中项
        // accent-subtle 底 + 左缘 2px accent 竖线 + 文字提权
        //（`ModelInfoTab.tsx:422-446`）。
        SizedBox(
          width: 220,
          child: Tile(
            padding: const EdgeInsets.all(6),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 520),
              child: SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    for (final code in _c.visiblePlatformCodes)
                      InkWell(
                        key: ValueKey('model-info-platform-$code'),
                        borderRadius: BorderRadius.circular(AidogRadius.sm),
                        onTap: () => _c.selectPlatform(code),
                        child: Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 8,
                            vertical: 6,
                          ),
                          decoration: BoxDecoration(
                            color: _c.activePlatform == code
                                ? theme.c.accentWash
                                : null,
                            borderRadius: BorderRadius.circular(AidogRadius.sm),
                            border: BorderDirectional(
                              start: BorderSide(
                                color: _c.activePlatform == code
                                    ? theme.c.accent
                                    : Colors.transparent,
                                width: 2,
                              ),
                            ),
                          ),
                          child: Row(
                            children: [
                              // 行首 16px logo（`ModelInfoTab.tsx:414`），原先只有名字。
                              ProtocolLogo(protocol: code, size: 16),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(
                                  _c.platformLabel(code),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: AidogType.caption.copyWith(
                                    fontSize: 12,
                                    fontWeight: _c.activePlatform == code
                                        ? FontWeight.w600
                                        : FontWeight.w400,
                                    color: _c.activePlatform == code
                                        ? theme.c.accentText
                                        : theme.c.fg2,
                                  ),
                                ),
                              ),
                              Text(
                                ltr('${_c.byPlatform[code]?.length ?? 0}'),
                                style: AidogType.caption.copyWith(
                                  fontSize: 11,
                                  color: theme.c.fg3,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
        // 左右两栏 `gap: 12`（`ModelInfoTab.tsx:396`）。
        const SizedBox(width: 12),
        Expanded(
          child: _c.activePlatform == null || entries.isEmpty
              // 右侧空态 `padding 30, fontSize 13`（同上 :425-427）。
              ? CenteredNote(
                  text: t.t('modelInfo.selectPlatform'),
                  padding: 30,
                  fontSize: 13,
                )
              : ListingTile.table(
                  uppercaseHeaders: false,
                  startAlignAll: true,
                  headerStyle: AidogType.micro.copyWith(
                    fontSize: 12,
                    letterSpacing: 0,
                    fontWeight: FontWeight.w600,
                    color: theme.c.fg2,
                  ),
                  headerBackground: theme.c.accentWash,
                  cellPadding: const EdgeInsets.symmetric(
                    vertical: 8,
                    horizontal: 12,
                  ),
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
                          _capBadges(t, e.capabilities),
                          _cellText(ltr(fmtTokens(e.contextWindow)), size: 12),
                          _cellText(ltr(fmtPricePerM(price.input))),
                          _cellText(ltr(fmtPricePerM(price.output))),
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
    final active = _c.activeDetailEntry(g);
    if (active == null) return const SizedBox.shrink();
    // React 是普通 `Dialog`（`ModelDetailDialog.tsx:43`，maxWidth 720），点遮罩可关。
    return AidogModal(
      maxWidth: 720,
      onBarrierTap: () => _c.select(null),
      // `maxHeight: "82vh"` + `overflow: auto`（`ModelDetailDialog.tsx:42`）：
      // 内容长了在面板内滚，不把弹窗撑出屏幕。
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight: MediaQuery.sizeOf(context).height * 0.82,
        ),
        child: ModalCard(
          // `DialogContent` 自带 ✕（`ui/dialog.tsx:47-50`）。
          onClose: () => _c.select(null),
          // 这个弹窗没挂 glass-elevated，圆角是 `sm:rounded-lg` = 16；
          // 标题 `text-lg` 18 w600，内容恒是 `<code>{canonical_model}</code>`
          //（`ModelDetailDialog.tsx:42-46`）—— 等宽，且没有副标题。
          radius: AidogRadius.lg,
          titleStyle: AidogType.numSm.copyWith(
            fontSize: 18,
            fontWeight: FontWeight.w600,
          ),
          title: g.canonicalModel,
          child: Flexible(
            child: SingleChildScrollView(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: [
                  // `TabsList`：bg-muted 底 + p-1 + rounded-lg，可折行
                  //（`ModelDetailDialog.tsx:50`）。
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: Container(
                      padding: const EdgeInsets.all(4),
                      decoration: BoxDecoration(
                        color: theme.c.surface2,
                        borderRadius: BorderRadius.circular(AidogRadius.lg),
                      ),
                      child: Wrap(
                        spacing: 2,
                        runSpacing: 2,
                        children: [
                          for (final e in entries) _detailTab(t, g, e, active),
                        ],
                      ),
                    ),
                  ),
                  const SizedBox(height: AidogSpace.sxs),
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: SmallButton(
                      label: t.t('action.close'),
                      onTap: () => _c.select(null),
                    ),
                  ),
                  const SizedBox(height: AidogSpace.ssm),
                  _entryDetail(t, theme, active),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  /// 一颗平台 tab：14px logo + 平台名 12 + `· model_id` 后缀 10 tertiary
  /// （`ModelDetailDialog.tsx:52-62`）。后缀独立成段，不与平台名同色同字号。
  Widget _detailTab(
    I18nController t,
    ModelEntryGroup g,
    ModelEntry e,
    ModelEntry active,
  ) {
    final theme = AidogTheme.of(context);
    final key = ModelInfoController.detailTabKey(e);
    final on = key == ModelInfoController.detailTabKey(active);
    return InkWell(
      onTap: () => _c.setDetailTab(key),
      borderRadius: BorderRadius.circular(AidogRadius.md),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        decoration: BoxDecoration(
          color: on ? theme.c.surface : null,
          borderRadius: BorderRadius.circular(AidogRadius.md),
          boxShadow: on ? theme.shadowTile : null,
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            ProtocolLogo(protocol: e.platformCode, size: 14),
            const SizedBox(width: 5),
            Text(
              _c.platformLabel(e.platformCode),
              style: AidogType.micro.copyWith(
                fontSize: 12,
                letterSpacing: 0,
                color: on ? theme.c.fg : theme.c.fg2,
              ),
            ),
            // 同平台多 SKU：model_id ≠ canonical 时补后缀区分，
            // 否则三条 SKU 的 tab 会长得一模一样（已修过的 bug）。
            if (e.modelId != g.canonicalModel)
              Text(
                ' · ${e.modelId}',
                style: AidogType.micro.copyWith(
                  fontSize: 10,
                  letterSpacing: 0,
                  color: theme.c.fg3,
                ),
              ),
            if (_c.pricingOnly.contains(e.platformCode))
              Text(
                ' ${t.t('modelInfo.priceRefOnly')}',
                style: AidogType.micro.copyWith(
                  fontSize: 10,
                  letterSpacing: 0,
                  color: theme.c.fg3,
                ),
              ),
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

    // 一行字段：`gap 10`、13、标签 minWidth 96 tertiary（`ModelDetailDialog.tsx:193-195`）。
    Widget fieldOf(String label, Widget value) => Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ConstrainedBox(
          constraints: const BoxConstraints(minWidth: 96),
          child: Text(
            label,
            style: AidogType.micro.copyWith(
              fontSize: 13,
              letterSpacing: 0,
              color: theme.c.fg3,
            ),
          ),
        ),
        const SizedBox(width: 10),
        Expanded(child: value),
      ],
    );

    Widget valueText(String value) => Text(
      value,
      style: AidogType.micro.copyWith(
        fontSize: 13,
        letterSpacing: 0,
        color: theme.c.fg,
      ),
    );

    Widget field(String label, String value) =>
        fieldOf(label, valueText(value));

    // 三段价格是三个独立片段、`gap: 12`（同上 :159-167），不是一串空格分隔的文本。
    Widget priceRow(PriceTier? tier) => Wrap(
      spacing: 12,
      runSpacing: 4,
      children: [
        for (final seg in [
          '${t.t('modelInfo.colInput')}: ${fmtPricePerM(tier?.input)}',
          '${t.t('modelInfo.colOutput')}: ${fmtPricePerM(tier?.output)}',
          '${t.t('modelInfo.colCacheRead')}: ${fmtPricePerM(tier?.cacheRead)}',
        ])
          valueText(seg),
      ],
    );

    // 一个分区 = 标题（12 w700 fg2，不大写）+ 半透明面板，面板内字段 gap 6
    //（同上 :174-188）。原先分区标题走 TileMeta（micro 全大写）、字段裸排无面板。
    Widget section(String title, List<Widget> fields) => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          title,
          style: AidogType.micro.copyWith(
            fontSize: 12,
            letterSpacing: 0,
            fontWeight: FontWeight.w700,
            color: theme.c.fg2,
          ),
        ),
        const SizedBox(height: 6),
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          decoration: BoxDecoration(
            color: theme.c.surface.withValues(alpha: 0.6),
            border: Border.all(color: theme.c.line.withValues(alpha: 0.4)),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              for (var i = 0; i < fields.length; i++) ...[
                if (i > 0) const SizedBox(height: 6),
                fields[i],
              ],
            ],
          ),
        ),
      ],
    );

    return Padding(
      // 正文区 `paddingTop: 8`（同上 :89）。
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: _gapped(14, [
                section(t.t('modelInfo.basics'), [
                  fieldOf(
                    t.t('modelInfo.requestName'),
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Flexible(child: valueText(e.modelId)),
                        const SizedBox(width: 4),
                        // `CopyButton size={12}` 紧贴 `<code>` 右侧（同上 :92-95），
                        // 原先是整行右端一颗「复制请求名」文字按钮。
                        IconButton(
                          icon: const Icon(Icons.copy_outlined, size: 12),
                          color: theme.c.fg3,
                          padding: EdgeInsets.zero,
                          splashRadius: 12,
                          constraints: const BoxConstraints.tightFor(
                            width: 18,
                            height: 18,
                          ),
                          tooltip: t.t('modelInfo.copyRequestName'),
                          onPressed: () => native.writeText(e.modelId),
                        ),
                      ],
                    ),
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
                ]),
                section(t.t('modelInfo.versionChain'), [
                  field(
                    t.t('modelInfo.family'),
                    e.family.isEmpty ? '-' : e.family,
                  ),
                  field(
                    t.t('modelInfo.version'),
                    e.version.isEmpty ? '-' : e.version,
                  ),
                  field(
                    t.t('modelInfo.predecessor'),
                    e.predecessor.isEmpty ? '-' : e.predecessor,
                  ),
                ]),
                section(t.t('modelInfo.capabilities'), [
                  _capBadges(t, e.capabilities),
                  field(
                    t.t('modelInfo.thinkingSupported'),
                    flag(flags.thinkingSupported),
                  ),
                  field(
                    t.t('modelInfo.thinkingToggleable'),
                    flag(flags.thinkingToggleable),
                  ),
                  fieldOf(
                    t.t('modelInfo.builtinTools'),
                    e.builtinToolsExcluded.isEmpty
                        ? valueText(t.t('modelInfo.builtinToolsAll'))
                        // 一个工具一枚 10px 徽标（同上 :117-120），不是逗号串。
                        : Wrap(
                            spacing: 4,
                            runSpacing: 4,
                            children: [
                              for (final tool in e.builtinToolsExcluded)
                                MiniBadge(
                                  text: tool,
                                  color: theme.c.fg2,
                                  background: theme.c.surface2,
                                  borderColor: Colors.transparent,
                                ),
                            ],
                          ),
                  ),
                ]),
                section(t.t('modelInfo.limits'), [
                  field(t.t('modelInfo.maxInput'), fmtTokens(e.maxInputTokens)),
                  field(
                    t.t('modelInfo.maxOutput'),
                    fmtTokens(e.maxOutputTokens),
                  ),
                  field(
                    t.t('modelInfo.contextWindow'),
                    fmtTokens(e.contextWindow),
                  ),
                ]),
                section(t.t('modelInfo.prices'), [
                  // 非 token 计价（图像 / 视频 / 搜索）走 $/unit，token 价字段不适用。
                  fieldOf(
                    t.t('modelInfo.priceDefault'),
                    price.unit != null && price.unit != 'token'
                        ? valueText(
                            fmtPricePerUnit(price.unitPrice, price.unit),
                          )
                        : priceRow(price),
                  ),
                  if (price.peak != null)
                    fieldOf(t.t('modelInfo.pricePeak'), priceRow(price.peak)),
                  for (final tier in price.contextTiers)
                    fieldOf(
                      t
                          .t('modelInfo.priceContextTier')
                          .replaceAll('{tokens}', fmtTokens(tier.minTokens)),
                      priceRow(tier),
                    ),
                ]),
        ]),
      ),
    );
  }

  /// 在相邻元素之间插等距间隔（React 那边是 flex `gap`）。
  static List<Widget> _gapped(double gap, List<Widget> items) => [
    for (var i = 0; i < items.length; i++) ...[
      if (i > 0) SizedBox(height: gap),
      items[i],
    ],
  ];
}

/// 表格始终展示统一 canonical_model；请求名只在详情中展示。
class _ModelNameCell extends StatelessWidget {
  const _ModelNameCell({required this.canonicalModel});

  final String canonicalModel;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    // React 是 `<code>` → 等宽、12 w500（`ModelInfoTab.tsx:350`
    // → `ModelName.tsx:21`），不是 sans 的 micro。
    return Text(
      canonicalModel,
      style: AidogType.numSm.copyWith(
        fontSize: 12,
        fontWeight: FontWeight.w500,
        color: theme.c.fg,
      ),
    );
  }
}
