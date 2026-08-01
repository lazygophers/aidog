# perf-final-verification s6 — 未达标项登记 + 清场

Parent: [深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降](map.md)
承接 `s5-spec-writeup`（`.skein/spec/recall/optimization/perf-200mb-final-verification.md`）与
`s3-budget-reconcile.md`（`.scratch/perf-200mb/s3-budget-reconcile.md`）的 4 条超预算归因。
**本票不重新量测、不改任何生产代码、不给新建 task 写 PRD/subtask/confirm。**

---

## 一、未达标可优化项 — 逐条建 task

判定规则（团队指令）：只给判为「可优化」或「未定论/未验证但值得查」的条目建 task；判为「物理下限」的不建。

| # | 超预算项 | 判定 | 处置 |
|---|---|---|---|
| 1 | aidog 主进程(Rust) idle 23MB→S3负载30MB, 超 +12~20MB | **未定论**（MALLOC_SMALL 基线是否含未处置的 tokenizer 四单例常驻，未验证） | **建 task** `rust-main-heap-baseline-trim` |
| 2 | WebContent#1 非graphics 29.0→36.9MB(S1→S3), 超 +4.9MB | **未验证**（超支集中负载态 WebKit malloc，指向每连接内存构成，呼应遗留 issue 05-per-connection-cost，open 未产出结论） | **建 task** `webcontent-loadpath-malloc-trim` |
| 3 | GPU 进程 S2(隐藏) 37.9MB vs 18MB上限, 超 +19.9MB | **推测未验证**（WKWebView/CoreAnimation 隐藏后保留合成快照的假说，缺 profiler 抓栈/循环累积/长隐藏时长三项验证深度；S2 是三场景中富余最薄的 1.75%，此项异常若加剧有击穿200MB风险） | **建 task** `gpu-hidden-compositing-buffer` |
| 4 | Networking S3 9.5MB vs 6.7MB上限, 超 +2.8MB | **部分物理下限**（+2.8MB/50连接≈56KB/连接，量级不离谱，与 1-3 条"未定论/未验证"性质不同，未达到"值得查"的门槛） | **不建 task** |

三条新 task 均已 `skein create` 登记（只 create，未写 PRD/subtask/confirm，归各票自己的 plan 阶段）：

- `rust-main-heap-baseline-trim`（`.skein/task/rust-main-heap-baseline-trim/`）
- `webcontent-loadpath-malloc-trim`（`.skein/task/webcontent-loadpath-malloc-trim/`）
- `gpu-hidden-compositing-buffer`（`.skein/task/gpu-hidden-compositing-buffer/`）

三条 desc 内均已写明：**内存判决已 PASS，这些是「达标之后的余量优化」，非阻塞紧急项**（免得未来的人误当 bug 处理），并各自附对应超预算项 / 预估收益 MB / 判为可优化的具体理由 / 入口证据文件路径。

第 4 条（Networking）**零条**：理由已如实记录在上表——不是"没查"，是团队指令的判定规则（可优化/未定论/未验证 才建）在这条上不成立，其"部分物理下限"结论已在 s3-budget-reconcile.md 给出量级依据（56KB/连接不离谱），未达到"值得查"门槛，故不重复建票。

---

## 二、清场

### 2.1 `.scratch/perf-200mb/assets/results/` 处置清单

删前先列过清单（见对话内 `ls -la` 记录），处置依据：
- **perf-final-verification 自己的产物**：s1-preflight 的冒烟/探路数据已被 s2 的正式量测取代，且 s1-preflight 文档（`final-verification-preflight.md` 第174行）已明确「未清理...留给下游 s6」——本票据此执行删除。
- **兄弟 task 的产物**：`window-default-size` 的 `s6-verify.md`（`.skein/task/window-default-size/s6-verify.md` 第32-40行）已逐条判定「可删/须留」，把删除动作明确移交本票执行——本票只执行其已做出的判断，不重新判定。
- **不属于本票范围的产物**（`proxy-hotpath-buffers`，s7 命名）：`window-default-size` 自己也未越权判定，本票同样不越权——且核实到 `cpu-s7-after-run3.txt`/`run3b.txt` 被活跃 spec `.skein/spec/recall/optimization/measure-window-multi-probe.md` 引用为证据，**必须保留**。

