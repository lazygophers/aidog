# PRD: 托盘两行展示 + 有值隐藏 logo

## 需求
系统托盘（macOS 菜单栏）有 quota 值时：
- **隐藏 logo（icon）**
- **两行**：第一行=平台名；第二行=余额（coding plan 平台显剩余%、balance 平台显总余额）
- 无值时：恢复 icon、清 title

## 现状
- lib.rs:1228 tray_quota_text 单行（balance `💳{remaining}` / coding `🪙{util}%`）
- :1287 set_title 单行；:1330 icon 常驻
- est 来源：platform.est_balance_remaining / est_coding_plan(tiers est_utilization)

## 决策
| 项 | 结论 |
| --- | --- |
| 两行 | 尽力 `set_title("名\n余额")` 两行；macOS 不渲染两行 → 降级单行 "名 余额"（agent 验证 \n） |
| 第一行 | platform.name |
| 第二行 | coding plan → `🪙 剩 {100-util:.0}%`；balance → `💳 {remaining:.2}` |
| 隐藏 logo | 有值 `tray.set_icon(None)`；无值恢复 `default_window_icon` |

## 涉及面
- lib.rs：tray_quota_text 改两行（含 name）；refresh_tray_menu 有值 set_icon(None)+两行 title / 无值恢复 icon + set_title(None)

## 验收
- 有值：菜单栏隐 icon + 两行(名/余额)（或降级单行）；无值：icon + 无 title
- coding 剩余% / balance 总余额
- cargo build；macOS 实际渲染由用户验

## Subtask
- ST1: tray_quota_text 两行(名+余额按类型) + set_title + 隐藏/恢复 icon
- ST2: 验证 \n 两行可行性 + 降级单行
