# Research: PackyCode 全量研究

- **Query**: PackyCode (packyapi.com) 全量模型清单 + endpoints
- **Scope**: external
- **Date**: 2026-07-09

## 关键结论（TL;DR）

- **PackyCode 是多供应商聚合平台，非仅 Claude 兼容**。官方定价 API 返回 **53 个模型 / 12 个供应商**：Anthropic / OpenAI / Google / DeepSeek / 阿里 / 智谱 / Moonshot / MiniMax / Xiaomi MiMo / Hunyuan。
- 当前 preset `desc` "Claude 兼容模型" + 仅 7 个 claude 模型 **严重不完整**。
- 平台采用 **token group（令牌分组）机制**：单个 API key 只能访问其所属 group 包含的模型；`auto_groups: ["cc"]` 表示默认注册用户落到 `cc` 组（仅 Claude Code 系列）。要访问 GPT/Gemini/国产系，用户需在控制台切换或新建对应 group（`codex` / `bailian` / `gemini-officially` 等）。
- **官方权威数据源**：`GET https://www.packyapi.com/api/pricing` —— 返回全量 JSON（无鉴权可读），含 `data[]`(模型) / `vendors[]`(供应商) / `supported_endpoint`(路径) / `usable_group`(分组释义)。preset 补全应以该 API 实时返回为准。

## API Endpoints

来源：`/api/pricing` 返回的 `supported_endpoint` 字段（最权威）+ docs VuePress 配置页交叉验证。

| 协议 | method | path | base_url（推测默认） | 适用 |
|---|---|---|---|---|
| anthropic | POST | `/v1/messages` | `https://www.packyapi.com` | Claude 系 + 所有标 `anthropic` 的模型 |
| openai | POST | `/v1/chat/completions` | `https://www.packyapi.com/v1` | OpenAI 系 + 多数国产（带 `openai` 标） |
| openai-response | POST | `/v1/responses` | `https://www.packyapi.com/v1` | Codex / GPT-5.x（新 response API） |
| gemini | POST | `/v1beta/models/{model}:generateContent` | `https://www.packyapi.com` | Gemini 系 |

**现有 preset 三 endpoint 核对**：

| 现有 preset | 实际 | 结论 |
|---|---|---|
| anthropic `https://www.packyapi.com` (claude_code) | base 正确，path 自动拼 `/v1/messages` | ✅ |
| openai `https://www.packyapi.com/v1` (codex_tui) | base 正确，path 自动拼 `/chat/completions`（codex 实际走 `/responses`，aidog `provider_api_path` 按协议固定） | ✅（codex_tui 用 openai-response 走 `/v1/responses`，base 同） |
| gemini `https://www.packyapi.com` (default) | base 正确，path 自动拼 `/v1beta/models/{model}:generateContent` | ✅ |

> 三个 endpoint 全部正确，无需改。

## 全量模型清单（53 个，按供应商分组）

数据源 `https://www.packyapi.com/api/pricing`（访问 2026-07-09）。每行格式：`model_id` — ratio（输入倍率，1=官价）/ completion（输出对输入倍率）/ 支持协议 / 可用分组。

> **model id 全部为裸 id（无 `provider/` 前缀）**，与现有 preset 7 个格式一致。

### Anthropic（vendor_id=1，10 个）

- `claude-fable-5` — ratio 5 / comp 5 / [anthropic,openai] / [claude-officially,claude-sale,cc]
- `claude-haiku-4-5-20251001` — ratio 0.5 / comp 5 / [anthropic,openai] / [claude-sale,cc,cc-expensive,aws-q,cc-sale]
- `claude-opus-4-1-20250805` — ratio 7.5 / comp 5 / [anthropic,openai] / [claude-sale,cc]
- `claude-opus-4-5-20251101` — ratio 2.5 / comp 5 / [anthropic,openai] / [cc-sale,claude-sale,cc,aws-q,claude-officially]
- `claude-opus-4-6` — ratio 2.5 / comp 5 / [anthropic,openai] / [claude-vt,aws-q,claude-officially,claude-sale,cc,cc-expensive,cc-sale]
- `claude-opus-4-7` — ratio 2.5 / comp 5 / [anthropic,openai] / [cc-sale,claude-sale,cc,cc-expensive,claude-vt,claude-officially,aws-q]
- `claude-opus-4-8` — ratio 2.5 / comp 5 / [anthropic,openai] / [cc-sale,claude-sale,cc,cc-expensive,claude-vt,claude-officially,aws-q]
- `claude-sonnet-4-5-20250929` — ratio 1.5 / comp 5 / [anthropic,openai] / [claude-sale,cc,claude-officially]
- `claude-sonnet-4-6` — ratio 1.5 / comp 5 / [anthropic,openai] / [aws-q,claude-vt,claude-officially,cc-sale,claude-sale,cc,cc-expensive]
- `claude-sonnet-5` — ratio 1 / comp 5 / [anthropic,openai] / [cc,claude-officially,cc-expensive,cc-sale,claude-vt,claude-sale]

