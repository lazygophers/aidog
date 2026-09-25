/// 四种格子 + 12 列 bento 网格。
///
/// 票 10 布局规则第 3 条：**只有四种格子** —— 读数 [ReadoutTile] / 序列 [SeriesTile] /
/// 清单 [ListingTile]（行列表与表格是它的两种渲染） / 操作 [ActionTile]。
/// 19 个页面由这四种排列而成，**不发明第五种**。页面票（I06-I09）从这里消费布局，
/// 不自己拼 Container。
///
/// 规则 2：格子宽度只取 3 / 4 / 5 / 6 / 7 / 8 / 12 列。
/// 规则 5：可比较的数字走 [numStyle]（等宽 + tabular figures），元信息走 micro 全大写。
/// 规则 6：方向无关；四类例外用 [Ltr] 显式钉死。
library;

import 'package:flutter/material.dart';

import 'theme.dart';

/// 规则 6 的四类显式 LTR 例外：① 快捷键符号 ② 图表时间轴与刻度 ③ URL / 端口 / 代码
/// ④ 以数字开头或结尾的短标签（RTL 下会被 bidi 重排，写 dir 才不乱）。
class Ltr extends StatelessWidget {
  const Ltr({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) =>
      Directionality(textDirection: TextDirection.ltr, child: child);
}

/// 等宽 + tabular figures。列对齐靠这条，不靠手调宽度（规则 5）。
TextStyle numStyle(TextStyle base, Color color) => base.copyWith(
  color: color,
  fontFeatures: const [FontFeature.tabularFigures()],
);

/// React 的 `.counter`（`src/styles/globals.css:954`）：**只加**
/// `font-variant-numeric: tabular-nums`，字体族仍是系统 sans。
///
/// 余额、StatChip 值这类位置在 React 里挂的是 `.counter`，不是等宽族 ——
/// 用 [numStyle] 会把它们换成 SF Mono，一眼就是另一个界面。
TextStyle counterStyle({
  required double fontSize,
  required Color color,
  FontWeight fontWeight = FontWeight.w700,
}) => TextStyle(
  fontFamily: AidogType.familySans,
  fontSize: fontSize,
  fontWeight: fontWeight,
  color: color,
  fontFeatures: const [FontFeature.tabularFigures()],
);

/// 格子右上角的元信息：micro 字阶 / 全大写 / fg-3。
class TileMeta extends StatelessWidget {
  const TileMeta(this.text, {super.key, this.icon});

  final String text;

  /// 行首图标。长表单里纯文字标题难扫读，React 各分区标题都带一个
  /// （`SandboxSection.tsx:234,280,333,379` 等的 `SvgIcon`）。
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final label = HighlightedText(
      text.toUpperCase(),
      style: AidogType.micro.copyWith(color: t.c.fg3),
      // 单行省略：长 meta 挤爆宿主行（弹窗标题行的段 desc 等）时截断，
      // 与 React 的 truncate/ellipsis 同路。
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
    );
    if (icon == null) return label;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 13, color: t.c.fg3),
        const SizedBox(width: 5),
        Flexible(child: label),
      ],
    );
  }
}

/// 表单字段标签：caption 字阶、正常大小写、fg-2。
///
/// React 的字段标签就是这个形态（formSections.tsx:199 的 12 secondary、
/// McpModals.tsx:250、GroupEditPanel.tsx:77 的 13、editors/_shared.tsx:134 的 15），
/// 从来不是全大写——之前全站拿 [TileMeta]（micro 11 + toUpperCase + ls 0.66）当
/// 字段标签用，三重差（大小写 / 字号 / 色阶）。[TileMeta] 留给格子右上角的元信息。
class FieldLabel extends StatelessWidget {
  const FieldLabel(this.text, {super.key, this.icon, this.fontSize});

  final String text;

  /// 行首图标（同 [TileMeta.icon] 的用法）。
  final IconData? icon;

