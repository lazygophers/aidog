/// 输入框的内容不许被外部重建打掉。
///
/// 回归 2026-09-22：技能页项目路径、MCP 的名称 / 命令 / URL / KV 编辑器、
/// 模型信息页兜底价，一共 9 处直接写
/// `TextField(controller: TextEditingController(text: x))` ——
/// **每次 build 都造一个新控制器**，于是任何一次外部重建都会把正在输入的内容
/// 打回上一次提交的值。MCP 的 KV 编辑器最明显：增删任意一行就会触发。
library;

import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';

void main() {
  testWidgets('父级重建不会把正在输入的内容打回去', (tester) async {
    final i18n = await makeI18n(tester);
    var external = 'a';
    late StateSetter rebuild;

    await tester.pumpWidget(
      wrapPage(
        StatefulBuilder(
          builder: (context, setState) {
            rebuild = setState;
            return KeptTextField(value: external, onChanged: (_) {});
          },
        ),
        i18n,
      ),
    );
    await settle(tester);

    await tester.enterText(find.byType(TextField), '用户正在打的字');
    await settle(tester);

    // 外部值没变，只是父级重建了一次（同页别的东西变了 / 定时刷新到了）。
    rebuild(() {});
    await settle(tester);

    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      '用户正在打的字',
      reason: '控制器重建的话这里会变回 a',
    );
  });

  testWidgets('外部值真的变了才同步回输入框，光标落到末尾', (tester) async {
    final i18n = await makeI18n(tester);
    var external = 'a';
    late StateSetter rebuild;

    await tester.pumpWidget(
      wrapPage(
        StatefulBuilder(
          builder: (context, setState) {
            rebuild = setState;
            return KeptTextField(value: external, onChanged: (_) {});
          },
        ),
        i18n,
      ),
    );
    await settle(tester);

    // 外部把值改了（比如后端把非法字符过滤掉了）→ 必须同步进来。
    rebuild(() => external = 'bbb');
    await settle(tester);

    final ctrl = tester.widget<TextField>(find.byType(TextField)).controller!;
    expect(ctrl.text, 'bbb');
    expect(ctrl.selection.baseOffset, 3, reason: '光标要在末尾，不能停在 0');
  });
}
