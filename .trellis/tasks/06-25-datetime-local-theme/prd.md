# datetime-local 主题适配

## 需求 (用户)

全部主题下 datetime-local 输入框都没跟主题 (Tauri WKWebView 原生控件)。需适配: input 本体 (文字/背景/边框) + 日历弹出层。

## 现状

- `src/pages/Platforms.tsx:3162` 用 `style={{ colorScheme: themeMode }}` — 用户确认 **全部主题都不对**, 即 colorScheme 在 Tauri WKWebView 对原生 datetime-local 控件无效
- `src/themes/useThemeMode.ts` 已有 hook 读 `documentElement.dataset.mode` + MutationObserver

## 根因

`color-scheme` CSS 属性控原生表单控件外观, 但 Tauri macOS WKWebView 的 datetime-local 原生控件 (NSDatePicker 风格) 不响应 web `color-scheme`, 或响应不全 → 各主题下渲染异常。

## 方案

### 方案 A — CSS 强制染色 input 本体 (先做)

- `input[type="datetime-local"]` 加 CSS: `color: var(--text)`, `background: var(--bg-input)`, `border-color: var(--border)`
- `::-webkit-calendar-picker-indicator` 染色: 暗主题 `filter: invert(1)` 或 color 属性
- input 本体各主题跟主题变量

### 方案 B — 日历弹出层

- 原生弹出层 (点击 input 弹出的日历) 无法 CSS 染色
- 三选一:
  1. 接受弹层默认 (白底/暗底由系统定, 不跟主题) — 最简, 体验差
  2. `colorScheme` + `color-scheme: light dark` 兜底 (可能部分有效)
  3. 换自定义日期组件 (react-day-picker 等) 完全控样式 — 工作量大, 彻底解决

agent 实测各主题后定: input 本体必做 (方案 A), 弹层先试 colorScheme 兜底 (方案 B2), 实在不跟主题且体验差 → 报 main 拆子 task 换 react-day-picker (B3)。

## 依赖

**串行**: 依赖 `platform-expiry-toggle` 先完成 (datetime-local 被 toggle 包裹后的形态才适配, 避免结构改完再改 CSS 重复劳动)。platform-expiry-toggle finish 后再 start 本 task。

## 改造点

1. 定位主题 CSS (grep `--bg-input` / `--text` 定义, `src/themes/` 或 `src/index.css`)
2. 加 `input[type="datetime-local"]` 染色规则 + `::-webkit-calendar-picker-indicator`
3. 暗主题 indicator filter 处理
4. Platforms.tsx:3162 colorScheme 可保留 (兜底) 或删 (无效则删)
5. 实测各主题 (light/dark × 各色板) input 本体 + 弹层渲染

## 验收

1. 各主题 (light + dark) datetime-local input 本体文字/背景/边框跟主题
2. 日历指示器图标各主题可见 (暗主题不黑糊)
3. 弹层: 至少不刺眼 (colorScheme 兜底); 若仍严重不一致 → 报 main 决定升 react-day-picker
4. `yarn build` + `cargo test` + `cargo clippy -- -D warnings` + `check-i18n.mjs` 全绿, 无新 warning

## 不改

- 后端 / 业务逻辑
- datetime-local 的使用位置结构 (由 platform-expiry-toggle 改)