  /// 字号覆盖。null = caption 12.5；React 各处是裸值 11 / 12 / 13
  /// （`SkillInstallView.tsx:444` 的 11、`McpModals.tsx:250` 的 12、
  /// `SkillsView.tsx:261` 的 13）。
  final double? fontSize;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final label = HighlightedText(
      text,
      style: AidogType.caption.copyWith(color: t.c.fg2, fontSize: fontSize),
    );
    if (icon == null) return label;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 13, color: t.c.fg3),
        const SizedBox(width: 5),
        Flexible(child: label),
      ],
    );
  }
}

/// 往子树里放一个「要高亮的搜索词」。设置页搜索时套在 section 外面，
/// 下面所有 [HighlightedText]（字段标签走的就是它）自动把命中的那段标出来。
/// 对齐 React 的 `Highlighted`（`editors/_shared.tsx:101-114`）：
/// 只标**第一处**、大小写不敏感。
class TextHighlight extends InheritedWidget {
  const TextHighlight({super.key, required this.query, required super.child});

  final String query;

  static String queryOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<TextHighlight>()?.query ?? '';

  @override
  bool updateShouldNotify(TextHighlight oldWidget) => query != oldWidget.query;
}

/// 普通 [Text]，但若祖先有 [TextHighlight] 且本段文字命中，就把命中那段
/// 加底色标出来。没有祖先 / 没命中时与 [Text] 完全一样。
class HighlightedText extends StatelessWidget {
  const HighlightedText(
    this.text, {
    super.key,
    this.style,
    this.maxLines,
    this.overflow,
  });

  final String text;
  final TextStyle? style;
  final int? maxLines;
  final TextOverflow? overflow;

  @override
  Widget build(BuildContext context) {
    final q = TextHighlight.queryOf(context).trim();
    if (q.isEmpty) {
      return Text(text, style: style, maxLines: maxLines, overflow: overflow);
    }
    final i = text.toLowerCase().indexOf(q.toLowerCase());
    if (i < 0) return Text(text, style: style);
    final theme = AidogTheme.of(context);
    return Text.rich(
      TextSpan(
        children: [
          TextSpan(text: text.substring(0, i)),
          TextSpan(
            text: text.substring(i, i + q.length),
            style: TextStyle(
              backgroundColor: theme.c.accent,
              color: theme.c.accentText,
            ),
          ),
          TextSpan(text: text.substring(i + q.length)),
        ],
      ),
      style: style,
      maxLines: maxLines,
      overflow: overflow,
    );
  }
}

/// 所有格子的底盘：surface + 1px line + radius-lg + tile 内边距 + shadow-tile。
///
/// [live] = 「只有活着的东西才被强调」（规则 4）。深色下 live-fill 偏暗 + halo 发光，
/// 浅色下 token 把 halo 置 none、靠更饱和的 fill 与更实的 edge 表达，这里不分支。
class Tile extends StatelessWidget {
  const Tile({
    super.key,
    this.title,
    this.meta,
    this.live = false,
    this.padding,
    this.leadingAccent,
    this.metaStyle,
    required this.child,
  });

  final String? title;
  final String? meta;

  /// meta 的字阶覆盖。给了就**不走 [TileMeta]**（也就不再全大写）——
  /// React 有些位置的 meta 是正常大小写的正文，如统计页趋势图副标题
  /// 「粒度：按小时」13 tertiary（`src/pages/Stats.tsx:730,748`）。
  final TextStyle? metaStyle;

  final bool live;
  final EdgeInsetsGeometry? padding;

