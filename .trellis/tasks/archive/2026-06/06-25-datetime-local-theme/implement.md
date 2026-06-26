# 实施计划 — datetime-local-theme

读 prd.md。范围 = 主题 CSS + Platforms.tsx input style。**前置: platform-expiry-toggle 须 finish**。

## S1 — 定位主题 CSS

grep `--bg-input` / `--text` / `--border` 变量定义位置 (`src/themes/`, `src/index.css`)。确认 input 通用类 (`.input`?) 现有染色, datetime-local 是否漏。

## S2 — CSS 染色 input 本体

主题 CSS 加:
```css
input[type="datetime-local"] {
  color: var(--text);
  background: var(--bg-input, var(--bg));
  border: 1px solid var(--border);
  color-scheme: light dark;  /* 兜底 */
}
input[type="datetime-local"]::-webkit-calendar-picker-indicator {
  /* 暗主题 indicator 可见 */
  filter: var(--calendar-indicator-filter, none);
}
```
暗主题 (mode=dark) 下 `--calendar-indicator-filter: invert(1)` 或 hue-rotate。变量定义到主题文件。

## S3 — Platforms.tsx input

line 3162 `style={{ colorScheme: themeMode }}`:
- colorScheme 保留 (兜底, 无害) 或删 (确认无效则删)
- 确认 input className 走通用 `.input` 类 (若已染色则 S2 仅补 indicator)

## S4 — 实测各主题

agent 改完实测 (或报告 main 手动验):
- light + dark × 各色板 (Liquid Glass 主题集)
- input 本体: 文字/背景/边框跟主题
- indicator 图标: 暗主题不黑糊
- 弹层: 记录现象 (跟系统 or 白底刺眼)

弹层严重不一致 → 报 main, 拆子 task 换 react-day-picker。

## 验收

1. S1-S4 全做
2. `yarn build` + `cargo test` + `cargo clippy --all-targets -- -D warnings` + `check-i18n.mjs` 全绿
3. 无新 warning
4. 手动验 prd 验收 1-3

## 执行

单 agent。前置 platform-expiry-toggle finish 后 start。

## 禁

- 禁改后端 / 业务逻辑
- 禁改 datetime-local 结构 (platform-expiry-toggle 负责)
- 禁 git commit / push
