# macOS 通知授权分层引导

## 背景
弹窗通知已统一走 `tauri-plugin-notification`（commit 6d89e36，删 osascript）。但桌面端插件 `permission_state`/`request_permission` 硬编码 `Ok(Granted)`（registry 2.3.3 desktop.rs:61-67），发送走 `notify_rust` 旧 `NSUserNotification`，**无法弹原生授权框、无法可靠探测被禁**。约束：**不依赖 osascript**；不引 objc FFI（用户已选轻量分层方案 ①②③）。

目标：尽可能减少「通知静默不出现」的不授权情况，把「静默坏」变「可见可一键修」。

## 范围（三层，全部不碰 osascript / 不引 FFI）

### ① 启动时调 request_permission
- `src-tauri/src/lib.rs` setup() 闭包内（:3399，建议放 plugin/DB 初始化之后、托盘之前的安全位置）调用一次 `app.notification().request_permission()`（需 `use tauri_plugin_notification::NotificationExt;`）。
- desktop 上是 no-op 返回 Granted（无害）；mobile 真实弹框；未来兼容。
- 结果用 `tracing::info!`/`warn!` 记录（如 `notify: request_permission state={:?}`）。失败仅 warn，不 panic、不阻塞启动。

### ② 通知设置页常驻「打开系统通知设置」引导
- 文件：`src/components/settings/NotificationSettings.tsx`。
- 加一段说明 + 按钮：文案类似「没收到系统通知？可能需在系统设置中允许 aidog 发送通知」+ 按钮「打开系统通知设置」。
- 按钮点击用 `import { openUrl } from "@tauri-apps/plugin-opener";`（参考 src/pages/About.tsx:7,177 已有用法）打开 macOS 系统通知设置面板。
  - URL scheme：优先 `x-apple.systempreferences:com.apple.Notifications-Settings.extension`（Ventura/13+ 新设置app）。若 openUrl 对该 scheme 被 capability 拦截 → 在 `src-tauri/capabilities/default.json` 的 opener 段补 `opener:allow-open-url`（+ 必要 scope），并验证 `opener:default` 是否已含。先实测 default 能否开该 scheme，不够再补权限。
  - **仅 macOS 显示此按钮**：用平台判断（`@tauri-apps/plugin-os` 的 platform()，或复用项目已有平台判定方式——先查项目是否已有 isMacOS/platform 工具，有则复用，无则用 plugin-os）。非 macOS 隐藏（Windows/Linux 通知一般默认可用，避免误导）。
- i18n：新增文案 key 必须 7 语言全补（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP），跑 `yarn check:i18n` 必须过。参考项目现有 i18n 落 key 方式（查 NotificationSettings.tsx 现有 t() 用法 + i18n 资源文件位置）。

### ③ 签名 + 公证文档化（治本说明）
- 真正保证 macOS 默认授权 = app 已签名 + 公证（bundle 注册即默认开启通知）。
- 在 `notification.rs::show_popup` doc 注释补一行指向「打包须签名公证，否则 macOS 可能静默吞通知」（已有类似注释，确认/强化即可）。
- 若项目有 docs 站点通知相关页或 release/打包文档，补一句签名公证对通知的影响（先查 docs/ 是否有通知页；无则跳过，不强造）。

## 验收
- `cd src-tauri && cargo build && cargo clippy --quiet`（零 warning）+ `cargo test`（notification 套件全过）。
- `yarn build`（tsc + vite）通过；`yarn check:i18n` 通过（无裸 key / 无缺语言）。
- ① request_permission 在 setup 调用且日志可见；不 panic。
- ② NotificationSettings 页 macOS 下出现「打开系统通知设置」按钮，点击能打开系统设置通知面板（dev 实测一次）；非 macOS 隐藏。
- 全程无 osascript、无 objc FFI、无新增 unsafe。

## 失败处理
- openUrl 对 `x-apple.systempreferences:` scheme 被拦 → 先尝试补 opener capability；仍不行则改用 fallback scheme `x-apple.systempreferences:com.apple.preference.notifications`（旧）并记录；两者都不行则在 prd 标注、回报主会话定夺，不静默放弃。
- 平台判定工具不存在且 plugin-os 未装 → 回报主会话（是否装 @tauri-apps/plugin-os），不擅自加重依赖。
