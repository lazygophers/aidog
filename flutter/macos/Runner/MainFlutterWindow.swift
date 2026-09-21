import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow, NSWindowDelegate {
  /// 关窗只隐藏，不销毁。销毁会连 Flutter 引擎一起拆掉（页面状态、已建的连接全没），
  /// 下次从菜单栏「Show Window」回来要重新冷启动一遍。隐藏则是秒回。
  /// 菜单栏条目由 `MenuBar` 独立持有，不随窗口走。
  func windowShouldClose(_ sender: NSWindow) -> Bool {
    orderOut(nil)
    return false
  }

  override func awakeFromNib() {
    self.delegate = self
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)
    // 票 I11：菜单栏小窗（第二个引擎装在 NSPanel 里），懒建，首次 show 才起引擎。
    TrayPanel.register(messenger: flutterViewController.engine.binaryMessenger)
    TrayPanel.autoShowFromEnvironment()
    // 票 I20：菜单栏条目本身（NSStatusItem + 多列富文本标题 + 右键菜单）。
    // 必须在 TrayPanel.register 之后 —— 点图标的回调会去开小窗。
    MenuBar.register(messenger: flutterViewController.engine.binaryMessenger)

    super.awakeFromNib()
  }
}
