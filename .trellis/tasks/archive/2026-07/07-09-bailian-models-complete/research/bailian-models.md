# Research: 阿里云百炼（Bailian/DashScope）模型与端点调研

- **Query**: 调研阿里云百炼（Bailian/DashScope）全部官方模型清单 + 端点 + 认证方式
- **Scope**: 外部文档 + 本仓预设配置
- **Date**: 2026-07-09

## 官方文档源

| 类别 | URL | 说明 |
|------|-----|------|
| 模型选择总览 | https://help.aliyun.com/zh/model-studio/getting-started/models | Qwen 全系列模型概览 |
| API 调用参考 | https://help.aliyun.com/zh/model-studio/developer-reference/use-qwen-by-calling-api | Qwen API 调用，含模型 tab |
| OpenAI 兼容 | https://help.aliyun.com/zh/model-studio/developer-reference/compatibility-of-openai-with-dashscope | OpenAI 兼容模式端点与模型列表 |
| Anthropic 兼容 | https://help.aliyun.com/zh/model-studio/developer-reference/anthropic-api-messages | Anthropic Messages API 兼容（推测，文档未直接访问到） |
| 计费价格 | https://help.aliyun.com/zh/model-studio/billing-for-model-studio | 模型调用计费，含在售状态（2026-07-07 更新） |
| Claude Code 使用 | https://help.aliyun.com/zh/model-studio/developer-reference/use-claude-code | Claude Code 接入文档（推测路径） |
| 国际站（英文） | https://www.alibabacloud.com/help/en/model-studio/developer-reference/compatibility-of-openai-with-dashscope | 国际站端点与模型 |

## 认证方式

### API Key 获取

- 从百炼控制台获取 API Key：https://help.aliyun.com/zh/model-studio/get-api-key
- **重要**：各地域（北京、弗吉尼亚、新加坡）的 API Key 不同

### 认证方式

- **OpenAI 兼容端点**：Authorization Bearer `{API_KEY}`
- **Anthropic 兼容端点**：Authorization Bearer `{API_KEY}`（推测，与 Anthropic 协议一致）
- **DashScope 原生端点**：Authorization Bearer `{API_KEY}`

无需额外 header。API Key 需配置到环境变量 `DASHSCOPE_API_KEY` 以降低泄露风险。

## endpoints

### OpenAI 兼容端点

| 地域 | BASE_URL（SDK） | HTTP endpoint | 备注 |
|------|-----------------|---------------|------|
| 北京（华北2） | `https://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/compatible-mode/v1` | `POST https://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/compatible-mode/v1/chat/completions` | 推荐迁移至新域名 |
| 弗吉尼亚（美东） | `https://dashscope-us.aliyuncs.com/compatible-mode/v1` | `POST https://dashscope-us.aliyuncs.com/compatible-mode/v1/chat/completions` | 国际站 |
| 新加坡 | `https://{WorkspaceId}.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1` | `POST https://{WorkspaceId}.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1/chat/completions` | 推荐迁移至新域名 |
| 日本（东京） | `https://{WorkspaceId}.ap-northeast-1.maas.aliyuncs.com/compatible-mode/v1` | `POST https://{WorkspaceId}.ap-northeast-1.maas.aliyuncs.com/compatible-mode/v1/chat/completions` |  |

**说明**：
- `{WorkspaceId}` 为业务空间 ID，可在百炼控制台的「业务空间详情」页面查看
- 旧域名 `https://dashscope.aliyuncs.com/compatible-mode/v1` 仍可使用
- 旧域名 `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` 仍可使用（新加坡）

### Anthropic 兼容端点

| 端点类型 | base_url | 协议 | client_type | 出处 |
|----------|----------|------|-------------|------|
| 通用 | `https://dashscope.aliyuncs.com/apps/anthropic` | anthropic | claude_code | preset 配置 |
| 编程 | `https://coding.dashscope.aliyuncs.com/apps/anthropic` | anthropic | claude_code | preset 配置（bailian_coding） |

**说明**：
- 编程端点 `coding.dashscope.aliyuncs.com` 用于 Coding Plan 相关调用
- 端点路径可能与 Anthropic 原生协议有差异，需实测验证

### 国内 vs 国际站差异

