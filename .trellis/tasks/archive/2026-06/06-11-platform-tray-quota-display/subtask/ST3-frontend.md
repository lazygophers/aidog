# ST3: 前端 tray 开关 UI

- **目标**: enabled 平台 tray 开关 + 互斥 + 余额/coding 选择
- **产出** (Platforms.tsx):
  - 仅 `p.enabled` 平台卡片显示 tray 开关（toggle/star 图标）+ 余额/coding 二选一（select 或两按钮，对应 tray_display）
  - 开 → `trayApi.set(p.id, display)`（后端互斥清其他）；关 → `trayApi.clear()`
  - 互斥 UI：开一个后重载 platform list（或本地置其他 show_in_tray=false），保证仅一个高亮
  - 用 api.ts trayApi（ST1 已定义）+ Platform.show_in_tray/tray_display 字段
- **验证**: tsc 0 / yarn build；enabled 才显开关；互斥；display 切换
- **资源**: design.md、Platforms.tsx 卡片区、api.ts trayApi
- **依赖**: ST1
- **失败处理**: 禁 any；别窗口改 Platforms.tsx 冲突仅改 tray 开关区
