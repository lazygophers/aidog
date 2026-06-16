# 修复单一 skill 卸载 modal 不弹出

## 现象

Skills 页点行内「卸载」按钮 → 二次确认 modal 不弹出（用户反馈"直接灰度没反应"）。toggle enable/disable 正常。后端日志零 `skills_uninstall` invoke。

## 根因

`src/styles/globals.css` `@keyframes fadeIn` 终态 `transform: translateY(0)`。`translateY(0)` 仍是有效 transform 值（≠ `none`），配合 `.animate-fade-in { animation: fadeIn ... both }`（`both` 保留终态），使 App.tsx 包裹页面的 `<div className="animate-fade-in" key={effectiveNav}>` 持续持有 transform。

CSS 规范：transform 祖先创建 containing block，后代 `position: fixed` 退化为相对该祖先定位（表现如 absolute）。Skills 页的卸载 modal `position:fixed; inset:0` 相对 animate-fade-in div 而非 viewport，且被外层 `<main style={{overflow:auto}}>` 裁剪 → modal 渲染但不可见。

toggle 操作不弹 modal，故不受影响。`slideInLeft` 终态 `translateX(0)` 同理。

## 修复

`src/styles/globals.css`：
- `@keyframes fadeIn` 的 `to` 由 `transform: translateY(0)` 改为 `transform: none`。
- `@keyframes slideInLeft` 的 `to` 由 `transform: translateX(0)` 改为 `transform: none`。

动画视觉无变化（位移回 0 = none），但终态不再持有 transform → 不创建 containing block → modal `position:fixed` 恢复相对 viewport。

## 验证

- `yarn build`（tsc + vite）通过。
- 用户复现：Skills 页点「卸载」→ modal 正常弹出 → 确认 → 后端 `skills_uninstall` invoke → skill 从列表移除。

## 不做

- 不改 modal JSX / z-index / Portal（根因在 CSS 动画终态）。
- 不改 Skills.tsx 逻辑。
