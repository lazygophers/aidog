/// statusline 配置面板的 widget 行为（票 I19b / C5）。
///
/// 对齐 React `StatusLineSection/StatusLinePanel.tsx` + `SegmentEditModal.tsx`
/// + `useStatusLinePanel.ts`：开关 / 两种生成模式 / 段增删改排序 / 行对齐 /
/// 恢复默认布局 / 脚本预览（`preview_statusline_script`）/ 段编辑器。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/src/pages/settings/schema_config_page.dart';
import 'package:aidog_flutter/src/pages/settings/statusline_model.dart';
import 'package:aidog_flutter/src/pages/settings/statusline_panel.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';
import 'fake_invoke.dart';
import 'pages_c_widget_test.dart' show baseResponses;

/// 把面板挂进一个自己存 config 的宿主里 —— 与 `Settings.tsx` 的
/// `config` + `updateField` 同形状。
class _Host extends StatefulWidget {
  const _Host({
    required this.initial,
    required this.scriptType,
    required this.invoke,
    this.now,
  });

  final Map<String, Object?> initial;
  final String scriptType;
  final FakeInvoke invoke;
  final int Function()? now;

  @override
  State<_Host> createState() => _HostState();
}

class _HostState extends State<_Host> {
  late Map<String, Object?> config = {...widget.initial};

  @override
  Widget build(BuildContext context) => StatusLinePanel(
    config: config,
    scriptType: widget.scriptType,
    invoke: widget.invoke.fn,
    now: widget.now,
    updateField: (f, v) => setState(() {
      if (v == null) {
        config.remove(f);
      } else {
        config[f] = v;
      }
    }),
  );
}

