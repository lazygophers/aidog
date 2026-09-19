// SSE 事件流客户端（spec §1.1 第 2、3 条）。
//
// **整个进程只开一条 `/events` 连接**，按事件名扇出给 N 个订阅者。照抄
// `src/services/api/proxy.ts:109-130` 的形状 —— 那条注释记着一次已经踩过的坑：曾经 7 个页面
// 各自 listen，主窗口里同时挂 7 个监听。
//
// **为什么这里用 `HttpClient` 而 RPC 那边不用**：13 倍的差距是**每次请求**的客户端抽象开销
// （连接协商 + 请求对象构造 + 响应对象包装，862 µs vs 64 µs）。SSE 是一条开一次、活到进程
// 结束的长连接，那笔开销只付一次、且付在启动时，换来的是 chunked 解码与 keep-alive 分帧由
// 标准库负责 —— 自己写这段约 90 行，解错了是静默的事件丢失。
//
// 断线重连必须自己写：浏览器的 `EventSource` 自带重连（`transport.ts:105-108` 靠的就是它），
// Dart 没有对应物。

import 'dart:async';
import 'dart:convert';
import 'dart:io';

/// 一条后端事件。形状与 Tauri 的 `emit(name, payload)` 一字不差：
/// 事件名 = `emit` 的第一个参数，[payload] = payload 的 JSON 解码结果。
class KernelEvent {
  const KernelEvent(this.event, this.payload);
  final String event;
  final Object? payload;

  @override
  String toString() => 'KernelEvent($event, $payload)';
}

/// 管理面 SSE 端点，与 `aidog_kernel::server::management_router` 的路由一致。
const String kEventsPath = '/events';

const Duration _backoffInitial = Duration(seconds: 1);
const Duration _backoffMax = Duration(seconds: 30);

/// 单连接 SSE 客户端。[connect] 可重复调用（内核重拉后端口会变），每次都会丢掉旧连接。
class EventStreamClient {
  EventStreamClient({this.authToken});

  final String? authToken;

  final StreamController<KernelEvent> _out =
      StreamController<KernelEvent>.broadcast();

  /// 所有事件。按事件名过滤用 [on]。
  Stream<KernelEvent> get events => _out.stream;

  /// 订阅某个事件名，payload 按 [T] 取。与 `listen<T>(name, handler)` 对齐。
  Stream<T> on<T>(String name) =>
      _out.stream.where((e) => e.event == name).map((e) => e.payload as T);

  Uri? _base;
  HttpClient? _http;
  StreamSubscription<String>? _sub;
  Duration _backoff = _backoffInitial;
  bool _closed = false;
  int _generation = 0;

  /// 是否有一条活着的连接。
  bool get connected => _sub != null;

  /// 连到 [base]（内核的 `http://127.0.0.1:<port>`）。换地址即重连。
  void connect(Uri base) {
    if (_closed) return;
    _base = base;
    _generation++;
    _backoff = _backoffInitial;
    unawaited(_open(_generation));
  }

  Future<void> _open(int generation) async {
    if (_closed || generation != _generation) return;
    final base = _base;
    if (base == null) return;
    await _sub?.cancel();
    _sub = null;
    _http?.close(force: true);

    try {
      final http = _http = HttpClient()..idleTimeout = const Duration(days: 1);
      final req = await http.openUrl('GET', base.resolve(kEventsPath));
      req.headers.set(HttpHeaders.acceptHeader, 'text/event-stream');
      final token = authToken;
      if (token != null && token.isNotEmpty) {
        req.headers.set(HttpHeaders.authorizationHeader, 'Bearer $token');
      }
      final res = await req.close();
      if (res.statusCode != 200) {
        throw HttpException('/events returned ${res.statusCode}');
      }
      if (generation != _generation || _closed) {
        http.close(force: true);
        return;
      }
      _backoff = _backoffInitial;

      var name = 'message';
      final data = StringBuffer();
      _sub = res
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(
            (line) {
              if (line.isEmpty) {
                // 空行 = 一帧结束。
                if (data.isNotEmpty) {
                  _dispatch(name, data.toString());
                }
                name = 'message';
                data.clear();
                return;
              }
              if (line.startsWith(':')) return; // 注释帧（keep-alive）
              final colon = line.indexOf(':');
              final field = colon < 0 ? line : line.substring(0, colon);
              var value = colon < 0 ? '' : line.substring(colon + 1);
              if (value.startsWith(' ')) value = value.substring(1);
              switch (field) {
                case 'event':
                  name = value;
                case 'data':
                  if (data.isNotEmpty) data.write('\n');
                  data.write(value);
                default:
                  break; // id / retry：内核不发，不实现
              }
            },
            onError: (Object e) => _retry(generation, '$e'),
            onDone: () => _retry(generation, 'stream closed'),
            cancelOnError: true,
          );
    } catch (e) {
      _retry(generation, '$e');
    }
  }

  void _dispatch(String name, String raw) {
    Object? payload;
    try {
      payload = jsonDecode(raw);
    } catch (_) {
      // 能到这里的是坏帧（keep-alive 注释已在上面吃掉），跳过而不是炸掉整条流。
      return;
    }
    if (!_out.isClosed) _out.add(KernelEvent(name, payload));
  }

  void _retry(int generation, String reason) {
    if (_closed || generation != _generation) return;
    _sub = null;
    final delay = _backoff;
    _backoff = delay * 2 > _backoffMax ? _backoffMax : delay * 2;
    Timer(delay, () => _open(generation));
  }

  Future<void> close() async {
    _closed = true;
    _generation++;
    await _sub?.cancel();
    _sub = null;
    _http?.close(force: true);
    await _out.close();
  }
}
