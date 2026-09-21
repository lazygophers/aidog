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

/// 小窗宽高上下限（逐个对齐 React 的 `MIN_W` / `MAX_W` / `MIN_H` / `MAX_H`，
/// `src/popover.tsx:65-68`）。
///
/// **这四个数是唯一真值源**：Swift 侧只留一个「内容还没量出来之前先挂着」的占位宽，
/// 不再抄一份上下限（票 I20 之前 340 这个数在两边各写了一遍）。
const double kTrayPanelMinWidth = 300;
const double kTrayPanelMaxWidth = 480;
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

/// 小窗内容尺寸变了 → 让 Swift 改窗。两边都夹在上下限之间，超出部分由内容自己滚。
Future<void> reportTrayPanelSize(double width, double height) async {
  if (!trayPanelSupported) return;
  await kTrayPanelContentChannel.invokeMethod<void>('resize', <String, Object?>{
    'w': width.clamp(kTrayPanelMinWidth, kTrayPanelMaxWidth),
    'h': height.clamp(kTrayPanelMinHeight, kTrayPanelMaxHeight),
  });
}

/// 小窗自己请求关闭（Esc）。
Future<void> closeTrayPanelFromContent() async {
  if (!trayPanelSupported) return;
  await kTrayPanelContentChannel.invokeMethod<void>('close');
}
