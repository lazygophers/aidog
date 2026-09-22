/// 换成 `re_editor` 之后**输入法预编辑（composing）不能坏**（票 27 第 2 步）。
///
/// 换编辑器最容易坏在这里。而且它坏了普通 widget 测试照不出来：`re_editor` 只实现
/// 了 delta 那条路（`_code_input.dart:189` 的 `updateEditingValue` 是个空壳，真正
/// 干活的是 `:135` 的 `updateEditingValueWithDeltas`），所以
/// `tester.testTextInput.enterText` 这类常规助手打进去**什么也不会发生**。
///
/// 这里自己接管 `flutter/textinput` 通道：记下编辑器 attach 时拿到的连接 id，
/// 再按引擎的格式把 delta 发回去，把真实输入法那条路跑一遍 ——
/// 拼音逐字母进来、预编辑区间跟着走、选词后整段换成汉字。
///
/// 仍然照不出来的两样，交付说明里写明要人工看：候选框的**屏幕位置**
/// （要真窗口才有坐标）、系统输入源切换。
library;

import 'package:aidog_flutter/pages.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:re_editor/re_editor.dart';

import '../pages/harness.dart';
import 'pages_c_widget_test.dart' show baseResponses;

SchemaBundle bundle() => SchemaBundle(
  sections: [
    SchemaSection({
      'id': 'core',
      'labelKey': 'settings.sectionCore',
      'fields': [
        {'key': 'hooks', 'label': 'Hooks', 'type': 'json'},
      ],
    }),
  ],
  recommended: const {},
);

/// 接管文本输入通道，记下连接 id，并按引擎格式回灌 delta。
class _ImeHarness {
  int? clientId;
  bool deltaModelEnabled = false;

  /// 装在 `SystemChannels.textInput` 上，顶掉默认的 `TestTextInput`。
  /// 只需要两件事：记 id、记有没有开 delta 模式；其余调用一律放行。
  Future<Object?> handle(MethodCall call) async {
    if (call.method == 'TextInput.setClient') {
      final args = call.arguments as List<Object?>;
      clientId = args[0]! as int;
      final config = args[1]! as Map<Object?, Object?>;
      deltaModelEnabled = config['enableDeltaModel'] == true;
    }
    return null;
  }

  /// 发一条 delta。[deltaEnd] > [deltaStart] 表示替换掉这一段（选词 / 替换选区）。
  Future<void> send(
    WidgetTester tester, {
    required String oldText,
    required String deltaText,
    required int deltaStart,
    int? deltaEnd,
    required int selection,
    int composingBase = -1,
    int composingExtent = -1,
  }) async {
    await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      SystemChannels.textInput.name,
      SystemChannels.textInput.codec.encodeMethodCall(
        MethodCall('TextInputClient.updateEditingStateWithDeltas', <Object?>[
          clientId,
          <String, Object?>{
            'deltas': [
              {
                'oldText': oldText,
                'deltaText': deltaText,
                'deltaStart': deltaStart,
                'deltaEnd': deltaEnd ?? deltaStart,
                'selectionBase': selection,
                'selectionExtent': selection,
                'selectionAffinity': 'TextAffinity.downstream',
                'selectionIsDirectional': false,
                'composingBase': composingBase,
                'composingExtent': composingExtent,
              },
            ],
          },
        ]),
      ),
      (_) {},
    );
    await tester.pump();
  }
}

/// 收尾：把编辑器从树上摘掉走 `dispose`，光标闪烁的循环定时器才真的取消。
/// 不收就会被测试框架判「A Timer is still pending after the widget tree was
/// disposed」。
Future<void> blur(WidgetTester tester, CodeEditor editor) async {
  editor.focusNode!.unfocus();
  await tester.pump();
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump(const Duration(milliseconds: 200));
  debugDefaultTargetPlatformOverride = null;
}

