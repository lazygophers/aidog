# ST2: 前端 per-item line_mode UI

- **目标**: 每项单/两行配置，删全局 layout 切换
- **产出**:
  - api.ts: TrayItem +`line_mode: "single" | "two"`；TrayConfig 删 layout
  - TrayConfigTab.tsx: 每项编辑加 line_mode segmented（单行/两行）；删全局 layout 切换 UI(:161-179)；separator 保留
- **验证**: tsc 0 / yarn build
- **资源**: design.md、TrayConfigTab.tsx:66/161、api.ts:336/348
- **依赖**: ST1
- **失败处理**: 禁 any；别窗口冲突仅改 tray 相关
