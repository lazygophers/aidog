# CopyButton menu 改 hover 弹出（click→hover）

## Goal

分组列表「复制启动命令」按钮（CopyButton menu 模式）当前需 **click 才弹**菜单（Claude/Codex 二选一复制）。用户要求改 **hover 即弹**（移开即收），提升发现性 + 减一次点击。

## 现状（CopyButton.tsx）

- `:40` `const [open, setOpen] = useState(false)` — menu 开关
- `:56-63` `handleCopy` — click 时 menu 模式 `setOpen(o => !o)` toggle
- `:112-113` `onMouseEnter/Leave` — 仅切 `hovered`（默认态/悬浮态文案视觉切换，**不触发 open**）
- `:130` menu 经 `createPortal` 挂 body（不在 button DOM 树内）
- `:146-159` menu 项 click → `runCopy` + `setOpen(false)`

## Root Cause（设计选择，非 bug）

menu 模式触发绑定在 `onClick`（`:111`），hover 仅视觉。改触发为 hover。

## 技术难点（关键）

menu 是 `createPortal(document.body)`（`:130, :162`），**不在 button DOM 树内**。鼠标从 button 移到 menu 时会先触发 button `mouseLeave` → 若直接 `setOpen(false)` 则 menu 瞬间消失，无法点。

**解法**：延迟关闭（hover menu 标准模式）：
- button `mouseLeave` → `setTimeout(close, 120ms)`（非立即）
- menu div `mouseEnter` → `clearTimeout`（进 menu 即取消关闭）
- menu div `mouseLeave` → `setTimeout(close, 120ms)`（离开 menu 也关）
- button `mouseEnter` → `clearTimeout` + `setOpen(true)`

用 `useRef<number>` 存 timer id，`useEffect` cleanup 清理。

## Requirements

1. menu 模式：hover button 即弹 menu（无需 click）
2. 鼠标 button ↔ menu 间移动不闪退（延迟关闭 + clearTimeout）
3. 离开 button 且未进 menu（120ms 内）→ 收 menu
4. menu 项 click → 复制 + 收 menu（保持现状）
5. click button → 兼容触屏/键盘：保留 toggle 不变（触屏无 hover，click 仍可用）—— 或改为无操作，**默认保留 toggle**（安全，不破坏触屏）
6. Esc 关 menu 保持（`:74-76`）
7. 非菜单模式（普通复制按钮，Home 页 / Logs 页用法）**不受影响** —— hover 改动仅 `isMenu` 分支

## Decision

- **触发**：`onMouseEnter`（button）→ `setOpen(true)` + 清 close timer；`onMouseLeave`（button）→ 启动 120ms close timer
- **menu 容器**：portal div 加 `onMouseEnter`（清 close timer）/ `onMouseLeave`（启动 close timer）
- **click**：保留现有 toggle（触屏兼容），不冲突（hover 已开则 click toggle 关，符合预期）
- **延迟值**：120ms（足够 button→menu 移动，不过分黏滞）
- **timer 存储**：`useRef<number | null>(null)`，组件卸载 / open 变化时清理

## Acceptance Criteria

- [ ] hover button → menu 弹出（无 click）
- [ ] 鼠标移到 menu 项 → menu 不消失
- [ ] 鼠标离开 button 且 120ms 内未进 menu → menu 收
- [ ] 鼠标离开 menu → menu 收（120ms 后）
- [ ] click menu 项 → 复制到剪贴板 + menu 收
- [ ] click button → toggle（触屏兼容，不报错）
- [ ] Esc → menu 收（保持现状）
- [ ] 非菜单模式 CopyButton（无 menu prop）行为不变
- [ ] `yarn build` 通过

## Out of Scope

- 不改 CopyButton 非 menu 模式（普通复制）
- 不改 GroupListItem / GroupEditPanel 调用点（CopyButton 接口不变）
- 不改 menu 定位逻辑（`:92-104` boundary flip 保持）
- 不改 i18n（文案不变）
- 不加动画过渡（CSS transition 另说，本 task 仅改触发机制）

## Technical Notes

- 唯一改动文件：`src/components/shared/CopyButton.tsx`
- 调用点（不改，验证不破）：`GroupListItem.tsx:146`、`GroupEditPanel.tsx:42-43`
- React hover menu 经典模式（portal + delay）：参考 radix-ui / Menu pattern
- `writeText` 走 Tauri clipboard（`:7, :50`），保持
