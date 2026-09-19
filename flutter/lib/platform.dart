/// 桌面原生能力（票 I12）。React 版的 `src/services/platform.ts` +
/// `src/services/pathPicker.ts` + `src/services/updater.ts::relaunch` 在 Flutter 侧的对应物。
///
/// 需求文档就是 `src/services/platform.ts:7-15` 那张表。这里按**同一份语义**逐条落地，
/// 只把「桌面 = Tauri 插件 / 浏览器 = Web API」两列换成「Flutter 包 / 无该能力时的降级」：
///
/// | 能力 | React 桌面（Tauri 插件） | 本层（Flutter 包） | 降级 |
/// |---|---|---|---|
/// | 写剪贴板 | `plugin-clipboard-manager` | SDK `Clipboard.setData` | 无需降级 |
/// | 读剪贴板 | `plugin-clipboard-manager` | SDK `Clipboard.getData` | 空剪贴板 → `''` |
/// | 打开外部链接 | `plugin-opener::openUrl` | `url_launcher` | 打不开 → 抛（调用方原有失败路径） |
/// | 在文件管理器里定位 | `plugin-opener::revealItemInDir` | `dart:io` `Process`（`open -R` / `explorer /select,`） | 该 OS 无定位命令 → **复制路径到剪贴板**（照抄 platform.ts:95-98） |
/// | 应用版本号 | `@tauri-apps/api/app` | 后端 `about_info().app_version` | 同 platform.ts 的浏览器列 |
/// | 系统通知 | `plugin-notification` | `flutter_local_notifications` | 不可用 → 静默跳过，不拖崩调用方 |
/// | 文件 / 目录 / 保存对话框 | `plugin-dialog` | `file_selector` | 无（两端都是真原生面板） |
/// | 重启应用 | `plugin-process::relaunch` | `dart:io` `Process.start(detached)` + `exit(0)` | 无 |
///
/// **`fs` 与 `shell` 不在本层**：React 版前端对这两个 Tauri 插件零调用
/// （`plugin-shell` 早已迁到后端命令 `mitm_install_ca`，见 `src/services/api/mitm.ts:81`；
/// `plugin-fs` 全库只在 `package.json` 里，没有 import 点）。要读写文件的都在 Rust 侧，
/// 经 RPC 命令走。`path_provider` 同理 —— 数据目录由内核自己定，外壳不需要第二份。
///
/// **macOS 权限**：I01 已关掉 App Sandbox（沙箱会挡住外壳连自己的内核）。所以
/// `file_selector` 不需要 `com.apple.security.files.user-selected.read-write`、
/// `url_launcher` 不需要 `com.apple.security.network.client`，`Process.start` 也不受限。
/// 哪天把沙箱打开，这三项要同时补 entitlement，否则会静默失败。
library;

import 'dart:async';
import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/foundation.dart' show defaultTargetPlatform;
import 'package:flutter/services.dart' show Clipboard, ClipboardData;
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:url_launcher/url_launcher.dart';

import 'transport.dart';

// ─── 剪贴板 ─────────────────────────────────────────────────

/// 写剪贴板。对应 `platform.ts::writeText`。
Future<void> writeText(String text) =>
    Clipboard.setData(ClipboardData(text: text));

/// 读剪贴板。对应 `platform.ts::readText`。
///
/// 剪贴板里没有文本时返回 `''`，不抛 —— React 版那条 `throw`（platform.ts:72-74）针对的是
/// 「浏览器非安全上下文里 `navigator.clipboard` 根本不存在」，桌面壳没有这种形态。
Future<String> readText() async =>
    (await Clipboard.getData(Clipboard.kTextPlain))?.text ?? '';

// ─── 打开链接 / 定位文件 ──────────────────────────────────────

/// 用系统默认浏览器打开外部链接。对应 `platform.ts::openUrl`。
///
/// 打不开就抛（`url_launcher` 返回 false 时）—— 静默吞掉会让「点了没反应」查不出原因。
Future<void> openUrl(String url) async {
  final uri = Uri.parse(url);
  if (!await launchUrl(uri, mode: LaunchMode.externalApplication)) {
    throw StateError('cannot open url: $url');
  }
}

/// 「在文件管理器里定位」的命令行形态。`null` = 该 OS 上没有这回事。
///
/// 拆成纯函数是为了能不真的弹访达就把命令拼法测掉。`os` 仅测试传，生产用当前系统。
List<String>? revealCommand(String path, {String? os}) =>
    switch (os ?? Platform.operatingSystem) {
      // macOS：`open -R` = 打开访达并选中该项（不是打开该文件）。
      'macos' => <String>['open', '-R', path],
      // Windows：`/select,<path>` 必须是**同一个 argv**，逗号后不能有空格。
      'windows' => <String>['explorer', '/select,$path'],
      _ => null,
    };

