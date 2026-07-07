# Research: Batch 3 - Relay Platforms (A-L)

- **Query**: 第三方中转平台（字母 A-L）端点/模型/价格补全
- **Scope**: 外部调研 + preset 读取
- **Date**: 2026-07-08

## 目标协议

调研字母 A-L 范围内的中转平台协议：
- apikeyfun, apinebula, ccsub, cherryin, claudeapi, claudecn, compshare, compshare_coding, crazyrouter, ctok, eflowcode

---

## Findings

### 1. apikeyfun (APIKEY.FUN)

- **官网**: https://apikey.fun
- **性质**: Claude 中转（仅 Anthropic 协议）
- **支持协议**: anthropic
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://api.apikey.fun",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"claude-opus-4-8"`
- **model_list.default**: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-5"]
- **models.json 价格**:
  | Model ID | Input (¥/1M) | Output (¥/1M) | Cache Write (¥/1M) | Cache Read (¥/1M) |
  |----------|---------------|----------------|---------------------|--------------------|
  | claude-opus-4-8 | 3.50 | 17.50 | 4.38 | 0.35 |
  | claude-sonnet-4-6 | 2.10 | 10.50 | 2.63 | 0.21 |
  | claude-haiku-4-5 | 0.70 | 3.50 | 0.88 | 0.07 |
- **来源**: https://apikey.fun/pricing
- **备注**: 1 RMB = 1 USD 额度，按量付费，永不过期

---

### 2. apinebula (APINebula)

- **官网**: https://apinebula.com
- **性质**: Claude 中转（仅 Anthropic 协议）
- **支持协议**: anthropic
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://apinebula.com",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"claude-opus-4-8"`
- **model_list.default**: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-5"]
- **models.json 价格**:
  | Model ID | Input (¥/1M) | Output (¥/1M) | Cache Write (¥/1M) | Cache Read (¥/1M) |
  |----------|---------------|----------------|---------------------|--------------------|
  | claude-opus-4-8 | 2.50 | 12.50 | 3.125 | 0.250 |
  | claude-opus-4-7 | 2.50 | 12.50 | 3.125 | 0.250 |
  | claude-sonnet-4-6 | 1.50 | 7.50 | 1.875 | 0.150 |
  | claude-haiku-4-5 | 1.70 | 8.50 | 2.125 | 0.170 |
- **来源**: https://apinebula.com/pricing
- **备注**: 价格与官方折扣对比（如 claude-opus-4-8 为 0.74 折）

---

### 3. ccsub (CCSub)

- **官网**: https://www.ccsub.net
- **性质**: 多协议中转
- **支持协议**: anthropic, openai, gemini
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://www.ccsub.net",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"claude-opus-4-8"`
- **model_list.default**: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-5"]
- **models.json 价格**:
  | Model ID | Input ($/1M) | Output ($/1M) | Cache Write ($/1M) | Cache Read ($/1M) |
  |----------|---------------|----------------|---------------------|--------------------|
  | claude-opus-4-8 | 5.00 | 25.00 | 6.25 | 0.50 |
  | claude-sonnet-4-6 | 3.00 | 15.00 | 3.75 | 0.30 |
  | claude-haiku-4-5 | 0.80 | 4.00 | 1.00 | 0.08 |
  | gpt-5.5 | 1.75 | 10.50 | - | - |
  | gemini-2.5-pro | 1.25 | 10.00 | - | - |
- **来源**: https://www.ccsub.net/pricing
- **备注**: 1 RMB = 1 USD 额度，支持 VIP 等级折扣

---

### 4. cherryin (CherryIN)

- **官网**: https://cherryin.net, https://open.cherryin.net
- **性质**: 多协议中转
- **支持协议**: anthropic, openai, gemini, deepseek, glm, kimi
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://open.cherryin.net",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"anthropic/claude-opus-4.8"`
- **model_list.default**: ["anthropic/claude-opus-4.8", "anthropic/claude-sonnet-4.6", "anthropic/claude-opus-4.5", "openai/gpt-5.5", "openai/gpt-5.3-codex", "google/gemini-3.5-flash", "google/gemini-3-pro-preview", "deepseek/deepseek-v4-pro", "deepseek/deepseek-v4-flash", "deepseek/deepseek-v3.2", "agent/glm-5.2", "moonshotai/kimi-k2.7-code", "grok-4"]
- **models.json 价格**: **需要: 用户**（官网无法获取公开价格）
- **来源**: https://open.cherryin.net/
- **备注**: 模型 ID 使用带厂商前缀的格式（如 `anthropic/claude-opus-4.8`）

---

### 5. claudeapi (ClaudeAPI)