- **国内站**：dashscope.aliyuncs.com / {WorkspaceId}.cn-beijing.maas.aliyuncs.com
- **国际站**：dashscope-us.aliyuncs.com（弗吉尼亚）/ dashscope-intl.aliyuncs.com（新加坡）
- 国际站文档：https://www.alibabacloud.com/help/en/model-studio/

## model_list 最终清单（主线文本对话 + Coder）

### 主线文本对话模型（Qwen Max/Plus/Flash/Turbo/Long）

**Max 系列**（最强推理，支持思考模式）：

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `qwen3.7-max` | Stable | 当前能力等同于 qwen3.7-max-2026-05-20，支持非思考/思考模式 | 计费页 |
| `qwen3.7-max-2026-06-08` | Stable | 特定版本快照 | 计费页 |
| `qwen3.7-max-2026-05-20` | Stable | 特定版本快照 | 计费页 |
| `qwen3.7-max-preview` | Preview | 仅思考模式，等同于 qwen3.7-max-2026-05-17 | 计费页 |
| `qwen3.7-max-2026-05-17` | Stable | 特定版本快照（preview 基准） | 计费页 |
| `qwen3.6-max-preview` | Stable | 支持非思考/思考模式，0-128K/128K-256K 阶梯计费 | 计费页 |
| `qwen3-max` | Stable | 等同于 qwen3-max-2026-01-23，支持非思考/思考模式，多阶梯（32K/128K/256K） | 计费页 |
| `qwen3-max-2026-01-23` | Stable | 特定版本快照 | 计费页 |
| `qwen3-max-preview` | Stable | 支持非思考/思考模式，多阶梯 | 计费页 |
| `qwen-max` | Stable | 仅非思考模式，无阶梯计费 | 计费页 |

**Plus 系列**（性价比）：

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `qwen3.7-plus` | Stable | 等同于 qwen3.7-plus-2026-05-26，支持非思考/思考模式 | 计费页 |
| `qwen3.7-plus-2026-05-26` | Stable | 特定版本快照 | 计费页 |
| `qwen3.6-plus` | Stable | 等同于 qwen3.6-plus-2026-04-02 | 计费页 |
| `qwen3.6-plus-2026-04-02` | Stable | 特定版本快照 | 计费页 |
| `qwen3.5-plus` | Stable | 等同于 qwen3.5-plus-2026-02-15 | 计费页 |
| `qwen3.5-plus-2026-04-20` | Stable | 特定版本快照 | 计费页 |
| `qwen-plus` | Stable | 等同于 qwen-plus-2025-12-01 | 计费页 |
| `qwen-plus-latest` | Stable | 最新别名 | 计费页 |
| `qwen-plus-2025-12-01` | Stable | 特定版本快照 | 计费页 |
| `qwen-plus-2025-09-11` | Stable | 特定版本快照 | 计费页 |

**Flash 系列**（极速推理）：

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `qwen3.6-flash` | Stable | 等同于 qwen3.6-flash-2026-04-16 | 计费页 |
| `qwen3.6-flash-2026-04-16` | Stable | 特定版本快照 | 计费页 |
| `qwen3.5-flash` | Stable | 等同于 qwen3.5-flash-2026-02-23 | 计费页 |
| `qwen3.5-flash-2026-02-23` | Stable | 特定版本快照 | 计费页 |
| `qwen-flash` | Stable | 等同于 qwen-flash-2025-07-28 | 计费页 |
| `qwen-flash-2025-07-28` | Stable | 特定版本快照 | 计费页 |

**Turbo 系列**：

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `qwen-turbo` | Stable | 基础推理模型 | 计费页 |

**Long 系列**（长上下文）：

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `qwen-long` | Stable | 长上下文模型 | 计费页 |
| `qwen-long-latest` | Stable | 最新别名 | 计费页 |
| `qwen-long-2025-01-25` | Stable | 特定版本快照 | 计费页 |

