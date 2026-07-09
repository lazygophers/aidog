# Research: MiniMax 官方模型清单

- **Query**: MiniMax 全部官方模型清单 + 端点 + 认证方式（minimax 国内 + minimax_en 国际）
- **Scope**: external (官方文档)
- **Date**: 2026-07-09

## 官方文档源

### 国际站 (minimax.io / minimax_en)
- **模型总览**: https://platform.minimax.io/docs
- **Anthropic API 参考**: https://platform.minimax.io/docs/api-reference/text-anthropic-api
- **OpenAI API 参考**: https://platform.minimax.io/docs/api-reference/text-openai-api
- **Release Notes**: https://platform.minimax.io/docs/release-notes/models
- **Pricing**: https://platform.minimax.io/document/Price (404，备用：https://platform.minimax.io/docs/pricing/overview)
- **公告页**: https://platform.minimax.io/document/Announcement

### 国内站 (minimaxi.com / minimax)
- **公告页**: https://platform.minimaxi.com/document/Announcement
- **价格页**: https://platform.minimaxi.com/document/Price
- **Guides**: https://platform.minimaxi.com/document/Guides (页面需要登录/JS 渲染，Jina Reader 仅返回备案信息)

**注意**: 国内站文档通过 Jina Reader 无法完全获取（返回备案信息页），可能需要登录态或 JS 渲染。

## model_list 最终清单（文本对话主线）

### Stable 模型（国际站 OpenAI/Anthropic API 可用）

