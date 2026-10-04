/// 首页（对应 React 版 `src/pages/Home.tsx`）。
///
/// 功能逐条对齐 React 版，长相按 A′ 重做（票 I06 的口径：「对齐」指功能，不指长相）：
/// 状态行（运行态 + 端口 + 复制代理地址）→ 四 KPI 读数格（行内 sparkline）→
/// 24h 双轴趋势 → 平台 Top4 → 总余额 → 快捷键 footer（⌘N/⌘S/⌘L/⌘C，chip 与键盘同动作）。
///
/// 数据源六条命令各自独立 catch：单条失败只让那一区进空态，不整页崩。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../charts.dart';
import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../stats/models.dart';
import '../../utils/formatters.dart';
import '../shell/app_shell.dart';
import '../shell/nav.dart' show NavContext;
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'home_logic.dart';
import 'invoke.dart';
import 'platform_logo.dart' show ProtocolLogo;
import 'models.dart';
import 'ui_bits.dart' show Reveal;

class HomePage extends StatefulWidget {
  const HomePage({
    super.key,
    required this.onNavigate,
    this.invoke = kernelInvoke,
    this.logUpdates,
    this.copyText = native.writeText,
    this.now = DateTime.now,
  });

  /// 侧栏切页（footer chip 与 ⌘N/⌘S/⌘L 用）。第二参 = 跳转带参
  /// （首页「按模型/按分组」行下钻统计页，home-model-stats spec §3）。
  final void Function(String id, [NavContext? context]) onNavigate;
  final InvokeFn invoke;

  /// 「有新请求日志」流；缺省是内核事件的 500ms 防抖流。
  final Stream<void>? logUpdates;

  /// 复制到剪贴板；缺省走票 I12 的原生能力。
  final Future<void> Function(String) copyText;
  final DateTime Function() now;

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  bool? _running;
  int _port = kDefaultPort;
  TodayStats? _today;
  List<PlatformSummary> _platforms = const [];
  List<StatsBucket> _trend = const [];
  // 三维度行列表（今日窗口，`Home.tsx:202-207`）：platform/model/group 各一份。
  List<DimensionEntry> _dimPlatforms = const [];
  List<DimensionEntry> _dimModels = const [];
  List<DimensionEntry> _dimGroups = const [];
  // 维度小时序列（两套窗口并存，数据不混，2026-09-27 起分离）：
  //  - _series*（滚动 24h）→ 只喂维度趋势图（窗口恒 24h）。
  //  - _todaySeries*（今日）→ 喂维度行迷你走势（跟随面板窗口）。
  List<StatsSeries> _seriesPlatforms = const [];
  List<StatsSeries> _seriesModels = const [];
  List<StatsSeries> _seriesGroups = const [];
  List<StatsSeries> _todaySeriesPlatforms = const [];
  List<StatsSeries> _todaySeriesModels = const [];
  List<StatsSeries> _todaySeriesGroups = const [];
  // 趋势图状态：默认按平台、指标 Token（2026-09-27 两次用户拍板，HomeTrendChart.tsx:96-99）。
  // 默认总计（React 2026-10-03 拍板：缺全局总览，总计作默认；此前按平台是 2026-09-27 拍板）。
  String _trendDim = 'total';
  String _trendMetric = 'tokens';
  bool _loading = true;
  bool _copied = false;

  StreamSubscription<void>? _logSub;
  Timer? _copyTimer;

  /// 并发刷新去重：事件密集时上一轮还没回来就不再发第二轮（对齐 React 的
  /// Promise.all 单轮语义 —— 那边每次事件都发一轮，但 setState 幂等；这里显式挡一次，
  /// 免得慢响应乱序覆盖新数据）。
  bool _inFlight = false;

