/// 平台卡（票 I18b），对应 React 的 `src/components/platforms/PlatformCard.tsx`。
///
/// 三段结构与 React 逐段对应：
///   - **行 1（身份 + 快操作）**：拖拽手柄 / logo + 健康点 / 名称 + 协议·base_url +
///     一串状态徽标（Coding Plan、自动禁用、高峰、过期、所属分组、最近测试、最近错误）/ 按钮组。
///   - **行 2（余额区）**：余额进度条、手动预算、coding 已用、本周期折算、配额档位、速率余量。
///   - **展开区**：品牌外链、已使用 / 今日统计、配额档位大号版、端点与模型 badge。
///
/// 停用态整卡半透明（`PlatformCard.tsx:188` 的 `opacity: 0.5`）。
/// 独立文件而不是塞回 `platforms.dart`：那边是页面编排，这里是一张卡的渲染。
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../utils/color_level.dart';
import '../../utils/formatters.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'models.dart';
import 'platform_card_bits.dart';
import 'platforms_logic.dart';
import 'ui_bits.dart';

class PlatformCard extends StatelessWidget {
  const PlatformCard({
    super.key,
    required this.c,
    required this.platform,
    required this.index,
    required this.usage,
    required this.quota,
    required this.quotaPending,
    required this.quotaRefreshing,
    required this.lastTest,
    required this.testing,
    required this.onToggle,
    required this.onTest,
    required this.onModelTest,
    required this.onRefreshQuota,
    required this.onDelete,
    required this.onViewLogs,
    required this.onShare,
    this.onEdit,
    this.onDuplicate,
    this.draggable = true,
    this.nowMs,
  });

  final PlatformsController c;
  final PlatformRow platform;
  final int index;
  final UsageStats? usage;
  final PlatformQuota? quota;
  final bool quotaPending;
  final bool quotaRefreshing;
  final LastTestResult? lastTest;
  final bool testing;
  final VoidCallback onToggle;
  final VoidCallback onTest;
  final VoidCallback onModelTest;
  final VoidCallback onRefreshQuota;
  final VoidCallback onDelete;
  final VoidCallback onViewLogs;
  final VoidCallback onShare;

  /// 编辑 / 复制平台都要打开表单（另一张票）。没接上就不渲染这两颗按钮 ——
  /// 画一颗点了没反应的按钮比没有这颗按钮更糟。
  final VoidCallback? onEdit;
  final VoidCallback? onDuplicate;

  /// 只读场景（分组展开区）不给拖拽手柄，`PlatformCard.tsx:203` 的 `draggable` 同义。
  final bool draggable;

  /// 测试注入「现在」，让高峰 / 倒计时 / 过期这些时间相关的断言可复现。
  final int? nowMs;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final p = platform;
    final now = nowMs ?? DateTime.now().millisecondsSinceEpoch;
    final meta = c.protocolMeta;

    // logo 三级回退的第一级：本地缓存路径 → data URL（miss 会后台补拉，见控制器）。
    c.ensureProtocolLogo(p.platformType);

    final q = computeQuotaDisplay(
      p,
      quota,
      c.quotaRealIds[p.id] == true,
      nowMs: now,
    );
    final mb = computeManualBudgetDisplay(p.manualBudgets);
    final quotaCapable = meta.hasQuotaScript(p.platformType, p.extra);
    final showQuota = quotaCapable && q.hasData;
    final showQuotaSkeleton = quotaCapable && !q.hasData && quotaPending;
    final hasCodingEndpoint = p.endpoints.any((e) => e.codingPlan);
    // `PlatformCard.tsx:146`：显式配置优先；一个都没配但有 available_models 时
    // **仍返回空**（用户自己选过模型了，不要拿 preset 默认冒充）；否则回落 preset。
    final configuredModels = () {
      final explicit = allModelValues(p.models);
      if (explicit.isNotEmpty) return explicit;
      if (p.availableModels.isNotEmpty) return explicit;
      return meta.modelsFor(p.platformType, p.extra, now);
    }();
    final usagePending = c.usageLoading && usage == null;
    final hasDetail =
        usage != null ||
        usagePending ||
        p.endpoints.isNotEmpty ||
        configuredModels.isNotEmpty ||
        q.tiers.isNotEmpty;
    final expanded = hasDetail && c.expandedIds.contains(p.id);

