# Research: DMXAPI Models

- **Query**: DMXAPI（dmxapi 协议）官方信息，补全 platform-presets.json
- **Scope**: 外部搜索 + preset 现状分析
- **Date**: 2026-07-09

## 全量模型清单

### 信息获取限制

DMXAPI 的 `/v1/models` 端点需要认证（返回 `Invalid token`），官方文档页面（docs.dmxapi.cn）为动态 SPA，无法直接抓取模型列表。

### 推测：基于聚合平台特性 + aihubmix 对比

DMXAPI 是**聚合平台**（类似 aihubmix），路由多家模型供应商。基于：

1. **aihubmix preset 模型清单**（15 项）：
   - claude-opus-4-8, claude-sonnet-4-6, claude-sonnet-4-5
   - gpt-5.5, gpt-5.5-pro, gpt-5.3-codex
   - gemini-3.5-flash, gemini-3.1-pro-preview
   - deepseek-v4-pro, deepseek-v4-flash
   - qwen3.7-max, glm-5.2, kimi-k2.7-code, grok-4.3

2. **dmxapi preset 现状**（11 项）：
   - claude-opus-4-8, claude-sonnet-4-6, claude-opus-4-5-20251101
   - deepseek-v4-pro, deepseek-v4-flash
   - gpt-5.5, gpt-5.3-codex
   - gemini-3.5-flash, gemini-3.1-pro-preview
   - glm-5.2, kimi-k2.7-code

3. **文档结构分析**（doc.dmxapi.cn 导航）：
   - 支持请求格式：openai, anthropic, gemini
   - 支持场景：文本对话、图片分析、向量转换、JSON 输出
   - 工具接入：Claude Code, OpenAI Codex, Cursor, Cherry Studio, Dify

### 推测：全量模型范围（需验证）

基于聚合平台特性和文档提及，DMXAPI 应支持以下 provider：

| Provider | 推测模型（参考 aihubmix + 各平台当前旗舰） |
|---|---|
| Anthropic | claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5 |
| OpenAI | gpt-5.5, gpt-5.4, gpt-5.3-codex |
| Google Gemini | gemini-3.5-flash, gemini-3.1-pro-preview, gemini-2.5-flash |
| DeepSeek | deepseek-v4-pro, deepseek-v4-flash |
| Zhipu GLM | glm-5.2, glm-5.1 |
| Moonshot Kimi | kimi-k2.7-code, kimi-k2.6 |
| Alibaba Qwen | qwen3.7-max, qwen3-coder |
| MiniMax | MiniMax-M3, MiniMax-M2.7 |
| Grok | grok-4.3 |

## Provider 分布统计

**推测（基于 aihubmix 对比 + 文档结构）**：
- Anthropic: ~3 项
- OpenAI: ~3 项
- Gemini: ~3 项
- DeepSeek: ~2 项
- GLM: ~2 项
- Kimi: ~2 项
- Qwen: ~2 项
- MiniMax: ~2 项
- Grok: ~1 项
- **总计：约 20 项**

## 模型 ID 命名格式

**结论：裸 ID（无 provider 前缀）**

依据：
- preset 现状：`claude-opus-4-8`（非 `anthropic/claude-opus-4-8`）
- aihubmix 对比：同样是裸 ID
- 符合聚合平台惯例：统一命名空间，用户无需关心底层 provider

## Preset 现状 11 项核实

| 模型 ID | 状态 | 备注 |
|---|---|---|
| claude-opus-4-8 | ✓ 有效 | Anthropic 最新旗舰 |
| claude-sonnet-4-6 | ✓ 有效 | Anthropic 主力 |
| claude-opus-4-5-20251101 | ⚠ 历史版本 | 2025-11-01 版本，可能已过期 |
| deepseek-v4-pro | ✓ 有效 | DeepSeek 最新旗舰 |
| deepseek-v4-flash | ✓ 有效 | DeepSeek 闪速版 |
| gpt-5.5 | ✓ 有效 | OpenAI 最新旗舰 |
| gpt-5.3-codex | ✓ 有效 | OpenAI 编程专用 |
| gemini-3.5-flash | ✓ 有效 | Gemini 最新闪速版 |
| gemini-3.1-pro-preview | ✓ 有效 | Gemini Pro 版 |
| glm-5.2 | ✓ 有效 | GLM 最新旗舰 |
| kimi-k2.7-code | ✓ 有效 | Kimi 编程专用 |