  /// 卡自身**起始侧**的 2px 强调竖条，替换掉该侧的 1px `line`
  /// （React 通知卡的 `borderInlineStart: 2px solid var(--accent)`，
  /// `src/pages/Notifications.tsx:31`）。null = 四边都是 1px line。
  ///
  /// 画成条内第一个孩子而不是 `BorderDirectional(start: 2)`：非匀边 border
  /// 叠 borderRadius 在 Flutter 里直接抛断言（同 [InlineNote] 的做法）。
  final Color? leadingAccent;

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final head = title == null && meta == null
        ? null
        : Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.smd),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                if (title != null)
                  Expanded(
                    child: Text(
                      title!,
                      style: AidogType.tile.copyWith(color: t.c.fg),
                      overflow: TextOverflow.ellipsis,
                    ),
                  )
                else
                  const Spacer(),
                if (meta != null) ...[
                  const SizedBox(width: AidogSpace.smd),
                  if (metaStyle == null)
                    TileMeta(meta!)
                  else
                    Flexible(
                      child: Text(
                        meta!,
                        style: metaStyle,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                ],
              ],
            ),
          );
    final pad =
        padding ??
        const EdgeInsets.symmetric(
          horizontal: AidogLayout.tilePadX,
          vertical: AidogLayout.tilePadY,
        );
    final body = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [?head, child],
    );
    final accent = leadingAccent;
    return AnimatedContainer(
      duration: AidogMotion.base,
      curve: AidogMotion.easeStandard,
      clipBehavior: accent == null ? Clip.none : Clip.antiAlias,
      padding: accent == null ? pad : null,
      decoration: BoxDecoration(
        color: live ? t.c.liveFill : t.c.surface,
        border: Border.all(color: live ? t.c.liveEdge : t.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.md),
        boxShadow: t.shadowTile,
      ),
      child: accent == null
          ? body
          : IntrinsicHeight(
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Container(width: 2, color: accent),
                  Expanded(child: Padding(padding: pad, child: body)),
                ],
              ),
            ),
    );
  }
}

// ── 格子一：读数格（KPI）───────────────────────────────────────────
// 大等宽数字 + 环比 + sparkline。对照现仓 Stats.tsx = 读数格 × 8。

enum Trend { up, down, flat }

class ReadoutTile extends StatelessWidget {
  const ReadoutTile({
    super.key,
    required this.label,
    required this.value,
    this.delta,
    this.trend = Trend.flat,
    this.deltaNote,
    this.spark,
    this.live = false,
    this.padding,
    this.valueColor,
  });

  /// 小号说明（「费用」「缓存率」）。
  final String label;

  /// 已格式化好的读数本体；等宽显示，调用方负责格式化（对齐现仓 utils/formatters）。
  final String value;

  /// 环比，如 `+6.1%`。
  final String? delta;
  final Trend trend;

  /// 环比的基准说明，如「较昨日」。
  final String? deltaNote;

  /// sparkline 插槽 —— 图表由票 I04 提供，读数格只留位置，不自己画。
  final Widget? spark;
  final bool live;

  /// 卡内边距（React Stats Overview 卡是 16/20，其余读数卡走 Tile 缺省）。
  final EdgeInsetsGeometry? padding;

  /// 读数本体的颜色（React `levelColor(level)` 的色编码）。
  final Color? valueColor;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final deltaColor = switch (trend) {
      Trend.up => t.c.ok,
      Trend.down => t.c.bad,
      Trend.flat => t.c.fg3,
    };
    return Tile(
      live: live,
      padding: padding,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(label, style: AidogType.caption.copyWith(color: t.c.fg2)),
          const SizedBox(height: 2),
          // 以数字开头/结尾的读数在 RTL 下会被 bidi 重排 —— 规则 6 第 ④ 类。
          // React Overview 卡值：20 sans w700（`Stats.tsx:976` 的 F.title + 700），
          // 不是 28 mono —— 读数观感对齐现仓。
          Ltr(
            child: Text(
              value,
              style: AidogType.title.copyWith(
                fontSize: 20,
                fontWeight: FontWeight.w700,
                color: valueColor ?? t.c.fg,
              ),
            ),
          ),
          if (delta != null)
            Padding(
              padding: const EdgeInsets.only(top: AidogSpace.ssm),
              // deltaNote 是**悬浮提示**，不是常驻文字。
              // React 把它写在 `title=` 上（`Stats.tsx:965`），鼠标停上去才出现。
              // 原先这里当成一行文字画出来，统计页八张卡就一起多出八行「对比上一周期」
              // —— 每张卡都写着同一句话，全是噪声。
              child: Tooltip(
                message: deltaNote ?? '',
                child: Ltr(
                  child: Text(
                    delta!,
                    style: numStyle(AidogType.numSm, deltaColor),
                  ),
                ),
              ),
            ),
          if (spark != null)
            Padding(
              padding: const EdgeInsets.only(top: AidogSpace.ssm),
              child: SizedBox(height: 26, child: spark),
            ),
        ],
      ),
    );
  }
}

