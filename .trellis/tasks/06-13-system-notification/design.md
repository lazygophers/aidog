# Design — 系统通知模块

> 架构契约。N1/N2/N3 据此落地。PRD 见 `prd.md`。

## 数据模型（契约 — Rust ↔ TS 字段名一致）

### 通知类型枚举
```rust
enum NotifType { TaskComplete, WaitingInput, Error, Custom }  // serde snake_case
enum NotifForm { PopupOnly, InboxOnly, SoundOnly, Full }       // 仅弹窗/仅收件箱/仅提示音/完整播报
enum TtsBackend { CrossPlatform, MacSay, WebSpeech }           // 默认 CrossPlatform
```

### NotificationSettings（settings KV scope=`notification`）
```rust
pub struct NotificationSettings {
  pub enabled: bool,                        // 总开关 default true
  pub tts_enabled: bool,                    // default true
  pub tts_backend: TtsBackend,              // default CrossPlatform
  pub per_type: HashMap<String, TypeSetting>, // key=NotifType
}
pub struct TypeSetting { pub tts: bool, pub popup: bool, pub form: NotifForm, pub template: String }
// 音量无字段（跟随系统）
```

### notification 表（收件箱持久化）
```sql
CREATE TABLE IF NOT EXISTS notification (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  notif_type TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL DEFAULT '',
  read INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_notif_read ON notification(read, created_at);
```

### 自定义通知模板（用户定义的 custom 类型 + 内置模板覆盖）
存 NotificationSettings.per_type[type].template，含变量占位。

## 变量替换
占位 `{project}`(cwd basename)/`{status}`/`{time}`/`{session}`/`{group}`。端点收到 vars map → 模板替换。未知占位保留原文。

## 通知分发（notification.rs）
```
notify(type, content, vars):
  若 settings.enabled==false → return
  setting = per_type[type]（缺省全 true + Full）
  替换变量得 title/body
  按 setting.form 决定通道:
    Full       → TTS + popup + sound + inbox
    PopupOnly  → popup (+ inbox 落库)
    InboxOnly  → inbox
    SoundOnly  → sound
  TTS: setting.tts && settings.tts_enabled → 按 tts_backend 播报
    CrossPlatform: tts crate; MacSay: Command `say`; WebSpeech: emit 事件给前端 webview
  popup: tauri_plugin_notification
  inbox: insert notification 表 + emit 未读更新事件
  音量跟随系统（不设置）
```

## /api 通知端点（axum，proxy.rs router 或 lib.rs 本地 server）
- `POST /api/notify`：localhost-only，Bearer group_name（仿现有 /api/group-info）。body `{type, content?, vars?}`。
- hook 脚本调用此端点触发通知。

## hook 集成（N2，仿 statusline lib.rs:1604 范式）
- 生成脚本到 ~/.aidog/：`aidog-notify-complete.sh`（POST type=task_complete）、`aidog-notify-waiting.sh`（type=waiting_input）。脚本用 ANTHROPIC_BASE_URL 推导端点 + ANTHROPIC_AUTH_TOKEN(=group_name) 鉴权，project=cwd basename 作 vars。
- 一键注入：
  - **Claude Code** settings.{group}.json：写 `hooks.Stop`（任务完成）+ `hooks.Notification`（等待输入），指向脚本。`do_sync_group_settings` 的 strip 列表加 hook 标记（防回写污染，仿 _aidog_statusline）。
  - **Codex** config.toml（codex.rs）：写 `notify = ["<脚本>"]`（Codex notify 机制）。
- 移除：一键 strip。

## 契约冻结（N1 产出，N3 消费）
N1 在 api.ts 写：NotifType/NotifForm/TtsBackend 字面量 + NotificationSettings/TypeSetting/Notification(收件箱项) 类型 + `notificationApi.{getSettings,setSettings,listInbox,markRead,clearInbox,testNotify, injectHooks(group,client), removeHooks(group,client)}`。N3 消费不改契约。N2 的注入命令也在 N1 的契约里冻结（injectHooks/removeHooks）或 N2 自行加（约定 N1 冻结读写设置，N2 加 hook 注入命令）。

## 资源边界 / 跨树串行
N1/N2 改 lib.rs/models.rs/db.rs/codex.rs/proxy.rs(端点) → 与中间件树(C2-C4)/group树(GA) 全局后端串行。N3 改前端(AppSettings/新通知中心页/api.ts 消费/i18n) → 与 C5/GB 前端串行。

## Rollback
通知总开关 OFF 旁路；hook 一键移除；worktree 隔离。

## TTS 依赖
`tts` crate（跨平台）。验证 Windows 适配（WinRT/SAPI）。MacSay 走 std::process `say`。WebSpeech 走前端事件（N3 配合：N1 emit tts 事件，前端 webview speak）。
