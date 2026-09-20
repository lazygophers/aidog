/// 页面与传输层之间唯一的接缝（票 I06 定，I07-I09 复用）。
///
/// 没有「203 个 Dart 绑定」那一层（理由见 flutter/README.md）：页面直接 `kernel.invoke`
/// 传内联 map。这里只把它包成一个可注入的函数类型，让 widget 测试能塞假实现 ——
/// 全局 `kernel` 是 `final`，不留这个口子就没法在不起内核的情况下测页面。
library;

import 'dart:async';

import '../../transport.dart';

/// 一次命令调用。返回值是命令返回值本身（JSON 解码后的 `Map` / `List` / 标量）。
///
/// `args` 的键名按 Tauri v2 的约定走 camelCase，后端 `http_command.rs`
/// 负责 camelCase → snake_case。
typedef InvokeFn =
    Future<Object?> Function(String command, [Map<String, Object?>? args]);

/// 生产实现：走 I01 的持久 socket 客户端。
Future<Object?> kernelInvoke(String command, [Map<String, Object?>? args]) =>
    kernel.invoke<Object?>(command, args);

/// 后端「有新请求日志」事件名。React 版 `src/services/api/proxy.ts::onProxyLogUpdated`。
const String kProxyLogUpdatedEvent = 'proxy-log-updated';

/// 生产实现的事件流（未防抖，防抖由 [debounceStream] 做）。
Stream<Object?> kernelProxyLogUpdated() =>
    kernel.on<Object?>(kProxyLogUpdatedEvent);

/// 防抖：突发事件只在安静 [delay] 之后发一次。
///
/// 对齐 React 版 `onProxyLogUpdated` 的 `debounceMs = 500` —— 代理一秒能跑完十个请求，
/// 不合并就是十轮重查。尾沿触发（最后一个事件之后才发），与 `setTimeout` 重置同语义。
///
/// 托盘设置页用的是 1000 ms（`TrayConfigTab.tsx:208` 显式传 1000），调用方自己传 delay。
Stream<void> debounceStream(
  Stream<Object?> source, {
  Duration delay = const Duration(milliseconds: 500),
}) {
  late StreamController<void> out;
  StreamSubscription<Object?>? sub;
  Timer? timer;
  out = StreamController<void>(
    onListen: () {
      sub = source.listen(
        (_) {
          timer?.cancel();
          timer = Timer(delay, () {
            if (!out.isClosed) out.add(null);
          });
        },
        onError: out.addError,
      );
    },
    onCancel: () async {
      timer?.cancel();
      await sub?.cancel();
    },
  );
  return out.stream;
}
