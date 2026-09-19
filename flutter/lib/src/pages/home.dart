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
    _logSub =
        (widget.logUpdates ?? debounceStream(kernelProxyLogUpdated())).listen((
          _,
        ) {
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
    await Future.wait<void>([
      _guard(() async {
        final v = await widget.invoke('proxy_status');
        if (mounted) setState(() => _running = v as bool?);
      }, () => setState(() => _running = null)),
      _guard(() async {
        final v = await widget.invoke('proxy_get_settings');
        final s = ProxySettingsSummary.fromJson(v! as Map<String, dynamic>);
        if (mounted) setState(() => _port = s.port);
      }, null),
      _guard(() async {
        final v = await widget.invoke('tray_today_stats');
        final s = TodayStats.fromJson(v! as Map<String, dynamic>);
        if (mounted) setState(() => _today = s);
      }, () => setState(() => _today = null)),
      _guard(() async {
        final v = await widget.invoke('popover_platform_today');
        final list = [
          for (final e in v! as List)
            TodayPlatformStat.fromJson(e as Map<String, dynamic>),
        ];
        if (mounted) setState(() => _platformsToday = list);
      }, () => setState(() => _platformsToday = const [])),
      _guard(() async {
        final v = await widget.invoke('platform_list');
        final list = [
          for (final e in v! as List)
            PlatformSummary.fromJson(e as Map<String, dynamic>),
        ];
        if (mounted) setState(() => _platforms = list);
      }, () => setState(() => _platforms = const [])),
      _guard(() async {
        final v = await widget.invoke('stats_query', {
          'query': {
            'start': window.start,
            'end': window.end,
            'granularity': 'hourly',
          },
        });
        final r = StatsResult.fromJson(v! as Map<String, dynamic>);
        if (mounted) setState(() => _trend = r.buckets);
      }, () => setState(() => _trend = const [])),
    ]);
    _inFlight = false;
    if (mounted) setState(() => _loading = false);
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
    final palette = ChartPalette.of(context);

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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHead(title: tr.t('page.home'), subtitle: tr.t('home.desc')),
        Bento(
          children: [
            BentoCell(span: 12, child: _statusTile(t, tr)),
            if (has && today != null) ...[
              BentoCell(
                span: 3,
                child: ReadoutTile(
                  label: tr.t('home.cost'),
                  value: formatCostUsd(today.cost),
                  spark: _Spark(values: s.cost, color: palette.series(0)),
                ),
              ),
              BentoCell(
                span: 3,
                child: ReadoutTile(
                  label: tr.t('home.tokens'),
                  value: formatNumber(today.tokens),
                  spark: _Spark(values: s.tokens, color: palette.series(1)),
                ),
              ),
              BentoCell(
                span: 3,
                child: ReadoutTile(
                  label: tr.t('home.requests'),
                  value: formatNumber(today.totalRequests),
                  spark: _Spark(values: s.requests, color: palette.series(1)),
                ),
              ),
              BentoCell(
                span: 3,
                child: ReadoutTile(
                  label: tr.t('home.cacheRate'),
                  value: formatPercent(today.cacheRate),
                  spark: _Spark(values: s.cache, color: palette.series(1)),
                ),
              ),
            ] else
              BentoCell(
                span: 12,
                child: Tile(
                  child: Text(
                    emptyText,
                    style: AidogType.caption.copyWith(color: t.c.fg3),
                  ),
                ),
              ),
            BentoCell(
              span: 6,
              child: SeriesTile(
                title: tr.t('home.trend24h'),
                meta: trendOk
                    ? 'HOURLY · ${tr.t('home.trendPeak')} ${formatNumber(peak)}'
                    : 'HOURLY',
                chartHeight: 110,
                legend: [
                  (color: palette.series(0), label: tr.t('home.trendRequests')),
                  (color: palette.series(1), label: tr.t('home.trendCost')),
                ],
                chart: trendOk
                    ? Column(
                        children: [
                          Expanded(
                            child: AidogLineChart(
                              mini: true,
                              area: true,
                              series: [
                                ChartSeries(
                                  key: 'req',
                                  label: tr.t('home.trendRequests'),
                                  color: palette.series(0),
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
                                  color: palette.series(1),
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
                          SizedBox(height: 12, child: _HourAxis(buckets: _trend)),
                        ],
                      )
                    : ChartEmpty(emptyText),
              ),
            ),
            BentoCell(
              span: 6,
              child: top.isEmpty
                  ? Tile(
                      title: tr.t('home.topPlatforms'),
                      meta: 'TOP $kTopPlatforms · ${tr.t('home.trendCost')}',
                      child: Text(
                        emptyText,
                        style: AidogType.caption.copyWith(color: t.c.fg3),
                      ),
                    )
                  : ListingTile(
                      title: tr.t('home.topPlatforms'),
                      meta: 'TOP $kTopPlatforms · ${tr.t('home.trendCost')}',
                      rows: [
                        for (final p in top)
                          _platformRow(t, palette, p, maxCost, costSum),
                      ],
                    ),
            ),
            if (balance > 0)
              BentoCell(
                span: 12,
                child: ReadoutTile(
                  label: tr.t('home.totalBalance'),
                  value: formatCostUsd(balance),
                ),
              ),
            BentoCell(
              span: 12,
              child: ActionTile(
                fields: [
                  Wrap(
                    spacing: AidogSpace.ssm,
                    runSpacing: AidogSpace.ssm,
                    children: [
                      for (final c in _chips(tr))
                        _Chip(label: c.label, kbd: c.kbd, onTap: c.run),
                    ],
                  ),
                ],
              ),
            ),
          ],
        ),
      ],
    );
  }

  Widget _statusTile(AidogTheme t, I18nController tr) {
    final running = _running;
    final statusText = running == null
        ? tr.t('home.statusUnknown')
        : running
        ? tr.t('home.statusRunning')
        : tr.t('home.statusStopped');
    return Tile(
      live: running ?? false,
      child: Row(
        children: [
          LiveDot(on: running ?? false),
          const SizedBox(width: AidogSpace.smd),
          Expanded(
            child: Ltr(
              child: Text(
                '${tr.t('home.proxyStatus')} $statusText · '
                '${tr.t('home.port')} $_port',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.body.copyWith(color: t.c.fg),
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

  Widget _platformRow(
    AidogTheme t,
    ChartPalette palette,
    TodayPlatformStat p,
    double maxCost,
    double costSum,
  ) {
    return Row(
      children: [
        SizedBox(
          width: 26,
          height: 26,
          child: CustomPaint(
            painter: _RingPainter(
              share: costSum > 0 ? p.cost / costSum : 0,
              track: t.c.line,
              arc: palette.series(0),
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
              color: t.c.fg,
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
              backgroundColor: t.c.line,
              valueColor: AlwaysStoppedAnimation<Color>(palette.series(0)),
            ),
          ),
        ),
        const SizedBox(width: AidogSpace.smd),
        Ltr(
          child: Text(
            '${formatNumber(p.requests)} · ${formatNumber(p.tokens)}',
            style: numStyle(AidogType.numSm, t.c.fg3),
          ),
        ),
        const SizedBox(width: AidogSpace.smd),
        Ltr(
          child: Text(
            formatCostUsd(p.cost),
            style: numStyle(AidogType.numMd, palette.series(0)),
          ),
        ),
      ],
    );
  }
}

/// KPI 格行内 sparkline：走 [normPoints] 的 min-max 归一化（与 React 版同一函数）。
/// 少于两点不画（对齐 React 的 `values.length < 2 → null`）。
class _Spark extends StatelessWidget {
  const _Spark({required this.values, required this.color});

  final List<double> values;
  final Color color;

  @override
  Widget build(BuildContext context) {
    if (values.length < 2) return const SizedBox.shrink();
    return CustomPaint(painter: _SparkPainter(values: values, color: color));
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
    final r = size.shortestSide / 2 - 2.5;
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
    final t = AidogTheme.of(context);
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
                  style: AidogType.micro.copyWith(color: t.c.fg3),
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
    final t = AidogTheme.of(context);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(AidogRadius.sm),
      child: Container(
        padding: const EdgeInsets.symmetric(
          horizontal: AidogSpace.smd,
          vertical: AidogSpace.ssm,
        ),
        decoration: BoxDecoration(
          color: t.c.surface2,
          border: Border.all(color: t.c.line),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(label, style: AidogType.caption.copyWith(color: t.c.fg)),
            const SizedBox(width: AidogSpace.ssm),
            Ltr(
              child: Text(kbd, style: numStyle(AidogType.numSm, t.c.fg3)),
            ),
          ],
        ),
      ),
    );
  }
}
