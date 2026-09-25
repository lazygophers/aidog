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
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'home_logic.dart';
import 'invoke.dart';
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

  /// 侧栏切页（footer chip 与 ⌘N/⌘S/⌘L 用）。
  final void Function(String id) onNavigate;
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
  List<TodayPlatformStat> _platformsToday = const [];
  List<PlatformSummary> _platforms = const [];
  List<StatsBucket> _trend = const [];
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
    List<TodayPlatformStat> platformsToday = _platformsToday;
    List<PlatformSummary> platforms = _platforms;
    List<StatsBucket> trend = _trend;
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
        final v = await widget.invoke('popover_platform_today');
        platformsToday = [
          for (final e in v! as List)
            TodayPlatformStat.fromJson(e as Map<String, dynamic>),
        ];
      }, () => platformsToday = const []),
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
    ]);
    _inFlight = false;
    if (!mounted) return;
    setState(() {
      _running = running;
      _port = port;
      _today = today;
      _platformsToday = platformsToday;
      _platforms = platforms;
      _trend = trend;
      _loading = false;
    });
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
    final top = topPlatformsOf(_platformsToday);
    final maxCost = top.fold<double>(0, (m, p) => p.cost > m ? p.cost : m);
    final costSum = top.fold<double>(0, (acc, p) => acc + p.cost);
    final balance = totalBalanceOf(_platforms);
    final peak = trendPeakOf(_trend);
    final trendOk = hasTrend(_trend);
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHead(title: tr.t('page.home'), subtitle: tr.t('home.desc')),
        // 琥珀渐变眉条（`Home.tsx:273`）。
        Container(
          height: 3,
          margin: const EdgeInsets.only(bottom: AidogSpace.ssm),
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
                          style: AidogType.caption.copyWith(color: _panelMuted),
                        ),
                      )
                    : IntrinsicHeight(
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            for (var i = 0; i < kpis.length; i++) ...[
                              Expanded(
                                child: _KpiCell(
                                  label: kpis[i].label,
                                  value: kpis[i].value,
                                  spark: kpis[i].spark,
                                ),
                              ),
                              if (i < kpis.length - 1)
                                const VerticalDivider(
                                  width: 1,
                                  thickness: 1,
                                  color: _panelLine,
                                ),
                            ],
                          ],
                        ),
                      ),
              ),
              const _PanelDivider(),
              // ③+④ 趋势与平台并排，中间一条竖线
              Reveal(
                delayMs: 140,
                child: IntrinsicHeight(
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Expanded(
                        child: _PanelSection(
                          title: tr.t('home.trend24h'),
                          meta: trendOk
                              ? 'HOURLY · ${tr.t('home.trendPeak')} '
                                    '${formatNumber(peak)}'
                              : 'HOURLY',
                          child: trendOk
                              ? SizedBox(
                                  height: 88 + 12, // 图 88（Home.tsx:364）+ 小时轴行 12（:373）
                                  child: Column(
                                    children: [
                                      SizedBox(
                                        height: 88,
                                        child: AidogLineChart(
                                          mini: true,
                                          area: true,
                                          series: [
                                            ChartSeries(
                                              key: 'req',
                                              label: tr.t('home.trendRequests'),
                                              color: _panelAccent,
                                              points: [
                                                for (final b in _trend)
                                                  ChartPoint(
                                                    bucketMs(b.timeBucket),
                                                    b.totalRequests.toDouble(),
                                                  ),
                                              ],
                                              format: formatNumber,
                                            ),
                                            ChartSeries(
                                              key: 'cost',
                                              label: tr.t('home.trendCost'),
                                              color: _panelAux,
                                              points: [
                                                for (final b in _trend)
                                                  ChartPoint(
                                                    bucketMs(b.timeBucket),
                                                    b.totalCost,
                                                  ),
                                              ],
                                              format: formatCostUsd,
                                              dashed: true,
                                              rightAxis: true,
                                            ),
                                          ],
                                        ),
                                      ),
                                      SizedBox(
                                        height: 12,
                                        child: _HourAxis(buckets: _trend),
                                      ),
                                    ],
                                  ),
                                )
                              : Text(
                                  emptyText,
                                  style: AidogType.caption.copyWith(
                                    color: _panelMuted,
                                  ),
                                ),
                        ),
                      ),
                      const VerticalDivider(
                        width: 1,
                        thickness: 1,
                        color: _panelLine,
                      ),
                      Expanded(
                        child: _PanelSection(
                          title: tr.t('home.topPlatforms'),
                          meta:
                              'TOP $kTopPlatforms · ${tr.t('home.trendCost')}',
                          child: top.isEmpty
                              ? Text(
                                  emptyText,
                                  style: AidogType.caption.copyWith(
                                    color: _panelMuted,
                                  ),
                                )
                              : Column(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    for (var i = 0; i < top.length; i++) ...[
                                      if (i > 0)
                                        const Divider(
                                          height: 1,
                                          thickness: 1,
                                          color: Color(0x0DFFFFFF), // rgba(255,255,255,.05) 行分隔线（Home.tsx:420）
                                        ),
                                      Padding(
                                        padding: const EdgeInsets.symmetric(
                                          vertical: 9, // Home.tsx:417
                                        ),
                                        child: _platformRow(
                                          top[i],
                                          maxCost,
                                          costSum,
                                        ),
                                      ),
                                    ],
                                  ],
                                ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              const _PanelDivider(),
              // ⑤ 总余额行
              if (balance > 0) ...[
                Reveal(
                  delayMs: 210,
                  child: Padding(
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
                            style: AidogType.caption.copyWith(
                              color: _panelMuted,
                            ),
                          ),
                        ),
                        Ltr(
                          child: Text(
                            formatCostUsd(balance),
                            style: _panelMono(17, _panelFg, w: FontWeight.w700),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                const _PanelDivider(),
              ],
              // ⑥ 快捷键 footer
              Reveal(
                delayMs: 280,
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 14,
                    vertical: 12,
                  ),
                  child: Wrap(
                    spacing: AidogSpace.ssm,
                    runSpacing: AidogSpace.ssm,
                    children: [
                      for (final c in _chips(tr))
                        _Chip(label: c.label, kbd: c.kbd, onTap: c.run),
                    ],
                  ),
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
              onTap: _copyUrl,
            ),
          ),
        ],
      ),
    );
  }

  /// ④ 平台 Top：迷你环形（花费占比）+ 行内占比条 + 等宽数字。
  Widget _platformRow(TodayPlatformStat p, double maxCost, double costSum) {
    return Row(
      children: [
        SizedBox(
          width: 26,
          height: 26,
          child: CustomPaint(
            painter: _RingPainter(
              share: costSum > 0 ? p.cost / costSum : 0,
              track: _panelLine,
              arc: _panelAccent,
            ),
          ),
        ),
        const SizedBox(width: AidogSpace.smd),
        Expanded(
          child: Text(
            p.platformName,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: AidogType.label.copyWith(
              color: _panelFg,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
        const SizedBox(width: AidogSpace.smd),
        Expanded(
          child: ClipRRect(
            borderRadius: BorderRadius.circular(2),
            child: LinearProgressIndicator(
              value: maxCost > 0 ? (p.cost / maxCost).clamp(0.0, 1.0) : 0,
              minHeight: 3,
              backgroundColor: _panelLine,
              valueColor: const AlwaysStoppedAnimation<Color>(_panelAccent),
            ),
          ),
        ),
        const SizedBox(width: AidogSpace.smd),
        Ltr(
          child: Text(
            '${formatNumber(p.requests)} · ${formatNumber(p.tokens)}',
            style: numStyle(AidogType.numSm, _panelMuted),
          ),
        ),
        const SizedBox(width: AidogSpace.smd),
        Ltr(
          child: Text(
            formatCostUsd(p.cost),
            style: numStyle(AidogType.numMd, _panelAccent),
          ),
        ),
      ],
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
            style: _panelMono(24, _panelFg, w: FontWeight.w700), // Home.tsx:98-99
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
        Row(
          crossAxisAlignment: CrossAxisAlignment.baseline,
          textBaseline: TextBaseline.alphabetic,
          children: [
            Expanded(
              child: Text(
                title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.label.copyWith(
                  color: _panelFg,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            const SizedBox(width: AidogSpace.ssm),
            Ltr(
              child: Text(
                meta,
                style: _panelMono(10, _panelMuted), // Home.tsx:353
              ),
            ),
          ],
        ),
        const SizedBox(height: AidogSpace.sxs),
        child,
      ],
    ),
  );
}

/// KPI 格行内 sparkline：走 [normPoints] 的 min-max 归一化（与 React 版同一函数）。
/// 少于两点不画（对齐 React 的 `values.length < 2 → null`）。
class _Spark extends StatelessWidget {
  const _Spark({required this.values, required this.color});

  final List<double> values;
  final Color color;

  @override
  Widget build(BuildContext context) {
    // 无 child 的 CustomPaint 在 min Column 里拿无界约束 → 0×0 完全不可见
    // （React Sparkline 固定高 22，Home.tsx:67-84）。槽位必须给死高度。
    if (values.length < 2) return const SizedBox(height: 22);
    return SizedBox(
      height: 22,
      width: double.infinity,
      child: CustomPaint(painter: _SparkPainter(values: values, color: color)),
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

/// 平台行迷你环形：琥珀弧 = 该平台花费 / Top4 合计。
class _RingPainter extends CustomPainter {
  const _RingPainter({
    required this.share,
    required this.track,
    required this.arc,
  });

  final double share;
  final Color track;
  final Color arc;

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    const r = 9.0; // Home.tsx:117
    final stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 5;
    canvas.drawCircle(center, r, stroke..color = track);
    final swept = share.clamp(0.0, 1.0) * 2 * 3.141592653589793;
    if (swept <= 0) return;
    canvas.drawArc(
      Rect.fromCircle(center: center, radius: r),
      -3.141592653589793 / 2,
      swept,
      false,
      stroke..color = arc,
    );
  }

  @override
  bool shouldRepaint(_RingPainter old) =>
      old.share != share || old.arc != arc || old.track != track;
}

/// 趋势图下的整点小时标注：每 6 桶标一个（对齐 React 的 `i % 6 === 0`）。
class _HourAxis extends StatelessWidget {
  const _HourAxis({required this.buckets});

  final List<StatsBucket> buckets;

  @override
  Widget build(BuildContext context) {
    if (buckets.length < 2) return const SizedBox.shrink();
    return LayoutBuilder(
      builder: (context, c) => Stack(
        children: [
          for (var i = 0; i < buckets.length; i += 6)
            Positioned(
              left: (i / (buckets.length - 1)) * c.maxWidth - 9,
              child: SizedBox(
                width: 18,
                child: Text(
                  hourTickOf(buckets[i]),
                  textAlign: TextAlign.center,
                  style: _panelMono(8, _panelMuted), // Home.tsx:383-384 固定面板色，不跟主题
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// 快捷键 chip：文案 + 键位。键位符号是规则 6 第 ① 类显式 LTR。
class _Chip extends StatelessWidget {
  const _Chip({required this.label, required this.kbd, required this.onTap});

  final String label;
  final String kbd;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    // globals.css `.cmd-kb`：固定深色 s2 底，不跟主题（面板内 chip 浅色模式也保持深）。
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Container(
        padding: const EdgeInsets.symmetric(
          horizontal: 11,
          vertical: 7,
        ),
        decoration: BoxDecoration(
          color: _panelS2,
          border: Border.all(color: _panelLine),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              label,
              style: const TextStyle(
                fontSize: 12,
                fontWeight: FontWeight.w600,
                color: _panelFg,
              ),
            ),
            const SizedBox(width: 7),
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
          ],
        ),
      ),
    );
  }
}
