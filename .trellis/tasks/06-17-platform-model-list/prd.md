# 平台内置模型列表供下拉选择

## Goal

平台模型槽位选择支持「内置候选模型列表」下拉：用户未刷新模型时，下拉展示该平台内置的候选模型列表（而非只有一个默认填充值）；刷新（fetchModels）成功后改用接口返回的 available_models。供用户下拉选择，仍可自由输入自定义 model id。

## What I already know

- 模型槽位编辑器（`src/pages/Platforms.tsx:2412` MODEL_SLOTS map）**已有可搜索下拉**，但仅当 `availableModels.length > 0`（fetchModels 成功）才渲染 ▾ 按钮 + 列表（`:2414`、`:2441`）；未刷新时退化为纯文本 input + 单个 `getDefaultModels` 值。
- 数据：`models: Record<ModelSlot,string>`（单值/槽）、`available_models: string[]`（fetchModels 拉取，`api.ts:380`）。fetchModels → `autoCategorize`（`:599`）回填槽位。
- `getDefaultModels(protocol, cp)`（`:372`）现仅返回单值/槽，覆盖 ~10 平台。
- 优先级现状：`:1167` available_models>0 用 explicit；`:1992` defaultModel = models.default || available_models[0]。

## Decision (ADR-lite)

- **Context**: 未刷新时模型槽位无候选下拉，只有单默认值。
- **Decision**: ①覆盖**全部 ~60 平台**内置候选列表；②值**逐平台 WebSearch 核官方**（聚合/中转平台给「常用代表模型」子集，并注明 fetchModels 为主源）；③优先级 available_models 非空→接口列表，空→内置列表；④下拉保留自由输入（combobox，仍可手输自定义 model id）。
- **Consequences**: 研究量大（~60 平台官方模型列表），按 3 组并行；列表月级腐化，以核查日期为准 + fetchModels 兜底。代码侧新增 `getDefaultModelList(protocol, cp)` 返回 string[]，编辑器 dropdownSource 改造。

## Requirements (evolving)

- [ ] 未刷新时模型槽位下拉展示内置候选列表（平台有内置列表时）。
- [ ] fetchModels 成功后下拉改用 available_models。
- [ ] 仍可手输自定义 model id。

## Acceptance Criteria (evolving)

- [ ] 选一个有内置列表的平台、未刷新 → 模型槽位 ▾ 下拉可见且列出候选。
- [ ] fetchModels 后 → 下拉源切换为接口列表。
- [ ] yarn build 绿。

## Out of Scope

- 后端 fetchModels 逻辑改动（已工作）。

## Technical Notes

- 触点：`src/pages/Platforms.tsx`（新增 getDefaultModelList + 编辑器 dropdownSource 改造 :2412-2458 区段）。
- 参考记忆：platform-default-model、aidog-add-platform-skill。
