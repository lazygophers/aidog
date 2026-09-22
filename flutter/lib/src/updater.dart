/// 桌面壳的自动更新（票 I13）。macOS = Sparkle（EdDSA），Windows = WinSparkle（DSA）。
///
/// **共存铁律**：本壳只读 `flutter-appcast.xml`，老 Tauri 壳只读 `latest.json`——两个
/// feed 文件互不相认，格式也不兼容（Sparkle 的 RSS appcast vs Tauri 的 JSON + minisign
/// 签名），谁也不可能装上对方的产物。
library;

import 'dart:io';

import 'package:auto_updater/auto_updater.dart';
import 'package:flutter/foundation.dart';

/// 生产 feed：与 Tauri 的 `latest.json` 同一个 Release、不同文件。
const String kUpdateFeedUrl =
    'https://github.com/lazygophers/aidog/releases/latest/download/flutter-appcast.xml';

/// 本地 mock 更新源的逃生口（验收「更新流程实跑一次」用）：
/// `AIDOG_UPDATE_FEED=http://127.0.0.1:PORT/appcast.xml` 且设置时，启动即弹一次检查。
const String _kFeedEnv = 'AIDOG_UPDATE_FEED';

/// auto_updater 只覆盖 macOS / Windows；Linux 与 `flutter test` 宿主都不碰插件
///（widget 测试里平台通道不存在，调了就是 MissingPluginException）。
bool get desktopUpdaterSupported =>
    (Platform.isMacOS || Platform.isWindows) &&
    Platform.environment['FLUTTER_TEST'] == null;

/// 关于页「软件更新」那块的状态。React 用四态在页内打一行字
///（`About.tsx:82-95`：checking / uptodate / error + 原始错误串），Flutter 这边
/// 状态只有 Sparkle 知道，所以由监听器广播出来给页面订阅。
enum UpdateState { idle, checking, upToDate, error }

/// `(状态, 错误原文)`。错误原文只在 [UpdateState.error] 时非空。
final ValueNotifier<(UpdateState, String)> updateStatus =
    ValueNotifier((UpdateState.idle, ''));

/// Sparkle / WinSparkle 的回调只会走到监听器里：不挂监听器，feed 打不开、签名对不上、
/// XML 解析失败全都是**静默无事发生**——这正是「检查更新点了没反应」这类报告没法查的原因。
class _UpdaterLog with UpdaterListener {
  @override
  void onUpdaterError(UpdaterError? error) {
    debugPrint('[updater] error: $error');
    updateStatus.value = (UpdateState.error, '$error');
  }

  @override
  void onUpdaterCheckingForUpdate(Appcast? appcast) {
    debugPrint('[updater] checking: ${appcast?.items.length ?? 0} item(s)');
    updateStatus.value = (UpdateState.checking, '');
  }

  @override
  void onUpdaterUpdateAvailable(AppcastItem? item) {
    debugPrint('[updater] available: ${item?.displayVersionString}');
    // 有新版本时 Sparkle 自己弹原生窗接管后续，页内不再重复报一遍。
    updateStatus.value = (UpdateState.idle, '');
  }

  @override
  void onUpdaterUpdateNotAvailable(UpdaterError? error) {
    debugPrint('[updater] up to date${error == null ? '' : ' ($error)'}');
    updateStatus.value = (UpdateState.upToDate, '');
  }

  @override
  void onUpdaterUpdateDownloaded(AppcastItem? item) =>
      debugPrint('[updater] downloaded: ${item?.displayVersionString}');

  @override
  void onUpdaterBeforeQuitForUpdate(AppcastItem? item) =>
      debugPrint('[updater] quitting to install: ${item?.displayVersionString}');
}

/// 主窗口启动时调一次（托盘小窗的 popoverMain **不**调，Sparkle 一个进程一份）。
Future<void> initDesktopUpdater() async {
  if (!desktopUpdaterSupported) return;
  autoUpdater.addListener(_UpdaterLog());
  final feed = Platform.environment[_kFeedEnv] ?? kUpdateFeedUrl;
  await autoUpdater.setFeedURL(feed);
  await autoUpdater.setScheduledCheckInterval(86400);
  if (Platform.environment.containsKey(_kFeedEnv)) {
    await autoUpdater.checkForUpdates();
  }
}

/// 关于页「检查更新」按钮：Sparkle 自己接管后续 UI（下载进度、安装并重启）。
Future<void> checkForAppUpdates() async {
  if (!desktopUpdaterSupported) return;
  // 先置 checking：Sparkle 的 checking 回调不保证先于本行到达，按钮要当场变灰。
  updateStatus.value = (UpdateState.checking, '');
  try {
    await autoUpdater.checkForUpdates();
  } catch (e) {
    updateStatus.value = (UpdateState.error, '$e');
  }
}
