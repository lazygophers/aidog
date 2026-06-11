# ST1: 两行多列 NSTextTab 渲染

- **目标**: tray 多平台两行列对齐渲染
- **产出** (lib.rs):
  - tray_segments → 收集 `Vec<TrayColumn{name,value,color,font_size,two_line}>`；删"多 item 强制 single"(allow_two len==1)
  - set_tray_attributed_title 重构：有 two_line 列→两行模式（第一行各列 name/单行项"名 值"，第二行各列 value/单行项空，\t 分列 + 1 个 \n）；NSParagraphStyle.tabStops 每列 NSTextTab（location 按列 max(name,value) 字符×估字宽累加）；per-column setAttributes(NSForegroundColor+NSFont):range:；保留垂直居中(lineHeight+baselineOffset 合并同 paragraph)
  - 无 two_line → 单行 separator 路径（保留）
  - fallback：NSTextTab 受阻 → 等宽字体(monospacedDigitSystemFont)+空格填充列到 max 宽
- **验证**: cargo build + test 0
- **资源**: design.md、lib.rs:1284(tray_segments)/1490(set_tray_attributed_title)、objc2-app-kit NSTextTab/NSParagraphStyle
- **依赖**: 无
- **失败处理**: NSTextTab API 查 objc2-app-kit 0.3；难则等宽+填充 fallback
