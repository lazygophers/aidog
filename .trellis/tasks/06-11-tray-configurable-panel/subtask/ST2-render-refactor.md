# ST2: tray 渲染重构（多 item）

- **目标**: 托盘按 config 渲染多 item（颜色/字号/排序/布局）+ 今日 tokens
- **产出** (lib.rs/db.rs/Cargo.toml):
  - Cargo.toml objc2-app-kit +`"NSColor"` feature
  - db.rs `today_token_total(db) -> i64`：今日本地 0 点 ms → `SELECT SUM(input_tokens+output_tokens) FROM proxy_log WHERE created_at >= ? AND deleted_at=0`
  - lib.rs：替换单平台 tray_quota_text → 读 TrayConfig，遍历 enabled items 按 order：
    - platform item → est（balance 余额 / coding 剩余%，复用现逻辑）
    - today_usage item → today_token_total 格式化
    - 每 item 文字 + color(三态→NSColor: follow=labelColor / preset=systemRed/Green/Orange / custom=hex via NSColor) + NSFont(font_size) → NSMutableAttributedString append
    - layout single_line: separator 拼接；two_line: \n(≤2行)
  - set_tray_attributed_title 改多段 NSMutableAttributedString（每段 attributes），保留垂直居中(lineHeight/baselineOffset)
  - enabled items 非空隐 icon；空恢复 icon
- **验证**: cargo build 0
- **资源**: research/01-macos-menubar-boundary.md + 05-tray-render-refactor.md、design.md、a391ad4d 已落 attributedTitle
- **依赖**: ST1
- **失败处理**: NSColor hex 构造查 objc2-app-kit；多段 attributes 用 setAttributes range
