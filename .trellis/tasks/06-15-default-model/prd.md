# PRD — child A: 平台预设默认模型功能

> parent: `06-15-aidog-add-platform-skill`

## 问题

aidog 当前**无「平台预设默认模型」机制**（research/01 §平台默认模型，行 106-125）：
- `getDefaultEndpoints`（Platforms.tsx:150-360）只给 endpoints，不给 models
- 表单初始 models 全空：Platforms.tsx:1534-1535 / 1717 `{ default:"", sonnet:"", opus:"", haiku:"", gpt:"" }`
- 后果：加平台后 5 槽全空，用户须手填或点「拉取模型」走 `autoCategorize`（:566）正则归类

用户要：**① 表单选平台时预填默认模型；② 无 available_models / 无手填时，列表展示该平台默认模型。**

## 方案（纯前端预设，与 base_url 预设同模式）

base_url 预设住前端（research/01 行 9-10、89-102），默认模型同理 → 纯前端实现，**不改 Rust**（PlatformModels 结构 Rust+TS 已存在，仅填充预设值，走现有 CreatePlatform.models 落库链路）。

### 改动点

| # | 文件 | 改动 |
|---|---|---|
| 1 | `src/pages/Platforms.tsx`（getDefaultEndpoints 同址） | 新增 `getDefaultModels(protocol, codingPlan): Partial<PlatformModels>` — 平台 → 预设默认模型槽位 |
| 2 | `src/pages/Platforms.tsx:1534-1535 / 1717` | 表单选 protocol 时，models 初值从 `getDefaultModels` 取（替代硬编码空对象）|
| 3 | `src/pages/Platforms.tsx:523-533 / 1128`（allModelValues 展示链路）| 无配置模型时回退展示 `getDefaultModels` 预设值（确认展示语义）|

### 默认模型数据范围

建机制 + 填**主流平台**（其余留空，向后兼容）。初版覆盖：
- `openai` → gpt 槽 `gpt-4o`（待定，brainstorm 确认）
- `anthropic` → opus/sonnet/haiku 三槽对应 claude 主力
- `glm` → `glm-4.6`
- `kimi` → `kimi-k2`（coding plan 变体待确认）
- `deepseek` → `deepseek-chat`
- `minimax` / `通义千问` / 其余主流：按各家当前主力模型

> ⚠️ 具体模型名须查各平台当前真实主力（避免填过时模型）。brainstorm/实现期逐个核对，禁编造。

## 验收标准

- `yarn build`（tsc && vite build）零报错
- 选 glm/kimi/deepseek/openai/anthropic 等主流平台 → 表单 models 槽位自动预填对应默认模型
- 未覆盖的平台 → models 仍空（向后兼容，不报错）
- 若涉及列表展示回退：无 available_models 平台展示预设默认模型
- 跨层契约：若动到 PlatformModels 字段则 Rust↔TS 同步（本方案预计不动结构，仅填值）

## 失败处理

- 若纯前端无法满足「列表展示默认」语义（需后端回填）→ 升级为跨层改动，补 Rust 侧默认；先在实现期确认 allModelValues 展示链路是否够用
- 模型名拿不准 → 标「待确认」不编造，brainstorm 期问用户/查官方

## 依赖 / 资源

- research/01-platform-model-presets.md（§平台默认模型 + PlatformModels 结构 + 展示链路 file:line）
- 无前置 child 依赖，可与 child B 主体并行
