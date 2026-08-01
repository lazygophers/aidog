---
title: perf-200mb-final-verification
layer: recall
created: 1785573130
category: optimization
keywords: [memory,200mb,budget,verification,redline,cpu,footprint,peak-hours-unrelated]
status: active
inclusion: auto
anchors: .scratch/perf-200mb/s2-measure-3scenes.md,.scratch/perf-200mb/s3-budget-reconcile.md,.scratch/perf-200mb/s4-redline-regression.md,.scratch/perf-200mb/final-verification-preflight.md
---

## 深度性能优化 `perf-final-verification` 最终判决（8 个前置 task 合计效果）

本票是 `深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降`（map: `.scratch/perf-200mb/map.md`）
的收官判决，汇总 s1-preflight / s2-measure-3scenes / s3-budget-reconcile / s4-redline-regression 四份
量测与对账文档的结论。**本票不重新量测，只做最终判决 + 归因整理**，原始数据全部见上述文件与
`.scratch/perf-200mb/assets/results/` 下 `s2-*`/`s4-*` 采样。

窗口面积↔内存关系的独立事实见 [[window-size-memory-relation]]（本票不重复造第二份真值，仅在
「窗口物理事实」一节引用并标注本轮验证升级）。

---

## 一、达标判决

**内存判决：PASS —— 但是「拆东墙补西墙」式达标，不是逐项达标。**

- 判据场景 = **S3（50 路并发负载态，默认窗口尺寸 1026×759）**。理由：[03] target-feasibility-ruling
  对 200MB 目标的原始定义逐字是「全进程总和在**持续转发峰值（约 50 路并发）**下 ≤200MB」，S3 是唯一
  在字面上对应该定义的场景。
- 数字：TOTAL **184.5MB ≤ 200MB**，富余 15.5MB（7.75%）。
- **实质**：逐项预算表并非全部达标——非合成面小计 135.65MB 超 129MB 上限 **+6.65MB**；总账能 PASS
  纯粹是因为合成面（窗口 surface）实测 48.9MB，比 71MB 预算低了 22.1MB，靠这部分富余把非合成面的
  超支抵消掉。**这是"拆东墙补西墙"式的达标，禁止在任何转述里简化成"基本达标"或"全面达标"。**
  若后续改动（如流光边框等新合成层）让合成面占用回升，总账有重新击穿 200MB 的风险。

**必须一并声明的限定条件**：

1. **此 PASS 仅在默认窗口尺寸 1026×759 下成立**。最大化窗口（2304×1265）TOTAL 296.7MB，超预算
   96.7MB——是 [03] 已拍板接受的物理代价（合成面∝窗口面积，用户主动放大窗口的必然结果），不是本票
   需要重判的问题，但判决必须声明此前提。
2. **最薄弱场景不是 S3，是 S2（空闲隐藏）**：S2 TOTAL 196.5MB，富余仅 **3.5MB（1.75%）**，比判据场景
   S3 的 7.75% 薄得多。三场景内存判据摘录：

   | 场景 | TOTAL(MB) | 富余(MB) | 富余占比 |
   |---|---|---|---|
   | S1 空闲前台 | 136.7 | 63.3 | 31.65% |
   | **S2 空闲隐藏** | **196.5** | **3.5** | **1.75%（最薄）** |
   | S3 并发负载（判据） | 184.5 | 15.5 | 7.75% |

   陈述**风险**时必须以 S2（最差场景）为准，不能因为判决用 S3 就把 S2 的薄弱一笔带过。S2 的富余里
   还包含一个未验证归因深度的 GPU 图形合成异常（见下「未达标归因」第 3 条），若该异常在其他运行
   环境或更长隐藏时长下加剧，S2 有击穿 200MB 的风险。

**CPU 判决**：空闲态（S1/S2）实测 **0.0%**，远优于 [03] 的 <0.5% 目标，达标且大幅富余。负载态
（S3）实测 **76.0%**，[03] 对负载态只定性要求「三场景 CPU 下降」未给量化上限，**本票不代为判定
PASS/FAIL，如实陈述实测值**（CPU 归因见下「量测协议」节）。

---

## 二、未达标归因（4 条超预算项逐条）

以下对账口径：内存判据 = `footprint -p <pid>` 的 `phys_footprint`；graphics 判据 = footprint 输出中
带 `(graphics)` 后缀的行之和，其余归「非合成面」。三场景均默认窗口尺寸 1026×759、独立重启、ISO_HOME
隔离、稳态 ≥600s、`feature/next` 分支。