  @override
  void initState() {
    super.initState();
    _load();
    _logSub = (widget.logUpdates ?? debounceStream(kernelProxyLogUpdated()))
        .listen((_) {
          _load();
        });
    HardwareKeyboard.instance.addHandler(_onKey);
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_onKey);
    _logSub?.cancel();
    _copyTimer?.cancel();
    super.dispose();
  }

  String get _proxyBaseUrl => 'http://127.0.0.1:$_port/proxy';

  Future<void> _load() async {
    if (_inFlight) return;
    _inFlight = true;
    final window = last24h(widget.now());

    // 一轮事件只落一次 setState：六个区各自 await，结果先攒在局部变量里，
    // 全部回来（或兜底）后一次性提交 —— 原先六个 _guard 各自 setState，
    // 一轮事件最多把整页重建 6 次（性能审计 #4）。
    bool? running = _running;
    int port = _port;
    TodayStats? today = _today;
    List<PlatformSummary> platforms = _platforms;
    List<StatsBucket> trend = _trend;
    List<DimensionEntry> dimPlatforms = _dimPlatforms;
    List<DimensionEntry> dimModels = _dimModels;
    List<DimensionEntry> dimGroups = _dimGroups;
    List<StatsSeries> seriesPlatforms = _seriesPlatforms;
    List<StatsSeries> seriesModels = _seriesModels;
    List<StatsSeries> seriesGroups = _seriesGroups;
    List<StatsSeries> todaySeriesPlatforms = _todaySeriesPlatforms;
    List<StatsSeries> todaySeriesModels = _todaySeriesModels;
    List<StatsSeries> todaySeriesGroups = _todaySeriesGroups;
    await Future.wait<void>([
      _guard(() async {
        final v = await widget.invoke('proxy_status');
        running = v as bool?;
      }, () => running = null),
      _guard(() async {
        final v = await widget.invoke('proxy_get_settings');
        final s = ProxySettingsSummary.fromJson(v! as Map<String, dynamic>);
        port = s.port;
      }, null),
      _guard(() async {
        final v = await widget.invoke('tray_today_stats');
        final s = TodayStats.fromJson(v! as Map<String, dynamic>);
        today = s;
      }, () => today = null),
      _guard(() async {
        final v = await widget.invoke('platform_list');
        platforms = [
          for (final e in v! as List)
            PlatformSummary.fromJson(e as Map<String, dynamic>),
        ];
      }, () => platforms = const []),
      _guard(() async {
        final v = await widget.invoke('stats_query', {
          'query': {
            'start': window.start,
            'end': window.end,
            'granularity': 'hourly',
          },
        });
        final r = StatsResult.fromJson(v! as Map<String, dynamic>);
        trend = r.buckets;
      }, () => trend = const []),
      // 一次 batch 六条查询（`Home.tsx:227-234`）：前三条 24h 只取 series（趋势图，
      // 不带 group_by 免算 dimension_data），后三条今日窗口取行 + 行迷你走势。维度
      // 排序（TopN 截断）必须按各自窗口聚合，前端从 24h series 切今日会错序且要
      // 重算 cache_rate 等不可加指标，故走服务端双窗口而非单查询前端过滤。
      _guard(
        () async {
          final todayStart = _todayStartMs();
          final v = await widget.invoke('stats_query_batch', {
            'queries': [
              {
                'start': window.start,
                'end': window.end,
                'granularity': 'hourly',
                'series_by': 'platform',
              },
              {
                'start': window.start,
                'end': window.end,
                'granularity': 'hourly',
                'series_by': 'model',
              },
              {
                'start': window.start,
                'end': window.end,
                'granularity': 'hourly',
                'series_by': 'group',
              },
              {
                'start': todayStart,
                'end': window.end,
                'granularity': 'hourly',
                'group_by': 'platform',
                'series_by': 'platform',
              },
              {
                'start': todayStart,
                'end': window.end,
                'granularity': 'hourly',
                'group_by': 'model',
                'series_by': 'model',
              },
              {
                'start': todayStart,
                'end': window.end,
                'granularity': 'hourly',
                'group_by': 'group',
                'series_by': 'group',
              },
            ],
          });
          StatsResult at(int i) => StatsResult.fromJson(
            (v! as List)[i] as Map<String, dynamic>,
          );

          seriesPlatforms = at(0).series;
          seriesModels = at(1).series;
          seriesGroups = at(2).series;
          dimPlatforms = at(3).dimensionData;
          dimModels = at(4).dimensionData;
          dimGroups = at(5).dimensionData;
          todaySeriesPlatforms = at(3).series;
          todaySeriesModels = at(4).series;
          todaySeriesGroups = at(5).series;
        },
        () {
          seriesPlatforms = const [];
          seriesModels = const [];
          seriesGroups = const [];
          dimPlatforms = const [];
          dimModels = const [];
          dimGroups = const [];
          todaySeriesPlatforms = const [];
          todaySeriesModels = const [];
          todaySeriesGroups = const [];
        },
      ),
    ]);
    _inFlight = false;
    if (!mounted) return;
    setState(() {
      _running = running;
      _port = port;
      _today = today;
      _platforms = platforms;
      _trend = trend;
      _dimPlatforms = dimPlatforms;
      _dimModels = dimModels;
      _dimGroups = dimGroups;
      _seriesPlatforms = seriesPlatforms;
      _seriesModels = seriesModels;
      _seriesGroups = seriesGroups;
      _todaySeriesPlatforms = todaySeriesPlatforms;
      _todaySeriesModels = todaySeriesModels;
      _todaySeriesGroups = todaySeriesGroups;
      _loading = false;
    });
  }

  /// 今日窗口起点（本地时区 00:00）：与 KPI 行 tray_today_stats 同口径
  /// （React `todayStartMs`，Home.tsx:40）。
  int _todayStartMs([DateTime? now]) {
    final n = now ?? widget.now();
    return DateTime(n.year, n.month, n.day).millisecondsSinceEpoch;
  }

  /// 单区兜底：失败只跑 [onError]，不把异常往上抛（对齐 React 的 `.catch(...)`）。
  Future<void> _guard(
    Future<void> Function() body,
    void Function()? onError,
  ) async {
    try {
      await body();
    } catch (_) {
      if (mounted && onError != null) onError();
    }
  }

  void _copyUrl() {
    widget
        .copyText(_proxyBaseUrl)
        .then((_) {
          if (!mounted) return;
          setState(() => _copied = true);
          _copyTimer?.cancel();
          _copyTimer = Timer(const Duration(milliseconds: 1500), () {
            if (mounted) setState(() => _copied = false);
          });
        })
        .catchError((Object _) {
          // 复制失败静默（与 React 版 `.catch(() => {})` 同）。
        });
  }

  /// 键位 → 动作。**不含文案** —— 键盘处理器跑在 build 之外，那里拿不到 context，
  /// 读全局 `i18n` 又会在它还没 init 时抛。文案只在 [_chips] 里取。
  Map<String, ({String kbd, String labelKey, VoidCallback run})> get _actions =>
      {
        'n': (
          kbd: '⌘N',
          labelKey: 'home.addPlatform',
          run: () => widget.onNavigate('platforms'),
        ),
        's': (
          kbd: '⌘S',
          labelKey: 'home.viewStats',
          run: () => widget.onNavigate('stats'),
        ),
        'l': (
          kbd: '⌘L',
          labelKey: 'home.viewLogs',
          run: () => widget.onNavigate('logs'),
        ),
        'c': (kbd: '⌘C', labelKey: 'home.copyBaseUrl', run: _copyUrl),
      };

  List<({String label, String kbd, VoidCallback run})> _chips(
    I18nController t,
  ) => [
    for (final a in _actions.values)
      (label: t.t(a.labelKey), kbd: a.kbd, run: a.run),
  ];

  /// 快捷键：⌘（Mac）为主、Ctrl 兼容；按住 Alt 不触发；焦点在输入框里不触发（防打断输入）；
  /// 命中即消费事件 —— ⌘C 在这里是「复制代理地址」，不消费就变成系统复制。
  bool _onKey(KeyEvent e) {
    if (e is! KeyDownEvent) return false;
    final kb = HardwareKeyboard.instance;
    if (kb.isAltPressed) return false;
    if (!(kb.isMetaPressed || kb.isControlPressed)) return false;
    final k = e.logicalKey.keyLabel.toLowerCase();
    if (k.length != 1 || k.codeUnitAt(0) < 0x61 || k.codeUnitAt(0) > 0x7a) {
      return false;
    }
    // 焦点在输入框里就放行给系统（对齐 React 的 INPUT / TEXTAREA / contentEditable 判断）。
    // 焦点节点挂的是 EditableText **内部**那个 Focus，所以要往上找 EditableText 祖先，
    // 不能直接比 `primaryFocus.context.widget is EditableText`（永远为 false）。
    final focusCtx = FocusManager.instance.primaryFocus?.context;
    if (focusCtx != null &&
        focusCtx.findAncestorWidgetOfExactType<EditableText>() != null) {
      return false;
    }
    final action = _actions[k];
    if (action == null) return false;
    action.run();
    return true;
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final tr = AidogI18n.of(context);

    final today = _today;
    final has = hasTodayData(today);
    final s = trendSeriesOf(_trend);
    final balance = totalBalanceOf(_platforms);
    // 空态与加载态的分工与 React 版一致：加载中留白（不写「加载中」三个字），
    // 加载完仍无数据才显示「今日暂无请求」。
    final emptyText = _loading ? '' : tr.t('home.noToday');

    final kpis = has && today != null
        ? <({String label, String value, Widget? spark})>[
            (
              label: tr.t('home.cost'),
              value: formatCostUsd(today.cost),
              spark: _Spark(values: s.cost, color: _panelAccent),
            ),
            (
              label: tr.t('home.tokens'),
              value: formatNumber(today.tokens),
              spark: _Spark(values: s.tokens, color: _panelAux),
            ),
            (
              label: tr.t('home.requests'),
              value: formatNumber(today.totalRequests),
              spark: _Spark(values: s.requests, color: _panelAux),
            ),
            (
              label: tr.t('home.cacheRate'),
              value: formatPercent(today.cacheRate),
              spark: _Spark(values: s.cache, color: _panelAux),
            ),
          ]
        : const <({String label, String value, Widget? spark})>[];

    // ③ 维度趋势（home-dim-trend，`Home.tsx:426-436`）：2026-09-27 由 24h 总量
    // 双线趋势位换成维度趋势（三维度 tab + tokens/cost 指标 tab，堆叠面积 240）。
    final trendSection = _HomeTrendChart(
      platformSeries: _seriesPlatforms,
      modelSeries: _seriesModels,
      groupSeries: _seriesGroups,
      dim: _trendDim,
      metric: _trendMetric,
      ungroupedLabel: tr.t('platform.ungrouped'),
      emptyHint: emptyText,
      onDim: (d) => setState(() => _trendDim = d),
      onMetric: (m) => setState(() => _trendMetric = m),
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHead(
          title: tr.t('page.home'),
          subtitle: tr.t('home.desc'),
          bottom: 16, // React 全页 gap 16（Home.tsx:265）
        ),
        // 琥珀渐变眉条（`Home.tsx:273`）。
        Container(
          height: 3,
          margin: const EdgeInsets.only(
            bottom: 16,
          ), // 同上：全页 gap 16（Home.tsx:265）
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(2),
            gradient: const LinearGradient(
              colors: [Color(0xFF64521D), Color(0xFFE8C547), Color(0xFFF2DC8A)],
              stops: [0, 0.55, 1],
            ),
          ),
        ),
        // 命令面板单块（`Home.tsx:276-284`）：五个区块在同一块里，
        // **固定深色**不跟随主题 —— 它模仿的是命令行工具的样子，
        // 浅色主题下也保持深色（React 侧同样是写死的 PANEL 常量）。
        // 原先这里是 Bento 栅格里一堆各自独立、跟随主题的 Tile。
        Container(
          decoration: BoxDecoration(
            color: _panelS1,
            border: Border.all(color: _panelLine),
            borderRadius: BorderRadius.circular(14),
            boxShadow: const [
              BoxShadow(
                color: Color(0x99000000),
                blurRadius: 32,
                offset: Offset(0, 8),
              ),
            ],
          ),
          clipBehavior: Clip.antiAlias,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              // 五个区块入场错峰 0/70/140/210/280ms（`Home.tsx:219-223`）。
              // ① 搜索栏式状态行
              Reveal(child: _statusTile(t, tr)),
              const _PanelDivider(),
              // ② 四个 KPI 紧凑格，格间竖线
              Reveal(
                delayMs: 70,
                child: kpis.isEmpty
                    ? Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 16,
                          vertical: 14,
                        ),
                        child: Text(
                          emptyText,
                          // 空态 F.hint = 13（Home.tsx:329）
                          style: AidogType.caption.copyWith(
                            fontSize: 13,
                            color: _panelMuted,
                          ),
                        ),
                      )
                    : _KpiGrid(cells: kpis),
              ),
              const _PanelDivider(),
              // ③ 维度趋势（`Home.tsx:426-436`）：整宽一行，reveal 140。
              Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 14,
                ),
                child: Reveal(delayMs: 140, child: trendSection),
              ),
              const _PanelDivider(),
              // ④ 三维度行列表（今日窗口，`Home.tsx:438-494`）：platform / model /
              // group 三面板同构。React 是 `repeat(auto-fit, minmax(420px, 1fr))`
              // （`Home.tsx:440`）—— 塞得下几栏就并排（1px 线分隔），塞不下叠成一
              // 栏（横线分隔）。reveal 210。
              Reveal(
                delayMs: 210,
                child: _DimSection(
                  platformRows: _dimPlatforms,
                  modelRows: _dimModels,
                  groupRows: _dimGroups,
                  platformSparks: _todaySeriesPlatforms,
                  modelSparks: _todaySeriesModels,
                  groupSparks: _todaySeriesGroups,
                  platforms: _platforms,
                  loading: _loading,
                  emptyText: emptyText,
                  onNavigate: widget.onNavigate,
                ),
              ),
              const _PanelDivider(),
              // ⑤+⑥ 总余额行与快捷键 footer 同属 280ms 那一块（`Home.tsx:460`）。
              Reveal(
                delayMs: 280,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (balance > 0) ...[
                      Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 16,
                          vertical: 12,
                        ),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.baseline,
                          textBaseline: TextBaseline.alphabetic,
                          children: [
                            Expanded(
                              child: Text(
                                tr.t('home.totalBalance'),
                                // F.small = 12（Home.tsx:470）
                                style: AidogType.caption.copyWith(
                                  fontSize: 12,
                                  color: _panelMuted,
                                ),
                              ),
                            ),
                            Ltr(
                              child: Text(
                                formatCostUsd(balance),
                                style: _panelMono(
                                  17,
                                  _panelFg,
                                  w: FontWeight.w700,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                      const _PanelDivider(),
                    ],
                    Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 14,
                        vertical: 12,
                      ),
                      child: Wrap(
                        spacing: 8, // Home.tsx:480 gap 8
                        runSpacing: 8,
                        children: [
                          for (final c in _chips(tr))
                            _Chip(label: c.label, kbd: c.kbd, onTap: c.run),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }

  /// ① 搜索栏式状态行（`Home.tsx:285-315`）：运行态点 + 端口 + ⌘C 复制地址。
  Widget _statusTile(AidogTheme t, I18nController tr) {
    final running = _running;
    final statusText = running == null
        ? tr.t('home.statusUnknown')
        : running
        ? tr.t('home.statusRunning')
        : tr.t('home.statusStopped');
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
      child: Row(
        children: [
          Container(
            width: 10,
            height: 10,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: (running ?? false) ? t.c.ok : _panelMuted,
              // 只有活着的东西才发光。
              boxShadow: (running ?? false)
                  ? [
                      BoxShadow(
                        color: t.c.ok.withValues(alpha: 0.14),
                        blurRadius: 0,
                        spreadRadius: 4,
                      ),
                    ]
                  : const [],
            ),
          ),
          const SizedBox(width: AidogSpace.smd),
          Expanded(
            child: Ltr(
              child: Text(
                '${tr.t('home.proxyStatus')} $statusText · '
                '${tr.t('home.port')} $_port',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.body.copyWith(
                  color: _panelFg,
                  fontWeight: FontWeight.w600, // Home.tsx:301
                  letterSpacing: 0, // React 默认 normal（Home.tsx:301）
                ),
              ),
            ),
          ),
          const SizedBox(width: AidogSpace.smd),
          Tooltip(
            message: tr.t('home.copyBaseUrlTitle'),
            child: _Chip(
              label: _copied ? '✓' : tr.t('home.copyBaseUrl'),
              kbd: '⌘C',
              // 状态行这颗键帽排在文案**前**（`Home.tsx:311-312`），
              // 与 footer 的「文案 + 键帽」（`:483`）顺序相反。
              kbdFirst: true,
              onTap: _copyUrl,
            ),
          ),
        ],
      ),
    );
  }
}

// ── 命令面板的固定配色（`Home.tsx:32-41` 的 PANEL 常量）─────────────
//
// **写死不跟随主题**：这一块模仿的是命令行工具的样子，浅色主题下也保持深色。
// React 侧同样是常量，不取 CSS 变量。
const Color _panelFg = Color(0xFFF5F5F0);
const Color _panelMuted = Color(0xFF8A8580);
const Color _panelS1 = Color(0xFF0E0E0E);
const Color _panelS2 = Color(0xFF151514);
const Color _panelLine = Color(0x12FFFFFF); // rgba(255,255,255,.07)
const Color _panelAccent = Color(0xFFE8C547);

/// 辅线灰（React `seriesColor(1)` = `--chart-2`，明暗同值）：KPI 灰阶 sparkline、
/// 趋势 cost 虚线用它，与琥珀主线区分（Home.tsx:104/363）。
const Color _panelAux = Color(0xFF737373);

/// 迷你环 track：白 .14（Home.tsx:118），与面板 hairline（.07）分开。
const Color _panelTrack = Color(0x24FFFFFF);

/// 面板内等宽数字（`PANEL.mono` 的 Flutter 侧）：字号逐消费点对齐 React 裸值，
/// 不走 AidogType 字阶 —— 命令面板是固定深色，字号也按面板自己的体系。
TextStyle _panelMono(
  double size,
  Color color, {
  FontWeight w = FontWeight.w400,
  double tracking = 0,
}) => TextStyle(
  fontFamily: AidogType.familyMono,
  fontFamilyFallback: AidogType.familyMonoFallback,
  fontSize: size,
  fontWeight: w,
  letterSpacing: tracking * size,
  height: 1.35,
  color: color,
  fontFeatures: const [FontFeature.tabularFigures()],
);

/// 面板内的一条横分隔线。
class _PanelDivider extends StatelessWidget {
  const _PanelDivider();

  @override
  Widget build(BuildContext context) =>
      const Divider(height: 1, thickness: 1, color: _panelLine);
}

/// KPI 行的响应式栅格：React 是 `repeat(auto-fit, minmax(140px, 1fr))`
/// （`Home.tsx:320`）—— 窄窗自动降成 3 / 2 / 1 列，不是恒四列压扁到读不出数。
class _KpiGrid extends StatelessWidget {
  const _KpiGrid({required this.cells});

  final List<({String label, String value, Widget? spark})> cells;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, c) {
      final cols = (c.maxWidth / 140).floor().clamp(1, cells.length);
      // 向下取整到 0.01：浮点余数会让最后一格挤不进同一行。
      final w = (c.maxWidth / cols * 100).floorToDouble() / 100;
      return Wrap(
        children: [
          for (var i = 0; i < cells.length; i++)
            Container(
              width: w,
              // 格间竖线：React 给除末格外的每格画 borderInlineEnd（`Home.tsx:324`）。
              // 走 foregroundDecoration，这条边不占布局宽度。
              foregroundDecoration: i == cells.length - 1
                  ? null
                  : const BoxDecoration(
                      border: BorderDirectional(
                        end: BorderSide(color: _panelLine),
                      ),
                    ),
              child: _KpiCell(
                label: cells[i].label,
                value: cells[i].value,
                spark: cells[i].spark,
              ),
            ),
        ],
      );
    },
  );
}

