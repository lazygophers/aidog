# 前端页面

## 10 个页面组件

| 页面 | 文件 | 大小 | 职责 |
|------|------|------|------|
| Platforms | `Platforms.tsx` | 110.9K | 平台 CRUD、协议选择、模型映射、余额显示 |
| Groups | `Groups.tsx` | 39.0K | 分组 CRUD、平台关联、路由策略、拖拽排序 |
| TrayConfig | `TrayConfigTab.tsx` | 38.5K | 托盘状态栏配置、显示项选择 |
| Settings | `Settings.tsx` | 25.1K | 设置页编排容器，加载子组件 |
| Stats | `Stats.tsx` | 28.7K | 使用统计、趋势图、分组聚合 |
| Logs | `Logs.tsx` | 30.8K | 日志列表、筛选、搜索、详情弹窗 |
| AppSettings | `AppSettings.tsx` | 17.8K | 应用级设置（语言、主题、代理端口） |
| PricingTab | `PricingTab.tsx` | 17.7K | 模型定价管理、自定义价格、同步 |
| ModelTest | `ModelTestPanel.tsx` | 7.4K | 模型测试面板 |
| Proxy | `Proxy.tsx` | 3.7K | 代理状态显示 |

## 设置页拆分

Settings.tsx 是编排容器，子组件在 `components/settings/`：

| 子组件 | 职责 |
|--------|------|
| `editors.tsx` | 全部字段编辑器 + 特殊控件 |
| `SettingsHeader.tsx` | 页面头部 |
| `SectionAnchorNav.tsx` | 分段锚点导航 |
| `UnsavedChangesModal.tsx` | 未保存变更提示弹窗 |

## 共享组件

`components/shared/` — 三页共享展示组件：

| 组件 | 用途 |
|------|------|
| `CompactCard.tsx` | 紧凑卡片布局 |
| `StatChip.tsx` | 统计数据芯片 |
| `BalanceBar.tsx` | 余额进度条 |
| `colorScale.ts` | 颜色比例尺工具 |

## 导航架构

无 react-router：
- `App.tsx`：侧栏导航（本地 state 切换页面）
- `AppSettings.tsx`：设置页内部 tab 切换
- `utils/navGuard.ts`：离页拦截注册表（替代原生 confirm / beforeunload）

## 主题系统

`src/themes/` — 每主题 light + dark 两组 CSS 变量：
- `liquidGlass.ts` — 默认主题
- `nord.ts`
- `dracula.ts`
- `catppuccin.ts`
- `solarized.ts`
- `index.ts` — 注册入口

## i18n

`src/locales/` — 7 语言 JSON：
- zh-CN.json (41.7K)
- en-US.json (35.0K)
- ja-JP.json (38.0K)
- ar-SA.json (40.7K) — RTL
- fr-FR.json (35.8K)
- de-DE.json (34.8K)
- ru-RU.json (45.4K)