    // 上游速率限制快照：窗口通常 1 分钟，超 5 分钟的旧值不再代表现状，宁可不显示。
    final rl = () {
      final parsed = parseRateLimit(p.rateLimit);
      if (parsed == null) return null;
      return now - parsed.observedAt > 300000 ? null : parsed;
    }();

    final card = Tile(
      live: p.status == 'enabled',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              if (draggable) ...[
                ReorderableDragStartListener(
                  index: index,
                  child: Tooltip(
                    message: t.t('platform.dragReorder'),
                    child: MouseRegion(
                      cursor: SystemMouseCursors.grab,
                      child: Icon(
                        Icons.drag_indicator,
                        size: 16,
                        color: theme.c.fg3,
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: AidogSpace.ssm),
              ],
              _LogoDot(
                protocol: p.platformType,
                logoSrc: c.protocolLogos[p.platformType],
                health: deriveHealth(
                  status: p.status,
                  lastError: p.lastError,
                  manual: c.testResults[p.id],
                  recentTotal: usage?.recentTotal,
                  recentFailures: usage?.recentFailures,
                ),
                lastError: p.lastError,
                lastErrorAt: p.lastErrorAt,
              ),
              const SizedBox(width: AidogSpace.smd),
              Expanded(
                child: _Identity(
                  platform: p,
                  meta: meta,
                  lastTest: lastTest,
                  membership: c.membership[p.id],
                  nowMs: now,
                ),
              ),
              if (hasDetail)
                IconButton(
                  iconSize: 16,
                  visualDensity: VisualDensity.compact,
                  tooltip: t.t('platform.toggleDetail'),
                  icon: Icon(
                    expanded ? Icons.expand_less : Icons.expand_more,
                    color: theme.c.fg3,
                  ),
                  onPressed: () => c.toggleExpanded(p.id, !expanded),
                ),
              Wrap(
                spacing: AidogSpace.sxs,
                runSpacing: AidogSpace.sxs,
                children: [
                  SmallButton(
                    label: testing
                        ? t.t('status.loading')
                        : t.t('platform.quickTest'),
                    onTap: testing ? null : onTest,
                  ),
                  SmallButton(label: t.t('test.title'), onTap: onModelTest),
                  if (quotaCapable)
                    SmallButton(
                      label: t.t('platform.quotaRefresh'),
                      onTap: quotaRefreshing ? null : onRefreshQuota,
                    ),
                  SmallButton(
                    label: p.status == 'enabled'
                        ? t.t('platform.disable')
                        : p.status == 'auto_disabled'
                        ? t.t('platform.reenable')
                        : t.t('platform.enable'),
                    onTap: onToggle,
                  ),
                  SmallButton(label: t.t('page.logs'), onTap: onViewLogs),
                  if (onEdit != null)
                    SmallButton(label: t.t('action.edit'), onTap: onEdit),
                  SmallButton(
                    label: t.t('platform.share.button'),
                    onTap: onShare,
                  ),
                  if (onDuplicate != null)
                    SmallButton(
                      label: t.t('platform.duplicate'),
                      onTap: onDuplicate,
                    ),
                  SmallButton(
                    label: t.t('action.delete'),
                    danger: true,
                    onTap: onDelete,
                  ),
                ],
              ),
            ],
          ),
          if (showQuota &&
              (q.balanceRemaining != null ||
                  mb != null ||
                  q.tiers.isNotEmpty)) ...[
            const SizedBox(height: AidogSpace.ssm),
            _BalanceRow(
              platform: p,
              quota: q,
              budget: mb,
              usage: usage,
              hasCodingEndpoint: hasCodingEndpoint,
              rateLimit: rl,
              nowMs: now,
            ),
          ],
          // ④ 延迟档：可查 quota 的平台数据未回 → 余额区显骨架，不显空白也不显旧 est 值。
          if (showQuotaSkeleton) ...[
            const SizedBox(height: AidogSpace.ssm),
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: SkeletonBox(
                width: 120,
                height: 22,
                semanticLabel: t.t('platform.quotaLoading'),
              ),
            ),
          ],
          if (expanded) ...[
            const SizedBox(height: AidogSpace.smd),
            _DetailSection(
              platform: p,
              meta: meta,
              quota: q,
              usage: usage,
              usagePending: usagePending,
              configuredModels: configuredModels,
              nowMs: now,
            ),
          ],
        ],
      ),
    );
    // 停用态整卡压暗（`PlatformCard.tsx:188`：`p.enabled ? 1 : 0.5`）。
    return Opacity(opacity: p.enabled ? 1 : 0.5, child: card);
  }
}