/// 当前形态能否真的「在文件管理器里定位」。对应 `platform.ts::canRevealItemInDir`。
/// UI 要据此换按钮文案，别让用户以为窗口会弹出来。
bool canRevealItemInDir() => revealCommand('x') != null;

/// 在系统文件管理器里定位文件。对应 `platform.ts::revealItemInDir`。
///
/// 没有定位命令的 OS 上**退化成把路径复制到剪贴板** —— 与 React 版的浏览器降级
/// （platform.ts:95-98）一字不差的语义。
///
/// `os` 仅测试传（在 macOS 上也能真跑一遍降级分支），生产用当前系统。
Future<void> revealItemInDir(String path, {String? os}) async {
  final cmd = revealCommand(path, os: os);
  if (cmd == null) return writeText(path);
  // `explorer /select,` 即使成功也返回 1；退出码在这里没有信息量，不据此判失败。
  await Process.run(cmd.first, cmd.sublist(1));
}

// ─── 应用版本 ────────────────────────────────────────────────

/// 应用版本号。对应 `platform.ts::getAppVersion` 的**浏览器**那条路：问后端要。
///
/// Flutter 外壳没有 Tauri 的 `getVersion()`，而 `about_info().app_version` 本来就是同一个值
/// （都来自 `tauri.conf.json` 的 `version`），没有第二个真值源可对不齐。
Future<String> getAppVersion({Kernel? k}) async {
  final info = await (k ?? kernel).invoke<Map<String, dynamic>>('about_info');
  return info['app_version'] as String;
}

// ─── 文件对话框 ──────────────────────────────────────────────

/// 一次路径选择的参数。字段照抄 `pathPicker.ts::PickPathOptions`。
class PickPathOptions {
  const PickPathOptions({
    this.directory = false,
    this.save = false,
    this.defaultPath,
    this.title,
    this.filters = const <PickFilter>[],
  });

  /// 选目录（默认选文件）。与 [save] 互斥。
  final bool directory;

  /// 选「保存到哪」而非「打开哪个」。
  final bool save;

  /// 保存模式的预填文件名 / 打开模式的初始目录。
  final String? defaultPath;

  /// 确认按钮文案。
  ///
  /// **与 Tauri 的差异（唯一一处，照实写）**：Tauri 的 `title` 设的是对话框**窗口标题**，
  /// `file_selector` 只给 `confirmButtonText`（确认按钮文案），没有 title 入参。
  /// macOS 现代的 NSOpenPanel/NSSavePanel 本来就不显示窗口标题，观感无差；Windows 上
  /// 标题栏会退回系统默认文案（「打开」/「另存为」）。
  final String? title;

  /// 扩展名过滤。
  final List<PickFilter> filters;
}

/// 扩展名过滤项。照抄 `pathPicker.ts` 的 `{ name, extensions }`。
class PickFilter {
  const PickFilter(this.name, this.extensions);
  final String name;
  final List<String> extensions;
}

/// 过滤项 → `file_selector` 的类型组。纯函数，好测。
List<XTypeGroup> typeGroups(List<PickFilter> filters) => filters
    .map((f) => XTypeGroup(label: f.name, extensions: f.extensions))
    .toList();

/// 选一个本地路径。返回绝对路径；用户取消返回 `null`。对应 `pathPicker.ts::pickPath`。
Future<String?> pickPath([
  PickPathOptions options = const PickPathOptions(),
]) async {
  if (options.save) {
    final loc = await getSaveLocation(
      suggestedName: options.defaultPath,
      acceptedTypeGroups: typeGroups(options.filters),
      confirmButtonText: options.title,
    );
    return loc?.path;
  }
  if (options.directory) {
    return getDirectoryPath(
      initialDirectory: options.defaultPath,
      confirmButtonText: options.title,
    );
  }
  final file = await openFile(
    acceptedTypeGroups: typeGroups(options.filters),
    initialDirectory: options.defaultPath,
    confirmButtonText: options.title,
  );
  return file?.path;
}

// ─── 进程 / 重启 ─────────────────────────────────────────────

/// 重启自己要跑的命令。纯函数，好测。
///
/// macOS 上 [Platform.resolvedExecutable] 是 `AiDog.app/Contents/MacOS/AiDog`，直接拉起即可；
/// Windows 上是那个 `.exe`。两边都不需要 `open -a` / 壳层包装。
List<String> relaunchCommand() => <String>[
  Platform.resolvedExecutable,
  ...Platform.executableArguments,
];

/// 起一个**脱离本进程**的子进程。`relaunch` 与测试共用同一条 `dart:io` 代码路径。
///
/// `detached` 是重点：本进程马上 `exit(0)`，非 detached 的子进程会跟着一起没。
Future<int> spawnDetached(List<String> command) async {
  final p = await Process.start(
    command.first,
    command.sublist(1),
    mode: ProcessStartMode.detached,
  );
  return p.pid;
}

