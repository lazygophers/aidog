# ST3: AppSettings tray 配置 tab

- **目标**: 系统设置页 tray 配置 UI
- **产出** (AppSettings.tsx + 新 TrayConfigTab 组件):
  - AppSettings tab 列表加 `"tray"`（现有 proxy/claude/pricing tab 制）
  - TrayConfigTab：
    - 平台多选（enabled 平台）加为 platform item；今日消耗(tokens)项开关
    - 拖拽排序（复用 Groups.tsx HTML5 DnD 模式，research/04）→ order
    - 每项：display(balance/coding 二选) + 颜色三态(跟随/预设下拉 red/green/orange/自定义 colorpicker+可读性警告) + 字号 + 开关/删除
    - 全局 layout(单行/两行) + separator
    - 保存 → trayConfigApi.set(config)（后端 refresh tray）
  - 禁 any，Liquid Glass
- **验证**: tsc 0 / yarn build
- **资源**: research/03-settings-page-ui.md + 04-drag-reorder-pattern.md、design.md、AppSettings.tsx tab 结构、trayConfigApi(ST1)
- **依赖**: ST1
- **失败处理**: 别窗口改 AppSettings 冲突 → 仅加 tray tab
