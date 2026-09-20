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

    super.awakeFromNib()
  }
}
