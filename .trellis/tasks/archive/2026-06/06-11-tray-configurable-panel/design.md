# Design: 可配置 tray 面板

详见 research/ 6 文档。本文聚焦决策落地。

## 配置模型（settings KV，scope=tray key=config）
```json
{
  "layout": "single_line",        // single_line(分隔符拼接) | two_line
  "separator": "  ",
  "items": [
    {"type": "platform", "platform_id": 1, "display": "coding",
     "color": {"mode": "preset", "value": "green"}, "font_size": 9, "enabled": true, "order": 0},
    {"type": "today_usage", "metric": "tokens",
     "color": {"mode": "follow"}, "font_size": 9, "enabled": true, "order": 1}
  ]
}
```
- color.mode: `follow`(labelColor 跟随系统) / `preset`(red/green/orange → NSColor.systemRed/Green/Orange 自适应) / `custom`(value=hex，前端警告可读性)
- 无 config → 迁移生成默认（旧 show_in_tray=1 平台 → 一个 platform item）

## 后端
### settings tray config (db/lib)
- TrayConfig struct（serde，items: Vec<TrayItem>）；read: settingsApi get(tray,config)；write: command platform... → settings set
- 迁移：refresh/启动时若 tray config 缺失，从 get_tray_platform(旧 show_in_tray) 生成默认 TrayConfig 存入

### tray 渲染重构 (lib.rs)
- 替换单平台 tray_quota_text：读 TrayConfig → 遍历 enabled items 按 order：
  - platform item → 取 est（display balance/coding，同现逻辑：coding 剩余%/balance 余额）
  - today_usage item → db 查今日 tokens（SUM proxy_log input+output WHERE created_at >= 今日本地0点 ms）
  - 每 item 文字段 + color(三态→NSColor) + font_size → NSMutableAttributedString append
  - layout single_line: separator 拼接；two_line: \n（≤2 行，超出截断/仅前2项两行）
- set_tray_attributed_title 改 NSMutableAttributedString 多段（每段 attributes 不同色/字号），保留 baselineOffset/lineHeight 垂直居中
- 有 enabled items 隐 icon；空 → 恢复 icon
- Cargo.toml objc2-app-kit +`"NSColor"` feature

### 今日 tokens (db.rs)
- `today_token_total(db) -> i64`：今日本地 0 点 ms = (now 本地日期 00:00 → ms)；`SELECT SUM(input_tokens+output_tokens) FROM proxy_log WHERE created_at >= ? AND deleted_at=0`

## 前端 (AppSettings.tsx 新 tab)
- AppSettings tab 加 `"tray"`，独立 `TrayConfigTab` 组件：
  - 平台多选（enabled 平台）→ 加为 platform item
  - 今日消耗项开关（today_usage tokens）
  - 拖拽排序（复用 Groups.tsx HTML5 DnD 模式 research/04）
  - 每项：display(balance/coding) + 颜色三态(跟随/预设下拉/自定义 colorpicker+警告) + 字号 + 开关删除
  - layout(单行/两行) + separator
  - 保存 → trayConfigApi.set(config) → 后端 settings + refresh tray
- 删 Platforms.tsx tray 开关（show_in_tray/tray_display UI）；旧列保留兼容(迁移读) 或后续清

## 迁移
- 启动/首次：无 tray config → 旧 show_in_tray=1 平台生成默认 [{platform item}]，存 settings
- 删平台卡片 tray 开关 UI（功能移设置页）

## 验证
- cargo build+test+tsc；tray 多 item 渲染(颜色/字号/排序/布局)；今日 tokens；设置页配置+排序；迁移默认；GUI 多色/字号用户验
