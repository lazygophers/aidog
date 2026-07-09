# Research: ModelScope Official Inference API

- **Query**: 研究阿里魔搭 ModelScope（modelscope 协议）的官方信息，补全 platform-presets.json
- **Scope**: external (API endpoint verification + documentation)
- **Date**: 2026-07-09

---

## Findings

### 官方推理 API 支持模型清单

**来源**: `curl -s https://api-inference.modelscope.cn/v1/models`
**总数**: 55 个模型

#### 按 Provider 分组（18 个 provider）

| Provider | 模型数 | 模型列表 |
|---|---|---|
| deepseek-ai | 3 | DeepSeek-V3.2, DeepSeek-V4-Flash, DeepSeek-V4-Pro |
| Qwen | 20 | Qwen3-14B, Qwen3-235B-A22B, Qwen3-235B-A22B-Instruct-2507, Qwen3-235B-A22B-Thinking-2507, Qwen3-30B-A3B, Qwen3-30B-A3B-Thinking-2507, Qwen3-32B, Qwen3-4B, Qwen3-8B, Qwen3-Coder-30B-A3B-Instruct, Qwen3-Next-80B-A3B-Instruct, Qwen3-Next-80B-A3B-Thinking, Qwen3-VL-235B-A22B-Instruct, Qwen3-VL-8B-Instruct, Qwen3-VL-8B-Thinking, Qwen3.5-122B-A10B, Qwen3.5-27B, Qwen3.5-35B-A3B, Qwen3.5-397B-A17B, Qwen-Image-Edit |
| ZhipuAI | 4 | GLM-4.7-Flash, GLM-5, GLM-5.1, GLM-5.2 |
| MiniMax | 4 | MiniMax-M1-80k, MiniMax-M2.5, MiniMax-M2.7, MiniMax-M3 |
| moonshotai | 1 | Kimi-K2.5 |
| stepfun-ai | 2 | Step-3.5-Flash, Step-3.7-Flash |
| iic | 2 | GUI-Owl-1.5-8B-Instruct, GUI-Owl-1.5-8B-Think |
| PaddlePaddle | 4 | ERNIE-4.5-0.3B-PT, ERNIE-4.5-21B-A3B-PT, ERNIE-4.5-300B-A47B-PT, ERNIE-4.5-VL-28B-A3B-PT |
| Shanghai_AI_Laboratory | 3 | Intern-S1, Intern-S1-mini, Intern-S2-Preview |
| XGenerationLab | 2 | XiYanSQL-QwenCoder-32B-2412, XiYanSQL-QwenCoder-32B-2504 |
| LLM-Research | 1 | Llama-4-Maverick-17B-128E-Instruct |
| MedAIBase | 1 | AntAngelMed |
| MusePublic | 1 | Qwen-Image-Edit |
| OpenGVLab | 1 | InternVL3_5-241B-A28B |
| Tencent-Hunyuan | 1 | Hy3 |
| XiaomiMiMo | 1 | MiMo-V2-Flash |
| mistralai | 1 | Mistral-Large-Instruct-2407 |
| nex-agi | 1 | Nex-N2-Pro |
| opencompass | 1 | CompassJudger-1-32B-Instruct |
| meituan-longcat | 1 | LongCat-Flash-Lite |

### Provider 分布统计

- 主力 provider（模型数 >= 4）：Qwen (20), MiniMax (4), ZhipuAI (4), PaddlePaddle (4)
- 其他 provider：14 个，各 1-3 个模型
- 总 provider 数：18 个
- 总模型数：55 个

### 模型 id 命名格式

**格式**: `Provider/Model-Name`
- 示例：`deepseek-ai/DeepSeek-V4-Pro`, `Qwen/Qwen3.5-397B-A17B`
- Provider 名使用 kebab-case 或 CamelCase
- Model 名使用版本号/规格标识

### Preset 现状核实

**现有配置** (`src-tauri/defaults/platform-presets.json` line 1441-1497):