/// 面板里的一格 KPI：标签 + 大数 + 行内 sparkline。
class _KpiCell extends StatelessWidget {
  const _KpiCell({required this.label, required this.value, this.spark});

  final String label;
  final String value;
  final Widget? spark;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          label,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: _panelMono(10, _panelMuted, tracking: 0.1), // Home.tsx:92
        ),
        const SizedBox(height: 3), // Home.tsx:100
        Ltr(
          child: Text(
            value,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: _panelMono(
              24,
              _panelFg,
              w: FontWeight.w700,
            ), // Home.tsx:98-99
          ),
        ),
        if (spark case final sp?) ...[const SizedBox(height: 8), sp],
      ],
    ),
  );
}

/// 面板里的一个区块：小标题 + 右侧等宽 meta + 正文。
class _PanelSection extends StatelessWidget {
  const _PanelSection({
    required this.title,
    required this.meta,
    required this.child,
  });

  final String title;
  final String meta;
  final Widget child;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        // React 是 `flexWrap: wrap` 的 space-between 行（`Home.tsx:351,407`）：
        // 两段塞不下时 meta 落到下一行，不挤扁标题。
        Wrap(
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.end,
          spacing: 12, // Home.tsx:351,407 gap 12
          children: [
            Text(
              title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AidogType.label.copyWith(
                fontSize: 13, // F.small + 1（Home.tsx:352,408）
                color: _panelFg,
                fontWeight: FontWeight.w700,
              ),
            ),
            Ltr(
              child: Text(
                meta,
                style: _panelMono(10, _panelMuted), // Home.tsx:353
              ),
            ),
          ],
        ),
        const SizedBox(height: 4),
        child,
      ],
    ),
  );
}

