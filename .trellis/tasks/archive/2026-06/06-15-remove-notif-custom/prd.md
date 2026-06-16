# 移除通知 Custom 类型（4→3）

## 需求
删除通知类型 `Custom`，类型从 4 减为 3（task_complete / waiting_input / error）。用户确认「删 Custom 通知类型」。

> 编排：本任务**独立先做**；后续另起 task「按 claude code 全量 hook 事件加通知」（先删后加，本任务不含）。

## 关键设计：Custom 的双重身份
`NotifType::Custom` 当前兼任两职：① 设置页 4 类型之一；② **未知入站 type 的兜底桶**——`from_str_or_custom`（models.rs:1666）把任意未知 type 字符串映射到 Custom。删变体后**未知 type 必须有新归宿**。
- **方案（定）**：未知/空 type **兜底到 `TaskComplete`**（通知绝不丢；content 仍作 body 用）。把 `from_str_or_custom` 改名 `from_str_or_default`，`_ => NotifType::TaskComplete`。
- per_type 是 `HashMap<String,TypeSetting>`（models.rs:1752），**无需 DB 迁移**：旧 DB 里残留的 `"custom"` key 成孤儿条目，无 NotifType 再查它，无害（不强制清理；如顺手可在 load 时 strip，但非必须）。

## 触点清单（已核 file:line）
### 后端 `src-tauri/src/gateway/models.rs`
- :1644 枚举 doc 注释（去「custom 占位」描述）
- :1651 删 `Custom,` 变体
- :1661 删 `NotifType::Custom => "custom"` as_str arm
- :1666 `from_str_or_custom` → 改名 `from_str_or_default`，:1671 `_ => NotifType::TaskComplete`
- :1684 删 `NotifType::Custom => "{project} 通知"` default_template arm
- :1891 测试 serde 表删 `(NotifType::Custom, "\"custom\"")` 行
- :1899 测试 `from_str_or_custom("unknown_xyz") == Custom` → 改 `from_str_or_default("unknown_xyz") == TaskComplete`

### 后端 `src-tauri/src/gateway/notification.rs`
- :179 删 `NotifType::Custom => "Notification"` default_title arm
- :194 `from_str_or_custom` 调用点 → 改名后的 `from_str_or_default`
- :466/:480/:493/:515 测试中 `NotifType::Custom` 用例 → 删除或改用其它类型（保持测试语义：默认模板/兜底覆盖仍由 task_complete/error 覆盖即可）
- :605 `dispatch_unknown_type_as_custom_full` → 改名 `dispatch_unknown_type_as_task_complete`（或类似），:612 断言 `notif_type == "task_complete"`（不再 "custom"）

### 前端
- `src/services/api.ts`:874-875 `NotifType` 联合类型删 `"custom"` + 更新注释（去「custom = 用户自定义类型」）
- `src/components/settings/NotificationSettings.tsx`:22 `NOTIF_TYPES` 删 `"custom"`；:37 `NOTIF_DEFAULT_TEMPLATES` 删 `custom` 键
- 全 grep 确认无其它 `NotifType` 含 custom 的消费点遗漏（popover.tsx:93 的 `mode==="custom"` 是颜色模式，**无关，勿动**）

### i18n（8 locale）
- 删 `notif.type.custom` key（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES 全删，保持对称）
- 确认无其它 key 引用该类型；`yarn check:i18n` 须过（删除引用无处的 key 不应报缺失）

## 验收
- `cd src-tauri && cargo build && cargo clippy --quiet`（零项目 warning；删变体后 match 须穷尽 3 类型，编译器会强制）
- `cd src-tauri && cargo test`（notification + models 全过，含改名后的兜底测试）
- `yarn build`（tsc：NotifType 联合类型收窄后无类型错误）+ `yarn check:i18n` 过
- 重复 key 复检脚本（动了 locale）无新增重复
- 行为：未知/空入站 type → 落 task_complete（dispatch 测试断言）；设置页只剩 3 类型；无 `custom` 残留引用（grep `NotifType.*[Cc]ustom` 仅剩注释/无关颜色 mode）

## 失败处理
- 删变体后出现非穷尽 match 编译错 → 逐个补齐（这是预期，编译器是帮手）。
- tsc 报某处仍用 "custom" 作 NotifType → 那是遗漏消费点，补改；若涉及范围外文件标 `需要:` 回报。
- check:i18n 因删 key 报错（不应发生）→ 排查是否有 t("notif.type.custom") 残留引用，有则一并清。
