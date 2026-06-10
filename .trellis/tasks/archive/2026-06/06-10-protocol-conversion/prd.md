# protocol-conversion

## Goal

实现完整双向协议转换：支持多种入站协议（OpenAI / Anthropic / Claude Code 等），代理根据分组配置的源协议解析请求，转换为平台目标协议发送，响应按源协议格式返回客户端。

## What I already know

### 现状

- `adapter::convert_request()` 已支持 8 种出站协议转换（ChatRequest → 目标协议 JSON）
- `adapter::parse_sse()` + `to_anthropic_sse()` 将上游 SSE 转为 Anthropic 格式返回
- 入站请求始终按 Anthropic 格式解析（`source_protocol` 硬编码 `"anthropic"`）
- 平台已有 `protocol` 字段定义出站协议

### 调研结论

- 需新增 `parse_incoming_request()` 函数：将不同协议格式的请求解析为 ChatRequest
- 需新增 `to_client_sse()` 函数：将 ChatStreamEvent 转为客户端协议格式的 SSE
- 分组（group）需新增 `source_protocol` 字段，指定该分组接受什么格式的请求
- 当前 `ChatRequest` 基于 Anthropic 扩展，OpenAI 格式可直接映射

## Assumptions (temporary)

- 入站协议由分组配置决定，不同分组可接受不同协议
- 不支持自动检测协议（需明确配置）
- 响应格式与入站协议一致

## Open Questions

无

## Deliverable 矩阵

| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D1 | 入站协议解析器 | diff | cargo check；OpenAI 格式请求可解析为 ChatRequest | P0 |
| D2 | 客户端 SSE 格式化 | diff | 非 Anthropic 客户端收到正确格式响应 | P0 |
| D3 | 分组 source_protocol 配置 | diff | 分组可设置入站协议 | P0 |
| D4 | 前端平台/分组 UI 更新 | UI | 协议选择器可见 | P1 |

## Requirements

### R1 (D1) — 入站协议解析

- R1.1 新增 `parse_incoming_request(protocol, body) -> ChatRequest` 函数
- R1.2 支持 OpenAI / Anthropic / Claude Code 格式解析
- R1.3 proxy.rs 使用 `source_protocol` 选择解析器而非硬编码 Anthropic

### R2 (D2) — 客户端 SSE 格式化

- R2.1 新增 `to_client_sse(event, source_protocol) -> String` 函数
- R2.2 支持 Anthropic / OpenAI 格式输出
- R2.3 非流式响应也按源协议格式返回

### R3 (D3) — 分组配置

- R3.1 Group 模型新增 `source_protocol` 字段（默认 "anthropic"）
- R3.2 DB migration 添加该列
- R3.3 proxy.rs 从 group 读取 source_protocol

### R4 (D4) — 前端 UI

- R4.1 平台卡片清晰展示当前协议类型
- R4.2 分组编辑增加"入站协议"选择器

## Subtask 拆分

| ID | Subtask | 所属 D | 说明 |
| --- | --- | --- | --- |
| S1 | 入站解析器 + 客户端 SSE | D1,D2 | adapter 层新增 |
| S2 | Group source_protocol + DB | D3 | models, db, proxy |
| S3 | 前端 UI 更新 | D4 | Platforms, Groups |

## Acceptance Criteria

- [ ] cargo check 通过
- [ ] OpenAI 格式请求可正常代理到 Anthropic 上游
- [ ] 分组可配置 source_protocol
- [ ] 响应格式与入站协议一致

## Definition of Done

- Requirements 实现 + AC 勾选
- commit 完成
- worktree 合并 + 移除

## Out of Scope

- 自动协议检测
- 全协议对（仅需 OpenAI ↔ 其他 双向）
- 协议兼容性测试套件

## Technical Notes

### 转换链

入站: `Client Protocol → parse_incoming_request() → ChatRequest`
出站: `ChatRequest → convert_request() → Target Protocol → Upstream`
响应: `Upstream SSE → parse_sse() → ChatStreamEvent → to_client_sse() → Client Protocol`

### 验证命令

```bash
cd src-tauri && cargo check
cd .. && npx tsc --noEmit
```
