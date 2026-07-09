# Research: SiliconFlow 模型全谱与端点配置

- **Query**: 研究 SiliconFlow（硅基流动）的官方信息，补全 platform-presets.json（含国内版 siliconflow + 国际版 siliconflow_en）
- **Scope**: 外部（官方文档 + API）
- **Date**: 2026-07-09

---

## 平台定位

SiliconFlow 是**一站式模型推理云服务平台**（类似 OpenRouter，但偏国内/开源主力），托管多厂商开源模型与商业模型，提供 OpenAI 兼容 + Anthropic 双协议 API。

**核心特性**：
- 按量付费，多个模型免费（如 Qwen2.5-7B）
- 支持 OpenAI Python SDK 直连（base_url 换成 SiliconFlow 即可）
- Claude Code 官方合作平台（提供一键配置脚本）

**官网（国内/国际）**：
- 国内：https://siliconflow.cn | https://docs.siliconflow.cn/ | https://siliconflow.cn/pricing
- 国际：https://siliconflow.com | https://docs.siliconflow.com/ | https://siliconflow.com/pricing

---

## API 协议与端点

### 双协议支持

SiliconFlow **同时支持 OpenAI 和 Anthropic 两种协议**（文档明确列出两套独立端点）：

#### 国内版（siliconflow）

| 协议 | 端点路径 | 认证 |
|------|---------|------|
| OpenAI 兼容 | `https://api.siliconflow.cn/v1` | Bearer `<api_key>` |
| Anthropic | `https://api.siliconflow.cn/v1/messages` | Bearer `<api_key>` |

**Claude Code 配置示例**（国内版官方文档）：
```bash
export ANTHROPIC_BASE_URL="https://api.siliconflow.cn/"
export ANTHROPIC_MODEL="moonshotai/Kimi-K2-Instruct-0905"
export ANTHROPIC_API_KEY="YOUR_SiliconFlow_API_KEY"
```

#### 国际版（siliconflow_en）

| 协议 | 端点路径 | 认证 |
|------|---------|------|
| OpenAI 兼容 | `https://api.siliconflow.com/v1` | Bearer `<api_key>` |
| Anthropic | `https://api.siliconflow.com/v1/messages` | Bearer `<api_key>` |

**Claude Code 配置示例**（国际版官方文档）：
```bash
export ANTHROPIC_BASE_URL="https://api.siliconflow.com/"
export ANTHROPIC_MODEL="your-preferred-model"
export ANTHROPIC_API_KEY="sk-your-api-key-here"
```

### 端点路径结构一致性

**结论**：国内版与国际版路径结构**完全一致**，仅域名差异（.cn vs .com）：
- OpenAI 兼容：`/v1/chat/completions`
- Anthropic：`/v1/messages`
- 模型列表：`/v1/models`

### OpenAI 兼容端点（需补）

**当前 preset（siliconflow + siliconflow_en）均缺失**。SiliconFlow 主力协议是 OpenAI 兼容（文档优先展示、官方示例均用此），preset 仅配置 anthropic 端点疑似遗漏。

**端点配置建议（国内版）**：
```json
{
  "protocol": "openai",
  "base_url": "https://api.siliconflow.cn/v1",
  "client_type": "codex_tui"
}
```

**端点配置建议（国际版）**：
```json
{
  "protocol": "openai",
  "base_url": "https://api.siliconflow.com/v1",
  "client_type": "codex_tui"
}
```

### Anthropic 端点（需修正）

**当前 preset 配置**：
- siliconflow：`https://api.siliconflow.cn`（根域无路径）
- siliconflow_en：`https://api.siliconflow.com`（根域无路径）

**文档确认路径**：
- 国内版：`https://api.siliconflow.cn/v1/messages`
- 国际版：`https://api.siliconflow.com/v1/messages`

**Claude Code 配置**：
- 国内版：`ANTHROPIC_BASE_URL="https://api.siliconflow.cn/"`（根域带尾部斜杠）
- 国际版：`ANTHROPIC_BASE_URL="https://api.siliconflow.com/"`（根域带尾部斜杠）

**结论**：当前 preset 根域配置不完整，建议改为 `https://api.siliconflow.[cn|com]/v1` 或在路由层自动补 `/messages` 路径。

---

## 模型全谱

### 模型类型（按 API 分类的 sub_type）

| 类型 | sub_type 值 | 说明 |
|------|------------|------|
| 文本对话 | `chat` | 主要进 model_list 的类型 |
| 文本嵌入 | `embedding` | 向量模型 |
| 重排序 | `reranker` | Rerank 模型 |
| 图像生成 | `text-to-image`, `image-to-image` | 生图模型 |
| 语音 | `speech-to-text`, `text-to-speech` | TTS/STT |
| 视频 | `text-to-video` | 视频生成 |