### 1. aidog 主进程（Rust）：42.0→50.0MB（S1→S3）vs 30MB 上限，超 **+12~20MB**

- **分类**：malloc heap（Rust 运行时基线堆），非合成面。
- SQLite page cache 三场景均实测 0B，证实 `READ_CACHE_DEFAULT_KB=64`（前置 task）修复在此构建生效，
  SQLite 不是超支来源。
- 剩余主要类目是 `MALLOC_SMALL`：S1 idle 23MB → S3 负载 30MB，仅 +7MB。tokenizer 四单例（状态仍
  `open`，未处置）理论常驻上限 25.7MB（GLM4/QWEN2 两个 tokenizer json，不含解析后结构与两个 tiktoken
  单例），若真触发理应看到 idle→load 更大跳变，但实测只 +7MB——**推测**（未验证）mock 协议路径未
  触发本地 tokenizer 加载，可能走上游 usage 或未命中对应协议分支。
- **物理下限 vs 可优化：未定论**。即便后续落地「tokenizer 按需释放」，idle 态 23MB 的 `MALLOC_SMALL`
  基线本身已吃掉 30MB 预算的 77%，说明 30MB 上限对当前 Rust 运行时基线余量本就很紧，后续任何 tokenizer
  方案选型需重新核算这个更紧的余量。

### 2. WebContent#1 非 graphics 部分：29.0→36.9MB（S1→S3）vs 32MB 上限，超 **+4.9MB**

- **分类**：malloc heap（渲染进程侧的流式转发缓冲），非合成面。
- `WebKit malloc` 20→19→25MB、`MALLOC_SMALL` 4.3→4.4→5.3MB（S1/S2/S3），增量集中在负载态（S3），
  idle 两场景稳定在 29MB 左右、不超预算。
- **物理下限 vs 可优化：未验证**。超支只在负载态出现、且集中在 WebKit malloc 类目，指向「50 路并发
  下每连接内存构成」的调查范围（本票不越权重复验证，只确认这一事实指向）。

### 3. GPU 进程：S2（隐藏）37.9MB vs 18MB 上限（非合成面口径），超 **+19.9MB** —— 反直觉观察的直接成因

- **分类**：graphics/合成缓冲，非 malloc heap。
- 数据对比（`footprint-s2-scenario1-fg-*-GPU.txt` vs `footprint-s2-scenario2-hidden-*-GPU.txt`）：
  - `CoreAnimation`：S1 不存在此类目，S2 新增 **23MB (Dirty)**。
  - `IOSurface`（Reclaimable）：S1 18MB → S2 **42MB**（+24MB）。
  - `MALLOC_SMALL`：S1 5.4MB → S2 8.7MB，涨幅远小于上述两项，排除「通用堆增长」假说。
  - 同步看 WebContent#1 graphics 类目：S1 26MB → S2 **55.1MB**（+29.1MB），方向与 GPU 完全一致。
- **物理下限 vs 可优化：未验证**。**推测**为 WKWebView/CoreAnimation 在窗口隐藏后保留合成快照/离屏
  缓冲（用于快速恢复显示的平台行为），而非代码泄漏或本仓 CSS 问题——依据是新增内存精确集中在
  `CoreAnimation`/`IOSurface`/`Owned physical footprint (unmapped) (graphics)` 三个图形专属类目，
  跨 GPU 与 WebContent 两进程同步出现，通用 `MALLOC_*` 类目基本持平。**排查到此为止，未做**：
  Instruments Core Animation profiler 抓栈确认触发点、多次隐藏/显示循环是否累积、更长隐藏时长是否
  继续增长——这三项是未完成的验证深度，**照实力度陈述，禁升格成结论**。
- 与 s1-preflight 冒烟结论（隐藏 157.7MB < 前台 162.8MB，方向相反）的矛盾**未解释**——两次测量的窗口
  尺寸/构建版本/稳态时长是否严格一致未核实，s1-preflight 原始文件未在 `assets/results/` 留档比对，
  不具备核实条件。

### 4. Networking：S3 9.5MB vs 6.7MB 上限，超 **+2.8MB**

