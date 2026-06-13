# PRD — Popover 圆角外白底消除

## 背景

a57f524 `style(popover): opaque background` 为实现「不透明 popover」, 把窗口设为 `transparent(false)` + body 背景 `var(--bg)` 实色。后果: popover-root 有 `border-radius`, 圆角外四角露出不透明窗口的实色矩形 = 用户看到的「底下白色」。

## 用户需求

- popover 本体保持**不透明实色**(a57f524 的初衷正确, 不回退 glass 半透明)。
- **消除圆角外的白底**: 圆角外应透明(透出桌面), 而非实色矩形。
- 用户明确指出: 只需调「不透明度」即可 —— 即**窗口保持透明**, 仅 popover 本体不透明, 不该把窗口整体设为不透明实色。

## 根因

| 项 | a57f524 改动 | 问题 |
| --- | --- | --- |
| `src-tauri/src/lib.rs` | `transparent(true)` → `transparent(false)` | 窗口矩形不透明, 圆角外露实色 |
| `popover.css` html/body | `transparent` → `var(--bg)` | body 实色填满矩形 |

## 方案 (MVP)

1. `src-tauri/src/lib.rs`: 窗口恢复 `.transparent(true)`。
2. `popover.css` `html, body, #root`: 背景恢复 `transparent`。
3. `popover.css` `.popover-root`: **保留** a57f524 的不透明实色 `background: var(--bg-elevated, var(--bg))`(不恢复 glass 半透明 / backdrop-filter)。

= 透明窗口 + 不透明圆角本体, 圆角外透明无白底。

## 交付 2 — Popover 失焦自动关闭 (补充)

### 需求
点击 tray 出现 popover; 当焦点移到别处(点击其他窗口/桌面)时 popover 应自动关闭。

### 方案
Tauri 2.0 popover 窗口构建后挂 `on_window_event`, 监听 `WindowEvent::Focused(false)` → `close()`(或 hide)。已 `.focused(true)` 构建, 失焦即关闭符合预期。需防与构建时短暂失焦争用(必要时判窗口已就绪)。

### 验收
- 点击 tray 弹出 popover。
- 点击 popover 外部(别的窗口/桌面)→ popover 自动消失。
- 不误关(刚弹出瞬间不被构建过程误触发关闭)。

## 验收 (总)

- popover 本体不透明实色(看不到桌面/穿透)。
- 圆角四角外无白色/实色矩形(透出桌面)。
- 失焦自动关闭(交付 2)。
- light/dark 主题均正常。
- `cargo build` + `tsc` 无新增 warning/error。
