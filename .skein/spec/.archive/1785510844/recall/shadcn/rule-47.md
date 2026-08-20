---
title: dnd-kit SortableList 迁移保留拖拽逻辑
layer: recall
category: shadcn
keywords: [dnd-kit,SortableList,拖拽,迁移,shadcn,Button]
source: -
authored-by: skein-spec
created: 1784730633
status: active
related: []
updated: 1784730633
---

## 触发场景
dnd-kit SortableList 组件迁移时，只需替换内部 button/视觉组件，拖拽逻辑保持不变。

## 陷阱-正解
❌ **陷阱**：重写整个拖拽逻辑，破坏已有行为。
✅ **正解**：保留 dnd-kit 的 useSortable/Sensors 逻辑，仅替换 `<button>` → `<Button>`、样式 → shadcn 风格。

## 模式模板
```tsx
// 保留：拖拽逻辑
const { attributes, listeners, setNodeRef, transform } = useSortable({ id });
const style = transform ? { transform: CSS.Transform.toString(transform) } : undefined;

// 替换：button → Button + 样式
<React.Fragment ref={setNodeRef} style={style} {...attributes}>
  <div {...listeners}>
    <Button variant="ghost" size="icon">
      <svg>...</svg> {/* drag handle */}
    </Button>
  </div>
</React.Fragment>
```

## 适用
- dnd-kit SortableList 迁移至 shadcn
- 保留拖拽逻辑仅换视觉的场景

## 关联
[[rule-41]]

## 案例
- shadcn-pages task：Groups/GroupListItem SortableList 迁移，保留拖拽仅换 Button
