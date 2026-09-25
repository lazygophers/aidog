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

/// 外壳内边距取自 [PopoverRoot]（与设置页预览同一份真值）。
/// 宽高的上下限不在这里夹，由 `panel_channel.dart` 按 React 的 MIN_W/MAX_W 夹。
const double _rootPadY = PopoverRoot.padY;
const double _rootPadX = PopoverRoot.padX;

/// 小窗的根。固定深色（与 React 版 `applyTheme("dark")` 一致，不跟随主窗口 themeMode）。
class PopoverApp extends StatefulWidget {
  const PopoverApp({
    super.key,
    this.invoke,
    this.events,
    this.reportSize = reportTrayPanelSize,
  });

  final InvokeFn? invoke;

  /// 「有新请求日志」事件流。缺省走内核 SSE；测试塞假流。
  final Stream<Object?> Function()? events;

  /// 内容尺寸上报（测试里换成收集器）。参数 = (宽, 高)，逻辑像素。
  final Future<void> Function(double, double) reportSize;

  @override
  State<PopoverApp> createState() => _PopoverAppState();
}

class _PopoverAppState extends State<PopoverApp> {
  late final PopoverDataController _c = PopoverDataController(
    invoke: widget.invoke,
  );
  final _contentKey = GlobalKey();
  StreamSubscription<void>? _sub;
  Size _lastSize = Size.zero;

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

  /// 内容尺寸变了就让原生外壳改窗（≤1px 不动，防抖动循环 —— 对齐 React 的 `DELTA`）。
  ///
  /// **宽**取内容的 max-intrinsic 宽度，即「不被约束时它想要多宽」——
  /// 这正是 React 那边 `width: fit-content` + `offsetWidth` 量到的东西
  /// （`src/styles/popover.css:30` + `src/popover.tsx:180`）。`box.size.width`
  /// 量不出来：窗口给多宽，`Expanded` 的卡片就铺多宽，量到的永远是窗口宽本身。
  ///
  /// **高**取当前布局下的实测高度（跟着宽走，宽变了下一帧会重算）。
  ///
  /// 两者都要加上外壳自己占的那一圈：[SingleChildScrollView] 的 padding ×2
  /// （左右 14、上下 10，与 `.popover-root` 同值）。外框无边线（React 那条
  /// `border: var(--glass-border)` 少了 border-style，浏览器按 `none` 处理，
  /// 不画线），所以不再补边宽。
  void _syncSize() {
    final box = _contentKey.currentContext?.findRenderObject();
    if (box is! RenderBox || !box.hasSize) return;
    final size = Size(
      box.getMaxIntrinsicWidth(double.infinity) + 2 * _rootPadX,
      box.size.height + 2 * _rootPadY,
    );
    if ((size.width - _lastSize.width).abs() <= 1 &&
        (size.height - _lastSize.height).abs() <= 1) {
      return;
    }
    _lastSize = size;
    unawaited(widget.reportSize(size.width, size.height));
  }

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback((_) => _syncSize());
    return AnimatedBuilder(
      animation: i18n,
      builder: (context, _) => MaterialApp(
        debugShowCheckedModeBanner: false,
        locale: i18n.flutterLocale,
        theme: aidogThemeData(AidogMode.dark),
        // 小窗不走 Scaffold（窗本身就是那张卡），所以要自己给一层 Material。
        // 少了它，Flutter 会给**每一行文字**画上「缺 Material 祖先」的黄色下划线。
        // 主骨架同样的坑已在 `app_shell.dart:217` 修过，这里是第二个根。
        home: Material(
          type: MaterialType.transparency,
          child: Directionality(
            textDirection: i18n.textDirection,
            child: _body(),
          ),
        ),
      ),
    );
  }

  Widget _body() => Builder(
    builder: (context) {
      final t = AidogTheme.of(context);
      final frame = _c.frame;
      return Container(
        // `.popover-root`：底色 var(--popover)=surface、圆角 --radius-lg 16、
        // **无边框**（popover.css:38-40 的 `border: var(--glass-border)` 缺
        // border-style，浏览器按 none 处理）。`--shadow-lg` 画在盒子外面，
        // 而窗口就贴着盒子，两边都看不见，所以不画。
        decoration: BoxDecoration(
          color: t.c.surface,
          borderRadius: BorderRadius.circular(AidogRadius.lg),
        ),
        clipBehavior: Clip.antiAlias,
        child: SingleChildScrollView(
          padding: const EdgeInsets.symmetric(
            vertical: _rootPadY,
            horizontal: _rootPadX,
          ),
          child: KeyedSubtree(
            key: _contentKey,
            child: frame == null
                // `.popover-loading`：12 fg2，居中，最小高 80（popover.css:45-52）。
                ? Container(
                    constraints: const BoxConstraints(minHeight: 80),
                    alignment: Alignment.center,
                    child: Text(
                      AidogI18n.of(context).t('common.loading'),
                      style: TextStyle(
                        fontFamily: AidogType.familySans,
                        fontSize: 12,
                        color: t.c.fg2,
                      ),
                    ),
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
