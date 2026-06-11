# Research: 设置页 UI 扩展插入点

- **Query**: AppSettings.tsx vs Settings.tsx 哪个加 tray 配置区？现有结构 + 多选平台 + 拖拽排序 + 每项样式
- **Scope**: 内部前端
- **Date**: 2026-06-11

## 关键澄清：两个 "Settings" 文件用途不同

- `src/pages/Settings.tsx`（147KB）= **Claude Code 配置编辑器**（permissions / env / hooks / skills / plugins / sandbox），section tab 制（Settings.tsx:3182 `export function Settings`，3249 `activeTab`，SECTIONS 渲染 3252 `renderSectionContent`）。**与 app 自身系统设置无关**，不要往这里加 tray。
- `src/pages/AppSettings.tsx`（17KB）= **app 系统设置页**，nav `"settings"` 渲染它（`src/App.tsx:68` `effectiveNav === "settings" && <AppSettings/>`；nav 定义 App.tsx:16 `{id:"settings", labelKey:"nav.settings"}`）。
  - 现有 tab：`type Tab = "proxy" | "claude" | "pricing"`（AppSettings.tsx:7），`const [tab, setTab] = useState<Tab>("proxy")`（:11）。
  - "claude" tab 内嵌 `<Settings/>`（AppSettings.tsx:4 import）—— 即 Claude Code 配置编辑器是 AppSettings 的一个子 tab。
  - "pricing" tab 内嵌 `<PricingTab/>`。

→ **结论：tray 配置作为 AppSettings.tsx 新增 tab `"tray"`**（与 proxy/claude/pricing 平级），或并入 proxy tab。建议独立 tab `"tray"`（labelKey `nav.tray` / `settings.tray`）。

## 需新增的 UI 子能力

1. **多选平台**：`platformApi.list()`（api.ts:266）拿全部平台，多选加入 items（推翻原单选互斥）。
2. **拖拽排序**：复用 Groups.tsx 原生 HTML5 DnD 模式（见 04），无第三方库。
3. **每项配置行**：display(balance/coding) 切换、color 选择器（follow/预设/hex）、font_size、enabled 开关、删除。现 Platforms.tsx:1931-1958 已有 balance/coding 二选一 + 托盘开关的按钮组样式可参考迁移。
4. **今日消耗项**：特殊 item type，开关 + metric(cost/tokens) + 样式（见 05）。
5. **全局布局**：layout(single_line/two_line) + separator 输入。

## 现有可复用的 UI 元素 / 样式

- AppSettings 用 `btn / btn-primary / btn-ghost / card` 等类（Platforms.tsx:1936/1951 同款）。
- i18n：`useTranslation()` + `t(key, fallback)`（AppSettings.tsx:10，全项目 7 语言，需补 tray 相关 key，含 ar-SA RTL）。
- 状态保存模式：AppSettings 各设置项 onChange 即调 api（如 :55 handleAutostartChange），tray 可同样"改即存 + refresh 托盘"。

## Caveats

- AppSettings 现完全是受控组件 + 本地 useState 镜像 + onChange 落 api 模式；tray 配置较复杂（数组 + 拖拽），建议抽独立子组件 `TrayConfigTab`（类似 PricingTab/Settings 独立文件），避免 AppSettings.tsx 膨胀。
- 颜色选择器项目内未见现成组件，需自建（预设色板 + `<input type=color>`）。