**Omni 系列**（全模态，含实时）：

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `qwen3.5-omni-plus` | Stable | 等同于 qwen3.5-omni-plus-2026-03-15 | 计费页 |
| `qwen3.5-omni-plus-2026-03-15` | Stable | 特定版本快照 | 计费页 |
| `qwen3.5-omni-flash` | Stable | 等同于 qwen3.5-omni-flash-2026-03-15 | 计费页 |
| `qwen3.5-omni-flash-2026-03-15` | Stable | 特定版本快照 | 计费页 |
| `qwen3.5-omni-plus-realtime` | Stable | 实时语音/视频 | 计费页 |
| `qwen3.5-omni-plus-realtime-2026-03-15` | Stable | 特定版本快照 | 计费页 |
| `qwen3.5-omni-flash-realtime` | Stable | 实时语音/视频 | 计费页 |
| `qwen3.5-omni-flash-realtime-2026-03-15` | Stable | 特定版本快照 | 计费页 |

### Coder 系列（代码生成与补全）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `qwen3-coder-plus` | Stable | 等同于 qwen3-coder-plus-2025-09-23，主力代码模型 | 计费页 |
| `qwen3-coder-plus-2025-09-23` | Stable | 特定版本快照 | 计费页 |
| `qwen3-coder-plus-2025-07-22` | Stable | 特定版本快照 | 计费页 |
| `qwen3-coder-flash` | Stable | 等同于 qwen3-coder-flash-2025-07-28，极速代码模型 | 计费页 |
| `qwen3-coder-flash-2025-07-28` | Stable | 特定版本快照 | 计费页 |
| `qwen-coder-plus` | Stable | 旧版 Coder Plus（推测为 Qwen2.5 Coder） | 计费页 |
| `qwen-coder-turbo` | Stable | 旧版 Coder Turbo（推测为 Qwen2.5 Coder Turbo） | 计费页 |

### 推荐的精简 model_list

基于当前 preset 和在售状态，推荐以下精简清单（去重带日期快照）：

```json
[
  "qwen3.7-max",
  "qwen3.7-plus",
  "qwen3.6-flash",
  "qwen3.5-flash",
  "qwen-plus",
  "qwen-turbo",
  "qwen-long",
  "qwen3.5-omni-plus",
  "qwen3-coder-plus",
  "qwen3-coder-flash",
  "qwen-coder-plus",
  "qwen-coder-turbo"
]
```

**说明**：
- 主线文本对话：max/plus/flash/turbo/long/omni 各选最新版
- Coder：qwen3 系列（新版）+ qwen 系列（旧版兼容）
- 去除所有日期后缀快照版本
- Omni 单独列出（全模态）

## models.default.default 推荐

**当前推荐**：`qwen3.7-max`

**理由**：
- preset 现状（src-tauri/defaults/platform-presets.json）已配置 `qwen3.7-max`
- 计费页显示 qwen3.7-max 为当前主推 Max 系列最新版
- 支持非思考/思考模式，能力强
- 支持 Batch 调用半价、上下文缓存折扣

**备选**：
- `qwen3.7-plus`：性价比更高，适合大部分场景
- `qwen3.6-flash`：极速推理，适合实时响应场景
- `qwen3-coder-plus`：代码场景专用

## 非主线模型区

以下模型不并入主线 `model_list`，仅记录：

### Qwen-VL（视觉理解）

| 模型 ID | 说明 |
|---------|------|
| `qwen3-vl-plus` | Qwen3 VL Plus，等同于 qwen3-vl-plus-2025-12-19 |
| `qwen3-vl-plus-2025-12-19` | 特定版本快照 |
| `qwen3-vl-plus-2025-09-23` | 特定版本快照 |
| `qwen3-vl-flash` | Qwen3 VL Flash，等同于 qwen3-vl-flash-2026-01-22 |
| `qwen3-vl-flash-2026-01-22` | 特定版本快照 |
| `qwen3-vl-flash-2025-10-15` | 特定版本快照 |
| `qwen-vl-max` | 旧版 VL Max |
| `qwen-vl-plus` | 旧版 VL Plus |
| `qwen3.5-ocr` | OCR 专用 |
| `qwen-vl-ocr` | OCR 通用，等同于 qwen-vl-ocr-2025-11-20 |
| `qwen-vl-ocr-latest` | OCR 最新别名 |
| `qwen-vl-ocr-2025-11-20` | 特定版本快照 |

**说明**：
- VL 系列为视觉理解模型，需图片输入
- OCR 系列为文字识别专用

