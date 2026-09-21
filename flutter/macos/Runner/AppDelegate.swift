import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  /// 关掉主窗口**不退出**（用户 2026-09-21 定）。这是常驻菜单栏类应用的常规行为，
  /// 也与 Tauri 壳一致：那边有托盘在，关掉主窗口后进程照常活着代理流量。
  /// 真要退出走菜单栏右键的 Quit（`MenuBar.swift` 的 `aidogMenuBarOnQuit`）。
  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return false
  }

  /// 点 Dock 图标时把主窗口找回来 —— 关窗后 Dock 图标还在，点了没反应会让人以为卡死。
  override func applicationShouldHandleReopen(
    _ sender: NSApplication, hasVisibleWindows flag: Bool
  ) -> Bool {
    if !flag {
      NSApp.windows.first(where: { $0 is MainFlutterWindow })?.makeKeyAndOrderFront(nil)
    }
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
