/// 系统通知设置（`settings/notifications`）—— 对齐
/// `src/components/settings/NotificationSettings.tsx`。
///
/// 三条容易搬丢的规则：
/// ① **写 DB 防抖 200 ms**：模板连续输入合并成一次持久化；失败回滚到写前快照，
///    保证界面与 DB 一致（否则界面显示新值、DB 存旧值，用户看不出「没生效」）。
/// ② **总开关关掉时，「默认注入 hook」强制禁用**（通知都不发，hook 没意义）。
/// ③ **开启默认 hook 前要先确保脚本执行器就绪**：hook 脚本是 Python，uv 缺失时
///    弹框让用户选「装 uv」还是「用 python3」，选完才继续；取消就整个不做。
library;

import 'dart:async';

import '../invoke.dart';

const List<String> kTtsBackends = ['cross_platform', 'mac_say', 'web_speech'];

/// Ventura/13+ 的「系统设置 → 通知」面板；打不开时回退旧 scheme。
const String kMacNotifSettingsUrl =
    'x-apple.systempreferences:com.apple.Notifications-Settings.extension';
const String kMacNotifSettingsUrlLegacy =
    'x-apple.systempreferences:com.apple.preference.notifications';

class NotificationSettings {
  const NotificationSettings({
    required this.enabled,
    required this.ttsEnabled,
    required this.ttsBackend,
    required this.perType,
    required this.perEvent,
    required this.inboxRetentionDays,
  });

  /// `NotificationSettings.tsx:60-67` 的 DEFAULT_SETTINGS，一字不改。
  static const defaults = NotificationSettings(
    enabled: true,
    ttsEnabled: true,
    ttsBackend: 'cross_platform',
    perType: <String, Object?>{},
    perEvent: <String, Object?>{},
    inboxRetentionDays: 7,
  );

  final bool enabled;
  final bool ttsEnabled;
  final String ttsBackend;
  final Map<String, Object?> perType;
  final Map<String, Object?> perEvent;
  final int inboxRetentionDays;

  factory NotificationSettings.fromJson(Map<String, Object?> j) =>
      NotificationSettings(
        enabled: j['enabled'] as bool? ?? true,
        ttsEnabled: j['tts_enabled'] as bool? ?? true,
        ttsBackend: j['tts_backend'] as String? ?? 'cross_platform',
        perType: Map<String, Object?>.from((j['per_type'] as Map?) ?? const {}),
        perEvent: Map<String, Object?>.from(
          (j['per_event'] as Map?) ?? const {},
        ),
        inboxRetentionDays: (j['inbox_retention_days'] as num?)?.toInt() ?? 7,
      );

  Map<String, Object?> toJson() => {
    'enabled': enabled,
    'tts_enabled': ttsEnabled,
    'tts_backend': ttsBackend,
    'per_type': perType,
    'per_event': perEvent,
    'inbox_retention_days': inboxRetentionDays,
  };

  NotificationSettings copyWith({
    bool? enabled,
    bool? ttsEnabled,
    String? ttsBackend,
    Map<String, Object?>? perType,
    Map<String, Object?>? perEvent,
    int? inboxRetentionDays,
  }) => NotificationSettings(
    enabled: enabled ?? this.enabled,
    ttsEnabled: ttsEnabled ?? this.ttsEnabled,
    ttsBackend: ttsBackend ?? this.ttsBackend,
    perType: perType ?? this.perType,
    perEvent: perEvent ?? this.perEvent,
    inboxRetentionDays: inboxRetentionDays ?? this.inboxRetentionDays,
  );
}

/// uv 缺失时要用户拍板的三选一。
enum UvChoice { installUv, usePython3, cancel }

class NotificationTexts {
  const NotificationTexts({
    required this.testSent,
    required this.testTtsContent,
    required this.testPopupTitle,
    required this.testPopupBody,
    required this.uvInstalled,
    required this.uvInstallFailed,
    required this.defaultHooksOn,
    required this.defaultHooksOff,
  });

  final String testSent;
  final String testTtsContent;
  final String testPopupTitle;
  final String testPopupBody;
  final String uvInstalled;
  final String uvInstallFailed;
  final String defaultHooksOn;
  final String defaultHooksOff;
}

class NotificationsController {
  NotificationsController({
    InvokeFn? invoke,
    this.onChanged,
    this.onEnabledChanged,
    this.openUrlFn,
    Duration? persistDebounce,
  }) : _invoke = invoke ?? kernelInvoke,
       persistDebounce = persistDebounce ?? const Duration(milliseconds: 200);

