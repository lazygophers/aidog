/// 带补全的路径输入行（票 I19b / C6）。
///
/// 对齐 React `src/components/settings/editors/_shared.tsx::PathInput`：
/// 150 ms 防抖、`fs_autocomplete` 取候选、↑/↓/Tab/Enter/Esc 键盘操作、
/// 失焦 200 ms 收起、无候选不弹、选目录补 `/` 并就地再查、时间列的相对时间。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/src/pages/settings/path_input.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';
import 'fake_invoke.dart';

Map<String, Object?> entry(
  String name, {
  String? full,
  bool dir = false,
  int modified = 0,
}) => {
  'name': name,
  'full_path': full ?? '/home/$name',
  'is_dir': dir,
  'modified': modified,
};

void main() {
  late FakeInvoke fake;
  late List<String?> changes;

  setUp(() {
    fake = FakeInvoke();
    changes = [];
  });

  Future<I18nController> mount(
    WidgetTester tester, {
    String? value,
    String pathType = 'file',
    bool showPicker = false,
    Future<String?> Function(bool)? pick,
  }) async {
    await useBigSurface(tester);
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        PathInputRow(
          label: 'File Suggestion',
          value: value,
          pathType: pathType,
          invoke: fake.fn,
          showPicker: showPicker,
          pick: pick,
          onChanged: changes.add,
        ),
        i18n,
      ),
    );
    await settle(tester);
    return i18n;
  }

  /// 输入并推过 150 ms 防抖。
  Future<void> type(WidgetTester tester, String text) async {
    await tester.enterText(find.byKey(const ValueKey('path-input')), text);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    await settle(tester);
  }

  group('取候选', () {
    testWidgets('防抖 150 ms：不到点不发请求', (tester) async {
      fake.responses['fs_autocomplete'] = [entry('a.txt')];
      await mount(tester);
      await tester.enterText(find.byKey(const ValueKey('path-input')), '~/');
      await tester.pump(const Duration(milliseconds: 100));
      expect(fake.commands.contains('fs_autocomplete'), isFalse);
      await tester.pump(const Duration(milliseconds: 100));
      await settle(tester);
      expect(fake.lastCallTo('fs_autocomplete')!.args, {'input': '~/'});
      expect(find.byKey(const ValueKey('path-suggestions')), findsOneWidget);
    });

    testWidgets('连打只发最后一次', (tester) async {
      fake.responses['fs_autocomplete'] = [entry('a.txt')];
      await mount(tester);
      await tester.enterText(find.byKey(const ValueKey('path-input')), '~');
      await tester.pump(const Duration(milliseconds: 50));
      await tester.enterText(find.byKey(const ValueKey('path-input')), '~/');
      await tester.pump(const Duration(milliseconds: 50));
      await tester.enterText(find.byKey(const ValueKey('path-input')), '~/a');
      await tester.pump(const Duration(milliseconds: 200));
      await settle(tester);
      expect(fake.callsTo('fs_autocomplete').length, 1);
      expect(fake.lastCallTo('fs_autocomplete')!.args, {'input': '~/a'});
    });

    testWidgets('清空输入：不发请求、收起下拉', (tester) async {
      fake.responses['fs_autocomplete'] = [entry('a.txt')];
      await mount(tester);
      await type(tester, '~/');
      expect(find.byKey(const ValueKey('path-suggestions')), findsOneWidget);
      await type(tester, '');
      expect(find.byKey(const ValueKey('path-suggestions')), findsNothing);
      expect(fake.callsTo('fs_autocomplete').length, 1);
      expect(changes.last, isNull);
    });

    testWidgets('候选为空不弹', (tester) async {
      fake.responses['fs_autocomplete'] = <Object?>[];
      await mount(tester);
      await type(tester, '~/zzz');
      expect(find.byKey(const ValueKey('path-suggestions')), findsNothing);
    });

    testWidgets('命令报错：吞掉并收起，不炸页面', (tester) async {
      fake.errors['fs_autocomplete'] = StateError('denied');
      await mount(tester);
      await type(tester, '~/');
      expect(tester.takeException(), isNull);
      expect(find.byKey(const ValueKey('path-suggestions')), findsNothing);
    });

    testWidgets('每次输入都把 onChanged 往外抛（空串 → null）', (tester) async {
      fake.responses['fs_autocomplete'] = <Object?>[];
      await mount(tester);
      await type(tester, '/tmp');
      expect(changes, ['/tmp']);
      await type(tester, '');
      expect(changes, ['/tmp', null]);
    });
  });

  group('鼠标选中', () {
    testWidgets('选文件：写全路径并收起', (tester) async {
      fake.responses['fs_autocomplete'] = [
        entry('a.txt', full: '/home/a.txt'),
        entry('b.txt', full: '/home/b.txt'),
      ];
      await mount(tester);
      await type(tester, '/home/');
      await tester.tap(find.byKey(const ValueKey('path-sugg-1')));
      await settle(tester);
      expect(changes.last, '/home/b.txt');
      expect(find.byKey(const ValueKey('path-suggestions')), findsNothing);
    });

    testWidgets('选目录：补 / 并就地再查一次', (tester) async {
      fake.responses['fs_autocomplete'] = [entry('sub', full: '/home/sub', dir: true)];
      await mount(tester, pathType: 'directory');
      await type(tester, '/home/');
      await tester.tap(find.byKey(const ValueKey('path-sugg-0')));
      await tester.pump(const Duration(milliseconds: 200));
      await settle(tester);
      expect(changes.last, '/home/sub/');
      expect(fake.lastCallTo('fs_autocomplete')!.args, {'input': '/home/sub/'});
    });
  });

  group('键盘', () {
    Future<void> press(WidgetTester tester, LogicalKeyboardKey k) async {
      await tester.sendKeyEvent(k);
      await settle(tester);
    }

    int highlighted(WidgetTester tester, int count) {
      for (var i = 0; i < count; i++) {
        final c = tester.widget<Container>(
          find.descendant(
            of: find.byKey(ValueKey('path-sugg-$i')),
            matching: find.byType(Container),
          ),
        );
        if ((c.color ?? (c.decoration as BoxDecoration?)?.color) != null) return i;
      }
      return -1;
    }

    Future<void> withThree(WidgetTester tester) async {
      fake.responses['fs_autocomplete'] = [
        entry('a', full: '/h/a'),
        entry('b', full: '/h/b'),
        entry('c', full: '/h/c'),
      ];
      await mount(tester);
      await type(tester, '/h/');
      await tester.tap(find.byKey(const ValueKey('path-input')));
      await settle(tester);
    }

    testWidgets('↓ 循环向下，↑ 循环向上', (tester) async {
      await withThree(tester);
      expect(highlighted(tester, 3), -1);
      await press(tester, LogicalKeyboardKey.arrowDown);
      expect(highlighted(tester, 3), 0);
      await press(tester, LogicalKeyboardKey.arrowDown);
      await press(tester, LogicalKeyboardKey.arrowDown);
      expect(highlighted(tester, 3), 2);
      await press(tester, LogicalKeyboardKey.arrowDown);
      expect(highlighted(tester, 3), 0);
      await press(tester, LogicalKeyboardKey.arrowUp);
      expect(highlighted(tester, 3), 2);
    });

    testWidgets('Tab 无高亮时取第一条', (tester) async {
      await withThree(tester);
      await press(tester, LogicalKeyboardKey.tab);
      expect(changes.last, '/h/a');
    });

    testWidgets('Tab 有高亮时取高亮那条', (tester) async {
      await withThree(tester);
      await press(tester, LogicalKeyboardKey.arrowDown);
      await press(tester, LogicalKeyboardKey.arrowDown);
      await press(tester, LogicalKeyboardKey.tab);
      expect(changes.last, '/h/b');
    });

    testWidgets('Enter 只有在有高亮时才选中', (tester) async {
      await withThree(tester);
      final before = changes.length;
      await press(tester, LogicalKeyboardKey.enter);
      expect(changes.length, before);
      await press(tester, LogicalKeyboardKey.arrowDown);
      await press(tester, LogicalKeyboardKey.enter);
      expect(changes.last, '/h/a');
    });

    testWidgets('Esc 收起', (tester) async {
      await withThree(tester);
      await press(tester, LogicalKeyboardKey.escape);
      expect(find.byKey(const ValueKey('path-suggestions')), findsNothing);
    });

    testWidgets('下拉没开时按键什么也不做', (tester) async {
      fake.responses['fs_autocomplete'] = <Object?>[];
      await mount(tester);
      await type(tester, '/h/');
      await tester.tap(find.byKey(const ValueKey('path-input')));
      await settle(tester);
      await press(tester, LogicalKeyboardKey.arrowDown);
      await press(tester, LogicalKeyboardKey.enter);
      expect(changes, ['/h/']);
    });
  });

  group('焦点', () {
    testWidgets('失焦 200 ms 后收起；重新聚焦又弹回来', (tester) async {
      fake.responses['fs_autocomplete'] = [entry('a', full: '/h/a')];
      await mount(tester);
      await type(tester, '/h/');
      await tester.tap(find.byKey(const ValueKey('path-input')));
      await settle(tester);
      expect(find.byKey(const ValueKey('path-suggestions')), findsOneWidget);

      // 焦点挪走。
      final node = tester.widget<TextField>(
        find.byKey(const ValueKey('path-input')),
      ).focusNode!;
      node.unfocus();
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.byKey(const ValueKey('path-suggestions')), findsOneWidget);
      await tester.pump(const Duration(milliseconds: 150));
      expect(find.byKey(const ValueKey('path-suggestions')), findsNothing);

      node.requestFocus();
      await settle(tester);
      expect(find.byKey(const ValueKey('path-suggestions')), findsOneWidget);
    });
  });

  group('提示与外部值', () {
    testWidgets('空值且未弹下拉时给输入提示；目录 / 文件两套文案', (tester) async {
      var t = await mount(tester);
      expect(find.text(t.t('settings.editor.fileHint')), findsOneWidget);
      t = await mount(tester, pathType: 'directory');
      expect(find.text(t.t('settings.editor.dirHint')), findsOneWidget);
    });

    testWidgets('有值就不显示提示', (tester) async {
      final t = await mount(tester, value: '/etc/hosts');
      expect(find.text(t.t('settings.editor.fileHint')), findsNothing);
    });

    testWidgets('占位符按 pathType 分两套', (tester) async {
      final t = await mount(tester, pathType: 'directory');
      final f = tester.widget<TextField>(find.byKey(const ValueKey('path-input')));
      expect(f.decoration!.hintText, t.t('settings.editor.dirOrInputPh'));
    });
  });

  group('原生文件对话框按钮', () {
    testWidgets('选了就写进输入框', (tester) async {
      await mount(
        tester,
        showPicker: true,
        pick: (dir) async {
          expect(dir, isFalse);
          return '/picked/file.sh';
        },
      );
      await tester.tap(find.byKey(const ValueKey('path-pick')));
      await settle(tester);
      expect(changes.last, '/picked/file.sh');
    });

    testWidgets('取消（返回 null）不写', (tester) async {
      await mount(tester, showPicker: true, pick: (_) async => null);
      await tester.tap(find.byKey(const ValueKey('path-pick')));
      await settle(tester);
      expect(changes, isEmpty);
    });

    testWidgets('目录模式把 directory=true 传下去', (tester) async {
      var got = false;
      await mount(
        tester,
        pathType: 'directory',
        showPicker: true,
        pick: (dir) async {
          got = dir;
          return null;
        },
      );
      await tester.tap(find.byKey(const ValueKey('path-pick')));
      await settle(tester);
      expect(got, isTrue);
    });
  });

  group('PathSuggestion 解析与时间列', () {
    test('缺字段有兜底', () {
      final s = PathSuggestion.fromJson(const {});
      expect(s.name, '');
      expect(s.fullPath, '');
      expect(s.isDir, isFalse);
      expect(s.modified, 0);
    });

    testWidgets('formatSuggestionTime 的四段分支', (tester) async {
      final t = await makeI18n(tester);
      final now = DateTime(2026, 9, 21, 12);
      int ts(DateTime d) => d.millisecondsSinceEpoch ~/ 1000;

      expect(formatSuggestionTime(t, 0, now: now), '');
      expect(
        formatSuggestionTime(t, ts(now.subtract(const Duration(minutes: 5))), now: now),
        t.t('settings.editor.justNow'),
      );
      expect(
        formatSuggestionTime(t, ts(now.subtract(const Duration(hours: 5))), now: now),
        '5${t.t('settings.editor.hoursAgo')}',
      );
      expect(
        formatSuggestionTime(t, ts(now.subtract(const Duration(days: 3))), now: now),
        '3${t.t('settings.editor.daysAgo')}',
      );
      expect(
        formatSuggestionTime(t, ts(DateTime(2025, 1, 2)), now: now),
        '2025-01-02',
      );
    });
  });
}
