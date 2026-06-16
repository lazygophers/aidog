# 通知文案变更未及时生效

## 现象
用户在设置页改 `per_type[type].template`，**实时通知**（hook 触发 / `notification_test`）渲染 body 仍用旧/默认模板，非新值。

## 已排除（后端全实时，零 bug）
- `/api/notify` handler：type 字面量直传 dispatch，content=None，vars 注入 group/time（proxy.rs:361-432）
- dispatch：实时 `get_notification_settings` → `get_setting`（notification.rs:183）
- DB cache：`set_setting` 写后 `invalidate_settings_cache()` 清全表（db.rs），get miss 才查 DB，无 stale
- per_type key 三处一致：前端 NOTIF_TYPES / `NotifType::as_str()` / hook 脚本 `build_hook_script(notif_type)` 均 snake_case 字面量
- serde 对齐：api.ts `TypeSetting{tts,popup,form,template}` = Rust snake_case
- `seed_default_templates`：仅 template 空时填默认，不覆盖用户值（lib.rs:2321）

## 根因假设（前端，待确诊）
`src/components/settings/NotificationSettings.tsx`:
- `updateType`(L96-103) / `persist`(L82-90) 用**闭包捕获的 settings state** 合并，非 `setSettings(prev => ...)` functional update
- 无 debounce，每 onChange 一次 persist（乐观 setSettings + async DB 写）
- 风险：连续/交错 onChange 时后发 persist 基于旧 settings 闭包 → 新 template 被旧值覆盖写回 DB；或 async persist 失败时乐观更新不回滚（UI 新 / DB 旧）

## 修复方向
- persist/updateType 改 functional update：`setSettings(prev => compute(prev))`，持久化用计算后最新值
- template textarea 加 debounce（200ms）合并连续输入为一次 persist
- persist 失败回滚 prev + 错误提示

## 验证
- `cargo test`（后端无改动应全绿）
- `yarn build`（tsc）
- 手动：改 task_complete template → `notification_test` 返回 body 含新 template → 实时 hook 通知用新模板
