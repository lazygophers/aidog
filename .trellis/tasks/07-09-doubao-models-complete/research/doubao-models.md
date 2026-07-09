# Research: 火山引擎方舟（Volcengine Ark / 豆包 doubao）模型清单

- **Query**: 字节火山引擎方舟（Volcengine Ark / 豆包 doubao）全部官方模型清单 + models.default + byteplus 国际版
- **Scope**: 外部文档 + 本仓 preset 配置
- **Date**: 2026-07-09

## 官方文档源

### 国内版（doubao 协议）

| 文档类型 | URL | 说明 |
|---------|-----|------|
| 总入口 | https://www.volcengine.com/docs/82379 | 方舟模型接入文档总览 |
| 模型列表 | https://www.volcengine.com/docs/82379/1330310?lang=zh | 完整模型 ID 清单 |
| 模型价格 | https://www.volcengine.com/docs/82379/1544106?lang=zh | 在售状态与价格 |
| Seed 2.1 | https://www.volcengine.com/docs/82379/2549861?lang=zh | 最新模型详情 |
| 套餐概览 | https://www.volcengine.com/docs/82379/2366394?lang=zh | Agent Plan 套餐支持模型 |
| 接入三方工具 | https://www.volcengine.com/docs/82379/2160841?lang=zh | 端点协议说明 |
| API Key 管理 | https://console.volcengine.com/ark/region:ark+cn-beijing/apiKey | 获取 API Key |
| 签名鉴权 | https://www.volcengine.com/docs/82379/1465834?lang=zh | AK/SK 签名鉴权 |

### 国际版（byteplus 协议）

| 文档类型 | URL | 说明 |
|---------|-----|------|
| 总入口 | https://docs.byteplus.com/en/docs/ModelArk | ModelArk 文档总览 |
| 产品概览 | https://docs.byteplus.com/en/docs/ModelArk/1099455 | ModelArk 产品简介 |
| API Key | https://docs.byteplus.com/en/docs/ModelArk/1541594 | 获取 API Key |
| 模型价格 | https://docs.byteplus.com/en/docs/ModelArk/1544106 | 国际版模型价格 |
| AI 模型 | https://ai.byteplus.com/model | BytePlus AI 模型列表 |
| Coding Plan | https://ai.byteplus.com/activity/codingplan | Coding Plan 套餐 |

## doubao 协议（国内版）

### model_list 最终清单

#### 字节自营 doubao 系列（文本对话/code 模型）