- **官网**: https://claudeapi.com
- **性质**: Claude 中转（仅 Anthropic 协议）
- **支持协议**: anthropic
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://gw.claudeapi.com",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"claude-opus-4-7"`
- **model_list.default**: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-5"]
- **models.json 价格**: **需要: 用户**（宣传为官方价 8 折，但无公开明细）
- **来源**: https://claudeapi.com/, https://docs.claudeapi.com/
- **备注**: 宣称 20% 比官方便宜，零数据保留，99.8% SLA

---

### 6. claudecn (ClaudeCN)

- **官网**: https://claudecn.top
- **性质**: Claude 中转（仅 Anthropic 协议）
- **支持协议**: anthropic
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://claudecn.top",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"claude-opus-4-8"`
- **model_list.default**: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-5"]
- **models.json 价格**: **需要: 用户**（价格页面返回空内容）
- **来源**: https://claudecn.top/price
- **备注**: 价格页面无法访问

---

### 7. compshare (Compshare / 优云)

- **官网**: https://www.compshare.cn
- **性质**: 多协议中转（UCloud 优云智算）
- **支持协议**: anthropic, openai, gemini
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://api.modelverse.cn",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: 空
- **model_list.default**: 空（需要通过 `/v1/models` 获取）
- **models.json 价格**: **需要: 用户**（价格列表为 GPU 云服务器价格，非 API 价格）
- **来源**: https://www.compshare.cn/price-list, https://www.compshare.cn/docs/modelverse/models/quick-start
- **备注**: 主要销售 GPU 云服务器和套餐，API 价格不透明

---

### 8. compshare_coding (Compshare Coding Plan)

- **官网**: https://www.compshare.cn
- **性质**: Compshare 编程套餐端点
- **支持协议**: anthropic
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://cp.compshare.cn",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"claude-opus-4-8"`
- **model_list.default**: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-5"]
- **models.json 价格**: **需要: 用户**（套餐价格，无公开单模型价格）
- **来源**: https://www.compshare.cn/price-list
- **备注**: 编程套餐形式（Mini/Lite/Basic/Pro/Max/Ultra），按月订阅

---

### 9. crazyrouter (CrazyRouter)

- **官网**: https://crazyrouter.com
- **性质**: 聚合路由
- **支持协议**: anthropic, openai, gemini（多供应商）
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://cn.crazyrouter.com",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"claude-opus-4-8"`
- **model_list.default**: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-5"]
- **models.json 价格**: **需要: 用户**（模型广场显示 0 模型，需要登录后查看）
- **来源**: https://crazyrouter.com/pricing, https://docs.crazyrouter.com/
- **备注**: 需要登录后通过 `/v1/models` 获取可用模型和价格

---

### 10. ctok (CTok.ai)

- **官网**: https://ctok.ai
- **性质**: Claude Code 教程站 + 中转
- **支持协议**: anthropic
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://api.ctok.ai",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"claude-opus-4-8"`
- **model_list.default**: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-5"]
- **models.json 价格**: **需要: 用户**（主要为 Claude Code 教程站，价格页面 404）
- **来源**: https://ctok.ai/, https://docs.ctok.ai/（404）
- **备注**: 国际站为 etok.ai，价格不公开

---

### 11. eflowcode (E-FlowCode)

- **官网**: https://e-flowcode.cc
- **性质**: AI API 中转平台
- **支持协议**: anthropic, openai（兼容双协议）
- **endpoints.default[]**:
  ```json
  [
    {
      "protocol": "anthropic",
      "base_url": "https://e-flowcode.cc",
      "client_type": "claude_code"
    }
  ]
  ```
- **models.default.default**: `"claude-opus-4-8"`
- **model_list.default**: ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-sonnet-4-5"]
- **models.json 价格**: **需要: 用户**（价格通过分组倍率计算，需登录控制台查看）
- **来源**: https://e-flowcode.cc/, https://e-flowcode.cc/docs/guides/pricing
- **备注**: 使用 New API 模型广场，价格 = 官方价 × 模型倍率 × 分组倍率

---

## 总结

### 有公开价格的协议
1. **apikeyfun** - 人民币定价，按量付费
2. **apinebula** - 人民币定价，折扣公开
3. **ccsub** - 美元定价，1:1 兑换

### 价格不透明的协议（需要用户提供）
1. **cherryin** - 官网无法获取
2. **claudeapi** - 宣传折扣但无明细
3. **claudecn** - 价格页面无法访问
4. **compshare** - GPU 云价格，非 API 价格
5. **compshare_coding** - 套餐订阅
6. **crazyrouter** - 需要登录
7. **ctok** - 价格页面 404
8. **eflowcode** - 分组倍率模式，需登录查看

### 支持多协议的协议
- **ccsub** - anthropic, openai, gemini
- **cherryin** - anthropic, openai, gemini, deepseek, glm, kimi
- **compshare** - anthropic, openai, gemini
- **crazyrouter** - 多供应商聚合
- **eflowcode** - anthropic, openai 兼容

### 仅 Anthropic 的协议
- apikeyfun, apinebula, claudeapi, claudecn, compshare_coding, ctok
