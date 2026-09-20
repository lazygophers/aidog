/// `SystemSettingsController` 的分支与边界 —— 对照
/// `src/pages/AppSettings/useSystemSettings.ts`。
///
/// React 侧这个 hook 没有测试（`src/pages/AppSettings/` 下无 *.test.*），所以这里
/// 没有可照抄的断言；用例按「每个 handler 的成功支 + 失败支 + 它特有的那条规则」编，
/// 重点盯 React 里那几条容易被搬丢的：
///   - 每段 load 独立 try（一段失败不该把后面几段退回默认值）
///   - autolaunch=false 时强制关 silentLaunch **并持久化**
///   - proxy_start 失败分两路：结构化 ProxyStartError 挂持久错误条，其余走一闪而过的 message
///   - 启动成功要清掉上一次的错误条
library;

import 'package:aidog_flutter/src/pages/settings/system_logic.dart';
import 'package:aidog_flutter/transport.dart' show RpcException;
import 'package:flutter_test/flutter_test.dart';

import 'fake_invoke.dart';

/// 一份「全部命令都成功」的响应表，字段名照 generated TS。
Map<String, Object?> okResponses() => {
      'proxy_get_settings': {
        'port': 9999,
        'autostart': true,
        'silent_launch': true,
        'bind_lan': true,
      },
      'proxy_status': true,
      'app_get_autolaunch': true,
      'proxy_log_settings_get': {
        'enabled': true,
        'log_user_request': false,
        'log_upstream_request': false,
        'user_request_retention_days': 12,
        'user_request_retention_unit': 'hour',
        'upstream_request_retention_days': 13,
        'upstream_request_retention_unit': 'week',
        'retention_days': 30,
        'retention_unit': 'hour',
      },
      'proxy_timeout_get': {'request_timeout_secs': 111, 'connect_timeout_secs': 22},
      'settings_get': {'enabled': true},
      'app_log_settings_get': {'file_enabled': false, 'level': 'debug', 'retention_hours': 9},
      'proxy_client_get_settings': {
        'enabled': true,
        'proxy_type': 'http',
        'host': '10.0.0.1',
        'port': 1080,
        'username': 'u',
        'password': 'p',
        'dns_over_proxy': false,
        'no_proxy': 'localhost',
      },
      'stats_settings_get': {'retention_days': 7},
      'get_auto_update_enabled': false,
    };

