# Research: 数据模型现状 + 扩展点

- **Query**: NotificationSettings / TypeSetting / DB 存储 / 逐事件配置扩展点
- **Scope**: internal
- **Date**: 2026-06-15

## 现状模型 — models.rs:1642-1768

### NotifType（3 类型）— models.rs:1645-1684
- 枚举 `TaskComplete / WaitingInput / Error`，serde snake_case。
- `as_str()` → `task_complete/waiting_input/error`（per_type key + DB notif_type 列字面量）。
- `from_str_or_default(s)` 未知 → TaskComplete（models.rs:1664-1671，**端点收任意 type 字符串都不丢**）。
- `default_template()` 内置模板（zh 硬编码，跨层镜像前端，models.rs:1677-1683）：完成/等待用户输入/出错。

### TypeSetting（per_type 值）— models.rs:1708-1733
```rust
struct TypeSetting { tts: bool, popup: bool, form: NotifForm, template: String }
```
默认 `{tts:true, popup:true, form:Full, template:""}`。`NotifForm`: PopupOnly/InboxOnly/SoundOnly/Full（默认 Full，models.rs:1687-1695）。

### NotificationSettings — models.rs:1736-1768
```rust
struct NotificationSettings {
    enabled: bool,          // 总开关
    tts_enabled: bool,      // TTS 总开关
    tts_backend: TtsBackend,
    per_type: HashMap<String, TypeSetting>,  // key = NotifType 字面量
}
```
`type_setting(t)`：per_type 缺省返回默认（models.rs:1765-1767）。所有字段 `#[serde(default)]`，**向后兼容良好**（旧 JSON 缺字段自动默认）。

## DB 存储 — db.rs:1829-1835

`get_notification_settings(db)`：从 settings KV **scope=`notification`, key=`settings`** 读，**整 JSON blob** `serde_json::from_value` 反序列化，失败/缺省 → `NotificationSettings::default()`。

写入：走通用 `set_setting(SetSettingInput{scope:"notification", key:"settings", value})`（见 lib.rs:2336-2340 seed_default_templates 示例）。

**关键**：是整对象 JSON blob 单行存，非字段列。新增字段只需加到 struct + serde default，**零 migration**。

## 「逐事件配置」扩展方案

### 数据形状建议
在 `NotificationSettings` 新增字段（向后兼容，旧无此字段 → 空 map）：
```rust
#[serde(default)]
pub per_event: HashMap<String, EventSetting>,  // key = CC 事件名 "Stop"/"SubagentStop"/...
```
```rust
struct EventSetting {
    enabled: bool,        // 该事件是否触发
    notif_type: String,   // 复用 NotifType 字面量，决定走哪套 tts/popup/form/模板
    template: String,     // 可选自定义文案（空则回退 per_type[notif_type].template / default_template）
}
```

### 持久化
- 挂在同一 `NotificationSettings` blob 里，复用 `get/set_notification_settings`，**无需新表/新 migration**。
- 向后兼容：旧用户无 `per_event` → 反序列化为空 map → 用「精选默认集」兜底（见 05）。精选默认建议在 **读取后用代码兜底**（类似 `seed_default_templates`），或前端 DEFAULT_SETTINGS 提供，避免空 map 时无任何事件启用。

### 与现有「按类型配置」关系
- `per_type` 继续作「类型 → tts/popup/form/模板」的呈现配置载体。
- `per_event` 只决定「哪些事件开 + 各自走哪个 type + 可选覆盖文案」。
- 渲染时：事件触发 → 查 `per_event[event]` 拿 notif_type + template → 若 template 空回退 `per_type[notif_type].template` → 再回退 `default_template`。dispatch 的 tts/popup/form 仍取 `per_type[notif_type]`（复用现有 channels_for_form 逻辑，见 03）。

## 精选默认集物化建议
- 不建议写死进 DB（用户改不回默认）。建议：`per_event` 为空时，前端/后端用常量「精选默认集」展示，用户首次改动才落 DB。
- 或仿 `seed_default_templates`：开总开关时 seed 精选事件（但这会污染「用户主动关某事件」语义，需谨慎）。**推荐纯展示层默认 + 用户改动才存**。

## Caveats
- per_type key 用 NotifType 字面量；per_event 的 notif_type 字段也必须落在 3 类型内（否则 from_str_or_default 兜底 task_complete，dispatch 不报错但语义错）。前端选择器应限定 3 类型。
- 若未来事件特有字段要进模板（如 {agent_type}），需在 dispatch/render 的 vars 里注入，模型层无需改（substitute_vars 已支持任意 key，见 03）。