// ── 格子二：序列格（随时间 / 顺序变化的图）──────────────────────────
// 图本体由票 I04 提供；这里负责标题、元信息、图例，以及把时间轴钉成 LTR（规则 6 第 ② 类）。

class SeriesTile extends StatelessWidget {
  const SeriesTile({
    super.key,
    required this.title,
    this.meta,
    required this.chart,
    this.chartHeight = 150,
    this.legend = const [],
    this.live = false,
    this.metaStyle,
  });

  final String title;
  final String? meta;

  /// 透传 [Tile.metaStyle]（给了就不全大写）。
  final TextStyle? metaStyle;

  /// 图表插槽（I04 的 fl_chart / CustomPainter）。
  final Widget chart;
  final double chartHeight;

  /// 图例项：颜色 + 文案。
  final List<({Color color, String label})> legend;
  final bool live;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Tile(
      title: title,
      meta: meta,
      metaStyle: metaStyle,
      live: live,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 时间轴永远从左到右，RTL 下也不镜像。
          Ltr(
            child: SizedBox(height: chartHeight, child: chart),
          ),
          if (legend.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(top: AidogSpace.ssm),
              child: Wrap(
                spacing: AidogSpace.slg,
                runSpacing: AidogSpace.sxs,
                children: [
                  for (final l in legend)
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Container(
                          width: 9,
                          height: 9,
                          decoration: BoxDecoration(
                            color: l.color,
                            borderRadius: BorderRadius.circular(2),
                          ),
                        ),
                        const SizedBox(width: 5),
                        Text(
                          l.label,
                          style: AidogType.caption.copyWith(color: t.c.fg2),
                        ),
                      ],
                    ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

// ── 格子三：清单格（行列表 或 表格）──────────────────────────────────
// 两种渲染同属一类：默认构造 = 行列表（.prow），[ListingTile.table] = 表格。
// 对照现仓：Platforms.tsx = 清单格 + 操作格；Stats.tsx 的维度排行 = 表格形态。

class ListingTile extends StatelessWidget {
  /// 行列表形态：每行之间一条 hairline，最后一行无线。
  const ListingTile({
    super.key,
    this.title,
    this.meta,
    required this.rows,
    this.footer,
    this.live = false,
  }) : columns = null,
       cells = null,
       uppercaseHeaders = true,
       startAlignAll = false,
       headerStyle = null,
       headerBackground = null,
       cellPadding = null,
       onRowTap = null;

  /// 表格形态：表头 micro 全大写，首列 start 对齐、其余 end 对齐（数字列）。
  const ListingTile.table({
    super.key,
    this.title,
    this.meta,
    required List<String> this.columns,
    required List<List<Widget>> this.cells,
    this.footer,
    this.live = false,
    this.uppercaseHeaders = true,
    this.startAlignAll = false,
    this.headerStyle,
    this.headerBackground,
    this.cellPadding,
    this.onRowTap,
  }) : rows = const [];

  final String? title;
  final String? meta;
  final List<Widget> rows;
  final List<String>? columns;
  final List<List<Widget>>? cells;
  final Widget? footer;
  final bool live;

  /// 表头是否转大写。缺省 true；模型信息页的 `<Th>` 是原文直出
  /// （`src/pages/ModelInfo/ModelInfoTab.tsx:464-468`）。
  final bool uppercaseHeaders;

  /// 全列都按 start 对齐。缺省 false（首列 start、其余 end）；模型信息页两张表
  /// 的 `Th` / `Td` 都没写 `textAlign`，六列一律左对齐（同上 :462-475）。
  final bool startAlignAll;

  /// 表头字阶覆盖。null = 缺省 micro 11 fg3；模型信息页是 12 w600 fg2（同上 :465-466）。
  final TextStyle? headerStyle;

  /// 表头行底色。null = 无底；React `.glass-table thead th` 是 accent 5% 兑 surface
  /// （`src/styles/globals.css:290-292`）。
  final Color? headerBackground;

  /// 表头与单元格的内衬覆盖。null = 缺省（表头 7/6、单元格 rowH/6）；
  /// 模型信息页两处都是 `padding: "8px 12px"`（同上 :465,474）。
  final EdgeInsets? cellPadding;

  /// 每行的整行点击回调（与 [cells] 同长）。null = 不可点。React 的 `<TableRow onClick>`
  /// 是整行热区（同上 :343-347）。
  final List<VoidCallback?>? onRowTap;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final body = columns == null ? _rowList(t) : _table(t);
    return Tile(
      title: title,
      meta: meta,
      live: live,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          body,
          if (footer != null)
            Container(
              margin: const EdgeInsets.only(top: 11),
              padding: const EdgeInsets.only(top: 11),
              decoration: BoxDecoration(
                border: Border(top: BorderSide(color: t.c.line)),
              ),
              child: footer,
            ),
        ],
      ),
    );
  }

  Widget _rowList(AidogTheme t) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      for (var i = 0; i < rows.length; i++)
        Container(
          padding: const EdgeInsets.symmetric(vertical: AidogLayout.rowH),
          decoration: BoxDecoration(
            border: i == rows.length - 1
                ? null
                : Border(bottom: BorderSide(color: t.c.line)),
          ),
          child: rows[i],
        ),
    ],
  );

  /// 整行热区：React 把 onClick 挂在 `<TableRow>` 上，所以每一格都要接点击。
  Widget _rowTap(int row, Widget cell) {
    final tap = onRowTap == null || row >= onRowTap!.length
        ? null
        : onRowTap![row];
    if (tap == null) return cell;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: tap,
        child: cell,
      ),
    );
  }

  Widget _table(AidogTheme t) {
    final cols = columns!;
    final headStyle =
        headerStyle ?? AidogType.micro.copyWith(color: t.c.fg3);
    // 全列 start / 首列 start 其余 end：由 [startAlignAll] 选一档。
    AlignmentDirectional alignOf(int i) => startAlignAll || i == 0
        ? AlignmentDirectional.centerStart
        : AlignmentDirectional.centerEnd;
    return Table(
      defaultVerticalAlignment: TableCellVerticalAlignment.middle,
      children: [
        TableRow(
          decoration: BoxDecoration(
            color: headerBackground,
            border: Border(bottom: BorderSide(color: t.c.lineStrong)),
          ),
          children: [
            for (var i = 0; i < cols.length; i++)
              Padding(
                padding:
                    cellPadding ??
                    const EdgeInsets.symmetric(
                      vertical: 7,
                      horizontal: AidogSpace.ssm,
                    ),
                child: Align(
                  alignment: alignOf(i),
                  child: Text(
                    uppercaseHeaders ? cols[i].toUpperCase() : cols[i],
                    style: headStyle,
                  ),
                ),
              ),
          ],
        ),
        for (var r = 0; r < cells!.length; r++)
          TableRow(
            decoration: BoxDecoration(
              border: r == cells!.length - 1
                  ? null
                  : Border(bottom: BorderSide(color: t.c.line)),
            ),
            children: [
              for (var i = 0; i < cols.length; i++)
                _rowTap(
                  r,
                  Padding(
                    padding:
                        cellPadding ??
                        const EdgeInsets.symmetric(
                          vertical: AidogLayout.rowH,
                          horizontal: AidogSpace.ssm,
                        ),
                    child: Align(
                      alignment: alignOf(i),
                      child: i < cells![r].length
                          ? cells![r][i]
                          : const SizedBox.shrink(),
                    ),
                  ),
                ),
            ],
          ),
      ],
    );
  }
}

