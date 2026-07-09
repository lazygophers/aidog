# 补全 cherryin model_list+endpoints 全部官方信息

## Goal

CherryIN (open.cherryin.net) 是基于 new-api 的聚合路由网关，官方 `/api/pricing` 公开端点返回全量 **155 个模型**（跨 14 家供应商，2026-07-09 实拉 `pricing_version=5a90f2b8...`）。当前 preset 仅 13 个模型 + 单 anthropic 端点 + 空 `models.default`，需按官方真值全量补全，并修正 `grok-4` → `x-ai/grok-4`（官方无裸 id）。

## Research References

- [`research/cherryin-models.md`](research/cherryin-models.md) — 全量 155 模型 + 端点契约 + 计费规则 + caveat（实拉 `/api/pricing`，非营销名/非推测）

## Requirements

### 1. endpoints（default 分支，3 端点全补）

现有单 anthropic 端点 → 扩为 anthropic + openai + gemini 三端点（对齐 shengsuanyun 模式，使 grok/openai-response/gemini 模型可用）：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://open.cherryin.net", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://open.cherryin.net/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://open.cherryin.net", "client_type": "default"}
  ]
}
```

- anthropic / gemini 端点 base_url 写**裸 host**（适配器内部拼 `/v1/messages` / `/v1beta/...`），保留现有写法。
- **openai 端点 base_url 必须含 `/v1`**（aidog `provider_api_path()` 仅返 `/chat/completions`），`client_type: codex_tui`（覆盖 gpt-5 系 openai-response + codex 系 + grok 系）。

### 2. model_list.default（全量 chat/completion 模型，禁遗漏）

从 research 文件提取**全部对话/补全模型**，规则：

- ✅ 纳入：全部 anthropic(9) / openai 对话系(28, 排除 gpt-image-*) / google 全部(11, 多模态含 image-preview 保留) / deepseek 全部(11, 含 agent+free) / moonshot 全部(12) / glm 全部(10) / grok 全部(9) / minimax 全部(11) / qwen 对话系(文本+vl，约 32，排除纯 embedding/reranker/image) / 其他对话(bytedance/tencent/stepfun-ai，排除 bge/kolors/qwen-image 纯专用)
- ❌ 排除：纯 embeddings 专用（baai/bge-m3, qwen3-embedding-*）、纯 jina-rerank（qwen3-reranker-*, BAAI/bge-reranker）、纯 image-generation（gpt-image-*, qwen-image*, kolors）
- ✅ 保留 `agent/` 聚合通道双 entry（与 `<vendor>/` 并列，价同，双协议适配广，与现有 13 风格一致）
- ✅ 保留 `(free)` 0 倍率模型（标注保留，受 RPM 限但不消耗 quota）
- 🔴 **修正**：现有 `grok-4` → `x-ai/grok-4`（官方无裸 id，且 grok 系仅 openai/openai-response，新增 openai 端点后可用）

精确清单见 research 文件各供应商表（按表逐条提取，禁主观筛选旗舰）。

### 3. models.default（三档默认）

```json
"models": {
  "default": {
    "anthropic/claude-opus-4.8": {},
    "openai/gpt-5.5": {},
    "agent/glm-5.2": {}
  }
}
```

覆盖三档客户端默认（Claude 最新 opus / OpenAI 最新 gpt 旗舰 / 国产 GLM 旗舰，与现有 13 的 agent/glm-5.2 风格一致）。

## Acceptance Criteria

- [ ] endpoints default 分支 3 端点（anthropic 裸 host / openai 含 /v1 / gemini 裸 host）
- [ ] model_list.default 覆盖 research 全部对话模型（排除纯 embedding/rerank/image-gen 专用）
- [ ] `grok-4` 修正为 `x-ai/grok-4`
- [ ] `agent/` 双 entry 保留（agent/X 与 vendor/X 并列）
- [ ] `(free)` 模型保留
- [ ] models.default 三档（claude-opus-4.8 / gpt-5.5 / agent/glm-5.2）
- [ ] JSON 合法（python json.load 通过）
- [ ] 仅改 cherryin 协议块，不动其他协议

## Out of Scope

- 上下文窗口字段（pricing API 不返回，`models.default` value 留空 object，后续手补）
- 其他协议块改动
- STATIC_MODEL_IDS（passthrough.rs）— 仅 anthropic/openai/codex/gemini 官方协议静态列表，cherryin 不涉

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json` `protocols.cherryin`
- research 数据来源：`https://open.cherryin.net/api/pricing`（免鉴权，new-api 公开端点，155 条）
- 同源参考：shengsuanyun（172 模型 3 端点）已成功落地，同模式
- grok 系协议特殊性：仅 openai/openai-response，依赖新增的 openai 端点
- gpt-5 系原生走 openai-response（`/v1/responses`），codex_tui client_type 支持
