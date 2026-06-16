# Skills modal Portal 化修复定位

## 现象

上一修复（fadeIn 终态 transform:none）让 modal 可见，但用户反馈 modal 不在窗口中央 —— 因页面内容过长，modal 仍相对内容/滚动区定位而非 viewport，出现在内容顶部，用户需滚动才能看到。

## 根因

Skills 页 modal（`position:fixed; inset:0`）仍受祖先 containing block 影响（animate-fade-in 动画进行中 transform / 其他潜在 transform 祖先），fixed 退化为相对内容定位。页面长（已装 skills 多）→ modal 居中点落在内容中部而非可视窗口中部。

## 修复

`src/pages/Skills.tsx`：3 个 modal（confirmUninstall / uninstallTarget / alignOpen）用 `createPortal(<modal jsx>, document.body)` 渲染到 document.body，彻底脱离 Skills 页任何 transform 祖先，fixed 恢复 viewport 相对，无论页面多长 modal 始终居中窗口。

- import `{ createPortal } from "react-dom"`。
- 每个 modal 的最外层 overlay `<div>` 包 createPortal。
- 其余 JSX / 逻辑 / 样式不变。

## 验证

- `yarn build`（tsc + vite）通过。
- 用户复现：Skills 页内容很长时点「卸载」/「卸载全部」/「对齐配置」→ modal 始终居中可视窗口，无需滚动。

## 不做

- 不抽通用 Modal 组件（本次只修 Skills 3 modal；其他页 modal 未报问题，不扩散）。
- 不改 globals.css（上一修复保留）。
