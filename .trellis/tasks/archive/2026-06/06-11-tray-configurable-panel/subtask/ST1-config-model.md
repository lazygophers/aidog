# ST1: TrayConfig 模型 + settings + 迁移

- **目标**: tray 配置数据模型 + 读写 + 旧配置迁移
- **产出**:
  - models.rs: `TrayConfig{layout:String, separator:String, items:Vec<TrayItem>}`；`TrayItem{type, platform_id:Option<u64>, display:String, metric:Option<String>, color:TrayColor{mode,value}, font_size:f64, enabled:bool, order:i32}`（serde）
  - db.rs/lib.rs: read TrayConfig from settings(scope=tray,key=config)；write via settings set
  - 迁移：启动/读取时若 config 缺失 → 从 show_in_tray=1 平台生成默认 TrayConfig（一个 platform item，display=旧 tray_display），存 settings
  - api.ts: `trayConfigApi { get(), set(config) }`；TrayConfig/TrayItem TS 类型
  - command platform_tray_config_get/set + 注册（set 后 refresh tray）
- **验证**: cargo build + tsc 0
- **资源**: research/02-config-data-model.md + 06-migration.md、design.md、settingsApi
- **依赖**: 无
- **失败处理**: settings KV 复用 proxy_log_settings 先例
