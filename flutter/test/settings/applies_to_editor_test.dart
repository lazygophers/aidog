/// 中间件「应用范围」写出去的平台 id 必须是**数字**。
///
/// 后端 `applies_to.platforms` 是 `Vec<i64>`
/// （`src-tauri/crates/aidog_db/src/models/middleware.rs:250`）。写成字符串的后果是
/// 整条规则存不进去（serde 反序列化失败），或者平台限定被 `#[serde(default)]`
/// 吞成空数组 —— 规则看着存住了，限定却不生效。React 传的就是数字
/// （`MiddlewareRules.tsx:605`）。
///
/// `groups` / `models` 在后端是 `Vec<String>`，继续传字符串是对的，这里一并钉住。
library;

import 'package:aidog_flutter/src/pages/settings/middleware_editor.dart'
    show AppliesToEditor;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';

void main() {
  testWidgets('平台维度写出数字 id，分组维度写出字符串 group_key', (tester) async {
    await useBigSurface(tester);
    final i18n = await makeI18n(tester);
    Map<String, Object?> latest = const {};

    await tester.pumpWidget(
      wrapPage(
        AppliesToEditor(
          value: const {},
          onChanged: (v) => latest = v,
          platforms: const [(id: 7, name: 'P7')],
          groups: const [(id: 3, name: 'G3', groupKey: 'gk3')],
        ),
        i18n,
      ),
    );
    await settle(tester);

    // 2026-09-23 起平台 / 分组两维是收起式多选（选项多到平铺会撑爆屏幕），
    // 所以要先点开对应的下拉再勾。第一个是平台，第二个是分组。
    await tester.tap(find.byType(PopupMenuButton<void>).first);
    await settle(tester);
    await tester.tap(find.byKey(const ValueKey('platforms-7')));
    await settle(tester);
    // 关掉菜单，免得它盖住下面那个下拉。
    await tester.tapAt(const Offset(5, 5));
    await settle(tester);
    expect(
      latest['platforms'],
      [7],
      reason: '必须是 int 7，不是字符串 "7"',
    );
    // 列表本身是 List<dynamic>（走 JSON map），要断的是**元素**的类型：
    // 写成字符串 '7' 时 `[7]` 这条断言就已经红了，这里再钉一道更直白的。
    expect((latest['platforms']! as List).single, isA<int>());

    await tester.tap(find.byType(PopupMenuButton<void>).last);
    await settle(tester);
    await tester.tap(find.byKey(const ValueKey('groups-gk3')));
    await settle(tester);
    expect(latest['groups'], ['gk3'], reason: 'groups 在后端是 Vec<String>');
  });
}
