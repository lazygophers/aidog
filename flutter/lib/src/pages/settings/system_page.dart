/// 系统设置页（`settings/system`）的 widget 层 —— 对齐 `src/pages/AppSettings.tsx`
/// 的五个 section：StartupSection / ProxyStatusSection / KernelSection /
/// LogSettingsSection / SystemMiscSection。
///
/// 状态与动作全在 [SystemSettingsController] / [KernelSettingsController]
/// （票 I08 已测），这里只画界面 + 破坏性操作的确认卡。
///
/// 🔴 「上游代理写失败不回滚本地值」是照搬 React 的（`system_logic.dart:376`），
/// 不是本层的疏漏。改它属于产品改动，单开票。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../../platform.dart' as native;
import '../../../utils/formatters.dart';
import '../../shell/app_shell.dart' show LiveDot;
import '../../shell/theme.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'notifications_logic.dart' show KernelSettingsController;
import 'system_logic.dart';

/// 保留期单位选择器的三个档。
const List<RetentionUnit> _kUnits = RetentionUnit.values;

/// 应用日志级别，与 `LogSettingsSection.tsx:284` 的数组同序。
const List<String> kAppLogLevels = ['trace', 'debug', 'info', 'warn', 'error'];

class SystemSettingsPage extends StatefulWidget {
  const SystemSettingsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.onLogSettingsChanged,
    this.appVersionFn,
  });

  final InvokeFn invoke;

  /// 日志总开关变化上抛给外壳（关掉日志时侧栏藏掉「日志」项）。
  final void Function(bool enabled)? onLogSettingsChanged;

  /// 取应用版本号（I12 的 `getAppVersion`）。测试塞假的，避免真读 bundle。
  final Future<String> Function()? appVersionFn;

  @override
  State<SystemSettingsPage> createState() => _SystemSettingsPageState();
}

/// 待确认的破坏性操作。
enum _Confirm { compactDb, clearLogs, cleanupExpired }

class _SystemSettingsPageState extends State<SystemSettingsPage> {
  late final SystemSettingsController _c;
  late final KernelSettingsController _k;
  _Confirm? _confirm;

  /// `proxy_log_cleanup_estimate` 的结果；null = 还在统计。
  Map<String, Object?>? _estimate;

  @override
  void initState() {
    super.initState();
    void rebuild() {
      if (mounted) setState(() {});
    }

    _c = SystemSettingsController(
      invoke: widget.invoke,
      onChanged: rebuild,
      onLogSettingsChanged: widget.onLogSettingsChanged,
      appVersionFn: widget.appVersionFn ?? native.getAppVersion,
    );
    _k = KernelSettingsController(invoke: widget.invoke, onChanged: rebuild);
    unawaited(_c.load());
    unawaited(_k.load());
    unawaited(_loadEstimate());
  }

  Future<void> _loadEstimate() async {
    try {
      final e = await _c.cleanupEstimate();
      if (mounted) setState(() => _estimate = e);
    } catch (_) {
      // 预估失败不挡页面：确认框退回「无预估数字」的那句文案。
    }
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return SettingsPageBody(
      title: t.t('appSettings.systemTab'),
      subtitle: _c.appVersion.isEmpty ? null : ltr('v${_c.appVersion}'),
      children: [
        _startup(t),
        _kernel(t),
        _timeouts(t),
        _upstreamProxy(t),
        _proxyLogs(t),
        _appLogs(t),
        _stats(t),
        _misc(t),
        if (_confirm != null) _confirmCard(t),
        if (_c.proxyStartError != null)
          ErrorNote(
            text: t.t(
              _c.proxyStartError!.kind == 'addr_in_use'
                  ? 'proxy.startFailedPortInUse'
                  : 'proxy.startFailedOther',
              {'port': _c.proxyStartError!.port},
            ),
          ),
        if (_c.message.isNotEmpty) ToastBar(text: _c.message, ok: true),
      ],
    );
  }

  // ── 启动与端口 ──────────────────────────────────────────

