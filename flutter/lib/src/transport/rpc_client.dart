// RPC 客户端：持久 Socket + 自拼 HTTP/1.1（spec §1.1 第 1 条）。
//
// 为什么不用 `package:http` / `dart:io` 的 `HttpClient`：实测同一个请求走 Dart 标准
// `HttpClient` 是 862 µs，走持久 Socket 是 64 µs，**慢 13 倍**。那 800 µs 是客户端抽象的
// 开销，不是进程边界的。所以这一层自己拼请求行、自己解响应。
//
// 协议照抄 `aidog_core/src/http_command.rs`：
//   POST /rpc/<命令名>，body = args 对象；200 = 返回值 JSON；非 2xx = 错误值 JSON。
//   参数名先按 lowerCamelCase 取、取不到再按 snake_case 取，所以 Dart 侧发 camelCase 即可。
//
// 并发模型：一条连接同一时刻只跑一个请求（HTTP/1.1 不做 pipelining），多个并发请求各占一条
// 连接，用完放回空闲池复用。**不能只开一条连接** —— aidog 有会打上游、耗时数秒的命令
// （余额查询、价格同步），单连接会把它后面排队的所有 UI 请求一起卡住。

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/foundation.dart' show compute;

/// 命令失败（= Tauri 侧 `invoke` 的 reject）。
///
/// [body] 就是错误值本身，与 React 版 `catch (e)` 拿到的一字不差：`Result<_, String>` 的
/// 命令是一个 `String`，结构化错误（如 `ProxyStartError`）是一个 `Map`。
class RpcException implements Exception {
  RpcException(this.status, this.body, {this.command});

  /// HTTP 状态码。400 = 参数解析失败，500 = 命令自身失败，401 = 凭据不对。
  final int status;

  /// 错误值的 Dart 形态（已 JSON 解码）。
  final Object? body;

  /// 出错的命令名，只用于诊断。
  final String? command;

  @override
  String toString() => 'RpcException($status, command=$command): $body';
}

/// 传输层本身出错（连不上、响应不是合法 HTTP）。与 [RpcException] 分开：后者是内核说「不行」，
/// 这个是「话都没说上」。
class RpcTransportException implements Exception {
  RpcTransportException(this.message);
  final String message;
  @override
  String toString() => 'RpcTransportException: $message';
}

/// 未授权时管理面 reject 的值（内核 `server.rs::UNAUTHORIZED_BODY`）。
const String kRpcUnauthorized = 'unauthorized';

/// 管理面 RPC 前缀，与 `aidog_kernel::rpc::rpc_router` 的路由一致。
const String _rpcPrefix = '/rpc/';

/// 空闲连接上限。超出的直接关掉，不留着占 fd。
const int _maxIdleConnections = 8;

class RpcClient {
  RpcClient({required this.host, required this.port, this.authToken});

  final String host;
  final int port;

  /// 内核 `KernelSettings::auth_token`。空 = 内核不校验。
  final String? authToken;

  final List<_Conn> _idle = <_Conn>[];
  bool _closed = false;

  /// 调一个命令。语义等价前端的 `invoke<T>(name, args)`。
  ///
  /// 返回值就是命令返回值本身，**不包信封**。失败抛 [RpcException]。
  Future<T> invoke<T>(String command, [Map<String, Object?>? args]) async {
    if (_closed) throw RpcTransportException('client is closed');
    final payload = utf8.encode(jsonEncode(args ?? const <String, Object?>{}));
    final conn = await _acquire();
    final _HttpResponse res;
    try {
      res = await conn.request(
        path: '$_rpcPrefix$command',
        body: payload,
        authToken: authToken,
      );
    } catch (_) {
      conn.destroy();
      rethrow;
    }
    if (res.keepAlive) {
      _release(conn);
    } else {
      conn.destroy();
    }

    // 无返回值的命令（Rust `()`）body 是 "null"；空 body 也按 null 处理。
    Object? data;
    if (res.body.isNotEmpty) {
      final text = utf8.decode(res.body);
      try {
        // 大载荷（模型快照 3-4MB）扔到后台 isolate 解码，免得 UI 单帧长阻塞
        // （性能审计 #7）；100KB 以下留在原地，省一次 isolate 往返。
        data = text.length > 100_000
            ? await compute(_jsonDecodeTopLevel, text)
            : jsonDecode(text);
      } catch (_) {
        // 网络边界，回什么都可能：命中静态资源 fallback 拿到 index.html、被反向代理拦下拿到
        // 它自己的错误页，都不是 JSON。把原文当错误值抛出去，别让解析异常顶替真正的失败原因。
        throw RpcException(res.status, text, command: command);
      }
    }
    if (res.status < 200 || res.status >= 300) {
      throw RpcException(res.status, data, command: command);
    }
    return data as T;
  }

