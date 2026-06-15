# PRD: 通知关闭时隐藏通知中心菜单

## 目标
通知总开关 (`NotificationSettings.enabled`) 关闭时，隐藏侧栏「通知中心」nav 入口 (`notifications`)。

## 背景
- 总开关 `settings.enabled`，dispatch 早退 `src-tauri/src/gateway/notification.rs:176`
- 先例：日志关闭隐藏 logs `src/App.tsx:86-90` (`logEnabled` + `BASE_NAV.filter`)
- 侧栏静态 `BASE_NAV` `src/App.tsx:19`，`notifications` 项 L24
- API: `notificationSettingsApi.getSettings()` → `NotificationSettings` (`src/services/api.ts:944`)

## 范围
- ✅ 隐藏侧栏 `notifications` nav（通知中心 = 收件箱页）
- ✅ effectiveNav 回退：`activeNav==="notifications" && !notifEnabled` → `platforms`
- ⛔ 保留设置 tab `settings/notifications`（重开入口）
- ⛔ 不改后端 / dispatch / NotificationSettings 结构

## 产出
### `src/App.tsx`
1. state `notifEnabled`，启动读 `notificationSettingsApi.getSettings().enabled`（仿 logEnabled L78-84）
2. `navItems` 过滤：`!notifEnabled` 去 `notifications`（与 logs 过滤合并）
3. `effectiveNav` 回退：notif 页 + 关 → `platforms`
4. AppSettings 回调 `onNotifSettingsChanged(enabled)` 更新 state（仿 `onLogSettingsChanged` L113）

### `src/pages/AppSettings.tsx`
- 通知设置 enabled 开关 onChange → 调 `onNotifSettingsChanged` 回调（仿现有 log settings 回调）

## 验证
- enabled=true → 侧栏见通知中心
- enabled=false → 侧栏无入口；当前在通知页 → 回退 platforms
- 关通知 → 设置 tab 通知配置仍在 → 重开 → 入口重现
- `yarn build` 通过
- 无新增 i18n key

## 依赖
无后端改动