```json
{
  "endpoints": {
    "default": [{
      "protocol": "anthropic",
      "base_url": "https://api-inference.modelscope.cn",
      "client_type": "claude_code"
    }]
  },
  "models": {
    "default": {}  // 空，需补
  },
  "model_list": {
    "default": [
      "deepseek-ai/DeepSeek-V4-Pro",
      "deepseek-ai/DeepSeek-V4-Flash",
      "deepseek-ai/DeepSeek-V3.2",
      "Qwen/Qwen3.5-397B-A17B",
      "Qwen/Qwen3.5-122B-A10B",
      "Qwen/Qwen3-Coder-30B-A3B-Instruct",
      "ZhipuAI/GLM-5.2",
      "ZhipuAI/GLM-5.1",
      "ZhipuAI/GLM-5",
      "moonshotai/Kimi-K2.5",
      "MiniMax/MiniMax-M3",
      "MiniMax/MiniMax-M2.7"
    ]
  }
}
```

**验证结果**:
- ✓ 所有 12 个 preset 模型均在官方 API 列表中（有效）
- 模型分布：deepseek-ai (3), Qwen (3), ZhipuAI (3), moonshotai (1), MiniMax (2)
- 无下架/改名情况

### Endpoints 核实

**实验结果**:

| 端点路径 | 协议 | 状态 | 错误格式（无认证） |
|---|---|---|---|
| `https://api-inference.modelscope.cn/v1/models` | OpenAI | ✓ 200 | OpenAI 格式 |
| `https://api-inference.modelscope.cn/v1/chat/completions` | OpenAI | ✓ 401 auth required | `{"error":{"message":"...", "request_id":"..."}}` |
| `https://api-inference.modelscope.cn/v1/messages` | Anthropic | ✓ 401 auth required | `{"type":"error","error":{"type":"...","message":"..."}}` |
| `https://api-inference.modelscope.cn/` | - | ✓ 200 welcome | `{"message":"Welcome to... docs at ..."}` |

**结论**: ModelScope 官方推理 API **同时支持 OpenAI 和 Anthropic 两种协议**

**Preset 现状问题**:
- preset 仅配置了 anthropic 端点（1 个）
- **缺少 openai 兼容端点**（应该补上）

**建议 endpoints 配置**:
```json
"endpoints": {
  "default": [
    {
      "protocol": "openai",
      "base_url": "https://api-inference.modelscope.cn/v1",
      "client_type": "codex_tui"
    },
    {
      "protocol": "anthropic",
      "base_url": "https://api-inference.modelscope.cn",
      "client_type": "claude_code"
    }
  ]
}
```

**说明**:
- OpenAI 端点：`base_url` = `https://api-inference.modelscope.cn/v1`（含 `/v1` 前缀）
- Anthropic 端点：`base_url` = `https://api-inference.modelscope.cn`（根域，`provider_api_path()` 会自动添加 `/v1/messages`）
- OpenAI 端点放前面优先（更通用）

### Models.default 建议

**推荐模型**（按用途）:

```json
"models": {
  "default": {
    "default": "Qwen/Qwen3.5-397B-A17B",  // 主力对话模型
    "coder": "Qwen/Qwen3-Coder-30B-A3B-Instruct",  // 编程专用
    "fast": "deepseek-ai/DeepSeek-V4-Flash",  // 快速响应
    "pro": "deepseek-ai/DeepSeek-V4-Pro",  // 高质量
    "mini": "Qwen/Qwen3-8B"  // 轻量
  }
}
```

**选择理由**:
- `Qwen/Qwen3.5-397B-A17B`: Qwen 系列最强，通用对话主力
- `Qwen/Qwen3-Coder-30B-A3B-Instruct`: 官方标注编程专用
- `deepseek-ai/DeepSeek-V4-Flash`: 快速推理场景
- `deepseek-ai/DeepSeek-V4-Pro`: 高质量推理
- `Qwen/Qwen3-8B`: 轻量快速

### 认证方式

**API Key**: ModelScope API Token
- Header: `Authorization: Bearer <token>`
- 获取方式：https://www.modelscope.cn 用户中心生成

**推测**: 与其他平台类似，通过 Bearer token 认证（无实际 key 无法完全验证）

---

## 结论摘要

**一句话摘要**: ModelScope 官方推理 API（api-inference.modelscope.cn）支持 55 个模型（18 个 provider），同时提供 OpenAI 兼容（`/v1/chat/completions`）和 Anthropic 兼容（`/v1/messages`）两种协议端点，preset 需补 openai 端点和 models.default 推荐配置。

**关键数字**:
- 总模型数：55
- Provider 数：18
- Preset 覆盖率：12/55 (22%)
- 协议支持：OpenAI + Anthropic 双协议

**待补项**:
1. endpoints: 加 openai 兼容端点
2. models.default: 填推荐模型配置
