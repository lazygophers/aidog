# PRD: 请求日志页 tokens/cost/条目/字段 记录问题修复

## 背景

用户报告「请求日志页(Logs)的日志、tokens 等记录有问题」，涉四类症状：① tokens/cost 记 0 或偏低 ② 日志条目缺失/异常 ③ 统计聚合数值不对 ④ 字段显示错乱。

经 DB 取证审计（`research/recording-chain-audit.md`，~/.aidog/aidog.db 1.09GB，3 个实证 request_id）：
- **症状③（聚合数值）实测无 bug**：est_cost 对账 `stats_agg $1115.75 == proxy_log(终态) $1115.75` 完全一致；5 条历史 token-recording 记忆全部「已修，未复发」。→ **不在本次范围**。
- 真正暴露的真 bug 是 count_tokens 计费污染 + status=0 卡死行 + 空 model 行展示 + path 过滤 latent bug。

## 目标

修复审计实锤的 4 个真 bug（P0/P1/P2/P4），消除 Logs 页 cost 虚高、条目无状态、字段空白，并修一处 latent 参数错位 bug。

## 范围内（4 项修复）

### P0 — count_tokens 计费污染（头号，4204 条 / $195.89 / 占全库 cost 17.6%）

**现象**：`/v1/messages/count_tokens` 纯计数子端点落了 `input_tokens` 进 proxy_log，`upsert_log` 的 cost gate（`proxy/log.rs:31,90`，仅判 `input>0||output>0`）对其算 est_cost，且 `first_agg` gate（`log.rs:26-28`）不排除它 → 计入 `stats_agg_hourly` → Stats 页/托盘总成本虚高 17.6%。注释 `count_tokens.rs:15`「不参与计费」与实际不符。

**用户决策（2026-06-26）**：count_tokens 调用 **单条 proxy_log 保留 input_tokens + est_cost**（供单行审计可见），**但不计入 stats_agg 聚合/总统计**（去 Stats/托盘虚高）。

**修复**：
1. 代码：count_tokens 路径写库时，proxy_log 行照旧保留 tokens + est_cost；但聚合写入路径（`first_agg` gate / `upsert_stats_agg`）识别 count_tokens 调用并**跳过 stats_agg 写入**。识别方式：`request_url LIKE '%count_tokens%'` 或落库时打标（择代码侵入小者，倾向复用 request_url 判定，避免加列迁移）。
2. 历史数据：已污染的 4204 条已写入 `stats_agg_hourly`。需**重建/扣除**历史 stats_agg 中 count_tokens 贡献，使 Stats 历史值与新规则一致（rebuild 受影响小时桶，或迁移按 count_tokens 行扣减聚合）。proxy_log 历史行不动（保留可见）。

**验收**：
- 新 count_tokens 请求：proxy_log 行有 tokens+cost；`stats_agg_hourly` 不含其贡献。
- 历史重建后：`SELECT sum(est_cost) FROM stats_agg_hourly` 比修前少约 $195.89（≈扣除 count_tokens 部分），与 `proxy_log WHERE request_url NOT LIKE '%count_tokens%' AND status_code!=0` 对账一致。
- Stats 页/托盘总成本不再含 count_tokens。

### P1 — status=0 卡死行（765 条）

**现象**：请求落库但从未达终态 HTTP status（连接中断/客户端断开/上游无响应/流式占位 `[stream]` 后未 flush 终态）→ `status_code=0`、`est_cost=0`、不进 stats_agg（终态 gate `status_code!=0` 已挡）。Logs 页这些行 status/tokens 列空白，用户感知「条目异常」。画像：350 条 `/v1/messages` 流式占位后未 flush，415 条混合端点。

**修复**：
1. 后端：请求结束（含错误/断连）路径补写终态 status；排查流式占位 `[stream]` 后 `StreamLogGuard` Drop 兜底是否覆盖客户端断连/上游中断（漏 flush 的分支补 flush 终态）。客户端断开可记显式标记（如 499 或专用语义）。
2. 前端：Logs 对 `status_code=0` 给「未完成/中断」语义标签，不再显示空白。

**验收**：
- 新断连/中断请求：proxy_log 终态 status 非 0（或带明确中断标记）。
- Logs 页 status=0/中断行显示明确语义标签，非空白。
- 不回归正常请求记录。

### P2 — 空 model 行展示（1334 条，model + actual_model 双空）

**现象**：1334 条（status=200 中 1.6%）`model='' AND actual_model=''`，Logs 列 model 直读空字符串显示空白。多为非标准端点（embeddings/models/health）或异常请求。**= 用户所指「字段显示错乱」（已确认就是 model 等列空白）**。

**修复**：
1. 前端：Logs.tsx model/actualModel 列空值给占位符（如 `-`，对齐 actual_model 既有 `|| "-"`）。
2. 后端：排查这批行落库时 model 列为何为空（非标准端点是否应回填端点名/请求 model；异常请求是否丢了 model 字段）。

**验收**：
- Logs 列空 model 显占位符，无裸空白。
- 后端能区分「合理空（健康端点）」与「异常丢失」，后者补回填。

### P4 — path 过滤漏 `idx += 1`（latent bug）

**现象**：`db/proxy_log.rs:396-402` path 过滤分支 push 绑定参数后无 `idx += 1`。path 是最后分支单用不报错，但后续若加绑定参数会错位。命中记忆 `logs-path-search-idx-bug`。

**修复**：补 `idx += 1`。

**验收**：path 过滤查询正确；加测试覆盖 path + 其后参数组合不错位。

## 范围外（明确不做）

- **症状③ 聚合数值**：实测对账一致，无 bug，不动。
- **P3 openai 流式 `stream_options.include_usage`**：真 bug 但 3 个实证 id 均 anthropic 协议、无命中实证，本次不修（另立 task 待补查命中数）。
- **P5 stats_agg SQLite-localtime vs Rust-local DST 一致性**：对账已基本一致，残留隐患低，本次不修。
- **P6 日志主开关**：用户确认「开着，没动过」→ ② 根因坐实是 status=0 卡死行，非主开关，不排查。
- **模型映射/上游兼容**（实证 `ec3a` 上游 400「不支持 claude-opus-4-8」）：属路由模型映射问题，非记录链 bug，不在四类症状内。

## 门禁

- Rust：`cd src-tauri && cargo build && cargo clippy`（warning 清零）+ `cargo test`（含新增 P0 聚合排除 / P1 终态 / P4 path 测试）。
- 前端：`yarn build`（tsc+vite 零错误）+ 涉 i18n 文案走 `node scripts/check-i18n.mjs`（P1 中断标签 / P2 占位符若加新 key 须 8 locale 全补）。
- DB 迁移（P0 历史重建）：迁移幂等 + 不误伤非 count_tokens 行。

## 实证与引用

- 审计全文：`research/recording-chain-audit.md`
- 实证 id：`ac90`(count_tokens 计费污染) / `33ac`(正常流式无 bug) / `ec3a`(上游 400 模型不支持，范围外)
- 相关记忆：`logs-path-search-idx-bug`、`group-stats-aggregation`、`ui-feedback-pinpoint-element`、`streaming-sse-log-aggregation`、`est-cost-persistence`