  final InvokeFn _invoke;
  final void Function()? onChanged;

  /// 总开关变化要上抛给外壳（关掉通知时侧栏藏掉「通知」项）。
  final void Function(bool enabled)? onEnabledChanged;

  /// 打开外部链接（I12 的 `openUrl`）。测试传假的。
  final Future<void> Function(String url)? openUrlFn;

  final Duration persistDebounce;

  NotificationSettings settings = NotificationSettings.defaults;
  bool loading = true;
  String error = '';
  String message = '';
  bool defaultHooks = false;
  bool defaultHooksBusy = false;

  /// 非空 = uv 询问框开着，等用户选。
  Completer<UvChoice>? uvPrompt;
  bool uvInstalling = false;

  Timer? _persistTimer;

  /// 通知总开关关掉时，「默认注入 hook」开关强制禁用。
  bool get hooksDisabled => !settings.enabled;

  void _notify() => onChanged?.call();

  Future<void> load() async {
    try {
      settings = NotificationSettings.fromJson(
        _map(await _invoke('notification_settings_get')),
      );
    } catch (_) {
      /* console.error；保留默认值 */
    }
    try {
      defaultHooks =
          await _invoke('get_default_hooks_enabled') as bool? ?? false;
    } catch (_) {
      /* console.error；保留 false */
    }
    loading = false;
    _notify();
  }

  /// 乐观改本地值 → 防抖写 DB → 失败回滚到写前快照。
  void persist(
    NotificationSettings Function(NotificationSettings prev) updater,
  ) {
    final prev = settings;
    final next = updater(prev);
    settings = next;
    error = '';
    if (next.enabled != prev.enabled) onEnabledChanged?.call(next.enabled);
    _notify();

    _persistTimer?.cancel();
    _persistTimer = Timer(persistDebounce, () async {
      try {
        await _invoke('notification_settings_set', {
          'settings': settings.toJson(),
        });
      } catch (e) {
        // 回滚到写前快照，界面与 DB 一致。
        settings = prev;
        error = '$e';
        _notify();
      }
    });
  }

  /// 测试用：跳过防抖立刻落盘（widget 里由 Timer 自己到点）。
  Future<void> flushPersist() async {
    if (_persistTimer?.isActive ?? false) {
      _persistTimer!.cancel();
      try {
        await _invoke('notification_settings_set', {
          'settings': settings.toJson(),
        });
      } catch (e) {
        error = '$e';
        _notify();
      }
    }
  }

  void dispose() => _persistTimer?.cancel();

  void toggleEnabled() => persist((p) => p.copyWith(enabled: !p.enabled));

  void toggleTts() => persist((p) => p.copyWith(ttsEnabled: !p.ttsEnabled));

  void setTtsBackend(String b) => persist((p) => p.copyWith(ttsBackend: b));

  void setInboxRetentionDays(int d) =>
      persist((p) => p.copyWith(inboxRetentionDays: d));

  /// 逐事件配置写进 `per_event[event]`。
  void updateEvent(String event, Map<String, Object?> setting) =>
      persist((p) => p.copyWith(perEvent: {...p.perEvent, event: setting}));

  // ── 四个独立通道测试（绕过 dispatch，便于诊断）──

  Future<void> testNotify(NotificationTexts texts) async {
    try {
      await _invoke('notification_test', {
        'notifType': 'task_complete',
        'content': null,
      });
      message = texts.testSent;
    } catch (e) {
      message = '$e';
    }
    _notify();
  }