void main() {
  late _ImeHarness ime;

  setUp(() => ime = _ImeHarness());

  Future<CodeEditor> mountAndFocus(WidgetTester tester) async {
    // 测试默认平台是 Android，而 `_CodeCursorBlinkController.startBlink` 在
    // Android/iOS 上会多挂一个 100ms 的延迟 future（`_code_editable.dart:461`），
    // 跑完还悬着就被判「Timer is still pending」。本应用是桌面端，按真实平台跑。
    // **必须在用例体内设、体内清**：框架在 tearDown 之前就查这个变量。
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    await useBigSurface(tester);
    // 必须在编辑器 attach **之前**接管通道，否则拿不到连接 id。
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.textInput,
      ime.handle,
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.textInput,
        null,
      ),
    );
    final k = FakeKernel({
      ...baseResponses(),
      'settings_get': (_) => const <String, Object?>{},
      'settings_set': (_) => null,
      'fs_autocomplete': (_) => const [],
    });
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        SchemaConfigPage(
          kind: SchemaConfigKind.claude,
          invoke: k.invoke,
          bundleLoader: (_) async => bundle(),
        ),
        i18n,
      ),
    );
    await settle(tester);
    final editor = tester.widget<CodeEditor>(find.byType(CodeEditor));
    editor.focusNode!.requestFocus();
    await settle(tester);
    return editor;
  }

  testWidgets('聚焦就接上文本输入通道，并开着 delta 模式（输入法靠它）', (tester) async {
    final editor = await mountAndFocus(tester);
    expect(ime.clientId, isNotNull, reason: '没 attach 等于输入法完全不工作');
    expect(
      ime.deltaModelEnabled,
      isTrue,
      reason: 're_editor 只实现 delta 那条路，没开 delta 模式就收不到预编辑',
    );
    await blur(tester, editor);
  });

  testWidgets('拼音逐字母进来，预编辑一路跟着走，不被切成多次提交', (tester) async {
    final editor = await mountAndFocus(tester);
    final c = editor.controller!;
    c.text = '';
    await tester.pump();

    var acc = '';
    for (final ch in ['n', 'i', 'h', 'a', 'o']) {
      final before = acc;
      acc += ch;
      await ime.send(
        tester,
        oldText: before,
        deltaText: ch,
        deltaStart: before.length,
        selection: acc.length,
        composingBase: 0,
        composingExtent: acc.length,
      );
    }
    expect(c.text, 'nihao');
    expect(c.selection.extentOffset, 5, reason: '光标跟在预编辑末尾');
    await blur(tester, editor);
  });

  testWidgets('选词：整段预编辑换成汉字，拼音不残留', (tester) async {
    final editor = await mountAndFocus(tester);
    final c = editor.controller!;
    c.text = '';
    await tester.pump();

    await ime.send(
      tester,
      oldText: '',
      deltaText: 'nihao',
      deltaStart: 0,
      selection: 5,
      composingBase: 0,
      composingExtent: 5,
    );
    expect(c.text, 'nihao');

    await ime.send(
      tester,
      oldText: 'nihao',
      deltaText: '你好',
      deltaStart: 0,
      deltaEnd: 5,
      selection: 2,
    );
    expect(c.text, '你好', reason: '选词后不该留下拼音');
    expect(c.selection.extentOffset, 2, reason: '光标落在提交内容之后');
    await blur(tester, editor);
  });

  testWidgets('在已有文本中间打字：落在插入点之后，不跳行首行尾', (tester) async {
    final editor = await mountAndFocus(tester);
    final c = editor.controller!;
    c.text = '{\n  "ab": 1\n}';
    // 第 2 行是 `  "ab": 1`：0/1 空格、2 引号、3 是 a、4 是 b。
    // 偏移 4 = 光标停在 a 与 b 之间。
    c.selection = const CodeLineSelection.collapsed(index: 1, offset: 4);
    await tester.pump();

    await ime.send(
      tester,
      oldText: '  "ab": 1',
      deltaText: 'X',
      deltaStart: 4,
      selection: 5,
    );

    expect(c.text, '{\n  "aXb": 1\n}');
    expect(c.selection.extentIndex, 1, reason: '仍在第 2 行');
    expect(c.selection.extentOffset, 5, reason: '落在刚插入的字符之后');
    await blur(tester, editor);
  });

  testWidgets('选中一段后输入：整段被替换，不是插在旁边', (tester) async {
    final editor = await mountAndFocus(tester);
    final c = editor.controller!;
    c.text = '{\n  "ab": 1\n}';
    // 选中 `ab`（偏移 3..5）。
    c.selection = const CodeLineSelection(
      baseIndex: 1,
      baseOffset: 3,
      extentIndex: 1,
      extentOffset: 5,
    );
    await tester.pump();

    await ime.send(
      tester,
      oldText: '  "ab": 1',
      deltaText: 'zz',
      deltaStart: 3,
      deltaEnd: 5,
      selection: 5,
    );
    expect(c.text, '{\n  "zz": 1\n}');
    await blur(tester, editor);
  });
}