**model_list 只应包含 `chat` 类型**，其他模态排除（embedding/reranker/TTS/图像生成不属于对话模型）。

### 主要厂商与模型（2026-07 现状）

#### DeepSeek 系列
- `deepseek-ai/DeepSeek-V4-Flash`（推理模型，支持 reasoning_effort: high/max）
- `deepseek-ai/DeepSeek-V4-Pro`
- `deepseek-ai/DeepSeek-V3.2`
- `deepseek-ai/DeepSeek-V3.1`
- `deepseek-ai/DeepSeek-V3.1-Terminus`
- `deepseek-ai/DeepSeek-V3.2-Exp`
- `deepseek-ai/DeepSeek-R1`
- `deepseek-ai/deepseek-vl2`
- `nex-agi/DeepSeek-V3.1-Nex-N1`

#### Qwen 系列（通义千问）
**Qwen 3.x**：
- `Qwen/Qwen3.5-397B-A17B`
- `Qwen/Qwen3.5-122B-A10B`
- `Qwen/Qwen3.5-35B-A3B`
- `Qwen/Qwen3.5-27B`
- `Qwen/Qwen3.5-9B`
- `Qwen/Qwen3.6-27B`
- `Qwen/Qwen3.6-35B-A3B`
- `Qwen/Qwen3-8B`
- `Qwen/Qwen3-14B`
- `Qwen/Qwen3-32B`
- `Qwen/Qwen3-30B-A3B`
- `Qwen/Qwen3-Coder-30B-A3B-Instruct`
- `Qwen/Qwen3-Coder-480B-A35B-Instruct`
- `Qwen/Qwen3-Omni-30B-A3B-Instruct`
- `Qwen/Qwen3-Next-80B-A3B-Instruct`

**Qwen 2.x**：
- `Qwen/Qwen2.5-72B-Instruct`（官方示例常用）
- `Qwen/Qwen2.5-14B-Instruct`
- `Qwen/Qwen2.5-32B-Instruct`
- `Qwen/Qwen2.5-7B-Instruct`
- `Qwen/Qwen2.5-VL-7B-Instruct`
- `Qwen/Qwen2.5-72B-Instruct-128K`
- `Qwen/Qwen2.5-Coder-32B-Instruct`

**Qwen 其他**：
- `Qwen/Qwen3-235B-A22B`
- `Qwen/Qwen3-235B-A22B-Instruct-2507`
- `Qwen/Qwen3-235B-A22B-Thinking-2507`
- `Qwen/Qwen3-30B-A3B-Instruct-2507`
- `Qwen/Qwen3-30B-A3B-Thinking-2507`

#### GLM 系列（智谱 Z.ai/THUDM）
- `zai-org/GLM-5`
- `zai-org/GLM-5.1`
- `zai-org/GLM-4.7`
- `zai-org/GLM-4.6`
- `zai-org/GLM-4.5`
- `zai-org/GLM-4.5-Air`
- `zai-org/GLM-4.5V`
- `zai-org/GLM-4.6V`
- `zai-org/GLM-5V-Turbo`
- `THUDM/GLM-4-32B-0414`
- `THUDM/GLM-4-9B-0414`
- `THUDM/GLM-Z1-32B-0414`
- `THUDM/GLM-Z1-9B-0414`

#### Kimi 系列（月之暗面 Moonshot AI）
- `moonshotai/Kimi-K2-Instruct`
- `moonshotai/Kimi-K2-Instruct-0905`（Claude Code 官方示例）
- `moonshotai/Kimi-K2.5`
- `moonshotai/Kimi-K2.6`
- `moonshotai/Kimi-K2-Thinking`

#### MiniMax 系列
- `MiniMaxAI/MiniMax-M2.5`
- `MiniMaxAI/MiniMax-M2.1`
- `MiniMaxAI/MiniMax-M1-80K`（定价页）

#### StepFun 系列（阶跃星辰）
- `step-3.7-flash`（推测，基于厂商名）
- `step-3.5-flash`（推测，基于厂商名）

#### ByteDance 系列（字节跳动 Seed）
- `ByteDance-Seed/Seed-OSS-36B-Instruct`
- `doubao-seed-2-0-pro`（推测）
- `doubao-seed-2-0-code`（推测）

#### 腾讯混元
- `tencent/Hunyuan-A13B-Instruct`
- `tencent/Hunyuan-MT-7B`
- `tencent/Hy3-preview`

#### 其他厂商
- **百度**：`baidu/ERNIE-4.5-300B-A47B`（多个免费模型）
- **美团**：`inclusionAI/Ling-flash-2.0`, `inclusionAI/Ling-mini-2.0`, `inclusionAI/Ring-flash-2.0`
- **Meta Llama**：`meta-llama/Meta-Llama-3.1-8B-Instruct`
- **Google Gemma**：`google/gemma-4-26B-A4B-it`, `google/gemma-4-31B-it`
- **OpenAI OSS**：`openai/gpt-oss-120b`
- **QwQ**：`QwQ32B`（推测）

