# ClientType 模拟行为配置化（headers.rs → client-types.json 驱动）

## Goal

`crates/aidog_core/src/gateway/proxy/headers.rs` 的 client_type 模拟逻辑（match 臂 dispatch + per-variant UA + protocol×client_type auth 矩阵 + Codex-OpenAI 动态 uuid headers）全部迁移到 `src-tauri/defaults/client-types.json` 配置驱动。Rust 执行引擎读 JSON + 占位符求值，**禁写任何 client_type 特定代码依赖**（用户逐字约束）。

**边界（用户逐字）**：
- ✅ JSON 化：client_type 模拟行为（UA / auth headers / extra headers / family 归组）
- ❌ 留代码：协议转换逻辑（converter.rs message body 转换：anthropic→openai_responses 等）
- ❌ 不动：Protocol enum（用户明示「不应该存在 protocol 的 json 配置」）/ 转发其他内容（message 等）

## Context

- 前 task `07-10-client-types-json-sync`（已 archive）只移了 label 层（`CLIENT_TYPES` 常量 + Rust `enum ClientType` → `type ClientType = String`），**模拟执行层漏** —— 本 task 补
- 现状 `headers.rs` 写死点（grep 验）：
  - `apply_client_headers:148-160` match 臂 dispatch（default / claude_code 家族 / codex 家族 / cursor / windsurf）
  - `claude_code_ua` / `codex_ua`：per-variant UA 字符串
  - `apply_claude_code_family_headers` / `apply_codex_family_headers` / `apply_cursor_headers` / `apply_windsurf_headers`：UA + protocol auth 矩阵 + Codex-OpenAI extra（OpenAI-Beta + conversation_id/session_id uuid_sim）
  - `build_upstream_headers:355-410`：日志镜像 apply 逻辑（透传入站 + 覆盖 CT/auth/UA/codex extra）
  - `uuid_sim`：动态 uuid 生成
- `client-types.json` 现状：12 entry + label/group/name(8 locale) + last_updated + `client_types_sync.rs` 远端同步链已建（spec `backend/remote-json-sync.md` 7 件套）
- spec 先例：`backend/remote-json-sync.md`（7 件套）、`guides/cross-layer-rules.md`（公共契约层禁改）
- 前端 `ccswitchMatch.ts:124` `client_type: "codex_tui"` 字面量

## 数据流（强制，禁前端直读 github）

```
client-types.json (含 simulation 配置)
  ↓ include_str! bundled / ~/.aidog/ 远端同步
rust reader (get_client_types_json，已建)
  ↓ invoke
前端派生层（buildClientTypesFromPresets，已建，仅消费 label —— 不动）
  ↓
Rust 执行引擎 (headers.rs 重构)
  ↓ 读 simulation 配置 + 占位符求值
apply_client_headers / build_upstream_headers
```

simulation 配置由 **Rust 内部消费**（headers.rs 执行引擎），前端不感知 simulation 字段（前端只消费 label/group/name 展示层）。

## Requirements

### R1 client-types.json schema 扩展（simulation 字段）

每 entry 加 `simulation` 对象，全自包含（禁 family 继承 —— 继承需 Rust 代码，违「不写代码依赖」）：

```json
{
  "value": "codex_tui",
  "group": "Codex",
  "name": {"zh-Hans": "...", "en-US": "...", ...},
  "simulation": {
    "user_agent": "Codex/0.38.0",
    "auth": {
      "anthropic": [{"name": "x-api-key", "value": "{api_key}"}],
      "openai": [
        {"name": "Authorization", "value": "Bearer {api_key}"},
        {"name": "api-key", "value": "{api_key}"},
        {"name": "OpenAI-Beta", "value": "responses=experimental"},
        {"name": "conversation_id", "value": "{uuid}"},
        {"name": "session_id", "value": "{uuid}"}
      ],
      "gemini": [{"name": "x-goog-api-key", "value": "{api_key}"}],
      "default": [{"name": "Authorization", "value": "Bearer {api_key}"}]
    }
  }
}
```

- `user_agent`：string（default entry 可缺省 = 不注入 UA）
- `auth`：`<protocol_snake_lower>` → headers 数组；`default` 兜底（未知 protocol）；protocol key 与 Rust `Protocol` serde rename 值对齐（anthropic/openai/gemini/...）
- 占位符（Rust 通用引擎求值，非 client_type 特定）：
  - `{api_key}` → 平台 api_key
  - `{uuid}` → `uuid_sim()` 运行时生成（每次调用新 uuid）
- 12 entry 全补 simulation（从 headers.rs 现有逻辑 1:1 迁移）
- `last_updated` bump

### R2 Rust 执行引擎重构（删 match 臂）