##### 最新推荐模型（Stable）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `doubao-seed-evolving` | Stable | 快速迭代模型，周级更新，持续进化 Coding 与 Agent 能力 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |
| `doubao-seed-2-1-pro-260628` | Stable | Seed 2.1 旗舰版，面向高复杂度任务 | [Seed 2.1 文档](https://www.volcengine.com/docs/82379/2549861) |
| `doubao-seed-2-1-turbo-260628` | Stable | Seed 2.1 Turbo，效果与成本均衡 | [Seed 2.1 文档](https://www.volcengine.com/docs/82379/2549861) |

##### Seed 2.0 系列（Stable）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `doubao-seed-2-0-code` | Stable | Seed 2.0 Code 编程模型 | [模型价格](https://www.volcengine.com/docs/82379/1544106) |
| `doubao-seed-2-0-pro` | Stable | Seed 2.0 Pro 通用模型 | [模型价格](https://www.volcengine.com/docs/82379/1544106) |
| `doubao-seed-2-0-lite` | Stable | Seed 2.0 Lite 标准模型 | [模型价格](https://www.volcengine.com/docs/82379/1544106) |
| `doubao-seed-2-0-mini` | Stable | Seed 2.0 Mini 极速模型 | [模型价格](https://www.volcengine.com/docs/82379/1544106) |

##### Seed 1.x 系列（Legacy）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `doubao-seed-code` | Stable | Seed Code 编程模型 | [模型价格](https://www.volcengine.com/docs/82379/1544106) |
| `doubao-seed-character` | Stable | Seed Character 角色对话模型 | [模型价格](https://www.volcengine.com/docs/82379/1544106) |
| `doubao-seed-1.8` | Stable | Seed 1.8 通用模型 | [模型价格](https://www.volcengine.com/docs/82379/1544106) |
| `doubao-seed-1.6` | Stable | Seed 1.6 基础模型 | [模型价格](https://www.volcengine.com/docs/82379/1544106) |

##### Deprecated（即将下线，排除）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `doubao-seed-1-8-251228` | Deprecated | Seed 1.8 特定版本，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |
| `doubao-seed-code-preview-251028` | Deprecated | Seed Code Preview，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |
| `doubao-seed-1-6-flash-250828` | Deprecated | Seed 1.6 Flash，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |
| `doubao-seed-1-6-vision-250815` | Deprecated | Seed 1.6 Vision，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |
| `doubao-seed-1-6-251015` | Deprecated | Seed 1.6 特定版本，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |
| `doubao-seed-1-6-250615` | Deprecated | Seed 1.6 特定版本，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |
| `doubao-1-5-pro-32k-250115` | Deprecated | 豆包 1.5 Pro 32K，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |
| `doubao-1-5-pro-32k-character-250715` | Deprecated | 豆包 1.5 Pro Character，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |

#### 方舟聚合第三方模型（方舟平台可调用）

##### Stable（在售）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `minimax-m2.7` | Stable | MiniMax M2.7 模型 | [套餐概览](https://www.volcengine.com/docs/82379/2366394) |
| `minimax-m3` | Stable | MiniMax M3 模型 | [套餐概览](https://www.volcengine.com/docs/82379/2366394) |
| `glm-5.2` | Stable | 智谱 GLM-5.2 (glm-latest 别名) | [套餐概览](https://www.volcengine.com/docs/82379/2366394) |
| `deepseek-v4-flash` | Stable | DeepSeek V4 Flash（尝鲜体验版） | [套餐概览](https://www.volcengine.com/docs/82379/2366394) |
| `deepseek-v4-pro` | Stable | DeepSeek V4 Pro（尝鲜体验版） | [套餐概览](https://www.volcengine.com/docs/82379/2366394) |
| `kimi-k2.6` | Stable | 月之暗面 Kimi K2.6 | [套餐概览](https://www.volcengine.com/docs/82379/2366394) |
| `kimi-k2.7-code` | Stable | 月之暗面 Kimi K2.7 Code | [套餐概览](https://www.volcengine.com/docs/82379/2366394) |

##### Deprecated（即将下线，排除）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `glm-4-7-251222` | Deprecated | GLM-4.7 特定版本，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |
| `deepseek-v3-2-251201` | Deprecated | DeepSeek V3.2，即将下线 | [模型列表](https://www.volcengine.com/docs/82379/1330310) |

### models.default.default 推荐

#### 当前 preset 值
`doubao-seed-2-0-code`

#### 推荐更新为
`doubao-seed-2-0-code`（保持不变）或 `doubao-seed-evolving`

#### 推荐理由
1. **doubao-seed-2-0-code** 是当前 preset 配置的默认值，针对 Coding 场景优化，价格适中（3.2-9.6 元/百万 token 输入，16-48 元/百万 token 输出），上下文窗口 256k。
2. **doubao-seed-evolving** 是最新快速迭代模型，周级更新，当前版本与 doubao-seed-2.1-pro 相同，适合追求最新能力的用户，但价格较高（6 元/百万 token 输入，30 元/百万 token 输出）。
3. 若用户需要更强能力且预算充足，可选 `doubao-seed-2-1-pro-260628` 或 `doubao-seed-2-1-turbo-260628`。

#### 首选建议
**保持 `doubao-seed-2-0-code`** 作为默认值，理由是性价比高、针对 Coding 场景优化、且已在 preset 中稳定使用。

### endpoints（端点说明）

#### 套餐端点（Agent/Coding Plan）

| 协议 | Base URL | 用途 | 套餐专属 |
|------|----------|------|---------|
| Anthropic | `https://ark.cn-beijing.volces.com/api/plan` | Claude Code 等 Anthropic 协议工具 | 是 |
| OpenAI | `https://ark.cn-beijing.volces.com/api/plan/v3` | Codex CLI、Cursor 等 OpenAI 协议工具 | 是 |
| OpenAI_Responses | `https://ark.cn-beijing.volces.com/api/plan/v3` | OpenAI Responses API | 是 |

#### 普通端点（按量后付费）

| 协议 | Base URL | 用途 |
|------|----------|------|
| Anthropic | `https://ark.cn-beijing.volces.com/api/compatible` | Claude Code 等 Anthropic 协议工具 |
| OpenAI | `https://ark.cn-beijing.volces.com/api/v3` | OpenAI 协议工具 |

#### 套餐端点模型范围限制
根据 [套餐概览](https://www.volcengine.com/docs/82379/2366394)，Agent Plan 支持的模型为套餐表格中所列模型（约 15 个文本生成模型 + 多模态模型），**是普通端点模型全集的子集**。普通端点支持所有语言模型，可按需选择。

#### preset 当前配置
preset 中 `doubao` 协议的 `endpoints.default` 使用**套餐端点**（/api/plan 和 /api/plan/v3），这是面向套餐用户的配置。若用户未购买套餐，应使用普通端点（/api/compatible 和 /api/v3）。

### 认证方式

#### 1. API Key（Bearer Token）
- **适用场景**：常规 API 调用
- **获取方式**：[控制台 API Key 管理](https://console.volcengine.com/ark/region:ark+cn-beijing/apiKey)
- **使用方式**：HTTP Header `Authorization: Bearer $ARK_API_KEY`

#### 2. 签名鉴权（AK/SK）
- **适用场景**：企业级用户，需要更高安全性
- **文档**：[签名鉴权与调用示例](https://www.volcengine.com/docs/82379/1465834)
- **说明**：使用火山引擎 Access Key ID 和 Secret Access Key 进行请求签名

#### 3. Vaults 认证
- **适用场景**：Managed Agents，托管第三方凭据
- **文档**：[使用 Vaults 认证](https://www.volcengine.com/docs/82379/2553726)
- **说明**：一次性注册终端用户的第三方凭据，Session 级引用

#### API Key 类型
- **普通 API Key**：用于按量后付费调用
- **Agent Plan API Key**：套餐专属，用于套餐端点调用（/api/plan）
- **Coding Plan API Key**：套餐专属，用于 Coding Plan 套餐

## byteplus 协议（国际版）

### 模型 ID 格式差异说明

**重要差异**：BytePlus 国际版使用 `seed-` 前缀的模型 ID（如 `seed-2-0-pro`），而国内版使用 `doubao-seed-` 前缀（如 `doubao-seed-2-0-pro`）。两者指向同一模型系列，但 ID 格式不同。

### model_list 最终清单

#### 字节自营 Seed 系列（文本对话/code 模型）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `seed-2-0-pro` | Stable | Seed 2.0 Pro 通用模型 | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |
| `seed-2-0-code-preview` | Stable | Seed 2.0 Code Preview 编程模型 | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |
| `seed-2-0-lite` | Stable | Seed 2.0 Lite 标准模型 | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |
| `seed-2-0-mini` | Stable | Seed 2.0 Mini 极速模型 | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |
| `seed-1-8` | Stable | Seed 1.8 通用模型 | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |
| `seed-1-6` | Stable | Seed 1.6 基础模型 | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |

**注意**：国际版 preset 当前仅包含 4 个模型（seed-2-0-pro, seed-2-0-code-preview, seed-2-0-lite, seed-2-0-mini），但定价页面显示还有 seed-1-8 和 seed-1-6 可用。

#### 方舟聚合第三方模型（国际版）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `glm-5-2` | Stable | 智谱 GLM-5.2 | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |
| `deepseek-v4-pro` | Stable | DeepSeek V4 Pro | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |
| `deepseek-v4-flash` | Stable | DeepSeek V4 Flash | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |

**关键差异**：国际版聚合第三方模型范围**小于国内版**，国内版包含 minimax 和 kimi 系列，但国际版定价页面未显示这些模型。国际版仅包含 glm-5.2、deepseek-v4-pro、deepseek-v4-flash。

#### Deprecated（国际版即将下线）

| 模型 ID | 状态 | 说明 | 出处 |
|---------|------|------|------|
| `glm-4-7` | Deprecated | GLM-4.7，即将下线 | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |
| `deepseek-v3-2` | Deprecated | DeepSeek V3.2，即将下线 | [模型价格](https://docs.byteplus.com/en/docs/ModelArk/1544106) |

### models.default.default 推荐

#### 当前 preset 值
`doubao-seed-2-0-pro`（**注意**：preset 使用的是国内版 ID 格式）

#### 推荐更新为
`seed-2-0-pro` 或保持 `doubao-seed-2-0-pro`（需验证国际端点是否兼容国内版 ID 格式）

#### 推荐理由
1. **seed-2-0-pro** 是国际版主推通用模型，价格适中（0.50-1.00 USD/百万 token 输入，3.00-6.00 USD/百万 token 输出）。
2. 国际版 preset 当前使用 `doubao-seed-2-0-pro`（国内版 ID 格式），这可能是一个错误或兼容性配置，需要验证国际端点是否支持。

#### 首选建议
**更新为 `seed-2-0-pro`**（国际版标准 ID 格式），同时建议验证 `doubao-seed-2-0-pro` 是否在国际端点可用。

### endpoints（端点说明）

#### 当前 preset 配置

| 协议 | Base URL | 用途 |
|------|----------|------|
| Anthropic | `https://ark.ap-southeast.bytepluses.com/api/coding` | Claude Code 等 Anthropic 协议工具 |
| OpenAI | `https://ark.ap-southeast.bytepluses.com/api/plan/v3` | OpenAI 协议工具 |
| OpenAI_Responses | `https://ark.ap-southeast.bytepluses.com/api/plan/v3` | OpenAI Responses API |

#### 端点差异（国际版 vs 国内版）

| 对比项 | 国内版（doubao） | 国际版（byteplus） |
|-------|-----------------|-------------------|
| 域名 | `ark.cn-beijing.volces.com` | `ark.ap-southeast.bytepluses.com` |
| Anthropic 路径 | `/api/plan` | `/api/coding` |
| OpenAI 路径 | `/api/plan/v3` | `/api/plan/v3`（相同） |
| 套餐支持 | Agent Plan / Coding Plan | Coding Plan / ArkClaw |

**关键差异**：国际版 Anthropic 端点路径为 `/api/coding`（而非国内的 `/api/plan`），名称更明确指向 Coding 场景。

### 认证方式

#### API Key（Bearer Token）
- **适用场景**：常规 API 调用
- **获取方式**：[BytePlus 控制台](https://console.byteplus.com/ark/region:ark+ap-southeast-1/apiKey)
- **使用方式**：HTTP Header `Authorization: Bearer $BYTEPLUS_API_KEY`
- **与国内版互通性**：**不互通**，需要分别在国际版和国内版控制台获取 API Key

#### API Key 类型
- **普通 API Key**：用于按量后付费调用
- **Coding Plan API Key**：套餐专属，用于 Coding Plan 套餐

## 国内版 vs 国际版关键差异

### 模型清单差异

| 对比项 | 国内版（doubao） | 国际版（byteplus） |
|-------|-----------------|-------------------|
| 字节自营模型数量 | 11 个（含 evolving、2.1、2.0、1.x） | 6 个（仅 2.0、1.x，无 evolving/2.1） |
| 模型 ID 格式 | `doubao-seed-*` | `seed-*` |
| 第三方聚合范围 | minimax、glm、deepseek、kimi（7 个） | glm、deepseek（3 个） |
| 是否聚合 MiniMax | 是 | 否 |
| 是否聚合 Kimi | 是 | 否 |

**结论**：国际版模型清单是**国内版的子集**，且不包含最新模型（Seed 2.1、Evolving）。

### 端点差异

| 对比项 | 国内版（doubao） | 国际版（byteplus） |
|-------|-----------------|-------------------|
| 域名 | `ark.cn-beijing.volces.com` | `ark.ap-southeast.bytepluses.com` |
| Anthropic 路径 | `/api/plan` | `/api/coding` |
| OpenAI 路径 | `/api/plan/v3` | `/api/plan/v3` |
| 普通端点（Anthropic） | `/api/compatible` | 推测：`/api/compatible`（需验证） |
| 普通端点（OpenAI） | `/api/v3` | 推测：`/api/v3`（需验证） |

### 聚合第三方模型差异

| 第三方模型 | 国内版（doubao） | 国际版（byteplus） |
|-----------|-----------------|-------------------|
| MiniMax | `minimax-m2.7`、`minimax-m3` | ❌ 无 |
| GLM | `glm-5.2` | `glm-5-2` |
| DeepSeek | `deepseek-v4-flash`、`deepseek-v4-pro` | `deepseek-v4-flash`、`deepseek-v4-pro` |
| Kimi | `kimi-k2.6`、`kimi-k2.7-code` | ❌ 无 |

**结论**：国际版**仅聚合 GLM 和 DeepSeek**，不包含 MiniMax 和 Kimi。

### 价格差异

| 模型 | 国内版价格（CNY/百万 token） | 国际版价格（USD/百万 token） |
|------|----------------------------|---------------------------|
| seed-2.0-pro 输入 | 3.2-9.6 | 0.50-1.00 |
| seed-2.0-pro 输出 | 16.0-48.0 | 3.00-6.00 |
| seed-2.0-lite 输入 | 0.6-1.8 | 0.25-0.50 |
| seed-2.0-lite 输出 | 3.6-10.8 | 2.00-4.00 |

**注意**：国际版使用美元计价，国内版使用人民币计价。汇率换算后，国际版价格可能略高于或略低于国内版，取决于当前汇率。

## caveats / 需要 main 关注

### 国内版（doubao）

1. **套餐端点 vs 普通端点**：preset 中 doubao 协议使用套餐端点（/api/plan），若用户未购买套餐，调用会失败。需要考虑是否同时提供普通端点配置，或在文档中说明套餐要求。

2. **方舟聚合第三方模型稳定性**：第三方模型（minimax、glm、deepseek、kimi）由方舟平台聚合，模型 ID 可能随平台更新而变动。需要定期验证模型可用性。

3. **Evolving 模型自动升级**：`doubao-seed-evolving` 是周级迭代模型，版本自动升级，无需切换模型 ID，但能力可能随版本变化。

4. **DeepSeek 模型限流**：`deepseek-v4-flash` 和 `deepseek-v4-pro` 为尝鲜体验版，文档提示可能访问拥堵或频繁限流，建议有备用模型。

5. **第三方模型抵扣系数优惠**：2026-06-10 至 2026-07-15 期间，Agent/Coding Plan 个人版使用 `deepseek-v4-pro`、`kimi-k2.6`、`kimi-k2.7-code`、`glm-5.2` 时享最低 2.5 折优惠（限时活动）。

6. **套餐模型范围**：Agent Plan 套餐支持的模型是普通端点全集的子集（约 15 个文本生成模型），若需调用其他模型，需使用普通端点。

7. **API Key 互通性**：套餐 API Key 仅能用于套餐端点，普通 API Key 仅能用于普通端点，二者不互通。

### 国际版（byteplus）

1. **模型 ID 格式不兼容**：国际版使用 `seed-*` 格式，国内版使用 `doubao-seed-*` 格式。preset 中 byteplus 协议使用 `doubao-seed-2-0-pro`（国内版格式），需要验证是否在国际端点可用，或更新为 `seed-2-0-pro`。

2. **模型范围更窄**：国际版仅包含 6 个字节自营模型 + 3 个第三方模型，是国内版的子集。无 Seed 2.1、Evolving 等最新模型。

3. **第三方模型缺失**：国际版不聚合 MiniMax 和 Kimi，仅聚合 GLM 和 DeepSeek。

4. **端点路径差异**：Anthropic 端点使用 `/api/coding`（非 `/api/plan`），需要确保 preset 配置正确。

5. **API Key 不互通**：国际版 API Key 与国内版 API Key 不互通，需要分别获取。

6. **普通端点未验证**：国际版普通端点（/api/compatible、/api/v3）未在文档中明确说明，需要验证是否可用。

7. **preset 模型 ID 可能错误**：preset 中 byteplus 协议的 model_list 使用 `doubao-seed-*` 格式（国内版），可能需要更新为 `seed-*` 格式（国际版）。

## preset 配置建议

### doubao 协议（国内版）建议更新

#### model_list.default 建议更新
```json
"model_list": {
  "default": [
    // 字节自营 doubao 系列（文本对话/code）
    "doubao-seed-evolving",
    "doubao-seed-2-1-pro-260628",
    "doubao-seed-2-1-turbo-260628",
    "doubao-seed-2-0-code",
    "doubao-seed-2-0-pro",
    "doubao-seed-2-0-lite",
    "doubao-seed-2-0-mini",
    "doubao-seed-code",
    "doubao-seed-character",
    "doubao-seed-1.8",
    "doubao-seed-1.6",
    // 方舟聚合第三方模型
    "minimax-m2.7",
    "minimax-m3",
    "glm-5.2",
    "deepseek-v4-flash",
    "deepseek-v4-pro",
    "kimi-k2.6",
    "kimi-k2.7-code"
  ]
}
```

#### models.default.default 建议保持
```json
"models": {
  "default": {
    "default": "doubao-seed-2-0-code"
  }
}
```

### byteplus 协议（国际版）建议更新

#### model_list.default 建议更新
```json
"model_list": {
  "default": [
    // 字节自营 Seed 系列（国际版 ID 格式）
    "seed-2-0-pro",
    "seed-2-0-code-preview",
    "seed-2-0-lite",
    "seed-2-0-mini",
    "seed-1-8",
    "seed-1-6",
    // 方舟聚合第三方模型（国际版）
    "glm-5-2",
    "deepseek-v4-pro",
    "deepseek-v4-flash"
  ]
}
```

**注意**：当前 preset 使用 `doubao-seed-*` 格式，需要更新为 `seed-*` 格式，并验证国际端点是否兼容。

#### models.default.default 建议更新
```json
"models": {
  "default": {
    "default": "seed-2-0-pro"
  }
}
```

**变更原因**：
1. 国际版标准 ID 格式为 `seed-*`
2. 当前 preset 使用 `doubao-seed-2-0-pro`（国内版格式），可能不兼容
3. `seed-2-0-pro` 是国际版主推通用模型

#### endpoints 建议保持
当前配置已正确，使用国际端点（ark.ap-southeast.bytepluses.com）和正确路径（/api/coding, /api/plan/v3）。