#### 已删除（共 68 个文件/目录）

**perf-final-verification 自己的冒烟/探路数据（56 个，Aug 1，全部已被 s2/s4 正式数据取代）**：
- `footprint-scenario{1,2,3}-{fg,hidden,load}-30166/30179/30180/30181/30199-*.txt`（15 个，s1-preflight 冒烟第11节 pid 30166 系）
- `footprint-smoke-fg-15880/15927/15930/15931/15939-*.txt`（5 个，s1-preflight 第7节场景1冒烟）
- `footprint-smoke-idle-fg-13440/13447/13448/13449/13453-*.txt`（5 个，exec-pfv-s1 并发冲突期间产物）
- `footprint-smoke-idle-fg-release-19328/19335/19336/19337/19340-*.txt`（5 个，同上）
- `footprint-smoke-idle-hidden-13440/13447/13448/13449/13453-*.txt`（5 个，同上，与 fg 版重复 pid 集，二次落盘）
- `cpu-scenario{1,2,3}-{fg,hidden,load}.txt`（3 个）+ `cpu-smoke-{fg,hidden,idle-fg,idle-fg-release,idle-hidden,idle-hidden-release}.txt`（6 个）
- `mem-scenario{1,2,3}-{fg,hidden,load}.txt`（3 个）+ `mem-smoke-{fg,hidden,idle-fg,idle-fg-release,idle-hidden,idle-hidden-release,idle-hidden-release2}.txt`（7 个）
- `iso-app-stdout.aidog-pfv-final-30097.log`（s1-preflight 冒烟日志）
- `iso-app-stdout.log`（共享固定路径的过期通用日志，s1-preflight 文档已指出其"多进程互相覆盖竞争"的问题根源，无独立信息价值）

**window-default-size 已判定"无下游引用"的私有数据（10 个，Jul 28，已提炼进 `curve-result.md`）**：
- `cpu-idle-{foreground,hidden,small-window,visible-again}.txt`（4 个）
- `mem-idle-10min.txt` / `mem-idle-hidden.txt` / `mem-cold-start.txt` / `track-idle-10min.txt`（4 个）
- `mem-rel-P1-large.txt` / `mem-rel-P2-1150x750.txt`（2 个，4 档曲线里 2 档的逐次采样，表格已提炼）

**临时 pid 追踪文件（2 个，assets/ 根目录，非 results/）**：
- `assets/.pids`（30 字节）、`assets/.pids.aidog-pfv-final-30097`（同类，无信息价值）

**`/tmp` 残留隔离目录/文件（Aug 1 s4 遗留，s4 文档声称已清但实际残留）**：
- `/tmp/aidog-pfv-s4-baseline-home`、`/tmp/aidog-pfv-s4-coldstart-95007`、`/tmp/aidog-pfv-s4-current-45935`（ISO_HOME 目录）
- `/tmp/aidog-pfv-s4-*.log` / `*.txt` / `*.sh` / `*.sh.bak`（s4 baseline/tokentest/loadgen 的临时脚本与探针输出，共 10 个）
- `/tmp/aidog-test-s6`
- 删前核实：当前唯一活跃 `aidog` 进程（pid 64332, HOME=/Users/luoxin）是用户真实实例，与以上任何隔离目录无关，不受影响

#### 保留（须留，附理由）

