// 对着**真起起来的** `aidog-kernel --ui --port 0` 跑的集成测试。
//
// 硬约束：
//   - 用隔离的 `HOME`（`/tmp/aidog-I01-home*`），**绝不碰用户的 `~/.aidog`**，
//     也绝不去动用户正在跑的那个 AiDog。
//   - 两个测试组用**不同**的 HOME：内核的单实例锁按 data_dir 生效，同 HOME 起第二个
//     会打印已有地址后 `exit 0`，重连测试就无从做起。

import 'dart:convert';
import 'dart:io';

import 'package:aidog_flutter/transport.dart';
import 'package:flutter_test/flutter_test.dart';

// 三个组各用各的 HOME：内核的单实例锁按 data_dir 生效，共用一个 HOME 的话第二个内核
// 只会打印已有地址后 exit 0。
const String _home = '/tmp/aidog-I01-home';
const String _homeSse = '/tmp/aidog-I01-home-sse';
const String _homeReconnect = '/tmp/aidog-I01-home-reconnect';

Map<String, String> env(String home) => <String, String>{'HOME': home};

Future<int> freePort() async {
  final s = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
  final p = s.port;
  await s.close();
  return p;
}

/// 造一个干净的隔离 HOME，并**在内核第一次启动之前**就把代理自启关掉。
///
/// 这一步不是洁癖，是安全措施：`ProxySettings` 的默认值是 `autostart: true, port: 9890`
/// （`aidog_core/src/shared.rs:83-88`），内核起来就会去 bind 9890 —— **那是用户正在用的
/// 代理端口**。`load_proxy_settings` 在 DB 无记录时会迁移 `<data_dir>/proxy_settings.json`
/// （同文件 `:67-79`），所以先把这个文件放好，内核第一次读设置读到的就是 `autostart: false`。
///
/// 每次跑测试都先删干净：留着上一轮的 DB 就绕过了文件迁移那条路，自启开关的状态不可控。
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
  group('RPC 往返（真实内核）', () {
    late Kernel k;

    setUpAll(() async {
      await prepareIsolatedHome(_home, proxyPort: await freePort());
      k = Kernel(process: KernelProcess(environment: env(_home)));
      await k.start().timeout(const Duration(seconds: 60));
    });

    tearDownAll(() async => k.dispose());

    test('内核起来了，地址是 127.0.0.1 上的一个真端口', () {
      final addr = k.process.address!;
      expect(addr.scheme, 'http');
      expect(addr.host, '127.0.0.1');
      expect(addr.port, greaterThan(0));
      expect(k.state, KernelState.connected);
    });

    test('about_info 返回的就是命令返回值本身，没包信封', () async {
      final info = await k.invoke<Map<String, dynamic>>('about_info');
      // 字段名照抄 src/services/api/types/manual.ts:216 的 AboutInfo。
      expect(
        info.keys.toSet(),
        containsAll(<String>[
          'app_version',
          'tauri_version',
          'os',
          'arch',
          'family',
          'profile',
          'git_commit',
          'build_time',
        ]),
      );
      // 包了信封的话这里会是 {'data': ...} / {'result': ...} 之类。
      expect(info.containsKey('data'), isFalse);
      expect(info.containsKey('result'), isFalse);
      expect(info['app_version'], isA<String>());
    });

    test('无参命令：proxy_status 返回裸 bool', () async {
      expect(await k.invoke<bool>('proxy_status'), isA<bool>());
    });

    test('带参命令：参数按 lowerCamelCase 发，后端取得到', () async {
      // platform_get(id) —— 不存在的 id 会以错误 reject，正好证明参数被读到了
      // （读不到会是 400「invalid args」而不是业务错误）。
      try {
        final r = await k.invoke<Object?>('platform_get', {'id': 999999999});
        expect(r, isNull, reason: '不存在的平台应当是 null 或业务错误');
      } on RpcException catch (e) {
        expect(e.status, isNot(400), reason: '400 = 参数没被读到，键名发错了');
      }
    });

    test('未登记的命令名 → 大声失败（404），不是静默 null', () async {
      await expectLater(
        k.invoke<Object?>('no_such_command_at_all'),
        throwsA(isA<RpcException>().having((e) => e.status, 'status', 404)),
      );
    });

    test('参数类型不对 → 400，错误值是可读字符串', () async {
      await expectLater(
        k.invoke<Object?>('platform_get', {'id': 'not-a-number'}),
        throwsA(
          isA<RpcException>()
              .having((e) => e.status, 'status', 400)
              .having((e) => e.body, 'body', contains('invalid args')),
        ),
      );
    });

    test('50 个并发命令全部成功（连接池不串包）', () async {
      final rs = await Future.wait(
        List.generate(
          50,
          (_) => k.invoke<Map<String, dynamic>>('about_info'),
        ),
      );
      expect(rs.length, 50);
      expect(rs.every((r) => r['app_version'] == rs.first['app_version']), isTrue);
    });

    test('实测往返耗时：应当在 64 µs 量级，不是 862 µs（HttpClient 量级）', () async {
      final client = RpcClient(
        host: k.process.address!.host,
        port: k.process.address!.port,
      );
      // 预热：建连 + JIT。
      for (var i = 0; i < 200; i++) {
        await client.invoke<Object?>('about_info');
      }
      const n = 2000;
      final us = <int>[];
      for (var i = 0; i < n; i++) {
        final sw = Stopwatch()..start();
        await client.invoke<Object?>('about_info');
        sw.stop();
        us.add(sw.elapsedMicroseconds);
      }
      us.sort();
      final p50 = us[n ~/ 2];
      final p99 = us[(n * 99) ~/ 100];
      // ignore: avoid_print
      print('RPC round trip over persistent Socket: '
          'p50=$p50µs p99=$p99µs min=${us.first}µs max=${us.last}µs (n=$n)');

      // 对照组：同一个请求走 Dart 标准 HttpClient。
      final http = HttpClient();
      final base = k.process.address!;
      for (var i = 0; i < 100; i++) {
        final req = await http.postUrl(base.resolve('/rpc/about_info'));
        req.headers.contentType = ContentType.json;
        req.write('{}');
        await (await req.close()).drain<void>();
      }
      final httpUs = <int>[];
      for (var i = 0; i < 500; i++) {
        final sw = Stopwatch()..start();
        final req = await http.postUrl(base.resolve('/rpc/about_info'));
        req.headers.contentType = ContentType.json;
        req.write('{}');
        await (await req.close()).drain<void>();
        sw.stop();
        httpUs.add(sw.elapsedMicroseconds);
      }
      httpUs.sort();
      final httpP50 = httpUs[httpUs.length ~/ 2];
      // ignore: avoid_print
      print('RPC round trip over dart:io HttpClient: p50=$httpP50µs (n=500)');
      http.close(force: true);
      client.close();

      // 这条断言存在的意义是「哪天有人把 socket 换成 HttpClient，当场红」。
      //
      // 钉**比值**而不是绝对微秒数：绝对值随构建方式和机器变。同一台机器实测
      // release 内核 socket p50≈115 µs、debug 内核≈354 µs（debug 二进制 176 MB
      // 对 release 86 MB），拿一个绝对阈值会在 debug 下偶发红，而那时 socket 其实好好的。
      // 比值是真正的不变量：两条路在同一次运行里、同一个内核上量，倍数只反映客户端开销。
      expect(
        httpP50 / p50,
        greaterThan(3),
        reason:
            'socket p50=$p50µs / HttpClient p50=$httpP50µs，只差 '
            '${(httpP50 / p50).toStringAsFixed(1)} 倍 —— 没走持久 socket，'
            '整个迁移的性能前提就不成立了',
      );
      // 再加一道粗的绝对上限，防的是「两条路一起变慢」这种比值看不出来的退化。
      expect(
        p50,
        lessThan(1500),
        reason: 'socket p50=$p50µs 绝对值已经离谱，与走哪条路无关',
      );
    }, timeout: const Timeout(Duration(minutes: 3)));
  });

  group('SSE 事件流（真实请求打出真实事件）', () {
    late Kernel k;
    late int proxyPort;

    setUpAll(() async {
      proxyPort = await freePort();
      await prepareIsolatedHome(_homeSse, proxyPort: proxyPort);
      k = Kernel(process: KernelProcess(environment: env(_homeSse)));
      await k.start().timeout(const Duration(seconds: 60));
      // 自启已经关掉（见 prepareIsolatedHome），这里显式起在自己的空闲端口上。
      await k.invoke<Object?>('proxy_start', {'port': proxyPort});
    });

    tearDownAll(() async {
      try {
        await k.invoke<Object?>('proxy_stop');
      } on Object {
        // 进程马上要没了，停不掉也无所谓
      }
      await k.dispose();
    });

    test('真实打 3 个请求 → 收到 proxy-log-updated', () async {
      final got = <Object?>[];
      final sub = k.on<Object?>('proxy-log-updated').listen(got.add);
      addTearDown(sub.cancel);
      // SSE 连接是异步建的，先等它连上再打请求，否则事件发生在订阅之前。
      await Future<void>.delayed(const Duration(milliseconds: 500));

      final http = HttpClient();
      for (var i = 0; i < 3; i++) {
        // `GET /models` 是 tokenless 探测端点，不 relay 上游但**仍落 proxy_log**
        // （项目 CLAUDE.md「Local API」段），所以它会触发 proxy-log-updated。
        final req = await http.getUrl(
          Uri.parse('http://127.0.0.1:$proxyPort/v1/models'),
        );
        final res = await req.close();
        expect(res.statusCode, 200);
        await res.drain<void>();
      }
      http.close();

      final deadline = DateTime.now().add(const Duration(seconds: 15));
      while (got.isEmpty && DateTime.now().isBefore(deadline)) {
        await Future<void>.delayed(const Duration(milliseconds: 100));
      }
      expect(got, isNotEmpty, reason: '15 秒内没收到任何 proxy-log-updated');
      // payload 形状与 Tauri emit 一字不差：platform_id 是数字（无平台时可能是 null/0）。
      expect(got.first, anyOf(isA<num>(), isNull));
      // ignore: avoid_print
      print('SSE: got ${got.length} proxy-log-updated event(s), '
          'first payload = ${got.first}');
    }, timeout: const Timeout(Duration(seconds: 60)));
  });

  group('断线重连', () {
    test('杀掉内核子进程后自动重拉，端口变了也照样能调命令', () async {
      await prepareIsolatedHome(_homeReconnect, proxyPort: await freePort());
      final k = Kernel(
        process: KernelProcess(environment: env(_homeReconnect)),
      );
      addTearDown(k.dispose);

      final first = await k.start().timeout(const Duration(seconds: 60));
      expect(await k.invoke<bool>('proxy_status'), isA<bool>());

      final states = <KernelState>[];
      final sub = k.states.listen(states.add);
      addTearDown(sub.cancel);
      // 订阅要在杀之前挂好 —— `start()` 返回的是上一次那个**已完成**的 future（旧端口），
      // 等它等不到重连。下一个真实地址走 `addresses`。
      final nextAddress = k.process.addresses.first;

      final pid = k.process.pid!;
      expect(Process.killPid(pid, ProcessSignal.sigkill), isTrue);

      // 退避是 1s / 2s / 4s，第一次重拉 1 秒后发起；内核冷启动几秒。
      final second = await nextAddress.timeout(const Duration(seconds: 90));

      expect(
        states,
        contains(KernelState.reconnecting),
        reason: 'UI 要能看到「重连中」这个态',
      );
      expect(k.state, KernelState.connected);
      expect(
        second.port,
        isNot(first.port),
        reason: '--port 0 每次由系统挑，重拉后端口必然换一个',
      );
      // 门面自己换到了新地址：调用方不需要知道刚才断过。
      final info = await k.invoke<Map<String, dynamic>>('about_info');
      expect(info['app_version'], isA<String>());

      // 再杀一次，这回在**重连窗口里**发命令：应当等到新内核起来再打，而不是当场失败。
      final reconnecting = k.states.firstWhere(
        (s) => s == KernelState.reconnecting,
      );
      Process.killPid(k.process.pid!, ProcessSignal.sigkill);
      await reconnecting;
      expect(
        await k.invoke<bool>('proxy_status').timeout(
          const Duration(seconds: 90),
        ),
        isA<bool>(),
      );
    }, timeout: const Timeout(Duration(minutes: 4)));
  });

  group('可执行文件定位', () {
    test('AIDOG_KERNEL_BIN 之外，能从仓库里找到构建产物', () {
      final path = resolveKernelExecutable();
      expect(File(path).existsSync(), isTrue);
    });

    test('找不到就抛，不静默回落到一个猜出来的路径', () {
      expect(
        () => resolveKernelExecutable(startDir: Directory.systemTemp.path),
        Platform.environment.containsKey('AIDOG_KERNEL_BIN')
            ? returnsNormally
            : throwsStateError,
      );
    });
  });

  test('地址行前缀与 Rust 侧常量一致', () {
    // main.rs::LISTEN_LINE_PREFIX
    expect(kListenLinePrefix, 'AIDOG_KERNEL_LISTEN=');
    expect(kEventsPath, '/events');
    expect(kRpcUnauthorized, 'unauthorized');
  });
}