// ── 行 1：身份区 ──────────────────────────────────────────────────

/// logo（缓存图 → 协议首两字母）+ 右上角健康点。
class _LogoDot extends StatelessWidget {
  const _LogoDot({
    required this.protocol,
    required this.logoSrc,
    required this.health,
    required this.lastError,
    required this.lastErrorAt,
  });

  final String protocol;
  final String? logoSrc;
  final HealthStatus health;
  final String lastError;
  final int lastErrorAt;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final color = healthColor(health, theme.c);
    final bytes = decodeLogoDataUrl(logoSrc);
    return Tooltip(
      message: lastError.isEmpty
          ? ''
          : t
                .t('platform.lastErrorHint')
                .replaceAll('{{time}}', formatDateTime(lastErrorAt))
                .replaceAll('{{error}}', lastError),
      child: SizedBox(
        width: 40,
        height: 40,
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            Container(
              width: 36,
              height: 36,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                border: Border.all(color: theme.c.line),
                borderRadius: BorderRadius.circular(AidogRadius.sm),
              ),
              clipBehavior: Clip.antiAlias,
              child: bytes != null
                  ? Image.memory(bytes, fit: BoxFit.contain)
                  : Text(
                      protocol.isEmpty
                          ? '?'
                          : protocol
                                .substring(0, protocol.length < 2 ? 1 : 2)
                                .toUpperCase(),
                      style: AidogType.micro.copyWith(
                        color: theme.c.accentText,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
            ),
            Positioned(
              top: 0,
              right: 0,
              child: Container(
                width: 10,
                height: 10,
                decoration: BoxDecoration(
                  color: color,
                  shape: BoxShape.circle,
                  border: Border.all(color: theme.c.bg, width: 2),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// `data:image/png;base64,...` → 字节；不是 data URL（或解不开）→ null。
Uint8List? decodeLogoDataUrl(String? src) {
  if (src == null || src.isEmpty) return null;
  final i = src.indexOf('base64,');
  if (i < 0) return null;
  try {
    return base64Decode(src.substring(i + 7));
  } catch (_) {
    return null;
  }
}

/// 名称 + 协议·base_url + 一串状态徽标（`PlatformCard.tsx:255-411`）。
class _Identity extends StatelessWidget {
  const _Identity({
    required this.platform,
    required this.meta,
    required this.lastTest,
    required this.membership,
    required this.nowMs,
  });

  final PlatformRow platform;
  final ProtocolMetaTable meta;
  final LastTestResult? lastTest;
  final List<String>? membership;
  final int nowMs;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final p = platform;
    final disableDuringPeak = parseDisableDuringPeak(p.extra);
    final userPeak = parsePlatformPeak(p.extra);
    final peakWindows = userPeak.isNotEmpty
        ? userPeak
        : (meta.presetPeak[p.platformType] ?? const <TimeWindow>[]);
    final badges = <Widget>[];

    // Coding Plan 套餐协议徽标（数据驱动：读 preset.is_coding_plan）。
    if (meta.isCodingPlan(p.platformType)) {
      badges.add(
        MiniBadge(
          text: t.t('platform.codingPlanBadge'),
          color: theme.c.ok,
          tooltip: t.t('platform.codingPlanHint'),
        ),
      );
    }
    // 401/403 自动禁用 + 下次试探时间。
    if (p.status == 'auto_disabled') {
      badges.add(
        MiniBadge(
          text: t.t('platform.autoDisabled'),
          color: theme.c.peak,
          icon: Icons.warning_amber_rounded,
          tooltip: t
              .t('platform.autoDisabledHint')
              .replaceAll(
                '{{time}}',
                p.autoDisabledUntil > 0
                    ? formatDateTime(p.autoDisabledUntil)
                    : '-',
              ),
        ),
      );
    }
    // 高峰禁用中（开关 on 且此刻命中窗口）；与下面的「高峰」徽标互斥，不重复显示。
    if (disableDuringPeak && isCurrentlyPeak(peakWindows, nowMs)) {
      badges.add(
        MiniBadge(
          text: t.t('platform.peak_disabled_badge'),
          color: theme.c.peak,
          tooltip: t.t('platform.disable_during_peak_desc'),
        ),
      );
    } else if (!disableDuringPeak && isCurrentlyPeak(peakWindows, nowMs)) {
      // model scope 限定时显「高峰·N模型」+ tooltip 列模型；非限定显「高峰」。
      final hit = peakWindows.firstWhere(
        (w) => isCurrentlyPeak([w], nowMs),
        orElse: () => peakWindows.first,
      );
      final models = hit.models;
      final hasScope = models != null && models.isNotEmpty;
      badges.add(
        MiniBadge(
          text: hasScope
              ? t
                    .t('platform.peak_badge_limited')
                    .replaceAll('{{count}}', '${models.length}')
              : t.t('platform.peak_badge'),
          color: theme.c.accentText,
          tooltip: hasScope
              ? t
                    .t('platform.peak_badge_models_tooltip')
                    .replaceAll('{{models}}', models.join(', '))
              : t.t('platform.peak'),
        ),
      );
    }
    // 过期：已过期显红 badge，未过期显「到期 <时间>」小字（24h 内变黄）。
    if (p.expiresAt > 0) {
      if (nowMs >= p.expiresAt) {
        badges.add(
          MiniBadge(
            text: t.t('platform.expired'),
            color: theme.c.bad,
            tooltip: t
                .t('platform.expiredHint')
                .replaceAll('{{time}}', formatDateTime(p.expiresAt)),
          ),
        );
      } else {
        final soon = p.expiresAt - nowMs < 86400000;
        badges.add(
          Tooltip(
            message: t.t('platform.expiresAtHint'),
            child: Text(
              t
                  .t('platform.expiresAtBadge')
                  .replaceAll('{{time}}', formatDateTime(p.expiresAt)),
              style: AidogType.micro.copyWith(
                color: soon ? theme.c.peak : theme.c.fg3,
                fontWeight: soon ? FontWeight.w600 : FontWeight.w500,
              ),
            ),
          ),
        );
      }
    }
    // 所属分组。
    for (final g in membership ?? const <String>[]) {
      badges.add(MiniBadge(text: g, color: theme.c.fg3));
    }
    // 最近一次测试（无记录不渲染）。带响应正文时点一下展开。
    final lt = lastTest;
    if (lt != null) {
      badges.add(_LastTestBadge(result: lt, nowMs: nowMs));
    }
    // 最近一次代理错误（系统维护；最近一次成功即清空）。
    if (p.lastError.isNotEmpty) {
      badges.add(
        MiniBadge(
          text: '${t.t('platform.lastError')}: ${p.lastError}',
          color: theme.c.bad,
          icon: Icons.error_outline,
          tooltip: t
              .t('platform.lastErrorHint')
              .replaceAll(
                '{{time}}',
                p.lastErrorAt > 0 ? formatDateTime(p.lastErrorAt) : '-',
              )
              .replaceAll('{{error}}', p.lastError),
        ),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          p.name,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: AidogType.body.copyWith(
            color: theme.c.fg,
            fontWeight: FontWeight.w600,
          ),
        ),
        Text(
          '${meta.label(p.platformType)} · '
          '${getPrimaryBaseUrl(p.platformType, p.endpoints).isEmpty ? p.baseUrl : getPrimaryBaseUrl(p.platformType, p.endpoints)}',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
        if (badges.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 3),
            child: Wrap(
              spacing: AidogSpace.sxs,
              runSpacing: 3,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: badges,
            ),
          ),
      ],
    );
  }
}

/// 「最近测试」徽章（`PlatformCard.tsx:963::LastTestBadge`）。
/// 响应正文非空才可点：点开在徽章下面摊开解析后的正文，再点收起。
class _LastTestBadge extends StatefulWidget {
  const _LastTestBadge({required this.result, required this.nowMs});

  final LastTestResult result;
  final int nowMs;

  @override
  State<_LastTestBadge> createState() => _LastTestBadgeState();
}

class _LastTestBadgeState extends State<_LastTestBadge> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final lt = widget.result;
    final rel = relativeTimeShort(lt.createdAt, nowMs: widget.nowMs);
    final errorText = !lt.success && lt.error.isNotEmpty
        ? lt.error.substring(0, lt.error.length < 30 ? lt.error.length : 30)
        : '';
    final hasBody = lt.responseBody.trim().isNotEmpty;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        MiniBadge(
          text: [
            lt.success ? '✓' : '✗',
            if (lt.durationMs > 0) '${lt.durationMs}ms',
            if (rel.isNotEmpty) '· $rel',
            if (errorText.isNotEmpty) errorText,
            if (hasBody) _open ? '▾' : '▸',
          ].join(' '),
          color: lt.success ? theme.c.ok : theme.c.bad,
          onTap: hasBody ? () => setState(() => _open = !_open) : null,
          tooltip: lt.success
              ? t
                    .t('platform.lastTestOkHint')
                    .replaceAll('{{time}}', formatDateTime(lt.createdAt))
              : t
                    .t('platform.lastTestFailHint')
                    .replaceAll('{{time}}', formatDateTime(lt.createdAt))
                    .replaceAll(
                      '{{error}}',
                      lt.error.isEmpty ? '' : '\n${lt.error}',
                    ),
        ),
        if (_open && hasBody)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Container(
              padding: const EdgeInsets.symmetric(
                horizontal: AidogSpace.ssm,
                vertical: AidogSpace.sxs,
              ),
              decoration: BoxDecoration(
                color: theme.c.surface2,
                border: Border.all(color: theme.c.line),
                borderRadius: BorderRadius.circular(AidogRadius.sm),
              ),
              child: TestResultBody(body: lt.responseBody),
            ),
          ),
      ],
    );
  }
}