void main() {
  late FakeInvoke fake;

  setUp(() => fake = FakeInvoke({'preview_statusline_script': '#!/usr/bin/env python3'}));

  Future<(_HostState, I18nController)> mount(
    WidgetTester tester, {
    Map<String, Object?> config = const {},
    String scriptType = 'statusline',
    int Function()? now,
  }) async {
    await useBigSurface(tester);
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        _Host(
          initial: config,
          scriptType: scriptType,
          invoke: fake,
          now: now,
        ),
        i18n,
      ),
    );
    await settle(tester);
    return (tester.state<_HostState>(find.byType(_Host)), i18n);
  }

  Map<String, Object?> stored(_HostState s, [String key = '_aidog_statusline']) =>
      Map<String, Object?>.from(s.config[key] as Map? ?? const {});

  List<StatusLineSegment> segsOf(_HostState s,
      [String key = '_aidog_statusline']) {
    final raw = stored(s, key)['segments'] as List? ?? const [];
    return raw
        .whereType<Map>()
        .map((e) => StatusLineSegment.fromJson(Map<String, Object?>.from(e)))
        .toList();
  }

  String ids(List<StatusLineSegment> l) => l.map((e) => e.id).join(',');

  Future<void> tapKey(WidgetTester tester, String key) async {
    await tester.tap(find.byKey(ValueKey(key)));
    await settle(tester);
  }

  // ── 开关与模式 ──────────────────────────────────────────

  group('启用开关', () {
    testWidgets('关 → 开：只写 enabled，不碰原生字段', (tester) async {
      final (host, t) = await mount(tester);
      expect(find.text(t.t('statusline.useBuiltin')), findsOneWidget);
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('sl-statusline-enable')),
          matching: find.byType(Switch),
        ),
      );
      await settle(tester);
      expect(stored(host)['enabled'], true);
      expect(host.config.containsKey('statusLine'), isFalse);
      expect(find.textContaining(t.t('statusline.enabled')), findsOneWidget);
    });

    testWidgets('开 → 关：清掉 statusLine 并落 enabled=false', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {'enabled': true},
        'statusLine': {'type': 'command', 'command': 'x'},
      });
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('sl-statusline-enable')),
          matching: find.byType(Switch),
        ),
      );
      await settle(tester);
      expect(stored(host)['enabled'], false);
      expect(host.config.containsKey('statusLine'), isFalse);
    });

    testWidgets('未启用时不渲染模式切换与段列表', (tester) async {
      await mount(tester);
      expect(find.byKey(const ValueKey('sl-statusline-mode-builtin')), findsNothing);
      expect(find.byKey(const ValueKey('sl-statusline-preview')), findsNothing);
    });

    testWidgets('子代理面板用自己的键和自己的默认布局', (tester) async {
      final (host, t) = await mount(tester, scriptType: 'subagent');
      expect(find.text(t.t('statusline.useBuiltinSubagent')), findsOneWidget);
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('sl-subagent-enable')),
          matching: find.byType(Switch),
        ),
      );
      await settle(tester);
      expect(host.config.containsKey('_aidog_subagent_statusline'), isTrue);
      expect(host.config.containsKey('_aidog_statusline'), isFalse);
      await tapKey(tester, 'sl-subagent-reset-layout');
      expect(
        ids(segsOf(host, '_aidog_subagent_statusline')),
        ids(kDefaultSubagentSegments),
      );
    });
  });

  group('生成模式', () {
    testWidgets('切到自定义：清 statusLine + 落 mode', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {'enabled': true},
        'statusLine': {'type': 'command', 'command': 'old'},
      });
      await tapKey(tester, 'sl-statusline-mode-custom');
      expect(stored(host)['mode'], 'custom');
      expect(host.config.containsKey('statusLine'), isFalse);
    });

    testWidgets('点当前模式是空操作', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {'enabled': true},
        'statusLine': {'type': 'command', 'command': 'keep'},
      });
      await tapKey(tester, 'sl-statusline-mode-builtin');
      expect(host.config['statusLine'], isNotNull);
      expect(stored(host).containsKey('mode'), isFalse);
    });

    testWidgets('自定义模式：填命令 → 应用 → 写 statusLine', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {'enabled': true, 'mode': 'custom'},
      });
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('sl-statusline-custom-cmd')),
          matching: find.byType(TextField),
        ),
        '  ~/.claude/my.sh  ',
      );
      await settle(tester);
      await tapKey(tester, 'sl-statusline-apply-custom');
      expect(host.config['statusLine'], {
        'type': 'command',
        'command': '~/.claude/my.sh',
      });
    });

    testWidgets('自定义模式：空命令 → 清掉 statusLine', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {'enabled': true, 'mode': 'custom', 'customCommand': '   '},
        'statusLine': {'type': 'command', 'command': 'old'},
      });
      await tapKey(tester, 'sl-statusline-apply-custom');
      expect(host.config.containsKey('statusLine'), isFalse);
    });

    testWidgets('自定义模式不画段列表与脚本预览', (tester) async {
      await mount(tester, config: {
        '_aidog_statusline': {'enabled': true, 'mode': 'custom'},
      });
      expect(find.byKey(const ValueKey('sl-statusline-preview')), findsNothing);
      expect(find.byKey(const ValueKey('sl-statusline-toggle-script')), findsNothing);
      expect(fake.commands.contains('preview_statusline_script'), isFalse);
    });
  });

  // ── 段列表 ───────────────────────────────────────────────

  group('段列表', () {
    Map<String, Object?> twoSegs({bool secondEnabled = true}) => {
      '_aidog_statusline': {
        'enabled': true,
        'segments': [
          {'id': 'a', 'type': 'model', 'enabled': true, 'newline': false, 'options': <String, Object?>{}},
          {'id': 'b', 'type': 'separator', 'enabled': secondEnabled, 'newline': false, 'options': {'char': '|'}},
        ],
      },
    };

    testWidgets('首次进入按内置默认布局渲染（不写盘）', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {'enabled': true},
      });
      expect(stored(host).containsKey('segments'), isFalse);
      expect(
        find.byKey(const ValueKey('sl-statusline-seg-d-model')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('sl-statusline-seg-d-version')),
        findsOneWidget,
      );
    });

    testWidgets('停用某段：写回 enabled=false，行内仍在', (tester) async {
      final (host, _) = await mount(tester, config: twoSegs());
      await tester.tap(find.byKey(const ValueKey('sl-statusline-on-b')));
      await settle(tester);
      expect(segsOf(host).last.enabled, isFalse);
    });

    testWidgets('删除某段', (tester) async {
      final (host, _) = await mount(tester, config: twoSegs());
      await tapKey(tester, 'sl-statusline-del-a');
      expect(ids(segsOf(host)), 'b');
    });

    testWidgets('↵ 切换换行；首段的 newline 被 normalize 抹掉', (tester) async {
      final (host, _) = await mount(tester, config: twoSegs());
      await tapKey(tester, 'sl-statusline-nl-b');
      expect(segsOf(host)[1].newline, isTrue);
      await tapKey(tester, 'sl-statusline-nl-a');
      expect(segsOf(host)[0].newline, isFalse);
    });

    testWidgets('行对齐循环 left → center', (tester) async {
      final (host, t) = await mount(tester, config: twoSegs());
      expect(find.text(t.t('statusline.align.left')), findsOneWidget);
      await tapKey(tester, 'sl-statusline-align-a');
      expect(segsOf(host)[0].align, RowAlign.center);
    });

    testWidgets('删除整行：带走同一行的所有段', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {
          'enabled': true,
          'segments': [
            {'id': 'a', 'type': 'model', 'enabled': true, 'newline': false, 'options': <String, Object?>{}},
            {'id': 'b', 'type': 'vim', 'enabled': true, 'newline': false, 'options': <String, Object?>{}},
            {'id': 'c', 'type': 'effort', 'enabled': true, 'newline': true, 'options': <String, Object?>{}},
          ],
        },
      });
      await tapKey(tester, 'sl-statusline-delrow-a');
      expect(ids(segsOf(host)), 'c');
    });

    testWidgets('添加段：菜单展开 → 选一项 → 追加到末尾并收起菜单', (tester) async {
      final (host, _) = await mount(
        tester,
        config: twoSegs(),
        now: () => 4242,
      );
      await tapKey(tester, 'sl-statusline-add-segment');
      expect(find.byKey(const ValueKey('sl-statusline-add-menu')), findsOneWidget);
      // 菜单是个限高滚动区，目标项在下面 —— 先滚到可见再点。
      await tester.ensureVisible(find.byKey(const ValueKey('sl-add-git-branch')));
      await settle(tester);
      await tester.tap(find.byKey(const ValueKey('sl-add-git-branch')));
      await settle(tester);
      expect(find.byKey(const ValueKey('sl-statusline-add-menu')), findsNothing);
      final segs = segsOf(host);
      expect(ids(segs), 'a,b,s4242');
      expect(segs.last.type, 'git-branch');
      expect(segs.last.newline, isFalse);
    });

    testWidgets('添加行：追加一个 model 段并另起一行', (tester) async {
      final (host, _) = await mount(
        tester,
        config: twoSegs(),
        now: () => 7,
      );
      await tapKey(tester, 'sl-statusline-add-row');
      final segs = segsOf(host);
      expect(segs.last.type, 'model');
      expect(segs.last.newline, isTrue);
    });

    testWidgets('列表为空时添加行不带 newline', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {'enabled': true, 'segments': <Object?>[]},
      }, now: () => 9);
      await tapKey(tester, 'sl-statusline-add-row');
      expect(segsOf(host).single.newline, isFalse);
    });

    testWidgets('恢复默认布局', (tester) async {
      final (host, _) = await mount(tester, config: twoSegs());
      await tapKey(tester, 'sl-statusline-reset-layout');
      expect(ids(segsOf(host)), ids(kDefaultSegments));
    });
  });

  // ── 预览 ─────────────────────────────────────────────────

  group('实时预览', () {
    testWidgets('渲染 affix + toPreview，停用段不进预览', (tester) async {
      await mount(tester, config: {
        '_aidog_statusline': {
          'enabled': true,
          'segments': [
            {'id': 'a', 'type': 'model', 'enabled': true, 'newline': false,
             'options': {'affixPre': '[', 'affixSuf': ']'}},
            {'id': 'b', 'type': 'vim', 'enabled': false, 'newline': false,
             'options': <String, Object?>{}},
          ],
        },
      });
      final preview = tester.widget<StatusLinePreview>(
        find.byKey(const ValueKey('sl-statusline-preview')),
      );
      expect(preview.segments.length, 2);
      final text = tester.widget<Text>(
        find.descendant(
          of: find.byKey(const ValueKey('sl-statusline-preview')),
          matching: find.byType(Text),
        ),
      );
      expect(text.textSpan!.toPlainText(), '[Opus]');
    });

    testWidgets('全部停用 → 空态文案', (tester) async {
      final (_, t) = await mount(tester, config: {
        '_aidog_statusline': {
          'enabled': true,
          'segments': [
            {'id': 'a', 'type': 'model', 'enabled': false, 'newline': false,
             'options': <String, Object?>{}},
          ],
        },
      });
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('sl-statusline-preview')),
          matching: find.text(t.t('statusline.previewEmpty')),
        ),
        findsOneWidget,
      );
    });
  });

  // ── 脚本预览 ─────────────────────────────────────────────

  group('脚本预览（preview_statusline_script）', () {
    testWidgets('收起时不发请求；展开才发，参数与 React 一致', (tester) async {
      final (_, t) = await mount(tester, config: {
        '_aidog_statusline': {'enabled': true},
      });
      expect(fake.commands.contains('preview_statusline_script'), isFalse);
      await tapKey(tester, 'sl-statusline-toggle-script');
      final call = fake.lastCallTo('preview_statusline_script')!;
      expect(call.args!['scriptType'], 'statusline');
      expect((call.args!['stored']! as Map)['enabled'], true);
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('sl-statusline-script-body')),
          matching: find.text('#!/usr/bin/env python3'),
        ),
        findsOneWidget,
      );
      expect(t.t('statusline.scriptPreview'), isNotEmpty);
    });

    testWidgets('子代理传自己的 scriptType', (tester) async {
      await mount(tester, scriptType: 'subagent', config: {
        '_aidog_subagent_statusline': {'enabled': true},
      });
      await tapKey(tester, 'sl-subagent-toggle-script');
      expect(
        fake.lastCallTo('preview_statusline_script')!.args!['scriptType'],
        'subagent',
      );
    });

    testWidgets('stored 没变就不重复请求；变了才重取', (tester) async {
      await mount(tester, config: {
        '_aidog_statusline': {'enabled': true},
      });
      await tapKey(tester, 'sl-statusline-toggle-script');
      final first = fake.callsTo('preview_statusline_script').length;
      await settle(tester);
      expect(fake.callsTo('preview_statusline_script').length, first);
      await tapKey(tester, 'sl-statusline-reset-layout');
      expect(fake.callsTo('preview_statusline_script').length, first + 1);
    });

    testWidgets('命令失败不炸页面，面板照常可用', (tester) async {
      fake.errors['preview_statusline_script'] = StateError('boom');
      final (_, _) = await mount(tester, config: {
        '_aidog_statusline': {'enabled': true},
      });
      await tapKey(tester, 'sl-statusline-toggle-script');
      expect(tester.takeException(), isNull);
      expect(
        find.byKey(const ValueKey('sl-statusline-script-body')),
        findsOneWidget,
      );
    });
  });

  // ── 段编辑器 ─────────────────────────────────────────────

  group('段编辑器', () {
    Map<String, Object?> oneSeg(String type, {Map<String, Object?>? opts}) => {
      '_aidog_statusline': {
        'enabled': true,
        'segments': [
          {
            'id': 'a',
            'type': type,
            'enabled': true,
            'newline': false,
            'options': opts ?? <String, Object?>{},
          },
        ],
      },
    };

    testWidgets('改 select 字段 → 保存写回 options', (tester) async {
      final (host, _) = await mount(tester, config: oneSeg('model'));
      await tapKey(tester, 'sl-statusline-edit-a');
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('seg-edit-f-format')),
          matching: find.text('full'),
        ),
      );
      await settle(tester);
      expect(
        tester
            .widget<Text>(find.byKey(const ValueKey('seg-edit-preview')))
            .data,
        'claude-sonnet-4-6',
      );
      await tapKey(tester, 'seg-edit-save');
      expect(segsOf(host).single.options['format'], 'full');
      expect(find.byKey(const ValueKey('sl-statusline-edit')), findsNothing);
    });

    testWidgets('改 number 字段 → 存成数字', (tester) async {
      final (host, _) = await mount(tester, config: oneSeg('context-bar'));
      await tapKey(tester, 'sl-statusline-edit-a');
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('seg-edit-f-width')),
          matching: find.byType(TextField),
        ),
        '4',
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      await tapKey(tester, 'seg-edit-save');
      expect(segsOf(host).single.options['width'], 4);
    });

    testWidgets('改 string 字段 → 存成字符串', (tester) async {
      final (host, _) = await mount(tester, config: oneSeg('separator'));
      await tapKey(tester, 'sl-statusline-edit-a');
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('seg-edit-f-char')),
          matching: find.byType(TextField),
        ),
        ' / ',
      );
      await settle(tester);
      await tapKey(tester, 'seg-edit-save');
      expect(segsOf(host).single.options['char'], ' / ');
    });

    testWidgets('取消不写回', (tester) async {
      final (host, _) = await mount(tester, config: oneSeg('model'));
      await tapKey(tester, 'sl-statusline-edit-a');
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('seg-edit-f-format')),
          matching: find.text('full'),
        ),
      );
      await settle(tester);
      await tapKey(tester, 'seg-edit-cancel');
      expect(find.byKey(const ValueKey('sl-statusline-edit')), findsNothing);
      // 段还是原样：format 没被写进去。
      expect(segsOf(host).single.options.containsKey('format'), isFalse);
    });

    testWidgets('合法 hex 才写 color；非法值当没填', (tester) async {
      final (host, _) = await mount(tester, config: oneSeg('model'));
      await tapKey(tester, 'sl-statusline-edit-a');
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('seg-edit-color')),
          matching: find.byType(TextField),
        ),
        'zzz',
      );
      await settle(tester);
      await tapKey(tester, 'seg-edit-save');
      expect(segsOf(host).single.color, isNull);

      await tapKey(tester, 'sl-statusline-edit-a');
      await tester.enterText(
        find.descendant(
          of: find.byKey(const ValueKey('seg-edit-color')),
          matching: find.byType(TextField),
        ),
        '#34C759',
      );
      await settle(tester);
      await tapKey(tester, 'seg-edit-save');
      expect(segsOf(host).single.color, '#34C759');
    });

    testWidgets('非值型段不给 autoColor 开关', (tester) async {
      await mount(tester, config: oneSeg('model'));
      await tapKey(tester, 'sl-statusline-edit-a');
      expect(find.byKey(const ValueKey('seg-edit-autocolor')), findsNothing);
    });

    testWidgets('值型段给 autoColor 开关，开了就锁住固定色', (tester) async {
      final (host2, _) = await mount(tester, config: oneSeg('cost-usd'));
      await tapKey(tester, 'sl-statusline-edit-a');
      expect(find.byKey(const ValueKey('seg-edit-autocolor')), findsOneWidget);
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('seg-edit-autocolor')),
          matching: find.byType(Switch),
        ),
      );
      await settle(tester);
      final colorField = tester.widget<TextField>(
        find.descendant(
          of: find.byKey(const ValueKey('seg-edit-color')),
          matching: find.byType(TextField),
        ),
      );
      expect(colorField.enabled, isFalse);
      await tapKey(tester, 'seg-edit-save');
      expect(segsOf(host2).single.autoColor, isTrue);
    });

    testWidgets('清除颜色按钮', (tester) async {
      final (host, _) = await mount(
        tester,
        config: oneSeg('model')
          ..['_aidog_statusline'] = {
            'enabled': true,
            'segments': [
              {'id': 'a', 'type': 'model', 'enabled': true, 'newline': false,
               'color': '#4A9EFF', 'options': <String, Object?>{}},
            ],
          },
      );
      await tapKey(tester, 'sl-statusline-edit-a');
      await tapKey(tester, 'seg-edit-clear-color');
      await tapKey(tester, 'seg-edit-save');
      expect(segsOf(host).single.color, isNull);
    });

    testWidgets('行首段才有行对齐；勾上换行后也出现', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {
          'enabled': true,
          'segments': [
            {'id': 'a', 'type': 'model', 'enabled': true, 'newline': false, 'options': <String, Object?>{}},
            {'id': 'b', 'type': 'vim', 'enabled': true, 'newline': false, 'options': <String, Object?>{}},
          ],
        },
      });
      await tapKey(tester, 'sl-statusline-edit-b');
      expect(find.byKey(const ValueKey('seg-edit-align')), findsNothing);
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('seg-edit-newline')),
          matching: find.byType(Switch),
        ),
      );
      await settle(tester);
      expect(find.byKey(const ValueKey('seg-edit-align')), findsOneWidget);
      await tapKey(tester, 'seg-edit-save');
      final b = segsOf(host).last;
      expect(b.newline, isTrue);
      expect(b.align, RowAlign.left);
    });

    testWidgets('非行首且不换行 → align 不写回', (tester) async {
      final (host, _) = await mount(tester, config: {
        '_aidog_statusline': {
          'enabled': true,
          'segments': [
            {'id': 'a', 'type': 'model', 'enabled': true, 'newline': false, 'options': <String, Object?>{}},
            {'id': 'b', 'type': 'vim', 'enabled': true, 'newline': false,
             'align': 'right', 'options': <String, Object?>{}},
          ],
        },
      });
      await tapKey(tester, 'sl-statusline-edit-b');
      await tapKey(tester, 'seg-edit-save');
      expect(segsOf(host).last.align, isNull);
    });
  });

  // ── 数据字段参考 ─────────────────────────────────────────

  group('可用数据字段参考', () {
    testWidgets('默认收起，点开列出 7 组', (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(wrapPage(const StatusLineDataRef(), i18n));
      await settle(tester);
      expect(find.text('model.id'), findsNothing);
      await tapKey(tester, 'sl-dataref-toggle');
      expect(find.text('model.id'), findsOneWidget);
      expect(find.text('session_id'), findsOneWidget);
      for (final g in kStatuslineDataFields) {
        expect(
          find.text(i18n.t('statusline.dataGroup.${g.id}')),
          findsOneWidget,
          reason: g.id,
        );
      }
    });
  });

  // ── 接进 claude 设置页 ───────────────────────────────────

  group('挂在 claude 设置页的「状态栏」分区', () {
    testWidgets('两个面板 + 数据参考都在，开关能存进 settings_set', (tester) async {
      await useBigSurface(tester);
      final k = FakeKernel({
        ...baseResponses(),
        'settings_get': (_) => <String, Object?>{},
        'settings_set': (_) => null,
        'sync_group_settings': (_) => null,
      });
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          SchemaConfigPage(
            kind: SchemaConfigKind.claude,
            invoke: k.invoke,
            bundleLoader: (_) async => SchemaBundle(
              sections: [
                SchemaSection({
                  'id': 'status',
                  'labelKey': 'settings.sectionStatus',
                  'fields': [
                    {
                      'key': 'fileSuggestion',
                      'label': 'File Suggestion',
                      'type': 'string',
                      'pathType': 'file',
                    },
                  ],
                }),
              ],
              recommended: const {},
            ),
          ),
          i18n,
        ),
      );
      await settle(tester);

      expect(find.byKey(const ValueKey('sl-statusline-enable')), findsOneWidget);
      expect(find.byKey(const ValueKey('sl-subagent-enable')), findsOneWidget);
      expect(find.byKey(const ValueKey('section-status-dataref')), findsOneWidget);
      // pathType 字段走带补全的路径输入行，不是普通文本行。
      expect(find.byKey(const ValueKey('path-input')), findsOneWidget);

      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('sl-statusline-enable')),
          matching: find.byType(Switch),
        ),
      );
      await settle(tester);
      await tester.tap(find.text(i18n.t('action.save')));
      await settle(tester);
      final v = (k.lastArgsOf('settings_set')!['input']! as Map)['value']! as Map;
      expect((v['_aidog_statusline']! as Map)['enabled'], true);
    });
  });
}
