# ST2: 后端 tray 展示 + 刷新

- **目标**: 系统托盘展示选定平台 quota
- **产出** (lib.rs):
  - build_tray_menu 扩展：`get_tray_platform` 查 show_in_tray=1 平台 → 按 tray_display 取 est（balance→est_balance_remaining `💳{cur}{remaining}` / coding→est_coding_plan 解析首 tier est_utilization `🪙{util}%`）→ 加 menu item 展示
  - `tray.set_title(Some(text))`（macOS 菜单栏文字；非 macOS no-op 或降级 tooltip）；refresh_tray_menu 扩展 set_menu + set_title；无选定/无 est → 清 title
  - 刷新触发：现有 :305/:330 保留 + platform_set_tray 后 + quota 预估更新后（estimate.rs/proxy spawn 末尾经 AppHandle emit 事件或直接 tray_by_id set，确认线程安全；后台 task 用 emit 让主线程 refresh 更稳）
- **验证**: cargo build；tray 显示选定平台 quota
- **资源**: design.md、lib.rs:1196 Tray 区、estimate.rs spawn
- **依赖**: ST1
- **失败处理**: 后台 task 直接操作 tray 线程问题 → 改 emit 事件主线程 refresh；set_title 非 macOS 降级