// ── 行 2：余额 / 预算 / 配额档 ────────────────────────────────────

class _BalanceRow extends StatelessWidget {
  const _BalanceRow({
    required this.platform,
    required this.quota,
    required this.budget,
    required this.usage,
    required this.hasCodingEndpoint,
    required this.rateLimit,
    required this.nowMs,
  });

  final PlatformRow platform;
  final QuotaDisplay quota;
  final ManualBudgetDisplay? budget;
  final UsageStats? usage;
  final bool hasCodingEndpoint;
  final RateLimitSnapshot? rateLimit;
  final int nowMs;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final p = platform;
    final q = quota;
    final items = <Widget>[];

    if (q.balanceRemaining != null) {
      final isAcu = q.currency == 'ACU';
      final level = usageLevelToColor(p.balanceLevel);
      items.add(
        SizedBox(
          width: 120,
          child: BalanceBar(
            remaining: q.balanceRemaining,
            total: q.balanceTotal,
            currency: isAcu ? '' : (q.currency == 'USD' ? '\$' : q.currency),
            level: level == ColorLevel.neutral ? null : level,
            label: isAcu ? t.t('platform.acuUsage') : null,
          ),
        ),
      );
    }

    final mb = budget;
    if (mb != null) {
      items.add(
        SizedBox(
          width: 120,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              if (mb.unit == 'usd')
                BalanceBar(remaining: mb.remaining, total: mb.amount)
              else
                _TokenBudgetBar(budget: mb),
              Text(
                '${mb.depleted ? t.t('platform.manualBudgetDepleted') : t.t('platform.manualBudgetLabel')}'
                '${mb.unit == 'token' ? ' · ${t.t('platform.manualBudgetTokenApprox')}' : ''}',
                style: AidogType.micro.copyWith(
                  color: mb.depleted ? theme.c.bad : theme.c.fg3,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ],
          ),
        ),
      );
    }