  Future<void> testTts(NotificationTexts texts) async {
    try {
      await _invoke('notification_test_tts', {'text': texts.testTtsContent});
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  Future<void> testPopup(NotificationTexts texts) async {
    try {
      await _invoke('notification_test_popup', {
        'title': texts.testPopupTitle,
        'body': texts.testPopupBody,
      });
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  Future<void> testBeep() async {
    try {
      await _invoke('notification_test_beep');
    } catch (e) {
      message = '$e';
      _notify();
    }
  }

  // ── 脚本执行器就绪 + 默认 hook 开关 ──

  /// uv 可用就直接放行；否则开询问框等用户选。返回 false = 用户取消注入。
  Future<bool> ensureExecutorReady() async {
    try {
      if (await _invoke('check_uv') as bool? ?? false) return true;
    } catch (_) {
      /* console.error，继续弹框 */
    }
    final c = Completer<UvChoice>();
    uvPrompt = c;
    _notify();
    final choice = await c.future;
    return choice != UvChoice.cancel;
  }

  /// 「自动装 uv」。装失败也**不挡**注入：退回 python3 仍能生成脚本。
  Future<void> chooseInstallUv(NotificationTexts texts) async {
    final p = uvPrompt;
    if (p == null) return;
    uvInstalling = true;
    _notify();
    try {
      await _invoke('install_uv');
      message = texts.uvInstalled;
    } catch (_) {
      message = texts.uvInstallFailed;
      try {
        await _invoke('set_script_executor', {'executor': 'python3'});
      } catch (_) {
        /* best-effort */
      }
    }
    uvInstalling = false;
    uvPrompt = null;
    p.complete(UvChoice.installUv);
    _notify();
  }

  Future<void> chooseUsePython3() async {
    final p = uvPrompt;
    if (p == null) return;
    try {
      await _invoke('set_script_executor', {'executor': 'python3'});
    } catch (_) {
      /* console.error */
    }
    uvPrompt = null;
    p.complete(UvChoice.usePython3);
    _notify();
  }

  void cancelUvPrompt() {
    final p = uvPrompt;
    if (p == null) return;
    uvPrompt = null;
    p.complete(UvChoice.cancel);
    _notify();
  }

  /// 开启会为全分组生成 Python hook 脚本 → 先确保执行器就绪；用户取消就整个不做。
  Future<void> toggleDefaultHooks(NotificationTexts texts) async {
    final next = !defaultHooks;
    if (next && !await ensureExecutorReady()) return;
    defaultHooksBusy = true;
    defaultHooks = next;
    _notify();
    try {
      await _invoke('set_default_hooks_enabled', {'enabled': next});
      message = next ? texts.defaultHooksOn : texts.defaultHooksOff;
    } catch (e) {
      defaultHooks = !next;
      message = '$e';
    }
    defaultHooksBusy = false;
    _notify();
  }

  /// 打开 macOS 系统通知设置。新 scheme 失败回退旧 scheme，都失败才报错。
  Future<void> openSystemNotificationSettings() async {
    final open = openUrlFn;
    if (open == null) return;
    try {
      await open(kMacNotifSettingsUrl);
    } catch (_) {
      try {
        await open(kMacNotifSettingsUrlLegacy);
      } catch (e2) {
        message = '$e2';
        _notify();
      }
    }
  }

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}

/// 无界面内核**管理面**设置（`settings/system` 里的一段）—— 对齐
/// `src/pages/AppSettings/KernelSection.tsx`。
///
/// 管理面永远只监听 127.0.0.1，**没有开放到局域网的开关**。
/// `StartupSection` 里代理的「局域网访问」管的是转发端口，是另一个维度。
class KernelSettingsController {
  KernelSettingsController({InvokeFn? invoke, this.onChanged})
    : _invoke = invoke ?? kernelInvoke;

  final InvokeFn _invoke;
  final void Function()? onChanged;

  /// null = 还没读到 → 整段不渲染（React 的 `if (!settings) return null`）。
  ({int port, String authToken})? settings;
  String token = '';
  String error = '';
  String saved = '';

  void _notify() => onChanged?.call();

  Future<void> load() async {
    try {
      final s = _map(await _invoke('kernel_settings_get'));
      settings = (
        port: (s['port'] as num?)?.toInt() ?? 9891,
        authToken: s['auth_token'] as String? ?? '',
      );
    } catch (_) {
      // 读失败也要给一份默认，否则这一段永远不显示。
      settings = (port: 9891, authToken: '');
    }
    token = settings!.authToken;
    _notify();
  }

  void setToken(String v) {
    token = v;
    _notify();
  }

  /// onBlur 提交：**与已存值不同才写**（每次失焦都写会白打一遍 DB）。
  Future<void> commitToken(String savedText) async {
    final s = settings;
    if (s == null || token == s.authToken) return;
    error = '';
    try {
      await _invoke('kernel_settings_set', {
        'settings': {'port': s.port, 'auth_token': token},
      });
      settings = (port: s.port, authToken: token);
      saved = savedText;
    } catch (e) {
      error = '$e';
    }
    _notify();
  }

  void clearSavedFlash() {
    saved = '';
    _notify();
  }

  static Map<String, Object?> _map(Object? v) =>
      v is Map ? Map<String, Object?>.from(v) : <String, Object?>{};
}