### OpenAI（vendor_id=2，9 个）

- `codex-auto-review` — ratio 2.5 / comp 6 / [openai] / [cx-1,codex,hongjing,cx]
- `gpt-4.1` — ratio 1 / comp 2 / [openai] / [azure-officially]
- `gpt-5.3-codex` — ratio 0.875 / comp 8 / [openai] / [azure-officially]
- `gpt-5.4` — ratio 1.25 / comp 6 / [openai] / [codex,hongjing,cx,cx-1,azure-officially]
- `gpt-5.4-mini` — ratio 0.375 / comp 6 / [openai] / [hongjing,cx,cx-1,azure-officially,codex]
- `gpt-5.4-pro` — ratio 15 / comp 6 / [openai] / [azure-officially]
- `gpt-5.5` — ratio 2.5 / comp 6 / [openai] / [codex,hongjing,cx,cx-1]
- `gpt-image-2` — 图像 / ratio 0 / [openai] / [image,sora]
- `omni-moderation-latest` — 审核模型 / ratio 0 / [openai] / [default]

### Google（vendor_id=4，8 个）

- `gemini-2.5-flash` — ratio 0.15 / comp 8.33 / [gemini,openai] / [gemini-slb,gemini-officially]
- `gemini-2.5-flash-image` — 图像 / ratio 0 / [gemini,openai] / [gemini-officially]
- `gemini-2.5-pro` — ratio 0.625 / comp 8 / [gemini,openai] / [gemini-slb,gemini-officially]
- `gemini-3-flash-preview` — ratio 0.25 / comp 6 / [gemini,openai] / [gemini-slb,gemini-officially]
- `gemini-3-pro-image-preview` — 图像 / ratio 0 / [openai] / [image]
- `gemini-3-pro-preview` — ratio 1 / comp 6 / [gemini,openai] / [gemini-slb,gemini-officially]
- `gemini-3.1-flash-image-preview` — 图像 / ratio 0 / [gemini,openai] / [gemini-officially,image]
- `gemini-3.1-pro-preview` — ratio 1 / comp 6 / [gemini,openai] / [gemini-slb,gemini-officially]

### 阿里巴巴 / Qwen（vendor_id=8，9 个）

- `qwen3-coder-next` — ratio 0.5 / comp 4 / [openai,openai-response,anthropic] / [bailian]
- `qwen3-max` — ratio 1.25 / comp 4 / [openai,openai-response,anthropic] / [bailian]
- `qwen3-vl-flash` — VL 多模态 / ratio 0.075 / comp 10 / [openai,openai-response,anthropic] / [bailian]
- `qwen3.5-flash` — ratio 0.1 / comp 10 / [openai,openai-response,anthropic] / [bailian]
- `qwen3.5-plus` — ratio 0.4 / comp 6 / [openai,openai-response,anthropic] / [bailian]
- `qwen3.6-max-preview` — ratio 4.5 / comp 6 / [openai,openai-response,anthropic] / [bailian]
- `qwen3.6-plus` — ratio 1 / comp 6 / [openai,openai-response,anthropic] / [bailian]
- `qwen3.7-max` — ratio 6 / comp 3 / [openai,openai-response,anthropic] / [bailian]
- `qwen3.7-plus` — ratio 1 / comp 4 / [openai,openai-response,anthropic] / [bailian]

### 智谱 GLM（vendor_id=6，3 个）

- `glm-4.7` — ratio 2 / comp 4 / [openai,anthropic] / [zai-officially]
- `glm-5` — ratio 2 / comp 4.5 / [openai,openai-response,anthropic] / [bailian,zai-officially]
- `glm-5.2` — ratio 4 / comp 3.5 / [openai,anthropic] / [zai-officially,glm-sale,test]

### Moonshot / Kimi（vendor_id=7，3 个）

- `kimi-k2.5` — ratio 2 / comp 5.25 / [openai,openai-response,anthropic] / [bailian,kimi-officially]
- `kimi-k2.6` — ratio 3.25 / comp 4.15 / [anthropic,openai] / [kimi-officially]
- `kimi-k2.7-code` — ratio 3.25 / comp 4.15 / [anthropic,openai] / [kimi-officially]

### MiniMax（vendor_id=43，3 个）

- `MiniMax-M2.7` — ratio 1.05 / comp 4 / [openai,openai-response,anthropic] / [bailian]
- `MiniMax-M3` — ratio 2.1 / comp 4 / [anthropic,openai] / [minimax-officially]
- `minimax-m2.5` — ratio 1.05 / comp 4 / [openai,openai-response,anthropic] / [bailian]

