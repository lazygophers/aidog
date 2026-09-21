import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
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