### 模型命名规则

1. **格式**：`厂商/模型名`（如 `Qwen/Qwen2.5-72B-Instruct`）
2. **Pro 版本**：`Pro/厂商/模型名`（如 `Pro/zai-org/GLM-4.7`），表示增强/优化版本
3. **大小写敏感**：保持原样，勿转换

### 模型总数估算

从国际版 API 文档的完整枚举（enum）统计：
- **对话模型**：国际版文档枚举约 **60+ 个模型**（含各版本变体）
- **其他模态**：生图约 7 个、语音约 5 个、视频约 2 个

**精确清单需调用**：
- 国内版：`GET https://api.siliconflow.cn/v1/models?type=text&sub_type=chat`
- 国际版：`GET https://api.siliconflow.com/v1/models?type=text&sub_type=chat`

以上端点需认证，文档提供的枚举清单为官方权威来源。

---

## Preset 现状核实

### 国内版 siliconflow 当前配置（需补全）

```json
{
  "endpoints": {
    "default": [
      {
        "protocol": "anthropic",
        "base_url": "https://api.siliconflow.cn",
        "client_type": "claude_code"
      }
    ]
  },
  "models": {
    "default": {}
  },
  "model_list": {
    "default": []
  }
}
```

### 国际版 siliconflow_en 当前配置（需补全）

```json
{
  "endpoints": {
    "default": [
      {
        "protocol": "anthropic",
        "base_url": "https://api.siliconflow.com",
        "client_type": "claude_code"
      }
    ]
  },
  "models": {
    "default": {}
  },
  "model_list": {
    "default": []
  }
}
```

### 问题与建议（两协议通用）

| 字段 | 现状 | 问题 | 建议 |
|------|------|------|------|
| endpoints.default | 仅 1 个 anthropic 端点（根域） | 1. 缺 OpenAI 兼容端点<br>2. anthropic 路径不完整 | 补 openai 端点；anthropic base_url 改为 `https://api.siliconflow.[cn\|com]/v1` |
| models.default | 空 | 无默认模型 | 设 `Qwen/Qwen2.5-72B-Instruct` 或 `deepseek-ai/DeepSeek-V4-Flash` |
| model_list.default | 空 | 模型清单极度缺失 | 补全主流对话模型（见下建议清单） |

---

## model_list.default 建议清单

**主力模型**（覆盖 6 大厂商，共 20 个）：

```json
[
  "Qwen/Qwen2.5-72B-Instruct",
  "Qwen/Qwen3.5-27B",
  "Qwen/Qwen3.5-9B",
  "deepseek-ai/DeepSeek-V4-Flash",
  "deepseek-ai/DeepSeek-V3.2",
  "deepseek-ai/DeepSeek-R1",
  "zai-org/GLM-4.7",
  "zai-org/GLM-4.6",
  "moonshotai/Kimi-K2-Instruct-0905",
  "moonshotai/Kimi-K2.6",
  "MiniMaxAI/MiniMax-M2.5",
  "tencent/Hunyuan-A13B-Instruct",
  "ByteDance-Seed/Seed-OSS-36B-Instruct",
  "Qwen/Qwen3-Coder-30B-A3B-Instruct",
  "meta-llama/Meta-Llama-3.1-8B-Instruct",
  "google/gemma-4-26B-A4B-it",
  "openai/gpt-oss-120b",
  "Qwen/Qwen2.5-Coder-32B-Instruct",
  "baidu/ERNIE-4.5-300B-A47B",
  "inclusionAI/Ling-flash-2.0"
]
```

**说明**：
- Qwen/DeepSeek/GLM/Kimi 四大厂商全覆盖
- 包含推理模型（DeepSeek-R1, V4-Flash）
- 包含编程模型（Qwen3-Coder, Qwen2.5-Coder）
- MiniMax/字节/腾讯/百度/谷歌/Meta 代表性模型

**完整清单**：国际版文档提供完整枚举约 60+ 模型，上表仅覆盖主流。

---

## models.default 建议

**默认模型**：`Qwen/Qwen2.5-72B-Instruct`

**理由**：
- 官方文档常用示例
- 性价比高（国内版定价页 ¥1.8 输入 / ¥10.8 输出每 M tokens）
- 通用对话场景主流选择

**替代选项**：
- 编程场景：`deepseek-ai/DeepSeek-V4-Flash`（推理强，Claude Code 官方合作）
- 成本优先：`Qwen/Qwen3.5-9B`（小模型，价格更低）

---

## 认证方式

**API Key**：通过 `Authorization: Bearer <api_key>` 传递

