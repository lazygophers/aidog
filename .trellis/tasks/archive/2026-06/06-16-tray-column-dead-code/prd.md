# 清除 TrayColumn 非 macOS dead_code warning

## 背景
Windows 编译报 warning（src\lib.rs:2913）：`TrayColumn` 字段 `font_size`/`two_line`/`align`/`align_row2` never read。

## 根因
- `tray_layout()`（lib.rs:2956，构造 `Vec<TrayColumn>`）无 cfg，全平台执行。
- `set_tray_attributed_title()`（lib.rs:3171，读取这 4 字段）`#[cfg(target_os = "macos")]`，仅 macOS。
- 非 macOS 路径只用 `name`/`value`/`color`（fallback_text @3393），4 布局字段是 macOS 富文本渲染专属参数 → 非 macOS 编译时 dead。
- macOS 上 4 字段全有读取点（align@3329, font_size@3333, two_line@3196, align_row2@3349），非真死代码。

## 修复
4 字段各加属性：
```rust
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
font_size: f64,
```
同样作用于 two_line / align / align_row2。

理由：平台条件死代码的合法 idiom，非掩盖逻辑缺陷。符 [[warnings-are-issues]]（真清 warning，非存借口）。

## 验证
- macOS：`cargo build` + `cargo clippy` 无 warning（字段仍被读，allow 不影响）。
- 确认 TrayColumn 其余字段（name/value/color）无新 warning。

## 非目标
- 不重构 TrayColumn 为平台分裂 struct。
- 不动渲染逻辑。