- **分类**：网络连接的物理成本（每连接 socket/TLS 会话缓冲），非 malloc heap 异常、非 graphics。
- S1/S2 均落在预算内（6.7/6.5MB），仅 S3（50 路并发流式连接）超支。
- **物理下限 vs 可优化：部分物理下限**——+2.8MB / 50 连接 ≈ 56KB/连接，量级不算离谱；是否可通过收缩
  `reqwest` 连接池或调小 buffer 进一步压缩，未有结论。

### 「其余零散」预算行说明

[03] 原表第 6 行「其余零散 20MB」是当时基线口径下的算术残差，不是独立采样类目。s2 五进程模型下，
三场景 TOTAL 均能被「主进程+WebContent#1+WebContent#2(popover)+GPU+Networking」五项精确加总（无残差），
说明当前进程编制下不存在对应「其余零散」的可采样类目——**不是取不到数据，是 [03] 原表分类方式在当前
进程编制下已不成立**，不应在后续引用里继续填这个数。

---

## 三、窗口物理事实（与 [[window-size-memory-relation]] 交叉引用，不重复造真值）

`window-size-memory-relation.md`（07-29 沉淀）已确立的结论：**窗口面积与内存无可信线性拟合式**（档间
噪声 ±95MB 远超面积效应），但**合成面（compositing surface）本身是窗口面积的函数**，用户拉大窗口
导致内存上涨是 WKWebView 的**物理成本，非缺陷**，代码规避不了。

**本轮验证升级**：该文档写下这条结论时依据的是早期 dev 口径拟合外推，本轮 `s2-measure-3scenes.md`
的**最大化对照组**（独立重启、release 构建、背景态、≥600s 稳态、`2304×1265`）是该结论从「拟合外推」
变成「release 口径实测坐实」的证据：

- 主窗口 WebContent 从默认尺寸（1026×759，三场景 55~84MB）暴涨到最大化（2304×1265）的 **206MB**；
- 其中 **「Owned physical footprint (unmapped) (graphics)」单项占 147MB**（对照默认尺寸场景该项通常
  20~50MB 量级）；
- 面积扩大约 5.4 倍（778,734 → 2,914,560 px²），WebContent 内存增幅方向与面积增幅一致；
- TOTAL 296.7MB，超 200MB 预算 96.7MB —— **这是 [03] 已知且已接受的代价，不是本轮需要修的 bug**。

**结论没有变**（仍是「合成面是窗口面积的函数、代码规避不了」），**变的是证据强度**：从「早期 dev 口径
拟合式外推、且该拟合式本身已被证明不可信」升级为「本轮 release 口径独立实测直接坐实」。两份文档
不冲突：`window-size-memory-relation.md` 负责「无可信拟合式，禁外推」这条否定性结论 + 完整量测协议
教训；本票负责记录「本轮实测已把定性结论坐实」这一新证据，互相链接，不重复陈述对方已有内容。

---

## 四、量测协议与流形参数（可复现）

**APP 路径口径**：`src-tauri/target/release/bundle/macos/AiDog.app`（含全部 8 个前置 task 改动的
release 产物）；红线回归（s4）额外用 `/Applications/AiDog.app`（与前者字节数完全相同的同一产物，
由 s1-preflight 早前拷贝覆盖）。红线回归的 baseline 侧独立 `git worktree` 于 8-task 落地前的祖先
commit，独立 `cargo build --release` 产出对比二进制。

**ISO_HOME 隔离**：每场景/每次独立重启用随机目录 `/tmp/aidog-pfv-<subtask>-<scenario>-<pid>`（`open -a`
不传 env，需直接 fork/exec 注入 `HOME`）；采样完毕 `pkill -x aidog` + `rm -rf`，`pgrep -x aidog` 复核
为空。**量测设施全局单例**：同一时刻只允许一个 subtask 持有 `measure.sh`/`/Applications/AiDog.app`，
本轮 s1-preflight 曾三次误判并发冲突（后确认是同一个未死透的旧 executor 在跑），持有前必须先确认
`pgrep -x aidog` 与 `results/iso-app-stdout.log`（共享固定路径，非按 pid 隔离）无其他活跃占用者。

**mock 唯一**：一切测试/压测只允许用仓内 mock 协议的平台与分组（`Authorization: Bearer mock`），
禁用真实平台；种子写入需在 launch 建库后进行，若种子写入已运行较久的进程需 kill+relaunch 一次才
生效（同进程内不读新种子）。

