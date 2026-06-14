# silent-launch 依赖 autolaunch

## 背景
`silentLaunch` (静默启动) 仅在开机自启 (`autolaunch`) 生效时才有意义。当前两者独立, 用户可开启 silentLaunch 但 autolaunch off —— 此时静默启动永远不会触发, 配置无意义且误导。

## 目标
- `autolaunch` (开机自动启动) = off 时, silentLaunch UI **不展示**。
- `autolaunch` = off 时, silent_launch **强制 false 并持久化** (不仅 UI 隐藏, DB 也置 off)。

## 变更范围 (单文件)
`src/pages/AppSettings.tsx`:

1. **初始 load**: `getAutolaunch()` 后, 若 false → `setSilentLaunch(false)` + 调 `proxyApi.setSilentLaunch(false)` 持久化。  顺序: getSettings (含 silent_launch) → getAutolaunch → 若 al=false 则覆盖。
2. **handleAutolaunchChange(false)**: 关闭 autolaunch 时, 同时 `setSilentLaunch(false)` + `proxyApi.setSilentLaunch(false)` 持久化。
3. **UI**: silentLaunch toggle 块外层加 `{autolaunch && (...)}` 条件渲染。

## 非目标
- 不改后端 (silent_launch 字段保留, 后端仍按 silent_launch 标志决定是否 minimize)。
- 不改 autostart (proxy 自启) 行为。
- 不改 i18n 文案。

## 验收
- autolaunch off: UI 无 silentLaunch 块; DB silent_launch=false。
- autolaunch off → on: silentLaunch 块出现 (默认 off, 用户手动开)。
- autolaunch on, silentLaunch on → 用户关 autolaunch: silentLaunch 同步 off 持久化。
- tsc 通过 (yarn build)。
