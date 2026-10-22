# 前端组件重构：就近测试按文件名找，选择器锚稳定 id 不锚 DOM

2026-10-22 WindowsEditModal 重构（bef68fe6b）沉淀。重构渲染层后测试红，根因两层：
主会话 grep 测试文件时 pattern 没匹配到 `src/pages/platforms/WindowsEditModal.test.tsx`，
门禁 agent 才发现；测试选择器锚在旧 DOM 结构上，重设计渲染层就断。
同轮 formSections 重构（30e5caffd）因改的是纯逻辑 + 新增逻辑级测试，一次过全绿。

## 规则

1. **改组件渲染层前，按文件名定位就近测试，不靠 grep pattern**：测试与组件同目录同基名
   （`WindowsEditModal.tsx` ↔ `WindowsEditModal.test.tsx`），直接
   `ls <组件目录>/<组件名>.test.tsx` 核存在。grep 零命中只说明 pattern 不匹配，
   不说明测试不存在（同模式：registry-data-edit.md「验证输出先看格式再 grep」）。
2. **重构前把就近测试的每个选择器过一遍**：测试锚了什么（稳定 id / data-state /
   getByText 文案 / class 名 / DOM 层级），渲染层重设计后哪些会断——断的选择器
   与组件改动属同一任务文件集，同一个 commit 改掉，不等门禁红了再补。
3. **测试选择器禁锚 DOM 层级和 class 名**，锚稳定 id + `data-state`
   （WindowsEditModal.tsx:347-350 的 `dim-${widx}-${opt.key}` + data-state 即此轮重建的先例）；
   也不用 `getByText(文案)` 断言（另一 agent 改 locale 值会假红，见
   WindowsEditModal.test.tsx:2 头注释）。新测试照此写。
