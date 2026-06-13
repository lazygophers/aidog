# Kimi Code Plan 预设 client_type 修复

## 背景

model_test 对 `kimi-for-coding` 返回 403 `access_terminated_error`：
> "Kimi For Coding is currently only available for Coding Agents such as Kimi CLI, Claude Code, Roo Code, Kilo Code, etc."

用户确认密钥有效。根因非密钥失效，而是 **client 身份被上游拒绝**。

## 根因

`src/pages/Platforms.tsx:175` Kimi Code Plan 预设把 openai coding endpoint 配为 `client_type: "codex_tui"`：

```ts
{ protocol: "openai", base_url: cp ? "https://api.kimi.com/coding/v1" : "https://api.moonshot.cn/v1", client_type: "codex_tui", coding_plan: cp },
```

Kimi coding 上游（`api.kimi.com/coding/v1`）**只接受 Kimi CLI / Claude Code / Roo Code / Kilo Code，拒绝 Codex**。Codex headers（`User-Agent: Codex/0.38.0` + `originator: codex_cli_rs`）触发 `access_terminated_error`。

影响范围：proxy + model_test 全链路同病（都读 `ep.client_type`，见 `proxy.rs:738` + `lib.rs:793`）。model_test 优先选 coding_plan endpoint（`lib.rs:794`）→ openai coding（codex_tui）→ 403。

## 修复

### 1. 预设修正（新建平台）
`Platforms.tsx:175` Kimi openai coding_plan endpoint：`client_type: "codex_tui"` → `"claude_code"`。

`apply_claude_code_family_headers`（proxy.rs:1998）对 openai 协议发 `Authorization: Bearer {key}` + `claude-cli/1.0.117` UA + Stainless headers，Kimi 白名单接受。

### 2. DB 数据迁移（已有平台）
`db.rs init_db` 末尾加 Migration 012：扫描 `platform_type='kimi'` 的平台，解析 endpoints JSON，把 `protocol=openai && coding_plan=true && client_type=codex_tui` 的 endpoint 改为 `claude_code`。幂等（仅改 codex_tui，已 claude_code 不动）。

不迁 qianfan / 百炼等其他 coding plan 平台（用户未报问题，避免过度迁移）。

## 验证
- `cargo build` 通过
- `npx tsc` 无错
- 手动：model_test kimi-for-coding 返回 200（需有效 key）

## 非目标
- 不改 proxy / model_test client_type 选择逻辑（已对齐，根因在预设值）
- 不动 qianfan / 百炼等其他 coding plan 预设
