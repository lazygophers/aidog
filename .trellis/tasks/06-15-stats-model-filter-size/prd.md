# 使用统计：模型下拉筛选 item 过窄不可读

## 背景
Stats 页带搜索的下拉筛选（`SearchableFilter` + `FilterOption`，`src/pages/Stats.tsx:618`）：
- 模型名通常很长（如 `claude-3-5-sonnet-20241022`、`deepseek-chat`），item 行字号 12px、padding `6px 8px`，密集且字小。
- dropdown 宽度 `Math.max(width, 180)`；模型筛选传入 `width=170` → 实际 180px，长模型名被 ellipsis 截断到看不全。

## 目标
让模型（及分组/平台）筛选下拉的每一项足够大、可读，长模型名不被截断。

## 方案（单一交付，main 直接改）
1. `FilterOption`：
   - `fontSize: 12` → `13`
   - `padding: "6px 8px"` → `"8px 10px"`
2. `SearchableFilter` dropdown 容器：
   - 最小宽度 `Math.max(width, 180)` → `Math.max(width, 240)`（足以容纳常见长模型名）
3. 不动触发按钮内联样式（保持筛选条紧凑），仅放宽弹出面板。

## 验证
- `yarn build`（tsc + vite）通过，无新 warning。
- 视觉：模型列表 item 字号 13、行距宽松、长名完整显示。

## 非目标
- 不改触发按钮尺寸 / 不重构组件结构 / 不动 i18n。