    // Coding plan：累计已用 tokens + 预估金额（折叠态亦可见）。
    final u = usage;
    if (hasCodingEndpoint && u != null) {
      items.add(
        Tooltip(
          message: t.t('platform.codingUsedHint'),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.bolt, size: 12, color: theme.c.fg2),
              Text(
                formatNumber(u.totalInputTokens + u.totalOutputTokens),
                style: AidogType.numSm.copyWith(
                  color: theme.c.fg2,
                  fontWeight: FontWeight.w700,
                ),
              ),
              Text(
                'tok',
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              ),
              const SizedBox(width: AidogSpace.ssm),
              Icon(Icons.attach_money, size: 12, color: theme.c.fg2),
              Text(
                formatCostUsd(u.totalCost),
                style: AidogType.numSm.copyWith(
                  color: theme.c.fg2,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ],
          ),
        ),
      );
    }

    // B3 折算行：本周期累计折算 $ + 手填套餐价。
    if (hasCodingEndpoint && p.codingWindowCost > 0) {
      final planPrice = parsePlanPrice(p.extra);
      items.add(
        Text(
          '${t.t('platform.codingWindowCost')} ${formatCostUsd(p.codingWindowCost)}'
          '${planPrice == null ? '' : ' · ${t.t('platform.codingPlanPrice')} ¥$planPrice/月'}',
          style: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
      );
    }

    // Coding plan 档位（紧凑态）。余额和档位互斥：有余额就不画档位。
    if (q.balanceRemaining == null && q.tiers.isNotEmpty) {
      items.add(
        ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 300),
          child: Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            children: [
              for (final tier in q.tiers)
                QuotaTierBlock(
                  tier: tier,
                  remainSuffix: t.t('platform.quotaRemainSuffix'),
                  nowMs: nowMs,
                ),
            ],
          ),
        ),
      );
    }

    // 上游速率限制余量：与套餐额度是两个维度，单列一个 chip 不混进 tiers。
    final rl = rateLimit;
    if (rl != null) {
      final ratio = rateLimitRatio(rl);
      final color = ratio == null
          ? theme.c.fg3
          : ratio < 0.1
          ? theme.c.bad
          : ratio < 0.3
          ? theme.c.peak
          : theme.c.fg3;
      final text = ratio != null
          ? formatPercent(ratio * 100, 0)
          : () {
              final rem = rl.requestsRemaining ?? rl.tokensRemaining;
              return rem == null ? '' : formatNumber(rem);
            }();
      items.add(
        Tooltip(
          message: t
              .t('platform.rateLimitTitle')
              .replaceAll('{{vendor}}', rl.vendor),
          child: Text(
            '${t.t('platform.rateLimit')} $text',
            style: AidogType.micro.copyWith(color: color),
          ),
        ),
      );
    }

    return Padding(
      padding: const EdgeInsetsDirectional.only(start: 24),
      child: Wrap(
        spacing: AidogSpace.smd,
        runSpacing: AidogSpace.ssm,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: items,
      ),
    );
  }
}