/// KPI 格行内 sparkline：走 [normPoints] 的 min-max 归一化（与 React 版同一函数）。
/// 少于两点不画（对齐 React 的 `values.length < 2 → null`）。
/// [width] null = 占满父格（KPI 用）；维度行尾传定宽 72（`Home.tsx:647`）。
class _Spark extends StatelessWidget {
  const _Spark({required this.values, required this.color, this.width});

  final List<double> values;
  final Color color;
  final double? width;

  @override
  Widget build(BuildContext context) {
    // 无 child 的 CustomPaint 在 min Column 里拿无界约束 → 0×0 完全不可见
    // （React Sparkline 固定高 22，Home.tsx:67-84）。槽位必须给死高度。
    if (values.length < 2) return const SizedBox(height: 22);
    return SizedBox(
      height: 22,
      width: width ?? double.infinity,
      child: CustomPaint(
        painter: _SparkPainter(values: values, color: color),
      ),
    );
  }
}

class _SparkPainter extends CustomPainter {
  const _SparkPainter({required this.values, required this.color});

  final List<double> values;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final pts = normPoints(values, size.width, size.height);
    if (pts.length < 2) return;
    final path = Path()..moveTo(pts.first.$1, pts.first.$2);
    for (final p in pts.skip(1)) {
      path.lineTo(p.$1, p.$2);
    }
    canvas.drawPath(
      path,
      Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5
        ..strokeJoin = StrokeJoin.round
        ..strokeCap = StrokeCap.round,
    );
  }

  @override
  bool shouldRepaint(_SparkPainter old) =>
      old.color != color || !_sameList(old.values, values);
}

