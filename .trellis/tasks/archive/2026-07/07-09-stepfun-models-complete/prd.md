# 补全 stepfun(+stepfun_en) model_list+endpoints 全部官方信息

## Goal

阶跃星辰 StepFun。国内 stepfun（api.stepfun.com）+ 国际 stepfun_en（api.stepfun.ai）同源镜像，仅域名异。preset 现 model_list 2 项（step-3.7-flash + step-3.5-flash），research 核实官方文本主线在售 4 款（遗漏 step-3.5-flash-2603 Agent 优化版 + step-1o-turbo-vision 视觉理解，后者是已下线 step-1/step-2 系列的统一替代）。endpoints 2 端点（openai /v1 ✅ + anthropic /step_plan ✅，research 明确确认）。desc/source_urls 准确保留。改动：model_list 补 2 漏项 + models.default 补 default 档。

## Research References

- [`research/stepfun-models.md`](research/stepfun-models.md) — 文本主线 4 模型最终清单 + endpoints 双协议路径（含 step_plan 订阅制专用路径说明）+ 已 Deprecated 9 模型清理表 + 语音/图像非主线排除项 + 定价差异（国内人民币 / 国际美元 ~1:7）

## Requirements

### 1. endpoints（每协议块 2 端点，保持不动）

research line 50-71 明确确认路径正确。anthropic 端点用 `/step_plan`（Step Plan 订阅制专用，非 /v1），openai 端点用 `/v1`。两协议结构完全一致，仅域名异。

**stepfun（国内 .com）**：
```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.stepfun.com/step_plan", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.stepfun.com/v1", "client_type": "codex_tui"}
  ]
}
```

**stepfun_en（国际 .ai）**：域名替换 .com→.ai，结构完全一致。
```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.stepfun.ai/step_plan", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.stepfun.ai/v1", "client_type": "codex_tui"}
  ]
}
```

### 2. model_list.default（4 文本主线模型，两协议同清单）

research line 20-30 官方在售主线清单。补 2 漏项（step-3.5-flash-2603 Agent 优化 + step-1o-turbo-vision 视觉理解，step-1/step-2 系列统一替代）。

```json
"model_list": {
  "default": [
    "step-3.7-flash",
    "step-3.5-flash",
    "step-3.5-flash-2603",
    "step-1o-turbo-vision"
  ]
}
```

排序：旗舰多模态推理 → 推理 → Agent 优化 → 视觉理解。两协议同清单（research line 30-31 确认双方模型无差异）。已下线 9 模型（step-1-8k/step-1-32k/step-1v-8k/step-1v-32k/step-2-mini/step-1o-vision-32k/step-2-16k/step-3/step-1x-medium）preset 未含，无需清理。

### 3. models.default（1 档：default，两协议同）

```json
"models": {
  "default": {
    "default": "step-3.7-flash"
  }
}
```

`default` 档 = step-3.7-flash（官方「推荐使用」标记，旗舰多模态推理，原生支持图片/视频理解，256K 上下文，三档推理强度 low/medium/high，Step Plan 订阅制默认支持，是已下线 step-3 的官方替代）。其余 slot（sonnet/opus/haiku/gpt/fable/coder/fast/thinking）均不适用（非 Claude/GPT/带明确 slot 语义的命名；flash 后缀虽匹配 fast slot，但 step-3.7-flash 是旗舰推理非轻量 fast 档，语义不符，仅填 default）。

### 4. desc（保留，准确）

现有 desc 准确描述平台定位（「阶跃星辰 API, Step 系列模型」），保留不改。两协议各保留现有 8 语言。

### 5. source_urls（保留，正确）

research 确认 platform.stepfun.com/docs + platform.stepfun.ai/docs 正确。pricing 路径 preset 用 `/docs/product/price`，research 调研用 `/docs/zh/guides/pricing/details`（同站不同路径指向同一定价页），preset 路径可接受，保留。

## Acceptance Criteria

- [ ] stepfun + stepfun_en 各 endpoints 2 端点保留（openai /v1 + anthropic /step_plan）
- [ ] model_list.default 各 4 模型（补 step-3.5-flash-2603 + step-1o-turbo-vision，两协议同清单）
- [ ] models.default 各 1 档 = {"default": "step-3.7-flash"}
- [ ] desc 保留
- [ ] source_urls 保留
- [ ] JSON 合法
- [ ] 仅改 stepfun + stepfun_en 协议块

## Out of Scope

- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan 分支
- 语音 TTS/ASR 模型（stepaudio/step-tts/step-asr 系列，非文本推理）
- 图像模型（step-2x-large/step-image-edit-2/step-1x-edit，文生图/编辑）
- 智能路由（step-router-v1，非对话模型）
- 已下线 9 模型清理（preset 未含，无需操作）
- pricing 字段补全（独立 task）
- 其他协议块改动

## Technical Notes

- 真值源：`protocols.stepfun` + `protocols.stepfun_en`（同源镜像，仅域名 .com vs .ai）
- 数据来源：research/stepfun-models.md（platform.stepfun.com/docs 官方文档 + Models List API 参考 + 模型迁移公告）
- id 格式：裸 id（无 provider/ 前缀），如 `step-3.7-flash`
- step_plan 路径特殊性：Anthropic 兼容端点用 `/step_plan`（Step Plan 订阅制专用，非 /v1），按量付费 openai 端点用 `/v1`
- step-3.5-flash-2603：Step Plan 专供但 API 可调用，research line 142-145 建议并入 model_list
- step-1o-turbo-vision：视觉理解模型，是 step-1/step-2 系列下线后的统一替代（research line 122-130 迁移表），属主线
- 定价差异：国内人民币 / 国际美元 ~1:7，计费结构相同，不影响 preset
- stepfun_en 完全复用 stepfun 结论：endpoints 域名替换 .com→.ai，model_list + models.default 完全相同
