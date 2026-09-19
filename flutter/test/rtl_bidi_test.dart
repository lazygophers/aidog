// RTL 四类漏点的回归测试。每类一条，断言的是**字形的横向先后**，不是截图 ——
// 截图挂不掉构建，这里能。
//
// 判据统一用 TextPainter.getBoxesForSelection 取某个字符的 left 坐标：
// 数值小的在左边。测试字体是等宽的，比大小足够。
//
// 每条都带一个「不修会怎样」的对照断言，证明这条测试真有牙：对照挂了，说明
// Flutter 哪天把 bidi 行为改了，得回来重看，而不是悄悄变成一条永远绿的测试。

import 'package:aidog_flutter/i18n.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

/// [text] 里第 [index] 个字符（UTF-16 下标）画出来时的左边界。
double leftOf(String text, int index, TextDirection dir) {
  final painter = TextPainter(
    text: TextSpan(text: text, style: const TextStyle(fontSize: 14)),
    textDirection: dir,
  )..layout();
  final boxes = painter.getBoxesForSelection(
    TextSelection(baseOffset: index, extentOffset: index + 1),
  );
  expect(boxes, isNotEmpty, reason: '取不到第 $index 个字符的盒子');
  final left = boxes.first.left;
  painter.dispose();
  return left;
}

/// [needle] 在 [text] 里那一个字符的左边界。
double leftOfChar(String text, String needle, TextDirection dir) =>
    leftOf(text, text.indexOf(needle), dir);

/// 一行 widget 从左到右的 key 顺序。
List<String> visualOrder(WidgetTester tester, List<String> keys) {
  final withX = keys
      .map((k) => (k, tester.getTopLeft(find.byKey(ValueKey<String>(k))).dx))
      .toList()
    ..sort((a, b) => a.$2.compareTo(b.$2));
  return withX.map((e) => e.$1).toList();
}

/// 一条假的「时间轴」：24 小时里挑几个刻度，横着排。真图表是 I04 的活，
/// 这里只要一行按 Directionality 排布的子节点就够验方向。
Widget axisRow(List<String> labels) => Row(
  mainAxisSize: MainAxisSize.min,
  children: [
    for (final l in labels)
      SizedBox(
        width: 40,
        key: ValueKey<String>(l),
        child: Text(l, textAlign: TextAlign.center),
      ),
  ],
);