`headers.rs` 重构：
- 删 `apply_client_headers` match 臂 dispatch + `claude_code_ua` / `codex_ua` / `apply_*_family_headers` / `apply_cursor_headers` / `apply_windsurf_headers`
- 新 `apply_client_headers(req_builder, client_type, protocol, api_key)`：
  1. 读 `get_client_types_json()` bundled/同步 cache（启动时加载 OnceLock，禁每请求 IPC/读盘）
  2. 查 entry by `client_type.as_str()` → 取 `simulation`
  3. 注入 UA（若有 user_agent）
  4. 按 `protocol` serde rename 取 `auth` headers 数组（缺失 → `default` 兜底；全缺 → 仅 Bearer）
  5. 占位符替换（`{api_key}` / `{uuid}`）→ 注入 headers
- 未知 client_type（JSON 无 entry）→ 等价 default（仅 auth，不注入 UA），保留 client_type 字符串供审计
- `build_upstream_headers`（日志镜像）复用同一 simulation 配置（redact_key 日志安全）
- `uuid_sim` 保留（占位符引擎调用）
- 路径：`crates/aidog_core/src/gateway/proxy/headers.rs`

### R3 simulation 配置加载（OnceLock cache）

- 启动时 `OnceLock<SimulationConfig>` 加载 client-types.json（解析 simulation 字段）
- 读盘失败 → bundled `include_str!` fallback（同 `get_defaults_json` 模式）
- 禁每请求读盘 / IPC（性能 + 单次加载）
- 路径：`crates/aidog_core/src/gateway/proxy/headers.rs` 或 `client_types_sync.rs` 扩展

### R4 client_types_sync.rs schema gate 扩展

- `validate_structure` 加 `simulation` 字段校验（user_agent string + auth object + 每 protocol headers 数组）
- 远端 ⊇ bundled value 集合（已有）+ simulation 结构完整性
- 路径：`crates/aidog_core/src/gateway/client_types_sync.rs`

### R5 前端 ccswitchMatch.ts:124 字面量

- `client_type: "codex_tui"` 硬编码 —— **留单点兜底**（ccswitch 协议检测启发式的默认值，属代码逻辑非展示层常量，与 `ENDPOINT_PROTOCOLS` 同「请求格式协议」小常量例外类，spec `frontend/derived-constants.md` 保留条件三全中：① 后端无对应 simulation 真值源 ② 单点 ③ 业务稳定）。加注释说明「与 client-types.json codex_tui entry 对齐，改 default 值需同步此」

### R6 grep 验收（禁 client_type 特定代码）

- `grep -rn '"claude_code"\|"codex_tui"\|"claude_code_sdk_ts"\|"codex_cli"' crates/aidog_core/src/gateway/proxy/headers.rs` → 0（全移 JSON）
- `grep -rn 'claude_code_ua\|codex_ua\|apply_claude_code_family\|apply_codex_family\|apply_cursor_headers\|apply_windsurf_headers' crates/` → 0（fn 删）
- `grep -rn 'match client_type' crates/aidog_core/src/gateway/proxy/headers.rs` → 0（match 臂删）

## Acceptance Criteria

- [ ] client-types.json 12 entry 全补 simulation（UA + auth 矩阵 + extra headers + 占位符）
- [ ] headers.rs match 臂全删 → 读 JSON 配置 + 占位符引擎
- [ ] OnceLock 启动加载 simulation（禁每请求读盘）
- [ ] client_types_sync.rs schema gate 校验 simulation
- [ ] 前端 ccswitchMatch 字面量处理
- [ ] grep `"claude_code"` 等 client_type 字面量在 headers.rs = 0
- [ ] `cargo build --workspace` 0 errors
- [ ] `cargo test --workspace` baseline 不回归（headers 相关 test 改读 JSON 配置后行为等价）
- [ ] `cargo clippy --workspace --all-targets` 无新 warning
- [ ] `yarn build` / `yarn test` 全绿（前端无改动，simulation 不感知）
- [ ] 行为等价：apply_client_headers 输出 headers 与重构前 1:1（test 覆盖 12 client_type × 主要 protocol）

## Out of Scope

- Protocol enum JSON 化（用户明示不要）
- converter.rs 协议转换逻辑（用户明示留代码）
- 平台/转发其他配置（本 task 仅 client_type 模拟行为）
- commands-restructure C3-C10（另一线，独立执行）

## Technical Notes

- spec 合规：`backend/remote-json-sync.md`（7 件套，schema gate 扩展）、`guides/cross-layer-rules.md`（公共契约层：client_type 字段名 + ClientType 类型不动，仅内部 headers 逻辑换）
- 占位符引擎：通用 `{key}` 替换（HashMap<placeholder, value>），非 client_type 特定代码（合「不写代码依赖」）
- build_upstream_headers 复用 simulation（日志镜像实发，redact_key 安全）
- 行为等价 test：12 client_type × anthropic/openai/gemini 协议组合，验 apply 输出 headers 与重构前 diff 0

## Definition of Done

- R1-R6 全完成 + 验收全绿
- worktree 内 commit；主仓 post-merge `yarn tauri dev` 冒烟（apply_client_headers 运行时正确）