### Qwen-Audio（语音）

| 模型 ID | 说明 |
|---------|------|
| `qwen-audio-turbo` | 语音理解/生成（免费额度用完后不可调用，推荐使用 Qwen-Omni 作为替代） |
| `qwen-audio-turbo-latest` | 最新别名 |

**说明**：
- Qwen-Audio 不支持 OpenAI 兼容协议，仅支持 DashScope 协议
- 官方推荐迁移至 Qwen-Omni

### Qwen-Math（数学）

| 模型 ID | 说明 |
|---------|------|
| `qwen-math-plus` | 数学推理 Plus |
| `qwen-math-turbo` | 数学推理 Turbo |

### MT 系列及其他

| 模型 ID | 说明 |
|---------|------|
| `qwen-mt-plus` | 机器翻译 Plus |
| `qwen-mt-flash` | 机器翻译 Flash |
| `qwen-mt-lite` | 机器翻译 Lite |
| `qwen-mt-turbo` | 机器翻译 Turbo |
| `qwen-doc-turbo` | 文档理解 Turbo |
| `qwen-deep-research` | 深度研究 |
| `qwen-deep-research-2025-12-15` | 特定版本快照 |
| `tongyi-xiaomi-analysis-flash` | 小米分析 Flash |

### 开源系列（Qwen3.6/3.5/3）

**Qwen3.6**：
- `qwen3.6-35b-a3b`
- `qwen3.6-27b`

**Qwen3.5**：
- `qwen3.5-397b-a17b`
- `qwen3.5-122b-a10b`
- `qwen3.5-27b`
- `qwen3.5-35b-a3b`

**Qwen3**：
- `qwen3-next-80b-a3b-thinking`
- `qwen3-next-80b-a3b-instruct`
- `qwen3-235b-a22b-thinking-2507`
- `qwen3-235b-a22b-instruct-2507`
- `qwen3-30b-a3b-thinking-2507`
- `qwen3-30b-a3b-instruct-2507`

**说明**：
- 开源系列计费页有列，但可能为社区模型托管
- 部分模型命名带 `-thinking` 后缀，为思维链版本
- 部分模型带 `a3b`/`a22b`/`a10b` 等标签，可能为不同训练批次

## 排除项与原因

### 已下线/退役模型

- **Qwen-Audio**：免费额度用完后不可调用，官方推荐使用 Qwen-Omni 作为替代
- 其他已下线模型需查「模型下线公告」，未在本次调研范围内

### 不纳入主线的原因

- **Qwen-VL 系列**：视觉模型，需图片输入，与文本对话场景不同
- **Qwen-Audio 系列**：语音模型，已退役
- **Qwen-Math 系列**：数学专用，非通用对话
- **MT/Doc/Deep Research 系列等**：专用场景，非通用对话
- **日期快照版本**：与主版本能力相同，仅用于固定版本，去重

### 可能遗漏的模型

根据任务要求，以下模型需进一步确认是否在售：

- **Qwen3 Coder 其他版本**：是否存在 qwen3-coder-plus-2025-xx-xx、qwen3-coder-flash-2025-xx-xx 等更多版本
- **Qwen3.7 Coder**：是否存在 qwen3.7-coder-plus、qwen3.7-coder-flash
- **Qwen3.5 Max**：是否存在 qwen3.5-max 系列在售
- **Qwen2.5 Coder**：旧版 Coder 是否仍有在售版本

**建议**：
- 直接访问百炼控制台查看完整模型列表
- 对比计费页与控制台列表差异

## caveats / 需要 main 关注

### 1. Anthropic 兼容端点文档未直接获取

- `https://help.aliyun.com/zh/model-studio/developer-reference/anthropic-api-messages` 返回阿里云首页而非具体文档
- 推测路径可能已变更或需特殊权限
- 建议 main 实测验证 `https://dashscope.aliyuncs.com/apps/anthropic` 和 `https://coding.dashscope.aliyuncs.com/apps/anthropic` 的可用性

### 2. 国际站端点

- 国际站文档确认存在：dashscope-us.aliyuncs.com（弗吉尼亚）/ dashscope-intl.aliyuncs.com（新加坡）
- 是否需要单独配置 `bailian_intl` 协议待确认
- preset 当前未包含国际站配置

