import Cocoa
import FlutterMacOS

/// 菜单栏小窗（票 I11）：手搓的 `NSPanel`，里面装**第二个** `FlutterEngine` 的
/// `FlutterViewController`。
///
/// ## 这一票的全部理由：不抢焦点
///
/// 用户在全屏应用上方点菜单栏图标时，小窗必须**不切 Space、不切 App**。三件事凑齐才成立：
///
/// 1. `styleMask = .nonactivatingPanel` —— 官方原文「the window is a panel or a
///    subclass of that does not activate the owning app」
///    (developer.apple.com/documentation/appkit/nswindow/stylemask-swift.struct/nonactivatingpanel)
/// 2. `level = .statusBar` —— 浮在全屏应用的窗口之上（否则被全屏窗口盖住）
/// 3. `collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]` ——
///    跟着当前 Space 走而不是把用户拽回小窗所属的那个 Space
///
/// 以及一条禁令：**只用 `orderFrontRegardless()`**，绝不 `makeKeyAndOrderFront(_:)`、
/// 绝不 `NSApp.activate(ignoringOtherApps:)` —— 那两个正是「抢焦点」本身。
///
/// ## 第二个引擎的代价
///
/// macOS 没有 `FlutterEngineGroup`（iOS / Android 专有，
/// docs.flutter.dev/add-to-app/multiple-flutters），所以这是一个完整的第二引擎，
/// 没有共享 snapshot 可用。实测见 `.scratch/flutter-frontend/impl/I11-tray-popover.md`。
/// 引擎**懒建、建好不释放**（用户 2026-09-17 定：保留可配置小窗，不设内存门槛）。
final class TrayPanel: NSObject {
  static let shared = TrayPanel()

  /// 首帧的占位宽（逻辑像素）。**不是权威值** —— 真正的宽高由 Dart 量完内容报上来
  /// （`resize(width:height:)`），上下限也在 Dart 那边夹（`panel_channel.dart`）。
  /// 这里只需要一个「内容还没量出来之前先挂着」的数。
  private static let initialWidth: CGFloat = 300
  private static let initialHeight: CGFloat = 80
  /// 图标底边与小窗顶边的间距。
  private static let gap: CGFloat = 4

  private var panel: NSPanel?
  /// 最近一次 show 的锚点。改宽之后要重新居中到图标底下，所以得留着。
  private var anchor: NSRect = .zero
  private var engine: FlutterEngine?
  private var outsideClickMonitor: Any?
  /// 最近一次 show 的时刻：菜单栏那一下点击本身也是「面板外点击」，
  /// 不设静默期的话会开了立刻被自己关掉。
  private var shownAt: TimeInterval = 0

  var isVisible: Bool { panel?.isVisible ?? false }

  // MARK: - 显隐

  func show(anchor: NSRect) {
    let panel = ensurePanel()
    self.anchor = anchor
    position(panel, anchor: anchor)
    // 只 orderFrontRegardless：不激活本 app，不切 Space。
    panel.orderFrontRegardless()
    shownAt = Date.timeIntervalSinceReferenceDate
    installOutsideClickMonitor()
  }

  func hide() {
    panel?.orderOut(nil)
    removeOutsideClickMonitor()
  }

  func toggle(anchor: NSRect) {
    if isVisible { hide() } else { show(anchor: anchor) }
  }

  /// 内容自己量出来的宽高（Dart 已按 `panel_channel.dart` 的上下限夹好）。
  /// 顶边不动（向下长），横向重新居中到图标底下 —— 与 React 版保持 `centerX` 同语义
  /// （`src/popover.tsx:198`）。≤1px 的变化不动窗，防抖动循环（对齐 React 的 `DELTA`）。
  func resize(width: CGFloat, height: CGFloat) {
    guard let panel = panel else { return }
    var frame = panel.frame
    let dw = abs(frame.size.width - width)
    let dh = abs(frame.size.height - height)
    guard dw > 1 || dh > 1 else { return }
    frame.origin.y += frame.size.height - height
    frame.size.width = width
    frame.size.height = height
    panel.setFrame(frame, display: true)
    // 宽变了就得重新居中；顺带把贴边收回屏内那条也一并复用。
    if dw > 1 { position(panel, anchor: anchor) }
  }

  // MARK: - 构建