/// token / count 单位的手动预算条（usd 单位走 [BalanceBar]）。
class _TokenBudgetBar extends StatelessWidget {
  const _TokenBudgetBar({required this.budget});

  final ManualBudgetDisplay budget;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final mb = budget;
    final color = mb.depleted
        ? theme.c.bad
        : mb.ratio < 0.2
        ? theme.c.peak
        : theme.c.fg;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Text(
              formatNumber(mb.remaining < 0 ? 0 : mb.remaining),
              style: AidogType.numSm.copyWith(
                color: color,
                fontWeight: FontWeight.w700,
              ),
            ),
            Flexible(
              child: Text(
                ' / ${formatNumber(mb.amount)} '
                '${mb.unit == 'count' ? t.t('platform.manualBudgetUnitCountShort') : 'tok'}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AidogType.micro.copyWith(color: theme.c.fg3),
              ),
            ),
          ],
        ),
        const SizedBox(height: 3),
        ClipRRect(
          borderRadius: BorderRadius.circular(AidogRadius.sm),
          child: LinearProgressIndicator(
            value: clamp(mb.ratio, 0, 1),
            minHeight: 4,
            backgroundColor: theme.c.surface2,
            valueColor: AlwaysStoppedAnimation<Color>(
              mb.depleted
                  ? theme.c.bad
                  : mb.ratio < 0.2
                  ? theme.c.peak
                  : theme.c.ok,
            ),
          ),
        ),
      ],
    );
  }
}

// ── 展开区 ──────────────────────────────────────────────────────

class _DetailSection extends StatelessWidget {
  const _DetailSection({
    required this.platform,
    required this.meta,
    required this.quota,
    required this.usage,
    required this.usagePending,
    required this.configuredModels,
    required this.nowMs,
  });

