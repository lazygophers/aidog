/// aidog Flutter 外壳的传输层。其余所有票只从这里 import。
///
/// ```dart
/// import 'package:aidog_flutter/transport.dart';
///
/// await kernel.start();
/// final info = await kernel.invoke<Map<String, dynamic>>('about_info');
/// kernel.on<int?>('proxy-log-updated').listen(refresh);
/// ```
///
/// **禁止**绕过它自己开 HTTP 连接（`package:http` / `HttpClient` 打 RPC 慢 13 倍，
/// 见 spec §1.1）。
library;

export 'src/transport/event_stream.dart'
    show EventStreamClient, KernelEvent, kEventsPath;
export 'src/transport/kernel.dart' show Kernel, kernel;
export 'src/transport/kernel_process.dart'
    show KernelProcess, KernelState, kListenLinePrefix, resolveKernelExecutable;
export 'src/transport/rpc_client.dart'
    show RpcClient, RpcException, RpcTransportException, kRpcUnauthorized;
