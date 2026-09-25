// PageHead 窄窗不溢出：托盘小窗 500px 里 trailing（180 搜索框 + 三颗按钮的
// Wrap）自然宽超过剩余空间时，Flexible 要让 Wrap 换行，而不是把 Row 撑破
//（2026-09-25 实跑 RenderFlex overflowed by 44 pixels）。
import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';

void main() {
  testWidgets('PageHead trailing 在 500px 窄窗换行不溢出', (tester) async {
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        const SizedBox(
          width: 500,
          child: PageHead(
            title: '平台',
            subtitle: '3 / 8 active',
            trailing: Wrap(
              spacing: 8,
              children: [
                SizedBox(
                  width: 180,
                  child: TextField(
                    decoration: InputDecoration(isDense: true),
                  ),
                ),
                Text('+ 添加分组'),
                Text('+ 添加平台'),
                Text('清理已禁用'),
              ],
            ),
          ),
        ),
        c,
      ),
    );

    expect(tester.takeException(), isNull);
  });
}