**perf-final-verification 自己的最终指标集**（s2-measure-3scenes.md / s3-budget-reconcile.md / s4-redline-regression.md / 最终 spec 直接引用的原始数据）：
- `footprint-s2-scenario{1,2,3}-*.txt`（15 个）+ `footprint-s2-maximized-*.txt`（5 个）
- `mem-s2-scenario{1,2,3}-*.txt` + `mem-s2-maximized.txt`（4 个）
- `cpu-s2-scenario{1,2,3}-*.txt`（3 个）
- `iso-app-stdout.aidog-pfv-s2-{scenario1,scenario2,scenario3,maximized,maxctrl}-*.log`（5 个）
- `s4-coldstart-trials.txt` / `s4-ttft-latency.txt` / `s4-tokentest-baseline.txt` / `s4-tokentest-current.txt`（红线1/2/4 原始数据）
- `iso-app-stdout.aidog-pfv-s4-{current,ui}-*.log`（3 个）

**兄弟 task / 独立 spec 引用的数据（不属本票范围，未越权处置）**：
- `size-curve-raw.txt` —— `window-default-size/curve-result.md:15` 直接引用为「原始盘」
- `cpu-load-50.txt` / `cpu-load-50-steady.txt` / `mem-load-50.txt` / `mem-load-50-late.txt` / `cpu-s7-after-run{1,2,3,3b}.txt` / `mem-s7-after-run{1,2}.txt` / `s7-regime-heap-crosscheck.txt` —— 疑似 `proxy-hotpath-buffers`（s7 subtask 编号）产物，`window-default-size` 已判定"非其私有、建议移交该 task 自行判定"；本票核实其中 `cpu-s7-after-run3.txt`/`run3b.txt` 被活跃 spec `.skein/spec/recall/optimization/measure-window-multi-probe.md:26` 引用为证据，**保留待定，理由：出本票范围 + 被活跃 spec 引用，删除会破坏该 spec 的可追溯证据链**

**其余顶层文档/脚本（全部保留，均被协议文档或兄弟 task 引用）**：
- `assets/measure.sh` / `loadgen.sh` / `run-size-curve.sh` / `explain-baseline.sh` / `research-wkwebview-floor.md`
- `measure-protocol.md` / `window-size-measure-protocol.md` / `mock-loadgen-50x5min.md` / `map.md`
- `issues/01`–`10`（全部 10 个问题登记，被兄弟 task 引用）
- `final-verification-preflight.md` / `s2-measure-3scenes.md` / `s3-budget-reconcile.md` / `s4-redline-regression.md`（本票自身 s1-s4 的正式产出文档，非"临时脚本/原始采样"，是最终判决的组成部分，不属于清场对象）

### 2.2 清场结果

`results/` 清理后剩余 51 个文件（原 119 个），全部落在「本票最终指标集」或「兄弟 task 引用中的数据」两类，无遗留临时/冒烟/探路文件。

---

## 三、map.md fog 关闭记录

核对 `.scratch/perf-200mb/map.md` 的「Not yet specified」全部 7 条现存 fog 条目（常驻动画清点 / transform-rotate流光边框视觉等价性 / log.db体积治理 / SQLite侧占用调参 / 前端具体改法 / 异步化边界 / est_cost计算成本），**均非本票范围内可回答的问题**（分属其他已收敛或待后续开票的方向，本票只做 200MB 收口判决与登记清场）。

**本票零关闭 fog 条目** —— 显式记录，非默默跳过。本票新发现的 3 条余量优化点（对应 s3 归因的项1/2/3）不是既有 fog 表的延续，已通过第一节的 3 个新 task 独立登记，未强行塞进 map.md 的 fog 列表（map.md 本身已声明「本图已收敛→已转9个task，本图不再产出新决策」，不适合再挂新 fog 行）。

---

## 验收自查

- [x] 未达标可优化项已逐条建 task：3 条（`rust-main-heap-baseline-trim` / `webcontent-loadpath-malloc-trim` / `gpu-hidden-compositing-buffer`），第4条 Networking 显式记录零建票理由
- [x] 临时脚本与原始采样已删：68 个文件/目录（results/ 56+10 + assets/根 2）+ `/tmp` 残留 14 个路径，逐条列出删除清单与理由
- [x] `results/` 仅剩最终指标集与 spec：清理后 51 个文件，全部为 perf-final-verification 自身最终数据集或兄弟 task 引用中的数据，逐条附保留理由
