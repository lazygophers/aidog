# 每日检测更新并提醒用户

## 需求
应用每天检测是否有新版本；有则提醒用户「是否需要更新」，用户确认后下载安装并重启。

## 现状（已核）
- tauri-plugin-updater 已注册（lib.rs:3399）+ capabilities `updater:default`/`process:default`/`process:allow-restart`（default.json:28-30）+ tauri.conf endpoints（GitHub latest.json）+ pubkey 齐备。
- 前端依赖已装：`@tauri-apps/plugin-updater`^2 / `@tauri-apps/plugin-process`^2 / `@tauri-apps/plugin-dialog`^2（package.json）。
- **但前端无任何检查更新流程**（无 check()/prompt/手动按钮）。本任务纯前端补这条链路，**零后端改动**。
- App 入口：`src/App.tsx`（有 useEffect，:53/60/68），About 页 `src/pages/About.tsx`（已展示版本信息，适合放手动「检查更新」）。

## 设计（纯前端）
1. **新建** `src/services/updater.ts`：
   - `checkForUpdateDailyThrottled()`：读 localStorage `aidog_last_update_check`（ms 时间戳）；若不存在或距今 >24h → 调 `check()`（@tauri-apps/plugin-updater）。无论结果**写回当前时间戳**（避免反复检查）。dev/无签名环境 check 失败 → catch 静默（仅 console.debug），不打扰。
   - `runUpdate(update)`：`update.downloadAndInstall()`（可选进度回调）→ 成功后 `relaunch()`（@tauri-apps/plugin-process）。失败 toast/错误提示。
   - 暴露 `checkForUpdateManual()`（强制检查，忽略节流）供 About 手动按钮 + 区分「已是最新」提示。
2. **每日自动检查触发**：在 `App.tsx`（或 AppContext）顶层 useEffect（应用启动一次）调 `checkForUpdateDailyThrottled()`。有更新 → 弹提醒 UI。
3. **提醒 UI**：发现更新时展示「发现新版本 vX.Y.Z」+ 更新内容（update.body 若有）+ 「立即更新 / 稍后」两按钮。实现选项：
   - 复用项目 modal/toast 模式（参考现有 floating/glass 浮层；遵 floating-bg-variable 不透明 + 不用 fixed-transform 破坏）；或轻量自定义 modal。**禁用原生 confirm/alert**（破坏 Tauri，见项目约束 navGuard 禁原生 confirm）。
   - 「稍后」关闭（当天不再弹——节流时间戳已写，次日再检）。
4. **手动检查**（About 页）：加「检查更新」按钮 → `checkForUpdateManual()`；有更新弹同 UI，无更新 toast「已是最新版本」。
5. **i18n**：提醒/按钮文案（发现新版本/更新内容/立即更新/稍后/检查更新/已是最新/更新失败）8 locale 全补；加 key 后 Counter 查重。

## 验收
- `yarn build`（tsc+vite）+ `yarn check:i18n` 过；locale 无重复 key。
- 后端零改动（`git diff --name-only` 不含 src-tauri；插件/权限已就绪）。
- 逻辑自查（无头无法实跑 GUI 更新）：
  - 启动调节流检查；localStorage 时间戳读写正确；>24h 才 check。
  - 有更新 → 弹非原生 modal（立即更新/稍后）；立即更新 → downloadAndInstall + relaunch；稍后 → 关闭。
  - About 手动检查忽略节流 + 无更新提示。
  - dev/check 失败静默不报错弹窗。
- 不破坏现有 App/About 渲染。

## 失败处理
- check() 在 dev/未签名/无网络抛错 → catch 静默（console.debug），不弹错误（仅手动检查时可提示失败）。
- relaunch 权限：capabilities 已含 process:allow-restart；若 downloadAndInstall/relaunch 报权限 → 检查 capabilities 是否需补 updater 下载相关权限，回报。
- 提醒 UI 勿用原生 confirm/alert（Tauri 禁忌）。
- 门禁红修到绿；卡住标 `需要:`。

## 注
端到端真实更新需打包签名版 + 实际 release（latest.json）才能验；本任务交付检查+提醒+触发链路 + 代码层正确，真实更新流程由 owner 在签名版实测。
