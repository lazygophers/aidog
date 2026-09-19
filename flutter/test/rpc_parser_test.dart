// HTTP/1.1 解析层的单测：对着一个自己起的 TCP 服务，控制字节怎么切、怎么编码。
//
// 真内核只会走「Content-Length + 一次到位」那一条路径，但这层要面对的是 TCP：分片、
// chunked、Connection: close、非 2xx 带 JSON body。解错了是静默的数据损坏，所以这里逐条钉死。

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:aidog_flutter/transport.dart';
import 'package:flutter_test/flutter_test.dart';

/// 起一个只会照着 [reply] 回字节的 TCP 服务。[reply] 拿到请求原文，返回要写回去的分片列表
/// （分成多片 = 强制客户端跨包重组）。
Future<ServerSocket> fakeServer(
  List<List<int>> Function(String request) reply, {
  bool closeAfterReply = false,
}) async {
  final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
  server.listen((socket) {
    final buf = <int>[];
    socket.listen((chunk) async {
      buf.addAll(chunk);
      final text = latin1.decode(buf);
      final headEnd = text.indexOf('\r\n\r\n');
      if (headEnd < 0) return;
      final lenMatch = RegExp(
        r'content-length:\s*(\d+)',
        caseSensitive: false,
      ).firstMatch(text);
      final bodyLen = int.parse(lenMatch?.group(1) ?? '0');
      if (text.length < headEnd + 4 + bodyLen) return;
      final request = text.substring(0, headEnd + 4 + bodyLen);
      buf.clear();
      for (final part in reply(request)) {
        socket.add(part);
        await socket.flush();
        // 让客户端真的先看到前半段，再收到后半段。
        await Future<void>.delayed(const Duration(milliseconds: 5));
      }
      if (closeAfterReply) await socket.close();
    });
  });
  return server;
}

List<int> ascii(String s) => latin1.encode(s);

