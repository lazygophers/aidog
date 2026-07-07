# PRD: codex 协议切 OpenAI Responses API

## Goal

codex 平台预设的 `protocol` 当前为 `"openai"`（→ 新建 platform 出站走 `/v1/chat/completions`），但 OpenAI Codex CLI 0.38+（UA `Codex/0.38.0`, `proxy/headers.rs:184`）实际用 Responses API（`/v1/responses`）。改为 `openai_responses`，让新建 codex platform 的出站与 Codex CLI 入站格式自洽（responses in / responses out），避免响应格式错位致 Codex CLI 解析失败。

## 根因（auto-context 探明）

- **入站已自适应**: `proxy/handler.rs:276-277` `source_protocol = detect_source_protocol(&path)`（从 URL path 探测，注释 "group no longer restricts inbound protocol"）。Codex CLI 发 `/v1/responses` → 自动判 `openai_responses` → `parse_incoming_request("openai_responses")` → `from_responses`（`request.rs:72`）。**入站无需改**。
- **出站按 platform_protocol**: `convert_request` 按 group 选定平台的 endpoint protocol 分发（`request.rs:25-28` OpenAIResponses → `/v1/responses`；其余 openai 兜底 `/chat/completions`）。preset `protocol=openai` → 新建 codex platform 出站走 `/chat/completions` → 上游返 chat 格式 → Codex CLI 期望 responses 格式 → 解析失败。
- **adapter 全就绪**: `Protocol::OpenAIResponses` + `openai_responses` adapter（to_responses/from_responses）+ convert/passthrough/parse 三路均完整。

## 目标 (axis A)

- 新建 codex platform 默认 `protocol = "openai_responses"`（出站 `/v1/responses`，与入站一致）
- 不破坏入站自适应（detect_source_protocol 不动）
- 不影响其他 openai 系协议平台（openai/openai_completions）

## 非目标

- 不改 codex.rs（Codex CLI TOML 配置子系统，与本任务无关）
- 不改 detect_source_protocol（入站已自适应）
- 不改 client_type 映射（platform.rs:120 三协议共用 CodexTui，无需动）
- 不改其他协议 preset

## 交付 (axis B)

| # | 交付物 | 验收 |
|---|--------|------|
| D1 | `src-tauri/defaults/platform-presets.json` codex 条目 `protocol: "openai"` → `"openai_responses"` | `python3 -m json.tool` 有效；grep `codex` 条目 protocol=openai_responses |
| D2 | 验证：`cargo build` 0 error + `cargo test` 相关不回归 + grep 无其他 codex 硬编码 openai（defaults_sync / 前端 defaultClientForProtocol） | 门禁 exit 0 |
| D3（open decision） | 存量 DB codex platform endpoint protocol 迁移：是否加 DB migration（改存量 platform 表 codex endpoint protocol openai→openai_responses）？ | 见 Open Decision |

## Open Decision（start 前需用户拍板）

**存量迁移范围**（依赖用户实际是否已建 codex platform）：

- **方案 A（最小，推荐）**: 仅 preset JSON（D1+D2）。新建 codex platform 默认对；存量用户若建过 codex platform 需手动改 endpoint protocol。理由：preset = 模板，DB 存量是用户数据，自动改有风险；且入站已自适应，存量 platform 出站虽走 chat/completions 但上游 OpenAI 接受，仅 Codex CLI 响应格式可能不符 → 用户遇问题时手动改。
- **方案 B（含 migration）**: A + DB migration（自动改存量 codex platform endpoint protocol）。需查 DB schema migration 机制（schema_early.rs / mod.rs，当前到 ~034），加一条 migration。风险：自动改用户数据。
- **方案 C**: A + 前端提示（检测到用户 codex platform protocol=openai 时 UI 提示建议改）。

## Decision (ADR-lite)

**Context**: 存量迁移范围（用户多次超时，按推荐自主推进）。

**Decision**: **方案 A**（仅 preset JSON D1+D2，无 DB migration，无前端提示）。

**Consequences**:
- 新建 codex platform 默认 protocol=openai_responses（出站 /v1/responses，与 Codex CLI 入站一致）
- 存量用户若已建 codex platform（protocol=openai）不动 —— preset=模板，DB 存量是用户数据，自动改有风险
- 入站已自适应（detect_source_protocol），存量 platform 出站走 /chat/completions 上游接受，仅 Codex CLI 响应格式可能不符 → 用户遇问题时手动改
- 发版说明可补（非本 task 范围）

## 调度

单 task，1 文件 1 字段改（D1）+ 验证（D2）。trellis-implement 内联直做。flow 默认强制 worktree，遵默认。

## 风险

- **低**: 存量用户未感知（方案 A 下）。→ 缓解：发版说明 / 前端检测提示（方案 C）
- **低**: 其他 coding_plan 分支协议（cp 三元）若也误配 openai。→ D2 grep 确认

## Technical Notes

- 改点: `src-tauri/defaults/platform-presets.json` protocols.codex.endpoints.default[0].protocol
- 入站链（不改）: detect_source_protocol(/v1/responses) → openai_responses → from_responses → ChatRequest
- 出站链（改后）: ChatRequest → convert_request OpenAIResponses → /v1/responses → 上游
- Codex CLI UA: `Codex/0.38.0`（proxy/headers.rs:184）
