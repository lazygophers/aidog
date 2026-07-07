# Research: 非聚合平台旗舰模型列表

- **Query**: 8 非聚合平台旗舰 model 列表（3-5 个/平台）+ 2 平台 default model
- **Scope**: external (web search + official docs)
- **Date**: 2025-01-08

---

## 执行摘要

| 平台 | 状态 | 旗舰模型 (3-5个) | Default Model 建议 |
|------|------|-----------------|-------------------|
| gemini | ✅ 完成 | gemini-2.5-pro, gemini-2.5-flash, gemini-3.5-flash, gemini-3 | gemini-2.5-pro |
| bailian | ✅ 已有 | qwen3.7-max, qwen3.7-plus, qwen3-coder-plus, qwen3.6-flash | qwen3.7-max (已有) |
| bailian_coding | ⚠️ 部分 | 需确认 Qwen coding 模型 ID | qwen3-coder-plus (推测) |
| bailing | ❌ 待查 | 需要: 官网模型列表 | - |
| qianfan | ❌ 待查 | 需要: ERNIE 模型列表 | - |
| longcat | ❌ 待查 | 需要: 模型列表 | - |
| compshare | ✅ 聚合平台 | 动态模型库 (/v1/models API) | 留空 by design |
| opencode | ⚠️ 特殊 | 非 model provider，是 coding agent 工具 | 留空 |
| siliconflow | ✅ 聚合平台 | 多厂商聚合 (Qwen/DeepSeek/Kimi 等) | 可选填 Qwen 系列旗舰 |

---

## 详细发现

### 1. gemini (Google Gemini API)

**来源**: https://ai.google.dev/gemini-api/docs/models

**旗舰模型列表**:
- `gemini-2.5-pro` - 最先进模型，复杂任务，深度推理与编程能力
- `gemini-2.5-flash` - 最佳性价比，低延迟高吞吐任务
- `gemini-2.5-flash-lite` - Flash 轻量版
- `gemini-3.5-flash` - 稳定版
- `gemini-3` - 稳定/预览版

**Default Model 建议**: `gemini-2.5-pro` (most advanced flagship)

**注意**: `gemini-2.0-flash` 已 shut down，避免使用

---

### 2. bailian (阿里云百炼)

**来源**: platform-presets.json 已有数据

**当前模型**:
- `qwen3.7-max` (default)
- `qwen3.7-plus`
- `qwen3.6-flash`
- `qwen3.5-omni-plus`
- `qwen3-coder-plus`
- `qwen3-coder-flash`

**状态**: ✅ 已有完整模型列表，无需补充

---

### 3. bailian_coding (阿里云百炼编程)

**来源**: https://help.aliyun.com/zh/model-studio/developer-reference/use-claude-code

**当前状态**: models.default={}, model_list.default=[]

**说明**: Claude 兼容编程端点，推测使用 Qwen coding 模型

**建议模型** (基于 bailian 的 coding_plan 分支):
- `qwen3-coder-plus` (推测 default)
- `qwen3-coder-flash`
- `qwen3.7-max` (通用旗舰)
- `qwen3.7-plus`

**需要**: 确认 bailian_coding 端点实际支持的模型 ID 格式

---

### 4. bailing (TBox 百灵)

**来源**: https://www.tbox.cn/

**当前状态**: models.default={}, model_list.default=[]

**说明**: Claude 兼容模型

**需要**: ❌ **需要: bailing 官网模型列表** (官网页面信息较少)

---

### 5. qianfan (百度千帆)

**来源**: https://cloud.baidu.com/doc/QIANFAN/index.html

**当前状态**: models.default={}, model_list.default=[]

**说明**: ERNIE 系列模型

**需要**: ❌ **需要: 百度千帆 ERNIE 模型 ID 列表** (文档需登录/控制台查看)

---

### 6. longcat (美团 LongCat)

**来源**: https://longcat.chat/, https://longcat.chat/pricing

**当前状态**: models.default={}, model_list.default=[]

**说明**: Claude 兼容模型

**需要**: ❌ **需要: longcat 模型列表** (官网信息极少)

---

### 7. compshare (优云 UCloud)

**来源**: https://www.compshare.cn/docs/modelverse/models/quick-start, https://api.modelverse.cn/v1/models

**当前状态**: models.default={}, model_list.default=[]

**说明**: **聚合平台**，支持 OpenAI/Anthropic/Gemini 协议

**模型获取方式**: 动态 API
```bash
GET https://api.modelverse.cn/v1/models
```

**示例模型** (来自 API 响应):
- `deepseek-ai/DeepSeek-R1`
- `gpt-5`
- `glm-5.2`
- `kimi-k2.7-code`
- `gemini-3-pro-image`
- `claude-sonnet-5`
- `claude-fable-5`
- `grok-4.3`
- `qwen3.7-plus`
- `MiniMax-M3`
- 以及更多...

**建议**: 留空 by design (聚合平台模型动态变化)

