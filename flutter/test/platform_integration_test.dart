// 票 I12：需要**真内核**的那两项，对着真起起来的 `aidog-kernel --ui --port 0` 跑。
//
//   - `getAppVersion()` —— 真的问后端要，不是 mock 出来的字符串。
//   - 内核弹窗桥（`notif-popup`）—— 调真的 `notification_test_popup` 命令，看事件真的过来了。
//     这条线断了 = 后端所有通知的系统弹窗通道全哑（详见 platform.dart::bindKernelPopups）。
//
// 硬约束照抄 I01 的 harness：隔离 HOME，**在内核第一次启动前**就把代理自启关掉
// （默认 `autostart: true, port: 9890` 正是用户正在用的端口），绝不碰 `~/.aidog`，
// 也绝不去动用户正在跑的那个 AiDog。

import 'dart:convert';
import 'dart:io';

import 'package:aidog_flutter/platform.dart';
import 'package:aidog_flutter/transport.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_test/flutter_test.dart';

const String _home = '/tmp/aidog-I12-home';

const MethodChannel _notifChannel = MethodChannel(
  'dexterous.com/flutter/local_notifications',
);

Future<int> freePort() async {
  final s = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
  final p = s.port;
  await s.close();
  return p;
}

/// 与 `kernel_integration_test.dart::prepareIsolatedHome` 同一件事，同一个理由。
Future<void> prepareIsolatedHome(String home, {required int proxyPort}) async {
  final dir = Directory(home);
  if (dir.existsSync()) dir.deleteSync(recursive: true);
  Directory('$home/.aidog').createSync(recursive: true);
  File('$home/.aidog/proxy_settings.json').writeAsStringSync(
    jsonEncode(<String, Object>{
      'port': proxyPort,
      'autostart': false,
      'silent_launch': false,
      'bind_lan': false,
    }),
  );
}

void main() {
  // 需要 binding 才能拦 MethodChannel（第三条测试要看通知插件真收到了什么）。
  TestWidgetsFlutterBinding.ensureInitialized();
  // 但 binding 会把 `HttpOverrides.global` 换成一个「所有请求都回 400」的假实现
  // （flutter_test/binding.dart 的 `_MockHttpOverrides`），SSE 长连接当场被掐。
  // 这里要的是**真内核**，所以把它撤掉 —— 不撤的话第二条测试永远收不到事件，
  // 而且失败现象是「没事件」，看不出是被 flutter_test 掐的。
  HttpOverrides.global = null;

  group('原生能力 × 真内核', () {
    late Kernel k;

    setUpAll(() async {
      await prepareIsolatedHome(_home, proxyPort: await freePort());
      k = Kernel(
        process: KernelProcess(
          environment: <String, String>{'HOME': _home},
        ),
      );
      await k.start().timeout(const Duration(seconds: 60));
    });

    tearDownAll(() async => k.dispose());

    test('getAppVersion 真的从后端拿到版本号（对齐 .version 单一真值源）', () async {
      final v = await getAppVersion(k: k);
      expect(
        v,
        matches(RegExp(r'^\d+\.\d+\.\d+')),
        reason: '不是 x.y.z 就不是 tauri.conf.json 那个版本号',
      );
      // 与同一个命令的原始返回值对一遍：这层不该做任何加工。
      final raw = await k.invoke<Map<String, dynamic>>('about_info');
      expect(v, raw['app_version']);
    });

    test('后端要弹通知 → 外壳真的收到 notif-popup，title / body 一字不差', () async {
      final got = <Map<String, dynamic>>[];
      final sub = k.on<Map<String, dynamic>>(kNotifPopup).listen(got.add);
      addTearDown(sub.cancel);
      // SSE 连接是异步建的，先等它连上，否则事件发生在订阅之前。
      await Future<void>.delayed(const Duration(milliseconds: 500));

      // 真命令：`notification_test_popup` 绕过 dispatch 直接走弹窗通道
      // （aidog_core/src/system_cmd/notification.rs:74-82）。桌面壳那边它会调
      // tauri 插件；无界面内核这边它会 emit 给我们。
      await k.invoke<Object?>('notification_test_popup', <String, Object?>{
        'title': 'AiDog I12',
        'body': '弹窗桥活着',
      });

      final deadline = DateTime.now().add(const Duration(seconds: 15));
      while (got.isEmpty && DateTime.now().isBefore(deadline)) {
        await Future<void>.delayed(const Duration(milliseconds: 100));
      }
      expect(
        got,
        isNotEmpty,
        reason: '15 秒内没收到 notif-popup —— 后端通知的系统弹窗通道是哑的',
      );
      expect(got.first['title'], 'AiDog I12');
      expect(got.first['body'], '弹窗桥活着');
    }, timeout: const Timeout(Duration(seconds: 60)));

    test('bindKernelPopups 把事件接到本地通知上（端到端：真命令 → 真 SSE → 通知插件）',
        () async {
      // 这一条把 platform_test.dart 里被单测覆盖的「通知插件那一半」和这里的
      // 「内核那一半」接起来，证明中间没有断点。
      final shown = <MethodCall>[];
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      MacOSFlutterLocalNotificationsPlugin.registerWith();
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(_notifChannel, (call) async {
        shown.add(call);
        return call.method == 'initialize' ? true : null;
      });
      addTearDown(() {
        debugDefaultTargetPlatformOverride = null;
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_notifChannel, null);
      });

      final sub = bindKernelPopups(k: k);
      addTearDown(sub.cancel);
      await Future<void>.delayed(const Duration(milliseconds: 500));

      await k.invoke<Object?>('notification_test_popup', <String, Object?>{
        'title': '端到端',
        'body': '从 Rust 到通知中心',
      });

      final deadline = DateTime.now().add(const Duration(seconds: 15));
      while (!shown.any((c) => c.method == 'show') &&
          DateTime.now().isBefore(deadline)) {
        await Future<void>.delayed(const Duration(milliseconds: 100));
      }
      final show = shown.firstWhere(
        (c) => c.method == 'show',
        orElse: () => throw StateError('15 秒内通知插件没收到 show'),
      );
      expect((show.arguments as Map)['title'], '端到端');
      expect((show.arguments as Map)['body'], '从 Rust 到通知中心');
    }, timeout: const Timeout(Duration(seconds: 60)));
  });
}
