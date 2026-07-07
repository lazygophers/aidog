# Research: 原厂协议端点补全（Batch 1）

- **Query**: 查 15 个原厂大模型平台的官网，补全 preset 数据（端点/模型/价格）
- **Scope**: 外部调研 + 现状对比
- **Date**: 2026-07-08

## 研究目标

确认各平台是否支持 OpenAI 兼容端点，补全 `platform-presets.json` 中的 `endpoints.default[]` 字段。

## Findings

### stepfun（阶跃星辰）

- **官网**: https://platform.stepfun.com/docs
- **支持协议**: anthropic（已有）, **openai（需新增）**
- **endpoints.default[]**:
  - ```json
    {protocol: "anthropic", base_url: "https://api.stepfun.com/step_plan", client_type: "claude_code"}
    ```
  - **需新增**:
    ```json
    {protocol: "openai", base_url: "https://api.stepfun.com/v1", client_type: "codex_tui"}
    ```
- **models.default.default**: `"step-3.7-flash"`（已有）
- **model_list.default**: `["step-3.7-flash", "step-3.5-flash"]`（已有）
- **models.json 价格**:
  - `step-3.7-flash`: {default_platform: "stepfun", input_cost_per_token: 0.00019, output_cost_per_token: 0.00114}
    - 换算: 1.35元/M tokens ÷ 7 ≈ 0.193美元/M tokens, 8.1元/M tokens ÷ 7 ≈ 1.157美元/M tokens
  - `step-3.5-flash`: {default_platform: "stepfun", input_cost_per_token: 0.00010, output_cost_per_token: 0.00030}
    - 换算: 0.7元/M tokens ÷ 7 ≈ 0.10美元/M tokens, 2.1元/M tokens ÷ 7 ≈ 0.30美元/M tokens
- **来源**: https://platform.stepfun.com/docs/zh/guides/developer/openai
- **备注**: OpenAI 兼容端点文档已确认，base_url 为 `https://api.stepfun.com/v1`

---

### siliconflow（硅基流动）

- **官网**: https://docs.siliconflow.cn/
- **支持协议**: anthropic（已有）, **openai（需确认）**
- **endpoints.default[]**:
  - ```json
    {protocol: "anthropic", base_url: "https://api.siliconflow.cn", client_type: "claude_code"}
    ```
  - **需新增**:
    - **需要: 用户** - 文档显示有 OpenAI 兼容 chat completions API，但 base_url 需要确认（可能是 `https://api.siliconflow.cn/v1` 或类似）
- **models.default.default**: 需确认
- **model_list.default**: 需确认（从价格页面看有多个模型可用）
- **models.json 价格**: 需确认各模型价格（价格页显示多平台模型价格表）
- **来源**: https://docs.siliconflow.cn/cn/api-reference/chat-completions/chat-completions
- **备注**: 价格页显示托管了多家平台模型（Stepfun、Z-ai、Kimi、DeepSeek、MiniMax、Qwen、Baidu 等）

---

### modelscope（魔搭）

- **官网**: https://www.modelscope.cn/docs
- **支持协议**: anthropic（已有）, **openai（需确认）**
- **endpoints.default[]**:
  - ```json
    {protocol: "anthropic", base_url: "https://api-inference.modelscope.cn", client_type: "claude_code"}
    ```
  - **需确认**: 是否有 OpenAI 兼容端点
- **models.default.default**: 需确认
- **model_list.default**: `["deepseek-ai/DeepSeek-V4-Pro", "deepseek-ai/DeepSeek-V4-Flash", ...]`（已有）
- **models.json 价格**: **需要: 用户** - 价格页可能有验证码，无法直接抓取
- **来源**: https://www.modelscope.cn/docs/model-service/API-Inference/intro
- **备注**: 价格页面可能有验证码保护，需要用户手动确认

---

### qianfan（百度千帆）

- **官网**: https://cloud.baidu.com/doc/qianfan-api/
- **支持协议**: anthropic（已有）
- **endpoints.default[]**:
  - ```json
    {protocol: "anthropic", base_url: "https://qianfan.baidubce.com/anthropic/coding", client_type: "claude_code"}
    ```
  - **需确认**: 是否有 OpenAI 兼容端点
- **models.default.default**: 需确认
- **model_list.default**: 需确认
- **models.json 价格**: 需确认
- **来源**: https://cloud.baidu.com/doc/qianfan-api/s/7m0g3yofv
- **备注**: 价格页面结构复杂，需要进一步解析

---

### bailing（百灵 TBox）

- **官网**: https://www.tbox.cn/
- **支持协议**: anthropic（已有）
- **endpoints.default[]**:
  - ```json
    {protocol: "anthropic", base_url: "https://api.tbox.cn/api/anthropic", client_type: "claude_code"}
    ```
  - **需确认**: 是否有 OpenAI 兼容端点
- **models.default.default**: 需确认
- **model_list.default**: 需确认
- **models.json 价格**: **需要: 用户** - 官网主要是产品页面，API 文档可能需要登录
- **来源**: https://www.tbox.cn/
- **备注**: TBox 是一个产品而非开放 API 平台，可能没有公开的 API 文档

---

### bailian_coding（百炼编程）

- **官网**: https://help.aliyun.com/zh/model-studio/developer-reference/use-claude-code
- **支持协议**: anthropic（已有）, **openai（需确认）**
- **endpoints.default[]**:
  - ```json
    {protocol: "anthropic", base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic", client_type: "claude_code"}
    ```
  - **需确认**: 是否有 OpenAI 兼容端点
