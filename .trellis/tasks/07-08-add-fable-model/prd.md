# 添加 Fable 模型配置

## Goal

Anthropic 新发布 Fable 5（model id `claude-fable-5`）。aidog 需在 anthropic 协议模型配置 + 静态模型列表 + 定价表三处补入 Fable，让用户能在 Claude Code 选 Fable、代理 /v1/models 返回 fable、估算成本可用。

## What I already know

- model id = `claude-fable-5`（Anthropic 官方命名，Claude 5 家族旗舰）
- `platform-presets.json` anthropic 协议结构（line 17-29）：
  ```json
  "models": { "default": { "default": "claude-opus-4-8", "opus": "claude-opus-4-8", "sonnet": "claude-sonnet-4-6", "haiku": "claude-haiku-4-5" } },
  "model_list": { "default": ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5"] }
  ```
- `STATIC_MODEL_IDS`（`passthrough.rs:233`）：5 项（opus-4-8/sonnet-4-6/haiku-4-5/gpt-5.5-codex/gpt-5.5），`/v1/models` & `/proxy/models` 静态返回源。CLAUDE.md 约定「静态模型 id 月级腐化需手工核对」。
- `model_price` 表（litellm source 同步）：Fable 5 太新，litellm 可能暂无数据 → 需手加占位行。
- 用户决策（grill 硬门1 确认）：**新增 fable key**（不动 default/opus/sonnet/haiku）；**三处全改**（presets + STATIC_MODEL_IDS + model_price）。

## Requirements

1. **platform-presets.json** anthropic 协议：
   - `models.default` 加 `"fable": "claude-fable-5"`（与 opus/sonnet/haiku 并列，纯新增）
   - `model_list.default` 数组加 `"claude-fable-5"`（建议放 opus-4-8 后、sonnet-4-6 前，按旗舰优先序）
   - 更新 `last_updated` Unix 秒为当前时间
2. **STATIC_MODEL_IDS**（`passthrough.rs:233`）：加 `"claude-fable-5"`（放 `claude-opus-4-8` 后）。
3. **model_price** 手加占位行（Fable 5 定价）：
   - 🔴 **需要: 用户确认 Fable 5 官方定价**（input_cost_per_token / output_cost_per_token / cache cost）。若官网未公布，按 opus-4-8 同档占位（input $15/M, output $75/M）+ source 标 `manual`（litellm 同步后覆盖）。
   - price_data JSON 至少含 `input_cost_per_token` / `output_cost_per_token` / `litellm_provider: "anthropic"` / `max_input_tokens` / `max_output_tokens`。

## Acceptance Criteria

- [ ] `platform-presets.json` anthropic models.default 含 `"fable": "claude-fable-5"`，default/opus/sonnet/haiku 不变
- [ ] model_list.default 含 `"claude-fable-5"`
- [ ] `last_updated` 更新为当前 Unix 秒
- [ ] STATIC_MODEL_IDS 含 `"claude-fable-5"`，`/v1/models` & `/proxy/models` 返回 fable
- [ ] model_price 含 claude-fable-5 行（定价据用户确认或 opus 同档占位 + source=manual）
- [ ] est_cost 估算 claude-fable-5 请求命中 model_price 行（非 fallback）
- [ ] cargo test 通过（test_passthrough.rs STATIC_MODEL_IDS 测试更新计数）
- [ ] yarn build clean
- [ ] presets JSON 解析无错（启动加载）

## Out of Scope

- 不改 default/opus 指向（Fable 纯新增 key，不顶替默认）
- 不改其他协议（仅 anthropic）
- 不改前端模型选择器 UI（模型列表来自 presets / 用户配置，前端无硬编码）
- 不改 statusline 预览（statusline-segments.ts:117 sonnet 预览仅展示用，不动）
- Fable 5 上下文窗口 / max tokens 等规格若官方未公布，不臆测（留空或按 opus-4-8 占位）

## Technical Notes

- model_price INSERT 参考 litellm 同步行格式（price_data JSON）。source 字段标 `manual` 区分自动同步。
- price_sync.rs 自动同步逻辑：litellm 有数据时覆盖 manual 行（按 model_name 唯一）。
- 需重启 `yarn tauri dev` 让 presets + STATIC_MODEL_IDS 生效（memory `tauri-rust-command-needs-restart`）。