void main() {
  group('load', () {
    test('把 9 个来源的值逐字段落进 state', () async {
      final fake = FakeInvoke(okResponses());
      final c = SystemSettingsController(invoke: fake.fn, appVersionFn: () async => '1.2.3');
      await c.load();

      expect(c.appVersion, '1.2.3');
      expect(c.proxyPort, 9999);
      expect(c.autostart, isTrue);
      expect(c.silentLaunch, isTrue);
      expect(c.bindLan, isTrue);
      expect(c.running, isTrue);
      expect(c.autolaunch, isTrue);
      expect(c.logEnabled, isTrue);
      expect(c.logUserReq, isFalse);
      expect(c.logUpstreamReq, isFalse);
      expect(c.userReqRetention, 12);
      expect(c.userReqRetentionUnit, RetentionUnit.hour);
      expect(c.upstreamReqRetention, 13);
      expect(c.upstreamReqRetentionUnit, RetentionUnit.week);
      expect(c.logRetention, 30);
      expect(c.logRetentionUnit, RetentionUnit.hour);
      expect(c.reqTimeout, 111);
      expect(c.connTimeout, 22);
      expect(c.btcGlobalEnabled, isTrue);
      expect(c.logFileEnabled, isFalse);
      expect(c.logLevel, 'debug');
      expect(c.logRetHours, 9);
      expect(c.proxyClient.proxyType, 'http');
      expect(c.proxyClient.host, '10.0.0.1');
      expect(c.proxyClient.dnsOverProxy, isFalse);
      expect(c.statsRetention, 7);
      expect(c.autoUpdateEnabled, isFalse);
    });

    test('单段失败只退回该段默认值，后面几段照常生效', () async {
      final fake = FakeInvoke(okResponses());
      // 第一段就炸：React 里它是独立 try，后面 8 段必须照常。
      fake.errors['proxy_get_settings'] = StateError('boom');
      final c = SystemSettingsController(invoke: fake.fn);
      await c.load();

      expect(c.proxyPort, 9890, reason: '该段退回默认');
      expect(c.autostart, isFalse, reason: '该段退回默认');
      expect(c.reqTimeout, 111, reason: '后面的段不受影响');
      expect(c.statsRetention, 7, reason: '后面的段不受影响');
      expect(c.autoUpdateEnabled, isFalse, reason: '后面的段不受影响');
    });

    test('get_auto_update_enabled 失败时保持 true（不打扰存量用户）', () async {
      final fake = FakeInvoke(okResponses());
      fake.errors['get_auto_update_enabled'] = StateError('boom');
      final c = SystemSettingsController(invoke: fake.fn);
      await c.load();
      expect(c.autoUpdateEnabled, isTrue);
    });

    test('proxy_status 失败 → running=false', () async {
      final fake = FakeInvoke(okResponses());
      fake.errors['proxy_status'] = StateError('boom');
      final c = SystemSettingsController(invoke: fake.fn);
      await c.load();
      expect(c.running, isFalse);
    });

    test('autolaunch=false 时强制关掉静默启动，并持久化写回后端', () async {
      final fake = FakeInvoke({...okResponses(), 'app_get_autolaunch': false});
      final c = SystemSettingsController(invoke: fake.fn);
      await c.load();

      expect(c.autolaunch, isFalse);
      expect(c.silentLaunch, isFalse, reason: 'proxy_get_settings 读到的 true 要被强制压掉');
      expect(fake.lastCallTo('app_set_silent_launch')?.args, {'enabled': false});
    });

    test('autolaunch=true 时不动静默启动，也不多发一次写命令', () async {
      final fake = FakeInvoke(okResponses());
      final c = SystemSettingsController(invoke: fake.fn);
      await c.load();
      expect(c.silentLaunch, isTrue);
      expect(fake.callsTo('app_set_silent_launch'), isEmpty);
    });

    test('appVersion 取不到时是空串，不抛', () async {
      final c = SystemSettingsController(
        invoke: FakeInvoke(okResponses()).fn,
        appVersionFn: () async => throw StateError('no version'),
      );
      await c.load();
      expect(c.appVersion, '');
    });
  });

  group('代理启停', () {
    test('启动成功：running=true、message 取后端返回串、错误条清掉', () async {
      final fake = FakeInvoke({'proxy_start': 'listening on 9890'});
      final c = SystemSettingsController(invoke: fake.fn);
      c.proxyStartError = const ProxyStartError(kind: 'addr_in_use', port: 9890, message: 'old');
      await c.startProxy();

      expect(c.running, isTrue);
      expect(c.message, 'listening on 9890');
      expect(c.proxyStartError, isNull, reason: '重试成功后上一条错误条必须消失');
      expect(fake.lastCallTo('proxy_start')?.args, {'port': 9890});
    });

    test('启动失败且是结构化 ProxyStartError → 挂持久错误条，不写 message', () async {
      final fake = FakeInvoke();
      fake.errors['proxy_start'] = RpcException(
        500,
        {'kind': 'addr_in_use', 'port': 9890, 'message': 'Address already in use'},
        command: 'proxy_start',
      );
      final c = SystemSettingsController(invoke: fake.fn);
      await c.startProxy();

      expect(c.proxyStartError?.kind, 'addr_in_use');
      expect(c.proxyStartError?.port, 9890);
      expect(c.message, '', reason: 'message 会一闪而过，端口占用必须挂常驻错误条');
      expect(c.running, isFalse);
    });

    test('启动失败但不是结构化错误 → 走 message，不挂错误条', () async {
      final fake = FakeInvoke();
      fake.errors['proxy_start'] = StateError('plain failure');
      final c = SystemSettingsController(invoke: fake.fn);
      await c.startProxy();

      expect(c.proxyStartError, isNull);
      expect(c.message, contains('plain failure'));
    });

    test('缺 port 字段的错误体不算 ProxyStartError（与 isProxyStartError 同判据）', () {
      expect(ProxyStartError.tryParse({'kind': 'addr_in_use'}), isNull);
      expect(ProxyStartError.tryParse({'port': 1}), isNull);
      expect(ProxyStartError.tryParse('a string'), isNull);
      expect(ProxyStartError.tryParse({'kind': 'other', 'port': 1})?.message, '');
    });

    test('停止成功用传入的 i18n 串，失败用错误串', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn)..running = true;
      await c.stopProxy('已停止');
      expect(c.running, isFalse);
      expect(c.message, '已停止');

      fake.errors['proxy_stop'] = StateError('nope');
      await c.stopProxy('已停止');
      expect(c.message, contains('nope'));
    });
  });

  group('开关类', () {
    test('setAutostart 成功才改本地值', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn);
      await c.setAutostart(true);
      expect(c.autostart, isTrue);
      expect(fake.lastCallTo('proxy_set_autostart')?.args, {'enabled': true});

      fake.errors['proxy_set_autostart'] = StateError('denied');
      await c.setAutostart(false);
      expect(c.autostart, isTrue, reason: '写失败不该改本地值');
      expect(c.message, contains('denied'));
    });

    test('setBindLan 成功后给出「已生效」提示', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn);
      await c.setBindLan(true, '绑定已应用');
      expect(c.bindLan, isTrue);
      expect(c.message, '绑定已应用');
    });

    test('关掉开机自启时连带关掉静默启动并持久化', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn)
        ..autolaunch = true
        ..silentLaunch = true;
      await c.setAutolaunch(false);

      expect(c.autolaunch, isFalse);
      expect(c.silentLaunch, isFalse);
      expect(fake.lastCallTo('app_set_silent_launch')?.args, {'enabled': false});
    });

    test('关开机自启时若静默启动本来就是关的，不多发写命令', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn)
        ..autolaunch = true
        ..silentLaunch = false;
      await c.setAutolaunch(false);
      expect(fake.callsTo('app_set_silent_launch'), isEmpty);
    });

    test('内置工具兼容全局开关走 settings_set 的 input 包装', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn);
      await c.setBtcGlobal(true);
      expect(c.btcGlobalEnabled, isTrue);
      expect(fake.lastCallTo('settings_set')?.args, {
        'input': {
          'scope': 'proxy',
          'key': 'builtin_tool_compat',
          'value': {'enabled': true},
        },
      });
    });

    test('自动更新开关', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn);
      await c.setAutoUpdate(false);
      expect(c.autoUpdateEnabled, isFalse);
      expect(fake.lastCallTo('set_auto_update_enabled')?.args, {'enabled': false});
    });
  });

  group('日志设置', () {
    test('setLogEnabled 整份写回并上抛给外壳', () async {
      final fake = FakeInvoke();
      Object? notified;
      final c = SystemSettingsController(
        invoke: fake.fn,
        onLogSettingsChanged: (v) => notified = v,
      );
      await c.setLogEnabled(true);

      expect(c.logEnabled, isTrue);
      expect(notified, isTrue);
      final sent = fake.lastCallTo('proxy_log_settings_set')!.args!['settings']! as Map;
      expect(sent['enabled'], isTrue);
      // 其余 8 个字段必须一并回传（后端是整份覆盖，漏字段就把用户设置清了）
      expect(sent.keys.toSet(), {
        'enabled',
        'log_user_request',
        'log_upstream_request',
        'user_request_retention_days',
        'user_request_retention_unit',
        'upstream_request_retention_days',
        'upstream_request_retention_unit',
        'retention_days',
        'retention_unit',
      });
    });

    test('setLogEnabled 失败不改本地值也不上抛', () async {
      final fake = FakeInvoke();
      fake.errors['proxy_log_settings_set'] = StateError('io');
      var notified = false;
      final c = SystemSettingsController(
        invoke: fake.fn,
        onLogSettingsChanged: (_) => notified = true,
      );
      await c.setLogEnabled(true);
      expect(c.logEnabled, isFalse);
      expect(notified, isFalse);
    });

    test('updateLogSettings 以当前 state 为底做局部覆盖', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn)
        ..logEnabled = true
        ..logRetention = 90
        ..logRetentionUnit = RetentionUnit.day;
      await c.updateLogSettings({'retention_days': 5, 'retention_unit': 'week'});

      final sent = fake.lastCallTo('proxy_log_settings_set')!.args!['settings']! as Map;
      expect(sent['retention_days'], 5);
      expect(sent['retention_unit'], 'week');
      expect(sent['enabled'], isTrue, reason: '未覆盖的字段取当前 state');
    });

    test('RetentionUnit 未知值回落 day（老配置缺字段）', () {
      expect(RetentionUnit.parse(null), RetentionUnit.day);
      expect(RetentionUnit.parse('month'), RetentionUnit.day);
      expect(RetentionUnit.parse('week'), RetentionUnit.week);
    });

    test('应用日志设置：只传一个字段，其余取当前值', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn)
        ..logFileEnabled = true
        ..logLevel = 'info'
        ..logRetHours = 3;
      await c.updateAppLogSettings(level: 'trace');

      expect(c.logLevel, 'trace');
      expect(fake.lastCallTo('app_log_settings_set')?.args, {
        'settings': {'file_enabled': true, 'level': 'trace', 'retention_hours': 3},
      });
    });
  });

  group('数据库与统计', () {
    test('compactDb 算出 MB 与节省百分比', () async {
      final fake = FakeInvoke({
        'db_compact': {'before_bytes': 20 * 1024 * 1024, 'after_bytes': 5 * 1024 * 1024},
      });
      final c = SystemSettingsController(invoke: fake.fn);
      await c.compactDb((b, a, p) => '$b MB → $a MB（省 $p%）');

      expect(c.message, '20.0 MB → 5.0 MB（省 75%）');
      expect(c.dbCompacting, isFalse, reason: 'finally 必须复位');
    });

    test('compactDb 在 before_bytes=0 时百分比取 0，不除零', () async {
      final fake = FakeInvoke({
        'db_compact': {'before_bytes': 0, 'after_bytes': 0},
      });
      final c = SystemSettingsController(invoke: fake.fn);
      await c.compactDb((b, a, p) => p);
      expect(c.message, '0');
    });

    test('compactDb 失败也把 dbCompacting 复位', () async {
      final fake = FakeInvoke();
      fake.errors['db_compact'] = StateError('locked');
      final c = SystemSettingsController(invoke: fake.fn);
      await c.compactDb((b, a, p) => 'x');
      expect(c.dbCompacting, isFalse);
      expect(c.message, contains('locked'));
    });

    test('rebuildStats 成功后复位并给完成提示', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn);
      await c.rebuildStats('已重建');
      expect(c.statsRebuilding, isFalse);
      expect(c.message, '已重建');
      expect(fake.commands, contains('stats_rebuild_from_logs'));
    });

    test('setStatsRetention 立即改本地值并写回', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn);
      await c.setStatsRetention(30);
      expect(c.statsRetention, 30);
      expect(fake.lastCallTo('stats_settings_set')?.args, {
        'settings': {'retention_days': 30},
      });
    });

    test('setTimeouts 同时写两个值', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn);
      await c.setTimeouts(600, 30);
      expect(c.reqTimeout, 600);
      expect(c.connTimeout, 30);
      expect(fake.lastCallTo('proxy_timeout_set')?.args, {
        'settings': {'request_timeout_secs': 600, 'connect_timeout_secs': 30},
      });
    });
  });

  group('上游代理客户端', () {
    test('先改本地值再写回；写失败**不回滚**（与 React 版一致）', () async {
      final fake = FakeInvoke();
      fake.errors['proxy_client_set_settings'] = StateError('bad host');
      final c = SystemSettingsController(invoke: fake.fn);
      await c.updateProxyClient(ProxyClientSettings.initial.copyWith(host: '1.2.3.4'));

      expect(c.proxyClient.host, '1.2.3.4', reason: 'React 版也不回滚，行为不同就是 bug');
      expect(c.message, contains('bad host'));
    });

    test('整份 8 字段按 snake_case 发出去', () async {
      final fake = FakeInvoke();
      final c = SystemSettingsController(invoke: fake.fn);
      await c.updateProxyClient(ProxyClientSettings.initial.copyWith(enabled: true));
      final sent = fake.lastCallTo('proxy_client_set_settings')!.args!['settings']! as Map;
      expect(sent.keys.toSet(), {
        'enabled',
        'proxy_type',
        'host',
        'port',
        'username',
        'password',
        'dns_over_proxy',
        'no_proxy',
      });
      expect(sent['enabled'], isTrue);
      expect(sent['proxy_type'], 'socks5');
    });
  });

  group('日志维护命令', () {
    test('三个命令名与 React 一致', () async {
      final fake = FakeInvoke({
        'proxy_log_cleanup_estimate': {'rows': 10},
      });
      final c = SystemSettingsController(invoke: fake.fn);
      expect(await c.cleanupEstimate(), {'rows': 10});
      await c.cleanupExpired();
      await c.clearLogs();
      expect(fake.commands, {
        'proxy_log_cleanup_estimate',
        'proxy_log_cleanup_expired',
        'proxy_log_clear',
      });
    });
  });
}
