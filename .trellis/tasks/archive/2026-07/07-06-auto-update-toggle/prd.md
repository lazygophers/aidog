# 自动升级设置开关 (默认开, 设置>系统可关)

## Goal

给现有自动升级机制 (tauri-plugin-updater + services/updater.ts) 加设置开关。默认开启, 用户可在 **设置 > 系统** 关闭。关闭后启动时不自动检查更新。

## 现状 (auto-context)

- `tauri.conf.json:41` updater 已配置
- `src/services/updater.ts`:
  - `checkForUpdateDailyThrottled()` — 启动调用, localStorage 24h 节流, dev/失败静默
  - `checkForUpdateManual()` — 手动按钮, 忽略节流
  - `runUpdate(update)` — 下载安装重启
- `src/App.tsx:19` 启动调 `checkForUpdateDailyThrottled`
- `src/components/UpdatePromptModal.tsx` 升级提示弹窗
- `src/pages/AppSettings.tsx:18` Tab 类型含 `"system"` → `SystemTab` 组件
- db 无 `auto_update_enabled` 字段 (需新增)

## 改动

1. **db** (`src-tauri/src/gateway/db.rs`):
   - app_settings 表加字段 `auto_update_enabled BOOLEAN DEFAULT TRUE` (或 settings JSON 加 key, 看现有 schema 习惯)
   - `get_auto_update_enabled() -> bool` + `set_auto_update_enabled(v: bool)` 持久化
2. **Tauri command** (`src-tauri/src/lib.rs`):
   - `get_auto_update_enabled` / `set_auto_update_enabled` 注册 invoke_handler
3. **api.ts** (`src/services/api.ts`):
   - `getAutoUpdateEnabled(): Promise<boolean>` / `setAutoUpdateEnabled(v: boolean): Promise<void>` invoke 封装
4. **启动 gate** (`src/App.tsx` 启动 useEffect):
   - 调 `checkForUpdateDailyThrottled` 前 `await getAutoUpdateEnabled()`, false → 跳过自动检查
   - **手动按钮 (`checkForUpdateManual`) 不 gate** — 用户主动触发, 关了自动仍能手动查
5. **SystemTab UI** (`src/components/settings/` 系统 tab 子组件):
   - 加 toggle「自动检查更新」(默认 on), onChange → `setAutoUpdateEnabled`
   - 放在系统 tab 现有 section 中 (紧跟相关项, 如版本/更新区)
6. **i18n** (`src/locales/*.json` 8 语言): 加 `settings.autoUpdateEnabled` label + description key

## 决策 (推荐, 待 grill 确认)

- **gate 范围**: 仅 gate 启动自动 daily check (`checkForUpdateDailyThrottled`)。手动按钮 (`checkForUpdateManual`) + UpdatePromptModal 弹出**不 gate** — 用户关了自动仍能主动手动查/更新。语义: "关 = 不打扰", 非 "禁用升级"。
- **默认值**: true (默认开)
- **存储**: app_settings 表 (与现有 settings 一致, 看现有 schema 是表列 vs JSON key 决定)

## Acceptance

- [ ] app_settings 加 auto_update_enabled (默认 true) + get/set command
- [ ] App.tsx 启动 gate: setting=false 跳过 checkForUpdateDailyThrottled
- [ ] SystemTab toggle UI (默认 on, 切换持久化)
- [ ] 手动检查更新按钮不 gate (关自动仍可手动)
- [ ] 8 语言 i18n key 齐全
- [ ] cargo test + yarn build + check:i18n 全绿

## Out of Scope

- 升级机制本身 (tauri-plugin-updater 已工作)
- UpdatePromptModal 行为变更 (仅 gate 自动入口, 弹窗逻辑不动)
- downgrade / channel 选择

## Technical Notes

- 现有 app_settings schema: db.rs (看是表列加 column vs settings JSON 加 key)
- SystemTab 子组件位置: src/components/settings/ (找到 system tab 渲染处)
- check:i18n: `node scripts/check-i18n.mjs` (8 locale 对齐)