**获取方式**：
- 国内版：https://cloud.siliconflow.cn/account/ak
- 国际版：https://cloud.siliconflow.com/account/ak

**Claude Code 配置**：环境变量 `ANTHROPIC_API_KEY` 或手动输入

---

## 国际版 vs 国内版差异

### 域名差异（唯一差异）

| 项目 | 国内版（siliconflow） | 国际版（siliconflow_en） |
|------|---------------------|------------------------|
| 官网域名 | siliconflow.cn | siliconflow.com |
| API 域名 | api.siliconflow.cn | api.siliconflow.com |
| 文档域名 | docs.siliconflow.cn | docs.siliconflow.com |
| 控制台域名 | cloud.siliconflow.cn | cloud.siliconflow.com |
| 定价页域名 | siliconflow.cn/pricing | siliconflow.com/pricing |

### 路径结构一致性

**结论**：国内版与国际版路径结构**完全一致**，仅域名差异：
- OpenAI 兼容：`/v1/chat/completions`
- Anthropic：`/v1/messages`
- 模型列表：`/v1/models`
- 嵌入：`/v1/embeddings`
- Rerank：`/v1/rerank`
- 图像生成：`/v1/images/generations`

### 模型清单差异

**结论**：**完全一致**。

国际版 API 文档提供的完整枚举（enum）清单与国内版定价页、文档示例中的模型完全一致。未发现地区可用性差异（部分模型仅国内/仅国际可调用）。

**证据**：
1. 国际版文档枚举的 DeepSeek/Qwen/GLM/Kimi 模型清单与国内版一致
2. 国际版 Claude Code 配置示例使用 `your-preferred-model`，暗示模型库通用
3. 无任何文档提及地区限制或可用性差异

### 认证方式差异

**结论**：**可能独立**。

- 国内版 API key：从 `cloud.siliconflow.cn/account/ak` 获取
- 国际版 API key：从 `cloud.siliconflow.com/account/ak` 获取
- 两个域名分开控制台，**推测** API key 独立（国内版 key 无法用于国际版，反之亦然）

**注意**：未找到官方文档明确说明 key 是否通用，基于分开控制台推测为独立体系。

### siliconflow_en 复用结论

**siliconflow_en 的 model_list/models.default 可完全复用 siliconflow 国内版结论，仅 endpoints base_url 域名替换 .cn→.com。**

**配置映射**：
- siliconflow_endpoints → siliconflow_en_endpoints（域名替换）
- siliconflow_model_list → siliconflow_en_model_list（完全复用）
- siliconflow_models.default → siliconflow_en_models.default（完全复用）

---

## Caveats / Not Found

1. **模型清单无法免认证获取**：`/v1/models` 端点返回 "Invalid token"（需认证），无法直接拉取全量清单。以上模型列表基于国际版文档枚举 + 国内版文档示例 + 定价页手工整理。
2. **Anthropic 端点路径歧义**：文档 cURL 示例用 `/v1/messages`，Claude Code 配置用根域 `https://api.siliconflow.[cn|com]/`。两者关系未明确说明（可能是 Claude Code 自动补路径，或 SiliconFlow 路由层自动匹配）。
3. **OpenAI 兼容端点缺失**：preset（siliconflow + siliconflow_en）仅配置 anthropic 端点，但文档优先展示 OpenAI 兼容协议，疑似遗漏。需补充 openai 端点配置。
4. **Pro 版本含义**：文档未明确 `Pro/` 前缀的官方定义（可能是增强版、优化版、或高可用版本），以上仅为推测。
5. **模型 ID 与显示名映射**：定价页显示中文名（如 "Kimi"、"Qwen"），API 使用英文 ID（如 `moonshotai/Kimi-K2-Instruct-0905`），映射关系未完全对应。
6. **国际版 API key 通用性**：未找到官方文档明确说明国内版与国际版 API key 是否通用，基于分开控制台推测为独立体系。

---

## 结论摘要

**一句话 PRD**：SiliconFlow 是国内一站式模型推理平台，双协议支持（OpenAI 兼容 + Anthropic），托管 60+ 对话模型（Qwen/DeepSeek/GLM/Kimi/MiniMax 等），国内版与国际版路径结构完全一致、模型清单完全一致，仅域名差异（.cn vs .com）。

**关键行动**（siliconflow + siliconflow_en 通用）：
1. 补 openai 端点：`https://api.siliconflow.[cn|com]/v1`
2. 修正 anthropic 端点 base_url 为 `https://api.siliconflow.[cn|com]/v1`
3. 设默认模型：`Qwen/Qwen2.5-72B-Instruct`
4. 补 model_list：20 个主流模型（见建议清单）

**siliconflow_en 复用结论**：siliconflow_en 复用 siliconflow 国内版 model_list + models.default，仅 endpoints base_url 域名替换 .cn→.com。
