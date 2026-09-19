/// 页面层调命令的唯一入口 + 测试缝。
///
/// I01 定了「不生成 203 个 Dart 绑定，只有一个泛型 invoke」（见 flutter/README.md）。
/// 页面不该直接抓全局 `kernel`，否则 widget 测试必须真起内核。这里把它收成一个
/// 函数类型，页面构造函数默认取 [kernelInvoke]，测试传假的。
library;

import 'package:aidog_flutter/transport.dart';

/// 一次命令调用。`args` 的键名按 Tauri v2 的约定走 camelCase，
/// 后端 `http_command.rs` 负责 camelCase → snake_case。
typedef InvokeFn = Future<Object?> Function(String cmd, [Map<String, Object?>? args]);

/// 生产实现：走 I01 的持久 socket 客户端。
Future<Object?> kernelInvoke(String cmd, [Map<String, Object?>? args]) =>
    kernel.invoke<Object?>(cmd, args);
