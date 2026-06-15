# 模型筛选触发按钮字号过小（前 3 次改错位置）

## 根因（误诊纠正）
v1/v2/v3 (commit c771d86/bf6796c/9a43436) 全改 dropdown 内 FilterOption（已 15px），
但用户说的「筛选太低字看不到」指**触发按钮**（`SearchableFilter` 顶部显示选中项的 button）：
`style={{ fontSize: 12, ... ellipsis }}` → 12px 比 .input 全局 13 还小，长模型名还被 ellipsis 截断。

## 方案
`src/pages/Stats.tsx` SearchableFilter 触发按钮：
- `fontSize: 12` → **14**（与 dropdown item 15 协调，可读）
- 其余（width:100%、ellipsis、nowrap）保留——用户确认「不需要在一处展示全部」，截断 OK，靠 dropdown 搜索看全名

## 验证
- tsc 过。
- 视觉：三个筛选触发按钮字 14px 可读。

## 非目标
- 不再动 dropdown item（v3 已 15px）。