bool _sameList(List<double> a, List<double> b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

/// 面板底部快捷键 chip（`Home.tsx:483`）。
class _Chip extends StatelessWidget {
  const _Chip({
    required this.label,
    required this.kbd,
    required this.onTap,
    this.kbdFirst = false,
  });

  final String label;
  final String kbd;
  final VoidCallback onTap;

  /// 键帽排在文案前。状态行那颗是这个顺序（`Home.tsx:311-312`），
  /// footer 四颗是反的（`:483`）。
  final bool kbdFirst;

  List<Widget> _parts(Widget text, Widget kb) => kbdFirst
      ? [kb, const SizedBox(width: 7), text]
      : [text, const SizedBox(width: 7), kb];

  @override
  Widget build(BuildContext context) {
    // globals.css `.cmd-kb`：固定深色 s2 底，不跟主题（面板内 chip 浅色模式也保持深）。
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 7),
        decoration: BoxDecoration(
          color: _panelS2,
          border: Border.all(color: _panelLine),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final w in _parts(
              Text(
                label,
                style: const TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: _panelFg,
                ),
              ),
              // 键帽：mono 10、白 .14 边、radius 4、padding 1/5（`.cmd-kb .k`）。
              Ltr(
                child: Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 5,
                    vertical: 1,
                  ),
                  decoration: BoxDecoration(
                    border: Border.all(color: _panelTrack),
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: Text(kbd, style: _panelMono(10, _panelMuted)),
                ),
              ),
            ))
              w,
          ],
        ),
      ),
    );
  }
}

// ── 维度趋势图（home-dim-trend spec，React `HomeTrendChart.tsx`）──────
// 总计 / 按平台 / 按模型 / 按分组 24h 堆叠面积。数据源 = load() 里 batch 前三条 24h
// series_by 查询；窗口恒 24h；默认维度 total（2026-10-03 拍板，React 对齐）、默认指标
// tokens（2026-09-27 用户拍板）。面板是固定深色，图与文字都走 PANEL 显式色（图表组件读主题 token，
// 这里用 Theme override 压成深色 token，浅色主题下轴/图例仍可读）。

int _bucketTokens(StatsBucket b) =>
    b.inputTokens + b.outputTokens + b.cacheTokens;

int _seriesTokens(StatsSeries s) =>
    s.buckets.fold(0, (sum, b) => sum + _bucketTokens(b));

/// 多条维度序列合并成一条（「其它」层）：同桶逐字段求和，比率与均值不可加置 0。
StatsSeries _mergeSeries(List<StatsSeries> rest) {
  final m = <String, StatsBucket>{};
  for (final s in rest) {
    for (final b in s.buckets) {
      final cur = m[b.timeBucket];
      m[b.timeBucket] = cur == null
          ? b
          : StatsBucket(
              timeBucket: b.timeBucket,
              totalRequests: cur.totalRequests + b.totalRequests,
              successCount: cur.successCount + b.successCount,
              errorCount: cur.errorCount + b.errorCount,
              inputTokens: cur.inputTokens + b.inputTokens,
              outputTokens: cur.outputTokens + b.outputTokens,
              cacheTokens: cur.cacheTokens + b.cacheTokens,
              avgDurationMs: 0,
              totalCost: cur.totalCost + b.totalCost,
            );
    }
  }
  final keys = m.keys.toList()..sort();
  return StatsSeries(name: '', buckets: [for (final k in keys) m[k]!]);
}

/// 维度趋势图的一层（React `DimTrendData` 的 Flutter 侧）：label + 逐桶指标值。
class _DimTrendLayer {
  const _DimTrendLayer({required this.label, required this.values, required this.xs});
  final String label;

  /// 与 [xs] 对齐的逐桶指标值（tokens / cost 二选一，由 metric 决定）。
  final List<double> values;
  final List<double> xs;
}

/// 维度小时序列 → Top8 + 「其它」堆叠层（tokens 降序，与 _buildDimRows 口径一致，
/// React `buildDimTrend`）。[ungroupedLabel] 仅分组维度传（空名归一）。
List<_DimTrendLayer> _buildDimTrend(
  List<StatsSeries> series,
  String metric,
  String otherLabel, [
  String? ungroupedLabel,
]) {
  final sorted = [...series]..sort((a, b) => _seriesTokens(b) - _seriesTokens(a));
  final kept = sorted.take(_dimTopN).toList();
  final rest = sorted.skip(_dimTopN).toList();
  final layers = [...kept, if (rest.isNotEmpty) _mergeSeries(rest)];

  double val(StatsBucket b) =>
      metric == 'cost' ? b.totalCost : _bucketTokens(b).toDouble();
  String labelOf(StatsSeries s, bool isOther) => isOther
      ? otherLabel
      : (s.name.isEmpty ? (ungroupedLabel ?? s.name) : s.name);

  final xsByLayer = <List<double>>[];
  final valsByLayer = <List<double>>[];
  for (final s in layers) {
    xsByLayer.add([
      for (final b in s.buckets) bucketMs(b.timeBucket).toDouble(),
    ]);
    valsByLayer.add([for (final b in s.buckets) val(b)]);
  }
  return [
    for (var i = 0; i < layers.length; i++)
      _DimTrendLayer(
        label: labelOf(layers[i], i >= kept.length),
        values: valsByLayer[i],
        xs: xsByLayer[i],
      ),
  ];
}

