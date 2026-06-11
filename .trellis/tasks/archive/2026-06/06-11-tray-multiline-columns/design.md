# Design: tray 两行多列列对齐

## 渲染重构（lib.rs set_tray_attributed_title + tray_segments）
替换"段内 \n + separator 拼接" → **两行多列 NSTextTab**：

### 数据收集（tray_segments 改）
- 遍历 enabled items 按 order → `Vec<TrayColumn{ name:String, value:String, color:TrayColor, font_size:f64, two_line:bool }>`
- platform item: name=平台名, value=balance/coding 值；today_usage: name="今日", value="{n} tok"
- 删除"多 item 强制 single"逻辑（allow_two）

### 渲染（NSMutableAttributedString 两行 + NSTextTab）
- 有任一 two_line 列 → 两行模式；否则单行（separator 横排，保留旧路径）
- 两行模式：
  - 第一行 = 各列首段：two_line 列→name；single 列→"name value"
  - 第二行 = 各列次段：two_line 列→value；single 列→""（占位，tab 推进）
  - 列间用 `\t`；行间一个 `\n`
  - NSParagraphStyle.tabStops = 每列一个 `NSTextTab(alignment:left, location: 累加列宽)`；列宽按该列 max(name,value) 字符数 × 估字宽（等宽 menuBarFont 下 charWidth≈fontSize*0.6）+ padding
  - per-column 着色/字号：构造整串后 setAttributes(NSForegroundColor+NSFont):range: 对每列每行的 range（或逐段 append 带 attributes，配 tab 字符）
  - 保留垂直居中 paragraph（lineHeight min==max + baselineOffset）合并进同一 NSParagraphStyle（tabStops + lineHeight + center）
- 单行模式（无 two_line）：沿用现 separator append 路径

### 实现要点
- NSTextTab: `objc2_app_kit::NSTextTab::initWithType_location` 或 `initWithTextAlignment_location_options`；NSParagraphStyle setTabStops:
- 列宽估算：等宽字体（menuBarFont 近似等宽数字；或显式用 monospacedDigitSystemFont 保数字对齐）；location 累加
- range 着色：先拼完整两行 String（含 \t \n）记录每列每行的 char range，再 setAttributes:range: 逐列上色/字号
- 难点 fallback：若 NSTextTab 列对齐实现受阻 → 等宽字体 + 空格填充每列到 max 宽（保证上下对齐），降级但效果近似

## 不改
- per-item line_mode（上个 task 已有，前端 UI 已有）；TrayConfig 模型；迁移
- 颜色三态 resolve_tray_color；今日 tokens

## 验证
- cargo test + tsc；两行模式列对齐逻辑（列宽/tab/range 着色）；单行模式无回归；GUI 列对齐+垂直 用户 macOS 验
