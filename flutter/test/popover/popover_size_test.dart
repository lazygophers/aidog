// 票 I20：小窗把自己的尺寸报给原生外壳这一段。
//
// 两件事值得钉死，因为它们都是「差一点就看不出来、但用户能看见」的：
// 1. **上下左右的内边距不同**：`.popover-root` 是 `padding: 10px 14px`
//    （`src/styles/popover.css:33`），横竖各算各的，写成同一个数就会差 8px。
// 2. **宽度要跟着内容走**，不是写死 340 —— 与 React 版 `clamp(offsetWidth, 300, 480)`
//    同语义（`src/popover.tsx:180`）。
//
// 不起内核（假 invoke 顶掉传输层），不碰 9890 端口。
import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/popover.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';
import 'popover_widget_test.dart' show cfgItem, popoverData;

/// 外壳自己占的那一圈 = `.popover-root` 的内边距 ×2。**无边框**
/// （popover.css:39 那句 `border: var(--glass-border)` 缺 border-style，
/// 浏览器按 none 处理），所以不再加边宽。
const double kChromeX = 2 * PopoverRoot.padX;
const double kChromeY = 2 * PopoverRoot.padY;

/// 挂一个 PopoverApp，收集它报上来的尺寸。
Future<List<Size>> pumpPopover(
  WidgetTester tester,
  I18nController i18n,
  Map<String, Object?> data, {
  Size surface = const Size(480, 600),
}) async {
  await tester.binding.setSurfaceSize(surface);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final reported = <Size>[];
  await tester.pumpWidget(
    AidogI18n(
      controller: i18n,
      child: PopoverApp(
        invoke: (cmd, [args]) async => switch (cmd) {
          'popover_data' => data,
          'group_list' => const <Object?>[],
          'group_detail_list' => const <Object?>[],
          'stats_query_batch' => const <Object?>[],
          _ => throw StateError('未摆载荷的命令 $cmd'),
        },
        events: () => const Stream<Object?>.empty(),
        reportSize: (w, h) async => reported.add(Size(w, h)),
      ),
    ),
  );
  await settle(tester);
  return reported;
}

void main() {
  testWidgets('上报的高度含上下 padding（各一份）', (tester) async {
    final i18n = await makeI18n(tester);
    final reported = await pumpPopover(
      tester,
      i18n,
      popoverData([cfgItem('today_cost')]),
    );
    expect(reported, isNotEmpty, reason: '一帧都没报，测量回调没跑');

    // 直接量内容本体，再自己加一遍 chrome —— 与上报值必须分毫不差。
    final content = tester.renderObject<RenderBox>(
      find.byType(PopoverGrid).first,
    );
    expect(reported.last.height, content.size.height + kChromeY);
  });

  testWidgets('上报的宽度是内容想要的宽度，不是窗口宽也不是定宽', (tester) async {
    final i18n = await makeI18n(tester);
    // 画布给满 480（= MAX_W）。一张小卡片要不了这么宽，所以「内容想要多宽」
    // 与「窗口有多宽」在这一局里数值不同 —— 正好能分辨两种实现。
    const surfaceWidth = 480.0;
    final reported = await pumpPopover(
      tester,
      i18n,
      popoverData([cfgItem('today_cost')]),
      surface: const Size(surfaceWidth, 600),
    );
    expect(reported, isNotEmpty, reason: '一帧都没报，测量回调没跑');

    final content = tester.renderObject<RenderBox>(
      find.byType(PopoverGrid).first,
    );
    expect(
      reported.last.width,
      content.getMaxIntrinsicWidth(double.infinity) + kChromeX,
      reason: '宽度该取内容的 max-intrinsic 宽 + 外壳那一圈',
    );
    expect(
      reported.last.width,
      lessThan(surfaceWidth),
      reason: '等于画布宽说明量的是 box.size.width（窗口给多宽就报多宽），那是错的',
    );
  });

  test('夹取上下限与 React 的 MIN_W/MAX_W/MIN_H/MAX_H 逐个对齐', () {
    // React: src/popover.tsx:65-68。
    expect(kTrayPanelMinWidth, 300);
    expect(kTrayPanelMaxWidth, 480);
    expect(kTrayPanelMinHeight, 80);
    expect(kTrayPanelMaxHeight, 600);
  });
}