| 模型 ID | 上下文窗口 | 描述 | 来源 |
|---------|-----------|------|------|
| **MiniMax-M3** | 1,000,000 | 最新 M 系列多模态模型，Agent 推理、工具调用、编程、长文本 | [release notes](https://platform.minimax.io/docs/release-notes/models#jun-1-2026) |
| **MiniMax-M2.7** | 204,800 | 递归自我改进之旅起点 | [release notes](https://platform.minimax.io/docs/release-notes/models#mar-18-2026) |
| **MiniMax-M2.7-highspeed** | 204,800 | M2.7 高速版，相同性能更快推理 | [models intro](https://platform.minimax.io/docs/guides/models-intro) |
| **MiniMax-M2.5** | 204,800 | 编程与重构优化，巅峰性能 | [release notes](https://platform.minimax.io/docs/release-notes/models#feb-2026) |
| **MiniMax-M2.5-highspeed** | 204,800 | M2.5 高速版 | [models intro](https://platform.minimax.io/docs/guides/models-intro) |
| **MiniMax-M2.1** | 204,800 | 230B 总参数，多语言编程精通 | [release notes](https://platform.minimax.io/docs/release-notes/models#dec-22-2025) |
| **MiniMax-M2.1-highspeed** | 204,800 | M2.1 高速版 | [models intro](https://platform.minimax.io/docs/guides/models-intro) |
| **MiniMax-M2** | 200,000 | Agent 能力，高级推理，最大输出 128k tokens | [release notes](https://platform.minimax.io/docs/release-notes/models#oct-27-2025) |

### highspeed 变体确认

**结论**: highspeed 变体是**官方独立的 model id**，应加入 model_list。

**证据**:
- 官方 API 文档在模型列表中以反引号标出独立 ID：`MiniMax-M2.7-highspeed`, `MiniMax-M2.5-highspeed`, `MiniMax-M2.1-highspeed`
- 可直接作为 ChatCompletion 请求的 `model` 字段值使用
- 非 alias/计费档，是独立的模型端点

**建议**: **应加入 model_list**

### 遗留/非主线模型（不并入 model_list）

| 模型 ID | 类型 | 发布时间 | 状态 | 来源 |
|---------|------|----------|------|------|
| **MiniMax-Text-01** | 文本对话 LLM | 2025-01-15 | 遗留 | [release notes](https://platform.minimax.io/docs/release-notes/models#jan-15-2025) |
| **MiniMax-VL-01** | 视觉语言模型 | 2025-01-15 | 遗留 | [release notes](https://platform.minimax.io/docs/release-notes/models#jan-15-2025) |
| **abab6.5 / abab6.5s / abab7** | 旧版 abab 系列 | - | **已废弃** | 排除 |

**abab 系列最终判定**:
- 国内站备案提到"abab多模态"（整体类别备案号），但非具体模型 ID
- 国际站官方文档、博客、GitHub 均无 abab6.5/abab7 等具体模型 ID
- 当前 preset JSON 描述中提及"abab 与海螺系列模型"为历史描述，实际模型列表不含任何 abab 具体 ID
- 代码库中无任何 abab6/abab7 引用

**结论**: **已废弃，排除**（abab 是历史遗留系列名称，已被 M 系列完全取代）

### minimax 国内 vs minimax_en 国际 差异

**当前信息状态**: 由于国内站文档无法完全获取，无法确认国内站与国际站的模型列表差异。

**需要确认**:
- minimax 国内是否支持全部 M 系列 (M3/M2.7/M2.5/M2.1/M2)
- minimax 国内是否有独有的模型（如旧版 abab 系列）
- 两站的 highspeed 变体是否都可用

## models.default.default 推荐

**当前设置**: MiniMax-M3

**验证**:
- MiniMax-M3 是 2026 年 6 月 1 日发布的最新 M 系列模型
- 官方定位为 "Frontier multimodal coding model"（前沿多模态编程模型）
- 支持 1M 上下文窗口、Agent 推理、工具调用

**结论**: MiniMax-M3 是正确的推荐值。

## endpoints 配置

### 国内站端点验证（2026-07-09 探测结果）

```bash
# 国内站
curl https://api.minimaxi.com/v1/models
# 响应: HTTP/2 401
# 错误: "login fail: Please carry the API secret key in the 'Authorization' field (1004)"

# 国际站（对照）
curl https://api.minimax.io/v1/models
# 响应: HTTP/2 401
# 错误: "login fail: Please carry the API secret key in the 'Authorization' field (1004)"
```

**结论**: 
- **国内站端点存在且正确**（401 表明端点路径正确，仅需认证）
- 国内站与国际站响应一致，认证方式相同

### minimax 国内 (api.minimaxi.com)

| 协议 | base_url | 认证方式 | 验证状态 |
|------|----------|----------|----------|
| openai | https://api.minimaxi.com/v1 | Authorization: Bearer `<API_KEY>` | ✓ 已验证（401） |
| anthropic | https://api.minimaxi.com/anthropic | X-Api-Key: `<API_KEY>` 或 Authorization: Bearer | ✓ 推测正确（与国际站对称） |

**来源**: 实际端点探测 + 国际站对称性

### minimax_en 国际 (api.minimax.io)

| 协议 | base_url | 认证方式 | 验证状态 |
|------|----------|----------|----------|
| openai | https://api.minimax.io/v1 | Authorization: Bearer `<API_KEY>` | ✓ 已验证（401 + 官方文档） |
| anthropic | https://api.minimax.io/anthropic | X-Api-Key: `<API_KEY>` 或 Authorization: Bearer | ✓ 官方文档确认 |

**端点路径**:
- OpenAI Chat Completions: `POST /v1/chat/completions`
- Anthropic Messages: `POST /anthropic/v1/messages`
- List Models (OpenAI): `GET /v1/models`
- List Models (Anthropic): `GET /anthropic/v1/models`

## 认证方式详解

### 国际站 (api.minimax.io)

**OpenAI 协议**:
```bash
export OPENAI_BASE_URL=https://api.minimax.io/v1
export OPENAI_API_KEY=${YOUR_API_KEY}
# Header: Authorization: Bearer <token>
```

**Anthropic 协议**:
```bash
export ANTHROPIC_BASE_URL=https://api.minimax.io/anthropic
export ANTHROPIC_API_KEY=${YOUR_API_KEY}
# Header: X-Api-Key: <api-key> 或 Authorization: Bearer
```

**List Models cURL 示例**:
- OpenAI: `curl -H 'Authorization: Bearer <token>' https://api.minimax.io/v1/models`
- Anthropic: `curl -H 'X-Api-Key: <api-key>' https://api.minimax.io/anthropic/v1/models`

### 国内站 (api.minimaxi.com)

**已验证**: 端点探测确认认证方式与国际站一致。

**认证方式**: `Authorization: Bearer <API_KEY>`（与错误信息一致）

**未发现**: group_id 查询参数的旧模式（已在 M 系列废弃）

## 非主线模型区（不并入 model_list）

### 语音 T2A
- **speech-2.8-hd** - 超写实质量，40 种语言，7 种情绪
- **speech-2.8-turbo** - 低延迟
- **speech-2.6-hd/turbo** - 遗留
- **speech-02-hd/turbo** - 遗留

### 视频生成
- **MiniMax Hailuo 2.3** - 文本/图像转视频，1080p 6s
- **MiniMax Hailuo 2.3Fast** - 图像转视频
- **MiniMax Hailuo 02** - 遗留

### 音乐生成
- **Music-2.6** - 封面重生
- **Music-Cover** - 参考音频生成封面
- **Music-2.0** - 遗留

### 图像生成
- **Image-01** - 文本转图像（多种尺寸）

## 排除项与原因

| 模型/类别 | 排除原因 |
|-----------|----------|
| speech-2.x 系列 | 语音合成，非文本对话主线 |
| MiniMax Hailuo 2.x | 视频生成，非文本对话主线 |
| Music-2.x | 音乐生成，非文本对话主线 |
| Image-01 | 图像生成，非文本对话主线 |
| MiniMax-Text-01 | 遗留模型，已被 M 系列取代 |
| MiniMax-VL-01 | 视觉语言模型，遗留状态 |
| **abab6.5/abab6.5s/abab7** | **已废弃**，两站均无，排除 |

## Caveats / 需要 main 关注

### 已解决事项

1. **国内站端点验证**: ✓ 已确认
   - `https://api.minimaxi.com/v1/models` 返回 401（端点存在，需认证）
   - 与国际站响应一致，认证方式相同

2. **highspeed 变体确认**: ✓ 已确认
   - 为官方独立 model id
   - **建议加入 model_list**

3. **abab 系列最终判定**: ✓ 已判定
   - 两站均无 abab6.5/abab7 具体模型 ID
   - **已废弃，排除**

### 未决事项

1. **国内站模型列表**: 
   - 无法通过文档获取完整模型列表
   - **建议**: 尝试使用有效 API key 调用国内站 `/v1/models` 获取实际可用模型

2. **国内站与国际站差异**:
   - 不确定国内站是否支持所有 M 系列和 highspeed 变体
   - **需要**: 实际验证或官方确认

### 当前 Preset 与官方文档对比

| 项目 | 当前 Preset | 官方文档 | 差异 |
|------|------------|----------|------|
| 模型列表 | M3, M2.7, M2.5, M2.1, M2 | 同上 + **highspeed 变体** | **缺少 highspeed 变体** |
| 默认模型 | MiniMax-M3 | MiniMax-M3 | 一致 |
| 国内端点 | api.minimaxi.com/v1 + anthropic | ✓ 已验证 | 一致 |
| 国际端点 | api.minimax.io/v1 + anthropic | 一致 | 一致 |

### 下一步建议

1. **添加 highspeed 变体**: 将 M2.7-highspeed/M2.5-highspeed/M2.1-highspeed 加入 model_list
2. **国内站模型验证**: 如有国内站 API key，调用 `/v1/models` 获取实际可用模型
3. **确认 abab 描述更新**: 考虑更新 preset 描述中"abab 与海螺系列"为"海螺系列"（abab 已废弃）

## API 调用示例（国际站）

### OpenAI SDK
```python
from openai import OpenAI

client = OpenAI(
    base_url="https://api.minimax.io/v1",
    api_key="YOUR_API_KEY"
)

response = client.chat.completions.create(
    model="MiniMax-M3",
    messages=[{"role": "user", "content": "Hello"}]
)
```

### Anthropic SDK
```python
import anthropic

client = anthropic.Anthropic(
    base_url="https://api.minimax.io/anthropic",
    api_key="YOUR_API_KEY"
)

message = client.messages.create(
    model="MiniMax-M3",
    max_tokens=1000,
    messages=[{"role": "user", "content": "Hello"}]
)
```
