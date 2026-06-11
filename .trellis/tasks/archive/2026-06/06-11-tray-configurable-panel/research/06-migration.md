# Research: 迁移（删平台卡片 tray 开关 + 旧列去留）

- **Query**: 删 platform 卡片 tray 开关 + show_in_tray/tray_display 列去留
- **Scope**: 内部全栈
- **Date**: 2026-06-11

## 待删除：平台卡片 tray 开关 UI

`src/pages/Platforms.tsx`：
- :1152 `handleTrayToggle`（互斥单平台开关）
- :1168 `handleTrayDisplay`（balance/coding 切换）
- :1931-1958 UI：balance/coding 二选一按钮组 + 托盘开关按钮（"📌/📍 托盘"）
- :3 import `trayApi`
- 关联 i18n key：`platform.trayBalance/trayCoding/trayOn/trayOff/tray`

## 后端：单平台互斥 API / DB

- 命令 `platform_set_tray`（`src-tauri/src/lib.rs:194-209`）→ 注册于 invoke_handler（lib.rs:1512）。
- db 函数 `set_tray_platform / clear_tray / get_tray_platform`（`gateway/db.rs:258-294`）。
- 前端 `trayApi.set/clear`（`src/services/api.ts:293-308`）。

## DB 列去留：show_in_tray / tray_display

- 列定义：Migration 005（gateway/db.rs:59-61），PLATFORM_COLUMNS（:85/89），row_to_platform（:115-116），struct（models.rs:298/301），TS 类型（api.ts:150/152）。
- **建议保留列、停用语义**（最小风险）：
  - tray 配置迁到 settings("tray","config")，不再依赖这两列。
  - 列保留不删（SQLite ALTER DROP COLUMN 旧版不支持/有风险；保留不影响）。
  - `get_tray_platform`（WHERE show_in_tray=1）可改造为"读首个 enabled platform item from tray config"，或直接弃用。
- **平滑迁移**：首次加载 tray config 为空时，若存在 show_in_tray=1 的旧平台，自动生成单 item 默认 config（一次性），避免老用户升级后托盘空白。

## 迁移步骤（建议顺序，design 细化）

1. 后端加 TrayConfig 读取 + 多 item 渲染（05），保留旧 platform_set_tray 命令暂不删（兼容）。
2. 前端 AppSettings 加 tray tab（03）+ 拖拽（04），写 settings("tray","config")。
3. 旧 config 为空时从 show_in_tray 生成默认（平滑迁移）。
4. 删 Platforms.tsx tray UI（本文件上半）。
5. 废弃 platform_set_tray / trayApi（保留命令但前端不再调，或彻底删 + 清 invoke_handler 注册）。
6. show_in_tray/tray_display 列保留（不删，降风险）。

## Caveats

- 删命令需同步 `invoke_handler!` 列表（lib.rs:1512 `platform_set_tray`），否则编译/运行不一致。
- 是否需要老用户配置自动迁移，取决于用户量——需 design/用户确认（不迁移则升级后托盘需手动重配）。
