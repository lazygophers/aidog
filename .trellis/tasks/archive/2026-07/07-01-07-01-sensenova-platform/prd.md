# PRD — 商汤 SenseNova 日日新平台支持

> research: `.trellis/tasks/07-01-07-01-sensenova-platform/research/sensenova-api-spec.md`
> 加平台走 aidog-add-platform skill（exec 时 subagent 加载 [[aidog-add-platform-skill]]）

## 目标
加商汤 SenseNova（日日新）平台预设，token plan 公测全免费。

## 接入映射（research 5 维全清）
1. **Protocol**：复用现有 OpenAI + Anthropic Protocol（**不新增枚举变体**），无需新 adapter
2. **base_url**：`https://token.sensenova.cn`
   - OpenAI 端点：`https://token.sensenova.cn/v1`（+ /chat/completions）
   - Anthropic 端点：`https://token.sensenova.cn`（**裸 host**，SDK 自动加 /v1/messages；配 /v1 会 /v1/v1/messages 404）
   - ⚠️ anthropic base_url 裸 host 与 glm/kimi 的 host+/anthropic 模式不同，配错 404
3. **鉴权**：`Authorization: Bearer sk-xxx`（sk- 前缀，每账户 20 key，不过期）
4. **模型**（3 个）：
   - `sensenova-6.7-flash-lite`（chat + 图像理解，256K）
   - `sensenova-u1-fast`（仅图像生成）
   - `deepseek-v4-flash`（thinking mode + 1M，推理模型）
   - Token Plan 公测全免费，每模型 1500/500 次/5h
5. **Token Plan 配额接口**：**无 API-Key 配额接口**（[[xiaomi-mimo-token-plan-no-api]] 同型，token.sensenova.cn 只暴露 4 LLM 端点，usage/quota/balance 全 404）→ quota.rs **不加 case**，fallthrough unsupported

## 交付项
### D1 — 平台预设
- Platforms.tsx / 模型预设：加 sensenova preset（name + base_url + protocol + 默认模型）
- getDefaultEndpoints：加 **三端点**（用户决策：所有支持 codex 的平台都应支持 response 协议）：
  - openai 端点：`https://token.sensenova.cn/v1`（+ /chat/completions）
  - anthropic 端点：`https://token.sensenova.cn`（**裸 host**，SDK 自动加 /v1/messages）
  - **responses 端点**：`https://token.sensenova.cn/v1`（+ /responses，codex 用）
  - ⚠️ research 仅验证 openai chat + anthropic messages；**responses 端点需 implement 时主动探测**（curl /v1/responses → 401=真端点 vs 404=不支持）。若 404 则该端点移除或标 unsupported，preset 文档明示
- 默认模型：**三模型全默认**（sensenova-6.7-flash-lite chat + deepseek-v4-flash 推理 + sensenova-u1-fast 图像生成）

### D2 — 智能粘贴识别
- `src/utils/platformPaste.ts`：加 sensenova host 识别（`token.sensenova.cn` / `sensenova` / 商汤 / 日日新 关键词）
- 粘贴 base_url 自动识别 sensenova + 填预设

### D3 — 定价
- `deepseek-v4-flash`：复用 LiteLLM/本地 models.json 已有条目（不新增）
- sensenova-6.7-flash-lite / sensenova-u1-fast：公测免费（缺定价无影响，est_cost 走 default 回退）

### D4 — i18n + 文案
- 8 locale：平台预设 display name（商汤 SenseNova / SenseNova）+ 任何新 key
- 平台图标：**加 sensenova logo SVG**（src/assets/platforms/sensenova.svg）。用户确认加；若 implement 拿不到官方 logo，用首字母「商」/「S」占位 SVG（ponytail：免外部资源抓取）

## 验收
1. 平台预设出现在添加平台列表
2. 智能粘贴 `https://token.sensenova.cn/v1` 识别 sensenova + 填预设
3. openai 端点转发正常（chat completions + 流式）
4. anthropic 端点转发正常（裸 host，/v1/messages）
5. quota 显示「unsupported / 无配额接口」（不 crash，不假数据）
6. `yarn build` + `cargo build` + `cargo clippy` 绿

## 非目标
- 不新增 Protocol 枚举（复用 openai/anthropic）
- 不新 adapter（通用 converter 走）
- 不加 quota case（无 API 接口）
- 不实现 token plan 配额查询（无接口，控制台肉眼看）

## 风险
- anthropic base_url 裸 host 易配错 → 智能粘贴 + preset 文档明示
- 公测免费模型缺定价 → est_cost default 回退（[[pricing-resolve-single-source]]）
- 模型 id 月级腐化（STATIC_MODEL_IDS 同类风险）→ 模型表可配

## 待评审决策
- 是否同时加 openai + anthropic 两端点？（推荐：是，research 示双协议支持）
- 默认模型：sensenova-6.7-flash-lite（chat）+ deepseek-v4-flash（推理）？或仅 chat？
- 平台图标：加 sensenova logo SVG 还是占位？
