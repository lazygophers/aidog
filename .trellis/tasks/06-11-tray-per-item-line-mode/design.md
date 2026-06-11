# Design: tray per-item line_mode

## 后端
- models.rs: `TrayItem` 加 `#[serde(default = "default_line_mode")] line_mode: String`（"single"|"two"）；`default_line_mode()->"single"`；TrayConfig 删 `layout` + default_layout（保留 separator）
- lib.rs:
  - 删 tray_layout(全局 layout)；渲染遍历 items：每 item 文字按 line_mode —— single → `format!("{name} {value}")` 单段；two → `format!("{name}\n{value}")`（段内 \n）
  - 多 item 拼接：separator 连接各 item 段
  - **≤2 行约束**：统计拼接后 \n 数；若 >1（多个 two 或会超2行）→ 保留首个 two 的换行、其余 two 降 single（或全降 single），保证 ≤2 行。简单实现：仅当**单一 item 且 two** 时用两行；多 item 时全部按 single 横排（多 item 两行菜单栏无法表达）→ MVP 规则：items.len()==1 时尊重 line_mode；len>1 时强制 single 横排（注释说明菜单栏约束）
  - 迁移：读旧 config 若有 layout 字段 → 映射各 item line_mode（two_line→two, single_line→single）；无则 default single
- api.ts: TrayItem +line_mode，TrayConfig 删 layout

## 前端 TrayConfigTab
- 每项编辑加 line_mode 切换（单行/两行 segmented）
- 删全局 layout 切换 UI（:161-179 区）；separator 保留（多 item 间隔）

## 验证
- cargo test + tsc；单 item 尊重 line_mode 两行；多 item 横排单行；迁移旧 layout
