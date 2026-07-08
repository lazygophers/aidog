# 补全 openai model_list（endpoints 不动）

## Goal

aidog `platform-presets.json` openai 协议 `model_list.default` 现仅 `["gpt-5.5"]`，过窄。openai 协议 = 标准 API 调用（api.openai.com/v1 + API key），据 OpenAI 官方 GA canonical 模型清单补全，让客户端探测拿到完整列表。endpoints 保持不动（chatgpt backend codex 端点属 codex 协议，不归 openai）。

## What I already know

- openai 协议现状（`platform-presets.json`）：
  ```json
  "endpoints": { "default": [{ "protocol": "openai", "base_url": "https://api.openai.com/v1", "client_type": "codex_tui" }] },
  "models": { "default": { "gpt": "gpt-5.5" } },
  "model_list": { "default": ["gpt-5.5"] }
  ```
- **用户澄清**：openai 协议 = 标准 API；codex 协议 = Codex CLI/Coding Plan（chatgpt backend 订阅）。两者独立，endpoints/model_list 不混。
- **research 结论**（`research/openai-official-endpoints-models.md`）：
  - openai endpoints **不动**（api.openai.com/v1 保留；chatgpt backend `https://chatgpt.com/backend-api/codex` 归 codex 协议）
  - 建议补入 `model_list.default`（旗舰优先）：`gpt-5.5` / `gpt-5.4` / `gpt-5.4-mini` / `gpt-5.4-nano`
  - `models.default.gpt` 保持 `gpt-5.5`（Codex 默认）
  - **不应补**：gpt-5.2/gpt-5.3-codex（Codex 已 deprecated）、gpt-4o/o1/o3/o4 系列（2026-10-23 前后退役）、gpt-5 初代 snapshots（2026-12-11 退役）
  - 6 官方源 URL 在 research 文件末

## Decision (ADR-lite)

**Context**: openai model_list 缺同代/上一代 GA 旗舰。chatgpt backend 端点曾被误纳 openai，实为 codex 协议。
**Decision**:
- endpoints 不动（仅 api.openai.com/v1）
- model_list.default 补 4 项：`["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano"]`
- models.default.gpt 保持 `gpt-5.5`
**Consequences**: 客户端 /v1/models 探测拿到 GA 全系；不引入已退役模型避免腐化。

## Requirements

1. `platform-presets.json` openai 协议 `model_list.default` 改为 `["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano"]`。
2. `models.default.gpt` 保持 `gpt-5.5`（不动）。
3. `endpoints.default` 不动。
4. 更新 `last_updated` Unix 秒。
5. STATIC_MODEL_IDS（`passthrough.rs:233`）评估补 `gpt-5.4` / `gpt-5.4-mini` / `gpt-5.4-nano`（现有仅 gpt-5.5 + gpt-5.5-codex）。

## Acceptance Criteria

- [ ] openai model_list.default = `["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano"]`
- [ ] endpoints/models.default 不变
- [ ] STATIC_MODEL_IDS 同步补 3 项（若评估通过）
- [ ] `/v1/models`（openai 格式）返回补全后列表
- [ ] presets JSON 解析无错（启动加载）
- [ ] cargo test 通过（test_passthrough.rs STATIC_MODEL_IDS 计数更新）
- [ ] yarn build clean
- [ ] 需重启 `yarn tauri dev`（memory `tauri-rust-command-needs-restart`）

## Out of Scope

- 不改 endpoints（chatgpt backend 归 codex 协议）
- 不改 `models.default.gpt` 默认指向
- 不改其他协议（仅 openai）
- 不补已退役/preview 模型（gpt-4o/o-series/gpt-5 初代 snapshot/gpt-5.2·5.3-codex）
- 不改前端 UI

## Technical Notes

- 真值源 = `src-tauri/defaults/platform-presets.json`
- research: `.trellis/tasks/07-08-openai-endpoints-models/research/openai-official-endpoints-models.md`
- **跨 task 文件冲突**: 本 task + codex-model-list + add-fable-model 共改 platform-presets.json → task 级串行
- STATIC_MODEL_IDS 跨 openai+anthropic 两协议静态返回，改它影响 /v1/models + /proxy/models（两协议都变），需评估