### 3. WorkspaceId 参数

- 新域名需要 `{WorkspaceId}` 参数，用户需在控制台查看
- 旧域名仍可用，但官方推荐迁移至新域名以获得更好性能和稳定性
- preset 配置需考虑是否支持 WorkspaceId 替换

### 4. 计费页更新频率

- 计费页更新时间为 2026-07-07，相对较新
- 模型上下架频繁，需定期同步
- 建议订阅百炼产品动态或定期检查计费页

### 5. 模型版本命名规则

- 存在 `qwen3.7-max-2026-05-20`（日期快照）和 `qwen3.7-max-preview`（预览）两种命名模式
- 部分模型带 `-thinking` 后缀（思维链版本）
- 开源模型带 `a3b`/`a22b` 等批次标签
- 建议统一命名规则文档化

### 6. OpenAI 兼容支持差异

- Qwen-Audio 不支持 OpenAI 兼容协议，仅支持 DashScope 协议
- 部分专用模型（MT/Doc/Deep Research）可能也存在协议限制
- 需逐模型验证 OpenAI 兼容性

### 7. Coding Plan 与 Coding 端点

- `bailian_coding` 协议使用 `https://coding.dashscope.aliyuncs.com/apps/anthropic`
- 编程端点与通用端点差异需实测验证
- Coding Plan 计费模式与普通模型可能不同

## 推测: 未决项

以下内容因文档未直接获取，标注为推测：

1. **Anthropic 兼容端点完整路径**：推测为 `/apps/anthropic`，需实测
2. **编程端点路径**：推测与 Anthropic 兼容端点一致，需实测
3. **国际站是否支持 Anthropic 兼容**：推测支持，需验证
4. **WorkspaceId 对国际站是否生效**：推测国际站无 WorkspaceId 概念

## 相关 Spec

- `.trellis/spec/` - 相关规范文档（若有）

## 当前 preset 状态

来源：`src-tauri/defaults/platform-presets.json` line 508-564

```json
"bailian": {
  "endpoints": {
    "default": [
      {
        "protocol": "openai",
        "base_url": "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "client_type": "codex_tui"
      },
      {
        "protocol": "anthropic",
        "base_url": "https://dashscope.aliyuncs.com/apps/anthropic",
        "client_type": "claude_code"
      }
    ]
  },
  "models": {
    "default": {
      "default": "qwen3.7-max"
    }
  },
  "model_list": {
    "default": [
      "qwen3.7-max",
      "qwen3.7-plus",
      "qwen3.6-flash",
      "qwen3.5-omni-plus",
      "qwen3-coder-plus",
      "qwen3-coder-flash"
    ]
  }
}
```

## 建议更新内容

基于本次调研，建议更新 preset 如下：

### 新增模型

```json
"model_list": {
  "default": [
    "qwen3.7-max",
    "qwen3.7-plus",
    "qwen3.6-flash",
    "qwen3.5-flash",
    "qwen3.5-omni-plus",
    "qwen-plus",
    "qwen-turbo",
    "qwen-long",
    "qwen3-coder-plus",
    "qwen3-coder-flash",
    "qwen-coder-plus",
    "qwen-coder-turbo"
  ]
}
```

### 新增端点（可选）

考虑添加国际站端点配置或 WorkspaceId 支持。

---

**调研完成时间**: 2026-07-09
**下次建议更新**: 2026-08-09（每月检查计费页更新）

---

## bailian_coding 协议（Coding Plan 套餐）

### 协议概述

`bailian_coding` 协议是阿里云百炼的 Coding Plan 套餐专用端点，面向编程场景优化，提供 Anthropic 兼容的 Messages API。

**协议定位**：
- 独立于 `bailian` 协议的编程专用配置
- 使用专用域名 `coding.dashscope.aliyuncs.com`
- 面向 Claude Code / Cursor / Codex 等 Claude 生态工具

### endpoints

| 端点类型 | base_url | protocol | client_type | 出处 |
|----------|----------|----------|-------------|------|
| 编程专用 | `https://coding.dashscope.aliyuncs.com/apps/anthropic` | anthropic | claude_code | preset 配置 |

