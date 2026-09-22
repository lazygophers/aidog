/// 系统设置（`settings/system`）的状态与动作 —— 逐条对齐
/// `src/pages/AppSettings/useSystemSettings.ts`（23 个 state + 3 个 effect + 16 个 handler）。
///
/// React 那边已经把这层从组件里抽出来单独成文件了，这里沿用同一条缝：
/// 逻辑不碰 widget，所以能直接单测（`test/settings/system_logic_test.dart`）。
///
/// 字段名全部照抄 `src/services/api/types/generated/*.ts` 与 `types/manual.ts`
/// （I01 定的规矩：Dart 侧字段名不许凭记忆写）。
library;

import 'package:aidog_flutter/transport.dart' show RpcException;

import '../invoke.dart';

/// 保留期单位。serde lowercase，与 `generated/RetentionUnit.ts` 一致。
enum RetentionUnit {
  hour,
  day,
  week;

  String get wire => name;
  static RetentionUnit parse(Object? v) =>
      RetentionUnit.values.firstWhere((e) => e.name == v, orElse: () => RetentionUnit.day);
}

/// `types/manual.ts::ProxyStartError`。`message` 是英文调试串，**禁直接展示给用户**
/// （用户可见文案按 kind + port 拼 i18n 模板）。
class ProxyStartError {
  const ProxyStartError({required this.kind, required this.port, required this.message});

  final String kind; // "addr_in_use" | "other"
  final int port;
  final String message;

  /// 与 `proxy.ts::isProxyStartError` 同判据：有 kind 且有 port 才算。
  /// 失败值在 Dart 侧包在 [RpcException.body] 里（README「命令失败」那一行）。
  static ProxyStartError? tryParse(Object? e) {
    final body = e is RpcException ? e.body : e;
    if (body is! Map) return null;
    if (!body.containsKey('kind') || !body.containsKey('port')) return null;
    final port = body['port'];
    if (port is! num) return null;
    return ProxyStartError(
      kind: '${body['kind']}',
      port: port.toInt(),
      message: '${body['message'] ?? ''}',
    );
  }
}

/// `generated/ProxyClientSettings.ts`。
class ProxyClientSettings {
  const ProxyClientSettings({
    required this.enabled,
    required this.proxyType,
    required this.host,
    required this.port,
    required this.username,
    required this.password,
    required this.dnsOverProxy,
    required this.noProxy,
  });

  /// React 版 `useSystemSettings.ts:43-46` 的初值，一字不改。
  static const initial = ProxyClientSettings(
    enabled: false,
    proxyType: 'socks5',
    host: '127.0.0.1',
    port: 7890,
    username: '',
    password: '',
    dnsOverProxy: true,
    noProxy: '',
  );

  final bool enabled;
  final String proxyType;
  final String host;
  final int port;
  final String username;
  final String password;
  final bool dnsOverProxy;
  final String noProxy;

  factory ProxyClientSettings.fromJson(Map<String, Object?> j) => ProxyClientSettings(
        enabled: j['enabled'] as bool? ?? false,
        proxyType: j['proxy_type'] as String? ?? 'socks5',
        host: j['host'] as String? ?? '127.0.0.1',
        port: (j['port'] as num?)?.toInt() ?? 7890,
        username: j['username'] as String? ?? '',
        password: j['password'] as String? ?? '',
        dnsOverProxy: j['dns_over_proxy'] as bool? ?? true,
        noProxy: j['no_proxy'] as String? ?? '',
      );

  Map<String, Object?> toJson() => {
        'enabled': enabled,
        'proxy_type': proxyType,
        'host': host,
        'port': port,
        'username': username,
        'password': password,
        'dns_over_proxy': dnsOverProxy,
        'no_proxy': noProxy,
      };

  ProxyClientSettings copyWith({
    bool? enabled,
    String? proxyType,
    String? host,
    int? port,
    String? username,
    String? password,
    bool? dnsOverProxy,
    String? noProxy,
  }) =>
      ProxyClientSettings(
        enabled: enabled ?? this.enabled,
        proxyType: proxyType ?? this.proxyType,
        host: host ?? this.host,
        port: port ?? this.port,
        username: username ?? this.username,
        password: password ?? this.password,
        dnsOverProxy: dnsOverProxy ?? this.dnsOverProxy,
        noProxy: noProxy ?? this.noProxy,
      );
}

/// 三级日志开关 + 三级 retention，字段名照 `generated/ProxyLogSettings.ts`。
class ProxyLogSettings {
  const ProxyLogSettings({
    required this.enabled,
    required this.logUserRequest,
    required this.logUpstreamRequest,
    required this.userRequestRetentionDays,
    required this.userRequestRetentionUnit,
    required this.upstreamRequestRetentionDays,
    required this.upstreamRequestRetentionUnit,
    required this.retentionDays,
    required this.retentionUnit,
  });

