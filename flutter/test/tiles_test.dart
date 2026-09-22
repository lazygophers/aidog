// 四种格子 + 12 列 bento。页面票（I06-I09）靠这一层排版，布局规则在这里被钉死。

import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Widget wrap(Widget child, {AidogMode mode = AidogMode.dark, double width = 1180}) {
  return MaterialApp(
    theme: aidogThemeData(mode),
    home: Scaffold(
      body: Center(child: SizedBox(width: width, child: child)),
    ),
  );
}

void main() {
  group('effectiveSpan（窄窗降列，照抄原型的 @media max-width:1180px）', () {
    test('宽窗不降列', () {
      for (final s in [3, 4, 5, 6, 7, 8, 12]) {
        expect(effectiveSpan(s, 1180), s);
        expect(effectiveSpan(s, 1600), s);
      }
    });

    test('窄窗：7/8 → 12，3/4/5 → 6，6 与 12 不动', () {
      expect(effectiveSpan(8, 1000), 12);
      expect(effectiveSpan(7, 1000), 12);
      expect(effectiveSpan(5, 1000), 6);
      expect(effectiveSpan(4, 1000), 6);
      expect(effectiveSpan(3, 1000), 6);
      expect(effectiveSpan(6, 1000), 6);
      expect(effectiveSpan(12, 1000), 12);
    });
  });

  test('BentoCell 只接受 3/4/5/6/7/8/12 列（布局规则第 2 条）', () {
    for (final s in [3, 4, 5, 6, 7, 8, 12]) {
      expect(() => BentoCell(span: s, child: const SizedBox()), returnsNormally);
    }
    for (final s in [1, 2, 9, 10, 11, 13]) {
      expect(
        () => BentoCell(span: s, child: const SizedBox()),
        throwsAssertionError,
        reason: '$s 列不在允许集合里',
      );
    }
  });

  testWidgets('12 列网格：4 个 3 列格装成一行，宽度与 gap 对得上', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(wrap(
      Bento(
        children: [
          for (var i = 0; i < 4; i++)
            BentoCell(span: 3, child: Tile(child: Text('kpi$i'))),
        ],
      ),
    ));
    await tester.pumpAndSettle();

    const gap = AidogLayout.gridGap;
    final unit = (1180 - gap * 11) / 12;
    final expectedW = unit * 3 + gap * 2;
    for (var i = 0; i < 4; i++) {
      expect(tester.getSize(find.text('kpi$i')).width, lessThanOrEqualTo(expectedW));
    }
    // 同一行：四格的纵向位置一致。
    final tops = [for (var i = 0; i < 4; i++) tester.getTopLeft(find.text('kpi$i')).dy];
    expect(tops.toSet(), hasLength(1));
    // 相邻两格的左边距 = 格宽 + gap。
    final x0 = tester.getTopLeft(find.text('kpi0')).dx;
    final x1 = tester.getTopLeft(find.text('kpi1')).dx;
    expect(x1 - x0, closeTo(expectedW + gap, 0.5));
  });

  testWidgets('装不下就换行：7 + 5 一行，再来一个 5 另起一行', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(wrap(
      Bento(
        children: [
          BentoCell(span: 7, child: Tile(child: const Text('a'))),
          BentoCell(span: 5, child: Tile(child: const Text('b'))),
          BentoCell(span: 5, child: Tile(child: const Text('c'))),
        ],
      ),
    ));
    await tester.pumpAndSettle();

    final ya = tester.getTopLeft(find.text('a')).dy;
    final yb = tester.getTopLeft(find.text('b')).dy;
    final yc = tester.getTopLeft(find.text('c')).dy;
    expect(yb, ya);
    expect(yc, greaterThan(ya));
  });

  testWidgets('同一行的格子等高（CSS grid 的 stretch 行为）', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(wrap(
      Bento(
        children: [
          BentoCell(span: 6, child: Tile(child: const SizedBox(height: 120, child: Text('tall')))),
          BentoCell(span: 6, child: Tile(child: const Text('short'))),
        ],
      ),
    ));
    await tester.pumpAndSettle();

    final tiles = find.byType(Tile);
    expect(tiles, findsNWidgets(2));
    expect(
      tester.getSize(tiles.at(0)).height,
      tester.getSize(tiles.at(1)).height,
    );
  });

  group('四种格子各自渲染', () {
    testWidgets('读数格：标签 + 大数字 + 环比（上行取 ok 色、下行取 bad 色）', (tester) async {
      await tester.pumpWidget(wrap(const ReadoutTile(
        label: '费用',
        value: r'$2.5000',
        delta: '+6.1%',
        trend: Trend.up,
        deltaNote: '较昨日',
      )));
      expect(find.text('费用'), findsOneWidget);
      expect(find.text(r'$2.5000'), findsOneWidget);
      // deltaNote 是悬浮提示不是常驻文字（React `Stats.tsx:965` 写在 title= 上）。
      // 当成文字画出来的话，统计页八张卡会一起多出八行同样的字。
      expect(find.text('较昨日'), findsNothing);
      expect(
        tester.widget<Tooltip>(
          find.ancestor(of: find.text('+6.1%'), matching: find.byType(Tooltip)),
        ).message,
        '较昨日',
      );

      final up = tester.widget<Text>(find.text('+6.1%'));
      expect(up.style!.color, AidogColors.dark.ok);
      // 等宽 + tabular figures：列对齐靠这条（规则 5）。
      expect(up.style!.fontFeatures, contains(const FontFeature.tabularFigures()));

      await tester.pumpWidget(wrap(const ReadoutTile(
        label: 'x', value: '1', delta: '-3%', trend: Trend.down,
      )));
      expect(tester.widget<Text>(find.text('-3%')).style!.color, AidogColors.dark.bad);
    });

    testWidgets('读数格的数字钉成 LTR（RTL 下不被 bidi 重排）', (tester) async {
      await tester.pumpWidget(MaterialApp(
        theme: aidogThemeData(AidogMode.dark),
        home: const Directionality(
          textDirection: TextDirection.rtl,
          child: Scaffold(body: ReadoutTile(label: 'التكلفة', value: '1,284')),
        ),
      ));
      final dir = tester.widget<Directionality>(
        find.ancestor(of: find.text('1,284'), matching: find.byType(Directionality)).first,
      );
      expect(dir.textDirection, TextDirection.ltr);
    });

    testWidgets('序列格：标题 + 元信息（全大写）+ 图插槽 + 图例', (tester) async {
      await tester.pumpWidget(wrap(SeriesTile(
        title: '24 小时趋势',
        meta: 'hourly · 峰值 147',
        chart: const Placeholder(),
        legend: [(color: AidogColors.dark.accent, label: '请求数')],
      )));
      expect(find.text('24 小时趋势'), findsOneWidget);
      expect(find.text('HOURLY · 峰值 147'), findsOneWidget);
      expect(find.byType(Placeholder), findsOneWidget);
      expect(find.text('请求数'), findsOneWidget);
    });

    testWidgets('清单格（行列表）：行间 hairline，最后一行无线', (tester) async {
      await tester.pumpWidget(wrap(const ListingTile(
        title: '今日平台用量',
        rows: [Text('r0'), Text('r1'), Text('r2')],
      )));
      Border? borderOf(String t) {
        final c = tester.widget<Container>(
          find.ancestor(of: find.text(t), matching: find.byType(Container)).first,
        );
        return (c.decoration as BoxDecoration?)?.border as Border?;
      }

      expect(borderOf('r0')!.bottom.color, AidogColors.dark.line);
      expect(borderOf('r1')!.bottom.color, AidogColors.dark.line);
      expect(borderOf('r2'), isNull);
    });

    testWidgets('清单格（表格）：表头全大写，格子按列铺开', (tester) async {
      await tester.pumpWidget(wrap(const ListingTile.table(
        title: '维度排行',
        columns: ['名称', '请求', '预估成本'],
        cells: [
          [Text('智谱'), Text('812'), Text(r'$1.20')],
          [Text('DeepSeek'), Text('471'), Text(r'$0.44')],
        ],
      )));
      expect(find.text('名称'), findsOneWidget);
      expect(find.text('请求'), findsOneWidget);
      expect(find.text('智谱'), findsOneWidget);
      expect(find.text(r'$0.44'), findsOneWidget);
      expect(find.byType(Table), findsOneWidget);
    });

    testWidgets('操作格：说明 + 表单行 + 底部按钮', (tester) async {
      var saved = false;
      await tester.pumpWidget(wrap(ActionTile(
        title: '代理设置',
        description: '改完要重启代理才生效',
        fields: const [Text('端口'), Text('绑定局域网')],
        actions: [
          TextButton(onPressed: () => saved = true, child: const Text('保存')),
        ],
      )));
      expect(find.text('改完要重启代理才生效'), findsOneWidget);
      expect(find.text('端口'), findsOneWidget);
      await tester.tap(find.text('保存'));
      expect(saved, isTrue);
    });
  });

  testWidgets('live 格子：深色带 live-fill + live-edge，浅色同样上色（halo 由 token 决定）',
      (tester) async {
    for (final mode in AidogMode.values) {
      final c = mode == AidogMode.dark ? AidogColors.dark : AidogColors.light;
      await tester.pumpWidget(wrap(
        Tile(live: true, child: const Text('live')),
        mode: mode,
      ));
      await tester.pumpAndSettle();
      final deco = tester
          .widget<AnimatedContainer>(find.byType(AnimatedContainer).first)
          .decoration! as BoxDecoration;
      expect(deco.color, c.liveFill);
      expect((deco.border! as Border).top.color, c.liveEdge);
    }
  });
}