**说明**：
- 编程端点路径为 `/apps/anthropic`，与 Anthropic Messages API 一致
- 专用域名 `coding.dashscope.aliyuncs.com` 区别于通用端点 `dashscope.aliyuncs.com`
- 是否支持 OpenAI 兼容端点（`/compatible-mode/v1`）**未决**，需实测验证

### model_list 最终清单（Coding Plan 套餐限定模型）

**当前 preset 配置**（来源：`src-tauri/defaults/platform-presets.json` line 566-615）：

```json
"model_list": {
  "default": [
    "qwen3-coder-plus",
    "qwen3-coder-flash",
    "qwen3.7-max"
  ]
}
```

**模型清单**：

| 模型 ID | 状态 | 说明 | 计费页出处 |
|---------|------|------|-----------|
| `qwen3-coder-plus` | Stable | 主力代码模型，等同于 qwen3-coder-plus-2025-09-23 | 计费页 |
| `qwen3-coder-flash` | Stable | 极速代码模型，等同于 qwen3-coder-flash-2025-07-28 | 计费页 |
| `qwen3.7-max` | Stable | 通用推理模型，支持思考模式 | 计费页 |

**说明**：
- Coding Plan 套餐的模型清单为普通 bailian 模型的**子集**
- 套餐聚焦 Coder 系列代码模型，外加通用推理模型 `qwen3.7-max`
- 是否包含其他 Coder 变体（如 `qwen-coder-plus`/`qwen-coder-turbo`）**未决**，需套餐文档确认

### models.default.default 推荐

**当前推荐**：`qwen3-coder-plus`

**理由**：
- preset 现状配置 `qwen3-coder-plus`
- Coding Plan 套餐面向编程场景，优先推荐代码专用模型
- `qwen3-coder-plus` 为当前主力代码模型，能力更强

**备选**：
- `qwen3-coder-flash`：极速代码场景，延迟更低
- `qwen3.7-max`：通用推理场景，需要非代码任务时使用

### 套餐认证差异

**API Key**：
- Coding Plan 套餐 API Key 与普通 bailian **推测通用**（同一百炼账号）
- 各地域（北京、弗吉尼亚、新加坡）的 API Key 不同

**认证方式**：
- Authorization Bearer `{API_KEY}`
- 与普通 bailian 端点一致，无需额外 header 或套餐标识

**推测**：
- 套餐身份可能通过端点域名（`coding.dashscope.aliyuncs.com`）区分
- 无需额外 `X-Coding-Plan` 等 header
- 需实测验证套餐 API Key 是否独立

### 套餐端点 vs 普通端点的差异

| 对比项 | 普通端点 | 编程端点 |
|--------|----------|----------|
| 域名 | `dashscope.aliyuncs.com` | `coding.dashscope.aliyuncs.com` |
| 协议 | OpenAI + Anthropic | Anthropic |
| 路径 | `/compatible-mode/v1` + `/apps/anthropic` | `/apps/anthropic` |
| 模型范围 | 全集（Max/Plus/Flash/Turbo/Long/Omni/Coder 等） | 子集（Coder 系列 + Max） |
| 计费模式 | 按量计费 / 资源包 | Coding Plan 套餐（推测） |
| 目标场景 | 通用对话 | 编程专用（Claude Code / Cursor） |

**模型可用性差异**：

- **套餐专属**：`qwen3-coder-plus`、`qwen3-coder-flash`（在套餐内可能有优惠）
- **通用可用**：`qwen3.7-max`（套餐与普通端点均可调用）
- **普通端点专属**：其他非代码模型（VL/Audio/Math 等）套餐端点不可用

**说明**：
- 套餐模型为普通 bailian 模型的**严格子集**
- 套餐端点可能返回 403/400 用于非套餐模型
- 需实测验证 `qwen-coder-plus`/`qwen-coder-turbo` 在套餐端点的可用性

### 套餐支持的模型（详细对比）

**在套餐端点可调用**：
- `qwen3-coder-plus`（主力代码模型）
- `qwen3-coder-flash`（极速代码模型）
- `qwen3.7-max`（通用推理）
- 推测可能支持：`qwen-coder-plus`、`qwen-coder-turbo`（需实测）

