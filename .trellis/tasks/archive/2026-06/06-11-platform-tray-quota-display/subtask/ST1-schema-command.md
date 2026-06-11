# ST1: platform tray 列 + 互斥 command

- **目标**: platform 加 show_in_tray/tray_display + 互斥 set command + 前端类型/API
- **产出**:
  - 001_init.sql platform 加 `show_in_tray INTEGER NOT NULL DEFAULT 0` + `tray_display TEXT NOT NULL DEFAULT 'balance'`；migration 005 ALTER 幂等（承接 004）
  - db.rs PLATFORM_COLUMNS + PREFIXED + row_to_platform + **get_group_platforms 第二 parser**(偏移+2) + create/update；`set_tray_platform(db, platform_id, tray_display)` 单事务互斥（UPDATE 清所有 show_in_tray=0 → 置选中=1）+ `clear_tray` + `get_tray_platform`
  - lib.rs command `platform_set_tray(platform_id, tray_display, enabled)` + invoke_handler 注册（改后调 refresh_tray）
  - models.rs Platform + api.ts Platform 加 show_in_tray:boolean/tray_display:string + `trayApi { set(platformId, display), clear() }`
- **验证**: cargo build + tsc 0
- **资源**: design.md、db.rs 加列先例(quota-est 004)、现有 platform command
- **依赖**: 无
- **失败处理**: 两处 parser 都同步；互斥用单事务
