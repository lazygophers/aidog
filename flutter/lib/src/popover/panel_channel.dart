/// 托盘小窗的原生外壳通道（票 I11）。
///
/// 外壳是 `macos/Runner/TrayPanel.swift` 里的 `NSPanel(styleMask: .nonactivatingPanel)`，
/// 里面装第二个 `FlutterEngine` 的 `FlutterViewController`。
///
/// **不抢焦点是这一票的全部理由**：`.nonactivatingPanel` + `level = .statusBar` +
/// `collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]` 让小窗能浮在
/// 全屏应用上方而**不切 Space、不切 App**。Swift 侧一律 `orderFrontRegardless()`，
/// 绝不 `makeKeyAndOrderFront` / `NSApp.activate` —— 那两个就是抢焦点。
///
/// **非 macOS 走主窗口**（用户定）：`NSPanel` 没有 Windows 对应物，本层在别的平台上
/// 全部是 no-op，调用方据 [trayPanelSupported] 决定是开小窗还是跳设置页。
library;

import 'dart:io' show Platform;

import 'package:flutter/services.dart';

/// 主引擎侧：控制面板显隐。
const MethodChannel kTrayPanelChannel = MethodChannel('aidog/tray_panel');

/// 小窗引擎侧：内容把自己量出来的高度报回去，由 Swift 改窗高。
const MethodChannel kTrayPanelContentChannel = MethodChannel(
  'aidog/tray_panel_content',
);

/// 本平台有没有非激活面板这回事。false → 调用方并进主窗口。
bool get trayPanelSupported => Platform.isMacOS;

/// 小窗宽度（逻辑像素）。
///
/// ponytail: React 版按内容 300-480 自适应宽；Flutter 的内容宽度由窗口给定，
/// 反过来测不出「内容想要多宽」。定宽 + 高度自适应，真有人嫌窄再加。
const double kTrayPanelWidth = 340;

/// 小窗高度上下限（对齐 React 的 `MIN_H` / `MAX_H`）。
const double kTrayPanelMinHeight = 80;
const double kTrayPanelMaxHeight = 600;

/// 在菜单栏图标下方弹出小窗。
///
/// [anchor] = 图标在屏幕上的矩形，**左上角原点的逻辑坐标**，与
/// `aidog_core::menubar` 的 `on_click` 回调同坐标系（`menubar.rs:47`）。
Future<void> showTrayPanel(Rect anchor) async {
  if (!trayPanelSupported) return;
  await kTrayPanelChannel.invokeMethod<void>('show', <String, Object?>{
    'x': anchor.left,
    'y': anchor.top,
    'w': anchor.width,
    'h': anchor.height,
  });
}

Future<void> hideTrayPanel() async {
  if (!trayPanelSupported) return;
  await kTrayPanelChannel.invokeMethod<void>('hide');
}

/// 点一下：开着就收，收着就开。菜单栏图标左键点击接这个。
Future<void> toggleTrayPanel(Rect anchor) async {
  if (!trayPanelSupported) return;
  await kTrayPanelChannel.invokeMethod<void>('toggle', <String, Object?>{
    'x': anchor.left,
    'y': anchor.top,
    'w': anchor.width,
    'h': anchor.height,
  });
}

/// 小窗内容高度变了 → 让 Swift 改窗高。夹在 [kTrayPanelMinHeight] 与
/// [kTrayPanelMaxHeight] 之间，超出部分由内容自己滚。
Future<void> reportTrayPanelHeight(double height) async {
  if (!trayPanelSupported) return;
  final h = height.clamp(kTrayPanelMinHeight, kTrayPanelMaxHeight);
  await kTrayPanelContentChannel.invokeMethod<void>('resize', <String, Object?>{
    'h': h,
  });
}

/// 小窗自己请求关闭（Esc）。
Future<void> closeTrayPanelFromContent() async {
  if (!trayPanelSupported) return;
  await kTrayPanelContentChannel.invokeMethod<void>('close');
}
