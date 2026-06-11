# Design: 平台 quota 系统托盘展示

## Schema（migration 005）
platform 加 2 列：
```sql
show_in_tray  INTEGER NOT NULL DEFAULT 0,   -- 互斥，仅一平台=1
tray_display  TEXT NOT NULL DEFAULT 'balance' -- 'balance' | 'coding'
```
- 001_init.sql 加列 + migration 005 ALTER 幂等（`let _ = execute`，承接 004 模式）
- db.rs PLATFORM_COLUMNS + PLATFORM_COLUMNS_PREFIXED + row_to_platform(新 index) + **get_group_platforms 第二 parser**(偏移+2) + create(默认 0/'balance')/update_platform(..existing 不破坏，除非显式设)
- models.rs Platform + api.ts Platform 加 show_in_tray:bool / tray_display:string

## 互斥 set command
- db.rs `set_tray_platform(db, platform_id, tray_display)`：单事务 `UPDATE platform SET show_in_tray=0`（清所有）→ `UPDATE platform SET show_in_tray=1, tray_display=? WHERE id=?`
- 关闭：`set_tray_platform(db, 0/None, ...)` 或 `clear_tray`（全清）
- lib.rs command `platform_set_tray(platform_id, tray_display, enabled)` + 注册；改后调 refresh_tray + emit 前端

## 后端 tray 展示（lib.rs build_tray_menu 扩展）
- build_tray_menu 查 `show_in_tray=1` 平台（db `get_tray_platform`）：
  - 取 est：tray_display='balance' → est_balance_remaining；'coding' → est_coding_plan 解析首 tier est_utilization
  - 格式化：balance `💳 {currency}{remaining}` / coding `🪙 {util}%`
  - 加 menu item（status item 之上/下）展示该文字
- `tray.set_title(Some(text))`（macOS 菜单栏文字；Tauri TrayIcon::set_title，非 macOS 平台 no-op/降级 tooltip）
- refresh_tray_menu 扩展：set_menu + set_title
- 无选定平台 / 无 est：title 清空（仅 icon）

## 更新触发
- 现有 refresh_tray_menu 调用点(:305/:330) 保留
- quota 预估更新后（estimate.rs / proxy spawn 预估末尾）触发 refresh_tray（经 AppHandle；注意 estimate 在后台 task，需 app handle 克隆或 emit 事件让主线程 refresh）
- platform_set_tray command 后 refresh_tray
- 可选定时（已有机制）

## 前端（Platforms.tsx）
- enabled 平台卡片：tray 开关（toggle / star 图标）+ 余额/coding 二选一（select 或两按钮），仅 `p.enabled` 显示
- 开关 on → `trayApi.set(p.id, display)`（互斥，后端清其他）；off → `trayApi.clear()`
- 互斥 UI：开一个后其他平台开关自动 off（重载 platform list 或本地置 show_in_tray）
- api.ts trayApi { set(platformId, display), clear() }

## 不改 / 注意
- 复用 est_*（quota 预估 task 产出），tray 不单独查 quota
- 别窗口并行改 db.rs → 主工作区，commit 仅本 task 列
- 后台 estimate task refresh tray：用 emit 事件或 app.tray_by_id 直接 set（确认线程安全）

## 验证
- cargo build+test+tsc；tray 显示选定平台 quota；互斥单平台；enabled 才显开关；预估更新 tray 刷新
