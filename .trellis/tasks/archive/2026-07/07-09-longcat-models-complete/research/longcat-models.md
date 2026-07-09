# Research: Longcat Platform Models

- **Query**: 研究 Longcat（longcat 协议，龙猫）的官方信息，补全 platform-presets.json
- **Scope**: external + mixed (外部官方信息 + 现有配置核实)
- **Date**: 2026-07-09

## 平台定位

**自研模型平台**（非聚合平台）

LongCat 是美团（Meituan/北京酷讯互动技术有限公司）开发的 MoE 大语言模型系列，提供 API 服务。不是多厂商聚合平台，而是单一自研模型提供商。

- 开发方：美团 meituan-longcat 团队 (longcat-team@meituan.com)
- 官网：https://longcat.chat
- 聊天网站：https://longcat.ai
- GitHub：https://github.com/meituan-longcat
- HuggingFace：https://huggingface.co/meituan-longcat

**判定依据**：
- GitHub 组织 `meituan-longcat` 包含多个 LongCat 模型仓库（LongCat-2.0、LongCat-Flash-Omni 等）
- 官方文档显示"目前仅支持 LongCat-2.0"，单一模型产品
- 公司主体：北京酷讯互动技术有限公司（美团关联）

## 现状核实

### 现有 preset 配置（需修正）

```json
{
  "longcat": {
    "endpoints": {
      "default": [
        {
          "protocol": "anthropic",
          "base_url": "https://api.longcat.chat/anthropic",
          "client_type": "claude_code"
        },
        {
          "protocol": "openai",
          "base_url": "https://api.longcat.chat/openai/v1",
          "client_type": "codex_tui"
        }
      ]
    },
    "models": {
      "default": {}  // 空，需补全
    },
    "model_list": {
      "default": []  // 空，需补全
    }
  }
}
```

### 官方文档对照

**端点**（来源：https://longcat.chat/platform/docs/）：
- OpenAI 格式：`https://api.longcat.chat/openai` + `/v1/chat/completions`
- Anthropic 格式：`https://api.longcat.chat/anthropic` + `/v1/messages`

**当前配置问题**：
- ❌ OpenAI endpoint 配置为 `https://api.longcat.chat/openai/v1`
  - 实际 base_url 应为 `https://api.longcat.chat/openai`（不含 `/v1`）
  - provider_api_path() 返回 `/chat/completions`，拼接后为 `/openai/chat/completions`（错误）
  - 正确调用应为 `/openai/v1/chat/completions`
- ✅ Anthropic endpoint 配置 `https://api.longcat.chat/anthropic` 正确
  - 拼接后为 `/anthropic/v1/messages`，与文档一致

**修正建议**：
- OpenAI base_url 应改为 `https://api.longcat.chat/openai`，但需确认项目路由逻辑
- 或保持现状但需验证实际调用路径是否正确

## model_list 补全

### 官方支持模型（单一模型）

根据官方文档（https://longcat.chat/platform/docs/）：

| 模型名称 | API 格式 | 描述 |
|---------|---------|------|
| LongCat-2.0 | OpenAI/Anthropic | 高性能 Agent 原生模型 |

**参考来源**：https://longcat.chat/platform/docs/#supported-models

### 推荐默认模型

- `LongCat-2.0`（唯一可用模型）

### 补全建议

```json
{
  "models": {
    "default": {
      "default": "LongCat-2.0"
    }
  },
  "model_list": {
    "default": [
      "LongCat-2.0"
    ]
  }
}
```

## endpoints 核实

### 官方端点（已验证）

**OpenAI 格式**：
- Base URL: `https://api.longcat.chat/openai`
- 端点：`/v1/chat/completions`
- 示例：`curl https://api.longcat.chat/openai/v1/chat/completions`

**Anthropic 格式**：
- Base URL: `https://api.longcat.chat/anthropic`
- 端点：`/v1/messages`
- 示例：`curl https://api.longcat.chat/anthropic/v1/messages`

**参考来源**：
- Quick Start 文档：https://longcat.chat/platform/docs/
- cURL 示例（文档内已验证）

### 域名变体

- 主域名：`api.longcat.chat`
- 无国际/国内域名变体（仅单一域名）

## 认证方式

**API Key Bearer Token**

- Header: `Authorization: Bearer YOUR_APP_KEY`
- 获取方式：注册后访问 https://longcat.chat/platform/api_keys 创建
- Key 仅显示一次，丢失需重新创建

**参考来源**：https://longcat.chat/platform/docs/#how-to-get-an-api-key

## 模型特性

### LongCat-2.0 规格

- 参数量：1.6 万亿总参数，~48B 激活/令牌（MoE 架构）
- 上下文：1M token 上下文窗口，最大输出 128K tokens
- 特性：原生工具调用、多步推理、Agent 原生
- 集成：深度兼容 Claude Code、OpenClaw、Hermes 等主流编码工具

**参考来源**：
- GitHub README：https://github.com/meituan-longcat/LongCat-2.0
- 技术博客：https://longcat.chat/blog/longcat-2.0

## 定价

### LongCat-2.0 价格（Pay-As-You-Go）

| 项目 | 原价 ($/1M tokens) | 限时优惠 ($/1M tokens) |
|-----|------------------|-------------------|
| 未缓存输入 | $0.75 | $0.30 |
| 缓存输入 | $0.015 | $0.006 |
| 输出 | $2.95 | $1.20 |

**参考来源**：https://longcat.chat/platform/docs/Pricing/LongCat-2.0.html

## 结论摘要

LongCat 是美团开发的 MoE 大语言模型系列，当前仅提供 `LongCat-2.0` 一个模型（1.6T 参数，1M 上下文）。平台支持 OpenAI 和 Anthropic 两种 API 格式，base_url 分别为 `https://api.longcat.chat/openai` 和 `https://api.longcat.chat/anthropic`。

**关键修正**：
1. model_list 补全为 `["LongCat-2.0"]`
2. models.default.default 设为 `"LongCat-2.0"`
3. OpenAI endpoint base_url 路径需验证（当前 `/openai/v1` 可能应为 `/openai`）

## 参考链接

- 官网：https://longcat.chat
- API 文档：https://longcat.chat/platform/docs/
- Pricing：https://longcat.chat/platform/docs/Pricing/LongCat-2.0.html
- GitHub：https://github.com/meituan-longcat
- LongCat-2.0 技术博客：https://longcat.chat/blog/longcat-2.0
- LongCat-2.0 README：https://github.com/meituan-longcat/LongCat-2.0

## Caveats / Not Found

- 未找到其他模型（如 LongCat-Flash、LongCat-Next）的 API 服务信息，这些可能是研究模型或未对外开放
- OpenAI endpoint 的 `/v1` 路径配置需与项目内 `provider_api_path()` 函数逻辑交叉验证
- 未找到模型列表的 API 端点（`/models` 返回认证错误），模型清单基于文档静态页面
