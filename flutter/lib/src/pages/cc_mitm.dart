// ── Claude Code 订阅透传 MITM（cc-sub-mitm 票 12 的 Flutter 侧）──────
// 对应 React：`CcMitmAccessSection.tsx`（表单）、`CcMitmInfo.tsx`（卡片）、
// `MitmLog.tsx`（观测页）。wire 全 snake_case（`mitm_stats.rs` 非 camelCase 族）。
//
// 数据打开即拉一次，无轮询（被动数据：采样蹭 Claude Code 自然请求 usage 端点）。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../utils/formatters.dart';
import '../../charts.dart';
import '../shell/app_shell.dart' show PageHead;
import '../shell/theme.dart';
import '../shell/tiles.dart' show Tile;
import 'invoke.dart';
import 'ui_bits.dart' show AidogSwitch, SmallButton;

/// `mitm.ts:139::OauthUsageSample`：5h/7d 窗口利用率采样点。
class OauthUsageSample {
  const OauthUsageSample({
    required this.fiveHourPct,
    required this.sevenDayPct,
    required this.sampledAt,
  });

  factory OauthUsageSample.fromJson(Map<String, dynamic> j) => OauthUsageSample(
    fiveHourPct: (j['five_hour_pct'] as num?)?.toDouble() ?? 0,
    sevenDayPct: (j['seven_day_pct'] as num?)?.toDouble() ?? 0,
    sampledAt: (j['sampled_at'] as num?)?.toInt() ?? 0,
  );

  final double fiveHourPct;
  final double sevenDayPct;
  final int sampledAt;
}

/// `mitm.ts:146::MitmRefreshStats`：token 刷新观测统计（#91703 警示消费）。
class MitmRefreshStats {
  const MitmRefreshStats({required this.attempts, required this.failures});

  factory MitmRefreshStats.fromJson(Map<String, dynamic> j) => MitmRefreshStats(
    attempts: (j['attempts'] as num?)?.toInt() ?? 0,
    failures: (j['failures'] as num?)?.toInt() ?? 0,
  );

  final int attempts;
  final int failures;
}

/// `mitm.ts:161::CcProfile`：订阅套餐档位（无行 = null）。
class CcProfile {
  const CcProfile({required this.tier, required this.updatedAt});

  factory CcProfile.fromJson(Map<String, dynamic> j) => CcProfile(
    tier: j['tier'] as String? ?? '',
    updatedAt: (j['updated_at'] as num?)?.toInt() ?? 0,
  );

  final String tier;
  final int updatedAt;
}

/// 卡片一次拉齐的数据（`useCcMitmInfo`：refresh 计数恒拉；组名有则再拉趋势 + 档位）。
class CcMitmInfoData {
  const CcMitmInfoData({
    required this.samples,
    required this.refresh,
    required this.plan,
  });

  final List<OauthUsageSample> samples;
  final MitmRefreshStats? refresh;
  final CcProfile? plan;
}

/// `useCcMitmInfo`：激活时并行拉三路，各 catch 静默。
Future<CcMitmInfoData> fetchCcMitmInfo(
  InvokeFn invoke,
  String? groupName,
) async {
  MitmRefreshStats? refresh;
  try {
    final v = await invoke('mitm_oauth_refresh_stats', {'sinceMs': 0});
    refresh = MitmRefreshStats.fromJson(
      ((v as Map?) ?? const {}).cast<String, dynamic>(),
    );
  } catch (_) {
    refresh = null;
  }
  var samples = const <OauthUsageSample>[];
  CcProfile? plan;
  if (groupName != null && groupName.isNotEmpty) {
    final s = await invoke('mitm_usage_trend', {'groupName': groupName})
        .catchError((Object _) => const <Object?>[]);
    samples = [
      for (final e in (s as List? ?? const []))
        OauthUsageSample.fromJson(e as Map<String, dynamic>),
    ];
    final p = await invoke('cc_plan_info', {'groupName': groupName})
        .catchError((Object _) => null);
    if (p is Map) plan = CcProfile.fromJson(p.cast<String, dynamic>());
  }
  return CcMitmInfoData(samples: samples, refresh: refresh, plan: plan);
}

/// `mitm.ts:167::MitmBypassRow`：旁路观测行摘要（body 不返回）。
class MitmBypassRow {
  const MitmBypassRow({
    required this.id,
    required this.groupName,
    required this.host,
    required this.path,
    required this.statusCode,
    required this.reqBytes,
    required this.respBytes,
    required this.decrypted,
    required this.createdAt,
  });

