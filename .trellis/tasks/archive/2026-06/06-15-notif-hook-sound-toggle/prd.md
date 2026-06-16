# 逐 Hook 事件加提示音(sound)独立开关

## 需求
逐 Hook 事件触发的每个 hook 缺「提示音」开关。当前 EventSetting={enabled,tts,popup,template}，sound 隐式=popup（弹窗自带系统音）。加**独立 sound 开关**，开启时实际播提示音（play_beep）。

## 现状（已核）
- `EventSetting`（models.rs ~1744）= {enabled,tts,popup,template}，无 sound。
- dispatch event 路径（notification.rs:267-294）：do_tts/do_popup，`sound: do_popup`（跟随弹窗）。
- `play_beep()`（notification.rs:468）已存在（跨平台提示音，notification_test_beep 命令用它）。
- 前端 `NotificationEventList.tsx`：每事件行 启用+TTS+弹窗 开关 + 模板 + 入参提示，无 sound。

## 实现
1. **models.rs**：`EventSetting` 加 `#[serde(default = "default_true")] pub sound: bool`；`Default` sound=true。旧 per_event 无 sound → 默认 true（向后兼容）。
2. **notification.rs dispatch event 路径**：`let do_sound = es.sound;`；当 `do_sound` 且 `app.is_some()` → `play_beep()`（独立于 popup）；`DispatchResult.sound = do_sound`（不再 =do_popup）。注意避免与 popup 系统音重复扰民——独立开关由用户控制即可，按字面 es.sound 播。
3. **api.ts**：`EventSetting` 加 `sound: boolean`。
4. **NotificationEventList.tsx**：每事件行加「提示音」开关（与 TTS/弹窗并列），绑 sound 字段；默认 on 跟随 Default。
5. **i18n**：加提示音开关标签 key（若需新文案）8 locale 全补；加 key 后 Counter 查重。复用现有「提示音」相关 key 若有（检查 notif.testBeepLabel 等可复用语义）。
6. 更新相关测试：EventSetting 新字段 roundtrip + 向后兼容（旧无 sound→true）+ dispatch event do_sound。

## 验收
- `cd src-tauri && cargo build && cargo clippy --quiet`（零项目 warning）+ `cargo test`（全过，含 sound 字段测试）。
- `yarn build` + `yarn check:i18n` 过；locale 无重复 key。
- 行为：每事件行有 启用/TTS/弹窗/提示音 四开关；提示音开 → 触发时 play_beep；旧配置无 sound → 默认开。
- 不破坏 type 路径（Codex）+ 注入泛化。

## 失败处理
- 提示音与弹窗系统音重复 → 仍按 es.sound 字面播（用户可分别关）；记录说明。
- 门禁红修到绿；范围外标 `需要:`。
