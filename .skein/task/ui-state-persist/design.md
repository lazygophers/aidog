# ui-state-persist — 详细设计

## 存储模型
extra TEXT JSON 加 _ui_* 键(前端写,后端透明携带):

| UI 态 | 存储位置 | 键 | 值 |
|---|---|---|---|
| Groups 分组折叠 | group.extra | `_ui_collapsed` | bool |
| Platforms 页卡展开 | platform.extra | `_ui_expand_plat` | bool |
| Groups 页内卡展开 | platform.extra | `_ui_expand_grp` | bool |

后端 extra 是 `String`(JSON TEXT)。业务解析函数(peak_hours_for / parse_disable_during_peak / parse_platform_peak_hours)用 `serde_json` 读己键,**加 _ui_* 不破坏**(天然忽略未知键)。

## 后端设计
新 Rust command:
```rust
#[tauri::command]
async fn set_ui_extra(
    db: State<'_, Db>,
    target: String,   // "group" | "platform"
    id: i64,
    key: String,      // "_ui_collapsed" | "_ui_expand_plat" | "_ui_expand_grp"
    value: serde_json::Value,
) -> Result<(), String>
```
- db.rs 加 helper `update_extra_key(table, id, key, value)`:读 extra JSON → set key → 写回(单 SQL UPDATE)
- 注册进 `generate_handler!`(startup.rs)— **新 command 需 yarn tauri dev 重启**(HMR 仅前端)
- 命令落 commands_platform crate(platform/group extra 同 handler)

## 前端设计
- `api.ts` 加 `setUiExtra(target, id, key, value)` invoke 封装 + TS 类型
- 通用 hook `useUiPersist(target, id, key, initial)`:初始化从 extra 读 + toggle debounce 300ms 调 setUiExtra
- 接入点:
  - Groups.tsx:118 `collapsedGroups` → 持久化版(per-group 读 _ui_collapsed)
  - PlatformListView:`expandedIds` → _ui_expand_plat
  - GroupListItem:内卡展开 → _ui_expand_grp
- debounce:`setTimeout 300ms` 合并连续 toggle,仅末次写 DB

## 数据流
```
toggle → state 更新(立即响应 UI) → debounce 300ms → setUiExtra invoke
  → Rust set_ui_extra → db.update_extra_key(读改写 extra JSON)→ DB

启动 → fetch groups/platforms(已返 extra)→ JSON.parse → 读 _ui_* → 初始化 state
```

## import/export strip
- export snapshot 阶段(Rust import_export/snapshot):对 extra JSON 删 _ui_* 键再导出
- import 阶段:不恢复 _ui_*(配置文件本就无)
- 仿现有 `_aidog_statusline` strip 模式(do_sync_group_settings)

## 取舍
- 选 extra(用户指定)而非独立表:语义略污染但免 migration / 免新表;_ 前缀 + strip 缓解
- debounce 300ms:平衡响应性与 DB 压力
- 各页独立键:W4 用户定,同 platform 两键并存,语义清晰但 extra 略胖
