# trace-id 注入日志格式让 header↔日志可 grep

## Goal

`x-aidog-trace` 响应 header (已沉淀于 `proxy-diagnostic-headers.md`) 在 debug build 已注入, 但当前 `tracing` 默认 fmt 格式器不打 span 字段, console + file 日志行里**完全没有 trace_id 串**, 用户拿到 header id 后无法 grep 对应日志 → 诊断 header 设计目的 (header↔日志映射) 失效。本任务让 trace_id 出现在每条日志行, header→日志可 grep 直达。

## What I already know (auto-context)

**缺口 A (核心, 所有请求受影响)** — `src-tauri/src/logging.rs:157-161` (console layer) 与 `181-184` (file layer) 用 `tracing_subscriber::fmt::layer()` 默认格式器, 默认 **只打 span 名 + event message, 不打 span 字段** (trace_id 是 span 字段, 不是 event 字段)。

- `TraceIdLayer` (`logging.rs:56-86`) 只把 trace_id 存进 span extensions + 线程本地栈, 给 `inject_trace_header` 用
- 它**没改 fmt 输出格式**, 所以 log 行里 0 trace_id 痕迹
- 影响: 所有请求 (含正常代理请求 handler.rs:13 `info_span!("req", trace_id=..., request_id=...)`) 的 trace_id 在日志里都搜不到

**缺口 B (仅健康端点)** — `src-tauri/src/gateway/proxy/health.rs:6` `handle_root()` 无 span 包, `inject_trace_header` 调 `current_trace_id()` → None → `new_trace_id()` 现场造一个新 id。

- 该 id **从未被写过任何日志行** (因为没 span → 没 event 在该 span 内 → 日志里压根不存在)
- `32419a60` 这种 8-hex = helper 兜底新建的 trace_id, grep 0 结果完全符合当前设计

## Deep Requirement (用户本轮明确)

> "每一条日志都必须存在 reqid, 无论是从哪启动的。如果是一个流程中出现了异步, 异步的部分要有独立的 traceid, 比如主流程是 111, 出现了并行, 其 traceid 应该是 111.ferq, 这样才可以明确知道都是什么问题。"

拆解:
1. **每条日志行无条件含 id** — 不分启动来源 (命令 / 代理请求 / 后台), 不分 debug/release
2. **异步分支独立 id 但可映射父** — `父id.子段` 编码, 子段格式待 brainstorm
3. **header↔日志 grep 直达** — `x-aidog-trace` 的 id 能在日志里命中

### 现状架构障碍 (auto-context)

`logging.rs:7-15` `TRACE_ID_STACK` 是 **thread-local**, 而 tokio::spawn 新 task 跨线程执行, thread-local 栈**不跨 task 继承** → spawn 出的异步分支内 `current_trace_id()` = None → 兜底 `new_trace_id()` 造**孤儿 id**, 与主流程脱钩 = 用户痛点根因。

tracing span scope 本身**跨 .await 自然继承** (span 跟 future 走, 不依赖 thread), 但 thread-local 栈不走 future, 故现状 thread-local 设计在 async spawn 下失效。

## Decision (ADR-lite, 待 brainstorm 确认)

- **A 必修** (每行有 id) + **B 必修** (健康端点包 span)
- 修法待定 (见 Open Questions): id 取值源改为走 tracing span scope (替代 thread-local) / 保留 thread-local + 加 spawn 传播 helper
- 子段 id 格式待定: 序号 / 命名 / 短 hex

## 日志格式规范 (用户本轮明确)

字段顺序 (强制):
1. **时间** (timestamp)
2. **level** (动态颜色: error 红 / warn 黄 / info 蓝 / debug 灰 / trace 更灰, 标准约定)
3. **filepath:line func name** (源码定位)
4. **msg 主体**
5. **traceid** (放最后)

每字段专属颜色:
- console 用 ANSI 着色
- **file 必须 plain** (with_ansi(false) 已是现状 logging.rs:182, 保留; ANSI 进文件污染 grep)
- 各字段独立着色 (非整行单色)

## Requirements (evolving)

- 每条 console + file 日志行含 id (无条件, 无 span 兜底策略待定)
- 日志行格式: `时间 level filepath:line func msg traceid` 五段, 每段独立颜色 (console), file plain
- 异步分支 id = `父id.子段`, 与父可映射
- header id 与日志 id 取值链一致
- release build 日志格式不回归 (release 不注入 header 但日志行格式改动对称)

