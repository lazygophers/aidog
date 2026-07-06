# 日志格式器 MsgCollector 丢非 message event 字段

## Goal

`tracing::debug!(fn=, req=, dur=, sql=, "exec sql")` 这类 event 的结构化字段 (fn/req/dur/sql) 在 AidogFormat 格式器中全部丢失，日志只剩 msg + traceid。

证据: `2026-07-06 14:58:08.290 DEBUG src/gateway/db/trace.rs:61 sql exec sql bz5uph` — 应含 `[fn=db.rs:xx req=xxx dur=0.1ms] sql=SELECT ...`，实际全空。

影响面远超 SQL —— 所有 `tracing::debug!(field=val, ...)` / `tracing::info!(field=val, ...)` 形式的 event 字段都丢（含 HTTP method/path、proxy 诊断字段、关键业务上下文）。这是 07-06-trace-id-log-format task 的回归：AidogFormat MsgCollector 只收 message 字段，丢弃其他全部。

## What I already know

- **bug 点**: `src-tauri/src/logging.rs:462-482` `MsgCollector`
  - `record_debug` / `record_str` 只处理 `field.name() == "message"`，其他字段静默丢弃
  - 注释 (line 458) 写「其余字段已在 span 上经 trace_id_from_span_scope 取到, 不重复」—— 假设错误：业务 event (trace.rs:61) 字段挂 event 不挂 span
- **trace-id task 设计缺陷**: trace-id 取值链 (span scope → thread-local → gen) 只解决 traceid 字段，未考虑业务结构化字段
- **trace.rs:48-68** `sql_profile_callback` 是裸 `tracing::debug!(target="sql", fn=, req=, dur=, sql=, "exec sql")` — event 字段，无 span 包装（rusqlite profile 回调在 DB 后台线程，无业务 span）
- **logging-format spec** (`.trellis/spec/backend/logging-format.md:20`) 规定 5 段字段顺序 time/level/file:line func/msg/traceid，但 msg 段应包含业务字段（spec 未明确禁止字段渲染，是实现 bug）
- **Affected grep**: `tracing::debug!(` / `tracing::info!(` / `tracing::warn!(` / `tracing::error!(` 含 field=val 的 event 都受影响（待 subagent 全量盘点）

## Requirements

- MsgCollector 收集**所有** event 字段：`message` 字段 → msg 主体；其他字段 → 按 `key=value` 顺序追加到 msg 段尾部
- 字段值渲染：字符串去引号 (与现 message 处理一致)；Debug 类型保留 Debug 格式
- 字段顺序：tracing 默认记录顺序（message 字段优先主体，其余按出现顺序）
- trace_id 字段：若 event 显式带 trace_id 字段（罕见），与 span scope 取的 traceid 段重复时跳过 event 字段（避免 5 段格式 traceid 段重复）
- 5 段格式不变 (time/level/file:line func/msg/traceid)，业务字段塞 msg 段尾部
- console ANSI + file 纯文本 两层格式器同步修复（共用 AidogFormat，已对齐）

## Acceptance Criteria

- [ ] `tracing::debug!(target="sql", fn="db.rs:1", req="abc", dur="0.1ms", sql="SELECT 1", "exec sql")` 渲染含全部 4 字段
- [ ] 现有 trace-id 测试不回归（5 段格式 / traceid 取值链 / ANSI on-off / spawn_traced）
- [ ] logging.rs 新增 unit test：MsgCollector 收多字段 + 顺序 + trace_id 去重
- [ ] SQL 日志验证：debug build 跑代理触发 DB 操作，日志行含 `fn= req= dur= sql=`
- [ ] cargo clippy 0 new warning，cargo test 全绿
- [ ] spec logging-format.md 补字段渲染契约（msg 段含业务 key=value，禁丢字段）

## Definition of Done

- MsgCollector 单点修复（logging.rs:462-482）
- unit test 覆盖多字段 + trace_id 去重
- spec logging-format.md PATCH（补字段渲染契约段）
- 现有 1226 测试全绿不回归

## Technical Approach

### MsgCollector 修复 (logging.rs:462-482)

```rust
#[derive(Default)]
struct MsgCollector {
    msg: String,           // message 字段拼接
    extra: Vec<(String, String)>,  // 其他字段 (name, value) 按记录顺序
}

impl tracing::field::Visit for MsgCollector {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        if name == "message" {
            let raw = format!("{value:?}");
            self.msg.push_str(raw.trim_matches('"'));
            self.msg.push(' ');
        } else if name == "trace_id" {
            // trace_id 由 span scope / thread-local / gen 三级兜底单独取（5 段格式 traceid 段），
            // event 显式带 trace_id 字段时跳过避免重复。
        } else {
            self.extra.push((name.to_string(), format!("{value:?}")));
        }
    }
    // record_str 同理
}
```

### format_event 渲染 (logging.rs:451 msg 段)

```rust
// 4. msg (含 message 主体 + 业务字段 key=value)
write!(writer, "{msg} ")?;
for (k, v) in &msg_visitor.extra {
    write!(writer, "{k}={v} ")?;
}
```

### spec PATCH (logging-format.md)

在「日志字段顺序 (MUST)」段补一条：
> msg 段 MUST 包含 event 全部业务字段 (key=value 按记录顺序)，**禁丢字段**。trace_id 字段例外（5 段格式 traceid 段单独取，event 显式带则去重）。

## Decision (ADR-lite)

**Context**: trace-id task 改 AidogFormat 时假设字段都在 span，MsgCollector 只收 message。实际业务 event (SQL profile / HTTP 诊断) 把字段挂 event，导致全丢。
**Decision**: MsgCollector 收所有字段，message 进 msg 主体，其他 key=value 追加 msg 段尾。trace_id 字段去重（5 段格式 traceid 段单独取）。
**Consequences**: 5 段格式不变，业务字段回归 msg 段；trace_id 单点去重避免重复；logging-format spec 补字段渲染契约。

## Out of Scope

- 5 段格式调整（保持 time/level/file:line func/msg/traceid）
- 新加字段段（业务字段塞 msg 段，不开新段）
- JSON 结构化日志（仍文本格式）

## Technical Notes

- bug 点: logging.rs:462-482
- format 调用点: logging.rs:414-454
- trace.rs:48-68 SQL profile 回调（典型受影响场景）
- spec: .trellis/spec/backend/logging-format.md
- 前任务: 07-06-trace-id-log-format (已 archive, 引入此回归)
