# logs-refresh-btn

## Goal

实现 proxy log 渐进式写入：请求生命周期的每个阶段即时 upsert 日志，确保前端实时看到最新字段。

## What I already know

### 现状

- proxy.rs 已有 4 阶段 upsert：body read → group resolve → route resolve → upstream response
- 但部分字段未在对应阶段及时填充（如 target_protocol 在 route resolve 后才设置）
- 前端 Logs.tsx 详情页已有 2s 自动刷新

### 调研结论

- 当前 upsert 模式正确（INSERT OR REPLACE），但 upsert 时机和字段填充需完善
- 需确保每个阶段 upsert 前填好该阶段可获取的所有字段

## Assumptions (temporary)

- 不改变现有 ProxyLog 结构（刚加完 cache_tokens）
- 渐进式写入仅涉及 proxy.rs handle_proxy 函数内的 upsert 时机调整

## Open Questions

无

## Deliverable 矩阵

| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D1 | 渐进式日志写入完善 | diff | cargo check；日志字段在对应阶段填充 | P0 |

## Requirements

### R1 (D1) — 渐进式日志写入

4 个阶段的 upsert 时机与字段要求：

1. **接收请求**（body 已读）：group_name, model, request_headers, request_body, source_protocol
2. **选择平台**（route 已确定）：target_protocol, actual_model, group_name（token 对应的 group）
3. **处理结果**（upstream 响应）：status_code, duration_ms, input_tokens, output_tokens, cache_tokens, response_body
4. **类型转换**：如发生协议转换，记录 source_protocol → target_protocol 的转换信息

### R2 (D1) — 刷新按钮

- 请求列表和详情页已有刷新按钮（已实现）
- 确认 2s 自动刷新在详情页正常工作

## Subtask 拆分

| ID | Subtask | 所属 D | 说明 |
| --- | --- | --- | --- |
| S1 | proxy.rs upsert 时机完善 | D1 | 确保每个阶段填好字段后立即 upsert |

## Acceptance Criteria

- [ ] cargo check 通过
- [ ] 请求发出后，日志列表立即出现记录（group、model 已填）
- [ ] 平台选定后，日志更新 target_protocol、actual_model
- [ ] 响应返回后，日志更新 status、tokens、duration

## Definition of Done

- Requirements 实现 + AC 勾选
- commit 完成
- worktree 合并 + 移除

## Out of Scope

- 新字段添加（cache_tokens 刚完成）
- 前端 UI 改动
- 协议转换逻辑（另建 task）

## Technical Notes

### 当前 upsert 点

proxy.rs handle_proxy 中已有注释标记的 upsert 点，需逐个检查字段填充完整性。
