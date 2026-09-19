// 页面 widget 测试的公共夹具：记录型假 invoke + 真主题 + 真文案资产。
// **不起内核**（假 invoke 顶掉传输层），所以不碰用户 9890 端口，也不碰 ~/.aidog。
import 'dart:async';
import 'dart:convert';

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// 记录每一次命令调用，并按命令名返回事先摆好的载荷。
class FakeKernel {
  FakeKernel(this.responses);

  /// 命令名 → 载荷工厂。抛出的异常会原样传给调用方（用来测各区的失败兜底）。
  final Map<String, Object? Function(Map<String, Object?>? args)> responses;

  /// 调用流水（含重复），顺序即发生顺序。
  final List<String> calls = [];

  /// 每次调用的参数，与 [calls] 同下标。
  final List<Map<String, Object?>?> argsLog = [];

  /// 去重后的命令名，排序后拼成串 —— Dart 的 Set/List 按身份比较，
  /// 直接 `expect(setA, setB)` 会永远不相等（本项目踩过：2826 个 key 全报不等）。
  String get commandSetSignature {
    final s = calls.toSet().toList()..sort();
    return s.join(',');
  }

  int countOf(String command) => calls.where((c) => c == command).length;

  Future<Object?> invoke(String command, [Map<String, Object?>? args]) async {
    calls.add(command);
    argsLog.add(args);
    final f = responses[command];
    if (f == null) throw StateError('测试夹具没给命令 $command 摆载荷');
    return f(args);
  }

  /// 取某个命令最后一次调用的参数。
  Map<String, Object?>? lastArgsOf(String command) {
    for (var i = calls.length - 1; i >= 0; i--) {
      if (calls[i] == command) return argsLog[i];
    }
    return null;
  }
}

Map<String, dynamic> statsBucket(String tb, int req, {double cost = 0}) => {
  'time_bucket': tb,
  'total_requests': req,
  'success_count': req,
  'error_count': 0,
  'input_tokens': 0,
  'output_tokens': 0,
  'cache_tokens': 0,
  'avg_duration_ms': 0,
  'total_cost': cost,
};

Map<String, dynamic> statsOverview({
  int requests = 0,
  double successRate = 0,
  int input = 0,
  int output = 0,
  int cache = 0,
  double cacheRate = 0,
  double avgMs = 0,
  double cost = 0,
}) => {
  'total_requests': requests,
  'success_rate': successRate,
  'total_input_tokens': input,
  'total_output_tokens': output,
  'total_cache_tokens': cache,
  'cache_rate': cacheRate,
  'avg_duration_ms': avgMs,
  'total_cost': cost,
};

Map<String, dynamic> dimensionEntry(
  String name, {
  int req = 0,
  int success = 0,
  double cost = 0,
}) => {
  'name': name,
  'total_requests': req,
  'success_count': success,
  'input_tokens': 0,
  'output_tokens': 0,
  'cache_tokens': 0,
  'cache_rate': 0,
  'avg_duration_ms': 0,
  'total_cost': cost,
};

Map<String, dynamic> statsResult({
  Map<String, dynamic>? overview,
  List<Map<String, dynamic>> buckets = const [],
  List<Map<String, dynamic>> dimensions = const [],
  List<String> models = const [],
  List<Map<String, dynamic>> series = const [],
}) => {
  'overview': overview ?? statsOverview(),
  'buckets': buckets,
  'dimension_data': dimensions,
  'available_models': models,
  'series': series,
};

Map<String, dynamic> platformJson(
  int id,
  String name, {
  String type = 'openai',
  double balance = 0,
}) => {
  'id': id,
  'name': name,
  'platform_type': type,
  'est_balance_remaining': balance,
};

/// `get_defaults_json` 的最小可用载荷（返回的是**字符串**，不是对象）。
String defaultsJson() => jsonEncode({
  'protocols': {
    'openai': {
      'name': {'zh-Hans': '开放人工智能', 'en-US': 'OpenAI'},
      'keywords': ['oai', 'gpt'],
    },
    'nameless': {'name': <String, String>{}, 'keywords': <String>[]},
  },
});

/// 造一个已经 init 好的 i18n 控制器（真资产，不 mock 文案）。
Future<I18nController> makeI18n(
  WidgetTester tester, {
  String locale = 'zh-Hans',
}) async {
  final c = I18nController(persist: (_) async {});
  // 真实资产加载要走 runAsync：直接 await 会和 pumpWidget 的守卫撞。
  await tester.runAsync(() => c.init(initial: locale));
  return c;
}

/// 把页面挂进「主题 + Material + i18n + 足够宽的内容区」里，与真骨架同形。
Widget wrapPage(Widget page, I18nController c, {AidogMode mode = AidogMode.dark}) {
  return MaterialApp(
    theme: aidogThemeData(mode),
    home: AidogI18n(
      controller: c,
      child: Scaffold(
        body: SingleChildScrollView(
          child: SizedBox(width: 1280, child: Material(type: MaterialType.transparency, child: page)),
        ),
      ),
    ),
  );
}

/// 测试默认画布只有 800×600，统计页一屏放不下 —— 点不到的按钮会被 `tap()` 判成
/// 「不在可命中区域」。给一块够大的画布，交互测试才测的是页面逻辑而不是滚动位置。
Future<void> useBigSurface(WidgetTester tester) async {
  await tester.binding.setSurfaceSize(const Size(1400, 4200));
  addTearDown(() => tester.binding.setSurfaceSize(null));
}

/// 推进到稳定。**不能用 `pumpAndSettle`**：骨架的 `LiveDot` 是 3 秒无限循环的呼吸动画
/// （`AnimationController..repeat`），`pumpAndSettle` 永远等不到静止，会超时。
/// 这里推几帧，足够让命令的 future、setState 与 Tile 的 180ms 过渡全部落地。
Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 40));
  }
}

/// 手动驱动的「有新请求日志」流，测试里 `add(null)` 就等于后端 emit 了一次。
class LogTicker {
  final _c = StreamController<void>.broadcast();
  Stream<void> get stream => _c.stream;
  void fire() => _c.add(null);
  Future<void> close() => _c.close();
}