  Widget _startup(I18nController t) => SettingsCard(
    title: t.t('proxy.start'),
    children: [
      // 状态灯 + 状态字 + 运行中的监听地址（`ProxyStatusSection.tsx:29-53`）。
      // 原先只有卡片 meta 上一行「运行中 / 已停止」文字：没有状态灯，
      // 也看不到代理到底监听在哪个地址上。
      _ProxyStatusRow(running: _c.running, port: _c.proxyPort),
      Row(
        children: [
          Expanded(
            child: NumberRow(
              label: t.t('proxy.port'),
              value: _c.proxyPort,
              // 代理跑着时不许改端口（要先停）—— 改了也不会生效。
              onChanged: _c.running
                  ? null
                  : (v) => setState(() => _c.proxyPort = v),
            ),
          ),
          const SizedBox(width: AidogSpace.ssm),
          SmallButton(
            key: const ValueKey('proxy-toggle'),
            label: _c.running ? t.t('proxy.stop') : t.t('proxy.start'),
            onTap: () => _c.running
                ? _c.stopProxy(t.t('proxy.stopped'))
                : _c.startProxy(),
          ),
        ],
      ),
      SwitchRow(
        key: const ValueKey('proxy-autostart'),
        label: t.t('proxy.autostart'),
        description: t.t('proxy.autostartDesc'),
        value: _c.autostart,
        onChanged: _c.setAutostart,
      ),
      SwitchRow(
        key: const ValueKey('proxy-bindlan'),
        label: t.t('proxy.bindLan'),
        description:
            '${t.t('proxy.bindLanDesc')}\n${t.t('proxy.bindLanSecurity')}',
        value: _c.bindLan,
        onChanged: (v) => _c.setBindLan(v, t.t('proxy.bindLanApplied')),
      ),
      SwitchRow(
        key: const ValueKey('app-autolaunch'),
        label: t.t('proxy.autolaunch'),
        description: t.t('proxy.autolaunchDesc'),
        value: _c.autolaunch,
        onChanged: _c.setAutolaunch,
      ),
      // 静默启动只在开机自启开着时才有意义（关掉时逻辑层已强制置 false）。
      if (_c.autolaunch)
        SwitchRow(
          key: const ValueKey('app-silent-launch'),
          label: t.t('proxy.silentLaunch'),
          description: t.t('proxy.silentLaunchDesc'),
          value: _c.silentLaunch,
          onChanged: _c.setSilentLaunch,
        ),
    ],
  );

  // ── 内核管理面 ──────────────────────────────────────────

  Widget _kernel(I18nController t) {
    final s = _k.settings;
    // 还没读到就整段不渲染（React 的 `if (!settings) return null`）。
    if (s == null) return const SizedBox.shrink();
    return SettingsCard(
      title: t.t('kernel.loopbackOnly'),
      description:
          '${t.t('kernel.loopbackOnlyDesc')}\n'
          '${t.t('kernel.remoteAccess')}\n'
          '${t.t('kernel.notProxy')}',
      children: [
        InfoRow(label: t.t('proxy.port'), value: ltr('${s.port}')),
        TextRow(
          key: const ValueKey('kernel-token'),
          label: t.t('kernel.authToken'),
          description: t.t('kernel.authTokenDesc'),
          hint: t.t('kernel.authTokenPlaceholder'),
          // 🔴 这是内核管理面的访问令牌，拿到就能直连内核。React 是
          // `<input type="password">`（`KernelSection.tsx:72`），这边漏了遮挡，
          // 令牌明文显示在屏幕上，截图 / 投屏 / 旁人一眼就能看见。
          obscure: true,
          value: _k.token,
          onChanged: _k.setToken,
          onSubmitted: (_) => _k.commitToken(t.t('kernel.saved')),
        ),
        if (_k.error.isNotEmpty) ErrorNote(text: _k.error),
        if (_k.saved.isNotEmpty) ToastBar(text: _k.saved, ok: true),
      ],
    );
  }

  // ── 超时 ────────────────────────────────────────────────

  Widget _timeouts(I18nController t) => SettingsCard(
    title: t.t('proxy.timeout'),
    description: t.t('proxy.timeoutDesc'),
    children: [
      NumberRow(
        key: const ValueKey('req-timeout'),
        label: '${t.t('proxy.requestTimeout')} (${t.t('unit.sec')})',
        value: _c.reqTimeout,
        onChanged: (v) => _c.setTimeouts(v, _c.connTimeout),
      ),
      NumberRow(
        key: const ValueKey('conn-timeout'),
        label: '${t.t('proxy.connectTimeout')} (${t.t('unit.sec')})',
        value: _c.connTimeout,
        onChanged: (v) => _c.setTimeouts(_c.reqTimeout, v),
      ),
    ],
  );