/// 今日维度序列 → 行迷你曲线数据（React `buildSparkMap`）：
/// key = 维度名（分组空名归 [ungroupedLabel]），值 = 逐桶 token。
Map<String, List<double>> _buildSparkMap(
  List<StatsSeries> series, [
  String? ungroupedLabel,
]) => {
  for (final s in series)
    (s.name.isEmpty ? (ungroupedLabel ?? s.name) : s.name): [
      for (final b in s.buckets) _bucketTokens(b).toDouble(),
    ],
};

class _HomeTrendChart extends StatelessWidget {
  const _HomeTrendChart({
    required this.platformSeries,
    required this.modelSeries,
    required this.groupSeries,
    required this.dim,
    required this.metric,
    required this.ungroupedLabel,
    required this.emptyHint,
    required this.onDim,
    required this.onMetric,
  });

  final List<StatsSeries> platformSeries;
  final List<StatsSeries> modelSeries;
  final List<StatsSeries> groupSeries;

  /// total / platform / model / group（默认 total，2026-10-03 与 React 对齐）。状态在父级，本组件无本地态。
  final String dim;

  /// tokens / cost（默认 tokens）。
  final String metric;
  final String ungroupedLabel;
  final String emptyHint;
  final ValueChanged<String> onDim;
  final ValueChanged<String> onMetric;



  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    // 总计 = 平台序列全合并（平台维度是请求的完整划分，跨维度恒同一条总曲线），
    // 与 React totalSeries 同构（HomeTrendChart.tsx:143-145）。
    // 总计 = 平台序列全合并单层（平台维度是请求的完整划分，跨维度恒同一条总曲线），
    // 与 React totalSeries 同构（HomeTrendChart.tsx:143-145）。
    final List<StatsSeries> series = dim == 'total'
        ? <StatsSeries>[
            StatsSeries(
              name: t.t('home.tabTotal'),
              buckets: _mergeSeries(platformSeries).buckets,
            ),
          ]
        : dim == 'model'
        ? modelSeries
        : dim == 'group'
        ? groupSeries
        : platformSeries;
    // 全时段请求合计（各维度序列 total_requests 之和）。React 把请求走势画成右轴
    // 线（rightConfig），Flutter 堆叠图无右轴，这里以图例尾合计承接同一信息。
    final totalRequests = series.fold<int>(
      0,
      (n, s) => s.buckets.fold(n, (m, b) => m + b.totalRequests),
    );
    final layers = _buildDimTrend(
      series,
      metric,
      t.t('home.dimOther'),
      dim == 'group' ? ungroupedLabel : null,
    );
    final palette = ChartPalette.of(context);

    // 面板是固定深色：把图表组件读到的主题 token 压成深色版（React 的
    // `textColor={PANEL.muted}` 同一件事，Flutter 图表没有 textColor 入口）。
    final theme = Theme.of(context);
    final chart = Theme(
      data: theme.copyWith(
        extensions: <ThemeExtension<dynamic>>[
          AidogTheme.forMode(AidogMode.dark),
        ],
      ),
      child: AidogStackedAreaChart(
        series: [
          for (var i = 0; i < layers.length; i++)
            ChartSeries(
              key: 's$i',
              label: layers[i].label,
              color: palette.series(i),
              // 稀疏宽表：某层在某个桶没有值 → 该点 missing（断线语义，堆叠仍按 0 算，
              // 与 stats.dart 趋势图同一条注释的口径）。先按 x 全集对齐。
              points: [
                for (var r = 0; r < layers[i].xs.length; r++)
                  ChartPoint(layers[i].xs[r], layers[i].values[r]),
              ],
              format: metric == 'cost' ? formatCostUsd : formatNumber,
            ),
        ],
        // 空态文案尊重 loading：加载中留白，落空才显「今日暂无请求」
        //（React `emptyHint={loading ? "" : t(...)}`，HomeTrendChart.tsx:168）。
        emptyText: emptyHint,
      ),
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // 头行：标题左、维度 tab + 指标 tab 右（HomeTrendChart.tsx:123-147）。
        Wrap(
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: 12,
          runSpacing: 8,
          children: [
            Text(
              t.t('home.dimTrendTitle'),
              style: AidogType.label.copyWith(
                fontSize: 13,
                color: _panelFg,
                fontWeight: FontWeight.w700,
              ),
            ),
            Wrap(
              spacing: 10,
              runSpacing: 8,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Semantics(
                  label: t.t('home.dimTrendTitle'),
                  container: true,
                  child: Wrap(
                    spacing: 4,
                    children: [
                      for (final d in const [
                        ('total', 'home.tabTotal'),
                        ('platform', 'home.tabPlatform'),
                        ('model', 'home.tabModel'),
                        ('group', 'home.tabGroup'),
                      ])
                        _TrendTabButton(
                          label: t.t(d.$2),
                          active: dim == d.$1,
                          onTap: () => onDim(d.$1),
                        ),
                    ],
                  ),
                ),
                Semantics(
                  label: t.t('home.dimMetric'),
                  container: true,
                  child: Wrap(
                    spacing: 4,
                    children: [
                      for (final m in const [
                        ('tokens', 'home.tokens'),
                        ('cost', 'home.trendCost'),
                      ])
                        _TrendTabButton(
                          label: t.t(m.$2),
                          active: metric == m.$1,
                          onTap: () => onMetric(m.$1),
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ],
        ),
        const SizedBox(height: 8), // HomeTrendChart.tsx:118 gap 8
        SizedBox(height: 240, child: chart), // React height 240（:167）
        // 图例：公共 StackedAreaChart 自带图例，Flutter 侧的 AidogStackedAreaChart
        // 不带，这里补一行（色点 + label，面板色）。
        const SizedBox(height: 8),
        Wrap(
          spacing: 12,
          runSpacing: 4,
          children: [
            for (var i = 0; i < layers.length; i++)
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    width: 8,
                    height: 8,
                    margin: const EdgeInsets.only(right: 5),
                    decoration: BoxDecoration(
                      color: palette.series(i),
                      shape: BoxShape.circle,
                    ),
                  ),
                  Text(
                    layers[i].label,
                    style: AidogType.caption.copyWith(
                      fontSize: 11,
                      color: _panelMuted,
                    ),
                  ),
                ],
              ),
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  '${t.t('home.trendRequests')} · ${formatNumber(totalRequests)}',
                  style: _panelMono(11, _panelFg),
                ),
              ],
            ),
          ],
        ),
      ],
    );
  }
}

