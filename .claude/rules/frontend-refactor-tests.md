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

## 拖拽交互：Vite-only 测不到落点，列人工验收（2026-10-29 托盘拖拽）

托盘设置页依赖 Tauri IPC，Vite-only 不挂载（同 ui-color-bug-verify.md 回退节）。dnd-kit 拖拽
只能在 vitest 里 mock `DndContext` 直接喂 `onDragEnd`，落点判定（pointer 命中哪个槽）无覆盖。

- 测试断言 `onDragEnd` 之后的状态（如 `moveSegment` 用 arrayMove 而非 swap，2ffa12be4）。
- 回报与账本遗留单列人工验收：候选拖入槽、槽内手柄排序、拖回移除、键盘手柄
  （pr-merge-discipline.md §2 同口径）。

## 行为测试渲染真组件、断言走 data-testid；表单禁渲染期回落（2026-11-04）

来源 0a5c257b7、8558a5eee 两次复盘：测试通过但没测到真代码，表单回落让用户清不空字段。

1. **组件行为测试必须渲染真组件**，用 `data-testid` 取元素断言；禁在测试里重写一份被测逻辑
   再断言它（测的是测试自己，组件改坏照绿）。
2. **表单控件禁渲染期回落 `value={x || DEFAULT}`**：用户清空后立刻被默认值顶回，永远清不掉。
   默认值在加载 / 模式切换时一次性预填进 state，渲染只读 state。
3. code-review 派发时这两点都要查：测试是否渲染真组件、表单是否有渲染期回落。

## 同一组件两处调用，功能差距 = props 差异 + 挂载点差异（2026-11-06 分组平台对齐）

「A 页已经有 X，让 B 页也支持」类需求，排查从共用组件下手，不从两页各自读代码：

1. 先找共用组件与数据源（分组列表 vs 分组详情共用 PlatformRow + usePlatformCards），
   逐 props 对照——差距常只是某 prop 未传（列表页补 showLastOutcome 即对齐）。
2. 再对照 modal / 浮层的**挂载点**：提前 return 的分支会整段跳过后面的挂载
   （detailTarget 提前 return 使批量 modal 只在列表分支渲染），搬进共享段即修。
3. 反例：逐页读实现找差异——同一能力读两遍，还把「组件本来就有、只是没传」
   误判成「页面各自实现」，改动面随之翻倍。