void main() {
  test('普通 200：返回值就是命令返回值本身，不包信封', () async {
    final server = await fakeServer((_) {
      const body = '{"app_version":"1.2.3"}';
      return [
        ascii(
          'HTTP/1.1 200 OK\r\n'
          'content-type: application/json\r\n'
          'content-length: ${body.length}\r\n\r\n$body',
        ),
      ];
    });
    final c = RpcClient(host: server.address.address, port: server.port);
    final r = await c.invoke<Map<String, dynamic>>('about_info');
    expect(r, {'app_version': '1.2.3'});
    c.close();
    await server.close();
  });

  test('请求行与 body 照抄协议：POST /rpc/<cmd>，body = args 对象', () async {
    final seen = Completer<String>();
    final server = await fakeServer((req) {
      if (!seen.isCompleted) seen.complete(req);
      return [ascii('HTTP/1.1 200 OK\r\ncontent-length: 4\r\n\r\nnull')];
    });
    final c = RpcClient(host: server.address.address, port: server.port);
    await c.invoke<Object?>('group_platform_move', {
      'platformId': 1,
      'fromGroupId': 2,
      'toGroupId': 3,
    });
    final req = await seen.future;
    expect(req, startsWith('POST /rpc/group_platform_move HTTP/1.1\r\n'));
    expect(req, contains('Content-Type: application/json\r\n'));
    expect(
      req,
      endsWith('{"platformId":1,"fromGroupId":2,"toGroupId":3}'),
    );
    c.close();
    await server.close();
  });

  test('凭据：authToken 非空才带 Authorization 头', () async {
    final seen = <String>[];
    final server = await fakeServer((req) {
      seen.add(req);
      return [ascii('HTTP/1.1 200 OK\r\ncontent-length: 4\r\n\r\nnull')];
    });
    final withToken = RpcClient(
      host: server.address.address,
      port: server.port,
      authToken: 's3cret',
    );
    await withToken.invoke<Object?>('a');
    final without = RpcClient(host: server.address.address, port: server.port);
    await without.invoke<Object?>('b');
    expect(seen[0], contains('Authorization: Bearer s3cret\r\n'));
    expect(seen[1], isNot(contains('Authorization')));
    withToken.close();
    without.close();
    await server.close();
  });

  test('响应被 TCP 切成三片也能重组', () async {
    const body = '{"a":[1,2,3],"b":"x"}';
    final server = await fakeServer(
      (_) => [
        ascii('HTTP/1.1 200 OK\r\ncontent-len'),
        ascii('gth: ${body.length}\r\n\r\n${body.substring(0, 5)}'),
        ascii(body.substring(5)),
      ],
    );
    final c = RpcClient(host: server.address.address, port: server.port);
    expect(await c.invoke<Map<String, dynamic>>('x'), {
      'a': [1, 2, 3],
      'b': 'x',
    });
    c.close();
    await server.close();
  });

  test('chunked 编码（反向代理可能改写）也解得对', () async {
    const body = '{"ok":true}';
    final server = await fakeServer(
      (_) => [
        ascii(
          'HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n'
          '5\r\n${body.substring(0, 5)}\r\n',
        ),
        ascii('${(body.length - 5).toRadixString(16)}\r\n'
            '${body.substring(5)}\r\n0\r\n\r\n'),
      ],
    );
    final c = RpcClient(host: server.address.address, port: server.port);
    expect(await c.invoke<Map<String, dynamic>>('x'), {'ok': true});
    c.close();
    await server.close();
  });

  test('非 2xx：抛 RpcException，body 就是错误值本身（字符串）', () async {
    final server = await fakeServer((_) {
      const body = '"platform not found"';
      return [
        ascii(
          'HTTP/1.1 500 Internal Server Error\r\n'
          'content-length: ${body.length}\r\n\r\n$body',
        ),
      ];
    });
    final c = RpcClient(host: server.address.address, port: server.port);
    await expectLater(
      c.invoke<Object?>('platform_get'),
      throwsA(
        isA<RpcException>()
            .having((e) => e.status, 'status', 500)
            .having((e) => e.body, 'body', 'platform not found')
            .having((e) => e.command, 'command', 'platform_get'),
      ),
    );
    c.close();
    await server.close();
  });

  test('非 2xx：结构化错误按对象抛（ProxyStartError 那类）', () async {
    final server = await fakeServer((_) {
      const body = '{"kind":"port_in_use","port":9890}';
      return [
        ascii(
          'HTTP/1.1 500 x\r\ncontent-length: ${body.length}\r\n\r\n$body',
        ),
      ];
    });
    final c = RpcClient(host: server.address.address, port: server.port);
    await expectLater(
      c.invoke<Object?>('proxy_start'),
      throwsA(
        isA<RpcException>().having((e) => e.body, 'body', {
          'kind': 'port_in_use',
          'port': 9890,
        }),
      ),
    );
    c.close();
    await server.close();
  });

  test('401 未授权：错误值 = kRpcUnauthorized', () async {
    final server = await fakeServer((_) {
      const body = '"unauthorized"';
      return [
        ascii(
          'HTTP/1.1 401 Unauthorized\r\n'
          'content-length: ${body.length}\r\n\r\n$body',
        ),
      ];
    });
    final c = RpcClient(host: server.address.address, port: server.port);
    await expectLater(
      c.invoke<Object?>('about_info'),
      throwsA(
        isA<RpcException>()
            .having((e) => e.status, 'status', 401)
            .having((e) => e.body, 'body', kRpcUnauthorized),
      ),
    );
    c.close();
    await server.close();
  });

  test('body 不是 JSON（命中静态资源 fallback）：原文当错误抛，别让解析异常顶替', () async {
    final server = await fakeServer((_) {
      const body = '<!doctype html><html></html>';
      return [
        ascii(
          'HTTP/1.1 200 OK\r\ncontent-length: ${body.length}\r\n\r\n$body',
        ),
      ];
    });
    final c = RpcClient(host: server.address.address, port: server.port);
    await expectLater(
      c.invoke<Object?>('about_info'),
      throwsA(
        isA<RpcException>().having(
          (e) => e.body,
          'body',
          '<!doctype html><html></html>',
        ),
      ),
    );
    c.close();
    await server.close();
  });

  test('空 body（Rust `()` 命令）→ null', () async {
    final server = await fakeServer(
      (_) => [ascii('HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n')],
    );
    final c = RpcClient(host: server.address.address, port: server.port);
    expect(await c.invoke<Object?>('proxy_stop'), isNull);
    c.close();
    await server.close();
  });

  test('Connection: close 后不复用死连接，下一次自己重连', () async {
    var served = 0;
    final server = await fakeServer(
      (_) {
        served++;
        const body = '1';
        return [
          ascii(
            'HTTP/1.1 200 OK\r\nconnection: close\r\n'
            'content-length: ${body.length}\r\n\r\n$body',
          ),
        ];
      },
      closeAfterReply: true,
    );
    final c = RpcClient(host: server.address.address, port: server.port);
    expect(await c.invoke<int>('x'), 1);
    expect(await c.invoke<int>('x'), 1);
    expect(served, 2);
    c.close();
    await server.close();
  });

  test('连不上时抛传输层异常，不是命令异常', () async {
    final probe = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    final deadPort = probe.port;
    await probe.close();
    final c = RpcClient(host: '127.0.0.1', port: deadPort);
    await expectLater(c.invoke<Object?>('about_info'), throwsA(isA<Object>()));
    c.close();
  });

  test('并发请求各占一条连接，不互相排队', () async {
    var open = 0;
    var maxOpen = 0;
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    server.listen((socket) {
      open++;
      maxOpen = open > maxOpen ? open : maxOpen;
      socket.listen(
        (_) async {
          // 每条连接都慢 200 ms：串行的话 5 个请求要 1 秒。
          await Future<void>.delayed(const Duration(milliseconds: 200));
          socket.add(ascii('HTTP/1.1 200 OK\r\ncontent-length: 1\r\n\r\n7'));
        },
        onDone: () => open--,
      );
    });
    final c = RpcClient(host: server.address.address, port: server.port);
    final sw = Stopwatch()..start();
    final rs = await Future.wait(
      List.generate(5, (_) => c.invoke<int>('slow_command')),
    );
    sw.stop();
    expect(rs, [7, 7, 7, 7, 7]);
    expect(maxOpen, 5, reason: '5 个并发请求应各占一条连接');
    expect(
      sw.elapsedMilliseconds,
      lessThan(900),
      reason: '串行的话至少 1000 ms',
    );
    c.close();
    await server.close();
  });
}
