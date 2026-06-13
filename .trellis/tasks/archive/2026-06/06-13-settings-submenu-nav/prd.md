# 设置 tab 重构为侧栏子菜单

## 背景

当前 AppSettings (`src/pages/AppSettings.tsx`) 内部用顶部 tab bar 切换 9 个 sub-tab：`system / claude / codex / middleware / scheduling / notifications / pricing / tray / popover`。tab bar 横向排列，项多拥挤；与侧栏主导航风格不统一。

侧栏 (`src/App.tsx`) 6 项：platforms / groups / stats / logs / notifications / settings。其中 notifications 为收件箱查看入口，与 AppSettings.notifications（通知配置）语义不同，**保留并存**。

## 目标

把 AppSettings 内 9 个 sub-tab **提升为侧栏"设置"下的可折叠分组子菜单**，移除 AppSettings 顶部 tab bar。用户点侧栏"设置"展开分组子菜单，点子项直达对应设置页。

## 设计

### 交互
- 侧栏"设置"项：点击 **toggle 展开/收起** 子菜单区。
- 展开后按**二级组**分节渲染（组标题小字灰 + 组内项缩进）：
  - **常规**：system
  - **集成**：claude、codex
  - **规则**：middleware、scheduling
  - **通知**：notifications（仅配置；收件箱仍在侧栏独立 notifications 项）
  - **配置**：pricing、tray、popover
- active 子项高亮（accent 色 + 左边框）。
- 收起时仅显示"设置"图标行；若当前 activePage=settings，自动展开并高亮当前 sub。
- 子项切换走 `requestNavigation`（保留 dirty 拦截，与现有 `switchTab` 等价）。

### 状态模型
- `activePage` 仍为顶层 (`"settings"`)。
- 新增 App 级 state：`settingsTab: Tab`（默认 `"system"`）+ `settingsExpanded: boolean`。
- AppSettings 改为受控：`<AppSettings tab={settingsTab} onLogSettingsChanged={...} />`，**移除内部 tab bar UI 与 `useState<Tab>`**；`switchTab` 逻辑上移到 App（子菜单点击处）。
- 进入 settings 页（从其他页点"设置"主项）：若 settingsExpanded=false 则 toggle 展开；tab 维持上次。

### 组件
- 抽出 `src/components/Sidebar.tsx`（可选；若 App.tsx 已够清晰可就地改）。优先就地改 App.tsx 减少扩散。
- AppSettings.tsx：删 tab bar 渲染块（~line 181-215 tab button map），保留内容渲染 switch。`tab` 改 props。

### i18n
新增侧栏组标题 key（7 语言 zh/en/ja/fr/de/ar/es）：
- `nav.settingsGroup.general` / `.integration` / `.rules` / `.notification` / `.config`
子项 label 复用现有 `appSettings.*Tab`（codexTab/pricingTab/trayTab 等）+ 补 system/claude/middleware/scheduling/notifications/popover 的 label key（若缺）。

### navGuard
子菜单项 onClick → `requestNavigation(() => { setSettingsTab(next); })`。与原 `switchTab` 行为一致，dirty 页拦截不回归。

## 验收
- 侧栏"设置"可折叠展开，展开后见 5 组分节 + 9 子项。
- 点子项切到对应设置内容，无顶部 tab bar。
- active 子项高亮；切其他顶层页再回 settings，展开态 + tab 保持。
- dirty 页（如 Claude Code Settings 未保存）切子项弹 UnsavedChangesModal。
- 7 语言组标题正确；RTL（ar）布局不破。
- `yarn build` 通过（tsc + vite）。

## 范围
- 改：`src/App.tsx`、`src/pages/AppSettings.tsx`、i18n 7 语言 JSON。
- 不改：各设置子组件内部（editors/CodexSettings/PricingTab/...）、后端。
