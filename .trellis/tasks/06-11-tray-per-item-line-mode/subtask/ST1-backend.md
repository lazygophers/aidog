# ST1: 后端 per-item line_mode

- **目标**: TrayItem line_mode + 渲染 + 删全局 layout + 迁移
- **产出**:
  - models.rs: TrayItem +`line_mode:String`(default "single")；TrayConfig 删 layout/default_layout（留 separator）
  - lib.rs: 删 tray_layout 全局；渲染按 per-item line_mode（single→"名 值"段 / two→"名\n值"）；多 item separator 拼接；**≤2 行约束**：items.len()==1 尊重 line_mode；len>1 强制 single 横排（菜单栏无法多 item 多行，注释）
  - 迁移：旧 config.layout(two_line→item two/single_line→single) 映射各 item line_mode；无则 default single
  - 单测：line_mode serde + 单 item two 渲染两行 + 多 item single 横排
- **验证**: cargo build + test 0
- **资源**: design.md、models.rs:544(layout)、lib.rs:1258(渲染)/1322(tray_layout)
- **依赖**: 无
- **失败处理**: ≤2 行约束按 MVP 规则(len==1 尊重/len>1 single)