### Xiaomi MiMo（vendor_id=44，5 个）

- `mimo-v2-flash` — ratio 0.35 / comp 3 / [openai,openai-response,anthropic] / [mimo-officially]
- `mimo-v2-omni` — 多模态 / ratio 1.4 / comp 5 / [openai,openai-response,anthropic] / [mimo-officially]
- `mimo-v2-pro` — ratio 3.5 / comp 3 / [openai,openai-response,anthropic] / [mimo-officially]
- `mimo-v2.5` — ratio 0.5 / comp 2 / [openai,openai-response,anthropic] / [mimo-officially]
- `mimo-v2.5-pro` — ratio 1.5 / comp 2 / [openai,openai-response,anthropic] / [mimo-officially]

### DeepSeek（vendor_id=42，2 个）

- `deepseek-v4-flash` — ratio 0.5 / comp 2 / [anthropic,openai] / [deepseek-officially]
- `deepseek-v4-pro` — ratio 6 / comp 2 / [openai,anthropic] / [deepseek-officially]

### Hunyuan 腾讯混元（vendor_id=45，1 个）

- `hy3` — ratio 0.5 / comp 4 / [openai,anthropic] / [hunyuan-officially]

> 文档另列 `Meta`（vendor_id=5）但当前 `data[]` 中无模型，仅注册了供应商图标。推测: 预留位，未实际开放。

## 三档默认推荐（models.default）

参考其他多协议平台 preset 模式（default 分支按客户端类型选模型）。PackyCode 默认 token group = `cc`（仅 Claude），但用户切换 group 后可访问全部模型，故 `models.default` 应覆盖主流三档：

```jsonc
"models": {
  "default": {
    "claude_code": "claude-sonnet-4-6",   // Claude 系主力，ratio 1.5，cc 组可用
    "codex_tui": "gpt-5.4",                // OpenAI 系 codex/general，codex/azure 组可用
    "default": "claude-sonnet-4-6"         // gemini 端点（claude 兼容回退）
  }
}
```

备选推荐（按性价比）：
- Claude 入门档：`claude-haiku-4-5-20251001`（ratio 0.5，最便宜 Claude）
- Claude 旗舰：`claude-opus-4-8`（ratio 2.5）
- OpenAI codex 档：`gpt-5.3-codex`（ratio 0.875，codex 性价比）/ `gpt-5.4`（主力）
- 国产档（bailian 组）：`qwen3-coder-next`（ratio 0.5）/ `qwen3.5-flash`（ratio 0.1，最便宜）
- 国产档（zai）：`glm-5.2`（最新，但 ratio 4）/ `glm-4.7`（ratio 2）
- Gemini 档：`gemini-3-pro-preview`（ratio 1）/ `gemini-2.5-flash`（ratio 0.15，性价比）

## model_list 推荐补全策略

**核心建议**：由于 token group 机制，单个 key 实际可用模型取决于 group。`model_list.default` 应至少覆盖**所有 group 通用的「cc 组 Claude 全部」**，再补全主流多供应商模型（用户切组后可用）。**禁**照搬 53 个全塞入（含图像/审核/preview）。

推荐 `model_list.default` 至少包含（27 个核心文本对话/coding 模型）：

```
# Claude（10 全量，cc 默认组全可用）
claude-opus-4-8 / claude-opus-4-7 / claude-opus-4-6 / claude-opus-4-5-20251101 / claude-opus-4-1-20250805
claude-sonnet-5 / claude-sonnet-4-6 / claude-sonnet-4-5-20250929
claude-haiku-4-5-20251001 / claude-fable-5
# OpenAI（5）
gpt-5.5 / gpt-5.4 / gpt-5.4-mini / gpt-5.4-pro / gpt-5.3-codex / gpt-4.1 / codex-auto-review
# Google（4 主流，排除 image/preview）
gemini-3.1-pro-preview / gemini-3-pro-preview / gemini-2.5-pro / gemini-2.5-flash
# 国产（按需，bailian/zai/kimi 等组）
qwen3.7-max / qwen3-coder-next / qwen3.5-flash
glm-5.2 / glm-5 / glm-4.7
kimi-k2.7-code / kimi-k2.6 / kimi-k2.5
MiniMax-M3 / minimax-m2.5
deepseek-v4-pro / deepseek-v4-flash
```

排除项（不进 model_list）：
- 图像/审核专用：`gpt-image-2` / `gemini-2.5-flash-image` / `gemini-3-pro-image-preview` / `gemini-3.1-flash-image-preview` / `omni-moderation-latest`
- 重名同义：`MiniMax-M2.7` 与 `minimax-m2.5`（不同分组，按需保留）
- 实验性：`mimo-v2-omni`（除非用户明确要小米）

