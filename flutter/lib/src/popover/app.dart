/// 托盘小窗的第二个 Flutter 引擎里跑的那棵树（票 I11）——
/// 对齐 `src/popover.tsx`。
///
/// macOS 没有 `FlutterEngineGroup`（iOS / Android 专有，
/// <https://docs.flutter.dev/add-to-app/multiple-flutters>），所以这是一个**完整的
/// 第二引擎**：独立 isolate、独立 `kernel` 单例、独立 i18n。
///
/// **内核不会被起第二份**：`aidog-kernel` 自带按 data_dir 的单实例锁，第二个进程
/// 打印已有地址就 `exit 0`（`kernel_process.dart` 的 `_adoptedExternal`）。所以这里
/// 照常 `kernel.start()` 即可复用主窗口那个内核，不需要跨引擎传地址。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../transport.dart';
import '../pages/invoke.dart';
import '../shell/theme.dart';
import 'cards.dart';
import 'model.dart';
import 'panel_channel.dart';

/// 小窗一帧数据的装载器。四路来源各自独立失败（对齐 `popover.tsx` 的
/// `.catch(() => {})`）—— 一路挂掉只让那一块空着，不整窗白屏。
class PopoverDataController extends ChangeNotifier {
  PopoverDataController({InvokeFn? invoke}) : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;

  PopoverFrame? frame;

  /// 防慢后端晚到的 resolve 覆盖新一轮结果。
  int _generation = 0;

  Future<void> reload() async {
    final gen = ++_generation;
    Map<String, Object?> data;
    try {
      data = _map(await _invoke('popover_data'));
    } catch (_) {
      return; // 主数据拿不到就保持上一帧，不把已有内容擦成空
    }
    if (gen != _generation) return;

    // 主数据到手先渲染一帧（统计卡此时是 loading），再补统计 / 分组。
    frame = PopoverFrame(data: data, groups: frame?.groups ?? const []);
    notifyListeners();

    final config = PopoverFrame(data: data).config;
    final q = popoverStatsQueries(config);

    final results = await Future.wait<Object?>([
      _safe(() => _invoke('group_list')),
      _safe(() => _invoke('group_detail_list')),
      if (q.queries.isNotEmpty)
        _safe(() => _invoke('stats_query_batch', {'queries': q.queries})),
    ]);
    if (gen != _generation) return;

    final stats = <String, Map<String, Object?>>{};
    if (q.queries.isNotEmpty && results.length > 2 && results[2] is List) {
      final list = results[2]! as List;
      for (var i = 0; i < q.itemIds.length && i < list.length; i++) {
        final r = list[i];
        if (r is Map) stats[q.itemIds[i]] = Map<String, Object?>.from(r);
      }
    }
    frame = PopoverFrame(
      data: data,
      groups: _list(results[0]),
      // group_detail_list 失败 → 空列表（已加载但没这个组），不是 null（加载中）。
      groupDetails: _list(results[1]),
      stats: stats,
      statsLoaded: true,
    );
    notifyListeners();
  }

  static Future<Object?> _safe(Future<Object?> Function() f) async {
    try {
      return await f();
    } catch (_) {
      return null;
    }
  }

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};

  static List<Map<String, Object?>> _list(Object? v) => (v as List? ?? const [])
      .whereType<Map>()
      .map(Map<String, Object?>.from)
      .toList(growable: false);
}

/// 小窗的根。固定深色（与 React 版 `applyTheme("dark")` 一致，不跟随主窗口 themeMode）。
class PopoverApp extends StatefulWidget {
  const PopoverApp({
    super.key,
    this.invoke,
    this.events,
    this.reportHeight = reportTrayPanelHeight,
  });

  final InvokeFn? invoke;

  /// 「有新请求日志」事件流。缺省走内核 SSE；测试塞假流。
  final Stream<Object?> Function()? events;

  /// 内容高度上报（测试里换成收集器）。
  final Future<void> Function(double) reportHeight;

  @override
  State<PopoverApp> createState() => _PopoverAppState();
}

class _PopoverAppState extends State<PopoverApp> {
  late final PopoverDataController _c = PopoverDataController(
    invoke: widget.invoke,
  );
  final _contentKey = GlobalKey();
  StreamSubscription<void>? _sub;
  double _lastHeight = 0;

  @override
  void initState() {
    super.initState();
    _c.addListener(_onData);
    unawaited(_c.reload());
    // 代理跑起来后数字会变；1000 ms 防抖与 React 版一致。
    _sub = debounceStream(
      (widget.events ?? kernelProxyLogUpdated)(),
      delay: const Duration(seconds: 1),
    ).listen((_) => unawaited(_c.reload()));
  }

  void _onData() {
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    unawaited(_sub?.cancel());
    _c.removeListener(_onData);
    _c.dispose();
    super.dispose();
  }

  /// 内容高度变了就让原生外壳改窗高（≤1px 不动，防抖动循环 —— 对齐 React 的 `DELTA`）。
  void _syncHeight() {
    final box = _contentKey.currentContext?.findRenderObject();
    if (box is! RenderBox || !box.hasSize) return;
    final h = box.size.height + 2 * AidogSpace.smd;
    if ((h - _lastHeight).abs() <= 1) return;
    _lastHeight = h;
    unawaited(widget.reportHeight(h));
  }

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback((_) => _syncHeight());
    return AnimatedBuilder(
      animation: i18n,
      builder: (context, _) => MaterialApp(
        debugShowCheckedModeBanner: false,
        locale: i18n.flutterLocale,
        theme: aidogThemeData(AidogMode.dark),
        home: Directionality(
          textDirection: i18n.textDirection,
          child: _body(),
        ),
      ),
    );
  }

  Widget _body() => Builder(
    builder: (context) {
      final t = AidogTheme.of(context);
      final frame = _c.frame;
      return Container(
        decoration: BoxDecoration(
          color: t.c.bg,
          borderRadius: BorderRadius.circular(AidogRadius.lg),
          border: Border.all(color: t.c.line),
        ),
        clipBehavior: Clip.antiAlias,
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(AidogSpace.smd),
          child: KeyedSubtree(
            key: _contentKey,
            child: frame == null
                ? Text(
                    i18n.t('common.loading'),
                    style: AidogType.caption.copyWith(color: t.c.fg3),
                  )
                : PopoverGrid(frame: frame),
          ),
        ),
      );
    },
  );
}

/// 小窗引擎的入口。`TrayPanel.swift` 用 `engine.run(withEntrypoint: "popoverMain")`
/// 拉起的就是它（经 `lib/main.dart` 的同名转发）。
Future<void> runPopoverApp() async {
  WidgetsFlutterBinding.ensureInitialized();
  await i18n.init();
  runApp(const AidogI18n(child: PopoverApp()));
  // 内核地址由单实例锁复用主窗口那份，见本文件头。
  unawaited(
    kernel.start().then((_) => i18n.loadFromBackend()).catchError((Object e) {
      debugPrint('popover kernel start failed: $e');
      return null;
    }),
  );
}
