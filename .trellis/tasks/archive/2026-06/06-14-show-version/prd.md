# PRD: 页面展示版本信息

## 背景
应用版本号 (0.1.0) 定义在 `tauri.conf.json` / `Cargo.toml` / `package.json`,前端无任何展示入口,用户无法直观获知当前版本。

## 目标
在 Settings 页 "system" tab 底部展示当前应用版本号。

## 方案
- 前端用 `@tauri-apps/api/app` 的 `getVersion()`(读 tauri.conf.json version,单一事实源)
- AppSettings.tsx "system" tab 末尾追加版本卡片(只读展示,样式贴合现有 glass-surface 卡片)
- i18n key `app.version` (8 语言全补)

## 范围 (单交付, main worktree 内直接写)
- `src/pages/AppSettings.tsx`: 加 getVersion state + useEffect + 渲染卡片
- `src/services/api.ts`: 无需改(用官方 plugin,非 invoke)
- 8 个 locale json: 加 `app.version`

## 验证
- `yarn build` (tsc + vite) exit 0
- 版本号正确显示为 "0.1.0"
- 8 语言 key 覆盖 (check:i18n 通过)

## 不做
- 不加后端 command (Tauri 官方 API 已足够)
- 不展示 git commit / build date (YAGNI)