  private func ensurePanel() -> NSPanel {
    if let panel = panel { return panel }

    // `allowHeadlessExecution` 必须是 true（票 I20 修）：`run(withEntrypoint:)` 在这里
    // 跑在 `FlutterViewController` 附上来**之前**，false 会让引擎拒绝启动 ——
    // 日志里那句 "Attempted to run an engine with no view controller without headless
    // mode enabled."，后果是 popoverMain 根本没跑，小窗停在 0×0 什么都不显示。
    // 语义上也该是 true：小窗收起时引擎照样活着（懒建、建好不释放），那本来就是无视图状态。
    let engine = FlutterEngine(
      name: "aidog-tray-popover", project: nil, allowHeadlessExecution: true)
    engine.run(withEntrypoint: "popoverMain")
    self.engine = engine

    let vc = FlutterViewController(engine: engine, nibName: nil, bundle: nil)
    RegisterGeneratedPlugins(registry: vc)
    FlutterMethodChannel(
      name: "aidog/tray_panel_content", binaryMessenger: engine.binaryMessenger
    ).setMethodCallHandler { [weak self] call, result in
      switch call.method {
      case "resize":
        let args = call.arguments as? [String: Any]
        guard let w = args?["w"] as? Double, let h = args?["h"] as? Double else {
          result(FlutterError(code: "bad-args", message: "resize needs w and h", details: nil))
          return
        }
        self?.resize(width: CGFloat(w), height: CGFloat(h))
        result(nil)
      case "close":
        self?.hide()
        result(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
    }

    let panel = NSPanel(
      contentRect: NSRect(x: 0, y: 0, width: Self.initialWidth, height: Self.initialHeight),
      styleMask: [.nonactivatingPanel],
      backing: .buffered,
      defer: false)
    panel.contentViewController = vc
    // 赋 contentViewController 会把窗缩到该 VC 视图的尺寸，而 Flutter 视图在出第一帧
    // 之前是 0×0 —— 不重新撑开就是一个看不见的窗。Dart 量完内容会再报一次真尺寸。
    panel.setContentSize(NSSize(width: Self.initialWidth, height: Self.initialHeight))
    panel.isFloatingPanel = true
    panel.becomesKeyOnlyIfNeeded = true
    panel.hidesOnDeactivate = false
    panel.worksWhenModal = true
    panel.isReleasedWhenClosed = false
    // 全屏应用之上 + 跟着当前 Space 走 —— 缺一条就会切 Space 或被盖住。
    panel.level = .statusBar
    panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
    // 圆角与背景由 Flutter 自己画，窗本身透明。
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = true
    self.panel = panel
    return panel
  }

  /// 图标正下方居中。[anchor] 是**左上角原点**的逻辑坐标（与 `aidog_core::menubar`
  /// 的 `on_click` 同坐标系）；AppKit 屏幕坐标是左下原点，这里翻回去。
  private func position(_ panel: NSPanel, anchor: NSRect) {
    let screen = NSScreen.screens.first ?? NSScreen.main
    let screenH = screen?.frame.height ?? 0
    let anchorBottomFromTop = anchor.origin.y + anchor.size.height
    var x = anchor.origin.x + anchor.size.width / 2 - panel.frame.width / 2
    // 贴边时收回屏内，别让小窗有一半挂在屏幕外。
    if let visible = screen?.visibleFrame {
      x = min(max(x, visible.minX + Self.gap), visible.maxX - panel.frame.width - Self.gap)
    }
    let y = screenH - anchorBottomFromTop - Self.gap - panel.frame.height
    panel.setFrameOrigin(NSPoint(x: x, y: y))
  }

  // MARK: - 点外面收起

  /// 面板永远不成为 key window（非激活面板），所以没有 `resignKey` 可用，
  /// 只能全局监听点击。`shownAt` 的 300 ms 静默期挡住「点菜单栏图标那一下」。
  private func installOutsideClickMonitor() {
    guard outsideClickMonitor == nil else { return }
    outsideClickMonitor = NSEvent.addGlobalMonitorForEvents(
      matching: [.leftMouseDown, .rightMouseDown]
    ) { [weak self] _ in
      guard let self = self else { return }
      if Date.timeIntervalSinceReferenceDate - self.shownAt < 0.3 { return }
      self.hide()
    }
  }

  private func removeOutsideClickMonitor() {
    if let m = outsideClickMonitor { NSEvent.removeMonitor(m) }
    outsideClickMonitor = nil
  }

  // MARK: - 主引擎侧通道

  /// 主窗口引擎注册 `aidog/tray_panel`：菜单栏点击的那一跳最终落到这里。
  static func register(messenger: FlutterBinaryMessenger) {
    FlutterMethodChannel(name: "aidog/tray_panel", binaryMessenger: messenger)
      .setMethodCallHandler { call, result in
        let args = call.arguments as? [String: Any]
        let anchor = NSRect(
          x: args?["x"] as? Double ?? 0,
          y: args?["y"] as? Double ?? 0,
          width: args?["w"] as? Double ?? 0,
          height: args?["h"] as? Double ?? 0)
        switch call.method {
        case "show": shared.show(anchor: anchor); result(nil)
        case "hide": shared.hide(); result(nil)
        case "toggle": shared.toggle(anchor: anchor); result(nil)
        case "isVisible": result(shared.isVisible)
        default: result(FlutterMethodNotImplemented)
        }
      }
  }

  /// 验收 / 冒烟用的接缝：`AIDOG_TRAY_PANEL_ANCHOR="x,y,w,h"` 存在时启动即弹一次小窗。
  /// 菜单栏宿主（`aidog_core::menubar`）还没搬进 Flutter 壳（票 I13），在那之前
  /// 这是唯一能在真机上驱动「全屏应用上方弹窗」的路径。
  ///
  /// ponytail: 是测试接缝不是功能开关；I13 接上真实点击后可以删。
  static func autoShowFromEnvironment() {
    guard let raw = ProcessInfo.processInfo.environment["AIDOG_TRAY_PANEL_ANCHOR"] else {
      return
    }
    let parts = raw.split(separator: ",").compactMap { Double($0) }
    guard parts.count == 4 else { return }
    let anchor = NSRect(x: parts[0], y: parts[1], width: parts[2], height: parts[3])
    DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
      shared.show(anchor: anchor)
    }
  }
}
