# Research: 百度千帆（Baidu Qianfan）完整模型与端点清单

- **Query**: 百度千帆（Baidu Qianfan）全部官方模型清单 + 端点 + 认证方式
- **Scope**: external（外部文档搜索）
- **Date**: 2026-07-09

## 官方文档源

| URL | 描述 | 可访问性 |
|------|------|----------|
| https://cloud.baidu.com/doc/qianfan-api/ | 千帆 API 文档总览 | 产品页，需登录 |
| https://cloud.baidu.com/doc/qianfan-api/s/dm0g3yofv | 模型列表/在线调用 | 产品页，需登录 |
| https://cloud.baidu.com/doc/qianfan-api/s/Vm0g3yofu | OpenAI 兼容模式（推测） | 未确认 |
| https://cloud.baidu.com/doc/qianfan-api/s/7m0g3yofv | 计费页，模型在售状态 | 产品页，需登录 |
| https://console.bce.baidu.com/qianfan/modelcenter/model/buildIn | 模型广场（控制台） | 需登录 |
| https://qianfan.cloud.baidu.com/doc/ | 千帆文档中心 | 产品页，需登录 |

## 当前 Preset 配置（src-tauri/defaults/platform-presets.json）

```json
{
  "qianfan": {
    "client_type": "default",
    "endpoints": {
      "default": [
        {
          "protocol": "anthropic",
          "base_url": "https://qianfan.baidubce.com/anthropic/coding",
          "client_type": "claude_code"
        }
      ]
    },
    "models": {
      "default": {}
    },
    "model_list": {
      "default": []
    },
    "source_urls": {
      "docs": "https://cloud.baidu.com/doc/qianfan-api/",
      "pricing": "https://cloud.baidu.com/doc/qianfan-api/s/7m0g3yofv"
    },
    "homepage": "https://cloud.baidu.com",
    "logo_url": "baidu"
  }
}
```

## 已确认的千帆平台可用模型

### ERNIE 系列模型（文心大模型）

| 模型 ID | 状态 | 来源 URL | 说明 |
|---------|------|----------|------|
| ERNIE 5.1 | Stable | 控制台 | 搜索能力登顶国内，预训练成本仅为业界6% |
| ERNIE 5.0-正式版 | Stable | 控制台 | 原生全模态大模型，基础能力全面升级 |
| ERNIE 4.5 Turbo VL | Stable | 控制台 | 全新多模理解模型 |
| ERNIE 4.5 Turbo | Stable | 控制台 | 多模态基础大模型 |
| ERNIE X1 Turbo | Stable | 控制台 | 具备更长的思维链，更强的深度思考能力 |
| ERNIE X1.1 | Preview | 控制台 | 在事实性、指令遵循、智能体等能力上均有显著提升 |

### 推测 ERNIE 子型号（需验证）

以下模型 ID 基于 ERNIE 系列命名规范推测，需官方文档验证：

- `ernie-4.5-128k-preview` - ERNIE 4.5 预览版，128K 上下文
- `ernie-4.5-turbo-128k` - ERNIE 4.5 Turbo，128K 上下文
- `ernie-4.5-x1` - ERNIE 4.5 X1 变体
- `ernie-speed-pro-128k` - ERNIE Speed Pro，128K 上下文
- `ernie-speed-128k` - ERNIE Speed，128K 上下文
- `ernie-lite-128k` - ERNIE Lite，128K 上下文
- `ernie-tiny-128k` - ERNIE Tiny，128K 上下文
- `ernie-4.0-8k` / `ernie-4.0-32k` - ERNIE 4.0 系列（可能 deprecated）
- `ernie-3.5-8k` / `ernie-3.5-128k` - ERNIE 3.5 系列（可能 deprecated）

### 千帆平台聚合的第三方模型

| 模型 ID | 状态 | 说明 |
|---------|------|------|
| `deepseek-r1` | Stable | DeepSeek R1 推理模型 |
| `deepseek-v3` | Stable | DeepSeek V3 |
| `deepseek-v4` | Stable | DeepSeek V4（千帆独占？） |
| `glm-5.2` | Stable | 智谱 GLM-5.2，支持 1M 上下文 |
| `kimi-k2.6` | Stable | 月之暗面 Kimi K2.6 |
| `qwen3.5-397b-a17b` | Stable | 通义千问 Qwen3.5 |

## model_list 最终清单

### 主线文本对话模型（建议加入 model_list）

基于 ERNIE 主线模型：

```json
[
  "ernie-5.1",
  "ernie-5.0",
  "ernie-4.5-turbo-vl",
  "ernie-4.5-turbo",
  "ernie-4.5-128k-preview",
  "ernie-4.5-turbo-128k",
  "ernie-4.5-x1",
  "ernie-x1-turbo",
  "ernie-x1.1-preview",
  "ernie-speed-pro-128k",
  "ernie-speed-128k",
  "ernie-lite-128k",
  "ernie-tiny-128k"
]
```

### 第三方聚合模型（可选加入）

```json
[
  "deepseek-r1",
  "deepseek-v3",
  "deepseek-v4",
  "glm-5.2",
  "kimi-k2.6",
  "qwen3.5-397b-a17b"
]
```

**注意**：以上模型 ID 大部分基于命名规范推测，需官方文档验证实际 API 调用名称。

