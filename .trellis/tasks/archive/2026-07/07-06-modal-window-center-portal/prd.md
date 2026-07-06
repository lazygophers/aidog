# 弹窗全部 createPortal 到 body 实现真窗口居中

## Goal

用户多次强调: 弹窗确认必须**应用窗口居中**, 不是 page/滚动容器居中。根因不是缺 `position: fixed` (多数 modal 已用), 而是祖先 DOM 含 `transform`/`filter`/`backdrop-filter`/`will-change`/liquidGlass 动画类时, CSS 规范让 `position: fixed` 退化为相对该祖先 → 弹窗只在 page 内居中, 滚动时偏移甚至超出视口。修法 = `createPortal(<node>, document.body)` 脱离 transform 祖先。

## What I already know

- 已沉淀项目 memory `modal-window-center-rule.md` (feedback type)
- 已正确 (有 portal, 参考实现): `SmartPasteModal.tsx` / `ShareModal.tsx` / `ModelTestPanel.tsx` / `Groups/GroupListView.tsx` / `Skills/SkillModals.tsx` / `domains/groups/GroupTestPanel.tsx`
- 历史 commit `0aeff95` (toast 修复) 首次沉淀此根因
- 全局 css 祖先 transform 来源: `src/styles/globals.css` 的 `glass-surface` (backdrop-filter) / `animate-fade-in` (animation 终态 transform) / liquid glass 主题类

## Requirements

修复 6 个违规 modal (position: fixed 但未 portal), 全部包 `createPortal(node, document.body)` + `import { createPortal } from "react-dom"`:

1. `src/components/settings/MitmConfig.tsx` — `showClearConfirm` overlay (行 ~575)
2. `src/components/settings/UnsavedChangesModal.tsx` — 全局未保存确认 (整返回 JSX)
3. `src/components/settings/editors/ImportDiff.tsx` — 导入 diff 弹窗 (行 ~326)
4. `src/components/settings/editors/StatusLineSection/SegmentEditModal.tsx` — segment 编辑 (行 ~44)
5. `src/components/settings/NotificationSettings.tsx` — 通知设置内弹窗 (行 ~453)
6. `src/components/UpdatePromptModal.tsx` — 更新提示 (注释自相矛盾, 写"不用 transform 终态"但没 portal, 仍受祖先 transform 影响)

## Implementation Pattern

每文件统一改法:

```tsx
// 顶部 import 追加 (若未有):
import { createPortal } from "react-dom";

// 组件 return 处:
// 之前: return (<div style={{position:"fixed",inset:0,...}}>...</div>);
// 之后:
return createPortal(
  <div style={{position:"fixed",inset:0,...}}>...</div>,
  document.body
);
```

注意:
- 不要改 overlay 的 style (保持 `position: fixed; inset: 0; alignItems/JustifyContent: center`)
- 只在外层包 `createPortal(..., document.body)`
- 若原 return 是 `{condition && (<div...>)}` 形式, 改为 `{condition && createPortal(<div...>, document.body)}`
- 保留所有现有 onClick/stopPropagation/zIndex/animation 行为

## Acceptance Criteria

- [ ] 6 文件全部 `createPortal(node, document.body)` 包裹
- [ ] 每文件 `import { createPortal } from "react-dom"`
- [ ] `yarn build` 通过 (tsc + vite)
- [ ] grep 验证: `for f in <6 文件>; do grep -c createPortal "$f"; done` 全 ≥1
- [ ] dev 实测: Settings > MITM tab > 点"清空"按钮, 弹窗在窗口几何中心 (非滚动内容中心)

## Definition of Done

- 6 文件改完 + build 绿
- 无新增 lint warning (unused import 等)
- 用户 dev 实测确认窗口居中

## Out of Scope

- 已正确的 modal (SmartPasteModal / ShareModal / ModelTestPanel / GroupListView / SkillModals / GroupTestPanel) — 不动
- CcSwitchImport / Sub2ApiImport (grep 显示无 position:fixed, 不是 modal overlay) — 不动
- 改全局 css (transform 根源) — 风险大, 不在本 task 范围
- popover / dropdown / toast — 不属 modal 确认, 不在本 task

## Technical Notes

- memory: `/Users/luoxin/.claude/projects/-Users-luoxin-persons-lyxamour-aidog/memory/modal-window-center-rule.md`
- 参考实现: `src/components/platforms/SmartPasteModal.tsx` (createPortal 用法)
- CSS 规范根因: https://developer.mozilla.org/en-US/docs/Web/CSS/position#fixed (祖先 transform/filter/will-change 让 fixed 元素相对该祖先)
