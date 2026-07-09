# 模型矩阵组件尺寸统一增大

## Goal

ModelsMatrixSection（模型配置 card）当前组件大小太小（fontSize 10-11 居多，padding 紧凑），用户多次反馈"太小太小太小"。需全组件增大 + 统一尺寸标准（同类元素同一字号，禁散布 10/11/12 三档）。

## What I already know

- 目标文件：`src/pages/platforms/ModelsMatrixSection.tsx`（07-09-platform-presets-overhaul 刚 merge 的矩阵 card）
- 当前尺寸散布（grep fontSize）：
  - 单元格 input: `fontSize: 11, padding: "4px 6px"`（renderCell）
  - 默认列头: `fontSize: 11`
  - 时段档列头按钮 + ↑↓× 操作按钮: `fontSize: 10`
  - slot 标签（左列）: `fontSize: 11`
  - section action（fillAll/fetchModels/导入/+添加）: `fontSize: 12`
  - 空态提示: `fontSize: 11`
  - 下拉项: `fontSize: 12`
- 同类元素跨多档（10/11/12），不统一

## Decisions

| 元素类 | 现状 | 新值 |
|---|---|---|
| 矩阵主体（单元格 input / 默认列头 / slot 标签 / 空态提示 / 下拉项） | 10-12 散 | **统一 13** |
| 操作按钮（↑↓× / 时段档列头 / section action） | 10-12 散 | **统一 12** |
| 单元格 input padding | 4px 6px | **6px 8px**（增高，配 13 字号） |

## Requirements

- R1: 全 fontSize 统一到 2 档：主体 13 / 操作按钮 12（禁第三档残留）
- R2: 单元格 input padding 增大（4px 6px → 6px 8px）
- R3: 单元格 input 内 ▾ 按钮同步增大（width/height 20→22，配 13 字号）
- R4: 下拉项 padding 同步增大（6px 10px → 8px 12px）
- R5: 视觉验证：矩阵行高 / 列头 / 单元格 / 按钮高度协调，无错位

## Acceptance

- [ ] `grep -n "fontSize:" src/pages/platforms/ModelsMatrixSection.tsx` 仅出现 12 和 13（+ 下拉项 fontWeight 不计）
- [ ] padding 增大（input 6px 8px / 下拉项 8px 12px）
- [ ] yarn build 0 错
- [ ] 视觉无错位（列头 / 单元格 / 按钮对齐）

## Out of Scope

- 其他 section（Endpoints/Group/Breaker/PeakHours 等）不动
- 列宽逻辑（flex 平均分割，上次 task 已定）
- 响应式断点

## Technical Notes

- 文件：`src/pages/platforms/ModelsMatrixSection.tsx`
- 前端无 lint，验证走 yarn build
- modal/portal 规则不涉及（本 task 纯尺寸）