## models.default.default 推荐

**推荐值**：`ernie-4.5-turbo` 或 `ernie-5.1`

**理由**：
- ERNIE 4.5 Turbo 是当前主推的高性能模型，支持长上下文和多模态
- ERNIE 5.1 是最新旗舰模型，搜索能力突出
- 需验证实际 API 调用名称（可能需要加版本号前缀如 `ERNIE-4.5-Turbo-128K`）

## endpoints

### 当前配置

| Protocol | Base URL | Client Type | 状态 |
|----------|----------|-------------|------|
| anthropic | https://qianfan.baidubce.com/anthropic/coding | claude_code | 现有 |

### 待验证端点

#### OpenAI 兼容端点（未确认）

| Base URL | Protocol | 认证 | 状态 |
|----------|----------|------|------|
| https://qianfan.baidubce.com/v1 | openai | ? | 404（测试于 2026-07-09） |
| https://qianfan.baidubce.com/v2 | openai | ? | 未测试 |

**结论**：`/v1` 端点返回 404，千帆可能**没有**公开的 OpenAI 兼容端点，或路径不同。

#### 普通 Anthropic 端点（未确认）

| Base URL | Protocol | 用途 | 状态 |
|----------|----------|------|------|
| https://qianfan.baidubce.com/anthropic | anthropic | 普通 Anthropic 兼容 | 未确认 |
| https://qianfan.baidubce.com/anthropic/coding | anthropic | Coding Plan 套餐 | 现有 |

#### 国际端点

| Base URL | 用途 | 状态 |
|----------|------|------|
| 未发现 | 国际端点 | 未确认 |

**结论**：千帆可能仅提供国内端点，未发现独立的国际端点。

## 认证方式

### 推测：API Key 直连（主流方式）

基于主流 LLM 平台模式，千帆很可能支持：
- **API Key**：通过 `Authorization: Bearer <api_key>` 头部传递
- **应用场景**：anthropic coding 端点最可能使用此方式

### 可能的旧模式：AK/SK + access_token

百度智能云传统认证方式：
- **AK/SK**：Access Key ID / Secret Access Key
- **access_token**：OAuth 2.0 token
- **适用场景**：可能是旧版千帆 API 的认证方式

**未确认**：千帆是否已完全迁移到 API Key 直连，还是仍保留 AK/SK 旧模式。

### Coding Plan 套餐端点认证

`/anthropic/coding` 端点的认证方式：
- **推测**：使用 API Key（Bearer token）
- **验证方式**：需实际调用测试

## 非主线模型区（不并入 model_list）

### ERNIE Vision 系列

- `ernie-4.5-vl` - ERNIE 4.5 Turbo VL（已列入主线？）
- `ernie-vision-*` - 其他视觉模型变体

### TTS / 语音模型

- 端到端语音语言大模型
- 大模型语音合成
- 大模型声音复刻

### Embedding 模型

- ERNIE Embedding 系列模型（文本嵌入）

### PaddleOCR 系列

- `PaddleOCR-VL` - 文档解析模型
- `PP-StructureV3` - 高效文档解析模型

## 排除项与原因

### 已 Deprecated 的模型

| 模型 | 原因 | 下线公告 |
|------|------|----------|
| ERNIE 3.x 系列 | 旧版本，被 4.x/5.x 取代 | 未找到官方公告 |
| ERNIE 4.0 早期版本 | 被 4.5 取代 | 未找到官方公告 |

**注意**：未找到具体的下线公告文档，以上基于模型演进逻辑推测。

## Caveats / 需要 main 关注

### 关键未决问题

1. **千帆 API key 直连 vs AK/SK 旧模式**
   - 需确认 `/anthropic/coding` 端点是否已支持 API Key 直连
   - 或者仍需使用 AK/SK + access_token 流程

2. **OpenAI 兼容端点是否存在**
   - `/v1` 端点测试返回 404
   - 可能没有 OpenAI 兼容端点，或使用不同路径

3. **实际模型 ID 格式**
   - 控制台显示的模型名称（如 "ERNIE 4.5 Turbo"）与实际 API 调用的 model id（如 `ernie-4.5-turbo-128k`）可能不同
   - 需官方文档或实际调用验证

4. **Coding Plan 端点支持的模型子集**
   - `/anthropic/coding` 端点是否仅支持特定模型？
   - 是否为普通端点模型全集的子集？

5. **国际端点**
   - 未发现千帆有独立的国际端点
   - 国内端点可能需要国内网络访问

### 推测项（需验证）

- ERNIE 子型号的具体 ID 格式（如 `ernie-4.5-128k-preview` vs `ERNIE-4.5-128K-PREVIEW`）
- 第三方模型在千帆平台的准确 ID
- 是否有其他协议端点（如 Gemini 兼容）

## 后续行动建议

1. **获取官方技术文档**
   - 尝试登录千帆控制台查看 API 文档
   - 联系百度技术支持获取端点和认证信息

2. **实际调用测试**
   - 测试 `/anthropic/coding` 端点的认证方式
   - 验证模型 ID 的正确格式
   - 测试是否有其他端点可用

3. **关注官方公告**
   - 监控千帆官方博客/文档更新
   - 关注模型上线/下线公告
