# PRD: trace-id 全覆盖 — 全链路追踪

## 背景

前序已为 proxy 请求链加 trace-id（`handle_proxy` → `info_span!("req", id=...)`，覆盖 inbound→route→upstream→completed + mock/passthrough）。但其余日志入口无 id，无法串联：
- 69 个 tauri 命令（lib.rs）+ 其触发的 db/quota 调用
- proxy `handle_group_info` 端点（statusline）
- 后台 `tokio::spawn`：spawn_estimate、price_sync 周期、tray refresh
- 出站 quota 查询（命令触发时应继承命令 id；独立触发时需自身 id）

目标：**每条日志都带 trace_id**，可按 id grep 出完整一次操作的全部日志。

## 核心机制（tracing span 传播）

1. **同步/await 链自动继承**: 一个 span 内 `.await` 的所有子调用自动携带该 span 字段。故只需在**入口**建 span，下游（db/quota/router）零改动自动继承。
2. **tokio::spawn 不继承**: spawn 出的 future 脱离当前 span。需显式 `.instrument(span)`。要链回原请求 → `.instrument(tracing::Span::current())`（在 spawn 前捕获）；独立后台 → 新 span + fresh id。
3. **字段名统一**: 全部用 `trace_id`（proxy 现有 `id` 改名 `trace_id`，便于统一 grep `trace_id=`）。
4. **fmt 默认渲染当前 span** → 自动前缀，无需每宏手动加字段、无需改 logging.rs subscriber。

## 共享 helper（main 预置）

logging.rs 加 `pub fn new_trace_id() -> String`（`uuid::Uuid::new_v4().simple().to_string()` 取前 8 hex）。各处经 `crate::logging::new_trace_id()` 复用，禁各自造。

## 变更清单（零遗漏）

### 组 1 — lib.rs（69 命令）
- 每个 `#[tauri::command]` 加属性 `#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]`
  - `skip_all` 必须：不记参数（避免密钥泄漏 + 噪音），仅靠 fields 注入 id
  - span 名 = 函数名 → 日志前缀 `<cmd_name>{trace_id=xxxxxxxx}:`
  - 既有命令入口 `debug!("command invoked")` 保留（现在带 id）
  - 下游 db/quota/sync 调用自动继承，无需改
- 验证：69 个命令全部加属性；async fn 上 instrument 正确跨 await

### 组 2 — proxy.rs + price_sync.rs
- proxy.rs:
  - `handle_proxy` 现有 `info_span!("req", id=...)` 字段名 `id` → `trace_id`
  - `handle_group_info` 入口加 `info_span!("group_info", trace_id=%new_trace_id())` + `.instrument()` 或 enter（async → 用 instrument 包裹 body / 或 wrapper 模式同 handle_proxy）
  - `spawn_estimate` 内 `tokio::spawn(async move {...})` → `.instrument(tracing::Span::current())`（spawn 前 `let span = tracing::Span::current()` 捕获请求 span，链回原请求 trace_id）
  - 其他 `tokio::spawn`（done-log 回写 :882 区、tray-refresh emit）→ 同样 `.instrument(Span::current())` 继承请求 id
- price_sync.rs:
  - 后台周期同步任务起点加 `info_span!("price_sync", trace_id=%new_trace_id())`，每轮一个 id（独立后台，非请求触发）

## 非目标
- 不改 logging.rs subscriber 格式（fmt 默认渲染 span 已足够）
- 不改 ProxyLog DB schema（proxy req 仍复用 request_id 作主键，span 取其前 8 位）
- 不引入分布式 tracing（OpenTelemetry 等）
- 不改前端

## 验收标准
1. `cargo check` 0 warning 0 error
2. 69/69 命令带 `#[tracing::instrument(skip_all, fields(trace_id...))]`（grep 计数）
3. spawn_estimate / 其他 request 内 spawn 用 `Span::current()` 链回原 id
4. price_sync 周期任务有独立 trace_id
5. proxy req span 字段统一为 `trace_id`
6. 脱敏：skip_all 确保命令参数不入日志；grep 无明文 api_key/Bearer
7. 手动验证（可选 runtime）：跑一次命令 + 一次代理请求，终端每行均有 `trace_id=`，同一操作 id 一致

## 失败处理
- instrument 属性致某命令编译失败（如非 async 或返回类型冲突）→ 该命令单独处理，报告列出
- spawn future 类型不满足 Instrument → 检查 import `use tracing::Instrument`
- 不确定某 spawn 该继承还是新 id → 请求触发的继承（Span::current），独立后台新 id
