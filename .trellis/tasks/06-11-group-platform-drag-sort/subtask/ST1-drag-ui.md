# ST1: editPlatformIds 拖拽重排 UI

- **目标**: group 关联平台列表可拖动排序
- **产出** (Groups.tsx :339 区 selected platforms 列表):
  - HTML5 native drag（draggable + onDragStart 记 dragIndex / onDragOver preventDefault / onDrop 重排）→ setEditPlatformIds 移动项
  - 拖拽视觉：dragging 半透明、drag-over 高亮/插入线；拖拽手柄 ⠿ 或整行
  - 无第三方库（native HTML5 DnD）
  - save 不改（:194 已按顺序设 priority）
- **验证**: tsc 0；拖拽重排数组
- **资源**: design.md、Groups.tsx:339(列表)/:194(save priority)
- **依赖**: 无
- **失败处理**: 禁 any；别窗口改 Groups.tsx 冲突仅改列表区
