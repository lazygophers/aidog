# 模型下拉筛选 item 仍过小（v1 改动力度不足）

## 背景
v1 (commit c771d86): FilterOption fontSize 12→13, padding 6px8px→8px10px, dropdown min-width 180→240。
用户反馈仍未解决，截图证实 item 仍小、长模型名仍看不全。

## 方案（加大力度）
`src/pages/Stats.tsx` SearchableFilter + FilterOption：
1. `FilterOption.fontSize` 13 → **14**
2. `FilterOption.padding` "8px 10px" → **"10px 12px"**
3. `FilterOption` 加 `lineHeight: 1.4`（行高宽松，不止字号）
4. dropdown 容器 `width: Math.max(width, 240)` → **Math.max(width, 320)**（覆盖 30+ 字符长模型名）
5. dropdown 容器 `padding: 6` → `8`，`gap: 4` → `6`
6. 搜索 input `fontSize: 12` → `13`
7. 列表 `gap: 1` → `2`

## 验证
- `npx tsc --noEmit` 过。
- 视觉：模型列表 item 字号 14、行距宽松、长名完整不截断。

## 非目标
- 不改触发按钮（保持筛选条紧凑）。
