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

/// 格子右上角的元信息：micro 字阶 / 全大写 / fg-3。
class TileMeta extends StatelessWidget {
  const TileMeta(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return HighlightedText(
      text.toUpperCase(),
      style: AidogType.micro.copyWith(color: t.c.fg3),
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
  const HighlightedText(this.text, {super.key, this.style});

  final String text;
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final q = TextHighlight.queryOf(context).trim();
    if (q.isEmpty) return Text(text, style: style);
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
    required this.child,
  });

  final String? title;
  final String? meta;
  final bool live;
  final EdgeInsetsGeometry? padding;
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
                  TileMeta(meta!),
                ],
              ],
            ),
          );
    return AnimatedContainer(
      duration: AidogMotion.base,
      curve: AidogMotion.easeStandard,
      padding: padding ??
          const EdgeInsets.symmetric(
            horizontal: AidogLayout.tilePadX,
            vertical: AidogLayout.tilePadY,
          ),
      decoration: BoxDecoration(
        color: live ? t.c.liveFill : t.c.surface,
        border: Border.all(color: live ? t.c.liveEdge : t.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.lg),
        boxShadow: t.shadowTile,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [?head, child],
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
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(label, style: AidogType.caption.copyWith(color: t.c.fg2)),
          const SizedBox(height: 2),
          // 以数字开头/结尾的读数在 RTL 下会被 bidi 重排 —— 规则 6 第 ④ 类。
          Ltr(child: Text(value, style: numStyle(AidogType.numXl, t.c.fg))),
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
  });

  final String title;
  final String? meta;

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
      live: live,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 时间轴永远从左到右，RTL 下也不镜像。
          Ltr(child: SizedBox(height: chartHeight, child: chart)),
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
                        Text(l.label, style: AidogType.caption.copyWith(color: t.c.fg2)),
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
  })  : columns = null,
        cells = null;

  /// 表格形态：表头 micro 全大写，首列 start 对齐、其余 end 对齐（数字列）。
  const ListingTile.table({
    super.key,
    this.title,
    this.meta,
    required List<String> this.columns,
    required List<List<Widget>> this.cells,
    this.footer,
    this.live = false,
  }) : rows = const [];

  final String? title;
  final String? meta;
  final List<Widget> rows;
  final List<String>? columns;
  final List<List<Widget>>? cells;
  final Widget? footer;
  final bool live;

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

  Widget _table(AidogTheme t) {
    final cols = columns!;
    final headStyle = AidogType.micro.copyWith(color: t.c.fg3);
    return Table(
      defaultVerticalAlignment: TableCellVerticalAlignment.middle,
      children: [
        TableRow(
          decoration: BoxDecoration(
            border: Border(bottom: BorderSide(color: t.c.lineStrong)),
          ),
          children: [
            for (var i = 0; i < cols.length; i++)
              Padding(
                padding: const EdgeInsets.symmetric(
                  vertical: 7,
                  horizontal: AidogSpace.ssm,
                ),
                child: Align(
                  alignment: i == 0
                      ? AlignmentDirectional.centerStart
                      : AlignmentDirectional.centerEnd,
                  child: Text(cols[i].toUpperCase(), style: headStyle),
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
                Padding(
                  padding: const EdgeInsets.symmetric(
                    vertical: AidogLayout.rowH,
                    horizontal: AidogSpace.ssm,
                  ),
                  child: Align(
                    alignment: i == 0
                        ? AlignmentDirectional.centerStart
                        : AlignmentDirectional.centerEnd,
                    child: i < cells![r].length ? cells![r][i] : const SizedBox.shrink(),
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
          span == 3 || span == 4 || span == 5 || span == 6 || span == 7 || span == 8 || span == 12,
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
  const Bento({super.key, required this.children});

  final List<BentoCell> children;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        const gap = AidogLayout.gridGap;
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
                        if (i > 0) const SizedBox(width: gap),
                        SizedBox(width: widthFor(rows[r][i].span), child: rows[r][i].child),
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
