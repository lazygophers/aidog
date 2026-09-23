/// 搜索必须作用到分组区。
///
/// 回归 2026-09-22：搜索框就在平台页顶部，但只过滤未分组平台 ——
/// `GroupsSection` 连 `searchQuery` 入参都没有，源码还自述「搜索态在本页尚未接入」。
/// 后果是**已归组的平台一个都搜不到**，而用户看到的是一个能正常打字的搜索框。
///
/// React 的语义（`Groups.tsx:688-711` + `GroupListView.tsx:245` +
/// `GroupListItem.tsx:344-346`）：
///   - 命中组名 → 整组显示并强制展开
///   - 只命中组内某几个平台 → 只渲染命中的那几张，且强制展开
///   - 整组零命中 → 整组不渲染
library;

import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';
import 'pages_b_widget_test.dart' show groupsFake, plat;

Future<void> _mount(WidgetTester tester, String query, dynamic c) async {
  await useBigSurface(tester);
  final k = groupsFake(
    page: [
      {
        'group': {'id': 10, 'name': '主力分组', 'group_key': 'gk10'},
        'platforms': [
          {'platform': plat(1, 'Alpha')},
          {'platform': plat(2, 'Beta')},
        ],
        'model_mappings': <Object?>[],
      },
      {
        'group': {'id': 20, 'name': '备用组', 'group_key': 'gk20'},
        'platforms': [
          {'platform': plat(3, 'Gamma')},
        ],
        'model_mappings': <Object?>[],
      },
    ],
  );
  await tester.pumpWidget(
    wrapPage(
      GroupsSection(
        invoke: k.fn,
        searchQuery: query,
        buildPlatformCard: (p, i, {levelPriority, onLevelPriorityChange}) =>
            Text('卡:${p.name}'),
      ),
      c,
    ),
  );
  await settle(tester);
}

void main() {
  testWidgets('无搜索：两个组都在', (tester) async {
    final c = await makeI18n(tester);
    await _mount(tester, '', c);
    expect(find.text('主力分组'), findsOneWidget);
    expect(find.text('备用组'), findsOneWidget);
  });

  testWidgets('命中组名 → 只剩那个组，且整组展开', (tester) async {
    final c = await makeI18n(tester);
    await _mount(tester, '备用', c);
    expect(find.text('备用组'), findsOneWidget);
    expect(find.text('主力分组'), findsNothing, reason: '零命中的组整组不渲染');
    // 强制展开：不用再点一下就看得见组内的卡。
    expect(find.text('卡:Gamma'), findsOneWidget);
  });

  testWidgets('只命中组内某个平台 → 只渲染命中的那张卡', (tester) async {
    final c = await makeI18n(tester);
    await _mount(tester, 'Alpha', c);
    expect(find.text('主力分组'), findsOneWidget);
    expect(find.text('备用组'), findsNothing);
    expect(find.text('卡:Alpha'), findsOneWidget);
    expect(find.text('卡:Beta'), findsNothing, reason: '同组里没命中的不该画出来');
  });

  testWidgets('一个都搜不到 → 走「没有匹配」空态，不显「还没有分组」', (tester) async {
    final c = await makeI18n(tester);
    await _mount(tester, '这个词谁也不匹配', c);
    expect(find.text('主力分组'), findsNothing);
    expect(find.text('备用组'), findsNothing);
    expect(
      find.text(c.t('group.empty')),
      findsNothing,
      reason: '显「还没有分组」会让人以为分组全没了',
    );
  });
}