  // ── 上游代理 ────────────────────────────────────────────

  Widget _upstreamProxy(I18nController t) {
    final p = _c.proxyClient;
    return SettingsCard(
      title: t.t('proxy.upstreamProxy'),
      description: t.t('proxy.upstreamProxyDesc'),
      children: [
        SwitchRow(
          key: const ValueKey('upstream-proxy-enabled'),
          label: t.t('proxy.upstreamProxy'),
          value: p.enabled,
          onChanged: (v) => _c.updateProxyClient(p.copyWith(enabled: v)),
        ),
        if (p.enabled) ...[
          ChoiceRow(
            label: t.t('proxy.proxyType'),
            // 三个协议，少一个 HTTPS 就没法接走 HTTPS 的上游代理
            // （`ProxyStatusSection.tsx:142-144`）。
            options: const ['socks5', 'http', 'https'],
            value: p.proxyType,
            onChanged: (v) => _c.updateProxyClient(p.copyWith(proxyType: v)),
          ),
          TextRow(
            label: t.t('proxy.proxyHost'),
            value: p.host,
            onSubmitted: (v) =>
                _c.updateProxyClient(p.copyWith(host: v.trim())),
          ),
          NumberRow(
            label: t.t('proxy.proxyPort'),
            value: p.port,
            onChanged: (v) => _c.updateProxyClient(p.copyWith(port: v)),
          ),
          TextRow(
            label: t.t('proxy.proxyUser'),
            hint: t.t('proxy.proxyUserPlaceholder'),
            value: p.username,
            onSubmitted: (v) => _c.updateProxyClient(p.copyWith(username: v)),
          ),
          TextRow(
            label: t.t('proxy.proxyPass'),
            hint: t.t('proxy.proxyPassPlaceholder'),
            value: p.password,
            obscure: true,
            onSubmitted: (v) => _c.updateProxyClient(p.copyWith(password: v)),
          ),
          // 只有 SOCKS5 才谈得上「DNS 走代理」（`ProxyStatusSection.tsx:195`）：
          // HTTP / HTTPS 代理本来就是把域名整个交给代理去解析，这个开关在那两种
          // 模式下摆出来只会让人以为它起作用。
          if (p.proxyType == 'socks5')
            SwitchRow(
              label: t.t('proxy.dnsOverProxy'),
              description: t.t('proxy.dnsOverProxyDesc'),
              value: p.dnsOverProxy,
              onChanged: (v) =>
                  _c.updateProxyClient(p.copyWith(dnsOverProxy: v)),
            ),
          TextRow(
            label: t.t('proxy.noProxy'),
            description: t.t('proxy.noProxyDesc'),
            hint: t.t('proxy.noProxyPlaceholder'),
            value: p.noProxy,
            maxLines: 2,
            onSubmitted: (v) => _c.updateProxyClient(p.copyWith(noProxy: v)),
          ),
        ],
      ],
    );
  }

  // ── 代理日志三级开关 ────────────────────────────────────

