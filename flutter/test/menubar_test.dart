// 票 I20：菜单栏喂数循环的 Dart 侧。
//
// 原生那半边（NSStatusItem、富文本标题、右键菜单）在 Swift + Rust 里，跑不进
// `flutter test`，所以这里只钉可测的那半边：**喂什么** 和 **菜单动作回传后做什么**。
// 宿主接线留给实机验收。
//
// 不起内核（假 invoke 顶掉传输层），不碰 9890 端口。
import 'dart:async';
import 'dart:convert';

import 'package:aidog_flutter/src/menubar.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

/// 假的原生侧：记下 Dart 发过来的每一次 `render`，并能反向发菜单动作。
class FakeNative {
  FakeNative(this.channel) {
    TestDefaultBinaryMessengerBinding
        .instance
        .defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          return null;
        });
  }

  final MethodChannel channel;
  final List<MethodCall> calls = [];

  List<String> get rendered => calls
      .where((c) => c.method == 'render')
      .map((c) => c.arguments as String)
      .toList();

  /// 原生 → Dart（菜单项被点了）。
  Future<void> invokeDart(String method, Object? arguments) {
    return TestDefaultBinaryMessengerBinding
        .instance
        .defaultBinaryMessenger
        .handlePlatformMessage(
          channel.name,
          channel.codec.encodeMethodCall(MethodCall(method, arguments)),
          (_) {},
        );
  }
}

const Map<String, Object?> kState = {
  'layout': {
    'columns': [
      {
        'name': '今日',
        'value': r'$1.20',
        'color': {'mode': 'follow', 'value': ''},
        'font_size': 9.0,
        'two_line': false,
        'align': 'left',
        'align_row2': null,
      },
    ],
    'gaps': <Object?>[],
  },
  'separator': '  ',
  'status_text': '● Proxy Running :9890',
  'quota_text': null,
  'proxy_running': true,
};

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('aidog/menubar-test');
  late FakeNative native;

  setUp(() => native = FakeNative(channel));
  tearDown(() {
    TestDefaultBinaryMessengerBinding
        .instance
        .defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test('start() 先画一帧，喂的是 menu_bar_state 的原样 JSON', () async {
    final bridge = MenuBarBridge(
      invoke: (cmd, [args]) async => cmd == 'menu_bar_state'
          ? kState
          : throw StateError('没摆载荷的命令 $cmd'),
      channel: channel,
      refreshEvents: () => const Stream<Object?>.empty(),
    );
    addTearDown(bridge.dispose);

    await bridge.start();

    expect(native.rendered, hasLength(1));
    // 整块原样过去，不在 Dart 里拆结构 —— 拆了就会与 Rust 的 MenuBarState 漂移。
    expect(jsonDecode(native.rendered.single), kState);
  });

  test('命令挂了就保持上一帧，不把菜单栏擦成空', () async {
    var fail = false;
    final bridge = MenuBarBridge(
      invoke: (cmd, [args]) async =>
          fail ? throw StateError('kernel down') : kState,
      channel: channel,
      refreshEvents: () => const Stream<Object?>.empty(),
    );
    addTearDown(bridge.dispose);

    await bridge.start();
    expect(native.rendered, hasLength(1));

    fail = true;
    await bridge.refresh();
    expect(native.rendered, hasLength(1), reason: '失败那次不该发 render');
  });

  test('tray-refresh 事件（防抖后）触发重画', () async {
    final events = StreamController<Object?>.broadcast();
    addTearDown(events.close);
    final bridge = MenuBarBridge(
      invoke: (cmd, [args]) async => kState,
      channel: channel,
      refreshEvents: () => events.stream,
      debounce: const Duration(milliseconds: 5),
    );
    addTearDown(bridge.dispose);

    await bridge.start();
    expect(native.rendered, hasLength(1));

    // 连发三下只该重画一次 —— 防抖是尾沿触发。
    events
      ..add(null)
      ..add(null)
      ..add(null);
    await Future<void>.delayed(const Duration(milliseconds: 40));
    expect(native.rendered, hasLength(2));
  });

  test('菜单里点「停」→ proxy_stop；点「启」→ 按设置里的端口 proxy_start', () async {
    final calls = <(String, Map<String, Object?>?)>[];
    final bridge = MenuBarBridge(
      invoke: (cmd, [args]) async {
        calls.add((cmd, args));
        return switch (cmd) {
          'menu_bar_state' => kState,
          'proxy_get_settings' => {'port': 8123},
          _ => null,
        };
      },
      channel: channel,
      refreshEvents: () => const Stream<Object?>.empty(),
    );
    addTearDown(bridge.dispose);
    await bridge.start();
    calls.clear();

    await native.invokeDart('toggleProxy', true);
    expect(calls.map((c) => c.$1), ['proxy_stop']);

    calls.clear();
    await native.invokeDart('toggleProxy', false);
    expect(calls.map((c) => c.$1), ['proxy_get_settings', 'proxy_start']);
    expect(calls.last.$2, {'port': 8123});
  });

  test('端口读不到就回落 9890（与 system_logic 的默认值一致）', () async {
    final calls = <(String, Map<String, Object?>?)>[];
    final bridge = MenuBarBridge(
      invoke: (cmd, [args]) async {
        calls.add((cmd, args));
        return cmd == 'menu_bar_state' ? kState : null;
      },
      channel: channel,
      refreshEvents: () => const Stream<Object?>.empty(),
    );
    addTearDown(bridge.dispose);
    await bridge.start();
    calls.clear();

    await native.invokeDart('toggleProxy', false);
    expect(calls.last.$2, {'port': 9890});
  });
}