  final PlatformRow platform;
  final ProtocolMetaTable meta;
  final QuotaDisplay quota;
  final UsageStats? usage;
  final bool usagePending;
  final List<String> configuredModels;
  final int nowMs;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final p = platform;
    final links = <(String, String)>[
      if ((meta.homepages[p.platformType] ?? '').isNotEmpty)
        (meta.homepages[p.platformType]!, t.t('platform.homepage')),
      if ((meta.docsUrls[p.platformType] ?? '').isNotEmpty)
        (meta.docsUrls[p.platformType]!, t.t('platform.sourceDocs')),
      if ((meta.pricingUrls[p.platformType] ?? '').isNotEmpty)
        (meta.pricingUrls[p.platformType]!, t.t('platform.sourcePricing')),
    ];
    final u = usage;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        // 品牌外链（registry homepage + source_urls.docs/pricing；未配置的不渲染）。
        if (links.isNotEmpty) ...[
          Wrap(
            spacing: AidogSpace.smd,
            runSpacing: AidogSpace.sxs,
            children: [
              for (final (href, label) in links)
                InkWell(
                  onTap: () => native.openUrl(href),
                  child: Tooltip(
                    message: href,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(
                          Icons.open_in_new,
                          size: 11,
                          color: theme.c.accentText,
                        ),
                        const SizedBox(width: 3),
                        Text(
                          label,
                          style: AidogType.micro.copyWith(
                            color: theme.c.accentText,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
            ],
          ),
          const SizedBox(height: AidogSpace.smd),
        ],
        // 已使用（总计）+ 今日。
        if (u != null) ...[
          _LabeledChips(
            label: t.t('platform.usageLabel'),
            chips: [
              StatChip(
                icon: Icons.bolt,
                value: formatNumber(u.totalInputTokens + u.totalOutputTokens),
                label: 'tokens',
              ),
              StatChip(
                icon: Icons.attach_money,
                value: formatCostUsd(u.totalCost),
                label: 'cost',
                level: costLevel(u.totalCost),
              ),
              StatChip(
                icon: Icons.check_circle_outline,
                value: formatPercent(
                  successRate(u.successCount, u.totalRequests),
                ),
                label: 'ok',
                level: successRateLevel(
                  successRate(u.successCount, u.totalRequests),
                  u.totalRequests,
                ),
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.ssm),
          _LabeledChips(
            label: t.t('platform.todayUsageLabel'),
            chips: [
              StatChip(
                icon: Icons.bolt,
                value: formatNumber(u.todayTokens),
                label: 'tokens',
              ),
              StatChip(
                icon: Icons.attach_money,
                value: formatCostUsd(u.todayCost),
                label: 'cost',
                level: costLevel(u.todayCost),
              ),
            ],
          ),
        ] else if (usagePending)
          _LabeledChips(
            label: t.t('platform.usageLabel'),
            chips: [
              SkeletonBox(
                width: 80,
                height: 24,
                semanticLabel: t.t('platform.usageLoading'),
              ),
              const SkeletonBox(width: 70, height: 24),
              const SkeletonBox(width: 60, height: 24),
            ],
          ),
        // 配额各档明细（展开态大号版）。
        if (quota.tiers.isNotEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          _LabeledChips(
            label: t.t('platform.quotaLabel'),
            chips: [
              for (final tier in quota.tiers)
                QuotaTierBlock(
                  tier: tier,
                  remainSuffix: t.t('platform.quotaRemainSuffix'),
                  expanded: true,
                  nowMs: nowMs,
                ),
            ],
          ),
        ],
        // 端点 badge（协议名 + Code 角标）。
        if (p.endpoints.isNotEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          _LabeledChips(
            label: t.t('platform.endpoints'),
            chips: [
              for (final ep in p.endpoints)
                MiniBadge(
                  text: ep.codingPlan
                      ? '${meta.label(ep.protocol)} Code'
                      : meta.label(ep.protocol),
                  color: ep.codingPlan ? theme.c.ok : theme.c.fg3,
                ),
            ],
          ),
        ],
        // 已配置模型 badge。
        if (configuredModels.isNotEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          _LabeledChips(
            label: t.t('platform.models'),
            chips: [
              for (final m in configuredModels)
                MiniBadge(text: m, color: theme.c.fg3),
            ],
          ),
        ],
      ],
    );
  }
}

/// 小标题 + 一行 chip（展开区各段共用的形状）。
class _LabeledChips extends StatelessWidget {
  const _LabeledChips({required this.label, required this.chips});

  final String label;
  final List<Widget> chips;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    mainAxisSize: MainAxisSize.min,
    children: [
      Text(
        label,
        style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg3),
      ),
      const SizedBox(height: AidogSpace.sxs),
      Wrap(
        spacing: AidogSpace.ssm,
        runSpacing: AidogSpace.sxs,
        children: chips,
      ),
    ],
  );
}
