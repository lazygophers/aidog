# S1 — Groups.tsx group 列表卡片拖拽迁移 SortableList

**依赖**: 无

## 目标
`src/pages/Groups.tsx` 的 group 列表卡片排序从 pointer-event 自实现迁移到通用 SortableList。

## 产出
- 删：`groupDrag` state(96-97) + `groupListRef`/`groupDragStartRef`/`groupDidDragRef`(98-100) + `handleGroupPointerDown/Move/Up`(102-141) + 卡片上 `onPointerDown/Move/Up`(670-672) + `data-group-id` 命中检测 + drag ghost 视觉(636 区)
- 改：group 卡片列表用 `<SortableList<GroupRow> items={...} onReorder={...} renderItem={(row, handle)=>卡片} strategy="vertical">`
- id 包装：`String(detail.group.id)`（参照同文件已有 `SortablePlatform` 模式，已 import SortableList at line 8）
- onReorder：`(next) => { setDetails(next.map(r=>r.detail)); groupApi.reorder(next.map(r=>r.detail.group.id)).catch(console.error); }`
- 手柄：卡片含展开/编辑/映射快加等交互 —— 用专用 ☰ 拖拽手柄（handle.ref+attributes+listeners）或整卡 handle + isDragging 守卫，确保点击不被拖拽误触

## 工作目录与范围
- 工作根：`/Users/luoxin/persons/lyxamour/aidog/.trellis/worktrees/06-11-group-card-dnd`
- 只改 `src/pages/Groups.tsx`

## 验收
- `cd <worktree根> && yarn tsc --noEmit` exit 0
- `grep -nE 'onPointerDown|onPointerMove|onPointerUp|groupDrag|data-group-id' src/pages/Groups.tsx` 清零
- `grep -c 'SortableList' src/pages/Groups.tsx` ≥ 2
- platform 排序（已有 SortableList）不受影响
- 遵守 conventions.md（禁新增 any / inline+glass / catch console.error）

## 资源
- SortableList API：`src/components/SortableList.tsx`（items={T[] 含 id} / onReorder(next) / renderItem(item, handle{ref,attributes,listeners,isDragging}) / strategy）
- 同文件 platform 迁移参照：`SortablePlatform` 类型 + `handleReorderPlatforms`(145) + 渲染 `<SortableList<SortablePlatform>>`(401)
- 持久化：`groupApi.reorder(ids)` 现有契约不变
- spec：frontend/conventions.md、code-reuse-rules.md、cross-layer-rules.md

## 失败处理
- 卡片点击误触拖拽 → 用专用 ☰ 手柄而非整卡
- details 元素结构复杂 → GroupRow wrapper {id, detail}
- 缺信息 → 输出 `需要: <问题>`，禁碰其他文件