  final bool enabled;
  final bool logUserRequest;
  final bool logUpstreamRequest;
  final int userRequestRetentionDays;
  final RetentionUnit userRequestRetentionUnit;
  final int upstreamRequestRetentionDays;
  final RetentionUnit upstreamRequestRetentionUnit;
  final int retentionDays;
  final RetentionUnit retentionUnit;

  Map<String, Object?> toJson() => {
        'enabled': enabled,
        'log_user_request': logUserRequest,
        'log_upstream_request': logUpstreamRequest,
        'user_request_retention_days': userRequestRetentionDays,
        'user_request_retention_unit': userRequestRetentionUnit.wire,
        'upstream_request_retention_days': upstreamRequestRetentionDays,
        'upstream_request_retention_unit': upstreamRequestRetentionUnit.wire,
        'retention_days': retentionDays,
        'retention_unit': retentionUnit.wire,
      };
}

/// 系统设置控制器。UI 只读这些字段、只调这些方法。
class SystemSettingsController {
  SystemSettingsController({
    InvokeFn? invoke,
    this.onChanged,
    this.onLogSettingsChanged,
    this.appVersionFn,
  }) : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;

  /// 取应用版本号（I12 的 `getAppVersion`）。测试传假的，避免真读 bundle。
  final Future<String> Function()? appVersionFn;

  /// state 变了通知 UI 重绘（widget 里就是 `setState`）。
  final void Function()? onChanged;

  /// 日志总开关变化要上抛给外壳（关掉日志时侧栏藏掉「日志」项），
  /// 对应 React 的 `onLogSettingsChanged`。
  final void Function(bool enabled)? onLogSettingsChanged;

  // ── state（初值与 React 版逐条相同）──
  bool running = false;
  int proxyPort = 9890;
  bool autostart = false;
  bool bindLan = false;
  bool autolaunch = false;
  bool silentLaunch = false;
  bool logEnabled = false;
  int logRetention = 90;
  RetentionUnit logRetentionUnit = RetentionUnit.day;
  bool logUserReq = true;
  bool logUpstreamReq = true;
  int userReqRetention = 7;
  RetentionUnit userReqRetentionUnit = RetentionUnit.day;
  int upstreamReqRetention = 7;
  RetentionUnit upstreamReqRetentionUnit = RetentionUnit.day;
  int reqTimeout = 300;
  int connTimeout = 10;
  bool btcGlobalEnabled = false;
  bool logFileEnabled = true;
  String logLevel = 'info';
  int logRetHours = 3;
  String message = '';
  ProxyStartError? proxyStartError;
  String appVersion = '';
  bool dbCompacting = false;

  /// 日志清理 / 清空执行中。确认卡按这个变灰并显示「清理中…」，
  /// 对齐 `LogSettingsSection.tsx:313-343` 的 busy 态。
  bool logMaintBusy = false;
  int statsRetention = 365;
  bool statsRebuilding = false;
  bool autoUpdateEnabled = true;
  ProxyClientSettings proxyClient = ProxyClientSettings.initial;

  void _notify() => onChanged?.call();