**内存/CPU 判据**：内存 = `footprint -p <pid>` 的 `phys_footprint`（非 `ps rss`/`vmmap`）；CPU =
`ps -o time=` 区间差 / 墙钟（非 `ps %cpu`），均由 `measure.sh` 内建实现。graphics 判据 = footprint
输出中带 `(graphics)` 后缀的行之和（`Owned physical footprint (unmapped) (graphics)` +
`untagged (VM_ALLOCATE) (graphics)` + `IOAccelerator (graphics)`）。

**每场景独立重启 + ≥600s 稳态**：S1 611s / S2 606s / S3 617s（loadgen 持续时长），均满足 measure
protocol 规则「等满稳态 ≥10min」。最大化对照组背景态 613s。

**50 并发压测参数（定死，禁每次重新敲）**：`loadgen.sh 50 <duration>`，`chunk_count:200, delay_ms:50,
input_tokens:4000, output_tokens:2000, stream:true`，模型 `claude-sonnet-4-20250514`，端口用
`LOADGEN_PORT` 覆盖 app 实际监听端口（冲突时 app 自动 +1）。

**分支口径**：`feature/next`（非 `master`）——8 个前置 task 均已 finish 且在 `feature/next` 历史
祖先链中，但 `feature/next` 从未合并进 `master`（领先 259 commit / 落后 1 commit）。PRD 字面要求
「合入 master 后再量测」未满足，是已声明的已知偏差，非隐瞒，量测口径按 `feature/next` HEAD 执行。

---

## 五、CPU 归因

- **空闲态（S1/S2）**：TOTAL 0.0%（全进程 0.0%），远优于 <0.5% 目标。
- **50 路并发负载态（S3）**，20s 采样窗口，负载持续期间：

  | 进程 | %CPU |
  |---|---|
  | aidog(main) | 42.3 |
  | GPU | 1.3 |
  | Networking | 16.7 |
  | WebContent(主窗口) | 7.9 |
  | WebContent(popover预建) | 7.8 |
  | **TOTAL** | **76.0** |

  主进程（Rust 代理转发/路由/序列化）占大头 42.3%，Networking（50 路并发 TLS/socket）16.7%，两个
  WebContent 各 ~7.8~7.9%。[03] 未对负载态 CPU 给量化上限，只定性要求「三场景下降」，本票不据此
  判 PASS/FAIL，如实陈述实测分布。

---

## 六、红线 1-4 回归结论（摘录，全文见 `s4-redline-regression.md`）

| 红线 | 判定 | 摘要 |
|---|---|---|
| 1. TTFT/总延迟 | PASS | TTFT 中位数 -2.9%、总延迟中位数 -0.6%（均略快，无回归） |
| 2. token/est_cost 逐条一致 | **token PASS（6/6）；est_cost 4/6 不一致** | token 数完全一致无精度损失；est_cost 不一致已定位为独立定价功能 `model-price-time-tiers`（commit `0059f4e8` 引入的 `time_tiers` 价格分级）导致，与本轮 8 个性能优化 task 无重合，非性能回归。按「无性能回归」实质口径判 PASS，字面「逐条一致」不通过，两种口径的出入已显式区分，不藏进一个勾里 |
| 3. 18 页实渲染走查 | PASS | 用户实机走查整体口头确认无问题，缺陷清单为空。口径：非逐页独立书面签字，是用户整体口头确认——诚实区分证据链强度 |
| 4. 冷启动到首屏可交互 | PASS | 15 次独立冷启动合并中位数 1.003s vs 优化前基线 2.844s，降 64.7%（最保守批次口径降 46.5%），无回归 |

红线2 的 est_cost 不一致是否需要下游在同一价格基准下重测，交后续裁决，本票不越权判定。

---

## 七、历史基线可比性（07-29 378.7MB → 本轮 196.5MB）

对比对象选 **S2（空闲隐藏）196.5MB**（口径维度与 07-29 一致，均背景态；S1 前台不可比）。窗口尺寸
（均 1026×759）、状态（均背景态）、稳态时长（均 ≥600s）三项口径一致，构建版本不同（07-29 早于 8
个优化 task 完成前）——这是预期中的时间序列前后对比。

**降幅 182.2MB（378.7→196.5，-48.1%）是 8 个前置 task 的合计效果，无中间态数据，不可拆分到单个
task**，仅供整体进度参考，不构成本票判决依据（判决见「一、达标判决」）。

---

## 关联

[[window-size-memory-relation]]（窗口面积↔内存物理事实真值源，本票仅交叉引用不重复造）
