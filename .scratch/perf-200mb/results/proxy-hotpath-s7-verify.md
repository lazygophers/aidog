# 转发热路径缓冲与拷贝治理 — 整体验收 (proxy-hotpath-buffers s7-verify)

## 方法

- baseline = `e955d2d7`（s1 之前，本 task 起点的父提交），current = `c9d78aea`（HEAD，s1-s6 + 独立
  task `sse-chunk-line-reassembly` 均已落地）。
- baseline 代码经 `git worktree add /tmp/aidog-verify/baseline e955d2d7`（只读检出，禁改动、跑完即
  `git worktree remove`）编译；`Cargo.lock` 与 current 逐字节相同 → `CARGO_TARGET_DIR` 指向同一
  target 目录复用依赖编译缓存（仅 aidog_core/aidog 两 crate 各自重编，未违反"量测窗口内禁并发
  cargo build" —— 两次构建严格顺序执行，未与压测窗口重叠）。
- token/est_cost 逐条比对用 Path A（进程内集成测试，内存 SQLite，`test_integration.rs` 末尾临时
  追加 `temp_dump_token_cost_matrix`，跑完 `git checkout --` 还原）：6 组确定性 mock 请求（含流式/
  非流式、3 个模型、中文+emoji 混合内容），落 `(input_tokens,output_tokens,cache_tokens,est_cost)`
  到文件，baseline/current 各跑一次 `diff`。
- phys_footprint + TTFT 用 Path B（真实二进制，非 GUI .app 而是 `cargo build --release --bin aidog`
  直接产物，前端 dist 复用当前已构建产物，因本 task 未改前端）：`HOME` env 重定向到隔离数据目录
  （`/tmp/aidog-verify/home-{baseline,current}`，全新 `~/.aidog/aidog.db`，**未碰真实用户库**），
  临时 `examples/seed_mock.rs`（跑完即删）用 `create_platform`/`create_group`/`set_group_platforms`
  种一个 mock 平台 + `gkverify` 分组。50 路并发 curl `-N` 打 `/v1/messages`（`chunk_count:200,
  delay_ms:50` 长流），`footprint -p <pid>` 每 1.5s 采样取峰值，负载窗口内额外发一条
  `time_starttransfer` 测 TTFT。每档独立重启进程，各跑 2 轮。

## 结果

### 1. phys_footprint 峰值（50 路并发窗口内采样）

| 档位 | run1 峰值 | run2 峰值 |
|---|---|---|
| baseline (e955d2d7) | 53.0 MB | 45.0 MB |
| current (HEAD)      | 45.0 MB | 48.0 MB |

两档量级相当（进程本身小，mock 平台不产生真实上游网络缓冲），current 未劣化；无法在此量级
下做强归因（噪声窗口与信号同量级），已如实记录不编造下降幅度。**验收项 1（已采并落盘）满足。**

### 2. token 数与 est_cost 逐条一致

```
0	claude-sonnet-4-20250514	false	100	50	0	0.0010500000000000002
1	claude-sonnet-4-20250514	true	107	53	0	0.001116
2	claude-sonnet-4-20250514	true	114	56	0	0.001182
3	claude-3-5-haiku-20241022	false	121	59	0	0.00054
4	claude-3-5-haiku-20241022	true	128	62	0	0.00057
5	gpt-4o	true	135	65	0	0.0009875
```
baseline / current 两份文件 `diff` 零差异，6 条全部逐字节一致（含流式 + 非流式 + 3 个模型 +
中文/emoji 混合内容）。**验收项 2 满足。**

### 3. 首 token 时延（负载下 TTFT，`time_starttransfer`）

| 档位 | run1 | run2 |
|---|---|---|
| baseline | 0.211s | 0.292s |
| current  | 0.209s | 0.117s |

current 两轮均 ≤ baseline 两轮（噪声主要来自 50 路 curl 争抢本机调度，量级一致，无回归信号。
**验收项 3 满足（有数据，无回归）。**

### 4. s1 复现用例

`cargo test -p aidog_core --lib -- utf8_char_split` → 2 passed（`utf8_char_split_across_
network_chunk_corrupts_chinese_content` / `..._emoji_content`）。**验收项 4 满足。**

### 5. clippy

`cargo clippy --workspace --all-targets` → 零 clippy warning（仅 ts-rs derive 宏自身的
`note: ts-rs failed to parse this attribute` 提示，非 clippy lint，项目既有噪声非本 task 引入）。
**验收项 5 满足。**

### 6. cargo test --workspace

1639 passed / 1 failed / 5 ignored。失败项 `gateway::quota::http::test_http::
quota_get_json_network_error` 单跑 `ok`（网络依赖 flaky，非本 task 引入，属已知例外）。
**验收项 6 满足。**

### 7. 全程只用 mock

seed 的平台 `platform_type=mock`，分组 `gkverify`；loadgen 全部走 `/proxy/v1/messages` +
`Authorization: Bearer gkverify`；token/cost 集成测试全程 `Protocol::Mock`；无真实上游调用。
**验收项 7 满足。**

### 8. 清场

- `test_integration.rs` 的临时 `temp_dump_token_cost_matrix` 已 `git checkout --` 还原。
- `crates/aidog_core/examples/seed_mock.rs` 已删除，`examples/` 目录已清空并移除。
- `/tmp/aidog-verify/`（baseline worktree + 两份隔离 HOME + 采样文件）已整体 `rm -rf` +
  `git worktree remove`。
- 还原后重跑 `cargo build --release --bin aidog` 确认可正常编译。
- `git diff --stat` 核对：仅 `.skein/task/proxy-hotpath-buffers/task.json`（状态跟踪）改动，
  源码零残留。**验收项 8 满足。**

## 结论

六项改动（s1-s6）整体行为验证通过，无回归。s7-verify 全部 8 条验收标准 pass。