**仅在普通端点可调用**：
- `qwen3.7-plus`、`qwen3.6-flash`、`qwen-plus`、`qwen-turbo`、`qwen-long` 等非代码模型
- `qwen3.5-omni-plus`、`qwen3.5-omni-flash` 等 Omni 系列
- `qwen3-vl-plus`、`qwen3-vl-flash` 等 VL 系列
- `qwen-audio-turbo` 等 Audio 系列

### Caveats / 未决项

1. **套餐官方文档未直接获取**：
   - `https://help.aliyun.com/zh/model-studio/developer-reference/use-claude-code` 返回首页
   - Coding Plan 概述页存在但内容未完整解析
   - 套餐计费页、支持模型页路径未确认

2. **套餐模型清单完整性**：
   - 是否包含 `qwen-coder-plus`/`qwen-coder-turbo`（旧版 Coder）
   - 是否包含其他 Coder 变体（如 `qwen3-coder-turbo`，如果存在）
   - `qwen3.7-max` 是否为套餐内唯一非 Coder 模型

3. **套餐端点 OpenAI 兼容性**：
   - `https://coding.dashscope.aliyuncs.com/compatible-mode/v1` 是否存在
   - 如果存在，支持哪些模型

4. **套餐 API Key 独立性**：
   - 套餐是否需要单独开通并获取专用 API Key
   - 套餐 API Key 是否与普通 bailian 通用

5. **端点路径验证**：
   - `/apps/anthropic` 是否为完整路径或需要额外前缀
   - 是否支持 `/v1/messages` 等 Anthropic 标准路径

6. **计费模式差异**：
   - Coding Plan 套餐是否为固定费率 vs 普通端点按量计费
   - 套餐是否包含免费额度或配额

### 推测: 未决项

以下内容因文档未直接获取，标注为推测：

1. **套餐文档 URL**：
   - 推测套餐概览：`https://help.aliyun.com/zh/model-studio/coding-plan`
   - 推测套餐计费：`https://help.aliyun.com/zh/model-studio/coding-plan/billing`
   - 推测支持模型：`https://help.aliyun.com/zh/model-studio/coding-plan/supported-models`

2. **套餐与普通端点关系**：
   - 推测套餐端点为普通端点的**子集**，仅允许 Coder 系列 + Max
   - 推测套餐身份通过域名区分，无需额外 header
   - 推测套餐 API Key 与普通端点通用

3. **OpenAI 兼容端点**：
   - 推测 `https://coding.dashscope.aliyuncs.com/compatible-mode/v1` 可能存在
   - 推测即使存在，套餐模型可能仅支持 Anthropic 协议

### 相关文档 URL（待验证）

- 套餐概览：https://help.aliyun.com/zh/model-studio/coding-plan
- Claude Code 使用：https://help.aliyun.com/zh/model-studio/developer-reference/use-claude-code
- Anthropic 兼容：https://help.aliyun.com/zh/model-studio/developer-reference/anthropic-api-messages
- 编程计费：https://help.aliyun.com/zh/model-studio/coding-plan/billing-for-coding-plan（推测）

### 当前 preset 状态（bailian_coding）

来源：`src-tauri/defaults/platform-presets.json` line 566-615

```json
"bailian_coding": {
  "endpoints": {
    "default": [
      {
        "protocol": "anthropic",
        "base_url": "https://coding.dashscope.aliyuncs.com/apps/anthropic",
        "client_type": "claude_code"
      }
    ]
  },
  "models": {
    "default": {
      "default": "qwen3-coder-plus"
    }
  },
  "model_list": {
    "default": [
      "qwen3-coder-plus",
      "qwen3-coder-flash",
      "qwen3.7-max"
    ]
  }
}
```

### 建议更新内容（bailian_coding）

基于现有信息，bailian_coding 协议配置基本合理，建议：

1. **验证模型清单**：确认是否需要添加 `qwen-coder-plus`/`qwen-coder-turbo`
2. **验证端点路径**：实测 `/apps/anthropic` 是否为完整路径
3. **文档化套餐差异**：明确套餐端点 vs 普通端点的模型可用性差异

---

**调研更新时间**: 2026-07-09
**bailian_coding 协议状态**: 配置基本合理，需实测验证端点可用性
