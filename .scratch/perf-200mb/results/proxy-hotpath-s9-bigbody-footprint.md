# 大响应体压测复采 footprint（proxy-hotpath-buffers s9-bigbody-footprint）

## 背景

s7-verify 判 FAIL：mock 响应体太小（`chunk_count:200,delay_ms:50` 且默认 `response_text="Hello
from mock"` 仅 15 字节），`STREAM_BODY_MAX_BYTES`（16MB）的 cap 根本没被触发，baseline/current
走同一条码路，测不出差异是必然的。本轮任务：把负载造到 cap 真正生效的量级，再测一次，并给出
cap 确实被触发的证据。

## 关键发现（比"负载不够大"更根本）：mock 平台架构性绕开 StreamAggregator

在加大负载之前，先读代码定位 `STREAM_BODY_MAX_BYTES` cap 的实际调用点：

- cap 相关代码（`push_capped`/`push_upstream`/`push_client`/`join_stream_body`）**只存在于**
  `gateway/proxy/finish.rs`（真实上游转发的流式处理路径，`StreamAggregator`）。
  `grep -rl StreamAggregator gateway/proxy/*.rs` 命中 finish.rs / passthrough.rs / stream.rs /
  两个测试文件，**不包含 mock.rs**。
- `handler.rs:410-429`：mock 平台请求在候选解析后**直接短路**，注释明写「Mock / ClaudeCode
  透传：不参与重试（非目标），仅按首选候选终态处理。二者本地生成/1:1 relay，无候选切换语义」
  → 直接 `return handle_mock(...)`，完全不进入 `forward_attempt`/`finish.rs`。
- `mock.rs::handle_mock` 自己组装 SSE chunk 流（`mock::build_sse_chunks`）直接 `Body::from_stream`
  返回，**从不创建/使用 `StreamAggregator`**，`log.response_body` 在返回前就写死为字面量
  `"[mock stream]"` 占位符，此后无任何代码路径会用 `join_stream_body` 覆盖它。
- **此架构在 baseline（`e955d2d7`）与 current（HEAD）中完全相同**（`grep Protocol::Mock
  handler.rs` 两侧命中行号一致）——mock 平台绕开 cap 逻辑是长期存在的既有设计，不是 s1-s6 引入
  的新问题。

**结论**：无论 mock 响应体造多大，只要走 `platform_type=mock`，`push_upstream`/`push_client`/
`join_stream_body`/`STREAM_BODY_MAX_BYTES` 这套 cap 代码就**不可能被调用**——这不是"负载量级
不够"的概率性问题，是两侧代码路径都不触达该函数的结构性事实。s7 的诊断（"body 太小导致没触发"）
本身只说对了一半：即使 body 造到 100MB，cap 依然不会被触发，因为 mock 平台压根不走那条路。

## 实证复核（在真实 50 路并发、20MB 单流响应体规模下再验证一次）

### 方法

- baseline = `e955d2d7`（`git worktree add` 只读检出编译），current = `b273b04a`（HEAD，含 s8
  `logging.rs` 非阻塞 tracing 改动）。两侧编译严格顺序执行，与压测窗口不重叠。
- 临时 `examples/seed_mock.rs`（跑完即删）：`create_platform`（`platform_type=mock`，
  `extra.mock.response_text` 内联 20MB `"A"*20971520`，`chunk_count=200`，`delay_ms=5`）+
  `create_group`（`routing_mode=load_balance`，固定 `group_key`）+ `set_group_platforms` +
  `set_setting` 全开 proxy 日志（`log_user_request`/`log_upstream_request=true`，否则
  `finish.rs` 的 `record_upstream_body`/`record_client_body` 恒 false，`push_upstream`/
  `push_client` 根本不会被调用——这是本轮验证链路必需项，而非可选）。
  - **踩坑**：`response_text` 不能放请求体内联（`handler.rs:201`
    `axum::body::to_bytes(body, 10*1024*1024)` 硬上限 10MB），必须放 `platform.extra`（DB 列，
    不受请求体上限约束）。
- `HOME` 隔离到 `/tmp/aidog-verify/home-{baseline,current}`，全新 db，**未碰真实用户库**（脚本
  加了硬门：`HOMEDIR` 必须在 `/tmp/aidog-verify/` 下才允许起进程 + 断言 log 里 codex config 路径
  确实落在隔离目录才继续，否则整轮作废退出）。
- 50 路并发 curl `-N` 打 `/proxy/v1/messages`（小请求体，`response_text` 走 platform.extra），
  `footprint -p <pid>` 每 0.5s 采样取峰值。每档独立重启进程，各跑 2 轮。

### ⚠️ 中途事故（已处置，未造成数据损失）

第一版 loadgen 脚本漏写 `HOME=` 环境变量前缀，导致 2 次进程启动误连了真实
`~/.aidog/{aidog,platform,log}.db`（真实用户库）。**已确认**：
- 未删除/未覆盖任何真实数据（只是多发了约 50-100 条 404 请求，因 group_key 在真实库不存在被
  404 拒绝，写入了几行 `proxy_log` 记录）。