  Widget _proxyLogs(I18nController t) => SettingsCard(
    title: t.t('proxy.logRequests'),
    description: t.t('proxy.logRequestsDesc'),
    children: [
      SwitchRow(
        key: const ValueKey('log-enabled'),
        label: t.t('proxy.logRequests'),
        value: _c.logEnabled,
        onChanged: _c.setLogEnabled,
      ),
      if (_c.logEnabled) ...[
        SwitchRow(
          key: const ValueKey('log-user-req'),
          label: t.t('proxy.logUserReq'),
          description: t.t('proxy.logUserReqDesc'),
          value: _c.logUserReq,
          onChanged: (v) {
            setState(() => _c.logUserReq = v);
            _c.updateLogSettings({'log_user_request': v});
          },
        ),
        // 保留期跟着它那一类的开关走（`LogSettingsSection.tsx:145`）：
        // 关掉这类记录后，它的保留期还摆着可改，是改了也没用的旋钮。
        if (_c.logUserReq)
          _retentionRow(
            t,
            keyId: 'user-req-retention',
            label: t.t('proxy.userReqRetention'),
            days: _c.userReqRetention,
            unit: _c.userReqRetentionUnit,
            onDays: (v) {
              setState(() => _c.userReqRetention = v);
              _c.updateLogSettings({'user_request_retention_days': v});
            },
            onUnit: (u) {
              setState(() => _c.userReqRetentionUnit = u);
              _c.updateLogSettings({'user_request_retention_unit': u.wire});
            },
          ),
        SwitchRow(
          key: const ValueKey('log-upstream-req'),
          label: t.t('proxy.logUpstreamReq'),
          description: t.t('proxy.logUpstreamReqDesc'),
          value: _c.logUpstreamReq,
          onChanged: (v) {
            setState(() => _c.logUpstreamReq = v);
            _c.updateLogSettings({'log_upstream_request': v});
          },
        ),
        // 同上，`LogSettingsSection.tsx:166`。
        if (_c.logUpstreamReq)
          _retentionRow(
            t,
            keyId: 'upstream-req-retention',
            label: t.t('proxy.upstreamReqRetention'),
            days: _c.upstreamReqRetention,
            unit: _c.upstreamReqRetentionUnit,
            onDays: (v) {
              setState(() => _c.upstreamReqRetention = v);
              _c.updateLogSettings({'upstream_request_retention_days': v});
            },
            onUnit: (u) {
              setState(() => _c.upstreamReqRetentionUnit = u);
              _c.updateLogSettings({'upstream_request_retention_unit': u.wire});
            },
          ),
        _retentionRow(
          t,
          keyId: 'log-retention',
          label: t.t('proxy.logRetention'),
          description: t.t('proxy.logRetentionHint'),
          days: _c.logRetention,
          unit: _c.logRetentionUnit,
          onDays: (v) {
            setState(() => _c.logRetention = v);
            _c.updateLogSettings({'retention_days': v});
          },
          onUnit: (u) {
            setState(() => _c.logRetentionUnit = u);
            _c.updateLogSettings({'retention_unit': u.wire});
          },
        ),
      ],
      // 🔴 清理按钮**独立于记录开关**，不能包进 `if (_c.logEnabled)`。
      // React 在 `LogSettingsSection.tsx:210` 特意写了注释说明理由：
      // 「关闭记录后仍需可清已存日志」。包进去的后果是关掉日志记录以后，
      // 库里已经攒下的旧日志再也清不掉 —— 越想省空间越清不了。
      const SizedBox(height: AidogSpace.ssm),
      Row(
        children: [
          // 永久保留（0）时没有过期日志可清。按钮置灰之外还要说清**为什么**
          //（`LogSettingsSection.tsx:216` 的 title），否则用户只看到一个点不动的按钮。
          Tooltip(
            message: _c.logRetention == 0
                ? t.t('logs.cleanupDisabledHint')
                : '',
            child: SmallButton(
              key: const ValueKey('cleanup-expired'),
              // 清理跑起来要几秒，按钮上要看得出来（`LogSettingsSection.tsx:219`）。
              label: _c.logMaintBusy
                  ? t.t('logs.cleaning')
                  : t.t('logs.cleanupExpired'),
              onTap: (_c.logRetention == 0 || _c.logMaintBusy)
                  ? null
                  : () => setState(() => _confirm = _Confirm.cleanupExpired),
            ),
          ),
          const SizedBox(width: AidogSpace.ssm),
          SmallButton(
            key: const ValueKey('clear-logs'),
            label: _c.logMaintBusy
                ? t.t('logs.cleaning')
                : t.t('logs.clear'),
            danger: true,
            onTap: _c.logMaintBusy
                ? null
                : () => setState(() => _confirm = _Confirm.clearLogs),
          ),
        ],
      ),
      // 预估三个数：超期行数 / 可回收 body 体积 / 库总大小。按钮下面常驻。
      Text(
        _estimate == null
            ? t.t('logs.cleanupEstimateLoading')
            : t.t('logs.cleanupEstimate', {
                'rows': formatNumber((_estimate!['overdue_rows'] as num?) ?? 0),
                'bytes': formatBytes(
                  (_estimate!['overdue_body_bytes'] as num?) ?? 0,
                ),
                'size': formatBytes((_estimate!['db_size_bytes'] as num?) ?? 0),
              }),
        style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg3),
      ),
    ],
  );

  Widget _retentionRow(
    I18nController t, {
    required String keyId,
    required String label,
    String? description,
    required int days,
    required RetentionUnit unit,
    required ValueChanged<int> onDays,
    required ValueChanged<RetentionUnit> onUnit,
  }) => Column(
    key: ValueKey(keyId),
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      NumberRow(
        label: label,
        description: description,
        value: days,
        onChanged: (v) => onDays(v < 0 ? 0 : v),
      ),
      // 0 = 永久保留：这时单位（小时/天）没有意义，React 把整组单位藏掉、
      // 换成一行「永久保留」（`LogSettingsSection.tsx:155-162`）。
      // 原先把「永久保留」并进标签括号里，单位按钮组仍摆着可选 —— 选了也没用。
      if (days == 0)
        Text(
          t.t('proxy.logRetentionForever'),
          style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg3),
        )
      else
        ChoiceRow(
          label: '',
          options: _kUnits.map((e) => e.wire).toList(),
          value: unit.wire,
          labelOf: (o) => t.t('unit.$o'),
          onChanged: (v) => onUnit(RetentionUnit.parse(v)),
        ),
    ],
  );

  // ── 应用日志文件 ────────────────────────────────────────

  Widget _appLogs(I18nController t) => SettingsCard(
    title: t.t('appLog.title'),
    description: t.t('appLog.desc'),
    children: [
      SwitchRow(
        key: const ValueKey('app-log-file'),
        label: t.t('appLog.title'),
        value: _c.logFileEnabled,
        onChanged: (v) => _c.updateAppLogSettings(fileEnabled: v),
      ),
      if (_c.logFileEnabled) ...[
        ChoiceRow(
          label: t.t('appLog.level'),
          options: kAppLogLevels,
          value: _c.logLevel,
          labelOf: (o) => o.toUpperCase(),
          onChanged: (v) => _c.updateAppLogSettings(level: v),
        ),
        NumberRow(
          label: _c.logRetHours == 0
              ? '${t.t('appLog.retention')} (${t.t('appLog.retentionForever')})'
              : '${t.t('appLog.retention')} (${t.t('appLog.retentionUnit')})',
          value: _c.logRetHours,
          onChanged: (v) =>
              _c.updateAppLogSettings(retentionHours: v < 0 ? 0 : v),
        ),
      ],
    ],
  );

  // ── 聚合统计 ────────────────────────────────────────────

  Widget _stats(I18nController t) => SettingsCard(
    title: t.t('stats.aggSettings'),
    description: t.t('stats.aggSettingsHint'),
    children: [
      NumberRow(
        key: const ValueKey('stats-retention'),
        label: _c.statsRetention == 0
            ? '${t.t('stats.aggRetention')} (${t.t('proxy.logRetentionForever')})'
            : '${t.t('stats.aggRetention')} (${t.t('unit.days')})',
        value: _c.statsRetention,
        onChanged: (v) => _c.setStatsRetention(v < 0 ? 0 : v),
      ),
      Align(
        alignment: AlignmentDirectional.centerStart,
        child: SmallButton(
          key: const ValueKey('stats-rebuild'),
          label: _c.statsRebuilding
              ? t.t('common.loading')
              : t.t('stats.rebuild'),
          onTap: _c.statsRebuilding
              ? null
              : () => _c.rebuildStats(t.t('stats.rebuildDone')),
        ),
      ),
      Text(
        t.t('stats.rebuildHint'),
        style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg3),
      ),
    ],
  );

  // ── 杂项：内置工具兼容 / 压缩数据库 / 自动更新 ──────────

  Widget _misc(I18nController t) => SettingsCard(
    children: [
      SwitchRow(
        key: const ValueKey('btc-global'),
        label: t.t('proxy.btcGlobal'),
        description: t.t('proxy.btcGlobalDesc'),
        value: _c.btcGlobalEnabled,
        onChanged: _c.setBtcGlobal,
      ),
      SwitchRow(
        key: const ValueKey('auto-update'),
        label: t.t('settings.autoUpdate'),
        description: t.t('settings.autoUpdateHint'),
        value: _c.autoUpdateEnabled,
        onChanged: _c.setAutoUpdate,
      ),
      const SizedBox(height: AidogSpace.ssm),
      Align(
        alignment: AlignmentDirectional.centerStart,
        child: SmallButton(
          key: const ValueKey('db-compact'),
          label: _c.dbCompacting
              ? t.t('common.loading')
              : t.t('settings.dbCompact'),
          onTap: _c.dbCompacting
              ? null
              : () => setState(() => _confirm = _Confirm.compactDb),
        ),
      ),
      Text(
        t.t('settings.dbCompactHint'),
        style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg3),
      ),
      InfoRow(label: t.t('app.version'), value: ltr(_c.appVersion)),
    ],
  );

  // ── 破坏性操作的确认卡 ──────────────────────────────────

  Widget _confirmCard(I18nController t) {
    final e = _estimate;
    return switch (_confirm!) {
      _Confirm.compactDb => ConfirmCard(
        title: t.t('settings.dbCompact'),
        body: t.t('settings.dbCompactHint'),
        confirmLabel: _c.dbCompacting
            ? t.t('common.loading')
            : t.t('action.confirm'),
        // 执行中把卡留在屏幕上并变灰：原先是先关卡再跑命令，命令还在跑
        // 但界面上什么都没有了（React 那边一直开着显示「清理中...」）。
        busy: _c.dbCompacting,
        onCancel: () => setState(() => _confirm = null),
        onConfirm: () async {
          await _c.compactDb(
            (before, after, pct) => t.t('settings.dbCompactDone', {
              'before': before,
              'after': after,
              'pct': pct,
            }),
          );
          if (mounted) setState(() => _confirm = null);
        },
      ),
      _Confirm.clearLogs => ConfirmCard(
        title: t.t('logs.clearConfirmTitle'),
        body: t.t('logs.clearConfirm'),
        confirmLabel: _c.logMaintBusy
            ? t.t('logs.cleaning')
            : t.t('logs.clear'),
        busy: _c.logMaintBusy,
        onCancel: () => setState(() => _confirm = null),
        onConfirm: () async {
          await _c.clearLogs();
          if (!mounted) return;
          setState(() {
            _confirm = null;
            _c.message = t.t('logs.clearDone');
          });
          await _loadEstimate();
        },
      ),
      _Confirm.cleanupExpired => ConfirmCard(
        title: t.t('logs.cleanupConfirmTitle'),
        body: e == null
            ? t.t('logs.cleanupConfirmNoEstimate')
            : t.t('logs.cleanupConfirm', {
                'rows': formatNumber((e['overdue_rows'] as num?) ?? 0),
                'bytes': formatBytes((e['overdue_body_bytes'] as num?) ?? 0),
                'size': formatBytes((e['db_size_bytes'] as num?) ?? 0),
              }),
        confirmLabel: _c.logMaintBusy
            ? t.t('logs.cleaning')
            : t.t('logs.cleanupExpired'),
        busy: _c.logMaintBusy,
        onCancel: () => setState(() => _confirm = null),
        onConfirm: () async {
          await _c.cleanupExpired();
          if (!mounted) return;
          setState(() {
            _confirm = null;
            _c.message = t.t('logs.cleanupExpiredDone');
          });
          await _loadEstimate();
        },
      ),
    };
  }
}

/// 代理状态行：44px 圆形状态灯 + 状态字 + 运行中的 `localhost:<port>`。
/// 对齐 `src/pages/AppSettings/ProxyStatusSection.tsx:29-53`。
class _ProxyStatusRow extends StatelessWidget {
  const _ProxyStatusRow({required this.running, required this.port});

  final bool running;
  final int port;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Row(
        children: [
          Container(
            width: 44,
            height: 44,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: running ? theme.c.liveFill : theme.c.surface2,
              border: Border.all(
                color: running ? theme.c.liveEdge : theme.c.line,
              ),
              // 「只有活着的东西才发光」：停着的时候没有外发光。
              boxShadow: running ? theme.liveHalo : const [],
            ),
            child: LiveDot(on: running, size: 16),
          ),
          const SizedBox(width: AidogSpace.smd),
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                running ? t.t('proxy.running') : t.t('proxy.stopped'),
                style: AidogType.label.copyWith(
                  color: theme.c.fg,
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (running)
                Text(
                  // 地址是标识串，RTL 下不该被重排。
                  ltr('localhost:$port'),
                  style: AidogType.numSm.copyWith(color: theme.c.fg2),
                ),
            ],
          ),
        ],
      ),
    );
  }
}
