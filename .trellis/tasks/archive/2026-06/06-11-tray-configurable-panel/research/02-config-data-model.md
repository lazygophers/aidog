# Research: tray 配置数据模型

- **Query**: settings 存 tray 配置（多平台 + 排序 + 每项样式 + 今日消耗 + 全局布局）
- **Scope**: 内部（settings KV 机制）
- **Date**: 2026-06-11

## 现有 settings KV 机制（可直接复用）

后端 `settings_get/settings_set/settings_delete/settings_list`（`src-tauri/src/lib.rs:1037-1062`）：
- `db::get_setting(scope, key) -> Option<Value>`（`gateway/db.rs:560`），`set_setting(SetSettingInput{scope,key,value})`（db.rs:579）。
- 存储 = settings 表，value 为 JSON。先例：`proxy_log_settings`（scope="proxy",key="logging"）、`app/logging`、`pricing/sync`、`proxy/timeout`。

前端 `settingsApi.get/set/delete/list(scope,key,value)`（`src/services/api.ts:497-509`）：通用 KV，value 任意 `Record<string,any>`。

→ **新增 tray 配置无需建表 / 改 schema**：`settingsApi.set("tray", "config", {...})`。

## 建议数据模型（scope="tray", key="config"）

```jsonc
{
  "version": 1,
  "layout": "single_line",        // "single_line" | "two_line"
  "separator": " | ",             // 段间分隔符（single_line 时）
  "items": [
    {
      "type": "platform",         // "platform" | "today_usage"
      "platform_id": 12,          // type=platform 时必填
      "display": "balance",       // platform: "balance" | "coding"
      "label": "GLM",             // 可选覆盖显示名（默认平台 name）
      "color": "system_orange",   // "follow" | "system_red/green/blue/orange" | "#RRGGBB"
      "font_size": 9.0,           // pt
      "enabled": true,
      "order": 0
    },
    {
      "type": "today_usage",
      "metric": "cost",           // "cost" | "tokens"（见 05 今日消耗）
      "label": "今日",
      "color": "follow",
      "font_size": 9.0,
      "enabled": true,
      "order": 1
    }
  ]
}
```

设计要点（与 01 技术边界对齐）：
- **不放"绝对位置"字段**（做不到）；位置 = `order`（排序）+ 可选 `align`（全局）。
- `color` 三态：follow（labelColor 自适应）/ 预设语义色 / 自定义 hex。
- `layout` 全局二选一（单行横排 / 两行）；行数 >2 不提供。
- `items` 数组顺序或 `order` 字段二选一驱动排序（建议直接用数组顺序，拖拽后整体保存，免维护 order 字段——与 group_reorder 模式一致，见 04）。

## 兼容 / 迁移考量

- 旧 `platform.show_in_tray / tray_display`（单平台互斥）→ 见 `06-migration.md`。建议首次读 tray config 为空时，从旧 show_in_tray 平台生成单 item 默认配置（平滑迁移），或一次性迁移脚本。
- 后端渲染读取：`get_setting("tray","config")`，解析失败/空 → 降级（无 tray 文字，恢复 logo，等同现 `tray_quota_text` 返回 None）。

## Caveats

- settings value 是 `Option<serde_json::Value>`，后端需新增一个 typed struct（如 `TrayConfig`）+ serde 反序列化，参考 `ProxyLogSettings`（lib.rs:873-895）/`PriceSyncSettings` 先例。