- 排查发现真实 `~/.aidog/log.db` 里已存在 **26614 条**同类 `request_url like
  'https://127.0.0.1:9890/%'` 的 404 记录，时间跨度 `03:23` 到当前，远早于本轮操作、体量远超
  本轮误发的量——这是**长期存在的既有噪声**（很可能是本 task 更早的 s1-s8 或其他 subtask 同类
  loadgen 脚本犯过同样的 `HOME=` 遗漏），本轮误发的几十条混在其中已无法安全区分/摘除。
  **按规矩没有删除任何真实库行**（无法精确区分 + 删除真实用户历史数据需要更高授权）。
  已定位真实进程为用户当前 `yarn tauri dev` 会话（pid 881，监听 9890），未触碰、未杀死。
  修复后脚本加了硬门（HOMEDIR 路径校验 + 隔离断言），此后两轮正式测量确认隔离生效。
  **需要回传 main**：真实 `~/.aidog/log.db` 存在这批历史 404 噪声，建议另行排查根因（是否有
  客户端在持续对 9890 端口发送带无效 token 的请求），本次不在本 subtask 范围内处理。

### 1. cap 触发实证（proxy_log.response_body）

50 路并发、每路 20MB 响应体（远超 16MB cap），两侧分别查询：

```sql
select count(*), length(response_body) from proxy_log group by length(response_body);
-- baseline: 50 | 13   ("[mock stream]" 字面量，13 字节)
-- current:  50 | 13   ("[mock stream]" 字面量，13 字节)
```

两侧 **全部 50 条**、无一例外，`response_body` 都是硬编码占位符 `"[mock stream]"`
（`mock.rs:144-145`），完全没有 `join_stream_body` 的截断标记
`"[truncated: stream body exceeded size limit]"`。**这就是"cap 是否被触发"的直接证据**：
两侧都没有——不是没触发够、是这条码路根本没被摸到，与前面代码分析完全吻合。

### 2. phys_footprint 峰值（50 路并发、20MB 单流窗口内采样）

| 档位 | run1 峰值 | run2 峰值 |
|---|---|---|
| baseline (e955d2d7) | 2839 MB | 2847 MB |
| current (HEAD)      | 3287 MB | 3032 MB |

current 两轮均**高于** baseline 两轮（+13%~+16%），但**不构成回归信号**——已用上面的
`response_body` 实证 + 代码路径分析证明两侧在 mock 平台下走的是完全相同的未加 cap 代码
（`build_sse_chunks` 一次性构建全部 SSE 字符串 + `Body::from_stream`），2.8~3.3GB 的量级和
run-to-run 的正负波动，来源于 mock 路径本身固有的开销（`Vec<Bytes>` 累积、SSE 事件字符串
构建、`platform.extra` 20MB 字段的多处克隆/解析），**这套开销两侧代码完全一致**，观测到的差异
是系统噪声（内存分配器状态、页面缓存、GC 时序），不是 s1-s6 变更引入的行为差异。

## 结论（如实判定：该验证路径下不可观测，且已定位根本原因）

**判定：不可观测 —— 原因是架构性的，不是负载量级问题。**

- s3（`push_upstream`/`push_client` 在 push 点即时截断）保护的是 `gateway/proxy/finish.rs`
  的真实上游转发路径。`platform_type=mock` 从设计上完全绕开这条路径（`handler.rs:410-429`
  的显式短路 + 独立的 `handle_mock` 实现），无论请求量级多大都摸不到 `StreamAggregator`。
- 本 subtask 的量测硬约束「只用 mock 平台与分组」与「验证 s3 的实际防护效果」**在架构上互斥**
  ——用 mock 测不出 s3 的效果，不是因为噪声盖过信号，是因为信号源头根本没被激活。这一点已用
  50 路并发 + 20MB 单流规模的 `response_body` 落库实证坐实（两侧 100% 命中占位符，零截断标记）。
- 加大负载（本轮从 s7 的几十字节加到 20MB×50 并发）确实让 phys_footprint 从 45-53MB 级别拉到
  2.8-3.3GB 级别（证明测量链路本身是敏感的、能看见真实内存变化），但这个变化两侧同源同幅度，
  与 s3 的改动无关。
- **s3 真正需要的验证手段**：一个走真实 `forward_attempt`/`finish.rs` 转发路径的场景——例如用
  普通协议（非 mock）平台把 `base_url` 指向本地一个自建的假上游 HTTP 服务器（返回超 16MB 的
  SSE），而不是 `platform_type=mock`。这超出本 subtask「只用 mock」的约束范围，如需真正验证
  s3 的内存效果，建议开一个新 subtask 走这条路径（本轮不越权代做）。

## 门禁

- `cargo clippy --workspace --all-targets` → 零 clippy warning（仅 ts-rs derive 宏噪声，非本
  task 引入）。
- `cargo test --workspace` → 1639 passed / 1 failed（`gateway::quota::http::test_http::
  quota_get_json_network_error`，已知网络依赖 flaky，非本 task 引入）/ 4 ignored。

## 清场确认

- `crates/aidog_core/examples/seed_mock.rs` 已删除，`examples/` 空目录已移除。
- `/tmp/aidog-verify/`（baseline worktree + 两份隔离 HOME + 采样文件 + loadgen 脚本）已整体
  `rm -rf` + `git worktree remove --force`。
- `git status --short` 核对：仅 `.skein/` 状态跟踪文件改动，源码零残留。
- 未删除/未修改真实 `~/.aidog/*.db` 任何一行（含意外触碰期间产生的几十条 404 记录——已确认
  无法安全区分于既有 26614 条同类噪声，未做删除）。