// ── 格子四：操作格（表单与按钮）──────────────────────────────────────
// 对照现仓：Settings.tsx = 操作格堆叠。

class ActionTile extends StatelessWidget {
  const ActionTile({
    super.key,
    this.title,
    this.meta,
    this.description,
    required this.fields,
    this.actions = const [],
    this.live = false,
  });

  final String? title;
  final String? meta;

  /// 标题下的一句话说明。
  final String? description;

  /// 表单行（开关、输入框、下拉……由页面票提供具体控件）。
  final List<Widget> fields;

  /// 底部按钮组，右（start-end）对齐。
  final List<Widget> actions;
  final bool live;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Tile(
      title: title,
      meta: meta,
      live: live,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          if (description != null)
            Padding(
              padding: const EdgeInsets.only(bottom: AidogSpace.smd),
              child: Text(
                description!,
                style: AidogType.caption.copyWith(color: t.c.fg2),
              ),
            ),
          for (var i = 0; i < fields.length; i++)
            Padding(
              padding: EdgeInsets.only(top: i == 0 ? 0 : AidogSpace.smd),
              child: fields[i],
            ),
          if (actions.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(top: AidogSpace.slg),
              child: Wrap(
                alignment: WrapAlignment.end,
                spacing: AidogSpace.ssm,
                runSpacing: AidogSpace.ssm,
                children: actions,
              ),
            ),
        ],
      ),
    );
  }
}

