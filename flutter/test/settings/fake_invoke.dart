/// 设置页测试共用的假 [InvokeFn]：记下调用、按命令名给回值或抛错。
///
/// 不用 mock 框架 —— 一个可调用对象就够，行为一眼看得完。
library;

import 'package:aidog_flutter/src/pages/invoke.dart';

class InvokeCall {
  InvokeCall(this.cmd, this.args);

  final String cmd;
  final Map<String, Object?>? args;

  @override
  String toString() => '$cmd($args)';
}

class FakeInvoke {
  FakeInvoke([Map<String, Object?>? responses]) : responses = {...?responses};

  /// 命令名 → 返回值。值是 `Function()` 时会被调用（用来每次返回不同结果）。
  final Map<String, Object?> responses;

  /// 命令名 → 要抛的错误。
  final Map<String, Object> errors = {};

  final List<InvokeCall> calls = [];

  /// 本次测试里被调到的命令名集合（命令覆盖断言用）。
  Set<String> get commands => calls.map((c) => c.cmd).toSet();

  List<InvokeCall> callsTo(String cmd) => calls.where((c) => c.cmd == cmd).toList();

  InvokeCall? lastCallTo(String cmd) {
    final l = callsTo(cmd);
    return l.isEmpty ? null : l.last;
  }

  Future<Object?> call(String cmd, [Map<String, Object?>? args]) async {
    calls.add(InvokeCall(cmd, args));
    if (errors.containsKey(cmd)) throw errors[cmd]!;
    final r = responses[cmd];
    return r is Object? Function() ? r() : r;
  }

  /// 作为 [InvokeFn] 传给页面 / 控制器。
  InvokeFn get fn => call;
}
