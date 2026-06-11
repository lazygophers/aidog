# PRD: 平台 quota 任务栏(系统托盘)展示

## 需求（已确认）
- platform 加开关字段，**仅启用平台**显示该开关；作用：在**系统托盘 tray** 展示该平台的**余额或 coding% 二选一**
- **单平台互斥**（只一个平台可开 tray 展示）

## 现状（tray 基础已有）
- lib.rs:1196+ build_tray_menu（status/toggle/show/quit item）+ refresh_tray_menu(set_menu) + TrayIconBuilder("main") + tooltip；refresh 已在 :305/:330 调用
- Cargo tauri features=["tray-icon"]；无 set_title（菜单栏仅 icon）
- 刚完成 quota 预估：platform.est_balance_remaining / est_coding_plan（tray 复用这些预估值，无需额外查询）

## 决策
| 项 | 结论 |
| --- | --- |
| 展示载体 | 系统托盘 `tray.set_title`（macOS 菜单栏文字显示 quota%/余额）+ menu item 详情 |
| platform 加列 | `show_in_tray INTEGER NOT NULL DEFAULT 0`（互斥单平台）+ `tray_display TEXT NOT NULL DEFAULT 'balance'`（balance/coding 二选一），migration 005 |
| 数据源 | 复用 est_balance_remaining / est_coding_plan（预估值，请求驱动更新，无需 tray 单独查） |
| 互斥 | set show_in_tray 时 SQL 先 `UPDATE platform SET show_in_tray=0` 清所有，再置选中=1 |
| 更新触发 | refresh_tray 在 quota 预估更新后 + 平台 toggle/设置后 + 现有 :305/:330 调用点 |
| 开关可见 | 仅 platform.enabled 平台显示 tray 开关 |

## 涉及面
- 后端: migrations/001_init.sql + 005 + db.rs(加列链路两处 parser + set_tray_platform 互斥 command) + lib.rs(build_tray_menu 加 quota item + tray.set_title + refresh 触发) + models.rs + lib.rs command 注册
- 前端: api.ts(Platform 加字段 + trayApi) + Platforms.tsx(enabled 平台 tray 开关 + display 选择 + 互斥)

## 验收
- enabled 平台卡片有 tray 开关 + 余额/coding 选择；开一个自动关其他（互斥）
- 系统托盘 set_title/menu 展示选定平台 quota（余额或 coding%，用预估值）
- quota 预估更新后 tray 自动刷新
- cargo build + test + tsc

## 注意（多窗口 + migration 链）
别窗口并行改 db.rs/platform。migration 005 承接 004。主工作区改，commit 仅本 task 列。
