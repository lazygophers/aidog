# 补全 aicodemirror model_list+endpoints 全部官方信息

## Goal

AICodeMirror (aicodemirror.com) 是 **纯 Claude 代理共享平台**（前身 Claude Mirror），**仅支持 Claude 系**，非多供应商聚合。3 endpoint 全部存活（401 实测，三协议统一用 Anthropic key `sk-ant-api03-xxx` 鉴权 → 底层均 Claude 网关）。现有 7 个 model_list id 与 aidog 内 18+ 兄弟 Claude 代理 preset 完全同构（项目级 alias 约定），覆盖平台全部公开营销名（Opus 4 / Sonnet 4.5 / Haiku 3.5）。**无需增删 model_list**，仅补 models.default 三档（当前空）。

## Research References

- [`research/aicodemirror-models.md`](research/aicodemirror-models.md) — 纯 Claude 代理确认 + 3 endpoint 401 实测 + 7 alias 核对（无需改）+ alias 约定说明

## Requirements

### 1. endpoints（default 分支，3 端点全正确，不动）

3 endpoint 经 401 探测全部存活（路由存在）：
```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.aicodemirror.com/api/claudecode", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.aicodemirror.com/api/codex/backend-api/codex", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://api.aicodemirror.com/api/gemini", "client_type": "default"}
  ]
}
```

路径特殊性（平台自定义前缀，非统一网关）：
- anthropic: base + `/v1/messages`
- codex: base 本身即完整请求 URL（codex TUI 专用，无后缀；aidog `client_type: codex_tui` adapter 已处理）
- gemini: base + `/v1beta/models/{model}:generateContent`

三协议均用 Anthropic key 鉴权 → 底层统一 Claude。

### 2. model_list.default（7 模型全保留，不增删）

现有 7 alias 与 18+ 兄弟 Claude 代理 preset 完全同构，覆盖平台全部公开营销名：
- claude-opus-4-8 / claude-sonnet-4-6 / claude-haiku-4-5 / claude-opus-4-7 / claude-opus-4-6 / claude-opus-4-5 / claude-sonnet-4-5

🔴 **不增删**（研究结论）：
- 平台官方未公开 id 白名单（dashboard 需登录，无 `/api/models` 免鉴权端点）
- 现有 alias 是 aidog 项目级约定，一致性 > 追平台未公开真名
- 不补 `claude-sonnet-5`（首页 hero 提及但 footer 未列，平台是否真开通未验证）

### 3. models.default（补三档，当前空）

```json
"models": {
  "default": {
    "claude-sonnet-4-6": {},
    "claude-opus-4-8": {},
    "claude-haiku-4-5": {}
  }
}
```

三档：Sonnet 主力（性价比/高吞吐）/ Opus 重型（复杂分析）/ Haiku 轻量（快速动作）。

### 4. desc（保留不改）

现有 desc "Claude 兼容模型" 准确（平台确为 Claude 代理）。**保留**。

## Acceptance Criteria

- [ ] endpoints 3 保留（已核验 401 存活）
- [ ] model_list 7 保留不增删
- [ ] models.default 补三档（sonnet-4-6 / opus-4-8 / haiku-4-5）
- [ ] desc 保留
- [ ] JSON 合法
- [ ] 仅改 aicodemirror 块

## Out of Scope

- claude-sonnet-5（平台未确认开通，不补）
- id 日期后缀（aidog alias 约定，不补）
- OpenClaw 第四客户端（未深查，preset 不涉）
- 上下文窗口字段
- STATIC_MODEL_IDS
- 其他协议块

## Technical Notes

- 真值源：`protocols.aicodemirror`
- 平台定位：纯 Claude 代理（非聚合），三 endpoint 协议翻译入口底层 Claude
- 鉴权：三协议统一 Anthropic key `sk-ant-api03-xxx`
- id 精确性建立在 aidog 项目级 alias 约定 + 18+ 兄弟 preset 一致性，非平台官方直陈
- 同源参考：ccsub / apikeyfun / packycode 等 Claude 代理 preset（同 alias 集）