  factory MitmBypassRow.fromJson(Map<String, dynamic> j) => MitmBypassRow(
    id: (j['id'] as num?)?.toInt() ?? 0,
    groupName: j['group_name'] as String? ?? '',
    host: j['host'] as String? ?? '',
    path: j['path'] as String? ?? '',
    statusCode: (j['status_code'] as num?)?.toInt() ?? 0,
    reqBytes: (j['req_bytes'] as num?)?.toInt() ?? 0,
    respBytes: (j['resp_bytes'] as num?)?.toInt() ?? 0,
    decrypted: j['decrypted'] == true,
    createdAt: (j['created_at'] as num?)?.toInt() ?? 0,
  );

  final int id;
  final String groupName;
  final String host;
  final String path;
  final int statusCode;
  final int reqBytes;
  final int respBytes;
  final bool decrypted;
  final int createdAt;
}

// ── 平台卡子件（`CcMitmInfo.tsx`）──────────────────────────────────────

/// 头部警示 chip：token refresh 有失败才渲染（title 展开 #91703 应对文案）。
class CcMitmRefreshWarning extends StatelessWidget {
  const CcMitmRefreshWarning({super.key, required this.refresh});

  final MitmRefreshStats refresh;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Tooltip(
      message: t.t('platform.mitmRefreshWarnHint', {
        'fails': '${refresh.failures}',
        'total': '${refresh.attempts}',
      }),
      child: Container(
        margin: const EdgeInsets.only(top: 3),
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
        decoration: BoxDecoration(
          color: theme.c.peak.withValues(alpha: 0.14),
          border: Border.all(color: theme.c.peak.withValues(alpha: 0.35)),
          borderRadius: BorderRadius.circular(5),
        ),
        child: Text(
          t.t('platform.mitmRefreshWarn', {
            'fails': '${refresh.failures}',
            'total': '${refresh.attempts}',
          }),
          style: AidogType.caption.copyWith(
            fontSize: 10,
            fontWeight: FontWeight.w600,
            color: theme.c.peak,
          ),
        ),
      ),
    );
  }
}

/// 展开明细：最新窗口利用率 + 趋势线（≥2 个采样点才画图）。
class CcMitmTrendSection extends StatelessWidget {
  const CcMitmTrendSection({super.key, required this.samples});

  final List<OauthUsageSample> samples;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final latest = samples.isNotEmpty ? samples.last : null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          t.t('platform.mitmWindowsLabel'),
          style: AidogType.caption.copyWith(
            fontSize: 10,
            fontWeight: FontWeight.w600,
            letterSpacing: 0.3,
            color: theme.c.fg3,
          ),
        ),
        const SizedBox(height: 4),
        if (latest != null)
          Row(
            children: [
              Text('5h ', style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg2)),
              Text(
                formatPercent(latest.fiveHourPct),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  fontWeight: FontWeight.w700,
                  color: theme.c.fg,
                ),
              ),
              const SizedBox(width: 12),
              Text('7d ', style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg2)),
              Text(
                formatPercent(latest.sevenDayPct),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  fontWeight: FontWeight.w700,
                  color: theme.c.fg,
                ),
              ),
            ],
          )
        else
          Text(
            t.t('platform.mitmWindowsEmpty'),
            style: AidogType.caption.copyWith(fontSize: 11, color: theme.c.fg3),
          ),
        if (samples.length >= 2) ...[
          const SizedBox(height: 4),
          SizedBox(
            height: 110,
            child: AidogLineChart(
              mini: true,
              series: [
                ChartSeries(
                  key: 'five',
                  label: '5h',
                  color: ChartPalette.of(context).series(0),
                  points: [
                    for (final s in samples)
                      ChartPoint(s.sampledAt.toDouble(), s.fiveHourPct),
                  ],
                  format: (n) => formatPercent(n, 0),
                ),
                ChartSeries(
                  key: 'seven',
                  label: '7d',
                  color: ChartPalette.of(context).series(1),
                  points: [
                    for (final s in samples)
                      ChartPoint(s.sampledAt.toDouble(), s.sevenDayPct),
                  ],
                  format: (n) => formatPercent(n, 0),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }
}

/// 余额位块（balance-full）：套餐档位 + 5h/7d 剩余额度（剩余 = 100 − 最新利用率）。
/// 无任一数据不渲染。
class CcPlanBalance extends StatelessWidget {
  const CcPlanBalance({super.key, required this.plan, required this.samples});

  final CcProfile? plan;
  final List<OauthUsageSample> samples;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final latest = samples.isNotEmpty ? samples.last : null;
    if (plan == null && latest == null) return const SizedBox.shrink();
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (plan != null)
          Text(
            t.t('platform.mitmPlanTier', {'tier': plan!.tier}),
            style: AidogType.caption.copyWith(fontSize: 10, color: theme.c.fg3),
          ),
        if (plan != null && latest != null) const SizedBox(width: 6),
        if (latest != null)
          Text(
            t.t('platform.mitmPlanRemain', {
              'five': formatPercent(100 - latest.fiveHourPct, 0),
              'seven': formatPercent(100 - latest.sevenDayPct, 0),
            }),
            style: AidogType.caption.copyWith(fontSize: 10, color: theme.c.fg3),
          ),
      ],
    );
  }
}

