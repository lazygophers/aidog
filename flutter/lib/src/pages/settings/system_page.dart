/// 系统设置页（`settings/system`）的 widget 层 —— 对齐 `src/pages/AppSettings.tsx`
/// 的五个 section：StartupSection / ProxyStatusSection / KernelSection /
/// LogSettingsSection / SystemMiscSection。
///
/// 状态与动作全在 [SystemSettingsController] / [KernelSettingsController]
/// （票 I08 已测），这里只画界面 + 破坏性操作的确认弹窗。
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
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'notifications_logic.dart' show KernelSettingsController;
import 'system_logic.dart';

/// 卡与卡之间的间距：React System tab 容器 `gap: 20`
/// （`src/pages/AppSettings.tsx:71`），不是 `AidogSpace.sxl` 的 18。
const double _cardGap = 20;

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
      onLogSettingsChanged: widget.onLogSettingsChanged,
      appVersionFn: widget.appVersionFn ?? native.getAppVersion,
      onChanged: rebuild,
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
      children: [
        _proxyStatus(t),
        // 启动失败错误条紧跟状态卡之下（`ProxyStatusSection.tsx:80-101`），
        // 不再沉到页尾。
        if (_c.proxyStartError != null) _startError(t),
        ..._startup(t),
        _upstreamProxy(t),
        _kernel(t),
        _timeouts(t),
        _btcGlobal(t),
        _proxyLogs(t),
        _appLogs(t),
        _dbCompact(t),
        _stats(t),
        // 提示条是页面流里的 `.toast` 方条，排在 `VersionToastSection` **顶部**
        //（`SystemMiscSection.tsx:211`），不是浮在窗口顶部的胶囊。
        if (_c.message.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: _cardGap),
            child: InlineNote(
              text: _c.message,
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            ),
          ),
        _autoUpdate(t),
        _version(t),
        if (_confirm != null) _confirmCard(t),
      ],
    );
  }

  // ── 代理状态 hero 卡（`ProxyStatusSection.tsx:20-76`）──────────

  Widget _proxyStatus(I18nController t) {
    final theme = AidogTheme.of(context);
    return Padding(
      key: const ValueKey('proxy-status-card'),
      padding: const EdgeInsets.only(bottom: _cardGap),
      child: Tile(
        // React 是 24/20 的 hero 卡，比普通开关卡的 16/20 高一档。
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 24),
        child: Row(
          children: [
            Container(
              width: 44,
              height: 44,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: _c.running ? theme.c.liveFill : theme.c.surface2,
                border: Border.all(
                  color: _c.running ? theme.c.liveEdge : theme.c.line,
                ),
                // 「只有活着的东西才发光」：停着的时候没有外发光。
                boxShadow: _c.running ? theme.liveHalo : const [],
              ),
              child: LiveDot(on: _c.running, size: 16),
            ),
            const SizedBox(width: 20),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    _c.running ? t.t('proxy.running') : t.t('proxy.stopped'),
                    // React 状态字 14 w700（`ProxyStatusSection.tsx:45`）。
                    style: AidogType.label.copyWith(
                      fontSize: 14,
                      fontWeight: FontWeight.w700,
                      color: theme.c.fg,
                    ),
                  ),
                  if (_c.running)
                    Text(
                      // 地址是标识串，RTL 下不该被重排。
                      ltr('localhost:${_c.proxyPort}'),
                      style: AidogType.caption.copyWith(
                        fontSize: 12,
                        color: theme.c.fg2,
                      ),
                    ),
                ],
              ),
            ),
            const SizedBox(width: AidogSpace.smd),
            // 端口输入 + 启停按钮在状态行右侧同排，不另起一行。
            InlineRow(
              label: t.t('proxy.port'),
              child: NumberInput(
                key: const ValueKey('proxy-port'),
                value: '${_c.proxyPort}',
                // 代理跑着时不许改端口（要先停）—— 改了也不会生效。
                onChanged: _c.running
                    ? null
                    : (v) =>
                          setState(() => _c.proxyPort = int.tryParse(v) ?? 0),
                width: 80,
              ),
            ),
            const SizedBox(width: AidogSpace.ssm),
            SmallButton(
              key: const ValueKey('proxy-toggle'),
              // 启动是默认变体（实心），停止是 destructive（实心红）
              //（`ProxyStatusSection.tsx:67,71`）。
              filled: true,
              danger: _c.running,
              label: _c.running ? t.t('proxy.stop') : t.t('proxy.start'),
              onTap: () => _c.running
                  ? _c.stopProxy(t.t('proxy.stopped'))
                  : _c.startProxy(),
            ),
          ],
        ),
      ),
    );
  }

  /// 启动失败错误条：danger 边 + 底、13px、右端「重试」outline
  /// （`ProxyStatusSection.tsx:80-101`）。
  Widget _startError(I18nController t) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: _cardGap),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        decoration: BoxDecoration(
          color: theme.c.surface,
          border: Border.all(color: theme.c.bad),
          borderRadius: BorderRadius.circular(AidogRadius.md),
        ),
        child: Row(
          children: [
            Expanded(
              child: Text(
                t.t(
                  _c.proxyStartError!.kind == 'addr_in_use'
                      ? 'proxy.startFailedPortInUse'
                      : 'proxy.startFailedOther',
                  {'port': _c.proxyStartError!.port},
                ),
                style: AidogType.label.copyWith(
                  fontSize: 13,
                  color: theme.c.bad,
                ),
              ),
            ),
            SmallButton(
              key: const ValueKey('proxy-retry'),
              label: t.t('proxy.retry'),
              onTap: () => _c.startProxy(),
            ),
          ],
        ),
      ),
    );
  }

  // ── 启动四开关：一开关一卡（`StartupSection.tsx:19-85`）─────────

  List<Widget> _startup(I18nController t) => [
    ToggleCard(
      bottomGap: _cardGap,
      key: const ValueKey('proxy-autostart'),
      label: t.t('proxy.autostart'),
      descriptions: [t.t('proxy.autostartDesc')],
      value: _c.autostart,
      onChanged: _c.setAutostart,
    ),
    ToggleCard(
      bottomGap: _cardGap,
      key: const ValueKey('proxy-bindlan'),
      label: t.t('proxy.bindLan'),
      descriptions: [t.t('proxy.bindLanDesc'), t.t('proxy.bindLanSecurity')],
      value: _c.bindLan,
      onChanged: (v) => _c.setBindLan(v, t.t('proxy.bindLanApplied')),
    ),
    ToggleCard(
      bottomGap: _cardGap,
      key: const ValueKey('app-autolaunch'),
      label: t.t('proxy.autolaunch'),
      descriptions: [t.t('proxy.autolaunchDesc')],
      value: _c.autolaunch,
      onChanged: _c.setAutolaunch,
    ),
    // 静默启动只在开机自启开着时才有意义（关掉时逻辑层已强制置 false）。
    if (_c.autolaunch)
      ToggleCard(
        bottomGap: _cardGap,
        key: const ValueKey('app-silent-launch'),
        label: t.t('proxy.silentLaunch'),
        descriptions: [t.t('proxy.silentLaunchDesc')],
        value: _c.silentLaunch,
        onChanged: _c.setSilentLaunch,
      ),
  ];

  // ── 内核管理面（`KernelSection.tsx`：说明卡 + 令牌卡两张）─────────

  Widget _kernel(I18nController t) {
    final s = _k.settings;
    // 还没读到就整段不渲染（React 的 `if (!settings) return null`）。
    if (s == null) return const SizedBox.shrink();
    final theme = AidogTheme.of(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: _cardGap),
          child: Tile(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(t.t('kernel.loopbackOnly'), style: _titleStyle(theme)),
                _desc(t.t('kernel.loopbackOnlyDesc')),
                _desc(t.t('kernel.remoteAccess')),
                _desc(t.t('kernel.notProxy')),
              ],
            ),
          ),
        ),
        Padding(
          padding: const EdgeInsets.only(bottom: _cardGap),
          child: Tile(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(t.t('kernel.authToken'), style: _titleStyle(theme)),
                _desc(t.t('kernel.authTokenDesc')),
                Padding(
                  padding: const EdgeInsets.only(top: AidogSpace.smd),
                  child: TextRow(
                    key: const ValueKey('kernel-token'),
                    label: '',
                    // 🔴 内核管理面的访问令牌，拿到就能直连内核。React 是
                    // `<input type="password">`（`KernelSection.tsx:72`），
                    // 无明文切换 —— 令牌永远遮挡显示。
                    obscure: true,
                    value: _k.token,
                    onChanged: _k.setToken,
                    onSubmitted: (_) => _k.commitToken(t.t('kernel.saved')),
                    hint: t.t('kernel.authTokenPlaceholder'),
                  ),
                ),
                if (_k.error.isNotEmpty) ErrorNote(text: _k.error),
                if (_k.saved.isNotEmpty) ToastBar(text: _k.saved, ok: true),
              ],
            ),
          ),
        ),
      ],
    );
  }

  // ── 超时（`SystemMiscSection.tsx:38-65`：两组横排同一行）──────────

  Widget _timeouts(I18nController t) => HeaderCard(
    bottomGap: _cardGap,
    title: t.t('proxy.timeout'),
    descriptions: [t.t('proxy.timeoutDesc')],
    // 说明是卡里的**独立子元素**，与标题隔卡的 `gap: 12`，不是贴着标题的
    // 副标题（`SystemMiscSection.tsx:28-37`）。
    descGap: 12,
    child: Padding(
      // `gap: 12` + `marginTop: 4` = 16（`SystemMiscSection.tsx:38`）。
      padding: const EdgeInsets.only(top: 16),
      child: Wrap(
        spacing: 16,
        runSpacing: AidogSpace.ssm,
        children: [
          InlineRow(
            label: t.t('proxy.requestTimeout'),
            unit: t.t('unit.sec'),
            child: NumberInput(
              key: const ValueKey('req-timeout'),
              value: '${_c.reqTimeout}',
              onChanged: (v) =>
                  _c.setTimeouts(int.tryParse(v) ?? 0, _c.connTimeout),
              width: 80,
              // 超时两格是 `<Input>` 裸默认 = shadcn `h-9`(36) / `text-sm`(14)
              //（`SystemMiscSection.tsx:43-49,56-62`）。
              height: 36,
              fontSize: 14,
            ),
          ),
          InlineRow(
            label: t.t('proxy.connectTimeout'),
            unit: t.t('unit.sec'),
            child: NumberInput(
              key: const ValueKey('conn-timeout'),
              value: '${_c.connTimeout}',
              onChanged: (v) =>
                  _c.setTimeouts(_c.reqTimeout, int.tryParse(v) ?? 0),
              width: 80,
              height: 36,
              fontSize: 14,
            ),
          ),
        ],
      ),
    ),
  );

  // ── 上游代理（`ProxyStatusSection.tsx:109-220` 的 UpstreamProxySection）──

  Widget _upstreamProxy(I18nController t) {
    final p = _c.proxyClient;
    return HeaderCard(
      bottomGap: _cardGap,
      title: t.t('proxy.upstreamProxy'),
      descriptions: [t.t('proxy.upstreamProxyDesc')],
      trailing: AidogSwitch(
        key: const ValueKey('upstream-proxy-enabled'),
        value: p.enabled,
        compact: true,
        onChanged: () => _c.updateProxyClient(p.copyWith(enabled: !p.enabled)),
      ),
      child: p.enabled
          ? Padding(
              // 展开区：线上 12（卡的 `gap: 12`）/ 线下 8（`paddingTop: 8`），
              // 对齐 `LogSettingsSection.tsx:122`；原先是上 8 / 下 10。
              padding: const EdgeInsets.only(top: 12),
              child: Container(
                // 展开区与页头之间一道上边框（React `borderTop`）。
                decoration: BoxDecoration(
                  border: Border(
                    top: BorderSide(color: AidogTheme.of(context).c.line),
                  ),
                ),
                child: Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Wrap(
                        spacing: 12,
                        runSpacing: AidogSpace.ssm,
                        children: [
                          InlineRow(
                            label: t.t('proxy.proxyType'),
                            child: InlineSelect<String>(
                              key: const ValueKey('upstream-proxy-type'),
                              // 三个协议，少一个 HTTPS 就没法接走 HTTPS 的上游代理
                              //（`ProxyStatusSection.tsx:142-144`）。
                              value: p.proxyType,
                              options: const ['socks5', 'http', 'https'],
                              width: 100,
                              onChanged: (v) => _c.updateProxyClient(
                                p.copyWith(proxyType: v!),
                              ),
                            ),
                          ),
                          InlineRow(
                            label: t.t('proxy.proxyHost'),
                            child: SizedBox(
                              width: 120,
                              child: PlainTextField(
                                value: p.host,
                                onSubmitted: (v) => _c.updateProxyClient(
                                  p.copyWith(host: v.trim()),
                                ),
                              ),
                            ),
                          ),
                          InlineRow(
                            label: t.t('proxy.proxyPort'),
                            child: NumberInput(
                              value: '${p.port}',
                              onChanged: (v) => _c.updateProxyClient(
                                p.copyWith(port: int.tryParse(v) ?? 0),
                              ),
                              width: 70,
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: AidogSpace.ssm),
                      Wrap(
                        spacing: 12,
                        runSpacing: AidogSpace.ssm,
                        children: [
                          InlineRow(
                            label: t.t('proxy.proxyUser'),
                            child: SizedBox(
                              width: 100,
                              child: PlainTextField(
                                value: p.username,
                                hint: t.t('proxy.proxyUserPlaceholder'),
                                onSubmitted: (v) => _c.updateProxyClient(
                                  p.copyWith(username: v),
                                ),
                              ),
                            ),
                          ),
                          InlineRow(
                            label: t.t('proxy.proxyPass'),
                            child: SizedBox(
                              width: 100,
                              child: PlainTextField(
                                key: const ValueKey('upstream-proxy-pass'),
                                value: p.password,
                                hint: t.t('proxy.proxyPassPlaceholder'),
                                // React 是 `<input type="password">`
                                //（`ProxyStatusSection.tsx:187`），无明文切换。
                                obscure: true,
                                onSubmitted: (v) => _c.updateProxyClient(
                                  p.copyWith(password: v),
                                ),
                              ),
                            ),
                          ),
                        ],
                      ),
                      // 只有 SOCKS5 才谈得上「DNS 走代理」
                      //（`ProxyStatusSection.tsx:195`）：HTTP / HTTPS 代理本来
                      // 就是把域名整个交给代理去解析，这个开关在那两种模式下
                      // 摆出来只会让人以为它起作用。
                      if (p.proxyType == 'socks5')
                        Padding(
                          padding: const EdgeInsets.only(top: AidogSpace.smd),
                          child: Row(
                            children: [
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    Text(
                                      t.t('proxy.dnsOverProxy'),
                                      style: AidogType.caption.copyWith(
                                        fontSize: 12,
                                        fontWeight: FontWeight.w600,
                                        color: AidogTheme.of(context).c.fg,
                                      ),
                                    ),
                                    Text(
                                      t.t('proxy.dnsOverProxyDesc'),
                                      style: AidogType.caption.copyWith(
                                        fontSize: 11,
                                        color: AidogTheme.of(context).c.fg3,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                              AidogSwitch(
                                value: p.dnsOverProxy,
                                compact: true,
                                onChanged: () => _c.updateProxyClient(
                                  p.copyWith(dnsOverProxy: !p.dnsOverProxy),
                                ),
                              ),
                            ],
                          ),
                        ),
                      Padding(
                        padding: const EdgeInsets.only(top: AidogSpace.smd),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Text(
                              t.t('proxy.noProxy'),
                              style: AidogType.caption.copyWith(
                                fontSize: 12,
                                color: AidogTheme.of(context).c.fg2,
                              ),
                            ),
                            PlainTextField(
                              value: p.noProxy,
                              hint: t.t('proxy.noProxyPlaceholder'),
                              onSubmitted: (v) =>
                                  _c.updateProxyClient(p.copyWith(noProxy: v)),
                            ),
                            Text(
                              t.t('proxy.noProxyDesc'),
                              style: AidogType.caption.copyWith(
                                fontSize: 11,
                                color: AidogTheme.of(context).c.fg3,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            )
          : const SizedBox.shrink(),
    );
  }

  // ── 代理日志三级开关（`LogSettingsSection.tsx:101-238`）──────────

  Widget _proxyLogs(I18nController t) {
    final theme = AidogTheme.of(context);
    return HeaderCard(
      bottomGap: _cardGap,
      title: t.t('proxy.logRequests'),
      descriptions: [t.t('proxy.logRequestsDesc')],
      trailing: AidogSwitch(
        key: const ValueKey('log-enabled'),
        value: _c.logEnabled,
        compact: true,
        onChanged: () => _c.setLogEnabled(!_c.logEnabled),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          if (_c.logEnabled) ...[
            _sectionDivider(),
            // 子开关：12 w600 标题 + 11 tertiary 描述
            //（`LogSettingsSection.tsx:124-149`）。
            _subSwitch(
              key: const ValueKey('log-user-req'),
              label: t.t('proxy.logUserReq'),
              description: t.t('proxy.logUserReqDesc'),
              value: _c.logUserReq,
              onChanged: (v) {
                setState(() => _c.logUserReq = v);
                _c.updateLogSettings({'log_user_request': v});
              },
            ),
            _subSwitch(
              key: const ValueKey('log-upstream-req'),
              label: t.t('proxy.logUpstreamReq'),
              description: t.t('proxy.logUpstreamReqDesc'),
              value: _c.logUpstreamReq,
              onChanged: (v) {
                setState(() => _c.logUpstreamReq = v);
                _c.updateLogSettings({'log_upstream_request': v});
              },
            ),
            // 保留期是**独立的第二组**，自带 `paddingTop: 8 + borderTop`
            //（`LogSettingsSection.tsx:144`）：React 把三条保留期一起摆在两个
            // 子开关**之后**，不是各跟各的开关。原先少这条线、顺序也是交错的。
            _sectionDivider(),
            // 保留期跟着它那一类的开关走（`LogSettingsSection.tsx:145,166`）：
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
                  _c.updateLogSettings({
                    'upstream_request_retention_unit': u.wire,
                  });
                },
              ),
            _retentionRow(
              t,
              keyId: 'log-retention',
              label: t.t('proxy.logRetention'),
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
          _sectionDivider(),
          Row(
            children: [
              // 永久保留（0）时没有过期日志可清。按钮置灰之外还要说清**为什么**
              //（`LogSettingsSection.tsx:216` 的 title），否则用户只看到一个
              // 点不动的按钮。
              Tooltip(
                message: _c.logRetention == 0
                    ? t.t('logs.cleanupDisabledHint')
                    : '',
                child: SmallButton(
                  key: const ValueKey('cleanup-expired'),
                  // React outline 12px h28（4/12 内边距）。
                  fontSize: 12,
                  padding: (12, 4),
                  // 清理跑起来要几秒，按钮上要看得出来
                  //（`LogSettingsSection.tsx:219`）。
                  label: _c.logMaintBusy
                      ? t.t('logs.cleaning')
                      : t.t('logs.cleanupExpired'),
                  onTap: (_c.logRetention == 0 || _c.logMaintBusy)
                      ? null
                      : () =>
                            setState(() => _confirm = _Confirm.cleanupExpired),
                ),
              ),
              // 两颗按钮 `gap: 8`（`LogSettingsSection.tsx:212`）。
              const SizedBox(width: 8),
              SmallButton(
                key: const ValueKey('clear-logs'),
                fontSize: 12,
                padding: (12, 4),
                label: _c.logMaintBusy
                    ? t.t('logs.cleaning')
                    : t.t('logs.clear'),
                // React `variant="destructive"`（`LogSettingsSection.tsx:221`）；
                // 旁边的「清理过期」是 outline，保持描边。
                danger: true,
                filled: true,
                onTap: _c.logMaintBusy
                    ? null
                    : () => setState(() => _confirm = _Confirm.clearLogs),
              ),
            ],
          ),
          // 预估三个数：超期行数 / 可回收 body 体积 / 库总大小。按钮下面常驻。
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Text(
              _estimate == null
                  ? t.t('logs.cleanupEstimateLoading')
                  : t.t('logs.cleanupEstimate', {
                      'rows': formatNumber(
                        (_estimate!['overdue_rows'] as num?) ?? 0,
                      ),
                      'bytes': formatBytes(
                        (_estimate!['overdue_body_bytes'] as num?) ?? 0,
                      ),
                      'size': formatBytes(
                        (_estimate!['db_size_bytes'] as num?) ?? 0,
                      ),
                    }),
              style: AidogType.caption.copyWith(
                fontSize: 11,
                color: theme.c.fg3,
              ),
            ),
          ),
        ],
      ),
    );
  }

  /// 展开区的上边框分隔：线**上**留白 = 卡的 `gap: 12`，线**下** `paddingTop: 8`
  /// （`LogSettingsSection.tsx:122,144,211`）。原先是上 8 / 下 0。
  Widget _sectionDivider() => Container(
    margin: const EdgeInsets.only(top: 12, bottom: 8),
    decoration: BoxDecoration(
      border: Border(top: BorderSide(color: AidogTheme.of(context).c.line)),
    ),
  );

  Widget _subSwitch({
    Key? key,
    required String label,
    required String description,
    required bool value,
    required ValueChanged<bool> onChanged,
  }) => Padding(
    key: key,
    padding: const EdgeInsets.only(bottom: AidogSpace.smd),
    child: Row(
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                label,
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: AidogTheme.of(context).c.fg,
                ),
              ),
              // 说明上距 `marginTop: 1`（`LogSettingsSection.tsx:126,135`）。
              const SizedBox(height: 1),
              Text(
                description,
                style: AidogType.caption.copyWith(
                  fontSize: 11,
                  color: AidogTheme.of(context).c.fg3,
                ),
              ),
            ],
          ),
        ),
        AidogSwitch(
          value: value,
          compact: true,
          onChanged: () => onChanged(!value),
        ),
      ],
    ),
  );

  Widget _retentionRow(
    I18nController t, {
    required String keyId,
    required String label,
    required int days,
    required RetentionUnit unit,
    required ValueChanged<int> onDays,
    required ValueChanged<RetentionUnit> onUnit,
  }) => Padding(
    key: ValueKey(keyId),
    padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
    child: InlineRow(
      label: label,
      labelWidth: 120,
      // 0 = 永久保留：这时单位（小时/天）没有意义，React 把整组单位藏掉、
      // 换成一行「永久保留」（`LogSettingsSection.tsx:155-162`）。
      suffix: days == 0
          ? Text(
              t.t('proxy.logRetentionForever'),
              style: AidogType.caption.copyWith(
                fontSize: 11,
                color: AidogTheme.of(context).c.fg3,
              ),
            )
          : InlineSelect<RetentionUnit>(
              value: unit,
              options: _kUnits,
              width: 80,
              labelOf: (u) => t.t('unit.${u.wire}'),
              onChanged: (u) => onUnit(u!),
            ),
      child: NumberInput(
        value: '$days',
        onChanged: (v) =>
            onDays((int.tryParse(v) ?? 0) < 0 ? 0 : int.tryParse(v) ?? 0),
        width: 70,
        // 保留期三格显式 `height: 28, fontSize: 12`
        //（`LogSettingsSection.tsx:155,176,196`）。
        height: 28,
        fontSize: 12,
      ),
    ),
  );

  // ── 应用日志文件（`LogSettingsSection.tsx:248-309`）──────────────

  Widget _appLogs(I18nController t) => HeaderCard(
    bottomGap: _cardGap,
    title: t.t('appLog.title'),
    descriptions: [t.t('appLog.desc')],
    trailing: AidogSwitch(
      key: const ValueKey('app-log-file'),
      value: _c.logFileEnabled,
      compact: true,
      onChanged: () => _c.updateAppLogSettings(fileEnabled: !_c.logFileEnabled),
    ),
    child: _c.logFileEnabled
        ? Padding(
            // 线上 12（卡的 `gap: 12`）/ 线下 8（`LogSettingsSection.tsx:274`）。
            padding: const EdgeInsets.only(top: 12),
            child: Container(
              decoration: BoxDecoration(
                border: Border(
                  top: BorderSide(color: AidogTheme.of(context).c.line),
                ),
              ),
              child: Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Wrap(
                  // `gap: 16` 横竖同值（`LogSettingsSection.tsx:274`）。
                  spacing: 16,
                  runSpacing: 16,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    InlineRow(
                      label: t.t('appLog.level'),
                      child: InlineSelect<String>(
                        value: _c.logLevel,
                        options: kAppLogLevels,
                        width: 100,
                        labelOf: (o) => o.toUpperCase(),
                        onChanged: (v) => _c.updateAppLogSettings(level: v!),
                      ),
                    ),
                    InlineRow(
                      label: t.t('appLog.retention'),
                      unit: t.t('appLog.retentionUnit'),
                      child: NumberInput(
                        value: '${_c.logRetHours}',
                        onChanged: (v) => _c.updateAppLogSettings(
                          retentionHours: (int.tryParse(v) ?? 0) < 0
                              ? 0
                              : int.tryParse(v) ?? 0,
                        ),
                        width: 70,
                        // 同保留期档：`height: 28, fontSize: 12`
                        //（`LogSettingsSection.tsx:299`）。
                        height: 28,
                        fontSize: 12,
                      ),
                    ),
                    if (_c.logRetHours == 0)
                      Text(
                        t.t('appLog.retentionForever'),
                        style: AidogType.caption.copyWith(
                          fontSize: 11,
                          color: AidogTheme.of(context).c.fg3,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          )
        : const SizedBox.shrink(),
  );

  // ── 聚合统计（`SystemMiscSection.tsx` 的 DbStatsSection）──────────

  Widget _stats(I18nController t) => HeaderCard(
    bottomGap: _cardGap,
    title: t.t('stats.aggSettings'),
    descriptions: [t.t('stats.aggSettingsHint')],
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Padding(
          // 卡 `gap: 12`（`SystemMiscSection.tsx:154-197`）。
          padding: const EdgeInsets.only(top: 12),
          child: InlineRow(
            label: t.t('stats.aggRetention'),
            labelWidth: 120,
            suffix: Text(
              _c.statsRetention == 0
                  ? t.t('proxy.logRetentionForever')
                  : t.t('unit.days'),
              style: AidogType.caption.copyWith(
                fontSize: 11,
                color: AidogTheme.of(context).c.fg3,
              ),
            ),
            child: NumberInput(
              key: const ValueKey('stats-retention'),
              value: '${_c.statsRetention}',
              onChanged: (v) {
                final n = int.tryParse(v) ?? 0;
                _c.setStatsRetention(n < 0 ? 0 : n);
              },
              width: 70,
            ),
          ),
        ),
        const SizedBox(height: 12),
        // 文字左、按钮右（React space-between，`SystemMiscSection.tsx:181-196`）。
        Row(
          children: [
            Expanded(
              child: Text(
                t.t('stats.rebuildHint'),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: AidogTheme.of(context).c.fg2,
                ),
              ),
            ),
            SmallButton(
              key: const ValueKey('stats-rebuild'),
              fontSize: 13,
              padding: (16, 7),
              label: _c.statsRebuilding
                  ? t.t('common.loading')
                  : t.t('stats.rebuild'),
              onTap: _c.statsRebuilding
                  ? null
                  : () => _c.rebuildStats(t.t('stats.rebuildDone')),
            ),
          ],
        ),
      ],
    ),
  );

  // ── 杂项卡（React 把这几块拆在各自 glass-surface 里）────────────
  // React System tab 卡序（`AppSettings.tsx:72-79` + 各 section 内卡序）：
  // 状态卡 → 启动四开关 → 上游代理 → 内核 → 超时 → 内置工具兼容 → 日志设置 →
  // 压缩数据库 → 聚合统计 → 自动更新 → 版本。

  Widget _btcGlobal(I18nController t) => ToggleCard(
    bottomGap: _cardGap,
    key: const ValueKey('btc-global'),
    label: t.t('proxy.btcGlobal'),
    descriptions: [t.t('proxy.btcGlobalDesc')],
    value: _c.btcGlobalEnabled,
    onChanged: _c.setBtcGlobal,
  );

  Widget _dbCompact(I18nController t) => HeaderCard(
    bottomGap: _cardGap,
    title: t.t('settings.dbCompact'),
    descriptions: [t.t('settings.dbCompactHint')],
    // 卡是 `space-between`：文字左、按钮右，**同一行**
    //（`SystemMiscSection.tsx:102-125`）。挂在 child 上会被排到标题行下面。
    trailing: SmallButton(
      key: const ValueKey('db-compact'),
      fontSize: 13,
      padding: (16, 7),
      label: _c.dbCompacting
          ? t.t('common.loading')
          : t.t('settings.dbCompact'),
      onTap: _c.dbCompacting
          ? null
          : () => setState(() => _confirm = _Confirm.compactDb),
    ),
    child: const SizedBox.shrink(),
  );

  Widget _autoUpdate(I18nController t) => ToggleCard(
    bottomGap: _cardGap,
    key: const ValueKey('auto-update'),
    label: t.t('settings.autoUpdate'),
    descriptions: [t.t('settings.autoUpdateHint')],
    value: _c.autoUpdateEnabled,
    onChanged: _c.setAutoUpdate,
  );

  Widget _version(I18nController t) {
    if (_c.appVersion.isEmpty) return const SizedBox.shrink();
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: _cardGap),
      child: Tile(
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
        child: Row(
          children: [
            Text(t.t('app.version'), style: _titleStyle(theme)),
            // 卡是 `space-between`，版本号贴右（`SystemMiscSection.tsx:232-244`）。
            const Spacer(),
            // 版本号 mono 13 secondary（`SystemMiscSection.tsx:238-244`）。
            Text(
              ltr('v${_c.appVersion}'),
              style: AidogType.numMd.copyWith(fontSize: 13, color: theme.c.fg2),
            ),
          ],
        ),
      ),
    );
  }

  TextStyle _titleStyle(AidogTheme theme) =>
      AidogType.label.copyWith(fontSize: 13, fontWeight: FontWeight.w600);

  Widget _desc(String text) => Padding(
    padding: const EdgeInsets.only(top: 2),
    child: Text(
      text,
      style: AidogType.caption.copyWith(
        fontSize: 12,
        color: AidogTheme.of(context).c.fg2,
      ),
    ),
  );

  /// 三个确认框的共同档位：maxWidth **380**、padding **20**、标题 **13 w600**、
  /// 正文 **12 lh1.6 secondary**、页脚按钮 **12 / 6-14**
  /// （`SystemMiscSection.tsx:129-149`、`LogSettingsSection.tsx:314-345,350-376`）。
  /// 原先三处一个覆盖都没传，吃的是 420 / 24 / 17 / micro 11 那一档。
  TextStyle get _confirmTitleStyle =>
      AidogType.title.copyWith(fontSize: 13, fontWeight: FontWeight.w600);

  TextStyle get _confirmBodyStyle => AidogType.caption.copyWith(
    fontSize: 12,
    height: 1.6,
    color: AidogTheme.of(context).c.fg2,
  );

  Widget _confirmCard(I18nController t) {
    final e = _estimate;
    return switch (_confirm!) {
      // 压缩确认是 `AlertDialog`（`SystemMiscSection.tsx:128`）：没有 ✕、
      // 点遮罩不关、确认键是默认变体不是 destructive。
      _Confirm.compactDb => ConfirmCard(
        maxWidth: 380,
        padding: const EdgeInsets.all(20),
        titleStyle: _confirmTitleStyle,
        bodyStyle: _confirmBodyStyle,
        buttonFontSize: 12,
        buttonPadding: (14, 6),
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
      // 清空 / 清理过期两处是普通 `Dialog`（`LogSettingsSection.tsx:313,349`）：
      // `DialogContent` 自带右上角 ✕、点遮罩即关（busy 时都拦住），
      // 确认键是 `variant="destructive"`（`:367`、`:337`）。
      _Confirm.clearLogs => ConfirmCard(
        maxWidth: 380,
        padding: const EdgeInsets.all(20),
        titleStyle: _confirmTitleStyle,
        bodyStyle: _confirmBodyStyle,
        buttonFontSize: 12,
        buttonPadding: (14, 6),
        dismissOnBarrier: true,
        dangerConfirm: true,
        onClose: _c.logMaintBusy ? null : () => setState(() => _confirm = null),
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
        maxWidth: 380,
        padding: const EdgeInsets.all(20),
        titleStyle: _confirmTitleStyle,
        bodyStyle: _confirmBodyStyle,
        buttonFontSize: 12,
        buttonPadding: (14, 6),
        dismissOnBarrier: true,
        dangerConfirm: true,
        onClose: _c.logMaintBusy ? null : () => setState(() => _confirm = null),
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