- **models.default.default**: `"qwen3-coder-plus"`（已有）
- **model_list.default**: `["qwen3-coder-plus", "qwen3-coder-flash", "qwen3.7-max"]`（已有）
- **models.json 价格**: 需确认（与 bailian 共用计费系统）
- **来源**: https://help.aliyun.com/zh/model-studio/billing-for-model-studio
- **备注**: 与 bailian（千问）共用阿里云计费系统

---

### byteplus（BytePlus）

- **官网**: https://www.byteplus.com
- **支持协议**: anthropic（已有）, **openai（需确认）**
- **endpoints.default[]**:
  - ```json
    {protocol: "anthropic", base_url: "https://ark.ap-southeast.bytepluses.com/api/coding", client_type: "claude_code"}
    ```
  - **需确认**: 是否有 OpenAI 兼容端点（可能与 doubao 类似）
- **models.default.default**: `"doubao-seed-2-0-pro"`（已有）
- **model_list.default**: `["doubao-seed-2-0-pro", "doubao-seed-2-0-code-preview", ...]`（已有）
- **models.json 价格**: 需确认（与 doubao 共用火山引擎计费）
- **来源**: https://www.volcengine.com/docs/82379
- **备注**: BytePlus 是火山引擎的国际版，与 doubao 共用底层模型

---

### sensenova（商汤 SenseNova）

- **官网**: https://platform.sensenova.cn/document
- **支持协议**: openai（已有）, anthropic（已有）
- **endpoints.default[]**:
  - ```json
    {protocol: "openai", base_url: "https://token.sensenova.cn/v1", client_type: "codex_tui"}
    {protocol: "anthropic", base_url: "https://token.sensenova.cn", client_type: "claude_code"}
    ```
- **状态**: 端点已完整（2 个）
- **models.default.default**: `"sensenova-6.7-flash-lite"`（已有）
- **model_list.default**: `["sensenova-6.7-flash-lite", "deepseek-v4-flash", "sensenova-u1-fast"]`（已有）
- **models.json 价格**: **需要: 用户** - docs.platform.sensenova.cn/document 返回 404
- **来源**: 404 错误
- **备注**: 文档链接失效，需要用户确认新的文档地址

---

### longcat（龙猫）

- **官网**: https://longcat.chat/
- **支持协议**: anthropic（已有）, openai（已有）
- **endpoints.default[]**:
  - ```json
    {protocol: "anthropic", base_url: "https://api.longcat.chat/anthropic", client_type: "claude_code"}
    {protocol: "openai", base_url: "https://api.longcat.chat/openai/v1", client_type: "codex_tui"}
    ```
- **状态**: 端点已完整（2 个）
- **models.default.default**: 需确认
- **model_list.default**: 需确认
- **models.json 价格**: **需要: 用户** - 首页主要是产品介绍，无 API 文档
- **来源**: https://longcat.chat/
- **备注**: 首页没有公开的 API 文档链接

---

### xiaomi_mimo（小米 MiMo）

- **官网**: https://mimo.xiaomi.com/
- **支持协议**: anthropic（已有）, openai（已有）
- **endpoints.default[]**:
  - ```json
    {protocol: "anthropic", base_url: "https://api.xiaomimimo.com/anthropic", client_type: "claude_code"}
    {protocol: "openai", base_url: "https://api.xiaomimimo.com/v1", client_type: "codex_tui"}
    ```
- **状态**: 端点已完整（2 个）
- **models.default.default**: `"mimo-v2.5-pro"`（已有）
- **model_list.default**: `["mimo-v2.5-pro", "mimo-v2-pro", "mimo-v2.5", "mimo-v2-omni", "mimo-v2-flash"]`（已有）
- **models.json 价格**: 需确认（首页主要是产品介绍，无 API 文档）
- **来源**: https://mimo.xiaomi.com/
- **备注**: 首页没有公开的 API 文档链接

---

## 价格汇总表

| 协议 | 模型 | 输入价格（美元/M tokens） | 输出价格（美元/M tokens） | 来源 |
|------|------|---------------------------|---------------------------|------|
| stepfun | step-3.7-flash | 0.00019 | 0.00114 | https://platform.stepfun.com/docs/zh/guides/pricing/details |
| stepfun | step-3.5-flash | 0.00010 | 0.00030 | 同上 |
| bailian | qwen3.7-max | 0.00171 (12元/M) | 0.00514 (36元/M) | https://help.aliyun.com/zh/model-studio/billing-for-model-studio |
| doubao | doubao-seed-2.0-pro | 0.00046 (3.2元/M, [0,32k]区间) | 0.00229 (16元/M) | https://www.volcengine.com/docs/82379/1544106 |
| doubao | doubao-seed-2.0-lite | 0.00009 (0.6元/M, [0,32k]区间) | 0.00051 (3.6元/M) | 同上 |
| doubao | doubao-seed-2.0-mini | 0.00003 (0.2元/M, [0,32k]区间) | 0.00029 (2元/M) | 同上 |

*注: 汇率按 1 美元 = 7 人民币计算*

---

## 需要用户确认的事项

1. **SiliconFlow OpenAI 兼容端点的 base_url** - 文档显示有 chat completions API，但未明确 base_url
2. **ModelScope 价格信息** - 价格页可能有验证码保护
3. **Bailing TBox API 文档** - 官网主要是产品页面，可能没有公开 API
4. **Qianfan OpenAI 兼容端点** - 需确认是否有
5. **SenseNova API 文档地址** - 原 docs.platform.sensenova.cn/document 返回 404
6. **Longcat/MiMo 价格信息** - 首页无公开 API 文档

---

## 后续建议

1. 对于端点不全的平台，建议用户直接联系平台技术支持确认 API 端点
2. 对于价格信息缺失的平台，可能需要登录控制台查看
3. 建议补全 stepfun 的 OpenAI 兼容端点到 preset 文件
