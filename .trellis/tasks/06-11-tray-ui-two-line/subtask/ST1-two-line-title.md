# ST1: 两行 title + 隐藏/恢复 logo

- **目标**: 托盘有值两行(名+余额)+隐 icon，无值恢复
- **产出** (lib.rs):
  - tray_quota_text(:1228) 返回 `{platform.name}\n{second}`；second: coding→`🪙 剩 {100-util:.0}%`（est_coding_plan 首 tier est_utilization）/ balance→`💳 {est_balance_remaining:.2}`
  - refresh_tray_menu(:1279) macOS cfg：Some(text)→`tray.set_icon(None)` + `set_title(Some(text))`；None→`set_icon(Some(default_window_icon))` + `set_title(None)`
  - menu item tray_quota 保留（下拉详情）
- **验证**: cargo build 0
- **资源**: design.md、lib.rs:1228/1279/1330、EstCodingPlan::from_json
- **依赖**: 无
- **失败处理**: set_icon API 签名查 Tauri 2.0 tray（TrayIcon::set_icon Option<Image>）