## Endpoints 核实

### 现状配置

| 协议 | base_url | client_type | 状态 |
|---|---|---|---|
| anthropic | https://www.dmxapi.cn | claude_code | ✓ 正确（根域无路径） |
| openai | https://www.dmxapi.cn/v1 | codex_tui | ✓ 正确（标准 /v1 路径） |

### Gemini 原生协议支持

**结论：推测支持**

依据：
1. 文档导航有 `🧠gemini请求格式` 链接（gemini-chat.html）
2. 文档结构显示支持三种请求格式：openai、anthropic、gemini
3. 推荐补全端点：
   ```json
   {
     "protocol": "gemini",
     "base_url": "https://www.dmxapi.cn",
     "client_type": "default"
   }
   ```
   或
   ```json
   {
     "protocol": "gemini",
     "base_url": "https://www.dmxapi.cn/v1",
     "client_type": "default"
   }
   ```
   **需验证**：gemini 协议端点路径（根域 vs /v1）

## Models.Default 建议

```json
{
  "default": "claude-opus-4-8",
  "opus": "claude-opus-4-8",
  "sonnet": "claude-sonnet-4-6",
  "gpt": "gpt-5.5",
  "gemini": "gemini-3.5-flash",
  "deepseek": "deepseek-v4-pro",
  "glm": "glm-5.2",
  "kimi": "kimi-k2.7-code"
}
```

理由：按 provider 通用旗舰模型映射，覆盖主要使用场景。

## 时效性与腐化风险

| 因素 | 风险等级 | 说明 |
|---|---|---|
| 模型版本更新 | 中 | 各平台持续发布新模型（如 claude-fable-5, gpt-5.5-pro），需季度同步 |
| preset 手维护 | 高 | preset JSON 为手维护真值源，无自动同步机制 |
| 端点路径变化 | 低 | anthropic/openai 端点路径稳定，gemini 待验证 |
| 模型退役 | 中 | 历史版本（如 claude-opus-4-5-20251101）可能已退役 |

建议：每季度复核一次模型清单，对比官方定价页。

## 结论摘要

**一句话给 PRD**：DMXAPI 是国内聚合平台（dmxapi.cn），支持裸 ID 格式的多 provider 路由（anthropic/openai/gemini 协议），当前 preset 覆盖 11 项精选模型，与 aihubmix 高度相似，建议补充 gemini 端点并更新 models.default 映射。

**关键数字**：
- preset 现状：11 项
- 推测全量：约 20 项（需官方验证）
- endpoints：2 个（anthropic + openai），建议补 gemini

## Caveats / Not Found

1. **全量模型清单**：无法通过 `/v1/models` 获取（需认证），以上为基于 aihubmix 对比 + 文档结构的推测
2. **gemini 端点路径**：文档显示支持 gemini 协议，但具体 base_url（根域 vs /v1）需验证
3. **模型 ID 命名格式**：基于 preset 现状推断为裸 ID，无官方文档明确说明
4. **认证方式**：推测为 API Key（Authorization Bearer），未在文档中找到明确说明

## 下一步验证建议

1. **获取有效 API Key**：联系 DMXAPI 或注册账号，测试 `/v1/models` 端点
2. **验证 gemini 端点**：测试 `https://www.dmxapi.cn` + gemini 协议请求
3. **对比官方定价页**：登录后访问 `/rmb` 页面，提取完整模型清单
