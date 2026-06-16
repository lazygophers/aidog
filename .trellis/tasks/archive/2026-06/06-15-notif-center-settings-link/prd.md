# PRD: 通知中心页加快捷入口跳设置

## 目标
通知中心页（Notifications）顶部加快捷入口，点击直接跳到「设置 → 通知」tab。

## 背景
- 导航机制：App.tsx `handleNavigate` → `requestNavigation` (navGuard) → `setActiveNav`
- 设置子页 activeNav 形如 `settings/notifications`，effectiveNav=settings + settingsTab=notifications
- Notifications.tsx L16 `export function Notifications()` 当前无 props
- 通知开启时通知中心可访问（侧栏入口，hide-notify 任务逻辑）；快捷入口便于从通知中心快速去配置

## 范围
- ✅ App.tsx 给 `<Notifications>` 传 `onNavigate={handleNavigate}` prop
- ✅ Notifications.tsx props 加 `onNavigate?: (id: string) => void`
- ✅ 顶部加按钮「通知设置」→ `onNavigate?.("settings/notifications")`（无 prop 时隐藏按钮，向后兼容）
- ✅ i18n key `notifications.goSettings` (8 语言)
- ⛔ 不改导航核心逻辑 / navGuard

## 产出
### `src/App.tsx`
`{effectiveNav === "notifications" && <Notifications />}` → `<Notifications onNavigate={handleNavigate} />`

### `src/pages/Notifications.tsx`
1. props 加 `onNavigate?: (id: string) => void`
2. 页面标题栏右侧加按钮 `<button className="btn btn-ghost" onClick={() => onNavigate?.("settings/notifications")}>` 图标 + 文案 `notifications.goSettings`
3. 无 onNavigate 时不渲染按钮

### i18n（8 语言）
`notifications.goSettings` = "通知设置" / "Notification Settings" / ...

## 验证
- 通知中心页顶部见「通知设置」按钮
- 点击 → 跳设置页通知 tab
- yarn build 通过
- check-i18n 零缺失

## 依赖
无后端改动