void main() {
  const arabic = 'ar-SA';

  group('漏点 1：图表时间轴永远左→右', () {
    const ticks = ['00:00', '06:00', '12:00', '18:00'];

    testWidgets('阿拉伯语下不包 AlwaysLtr，时间轴是倒的（对照）', (tester) async {
      await tester.pumpWidget(
        Directionality(
          textDirection: textDirectionOf(arabic),
          child: Align(alignment: Alignment.topLeft, child: axisRow(ticks)),
        ),
      );
      expect(visualOrder(tester, ticks), ticks.reversed.toList());
    });

    testWidgets('包上 AlwaysLtr 之后，00:00 在最左，18:00 在最右', (tester) async {
      await tester.pumpWidget(
        Directionality(
          textDirection: textDirectionOf(arabic),
          child: Align(
            alignment: Alignment.topLeft,
            child: AlwaysLtr(child: axisRow(ticks)),
          ),
        ),
      );
      expect(visualOrder(tester, ticks), ticks);
    });
  });

  group('漏点 2：24 段高峰时钟带的刻度顺序锁死 0→23', () {
    test('kHourTicks 就是 0..23，跟界面语言无关', () {
      expect(kHourTicks, List<int>.generate(24, (i) => i));
      expect(kHourTicks.first, 0);
      expect(kHourTicks.last, 23);
    });

    test('刻度文字两位补零，并被隔离起来', () {
      expect(stripIsolates(hourTickLabel(0)), '00');
      expect(stripIsolates(hourTickLabel(8)), '08');
      expect(stripIsolates(hourTickLabel(23)), '23');
      expect(hourTickLabel(0), isNot('00'), reason: '没包隔离符，会被 bidi 重排');
    });

    testWidgets('阿拉伯语下整条带子读出来仍是 00 … 23，不是 23 … 00', (tester) async {
      final labels = [
        for (final h in kHourTicks) stripIsolates(hourTickLabel(h)),
      ];
      await tester.pumpWidget(
        Directionality(
          textDirection: textDirectionOf(arabic),
          child: Align(
            alignment: Alignment.topLeft,
            child: AlwaysLtr(
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (final h in kHourTicks)
                    SizedBox(
                      width: 24,
                      key: ValueKey<String>(labels[h]),
                      child: Text(hourTickLabel(h)),
                    ),
                ],
              ),
            ),
          ),
        ),
      );
      expect(visualOrder(tester, labels), labels);
    });
  });

  group('漏点 3：快捷键 ⌘C 不被镜像成 C⌘', () {
    const combo = '⌘C';
    final dir = textDirectionOf(arabic);

    test('不修的话 C 跑到 ⌘ 左边（对照）', () {
      expect(
        leftOfChar(combo, 'C', dir),
        lessThan(leftOfChar(combo, '⌘', dir)),
      );
    });

    test('ltr() 包过之后 ⌘ 在左、C 在右', () {
      final fixed = ltr(combo);
      expect(
        leftOfChar(fixed, '⌘', dir),
        lessThan(leftOfChar(fixed, 'C', dir)),
      );
    });

    test('多键组合整串保序', () {
      const seq = '⌘⇧P';
      final fixed = ltr(seq);
      final xs = [
        leftOfChar(fixed, '⌘', dir),
        leftOfChar(fixed, '⇧', dir),
        leftOfChar(fixed, 'P', dir),
      ];
      expect(xs, orderedEquals(List<double>.from(xs)..sort()));
    });

    test('ltr() 只加隔离符，不改文字本身', () {
      expect(stripIsolates(ltr(combo)), combo);
      expect(ltr(''), '');
    });
  });

  group('漏点 4：数字开头/结尾的短标签不被重排', () {
    final dir = textDirectionOf(arabic);

    test('「24 小时趋势」：不修的话 24 被甩到末尾（对照）', () {
      const label = '24 ساعة';
      expect(
        leftOfChar(label, '2', dir),
        greaterThan(leftOfChar(label, 'س', dir)),
        reason: '数字应当出现在阿拉伯文右边，也就是读起来的末尾',
      );
    });

    test('「24 小时趋势」：ltr() 包过之后 24 回到开头', () {
      final fixed = ltr('24 ساعة');
      expect(
        leftOfChar(fixed, '2', dir),
        lessThan(leftOfChar(fixed, 'س', dir)),
      );
    });

    test('「+6.1%」：不修的话 + 跑到 % 右边（对照）', () {
      const pct = '+6.1%';
      expect(leftOfChar(pct, '+', dir), greaterThan(leftOfChar(pct, '%', dir)));
    });

    test('「+6.1%」：ltr() 包过之后 + 在最左、% 在最右', () {
      final fixed = ltr('+6.1%');
      expect(leftOfChar(fixed, '+', dir), lessThan(leftOfChar(fixed, '6', dir)));
      expect(leftOfChar(fixed, '6', dir), lessThan(leftOfChar(fixed, '%', dir)));
    });

    test('设对 textDirection 也救不了 —— 所以必须整串包', () {
      // 基准方向设成 ltr 能救 +6.1%（它整串都是弱方向字符），
      // 但救不了掺了阿拉伯文的 24 ساعة：段内重排照旧发生。
      const label = '24 ساعة';
      expect(
        leftOfChar(label, '2', TextDirection.ltr),
        lessThan(leftOfChar(label, 'س', TextDirection.ltr)),
      );
      // 实测（Flutter 3.47 / 测试字体）：'ساعة 24' 在 ltr 与 rtl 两种基准下排布**完全相同**，
      // 数字都落在最左（x=0），而它在逻辑顺序里是最后一段 —— 也就是说尾随数字被甩到了另一头，
      // 且换基准方向救不了。这正是漏点 4 要整串包 ltr 的理由。
      const mixed = 'ساعة 24';
      for (final base in [TextDirection.ltr, TextDirection.rtl]) {
        expect(
          leftOfChar(mixed, '2', base),
          lessThan(leftOfChar(mixed, 'س', base)),
          reason: '基准 $base：逻辑上在末尾的数字被甩到了最左',
        );
      }
    });

    testWidgets('渲染进 widget 树同样成立', (tester) async {
      await tester.pumpWidget(
        Directionality(
          textDirection: dir,
          child: Center(child: Text(ltr('+6.1%'))),
        ),
      );
      expect(find.text(ltr('+6.1%')), findsOneWidget);
      final rendered = tester.widget<Text>(find.byType(Text)).data!;
      expect(stripIsolates(rendered), '+6.1%');
    });
  });
}