  Future<_Conn> _acquire() async {
    while (_idle.isNotEmpty) {
      final c = _idle.removeLast();
      if (c.usable) return c;
      c.destroy();
    }
    return _Conn.connect(host, port);
  }

  void _release(_Conn c) {
    if (_closed || !c.usable || _idle.length >= _maxIdleConnections) {
      c.destroy();
      return;
    }
    _idle.add(c);
  }

  /// 关掉所有空闲连接。在途请求各自的连接由它自己结束时回收。
  void close() {
    _closed = true;
    for (final c in _idle) {
      c.destroy();
    }
    _idle.clear();
  }
}

// ─── HTTP/1.1 手工层 ────────────────────────────────────────────

class _HttpResponse {
  _HttpResponse(this.status, this.body, this.keepAlive);
  final int status;
  final Uint8List body;
  final bool keepAlive;
}

class _Conn {
  _Conn._(this._socket, this._hostHeader) {
    _sub = _socket.listen(
      (chunk) {
        _buf.add(chunk);
        _wake();
      },
      onError: (Object e) {
        _error = e;
        _eof = true;
        _wake();
      },
      onDone: () {
        _eof = true;
        _wake();
      },
      cancelOnError: false,
    );
  }

  static Future<_Conn> connect(String host, int port) async {
    final s = await Socket.connect(host, port);
    // Nagle 会把小请求攒到几十毫秒再发，64 µs 的往返就无从谈起。
    s.setOption(SocketOption.tcpNoDelay, true);
    return _Conn._(s, '$host:$port');
  }

  final Socket _socket;
  final String _hostHeader;
  late final StreamSubscription<Uint8List> _sub;
  final _ByteBuf _buf = _ByteBuf();
  Completer<void>? _waiter;
  Object? _error;
  bool _eof = false;
  bool _dead = false;
  bool _busy = false;

  bool get usable => !_dead && !_eof && _error == null && !_busy;

  void _wake() {
    final w = _waiter;
    if (w != null && !w.isCompleted) {
      _waiter = null;
      w.complete();
    }
  }

  Future<void> _fill() {
    if (_error != null) {
      return Future<void>.error(RpcTransportException('socket error: $_error'));
    }
    if (_eof) {
      return Future<void>.error(
        RpcTransportException('connection closed by kernel'),
      );
    }
    final w = _waiter ??= Completer<void>();
    return w.future;
  }

  Future<_HttpResponse> request({
    required String path,
    required List<int> body,
    String? authToken,
  }) async {
    _busy = true;
    try {
      final head = StringBuffer()
        ..write('POST $path HTTP/1.1\r\n')
        ..write('Host: $_hostHeader\r\n')
        ..write('Content-Type: application/json\r\n')
        ..write('Content-Length: ${body.length}\r\n');
      if (authToken != null && authToken.isNotEmpty) {
        head.write('Authorization: Bearer $authToken\r\n');
      }
      head.write('\r\n');
      // 头和体一次写出：分两次 write 会多一个 syscall，也可能拆成两个包。
      final headBytes = utf8.encode(head.toString());
      final frame = Uint8List(headBytes.length + body.length)
        ..setRange(0, headBytes.length, headBytes)
        ..setRange(headBytes.length, headBytes.length + body.length, body);
      _socket.add(frame);
      return await _readResponse();
    } finally {
      _busy = false;
    }
  }

  Future<_HttpResponse> _readResponse() async {
    int headEnd;
    var scanFrom = 0;
    while (true) {
      headEnd = _buf.indexOfHeaderEnd(scanFrom);
      if (headEnd >= 0) break;
      scanFrom = _buf.length >= 3 ? _buf.length - 3 : 0;
      await _fill();
    }
    final headText = latin1.decode(_buf.view(0, headEnd));
    _buf.consume(headEnd + 4);

    final lines = headText.split('\r\n');
    final statusLine = lines.first;
    // "HTTP/1.1 200 OK"
    final sp1 = statusLine.indexOf(' ');
    if (sp1 < 0) {
      throw RpcTransportException('malformed status line: $statusLine');
    }
    final sp2 = statusLine.indexOf(' ', sp1 + 1);
    final code = int.tryParse(
      statusLine.substring(sp1 + 1, sp2 < 0 ? statusLine.length : sp2).trim(),
    );
    if (code == null) {
      throw RpcTransportException('malformed status line: $statusLine');
    }

    int? contentLength;
    var chunked = false;
    var keepAlive = true; // HTTP/1.1 默认长连接
    for (var i = 1; i < lines.length; i++) {
      final colon = lines[i].indexOf(':');
      if (colon < 0) continue;
      final name = lines[i].substring(0, colon).trim().toLowerCase();
      final value = lines[i].substring(colon + 1).trim();
      switch (name) {
        case 'content-length':
          contentLength = int.tryParse(value);
        case 'transfer-encoding':
          chunked = value.toLowerCase().contains('chunked');
        case 'connection':
          keepAlive = value.toLowerCase() != 'close';
      }
    }

    final Uint8List payload;
    if (chunked) {
      payload = await _readChunked();
    } else if (contentLength != null) {
      while (_buf.length < contentLength) {
        await _fill();
      }
      payload = Uint8List.fromList(_buf.view(0, contentLength));
      _buf.consume(contentLength);
    } else {
      // 既无 Content-Length 也非 chunked → 读到连接关闭为止。
      keepAlive = false;
      try {
        while (true) {
          await _fill();
        }
      } on RpcTransportException {
        // EOF 即正常结束
      }
      payload = Uint8List.fromList(_buf.view(0, _buf.length));
      _buf.consume(_buf.length);
    }
    return _HttpResponse(code, payload, keepAlive);
  }

