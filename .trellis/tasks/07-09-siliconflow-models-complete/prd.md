# 补全 siliconflow(+siliconflow_en) model_list+endpoints 全部官方信息

## Goal

硅基流动 SiliconFlow。一站式模型推理云服务平台（类 OpenRouter，偏国内/开源主力），托管多厂商开源 + 商业模型（Qwen / DeepSeek / GLM / Kimi / MiniMax 等 60+ 对话模型），OpenAI 兼容 + Anthropic 双协议。国内 siliconflow（.cn）+ 国际 siliconflow_en（.com）同源镜像，仅域名异。preset 现 endpoints 仅 1 anthropic 端点（缺 openai /v1），model_list + models.default 均空。需补 openai 端点 + 20 主流对话模型 + 3 档默认（default/coder/fast）。desc/source_urls 准确，保留。

## Research References

- [`research/siliconflow-models.md`](research/siliconflow-models.md) — 双协议端点路径（.cn/.com 同构）+ 60+ 对话模型清单（6 大厂商）+ model_list 只含 chat 类型（排除 embedding/reranker/TTS/图像生成）+ 国际版完全复用国内版 model_list 结论

## Requirements

### 1. endpoints（每协议块 2 端点：补 openai + 保留 anthropic）

现仅 1 anthropic 端点（根域），需补 openai 兼容端点（research line 67-85 明确建议）。anthropic base_url 按 preset 约定保持仅 host（task 全局约定：anthropic/gemini 仅 host，openai 带 /v1；Claude Code 配置 `ANTHROPIC_BASE_URL` 用根域带尾斜杠，路由层自动补路径）。

**siliconflow（国内 .cn）**：
```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.siliconflow.cn", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.siliconflow.cn/v1", "client_type": "codex_tui"}
  ]
}
```

**siliconflow_en（国际 .com）**：域名替换 .cn→.com，结构完全一致。
```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.siliconflow.com", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.siliconflow.com/v1", "client_type": "codex_tui"}
  ]
}
```

### 2. model_list.default（20 主流对话模型，两协议同清单）

research line 294-319 建议清单（覆盖 Qwen / DeepSeek / GLM / Kimi / MiniMax / 腾讯 / 字节 / 百度 / Meta / Google / OpenAI OSS / 美团 12 厂商），仅 chat 类型（排除 embedding/reranker/TTS/图像生成/视频）。格式 `厂商/模型名`。

```json
"model_list": {
  "default": [
    "Qwen/Qwen2.5-72B-Instruct",
    "Qwen/Qwen3.5-27B",
    "Qwen/Qwen3.5-9B",
    "deepseek-ai/DeepSeek-V4-Flash",
    "deepseek-ai/DeepSeek-V3.2",
    "deepseek-ai/DeepSeek-R1",
    "zai-org/GLM-4.7",
    "zai-org/GLM-4.6",
    "moonshotai/Kimi-K2-Instruct-0905",
    "moonshotai/Kimi-K2.6",
    "MiniMaxAI/MiniMax-M2.5",
    "tencent/Hunyuan-A13B-Instruct",
    "ByteDance-Seed/Seed-OSS-36B-Instruct",
    "Qwen/Qwen3-Coder-30B-A3B-Instruct",
    "meta-llama/Meta-Llama-3.1-8B-Instruct",
    "google/gemma-4-26B-A4B-it",
    "openai/gpt-oss-120b",
    "Qwen/Qwen2.5-Coder-32B-Instruct",
    "baidu/ERNIE-4.5-300B-A47B",
    "inclusionAI/Ling-flash-2.0"
  ]
}
```

完整枚举约 60+（国际版文档 enum），上表为主流覆盖。siliconflow_en 完全复用此清单（research line 380-389 确认两版模型清单无差异）。

### 3. models.default（3 档：default / coder / fast，两协议同）

```json
"models": {
  "default": {
    "default": "Qwen/Qwen2.5-72B-Instruct",
    "coder": "Qwen/Qwen3-Coder-30B-A3B-Instruct",
    "fast": "deepseek-ai/DeepSeek-V4-Flash"
  }
}
```

档位选择理由（task 全局 slot 映射：coder/codex→coder，flash/mini/nano→fast，主力兜底→default）：
- `default`：Qwen/Qwen2.5-72B-Instruct（官方文档常用示例，性价比高，通用对话主流）
- `coder`：Qwen/Qwen3-Coder-30B-A3B-Instruct（编程专用 Coder 模型）
- `fast`：deepseek-ai/DeepSeek-V4-Flash（带 Flash 后缀，推理强，Claude Code 官方合作示例）

### 4. desc（保留，准确）

现有 desc 准确描述平台定位，无需改写：
- siliconflow zh: 硅基流动（siliconflow.cn）推理与模型 API / en: SiliconFlow (siliconflow.cn) inference and model API
- siliconflow_en zh: 硅基流动国际版（siliconflow.com）端点 / en: SiliconFlow international (siliconflow.com) endpoint

### 5. source_urls（保留，正确）

research 确认 docs.siliconflow.cn / docs.siliconflow.com / siliconflow.{cn|com}/pricing 均正确，保留。

## Acceptance Criteria

- [ ] siliconflow + siliconflow_en 各补 openai 端点（/v1）+ 保留 anthropic 端点（host）
- [ ] model_list.default 各 20 模型（两协议同清单，JSON 合法 + 无重复）
- [ ] models.default 各 3 档（default=Qwen2.5-72B-Instruct / coder=Qwen3-Coder-30B / fast=DeepSeek-V4-Flash）
- [ ] desc 保留
- [ ] source_urls 保留
- [ ] JSON 合法
- [ ] 仅改 siliconflow + siliconflow_en 协议块

## Out of Scope

- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan 分支
- 完整 60+ 模型枚举（需认证调 /v1/models，仅列 20 主流覆盖）
- embedding/reranker/TTS/图像生成/视频模型（非 chat 对话）
- Pro/ 前缀版本（research 标含义不明，推测增强版）
- pricing 字段补全（独立 task）
- 其他协议块改动

## Technical Notes

- 真值源：`protocols.siliconflow` + `protocols.siliconflow_en`（同源镜像，仅域名 .cn vs .com）
- 数据来源：research/siliconflow-models.md（国际版 API 文档 enum 枚举 + 国内版文档示例 + 定价页手工整理；/v1/models 需认证无法免鉴权拉取全量）
- id 格式：`厂商/模型名`（如 `Qwen/Qwen2.5-72B-Instruct`），大小写敏感
- anthropic 端点路径歧义：cURL 示例用 `/v1/messages`，Claude Code 配置用根域 `/`。preset 约定 anthropic 仅 host（路由层补 /messages），保守用根域
- siliconflow_en 完全复用 siliconflow 结论：endpoints 域名替换 .cn→.com，model_list + models.default 完全相同
- 认证：国内版 / 国际版 API key 推测独立（分开控制台 cloud.siliconflow.{cn|com}/account/ak）