/// 重启应用。对应 `updater.ts:83` 的 `relaunch()`（`plugin-process`）。
///
/// 正常情况下**不返回** —— 与 Tauri 版一致（`UpdatePromptModal.tsx:36` 那条注释说的就是这个）。
Future<void> relaunch() async {
  await spawnDetached(relaunchCommand());
  exit(0);
}

// ─── 系统通知 ────────────────────────────────────────────────

/// 内核广播「该弹系统通知了」的事件名。必须与 Rust 侧
/// `aidog_notification::NOTIF_POPUP`（`tts.rs`）一字不差。
const String kNotifPopup = 'notif-popup';

/// Windows toast 要的三件套。`appUserModelId` 与 `tauri.conf.json` 的 `identifier` 对齐；
/// `guid` 是本应用的通知激活回调标识，随便换会让已投递的 toast 点不回来，**别改**。
const String kWindowsAppName = 'AiDog';
const String kWindowsAppUserModelId = 'com.aidog.desktop';
const String kWindowsGuid = '4a1b2c3d-5e6f-4a7b-8c9d-0e1f2a3b4c5d';

final FlutterLocalNotificationsPlugin _plugin = FlutterLocalNotificationsPlugin();
bool _initialized = false;
int _nextId = 0;

/// 通知插件的初始化 + 权限申请。幂等；失败返回 `false`（不抛）。
///
/// macOS 的权限在 `DarwinInitializationSettings` 里一次性请求，对齐 Rust 桌面壳启动时的
/// `request_permission()`（`app_setup.rs:406`）。
Future<bool> initNotifications() async {
  if (_initialized) return true;
  try {
    await _plugin.initialize(
      settings: const InitializationSettings(
        macOS: DarwinInitializationSettings(),
        windows: WindowsInitializationSettings(
          appName: kWindowsAppName,
          appUserModelId: kWindowsAppUserModelId,
          guid: kWindowsGuid,
        ),
      ),
    );
    _initialized = true;
    return true;
  } on Object catch (e) {
    // ignore: avoid_print
    print('[platform] notification init failed: $e');
    return false;
  }
}

/// 弹一条系统通知（best-effort，失败不抛）。对应 `platform.ts::notify`。
///
/// 与 React 版同一条纪律：通知**内容**在应用内的通知页里照样看得到，系统级弹窗只是额外提醒，
/// 弹不出来不该把调用方拖崩。
Future<void> notify(String title, String body) async {
  try {
    if (!await initNotifications()) return;
    await _plugin.show(id: _nextId++, title: title, body: body);
  } on Object catch (e) {
    // ignore: avoid_print
    print('[platform] notify failed: $e');
  }
}

/// 把内核广播的弹窗请求接到本地通知上。
///
/// **这条线不接 = 掉功能**：通知的分发在 Rust 侧（`aidog_notification::dispatch`），桌面壳
/// 那边由 `TauriCtx::show_popup` 直接调 tauri 插件弹；无界面内核没有桌面会话，只能把请求
/// 广播出来（`HeadlessCtx::show_popup` → [kNotifPopup]），由外壳代弹。外壳不订阅，所有
/// 后端通知（额度告警、任务完成……）的弹窗通道就全哑了。
///
/// 在 `main()` 里 `kernel.start()` 之后调一次。
StreamSubscription<Map<String, dynamic>> bindKernelPopups({Kernel? k}) =>
    (k ?? kernel).on<Map<String, dynamic>>(kNotifPopup).listen((e) {
      notify(e['title'] as String? ?? '', e['body'] as String? ?? '');
    });

/// 当前平台在定时 / 重复通知上的缺口说明；`null` = 无缺口。
///
/// `flutter_local_notifications` 的已知空洞：macOS 不实现旧的 `schedule` /
/// `showDailyAtTime` / `showWeeklyAtDayAndTime`，Linux 完全没有调度 API，Windows 的
/// `periodicallyShow*` 抛 `UnsupportedError`。
///
/// **本项目一条都没用到**：全库唯一的系统通知入口是「立刻弹一条」（React 版
/// `platform.ts::notify` 只被 `App.tsx:156` 的 `proxy-start-failed` 调用，Rust 侧
/// `aidog_notification::show_popup` 也只有立即弹这一种）。定时/重复那一层在 Rust 的
/// 定时任务里（`app_setup.rs` 的 cycle），到点了才调「立刻弹」——调度权从来不在通知插件手上。
/// 所以这些缺口对现有行为零影响。函数留着是为了**哪天真要定时通知时当场能看见代价**。
String? schedulingGap() => switch (defaultTargetPlatform.name) {
  // 注意：22.3.1 的 macOS 侧**有** zonedSchedule，缺的是那三个旧 API。
  'macOS' => 'no schedule / showDailyAtTime / showWeeklyAtDayAndTime',
  'linux' => 'no scheduling API at all',
  'windows' => 'periodicallyShow* throws UnsupportedError',
  _ => null,
};