// ── 表单区块（`CcMitmAccessSection.tsx`）───────────────────────────────

/// 订阅透传 MITM 统计区块：开关 + CA 引导 + export 语句 + 三条须知。
/// 开关状态由父级（表单控制器）持有，本组件只做呈现。
class CcMitmAccessSection extends StatelessWidget {
  const CcMitmAccessSection({
    super.key,
    required this.enabled,
    required this.onToggle,
    required this.caReady,
    required this.exportLine,
    required this.onCopy,
  });

  final bool enabled;
  final ValueChanged<bool> onToggle;

  /// null = 还没查回（不显 CA 警示）。
  final bool? caReady;
  final String exportLine;
  final VoidCallback onCopy;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // 区块小标题（React `FormSection title`，无卡面 —— 表单是子块不是分区）。
        Text(
          t.t('platform.mitmSection'),
          style: AidogType.label.copyWith(
            fontSize: 13,
            fontWeight: FontWeight.w700,
            color: theme.c.fg,
          ),
        ),
        const SizedBox(height: 10),
        Row(
          children: [
            AidogSwitch(value: enabled, onChanged: () => onToggle(!enabled)),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                t.t('platform.mitmStatsToggle'),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg2,
                ),
              ),
            ),
          ],
        ),
        // Root CA 启用引导：MITM 未启用 / CA 未装时指路（装 CA 流程在 设置 → MITM）。
        if (enabled && caReady == false) ...[
          const SizedBox(height: 10),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            decoration: BoxDecoration(
              color: theme.c.peak.withValues(alpha: 0.10),
              border: Border.all(color: theme.c.peak.withValues(alpha: 0.30)),
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: Text(
              t.t('platform.mitmCaHint'),
              style: AidogType.caption.copyWith(
                fontSize: 12,
                height: 1.5,
                color: theme.c.fg2,
              ),
            ),
          ),
        ],
        // export 语句（Desktop / 多 shell；密码/端口与 sync 注入同一份）。
        if (exportLine.isNotEmpty) ...[
          const SizedBox(height: 10),
          Text(
            t.t('platform.mitmExportLabel'),
            style: AidogType.caption.copyWith(fontSize: 11, color: theme.c.fg3),
          ),
          const SizedBox(height: 4),
          Row(
            children: [
              Expanded(
                child: Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 6,
                  ),
                  decoration: BoxDecoration(
                    color: theme.c.surface2,
                    border: Border.all(color: theme.c.line),
                    borderRadius: BorderRadius.circular(AidogRadius.sm),
                  ),
                  child: Text(
                    exportLine,
                    style: AidogType.caption.copyWith(
                      fontSize: 11,
                      color: theme.c.fg,
                      fontFamily: AidogType.familyMono,
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 6),
              SmallButton(
                label: t.t('action.copy'),
                fontSize: 11,
                padding: (10, 5),
                onTap: onCopy,
              ),
            ],
          ),
        ],
        // 三条接入须知（#96258 / undici decodeURIComponent / CC 启动只读一次 env）。
        const SizedBox(height: 10),
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            for (final k in const [
              'platform.mitmNoteRestart',
              'platform.mitmNoteName',
              'platform.mitmNoteDesktop',
            ])
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '• ',
                    style: AidogType.caption.copyWith(
                      fontSize: 11,
                      height: 1.7,
                      color: theme.c.fg3,
                    ),
                  ),
                  Expanded(
                    child: Text(
                      t.t(k),
                      style: AidogType.caption.copyWith(
                        fontSize: 11,
                        height: 1.7,
                        color: theme.c.fg3,
                      ),
                    ),
                  ),
                ],
              ),
          ],
        ),
      ],
    );
  }
}

// ── MITM 观测页（`MitmLog.tsx`）────────────────────────────────────────

/// 独立观测页：旁路行表格 + 行点开按需拉 body 详情。proxy_log 日志页不混入观测行。
class MitmLogPage extends StatefulWidget {
  const MitmLogPage({super.key, this.invoke});

  final InvokeFn? invoke;

  @override
  State<MitmLogPage> createState() => _MitmLogPageState();
}

