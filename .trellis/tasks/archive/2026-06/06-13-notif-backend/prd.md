# 通知 N1 — 后端核心

Parent: `06-13-system-notification` — 系统通知模块。共享契约见 `../06-13-system-notification/design.md`。

## Goal

实现通知后端核心：通知服务（TTS 跨平台/弹窗/提示音/收件箱持久化）+ NotificationSettings（按类型）+ 类型枚举 + /api/notify 端点 + 变量替换 + commands + 冻结 api.ts 契约。完成后：调端点按类型设置分发通知；设置/收件箱可经 command 读写。

## What I already know
- 依赖：无（独立）；与其他后端树共享 lib.rs/models.rs/db.rs → 全局后端串行。
- 复用 tauri_plugin_notification（弹窗，lib.rs:2397）；/api 端点仿 /api/group-info（localhost+Bearer group_name）。
- 跨平台 TTS 用 `tts` crate。
- 数据模型/分发/变量/端点/契约见 parent design.md（权威）。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| N1.1 | models/db：枚举 + NotificationSettings + notification 表 + 迁移 | diff | serde 往返 + CRUD 单测 | P0 |
| N1.2 | notification.rs 分发服务（TTS/弹窗/提示音/收件箱 + 变量替换） | diff | 分发(按 form)/变量替换单测 | P0 |
| N1.3 | /api/notify 端点 + commands + api.ts 契约 | diff/契约 | 端点收消息触发；commands 注册 | P0 |

## Requirements
- NR1 类型枚举 task_complete/waiting_input/error/custom；NotificationSettings 按类型 {tts,popup,form,template}。
- NR2 呈现：TTS(tts crate 默认 + macSay + webspeech)/tauri 弹窗/提示音/收件箱(notification 表+未读)；音量跟随系统。
- NR3 /api/notify localhost POST + Bearer group_name；body {type,content?,vars?}。
- NR4 变量替换 {project}/{status}/{time}/{session}/{group}；未知占位保留。
- 总开关 OFF 旁路。

## Acceptance Criteria
- [ ] cargo test：分发(按 form 选通道)/设置往返/变量替换/收件箱 CRUD 单测过。
- [ ] /api/notify 端点收消息触发分发（集成/手测）。
- [ ] cargo clippy --all-targets -- -D warnings 零警告。
- [ ] api.ts 含 NotifType/NotifForm/TtsBackend + NotificationSettings/TypeSetting/Notification + notificationApi.*；yarn build 类型层过。
- [ ] commands 注册。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交；跨平台 TTS 选型 + 端点鉴权落 cortex；契约冻结通知 parent。

## Out of Scope
- hook 脚本生成/注入（N2）；前端 UI（N3）；独立音量。

## Technical Notes
- 新增 notification.rs；改 models.rs/db.rs/lib.rs(commands+端点)/proxy.rs(若端点挂 axum)/api.ts；Cargo.toml 加 tts crate。
- WebSpeech 后端：N1 emit tts 事件给前端（N3 webview speak），N1 侧仅 emit。
- **全局后端串行**：开工前确认无其他后端 child 改 lib.rs/models.rs，合入最新 master。
- 验证：cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cd .. && yarn build。
