# s6 — window-default-size 验收记录

验收时间 2026-08-01。对应 task.json 的 s6-verify 验收列表（6 项，200MB 判决项已移除，实际验收 5 项 + 1 项移交）。

## 逐条判定

| # | 验收项 | 判定 | 证据 |
|---|---|---|---|
| 1 | 200MB 判决已明确移交 perf-final-verification | 移交 | `.skein/task/perf-final-verification/task.json` deps 含 `window-default-size`，其 s3-budget-reconcile 负责总预算判决。本 task `curve-result.md` 结论 2/3（`.skein/task/window-default-size/curve-result.md:31-38`）已给出「1026×759 下 378.7MB，离 200MB 差 179MB，救不了目标」的事实陈述，但不在本 task 做总目标判决——依据 spec `[[window-size-memory-relation]]`（`.skein/spec/recall/optimization/window-size-memory-relation.md`）：窗口面积与内存呈负相关、非杠杆变量 |
| 2 | 曲线表与 spec 段落均已落盘 | 通过 | `curve-result.md`（60 行，3161 字节，Jul 29）含 4 档曲线表；spec `window-size-memory-relation.md`（51 行，2984 字节，Aug 1）落盘于 `.skein/spec/recall/optimization/` |
| 3 | yarn build 通过 | 通过 | `/tmp/wds-yarn-build.log` 末尾 `✓ built in 3.61s` + `YARN_EXIT=0`，全文 grep `error` 0 命中 |
| 4 | cargo build 通过 | 通过 | `/tmp/wds-cargo-build.log` 末尾 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 15.42s` + `CARGO_EXIT=0`，仅 ts-rs serde 属性解析 warning（既有噪声，非本次改动引入），grep `^error` 0 命中 |
| 5 | 全程只用 mock 平台与分组 | 不适用 | 本 task 全部 subtask 均无真实网络请求：s3 是纯配置文件改动（`tauri.conf.json` 删 `maximized:true`），s4 是静态代码审计（读 `src/` + grep，`layout-regression.md` 头部已声明「静态代码审计...未做像素级实渲染验证」），s5 是写文档（spec 落盘）。三者均不触发代理转发路径，无平台请求可言 |
| 6 | 临时脚本与逐次原始采样已清 | 移交 perf-final-verification 收尾时执行 | 见下方清单盘点，本次仅盘点、零删除 |

## `.scratch/perf-200mb/` 清单盘点（可删 / 须留 两栏，本次零删除）

### 须留（被下游引用）

| 文件 | 引用方 |
|---|---|
| `window-size-measure-protocol.md` | `perf-final-verification` s1-preflight 复用「三场景各自独立重启进程、等满 ≥10min 稳态、禁同进程内切场景连采」协议（其 desc 与验收项文字与本协议同源表述） |
| `assets/results/size-curve-raw.txt` | `curve-result.md:15` 直接引用为「原始盘」，曲线表数据来源 |
| `map.md` / `measure-protocol.md` | 通用量测台文档，`perf-final-verification` s1-preflight「量测脚本三场景各跑通一次冒烟」依赖同一套量测台 |
| `assets/measure.sh` / `assets/loadgen.sh` / `assets/run-size-curve.sh` / `assets/explain-baseline.sh` | 量测协议引用的可执行脚本本体，协议文档若留则脚本必须同留（否则协议不可复现） |
| `issues/03-target-feasibility-ruling.md` | 200MB 可行性判决记录，`perf-final-verification` s3-budget-reconcile 对账预算表的前置依据 |
| `issues/01` `02` `04`–`10`（tokenizer/sqlite/per-connection/idle-cpu/glass-buffer/s3-ambient 等） | 对应 `perf-final-verification` deps 里的兄弟 task（`tokenizer-residency-trim` / `sqlite-page-cache-residency` / `proxy-hotpath-buffers` 等）已引用或将引用的问题登记，非本 task 专属 |
| `results/proxy-hotpath-*.md` / `results/sqlite-cache-*` / `results/baseline-*` | 兄弟 task 的实测产物，同属共享量测台，非本 task 私有临时件 |
| `mock-loadgen-50x5min.md` | `perf-final-verification` deps 含 `mock-loadgen-capability`，同一压测流形文档 |
| `assets/research-wkwebview-floor.md` | `issues/02-wkwebview-floor.md` 的调研支撑材料，同引用链 |

### 可删（本 task 私有、无下游引用迹象）

| 文件 | 说明 |
|---|---|
| `assets/results/cpu-idle-*.txt`（4 个：foreground/hidden/small-window/visible-again） | window-default-size s1/s2 量测期间的逐次 CPU 原始采样，曲线表已提炼进 `curve-result.md`，未见被其他文档引用 |
| `assets/results/mem-idle-10min.txt` / `mem-idle-hidden.txt` / `mem-cold-start.txt` / `track-idle-10min.txt` | 同上，逐次原始采样，已提炼 |
| `assets/results/mem-rel-P1-large.txt` / `mem-rel-P2-1150x750.txt` | 曲线表 4 档中 2 档的逐次原始采样，已提炼进 `curve-result.md` 表格 |
| `assets/results/cpu-load-50*.txt` / `cpu-s7-after-run*.txt` / `mem-load-50*.txt` / `mem-s7-after-run*.txt` / `s7-regime-heap-crosscheck.txt` | 疑似 `proxy-hotpath-buffers`（s7 命名对应其 subtask 编号）的逐次采样，非 window-default-size 产物；**建议移交该 task 自行判定**，本次不代为处置 |
| `assets/.pids` | 量测期间的进程 pid 记录临时件，30 字节，无信息价值 |

**处置口径**：以上「可删」栏本次**一个文件都未删除**，均按团队指令移交 `perf-final-verification` 收尾时统一执行（该 task 的 PRD 把清场规定在自己结束时做，且 s1-preflight 需要先确认量测台可复现，过早删除会破坏其冒烟验证）。删除动作需 main 请用户裁定。

## 结论

5 条可判定项：4 项**通过**（#2/#3/#4 属通过，#1 属移交但有明确交接点），1 项**不适用**（#5，理由充分）。清场项（#6）**移交**，零删除已达成。s6 可标记完成。