  /// 把 React 的 `try { ... } catch { /* defaults */ }` 逐段照搬：
  /// **每一段独立 try**，一段失败不影响后面几段。整段合成一个 try 会让第一个
  /// 失败的命令把后面所有设置都退回默认值。
  Future<void> load() async {
    // 版本号（React 是独立的 useEffect）。
    try {
      appVersion = await (appVersionFn?.call() ?? Future.value(''));
    } catch (_) {
      appVersion = '';
    }
    _notify();

    try {
      final s = _map(await _invoke('proxy_get_settings'));
      autostart = s['autostart'] as bool? ?? false;
      silentLaunch = s['silent_launch'] as bool? ?? false;
      bindLan = s['bind_lan'] as bool? ?? false;
      proxyPort = (s['port'] as num?)?.toInt() ?? 9890;
    } catch (_) {/* defaults */}
    try {
      running = await _invoke('proxy_status') as bool? ?? false;
    } catch (_) {
      running = false;
    }
    try {
      final al = await _invoke('app_get_autolaunch') as bool? ?? false;
      autolaunch = al;
      // silentLaunch 仅在开机自启生效时有意义；autolaunch off 时强制 false 并持久化。
      if (!al) {
        silentLaunch = false;
        try {
          await _invoke('app_set_silent_launch', {'enabled': false});
        } catch (_) {/* ignore */}
      }
    } catch (_) {/* defaults */}
    try {
      final ls = _map(await _invoke('proxy_log_settings_get'));
      logEnabled = ls['enabled'] as bool? ?? false;
      logRetention = (ls['retention_days'] as num?)?.toInt() ?? 90;
      logRetentionUnit = RetentionUnit.parse(ls['retention_unit']);
      logUserReq = ls['log_user_request'] as bool? ?? true;
      logUpstreamReq = ls['log_upstream_request'] as bool? ?? true;
      userReqRetention = (ls['user_request_retention_days'] as num?)?.toInt() ?? 7;
      userReqRetentionUnit = RetentionUnit.parse(ls['user_request_retention_unit']);
      upstreamReqRetention = (ls['upstream_request_retention_days'] as num?)?.toInt() ?? 7;
      upstreamReqRetentionUnit = RetentionUnit.parse(ls['upstream_request_retention_unit']);
    } catch (_) {/* defaults */}
    try {
      final ts = _map(await _invoke('proxy_timeout_get'));
      reqTimeout = (ts['request_timeout_secs'] as num?)?.toInt() ?? 300;
      connTimeout = (ts['connect_timeout_secs'] as num?)?.toInt() ?? 10;
    } catch (_) {/* defaults */}
    try {
      final btc = await _invoke('settings_get', {'scope': 'proxy', 'key': 'builtin_tool_compat'});
      btcGlobalEnabled = btc is Map ? (btc['enabled'] as bool? ?? false) : false;
    } catch (_) {/* defaults */}
    try {
      final ls = _map(await _invoke('app_log_settings_get'));
      logFileEnabled = ls['file_enabled'] as bool? ?? true;
      logLevel = ls['level'] as String? ?? 'info';
      logRetHours = (ls['retention_hours'] as num?)?.toInt() ?? 3;
    } catch (_) {/* defaults */}
    try {
      proxyClient = ProxyClientSettings.fromJson(_map(await _invoke('proxy_client_get_settings')));
    } catch (_) {/* defaults */}
    try {
      final ss = _map(await _invoke('stats_settings_get'));
      statsRetention = (ss['retention_days'] as num?)?.toInt() ?? 365;
    } catch (_) {/* defaults */}
    try {
      autoUpdateEnabled = await _invoke('get_auto_update_enabled') as bool? ?? true;
    } catch (_) {/* defaults: keep true */}
    _notify();
  }

  // ── actions ──

  Future<void> startProxy() async {
    try {
      final msg = await _invoke('proxy_start', {'port': proxyPort});
      running = true;
      message = '$msg';
      proxyStartError = null; // 重试成功：错误条消失
    } catch (e) {
      final pse = ProxyStartError.tryParse(e);
      if (pse != null) {
        // 持久错误条，不走 message（那条会一闪而过）
        proxyStartError = pse;
      } else {
        message = '$e';
      }
    }
    _notify();
  }

  /// `stoppedText` = `t("proxy.stopped")`，由 UI 传进来（这一层不依赖 i18n context）。
  Future<void> stopProxy(String stoppedText) async {
    try {
      await _invoke('proxy_stop');
      running = false;
      message = stoppedText;
    } catch (e) {
      message = '$e';
    }
    _notify();
  }

  Future<void> setAutostart(bool val) async {
    try {
      await _invoke('proxy_set_autostart', {'enabled': val});
      autostart = val;
    } catch (e) {
      message = '$e';
    }
    _notify();
  }

  /// `appliedText` = `t("proxy.bindLanApplied")`。后端会在代理运行时自动重启使新绑定生效。
  Future<void> setBindLan(bool val, String appliedText) async {
    try {
      await _invoke('proxy_set_bind_lan', {'enabled': val});
      bindLan = val;
      message = appliedText;
    } catch (e) {
      message = '$e';
    }
    _notify();
  }

  Future<void> setAutolaunch(bool val) async {
    try {
      await _invoke('app_set_autolaunch', {'enabled': val});
      autolaunch = val;
      // 关开机自启时同步关掉并持久化静默启动（UI 也随之隐藏）。
      if (!val && silentLaunch) {
        try {
          await _invoke('app_set_silent_launch', {'enabled': false});
          silentLaunch = false;
        } catch (_) {/* ignore */}
      }
    } catch (e) {
      message = '$e';
    }
    _notify();
  }

  Future<void> setSilentLaunch(bool val) async {
    try {
      await _invoke('app_set_silent_launch', {'enabled': val});
      silentLaunch = val;
    } catch (e) {
      message = '$e';
    }
    _notify();
  }

