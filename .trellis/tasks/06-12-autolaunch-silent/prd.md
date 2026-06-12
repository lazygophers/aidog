# PRD: 开机自动启动 + 静默启动

## 目标

在系统设置页添加两个 toggle 开关：

1. **开机自动启动** — OS 登录时自动启动 AiDog app
2. **静默启动** — 启动时隐藏主窗口，仅系统托盘运行

## 范围

### 后端 (Rust)

1. 添加 `tauri-plugin-autostart` 依赖 (Tauri 2.x 兼容版本)
2. `ProxySettings` struct 扩展: 新增 `silent_launch: bool` 字段 (backward compatible, 默认 false)
3. 新增 Tauri command `set_launch_settings(autolaunch: bool, silent_launch: bool)`
4. `setup()` 中:
   - 注册 autostart 插件
   - 若 `silent_launch == true`, 主窗口 `hide()` 而非 `show()`
5. DB: `ProxySettings` 已存 setting 表 JSON, 新字段自动序列化, 无 migration

### 前端 (React)

1. `api.ts`: `ProxySettings` interface 新增 `silent_launch: boolean`
2. `api.ts`: 更新 `setAutostart` → `setLaunchSettings` 或新增 `setSilentLaunch`
3. `AppSettings.tsx` 系统设置 tab:
   - 现有 autostart toggle 改为 "开机自动启动代理" (仅控制代理自启)
   - 新增 "开机自动启动" toggle (OS login autostart)
   - 新增 "静默启动" toggle (启动时隐藏窗口)
4. i18n: 所有 7 种语言添加对应 key

## 不改

- 现有 autostart (代理自动启动) 语义不变
- 托盘行为不变
- 不改 tauri.conf.json windows 配置

## 依赖

- `tauri-plugin-autostart` crate

## 验证

- 开机自启: toggle 开 → macOS Login Items 出现 AiDog / 重启后 app 自启
- 静默启动: toggle 开 → 启动 app 后主窗口不显示, 托盘正常
- cargo build 通过
- yarn tsc --noEmit 通过
