# PRD: tray 单/两行改为每平台项配置

## 需求
单行/两行不再是全局 TrayConfig.layout，改为**每个 tray item 各自配置** line_mode（每平台项独立选单行"名 值"或两行"名\n值"）。

## 现状
- models.rs TrayConfig.layout(single_line/two_line) 全局 + separator；lib.rs tray_layout 读全局；前端 TrayConfigTab 全局 layout 切换
- 渲染：全局 layout 决定所有 item 拼接（single separator / two \n）

## 改动
- **models.rs**: TrayItem 加 `line_mode: String`("single"|"two", default "single")；TrayConfig **删 layout**（保留 separator 作多 item 间分隔）
- **lib.rs 渲染**: 每 item 按自身 line_mode 渲染段（single→"名 值" 同段 / two→"名\n值" 段内换行）；多 item 间 separator 拼接；**整体 ≤2 行约束**：拼接后总行数 >2 时 fallback（超出的 two item 降为 single，保证菜单栏 ≤2 行）
- **前端 TrayConfigTab**: 每项加 line_mode 选择（单行/两行），删全局 layout 切换；separator 保留（多 item 间隔，全局）
- **api.ts**: TrayItem +line_mode，TrayConfig 删 layout
- 迁移：旧 config.layout → 各 item line_mode（全局 two_line → 所有 item two；single → all single）；或 default single

## 验收
- 每平台项可独立选单/两行
- 渲染按 per-item line_mode（菜单栏 ≤2 行 fallback）
- 删全局 layout 切换
- cargo test + tsc

## Subtask
- ST1: 后端 TrayItem line_mode + 渲染 per-item + 删全局 layout + 迁移
- ST2: 前端 per-item line_mode UI + api.ts + 删全局 layout 切换