---

### 8. opencode (OpenCode Go)

**来源**: https://opencode.ai/docs/

**当前状态**: models.default={}, model_list.default=[]

**重要发现**: **OpenCode 不是 model provider！**

**说明**:
- OpenCode 是开源 AI coding agent 工具
- 支持多种 LLM provider (需配置 API key)
- `opencode.ai/zen/go/v1` 是其 curated model service (OpenCode Zen)
- 非 LLM 提供商，只是多模型路由工具

**建议**:
- `opencode` protocol 可保留为用户自定义端点
- models.default/model_list.default 留空
- 或考虑是否需要在 platform-presets.json 中保留此协议

---

### 9. siliconflow (硅基流动) - Default Model 判断

**来源**: https://docs.siliconflow.cn/, https://siliconflow.cn/pricing

**说明**: **聚合平台**，多厂商模型一站式服务

**定价页显示的厂商/模型**:
- Qwen 系列 (¥1.8-18/M tokens)
- DeepSeek 系列 (¥1-24/M tokens)
- Kimi (¥6.5-27/M tokens)
- MiniMax (¥2.1-8.4/M tokens)
- meituan-longcat (¥5-20/M tokens)
- Stepfun-ai (¥0.7-2.1/M tokens)
- Baidu (免费)
- 以及更多...

**Default Model 建议**:
- **选项 A**: 留空 by design (聚合平台)
- **选项 B**: 填 `Qwen/Qwen2.5-72B-Instruct` 或类似 Qwen 旗舰 (国产热门)
- **选项 C**: 填 `deepseek-ai/DeepSeek-V3` (当前热门)

**需要**: 用户确认聚合平台是否补 default model

---

## 来源 URL 汇总

| 平台 | Docs URL | Pricing URL |
|------|----------|-------------|
| gemini | https://ai.google.dev/gemini-api/docs/models | https://ai.google.dev/gemini-api/docs/pricing |
| bailian | https://help.aliyun.com/zh/model-studio/ | https://help.aliyun.com/zh/model-studio/billing-for-model-studio |
| bailian_coding | https://help.aliyun.com/zh/model-studio/developer-reference/use-claude-code | - |
| bailing | https://www.tbox.cn/ | https://www.tbox.cn/ |
| qianfan | https://cloud.baidu.com/doc/QIANFAN/index.html | - |
| longcat | https://longcat.chat/ | https://longcat.chat/pricing |
| compshare | https://www.compshare.cn/docs/modelverse/models/quick-start | https://www.compshare.cn/price-list |
| opencode | https://opencode.ai/docs/ | - |
| siliconflow | https://docs.siliconflow.cn/ | https://siliconflow.cn/pricing |

---

## 待确认项

1. **bailing**: 需要 TBox 官网模型列表
2. **qianfan**: 需要百度千帆 ERNIE 模型 ID 列表 (可能需要登录)
3. **bailian_coding**: 确认 Claude 兼容端点支持的模型 ID 格式
4. **longcat**: 需要模型列表
5. **siliconflow default**: 聚合平台是否补 default model

---

## 推荐操作

### 可立即填充 (有明确来源):
1. **gemini**: 填 `gemini-2.5-pro`, `gemini-2.5-flash`, `gemini-2.5-flash-lite`, `gemini-3.5-flash` to model_list.default
2. **gemini default**: `gemini-2.5-pro`

### 需进一步调研:
1. **bailing, qianfan, longcat**: 需要官网/控制台访问获取模型列表
2. **bailian_coding**: 确认模型 ID 格式

### 聚合平台决策:
1. **compshare, siliconflow**: 确认是否留空 by design
2. **opencode**: 确认协议保留必要性

---

## 附录：Compshare /v1/models API 示例响应

```json
{
  "data": [
    {"id": "google/gemma-4-31b-it", "object": "model", "owned_by": "UCloud_UModelverse"},
    {"id": "claude-sonnet-5", "object": "model", "owned_by": "UCloud_UModelverse"},
    {"id": "glm-5.2", "object": "model", "owned_by": "UCloud_UModelverse"},
    {"id": "kimi-k2.7-code", "object": "model", "owned_by": "UCloud_UModelverse"},
    {"id": "gemini-3-pro-image", "object": "model", "owned_by": "UCloud_UModelverse"},
    {"id": "qwen3.7-plus", "object": "model", "owned_by": "UCloud_UModelverse"},
    {"id": "MiniMax-M3", "object": "model", "owned_by": "UCloud_UModelverse"},
    {"id": "deepseek-ai/DeepSeek-R1", "object": "model", "owned_by": "UCloud_UModelverse"},
    {"id": "gpt-5", "object": "model", "owned_by": "UCloud_UModelverse"},
    ...
  ],
  "object": "list"
}
```

**注意**: Compshare 模型 ID 格式不统一，部分带厂商前缀 (`deepseek-ai/`, `google/`)，部分不带
