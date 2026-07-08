# 补全 codex model_list + 修正默认指向

## Goal

aidog `platform-presets.json` codex 协议 preset 默认指向 `gpt-5.5-codex`，经 Codex CLI 源码全仓搜证伪（openai/codex 0 命中）= 抄错外推。同时 `model_list.default` 仅该 1 项，过窄。据 Codex CLI 源码 + 官方文档修正默认指向 + 补全 model_list。

## What I already know

- codex 协议现状（`platform-presets.json`）：
  ```json
  "models": { "default": { "gpt": "gpt-5.5-codex" } },
  "model_list": { "default": ["gpt-5.5-codex"] }
  ```
- 🔴 **research v2 证伪**（`research/codex-official-models.md`，方向修正版）：
  - `gh search code "gpt-5.5-codex" --repo openai/codex` → **0 命中**（含测试）。aidog preset 抄错外推确认。
  - Codex CLI 5.4/5.5 代 model id **全部无 `-codex` 后缀**：`gpt-5.5` / `gpt-5.4` / `gpt-5.4-mini`（源码字面量 + 官方 Codex Models 页双确认）
  - `-codex` 后缀是 5.1/5.2/5.3 代历史命名（model_migration.rs），当前仅 5.1 代有迁出提示
  - Codex CLI 不硬编码默认 model（`model: Option<String>`，从 catalog 动态取，TTL 300s）
  - chatgpt backend URL = `https://chatgpt.com/backend-api/`（config/mod.rs:3906），走 `/responses`
  - chatgpt backend 与标准 API **共用 model id 命名空间**（无「chatgpt backend 专用」id）
- 🔴 **aidog gateway 无 model alias 映射机制**（main 核查）：
  - `needs_model_remap`（forward.rs:80）= 路由级 actual_model≠requested_model 替换（如 claude→doubao），**非协议级 alias→canonical**
  - grep `model_alias` / `canonical_model` / `codex.*alias` 全 0 命中
  - → **b 案（保留 alias + 代理映射）不可行**，需新开发映射逻辑（scope 膨胀）

## Decision (ADR-lite)

**Context**: preset 默认 `gpt-5.5-codex` 官方源 + Codex CLI 源码全程不存在。补 model_list 必须先定该 id 去留。
**Decision**: **A 案 —— 改默认指向官方 `gpt-5.5`**（AFK best-judgment，用户回来可推翻）
- 理由：① b 案不可行（无 alias 机制）② c 案留腐化（probe 返回不存在 id）③ a 与真值对齐，preset 默认应有效 id ④ 只影响新平台默认，已存平台配置已持久化不受影响
**Consequences**: 新建 codex 协议平台默认填 `gpt-5.5`；model_list 返回有效 canonical id。

## Requirements

1. `platform-presets.json` codex 协议：
   - `models.default.gpt`: `gpt-5.5-codex` → `gpt-5.5`
   - `model_list.default`: `["gpt-5.5-codex"]` → `["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"]`
2. STATIC_MODEL_IDS（`passthrough.rs:233`）：`gpt-5.5-codex` → `gpt-5.5`（去重，已有 gpt-5.5）+ 加 `gpt-5.4` / `gpt-5.4-mini`
   - 注意 STATIC_MODEL_IDS 跨 openai+anthropic 两协议静态返回，改它影响 /v1/models + /proxy/models 两协议
   - test_passthrough.rs:235 断言更新（`gpt-5.5-codex` → `gpt-5.5`，计数变）
3. 更新 `last_updated` Unix 秒。
4. **与 openai task 协调**：openai task 也改 STATIC_MODEL_IDS（加 gpt-5.4/5.4-mini/5.4-nano）+ model_list。两 task 共改 passthrough.rs + platform-presets.json → **必须串行**（task 级文件冲突）。

## Acceptance Criteria

- [ ] codex `models.default.gpt` = `gpt-5.5`（不再 gpt-5.5-codex）
- [ ] codex `model_list.default` = `["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"]`
- [ ] STATIC_MODEL_IDS 去重 + 补 gpt-5.4/5.4-mini（与 openai task 合并改一次）
- [ ] test_passthrough.rs 断言更新通过
- [ ] `/v1/models` + `/proxy/models` 返回补全后列表（无 gpt-5.5-codex 腐化 id）
- [ ] presets JSON 解析无错（启动加载）
- [ ] cargo clippy 0 warning, cargo test 通过
- [ ] 需重启 `yarn tauri dev`（memory `tauri-rust-command-needs-restart`）

## Out of Scope

- 不加 `gpt-5.3-codex-spark`（research 标「源码无字面量，仅官方页列」= 未实证，preset 不放）
- 不加 chatgpt/azure provider 前缀变体（非 canonical）
- 不改前端 UI
- 不补 5.1/5.2/5.3-codex 历史命名（deprecated）
- 不实现 model alias 映射机制（b 案，scope 膨胀，另立 task 若需要）
- 不改 chatgpt backend endpoint（`/backend-api/codex` 走 /responses，aidog converter 适配另立 task，参 openai-research caveat）

## Technical Notes

- 真值源 = `src-tauri/defaults/platform-presets.json`
- research: `.trellis/tasks/07-08-codex-model-list/research/codex-official-models.md`
- 🔴 **跨 task 文件冲突**: 本 task + openai-endpoints-models + add-fable-model 共改 platform-presets.json + passthrough.rs → task 级串行
- **STATIC_MODEL_IDS 合并改**: codex + openai 两 task 都改它，建议串行后 task 接前 task 结果一次性改（避免冲突）
