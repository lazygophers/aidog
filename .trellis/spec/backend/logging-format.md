---
updated: 2026-07-06
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# 日志格式 + traceid 取值链

何时被读: 改 `src-tauri/src/logging.rs` 的格式器 / 新加 `tokio::spawn` 异步任务 / 新加诊断 id / 改 traceid 取值链 时
谁读: main / sub-agent
不遵守的代价: traceid 在日志行搜不到 (header↔日志脱钩) / 异步分支孤儿 id (父子树断) / dev 与 release 格式漂移 / 重复实现格式器。`07-06-trace-id-log-format` 实证。

---

## 日志字段顺序 (MUST)

> 违反代价: 用户诊断时按位置 grep 失败, dev/release 字段顺序不一致需两套解析。

- **MUST 5 段严格顺序**: `time` → `level` → `file:line func` → `msg` → `traceid`
- **禁增减字段** (新加诊断字段统一塞 traceid 段或 msg 内, 不开新段)
- **console 与 file 共用 `FormatEvent` impl**, 仅 ANSI 标志不同, 字段顺序对称

## ANSI 着色 (MUST)

- **console MUST ANSI on** (`AidogFormat { ansi: true }`), file MUST ANSI off (`ansi: false`)
- **level 动态色**: error=31 红 / warn=33 黄 / info=32 绿 / debug=34 蓝 / trace=90 灰
- **字段固定色**: time=90 灰 / file:line func=35 紫 / msg 默认 / traceid=36 cyan
- **禁 file ANSI**: ANSI escape 序列污染 grep, file 行必须纯文本

## traceid 取值链 (MUST)

> 违反代价: 日志行无 id 可 grep = 诊断 header 设计目的 (header↔日志映射) 失效。

- **每行 MUST 含 traceid**, 无条件 (不分启动来源 / debug/release)
- **取值三级兜底 MUST**: `trace_id_from_span_scope` (span scope walk) → `current_trace_id()` (thread-local 栈) → `gen_trace_id()` (现场新生 root)
- **禁字面 `-` / `unknown`** 占位 — 用户明确"无论从哪启动都必须有 id"

## id 格式规范 (MUST)

- **每级 id MUST 6 位 `[0-9a-z]`** (36^6 ≈ 2.2B 空间)
- **多级 MUST `.` 分割**: 顶级 `a3f9k2` / 二级 `a3f9k2.b7x1mq` / 三级 `a3f9k2.b7x1mq.c2p9nd`
- **父子树 MUST 可 grep**: 顶级 id grep 捞回整棵子树

## id 双轨映射 (MUST)

> 违反代价: proxy 请求 header id 不能反查 proxy_log 行; 或全局统一随机失去诊断关联。

- **proxy 请求顶级 id MUST** = `trace_id_from_request_id(request_id)` = request_id (32-hex proxy_log.id) 的 base36 前 6 位
- **非 proxy 路径** (命令 / 后台 / 健康端点) MUST = `gen_trace_id()` 独立随机
- **异步分支** 无论父路径类型, MUST `gen_child_id(parent)` = `parent + "." + 6 [0-9a-z]`
- **request_id 字段** (32-hex) 保持不变, 仍入 proxy_log 主键

## 异步分支 id 传播 (MUST)

> 违反代价: thread-local 栈在 tokio spawn 后失效 (跨线程执行不继承), 子任务内 traceid 变孤儿。

- **新加 `tokio::spawn` MUST 走 `spawn_traced(name, fut)` helper** (`logging.rs::spawn_traced`)
  - helper 内: `current_trace_id().unwrap_or_else(gen_trace_id)` 取父 → `gen_child_id(parent)` 生成子段 → `info_span!("spawn", name, trace_id=child).instrument(fut)` → `tokio::spawn`
- **不能走 spawn_traced 的场景** (handle.spawn 需保留 Drop guard / 调用方已 instrument 双重顾虑): MUST 手动等价 instrument + 注释说明"spawn_traced 不适用原因"
- **禁裸 `tokio::spawn(fut)`** 无 instrument — 异步分支丢父子关联

## thread-local 栈角色 (MUST)

- **thread-local `TRACE_ID_STACK` 仅同步业务代码 fallback** (inject_trace_header 等同步路径)
- **async 取值 MUST 走 span scope walk** (`trace_id_from_span_scope`), 不依赖 thread-local
- **业务同步代码读 `current_trace_id()`** (thread-local 主) — 类型擦除不暴露 LookupSpan, 无法 walk; async spawn 失效由 spawn_traced instrument 修复

## 健康端点 span (MUST)

> 违反代价: 健康端点无 span → inject_trace_header 兜底现场造孤儿 id, header↔日志脱钩。

- **`health.rs::handle_root` MUST 包 `info_span!("health", trace_id=...)` + 一行 `tracing::info!("health probe")`** 让 tid 进日志
- **顶级 tid 取值**: `current_trace_id().unwrap_or_else(gen_trace_id)`

## 验收基准 (可复用)

- [ ] debug build: header `x-aidog-trace` id grep 日志命中 ≥1 行 (含健康端点)
- [ ] 异步分支: 顶级 id grep 拿到全树 (`父.*` + `父.子.*`)
- [ ] console ANSI 序列存在 (`\x1b[3Xm`), file 纯文本无 ANSI
- [ ] traceid 每行必有 (无 `-` / `unknown` / 空)
- [ ] id 格式: 6 [0-9a-z], 多级 `.`
- [ ] grep `tokio::spawn\|handle.spawn` src-tauri/src 残留清单: 仅余 spawn_traced 自身定义 / 手动 instrument + 注释点 / Command::spawn (ca.rs) / 测试代码

## 验证命令

```bash
# 格式器装在 console + file 两层
grep -n "AidogFormat\|event_format" src-tauri/src/logging.rs

# 裸 tokio::spawn 残留 (除 spawn_traced 定义 / Command::spawn / 测试)
grep -rn "tokio::spawn\|handle\.spawn" src-tauri/src --include="*.rs" | grep -v "test\|//\|Command::spawn\|spawn_traced"

# id 6 [0-9a-z] 格式断言
grep -n "is_ascii_digit\|is_ascii_lowercase\|0123456789abcdef" src-tauri/src/logging.rs

# 健康端点 span
grep -n "info_span\|health probe" src-tauri/src/gateway/proxy/health.rs
```

## 跨层 / 关联 spec

- [Proxy Diagnostic Headers](./proxy-diagnostic-headers.md) — header 注入侧契约 (本 spec 是日志侧对偶), `current_trace_id() → unwrap_or_else(new_trace_id)` 取值链 MUST 两边一致, header id ↔ 日志 traceid 字段格式严格对齐