## traceid 格式规范 (用户本轮明确)

- 每级 id = **6 位随机**, 字符集 `[0-9a-z]` (数字 + 小写字母, 36^6 ≈ 2.2B 空间)
- 多级用 `.` 分割: 顶级 `a3f9k2` / 二级 `a3f9k2.b7x1mq` / 三级 `a3f9k2.b7x1mq.c2p9nd`
- 顶级 = root span (命令/请求入口), 子级 = 异步分支 (spawn / 显式 instrument 子 span)
- 父子关系可逆: 日志里看到 `a3f9k2.b7x1mq.*` 全部 grep `a3f9k2` 可捞回整棵树

> 与现状冲突: 现 `new_trace_id()` 生成 8-hex (logging.rs 命名空间 [0-9a-f]), `request_id` 是 32-hex (proxy_log.id)。本任务统一改为 6-[0-9a-z] 顶级格式, 异步分支加 `.`+6 位。request_id (32-hex proxy_log 主键) 保持不变, 仅日志行 traceid 字段走新格式。

## 决策 (用户已拍板)

| 项 | 决策 |
|---|---|
| 异步分支传播 | **spawn_traced helper** — wrap tokio::spawn, 自动基于父 id 生成子段 + instrument, 改全部 tokio::spawn 调用点 (~13 处, `tokio::spawn` + `handle.spawn`; `Command::spawn` 是 std::process 不动) |
| 无 span 兜底 | **现场新生 root** — 无活跃 span 时 `new_trace_id()` 生成一个独立 6-[0-9a-z] root, 接受 grep 不到父子树的权衡 |
| 颜色方案 | **标准 ANSI** — error 红31 / warn 黄33 / info 绿32 / debug 蓝34 / trace 灰90; 字段: time 灰90、file:line func 紫35、msg 默认色、traceid cyan36 |
| ANSI 生效域 | **console ANSI on / file ANSI off** (file 纯文本, 已是现状 logging.rs:182 `with_ansi(false)`, 保留) |

## 实现路径 (design.md 详)

- 新 `gen_trace_id()` 6 [0-9a-z] 替换现 8-hex
- 新 `gen_child_id(parent)` = `parent + "." + gen_trace_id()`
- `spawn_traced(fut)` helper: 读 `current_trace_id()` → 兜底 gen root → 子 span 用 child id instrument → tokio::spawn
- 自定义 `FormatEvent` impl: 字段顺序 time/level/file:line func/msg/traceid, console ANSI on, file ANSI off
- traceid 取值: span scope walk (跨 async 继承) 替代 thread-local 栈 (thread-local 在 spawn 后失效 = 用户痛点根因)
- 13 处 tokio::spawn + handle.spawn 改 spawn_traced

## Out of Scope

- 改 `TraceIdLayer` 的 span 字段捕获机制 (已工作正常)
- 改 `inject_trace_header` helper (已沉淀于 spec, id 取值链不变)
- 调整 retention / rotation
- request_id (32-hex proxy_log 主键) 保持不变

## Acceptance Criteria (evolving)

- [ ] debug build: 拿响应 header `x-aidog-trace` 的 id, grep 日志能命中至少一行
- [ ] release build: 日志格式无回归 (release 不注入 header 但仍需保证日志格式改动不破坏现有日志解析)
- [ ] `cargo clippy --lib` 0 项目警告
- [ ] 单测: 验证格式器在带 trace_id 的 span 内产生的 event 含 trace_id 串

## Out of Scope

- 改 `TraceIdLayer` 的 span 字段捕获机制 (已工作正常)
- 改 `inject_trace_header` helper (已沉淀于 spec)
- 调整 retention / rotation

## Open Questions

1. A 修法: 自定义 fmt format 注入 trace_id 到每行 (全行可 grep) / `.with_span_events(FmtSpan::ACTIVE)` (轻, 仅 span 边界) / 其他
2. B 是否修: 健康端点包 `info_span!` + 一行 log 让健康探测可追溯 / 不修 (健康端点无需日志追溯)

## Technical Notes

- 唯一改动点: `src-tauri/src/logging.rs` 的 console_layer + file_layer 格式器构造 (debug + release 都改)
- 可能改 `health.rs` (如 B 修)
- 已读 spec: `.trellis/spec/backend/proxy-diagnostic-headers.md` (header 注入侧契约, 本任务是其日志侧对偶)
