// 传输层门面：其余 13 张票唯一该碰的东西。
//
// ```dart
// await kernel.start();
// final info = await kernel.invoke<Map<String, dynamic>>('about_info');
// kernel.on<int?>('proxy-log-updated').listen((platformId) { ... });
// ```
//
// 三件事由它兜住：
// 1. 拉起 / 重拉 `aidog-kernel` 子进程（[KernelProcess]）。
// 2. 命令走持久 Socket（[RpcClient]），**不要自己开 HTTP 连接**。
// 3. 事件走单条 SSE 扇出（[EventStreamClient]）。
//
// 内核重拉后端口会变（`--port 0`），门面监听 [KernelProcess.addresses] 自动重建 RPC 连接池
// 与 SSE 连接 —— 调用方不需要知道这件事。

import 'dart:async';

import 'event_stream.dart';
import 'kernel_process.dart';
import 'rpc_client.dart';

/// 进程级单例。测试里自己 new 一个，别用它。
final Kernel kernel = Kernel();

class Kernel {
  Kernel({KernelProcess? process, this.authToken, this.invokeTimeout})
    : process = process ?? KernelProcess() {
    _events = EventStreamClient(authToken: authToken);
    this.process.addresses.listen(_onAddress);
  }

  final KernelProcess process;

  /// 内核管理面凭据（`KernelSettings::auth_token`）。
  ///
  /// 默认 `null` = 不带 Bearer。内核默认也不配凭据，且管理面永远只绑 127.0.0.1。
  /// 用户若在设置里配了凭据，外壳得先从别处拿到它再传进来 —— 那条路还没接（见 I01 交付说明）。
  final String? authToken;

  /// 单条命令的超时。`null` = 不超时（有的命令本来就要打上游，秒级是正常的）。
  final Duration? invokeTimeout;

  late final EventStreamClient _events;
  RpcClient? _rpc;

  KernelState get state => process.state;
  Stream<KernelState> get states => process.states;

  /// 所有后端事件。按名字取用 [on]。
  Stream<KernelEvent> get events => _events.events;

  /// 订阅一个事件名，payload 按 [T] 取。等价 React 版的 `listen<T>(name, handler)`。
  Stream<T> on<T>(String name) => _events.on<T>(name);

  /// 拉起内核并等到可用。重复调用无副作用。
  Future<Uri> start() async {
    final url = await process.start();
    _onAddress(url);
    return url;
  }

  /// 调一个命令。返回值就是命令返回值本身，不包信封。
  ///
  /// 内核还没连上时会等到连上为止（首帧渲染时命令先于内核就绪是常态）。
  Future<T> invoke<T>(String command, [Map<String, Object?>? args]) async {
    final client = await _clientReady();
    final call = client.invoke<T>(command, args);
    final t = invokeTimeout;
    return t == null ? call : call.timeout(t);
  }

  Future<RpcClient> _clientReady() async {
    await process.start();
    // 重连中：`start()` 返回的是上一次那个已完成的 future（旧端口），不能拿来用。等下一个
    // 真实地址。**不会自动重试已经发出去的命令** —— 命令不是幂等的，`platform_create`
    // 重试一次就是两个平台。连接在途中断由调用方处理。
    var url = process.state == KernelState.connected ? process.address : null;
    url ??= await process.addresses.first;
    // 不靠「地址流的回调先于 start() 的 then 跑」这种事件顺序，自己对一遍。
    // [_onAddress] 对同一个地址是幂等的。
    _onAddress(url);
    return _rpc!;
  }

  void _onAddress(Uri url) {
    if (_rpcUrl == url && _rpc != null) return;
    _rpcUrl = url;
    _rpc?.close();
    _rpc = RpcClient(host: url.host, port: url.port, authToken: authToken);
    _events.connect(url);
  }

  Uri? _rpcUrl;

  Future<void> dispose() async {
    _rpc?.close();
    _rpc = null;
    _rpcUrl = null;
    await _events.close();
    await process.stop();
  }
}