/// 面板内 tab 小按钮（React `tabButton`，HomeTrendChart.tsx:105-115）：11 号字、
/// 3/10 内衬、radius 6；激活 = 琥珀描边 .4 + 琥珀 12% 底 + 琥珀字，未激活 = 面板
/// line 描边 + muted 字。不走主题 CSS 变量（面板是硬编码深色面）。
class _TrendTabButton extends StatelessWidget {
  const _TrendTabButton({
    required this.label,
    required this.active,
    required this.onTap,
  });

  final String label;
  final bool active;
  final VoidCallback onTap;

  static const Color _amber = Color(0xFFE8C547);

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(6),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 3),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(6),
          border: Border.all(
            color: active ? const Color(0x66E8C547) : _panelLine,
          ),
          color: active ? const Color(0x1FE8C547) : null,
        ),
        child: Text(
          label,
          style: AidogType.caption.copyWith(
            fontSize: 11,
            color: active ? _amber : _panelMuted,
          ),
        ),
      ),
    );
  }
}

// ── 三维度行列表（platform / model / group 同构，今日窗口）─────────────
// 行结构：logo（仅平台维度）｜名称（弹性省略）｜tokens 占比条｜tokens｜cost｜
// 请求数｜成功率｜缓存率｜延迟｜今日迷你走势。「其它」行灰显不可点；
// 缓存率/延迟分母不可加，合并行显 --。

const int _dimTopN = 8;

int _dimTokens(DimensionEntry d) =>
    d.inputTokens + d.outputTokens + d.cacheTokens;

class _DimRowData {
  const _DimRowData({
    required this.name,
    required this.d,
    required this.unclickable,
    this.other = false,
  });
  final String name;
  final DimensionEntry d;

  /// 「其它」合并行（label 取 home.dimOther，缓存率/延迟显 --）。
  final bool other;

  /// 「其它」与「未分组平台」都不可点（点了没有唯一下钻目标）。
  final bool unclickable;
}

/// tokens 降序前 [_dimTopN]，其余合并成一条「其它」行（React `buildDimRows`）。
///
/// [ungroupedLabel] 分组维度传入「未分组平台」：空 group_key 是真实语义，标出
/// 且不可点下钻（groupKey='' 到统计页等于不带筛选）。模型维度不传：空 model
/// 行已被 Rust 过滤，这里再滤一道防旧内核。
({List<_DimRowData> rows, int total}) _buildDimRows(
  List<DimensionEntry> data, [
  String? ungroupedLabel,
]) {
  final named = ungroupedLabel != null
      ? [
          for (final d in data)
            if (d.name.isEmpty) d.withName(ungroupedLabel) else d,
        ]
      : [
          for (final d in data)
            if (d.name.isNotEmpty) d,
        ];
  final sorted = [...named]..sort((a, b) => _dimTokens(b) - _dimTokens(a));
  final top = sorted.take(_dimTopN).toList();
  final rest = sorted.skip(_dimTopN).toList();
  final rows = [
    for (final d in top)
      _DimRowData(
        name: d.name,
        d: d,
        // 未分组行显示自己的名字（ungroupedLabel），只是不可点。
        unclickable: ungroupedLabel != null && d.name == ungroupedLabel,
      ),
  ];
  if (rest.isNotEmpty) {
    rows.add(
      _DimRowData(
        name: '',
        other: true,
        unclickable: true,
        d: DimensionEntry(
          name: '',
          totalRequests: rest.fold(0, (a, d) => a + d.totalRequests),
          successCount: rest.fold(0, (a, d) => a + d.successCount),
          inputTokens: rest.fold(0, (a, d) => a + d.inputTokens),
          outputTokens: rest.fold(0, (a, d) => a + d.outputTokens),
          cacheTokens: rest.fold(0, (a, d) => a + d.cacheTokens),
          cacheRate: 0, // 分母不可加，合并行不展示（显 --）
          avgDurationMs: 0,
          totalCost: rest.fold(0.0, (a, d) => a + d.totalCost),
        ),
      ),
    );
  }
  return (rows: rows, total: sorted.fold(0, (s, d) => s + _dimTokens(d)));
}

/// 三维度行区（React `Home.tsx:438-494`）：platform / model / group 三面板同构，
/// `repeat(auto-fit, minmax(420px, 1fr))` 的 Flutter 侧 = 按可用宽分 1/2/3 栏，
/// 面板间 1px 线分隔（栏间竖线、行间横线）。
class _DimSection extends StatelessWidget {
  const _DimSection({
    required this.platformRows,
    required this.modelRows,
    required this.groupRows,
    required this.platformSparks,
    required this.modelSparks,
    required this.groupSparks,
    required this.platforms,
    required this.loading,
    required this.emptyText,
    required this.onNavigate,
  });

  final List<DimensionEntry> platformRows;
  final List<DimensionEntry> modelRows;
  final List<DimensionEntry> groupRows;
  final List<StatsSeries> platformSparks;
  final List<StatsSeries> modelSparks;
  final List<StatsSeries> groupSparks;

  /// 平台清单（行 logo 与下钻 id 用；维度名 = 平台显示名，后端回填）。
  final List<PlatformSummary> platforms;
  final bool loading;
  final String emptyText;
  final void Function(String id, [NavContext? context]) onNavigate;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final ungroupedLabel = t.t('platform.ungrouped');
    final platformByName = {
      for (final p in platforms) p.name: p,
    };
    // 行 logo（仅平台维度，`Home.tsx:292-296` 的 platformLogoOf）。
    String? logoOf(String name) => platformByName[name]?.platformType;