class _MitmLogPageState extends State<MitmLogPage> {
  late final InvokeFn _invoke = widget.invoke ?? kernelInvoke;
  List<MitmBypassRow>? _rows;
  int? _openId;
  final Map<int, MitmBypassDetail?> _details = {};

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final v = await _invoke('mitm_bypass_list', {'limit': 200});
      if (!mounted) return;
      setState(() {
        _rows = [
          for (final e in (v as List? ?? const []))
            MitmBypassRow.fromJson(e as Map<String, dynamic>),
        ];
      });
    } catch (_) {
      if (mounted) setState(() => _rows = const []);
    }
  }

  void _toggle(int id) {
    if (_openId == id) {
      setState(() => _openId = null);
      return;
    }
    setState(() => _openId = id);
    if (!_details.containsKey(id)) {
      _invoke('mitm_bypass_detail', {'id': id}).then((d) {
        if (!mounted) return;
        setState(
          () => _details[id] = d is Map
              ? MitmBypassDetail.fromJson(d.cast<String, dynamic>())
              : null,
        );
      }).catchError((Object _) {
        if (mounted) setState(() => _details[id] = null);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final rows = _rows;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHead(title: t.t('page.mitmLog')),
        if (rows == null)
          Text(
            t.t('status.loading'),
            style: AidogType.caption.copyWith(color: theme.c.fg3),
          )
        else if (rows.isEmpty)
          Text(
            t.t('stats.bypassEmpty'),
            style: AidogType.caption.copyWith(color: theme.c.fg3),
          )
        else
          Tile(
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                _headerCell(t.t('stats.bypassTime')),
                for (final r in rows) ...[
                  const Divider(height: 1),
                  _row(context, r),
                  if (_openId == r.id) ...[
                    const Divider(height: 1),
                    _detailCell(context, r),
                  ],
                ],
              ],
            ),
          ),
      ],
    );
  }

  Widget _headerCell(String s) => Text(s, style: AidogType.micro);

  Widget _row(BuildContext context, MitmBypassRow r) {
    final theme = AidogTheme.of(context);
    final open = _openId == r.id;
    return InkWell(
      onTap: () => _toggle(r.id),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Row(
          children: [
            Expanded(
              flex: 2,
              child: Text(
                formatDateTime(r.createdAt),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg2,
                ),
              ),
            ),
            Expanded(
              flex: 2,
              child: Text(
                r.groupName.isEmpty ? '-' : r.groupName,
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg2,
                ),
              ),
            ),
            Expanded(
              flex: 4,
              child: Text(
                '${open ? '▾ ' : '▸ '}${r.host}${r.path}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.caption.copyWith(
                  fontSize: 11,
                  fontFamily: AidogType.familyMono,
                  color: theme.c.fg2,
                ),
              ),
            ),
            SizedBox(
              width: 48,
              child: Text(
                '${r.statusCode}',
                textAlign: TextAlign.end,
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: r.statusCode >= 400 ? theme.c.bad : theme.c.fg2,
                ),
              ),
            ),
            const SizedBox(width: 8),
            SizedBox(
              width: 72,
              child: Text(
                formatBytes(r.reqBytes + r.respBytes),
                textAlign: TextAlign.end,
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: theme.c.fg2,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _detailCell(BuildContext context, MitmBypassRow r) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    if (!_details.containsKey(r.id)) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Text(
          t.t('status.loading'),
          style: AidogType.caption.copyWith(color: theme.c.fg3),
        ),
      );
    }
    final d = _details[r.id];
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Wrap(
        spacing: 8,
        runSpacing: 8,
        children: [
          _bodyBlock(context, t.t('page.mitmBodyRequest'), d?.requestBody ?? ''),
          _bodyBlock(
            context,
            t.t('page.mitmBodyResponse'),
            d?.responseBody ?? '',
          ),
        ],
      ),
    );
  }

  Widget _bodyBlock(BuildContext context, String label, String body) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return SizedBox(
      width: 380,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            label,
            style: AidogType.caption.copyWith(
              fontSize: 11,
              fontWeight: FontWeight.w600,
              color: theme.c.fg2,
            ),
          ),
          const SizedBox(height: 4),
          Container(
            constraints: const BoxConstraints(maxHeight: 220),
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: theme.c.surface2,
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
            child: SingleChildScrollView(
              child: Text(
                body.isEmpty ? t.t('page.mitmBodyEmpty') : body,
                style: AidogType.caption.copyWith(
                  fontSize: 11,
                  fontFamily: AidogType.familyMono,
                  color: theme.c.fg2,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// `mitm.ts:174::MitmBypassDetail`。
class MitmBypassDetail {
  const MitmBypassDetail({required this.requestBody, required this.responseBody});

  factory MitmBypassDetail.fromJson(Map<String, dynamic> j) => MitmBypassDetail(
    requestBody: j['request_body'] as String? ?? '',
    responseBody: j['response_body'] as String? ?? '',
  );

  final String requestBody;
  final String responseBody;
}