  /// 注意：React 版**先改本地 state 再发命令**，失败也不回滚（只挂一条 message）。
  /// 这里照搬，不「顺手修好」——行为不同就是 bug。
  Future<void> updateProxyClient(ProxyClientSettings next) async {
    proxyClient = next;
    _notify();
    try {
      await _invoke('proxy_client_set_settings', {'settings': next.toJson()});
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  ProxyLogSettings buildLogSettings() => ProxyLogSettings(
        enabled: logEnabled,
        logUserRequest: logUserReq,
        logUpstreamRequest: logUpstreamReq,
        userRequestRetentionDays: userReqRetention,
        userRequestRetentionUnit: userReqRetentionUnit,
        upstreamRequestRetentionDays: upstreamReqRetention,
        upstreamRequestRetentionUnit: upstreamReqRetentionUnit,
        retentionDays: logRetention,
        retentionUnit: logRetentionUnit,
      );

  Future<void> setLogEnabled(bool val) async {
    try {
      final settings = {...buildLogSettings().toJson(), 'enabled': val};
      await _invoke('proxy_log_settings_set', {'settings': settings});
      logEnabled = val;
      onLogSettingsChanged?.call(val);
    } catch (e) {
      message = '$e';
    }
    _notify();
  }

  /// 局部更新：以当前 state 为底，覆盖 [partial] 后整份写回。
  /// **不改本地 state**（与 React 一致：本地值由各控件自己的 setter 先改好）。
  Future<void> updateLogSettings(Map<String, Object?> partial) async {
    final settings = {...buildLogSettings().toJson(), ...partial};
    try {
      await _invoke('proxy_log_settings_set', {'settings': settings});
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  /// 压缩数据库。**破坏性/阻塞操作**：调用方必须先弹确认框，确认了才调本方法
  /// （React 是 `window.confirm`，Flutter 侧走 AlertDialog）。
  ///
  /// [doneText] 是已经填好 before/after/pct 的成品串 —— 模板在 UI 层套，这层不碰 i18n。
  Future<void> compactDb(String Function(String before, String after, String pct) doneText) async {
    dbCompacting = true;
    _notify();
    try {
      final r = _map(await _invoke('db_compact'));
      final before = (r['before_bytes'] as num).toDouble();
      final after = (r['after_bytes'] as num).toDouble();
      final beforeMB = (before / 1024 / 1024).toStringAsFixed(1);
      final afterMB = (after / 1024 / 1024).toStringAsFixed(1);
      final pct = before > 0 ? ((1 - after / before) * 100).round() : 0;
      message = doneText(beforeMB, afterMB, '$pct');
    } catch (e) {
      message = '$e';
    } finally {
      dbCompacting = false;
      _notify();
    }
  }

  Future<void> setStatsRetention(int v) async {
    statsRetention = v;
    _notify();
    try {
      await _invoke('stats_settings_set', {
        'settings': {'retention_days': v},
      });
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  Future<void> rebuildStats(String doneText) async {
    statsRebuilding = true;
    _notify();
    try {
      await _invoke('stats_rebuild_from_logs');
      message = doneText;
    } catch (e) {
      message = '$e';
    } finally {
      statsRebuilding = false;
      _notify();
    }
  }

  Future<void> setTimeouts(int req, int conn) async {
    reqTimeout = req;
    connTimeout = conn;
    _notify();
    try {
      await _invoke('proxy_timeout_set', {
        'settings': {'request_timeout_secs': req, 'connect_timeout_secs': conn},
      });
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  Future<void> setAutoUpdate(bool val) async {
    try {
      await _invoke('set_auto_update_enabled', {'enabled': val});
      autoUpdateEnabled = val;
    } catch (e) {
      message = '$e';
    }
    _notify();
  }

  Future<void> setBtcGlobal(bool val) async {
    try {
      await _invoke('settings_set', {
        'input': {
          'scope': 'proxy',
          'key': 'builtin_tool_compat',
          'value': {'enabled': val},
        },
      });
      btcGlobalEnabled = val;
    } catch (e) {
      message = '$e';
    }
    _notify();
  }

  Future<void> updateAppLogSettings({bool? fileEnabled, String? level, int? retentionHours}) async {
    logFileEnabled = fileEnabled ?? logFileEnabled;
    logLevel = level ?? logLevel;
    logRetHours = retentionHours ?? logRetHours;
    _notify();
    try {
      await _invoke('app_log_settings_set', {
        'settings': {
          'file_enabled': logFileEnabled,
          'level': logLevel,
          'retention_hours': logRetHours,
        },
      });
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  // ── 日志维护（LogSettingsSection 用）──

  /// 只读预估：超期行数 + 这些行 body 字节总和 + log.db 当前大小。
  Future<Map<String, Object?>> cleanupEstimate() async => _map(await _invoke('proxy_log_cleanup_estimate'));

  Future<void> cleanupExpired() => _logMaint('proxy_log_cleanup_expired');

  /// **破坏性**：清空全部代理日志，调用方必须先确认。
  Future<void> clearLogs() => _logMaint('proxy_log_clear');

  /// 两条日志维护命令共用的执行中态。失败也要把标记收回来，
  /// 否则确认卡会永远卡在「清理中…」。
  Future<void> _logMaint(String cmd) async {
    logMaintBusy = true;
    _notify();
    try {
      await _invoke(cmd);
    } finally {
      logMaintBusy = false;
      _notify();
    }
  }

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}
