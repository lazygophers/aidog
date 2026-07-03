# PRD — 分组筛选 item 增高

## 目标
FilterDropdown 分组（及平台/模型）下拉的 option item 垂直 padding 偏矮，用户反馈展示局促。增高垂直 padding 11→20px。

## 范围（单文件单组件）
- `src/components/shared/FilterDropdown.tsx` 的 `FilterOption` 组件 padding `"11px 14px"` → `"20px 14px"`（垂直增 11→20，水平 14 保留）
- item 高度 ~44px → ~62px

## 决策锁（用户 2026-07-02 AskUserQuestion）
- 选项 2「仅增高 item」+ padding 增到 20
- **不动** label 逻辑（仍 nowrap ellipsis）/ 不加富信息 / 不动 trigger button / 不动其他选项

## 验收
1. FilterOption padding 垂直 = 20px（`grep 'padding:.*20px' src/components/shared/FilterDropdown.tsx` 命中）
2. `yarn build` 绿
3. 纯样式调，行为零变更

## 非目标
- 不改水平 padding / fontSize / lineHeight
- 不改 FilterDropdown 本体（trigger + 浮层容器）
- 不改其他筛选组件
