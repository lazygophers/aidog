# Research: 商汤日日新 SenseNova 模型与端点

- **Query**: 研究商汤日日新 SenseNova（sensenova 协议）的官方信息，补全 platform-presets.json
- **Scope**: 外部调研（官方文档、GitHub、官网）
- **Date**: 2026-07-09

## 现状核实（preset 各字段对照官方）

### 当前 preset 配置（`src-tauri/defaults/platform-presets.json:1103-1157`）

```json
{
  "endpoints": {
    "default": [
      {
        "protocol": "openai",
        "base_url": "https://token.sensenova.cn/v1",
        "client_type": "codex_tui"
      },
      {
        "protocol": "anthropic",
        "base_url": "https://token.sensenova.cn",
        "client_type": "claude_code"
      }
    ]
  },
  "models": {
    "default": {
      "default": "sensenova-6.7-flash-lite"
    }
  },
  "model_list": {
    "default": [
      "sensenova-6.7-flash-lite",
      "deepseek-v4-flash",
      "sensenova-u1-fast"
    ]
  },
  "source_urls": {
    "docs": "https://platform.sensenova.cn/document",
    "pricing": "https://platform.sensenova.cn/pricing"
  },
  "homepage": "https://www.sense_time.com"
}
```

### 核实结果

| 字段 | 现状 | 核实结果 | 来源 |
|------|------|----------|------|
| `endpoints[0].base_url` | `https://token.sensenova.cn/v1` | ✅ 正确 | [官方 API 文档](https://github.com/OpenSenseNova/SenseNova6.7/blob/main/API.md#41-curl) |
| `endpoints[1].base_url` | `https://token.sensenova.cn` | ⚠️ 未在官方文档明确提及 | 待确认 |
| `models.default.default` | `sensenova-6.7-flash-lite` | ✅ 正确 | 官方推荐默认模型 |
| `model_list[0]` | `sensenova-6.7-flash-lite` | ✅ 正确 | [官方文档](https://github.com/OpenSenseNova/SenseNova6.7) |
| `model_list[1]` | `deepseek-v4-flash` | ⚠️ 非商汤自研，推测为第三方转发 | [DeepSeek 官网](https://www.deepseek.com) |
| `model_list[2]` | `sensenova-u1-fast` | ✅ 正确 | [Token Plan 页面](https://www.sensenova.cn/token-plan) |
| `homepage` | `https://www.sense_time.com` | ⚠️ 域名可能过期，建议改为 `https://www.sensetime.com` | [商汤官网](https://www.sensetime.com) |

## model_list 补全建议

### 官方在售/可用模型（2026-07 现状）

根据官方文档和官网，商汤日日新平台目前主要提供以下 API 模型：

| 模型 ID | 描述 | 官方来源 |
|---------|------|----------|
| `sensenova-6.7-flash-lite` | 轻量多模态智能体模型，面向真实工作流 | [GitHub SenseNova6.7](https://github.com/OpenSenseNova/SenseNova6.7) |
| `sensenova-u1-fast` | 新一代原生多模态模型，理解生成一体 | [Token Plan](https://www.sensenova.cn/token-plan) |

### 推测：第三方转发模型

| 模型 ID | 来源推测 | 说明 |
|---------|----------|------|
| `deepseek-v4-flash` | DeepSeek 第三方转发 | DeepSeek-V4 是深度求索的模型，非商汤自研。商汤可能作为聚合平台提供转发访问。 |

### 开源模型（不在 API 在售列表，仅供参考）

以下模型已在 GitHub 开源，但**不通过商汤 API 提供**：

| 模型 | 描述 | GitHub |
|------|------|--------|
| SenseNova-U1 | 理解生成一体模型 | [OpenSenseNova/SenseNova-U1](https://github.com/OpenSenseNova/SenseNova-U1) |
| SenseNova-SI | 空间智能大模型 | [OpenSenseNova/SenseNova-SI](https://github.com/OpenSenseNova/SenseNova-SI) |
| SenseNova-MARS | 多模态自主推理模型 | [OpenSenseNova/SenseNova-MARS](https://github.com/OpenSenseNova/SenseNova-MARS) |
| Piccolo Embedding | 通用 embedding 模型 | [OpenSenseNova/piccolo-embedding](https://github.com/OpenSenseNova/piccolo-embedding) |
| NEO v1 | 新一代多模态大模型 | [EvolvingLMMS-Lab/NEO](https://github.com/EvolvingLMMS-Lab/NEO) |
| Kairos-SenseNova | 时序预测与决策模型 | [kairos-agi/kairos-sensenova](https://github.com/kairos-agi/kairos-sensenova) |

### model_list 建议配置

保持现状即可，无需大改：

```json
"model_list": {
  "default": [
    "sensenova-6.7-flash-lite",
    "sensenova-u1-fast",
    "deepseek-v4-flash"
  ]
}
```

**建议排序**：将商汤自研模型前置，第三方转发模型后置。

## endpoints 核实

### OpenAI 兼容端点

✅ **已确认正确**

- **base_url**: `https://token.sensenova.cn/v1`
- **验证方式**: Bearer Token
- **官方文档**: [SenseNova6.7/API.md](https://github.com/OpenSenseNova/SenseNova6.7/blob/main/API.md)
- **curl 示例**（官方）:
  ```bash
  curl 'https://token.sensenova.cn/v1/chat/completions' \
    -H "Authorization: Bearer <YOUR_API_KEY>" \
    -H 'Content-Type: application/json' \
    -d '{
      "model": "sensenova-6.7-flash-lite",
      "max_tokens": 2000,
      "messages": [{"role": "user", "content": "Hi"}]
    }'
  ```

### Anthropic 兼容端点

⚠️ **未在官方文档明确提及，需用户确认**

- **preset 配置**: `https://token.sensenova.cn`（无路径前缀）
- **验证方式**: 推测为 Bearer Token（与 OpenAI 端点相同）
- **说明**:
  - 商汤官方文档和 GitHub 仓库**未明确提及** Anthropic/Claude 协议兼容性
  - preset 中的配置可能来自实际测试或非公开文档
  - 与其他协议对比（如 Kimi 的 `https://api.moonshot.cn/anthropic`），商汤使用根域无前缀的路径较为特殊
- **待确认事项**:
  1. 该端点是否实际可用？
  2. 路径是否应为 `/v1/messages` 或其他？
  3. 是否需要特殊的 API 版本头？

### endpoints 建议配置

保持现状，但建议进行验证测试：

```json
"endpoints": {
  "default": [
    {
      "protocol": "openai",
      "base_url": "https://token.sensenova.cn/v1",
      "client_type": "codex_tui"
    },
    {
      "protocol": "anthropic",
      "base_url": "https://token.sensenova.cn",
      "client_type": "claude_code"
    }
  ]
}
```

## models.default 建议

### 当前配置

```json
"models": {
  "default": {
    "default": "sensenova-6.7-flash-lite"
  }
}
```

### 评估

✅ **合理，建议保持**

- `sensenova-6.7-flash-lite` 是商汤官方推荐的轻量级多模态智能体模型
- 官方定位为"面向真实工作流"，适合编码和日常任务
- Token Plan 中明确将其列为核心模型
- 相比 `sensenova-u1-fast` 更成熟、文档更完善

### 替代方案

如果需要更强的多模态生成能力，可以考虑：
- `sensenova-u1-fast` - 新一代原生多模态模型，理解生成一体

但建议保持 `sensenova-6.7-flash-lite` 作为默认，因为它更稳定、文档更完善。

## 认证方式

### API Key 获取

1. 访问 [SenseNova 控制台](https://platform.sensenova.cn/console)
2. 完成注册和实名认证
3. 管理中心 → API Key 管理 → 创建 API Key

### 鉴权方式

- **方式**: Bearer Token
- **Header**: `Authorization: Bearer <YOUR_API_KEY>`
- **来源**: [官方 API 文档](https://github.com/OpenSenseNova/SenseNova6.7/blob/main/API_CN.md)

### Token Plan（订阅方案）

根据 [Token Plan 页面](https://www.sensenova.cn/token-plan)：

- **Free（公测）**: ¥0/月，每模型 1,500 次调用
  - 包含 `SenseNova 6.7 Flash-Lite` 与 `SenseNova U1 Fast`
  - 最多 20 个 API Key
- **Lite / Pro**: 即将上线

## 结论摘要

### 确认正确的配置

1. ✅ OpenAI 端点：`https://token.sensenova.cn/v1`
2. ✅ 默认模型：`sensenova-6.7-flash-lite`
3. ✅ 模型列表包含：`sensenova-6.7-flash-lite`, `sensenova-u1-fast`

### 需要确认的配置

1. ⚠️ Anthropic 端点：`https://token.sensenova.cn`（官方文档未明确）
2. ⚠️ `deepseek-v4-flash` 是否确实通过商汤 API 提供（第三方转发）

### 建议修改

1. **homepage**: `https://www.sense_time.com` → `https://www.sensenova.cn`（商汤日日新官网）
2. **source_urls.docs**: 考虑添加 GitHub API 文档链接
3. **model_list 排序**: 商汤自研模型优先

### PRD 用一句话总结

商汤日日新（SenseNova）提供 `sensenova-6.7-flash-lite`（默认，轻量多模态智能体）和 `sensenova-u1-fast`（原生多模态）两款 API 模型，OpenAI 兼容端点为 `https://token.sensenova.cn/v1`，可能通过第三方转发提供 `deepseek-v4-flash`。

## 引用来源

- [SenseNova 6.7 GitHub](https://github.com/OpenSenseNova/SenseNova6.7) - API 文档和模型信息
- [SenseNova U1 GitHub](https://github.com/OpenSenseNova/SenseNova-U1) - U1 模型详情
- [商汤日日新官网](https://www.sensenova.cn) - 产品介绍和 Token Plan
- [商汤官网](https://www.sensetime.com) - SenseTime 主站
- [DeepSeek 官网](https://www.deepseek.com) - DeepSeek-V4 信息
- [OpenSenseNova GitHub](https://github.com/OpenSenseNova) - 开源模型列表

## Caveats / Not Found

1. **Anthropic 端点未在官方文档明确** - preset 中的 `https://token.sensenova.cn` anthropic 端点未在官方 API 文档中提及，可能是非公开配置或需要特殊验证
2. **`deepseek-v4-flash` 来源不明** - 该模型明显是 DeepSeek 的产品，在商汤 preset 中的出现推测为第三方转发，但未找到官方说明
3. **旧版 model_list 中的 Nova 命名** - 商汤早期可能使用 Nova 系列命名（如 Nova-Max / Nova-Plus），但当前官方文档未提及，可能是历史遗留
4. **`platform.sensenova.cn/document` 返回 404** - preset 中引用的文档链接无法访问，建议改为 GitHub API 文档链接