// ── 12 列 bento 网格 ─────────────────────────────────────────────

/// 网格里的一格。[span] 只取 3 / 4 / 5 / 6 / 7 / 8 / 12（规则 2）。
@immutable
class BentoCell {
  const BentoCell({required this.span, required this.child})
    : assert(
        span == 3 ||
            span == 4 ||
            span == 5 ||
            span == 6 ||
            span == 7 ||
            span == 8 ||
            span == 12,
        '格子宽度只取 3 / 4 / 5 / 6 / 7 / 8 / 12 列（票 10 布局规则第 2 条）',
      );

  final int span;
  final Widget child;
}

/// 窄窗（< content-max）下的降列规则，照抄原型的 `@media(max-width:1180px)`：
/// c8/c7 → 12，c5/c4/c3 → 6。
int effectiveSpan(int span, double width) {
  if (width >= AidogLayout.contentMax) return span;
  if (span >= 7) return 12;
  if (span >= 3) return 6;
  return span;
}

/// 12 列网格。行内各格等高（同一行共享行高，靠 IntrinsicHeight），
/// 与 CSS grid 的 `align-items: stretch` 行为一致。
class Bento extends StatelessWidget {
  const Bento({super.key, required this.children, this.gap = AidogLayout.gridGap});

  final List<BentoCell> children;

  /// 行距 / 列距。缺省是 token 的 grid-gap；React 页面各自的全页 gap（如 Stats 的
  /// 16）经这里传入，不改 token 全局值。
  final double gap;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final gap = this.gap;
        const cols = 12;
        final width = constraints.maxWidth;
        final unit = (width - gap * (cols - 1)) / cols;
        double widthFor(int span) => unit * span + gap * (span - 1);

        // 贪心装行：装不下就换行（CSS grid 的 auto-placement 同行为）。
        final rows = <List<({int span, Widget child})>>[];
        var used = 0;
        for (final cell in children) {
          final span = effectiveSpan(cell.span, width).clamp(1, cols);
          if (rows.isEmpty || used + span > cols) {
            rows.add([]);
            used = 0;
          }
          rows.last.add((span: span, child: cell.child));
          used += span;
        }

        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            for (var r = 0; r < rows.length; r++)
              Padding(
                padding: EdgeInsets.only(top: r == 0 ? 0 : gap),
                child: IntrinsicHeight(
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      for (var i = 0; i < rows[r].length; i++) ...[
                        if (i > 0) SizedBox(width: gap),
                        SizedBox(
                          width: widthFor(rows[r][i].span),
                          child: rows[r][i].child,
                        ),
                      ],
                    ],
                  ),
                ),
              ),
          ],
        );
      },
    );
  }
}