## 现有 7 模型核对

| 现有 preset id | 官方实际 id | 状态 |
|---|---|---|
| `claude-opus-4-8` | `claude-opus-4-8` | ✅ 完全匹配 |
| `claude-sonnet-4-6` | `claude-sonnet-4-6` | ✅ 完全匹配 |
| `claude-haiku-4-5` | `claude-haiku-4-5-20251001` | ⚠️ 缺日期后缀，API 可能不识别（推测: 短 id 或被服务端模糊匹配，但官方精确 id 是带日期版） |
| `claude-opus-4-7` | `claude-opus-4-7` | ✅ 完全匹配 |
| `claude-opus-4-6` | `claude-opus-4-6` | ✅ 完全匹配 |
| `claude-opus-4-5` | `claude-opus-4-5-20251101` | ⚠️ 缺日期后缀 `-20251101` |
| `claude-sonnet-4-5` | `claude-sonnet-4-5-20250929` | ⚠️ 缺日期后缀 `-20250929` |

**结论**：3 个 id（haiku-4-5 / opus-4-5 / sonnet-4-5）缺日期后缀，应改为官方精确 id。`cc` 组内还遗漏 3 个 Claude 模型（`claude-fable-5` / `claude-opus-4-1-20250805` / `claude-sonnet-5`）。

## 重大缺失 / 模型范围结论

**PackyCode ≠ Claude-only**。它是 12 供应商聚合（Anthropic/OpenAI/Google/DeepSeek/Qwen/GLM/Kimi/MiniMax/MiMo/Hunyuan），覆盖：
- Claude Code（cc 组，默认）
- Codex / GPT-5.x（codex/cx/azure-officially 组）
- Gemini（gemini-officially/slb 组）
- 阿里百炼全家桶（bailian 组）
- 各国产官方渠道（zai/kimi-officially/minimax-officially 等）

现有 preset `desc` "Claude 兼容模型" 误导性强，建议改为 "多供应商 AI API 聚合（Claude/GPT/Gemini/Qwen/GLM/Kimi 等）"。

## token group 机制要点（影响 preset 设计）

- 单 API key → 单 group → 仅该 group 列出的模型可调
- `auto_groups: ["cc"]`：新注册默认 cc 组（Claude Code 专用，10 个 Claude 模型）
- 用户需在控制台「模型广场」为 key 选/换 group，或一个账号多 key 对应多 group
- `group_ratio` 字段记录每组折扣倍率（cc 组通常 1.0 官价）
- **意味着**：preset 的 model_list 不能假设所有用户都能调全部 53 模型；只能作为「平台支持的全集」，实际可用性取决于用户配置的 group

## Caveats / Not Found

- **未实测 API 调用**：所有 model id 来自 `/api/pricing` 返回（官方控制台数据源，可信度高），但未发实际 `/v1/messages` 请求验证每个 id 可达性
- **上下文窗口**：`/api/pricing` 不返回 context window 字段；docs.packyapi.com 亦未列出统一规格表，需要: 用户自行以 `model_name` 对照官方供应商文档（如 claude-opus-4-8 在 Anthropic 官方 200K，gemini-3-pro 在 Google 官方 1M）
- **Meta 供应商**：vendors 列表含 `Meta`(id=5) 但无对应模型，推测: 预留未开放
- **图像/审核模型**：`gpt-image-2` / `omni-moderation-latest` / `*-image*` 等非对话模型，不适合 aidog 路由，model_list 应排除
- **模型版本时效**：preview/next 后缀模型（`gemini-3-flash-preview` / `qwen3-coder-next` / `qwen3.6-max-preview`）可能随时下线，preset 跟进需定期重取 `/api/pricing`
- **价格倍率变动**：所有 ratio 数字为 2026-07-09 快照，packycode 可能随时调整

## 数据来源

- `GET https://www.packyapi.com/api/pricing`（2026-07-09，53 模型 + 12 供应商 + 4 endpoint 路径 + 26 token groups）— 主数据源
- `https://docs.packyapi.com/` VuePress 文档（accessed 2026-07-09）— endpoint 路径交叉验证
  - `/docs/cli/2-claude.html` Claude Code 配置（确认 anthropic base `https://www.packyapi.com`）
  - `/docs/cli/3-codex.html` Codex 配置（确认 openai base `https://www.packyapi.com/v1`）
  - `/docs/ccswitch/4-gemini.html` Gemini 配置
  - `/docs/token/1-intro.html` 模型广场 / token group 机制说明
- `https://www.packyapi.com/pricing` SPA（控制台「模型广场」对应 `/api/pricing` 后端）
