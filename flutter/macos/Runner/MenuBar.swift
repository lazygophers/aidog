import Cocoa
import FlutterMacOS

/// 菜单栏（`NSStatusItem`）的 Swift 侧接线（票 I20）。
///
/// ## 为什么这一层这么薄
///
/// 菜单栏那行「多列 / 多字号 / 带颜色」的 `NSAttributedString` 标题由
/// `aidog_core::tray_render` 排版（340 行），抄一份到 Swift 就会漂移。所以 Rust 那份
/// 宿主原样编成 staticlib 链进 Runner（`AidogMenuBar.h`），这里只负责**接线**：
///
/// ```text
/// 内核 menu_bar_state ─RPC→ Dart ─method channel→ 本文件 ─C ABI→ aidog_menubar_render
///                                                        ↑
/// 点图标 ─AppKit→ aidog_core::menubar ─C 回调→ 本文件 ─→ TrayPanel.show / Dart
/// ```
///
/// ## 两条约束
///
/// 1. `install` / `render` 必须在主线程（AppKit）。method channel 的 handler 本来就在
///    主线程上跑，所以这里不需要额外 dispatch。
/// 2. 回调是 `@convention(c)` 函数指针，**不能带捕获** —— 所以四个 handler 是文件级
///    全局函数，状态走 `MenuBar.shared`。
final class MenuBar {
  static let shared = MenuBar()

  /// 回主引擎的通道。菜单里的「启停代理」要打内核 RPC，只有 Dart 那边有连接。
  fileprivate var channel: FlutterMethodChannel?

  /// 主窗口引擎注册 `aidog/menubar`，并在同一刻把 NSStatusItem 建出来。
  ///
  /// 建条目不等 Dart：菜单栏图标该在启动那一瞬就出现，数据晚几百毫秒到无所谓
  /// （`render` 之前 Rust 侧显示的是 app logo）。
  static func register(messenger: FlutterBinaryMessenger) {
    let channel = FlutterMethodChannel(name: "aidog/menubar", binaryMessenger: messenger)
    shared.channel = channel
    channel.setMethodCallHandler { call, result in
      switch call.method {
      case "render":
        guard let json = call.arguments as? String else {
          result(
            FlutterError(
              code: "bad-args", message: "render expects a JSON string", details: nil))
          return
        }
        json.withCString { aidog_menubar_render($0) }
        result(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
    }
    shared.install()
  }

  private func install() {
    aidog_menubar_install(
      AidogMenuBarActions(
        on_click: aidogMenuBarOnClick,
        on_toggle_proxy: aidogMenuBarOnToggleProxy,
        on_show: aidogMenuBarOnShow,
        on_quit: aidogMenuBarOnQuit))
  }

  /// 回 Dart。目前只有「启停代理」一件事要回去（它得打内核 RPC）。
  fileprivate func send(_ method: String, _ arguments: Any? = nil) {
    channel?.invokeMethod(method, arguments: arguments)
  }
}

// MARK: - C 回调（无捕获，故必须是全局函数）

/// 左键点击图标 → 开合小窗。`anchor` 已是左上角原点的逻辑坐标，与
/// `TrayPanel.position(_:anchor:)` 期望的坐标系一致。
private func aidogMenuBarOnClick(x: Double, y: Double, w: Double, h: Double) {
  TrayPanel.shared.toggle(anchor: NSRect(x: x, y: y, width: w, height: h))
}

/// 启停代理要打内核 RPC → 交给 Dart。`running` = 点击时的状态，true 意味着该停。
private func aidogMenuBarOnToggleProxy(running: Bool) {
  MenuBar.shared.send("toggleProxy", running)
}

/// 「Show Window」：主窗口可能已被关掉（`applicationShouldTerminateAfterLastWindowClosed`
/// 之外还有最小化 / 隐藏），先尽量找回来，再前置。这里**允许**抢焦点 —— 用户点的就是
/// 「把主窗口拿过来」，与小窗那条「绝不抢焦点」的禁令不是一回事。
private func aidogMenuBarOnShow() {
  NSApp.activate(ignoringOtherApps: true)
  if let window = NSApp.windows.first(where: { $0 is MainFlutterWindow }) {
    window.makeKeyAndOrderFront(nil)
  }
}

private func aidogMenuBarOnQuit() {
  NSApp.terminate(nil)
}