    final p = _buildDimRows(platformRows);
    final m = _buildDimRows(modelRows);
    final g = _buildDimRows(groupRows, ungroupedLabel);
    final panels = [
      _DimPanel(
        title: t.t('home.byPlatform'),
        rows: p.rows,
        totalTokens: p.total,
        sparks: _buildSparkMap(platformSparks),
        logoOf: logoOf,
        loading: loading,
        emptyText: emptyText,
        onRow: (name) {
          final plat = platformByName[name];
          onNavigate(
            'stats',
            plat == null
                ? NavContext(platformName: name)
                : NavContext(platformId: plat.id, platformName: plat.name),
          );
        },
      ),
      _DimPanel(
        title: t.t('home.byModel'),
        rows: m.rows,
        totalTokens: m.total,
        sparks: _buildSparkMap(modelSparks),
        loading: loading,
        emptyText: emptyText,
        onRow: (name) => onNavigate('stats', NavContext(model: name)),
      ),
      _DimPanel(
        title: t.t('home.byGroup'),
        rows: g.rows,
        totalTokens: g.total,
        sparks: _buildSparkMap(groupSparks, ungroupedLabel),
        loading: loading,
        emptyText: emptyText,
        onRow: (name) => onNavigate('stats', NavContext(groupKey: name)),
      ),
    ];

    return LayoutBuilder(
      builder: (context, c) {
        // `repeat(auto-fit, minmax(420px, 1fr))`：每栏至少 420，栏间 1px。
        final cols = ((c.maxWidth + 1) / 421).floor().clamp(1, 3);
        if (cols == 1) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              for (var i = 0; i < panels.length; i++) ...[
                if (i > 0) const _PanelDivider(),
                panels[i],
              ],
            ],
          );
        }
        return IntrinsicHeight(
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (var i = 0; i < panels.length; i++) ...[
                if (i > 0)
                  const VerticalDivider(width: 1, thickness: 1, color: _panelLine),
                Expanded(child: panels[i]),
              ],
            ],
          ),
        );
      },
    );
  }
}

class _DimPanel extends StatelessWidget {
  const _DimPanel({
    required this.title,
    required this.rows,
    required this.totalTokens,
    required this.sparks,
    required this.loading,
    required this.emptyText,
    required this.onRow,
    this.logoOf,
  });

  final String title;
  final List<_DimRowData> rows;
  final int totalTokens;

  /// 行名 → 今日逐桶 token 序列（缺名不画，React `DimPanel` 的 sparks prop）。
  final Map<String, List<double>> sparks;
  final bool loading;
  final String emptyText;

  /// 行名 → 协议（仅平台维度传）：命中画 ProtocolLogo。
  final String? Function(String name)? logoOf;
  final void Function(String name) onRow;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    if (rows.isEmpty) {
      // 空态与 KPI 区同款：加载中不显字，落空显「今日暂无请求」。
      return _PanelSection(
        title: title,
        meta: 'TODAY · ${t.t('home.dimMetric')}',
        child: Text(
          emptyText,
          style: AidogType.caption.copyWith(fontSize: 11, color: _panelMuted),
        ),
      );
    }
    return _PanelSection(
      title: title,
      meta: 'TODAY · ${t.t('home.dimMetric')}',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var i = 0; i < rows.length; i++)
            _dimRowWidget(t, rows[i], i == rows.length - 1),
        ],
      ),
    );
  }

  Widget _dimRowWidget(I18nController t, _DimRowData r, bool last) {
    final label = r.other ? t.t('home.dimOther') : r.name;
    final share = totalTokens > 0 ? _dimTokens(r.d) / totalTokens : 0.0;
    final success = r.d.totalRequests > 0
        ? formatPercent(r.d.successCount / r.d.totalRequests * 100, 0)
        : '--';
    final cache = r.other ? '--' : formatPercent(r.d.cacheRate, 0);
    // 「其它」行的均值不可加；零请求的行没有延迟可言（Home.tsx:640）。
    final duration = r.other || r.d.totalRequests == 0
        ? '--'
        : formatDurationMs(r.d.avgDurationMs);
    final proto = r.other ? null : logoOf?.call(r.name);
    final spark = r.other ? const <double>[] : (sparks[r.name] ?? const []);
    final row = Row(
      children: [
        if (proto != null) ...[ProtocolLogo(protocol: proto, size: 18), const SizedBox(width: 12)],
        Expanded(
          flex: 3,
          child: Tooltip(
            message: label,
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AidogType.label.copyWith(
                fontSize: 13,
                color: r.unclickable ? _panelMuted : _panelFg,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
        ),
        // 条长 = 该行 tokens 占全量比例（不是相对 top1，占比和为 100%）。
        Expanded(
          flex: 4,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(2),
              child: TweenAnimationBuilder<double>(
                duration: const Duration(milliseconds: 300),
                tween: Tween(begin: 0, end: share),
                builder: (context, v, _) => LinearProgressIndicator(
                  value: v,
                  minHeight: 3,
                  backgroundColor: const Color(0x1EE8C547),
                  valueColor: AlwaysStoppedAnimation(
                    r.unclickable ? _panelMuted : const Color(0xFFE8C547),
                  ),
                ),
              ),
            ),
          ),
        ),
        _dimNum(formatNumber(_dimTokens(r.d)), 64, muted: true),
        _dimNum(formatCostUsd(r.d.totalCost), 56, amber: true),
        _dimNum(formatNumber(r.d.totalRequests), 44, muted: true),
        _dimNum(success, 42, muted: true),
        _dimNum(cache, 42, muted: true),
        _dimNum(duration, 46, muted: true),
        // 行尾今日 token 迷你走势（跟随面板窗口），灰阶不与占比条抢焦点。
        _Spark(values: spark, color: _panelAux, width: 72),
      ],
    );
    final body = Padding(
      padding: const EdgeInsets.symmetric(vertical: 9),
      child: row,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (r.unclickable)
          // 「其它」/「未分组平台」行灰显不可点（点了没有唯一下钻目标）。
          Opacity(opacity: 0.55, child: body)
        else
          InkWell(onTap: () => onRow(r.name), child: body),
        if (!last)
          const Divider(height: 1, thickness: 1, color: Color(0x0DFFFFFF)),
      ],
    );
  }

  static Widget _dimNum(
    String s,
    double w, {
    bool muted = false,
    bool amber = false,
  }) {
    return SizedBox(
      width: w,
      child: Ltr(
        child: Text(
          s,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          textAlign: TextAlign.end,
          style: _panelMono(11, amber ? const Color(0xFFE8C547) : _panelMuted),
        ),
      ),
    );
  }
}