  /// chunked 解码。`/rpc/*` 的响应是 `Json<Value>`，axum 会给它 Content-Length，走不到这里；
  /// 留着是因为反向代理可能改写编码，解错了会是静默的数据损坏。
  Future<Uint8List> _readChunked() async {
    final out = BytesBuilder(copy: false);
    while (true) {
      int lineEnd;
      while ((lineEnd = _buf.indexOfCrlf(0)) < 0) {
        await _fill();
      }
      final sizeLine = latin1.decode(_buf.view(0, lineEnd));
      _buf.consume(lineEnd + 2);
      final semi = sizeLine.indexOf(';'); // chunk extension
      final size = int.tryParse(
        (semi < 0 ? sizeLine : sizeLine.substring(0, semi)).trim(),
        radix: 16,
      );
      if (size == null) {
        throw RpcTransportException('malformed chunk size: $sizeLine');
      }
      if (size == 0) {
        // trailer 段：读到空行为止
        while (true) {
          int e;
          while ((e = _buf.indexOfCrlf(0)) < 0) {
            await _fill();
          }
          final line = _buf.view(0, e);
          _buf.consume(e + 2);
          if (line.isEmpty) break;
        }
        return out.takeBytes();
      }
      while (_buf.length < size + 2) {
        await _fill();
      }
      out.add(Uint8List.fromList(_buf.view(0, size)));
      _buf.consume(size + 2); // 数据 + 结尾 CRLF
    }
  }

  void destroy() {
    if (_dead) return;
    _dead = true;
    unawaited(_sub.cancel());
    _socket.destroy();
    _wake();
  }
}

/// 可增长的字节缓冲。用 `List<int>` 也能写，但 MB 级响应（请求日志正文）下逐元素装箱太慢，
/// 而 `BytesBuilder` 不支持在不取走内容的前提下反复查看 —— 解 HTTP 需要边看边留。
class _ByteBuf {
  Uint8List _d = Uint8List(4096);
  int _len = 0;

  int get length => _len;

  void add(List<int> b) {
    if (_len + b.length > _d.length) {
      var cap = _d.length;
      while (cap < _len + b.length) {
        cap *= 2;
      }
      _d = Uint8List(cap)..setRange(0, _len, _d);
    }
    _d.setRange(_len, _len + b.length, b);
    _len += b.length;
  }

  /// 不拷贝的只读视图，调用方要留着就自己 copy。
  Uint8List view(int start, int end) => Uint8List.sublistView(_d, start, end);

  void consume(int n) {
    _d.setRange(0, _len - n, _d, n);
    _len -= n;
  }

  /// 找 `\r\n\r\n`，返回头部结束的下标（不含分隔符本身）。
  int indexOfHeaderEnd(int from) {
    for (var i = from < 0 ? 0 : from; i + 3 < _len; i++) {
      if (_d[i] == 13 &&
          _d[i + 1] == 10 &&
          _d[i + 2] == 13 &&
          _d[i + 3] == 10) {
        return i;
      }
    }
    return -1;
  }

  /// 找 `\r\n`，返回行结束的下标（不含分隔符本身）。
  int indexOfCrlf(int from) {
    for (var i = from < 0 ? 0 : from; i + 1 < _len; i++) {
      if (_d[i] == 13 && _d[i + 1] == 10) return i;
    }
    return -1;
  }
}

/// `compute` 的入口必须是顶层/静态函数：大 JSON 的后台 isolate 解码走这里。
Object? _jsonDecodeTopLevel(String text) => jsonDecode(text);
