# 添加 Devin 平台支持 — PRD (主入口)

## 目标
把 Devin (api.devin.ai) 作为**特殊平台类型**接入 aidog。客户端发 OpenAI/Anthropic 兼容 chat 请求 → aidog `handle_devin` 编排 Devin session（create→poll→messages）→ 包成 chat response 回传。让 claude_code/codex_tui 等客户端经 aidog 调 Devin agent。

**用户价值**：Devin agent 能力（长任务编码/PR review）经 aidog 统一审计 + 多 key 轮询 + quota 管理。

**架构定位**：Devin 是**平台**（platform_type，归 `protocol.rs` 「平台类型」组），非 wire 协议。接入范式同 ClaudeCode/CliProxy —— `handler.rs` 平台级分支，不经标准 `forward_attempt` / adapter/converter wire 层。

## 边界

### 范围内
- `platform-presets.json` 加 devin 平台条目（platform_type 标识 + cog_ key/org_id 配置，**无标准 endpoint**）
- `Protocol::Devin` enum 归「平台类型」组 + TS union 同步
- `handler.rs` 加 `Protocol::Devin` 分支 → `handle_devin`（session 编排）
- chat↔session 转换（messages→prompt, model→mode, response 包装）在 `handle_devin` 内
- 伪流式（轮询 messages 切块 SSE）
- stateful session（`X-Devin-Session-Id` header 映射）
- 短超时 504 + session_id 异步
- model→mode 映射（5 档虚拟 model）
- quota 查 `consumption/daily`（ACU 用量）
- est_cost 记 ACU 数

### 范围外（非目标）
- tool/function calling（Devin 无 chat tool 语义，丢弃 + warn）
- 真流式（Devin 无原生 SSE）
- 实时余额（Devin 无余额端点，仅 consumption 累计）
- playbook/knowledge/secrets 管理（Devin API 深功能，v1 不接）
- adapter/converter wire 层改动（Devin 不经标准转发路径）

### 已知约束
- 需 `cog_` key + `org_id`（`org-` 前缀）两个配置值（api_key + extra.org_id）
- **mode 字段冲突**（需实测裁定）：researcher `.md` 源报 `advanced_mode[analyze/create/improve/batch/manage]`，WebFetch 渲染页报 `devin_mode[normal/fast/lite/ultra/fusion]`
- `POST /sessions/{id}/messages` v3 schema 被 gate（v1 佐证 body=`{message}`）
- 429 响应头未文档化

## 验收标准
- [ ] preset devin 条目 serde round-trip（Rust + TS）
- [ ] `Protocol::Devin` serde `"devin"` round-trip 测试过（归平台类型组）
- [ ] `handler.rs` Devin 分支拦截 + `handle_devin` session 编排全链路单测（create→poll→messages→chat response）
- [ ] 伪流式：`stream:true` 时轮询切块发 chat SSE chunk，终态 `[DONE]`
- [ ] stateful：`X-Devin-Session-Id` header → 复用 session（POST messages）；无 header → 新建
- [ ] 短超时（默认 300s，`platform.extra.devin_timeout` 可配）：超时返 504 + body{session_id,url}
- [ ] model→mode 映射：5 虚拟 model 映射 mode 字段
- [ ] tool calling：请求带 tools 时丢弃 + log warn，不崩
- [ ] quota：`GET consumption/daily` → ACU 用量（balance.used = total_acus）
- [ ] est_cost：记 `session.acus_consumed`（非 token 折算）
- [ ] cargo clippy 零 warning + cargo test 全过 + yarn check:i18n
- [ ] 前端 ProtocolCard / formSections 支持 devin 平台展示 + 编辑（cog_ key + org_id 字段）

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json (`skein subtask list add-devin-support`)
